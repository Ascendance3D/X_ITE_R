#!/usr/bin/env python3
"""MovieTexture GIF / APNG playback (issue #90): synthetic media, a stepped world and its event scripts
(oracle/synthetic-movie-anim). Recorded with oracle/tools/movie_anim.py; compared by
crates/x3d-viewer/tests/movie_anim.rs.

  python3 -I oracle/tools/gen_movie_anim.py

The media are 8 × 8 (below 16 texels, so both renderers sample NEAREST) and opaque. They are written by this
script with the standard library only, so every frame rectangle, delay, disposal and blend operation is exact:

  media/anim4.gif     4 full frames red, green, blue, yellow; delays 10, 20, 30, 40 cs (1 s); loops. A white 2 × 2
                      block at the top left of every frame shows the row order. X_ITE (GifMedia) picks the frame
                      from the elapsed time in equal steps (ceil (t / duration · length) − 1), not from the delays.
  media/dispose.gif   blue; red 4 × 4 at (0, 0) with disposal 3 (restore previous); green 4 × 4 at (4, 4);
                      yellow 4 × 4 at (4, 0) whose odd pixels are the transparent index. 25 cs each; loops.
  media/hold.png      APNG, red (60 s), green, blue (10 ms each). apng-js plays on the wall clock, so only a
                      frame that lasts longer than the recording is fixed: the texture stays red.
  media/once.png      APNG, 10 ms frames: blue; red 4 × 4 at (0, 0) with dispose_op PREVIOUS; green 4 × 4 at (4, 4)
                      with blend_op OVER; yellow 4 × 4 at (4, 0). The world plays it at speed 0 (frame 0 only).

apng-js advances on requestAnimationFrame and the wall clock, not on the browser time the harness steps. In the
headless recorder its ticks are rare and irregular: a world that played once.png once (loop FALSE) showed an
arbitrary middle frame 8 virtual seconds after the start, and the 5 runs did not agree. So the APNG cases are only
the states that do not depend on the wall clock (before the start, a first frame of 60 s, speed 0, disabled); the
later APNG frames are covered by the unit tests of crates/x3d-runtime/src/nodes/movie_texture.rs.

The world has one textured unlit quad per case. A key press at frame 32 (dt = 1/32, exact binary times) starts
them; the captures fall between frame changes. All quads show frame 0 before the start.
"""
import json, os, struct, zlib

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-movie-anim"))
os.makedirs(os.path.join(root, "media"), exist_ok=True)

W = H = 8
RED, GREEN, BLUE, YELLOW, WHITE = (255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0), (255, 255, 255)
PALETTE = [(0, 0, 0), RED, GREEN, BLUE, YELLOW, (255, 0, 255), (0, 255, 255), WHITE]


def solid(w, h, c):
    return [[c] * w for _ in range(h)]


def marked(c):
    rows = solid(W, H, c)
    for y in range(2):
        for x in range(2):
            rows[y][x] = WHITE
    return rows


# --- GIF ------------------------------------------------------------------------------------------

def lzw(indices, min_code=3):
    """Variable-length LZW without compression: a clear code before every 2^min_code - 3 indices keeps the
    code width at min_code + 1 bits."""
    clear, eoi, width = 1 << min_code, (1 << min_code) + 1, min_code + 1
    codes, run = [], (1 << min_code) - 3
    for i in range(0, len(indices), run):
        codes.append(clear)
        codes.extend(indices[i:i + run])
    codes.append(eoi)
    out, acc, bits = bytearray(), 0, 0
    for c in codes:
        acc |= c << bits
        bits += width
        while bits >= 8:
            out.append(acc & 0xff)
            acc >>= 8
            bits -= 8
    if bits:
        out.append(acc & 0xff)
    blocks = bytearray([min_code])
    for i in range(0, len(out), 255):
        part = out[i:i + 255]
        blocks += bytes([len(part)]) + part
    return bytes(blocks + b"\x00")


def gif(frames):
    """frames: (left, top, rows of palette indices, delay cs, disposal, transparent index or None)."""
    data = bytearray(b"GIF89a" + struct.pack("<HHBBB", W, H, 0xf2, 0, 0))
    for c in PALETTE:
        data += bytes(c)
    data += b"\x21\xff\x0bNETSCAPE2.0\x03\x01\x00\x00\x00"
    for left, top, rows, delay, disposal, transparent in frames:
        flags = (disposal << 2) | (transparent is not None)
        data += b"\x21\xf9\x04" + struct.pack("<BHB", flags, delay, transparent or 0) + b"\x00"
        data += b"\x2c" + struct.pack("<HHHHB", left, top, len(rows[0]), len(rows), 0)
        data += lzw([i for row in rows for i in row])
    return bytes(data + b"\x3b")


def idx(rows):
    return [[PALETTE.index(c) for c in row] for row in rows]


def checker(c, t):
    return [[c if (x + y) % 2 == 0 else t for x in range(4)] for y in range(4)]


anim4 = gif([(0, 0, idx(marked(c)), d, 1, None) for c, d in [(RED, 10), (GREEN, 20), (BLUE, 30), (YELLOW, 40)]])
dispose = gif([
    (0, 0, idx(marked(BLUE)), 25, 1, None),
    (0, 0, idx(solid(4, 4, RED)), 25, 3, None),
    (4, 4, idx(solid(4, 4, GREEN)), 25, 1, None),
    # Index 0 (black) is the transparent index: the blue below shows through.
    (4, 0, [[PALETTE.index(YELLOW) if v == YELLOW else 0 for v in row] for row in checker(YELLOW, None)], 25, 1, 0),
])

# --- APNG -----------------------------------------------------------------------------------------

def chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def raw(rows):
    return zlib.compress(b"".join(b"\x00" + b"".join(bytes(c) + b"\xff" for c in row) for row in rows), 9)


def apng(frames):
    """frames: (left, top, rows of RGB, delay num, delay den, dispose_op, blend_op); frame 0 is the default image."""
    out = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", W, H, 8, 6, 0, 0, 0))
    out += chunk(b"acTL", struct.pack(">II", len(frames), 0))
    seq = 0
    for i, (left, top, rows, num, den, dispose_op, blend_op) in enumerate(frames):
        out += chunk(b"fcTL", struct.pack(">IIIIIHHBB", seq, len(rows[0]), len(rows), left, top, num, den, dispose_op, blend_op))
        seq += 1
        if i == 0:
            out += chunk(b"IDAT", raw(rows))
        else:
            out += chunk(b"fdAT", struct.pack(">I", seq) + raw(rows))
            seq += 1
    return out + chunk(b"IEND", b"")


hold = apng([(0, 0, marked(RED), 60, 1, 0, 0), (0, 0, marked(GREEN), 10, 1000, 0, 0), (0, 0, marked(BLUE), 10, 1000, 0, 0)])
once = apng([
    (0, 0, marked(BLUE), 10, 1000, 0, 0),
    (0, 0, solid(4, 4, RED), 10, 1000, 2, 0),
    (4, 4, solid(4, 4, GREEN), 10, 1000, 0, 1),
    (4, 0, solid(4, 4, YELLOW), 10, 1000, 0, 0),
])

for name, data in [("anim4.gif", anim4), ("dispose.gif", dispose), ("hold.png", hold), ("once.png", once)]:
    open(os.path.join(root, "media", name), "wb").write(data)

# --- worlds -----------------------------------------------------------------------------------------

T0, DT, START = 946684800, 1 / 32, 32
QUAD = ("IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] } "
        "coordIndex [ 0 1 2 3 -1 ] texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] } }")


def world(name, cases, frames, capture, stopped=None):
    """One unlit quad per case in rows of four. X_ITE starts a time-dependent node from the file only through
    `loop` (set_loop__ at initialize, before the media are in); a startTime takes effect when it changes. So no
    node loops in the file: a key press at frame START makes a Script set startTime of every MovieTexture to its
    timestamp, then loop TRUE on the looping ones and the stopTime (0.6 s later) of case `stopped`."""
    looping = [i for i, (_, f) in enumerate(cases) if "loop TRUE" in f]
    cases = [(url, f.replace("loop TRUE", "").strip()) for url, f in cases]
    text = "#X3D V4.0 utf8\nPROFILE Full\nViewpoint { position 0 0 10 }\nNavigationInfo { type \"NONE\" headlight FALSE }\n"
    text += "Background { skyColor 0.12 0.14 0.2 }\n"
    for i, (url, fields) in enumerate(cases):
        x, y = -3.3 + 2.2 * (i % 4), 2.2 - 2.2 * (i // 4)
        text += (f"Transform {{ translation {x:.1f} {y:.1f} 0 children Shape {{ appearance Appearance {{ "
                 f"texture DEF M{i} MovieTexture {{ url \"media/{url}\" {fields} }} }} geometry {QUAD} }} }}\n")
    text += "DEF K KeySensor { }\n"
    text += ("DEF S Script {\n  directOutput TRUE\n  inputOnly SFString press\n"
             f"  initializeOnly MFNode movies [ {' '.join(f'USE M{i}' for i in range(len(cases)))} ]\n"
             f"  initializeOnly MFNode looping [ {' '.join(f'USE M{i}' for i in looping)} ]\n"
             + (f"  initializeOnly SFNode stopped USE M{stopped}\n" if stopped is not None else "")
             + "  url \"javascript:\nfunction press (value, time)\n{\n"
             "   for (var i = 0; i < movies .length; ++ i)\n      movies [i] .startTime = time;\n\n"
             "   for (var i = 0; i < looping .length; ++ i)\n      looping [i] .loop = true;\n"
             + ("\n   stopped .stopTime = time + 0.6;\n" if stopped is not None else "")
             + "}\n\"\n}\nROUTE K.keyPress TO S.press\n")
    open(os.path.join(root, name + ".wrl"), "w").write(text)
    script = {"settle": False, "size": [512, 512], "t0": T0, "dt": DT, "frames": frames, "capture": capture,
              "dump": [frames - 1], "events": [{"frame": START, "type": "key", "key": "s"}]}
    json.dump(script, open(os.path.join(root, name + ".json"), "w"), indent=1)


# Playback on the X3D clock (GIF) and the APNG states that do not depend on the wall clock. Frame 4: before the
# start. Then about 0.125, 0.375, 0.47 (equal steps: frame 1; by the delays it would be frame 2), 0.625, 0.875,
# 1.125 (second cycle), 1.5 and 2 s after it.
world("movie-anim", [
    ("anim4.gif", "loop TRUE"),
    ("anim4.gif", ""),
    ("anim4.gif", "speed 2"),
    ("anim4.gif", "loop TRUE speed -1"),
    ("dispose.gif", "loop TRUE"),
    ("anim4.gif", "loop TRUE"),
    ("hold.png", "loop TRUE"),
    ("hold.png", ""),
    ("once.png", "loop TRUE speed 0"),
    ("anim4.gif", "loop TRUE enabled FALSE"),
    ("hold.png", "enabled FALSE"),
    ("anim4.gif", "loop TRUE speed 0.5"),
], 97, [4] + [START + int(e * 32) for e in (0.125, 0.375, 0.46875, 0.625, 0.875, 1.125, 1.5, 2.0)], stopped=5)
