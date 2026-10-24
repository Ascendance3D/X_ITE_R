#!/usr/bin/env python3
"""Wide and stippled instanced line baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU) for
crates/x3d-viewer/tests/rea_b18_particle_lines.rs.

  python3 -I -B oracle/tools/rea_b18_particle_lines.py record NAME [X3D_ORACLE]

runs `x3d-oracle particles` (default target/release/x3d-oracle) three times per image pass on
oracle/synthetic-rea-b18-particle-lines/NAME.x3dv with scripts/NAME.json: `png` (antialiased="false") and `msaa4`
(--ms 4). Within a pass the three js.json and js.f32 must be identical and every PNG must decode to the same pixels
(the encoder may differ, so file bytes are not compared). The first run of each pass is kept as
oracle/rea-batch18-particle-lines-baseline/NAME/ in the layout of rea-batch13-particles-baseline (js.json, js.f32,
png/, msaa4/). A baseline is never taken from Rust.
"""
import filecmp, os, shutil, struct, subprocess, sys, zlib

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))


def pixels(path):
    """Decoded 8-bit RGB / RGBA rows of a non-interlaced PNG (what the canvas capture writes)."""
    data = open(path, "rb").read()
    assert data[:8] == b"\x89PNG\r\n\x1a\n", path
    pos, idat, header = 8, b"", None
    while pos < len(data):
        length, kind = struct.unpack(">I4s", data[pos:pos + 8])
        body = data[pos + 8:pos + 8 + length]
        if kind == b"IHDR":
            header = struct.unpack(">IIBBBBB", body)
        elif kind == b"IDAT":
            idat += body
        pos += 12 + length
    width, height, depth, color, _, _, interlace = header
    assert depth == 8 and color in (2, 6) and interlace == 0, (path, header)
    bpp = 4 if color == 6 else 3
    raw, stride, out, prev = zlib.decompress(idat), width * bpp, bytearray(), bytearray(width * bpp)
    for y in range(height):
        f, line = raw[y * (stride + 1)], bytearray(raw[y * (stride + 1) + 1:(y + 1) * (stride + 1)])
        for i in range(stride):
            a = line[i - bpp] if i >= bpp else 0
            b, c = prev[i], prev[i - bpp] if i >= bpp else 0
            if f == 1:
                line[i] = (line[i] + a) & 255
            elif f == 2:
                line[i] = (line[i] + b) & 255
            elif f == 3:
                line[i] = (line[i] + (a + b) // 2) & 255
            elif f == 4:
                p = a + b - c
                pa, pb, pc = abs(p - a), abs(p - b), abs(p - c)
                line[i] = (line[i] + (a if pa <= pb and pa <= pc else b if pb <= pc else c)) & 255
        out += line
        prev = line
    return width, height, bpp, bytes(out)


def main():
    mode, name = sys.argv[1:3]
    assert mode == "record"
    oracle = sys.argv[3] if len(sys.argv) > 3 else os.path.join(root, "target", "release", "x3d-oracle")
    world = os.path.join(root, "oracle", "synthetic-rea-b18-particle-lines", name + ".x3dv")
    script = os.path.join(root, "oracle", "synthetic-rea-b18-particle-lines", "scripts", name + ".json")
    kept = {}

    for image_pass, extra in (("png", []), ("msaa4", ["--ms", "4"])):
        runs = []
        for i in (1, 2, 3):
            out = f"/tmp/rea-b18-lines-record/{name}-{image_pass}-{i}"
            shutil.rmtree(out, ignore_errors=True)
            os.makedirs(out)
            subprocess.run([oracle, "particles", world, script, "--out", os.path.join(out, "js.json"),
                            "--png-dir", os.path.join(out, image_pass)] + extra,
                           check=True, cwd=root, timeout=300, stdout=subprocess.DEVNULL)
            runs.append(out)
        a = runs[0]
        pngs = sorted(os.listdir(os.path.join(a, image_pass)))
        same = bool(pngs) and all(
            filecmp.cmp(os.path.join(a, f), os.path.join(b, f), shallow=False) for b in runs[1:] for f in ("js.json", "js.f32")
        ) and all(
            sorted(os.listdir(os.path.join(b, image_pass))) == pngs and
            pixels(os.path.join(a, image_pass, p)) == pixels(os.path.join(b, image_pass, p))
            for b in runs[1:] for p in pngs)
        print(f"{name} {image_pass}: three X_ITE runs {'identical' if same else 'DIFFER'} ({len(pngs)} images)")
        if not same:
            sys.exit(1)
        kept[image_pass] = a

    # The records do not depend on the image pass.
    if not all(filecmp.cmp(os.path.join(kept["png"], f), os.path.join(kept["msaa4"], f), shallow=False) for f in ("js.json", "js.f32")):
        print(f"{name}: records differ between the png and msaa4 passes")
        sys.exit(1)

    dest = os.path.join(root, "oracle", "rea-batch18-particle-lines-baseline", name)
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest)
    for f in ("js.json", "js.f32"):
        shutil.copy(os.path.join(kept["png"], f), dest)
    for image_pass, src in kept.items():
        shutil.copytree(os.path.join(src, image_pass), os.path.join(dest, image_pass))


if __name__ == "__main__":
    main()
