#!/usr/bin/env python3
"""No AAC decoder in the resolved dependency graph: python3 -I tools/check_no_aac.py [Cargo.lock]

Reads Cargo.lock (which lists every package any build of this workspace can use, native and wasm32, release and test) and
fails when a package that decodes AAC is in it: symphonia-codec-aac, fdk-aac and the like (any package whose name has an
"aac" word). Owner direction 2026-11-02, issue #90: the Chromium AAC decode is recorded as unsupported, and the decoder is
not compiled into any build. A code path that refuses AAC is not enough; the package must be absent.

Also checks that Cargo.lock agrees with `cargo tree`'s view when cargo is on PATH and `--cargo` is given:
    python3 -I tools/check_no_aac.py --cargo
runs `cargo tree --locked -e normal,build,dev --target all -i symphonia-codec-aac@<version>` for each version that
would have been present (0.5.5, 0.6.1) and requires "did not match any packages"."""
import re
import subprocess
import sys
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
AAC_NAME = re.compile(r"(^|[-_])aac([-_]|$)")
VERSIONS = ("0.5.5", "0.6.1")


def packages(lock_text):
    return re.findall(r'\[\[package\]\]\nname = "([^"]+)"\nversion = "([^"]+)"', lock_text)


def aac_packages(lock_text):
    return [(n, v) for n, v in packages(lock_text) if AAC_NAME.search(n)]


def main(argv):
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
