#!/usr/bin/env python3
"""REA batch 03 Texturing3D worlds (tex3d-*.wrl) for oracle/synthetic-rea-b03.

  python3 -I oracle/tools/gen_rea_b03_texture3d.py oracle/synthetic-rea-b03

PixelTexture3D (1-4 components, NEAREST below 16 texels, LINEAR above, repeat and
TextureProperties boundary modes, transparency, invalid images), ComposedTexture3D (PixelTexture and
ImageTexture slices with the row flip, mipmaps, a slice of another size, incomplete slices, an empty
list), ImageTextureAtlas (tiles, numberOfSlices limits, slicesOverX 0, a failing url, outputs), with
default, TextureCoordinate3D, TextureCoordinate4D and TextureCoordinateGenerator coordinates and
TextureTransform3D; MultiTexture, cube maps, PhysicalMaterial with EnvironmentLight, LayerSet with
Viewport and LayoutLayer, GeneratedCubeMapTexture; a stepped runtime case. The PNG images are
generated here (clean synthetic images).
"""
import json, os, struct, sys, zlib

def png(path, w, h, pixel):
    raw = b"".join(b"\x00" + bytes(c for x in range(w) for c in pixel(x, y)) for y in range(h))
    chunk = lambda t, d: struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(path, "wb").write(data)

