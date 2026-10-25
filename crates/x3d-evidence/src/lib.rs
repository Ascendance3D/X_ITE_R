//! Finds and verifies the gitignored X_ITE answer keys listed in `oracle/answer-keys.tsv` (issue #65).
//!
//! Search order, first hit wins (the same as `tools/answer_keys.py`):
//!
//! 1. `$X3D_ORACLE_KEYS/<name>`. When the variable is set, it is the only location searched.
//! 2. `<this worktree>/oracle/<name>`, when it is a real file or directory (not a symlink).
//! 3. `<main worktree>/oracle/<name>`, the parent of `git rev-parse --git-common-dir`.
//!
//! A key is accepted only when its kind, file count, byte count and sha256 equal the manifest row.
//! For a directory the sha256 is the tree digest: sha256 of the `sha256sum` lines of every file,
//! relative paths, sorted bytewise. The resolver only reads; it never writes a key.
//!
//! # Evidence policy (issue #66)
//!
//! A test that cannot find its evidence must fail or skip by name; it never passes in silence.
//! [`require_key`], [`key_or_tracked`], [`no_gpu`] and [`tool_absent`] own that decision:
//!
//! - `X3D_EVIDENCE=strict` (the default): every skip fails the test.
//! - `X3D_EVIDENCE=ci`: a skip passes only when `X3D_EVIDENCE_ALLOW` (comma list) names its reason
//!   (`no-gpu-adapter`, `msaa-unsupported`, `tool-absent`) or, for an absent key, the key name.
//!   Each permitted skip appends one line to `X3D_EVIDENCE_LOG` (default `target/evidence-skips.tsv`):
//!   test, reason, key, caller, detail. Cargo hides the stderr of a passing test, so the log is the record.
//! - A key that differs from its manifest row and an absent tracked fixture always fail.

use sha2::{Digest, Sha256};
use std::collections::HashMap;
use std::fmt;
use std::io::Read;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::{Mutex, OnceLock};

/// Kind, file count, byte count and sha256 of a key, as in one manifest row.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Measure {
    pub dir: bool,
    pub files: u64,
    pub bytes: u64,
    pub sha256: String,
}

impl fmt::Display for Measure {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        let kind = if self.dir { "dir" } else { "file" };

        write!(
            f,
            "{kind}, {} files, {} bytes, sha256 {}",
            self.files, self.bytes, self.sha256
        )
    }
}

#[derive(Debug)]
pub enum KeyError {
    /// The name has no row in the manifest.
    NotInManifest(String),
    /// No searched location holds the key.
    Absent {
        name: String,
        searched: Vec<PathBuf>,
    },
    /// A location holds the key, but it differs from the manifest.
    Mismatch {
        name: String,
        path: PathBuf,
        found: Box<Measure>,
        want: Box<Measure>,
    },
    Io(String),
}

