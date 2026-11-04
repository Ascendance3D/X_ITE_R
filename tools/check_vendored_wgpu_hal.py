#!/usr/bin/env python3
"""vendor/wgpu-hal is the upstream crate plus one recorded patch (issue #89):
    python3 -I -B tools/check_vendored_wgpu_hal.py [path/to/wgpu-hal-30.0.1.crate]
    python3 -I -B tools/check_vendored_wgpu_hal.py --self-test

Without an argument it downloads the crate from static.crates.io. Either way the file must have the recorded sha256, which
is also the checksum that Cargo.lock held for wgpu-hal before the patch (VENDOR.md). Then:

  * every file of the crate except src/vulkan/descriptor.rs must be byte-identical in vendor/wgpu-hal;
  * src/vulkan/descriptor.rs must equal the crate's file with x3d-descriptor-pool.patch applied (the patch is kept in the
    directory, visible to the review);
  * the directory must hold nothing else except VENDOR.md and x3d-descriptor-pool.patch (the allowlist below);
  * the crate's Cargo.toml must be the vendored one (checked above), the root Cargo.toml must patch wgpu-hal to this path,
    and Cargo.lock must list wgpu-hal 30.0.1 without a registry source.

Reads nothing from the archives or the evidence stores. --self-test runs the patch applier and the comparison on synthetic
data, including a tampered copy that must fail."""
import hashlib
import io
import re
import sys
import tarfile
import urllib.request
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
VENDOR = ROOT / "vendor" / "wgpu-hal"
NAME = "wgpu-hal-30.0.1"
VERSION = "30.0.1"
SHA256 = "b6b7fb58561a792bc237628ba0792e332de418fefe145f13b5ed8201e6d52f58"
URL = f"https://static.crates.io/crates/wgpu-hal/{NAME}.crate"
OMITTED = (".cargo-ok",)
PATCHED = "src/vulkan/descriptor.rs"
PATCH_FILE = "x3d-descriptor-pool.patch"
OURS = ("VENDOR.md", PATCH_FILE)


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


def apply_patch(original, patch):
    """Apply a unified diff (bytes) to bytes. Lines keep their own line endings (the crate file uses CRLF), so a hunk line
    matches only when its bytes are equal. Raises ValueError when a hunk does not fit."""
    src = original.splitlines(keepends=True)
    out, pos = [], 0
    lines = patch.splitlines(keepends=True)
    i = 0
    while i < len(lines) and not lines[i].startswith(b"@@"):
        i += 1
    while i < len(lines):
        m = re.match(rb"@@ -(\d+)(?:,(\d+))? \+(\d+)(?:,(\d+))? @@", lines[i])
        if not m:
            raise ValueError(f"bad hunk header {lines[i]!r}")
        start = int(m.group(1)) - 1 if int(m.group(2) or 1) else int(m.group(1))
        i += 1
        if start < pos:
            raise ValueError("hunks overlap or are out of order")
        out.extend(src[pos:start])
        pos = start
        while i < len(lines) and not lines[i].startswith(b"@@"):
            tag, text = lines[i][:1], lines[i][1:]
            i += 1
            if tag == b"\\":
                continue
            if tag in (b" ", b"-"):
                if pos >= len(src) or src[pos] != text:
                    raise ValueError(f"context or removed line does not match at line {pos + 1}")
                if tag == b" ":
                    out.append(text)
                pos += 1
            elif tag == b"+":
                out.append(text)
            else:
                raise ValueError(f"bad patch line {lines[i - 1]!r}")
    out.extend(src[pos:])
    return b"".join(out)


def compare(files, mine):
    """files: the crate (name -> bytes); mine: the vendor directory (name -> bytes). Returns a list of failures."""
    bad = []
    want = {k for k in files if not k.startswith(OMITTED)}
    for k in sorted(want):
        if k not in mine:
            bad.append(f"{k}: missing")
        elif k == PATCHED:
            if PATCH_FILE not in mine:
                bad.append(f"{PATCH_FILE}: missing")
                continue
            try:
                patched = apply_patch(files[k], mine[PATCH_FILE])
            except ValueError as e:
                bad.append(f"{PATCH_FILE}: does not apply to the crate's {k}: {e}")
                continue
            if mine[k] != patched:
                bad.append(f"{k}: is not the crate's file with {PATCH_FILE} applied")
            if mine[k] == files[k]:
                bad.append(f"{k}: equals the crate's file, the patch is not applied")
        elif mine[k] != files[k]:
            bad.append(f"{k}: differs from the crate")
    for k in sorted(set(mine) - want - set(OURS)):
        bad.append(f"{k}: not in the crate and not an allowed file")
    for k in OURS:
        if k not in mine:
            bad.append(f"{k}: missing")
    return bad


