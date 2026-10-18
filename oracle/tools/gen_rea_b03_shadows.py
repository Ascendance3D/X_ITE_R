#!/usr/bin/env python3
"""REA batch 03 shadow worlds (shadow-*.wrl) for oracle/synthetic-rea-b03.

  python3 -I oracle/tools/gen_rea_b03_shadows.py oracle/synthetic-rea-b03

X3DLightNode shadows: shadowColor, shadowIntensity, shadowBias, shadowMapSize for Directional
(orthographic box around the group's shadow casters), Spot (perspective of cutOffAngle) and Point
lights (six views in one map), global and local lights, castShadow, clip planes, Phong and
physical receivers, LayerSet, runtime changes.
"""
import json, os, sys

HEAD = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Lighting:3
COMPONENT Layering:1
COMPONENT Rendering:5
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
COMPONENT Interpolation:1
COMPONENT Time:1
NavigationInfo { headlight FALSE }
Viewpoint { position 0.013 6.07 16.11 orientation 1 0 0 -0.3517 }
"""

def shape(geo, x=0, y=0, z=0, mat="Material { diffuseColor 0.8 0.8 0.8 }", extra=""):
    return f"Transform {{ translation {x} {y} {z} children Shape {{ {extra} appearance Appearance {{ material {mat} }} geometry {geo} }} }}"

FLOOR = shape("Box { size 16 0.2 12 }", 0, -1, 0)
STUFF = "\n".join([
    shape("Sphere { radius 1 }", -3, 1, 0, "Material { diffuseColor 0.9 0.3 0.2 }"),
    shape("Box { size 1.5 1.5 1.5 }", 0, 0.5, 1, "Material { diffuseColor 0.2 0.8 0.3 }"),
    shape("Cone { bottomRadius 1 height 2 }", 3, 0.5, -1, "Material { diffuseColor 0.3 0.4 0.9 }"),
    shape("Cylinder { radius 0.4 height 3 }", 1.5, 1, -3, "Material { diffuseColor 0.9 0.9 0.2 }"),
])
AMBIENT = "DirectionalLight { global TRUE direction 0 -1 0 intensity 0.25 }"

worlds, scripts = {}, {}

worlds["shadow-directional.wrl"] = HEAD + "\n".join([AMBIENT, "DirectionalLight { direction -0.5 -1 -0.4 shadows TRUE }", FLOOR, STUFF]) + "\n"
worlds["shadow-directional-color.wrl"] = HEAD + "\n".join([AMBIENT, "DirectionalLight { direction 0.6 -1 0.2 color 1 0.9 0.8 shadows TRUE shadowColor 0.2 0 0.4 shadowIntensity 0.6 shadowBias 0.02 }", FLOOR, STUFF]) + "\n"
worlds["shadow-mapsize.wrl"] = HEAD + "\n".join([AMBIENT, "DirectionalLight { direction -0.5 -1 -0.4 shadows TRUE shadowMapSize 64 }", FLOOR, STUFF]) + "\n"
worlds["shadow-mapsize-odd.wrl"] = HEAD + "\n".join([AMBIENT, "DirectionalLight { direction 0.3 -1 0.5 shadows TRUE shadowMapSize 333 shadowBias 0.001 }", FLOOR, STUFF]) + "\n"
# A local light in a moved and rotated Transform (the group's matrix is applied again in the
# shadow pass), and shapes outside the group (not lit, not casting).
worlds["shadow-local.wrl"] = HEAD + "\n".join([AMBIENT,
    "Transform { translation 1 0.5 0 rotation 0 1 0 0.3 children [ DirectionalLight { direction -0.4 -1 -0.3 shadows TRUE } " + FLOOR + " " + shape("Sphere { radius 1 }", -2, 1, 0, "Material { diffuseColor 0.9 0.3 0.2 }") + " ] }",
    shape("Box { size 1.5 1.5 1.5 }", 3, 1.5, 0, "Material { diffuseColor 0.2 0.8 0.3 }"),
]) + "\n"
worlds["shadow-global-in-group.wrl"] = HEAD + "\n".join([AMBIENT,
    "Transform { translation 2 1 0 children [ DirectionalLight { global TRUE direction -0.5 -1 0.3 shadows TRUE } ] }", FLOOR, STUFF]) + "\n"
worlds["shadow-spot.wrl"] = HEAD + "\n".join([AMBIENT, "SpotLight { location 0 7 3 direction 0 -1 -0.3 cutOffAngle 0.9 beamWidth 0.7 radius 50 shadows TRUE }", FLOOR, STUFF]) + "\n"
worlds["shadow-spot-local.wrl"] = HEAD + "\n".join([AMBIENT, "Transform { translation -1 0 0 children [ SpotLight { global FALSE location 2 6 2 direction -0.2 -1 -0.2 cutOffAngle 0.7 radius 40 shadows TRUE shadowIntensity 0.8 } " + FLOOR + " " + STUFF + " ] }"]) + "\n"
worlds["shadow-point.wrl"] = HEAD + "\n".join([AMBIENT, "PointLight { location 0 4 0 radius 50 shadows TRUE }", FLOOR, STUFF]) + "\n"
worlds["shadow-point-low.wrl"] = HEAD + "\n".join([AMBIENT, "PointLight { location 0.5 1.2 2.5 radius 50 attenuation 1 0 0 shadows TRUE shadowMapSize 256 }", FLOOR, STUFF]) + "\n"
worlds["shadow-castshadow.wrl"] = HEAD + "\n".join([AMBIENT, "DirectionalLight { direction -0.5 -1 -0.4 shadows TRUE }", FLOOR,
    shape("Sphere { radius 1 }", -3, 1, 0, "Material { diffuseColor 0.9 0.3 0.2 }", "castShadow FALSE"),
    shape("Box { size 1.5 1.5 1.5 }", 0, 0.5, 1, "Material { diffuseColor 0.2 0.8 0.3 }"),
    "Transform { translation 3 0.5 -1 visible FALSE children Shape { appearance Appearance { material Material { } } geometry Cone { } } }",
    "Group { bboxSize 2 2 2 children " + shape("Cylinder { radius 0.4 height 3 }", 1.5, 1, -3, "Material { diffuseColor 0.9 0.9 0.2 }") + " }",
]) + "\n"
worlds["shadow-clip.wrl"] = HEAD + "\n".join([AMBIENT, "DirectionalLight { direction -0.5 -1 -0.4 shadows TRUE }", FLOOR,
    "Group { children [ ClipPlane { plane 0 -1 0 1.2 } " + STUFF + " ] }"]) + "\n"
worlds["shadow-pbr.wrl"] = HEAD + "\n".join([AMBIENT, "DirectionalLight { direction -0.5 -1 -0.4 intensity 2 shadows TRUE shadowColor 0.1 0.1 0.3 }",
    shape("Box { size 16 0.2 12 }", 0, -1, 0, "PhysicalMaterial { baseColor 0.8 0.8 0.8 roughness 0.6 }"),
    shape("Sphere { radius 1 }", -3, 1, 0, "PhysicalMaterial { baseColor 0.9 0.3 0.2 metallic 0.5 roughness 0.3 }"),
    shape("Box { size 1.5 1.5 1.5 }", 0, 0.5, 1, "Material { diffuseColor 0.2 0.8 0.3 }"),
    shape("Cone { bottomRadius 1 height 2 }", 3, 0.5, -1, "PhysicalMaterial { baseColor 0.3 0.4 0.9 }"),
    "PointLight { location -4 3 3 color 1 0.8 0.6 radius 50 shadows TRUE shadowIntensity 0.7 }",
]) + "\n"
worlds["shadow-multi.wrl"] = HEAD + "\n".join([AMBIENT,
    "DirectionalLight { direction -0.5 -1 -0.4 intensity 0.6 shadows TRUE shadowMapSize 512 }",
    "SpotLight { location 4 6 2 direction -0.5 -1 -0.2 cutOffAngle 0.8 color 1 0.6 0.4 radius 40 shadows TRUE shadowMapSize 256 }",
    "PointLight { location -4 3 2 color 0.4 0.6 1 intensity 0.6 radius 40 shadows TRUE shadowMapSize 128 }",
    FLOOR, STUFF]) + "\n"
worlds["shadow-layers.wrl"] = HEAD + "\n".join([
    "LayerSet { order [ 1 2 ] layers [",
    "  Layer { children [ NavigationInfo { headlight FALSE } Viewpoint { position 0.013 6.07 16.11 orientation 1 0 0 -0.3517 } " + AMBIENT + " DirectionalLight { direction -0.5 -1 -0.4 shadows TRUE } Transform { translation -4 0 0 children [ " + FLOOR + " ] } " + shape("Sphere { radius 1 }", -4, 1, 0) + " ] }",
    "  Layer { viewport Viewport { clipBoundary [ 0.5 1 0 1 ] } children [ NavigationInfo { headlight FALSE } Viewpoint { position 0.013 6.07 16.11 orientation 1 0 0 -0.3517 } " + AMBIENT + " SpotLight { location 0 7 3 direction 0 -1 -0.3 cutOffAngle 0.9 radius 50 shadows TRUE } " + FLOOR + " " + STUFF + " ] }",
    "] }"]) + "\n"

# Runtime: shadows on/off, intensity, color, map size, bias, a moving caster.
worlds["shadow-runtime.wrl"] = HEAD + "\n".join([AMBIENT,
    "DEF L DirectionalLight { direction -0.5 -1 -0.4 shadows TRUE }",
    "DEF P PointLight { location 3 3 2 radius 30 intensity 0.6 }",
    FLOOR,
    "DEF M Transform { translation -3 1 0 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.3 0.2 } } geometry Sphere { } } }",
    shape("Box { size 1.5 1.5 1.5 }", 0, 0.5, 1, "Material { diffuseColor 0.2 0.8 0.3 }"),
    "DEF C TimeSensor { cycleInterval 4 loop TRUE }",
    "DEF PI PositionInterpolator { key [ 0 0.5 1 ] keyValue [ -3 1 0, 3 2 -1, -3 1 0 ] }",
    "ROUTE C.fraction_changed TO PI.set_fraction",
    "ROUTE PI.value_changed TO M.set_translation",
    "DEF K KeySensor { }",
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  initializeOnly SFNode l USE L
  initializeOnly SFNode p USE P
  url "javascript:
function on_key (k)
{
   if (k == 'a') l .shadows = false;
   if (k == 'b') l .shadows = true;
   if (k == 'c') l .shadowIntensity = 0.4;
   if (k == 'd') l .shadowColor = new SFColor (0.5, 0, 0);
   if (k == 'e') l .shadowMapSize = 128;
   if (k == 'f') p .shadows = true;
   if (k == 'g') l .direction = new SFVec3f (0.5, -1, 0.2);
}
"
}""",
    "ROUTE K.keyPress TO R.on_key",
]) + "\n"
FOCUS = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507}, {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
keys = [(20, "a"), (30, "b"), (40, "c"), (50, "d"), (60, "e"), (70, "f"), (80, "g")]
scripts["shadow-runtime.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 100, "capture": [15, 25, 35, 45, 55, 65, 75, 85, 99], "dump": [99], "events": FOCUS + [{"frame": f, "type": "key", "key": k} for f, k in keys]}

# Cross-system: a GeneratedCubeMapTexture pass (DependentRenderer) uses the main pass's shadow
# maps; an EnvironmentLight with shadows on the physical receivers; Phong, Unlit and textured
# shapes; MultiTexture on a shadowed floor.
worlds["shadow-cubemap.wrl"] = HEAD.replace("COMPONENT Time:1\n", "COMPONENT Time:1\nCOMPONENT CubeMapTexturing:3\n") + "\n".join([AMBIENT, "DirectionalLight { direction -0.5 -1 -0.4 shadows TRUE }", FLOOR, STUFF,
    "Transform { translation -0.5 3.2 2 children Shape { appearance Appearance { material Material { diffuseColor 0 0 0 specularColor 0.3 0.3 0.3 } texture GeneratedCubeMapTexture { update \"ALWAYS\" size 128 } } geometry Sphere { radius 0.9 } } }"]) + "\n"
worlds["shadow-environment.wrl"] = HEAD + "\n".join([
    "EnvironmentLight { intensity 0.6 specularTexture ComposedCubeMapTexture { frontTexture PixelTexture { image 1 1 3 0x8080ff } backTexture PixelTexture { image 1 1 3 0xff8080 } leftTexture PixelTexture { image 1 1 3 0x80ff80 } rightTexture PixelTexture { image 1 1 3 0xffff80 } topTexture PixelTexture { image 1 1 3 0xffffff } bottomTexture PixelTexture { image 1 1 3 0x404040 } } }",
    "DirectionalLight { direction -0.5 -1 -0.4 intensity 1.5 shadows TRUE }",
    shape("Box { size 16 0.2 12 }", 0, -1, 0, "PhysicalMaterial { baseColor 0.8 0.8 0.8 roughness 0.5 }"),
    shape("Sphere { radius 1 }", -3, 1, 0, "PhysicalMaterial { baseColor 0.9 0.3 0.2 metallic 0.8 roughness 0.2 }"),
    shape("Box { size 1.5 1.5 1.5 }", 0, 0.5, 1, "UnlitMaterial { emissiveColor 0.2 0.8 0.3 }"),
    shape("Cone { bottomRadius 1 height 2 }", 3, 0.5, -1, "PhysicalMaterial { baseColor 0.3 0.4 0.9 }"),
]) + "\n"
worlds["shadow-multitexture.wrl"] = HEAD + "\n".join([AMBIENT, "DirectionalLight { direction -0.5 -1 -0.4 shadows TRUE }",
    "Transform { translation 0 -1 0 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture MultiTexture { mode [ \"MODULATE\" \"ADD\" ] texture [ PixelTexture { image 2 2 3 0xff0000 0x00ff00 0x0000ff 0xffffff } PixelTexture { image 1 1 3 0x202020 } ] } } geometry Box { size 16 0.2 12 } } }",
    STUFF]) + "\n"

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
for name, text in worlds.items():
    with open(os.path.join(out, name), "w") as f:
        f.write(text)
for name, script in scripts.items():
    with open(os.path.join(out, name), "w") as f:
        json.dump(script, f, indent=1)
print(len(worlds), "worlds,", len(scripts), "scripts")
