#!/usr/bin/env python3
"""REA batch 09 still-image worlds (vis-*.wrl) for oracle/synthetic-rea-b09-visual.

  python3 -I oracle/tools/gen_rea_b09_visual.py oracle/synthetic-rea-b09-visual

Visual evidence for the 33-node survey (node-* worlds of synthetic-rea-b09-sai cover fields and
events only). Each world is a grid of cells that vary one node's fields side by side:

  vis-rendering     Color, ColorRGBA, Coordinate, CoordinateDouble, Normal, IndexedLineSet, PointSet
  vis-shape         Shape, Appearance, Material
  vis-light-dir     DirectionalLight   (headlight off, each light scoped to its own cell)
  vis-light-point   PointLight
  vis-light-spot    SpotLight
  vis-texturing     ImageTexture, PixelTexture (1-4 components), TextureTransform
  vis-texturing3d   TextureCoordinate3D, TextureCoordinate4D (with a PixelTexture3D)

Not covered: MovieTexture (the corpus holds no movie file; a frame at a fixed time cannot be
recorded without one). Textures are the clean synthetic PNGs of synthetic-rea-b02 / b03.
"""
import os, sys

HEAD = "#X3D V4.0 utf8\nPROFILE Interchange\n"
HEAD3D = "#X3D V4.1 utf8\nPROFILE Interchange\nCOMPONENT Texturing3D:2\n"
BG = "Background { skyColor 0.12 0.14 0.2 }"
PBR = "../synthetic-rea-b02/pbr/"
CHECKER = "../synthetic-rea-b03/proj-checker.png"
MAT = "Material { diffuseColor 0.8 0.8 0.8 }"
QUAD = "coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] }"
SPHERE = "Sphere { radius 0.9 }"