def check_manifests():
    bad = []
    root = (ROOT / "Cargo.toml").read_text()
    if not re.search(r'(?m)^wgpu-hal = \{ path = "vendor/wgpu-hal" \}$', root):
        bad.append('Cargo.toml: [patch.crates-io] lacks wgpu-hal = { path = "vendor/wgpu-hal" }')
    lock = (ROOT / "Cargo.lock").read_text()
    m = re.search(r'\[\[package\]\]\nname = "wgpu-hal"\nversion = "([^"]+)"\n((?:[a-z]+ = .*\n)*)', lock)
    if not m:
        bad.append("Cargo.lock: no wgpu-hal package")
    else:
        if m.group(1) != VERSION:
            bad.append(f"Cargo.lock: wgpu-hal {m.group(1)}, not {VERSION}")
        if "source =" in m.group(2) or "checksum =" in m.group(2):
            bad.append("Cargo.lock: wgpu-hal still has a registry source or checksum, so the patch is not in use")
    vt = (VENDOR / "Cargo.toml").read_text()
    if not re.search(rf'(?m)^name = "wgpu-hal"\nversion = "{re.escape(VERSION)}"$', vt):
        bad.append("vendor/wgpu-hal/Cargo.toml: not wgpu-hal " + VERSION)
    return bad


def read_vendor(vendor):
    return {p.relative_to(vendor).as_posix(): p.read_bytes() for p in vendor.rglob("*") if p.is_file()}


def self_test():
    orig = b"a\r\nb\r\nc\r\nd\r\n"
    patch = b"--- a/f\n+++ b/f\n@@ -2,2 +2,3 @@\n b\r\n+x\r\n c\r\n"
    assert apply_patch(orig, patch) == b"a\r\nb\r\nx\r\nc\r\nd\r\n"
    for broken in (b"@@ -2,2 +2,3 @@\n z\r\n+x\r\n c\r\n", b"@@ -2,2 +2,3 @@\n b\n+x\r\n c\r\n"):
        try:
            apply_patch(orig, broken)
        except ValueError:
            continue
        raise SystemExit("self-test: a patch that does not fit was applied")
    global PATCHED
    saved = PATCHED
    PATCHED = "f"
    try:
        crate = {"f": orig, "g": b"g\n", "h/i": b"i\n"}
        good = {"f": apply_patch(orig, patch), "g": b"g\n", "h/i": b"i\n", PATCH_FILE: patch, "VENDOR.md": b"doc\n"}
        assert compare(crate, good) == [], compare(crate, good)
        for label, change in (
            ("changed file", lambda d: d.update(g=b"G\n")),
            ("missing file", lambda d: d.pop("h/i")),
            ("extra file", lambda d: d.update(extra=b"x\n")),
            ("patched file edited", lambda d: d.update(f=d["f"] + b"y\r\n")),
            ("patched file unpatched", lambda d: d.update(f=orig)),
            ("patch missing", lambda d: d.pop(PATCH_FILE)),
            ("doc missing", lambda d: d.pop("VENDOR.md")),
        ):
            tampered = dict(good)
            change(tampered)
            if not compare(crate, tampered):
                raise SystemExit(f"self-test: {label} was not detected")
    finally:
        PATCHED = saved
    print("self-test ok: the patch applier fits and refuses, and the comparison catches 7 kinds of change")
    return 0


def main(argv):
    if argv == ["--self-test"]:
        return self_test()
    bad = compare(load_crate(argv), read_vendor(VENDOR)) + check_manifests()
    for b in bad:
        print("FAIL", b)
    if bad:
        return 1
    print(f"vendor/wgpu-hal is {NAME} plus {PATCH_FILE}")
    return 0


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
