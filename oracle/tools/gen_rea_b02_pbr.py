#!/usr/bin/env python3
"""REA batch 02: authored physical material, environment light and tone mapping worlds, and their
PNG images (no archive content).

  python3 -I oracle/tools/gen_rea_b02_pbr.py oracle/synthetic-rea-b02
"""
import json, math, os, struct, sys, zlib

out = sys.argv[1]
os.makedirs(os.path.join(out, "pbr"), exist_ok=True)


def png(path, w, h, pixel):
    raw = b"".join(b"\0" + bytes(max(0, min(255, int(c))) for x in range(w) for c in pixel(x, y)) for y in range(h))

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)

    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(os.path.join(out, path), "wb").write(data)


# Material textures, 64 x 64.
png("pbr/base.png", 64, 64, lambda x, y: (230, 80, 50, 255) if (x // 16 + y // 16) % 2 else (60, 150, 230, 255))
png("pbr/base-alpha.png", 64, 64, lambda x, y: (240, 200, 60, 40 + 3 * x))
# metallicRoughness: roughness in g (along x), metallic in b (along y).
png("pbr/mr.png", 64, 64, lambda x, y: (255, 30 + 3.5 * x, 255 if y < 32 else 20, 255))
png("pbr/occlusion.png", 64, 64, lambda x, y: (60 + 195 * min(1.0, math.hypot(x - 32, y - 32) / 32), 0, 0, 255))
png("pbr/emissive.png", 64, 64, lambda x, y: (255, 140, 0, 255) if y % 16 < 3 else (0, 0, 0, 255))


def bump(x, y):
    # Tangent-space normals of round bumps, 8 per row.
    cx, cy = (x % 16) - 7.5, (y % 16) - 7.5
    r = math.hypot(cx, cy)
    if r < 6:
        nx, ny = cx / 8, -cy / 8
    else:
        nx, ny = 0.0, 0.0
    nz = math.sqrt(max(0.0, 1 - nx * nx - ny * ny))
    return (128 + 127 * nx, 128 + 127 * ny, 128 + 127 * nz, 255)


png("pbr/normal.png", 64, 64, bump)
# specularGlossiness: specular rgb, glossiness in a (along x).
png("pbr/sg.png", 64, 64, lambda x, y: (255, 220 if y < 32 else 120, 180, 40 + 3.4 * x))

# Environment: a 4:3 sky box cross, faces 64 x 64 (front +Z, back -Z, left -X, right +X, top, bottom).
CROSS = {(1, 1): "front", (3, 1): "back", (0, 1): "left", (2, 1): "right", (1, 0): "top", (1, 2): "bottom"}


def env_face(face, u, v):
    # u, v in [0, 1] across the face as stored; a sky gradient, a warm sun on the right face and
    # colored bands so reflections show direction.
    if face == "top":
        return (120, 170, 255)
    if face == "bottom":
        return (70, 55, 40) if (int(u * 8) + int(v * 8)) % 2 else (110, 90, 60)
    sky = (90 + 120 * v, 140 + 80 * v, 255)
    if face == "right" and math.hypot(u - 0.5, v - 0.35) < 0.12:
        return (255, 250, 220)
    band = {"front": (255, 90, 60), "back": (60, 255, 120), "left": (200, 80, 255)}.get(face)
    if band and 0.55 < v < 0.65:
        return band
    return sky


def skybox(x, y):
    face = CROSS.get((x // 64, y // 64))
    if face is None:
        return (0, 0, 0, 0)
    r, g, b = env_face(face, (x % 64) / 63, (y % 64) / 63)
    return (r, g, b, 255)


png("pbr/env.png", 256, 192, skybox)
for name in ["front", "back", "left", "right", "top", "bottom"]:
    png(f"pbr/env-{name}.png", 64, 64, lambda x, y, n=name: tuple(env_face(n, x / 63, y / 63)) + (255,))

HEAD = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Shape:4
COMPONENT Lighting:3
COMPONENT CubeMapTexturing:3
COMPONENT Scripting:1
COMPONENT PointingDeviceSensor:1
COMPONENT X_ITE:1
"""


def grid(material, rows, cols, dx=1.1, dy=1.1, geometry="Sphere { radius 0.45 }"):
    out = []
    for j, row in enumerate(rows):
        for i, col in enumerate(cols):
            x = (i - (len(cols) - 1) / 2) * dx
            y = ((len(rows) - 1) / 2 - j) * dy
            out.append(f"Transform {{ translation {x:.3f} {y:.3f} 0 children Shape {{ appearance Appearance {{ material {material(row, col)} }} geometry {geometry} }} }}")
    return "\n".join(out)


def options(**o):
    # Browser options set by the world (X3DBrowser.setBrowserOption).
    calls = "".join(f"   Browser .setBrowserOption ('{k}', {repr(v)});\n" for k, v in o.items())
    return f"""DEF OPTIONS Script {{
  url "javascript:
function initialize ()
{{
{calls}}}
"
}}
"""


LIGHTS = """DirectionalLight { direction -0.4 -0.6 -0.7 intensity 0.8 color 1 0.95 0.9 }
PointLight { location 2.5 2 3 intensity 2 color 0.6 0.8 1 radius 20 attenuation 0 0 1 }
SpotLight { location -2.5 2 3 direction 0.6 -0.5 -0.6 intensity 3 cutOffAngle 0.6 beamWidth 0.3 radius 15 color 1 0.7 0.5 }
"""
VIEW = "Viewpoint { position 0 0 6 }\nBackground { skyColor 0.15 0.15 0.18 }\n"
ENV = 'EnvironmentLight { intensity 1 specularTexture ImageCubeMapTexture { url "pbr/env.png" } }\n'
COMPOSED = 'EnvironmentLight { intensity 0.8 color 1 0.9 0.8 rotation 0 1 0 0.8 specularTexture ComposedCubeMapTexture { ' + " ".join(
    f'{n}Texture ImageTexture {{ url "pbr/env-{n}.png" }}' for n in ["front", "back", "left", "right", "top", "bottom"]) + " } }\n"

MR = lambda m, r: f"PhysicalMaterial {{ baseColor 0.9 0.6 0.3 metallic {m} roughness {r} }}"
SG = lambda s, g: f"SpecularGlossinessMaterial {{ diffuseColor 0.5 0.6 0.8 specularColor {s} glossiness {g} }}"
METALLIC = [0, 0.5, 1]
ROUGH = [0.05, 0.3, 0.6, 1]
SPEC = ["0.04 0.04 0.04", "0.5 0.5 0.5", "1 0.8 0.4"]
GLOSS = [1, 0.7, 0.4, 0]

TEXTURED = """
Transform { translation -1.8 0.9 0 children Shape { appearance Appearance { material PhysicalMaterial { metallic 0 roughness 0.5
  baseTexture ImageTexture { url "pbr/base.png" } } } geometry Sphere { radius 0.7 } } }
Transform { translation 0 0.9 0 children Shape { appearance Appearance { material PhysicalMaterial { metallic 1 roughness 1
  metallicRoughnessTexture ImageTexture { url "pbr/mr.png" } occlusionTexture ImageTexture { url "pbr/occlusion.png" } occlusionStrength 0.8 } } geometry Sphere { radius 0.7 } } }
Transform { translation 1.8 0.9 0 children Shape { appearance Appearance { material PhysicalMaterial { baseColor 0.3 0.3 0.35 metallic 0 roughness 0.6
  emissiveColor 1 1 1 emissiveTexture ImageTexture { url "pbr/emissive.png" } } } geometry Sphere { radius 0.7 } } }
Transform { translation -1.8 -0.9 0 children Shape { appearance Appearance { material PhysicalMaterial { baseColor 0.8 0.8 0.8 metallic 0 roughness 0.35
  normalTexture ImageTexture { url "pbr/normal.png" } normalScale 1.5 } } geometry Box { size 1.2 1.2 1.2 } } }
Transform { translation 0 -0.9 0 rotation 0 1 0 0.5 children Shape { appearance Appearance { material PhysicalMaterial { metallic 0.2 roughness 0.4 }
  texture ImageTexture { url "pbr/base.png" } textureTransform TextureTransform { scale 2 2 rotation 0.3 } } geometry Box { size 1.2 1.2 1.2 } } }
Transform { translation 1.8 -0.9 0.5 children Shape { appearance Appearance { material PhysicalMaterial { metallic 0 roughness 0.3 transparency 0.2
  baseTexture ImageTexture { url "pbr/base-alpha.png" } } } geometry Sphere { radius 0.7 } } }
"""

MAPPING = """
Transform { translation -1.2 0 0 children Shape { appearance Appearance { material PhysicalMaterial { metallic 0 roughness 0.5
  baseTexture ImageTexture { url "pbr/base.png" } baseTextureMapping "big"
  normalTexture ImageTexture { url "pbr/normal.png" } normalTextureMapping "small" }
  textureTransform MultiTextureTransform { textureTransform [ TextureTransform { mapping "small" scale 3 3 } TextureTransform { mapping "big" scale 0.5 0.5 } ] } }
  geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -1 -1 0, 1 -1 0, 1 1 0, -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ]
    texCoord MultiTextureCoordinate { texCoord [ TextureCoordinate { mapping "small" point [ 0 0, 1 0, 1 1, 0 1 ] } TextureCoordinate { mapping "big" point [ 0 0, 2 0, 2 2, 0 2 ] } ] } } } }
Transform { translation 1.2 0 0 children Shape { appearance Appearance { material PhysicalMaterial { metallic 0 roughness 0.5 baseColor 1 0.4 0.4 }
  backMaterial PhysicalMaterial { metallic 1 roughness 0.2 baseColor 0.4 0.8 1 } }
  geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0.6, 0.9 0.9 0, -0.9 0.9 -0.6 ] } coordIndex [ 0 1 2 3 -1 ] } } }
