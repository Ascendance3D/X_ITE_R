#!/usr/bin/env python3
"""vendor/web-audio-api is the upstream crate plus the recorded Cargo.toml change:
    python3 -I tools/check_vendored_web_audio_api.py [path/to/web-audio-api-1.7.0.crate]

Without an argument it downloads the crate from static.crates.io. Either way the file must have the sha256 that Cargo.lock
recorded before the patch. Then every file under src/ and resources/, LICENSE and README.md must be byte-identical to the
crate, the vendored tree must hold nothing else (except VENDOR.md and Cargo.toml), and Cargo.toml must differ from the
crate's only in the three ways VENDOR.md lists. Reads nothing from the archives or the evidence stores."""
import hashlib
import io
import re
import sys
import tarfile
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


def main(argv):
    data = Path(argv[0]).read_bytes() if argv else urllib.request.urlopen(URL, timeout=60).read()
    got = hashlib.sha256(data).hexdigest()
    if got != SHA256:
        print(f"crate sha256 {got} is not the recorded {SHA256}")
        return 1
    files = {}
    with tarfile.open(fileobj=io.BytesIO(data), mode="r:gz") as t:
        for m in t.getmembers():
            if m.isfile():
                files[m.name.split("/", 1)[1]] = t.extractfile(m).read()
    bad = []
    mine = {p.relative_to(VENDOR).as_posix(): p.read_bytes() for p in VENDOR.rglob("*") if p.is_file()}
    want = {k for k in files if k in SAME or k.split("/")[0] in SAME_DIRS}
    for k in sorted(want):
        if mine.get(k) != files[k]:
            bad.append(f"{k}: " + ("missing" if k not in mine else "differs from the crate"))
    for k in sorted(set(mine) - want - set(OURS)):
        bad.append(f"{k}: not in the crate")
    if mine.get("Cargo.toml") != expected_manifest(files["Cargo.toml"].decode()).encode():
        bad.append("Cargo.toml: differs from the crate by more than the three recorded changes")
    print("\n".join(bad) or f"OK: {len(want)} files identical to {NAME}, Cargo.toml = crate + the recorded change")
    return 1 if bad else 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
