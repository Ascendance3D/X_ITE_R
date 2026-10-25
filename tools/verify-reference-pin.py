#!/usr/bin/env python3
"""Verify the inert X_ITE 16.6.0 reference pin against its recorded identity.

`oracle/x_ite-pin-16.6.0/` is an immutable identity record: 846 upstream `src/`
blobs, their per-file manifest, and the metadata that names which upstream commit
they came from. Nothing in the build reads it, so nothing in the build notices if
one of its bytes changes. This checker is that notice.

It proves four things, and refuses on any of them:

  * every manifest entry exists under `src/` with the manifest's own SHA-256;
  * `src/` holds exactly those files and no others;
  * the tree digest `tools/reference-digest.py` computes is the recorded one;
  * the identity metadata on disk is the identity this file expects.

`EXPECTED` below is the second witness. The metadata files are the first. Editing
either one alone makes the two disagree and the check fail, which is the point: a
single-sided edit cannot quietly re-bless the pin.

Pure Python, hashlib only: no GNU `sha256sum` (the manifest's `sha256sum -c` form
is still what the README documents for a human, but it is not what gates this).

Usage:
  verify-reference-pin.py [--root <pin dir>] [--quiet]
  verify-reference-pin.py --self-test   # prove the checker refuses a tampered pin

Exit codes:
  0  the pin matches its recorded identity
  1  it does not (every mismatch is reported)
  2  usage error
  3  the pin directory or a metadata file is missing
"""
import hashlib
import os
import re
import shutil
import subprocess
import sys
import tempfile

HERE = os.path.dirname(os.path.abspath(__file__))
DEFAULT_ROOT = os.path.join(os.path.dirname(HERE), "oracle", "x_ite-pin-16.6.0")

# What the pin is. Checked against the metadata files in the pin itself, which hold
# the same facts independently.
EXPECTED = {
    "upstream": "create3000/x_ite",
    "source_sha": "5f282709c124ffa4e2bbd57d9b199893bf94bd32",
    # PINNED_COMMIT records the short form, matching the active 16.5.0 pin's convention.
    "pinned_commit": "5f282709c",
    "package_version": "16.6.0",
    "browser_version": "16.6.0",
    "manifest_sha256": "ef02ce267103193df166cff0e6f1875c9a50c0ff5d595aa629028a56e0185542",
    "src_files": 846,
    "src_digest": "1972df1f225b620acf979cde1fac1da29f11cda05b7894a10cc4832fc5960891",
}


def sha256_file(path):
    digest = hashlib.sha256()
    with open(path, "rb") as handle:
        for chunk in iter(lambda: handle.read(1 << 20), b""):
            digest.update(chunk)
    return digest.hexdigest()


def read_text(path, problems):
    """Read a metadata file, or record its absence and return None."""
    try:
        with open(path, "r", encoding="utf-8") as handle:
            return handle.read()
    except OSError as error:
        problems.append("cannot read %s: %s" % (path, error))
        return None


def parse_manifest(path, problems):
    """Return {relative path: sha256} from a `sha256sum` manifest."""
    text = read_text(path, problems)
    if text is None:
        return None
    entries = {}
    for number, line in enumerate(text.splitlines(), 1):
        if not line.strip():
            continue
        # `sha256sum` writes "<64 hex><two spaces><path>"; the second space is
        # "binary mode" in GNU's format and may be "*" instead.
        match = re.match(r"^([0-9a-f]{64})[ ][ *](.+)$", line)
        if not match:
            problems.append("%s:%d: not a sha256sum line: %r" % (path, number, line))
            continue
        digest, name = match.group(1), match.group(2)
        if name in entries:
            problems.append("%s:%d: duplicate entry: %s" % (path, number, name))
        entries[name] = digest
    return entries


def walk_src(src):
    """Return every regular file under `src`, as forward-slash relative paths."""
    found = set()
    for dirpath, dirnames, filenames in os.walk(src):
        dirnames[:] = sorted(d for d in dirnames if d != ".git")
        for name in sorted(filenames):
            path = os.path.join(dirpath, name)
            if os.path.islink(path) or not os.path.isfile(path):
                continue
            found.add(os.path.relpath(path, src).replace(os.sep, "/"))
    return found


def tree_digest(src, names, shas):
    """The digest `tools/reference-digest.py` computes, from hashes already taken.

    Same construction: sorted relative path, NUL, hex digest, newline. Keeping it
    here rather than shelling out means one pass over 15 MB instead of two.
    """
    overall = hashlib.sha256()
    for rel in sorted(names):
        overall.update(rel.encode("utf-8"))
        overall.update(b"\0")
        overall.update(shas[rel].encode("ascii"))
        overall.update(b"\n")
    return overall.hexdigest()