impl fmt::Display for KeyError {
    fn fmt(&self, f: &mut fmt::Formatter) -> fmt::Result {
        match self {
            KeyError::NotInManifest(n) => write!(f, "{n}: not in oracle/answer-keys.tsv"),
            KeyError::Absent { name, searched } => {
                let list: Vec<String> = searched.iter().map(|p| p.display().to_string()).collect();

                write!(
                    f,
                    "{name}: absent; searched {}; restore it as oracle/README.md describes, or set X3D_ORACLE_KEYS",
                    list.join(", ")
                )
            }
            KeyError::Mismatch {
                name,
                path,
                found,
                want,
            } => write!(
                f,
                "{name}: mismatch at {}: found {found}; manifest wants {want}",
                path.display()
            ),
            KeyError::Io(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for KeyError {}

/// The checkout that holds this crate.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Reads `oracle/answer-keys.tsv` under `root`.
pub fn manifest(root: &Path) -> Result<HashMap<String, Measure>, KeyError> {
    let path = root.join("oracle/answer-keys.tsv");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| KeyError::Io(format!("{}: {e}", path.display())))?;
    let mut rows = HashMap::new();

    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let c: Vec<&str> = line.split('\t').collect();
        let bad = || KeyError::Io(format!("{}: bad row {line:?}", path.display()));

        if c.len() != 5 || !(c[1] == "file" || c[1] == "dir") {
            return Err(bad());
        }

        rows.insert(
            c[0].to_string(),
            Measure {
                dir: c[1] == "dir",
                files: c[2].parse().map_err(|_| bad())?,
                bytes: c[3].parse().map_err(|_| bad())?,
                sha256: c[4].to_string(),
            },
        );
    }

    Ok(rows)
}

fn hex(d: impl AsRef<[u8]>) -> String {
    d.as_ref().iter().map(|b| format!("{b:02x}")).collect()
}

fn file_sha256(path: &Path) -> std::io::Result<String> {
    let mut f = std::fs::File::open(path)?;
    let mut h = Sha256::new();
    let mut buf = vec![0; 1 << 20];

    loop {
        let n = f.read(&mut buf)?;

        if n == 0 {
            return Ok(hex(h.finalize()));
        }

        h.update(&buf[..n]);
    }
}

fn walk(dir: &Path, rel: &Path, out: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for e in std::fs::read_dir(dir)? {
        let e = e?;
        let r = rel.join(e.file_name());

        let ty = e.file_type()?;

        if ty.is_symlink() {
            return Err(std::io::Error::other(format!(
                "{}: a symlink inside a key directory is refused",
                e.path().display()
            )));
        }

        if ty.is_dir() {
            walk(&e.path(), &r, out)?;
        } else {
            out.push(r);
        }
    }

    Ok(())
}

/// Measures a file or a directory tree.
pub fn measure(path: &Path) -> std::io::Result<Measure> {
    let meta = std::fs::metadata(path)?;

    if !meta.is_dir() {
        return Ok(Measure {
            dir: false,
            files: 1,
            bytes: meta.len(),
            sha256: file_sha256(path)?,
        });
    }

    let mut rels = vec![];

    walk(path, Path::new(""), &mut rels)?;
    rels.sort_by(|a, b| {
        a.as_os_str()
            .as_encoded_bytes()
            .cmp(b.as_os_str().as_encoded_bytes())
    });

    let (mut listing, mut bytes) = (Sha256::new(), 0);

    for r in &rels {
        let p = path.join(r);

        bytes += std::fs::metadata(&p)?.len();
        listing.update(format!("{}  {}\n", file_sha256(&p)?, r.display()));
    }

    Ok(Measure {
        dir: true,
        files: rels.len() as u64,
        bytes,
        sha256: hex(listing.finalize()),
    })
}

/// The main worktree of the repository that holds `root`, when `root` is in a Git checkout.
pub fn main_worktree(root: &Path) -> Option<PathBuf> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(["rev-parse", "--path-format=absolute", "--git-common-dir"])
        .output()
        .ok()?;

    if !out.status.success() {
        return None;
    }

    let common = PathBuf::from(String::from_utf8(out.stdout).ok()?.trim());

    (common.file_name()? == ".git").then(|| common.parent().unwrap().to_path_buf())
}

/// The locations searched for `name`, in order.
pub fn candidates(root: &Path, name: &str, explicit: Option<&Path>) -> Vec<PathBuf> {
    if let Some(dir) = explicit {
        return vec![dir.join(name)];
    }

    let mut out = vec![root.join("oracle").join(name)];

    if let Some(main) = main_worktree(root) {
        let same = match (main.canonicalize(), root.canonicalize()) {
            (Ok(a), Ok(b)) => a == b,
            _ => false,
        };

        if !same {
            out.push(main.join("oracle").join(name));
        }
    }

    out
}

/// Resolves and verifies `name` for the checkout at `root`. `explicit` is the `X3D_ORACLE_KEYS` value.
pub fn resolve_in(root: &Path, name: &str, explicit: Option<&Path>) -> Result<PathBuf, KeyError> {
    let rows = manifest(root)?;
    let want = rows
        .get(name)
        .ok_or_else(|| KeyError::NotInManifest(name.to_string()))?;
    let searched = candidates(root, name, explicit);

    for (i, p) in searched.iter().enumerate() {
        // Step 2 skips a symlink in this worktree; an explicit location or the main worktree may be a link.
        let own_link = i == 0 && explicit.is_none() && p.is_symlink();

        if !p.exists() || own_link {
            continue;
        }

        let found = match measure(p) {
            Ok(m) => m,
            Err(e) => {
                return Err(KeyError::Io(format!(
                    "{name}: mismatch at {}: {e}",
                    p.display()
                )));
            }
        };

        if &found != want {
            return Err(KeyError::Mismatch {
                name: name.to_string(),
                path: p.clone(),
                found: Box::new(found),
                want: Box::new(want.clone()),
            });
        }

        return Ok(std::path::absolute(p).unwrap_or_else(|_| p.clone()));
    }

    Err(KeyError::Absent {
        name: name.to_string(),
        searched,
    })
}

/// Resolves and verifies `name` for this workspace, with `$X3D_ORACLE_KEYS`. A verified path is
/// cached for the rest of the process, so a key is hashed once per test binary.
pub fn resolve(name: &str) -> Result<PathBuf, KeyError> {
    static CACHE: OnceLock<Mutex<HashMap<String, PathBuf>>> = OnceLock::new();
    let cache = CACHE.get_or_init(Default::default);

    if let Some(p) = cache.lock().unwrap().get(name) {
        return Ok(p.clone());
    }

    let explicit = std::env::var_os("X3D_ORACLE_KEYS")
        .filter(|v| !v.is_empty())
        .map(PathBuf::from);
    let p = resolve_in(&workspace_root(), name, explicit.as_deref())?;

    cache.lock().unwrap().insert(name.to_string(), p.clone());

    Ok(p)
}

/// How a run treats missing evidence.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Mode {
    Strict,
    Ci,
}

