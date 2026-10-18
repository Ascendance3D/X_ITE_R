#!/usr/bin/env python3
"""REA batch 02: authored MultiTexture / texture transform worlds (no archive content).

  python3 -I oracle/tools/gen_rea_b02_multitex.py oracle/synthetic-rea-b02

A 5 x 4 grid of textured quads per world; each cell is one MultiTexture case.
"""
import os, sys

out = sys.argv[1]
MODES = ["REPLACE", "MODULATE", "MODULATE2X", "MODULATE4X", "ADD", "ADDSIGNED", "ADDSIGNED2X", "ADDSMOOTH",
         "SUBTRACT", "BLENDDIFFUSEALPHA", "BLENDTEXTUREALPHA", "BLENDFACTORALPHA", "BLENDCURRENTALPHA",
         "MODULATEALPHA_ADDCOLOR", "MODULATEINVALPHA_ADDCOLOR", "MODULATEINVCOLOR_ADDALPHA", "DOTPRODUCT3",
         "SELECTARG1", "SELECTARG2", "OFF"]
# 4x4 RGBA patterns (PixelTexture: first pixel lower left).
T1 = "PixelTexture { image 4 4 4 " + " ".join("0x%02x%02x%02x%02x" % ((40 + 60 * (i % 4)) % 256, (200 - 40 * (i // 4)) % 256, (30 * i) % 256, 255 - 15 * i) for i in range(16)) + " }"
T2 = "PixelTexture { image 4 4 4 " + " ".join("0x%02x%02x%02x%02x" % (255 - 16 * i, (90 + 25 * i) % 256, 128, (64 + 12 * i) % 256) for i in range(16)) + " }"
T3 = "PixelTexture { image 2 2 3 0xff4020 0x20ff40 0x4020ff 0xffffff }"

def quad(x, y, appearance):
    return f"""Transform {{ translation {x} {y} 0 children Shape {{
  appearance {appearance}
  geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -0.45 -0.45 0, 0.45 -0.45 0, 0.45 0.45 0, -0.45 0.45 0 ] }}
    texCoord TextureCoordinate {{ point [ 0 0, 1 0, 1 1, 0 1 ] }} coordIndex [ 0 1 2 3 -1 ] }}
}} }}"""

def grid(cells):
    shapes = []
    for k, app in enumerate(cells):
        col, row = k % 5, k // 5
        shapes.append(quad(-2 + col, 1.5 - row, app))
    return "\n".join(shapes)

def header(version, components):
    return f"#X3D V{version} utf8\nPROFILE Interchange\n" + "".join(f"COMPONENT {c}\n" for c in components) + \
        "Viewpoint { position 0 0 6.2 }\nBackground { skyColor 0.2 0.25 0.3 }\nNavigationInfo { headlight FALSE }\n" + \
        "DirectionalLight { direction -0.3 -0.4 -1 intensity 0.9 ambientIntensity 0.3 }\n"

def write(name, text):
    with open(os.path.join(out, name), "w") as f:
        f.write(text)

def mat(version, transparency=0):
    return f"Material {{ diffuseColor 0.8 0.6 0.4 specularColor 0.4 0.3 0.2 shininess 0.3 ambientIntensity 0.4 transparency {transparency} }}"

# 1/2: every RGB mode for unit 2 (unit 1 MODULATE), X3D 4 (Phong path) and 3.3 (Gouraud path).
for version, name in [("4.0", "multitex-modes-v4.wrl"), ("3.3", "multitex-modes-v33.wrl")]:
    cells = [f'Appearance {{ material {mat(version)} texture MultiTexture {{ mode [ "MODULATE" "{m}" ] texture [ {T1} {T2} ] }} }}' for m in MODES]
    write(name, header(version, ["Texturing:3"]) + grid(cells) + "\n")

# 3: alpha modes, revealed by a third unit BLENDCURRENTALPHA over a white factor, and by transparency.
cells = []
for m in MODES:
    cells.append(f'Appearance {{ material {mat("4.0", 0.2)} texture MultiTexture {{ color 1 1 1 alpha 0.5 mode [ "MODULATE" "MODULATE,{m}" "BLENDCURRENTALPHA,REPLACE" ] source [ "" "" "FACTOR" ] texture [ {T1} {T2} {T3} ] }} }}')
write("multitex-alpha.wrl", header("4.0", ["Texturing:3"]) + grid(cells) + "\n")

# 4: source, function, color / alpha, unlit, the texture count limit and non-texture entries.
srcfun = [
    ('mode [ "MODULATE" "REPLACE" ] source [ "" "DIFFUSE" ]', mat("4.0")),
    ('mode [ "MODULATE" "REPLACE" ] source [ "" "SPECULAR" ]', mat("4.0")),
    ('color 0.2 0.9 0.4 alpha 0.3 mode [ "MODULATE" "REPLACE" ] source [ "" "FACTOR" ]', mat("4.0")),
    ('color 0.2 0.9 0.4 alpha 0.3 mode [ "MODULATE" "BLENDFACTORALPHA" ]', mat("4.0")),
    ('mode [ "MODULATE" "MODULATE" ] function [ "" "COMPLEMENT" ]', mat("4.0")),
    ('mode [ "MODULATE" "BLENDTEXTUREALPHA" ] function [ "" "ALPHAREPLICATE" ]', mat("4.0")),
    ('mode [ "MODULATE" "BLENDTEXTUREALPHA" ] function [ "" "ALPHA_REPLICATE" ]', mat("4.0")),
    ('mode [ "MODULATE" "unknownmode" ] source [ "" "diffuse" ] function [ "" "bogus" ]', mat("4.0")),
    ('mode [ " ADD , SUBTRACT " ]', mat("4.0")),
    ('mode [ "MODULATE_2X" "ADD_SIGNED_2X" ]', mat("4.0")),
    ('mode [ "MODULATE" "ADD" ]', "UnlitMaterial { emissiveColor 0.5 0.7 0.9 }"),
    ('mode [ "MODULATE" "DOTPRODUCT3" ]', "UnlitMaterial { emissiveColor 0.5 0.7 0.9 transparency 0.3 }"),
    ('mode [ "MODULATE" "SUBTRACT" ]', None),
    ('mode [ "REPLACE" ]', mat("4.0")),
    ('mode [ ]', mat("4.0")),
]
cells = []
for i, (fields, m) in enumerate(srcfun):
    texs = f"{T1} {T2}"
    if i == 13:
        texs = f"{T1} {T2} {T3} {T1} {T2}"  # five textures, maxTextures 4
    if i == 14:
        texs = f"NULL {T2} Material {{ }} {T3}"
    material = f" material {m}" if m else ""
    cells.append(f'Appearance {{{material} texture MultiTexture {{ {fields} texture [ {texs} ] }} }}')
cells.append(f'Appearance {{ material {mat("4.0")} texture MultiTexture {{ }} }}')
cells.append(f'Appearance {{ material {mat("3.3")} texture MultiTexture {{ mode [ "MODULATE" "ADD" ] texture [ {T1} {T2} ] }} }}')
write("multitex-sources.wrl", header("4.0", ["Texturing:3", "Shape:2"]) + grid(cells) + "\n")

# 5: coordinate channels, MultiTextureTransform, generators and texture transforms (2D and 3D).
def ifs(texcoord):
    return f"""IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -0.45 -0.45 0, 0.45 -0.45 0, 0.45 0.45 0, -0.45 0.45 0 ] }}
    texCoord {texcoord} coordIndex [ 0 1 2 3 -1 ] }}"""
TC = "TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] }"
TC2 = "TextureCoordinate { point [ 0 0, 2 0, 2 2, 0 2 ] }"
TC3 = "TextureCoordinate3D { point [ 0 0 0.5, 1 0 0.5, 1 1 0.5, 0 1 0.5 ] }"
TC4 = "TextureCoordinate4D { point [ 0 0 0 1, 2 0 0 2, 2 2 0 2, 0 1 0 1 ] }"
MT = f'MultiTexture {{ mode [ "MODULATE" "ADD" ] texture [ {T1} {T3} ] }}'
cases = [
    (f"MultiTextureCoordinate {{ texCoord [ {TC} {TC2} ] }}", MT, None),
    (f"MultiTextureCoordinate {{ texCoord [ {TC2} ] }}", MT, None),
    (f"MultiTextureCoordinate {{ texCoord [ {TC} TextureCoordinateGenerator {{ mode \"COORD\" }} ] }}", MT, None),
    (f"MultiTextureCoordinate {{ texCoord [ {TC3} {TC4} ] }}", MT, None),
    (TC, MT, "MultiTextureTransform { textureTransform [ TextureTransform { scale 2 2 } TextureTransform { rotation 0.5 center 0.5 0.5 } ] }"),
    (TC, MT, "MultiTextureTransform { textureTransform [ TextureTransform { translation 0.25 0 } ] }"),
    (TC, MT, "MultiTextureTransform { }"),
    (TC, MT, "MultiTextureTransform { textureTransform [ Material { } TextureTransform3D { scale 3 1 1 } ] }"),
    (TC, T1, "TextureTransform3D { translation 0.1 0.2 0 rotation 0 0 1 0.4 scale 1.5 2 1 center 0.5 0.5 0 }"),
    (TC, T1, "TextureTransform3D { rotation 0 1 0 0.8 }"),
    (TC, T1, "TextureTransformMatrix3D { matrix 2 0 0 0 0 1 0 0 0 0 1 0 0.25 0 0 1 }"),
    (TC, T1, "TextureTransformMatrix3D { matrix 1 0.5 0 0 0 1 0 0 0 0 1 0 0 0 0 1 }"),
    (TC, MT, "MultiTextureTransform { textureTransform [ TextureTransform3D { scale 2 2 1 } TextureTransformMatrix3D { matrix 0 1 0 0 1 0 0 0 0 0 1 0 0 0 0 1 } ] }"),
    ("TextureCoordinateGenerator { mode \"COORD\" }", T1, None),
    ("TextureCoordinateGenerator { mode \"COORD-EYE\" }", T1, "TextureTransform { scale 0.5 0.5 }"),
    ("TextureCoordinateGenerator { mode \"SPHERE\" }", T1, None),
    ("TextureCoordinateGenerator { mode \"CAMERASPACENORMAL\" }", T1, None),
    ("TextureCoordinateGenerator { mode \"CAMERASPACEPOSITION\" }", T1, None),
    ("TextureCoordinateGenerator { mode \"SPHERE-LOCAL\" }", T1, None),
    (f"MultiTextureCoordinate {{ texCoord [ {TC} {TC2} {TC3} {TC4} ] }}", f'MultiTexture {{ mode [ "MODULATE" "ADD" "MODULATE" "SUBTRACT" ] texture [ {T1} {T2} {T3} {T1} ] }}', None),
]
shapes = []
for k, (tc, tex, tt) in enumerate(cases):
    col, row = k % 5, k // 5
    app = f"Appearance {{ material {mat('4.0')} texture {tex}" + (f" textureTransform {tt}" if tt else "") + " }"
    shapes.append(f"Transform {{ translation {-2 + col} {1.5 - row} 0 children Shape {{ appearance {app} geometry {ifs(tc)} }} }}")
write("multitex-coords.wrl", header("4.0", ["Texturing:3", "Texturing3D:2"]) + "\n".join(shapes) + "\n")
