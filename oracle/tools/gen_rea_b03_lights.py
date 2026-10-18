#!/usr/bin/env python3
"""REA batch 03 light scope worlds (light-*.wrl) for oracle/synthetic-rea-b03.

  python3 -I oracle/tools/gen_rea_b03_lights.py oracle/synthetic-rea-b03

X3DLightNode.push: a `global TRUE` light lights every shape of its layer, a `global FALSE` light
the shapes of its grouping node (DirectionalLight defaults to FALSE, PointLight and SpotLight to
TRUE). Each shape gets the global lights, then the local ones, at most 8.
"""
import json, os, sys

HEAD = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Lighting:3
COMPONENT Layering:1
NavigationInfo { headlight FALSE }
Viewpoint { position 0 0 16 }
"""

def ball(x, y, mat="Material { diffuseColor 0.8 0.8 0.8 }", geo="Sphere { radius 0.9 }"):
    return f"Transform {{ translation {x} {y} 0 children Shape {{ appearance Appearance {{ material {mat} }} geometry {geo} }} }}"

worlds, scripts = {}, {}

# Scope: root local light (all root shapes), group local lights (their children only), nested
# groups, a global light deep in a group (every shape, also earlier ones), local Point and Spot
# lights, an Inline whose local light stays inside it, Switch / Collision / Billboard / Anchor / LOD.
worlds["light-scope.wrl"] = HEAD + "\n".join([
    "DirectionalLight { direction 0 0 -1 color 0.15 0.15 0.15 }",
    ball(-6, 4),
    "Group { children [ DirectionalLight { direction 1 0 0 color 1 0 0 } " + ball(-3, 4) + " Group { children [ DirectionalLight { direction -1 0 0 color 0 0 1 } " + ball(0, 4) + " ] } " + ball(3, 4) + " ] }",
    ball(6, 4),
    "Transform { translation 0 0 0 children [ " + ball(-6, 0) + " PointLight { global FALSE location -3 0 3 color 0 1 0 radius 20 } " + ball(-3, 0) + " ] }",
    ball(0, 0),
    "Transform { children [ SpotLight { global FALSE location 3 0 4 direction 0 0 -1 cutOffAngle 0.6 beamWidth 0.3 color 1 1 0 } " + ball(3, 0) + " ] }",
    "Transform { children [ Transform { children [ DirectionalLight { global TRUE direction 0 -1 -1 color 0.2 0.1 0.3 intensity 0.8 } ] } ] }",
    ball(6, 0),
    'Inline { url "light-scope-inline.wrl" }',
    ball(-3, -4),
    "Switch { whichChoice 0 children [ Group { children [ DirectionalLight { direction 0 1 -1 color 0 1 1 } " + ball(0, -4) + " ] } ] }",
    "Collision { children [ DirectionalLight { direction 0 1 0 color 1 0 1 } " + ball(3, -4) + " ] }",
    "Billboard { children [ DirectionalLight { direction 1 1 -1 color 1 0.5 0 } " + ball(6, -4) + " ] }",
    "Anchor { children [ PointLight { global FALSE location 0 -7 3 color 0.5 0.5 1 } " + ball(-3, -7) + " ] }",
    "LOD { range [ 1000 ] level [ Group { children [ DirectionalLight { direction -1 1 -1 color 0.6 1 0.2 } " + ball(0, -7) + " ] } Group { } ] }",
    ball(3, -7),
]) + "\n"
worlds["light-scope-inline.wrl"] = """#X3D V4.0 utf8
PROFILE Interchange
DirectionalLight { direction 0 -1 -1 color 1 0.3 0.3 }
""" + ball(-6, -4) + "\n"

# More than 8 lights for one shape: globals first, then local ones, the rest dropped.
lights = []
for i in range(6):
    lights.append(f"PointLight {{ location {-7 + 3 * i} 6 4 color {0.3 + 0.1 * i} 0.2 {0.9 - 0.1 * i} intensity 0.6 radius 30 }}")
worlds["light-scope-max.wrl"] = HEAD + "\n".join(lights + [
    "Group { children [ " + " ".join(f"DirectionalLight {{ direction {(-1) ** i} {0.2 * i - 0.5} -1 color {0.1 * i} 0.6 {1 - 0.1 * i} intensity 0.5 }}" for i in range(5)) + " " + ball(-3, 0, geo="Box { size 2 2 2 }") + " ] }",
    ball(3, 0, geo="Box { size 2 2 2 }"),
]) + "\n"

# Physical and Phong materials with the same local lights.
worlds["light-scope-pbr.wrl"] = HEAD + "\n".join([
    "Group { children [ DirectionalLight { direction 1 -1 -1 color 1 0.8 0.6 intensity 2 } "
    + ball(-4, 2, "PhysicalMaterial { baseColor 0.8 0.3 0.2 roughness 0.4 metallic 0.2 }") + " "
    + ball(0, 2, "Material { diffuseColor 0.8 0.3 0.2 specularColor 0.5 0.5 0.5 shininess 0.3 }") + " ] }",
    ball(4, 2, "PhysicalMaterial { baseColor 0.8 0.3 0.2 roughness 0.4 metallic 0.2 }"),
    "Group { children [ SpotLight { global FALSE location 0 -2 5 direction 0 0 -1 cutOffAngle 0.5 beamWidth 0.4 color 0.4 0.7 1 intensity 3 } "
    + ball(-2, -2, "PhysicalMaterial { baseColor 0.9 0.9 0.9 roughness 0.2 }") + " "
    + ball(2, -2, "Material { diffuseColor 0.9 0.9 0.9 }") + " ] }",
    ball(0, -6, "PhysicalMaterial { baseColor 0.9 0.9 0.9 }"),
]) + "\n"

# Runtime: global, on and intensity changes of local lights.
worlds["light-scope-runtime.wrl"] = HEAD.replace("COMPONENT Layering:1\n", "COMPONENT Layering:1\nCOMPONENT Scripting:1\nCOMPONENT KeyDeviceSensor:1\n") + """Group { children [ DEF L1 DirectionalLight { direction 1 0 -1 color 1 0.2 0.2 } """ + ball(-3, 2) + """ ] }
""" + ball(3, 2) + """
Group { children [ DEF L2 PointLight { location 0 -2 3 color 0.2 1 0.2 } """ + ball(-3, -2) + """ ] }
""" + ball(3, -2) + """
DEF K KeySensor { }
DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  initializeOnly SFNode l1 USE L1
  initializeOnly SFNode l2 USE L2
  url "javascript:
