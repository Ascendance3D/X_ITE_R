#!/usr/bin/env python3
"""Read-only resolver for the gitignored X_ITE answer keys listed in oracle/answer-keys.tsv (issue #65).

    python3 -I -B tools/answer_keys.py resolve NAME     print the verified path of one key
    python3 -I -B tools/answer_keys.py verify [NAME...] resolve and verify keys (default: every key)
    python3 -I -B tools/answer_keys.py link DEST NAME... symlink DEST/oracle/NAME to each verified key
    python3 -I -B tools/answer_keys.py guard WORKTREE [REV...]
                                                     refuse before a Git operation on WORKTREE to REV (default HEAD)

Search order, first hit wins:
  1. $X3D_ORACLE_KEYS/NAME. When the variable is set, it is the only location searched.
  2. <this worktree>/oracle/NAME, when it is a real file or directory (not a symlink).
  3. <main worktree>/oracle/NAME. The main worktree is the parent of `git rev-parse --git-common-dir`.

A key is accepted only when its kind, file count, byte count and sha256 equal the manifest row. For a directory
the sha256 is the tree digest: sha256 of the `sha256sum` lines of every file, relative paths, sorted bytewise.
A found key that does not match is a mismatch, and the resolver does not try the next location.

An explicit store ($X3D_ORACLE_KEYS) must be an absolute path to an existing directory. Its real path must not
overlap any worktree of this repository (inside it or holding it), and every manifest name must be an entry in
it, of the right kind, whose real path is outside every worktree. An entry may be a symlink to a place outside
the store (for example a read-only backup); its content is still measured. A manifest name is one plain path
component. Containment compares whole components of real paths. An invalid store is an error: nothing else is
searched.

The guard refuses when real data (not a symlink, below no symlink) sits at a key path or `upstream`, and a
REV tracks that path, a path below it, or a parent of it as anything but a directory. A checkout, reset,
restore, stash or merge to REV could then delete the data, because Git treats ignored files as expendable. It
also refuses such data that the worktree's ignore rules do not cover (`git clean -fd` deletes it), and an
invalid $X3D_ORACLE_KEYS. Run it again after a checkout that changes the ignore rules. The key paths are the names in this manifest and in each REV's one.

The resolver never writes, moves or renames a key. `link` writes only symlinks under DEST/oracle and refuses to
replace a real file or directory. Exit codes: 0 found (or guard passed), 2 absent, 3 mismatch, 4 unsafe (invalid
store, or guard refused), 1 usage or manifest error."""
import hashlib, os, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = os.path.join(ROOT, "oracle", "answer-keys.tsv")
UNSAFE = 4
# Ignored and tracked as a symlink, like the render keys; not a key, but it sits at a tracked path too.
GUARDED = ("upstream",)
KINDS = {"120000": "symlink", "160000": "submodule"}


class KeyError_(Exception):
    def __init__(self, code, msg):
        super().__init__(msg)
        self.code = code


def parse(lines):
    rows = {}
    for line in lines:
        if line.startswith("#") or not line.strip():
            continue
        cols = line.rstrip("\n").split("\t")
        # A name is one plain path component, so <store>/<name> cannot step out of the store.
        if (len(cols) != 5 or cols[0] in ("", ".", "..") or any(c in cols[0] for c in "/\\\0")
                or cols[1] not in ("file", "dir") or not all(c.isascii() and c.isdigit() for c in cols[2] + cols[3]) or not (cols[2] and cols[3])):
            raise KeyError_(1, f"answer-keys.tsv: bad row {line!r}")
        name, kind, files, size, digest = cols
        rows[name] = (kind, int(files), int(size), digest)
    return rows


def manifest(path=MANIFEST):
    with open(path, encoding="utf-8") as f:
        return parse(f)


def _sha256(path):
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for block in iter(lambda: f.read(1 << 20), b""):
            h.update(block)
    return h.hexdigest()


def measure(path):
    """(kind, files, bytes, sha256) of a file or a directory tree."""
    if os.path.isfile(path):
        return "file", 1, os.path.getsize(path), _sha256(path)
    rels = []
    for d, dirs, names in os.walk(path):
        for n in dirs + names:
            if os.path.islink(os.path.join(d, n)):
                raise OSError(f"{os.path.join(d, n)}: a symlink inside a key directory is refused")
        for n in names:
            rels.append(os.path.relpath(os.path.join(d, n), path))
    rels.sort(key=os.fsencode)
    listing, size = hashlib.sha256(), 0
    for r in rels:
        p = os.path.join(path, r)
        size += os.path.getsize(p)
        listing.update(f"{_sha256(p)}  {r}\n".encode())
    return "dir", len(rels), size, listing.hexdigest()


