#!/usr/bin/env python3
"""vendor/web-audio-api is the upstream crate plus the recorded Cargo.toml change:
    python3 -I tools/check_vendored_web_audio_api.py [--self-test] [path/to/web-audio-api-1.7.0.crate]

Without an argument it downloads the crate from static.crates.io. Either way the file must have the sha256 that Cargo.lock
recorded before the patch. Then every file under src/ and resources/, LICENSE and README.md must be byte-identical to the
crate, the vendored tree must hold nothing else (except VENDOR.md and Cargo.toml), and Cargo.toml must differ from the
crate's only in the three ways VENDOR.md lists. Reads nothing from the archives or the evidence stores."""
import hashlib
import io
import re
import shutil
import sys
import tarfile
import tempfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VENDOR = ROOT / "vendor" / "web-audio-api"
NAME = "web-audio-api-1.7.0"
SHA256 = "013903031fb735e7fd9aee7c159f3440a19c6969dc81365aec8c64caddcf39bc"
URL = f"https://static.crates.io/crates/web-audio-api/{NAME}.crate"
SAME = ("LICENSE", "README.md")
SAME_DIRS = ("src", "resources")
OURS = ("Cargo.toml", "VENDOR.md")


def expected_manifest(upstream):
    """The three changes of VENDOR.md, applied to the upstream manifest."""
    s = re.sub(r"\[dev-dependencies\.[^\]]+\]\n(?:(?!\[).*\n)*\n?", "", upstream)
    for old, new in (('    "decode-all",\n', "".join(f'    "{x}",\n' for x in (
                          "decode-alac", "decode-flac", "decode-mp3", "decode-pcm", "decode-isomp4", "decode-ogg",
                          "decode-vorbis", "decode-wav"))),
                     ('    "all",\n', "".join(f'    "{x}",\n' for x in (
                          "adpcm", "alac", "flac", "mp1", "mp2", "mp3", "pcm", "vorbis", "caf", "isomp4", "mkv", "ogg",
                          "aiff", "wav", "all-meta")))):
        if s.count(old) != 1:
            raise SystemExit(f"upstream manifest has no unique {old.strip()}")
        s = s.replace(old, new)
    return s


def load_crate(argv):
    data = Path(argv[0]).read_bytes() if argv else urllib.request.urlopen(URL, timeout=60).read()
    got = hashlib.sha256(data).hexdigest()
    if got != SHA256:
        raise SystemExit(f"crate sha256 {got} is not the recorded {SHA256}")
    files = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as t:
        for m in t.getmembers():
            if m.isfile():
                files[m.name.split("/", 1)[1]] = t.extractfile(m).read()
    return files


def compare(files, vendor):
    """The list of differences between the crate (name -> bytes) and the vendored directory."""
    bad = []
    mine = {p.relative_to(vendor).as_posix(): p.read_bytes() for p in vendor.rglob("*") if p.is_file()}
    want = {k for k in files if k in SAME or k.split("/")[0] in SAME_DIRS}
    for k in sorted(want):
        if mine.get(k) != files[k]:
            bad.append(f"{k}: " + ("missing" if k not in mine else "differs from the crate"))
    for k in sorted(set(mine) - want - set(OURS)):
        bad.append(f"{k}: not in the crate")
    if mine.get("Cargo.toml") != expected_manifest(files["Cargo.toml"].decode()).encode():
        bad.append("Cargo.toml: differs from the crate by more than the three recorded changes")
    if "VENDOR.md" not in mine:
        bad.append("VENDOR.md: missing")
    return bad, len(want)


def self_test(files):
    """The comparison must fail for each kind of unexpected change, on a temporary copy of the vendored tree."""
    cases = {
        "a changed source byte": lambda d: (d / "src/lib.rs").write_bytes((d / "src/lib.rs").read_bytes() + b" "),
        "an added file": lambda d: (d / "src/extra.rs").write_text("pub fn x() {}\n"),
        "a removed file": lambda d: (d / "LICENSE").unlink(),
        "a changed license": lambda d: (d / "LICENSE").write_bytes(b"MIT\n"),
        "an extra manifest change": lambda d: (d / "Cargo.toml").write_text((d / "Cargo.toml").read_text() + "\n# x\n"),
        "a changed resource": lambda d: (d / "resources/IRC_1003_C.bin").write_bytes(b"x"),
    }
    failed = 0
    for name, change in cases.items():
        with tempfile.TemporaryDirectory() as tmp:
            d = Path(tmp) / "v"
            shutil.copytree(VENDOR, d)
            change(d)
            bad, _ = compare(files, d)
            print(("ok   " if bad else "FAIL ") + f"{name}: " + (bad[0] if bad else "not detected"))
            failed += not bad
    return failed


def main(argv):
    args = [a for a in argv if a != "--self-test"]
    files = load_crate(args)
    bad, n = compare(files, VENDOR)
    print("\n".join(bad) or f"OK: {n} files identical to {NAME}, Cargo.toml = crate + the recorded change")
    if "--self-test" in argv:
        bad = bad or ["self-test failed"] * self_test(files)
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
