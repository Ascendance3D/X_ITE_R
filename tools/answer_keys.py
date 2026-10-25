#!/usr/bin/env python3
"""Read-only resolver for the gitignored X_ITE answer keys listed in oracle/answer-keys.tsv (issue #65).

    python3 -I -B tools/answer_keys.py resolve NAME     print the verified path of one key
    python3 -I -B tools/answer_keys.py verify [NAME...] resolve and verify keys (default: every key)
    python3 -I -B tools/answer_keys.py link DEST NAME... symlink DEST/oracle/NAME to each verified key

Search order, first hit wins:
  1. $X3D_ORACLE_KEYS/NAME. When the variable is set, it is the only location searched.
  2. <this worktree>/oracle/NAME, when it is a real file or directory (not a symlink).
  3. <main worktree>/oracle/NAME. The main worktree is the parent of `git rev-parse --git-common-dir`.

A key is accepted only when its kind, file count, byte count and sha256 equal the manifest row. For a directory
the sha256 is the tree digest: sha256 of the `sha256sum` lines of every file, relative paths, sorted bytewise.
A found key that does not match is a mismatch, and the resolver does not try the next location.

The resolver never writes, moves or renames a key. `link` writes only symlinks under DEST/oracle and refuses to
replace a real file or directory. Exit codes: 0 found, 2 absent, 3 mismatch, 1 usage or manifest error."""
import hashlib, os, subprocess, sys

ROOT = os.path.dirname(os.path.dirname(os.path.abspath(__file__)))
MANIFEST = os.path.join(ROOT, "oracle", "answer-keys.tsv")


class KeyError_(Exception):
    def __init__(self, code, msg):
        super().__init__(msg)
        self.code = code


def manifest(path=MANIFEST):
    rows = {}
    with open(path, encoding="utf-8") as f:
        for line in f:
            if line.startswith("#") or not line.strip():
                continue
            name, kind, files, size, digest = line.rstrip("\n").split("\t")
            rows[name] = (kind, int(files), int(size), digest)
    return rows


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


def candidates(name, root=ROOT):
    explicit = os.environ.get("X3D_ORACLE_KEYS")
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
    searched = candidates(name, root)
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
        else:
            raise KeyError_(1, __doc__.split("\n\n")[1])
    except KeyError_ as e:
        print(e, file=sys.stderr)
        sys.exit(e.code)
