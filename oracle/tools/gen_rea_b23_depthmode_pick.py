#!/usr/bin/env python3
"""REA batch 23 DepthMode pointing world (issue #79): which shape the pointing buffer ends with under 52 cell centers.

X3DRenderObject.pointing (pinned X_ITE 16.5.0) wraps every shape's draw in `depthModeNode .enable (gl)` / `.disable (gl)`,
so the appearance's depthTest, depthFunction, depthMask, depthRange and polygonOffset decide which x3d_Id the 1x1
pointing buffer keeps. Every cell holds two or three Rectangle2D shapes under one pointer, drawn in scene order. The
recorder is `x3d-oracle particles` (an inert, disabled ParticleSystem named PS satisfies its `node` key) with the script
key `pickProbes`, which records `browser .touch (x, y)` and the DEF name of the hit shape node at every cell center.

  python3 -I -B oracle/tools/gen_rea_b23_depthmode_pick.py

Writes oracle/synthetic-rea-b23-depthmode-pick/dm-pick.x3dv, scripts/dm-pick.json and cases.json (the shapes of every cell
in drawing order, for the Rust test and the report). 512x512, Viewpoint at 0 0 10 with the default fieldOfView, pointer y up.
Cell shapes are named cNN_A, cNN_B, cNN_C in drawing order. A shape at z = +1 is nearer than one at z = 0 or -1.
"""
import json, math, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b23-depthmode-pick"))
os.makedirs(os.path.join(root, "scripts"), exist_ok=True)

NEAR, TIE, FAR = 1.0, 0.0, -1.0
FUNCS = ["NEVER", "LESS", "EQUAL", "LESS_EQUAL", "GREATER", "NOT_EQUAL", "GREATER_EQUAL", "ALWAYS"]

# A shape: (z, DepthMode fields or None, rotation-about-Y degrees).
def S(z, mode=None, rot=0.0):
    return (z, mode, rot)

cases = []  # (name, requirement, [shapes in drawing order])

def case(name, req, *shapes):
    cases.append((name, req, list(shapes)))

# 1. Default nearest pick, in both drawing orders and with a third shape.
case("default-near-first", "default nearest", S(NEAR), S(FAR))
case("default-near-second", "default nearest", S(FAR), S(NEAR))
case("default-three", "default nearest", S(TIE), S(FAR), S(NEAR))
case("default-explicit-node", "default nearest", S(NEAR, ""), S(FAR, ""))

# 2. depthFunction: a depth tie, a farther and a nearer second shape, for every function (first shape is the default draw).
for f in FUNCS:
    for tag, z in (("tie", TIE), ("far", FAR), ("near", NEAR)):
        case(f"func-{f.lower()}-{tag}", "depthFunction", S(TIE), S(z, f'depthFunction "{f}"'))

# 3. depthMask: a shape that is tested but writes no depth.
case("mask-false-first-near", "depthMask", S(NEAR, "depthMask FALSE"), S(FAR))
case("mask-false-second-far", "depthMask", S(NEAR), S(FAR, "depthMask FALSE"))
case("mask-false-then-true", "depthMask", S(NEAR, "depthMask FALSE"), S(TIE, "depthMask TRUE"), S(FAR))

# 4. depthTest FALSE: always passes and writes no depth.
case("test-false-second-far", "depthTest", S(NEAR), S(FAR, "depthTest FALSE"))
case("test-false-first-near", "depthTest", S(NEAR, "depthTest FALSE"), S(FAR))

# 5. Per-shape state: the odd shape's state must not reach the next draws (DepthMode.disable restores it).
case("state-per-shape", "per-shape state",
     S(FAR, "depthTest FALSE depthMask FALSE depthFunction \"NEVER\" depthRange 1 1"), S(NEAR), S(TIE))

# 6. depthRange, including reversed and degenerate ranges.
case("range-1-1-near", "depthRange", S(FAR), S(NEAR, "depthRange 1 1"))
case("range-0-0-far", "depthRange", S(NEAR), S(FAR, "depthRange 0 0"))
case("range-reversed-near", "depthRange", S(FAR), S(NEAR, "depthRange 1 0"))
case("range-reversed-far", "depthRange", S(NEAR), S(FAR, "depthRange 1 0"))
# The same reversed range with a wider depth gap (z = +3 / -3) and a narrower reversed range, to tell a borderline depth
# comparison from a different meaning of the range.
case("range-reversed-wide-near", "depthRange", S(-3.0), S(3.0, "depthRange 1 0"))
case("range-reversed-wide-far", "depthRange", S(3.0), S(-3.0, "depthRange 1 0"))
case("range-narrow-reversed-near", "depthRange", S(-3.0), S(3.0, "depthRange 0.75 0.25"))
case("range-narrow-reversed-far", "depthRange", S(3.0), S(-3.0, "depthRange 0.75 0.25"))
case("range-half-near", "depthRange", S(FAR), S(NEAR, "depthRange 0.5 0.5"))
case("range-upper-half", "depthRange", S(TIE), S(TIE, "depthRange 0.5 1"))
case("range-lower-half", "depthRange", S(TIE), S(TIE, "depthRange 0 0.5 depthFunction \"LESS\""))

