#!/usr/bin/env python3
"""REA batch 08 (lane render2) worlds for GaussianSplats: oracle/synthetic-rea-b08-gs.

  python3 -I oracle/tools/gen_rea_b08_gs.py oracle/synthetic-rea-b08-gs

GaussianSplats (X_ITE 16.5.0, component GaussianSplats, experimental). Every splat is generated here
from a fixed seed; nothing is copied from a historical world. Static worlds (gs-*.wrl) are compared
as still pictures at MSAA 0 and 4. The *-runtime.wrl worlds with their *.json scripts are stepped
cases (field changes, Group removal, Script-created nodes).
"""
import json, math, os, random, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "oracle/synthetic-rea-b08-gs"
os.makedirs(OUT, exist_ok=True)

HEAD = """#X3D V4.1 utf8
PROFILE Interchange
COMPONENT Shape:4
COMPONENT Lighting:3
COMPONENT Layering:2
COMPONENT Navigation:3
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
COMPONENT Interpolation:1
COMPONENT X_ITE:1
COMPONENT GaussianSplats:1
"""
BG = "Background { skyColor 0.2 0.2 0.28 }"
SH_NAMES = ["sphericalHarmonicsDegree0Coef0",
            "sphericalHarmonicsDegree1Coef0", "sphericalHarmonicsDegree1Coef1", "sphericalHarmonicsDegree1Coef2",
            "sphericalHarmonicsDegree2Coef0", "sphericalHarmonicsDegree2Coef1", "sphericalHarmonicsDegree2Coef2",
            "sphericalHarmonicsDegree2Coef3", "sphericalHarmonicsDegree2Coef4",
            "sphericalHarmonicsDegree3Coef0", "sphericalHarmonicsDegree3Coef1", "sphericalHarmonicsDegree3Coef2",
            "sphericalHarmonicsDegree3Coef3", "sphericalHarmonicsDegree3Coef4", "sphericalHarmonicsDegree3Coef5",
            "sphericalHarmonicsDegree3Coef6"]
DEGREE_END = [1, 4, 9, 16]


def f(v):
    return "%.6g" % v


def vec(rows):
    return "[ " + ", ".join(" ".join(f(x) for x in r) for r in rows) + " ]"


def flat(values):
    return "[ " + ", ".join(f(v) for v in values) + " ]"


def quat(rng):
    q = [rng.gauss(0, 1) for _ in range(4)]
    n = math.sqrt(sum(x * x for x in q))
    return [x / n for x in q]


def cloud(seed, n, spread=(2.0, 1.6, 1.4), scale=(0.05, 0.3), opacity=(0.35, 1.0), degree=0, sh_amp=(1.2, 0.8, 0.5, 0.35)):
    """Random splat data: positions, orientations, scales, opacities and SH coefficient lists."""
    rng = random.Random(seed)
    d = {"positions": [], "orientations": [], "scales": [], "opacities": [], "sh": [[] for _ in range(16)]}
    for _ in range(n):
        d["positions"].append([rng.uniform(-s, s) for s in spread])
        d["orientations"].append(quat(rng))
        d["scales"].append([rng.uniform(*scale) for _ in range(3)])
        d["opacities"].append(rng.uniform(*opacity))
        for k in range(DEGREE_END[degree]):
            amp = sh_amp[0 if k == 0 else 1 if k < 4 else 2 if k < 9 else 3]
            d["sh"][k].append([rng.uniform(-amp, amp) for _ in range(3)])
    return d


def splats(d, extra="", sh=True, keys=None, defn=""):
    """A GaussianSplats node from cloud data."""
    keys = keys or ["positions", "orientations", "scales", "opacities"]
    parts = [f"GaussianSplats {{ {extra}"]
    if "positions" in keys:
        parts.append(f"  positions {vec(d['positions'])}")
    if "orientations" in keys:
        parts.append(f"  orientations {vec(d['orientations'])}")
    if "scales" in keys:
        parts.append(f"  scales {vec(d['scales'])}")
    if "opacities" in keys:
        parts.append(f"  opacities {flat(d['opacities'])}")
    if sh:
        for name, vals in zip(SH_NAMES, d["sh"]):
            if vals:
                parts.append(f"  {name} {vec(vals)}")
    parts.append("}")
    node = "\n".join(parts)
    return f"DEF {defn} {node}" if defn else node


