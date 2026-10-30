#!/usr/bin/env python3
"""MovieTexture PNG load errors (issue #90, PR #190 review): synthetic media, a stepped world and its event script
(oracle/synthetic-movie-load-errors). Recorded with `oracle/tools/movie_anim.py record 5 load-errors`; compared by
crates/x3d-viewer/tests/movie_anim.rs.

  python3 -I oracle/tools/gen_movie_load_errors.py

X_ITE gives a .png URL to apng-js parseAPNG and then, without waiting, to setAPNG:

  media/static.png      a valid PNG without acTL: parseAPNG returns "Not an animated PNG", the getPlayer call of
                        setAPNG fails in an unhandled rejection, the node stays IN_PROGRESS without a message.
  media/notpng.png      no PNG signature: "Not a PNG", the same.
  media/noframes.png    acTL but no fcTL: "Not an animated PNG", the same.
  media/trunc-head.png  the signature, IHDR and 2 bytes: a DataView read past the end throws in parseAPNG, so
                        loadData warns, tries the next URL and sets FAILED_STATE (case trunchead; case fallback
                        then loads good.png).
  media/trunc-anim.png  good.png without its last 30 bytes (the end of the second frame's fdAT and IEND): the chunk
                        structure is valid, the browser decodes the frame images, COMPLETE_STATE with frame 0.
  media/bad-data.png    acTL, fcTL and an IDAT that is not zlib data: COMPLETE_STATE, and the canvas stays empty.
  media/good.png        2 frames, red and green, 1 s each.

All media are 8 x 8. A MovieTexture with a URL that stays IN_PROGRESS at the first load stops X_ITE's loadURL,
so the world has no URLs: a key press at frame 4 + 4 i makes a Script set the url of case i (loads settle between
the presses). A LoadSensor for each case gives isLoaded (TRUE for COMPLETE, FALSE for FAILED, nothing while
IN_PROGRESS); the Script prints it and the node's width, height and duration_changed. Nothing is started, so the
APNG frames do not depend on the wall clock.
"""
import json, os, struct, zlib

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-movie-load-errors"))
os.makedirs(os.path.join(root, "media"), exist_ok=True)

SIGNATURE = b"\x89PNG\r\n\x1a\n"
RED, GREEN = (255, 0, 0, 255), (0, 255, 0, 255)


def chunk(kind, body):
    return struct.pack(">I", len(body)) + kind + body + struct.pack(">I", zlib.crc32(kind + body))


def raw(color):
    return zlib.compress(b"".join(b"\x00" + bytes(color) * 8 for _ in range(8)), 9)


def fctl(seq, num, den):
    return chunk(b"fcTL", struct.pack(">IIIIIHHBB", seq, 8, 8, 0, 0, num, den, 0, 0))


IHDR = chunk(b"IHDR", struct.pack(">IIBBBBB", 8, 8, 8, 6, 0, 0, 0))
IEND = chunk(b"IEND", b"")
good = (SIGNATURE + IHDR + chunk(b"acTL", struct.pack(">II", 2, 0)) + fctl(0, 1, 1) + chunk(b"IDAT", raw(RED))
        + fctl(1, 1, 1) + chunk(b"fdAT", struct.pack(">I", 2) + raw(GREEN)) + IEND)

media = {
    "static.png": SIGNATURE + IHDR + chunk(b"IDAT", raw(RED)) + IEND,
    "notpng.png": b"This is not a PNG file.\n",
    "noframes.png": SIGNATURE + IHDR + chunk(b"acTL", struct.pack(">II", 1, 0)) + chunk(b"IDAT", raw(RED)) + IEND,
    "trunc-head.png": SIGNATURE + IHDR + b"\x00\x00",
    "trunc-anim.png": good[:-30],
    "bad-data.png": (SIGNATURE + IHDR + chunk(b"acTL", struct.pack(">II", 1, 0)) + fctl(0, 1, 1)
                     + chunk(b"IDAT", b"this is not zlib data") + IEND),
    "good.png": good,
}

for name, data in media.items():
    open(os.path.join(root, "media", name), "wb").write(data)

CASES = [
    ("static", ["media/static.png"]),
    ("notpng", ["media/notpng.png"]),
    ("noframes", ["media/noframes.png"]),
    ("trunchead", ["media/trunc-head.png"]),
    ("fallback", ["media/trunc-head.png", "media/good.png"]),
    ("truncanim", ["media/trunc-anim.png"]),
    ("baddata", ["media/bad-data.png"]),
    ("good", ["media/good.png"]),
]
QUAD = ("IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] } "
        "coordIndex [ 0 1 2 3 -1 ] texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] } }")
T0, DT, FIRST, STEP = 946684800, 1 / 32, 4, 4

text = "#X3D V4.0 utf8\nPROFILE Full\nViewpoint { position 0 0 8 }\nNavigationInfo { type \"NONE\" headlight FALSE }\n"
text += "Background { skyColor 0.12 0.14 0.2 }\n"

for i, (name, _) in enumerate(CASES):
    x, y = -3.3 + 2.2 * (i % 4), 1.1 - 2.2 * (i // 4)
    text += (f"Transform {{ translation {x:.1f} {y:.1f} 0 children Shape {{ appearance Appearance {{ "
             f"texture DEF M_{name} MovieTexture {{ }} }} geometry {QUAD} }} }}\n")
    text += f"DEF L_{name} LoadSensor {{ children USE M_{name} }}\n"

text += "DEF K KeySensor { }\nDEF S Script {\n  directOutput TRUE\n  inputOnly SFString press\n"

for name, _ in CASES:
    text += f"  inputOnly SFBool {name}_loaded\n  inputOnly SFInt32 {name}_width\n"
    text += f"  inputOnly SFInt32 {name}_height\n  inputOnly SFTime {name}_duration\n"

text += f"  initializeOnly MFNode movies [ {' '.join(f'USE M_{n}' for n, _ in CASES)} ]\n"
text += "  url \"javascript:\nvar next = 0;\n\nfunction press (value, time)\n{\n   switch (next ++)\n   {\n"

for i, (name, urls) in enumerate(CASES):
    quoted = ", ".join(f"'{u}'" for u in urls)
    text += f"      case {i}: movies [{i}] .url = new MFString ({quoted}); print ('{name} url'); break;\n"

text += "   }\n}\n"

for name, _ in CASES:
    for field in ("loaded", "width", "height", "duration"):
        text += f"\nfunction {name}_{field} (value)\n{{\n   print ('{name} {field} ' + value);\n}}\n"

text += "\"\n}\nROUTE K.keyPress TO S.press\n"

for name, _ in CASES:
    text += (f"ROUTE L_{name}.isLoaded TO S.{name}_loaded\nROUTE M_{name}.width TO S.{name}_width\n"
             f"ROUTE M_{name}.height TO S.{name}_height\nROUTE M_{name}.duration_changed TO S.{name}_duration\n")

open(os.path.join(root, "movie-load-errors.wrl"), "w").write(text)

frames = FIRST + STEP * len(CASES) + 8
script = {"settle": True, "console": True, "size": [512, 512], "t0": T0, "dt": DT, "frames": frames,
          "capture": [frames - 2], "dump": [frames - 1],
          "events": [{"frame": FIRST + STEP * i, "type": "key", "key": "u"} for i in range(len(CASES))]}
json.dump(script, open(os.path.join(root, "movie-load-errors.json"), "w"), indent=1)