def main_worktree(root=ROOT):
    try:
        out = subprocess.run(["git", "-C", root, "rev-parse", "--path-format=absolute", "--git-common-dir"],
                             capture_output=True, text=True, check=True).stdout.strip()
    except (OSError, subprocess.CalledProcessError):
        return None
    return os.path.dirname(out) if os.path.basename(out) == ".git" else None


def _git(root, *args):
    return subprocess.run(["git", "-C", root, *args], capture_output=True, check=True).stdout


def worktrees(root=ROOT):
    """Real paths of every worktree of the repository that holds root, and of root itself."""
    try:
        out = _git(root, "worktree", "list", "--porcelain", "-z")
    except (OSError, subprocess.CalledProcessError) as e:
        raise KeyError_(UNSAFE, f"{root}: cannot list the worktrees of this repository ({e}); "
                                "X3D_ORACLE_KEYS cannot be checked")
    paths = [os.fsdecode(r[len(b"worktree "):]) for r in out.split(b"\0") if r.startswith(b"worktree ")]
    return sorted({os.path.realpath(p) for p in paths + [root]})


def inside(path, parent):
    """Whether real path `path` is `parent` or below it. Whole components, never a string prefix."""
    return os.path.commonpath([path, parent]) == parent


def store(root=ROOT, rows=None):
    """The checked real path of $X3D_ORACLE_KEYS, or None when it is unset. An invalid store raises UNSAFE."""
    value = os.environ.get("X3D_ORACLE_KEYS")
    if not value:
        return None
    rows = rows if rows is not None else manifest(os.path.join(root, "oracle", "answer-keys.tsv"))
    bad = lambda why: KeyError_(UNSAFE, f"X3D_ORACLE_KEYS={value}: {why}; no other location is searched")
    if not os.path.isabs(value):
        raise bad("not an absolute path")
    if not os.path.exists(value):
        raise bad("does not exist")
    real, trees = os.path.realpath(value), worktrees(root)
    if not os.path.isdir(real):
        raise bad(f"{real} is not a directory")
    for wt in trees:
        if inside(real, wt) or inside(wt, real):
            raise bad(f"{real} overlaps the worktree {wt}")
    problems = []
    for name, (kind, *_) in sorted(rows.items()):
        p = os.path.join(real, name)
        if not os.path.exists(p):
            problems.append(f"{name} is absent")
            continue
        rp = os.path.realpath(p)
        if os.path.isdir(rp) != (kind == "dir"):
            problems.append(f"{name} is not a {kind}")
        problems += [f"{name} resolves to {rp}, inside the worktree {wt}" for wt in trees if inside(rp, wt)]
    if problems:
        raise bad("; ".join(problems))
    return real


def candidates(name, root=ROOT, rows=None):
    explicit = store(root, rows)
    if explicit:
        return [os.path.join(explicit, name)]
    out = [os.path.join(root, "oracle", name)]
    main = main_worktree(root)
    if main and os.path.realpath(main) != os.path.realpath(root):
        out.append(os.path.join(main, "oracle", name))
    return out


def resolve(name, root=ROOT, rows=None):
    rows = rows if rows is not None else manifest(os.path.join(root, "oracle", "answer-keys.tsv"))
    if name not in rows:
        raise KeyError_(1, f"{name}: not in oracle/answer-keys.tsv")
    searched = candidates(name, root, rows)
    for i, p in enumerate(searched):
        # Step 2 skips a symlink in this worktree; an explicit location or the main worktree may be a link.
        if not os.path.exists(p) or (i == 0 and not os.environ.get("X3D_ORACLE_KEYS") and os.path.islink(p)):
            continue
        try:
            got = measure(p)
        except OSError as e:
            raise KeyError_(3, f"{name}: mismatch at {p}: {e}")
        if got != rows[name]:
            raise KeyError_(3, f"{name}: mismatch at {p}: found {got[0]}, {got[1]} files, {got[2]} bytes, sha256 {got[3]}; "
                               f"manifest wants {rows[name][0]}, {rows[name][1]} files, {rows[name][2]} bytes, "
                               f"sha256 {rows[name][3]}")
        return os.path.abspath(p)
    raise KeyError_(2, f"{name}: absent; searched " + ", ".join(searched)
                       + "; restore it as oracle/README.md describes, or set X3D_ORACLE_KEYS")


def _real_data(root, rel):
    """Whether root/rel exists, is not a symlink, and no directory between root and it is a symlink."""
    p = root
    for part in rel.split("/"):
        p = os.path.join(p, part)
        if os.path.islink(p) or not os.path.lexists(p):
            return False
    return True


