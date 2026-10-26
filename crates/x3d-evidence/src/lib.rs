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
//! # Explicit store and guard (issue #65)
//!
//! [`store`] checks `$X3D_ORACLE_KEYS` before any search: an absolute path to an existing directory
//! whose real path does not overlap any worktree of this repository (inside it or holding it), with
//! every manifest name as an entry in it, of the right kind, whose real path is outside every
//! worktree. An entry may be a symlink to a place outside the store (for example a read-only
//! backup); its content is still measured. A manifest name is one plain path component.
//! Containment compares whole components of canonical paths. An invalid store is
//! [`KeyError::Unsafe`]: nothing else is searched, and no test may skip for it.
//!
//! [`guard`] refuses when real data (not a symlink, below no symlink) sits at a key path or
//! `upstream`, and a commit tracks that path, a path below it, or a parent of it as anything but a
//! directory. A checkout, reset, restore, stash or merge to that commit could delete the data,
//! because Git treats ignored files as expendable. It also refuses such data that the worktree's
//! ignore rules do not cover (`git clean -fd` deletes it); run it again after a checkout that
//! changes those rules. `tools/answer_keys.py guard` is the same check.
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
    /// An invalid explicit store, or a state in which a Git operation could delete a key.
    Unsafe(String),
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
            KeyError::Io(e) | KeyError::Unsafe(e) => write!(f, "{e}"),
        }
    }
}

impl std::error::Error for KeyError {}

/// The checkout that holds this crate.
pub fn workspace_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

/// Ignored and tracked as a symlink, like the render keys; not a key, but it sits at a tracked path too.
pub const GUARDED: &[&str] = &["upstream"];

/// Reads `oracle/answer-keys.tsv` under `root`.
pub fn manifest(root: &Path) -> Result<HashMap<String, Measure>, KeyError> {
    let path = root.join("oracle/answer-keys.tsv");
    let text = std::fs::read_to_string(&path)
        .map_err(|e| KeyError::Io(format!("{}: {e}", path.display())))?;

    parse_manifest(&text, &path)
}