def atlas_pixel(x, y):
    # 64 × 64: 4 × 4 tiles of 16 × 16, each tile its own color, with a light top-left mark (row
    # order) and a darker diagonal; tile 13 (row 3, column 1) half transparent.
    tx, ty = x // 16, y // 16
    i = ty * 4 + tx
    lx, ly = x % 16, y % 16
    base = ((i * 53) % 256, (i * 97 + 60) % 256, (255 - i * 15) % 256)
    if lx < 4 and ly < 4:
        base = (255, 255, 255)
    elif lx == ly:
        base = tuple(c // 3 for c in base)
    return base + ((128,) if i == 13 else (255,))

def slice_pixel(color):
    def p(x, y):
        # 16 × 16, a quadrant pattern that shows the row order.
        if y < 4:
            return (255, 255, 255, 255)
        return color + (255,) if (x // 4 + y // 4) % 2 == 0 else tuple(c // 2 for c in color) + (255,)
    return p

def img3d(comp, w, h, d, f):
    vals = [comp, w, h, d]
    for z in range(d):
        for y in range(h):
            for x in range(w):
                vals.append(f(x, y, z))
    return " ".join(str(v) for v in vals)

def rgb(r, g, b):
    return (r << 16) | (g << 8) | b

def rgba(r, g, b, a):
    return ((r << 24) | (g << 16) | (b << 8) | a) - (1 << 32 if r >= 128 else 0)

HEAD = """#X3D V4.1 utf8
PROFILE Interchange
COMPONENT Texturing3D:2
COMPONENT CubeMapTexturing:3
COMPONENT Layering:1
COMPONENT Layout:2
COMPONENT Lighting:3
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
COMPONENT Networking:4
Viewpoint { position 0 0 15 }
NavigationInfo { headlight TRUE }
"""

GEN = 'texCoord TextureCoordinateGenerator { mode "COORD" }'
TT = "textureTransform TextureTransform3D { translation 0.5 0.5 0.5 scale 0.5 0.5 0.5 }"

def sphere(tex, x, y, mat="Material { diffuseColor 1 1 1 }", extra=TT):
    return f"Transform {{ translation {x} {y} 0 children Shape {{ appearance Appearance {{ material {mat} texture {tex} {extra} }} geometry Sphere {{ radius 0.9 }} }} }}"

def box(tex, x, y, mat="Material { diffuseColor 1 1 1 }", extra=""):
    return f"Transform {{ translation {x} {y} 0 rotation 1 1 0 0.6 children Shape {{ appearance Appearance {{ material {mat} texture {tex} {extra} }} geometry Box {{ size 1.4 1.4 1.4 }} }} }}"

def ifs(tex, x, y, coord, mat="Material { diffuseColor 1 1 1 }"):
    # Two quads at different depths p, with explicit 3D / 4D texture coordinates.
    return (f"Transform {{ translation {x} {y} 0 children Shape {{ appearance Appearance {{ material {mat} texture {tex} }} "
            f"geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] }} coordIndex [ 0 1 2 3 -1 ] "
            f"texCoord {coord} }} }} }}")

TC3 = "TextureCoordinate3D { point [ 0 0 0.1, 1 0 0.4, 1 1 0.9, 0 1 0.6 ] }"
TC4 = "TextureCoordinate4D { point [ 0 0 0.2 1, 2 0 0.4 2, 2 2 1.8 2, 0 1 0.6 1 ] }"

# 4 × 4 × 4 textures (NEAREST) with each component count.
P1 = "PixelTexture3D { image [ " + img3d(1, 4, 4, 4, lambda x, y, z: (x * 60 + y * 20 + z * 5) % 256) + " ] }"
P2 = "PixelTexture3D { image [ " + img3d(2, 4, 4, 4, lambda x, y, z: (((x * 60 + z * 30) % 256) << 8) | (255 if (x + y + z) % 2 else 96)) + " ] }"
P3 = "PixelTexture3D { image [ " + img3d(3, 4, 4, 4, lambda x, y, z: rgb(x * 80, y * 80, z * 80)) + " ] }"
P4 = "PixelTexture3D { image [ " + img3d(4, 4, 4, 4, lambda x, y, z: rgba(x * 80, 255 - y * 80, z * 80, 255 if (x + z) % 2 else 64)) + " ] }"
P3R = "PixelTexture3D { repeatS TRUE repeatT TRUE repeatR TRUE image [ " + img3d(3, 4, 4, 4, lambda x, y, z: rgb(x * 80, y * 80, z * 80)) + " ] }"
# 16 × 16 × 8: LINEAR without mipmaps.
PL = "PixelTexture3D { image [ " + img3d(3, 16, 16, 8, lambda x, y, z: rgb(x * 16, y * 16, z * 32)) + " ] }"
PLP = ("PixelTexture3D { textureProperties TextureProperties { boundaryModeS \"MIRRORED_REPEAT\" boundaryModeT \"REPEAT\" boundaryModeR \"CLAMP_TO_EDGE\" magnificationFilter \"NEAREST_PIXEL\" minificationFilter \"NEAREST_PIXEL\" } image [ "
       + img3d(3, 4, 4, 4, lambda x, y, z: rgb(x * 80, y * 80, z * 80)) + " ] }")
PLL = ("PixelTexture3D { textureProperties TextureProperties { magnificationFilter \"AVG_PIXEL\" minificationFilter \"AVG_PIXEL\" boundaryModeS \"REPEAT\" boundaryModeT \"REPEAT\" boundaryModeR \"REPEAT\" } image [ "
       + img3d(3, 4, 4, 4, lambda x, y, z: rgb(x * 80, y * 80, z * 80)) + " ] }")

worlds, scripts = {}, {}
worlds["tex3d-pixel.wrl"] = HEAD + "\n".join([
    sphere(P1, -4.5, 2.5), sphere(P2, -1.5, 2.5), sphere(P3, 1.5, 2.5), sphere(P4, 4.5, 2.5),
    box(P3, -4.5, -0.2), box(P3R, -1.5, -0.2), ifs(P3, 1.5, -0.2, TC3), ifs(P3R, 4.5, -0.2, TC4),
    sphere(PL, -4.5, -3), sphere(PLP, -1.5, -3, extra="textureTransform TextureTransform3D { scale 0.7 0.7 0.7 }"), sphere(PLL, 1.5, -3, extra="textureTransform TextureTransform3D { scale 0.7 0.7 0.7 rotation 0 0 1 0.3 }"),
    f"Transform {{ translation 4.5 -3 0 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {P3} }} geometry IndexedFaceSet {{ coord Coordinate {{ point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] }} coordIndex [ 0 1 2 3 -1 ] {GEN} }} }} }}",
]) + "\n"
# Invalid images: too short, components 0 and 5, a negative size, a zero size (incomplete), missing pixels.
worlds["tex3d-pixel-invalid.wrl"] = HEAD + "\n".join([
    sphere("PixelTexture3D { image [ 3 2 2 ] }", -4.5, 1.5),
    sphere("PixelTexture3D { image [ 0 2 2 2 1 2 3 4 5 6 7 8 ] }", -1.5, 1.5),
    sphere("PixelTexture3D { image [ 5 1 1 1 255 ] }", 1.5, 1.5),
    sphere("PixelTexture3D { image [ 3 -1 2 2 ] }", 4.5, 1.5),
    sphere("PixelTexture3D { image [ 3 0 2 2 ] }", -3, -1.5),
    sphere("PixelTexture3D { image [ 3 2 2 2 0xff0000 0x00ff00 0x0000ff ] }", 0, -1.5),
    sphere("PixelTexture3D { }", 3, -1.5),
]) + "\n"

SL = [("slice-red.png", (230, 40, 40)), ("slice-green.png", (40, 200, 60)), ("slice-blue.png", (50, 80, 230)), ("slice-yellow.png", (220, 210, 40))]
def pix2d(color, alpha=255):
    r, g, b = color
    return f"PixelTexture {{ image 2 2 4 {rgba(r, g, b, alpha)} {rgba(b, r, g, alpha)} {rgba(g, b, r, alpha)} {rgba(255, 255, 255, alpha)} }}"
CP = "ComposedTexture3D { texture [ " + " ".join(pix2d(c) for _, c in SL) + " ] }"
CPT = "ComposedTexture3D { repeatS TRUE repeatT TRUE repeatR TRUE texture [ " + " ".join(pix2d(c, 128 if i == 1 else 255) for i, (_, c) in enumerate(SL)) + " ] }"
CI = "ComposedTexture3D { texture [ " + " ".join(f'ImageTexture {{ url "{f}" }}' for f, _ in SL) + " ] }"
CM = "ComposedTexture3D { texture [ " + f'ImageTexture {{ url "{SL[0][0]}" }} ' + pix2d((10, 200, 200)) + f' ImageTexture {{ url "{SL[2][0]}" }} ] }}'
CF = "ComposedTexture3D { texture [ " + f'ImageTexture {{ url "{SL[0][0]}" }} ImageTexture {{ url "missing-slice.png" }} ] }}'
CE = "ComposedTexture3D { }"
CX = "ComposedTexture3D { texture [ Box { } " + pix2d((200, 50, 200)) + " ] }"
worlds["tex3d-composed.wrl"] = HEAD + "\n".join([
    sphere(CP, -4.5, 2.5), sphere(CPT, -1.5, 2.5), sphere(CI, 1.5, 2.5), sphere(CM, 4.5, 2.5),
    box(CI, -4.5, -0.2), ifs(CI, -1.5, -0.2, TC3), ifs(CP, 1.5, -0.2, TC4), sphere(CF, 4.5, -0.2),
    sphere(CE, -3, -3), sphere(CX, 0, -3),
    sphere("ComposedTexture3D { textureProperties TextureProperties { generateMipMaps TRUE minificationFilter \"NICEST\" magnificationFilter \"NICEST\" } texture [ " + " ".join(f'ImageTexture {{ url "{f}" }}' for f, _ in SL) + " ] }", 3, -3, extra="textureTransform TextureTransform3D { scale 6 6 0.5 }"),
]) + "\n"

AT = 'ImageTextureAtlas { url "atlas-4x4.png" slicesOverX 4 slicesOverY 4 numberOfSlices 16 }'
worlds["tex3d-atlas.wrl"] = HEAD + "\n".join([
    sphere(AT, -4.5, 2.5),
    sphere('ImageTextureAtlas { url "atlas-4x4.png" slicesOverX 4 slicesOverY 4 numberOfSlices 6 repeatS TRUE repeatT TRUE }', -1.5, 2.5),
    sphere('ImageTextureAtlas { url "atlas-4x4.png" slicesOverX 4 slicesOverY 4 numberOfSlices 40 }', 1.5, 2.5),
    sphere('ImageTextureAtlas { url "atlas-4x4.png" slicesOverX 3 slicesOverY 2 numberOfSlices 5 }', 4.5, 2.5),
    box(AT, -4.5, -0.2), ifs(AT, -1.5, -0.2, TC3), ifs(AT, 1.5, -0.2, TC4),
    sphere('ImageTextureAtlas { url "atlas-4x4.png" slicesOverY 4 numberOfSlices 4 }', 4.5, -0.2),
    sphere('ImageTextureAtlas { url [ "missing-atlas.png" "atlas-4x4.png" ] slicesOverX 2 slicesOverY 2 numberOfSlices 4 }', -3, -3),
    sphere('ImageTextureAtlas { url "missing-atlas.png" slicesOverX 4 slicesOverY 4 numberOfSlices 16 }', 0, -3),
    sphere('ImageTextureAtlas { url "atlas-4x4.png" load FALSE slicesOverX 4 slicesOverY 4 numberOfSlices 16 }', 3, -3),
]) + "\n"

CUBE = "ComposedCubeMapTexture { " + " ".join(f"{f} {pix2d(c)}" for f, c in zip(["front", "back", "left", "right", "top", "bottom"], [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0), (0, 255, 255), (255, 0, 255)])) + " }"
worlds["tex3d-cross.wrl"] = HEAD + "\n".join([
    # MultiTexture: two 3D units, a 2D unit, a cube unit.
    sphere(f'MultiTexture {{ mode [ "MODULATE" "ADD" "MODULATE" "MODULATE" ] texture [ {P3} ImageTexture {{ url "{SL[1][0]}" }} {AT} {CUBE} ] }}', -4.5, 2.5,
           extra="textureTransform MultiTextureTransform { textureTransform [ TextureTransform3D { translation 0.5 0.5 0.5 scale 0.5 0.5 0.5 } TextureTransform { } TextureTransform3D { translation 0.5 0.5 0.5 scale 0.5 0.5 0.5 } TextureTransform { } ] }"),
    # PhysicalMaterial with an EnvironmentLight and a 3D base texture.
    sphere(CI, -1.5, 2.5, mat="PhysicalMaterial { baseColor 1 1 1 metallic 0.2 roughness 0.4 }"),
    sphere(PL, 1.5, 2.5, mat="UnlitMaterial { emissiveColor 1 1 1 }"),
    # A GeneratedCubeMapTexture that sees 3D textured shapes.
    "Transform { translation 4.5 2.5 0 children Shape { appearance Appearance { material Material { diffuseColor 0 0 0 specularColor 0.2 0.2 0.2 } texture GeneratedCubeMapTexture { update \"ALWAYS\" size 64 } } geometry Sphere { radius 0.9 } } }",
    "EnvironmentLight { intensity 0.6 diffuseCoefficients [ 0.6 0.6 0.6, 0.1 0.1 0.1, 0.2 0.2 0.2, 0 0 0.1, 0 0 0, 0 0 0, 0 0 0, 0 0 0, 0 0 0 ] }",
    sphere(P4, 3.2, 0.6),
    sphere(CP, 5.8, 0.6),
    # LayerSet: a Viewport layer and a LayoutLayer with 3D textured shapes.
    "LayerSet { order [ 0 1 2 ] layers [",
    "  Layer { viewport Viewport { clipBoundary [ 0 0.5 0 0.45 ] } children [ Viewpoint { position 0 0 6 } " + sphere(P3, -1, 0) + " " + sphere(AT, 1, 0) + " ] }",
    "  LayoutLayer { layout Layout { size [ 0.4 0.4 ] sizeUnits [ \"FRACTION\" \"FRACTION\" ] align [ \"RIGHT\" \"BOTTOM\" ] } children [ Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture " + P3 + " } geometry Rectangle2D { size 0.4 0.4 } } ] }",
    "] }",
]) + "\n"
# PickableGroup with 3D textured shapes and a LinePickSensor (picking ignores textures).
worlds["tex3d-pickable.wrl"] = HEAD.replace("COMPONENT Networking:4\n", "COMPONENT Networking:4\nCOMPONENT Picking:3\n") + "\n".join([
    "DEF PG PickableGroup { children [ " + sphere(P3, -1.5, 0) + " " + sphere(CI, 1.5, 0) + " ] }",
    "DEF LPS LinePickSensor { pickingGeometry IndexedLineSet { coord Coordinate { point [ -5 0 0, 5 0 0 ] } coordIndex [ 0 1 ] } pickTarget USE PG intersectionType \"GEOMETRY\" sortOrder \"ALL_SORTED\" }",
]) + "\n"

worlds["tex3d-runtime.wrl"] = HEAD + "\n".join([
    "DEF S1 Transform { translation -3 1.5 0 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture DEF P " + P3 + " " + TT + " } geometry Sphere { radius 1.2 } } }",
    "DEF S2 Transform { translation 0 1.5 0 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture DEF C " + CP + " " + TT + " } geometry Sphere { radius 1.2 } } }",
    "DEF S3 Transform { translation 3 1.5 0 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture DEF A ImageTextureAtlas { url \"atlas-4x4.png\" slicesOverX 4 slicesOverY 4 numberOfSlices 16 } " + TT + " } geometry Sphere { radius 1.2 } } }",
    "DEF LS LoadSensor { children USE A }",
    "DEF K KeySensor { }",
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOnly SFInt32 on_width
  inputOnly SFInt32 on_height
  inputOnly SFInt32 on_depth
  inputOnly SFInt32 on_color
  inputOnly SFBool on_loaded
  inputOutput SFString log ""
  initializeOnly SFNode p USE P
  initializeOnly SFNode c USE C
  initializeOnly SFNode a USE A
  url "javascript:
function on_width (v) { log = log + 'W' + v + ' '; }
function on_height (v) { log = log + 'H' + v + ' '; }
function on_depth (v) { log = log + 'D' + v + ' '; }
function on_color (v) { log = log + 'C' + v + ' '; }
function on_loaded (v) { log = log + 'L' + v + ' '; }
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') p .image = new MFInt32 (1, 2, 2, 2, 0, 80, 160, 240, 40, 120, 200, 255);
   if (k == 'b') p .image = new MFInt32 (7, 1, 1, 1, 5);
   if (k == 'c') c .texture = new MFNode (c .texture [2], c .texture [0]);
   if (k == 'd') c .texture = new MFNode ();
   if (k == 'e') { a .slicesOverX = 2; a .numberOfSlices = 2; }
   if (k == 'f') a .url = new MFString ('atlas-4x4.png', 'x.png');
   if (k == 'g') a .url = new MFString ('missing-atlas.png');
   if (k == 'h') a .load = false;
   if (k == 'i') a .load = true;
}
"
}""",
    "ROUTE K.keyPress TO R.on_key",
    "ROUTE A.width TO R.on_width",
    "ROUTE A.height TO R.on_height",
    "ROUTE A.depth TO R.on_depth",
    "ROUTE A.colorDepth TO R.on_color",
    "ROUTE LS.isLoaded TO R.on_loaded",
]) + "\n"
FOCUS = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507}, {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
# A key every 12 frames; pictures and dumps 10 frames after each (url changes need X_ITE's fetch and
# decode, which take real time).
keys = [(20 + 12 * i, k) for i, k in enumerate("abcdefghi")]
after = [f + 10 for f, _ in keys]
scripts["tex3d-runtime.json"] = {"settle": True, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 130, "capture": [15] + after, "dump": [10, 15] + after + [129], "events": FOCUS + [{"frame": f, "type": "key", "key": k} for f, k in keys]}

# The ComposedTexture3D world stepped: its warning log (the size warning of update) and pictures.
scripts["tex3d-composed.json"] = {"settle": True, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 20, "capture": [19], "dump": [19], "events": []}

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
png(os.path.join(out, "atlas-4x4.png"), 64, 64, atlas_pixel)
for f, c in SL:
    png(os.path.join(out, f), 16, 16, slice_pixel(c))
for name, text in worlds.items():
    with open(os.path.join(out, name), "w") as f:
        f.write(text)
for name, script in scripts.items():
    with open(os.path.join(out, name), "w") as f:
        json.dump(script, f, indent=1)
print(len(worlds), "worlds,", len(scripts), "scripts")
