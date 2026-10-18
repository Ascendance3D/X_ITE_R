#!/usr/bin/env python3
"""REA batch 02: authored GeneratedCubeMapTexture worlds (no archive content).

  python3 -I oracle/tools/gen_rea_b02_gencube.py oracle/synthetic-rea-b02
"""
import math, os, sys

out = sys.argv[1]

def sphere_ifs(r=0.8, n=32, m=16):
    pts, idx = [], []
    for j in range(m + 1):
        phi = math.pi * j / m
        for i in range(n + 1):
            th = 2 * math.pi * i / n
            pts.append((r * math.sin(phi) * math.sin(th), r * math.cos(phi), r * math.sin(phi) * math.cos(th)))
    for j in range(m):
        for i in range(n):
            a = j * (n + 1) + i
            idx += [a, a + n + 1, a + n + 2, a + 1, -1]
    return "Coordinate { point [ " + ", ".join("%.5f %.5f %.5f" % p for p in pts) + " ] }", " ".join(map(str, idx))

COORD, INDEX = sphere_ifs()
REFL = 'TextureCoordinateGenerator { mode "CAMERASPACEREFLECTIONVECTOR" }'

def mirror(defname, update, size, x=0, y=0, material="UnlitMaterial { }", extra=""):
    return f"""Transform {{ translation {x} {y} 0 children Shape {{
  appearance Appearance {{ material {material} texture DEF {defname} GeneratedCubeMapTexture {{ update "{update}" size {size}{extra} }} }}
  geometry IndexedFaceSet {{ creaseAngle 3.2 coord {COORD} coordIndex [ {INDEX} ] texCoord {REFL} }}
}} }}"""

ROOM = """DEF ROOM Group { children [
  Transform { translation 3 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.2 0.1 } } geometry Box { size 0.5 2 2 } } }
  Transform { translation -3 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.1 0.8 0.2 } } geometry Box { size 0.5 2 2 } } }
  Transform { translation 0 3 0 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.3 0.9 } } geometry Box { size 2 0.5 2 } } }
  Transform { translation 0 -3 0 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.9 0.1 } } geometry Box { size 2 0.5 2 } } }
  Transform { translation 0 0 -3 children Shape { appearance Appearance { material Material { diffuseColor 0.8 0.2 0.8 } } geometry Box { size 2 2 0.5 } } }
  Transform { translation 1.2 1.2 2.5 children Shape { appearance Appearance { material Material { diffuseColor 0.1 0.8 0.8 } } geometry Sphere { radius 0.4 } } }
] }
DirectionalLight { direction 0.3 -0.5 -0.8 intensity 0.6 }
"""

HEAD = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT CubeMapTexturing:3
COMPONENT Lighting:2
Viewpoint { position 0.5 0.4 5 orientation 0 1 0 0.1 }
Background { skyColor [ 0.1 0.15 0.3, 0.5 0.6 0.7 ] skyAngle 1.4 groundColor [ 0.2 0.15 0.1, 0.3 0.25 0.2 ] groundAngle 1.4 }
"""

worlds = {
    "gencube-always.wrl": HEAD + ROOM + mirror("G", "ALWAYS", 64),
    "gencube-next.wrl": HEAD + ROOM + mirror("G", "NEXT_FRAME_ONLY", 64, material="Material { diffuseColor 1 1 1 specularColor 0.5 0.5 0.5 }"),
    "gencube-lit.wrl": HEAD + ROOM + mirror("G", "ALWAYS", 64, material="Material { diffuseColor 1 1 1 specularColor 0.5 0.5 0.5 }"),
    "gencube-none.wrl": HEAD + ROOM + mirror("G", "NONE", 64),
    "gencube-small.wrl": HEAD + ROOM + mirror("G", "ALWAYS", 8),
    "gencube-noheadlight.wrl": HEAD + "NavigationInfo { headlight FALSE }\n" + ROOM + mirror("G", "ALWAYS", 32, extra=' textureProperties TextureProperties { minificationFilter "NEAREST_PIXEL" magnificationFilter "NEAREST_PIXEL" }'),
}
for name, text in worlds.items():
    open(os.path.join(out, name), "w").write(text + "\n")

# Runtime: update modes, size and scene changes driven by a root TouchSensor (four clicks).
RUNTIME = HEAD.replace("COMPONENT Lighting:2", "COMPONENT Lighting:2\nCOMPONENT Scripting:1") + """NavigationInfo { type "NONE" }
DEF TS TouchSensor { }
""" + ROOM.replace("Transform { translation 1.2 1.2 2.5", "DEF MOVER Transform { translation 1.2 1.2 2.5") + mirror("G1", "ALWAYS", 64, x=-0.9) + "\n" + mirror("G2", "NEXT_FRAME_ONLY", 64, x=0.9) + """
DEF S Script {
  directOutput TRUE
  inputOnly SFTime touch
  initializeOnly SFNode g1 USE G1
  initializeOnly SFNode g2 USE G2
  initializeOnly SFNode mover USE MOVER
  inputOutput SFInt32 step 0
  url "javascript:
function touch (value)
{
   step = step + 1;

   if (step == 1) { mover .translation = new SFVec3f (-1.2, 1.2, 2.5); g1 .update = 'NONE'; }
   if (step == 2) { g2 .update = 'NEXT_FRAME_ONLY'; }
   if (step == 3) { g1 .size = 16; g1 .update = 'ALWAYS'; }
   if (step == 4) { mover .translation = new SFVec3f (0, -1.2, 2.5); g2 .update = 'ALWAYS'; g1 .size = 0; }
}
"
}
ROUTE TS.touchTime TO S.touch
"""
open(os.path.join(out, "gencube-runtime.wrl"), "w").write(RUNTIME)

events = []
for f in (24, 40, 56, 72):
    events += [{"frame": f, "type": "mousemove", "x": 145, "y": 300},
               {"frame": f, "type": "mousedown", "button": 0, "x": 145, "y": 300},
               {"frame": f + 1, "type": "mouseup", "button": 0, "x": 145, "y": 300}]
import json
json.dump({"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 90, "capture": [20, 36, 52, 68, 84],
           "dump": [20, 36, 52, 68, 90], "events": events}, open(os.path.join(out, "gencube-runtime.json"), "w"), indent=1)