fn parse_manifest(text: &str, path: &Path) -> Result<HashMap<String, Measure>, KeyError> {
    let mut rows = HashMap::new();

    for line in text
        .lines()
        .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
    {
        let c: Vec<&str> = line.split('\t').collect();
        let bad = || KeyError::Io(format!("{}: bad row {line:?}", path.display()));

        // A name is one plain path component, so `<store>/<name>` cannot step out of the store.
        if c.len() != 5 {
            return Err(bad());
        }

        let plain = !matches!(c[0], "" | "." | "..") && !c[0].contains(['/', '\\', '\0']);

        if !plain || !(c[1] == "file" || c[1] == "dir") {
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

fn git(root: &Path, args: &[&str]) -> Option<Vec<u8>> {
    let out = Command::new("git")
        .arg("-C")
        .arg(root)
        .args(args)
        .output()
        .ok()?;

    out.status.success().then_some(out.stdout)
}

fn real(p: &Path) -> PathBuf {
    p.canonicalize().unwrap_or_else(|_| p.to_path_buf())
}

/// Canonical paths of every worktree of the repository that holds `root`, and of `root` itself.
pub fn worktrees(root: &Path) -> Result<Vec<PathBuf>, KeyError> {
    let out = git(root, &["worktree", "list", "--porcelain", "-z"]).ok_or_else(|| {
        KeyError::Unsafe(format!(
            "{}: cannot list the worktrees of this repository; X3D_ORACLE_KEYS cannot be checked",
            root.display()
        ))
    })?;
    let mut all: Vec<PathBuf> = out
        .split(|&b| b == 0)
        .filter_map(|r| r.strip_prefix(b"worktree "))
        .map(|p| real(Path::new(&*String::from_utf8_lossy(p))))
        .chain([real(root)])
        .collect();

    all.sort();
    all.dedup();
    Ok(all)
}

/// Checks the explicit store `explicit` (`$X3D_ORACLE_KEYS`) for the checkout at `root` and returns
/// its canonical path. Every way it fails is [`KeyError::Unsafe`].
pub fn store(
    root: &Path,
    explicit: &Path,
    rows: &HashMap<String, Measure>,
) -> Result<PathBuf, KeyError> {
    let bad = |why: String| {
        KeyError::Unsafe(format!(
            "X3D_ORACLE_KEYS={}: {why}; no other location is searched",
            explicit.display()
        ))
    };

    if !explicit.is_absolute() {
        return Err(bad("not an absolute path".into()));
    }

    let store = explicit
        .canonicalize()
        .map_err(|e| bad(format!("does not exist ({e})")))?;
    let trees = worktrees(root)?;

    if !store.is_dir() {
        return Err(bad(format!("{} is not a directory", store.display())));
    }

    // Path::starts_with compares whole components: /a/wt-keys is not inside /a/wt.
    if let Some(wt) = trees
        .iter()
        .find(|wt| store.starts_with(wt) || wt.starts_with(&store))
    {
        return Err(bad(format!(
            "{} overlaps the worktree {}",
            store.display(),
            wt.display()
        )));
    }

    let mut names: Vec<&String> = rows.keys().collect();
    let mut problems = vec![];

    names.sort();

    for name in names {
        let Ok(key) = store.join(name).canonicalize() else {
            problems.push(format!("{name} is absent"));
            continue;
        };

        if key.is_dir() != rows[name].dir {
            let kind = if rows[name].dir { "dir" } else { "file" };

            problems.push(format!("{name} is not a {kind}"));
        }

        for wt in trees.iter().filter(|wt| key.starts_with(wt)) {
            problems.push(format!(
                "{name} resolves to {}, inside the worktree {}",
                key.display(),
                wt.display()
            ));
        }
    }

    if problems.is_empty() {
        Ok(store)
    } else {
        Err(bad(problems.join("; ")))
    }
}

/// Whether `root/rel` exists, is not a symlink, and no directory between `root` and it is a symlink.
fn real_data(root: &Path, rel: &str) -> bool {
    let mut p = root.to_path_buf();

    rel.split('/').all(|part| {
        p.push(part);
        p.symlink_metadata()
            .is_ok_and(|m| !m.file_type().is_symlink())
    })
}

fn tracked_kind(mode: &str) -> &'static str {
    match mode {
        "120000" => "symlink",
        "160000" => "submodule",
        _ => "file",
    }
}

/// Refuses ([`KeyError::Unsafe`]) when a Git operation on the worktree at `root` to any of `revs`
/// could delete real key data, or when `explicit` (`$X3D_ORACLE_KEYS`) is an invalid store. The key
/// paths are the names in the manifest at `root` and in each rev's manifest, and [`GUARDED`].
pub fn guard(root: &Path, revs: &[&str], explicit: Option<&Path>) -> Result<(), KeyError> {
    // A worktree at a commit before the manifest has none; this workspace's manifest applies then.
    // A manifest that is present but unreadable or malformed is an error.
    let ours = manifest(&workspace_root())?;
    let rows = if root
        .join("oracle/answer-keys.tsv")
        .symlink_metadata()
        .is_ok()
    {
        manifest(root)?
    } else {
        ours.clone()
    };
    let mut names: Vec<String> = rows
        .keys()
        .chain(ours.keys())
        .map(|n| format!("oracle/{n}"))
        .collect();
    let mut problems = vec![];

    for rev in revs {
        git(
            root,
            &[
                "rev-parse",
                "--verify",
                "--quiet",
                &format!("{rev}^{{commit}}"),
            ],
        )
        .ok_or_else(|| {
            KeyError::Unsafe(format!(
                "guard: {rev} is not a commit in {}",
                root.display()
            ))
        })?;

        if let Some(text) = git(root, &["show", &format!("{rev}:oracle/answer-keys.tsv")]) {
            let theirs = parse_manifest(&String::from_utf8_lossy(&text), Path::new(rev))?;

            names.extend(theirs.keys().map(|n| format!("oracle/{n}")));
        }
    }

    names.extend(GUARDED.iter().map(|g| g.to_string()));
    names.sort();
    names.dedup();

    let held: Vec<&String> = names.iter().filter(|p| real_data(root, p)).collect();
    let mut parents: Vec<String> = held
        .iter()
        .flat_map(|p| {
            let parts: Vec<&str> = p.split('/').collect();

            (1..parts.len()).map(move |i| parts[..i].join("/"))
        })
        .collect();

    parents.sort();
    parents.dedup();

    if !held.is_empty() {
        // `git clean -fd` deletes untracked data that the ignore rules do not cover.
        let out = Command::new("git")
            .arg("-C")
            .arg(root)
            .args(["check-ignore", "--no-index", "--"])
            .args(held.iter().map(|p| p.as_str()))
            .output()
            .ok()
            .filter(|o| matches!(o.status.code(), Some(0 | 1)))
            .ok_or_else(|| {
                KeyError::Unsafe(format!(
                    "guard: cannot read the ignore rules in {}",
                    root.display()
                ))
            })?;
        // A manifest name cannot hold a newline (the manifest is line-based).
        let ignored: Vec<String> = String::from_utf8_lossy(&out.stdout)
            .lines()
            .map(str::to_string)
            .collect();

        for p in held.iter().filter(|p| !ignored.contains(p)) {
            problems.push(format!(
                "{p}: real data that the ignore rules do not cover; git clean -fd would delete it"
            ));
        }
    }

    for rev in revs.iter().filter(|_| !held.is_empty()) {
        let mut args = vec!["ls-tree", "-z", "--full-tree", rev, "--"];

        args.extend(held.iter().map(|p| p.as_str()));
        args.extend(parents.iter().map(String::as_str));

        let out = git(root, &args).ok_or_else(|| {
            KeyError::Unsafe(format!(
                "guard: cannot read the tree of {rev} in {}",
                root.display()
            ))
        })?;
        let tracked: HashMap<String, String> = out
            .split(|&b| b == 0)
            .filter_map(|r| {
                let r = String::from_utf8_lossy(r);
                let (meta, path) = r.split_once('\t')?;

                Some((path.to_string(), meta.split(' ').next()?.to_string()))
            })
            .collect();

        for p in &held {
            match tracked.get(p.as_str()).map(String::as_str) {
                Some("040000") => {
                    problems.push(format!("{p}: real data, and {rev} tracks files below it"))
                }
                Some(mode) => problems.push(format!(
                    "{p}: real data, and {rev} tracks it as a {}",
                    tracked_kind(mode)
                )),
                None => {}
            }

            for a in parents.iter().filter(|a| p.starts_with(&format!("{a}/"))) {
                if let Some(mode) = tracked.get(a).filter(|m| *m != "040000") {
                    problems.push(format!(
                        "{p}: real data, and {rev} tracks its parent {a} as a {}",
                        tracked_kind(mode)
                    ));
                }
            }
        }
    }

    if let Some(e) = explicit
        && let Err(err) = store(root, e, &rows)
    {
        problems.push(err.to_string());
    }

    if problems.is_empty() {
        Ok(())
    } else {
        Err(KeyError::Unsafe(format!(
            "guard: refused for {}:\n  {}",
            root.display(),
            problems.join("\n  ")
        )))
    }
}

/// The locations searched for `name`, in order. `explicit` must already be a checked [`store`].
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
    let explicit = match explicit {
        Some(e) => Some(store(root, e, &rows)?),
        None => None,
    };
    let searched = candidates(root, name, explicit.as_deref());

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

    fn git(dir: &Path, args: &[&str]) {
        let out = Command::new("git")
            .arg("-C")
            .arg(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@t"])
            .args(args)
            .output()
            .unwrap();

        assert!(out.status.success(), "git {args:?}: {out:?}");
    }

    fn unsafe_with(r: Result<impl fmt::Debug, KeyError>, text: &str) {
        match r {
            Err(KeyError::Unsafe(e)) => assert!(e.contains(text), "{e:?} lacks {text:?}"),
            r => panic!("want Unsafe({text:?}), got {r:?}"),
        }
    }

    /// A Git checkout (one commit: the manifest) with one file key `k.json` = "abc" and one dir key
    /// `d` = {a: "1", b/c: "2"}. The keys themselves are not written.
    fn checkout(tag: &str) -> PathBuf {
        let root = manifest_only(tag);

        // Ignore rules like the real repository's: the keys and upstream/ as ignored paths.
        std::fs::write(
            root.join(".gitignore"),
            "/oracle/k.json\n/oracle/d/\n/upstream/\n",
        )
        .unwrap();
        git(&root, &["init", "-q"]);
        git(&root, &["add", "oracle/answer-keys.tsv", ".gitignore"]);
        git(&root, &["commit", "-qm", "m"]);
        root
    }

    fn manifest_only(tag: &str) -> PathBuf {
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

    /// Valid keys in this worktree do not help: an invalid explicit store is an error, not a miss.
    #[test]
    fn invalid_explicit_store_has_no_fallback() {
        let root = checkout("explicit-only");
        let keys = scratch("explicit-only-keys");

        put_keys(&root.join("oracle"));
        assert_eq!(resolve_in(&root, "d", None).unwrap(), root.join("oracle/d"));

        let e = resolve_in(&root, "k.json", Some(&keys)).unwrap_err();

        assert!(
            e.to_string().contains("no other location is searched"),
            "{e}"
        );
        unsafe_with(resolve_in(&root, "k.json", Some(&keys)), "k.json is absent");
    }

    #[test]
    fn store_missing_root_is_refused() {
        let root = checkout("store-missing");

        unsafe_with(
            resolve_in(&root, "d", Some(&root.with_extension("nowhere"))),
            "does not exist",
        );
    }

    #[test]
    fn store_regular_file_is_refused() {
        let root = checkout("store-file");
        let file = scratch("store-file-keys").join("keys");

        std::fs::write(&file, "").unwrap();
        unsafe_with(resolve_in(&root, "d", Some(&file)), "is not a directory");
    }

    #[test]
    fn store_missing_required_key_is_refused() {
        let root = checkout("store-partial");
        let keys = scratch("store-partial-keys");

        put_keys(&keys);
        std::fs::remove_dir_all(keys.join("d")).unwrap();
        // k.json itself is fine; the store as a whole is not.
        unsafe_with(resolve_in(&root, "k.json", Some(&keys)), "d is absent");
    }

    #[test]
    fn store_key_of_the_wrong_kind_is_refused() {
        let root = checkout("store-kind");
        let keys = scratch("store-kind-keys");

        put_keys(&keys);
        std::fs::remove_file(keys.join("k.json")).unwrap();
        std::fs::create_dir(keys.join("k.json")).unwrap();
        unsafe_with(resolve_in(&root, "d", Some(&keys)), "k.json is not a file");
    }

    #[test]
    fn store_through_a_link_into_a_worktree_is_refused() {
        let root = checkout("store-link");
        let base = scratch("store-link-keys");
        let link = base.join("keys");

        put_keys(&root.join("oracle"));
        std::os::unix::fs::symlink(root.join("oracle"), &link).unwrap();
        unsafe_with(resolve_in(&root, "d", Some(&link)), "overlaps the worktree");

        // A real store directory whose key entries link into the worktree.
        let keys = base.join("entries");

        std::fs::create_dir(&keys).unwrap();
        for k in ["k.json", "d"] {
            std::os::unix::fs::symlink(root.join("oracle").join(k), keys.join(k)).unwrap();
        }
        unsafe_with(resolve_in(&root, "d", Some(&keys)), "inside the worktree");
    }

    #[test]
    fn store_inside_or_holding_any_worktree_is_refused() {
        let root = checkout("store-wt");
        let linked = scratch("store-wt-linked").join("wt");

        git(
            &root,
            &[
                "worktree",
                "add",
                "-q",
                "--detach",
                linked.to_str().unwrap(),
            ],
        );

        // Inside a linked worktree, asked from the main one.
        let keys = linked.join("keys");

        put_keys(&keys);
        unsafe_with(resolve_in(&root, "d", Some(&keys)), "overlaps the worktree");

        // A store that holds a worktree: its parent directory.
        put_keys(linked.parent().unwrap());
        unsafe_with(
            resolve_in(&root, "d", Some(linked.parent().unwrap())),
            "overlaps the worktree",
        );
    }

    /// Containment compares path components: `main-keys` is next to `main`, not inside it.
    #[test]
    fn store_in_a_sibling_with_a_similar_name_is_accepted() {
        let base = scratch("store-sibling");
        let root = base.join("main");
        let keys = base.join("main-keys");

        std::fs::rename(checkout("store-sibling-src"), &root).unwrap();
        put_keys(&keys);

        assert_eq!(resolve_in(&root, "d", Some(&keys)).unwrap(), keys.join("d"));
        // An absolute path with `..` is canonicalized first.
        assert_eq!(
            resolve_in(&root, "d", Some(&root.join("../main-keys"))).unwrap(),
            keys.join("d")
        );
        unsafe_with(
            resolve_in(&root, "d", Some(&keys.join("../main/oracle"))),
            "overlaps the worktree",
        );
    }

    /// A manifest name cannot step out of the store: one plain path component only.
    #[test]
    fn manifest_name_with_a_path_is_refused() {
        let root = checkout("bad-name");
        let tsv = root.join("oracle/answer-keys.tsv");
        let text = std::fs::read_to_string(&tsv).unwrap();

        for name in ["../k.json", "a/k.json", "..", "/k.json"] {
            std::fs::write(&tsv, text.replace("k.json\t", &format!("{name}\t"))).unwrap();
            assert!(
                matches!(manifest(&root), Err(KeyError::Io(e)) if e.contains("bad row")),
                "{name}"
            );
        }
    }

    #[test]
    fn short_or_malformed_manifest_row_is_an_error() {
        let root = checkout("short-row");
        let tsv = root.join("oracle/answer-keys.tsv");

        for row in ["k.json", "k.json\tfile\t1", "k.json\tfile\tone\t3\tx"] {
            std::fs::write(&tsv, format!("{row}\n")).unwrap();
            assert!(matches!(manifest(&root), Err(KeyError::Io(e)) if e.contains("bad row")));
            // The guard does not fall back to another manifest when this one is malformed.
            assert!(guard(&root, &["HEAD"], None).is_err(), "{row}");
        }
    }

    /// An entry may link out of the store (a read-only backup), but never into a worktree.
    #[test]
    fn store_entry_linking_outside_the_store_is_measured() {
        let root = checkout("store-backup");
        let backup = scratch("store-backup-copy");
        let keys = scratch("store-backup-keys");

        put_keys(&backup);
        std::os::unix::fs::symlink(backup.join("d"), keys.join("d")).unwrap();
        std::os::unix::fs::symlink(backup.join("k.json"), keys.join("k.json")).unwrap();
        assert_eq!(resolve_in(&root, "d", Some(&keys)).unwrap(), keys.join("d"));

        std::fs::write(backup.join("d/a"), "changed").unwrap();
        assert!(matches!(
            resolve_in(&root, "d", Some(&keys)),
            Err(KeyError::Mismatch { .. })
        ));
    }

    #[test]
    fn store_relative_path_is_refused() {
        let root = checkout("store-rel");
        let keys = scratch("store-rel-keys");

        put_keys(&keys);
        unsafe_with(
            resolve_in(&root, "d", Some(Path::new("store-rel-keys"))),
            "not an absolute path",
        );
        assert_eq!(resolve_in(&root, "d", Some(&keys)).unwrap(), keys.join("d"));
    }

    #[test]
    fn store_outside_a_git_checkout_is_refused() {
        let root = manifest_only("store-nogit");
        let keys = scratch("store-nogit-keys");

        put_keys(&keys);
        unsafe_with(
            resolve_in(&root, "d", Some(&keys)),
            "cannot list the worktrees",
        );
    }

    /// A checkout whose HEAD tracks `oracle/d` and `upstream` as symlinks, like X_ITE_R today.
    fn linked_checkout(tag: &str) -> PathBuf {
        let root = checkout(tag);

        std::os::unix::fs::symlink("../../store/d", root.join("oracle/d")).unwrap();
        std::os::unix::fs::symlink("../store/upstream", root.join("upstream")).unwrap();
        git(&root, &["add", "oracle/d", "upstream"]);
        git(&root, &["commit", "-qm", "links"]);
        root
    }

    #[test]
    fn guard_passes_for_symlinks_and_untracked_keys() {
        let root = linked_checkout("guard-ok");

        // k.json is real but untracked; d and upstream are the tracked symlinks.
        std::fs::write(root.join("oracle/k.json"), "abc").unwrap();
        guard(&root, &["HEAD"], None).unwrap();
    }

    #[test]
    fn guard_refuses_real_data_that_is_not_ignored() {
        let root = checkout("guard-clean");

        put_keys(&root.join("oracle"));
        guard(&root, &["HEAD"], None).unwrap();
        std::fs::write(root.join(".gitignore"), "/oracle/k.json\n").unwrap();
        unsafe_with(
            guard(&root, &["HEAD"], None),
            "oracle/d: real data that the ignore rules do not cover",
        );
    }

    #[test]
    fn guard_refuses_real_files_at_a_tracked_symlink() {
        let root = linked_checkout("guard-link");

        std::fs::remove_file(root.join("oracle/d")).unwrap();
        put_keys(&root.join("oracle"));
        unsafe_with(
            guard(&root, &["HEAD"], None),
            "oracle/d: real data, and HEAD tracks it as a symlink",
        );

        std::fs::remove_file(root.join("upstream")).unwrap();
        std::fs::create_dir(root.join("upstream")).unwrap();
        unsafe_with(
            guard(&root, &["HEAD"], None),
            "upstream: real data, and HEAD tracks it as a symlink",
        );
    }

    /// Both directions: a commit that tracks a parent of a key path, or files below it.
    #[test]
    fn guard_refuses_a_tracked_parent_or_child() {
        let root = checkout("guard-parent");
        let other = scratch("guard-parent-wt").join("wt");

        git(
            &root,
            &[
                "worktree",
                "add",
                "-q",
                "-b",
                "old",
                other.to_str().unwrap(),
            ],
        );
        std::fs::remove_dir_all(other.join("oracle")).unwrap();
        std::os::unix::fs::symlink("elsewhere", other.join("oracle")).unwrap();
        git(&other, &["add", "-A"]);
        git(&other, &["commit", "-qm", "oracle is a link"]);

        git(&other, &["checkout", "-q", "-b", "child", "HEAD~1"]);
        std::fs::create_dir_all(other.join("oracle/d")).unwrap();
        std::fs::write(other.join("oracle/d/a"), "1").unwrap();
        git(&other, &["add", "-f", "oracle/d/a"]);
        git(&other, &["commit", "-qm", "a file below d"]);

        put_keys(&root.join("oracle"));
        guard(&root, &["HEAD"], None).unwrap();
        unsafe_with(
            guard(&root, &["HEAD", "old"], None),
            "oracle/d: real data, and old tracks its parent oracle as a symlink",
        );
        unsafe_with(
            guard(&root, &["child"], None),
            "oracle/d: real data, and child tracks files below it",
        );
    }

    #[test]
    fn guard_ignores_data_below_a_symlink() {
        let root = checkout("guard-below-link");
        let real = scratch("guard-below-link-real");

        put_keys(&real.join("oracle"));
        std::fs::copy(
            root.join("oracle/answer-keys.tsv"),
            real.join("oracle/answer-keys.tsv"),
        )
        .unwrap();
        std::fs::create_dir(root.join("sub")).unwrap();
        std::os::unix::fs::symlink(real.join("oracle"), root.join("sub/oracle")).unwrap();
        assert!(!real_data(&root, "sub/oracle/d"));
        assert!(real_data(&real, "oracle/d"));
    }

    #[test]
    fn guard_refuses_an_invalid_store_and_an_unknown_rev() {
        let root = linked_checkout("guard-store");
        let keys = scratch("guard-store-keys");

        put_keys(&keys);
        guard(&root, &["HEAD"], Some(&keys)).unwrap();
        put_keys(&root.join("store"));
        unsafe_with(
            guard(&root, &["HEAD"], Some(&root.join("store"))),
            "overlaps the worktree",
        );
        unsafe_with(guard(&root, &["no-such-rev"], None), "not a commit");
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

        std::fs::rename(checkout("git-src"), &main).unwrap();
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