def verify(root, quiet=False):
    """Return a list of problems; empty means the pin is intact."""
    problems = []
    if not os.path.isdir(root):
        return ["not a directory: %s" % root]

    src = os.path.join(root, "src")
    if not os.path.isdir(src):
        return ["the pin has no src/ tree: %s" % src]

    # 1. The manifest's own bytes.
    manifest_path = os.path.join(root, "MANIFEST.sha256")
    if os.path.isfile(manifest_path):
        got = sha256_file(manifest_path)
        if got != EXPECTED["manifest_sha256"]:
            problems.append(
                "MANIFEST.sha256 is %s, expected %s" % (got, EXPECTED["manifest_sha256"])
            )
    manifest = parse_manifest(manifest_path, problems)
    if manifest is None:
        return problems

    if len(manifest) != EXPECTED["src_files"]:
        problems.append(
            "MANIFEST.sha256 has %d entries, expected %d"
            % (len(manifest), EXPECTED["src_files"])
        )

    # 2. Every manifest entry, by content. Hash once and reuse for the tree digest.
    shas = {}
    for rel in sorted(manifest):
        path = os.path.join(src, *rel.split("/"))
        if not os.path.isfile(path):
            problems.append("missing from src/: %s" % rel)
            continue
        shas[rel] = sha256_file(path)
        if shas[rel] != manifest[rel]:
            problems.append(
                "content changed: %s is %s, manifest says %s"
                % (rel, shas[rel], manifest[rel])
            )

    # 3. Nothing extra. An added file leaves the manifest passing but the tree wrong.
    for rel in sorted(walk_src(src) - set(manifest)):
        problems.append("not in the manifest: src/%s" % rel)

    # 4. The recorded tree digest, when every file was present to hash.
    if len(shas) == len(manifest):
        got = tree_digest(src, manifest.keys(), shas)
        if got != EXPECTED["src_digest"]:
            problems.append(
                "src/ tree digest is %d %s, expected %d %s"
                % (len(shas), got, EXPECTED["src_files"], EXPECTED["src_digest"])
            )

    # 5. Identity metadata.
    source_sha = read_text(os.path.join(root, "SOURCE_SHA"), problems)
    if source_sha is not None:
        source_sha = source_sha.strip()
        if source_sha != EXPECTED["source_sha"]:
            problems.append(
                "SOURCE_SHA is %r, expected %r" % (source_sha, EXPECTED["source_sha"])
            )

    pinned = read_text(os.path.join(root, "PINNED_COMMIT"), problems)
    if pinned is not None:
        pinned = pinned.strip()
        if pinned != EXPECTED["pinned_commit"]:
            problems.append(
                "PINNED_COMMIT is %r, expected %r" % (pinned, EXPECTED["pinned_commit"])
            )
        elif not EXPECTED["source_sha"].startswith(pinned):
            problems.append(
                "PINNED_COMMIT %r is not a prefix of SOURCE_SHA %r"
                % (pinned, EXPECTED["source_sha"])
            )

    package = read_text(os.path.join(root, "package.json"), problems)
    if package is not None:
        match = re.search(r'"version"\s*:\s*"([^"]+)"', package)
        if not match:
            problems.append("package.json has no \"version\"")
        elif match.group(1) != EXPECTED["package_version"]:
            problems.append(
                "package.json version is %r, expected %r"
                % (match.group(1), EXPECTED["package_version"])
            )

    # The pin carries BROWSER_VERSION.js twice: upstream's root copy and the one
    # inside src/. Both have to name the pinned version.
    for rel in ("BROWSER_VERSION.js", os.path.join("src", "x_ite", "BROWSER_VERSION.js")):
        text = read_text(os.path.join(root, rel), problems)
        if text is None:
            continue
        match = re.search(r'"([^"]+)"', text)
        if not match:
            problems.append("%s exports no version string" % rel)
        elif match.group(1) != EXPECTED["browser_version"]:
            problems.append(
                "%s is %r, expected %r"
                % (rel, match.group(1), EXPECTED["browser_version"])
            )

    if not problems and not quiet:
        print(
            "x_ite-pin-16.6.0: %d src/ files verified, digest %s, %s @ %s (%s)"
            % (
                EXPECTED["src_files"],
                EXPECTED["src_digest"],
                EXPECTED["upstream"],
                EXPECTED["source_sha"],
                EXPECTED["package_version"],
            )
        )
    return problems


