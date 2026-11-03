#!/usr/bin/env python3
"""vendor/rav1d is the upstream crate plus the one recorded source change:
    python3 -I tools/check_vendored_rav1d.py [path/to/rav1d-1.1.0.crate]

Without an argument it downloads the crate from static.crates.io. Either way the file must have the recorded sha256.
Then every file of the crate, except the four it ships for upstream's own CI (.github, .gitlab-ci.yml, .gitignore,
.cargo-ok), must be byte-identical in vendor/rav1d, except src/decode.rs, which must be identical once the patch is
reversed; and the directory must hold nothing else (except VENDOR.md). Reads nothing from the archives or the
evidence stores."""
import hashlib
import io
import sys
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VENDOR = ROOT / "vendor" / "rav1d"
NAME = "rav1d-1.1.0"
SHA256 = "1932f060d5e7bd49dc9f8b272c1dc5e9ce0ffe141c28be900265d3989b36c9ed"
URL = f"https://static.crates.io/crates/rav1d/{NAME}.crate"
OMITTED = (".github/", ".gitlab-ci.yml", ".gitignore", ".cargo-ok")
OURS = ("VENDOR.md",)

OLD = "        if f.frame_hdr.as_ref().unwrap().refresh_context != 0 {\n            let _ = mem::take(&mut f.out_cdf);\n        }"
NEW = (
    "        // x3d patch (vendor/rav1d/VENDOR.md): `rav1d_decode_frame` has already cleared `f.frame_hdr` when it fails (its\n"
    "        // exit path), so the unwrap that was here panicked, inside an `extern \"C\"` function, which aborts the process.\n"
    "        if f.frame_hdr.as_ref().map_or(true, |h| h.refresh_context != 0) {\n"
    "            let _ = mem::take(&mut f.out_cdf);\n"
    "        }"
)


def load_crate(argv):
    data = Path(argv[0]).read_bytes() if argv else urllib.request.urlopen(URL, timeout=120).read()
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
    bad = []
    mine = {p.relative_to(vendor).as_posix(): p.read_bytes() for p in vendor.rglob("*") if p.is_file()}
    want = {k for k in files if not k.startswith(OMITTED)}
    for k in sorted(want):
        if k not in mine:
            bad.append(f"{k}: missing")
        elif k == "src/decode.rs":
            if NEW.encode() not in mine[k] or mine[k].replace(NEW.encode(), OLD.encode(), 1) != files[k]:
                bad.append("src/decode.rs: differs from the crate by more than the recorded patch")
        elif mine[k] != files[k]:
            bad.append(f"{k}: differs from the crate")
    for k in sorted(set(mine) - want - set(OURS)):
        bad.append(f"{k}: not in the crate")
    if "VENDOR.md" not in mine:
        bad.append("VENDOR.md: missing")
    return bad


def main(argv):
    bad = compare(load_crate(argv), VENDOR)
    for b in bad:
        print("FAIL", b)
    if bad:
        return 1
    print(f"vendor/rav1d is {NAME} plus the recorded patch")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
