#!/usr/bin/env python3
"""The browser module carries no native video decoder: python3 -I tools/check_no_native_video.py [--cargo] [--binary FILE...]

Owner architecture decision 2026-11-02 (issue #90, docs/media-architecture.md section 8.2): the browser build plays a
MovieTexture in the page's <video> element, so the engine's own video decoders (x3d-runtime feature `native-video`:
webm.rs, vp8/, the vp9dec crate) are not part of the wasm module.

    --cargo   `cargo tree` for x3d-web on wasm32-unknown-unknown must not contain vp9dec. This is the authoritative check:
              it reads the dependency graph Cargo resolves for the browser build.
    --binary  a SUPPLEMENTAL smoke check on a built artifact (the wasm module): the crate path strings of the native video
              decoders must not appear, and the crate path of a codec that stays (symphonia-bundle-mp3) must, so a build
              without embedded crate paths cannot pass by saying nothing. It is not proof of absence by itself: stripping or
              renamed symbols can hide strings.
    --self-test  the binary check on small synthetic files.
"""
import subprocess
import sys
import tempfile
from pathlib import Path

ROOT = Path(__file__).resolve().parent.parent
FORBIDDEN = (b"vp9dec", b"x3d-runtime/src/vp8/", b"x3d-runtime/src/webm.rs", b"x3d_runtime/src/vp8/")
CONTROL = (b"symphonia-bundle-mp3", b"symphonia_bundle_mp3")


def check_cargo():
    r = subprocess.run(
        ["cargo", "tree", "--locked", "--offline", "-p", "x3d-web", "--target", "wasm32-unknown-unknown",
         "-e", "normal,build", "-i", "vp9dec"],
        cwd=ROOT, capture_output=True, text=True)
    if "did not match any packages" in r.stderr:
        print("OK: vp9dec is not in the wasm32 dependency graph of x3d-web")
        return 0
    print("ERROR: vp9dec is in the wasm32 graph of x3d-web, or cargo tree failed:\n" + (r.stdout + r.stderr)[:600])
    return 1


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
        if not any(c in data for c in CONTROL):
            bad.append(f"{path}: no control string (symphonia-bundle-mp3): this check says nothing about the build")
        for needle in FORBIDDEN:
            if needle in data:
                bad.append(f"{path}: contains {needle.decode()}")
    print("\n".join(bad) or f"OK: {len(paths)} artifact(s) hold the control string and no native video decoder path")
    return 1 if bad else 0


def self_test():
    failures = []
    with tempfile.TemporaryDirectory() as d:
        cases = {
            "clean": (b"..." + CONTROL[0] + b"...", 0),
            "has-vp9dec": (b"x " + CONTROL[0] + b" vp9dec-0.1.1/src/lib.rs", 1),
            "has-vp8": (b"x " + CONTROL[0] + b" crates/x3d-runtime/src/vp8/decoder.rs", 1),
            "no-control": (b"nothing here", 1),
        }
        for name, (data, want) in cases.items():
            f = Path(d) / name
            f.write_bytes(data)
            got = check_binaries([str(f)])
            if got != want:
                failures.append(f"{name}: exit {got}, wanted {want}")
        if check_binaries([]) != 1:
            failures.append("empty path list did not fail")
        if check_binaries([str(Path(d) / "missing")]) != 1:
            failures.append("missing file did not fail")
    print("\n".join(failures) or "self-test OK")
    return 1 if failures else 0


def main(argv):
    if "--self-test" in argv:
        return self_test()
    status = 0
    if "--cargo" in argv:
        status |= check_cargo()
    if "--binary" in argv:
        status |= check_binaries([a for a in argv if not a.startswith("--")])
    if "--cargo" not in argv and "--binary" not in argv:
        print(__doc__)
        return 2
    return status


if __name__ == "__main__":
    sys.exit(main(sys.argv[1:]))