"""

LINES = """
Shape { appearance Appearance { material PhysicalMaterial { baseColor 0.2 0.8 0.3 emissiveColor 0.3 0 0 } }
  geometry IndexedLineSet { coord Coordinate { point [ -2 -1 0, 2 -1 0, 2 1 0, -2 1 0 ] } coordIndex [ 0 1 2 3 0 -1 ] } }
Shape { appearance Appearance { material SpecularGlossinessMaterial { diffuseColor 0.2 0.4 1 } pointProperties PointProperties { pointSizeScaleFactor 6 } }
  geometry PointSet { coord Coordinate { point [ -1 0 0, 0 0 0, 1 0 0 ] } } }
Transform { translation 0 -1.8 0 children Shape { appearance Appearance { material PhysicalMaterial { baseColor 1 1 1 }
  texture ImageTexture { url "pbr/base.png" } } geometry IndexedLineSet { coord Coordinate { point [ -2 0 0, 2 0 0 ] } coordIndex [ 0 1 -1 ] } } }
"""

PHONG = """
Transform { translation -1.5 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.8 0.5 0.3 specularColor 0.6 0.6 0.6 shininess 0.4 }
  texture ImageTexture { url "pbr/base.png" } } geometry Sphere { radius 0.8 } } }
Transform { translation 0 0 0 children Shape { appearance Appearance { material UnlitMaterial { emissiveColor 0.9 0.9 0.9 }
  texture ImageTexture { url "pbr/base.png" } } geometry Box { size 1.2 1.2 1.2 } } }
