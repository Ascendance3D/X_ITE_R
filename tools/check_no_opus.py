#!/usr/bin/env python3
"""No Opus decoder in any build: python3 -I tools/check_no_opus.py [Cargo.lock] [--cargo] | --binary FILE... | --self-test

Opus is not decodable natively (rejected; docs/codec-licensing-inventory.md, Audio table). Item 3 of that file's release
list asks for an exclusion check for every excluded codec, as tools/check_no_aac.py is for AAC. A code path that refuses
Opus is not enough; no Opus decoder package may be in the build.

    (default)    Cargo.lock, which lists every package any build of this workspace can use (native and wasm32, release
                 and test), must hold no Opus package: a name with an "opus" word, or libopus/audiopus forms
                 (symphonia-codec-opus, opus, opus-sys, libopus-sys, audiopus, audiopus_sys, magnum-opus, ogg-opus).
    --cargo      also `cargo metadata --locked --offline` (the graph Cargo resolves) must hold no such package.
    --binary     a SUPPLEMENTAL smoke check on built artifacts, not proof of absence (stripping, LTO or renamed symbols can
                 hide strings; the lockfile and --cargo checks are the authoritative ones). The crate path strings of the
                 Opus decoders must not appear, and the crate path of a codec that stays (symphonia-bundle-mp3) must, so a
                 build without embedded crate paths cannot pass by saying nothing.
    --self-test  the name rule and the binary check on synthetic input.
"""
import json
import re
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
# "opus" as a word, or glued to "lib"/"audi" (libopus-sys, audiopus); not inside another word ("corpus").
OPUS_NAME = re.compile(r"(^|[-_]|lib|audi)opus([-_]|$|ic)")
FORBIDDEN = (b"symphonia-codec-opus", b"symphonia_codec_opus", b"audiopus", b"libopus", b"magnum-opus", b"magnum_opus",
             b"opus-sys", b"opus_sys", b"ogg-opus", b"ogg_opus")
CONTROL = (b"symphonia-bundle-mp3", b"symphonia_bundle_mp3")


def packages(lock_text):
    """(name, version) of every [[package]] block, whatever order Cargo writes the keys in."""
    found = []
    for block in lock_text.split("[[package]]")[1:]:
        name = re.search(r'^name\s*=\s*"([^"]+)"', block, re.MULTILINE)
        version = re.search(r'^version\s*=\s*"([^"]+)"', block, re.MULTILINE)
        if name:
            found.append((name.group(1), version.group(1) if version else ""))
    return found


def opus_names(names):
    return [n for n in names if OPUS_NAME.search(n)]


def check_lock(lock):
    all_packages = packages(lock.read_text())
    bad = [f"{n} is in {lock.name}" for n in opus_names([n for n, _ in all_packages])]
    if not all_packages:
        bad.append(f"{lock}: no [[package]] blocks read")
    print("\n".join(bad) or f"OK: no Opus decoder among {len(all_packages)} packages in {lock.name}")
    return 1 if bad else 0


def check_cargo():
    r = subprocess.run(["cargo", "metadata", "--locked", "--offline", "--format-version", "1"],
                       cwd=ROOT, capture_output=True, text=True)
    if r.returncode != 0:
        print(f"cargo metadata failed: {r.stderr.strip()[:300]}")
        return 1
    names = [p["name"] for p in json.loads(r.stdout)["packages"]]
    bad = opus_names(names)
    print("\n".join(f"{n} is in the cargo metadata graph" for n in bad) or f"OK: no Opus decoder among {len(names)} packages (cargo metadata)")
    return 1 if bad else 0


def check_binaries(paths):
    if not paths:
        print("ERROR: no artifact paths given (an unmatched glob would otherwise pass)")
        return 1
    bad = []
    for path in paths:
        if not Path(path).is_file():
            bad.append(f"{path}: not a file")
            continue
        data = Path(path).read_bytes()
        if not any(c in data for c in CONTROL):
            bad.append(f"{path}: no MP3 decoder path string (control): this check says nothing about the build")
        for needle in FORBIDDEN:
            if needle in data:
                bad.append(f"{path}: contains {needle.decode()}")
    print("\n".join(bad) or f"OK: {len(paths)} artifact(s) hold the control string and no Opus decoder path")
    return 1 if bad else 0


def self_test():
    failures = []
    for name in ("opus", "opus-sys", "symphonia-codec-opus", "libopus-sys", "audiopus", "audiopus_sys", "magnum-opus",
                 "ogg-opus", "opusic-sys"):
        if not OPUS_NAME.search(name):
            failures.append(f"name rule misses {name}")
    for name in ("corpus", "corpus-items", "symphonia-bundle-mp3", "symphonia-codec-vorbis", "octopus-tree", "opusculum"):
        if OPUS_NAME.search(name):
            failures.append(f"name rule matches {name}")
    with tempfile.TemporaryDirectory() as d:
        cases = {
            "clean": (b"..." + CONTROL[0] + b"...", 0),
            "has-symphonia-opus": (b"x " + CONTROL[0] + b" symphonia-codec-opus-0.6.1/src/lib.rs", 1),
            "has-audiopus": (b"x " + CONTROL[1] + b" audiopus_sys-0.2.2/src/lib.rs", 1),
            "has-libopus": (b"x " + CONTROL[0] + b" libopus 1.4", 1),
            "no-control": (b"nothing here", 1),
        }
        for name, (data, want) in cases.items():
            f = Path(d) / name
            f.write_bytes(data)
            if check_binaries([str(f)]) != want:
                failures.append(f"binary case {name}: wrong exit")
        if check_binaries([]) != 1:
            failures.append("empty path list did not fail")
        if check_binaries([str(Path(d) / "missing")]) != 1:
            failures.append("missing file did not fail")
        lock = Path(d) / "Cargo.lock"
        lock.write_text('[[package]]\nname = "symphonia-codec-opus"\nversion = "0.6.1"\n')
        if check_lock(lock) != 1:
            failures.append("lockfile with an Opus package did not fail")
        lock.write_text('[[package]]\nversion = "0.6.1"\nname = "symphonia-bundle-mp3"\n')
        if check_lock(lock) != 0:
            failures.append("clean lockfile failed")
        lock.write_text("")
        if check_lock(lock) != 1:
            failures.append("empty lockfile did not fail")
    print("\n".join(failures) or "self-test OK")
    return 1 if failures else 0


def main(argv):
    if "--self-test" in argv:
        return self_test()
    if "--binary" in argv:
        return check_binaries([a for a in argv if not a.startswith("--")])
    args = [a for a in argv if not a.startswith("--")]
    status = check_lock(Path(args[0]) if args else ROOT / "Cargo.lock")
    if "--cargo" in argv:
        status |= check_cargo()
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
