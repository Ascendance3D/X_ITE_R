#!/usr/bin/env python3
"""No AAC decoder in the resolved dependency graph: python3 -I tools/check_no_aac.py [Cargo.lock]

Reads Cargo.lock (which lists every package any build of this workspace can use, native and wasm32, release and test) and
fails when a package that decodes AAC is in it: symphonia-codec-aac, fdk-aac and the like (any package whose name has an
"aac" word). Owner direction 2026-11-02, issue #90: the Chromium AAC decode is recorded as unsupported, and the decoder is
not compiled into any build. A code path that refuses AAC is not enough; the package must be absent.

Also checks that Cargo.lock agrees with `cargo tree`'s view when cargo is on PATH and `--cargo` is given:
    python3 -I tools/check_no_aac.py --cargo
runs `cargo tree --locked -e normal,build,dev --target all -i symphonia-codec-aac@<version>` for each version that
would have been present (0.5.5, 0.6.1) and requires "did not match any packages".

    python3 -I tools/check_no_aac.py --binary <built file>...
checks built artifacts (a release executable, the wasm module or the wasm-bindgen output): the crate paths of the AAC decoder
must not appear in the bytes, and the crate path of the MP3 decoder (symphonia-bundle-mp3, a codec that stays) must, so a
build without embedded crate paths cannot pass by saying nothing."""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
AAC_NAME = re.compile(r"(^|[-_])aac([-_]|$)")
VERSIONS = ("0.5.5", "0.6.1")


def packages(lock_text):
    """(name, version) of every [[package]] block, whatever order Cargo writes the keys in."""
    found = []
    for block in lock_text.split("[[package]]")[1:]:
        name = re.search(r'^name\s*=\s*"([^"]+)"', block, re.MULTILINE)
        version = re.search(r'^version\s*=\s*"([^"]+)"', block, re.MULTILINE)
        if name:
            found.append((name.group(1), version.group(1) if version else ""))
    return found


def aac_packages(lock_text):
    return [(n, v) for n, v in packages(lock_text) if AAC_NAME.search(n)]


def check_binaries(paths):
    if not paths:
        print("ERROR: no artifact paths given (an unmatched glob would otherwise pass)", file=sys.stderr)
        return 1
    bad = []
    for path in paths:
        if not Path(path).is_file():
            bad.append(f"{path}: not a file")
            continue
        data = Path(path).read_bytes()
        if b"symphonia-bundle-mp3" not in data and b"symphonia_bundle_mp3" not in data:
            bad.append(f"{path}: no MP3 decoder path string (control): this check says nothing about the build")
        for needle in (b"symphonia-codec-aac", b"symphonia_codec_aac"):
            if needle in data:
                bad.append(f"{path}: contains {needle.decode()}")
    print("\n".join(bad) or f"OK: {len(paths)} artifact(s) hold the control string and no AAC decoder path")
    return 1 if bad else 0


def main(argv):
    if "--binary" in argv:
        return check_binaries([a for a in argv if not a.startswith("--")])
    args = [a for a in argv if not a.startswith("--")]
    lock = Path(args[0]) if args else ROOT / "Cargo.lock"
    found = aac_packages(lock.read_text())
    bad = [f"{n} {v} is in {lock.name}" for n, v in found]
    if "--cargo" in argv:
        for v in VERSIONS:
            r = subprocess.run(
                ["cargo", "tree", "--locked", "--offline", "-e", "normal,build,dev", "--target", "all", "-i", f"symphonia-codec-aac@{v}"],
                cwd=ROOT, capture_output=True, text=True)
            if "did not match any packages" not in r.stderr:
                bad.append(f"symphonia-codec-aac@{v}: cargo tree did not say 'did not match any packages': {r.stderr.strip()[:200]}")
    print("\n".join(bad) or f"OK: no AAC decoder among {len(packages(lock.read_text()))} packages")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