def guard(root, revs=("HEAD",)):
    """Refuses (UNSAFE) when a Git operation on the worktree at root to any of revs could delete real key data."""
    own = os.path.join(root, "oracle", "answer-keys.tsv")
    rows = manifest(own) if os.path.lexists(own) else manifest()  # present but malformed: an error
    problems, names = [], set(rows) | set(manifest())
    for rev in revs:
        try:
            _git(root, "rev-parse", "--verify", "--quiet", f"{rev}^{{commit}}")
        except (OSError, subprocess.CalledProcessError):
            raise KeyError_(UNSAFE, f"guard: {rev} is not a commit in {root}")
        # A commit before the manifest has none, and this manifest's names still apply. Any Git failure is unsafe.
        try:
            if _git(root, "ls-tree", "--full-tree", rev, "--", "oracle/answer-keys.tsv"):
                names |= set(parse(_git(root, "show", f"{rev}:oracle/answer-keys.tsv").decode().splitlines()))
        except (OSError, subprocess.CalledProcessError, UnicodeDecodeError) as e:
            raise KeyError_(UNSAFE, f"guard: cannot read the manifest of {rev} in {root}: {e}")
    held = [p for p in sorted("oracle/" + n for n in names) + list(GUARDED) if _real_data(root, p)]
    parents = sorted({"/".join(p.split("/")[:i]) for p in held for i in range(1, p.count("/") + 1)})
    if held:
        # `git clean -fd` deletes untracked data that the ignore rules do not cover.
        try:
            r = subprocess.run(["git", "-C", root, "check-ignore", "--no-index", "--stdin", "-z"],
                               input=b"".join(os.fsencode(p) + b"\0" for p in held), capture_output=True)
        except OSError as e:
            raise KeyError_(UNSAFE, f"guard: cannot read the ignore rules in {root}: {e}")
        if r.returncode not in (0, 1):
            raise KeyError_(UNSAFE, f"guard: cannot read the ignore rules in {root}: {r.stderr.decode().strip()}")
        ignored = {os.fsdecode(p) for p in r.stdout.split(b"\0") if p}  # -z: no pathname quoting
        problems += [f"{p}: real data that the ignore rules do not cover; git clean -fd would delete it"
                     for p in held if p not in ignored]
    for rev in revs:
        try:
            out = _git(root, "ls-tree", "-z", "--full-tree", rev, "--", *held, *parents) if held else b""
        except (OSError, subprocess.CalledProcessError) as e:
            raise KeyError_(UNSAFE, f"guard: cannot read the tree of {rev} in {root}: {e}")
        tracked = {}
        for rec in filter(None, out.split(b"\0")):
            meta, path = rec.split(b"\t", 1)
            tracked[os.fsdecode(path)] = meta.split(b" ")[0].decode()
        for p in held:
            if p in tracked:
                mode = tracked[p]
                what = "files below it" if mode == "040000" else f"it as a {KINDS.get(mode, 'file')}"
                problems.append(f"{p}: real data, and {rev} tracks {what}")
            for a in (q for q in parents if p.startswith(q + "/") and q in tracked):
                if tracked[a] != "040000":
                    problems.append(f"{p}: real data, and {rev} tracks its parent {a} as a "
                                    + KINDS.get(tracked[a], "file"))
    try:
        store(root, rows)
    except KeyError_ as e:
        problems.append(str(e))
    if problems:
        raise KeyError_(UNSAFE, f"guard: refused for {root}:\n  " + "\n  ".join(problems))


def link(dest, names, root=ROOT):
    for name in names:
        src = resolve(name, root)
        at = os.path.join(dest, "oracle", name)
        if os.path.realpath(at) == os.path.realpath(src) and not os.path.islink(at):
            continue  # dest is the store itself
        if os.path.exists(at) and not os.path.islink(at):
            raise KeyError_(1, f"{at}: a real file or directory is present; not replaced")
        tmp = f"{at}.answer-keys-tmp-{os.getpid()}"
        if os.path.lexists(tmp) and not os.path.islink(tmp):
            raise KeyError_(1, f"{tmp}: a real file is present; not replaced")
        if os.path.lexists(tmp):
            os.remove(tmp)
        os.symlink(src, tmp)
        os.replace(tmp, at)
        print(f"{at} -> {src}")


if __name__ == "__main__":
    a = sys.argv[1:]
    try:
        if len(a) == 2 and a[0] == "resolve":
            print(resolve(a[1]))
        elif a and a[0] == "verify":
            names = a[1:] or list(manifest())
            for n in names:
                print(f"OK {n} {resolve(n)}")
        elif len(a) >= 3 and a[0] == "link":
            link(a[1], a[2:])
        elif len(a) >= 2 and a[0] == "guard":
            guard(os.path.abspath(a[1]), a[2:] or ["HEAD"])
            print(f"guard: passed for {os.path.abspath(a[1])} at " + ", ".join(a[2:] or ["HEAD"]))
        else:
            raise KeyError_(1, __doc__.split("\n\n")[1])
    except KeyError_ as e:
        print(e, file=sys.stderr)
        sys.exit(e.code)