def self_test(root):
    """Prove the checker refuses a tampered pin, on a throwaway copy.

    A verifier that cannot fail is not a verifier. Each case tampers with the copy,
    requires a refusal naming the right reason, then restores it and requires a pass
    again — so a case that "fails" because the copy stayed broken cannot pass.
    """
    if verify(root, quiet=True):
        sys.stderr.write("self-test: the real pin does not verify; fix that first\n")
        return 1

    with tempfile.TemporaryDirectory(prefix="verify-reference-pin-") as temp:
        copy = os.path.join(temp, "x_ite-pin-16.6.0")
        shutil.copytree(root, copy, symlinks=True)

        if verify(copy, quiet=True):
            sys.stderr.write("self-test: the copy does not verify\n")
            return 1

        victim = os.path.join(copy, "src", "x_ite", "Browser", "X3DBrowser.js")
        manifest = os.path.join(copy, "MANIFEST.sha256")
        pinned = os.path.join(copy, "PINNED_COMMIT")
        source = os.path.join(copy, "SOURCE_SHA")
        failures = 0

        def case(name, mutate, restore, expect):
            nonlocal failures
            mutate()
            problems = verify(copy, quiet=True)
            if not any(expect in p for p in problems):
                sys.stderr.write(
                    "self-test FAILED: %s did not refuse with %r; got %r\n"
                    % (name, expect, problems)
                )
                failures += 1
            else:
                print("self-test ok: %s -> %s" % (name, problems[0]))
            restore()
            left = verify(copy, quiet=True)
            if left:
                sys.stderr.write(
                    "self-test FAILED: %s did not restore cleanly: %r\n" % (name, left)
                )
                failures += 1

        original = open(victim, "rb").read()
        case(
            "a pinned source file changes",
            lambda: open(victim, "ab").write(b"\n// tampered\n"),
            lambda: open(victim, "wb").write(original),
            "content changed: x_ite/Browser/X3DBrowser.js",
        )
        case(
            "a pinned source file is removed",
            lambda: os.remove(victim),
            lambda: open(victim, "wb").write(original),
            "missing from src/: x_ite/Browser/X3DBrowser.js",
        )
        extra = os.path.join(copy, "src", "x_ite", "intruder.js")
        case(
            "an unlisted file is added",
            lambda: open(extra, "wb").write(b"export default 0;\n"),
            lambda: os.remove(extra),
            "not in the manifest: src/x_ite/intruder.js",
        )
        manifest_bytes = open(manifest, "rb").read()
        case(
            "the manifest is edited",
            lambda: open(manifest, "ab").write(
                b"0" * 64 + b"  x_ite/nowhere.js\n"
            ),
            lambda: open(manifest, "wb").write(manifest_bytes),
            "MANIFEST.sha256 is",
        )
        pinned_bytes = open(pinned, "rb").read()
        case(
            "the expected identity is edited (PINNED_COMMIT)",
            lambda: open(pinned, "wb").write(b"deadbeef1\n"),
            lambda: open(pinned, "wb").write(pinned_bytes),
            "PINNED_COMMIT is",
        )
        source_bytes = open(source, "rb").read()
        case(
            "the expected identity is edited (SOURCE_SHA)",
            lambda: open(source, "wb").write(b"0" * 40 + b"\n"),
            lambda: open(source, "wb").write(source_bytes),
            "SOURCE_SHA is",
        )
        version = os.path.join(copy, "BROWSER_VERSION.js")
        version_bytes = open(version, "rb").read()
        case(
            "the expected identity is edited (BROWSER_VERSION.js)",
            lambda: open(version, "wb").write(b'export default "16.5.0";\n'),
            lambda: open(version, "wb").write(version_bytes),
            "BROWSER_VERSION.js is",
        )
        package = os.path.join(copy, "package.json")
        package_bytes = open(package, "rb").read()
        case(
            "the expected identity is edited (package.json)",
            lambda: open(package, "wb").write(
                package_bytes.replace(b'"version": "16.6.0"', b'"version": "16.5.0"', 1)
            ),
            lambda: open(package, "wb").write(package_bytes),
            "package.json version is",
        )

        if failures:
            sys.stderr.write("self-test: %d case(s) failed\n" % failures)
            return 1
        print("self-test: 8 cases, every tamper refused")
        return 0


def main(argv):
    root, quiet, test = DEFAULT_ROOT, False, False
    args = argv[1:]
    while args:
        arg = args.pop(0)
        if arg == "--root":
            if not args:
                sys.stderr.write("verify-reference-pin: --root needs a value\n")
                return 2
            root = args.pop(0)
        elif arg == "--quiet":
            quiet = True
        elif arg == "--self-test":
            test = True
        elif arg in ("-h", "--help"):
            sys.stdout.write(__doc__)
            return 0
        else:
            sys.stderr.write("verify-reference-pin: unknown argument: %s\n" % arg)
            return 2

    if not os.path.isdir(root):
        sys.stderr.write("verify-reference-pin: not a directory: %s\n" % root)
        return 3

    if test:
        return self_test(root)

    problems = verify(root, quiet)
    if problems:
        sys.stderr.write("verify-reference-pin: the 16.6.0 pin does not match:\n")
        for problem in problems:
            sys.stderr.write("  %s\n" % problem)
        return 1
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv))
