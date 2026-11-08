#!/usr/bin/env python3
"""The MPL-2.0 source of the browser package: python3 -I -B tools/mpl_sources.py (--write DIR | --verify DIR | --list)

MPL-2.0 section 3.2 (a): who distributes a Covered Software in Executable Form must also make its Source Code Form
available. The browser module (pkg/x3d_web_bg.wasm) holds the MPL-2.0 crates that THIRD_PARTY_CRATES.md lists (the
feature-resolved wasm32 crate set of x3d-web). This repository does not change them (no [patch] entry), so their Source
Code Form is the published crate archive.

    --list         the MPL-2.0 crates of THIRD_PARTY_CRATES.md with their Cargo.lock checksums
    --write DIR    copy each crate archive from the Cargo registry cache to DIR/third-party-source/mpl-2.0/, after a check
                   that its SHA-256 is the Cargo.lock checksum (the archive Cargo built from), and write
                   DIR/third-party-source/README.md and DIR/third-party-source/MPL-2.0.txt
    --verify DIR   check a package: every listed crate archive is present with its Cargo.lock checksum, no other archive
                   is there, the README lists each one, and the license text is the MPL-2.0 text
    --self-test    --write and --verify on a temporary directory, then the failure cases

This prepares the source delivery. It is not a legal finding: the method (archive in the package, or a link to the
source) is an owner decision after legal review.
"""
import hashlib
import os
import re
import shutil
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
NOTICES = ROOT / "THIRD_PARTY_CRATES.md"
LOCK = ROOT / "Cargo.lock"
SUB = Path("third-party-source")
CRATES_IO = "registry+https://github.com/rust-lang/crates.io-index"


def mpl_crates():
    """(name, version) of each crate whose license expression in THIRD_PARTY_CRATES.md names MPL-2.0, sorted."""
    found, head = set(), None
    for line in NOTICES.read_text(encoding="utf-8").splitlines():
        m = re.match(r"^### (\S+) (\d\S*)$", line)
        if m:
            head = (m.group(1), m.group(2))
        elif head and line.startswith("- License: "):
            if "MPL-2.0" in line:
                found.add(head)
            head = None
    return sorted(found)


def lock_entries():
    """(name, version) -> (source, checksum) from Cargo.lock."""
    entries = {}
    for block in LOCK.read_text(encoding="utf-8").split("[[package]]")[1:]:
        f = dict(re.findall(r'^(\w+) = "([^"]*)"$', block, re.M))
        entries[(f.get("name"), f.get("version"))] = (f.get("source"), f.get("checksum"))
    return entries


def table():
    lock = lock_entries()
    rows, bad = [], []
    for name, version in mpl_crates():
        source, checksum = lock.get((name, version), (None, None))
        if source != CRATES_IO or not checksum:
            bad.append(f"{name} {version}: not a crates.io package with a checksum in Cargo.lock ({source})")
            continue
        rows.append((name, version, checksum))
    if not rows:
        bad.append("no MPL-2.0 crate found in THIRD_PARTY_CRATES.md")
    return rows, bad


def mpl_text():
    """The MPL-2.0 text that THIRD_PARTY_CRATES.md reproduces (the first text block that starts with its title)."""
    text = NOTICES.read_text(encoding="utf-8")
    m = re.search(r"```text\n(Mozilla Public License Version 2\.0\n.*?)\n```", text, re.S)
    if not m:
        raise SystemExit("ERROR: THIRD_PARTY_CRATES.md holds no MPL-2.0 text")
    return m.group(1) + "\n"


def registry_caches():
    home = Path(os.environ.get("CARGO_HOME", Path.home() / ".cargo"))
    return sorted((home / "registry" / "cache").glob("*"))


def sha256(path):
    return hashlib.sha256(Path(path).read_bytes()).hexdigest()


def readme(rows):
    lines = [
        "# MPL-2.0 source code of the browser module",
        "",
        "`pkg/x3d_web_bg.wasm` contains the crates below. They are licensed under the Mozilla Public License 2.0",
        "(`MPL-2.0.txt`). X_ITE_R uses them without changes. Their Source Code Form is in `mpl-2.0/`: each file is the",
        "crate archive published on crates.io, and its SHA-256 is the checksum in the `Cargo.lock` of the commit in",
        "`build-info.json`. Extract an archive with `tar -xzf <file>`.",
        "",
        "The same source is at `https://crates.io/api/v1/crates/<crate>/<version>/download`.",
        "",
        "| crate | version | file | SHA-256 |",
        "| --- | --- | --- | --- |",
    ]
    lines += [f"| {n} | {v} | `mpl-2.0/{n}-{v}.crate` | `{c}` |" for n, v, c in rows]
    return "\n".join(lines) + "\n"