def cell(i, body, cols=4, spacing=2.2, rows=3):
    x = (i % cols - (cols - 1) / 2) * spacing
    y = ((rows - 1) / 2 - i // cols) * spacing
    return f"Transform {{ translation {x:.3f} {y:.3f} 0 children [\n{body}\n] }}"

def world(cells, head=HEAD, extra="", nav="", vp="Viewpoint { position 0 0 12 }"):
    return head + vp + "\n" + nav + "\n" + BG + "\n" + extra + "\n" + "\n".join(cell(i, c) for i, c in enumerate(cells)) + "\n"

def shape(geometry, appearance=f"Appearance {{ material {MAT} }}"):
    return f"Shape {{ appearance {appearance} geometry {geometry} }}"

def ifs(extra, appearance=f"Appearance {{ material {MAT} }}", coord=QUAD, index="coordIndex [ 0 1 2 3 -1 ]"):
    return shape(f"IndexedFaceSet {{ solid FALSE {coord} {index} {extra} }}", appearance)

worlds = {}

# --- Color, ColorRGBA, Coordinate, CoordinateDouble, Normal, IndexedLineSet, PointSet ------------
TWO = "coord Coordinate { point [ -0.9 -0.9 0, 0 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, 0 0.9 0, -0.9 0.9 0 ] }"
TWO_IDX = "coordIndex [ 0 1 4 5 -1 1 2 3 4 -1 ]"
CUR = "Normal { vector [ -0.7 -0.7 0.5, 0.7 -0.7 0.5, 0.7 0.7 0.5, -0.7 0.7 0.5 ] }"
FACE_N = "Normal { vector [ 0 0.6 0.8, 0.6 0 0.8 ] }"
LINES = "coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0 0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] }"
PTS = "coord Coordinate { point [ " + ", ".join(f"{-0.9 + 0.3 * (i % 7):.2f} {-0.9 + 0.3 * (i // 7):.2f} 0" for i in range(49)) + " ] }"
def pt_colors(rgba):
    n = 49
    cols = [(i % 7 / 6, i // 7 / 6, 1 - i / n) for i in range(n)]
    return ", ".join(" ".join(f"{c:.2f}" for c in col + ((0.3 + 0.7 * (i % 7) / 6,) if rgba else ())) for i, col in enumerate(cols))
UNLIT = "Appearance { }"
worlds["vis-rendering.wrl"] = world([
    ifs("colorPerVertex TRUE color Color { color [ 1 0 0, 0 1 0, 0 0 1, 1 1 0 ] } colorIndex [ 0 1 2 3 -1 ]"),
    ifs("colorPerVertex FALSE color Color { color [ 0.9 0.2 0.2, 0.2 0.9 0.4 ] }", coord=TWO, index=TWO_IDX),
    ifs("colorPerVertex TRUE color ColorRGBA { color [ 1 0 0 1, 0 1 0 0.6, 0 0 1 0.3, 1 1 0 0.1 ] }"),
    ifs("", coord="coord CoordinateDouble { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.6 0.9 0, -0.6 0.9 0 ] }"),
    ifs(f"normalPerVertex TRUE normal {CUR}"),
    ifs(f"normalPerVertex FALSE normal {FACE_N}", coord=TWO, index=TWO_IDX),
    ifs("creaseAngle 0", coord="coord Coordinate { point [ -0.9 -0.9 0.4, 0.9 -0.9 0, 0.9 0.9 0.4, -0.9 0.9 0 ] }", index="coordIndex [ 0 1 2 -1 0 2 3 -1 ]"),
    shape(f"IndexedLineSet {{ {LINES} coordIndex [ 0 1 2 0 -1 1 3 2 -1 2 4 -1 ] colorPerVertex TRUE color Color {{ color [ 1 0 0, 0 1 0, 0 0 1, 1 1 0, 1 0 1 ] }} }}", UNLIT),
    shape(f"IndexedLineSet {{ {LINES} coordIndex [ 0 1 3 4 0 -1 ] }}", UNLIT),
    shape(f"PointSet {{ {PTS} color Color {{ color [ {pt_colors(False)} ] }} }}", UNLIT),
    shape(f"PointSet {{ {PTS} color ColorRGBA {{ color [ {pt_colors(True)} ] }} }}", UNLIT),
    shape(f"PointSet {{ {PTS} }}", UNLIT),
])

# --- Shape, Appearance, Material -----------------------------------------------------------------
def mat(fields, geometry=SPHERE):
    return shape(geometry, f"Appearance {{ material Material {{ {fields} }} }}")
BACK = "Transform { translation 0 0 -0.6 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.2 0.2 } } geometry Box { size 1.6 1.6 0.2 } } }"
worlds["vis-shape.wrl"] = world([
    "Shape { geometry Sphere { radius 0.9 } }",
    "Shape { appearance Appearance { } geometry Sphere { radius 0.9 } }",
    mat("diffuseColor 0.2 0.6 0.9"),
    mat("diffuseColor 0 0 0 emissiveColor 0.9 0.5 0.1"),
    mat("diffuseColor 0.3 0.3 0.8 specularColor 1 1 1 shininess 0.9"),
    mat("diffuseColor 0.3 0.3 0.8 specularColor 1 1 1 shininess 0.1"),
    mat("diffuseColor 0.8 0.3 0.3 ambientIntensity 0"),
    BACK + "\n" + mat("diffuseColor 0.2 0.8 0.3 transparency 0.5", "Box { size 1.4 1.4 1.4 }"),
    BACK + "\n" + mat("diffuseColor 0.2 0.8 0.3 transparency 1", "Box { size 1.4 1.4 1.4 }"),
    mat("diffuseColor 0.4 0.4 0.1 emissiveColor 0.1 0.2 0.4 specularColor 0.8 0.8 0.8 shininess 0.5 transparency 0.2"),
    "Shape { appearance Appearance { material Material { diffuseColor 1 0 0 } } }",
    BACK + "\n" + mat("diffuseColor 0.2 0.5 0.9 transparency 0.3", "Sphere { radius 0.7 }")
    + "\n" + "Transform { translation 0.3 0.2 0.9 children " + mat("diffuseColor 0.9 0.8 0.2 transparency 0.6", "Sphere { radius 0.5 }") + " }",
])

# --- Lights (headlight off; each light is scoped to the group of its cell) ----------------------
NOHEAD = "NavigationInfo { headlight FALSE }"
LIT_SPHERE = shape(SPHERE, "Appearance { material Material { diffuseColor 0.9 0.9 0.9 specularColor 0.5 0.5 0.5 shininess 0.3 } }")
PLANE = shape("IndexedFaceSet { solid FALSE coord Coordinate { point [ -1 -1 0, 1 -1 0, 1 1 0, -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] normal Normal { vector [ 0 0 1 ] } }",
              "Appearance { material Material { diffuseColor 0.9 0.9 0.9 } }")
# a plane tessellated so a point or spot cone shows even for a light with per-vertex falloff
GRID = ("Shape { appearance Appearance { material Material { diffuseColor 0.9 0.9 0.9 } } geometry ElevationGrid { solid FALSE xDimension 17 zDimension 17 xSpacing 0.125 zSpacing 0.125 height [ "
        + " ".join(["0"] * 289) + " ] } }")
GRID = f"Transform {{ translation -1 1 0 rotation 1 0 0 1.5708 children {GRID} }}"
def lit(light, body=LIT_SPHERE):
    return f"Group {{ children [ {light}\n{body} ] }}"

D = lambda f: f"DirectionalLight {{ {f} }}"
worlds["vis-light-dir.wrl"] = world([
    lit(D("direction 0 0 -1")),
    lit(D("direction -1 -1 -1")),
    lit(D("direction 1 0 -0.3 color 1 0.4 0.2")),
    lit(D("direction 0 -1 -0.3 intensity 0.4")),
    lit(D("direction 0 0 -1 intensity 2")),
    lit(D("direction 0 0 -1 intensity 0 ambientIntensity 0.7 color 0.4 0.8 1")),
    lit(D("direction 0 0 -1 on FALSE")),
    lit(D("direction 2 -2 -2 color 0.6 1 0.6")),
    lit(D("direction 0 0 -1 intensity 1.5 color 1 0 0") + "\n" + D("direction 0 0 1 intensity 1.5 color 0 0 1")),
    f"Group {{ children [ Group {{ children {D('direction -1 0 -1 global TRUE')} }}\n{LIT_SPHERE} ] }}",
    f"Group {{ children [ Group {{ children {D('direction -1 0 -1 global FALSE')} }}\n{LIT_SPHERE} ] }}",
    lit(D("direction 0 0 -1 intensity 0.5") + "\n" + D("direction 1 1 -1 intensity 0.5 color 1 1 0")),
], nav=NOHEAD)

P = lambda f: f"PointLight {{ {f} }}"
worlds["vis-light-point.wrl"] = world([
    lit(P("location 0 0 1 radius 10"), GRID),
    lit(P("location 0.7 0.7 0.5 radius 10"), GRID),
    lit(P("location 0 0 1 radius 0.9"), GRID),
    lit(P("location 0 0 1 radius 10 attenuation 1 0 0"), GRID),
    lit(P("location 0 0 1 radius 10 attenuation 0 0 1"), GRID),
    lit(P("location 0 0 1 radius 10 intensity 0.4"), GRID),
    lit(P("location 0 0 1 radius 10 intensity 2 color 1 0.4 0.4"), GRID),
    lit(P("location 0 0 1 radius 10 intensity 0 ambientIntensity 0.6"), GRID),
    lit(P("location 0 0 1 radius 10 on FALSE"), GRID),
    lit(P("location 0 0 -1 radius 10"), GRID),
    f"Group {{ children [ Group {{ children {P('location 0 0 1 radius 10 global TRUE')} }}\n{GRID} ] }}",
    lit(P("location -0.5 0.3 0.4 radius 10 color 0.2 0.5 1 attenuation 1 0 0.2"), GRID),
], nav=NOHEAD)

S = lambda f: f"SpotLight {{ {f} }}"
worlds["vis-light-spot.wrl"] = world([
    lit(S("location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 0.3 cutOffAngle 0.6"), GRID),
    lit(S("location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 0.05 cutOffAngle 0.5"), GRID),
    lit(S("location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 1.2 cutOffAngle 0.5"), GRID),
    lit(S("location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 1.5 cutOffAngle 1.5"), GRID),
    lit(S("location 0 0 1.5 direction 0 0 -1 radius 1.2 beamWidth 0.8 cutOffAngle 0.9"), GRID),
    lit(S("location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 0.8 cutOffAngle 0.9 attenuation 0 0 1"), GRID),
    lit(S("location 0 0 1.5 direction 0.4 -0.3 -1 radius 10 beamWidth 0.3 cutOffAngle 0.6"), GRID),
    lit(S("location 0.6 0.5 1 direction 0 0 -1 radius 10 beamWidth 0.4 cutOffAngle 0.7"), GRID),
    lit(S("location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 0.3 cutOffAngle 0.6 intensity 2"), GRID),
    lit(S("location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 0.3 cutOffAngle 0.6 color 1 0.3 0.3"), GRID),
    lit(S("location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 0.3 cutOffAngle 0.6 on FALSE"), GRID),
    lit(S("location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 0.3 cutOffAngle 0.6 intensity 0 ambientIntensity 0.6"), GRID),
], nav=NOHEAD)

# --- ImageTexture, PixelTexture, TextureTransform ------------------------------------------------
def rgba(r, g, b, a):
    return ((r << 24) | (g << 16) | (b << 8) | a) - (1 << 32 if r >= 128 else 0)
def rgb(r, g, b):
    return (r << 16) | (g << 8) | b
BOXG = "Box { size 1.5 1.5 1.5 }"
TC2 = "texCoord TextureCoordinate { point [ 0 0, 2 0, 2 2, 0 2 ] }"
def tq(tex, tt="", mat=MAT, extra=TC2):
    ap = f"Appearance {{ material {mat} texture {tex} {('textureTransform ' + tt) if tt else ''} }}"
    return ifs(extra, ap)
IMG = lambda url, f="": f'ImageTexture {{ url "{url}" {f} }}'
PX = lambda img, f="": f"PixelTexture {{ image {img} {f} }}"
px1 = f"2 2 1 0 255 128 64"
px2 = f"2 2 2 {0x00FF} {0xFF80} {0x80FF} {0x40C0}"
px3 = f"2 2 3 {rgb(255,0,0)} {rgb(0,255,0)} {rgb(0,0,255)} {rgb(255,255,0)}"
px4 = f"2 2 4 {rgba(255,0,0,255)} {rgba(0,255,0,128)} {rgba(0,0,255,255)} {rgba(255,255,0,64)}"
worlds["vis-texturing.wrl"] = world([
    shape(BOXG, f"Appearance {{ material {MAT} texture {IMG(PBR + 'base.png')} }}"),
    shape(BOXG, f"Appearance {{ material {MAT} texture {IMG(PBR + 'base-alpha.png')} }}"),
    tq(IMG(CHECKER, "repeatS FALSE repeatT FALSE")),
    tq(IMG(CHECKER, "repeatS TRUE repeatT TRUE")),
    tq(PX(px1), extra="texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] }"),
    tq(PX(px2), extra="texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] }"),
    tq(PX(px3), extra="texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] }"),
    tq(PX(px4), extra="texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] }"),
    tq(IMG(CHECKER), "TextureTransform { translation 0.25 0.25 }", extra="texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] }"),
    tq(IMG(CHECKER), "TextureTransform { rotation 0.5 center 0.5 0.5 scale 1.5 1.5 }", extra="texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] }"),
    tq(IMG(CHECKER), "TextureTransform { scale 0.5 1.5 rotation 0.3 translation 0.1 -0.2 }", extra="texCoord TextureCoordinate { point [ 0 0, 1 0, 1 1, 0 1 ] }"),
    shape(SPHERE, f"Appearance {{ material Material {{ diffuseColor 1 0.5 0.4 }} texture {IMG(PBR + 'base.png')} }}"),
])

# --- TextureCoordinate3D, TextureCoordinate4D with a PixelTexture3D ------------------------------
def img3d(comp, w, h, d, f):
    vals = [comp, w, h, d] + [f(x, y, z) for z in range(d) for y in range(h) for x in range(w)]
    return " ".join(str(v) for v in vals)
P3 = "PixelTexture3D { image [ " + img3d(3, 2, 2, 2, lambda x, y, z: rgb(255 * x, 255 * y, 255 * z)) + " ] }"
Q3 = "coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] } coordIndex [ 0 1 2 3 -1 ]"
t3 = lambda tc: shape(f"IndexedFaceSet {{ solid FALSE {Q3} texCoord {tc} }}", f"Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {P3} }}")
worlds["vis-texturing3d.wrl"] = world([
    t3("TextureCoordinate3D { point [ 0 0 0.1, 1 0 0.4, 1 1 0.9, 0 1 0.6 ] }"),
    t3("TextureCoordinate4D { point [ 0 0 0.2 1, 2 0 0.4 2, 2 2 1.8 2, 0 1 0.6 1 ] }"),
    t3("TextureCoordinate3D { point [ 0 0 0, 1 0 0, 1 1 1, 0 1 1 ] }"),
    t3("TextureCoordinate4D { point [ 0 0 0 1, 1 0 0 1, 1 1 1 1, 0 1 1 1 ] }"),
], head=HEAD3D)

if __name__ == "__main__":
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for name, text in worlds.items():
        open(os.path.join(out, name), "w").write(text)
    print(len(worlds), "worlds ->", out)