def world(*parts, vp="Viewpoint { position 0 0 7 }", head=HEAD):
    return head + vp + "\n" + BG + "\n" + "\n".join(parts) + "\n"


def runtime_json(frames, keys, start=20, step=12, extra_events=()):
    """A stepped script: the first click focuses, then one key per `step` frames; a frame is captured
    after each key (and one before the first)."""
    events = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507},
              {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
    caps = []
    for i, k in enumerate(keys):
        fr = start + i * step
        events.append({"frame": fr, "type": "key", "key": k})
        caps.append(fr + step - 2)
    events += list(extra_events)
    return {"settle": False, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames,
            "capture": [start - 4] + caps, "dump": [start - 4] + caps + [frames - 1], "events": events}


worlds, scripts, files = {}, {}, {}

# ---------------------------------------------------------------------------------------------
# Static worlds.

worlds["gs-basic.wrl"] = world("""GaussianSplats {
  positions [ -1 0 0, 0 0.5 0.5, 1 0 -0.5 ]
  orientations [ 0 0 0 1, 0 0 0.3826834 0.9238795, 0 0 0 1 ]
  scales [ 0.3 0.1 0.1, 0.4 0.15 0.1, 0.2 0.2 0.2 ]
  opacities [ 0.9 0.8 0.7 ]
  sphericalHarmonicsDegree0Coef0 [ 1 0 0, 0 1 0, 0 0 1 ]
}""")

# Order dependent blending: many overlapping splats at different depths, and a plane of splats at
# exactly the same depth (ties of the counting sort).
c = cloud(11, 70, spread=(0.9, 0.7, 1.6), scale=(0.15, 0.45), opacity=(0.5, 0.95))
worlds["gs-overlap.wrl"] = world(splats(c))
tie = {"positions": [], "orientations": [], "scales": [], "opacities": [], "sh": [[] for _ in range(16)]}
rng = random.Random(5)
for i in range(24):
    tie["positions"].append([rng.uniform(-0.6, 0.6), rng.uniform(-0.5, 0.5), 0.0])
    tie["orientations"].append([0, 0, 0, 1])
    tie["scales"].append([0.35, 0.35, 0.35])
    tie["opacities"].append(0.8)
    tie["sh"][0].append([rng.uniform(-1.5, 1.5) for _ in range(3)])
worlds["gs-ties.wrl"] = world(splats(tie))

big = cloud(21, 3000, spread=(2.6, 2.0, 1.6), scale=(0.02, 0.12), opacity=(0.2, 0.9))
worlds["gs-random-3000.wrl"] = world(splats(big))

# Spherical harmonics degrees 0..3, each from the same seed, three viewpoints for the view dependent ones.
for degree in range(4):
    d = cloud(30 + degree, 160, spread=(1.8, 1.4, 1.2), scale=(0.08, 0.22), opacity=(0.6, 1.0), degree=degree)
    worlds[f"gs-sh{degree}.wrl"] = world(splats(d))
d3 = cloud(40, 220, spread=(1.8, 1.4, 1.2), scale=(0.08, 0.22), opacity=(0.6, 1.0), degree=3)
for i, vp in enumerate(["Viewpoint { position 4 2 5 orientation -0.4 0.9 0.1 0.8 }",
                        "Viewpoint { position -5 -1 4 orientation 0.1 -0.95 0 0.9 }",
                        "Viewpoint { position 0 6 3 orientation -1 0 0 1.1 }"]):
    worlds[f"gs-sh3-view{i + 1}.wrl"] = world(splats(d3), vp=vp)

dl = cloud(50, 140, spread=(1.8, 1.4, 1.2), scale=(0.08, 0.22), opacity=(0.6, 1.0), degree=1)
worlds["gs-linear.wrl"] = world(splats(dl, extra='colorSpace "LIN_REC709_DISPLAY"'))
worlds["gs-srgb.wrl"] = world(splats(dl, extra='colorSpace "SRGB_REC709_DISPLAY"'))
worlds["gs-colorspace-unknown.wrl"] = world(splats(dl, extra='colorSpace "FOO"'))

# Ortho viewpoint.
# (X_ITE reads the OrthoViewpoint fieldOfView of an X3D 4 file as an SFVec4f and the Rust scene data as an MFFloat, so
# the default -1 -1 1 1 is used; the cloud is small enough to fill it.)
do = cloud(55, 110, spread=(0.8, 0.8, 0.8), scale=(0.04, 0.14), opacity=(0.5, 1.0), degree=1)
worlds["gs-ortho.wrl"] = world(splats(do), vp="OrthoViewpoint { position 0 0 7 }")

# Fog (linear and exponential) and clip plane.
fogd = cloud(60, 120, spread=(1.8, 1.4, 3.0), scale=(0.1, 0.25), opacity=(0.6, 1.0))
worlds["gs-fog-linear.wrl"] = world(splats(fogd), "Fog { color 0.6 0.7 0.9 fogType \"LINEAR\" visibilityRange 11 }")
worlds["gs-fog-exp.wrl"] = world(splats(fogd), "Fog { color 0.9 0.6 0.5 fogType \"EXPONENTIAL\" visibilityRange 9 }")
worlds["gs-clip.wrl"] = world("Group { children [ ClipPlane { plane 1 0.3 0 0.2 }\n" + splats(fogd) + " ] }")

# Together with opaque and transparent shapes.
mix = cloud(70, 90, spread=(1.4, 1.1, 1.2), scale=(0.12, 0.3), opacity=(0.6, 1.0))
worlds["gs-opaque.wrl"] = world(
    "DirectionalLight { direction -0.3 -0.5 -0.8 }",
    "Transform { translation 0.5 -0.2 0.2 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.5 0.2 } } geometry Box { size 1.2 1.2 1.2 } } }",
    splats(mix))
worlds["gs-transparent-mix.wrl"] = world(
    "DirectionalLight { direction -0.3 -0.5 -0.8 }",
    "Transform { translation -0.6 0 2.5 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.6 1 transparency 0.5 } } geometry Sphere { radius 1 } } }",
    "Transform { translation 0.7 0.2 -1.5 children Shape { appearance Appearance { material Material { diffuseColor 1 0.4 0.4 transparency 0.4 } } geometry Sphere { radius 1 } } }",
    splats(mix))

# Transforms around the node.
worlds["gs-transform.wrl"] = world("""Transform { translation 0.5 -0.3 0 rotation 0.3 1 0.2 0.9 scale 1.3 0.7 1.1 children [
  Transform { rotation 0 0 1 0.6 scale 0.8 1.2 1 children [ """ + splats(mix) + """ ] }
] }""")

# bboxSize / bboxCenter and the sort key of a transparent shape.
worlds["gs-bbox.wrl"] = world(
    "Transform { translation 0 0 -4 children Shape { appearance Appearance { material Material { diffuseColor 0.3 1 0.5 transparency 0.5 } } geometry Box { size 6 6 0.2 } } }",
    splats(mix, extra="bboxSize 4 4 4 bboxCenter 0 0 -9"))
worlds["gs-bbox-default.wrl"] = world(
    "Transform { translation 0 0 -4 children Shape { appearance Appearance { material Material { diffuseColor 0.3 1 0.5 transparency 0.5 } } geometry Box { size 6 6 0.2 } } }",
    splats(mix))

# visible FALSE, partially defined fields, no SH, empty.
# A GaussianSplats that is never displayed (visible FALSE, no splats) keeps X_ITE's scene loading forever: the first
# sort never arrives, so these are only run in the stepped worlds.
p = cloud(80, 40, spread=(1.5, 1.2, 1.0), scale=(0.1, 0.3), opacity=(0.6, 1.0), degree=2)
# Degree 1 coefficient 2 is missing: the highest complete degree is 0.
q = splats(p)
q = q.replace("\n  sphericalHarmonicsDegree1Coef2 " + vec(p["sh"][3]), "")
worlds["gs-sh-partial.wrl"] = world(q)
worlds["gs-nosh.wrl"] = world(splats(mix, sh=False))
# Shorter scales / opacities / SH arrays: the rest are zeros (opacity 0, scale 0 -> low pass only).
short = splats(mix).replace(f"scales {vec(mix['scales'])}", f"scales {vec(mix['scales'][:45])}")
worlds["gs-short-scales.wrl"] = world(short)
short = splats(mix).replace(f"opacities {flat(mix['opacities'])}", f"opacities {flat(mix['opacities'][:30])}")
worlds["gs-short-opacities.wrl"] = world(short)
mix1 = cloud(70, 90, spread=(1.4, 1.1, 1.2), scale=(0.12, 0.3), opacity=(0.6, 1.0), degree=1)
short = splats(mix1).replace(f"{SH_NAMES[2]} {vec(mix1['sh'][2])}", f"{SH_NAMES[2]} {vec(mix1['sh'][2][:50])}")
worlds["gs-short-sh.wrl"] = world(short)

# Large and near splats: the discard rules of the vertex shader (size above the screen, behind the camera).
near = {"positions": [[0, 0, 5.5], [0.6, 0.2, 3], [-1, 0, -2], [0, 0, 6.9], [1.2, 1.1, 0], [0, 0, 8]], "orientations": [[0, 0, 0, 1]] * 6,
        "scales": [[0.4, 0.4, 0.4], [0.3, 0.3, 0.3], [4, 4, 4], [0.5, 0.5, 0.5], [0.5, 0.1, 0.1], [0.5, 0.5, 0.5]],
        "opacities": [0.9] * 6, "sh": [[[1, 0, 0], [0, 1, 0], [0, 0, 1], [1, 1, 0], [0, 1, 1], [1, 0, 1]]] + [[] for _ in range(15)]}
worlds["gs-near-far.wrl"] = world(splats(near))
# Edge: centers beyond the frustum but within the 1.3 NDC culling margin.
edge = {"positions": [[2.9, 0, 0], [-2.9, 0.5, 0], [0, 2.9, 0], [3.8, 0, 0], [0, -3.4, 0], [3.5, 3.5, 0]], "orientations": [[0, 0, 0, 1]] * 6,
        "scales": [[0.3, 0.3, 0.3]] * 6, "opacities": [0.9] * 6,
        "sh": [[[1, 0.2, 0], [0, 1, 0.2], [0.2, 0, 1], [1, 1, 0], [0, 1, 1], [1, 0, 1]]] + [[] for _ in range(15)]}
worlds["gs-edge.wrl"] = world(splats(edge))

# Layers.
worlds["gs-layers.wrl"] = HEAD + """LayerSet {
  activeLayer 0
  order [ 1 2 ]
  layers [
    Layer { children [ Viewpoint { position 0 0 7 } Background { skyColor 0.2 0.2 0.28 } """ + splats(mix) + """ ] }
    Layer { viewport Viewport { clipBoundary [ 0.5 1 0 0.5 ] } children [ Viewpoint { position 0 0 8 } """ + splats(dl) + """ ] }
  ]
}
"""

# Splats in the transmission pass: a transmissive sphere in front of the cloud.
worlds["gs-transmission.wrl"] = world(
    "DirectionalLight { direction -0.3 -0.5 -0.8 }",
    "Transform { translation 0 0 2.5 children Shape { appearance Appearance { material PhysicalMaterial { baseColor 1 1 1 extensions [ TransmissionMaterialExtension { transmission 1 } VolumeMaterialExtension { thickness 0.5 } ] } } geometry Sphere { radius 1.2 } } }",
    splats(mix))

# A PLY file of splats loaded through an Inline.
rng = random.Random(90)
ply = ["ply", "format ascii 1.0", "element vertex 60",
       "property float x", "property float y", "property float z"]
ply += [f"property float f_dc_{i}" for i in range(3)]
ply += [f"property float f_rest_{i}" for i in range(9)]
ply += ["property float opacity"] + [f"property float scale_{i}" for i in range(3)] + [f"property float rot_{i}" for i in range(4)]
ply.append("end_header")
for _ in range(60):
    row = [rng.uniform(-1.5, 1.5), rng.uniform(-1.2, 1.2), rng.uniform(-1.2, 1.2)]
    row += [rng.uniform(-1.5, 1.5) for _ in range(3)]
    row += [rng.uniform(-0.8, 0.8) for _ in range(9)]
    row += [rng.uniform(-1, 2.5)]
    row += [math.log(rng.uniform(0.08, 0.3)) for _ in range(3)]
    row += quat(rng)
    ply.append(" ".join(f(v) for v in row))
files["gs-data.ply"] = "\n".join(ply) + "\n"
worlds["gs-ply.wrl"] = world('Inline { url "gs-data.ply" }')

# ---------------------------------------------------------------------------------------------
# Stepped runtime worlds. The splats of a runtime world do not overlap on screen, so the order of
# the asynchronous sort does not change the picture while a rebuild is under way.

def spread_cloud(n, seed, y0=1.2):
    rng = random.Random(seed)
    pos, ori, sc, op, sh0 = [], [], [], [], []
    for i in range(n):
        pos.append([-2.0 + 4.0 * i / max(n - 1, 1), y0, 0.0])
        ori.append(quat(rng))
        sc.append([0.16, 0.1, 0.1])
        op.append(0.9)
        sh0.append([rng.uniform(-1.5, 1.5) for _ in range(3)])
    return pos, ori, sc, op, sh0

pos, ori, sc, op, sh0 = spread_cloud(5, 3)
sh1 = [[0.3 * i, 0.5, -0.4] for i in range(5)]
sh1b = [[0.5, -0.3 * i, 0.4] for i in range(5)]
sh1c = [[-0.4, 0.5, 0.3 * i] for i in range(5)]

gs_rt = HEAD + """Viewpoint { position 0 0 7 }
""" + BG + """
DEF G1 Group { children [
  DEF S1 GaussianSplats {
    positions """ + vec(pos) + """
    orientations """ + vec(ori) + """
    scales """ + vec(sc) + """
    opacities """ + flat(op) + """
    sphericalHarmonicsDegree0Coef0 """ + vec(sh0) + """
  }
] }
DEF S2 GaussianSplats {
  positions [ -1.5 -1 0, 0 -1 0, 1.5 -1 0 ]
  orientations [ 0 0 0 1, 0 0 0.38 0.92, 0 0 0 1 ]
  scales [ 0.25 0.12 0.1, 0.3 0.1 0.1, 0.2 0.2 0.2 ]
  opacities [ 0.9 0.8 0.7 ]
  sphericalHarmonicsDegree0Coef0 [ 1 0 0, 0 1 0, 0 0 1 ]
}
DEF K KeySensor { }
DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOutput SFString log ""
  initializeOnly SFNode s1 USE S1
  initializeOnly SFNode s2 USE S2
  initializeOnly SFNode g1 USE G1
  url "javascript:
var made = null;
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') s1 .positions = new MFVec3f (new SFVec3f (-2, 1.2, 0), new SFVec3f (-1, 1.5, 0), new SFVec3f (0, 1.2, 0), new SFVec3f (1, 0.9, 0), new SFVec3f (2, 1.2, 0));
   if (k == 'b') s1 .opacities = new MFFloat (0.9, 0.5, 0.9, 0.3, 0.9);
   if (k == 'c') s1 .sphericalHarmonicsDegree0Coef0 = new MFVec3f (new SFVec3f (1, 0, 0), new SFVec3f (0, 1, 0), new SFVec3f (0, 0, 1), new SFVec3f (1, 1, 0), new SFVec3f (0, 1, 1));
   if (k == 'd') s1 .scales = new MFVec3f (new SFVec3f (0.1, 0.1, 0.1), new SFVec3f (0.2, 0.1, 0.1), new SFVec3f (0.3, 0.1, 0.1), new SFVec3f (0.2, 0.2, 0.1), new SFVec3f (0.1, 0.3, 0.1));
   if (k == 'e') s1 .colorSpace = 'LIN_REC709_DISPLAY';
   if (k == 'f') s1 .colorSpace = 'SRGB_REC709_DISPLAY';
   if (k == 'g') { s1 .sphericalHarmonicsDegree1Coef0 = new MFVec3f (new SFVec3f (0.5, 0, 0), new SFVec3f (0, 0.5, 0), new SFVec3f (0, 0, 0.5), new SFVec3f (0.5, 0.5, 0), new SFVec3f (0, 0.5, 0.5));
                   s1 .sphericalHarmonicsDegree1Coef1 = new MFVec3f (new SFVec3f (0.5, 0.2, 0), new SFVec3f (0, 0.5, 0.2), new SFVec3f (0.2, 0, 0.5), new SFVec3f (0.5, 0.5, 0.2), new SFVec3f (0.2, 0.5, 0.5));
                   s1 .sphericalHarmonicsDegree1Coef2 = new MFVec3f (new SFVec3f (-0.5, 0, 0.3), new SFVec3f (0.3, -0.5, 0), new SFVec3f (0, 0.3, -0.5), new SFVec3f (-0.5, -0.5, 0), new SFVec3f (0, -0.5, -0.5)); }
   if (k == 'h') s1 .sphericalHarmonicsDegree1Coef1 = new MFVec3f ();
   if (k == 'i') s1 .visible = false;
   if (k == 'j') s1 .visible = true;
   if (k == 'k') g1 .removeChildren = new MFNode (s1);
   if (k == 'l') g1 .addChildren = new MFNode (s1);
   if (k == 'm') { made = Browser .currentScene .createNode ('GaussianSplats'); made .positions = new MFVec3f (new SFVec3f (-1, 0, 0), new SFVec3f (1, 0, 0)); made .orientations = new MFQuaternion (new SFQuaternion (0, 0, 0, 1), new SFQuaternion (0, 0, 0.38, 0.92)); made .scales = new MFVec3f (new SFVec3f (0.3, 0.3, 0.3), new SFVec3f (0.2, 0.4, 0.3)); made .opacities = new MFFloat (0.9, 0.9); made .sphericalHarmonicsDegree0Coef0 = new MFVec3f (new SFVec3f (1, 1, 0), new SFVec3f (0, 1, 1)); g1 .addChildren = new MFNode (made); }
   if (k == 'n') made .positions = new MFVec3f (new SFVec3f (-1, 0.3, 0), new SFVec3f (1, -0.3, 0));
   if (k == 'o') g1 .removeChildren = new MFNode (made);
   if (k == 'p') s2 .positions = new MFVec3f ();
   if (k == 'q') s2 .positions = new MFVec3f (new SFVec3f (-1.5, -1, 0), new SFVec3f (0, -1, 0), new SFVec3f (1.5, -1, 0));
   if (k == 'r') s2 .castShadow = false;
   if (k == 's') s2 .pointerEvents = true;
   if (k == 't') s1 .orientations = new MFQuaternion (new SFQuaternion (0, 0, 0.38, 0.92), new SFQuaternion (0, 0, 0.7, 0.7), new SFQuaternion (0, 0, 0, 1), new SFQuaternion (0, 0, -0.38, 0.92), new SFQuaternion (0, 0, 0.92, 0.38));
}
"
}
ROUTE K.keyPress TO R.on_key
"""
worlds["gs-runtime.wrl"] = gs_rt
scripts["gs-runtime.json"] = runtime_json(330, list("abcdefghijklmnopqrst"), step=14, start=18)

# ROUTE driven: a CoordinateInterpolator moves the positions, a ScalarInterpolator is not an MFFloat
# source, so the opacities come from a Script output; the splats do not overlap.
route_world = HEAD + """Viewpoint { position 0 0 7 }
""" + BG + """
DEF S1 GaussianSplats {
  positions [ -1.5 0.8 0, 0 0.8 0, 1.5 0.8 0 ]
  orientations [ 0 0 0 1, 0 0 0 1, 0 0 0 1 ]
  scales [ 0.3 0.15 0.1, 0.3 0.15 0.1, 0.3 0.15 0.1 ]
  opacities [ 0.9 0.9 0.9 ]
  sphericalHarmonicsDegree0Coef0 [ 1 0.2 0, 0 1 0.2, 0.2 0 1 ]
}
DEF T TimeSensor { cycleInterval 2 loop TRUE }
DEF CI CoordinateInterpolator {
  key [ 0 1 ]
  keyValue [ -1.5 0.8 0, 0 0.8 0, 1.5 0.8 0,  -1.5 -0.8 0, 0 -0.2 0, 1.5 -0.8 0 ]
}
DEF K KeySensor { }
DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  outputOnly MFFloat opacity_out
  outputOnly MFVec3f sh_out
  url "javascript:
function on_key (k)
{
   if (k == 'a') opacity_out = new MFFloat (0.9, 0.4, 0.9);
   if (k == 'b') sh_out = new MFVec3f (new SFVec3f (0, 1, 1), new SFVec3f (1, 0, 1), new SFVec3f (1, 1, 0));
}
"
}
ROUTE K.keyPress TO R.on_key
ROUTE R.opacity_out TO S1.set_opacities
ROUTE R.sh_out TO S1.set_sphericalHarmonicsDegree0Coef0
ROUTE T.fraction_changed TO CI.set_fraction
ROUTE CI.value_changed TO S1.set_positions
"""
worlds["gs-route.wrl"] = route_world
route_json = runtime_json(160, ["a", "b"], step=16, start=16)
scripts["gs-route.json"] = route_json

# ---------------------------------------------------------------------------------------------
for name, text in worlds.items():
    open(os.path.join(OUT, name), "w").write(text)
for name, obj in scripts.items():
    json.dump(obj, open(os.path.join(OUT, name), "w"), indent=1)
for name, text in files.items():
    open(os.path.join(OUT, name), "w").write(text)
print(len(worlds), "worlds,", len(scripts), "scripts,", len(files), "files in", OUT)
