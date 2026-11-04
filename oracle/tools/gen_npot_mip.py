#!/usr/bin/env python3
"""Generate oracle/synthetic-npot-mip/ (issue #51, ledger sub:render.core): synthetic ImageTexture PNGs and worlds for the
mipmaps of textures whose width or height is not a power of two.

  python3 -I -B oracle/tools/gen_npot_mip.py [OUT_DIR]

Every file is written from fixed arithmetic (no clock, no random source), so a second run gives the same bytes.
The textures carry fine detail on purpose: a left part of 1-pixel checkers, a middle part of gradients and a right part
of per-texel hash noise, so a wrong mip filter or a wrong level size changes many pixels.
Nothing here comes from any historical asset (clean-room rule).
"""
import os
import struct
import sys
import zlib

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))


def png(path, w, h, pixels, alpha=False):
    def chunk(t, d):
        c = struct.pack(">I", len(d)) + t + d
        return c + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    n = 4 if alpha else 3
    raw = b"".join(b"\x00" + bytes(pixels[y * w * n:(y + 1) * w * n]) for y in range(h))
    data = (b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6 if alpha else 2, 0, 0, 0))
            + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b""))
    with open(path, "wb") as f:
        f.write(data)


def pattern(w, h, alpha=False):
    """RGB (or RGBA) bytes, row 0 first. Left third: 1-pixel checker; middle third: gradients; right third: hash noise."""
    out = bytearray()
    for y in range(h):
        for x in range(w):
            zone = (3 * x) // w
            if zone == 0:
                v = 255 if (x + y) % 2 else 0
                px = (v, 255 - v, 128)
            elif zone == 1:
                px = ((x * 255) // max(w - 1, 1), (y * 255) // max(h - 1, 1), ((x + y) * 37) % 256)
            else:
                s = (x * 73856093) ^ (y * 19349663) ^ (w * 83492791)
                s = (s * 1103515245 + 12345) & 0x7FFFFFFF
                px = ((s >> 3) & 255, (s >> 11) & 255, (s >> 19) & 255)
            if alpha:
                # alpha from 40 to 255 in a diagonal ramp with a 3-texel stripe pattern, so no texel is opaque everywhere
                px += (40 + ((x * 5 + y * 3) % 216) if (x // 3 + y // 3) % 2 else 255 - ((x * 7 + y) % 120),)
            out += bytes(px)
    return out


TP = """    textureProperties TextureProperties { generateMipMaps TRUE }"""

# (world name, [(width, height, explicit TextureProperties generateMipMaps TRUE, PNG with alpha)], layout name)
WORLDS = [
    ("npot-100x60", [(100, 60, False, False)], "near"),
    ("npot-129x65", [(129, 65, False, False)], "near"),
    ("npot-255x255-tp", [(255, 255, True, False)], "near"),
    ("npot-thin-tp", [(1, 7, True, False), (33, 1, False, False), (7, 33, True, False)], "near"),
    ("npot-mixed-tp", [(255, 129, True, False), (17, 33, True, False), (63, 31, True, False)], "near"),
    ("npot-deep", [(301, 201, False, False), (399, 131, True, False)], "far"),
    ("npot-alpha", [(99, 77, False, True), (203, 51, True, True)], "far"),
    ("pot-128x64", [(128, 64, False, False)], "near"),
    # Mixed parity: one side odd, the other even. The pinned NVIDIA driver builds a different mip chain for some of
    # these sizes (a shifted box on the even side, see crates/x3d-render/tests/render_npot_mip.rs); "plain" sizes get the
    # standard chain, "shifted" sizes the shifted one (measured with oracle/tools/npot_mip_probe.mjs).
    ("npot-parity-plain", [(105, 64, False, False), (107, 64, True, False), (109, 64, False, False), (113, 64, True, False), (115, 64, False, False), (119, 64, True, False)], "mid"),
    ("npot-parity-shifted", [(111, 64, False, False), (117, 64, True, False), (123, 64, False, False), (135, 64, True, False), (101, 100, False, False), (175, 100, True, False)], "mid"),
]

# (x, y, minification, rotation axis+angle): the quad is the texture's long side divided by the minification, in pixels
# (the Viewpoint at z 10 with the default field of view draws one unit of the z 0 plane as 61.8 pixels of a 512 x 512
# picture); the last three quads are tilted, which makes the minification different along the two axes. "far" reaches
# a minification of 24 (mip level 4 or 5).
PX_PER_UNIT = 61.8
LAYOUTS = {
    "near": [
        (-2.6, 2.4, 1.4, "0 1 0 0"),
        (1.6, 2.7, 2.6, "0 1 0 0"),
        (3.4, 2.8, 5.0, "0 1 0 0"),
        (-2.4, -1.0, 1.6, "0 1 0 1.2"),
        (2.0, -1.0, 1.8, "1 0 0 -1.15"),
        (-0.3, -3.2, 3.0, "0.3 1 0 0.9"),
    ],
    "mid": [
        (-2.7, 2.5, 1.6, "0 1 0 0"),
        (0.3, 2.6, 1.8, "0 1 0 0"),
        (3.0, 2.5, 2.2, "0 1 0 0"),
        (-2.5, -0.6, 1.7, "0 1 0 0"),
        (0.4, -0.6, 2.0, "0 1 0 0.8"),
        (3.0, -0.6, 2.4, "1 0 0 -0.9"),
    ],
    "far": [
        (-2.6, 2.4, 3.0, "0 1 0 0"),
        (1.6, 2.7, 6.0, "0 1 0 0"),
        (3.4, 2.8, 12.0, "0 1 0 0"),
        (-2.4, -1.0, 4.0, "0 1 0 1.2"),
        (2.0, -1.0, 8.0, "1 0 0 -1.15"),
        (-0.3, -3.2, 24.0, "0.3 1 0 0.9"),
    ],
}


def tex_name(w, h, alpha):
    pot = (w & (w - 1)) == 0 and (h & (h - 1)) == 0
    return f"{'pot' if pot else 'npot'}-{w}x{h}{'a' if alpha else ''}.png"


def world(name, textures, layout):
    lines = ["#X3D V4.1 utf8", "PROFILE Interchange", "",
             f"# Synthetic NPOT mipmap case {name} (clean-room, generated by oracle/tools/gen_npot_mip.py).",
             "Viewpoint { position 0 0 10 }", "Background { skyColor 0.15 0.15 0.2 }"]

    for i, (x, y, s, rot) in enumerate(LAYOUTS[layout]):
        w, h, tp, alpha = textures[i % len(textures)]
        tex = tex_name(w, h, alpha)
        long_px = max(max(w, h) / s, 40.0)
        aspect = min(max(w / h, 0.5), 2.0)
        sx, sy = (long_px, long_px / aspect) if aspect >= 1 else (long_px * aspect, long_px)
        sx, sy = sx / PX_PER_UNIT, sy / PX_PER_UNIT
        lines += [
            f"Transform {{ translation {x} {y} 0 rotation {rot} children Shape {{",
            "  appearance Appearance { texture ImageTexture {",
            f'    url "{tex}"',
        ]
        if tp:
            lines.append(TP)
        lines += [
            "  } }",
            "  geometry IndexedFaceSet {",
            "    solid FALSE",
            f"    coord Coordinate {{ point [ {-sx / 2:.4f} {-sy / 2:.4f} 0, {sx / 2:.4f} {-sy / 2:.4f} 0, {sx / 2:.4f} {sy / 2:.4f} 0, {-sx / 2:.4f} {sy / 2:.4f} 0 ] }}",
            "    texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] }",
            "    coordIndex [ 0 1 2 3 -1 ]",
            "  }",
            "} }",
        ]

    return "\n".join(lines) + "\n"


def main():
    out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(root, "oracle", "synthetic-npot-mip")
    os.makedirs(out, exist_ok=True)
    sizes = sorted({(w, h, a) for _, ts, _ in WORLDS for (w, h, _, a) in ts})

    for w, h, a in sizes:
        png(os.path.join(out, tex_name(w, h, a)), w, h, pattern(w, h, a), a)

    for name, textures, layout in WORLDS:
        with open(os.path.join(out, name + ".wrl"), "w") as f:
            f.write(world(name, textures, layout))

    print(f"{len(WORLDS)} worlds, {len(sizes)} textures in {out}")


if __name__ == "__main__":
    main()