Transform { translation 1.5 0 0 children Shape { appearance Appearance { material PhysicalMaterial { baseColor 1 0.8 0.6 metallic 0.3 roughness 0.4 emissiveColor 0.3 0.1 0 } } geometry Sphere { radius 0.8 } } }
"""

worlds = {
    "pbr-metallic-roughness.wrl": HEAD + VIEW + LIGHTS + grid(MR, METALLIC, ROUGH),
    "pbr-specular-glossiness.wrl": HEAD + VIEW + LIGHTS + grid(SG, SPEC, GLOSS),
    "pbr-textures.wrl": HEAD + VIEW + LIGHTS + TEXTURED,
    "pbr-mapping.wrl": HEAD + VIEW + LIGHTS + MAPPING,
    "pbr-lines-points.wrl": HEAD + VIEW + LIGHTS + LINES,
    "pbr-ibl.wrl": HEAD + VIEW + ENV + 'NavigationInfo { headlight FALSE }\n' + grid(MR, METALLIC, ROUGH),
    "pbr-ibl-lit.wrl": HEAD + VIEW + ENV + LIGHTS + grid(MR, METALLIC, ROUGH[:3]) + "\n" + "Transform { translation 0 -1.9 0 children Group { children [\n" + grid(SG, ["1 1 1"], GLOSS[:3], dx=1.1) + "\n] } }",
    "pbr-ibl-composed.wrl": HEAD + VIEW + COMPOSED + 'NavigationInfo { headlight FALSE }\n' + TEXTURED,
    "pbr-ibl-off.wrl": HEAD + VIEW + 'EnvironmentLight { on FALSE specularTexture ImageCubeMapTexture { url "pbr/env.png" } }\n' + LIGHTS + grid(MR, METALLIC, ROUGH[:2]),
    "pbr-ibl-empty.wrl": HEAD + VIEW + 'EnvironmentLight { intensity 0.5 }\nNavigationInfo { headlight FALSE }\n' + grid(MR, METALLIC, ROUGH[:2]),
}
for mode in ["ACES_NARKOWICZ", "ACES_HILL", "ACES_HILL_EXPOSURE_BOOST", "KHR_PBR_NEUTRAL"]:
    worlds[f"pbr-tone-{mode.lower().replace('_', '-')}.wrl"] = HEAD + VIEW + LIGHTS + options(ToneMapping=mode, Exposure=1.6) + PHONG + "\nTransform { translation 0 -1.6 0 children Group { children [\n" + grid(MR, [0.5], ROUGH, dx=1.0) + "\n] } }"
worlds["pbr-shading-flat.wrl"] = HEAD + VIEW + LIGHTS + ENV + options(Shading="FLAT") + TEXTURED
worlds["pbr-exposure.wrl"] = HEAD + VIEW + LIGHTS + options(Exposure=0.5) + PHONG
worlds["pbr-colorspace-linear.wrl"] = HEAD + VIEW + LIGHTS + options(ColorSpace="LINEAR") + PHONG
worlds["pbr-colorspace-srgb.wrl"] = HEAD + VIEW + LIGHTS + options(ColorSpace="SRGB") + PHONG + "\n" + TEXTURED.replace("translation -1.8 0.9 0", "translation -1.8 2.2 0")

for name, text in worlds.items():
    open(os.path.join(out, name), "w").write(text + "\n")

# Runtime: material fields, material textures, EnvironmentLight fields and browser options changed
# by a root TouchSensor (four clicks at the center sphere).
RUNTIME = HEAD + VIEW + LIGHTS + """NavigationInfo { type "NONE" }
DEF TS TouchSensor { }
DEF ENV EnvironmentLight { intensity 1 specularTexture ImageCubeMapTexture { url "pbr/env.png" } }
DEF MAT PhysicalMaterial { baseColor 0.9 0.6 0.3 metallic 1 roughness 0.2 baseTexture ImageTexture { url "pbr/base.png" } normalTexture ImageTexture { url "pbr/normal.png" } }
DEF SGM SpecularGlossinessMaterial { diffuseColor 0.4 0.5 0.9 specularColor 0.5 0.5 0.5 glossiness 0.9 }
Transform { translation 0 0 0 children Shape { appearance Appearance { material USE MAT } geometry Sphere { radius 1.2 } } }
Transform { translation -2.2 0 0 children Shape { appearance Appearance { material USE SGM } geometry Sphere { radius 0.8 } } }
Transform { translation 2.2 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.8 0.3 0.3 } texture ImageTexture { url "pbr/base.png" } } geometry Box { size 1.2 1.2 1.2 } } }
DEF S Script {
  directOutput TRUE
  inputOnly SFTime touch
  initializeOnly SFNode mat USE MAT
  initializeOnly SFNode sgm USE SGM
  initializeOnly SFNode env USE ENV
  initializeOnly SFNode other ImageTexture { url "pbr/base-alpha.png" }
  initializeOnly SFNode composed ComposedCubeMapTexture { """ + " ".join(
    f'{n}Texture ImageTexture {{ url "pbr/env-{n}.png" }}' for n in ["front", "back", "left", "right", "top", "bottom"]) + """ }
  inputOutput SFInt32 step 0
  url "javascript:
function touch (value)
{
   step = step + 1;

   if (step == 1) { mat .metallic = 0.2; mat .roughness = 0.7; mat .baseColor = new SFColor (0.4, 0.9, 0.5); env .intensity = 0.4; env .rotation = new SFRotation (0, 1, 0, 1.5); }
   if (step == 2) { mat .baseTexture = other; mat .normalScale = 0.3; sgm .glossiness = 0.2; Browser .setBrowserOption ('ToneMapping', 'ACES_NARKOWICZ'); }
   if (step == 3) { env .specularTexture = composed; env .intensity = 1.2; Browser .setBrowserOption ('Exposure', 0.7); }
   if (step == 4) { env .on = false; mat .emissiveColor = new SFColor (0.2, 0, 0.3); Browser .setBrowserOption ('ColorSpace', 'LINEAR'); }
}
"
}
ROUTE TS.touchTime TO S.touch
"""
open(os.path.join(out, "pbr-runtime.wrl"), "w").write(RUNTIME)
events = []
for f in (24, 40, 56, 72):
    events += [{"frame": f, "type": "mousemove", "x": 256, "y": 256},
               {"frame": f, "type": "mousedown", "button": 0, "x": 256, "y": 256},
               {"frame": f + 1, "type": "mouseup", "button": 0, "x": 256, "y": 256}]
json.dump({"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 90, "capture": [20, 36, 52, 68, 84],
           "dump": [20, 90], "events": events}, open(os.path.join(out, "pbr-runtime.json"), "w"), indent=1)

# EnvironmentLight with a GeneratedCubeMapTexture: rendered at the light's origin every frame and
# filtered again after each pass (stepped only: the physical spheres see each other through it).
GENENV = HEAD + VIEW + """NavigationInfo { type "NONE" headlight FALSE }
DirectionalLight { direction 0 -1 -0.3 intensity 0.7 }
DEF ENV EnvironmentLight { origin 0 0 0.5 specularTexture DEF GEN GeneratedCubeMapTexture { update "ALWAYS" size 32 } }
DEF MOVER Transform { translation 3 0 0 children Shape { appearance Appearance { material Material { diffuseColor 1 0.2 0.1 emissiveColor 0.5 0.1 0 } } geometry Box { size 1 3 3 } } }
Transform { translation -3 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.1 0.8 0.2 emissiveColor 0 0.4 0.1 } } geometry Box { size 1 3 3 } } }
Transform { translation 0 3 0 children Shape { appearance Appearance { material UnlitMaterial { emissiveColor 0.6 0.7 1 } } geometry Box { size 6 0.5 6 } } }
""" + grid(MR, [1], [0.1, 0.5, 1]) + """
DEF TS TouchSensor { }
DEF S Script {
  directOutput TRUE
  inputOnly SFTime touch
  initializeOnly SFNode mover USE MOVER
  initializeOnly SFNode gen USE GEN
  initializeOnly SFNode env USE ENV
  inputOutput SFInt32 step 0
  url "javascript:
function touch (value)
{
   step = step + 1;

   if (step == 1) { mover .translation = new SFVec3f (0, -3, 0); }
   if (step == 2) { gen .update = 'NONE'; mover .translation = new SFVec3f (3, 0, 0); }
   if (step == 3) { gen .update = 'NEXT_FRAME_ONLY'; env .origin = new SFVec3f (0, 2, 0); }
}
"
}
ROUTE TS.touchTime TO S.touch
"""
open(os.path.join(out, "pbr-ibl-generated.wrl"), "w").write(GENENV)
events = []
for f in (24, 40, 56):
    events += [{"frame": f, "type": "mousemove", "x": 256, "y": 256},
               {"frame": f, "type": "mousedown", "button": 0, "x": 256, "y": 256},
               {"frame": f + 1, "type": "mouseup", "button": 0, "x": 256, "y": 256}]
json.dump({"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 72, "capture": [20, 36, 52, 68],
           "dump": [20, 72], "events": events}, open(os.path.join(out, "pbr-ibl-generated.json"), "w"), indent=1)
json.dump({"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 20, "capture": [10, 20], "dump": [20], "events": []},
          open(os.path.join(out, "pbr-shading-flat.json"), "w"), indent=1)