def write(out):
    rows, bad = table()
    if bad:
        print("\n".join(f"ERROR: {b}" for b in bad), file=sys.stderr)
        return 1
    final = Path(out) / SUB
    if final.exists():
        print(f"ERROR: {final} exists", file=sys.stderr)
        return 1
    Path(out).mkdir(parents=True, exist_ok=True)
    caches = registry_caches()
    # Built in a temporary directory beside the target and renamed into place only when complete, so a failure leaves
    # nothing behind and a retry (after cargo fetch) can run.
    with tempfile.TemporaryDirectory(dir=out, prefix=".mpl-sources-") as tmp:
        base = Path(tmp) / SUB
        dest = base / "mpl-2.0"
        dest.mkdir(parents=True)
        for name, version, checksum in rows:
            file = f"{name}-{version}.crate"
            hits = [c / file for c in caches if (c / file).is_file() and sha256(c / file) == checksum]
            if not hits:
                print(f"ERROR: {file} with SHA-256 {checksum} is not in the Cargo registry cache (run cargo fetch)", file=sys.stderr)
                return 1
            shutil.copyfile(hits[0], dest / file)
        (base / "README.md").write_text(readme(rows), encoding="utf-8")
        (base / "MPL-2.0.txt").write_text(mpl_text(), encoding="utf-8")
        base.rename(final)
    dest = final / "mpl-2.0"
    print(f"OK: {len(rows)} MPL-2.0 crate archives in {dest}")
    return 0


def verify(out):
    rows, bad = table()
    base = Path(out) / SUB
    dest = base / "mpl-2.0"
    want = {f"{n}-{v}.crate": c for n, v, c in rows}
    have = {p.name for p in dest.glob("*")} if dest.is_dir() else set()
    for file, checksum in want.items():
        if file not in have:
            bad.append(f"missing {SUB}/mpl-2.0/{file}")
        elif not (dest / file).is_file() or sha256(dest / file) != checksum:
            bad.append(f"{file}: SHA-256 differs from Cargo.lock")
    bad += [f"unexpected {SUB}/mpl-2.0/{f}" for f in sorted(have - set(want))]
    if base.exists() and not dest.is_dir():
        bad.append(f"{SUB}/mpl-2.0 is not a directory")
    readme_path = base / "README.md"
    if not readme_path.is_file() or readme_path.read_text(encoding="utf-8") != readme(rows):
        bad.append(f"{SUB}/README.md is missing or not the generated text")
    lic = base / "MPL-2.0.txt"
    if not lic.is_file() or lic.read_text(encoding="utf-8") != mpl_text():
        bad.append(f"{SUB}/MPL-2.0.txt is missing or not the MPL-2.0 text")
    print("\n".join(f"ERROR: {b}" for b in bad) or f"OK: {len(want)} MPL-2.0 crate archives match Cargo.lock")
    return 1 if bad else 0


def self_test():
    failures = []
    with tempfile.TemporaryDirectory() as d:
        if write(d) != 0 or verify(d) != 0:
            failures.append("write then verify did not pass")
        dest = Path(d) / SUB / "mpl-2.0"
        first = sorted(dest.glob("*.crate"))[0]
        data = first.read_bytes()
        first.write_bytes(data + b"x")
        if verify(d) != 1:
            failures.append("a changed archive passed")
        first.unlink()
        if verify(d) != 1:
            failures.append("a missing archive passed")
        first.write_bytes(data)
        (dest / "extra-0.1.0.crate").write_bytes(b"")
        if verify(d) != 1:
            failures.append("an extra archive passed")
        (dest / "extra-0.1.0.crate").unlink()
        (Path(d) / SUB / "MPL-2.0.txt").write_text("other", encoding="utf-8")
        if verify(d) != 1:
            failures.append("a wrong license text passed")
        if write(d) != 1:
            failures.append("write into an existing directory did not fail")
        shutil.rmtree(Path(d) / SUB)
        first.unlink(missing_ok=True)
    with tempfile.TemporaryDirectory() as d:
        if write(d) != 0:
            failures.append("second write did not pass")
        dest = Path(d) / SUB / "mpl-2.0"
        first = sorted(dest.glob("*.crate"))[0]
        first.unlink()
        first.mkdir()
        if verify(d) != 1:
            failures.append("a directory in place of an archive passed")
    with tempfile.TemporaryDirectory() as d, tempfile.TemporaryDirectory() as empty:
        old_home = os.environ.get("CARGO_HOME")
        os.environ["CARGO_HOME"] = empty
        try:
            if write(d) != 1:
                failures.append("write without the registry cache did not fail")
        finally:
            if old_home is None:
                del os.environ["CARGO_HOME"]
            else:
                os.environ["CARGO_HOME"] = old_home
        if any(Path(d).iterdir()):
            failures.append("a failed write left files behind")
    print("\n".join(failures) or "self-test OK")
    return 1 if failures else 0


def main(argv):
    if argv[:1] == ["--list"]:
        rows, bad = table()
        for n, v, c in rows:
            print(f"{n}\t{v}\t{c}")
        print("\n".join(f"ERROR: {b}" for b in bad), file=sys.stderr)
        return 1 if bad else 0
    if len(argv) == 2 and argv[0] == "--write":
        return write(argv[1])
    if len(argv) == 2 and argv[0] == "--verify":
        return verify(argv[1])
    if argv == ["--self-test"]:
        return self_test()
    print(__doc__)
    return 2


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