# 7. polygonOffset (factor, units): coplanar ties, small and large units, and a tilted pair where the slope factor counts.
case("poly-none-less", "polygonOffset", S(TIE), S(TIE, 'depthFunction "LESS"'))
for u in (-1, -2, -4, -16, -100000):
    case(f"poly-units-minus{-u}-less", "polygonOffset",
         S(TIE), S(TIE, f'depthFunction "LESS" polygonOffset 0 {u}'))
case("poly-units-plus1-lequal", "polygonOffset", S(TIE), S(TIE, "polygonOffset 0 1"))
case("poly-units-plus100000-lequal", "polygonOffset", S(TIE), S(TIE, "polygonOffset 0 100000"))
case("poly-factor-flat-minus1", "polygonOffset", S(TIE), S(TIE, 'depthFunction "LESS" polygonOffset -1 0'))
case("poly-tilted-none-less", "polygonOffset", S(TIE, None, 45), S(TIE, 'depthFunction "LESS"', 45))
case("poly-tilted-factor-minus1", "polygonOffset",
     S(TIE, None, 45), S(TIE, 'depthFunction "LESS" polygonOffset -1 0', 45))
case("poly-tilted-factor-minus-half-units-minus1", "polygonOffset",
     S(TIE, None, 45), S(TIE, 'depthFunction "LESS" polygonOffset -0.5 -1', 45))
case("poly-tilted-factor-plus1-lequal", "polygonOffset", S(TIE, None, 45), S(TIE, "polygonOffset 1 0", 45))

COLS, PITCH, SIZE, DISTANCE = 8, 1.0, 0.8, 10.0
HALF = DISTANCE * math.tan(math.pi / 8)  # half the visible extent at z = 0 for a 512x512 canvas and fieldOfView pi/4
rows = (len(cases) + COLS - 1) // COLS

def cell(i):
    c, r = i % COLS, i // COLS
    return ((c - (COLS - 1) / 2) * PITCH, ((rows - 1) / 2 - r) * PITCH)

head = ("#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT X_ITE:1\n"
        "# REA batch 23 DepthMode pointing oracle world (generated by oracle/tools/gen_rea_b23_depthmode_pick.py).\n")
body = [head, f"Viewpoint {{ position 0 0 {DISTANCE:g} }}\n",
        "DEF PS ParticleSystem { enabled FALSE maxParticles 1 geometryType \"POINT\" }\n"]
points, manifest = [], []

for i, (name, req, shapes) in enumerate(cases):
    x, y = cell(i)
    px, py = round(256 + x / HALF * 256) + 0.5, round(256 + y / HALF * 256) + 0.5
    points.append([px, py])
    names = []

    for k, (z, mode, rot) in enumerate(shapes):
        n = f"c{i:02d}_{'ABC'[k]}"
        names.append(n)
        app = "Appearance { }" if mode is None else f"Appearance {{ depthMode DepthMode {{ {mode} }} }}"
        rot_s = f" rotation 0 1 0 {math.radians(rot):.9f}" if rot else ""
        # Every shape of a cell has the same projected footprint, whatever its z: the cell center scales with the distance.
        k = (DISTANCE - z) / DISTANCE
        body.append(f"Transform {{ translation {x * k:.9g} {y * k:.9g} {z:g}{rot_s} children DEF {n} Shape {{ appearance {app} "
                    f"geometry Rectangle2D {{ size {SIZE * k:.9g} {SIZE * k:.9g} }} }} }}\n")

    manifest.append({"case": name, "requirement": req, "cell": i, "point": [px, py], "shapes": [
        {"def": n, "z": z, "mode": mode, "rotationY": rot} for n, (z, mode, rot) in zip(names, shapes)]})

with open(os.path.join(root, "dm-pick.x3dv"), "w") as f:
    f.write("".join(body))

script = {"dt": 1 / 30, "t0": 946684800, "frames": 3, "warmup": 8, "node": "PS", "dump": [3], "seed": 23000001,
          "pickProbes": {"frames": [3], "points": points}}
with open(os.path.join(root, "scripts", "dm-pick.json"), "w") as f:
    json.dump(script, f)
with open(os.path.join(root, "cases.json"), "w") as f:
    json.dump(manifest, f, indent=1)

print(f"{len(cases)} cases, {rows} rows x {COLS} columns")