function on_key (k)
{
   if (k == 'g') l1 .global = true;
   if (k == 'h') l1 .global = false;
   if (k == 'p') l2 .global = false;
   if (k == 'o') l1 .on = false;
   if (k == 'n') l1 .on = true;
   if (k == 'i') l2 .intensity = 0.3;
}
"
}
ROUTE K.keyPress TO R.on_key
"""
FOCUS = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507}, {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
keys = [(20, "g"), (40, "h"), (60, "p"), (80, "o"), (100, "n"), (120, "i")]
scripts["light-scope-runtime.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 140, "capture": [15, 35, 55, 75, 95, 115, 135], "dump": [15, 135], "events": FOCUS + [{"frame": f, "type": "key", "key": k} for f, k in keys]}

# VRML 2.0 / X3D 3.0: DirectionalLight starts global TRUE (DirectionalLight constructor, Legacy).
for name, head in [("light-scope-vrml97.wrl", "#VRML V2.0 utf8\n"), ("light-scope-x3d30.wrl", "#X3D V3.0 utf8\nPROFILE Interchange\n"), ("light-scope-x3d31.wrl", "#X3D V3.1 utf8\nPROFILE Interchange\n")]:
    worlds[name] = head + "NavigationInfo { headlight FALSE }\nViewpoint { position 0 0 16 }\n" + "\n".join([
        "Group { children [ DirectionalLight { direction 1 0 -1 color 1 0.2 0.2 } " + ball(-3, 0) + " ] }",
        ball(3, 0),
    ]) + "\n"

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
for name, text in worlds.items():
    with open(os.path.join(out, name), "w") as f:
        f.write(text)
for name, script in scripts.items():
    with open(os.path.join(out, name), "w") as f:
        json.dump(script, f, indent=1)
print(len(worlds), "worlds,", len(scripts), "scripts")
