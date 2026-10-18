#!/usr/bin/env python3
"""REA batch 03 texture projector worlds (proj-*.wrl) for oracle/synthetic-rea-b03.

  python3 -I oracle/tools/gen_rea_b03_projectors.py oracle/synthetic-rea-b03

TextureProjector (perspective of fieldOfView, aspect of the texture) and TextureProjectorParallel
(fieldOfView rectangle): global and local, upVector (default 0 0 1 up to X3D 4.0), near and far
distances, color, intensity, texture alpha, Phong / Gouraud (legacy Material) / Unlit / Physical
receivers, several projectors, GeneratedCubeMapTexture, shadows, LayerSet, runtime changes and
aspectRatio events. proj-checker.png is generated here (a clean synthetic image).
"""
import json, os, struct, sys, zlib

def png(path, w, h, pixel):
    raw = b"".join(b"\x00" + bytes(c for x in range(w) for c in pixel(x, y)) for y in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(path, "wb").write(data)

def checker(x, y):
    # 64 × 32: 8 × 4 cells, a red border column and a blue top row (orientation), alpha 255.
    if x < 4:
        return (255, 0, 0, 255)
    if y < 4:
        return (0, 0, 255, 255)
    return (240, 240, 60, 255) if (x // 8 + y // 8) % 2 == 0 else (40, 160, 40, 255)

def version(v):
    return f"""#X3D V{v} utf8
PROFILE Interchange
COMPONENT TextureProjection:2
COMPONENT Layering:1
COMPONENT Shape:4
COMPONENT Lighting:3
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
COMPONENT CubeMapTexturing:3
Viewpoint {{ position 0.013 6.07 16.11 orientation 1 0 0 -0.3517 }}
"""
HEAD = version("4.1")

def shape(geo, x=0, y=0, z=0, mat="Material { diffuseColor 0.8 0.8 0.8 }", tex=""):
    return f"Transform {{ translation {x} {y} {z} children Shape {{ appearance Appearance {{ material {mat} {tex} }} geometry {geo} }} }}"

FLOOR = shape("Box { size 16 0.2 12 }", 0, -1, 0)
STUFF = "\n".join([
    shape("Sphere { radius 1 }", -3, 1, 0, "Material { diffuseColor 0.9 0.6 0.5 }"),
    shape("Box { size 1.5 1.5 1.5 }", 0, 0.5, 1, "Material { diffuseColor 0.6 0.9 0.6 }"),
    shape("Cone { bottomRadius 1 height 2 }", 3, 0.5, -1, "Material { diffuseColor 0.6 0.7 0.9 }"),
])
IMG = 'ImageTexture { url "proj-checker.png" }'
PIX = "PixelTexture { image 4 2 4 0xff0000ff 0x00ff0080 0x0000ffff 0xffffff00 0xffff00ff 0x00ffffff 0xff00ff80 0x000000ff }"

worlds, scripts = {}, {}
worlds["proj-basic.wrl"] = HEAD + "\n".join(["TextureProjector { location 0 5 6 direction 0 -0.7 -1 upVector 0 1 0 fieldOfView 0.9 texture " + IMG + " }", FLOOR, STUFF]) + "\n"
worlds["proj-upvector-v40.wrl"] = version("4.0") + "\n".join(["TextureProjector { location 0 6 2 direction 0 -1 -0.2 texture " + IMG + " }", FLOOR, STUFF]) + "\n"
worlds["proj-upvector-v41.wrl"] = HEAD + "\n".join(["TextureProjector { location 0 6 2 direction 0 -1 -0.2 texture " + IMG + " }", FLOOR, STUFF]) + "\n"
worlds["proj-parallel.wrl"] = HEAD + "\n".join(["TextureProjectorParallel { location 0 5 5 direction 0 -1 -0.8 upVector 0 1 0 fieldOfView -4 -2 4 3 texture " + IMG + " }", FLOOR, STUFF]) + "\n"
worlds["proj-parallel-tall.wrl"] = HEAD + "\n".join(["TextureProjectorParallel { location 1 6 0 direction 0 -1 0 upVector 0 0 -1 fieldOfView -1 -4 2 4 texture " + IMG + " }", FLOOR, STUFF]) + "\n"
worlds["proj-local.wrl"] = HEAD + "\n".join([
    "Transform { translation -1 0.5 0 rotation 0 1 0 0.4 children [ TextureProjector { global FALSE location 0 5 5 direction 0 -0.8 -1 upVector 0 1 0 fieldOfView 1.1 texture " + IMG + " } " + FLOOR + " " + shape("Sphere { radius 1 }", -2, 1, 0) + " ] }",
    shape("Box { size 1.5 1.5 1.5 }", 3, 1.5, 0, "Material { diffuseColor 0.6 0.9 0.6 }"),
]) + "\n"
worlds["proj-nearfar.wrl"] = HEAD + "\n".join(["TextureProjector { location 0 5 6 direction 0 -0.7 -1 upVector 0 1 0 fieldOfView 1.2 nearDistance 6.5 farDistance 9 texture " + IMG + " }", FLOOR, STUFF]) + "\n"
worlds["proj-pixel.wrl"] = HEAD + "\n".join(["TextureProjector { location 0 6 3 direction 0 -1 -0.4 upVector 0 1 0 fieldOfView 1.2 color 1 0.8 0.6 intensity 0.7 texture " + PIX + " }", FLOOR, STUFF]) + "\n"
worlds["proj-materials.wrl"] = HEAD + "\n".join([
    "TextureProjector { location 0 6 4 direction 0 -1 -0.5 upVector 0 1 0 fieldOfView 1.3 texture " + IMG + " }",
    shape("Box { size 16 0.2 12 }", 0, -1, 0, "PhysicalMaterial { baseColor 0.9 0.9 0.9 roughness 0.7 }"),
    shape("Sphere { radius 1 }", -4, 1, 0, "UnlitMaterial { emissiveColor 0.9 0.9 0.9 }"),
    shape("Box { size 1.5 1.5 1.5 }", -1, 0.5, 1, "Material { diffuseColor 0.9 0.9 0.9 }", "texture PixelTexture { image 2 2 3 0xffffff 0x808080 0x808080 0xffffff }"),
    shape("Cone { bottomRadius 1 height 2 }", 2, 0.5, -1, "PhysicalMaterial { baseColor 0.6 0.7 0.9 metallic 0.3 }"),
    shape("Cylinder { radius 0.6 height 2 }", 4.5, 0.5, 1, "Material { diffuseColor 0.9 0.9 0.9 }", "texture MultiTexture { texture [ PixelTexture { image 1 1 3 0xff8080 } PixelTexture { image 1 1 3 0x8080ff } ] mode [ \"MODULATE\" \"ADD\" ] }"),
    "Transform { translation 0 3 -3 children Shape { geometry PointSet { coord Coordinate { point [ -2 0 0, -1 0 0, 0 0 0, 1 0 0, 2 0 0 ] } } } }",
    "Transform { translation 0 2.5 -3 children Shape { geometry IndexedLineSet { coord Coordinate { point [ -3 0 0, 3 0 0 ] } coordIndex [ 0 1 ] } } }",
]) + "\n"
# A legacy Material (X3D 3.3: X3D_GOURAUD_MATERIAL, where projection has no effect) next to the
# same scene in Shading PHONG is in proj-legacy-*.wrl.
worlds["proj-legacy-gouraud.wrl"] = version("3.3").replace("COMPONENT TextureProjection:2\n", "COMPONENT TextureProjection:2\n") + "\n".join(["TextureProjector { location 0 5 6 direction 0 -0.7 -1 upVector 0 1 0 fieldOfView 0.9 texture " + IMG + " }", FLOOR, STUFF]) + "\n"
worlds["proj-multi.wrl"] = HEAD + "\n".join([
    "TextureProjector { location -3 6 2 direction 0.2 -1 -0.2 upVector 0 1 0 fieldOfView 0.9 color 1 0.5 0.5 texture " + IMG + " }",
    "TextureProjector { location 3 6 2 direction -0.2 -1 -0.2 upVector 0 1 0 fieldOfView 0.9 color 0.5 0.5 1 texture " + PIX + " }",
    "TextureProjectorParallel { location 0 6 0 direction 0 -1 0 upVector 0 0 -1 fieldOfView -2 -2 2 2 intensity 0.5 texture " + IMG + " }",
    "Group { children [ TextureProjector { global FALSE location 0 6 4 direction 0 -1 -0.6 upVector 0 1 0 texture " + PIX + " } TextureProjector { global FALSE location 1 6 4 direction 0 -1 -0.6 upVector 0 1 0 texture " + IMG + " } " + FLOOR + " ] }",
    STUFF,
]) + "\n"
worlds["proj-cubemap.wrl"] = HEAD + "\n".join(["TextureProjector { location 0 5 6 direction 0 -0.7 -1 upVector 0 1 0 fieldOfView 0.9 texture " + IMG + " }", FLOOR, STUFF,
    "Transform { translation -0.5 3.2 2 children Shape { appearance Appearance { material Material { diffuseColor 0 0 0 specularColor 0.3 0.3 0.3 } texture GeneratedCubeMapTexture { update \"ALWAYS\" size 128 } } geometry Sphere { radius 0.9 } } }"]) + "\n"
worlds["proj-shadow.wrl"] = HEAD + "\n".join(["NavigationInfo { headlight FALSE }", "DirectionalLight { global TRUE direction 0 -1 0 intensity 0.3 }", "DirectionalLight { direction -0.5 -1 -0.4 shadows TRUE }",
    "TextureProjector { location 0 5 6 direction 0 -0.7 -1 upVector 0 1 0 fieldOfView 0.9 texture " + IMG + " }", FLOOR, STUFF]) + "\n"
worlds["proj-layers.wrl"] = HEAD + "\n".join([
    "LayerSet { order [ 1 2 ] layers [",
    "  Layer { children [ Viewpoint { position 0.013 6.07 16.11 orientation 1 0 0 -0.3517 } TextureProjector { location -4 5 6 direction 0 -0.7 -1 upVector 0 1 0 fieldOfView 0.9 texture " + IMG + " } Transform { translation -4 0 0 children [ " + FLOOR + " ] } ] }",
    "  Layer { viewport Viewport { clipBoundary [ 0.5 1 0 1 ] } children [ Viewpoint { position 0.013 6.07 16.11 orientation 1 0 0 -0.3517 } TextureProjectorParallel { location 0 5 5 direction 0 -1 -0.8 upVector 0 1 0 fieldOfView -4 -2 4 3 texture " + PIX + " } " + STUFF + " ] }",
    "] }"]) + "\n"
worlds["proj-runtime.wrl"] = HEAD + "\n".join([
    "DEF P TextureProjector { location 0 5 6 direction 0 -0.7 -1 upVector 0 1 0 fieldOfView 0.9 texture DEF T1 " + IMG + " }",
    FLOOR, STUFF,
    "DEF K KeySensor { }",
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOnly SFFloat on_aspect
  inputOutput SFString log ""
  initializeOnly SFNode p USE P
  url "javascript:
function on_aspect (value) { log = log + 'A' + value .toFixed (4) + ' '; }
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') p .location = new SFVec3f (2, 5, 5);
   if (k == 'b') p .on = false;
   if (k == 'c') p .on = true;
   if (k == 'd') p .intensity = 0.4;
   if (k == 'e') p .texture = Browser .currentScene .createNode ('PixelTexture');
   if (k == 'f') { var t = Browser .currentScene .createNode ('PixelTexture'); t .image = new SFImage (3, 1, 3, new MFInt32 (0xff0000, 0x00ff00, 0x0000ff)); p .texture = t; }
   if (k == 'g') p .fieldOfView = 0.5;
   if (k == 'h') p .global = false;
}
"
}""",
    "ROUTE K.keyPress TO R.on_key",
    "ROUTE P.aspectRatio TO R.on_aspect",
]) + "\n"
FOCUS = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507}, {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
keys = [(20, "a"), (30, "b"), (40, "c"), (50, "d"), (60, "e"), (70, "f"), (80, "g"), (90, "h")]
scripts["proj-runtime.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 100, "capture": [15, 25, 35, 45, 55, 65, 75, 85, 99], "dump": [10, 15, 25, 35, 45, 55, 65, 75, 85, 95, 99], "events": FOCUS + [{"frame": f, "type": "key", "key": k} for f, k in keys]}

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
png(os.path.join(out, "proj-checker.png"), 64, 32, checker)
for name, text in worlds.items():
    with open(os.path.join(out, name), "w") as f:
        f.write(text)
for name, script in scripts.items():
    with open(os.path.join(out, name), "w") as f:
        json.dump(script, f, indent=1)
print(len(worlds), "worlds,", len(scripts), "scripts")