/// The policy from `X3D_EVIDENCE` and `X3D_EVIDENCE_ALLOW`.
#[derive(Clone, Debug)]
pub struct Policy {
    pub mode: Mode,
    pub allow: Vec<String>,
    pub log: PathBuf,
}

impl Policy {
    pub fn from_env() -> Policy {
        let mode = match std::env::var("X3D_EVIDENCE").as_deref() {
            Err(_) | Ok("") | Ok("strict") => Mode::Strict,
            Ok("ci") => Mode::Ci,
            Ok(v) => panic!("X3D_EVIDENCE={v:?}: use strict or ci"),
        };
        let allow = std::env::var("X3D_EVIDENCE_ALLOW")
            .unwrap_or_default()
            .split(',')
            .map(|s| s.trim().to_string())
            .filter(|s| !s.is_empty())
            .collect();
        let log = std::env::var_os("X3D_EVIDENCE_LOG")
            .filter(|v| !v.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(|| workspace_root().join("target/evidence-skips.tsv"));

        Policy { mode, allow, log }
    }

    /// Whether a ci run may skip for `reason` (and `key`, for `key-absent`). A mismatch never may.
    pub fn allows(&self, reason: &str, key: Option<&str>) -> bool {
        let named = |s: &str| self.allow.iter().any(|a| a == s);

        self.mode == Mode::Ci
            && match (reason, key) {
                ("key-mismatch", _) => false,
                ("key-absent", Some(k)) => named(k),
                ("key-absent", None) => false,
                (r, _) => named(r),
            }
    }
}

fn clean(s: &str) -> String {
    s.replace(['\t', '\n', '\r'], " ")
}

/// Fails the test, or (ci mode, allowed) records a named skip and returns. The caller then returns.
#[track_caller]
pub fn skip(reason: &str, key: Option<&str>, detail: impl fmt::Display) {
    let policy = Policy::from_env();
    let at = std::panic::Location::caller();
    let thread = std::thread::current();
    let test = thread.name().unwrap_or("-");
    let what = match key {
        Some(k) => format!("{reason} ({k})"),
        None => reason.to_string(),
    };

    if !policy.allows(reason, key) {
        let how = match policy.mode {
            Mode::Strict => "X3D_EVIDENCE is strict".to_string(),
            Mode::Ci => format!("not in X3D_EVIDENCE_ALLOW={:?}", policy.allow.join(",")),
        };

        panic!("evidence {what}: {detail} [{how}]");
    }

    let line = format!(
        "{}\t{reason}\t{}\t{}:{}\t{}\n",
        clean(test),
        key.unwrap_or("-"),
        at.file(),
        at.line(),
        clean(&detail.to_string())
    );

    if let Some(dir) = policy.log.parent() {
        let _ = std::fs::create_dir_all(dir);
    }

    let written = std::fs::OpenOptions::new()
        .create(true)
        .append(true)
        .open(&policy.log)
        .and_then(|mut f| std::io::Write::write_all(&mut f, line.as_bytes()));

    if let Err(e) = written {
        panic!(
            "evidence {what}: cannot write {}: {e}",
            policy.log.display()
        );
    }

    eprintln!("evidence skip {what}: {detail}");
}

/// The verified path of a gitignored key, or `None` after a permitted skip. An absent key fails in
/// strict mode; a mismatched key always fails.
#[track_caller]
pub fn require_key(name: &str) -> Option<PathBuf> {
    match resolve(name) {
        Ok(p) => Some(p),
        Err(e @ KeyError::Absent { .. }) => {
            skip("key-absent", Some(name), e);
            None
        }
        Err(e) => panic!("evidence: {e}"),
    }
}

/// [`require_key`] for a name in `oracle/answer-keys.tsv`; otherwise the tracked file `tracked`,
/// which must exist (an absent tracked fixture is a broken checkout).
#[track_caller]
pub fn key_or_tracked(name: &str, tracked: &Path) -> Option<PathBuf> {
    match manifest(&workspace_root()) {
        Ok(rows) if rows.contains_key(name) => require_key(name),
        Ok(_) if tracked.exists() => Some(tracked.to_path_buf()),
        Ok(_) => panic!("evidence: tracked fixture {} is absent", tracked.display()),
        Err(e) => panic!("evidence: {e}"),
    }
}

/// No wgpu adapter: fails in strict mode, or records a `no-gpu-adapter` skip.
#[track_caller]
pub fn no_gpu(detail: impl fmt::Display) {
    skip("no-gpu-adapter", None, detail)
}

/// A required program is absent: fails in strict mode, or records a `tool-absent` skip.
#[track_caller]
pub fn tool_absent(detail: impl fmt::Display) {
    skip("tool-absent", None, detail)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(tag: &str) -> PathBuf {
        let d = std::env::temp_dir().join(format!("x3d-evidence-{tag}-{}", std::process::id()));

        let _ = std::fs::remove_dir_all(&d);
        std::fs::create_dir_all(d.join("oracle")).unwrap();
        d
    }

    fn sha(bytes: &[u8]) -> String {
        hex(Sha256::digest(bytes))
    }

    /// A checkout with one file key `k.json` = "abc" and one dir key `d` = {a: "1", b/c: "2"}.
    fn checkout(tag: &str) -> PathBuf {
        let root = scratch(tag);
        let listing = format!("{}  a\n{}  b/c\n", sha(b"1"), sha(b"2"));

        std::fs::write(
            root.join("oracle/answer-keys.tsv"),
            format!(
                "# name\tkind\tfiles\tbytes\tsha256\nk.json\tfile\t1\t3\t{}\nd\tdir\t2\t2\t{}\n",
                sha(b"abc"),
                sha(listing.as_bytes())
            ),
        )
        .unwrap();
        root
    }

    fn put_keys(dir: &Path) {
        std::fs::create_dir_all(dir.join("d/b")).unwrap();
        std::fs::write(dir.join("k.json"), "abc").unwrap();
        std::fs::write(dir.join("d/a"), "1").unwrap();
        std::fs::write(dir.join("d/b/c"), "2").unwrap();
    }

    fn policy(mode: Mode, allow: &str) -> Policy {
        Policy {
            mode,
            allow: allow.split(',').map(str::to_string).collect(),
            log: PathBuf::from("/dev/null"),
        }
    }

    #[test]
    fn strict_allows_no_skip() {
        let p = policy(Mode::Strict, "no-gpu-adapter,x_ite-text.json");

        assert!(!p.allows("no-gpu-adapter", None));
        assert!(!p.allows("key-absent", Some("x_ite-text.json")));
    }

    #[test]
    fn ci_allows_only_named_skips() {
        let p = policy(Mode::Ci, "no-gpu-adapter,x_ite-text.json,key-absent");

        assert!(p.allows("no-gpu-adapter", None));
        assert!(!p.allows("tool-absent", None));
        assert!(p.allows("key-absent", Some("x_ite-text.json")));
        // A bare "key-absent" entry does not allow every key; each key is named.
        assert!(!p.allows("key-absent", Some("items-baseline.json")));
        assert!(!p.allows("key-mismatch", Some("x_ite-text.json")));
    }

    /// The old silent forms (`eprintln!` and return) must not come back in a workspace test.
    #[test]
    fn no_silent_skip_remains_in_workspace_tests() {
        let crates = workspace_root().join("crates");
        let pattern = [
            "eprintln!(\"no adapter",
            "eprintln!(\"skipped: no GPU",
            "eprintln!(\"no pointing GPU",
            "eprintln!(\"xvfb-run not found",
            "eprintln!(\"no GL device",
            "x_ite-text.json missing",
        ];
        let mut found = vec![];
        let mut files = vec![];

        for c in std::fs::read_dir(&crates).unwrap() {
            let tests = c.unwrap().path().join("tests");

            if tests.is_dir() {
                walk(&tests, Path::new(""), &mut files).unwrap();
                files.iter_mut().for_each(|f| *f = tests.join(&*f));
                for f in files
                    .drain(..)
                    .filter(|f| f.extension().is_some_and(|e| e == "rs"))
                {
                    let text = std::fs::read_to_string(&f).unwrap();

                    for (n, line) in text.lines().enumerate() {
                        if pattern.iter().any(|p| line.contains(p)) {
                            found.push(format!("{}:{}", f.display(), n + 1));
                        }
                    }
                }
            }
        }

        assert!(found.is_empty(), "silent skips: {found:#?}");
    }

    #[test]
    fn explicit_location_finds_both_kinds() {
        let root = checkout("explicit");
        let keys = scratch("explicit-keys");

        put_keys(&keys);

        assert_eq!(
            resolve_in(&root, "k.json", Some(&keys)).unwrap(),
            keys.join("k.json")
        );
        assert_eq!(resolve_in(&root, "d", Some(&keys)).unwrap(), keys.join("d"));
    }

    #[test]
    fn explicit_location_is_the_only_one_searched() {
        let root = checkout("explicit-only");
        let keys = scratch("explicit-only-keys");

        put_keys(&root.join("oracle"));

        match resolve_in(&root, "k.json", Some(&keys)) {
            Err(KeyError::Absent { searched, .. }) => {
                assert_eq!(searched, vec![keys.join("k.json")])
            }
            r => panic!("{r:?}"),
        }
    }

    #[test]
    fn real_key_in_this_worktree() {
        let root = checkout("real");

        put_keys(&root.join("oracle"));

        assert_eq!(resolve_in(&root, "d", None).unwrap(), root.join("oracle/d"));
    }

    #[test]
    fn symlink_in_this_worktree_is_not_trusted() {
        let root = checkout("link");
        let keys = scratch("link-keys");

        put_keys(&keys);
        std::os::unix::fs::symlink(keys.join("k.json"), root.join("oracle/k.json")).unwrap();

        assert!(matches!(
            resolve_in(&root, "k.json", None),
            Err(KeyError::Absent { .. })
        ));
    }

    #[test]
    fn absent_names_every_searched_path() {
        let root = checkout("absent");
        let e = resolve_in(&root, "k.json", None).unwrap_err();

        assert!(
            e.to_string()
                .contains(&root.join("oracle/k.json").display().to_string())
        );
        assert!(matches!(e, KeyError::Absent { .. }));
    }

    #[test]
    fn one_changed_byte_is_a_mismatch() {
        let root = checkout("mismatch");
        let keys = scratch("mismatch-keys");

        put_keys(&keys);
        std::fs::write(keys.join("d/b/c"), "3").unwrap();

        assert!(matches!(
            resolve_in(&root, "d", Some(&keys)),
            Err(KeyError::Mismatch { .. })
        ));
    }

    #[test]
    fn extra_file_in_a_dir_key_is_a_mismatch() {
        let root = checkout("extra");
        let keys = scratch("extra-keys");

        put_keys(&keys);
        std::fs::write(keys.join("d/z"), "").unwrap();

        assert!(matches!(
            resolve_in(&root, "d", Some(&keys)),
            Err(KeyError::Mismatch { .. })
        ));
    }

    #[test]
    fn symlink_inside_a_dir_key_is_refused() {
        let root = checkout("inner-link");
        let keys = scratch("inner-link-keys");

        put_keys(&keys);
        std::os::unix::fs::symlink("..", keys.join("d/b/loop")).unwrap();

        let e = resolve_in(&root, "d", Some(&keys)).unwrap_err();

        assert!(
            e.to_string().contains("symlink inside a key directory"),
            "{e}"
        );
    }

    #[test]
    fn name_outside_the_manifest_is_refused() {
        let root = checkout("unknown");

        assert!(matches!(
            resolve_in(&root, "other.json", None),
            Err(KeyError::NotInManifest(_))
        ));
    }

    #[test]
    fn main_worktree_fallback_at_any_depth() {
        let base = scratch("git");
        let main = base.join("main");
        let git = |dir: &Path, args: &[&str]| {
            let ok = Command::new("git")
                .arg("-C")
                .arg(dir)
                .args(["-c", "user.name=t", "-c", "user.email=t@t"])
                .args(args)
                .output()
                .unwrap()
                .status
                .success();

            assert!(ok, "git {args:?}");
        };

        std::fs::create_dir_all(&main).unwrap();
        git(&main, &["init", "-q"]);
        std::fs::create_dir_all(main.join("oracle")).unwrap();
        std::fs::copy(
            checkout("git-src").join("oracle/answer-keys.tsv"),
            main.join("oracle/answer-keys.tsv"),
        )
        .unwrap();
        git(&main, &["add", "oracle/answer-keys.tsv"]);
        git(&main, &["commit", "-qm", "m"]);
        put_keys(&main.join("oracle"));

        for (i, wt) in [base.join("wt"), base.join("a/b/c/wt")].iter().enumerate() {
            git(
                &main,
                &["worktree", "add", "-q", "--detach", wt.to_str().unwrap()],
            );

            assert_eq!(
                resolve_in(wt, "d", None).unwrap(),
                main.join("oracle/d"),
                "worktree {i}"
            );
            assert_eq!(
                resolve_in(wt, "k.json", None).unwrap(),
                main.join("oracle/k.json")
            );
        }
    }
}
