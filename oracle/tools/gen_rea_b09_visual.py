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
  vis-text-font     Text, FontStyle: family, style, size, spacing
  vis-text-layout   Text, FontStyle: justify, direction, length, maxExtent, several strings
  vis-colorrgba     ColorRGBA: alpha per face, per vertex, on lines, with Material transparency
  vis-linecolor     IndexedLineSet: colorPerVertex FALSE, colorIndex, ColorRGBA, index boundaries
  vis-material-ambient   Material ambientIntensity under ambient light (Directional, Point, Spot)
  vis-layout        LayoutGroup (Layout, Viewport)
  vis-screen        ScreenGroup, ScreenFontStyle
  vis-screen-text   ScreenFontStyle: size, direction, justify, length, maxExtent, family, style, spacing
  vis-screen-ortho  ScreenGroup, ScreenFontStyle under an OrthoViewpoint
  vis-screen-view   ScreenGroup, ScreenFontStyle with a rotated, scaled, distant parent and a non-default viewpoint
  vis-localfog      LocalFog
  vis-texturebackground-*   TextureBackground (front, corner, sky + transparency, partial)
  vis-ortho-40-* / -41-*    OrthoViewpoint with an MFFloat (4.0) and an SFVec4f (4.1) fieldOfView

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

# --- ColorRGBA: alpha per face, per vertex, on lines, with Material transparency -----------------
# A two-tone backdrop (white left, dark right) shows the alpha of what is drawn in front of it.
BD = ("Transform { translation 0 0 -0.4 children [ Shape { appearance Appearance { material Material { diffuseColor 0 0 0 emissiveColor 1 1 1 } } "
      "geometry IndexedFaceSet { coord Coordinate { point [ -1.05 -1.05 0, 0 -1.05 0, 0 1.05 0, -1.05 1.05 0 ] } coordIndex [ 0 1 2 3 -1 ] } } "
      "Shape { appearance Appearance { material Material { diffuseColor 0 0 0 emissiveColor 0.15 0.15 0.15 } } "
      "geometry IndexedFaceSet { coord Coordinate { point [ 0 -1.05 0, 1.05 -1.05 0, 1.05 1.05 0, 0 1.05 0 ] } coordIndex [ 0 1 2 3 -1 ] } } ] }")
def over(body):
    return BD + "\n" + body
LINES2 = "coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, -0.9 -0.3 0, 0.9 -0.3 0, -0.9 0.3 0, 0.9 0.3 0, -0.9 0.9 0, 0.9 0.9 0 ] }"
LIDX = "coordIndex [ 0 1 -1 2 3 -1 4 5 -1 6 7 -1 ]"
GRID4 = ("ElevationGrid { solid FALSE xDimension 3 zDimension 3 xSpacing 0.9 zSpacing 0.9 height [ 0 0 0 0 0 0 0 0 0 ] colorPerVertex TRUE "
         "color ColorRGBA { color [ 1 0 0 0.2, 1 0 0 0.6, 1 0 0 1, 0 1 0 0.2, 0 1 0 0.6, 0 1 0 1, 0 0 1 0.2, 0 0 1 0.6, 0 0 1 1 ] } }")
GRID4 = f"Transform {{ translation -0.9 0.9 0 rotation 1 0 0 1.5708 children Shape {{ appearance Appearance {{ material {MAT} }} geometry {GRID4} }} }}"
worlds["vis-colorrgba.wrl"] = world([
    over(ifs("colorPerVertex FALSE color ColorRGBA { color [ 1 0 0 1, 0 0 1 0.5 ] }", coord=TWO, index=TWO_IDX)),
    over(ifs("colorPerVertex FALSE color ColorRGBA { color [ 0.9 0.2 0.2 0.1, 0.2 0.2 0.9 0.9 ] }", coord=TWO, index=TWO_IDX)),
    over(ifs("colorPerVertex FALSE color ColorRGBA { color [ 1 0 0 0, 0 0 1 1 ] }", coord=TWO, index=TWO_IDX)),
    over(ifs("colorPerVertex TRUE color ColorRGBA { color [ 1 0 0 1, 1 0 0 0.66, 1 0 0 0.33, 1 0 0 0 ] }")),
    over(ifs("colorPerVertex TRUE color ColorRGBA { color [ 1 0 0 0.2, 0 1 0 0.5, 0 0 1 0.8 ] } colorIndex [ 2 0 1 0 -1 ]")),
    over(ifs("colorPerVertex TRUE color ColorRGBA { color [ 1 0 0 0.5, 0 1 0 0.5, 0 0 1 0.5, 1 1 0 0.5 ] }", appearance=f"Appearance {{ material Material {{ diffuseColor 0.8 0.8 0.8 transparency 0.5 }} }}")),
    over(ifs("colorPerVertex TRUE color ColorRGBA { color [ 1 0 0 0.5, 0 1 0 0.5, 0 0 1 0.5, 1 1 0 0.5 ] }", appearance=f"Appearance {{ material Material {{ diffuseColor 0.8 0.8 0.8 specularColor 1 1 1 shininess 0.6 }} }}")),
    over(ifs("colorPerVertex FALSE color ColorRGBA { color [ 1 0 0 0.5, 0 0 1 0.5 ] }", appearance=UNLIT, coord=TWO, index=TWO_IDX)),
    over(shape(f"IndexedLineSet {{ {LINES2} {LIDX} colorPerVertex TRUE color ColorRGBA {{ color [ 1 0 0 1, 1 0 0 0.5, 0 0 1 1, 0 0 1 0.5, 0 1 0 1, 0 1 0 0.5, 1 1 0 1, 1 1 0 0.5 ] }} }}", UNLIT)),
    over(shape(f"IndexedLineSet {{ {LINES2} {LIDX} colorPerVertex FALSE color ColorRGBA {{ color [ 1 0 0 1, 1 0 0 0.6, 0 0 1 0.3, 0 0 1 0 ] }} }}", UNLIT)),
    over(GRID4),
    over(shape("IndexedTriangleSet { coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0 0.9 0, 0.9 0.9 0.2, -0.9 0.9 0.2 ] } index [ 0 1 2, 2 3 4 ] "
               "color ColorRGBA { color [ 1 0 0 0.7, 0 1 0 0.4, 0 0 1 0.7, 1 1 0 0.3, 0 1 1 0.3 ] } }", f"Appearance {{ material {MAT} }}")),
])

# --- IndexedLineSet colors: colorPerVertex FALSE, explicit colorIndex, RGBA, index boundaries ----
FAN = "coord Coordinate { point [ " + ", ".join(f"{-0.9 + 0.45 * (i % 5):.2f} {-0.9 + 0.9 * (i // 5):.2f} 0" for i in range(15)) + " ] }"
# rows of 5 points; each row is one polyline of 5 vertices
ROWS = "coordIndex [ 0 1 2 3 4 -1 5 6 7 8 9 -1 10 11 12 13 14 -1 ]"
CX = "color Color { color [ 1 0 0, 0 1 0, 0 0 1, 1 1 0, 1 0 1 ] }"
CA = "color ColorRGBA { color [ 1 0 0 1, 0 1 0 0.5, 0 0 1 0.25, 1 1 0 0.75, 1 0 1 1 ] }"
def ils(fields, coord=FAN, idx=ROWS, ap=UNLIT):
    return over(shape(f"IndexedLineSet {{ {coord} {idx} {fields} }}", ap))
worlds["vis-linecolor.wrl"] = world([
    ils(f"colorPerVertex FALSE color Color {{ color [ 1 0 0, 0 1 0, 0 0 1 ] }}"),
    ils(f"colorPerVertex FALSE color Color {{ color [ 1 0 0, 0 1 0, 0 0 1 ] }} colorIndex [ 2 0 1 ]"),
    ils(f"colorPerVertex FALSE {CX} colorIndex [ 4 4 0 ]"),
    ils(f"colorPerVertex TRUE {CX} colorIndex [ 0 1 2 3 4 -1 4 3 2 1 0 -1 0 0 4 4 2 -1 ]"),
    ils(f"colorPerVertex TRUE {CX}"),
    ils(f"colorPerVertex TRUE {CX} colorIndex [ 0 0 0 0 0 -1 4 4 4 4 4 -1 0 1 2 3 4 -1 ]"),
    ils(f"colorPerVertex TRUE {CA}"),
    ils(f"colorPerVertex FALSE color ColorRGBA {{ color [ 1 0 0 1, 0 1 0 0.5, 0 0 1 0.2 ] }}"),
    ils(f"colorPerVertex FALSE color ColorRGBA {{ color [ 1 0 0 1, 0 1 0 0.5, 0 0 1 0.2 ] }} colorIndex [ 1 2 0 ]"),
    ils(f"colorPerVertex TRUE {CA} colorIndex [ 4 3 2 1 0 -1 0 1 2 3 4 -1 2 2 2 2 2 -1 ]"),
    ils(f"colorPerVertex TRUE {CX}", idx="coordIndex [ 0 1 2 3 4 -1 5 -1 6 7 -1 8 9 10 11 12 13 14 ]"),
    ils(f"colorPerVertex FALSE {CX} colorIndex [ 0 1 2 ]", idx="coordIndex [ 0 1 2 3 4 -1 5 6 7 8 9 -1 10 11 12 13 14 ]"),
])

# --- Material ambientIntensity under ambient light ---------------------------------------------
AM = lambda f, c="0.9 0.9 0.9": mat(f"diffuseColor {c} {f}")
# the light shines away from the camera (+z), so the lit side gets no diffuse light: what shows is ambient
DA = lambda a, extra="": D(f"direction 0 0 1 intensity 0.5 ambientIntensity {a} {extra}")
worlds["vis-material-ambient.wrl"] = world([
    lit(DA(0.7), AM("ambientIntensity 0")),
    lit(DA(0.7), AM("ambientIntensity 0.2")),
    lit(DA(0.7), AM("ambientIntensity 0.5")),
    lit(DA(0.7), AM("ambientIntensity 1")),
    lit(DA(0.7, "color 0.4 0.8 1"), AM("ambientIntensity 1", "0.9 0.5 0.3")),
    lit(D("direction 0 0 -1 intensity 0.6 ambientIntensity 0.5"), AM("ambientIntensity 0.5", "0.8 0.4 0.8")),
    lit(P("location 0 0 -2 radius 10 intensity 0.5 ambientIntensity 0.8"), AM("ambientIntensity 0.5")),
    lit(S("location 0 0 -2 direction 0 0 1 radius 10 beamWidth 0.3 cutOffAngle 0.6 intensity 0.5 ambientIntensity 0.8"), AM("ambientIntensity 1")),
    lit(DA(0.3) + "\n" + DA(0.3, "color 1 0.2 0.2"), AM("ambientIntensity 1")),
    lit(DA(0), AM("ambientIntensity 1")),
    lit(DA(0.7), mat("diffuseColor 0.3 0.6 0.3 emissiveColor 0.4 0 0 specularColor 1 1 1 shininess 0.5 ambientIntensity 0.6")),
    lit(DA(0.7), mat("diffuseColor 0.8 0.8 0.8 ambientIntensity 0.8 transparency 0.5")),
], nav=NOHEAD)

# --- IndexedLineSet: invalid and out-of-range indexes (X_ITE decides what is drawn) -----------------
def ilsx(fields, idx=ROWS, coord=FAN):
    return ils(fields, coord=coord, idx=idx)
worlds["vis-linecolor-invalid.wrl"] = world([
    ilsx(f"colorPerVertex TRUE {CX} colorIndex [ 0 1 -5 3 4 -1 4 3 2 1 0 -1 0 1 2 3 4 -1 ]"),
    ilsx(f"colorPerVertex TRUE {CX} colorIndex [ 0 1 2 3 99 -1 4 3 2 1 0 -1 0 1 2 3 4 -1 ]"),
    ilsx(f"colorPerVertex FALSE {CX} colorIndex [ ]"),
    ilsx(f"colorPerVertex TRUE {CX} colorIndex [ ]"),
    ilsx(f"colorPerVertex FALSE {CX} colorIndex [ 2 99 1 ]"),
    ilsx(f"colorPerVertex FALSE {CX} colorIndex [ 2 -3 1 ]"),
    ilsx("colorPerVertex TRUE color Color { color [ ] }"),
    ilsx(f"colorPerVertex TRUE {CX}", idx="coordIndex [ 0 1 -2 3 4 -1 5 6 7 8 9 -1 10 11 12 13 14 -1 ]"),
    ilsx(f"colorPerVertex TRUE {CX}", idx="coordIndex [ 0 1 2 99 4 -1 5 6 7 8 9 -1 10 11 12 13 14 -1 ]"),
    ilsx(f"colorPerVertex TRUE {CX} colorIndex [ 0 1 2 -1 ]"),
    ilsx(f"colorPerVertex TRUE {CX} colorIndex [ 0 1 2 3 4 -1 0 1 2 3 4 -1 0 1 2 3 4 -1 4 4 4 -1 3 3 3 3 3 -1 ]"),
    ilsx(f"colorPerVertex TRUE color ColorRGBA {{ color [ 1 0 0 1, 0 1 0 0.5, 0 0 1 0.2 ] }} colorIndex [ 0 1 2 3 4 -1 2 1 0 7 7 -1 ]"),
])

# --- Material ambientIntensity with textured surfaces ------------------------------------------
TXT = "texture ImageTexture { url \"../synthetic-rea-b03/proj-checker.png\" }"
def tmat(fields, light, geometry=SPHERE, tex=TXT, extra=""):
    return lit(light, shape(geometry, f"Appearance {{ material Material {{ diffuseColor 0.9 0.9 0.9 {fields} }} {tex} {extra} }}"))
worlds["vis-material-ambient-tex.wrl"] = world([
    tmat("ambientIntensity 0", DA(0.7)),
    tmat("ambientIntensity 0.2", DA(0.7)),
    tmat("ambientIntensity 0.5", DA(0.7)),
    tmat("ambientIntensity 1", DA(0.7)),
    tmat("ambientIntensity 1", DA(0.7, "color 0.4 0.8 1")),
    tmat("diffuseColor 0.9 0.4 0.3 ambientIntensity 1", DA(0.7)),
    tmat("ambientIntensity 0.5", P("location 0 0 -2 radius 10 intensity 0.5 ambientIntensity 0.8")),
    tmat("ambientIntensity 1", S("location 0 0 -2 direction 0 0 1 radius 10 beamWidth 0.3 cutOffAngle 0.6 intensity 0.5 ambientIntensity 0.8")),
    tmat("ambientIntensity 1", DA(0.7), geometry="Box { size 1.4 1.4 1.4 }"),
    tmat("ambientIntensity 1", DA(0.7), tex="texture PixelTexture { image 2 2 3 0xff0000 0x00ff00 0x0000ff 0xffff00 }"),
    tmat("ambientIntensity 1 transparency 0.5", DA(0.7)),
    tmat("ambientIntensity 1", DA(0.7), tex="texture ImageTexture { url \"../synthetic-rea-b03/proj-checker.png\" } textureTransform TextureTransform { scale 2 2 }"),
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

# --- Text, FontStyle -------------------------------------------------------------------------------
HEADF = "#X3D V4.0 utf8\nPROFILE Full\n"
def txt(strings, fs="", extra="", mat="Material { diffuseColor 0.95 0.95 0.95 }"):
    s = " ".join(f'"{x}"' for x in strings)
    fontstyle = f"fontStyle FontStyle {{ {fs} }}" if fs else ""
    return shape(f"Text {{ string [ {s} ] {fontstyle} {extra} solid FALSE }}", f"Appearance {{ material {mat} }}")
MARK = "Shape { appearance Appearance { material Material { emissiveColor 1 0.2 0.2 diffuseColor 0 0 0 } } geometry Box { size 0.08 0.08 0.08 } }"
def txm(strings, fs="", extra=""):
    return MARK + "\n" + txt(strings, fs, extra)

worlds["vis-text-font.wrl"] = world([
    txt(["Abc"]),
    txt(["Abc"], 'family "SERIF"'),
    txt(["Abc"], 'family "SANS"'),
    txt(["Abc"], 'family "TYPEWRITER"'),
    txt(["Abc"], 'family "SERIF" style "BOLD"'),
    txt(["Abc"], 'family "SERIF" style "ITALIC"'),
    txt(["Abc"], 'family "SANS" style "BOLDITALIC"'),
    txt(["Ag"], "size 0.5"),
    txt(["Ag"], "size 1.4"),
    txt(["Ab", "Cd"], "spacing 2"),
    txt(["Abc"], 'family [ "NoSuchFont" "SANS" ]'),
    txt(["Abc"], 'family "Arial" style "PLAIN"'),
], head=HEADF)

worlds["vis-text-layout.wrl"] = world([
    txm(["Ab", "Xy"], 'justify [ "BEGIN" "BEGIN" ]'),
    txm(["Ab", "Xy"], 'justify [ "MIDDLE" "MIDDLE" ]'),
    txm(["Ab", "Xy"], 'justify [ "END" "END" ]'),
    txm(["Ab", "Xy"], 'justify [ "BEGIN" "FIRST" ]'),
    txm(["Abc"], "horizontal FALSE"),
    txm(["Abc"], "horizontal FALSE leftToRight FALSE topToBottom FALSE"),
    txm(["Abc"], "leftToRight FALSE"),
    txm(["Ab", "Xy"], "topToBottom FALSE"),
    txm(["Ab"], "", "length [ 3 ]"),
    txm(["Abcdefg"], "", "maxExtent 1"),
    txm(["Ab", "Xyz"], "", "length [ 1 2 ] maxExtent 2.5"),
    txm(["A", "", "B"], "spacing 1.5"),
], head=HEADF)

# --- LayoutGroup, ScreenGroup, ScreenFontStyle ---------------------------------------------------
LAYOUT_HEAD = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT Layout:2\n"
def rect(w, h, color):
    return (f"Shape {{ appearance Appearance {{ material Material {{ emissiveColor {color} diffuseColor 0 0 0 }} }} geometry IndexedFaceSet "
            f"{{ coord Coordinate {{ point [ {-w/2} {-h/2} 0, {w/2} {-h/2} 0, {w/2} {h/2} 0, {-w/2} {h/2} 0 ] }} coordIndex [ 0 1 2 3 -1 ] }} }}")
def layout(fields, children, viewport=""):
    return f"LayoutGroup {{ layout Layout {{ {fields} }} {viewport} children [ {children} ] }}"
LP = 'sizeUnits [ "PIXEL" "PIXEL" ] scaleMode [ "PIXEL" "PIXEL" ]'
SCENE = "Transform { translation -1.5 0 0 children " + shape("Box { size 1.5 1.5 1.5 }", "Appearance { material Material { diffuseColor 0.3 0.5 0.9 } }") + " }\n"
SCENE += "Transform { translation 1.5 0 0 children " + shape("Sphere { radius 0.9 }", "Appearance { material Material { diffuseColor 0.9 0.6 0.2 } }") + " }"
worlds["vis-layout.wrl"] = (
    LAYOUT_HEAD + "Viewpoint { position 0 0 8 }\n" + BG + "\n" + SCENE + "\n"
    + layout(f'align [ "LEFT" "BOTTOM" ] offsetUnits [ "PIXEL" "PIXEL" ] offset [ 10 10 ] size [ 100 50 ] {LP}', rect(100, 50, "1 0.4 0.1")) + "\n"
    + layout(f'align [ "RIGHT" "TOP" ] offsetUnits [ "PIXEL" "PIXEL" ] offset [ -10 -10 ] size [ 80 80 ] {LP}', rect(80, 80, "0.2 0.9 0.4")) + "\n"
    + layout(f'align [ "CENTER" "BOTTOM" ] offsetUnits [ "FRACTION" "PIXEL" ] offset [ 0 10 ] sizeUnits [ "FRACTION" "PIXEL" ] size [ 0.5 30 ] scaleMode [ "FRACTION" "PIXEL" ]', rect(1, 1, "0.9 0.2 0.9")) + "\n"
    + layout(f'align [ "LEFT" "TOP" ] offset [ 0.05 -0.05 ] size [ 0.2 0.2 ] scaleMode [ "STRETCH" "STRETCH" ]', rect(1, 1, "0.2 0.7 0.9")) + "\n"
    + layout(f'align [ "CENTER" "CENTER" ] size [ 120 60 ] {LP}', rect(60, 30, "0.9 0.9 0.2") + "\n" + rect(20, 20, "0.1 0.1 0.1"),
             "viewport Viewport { clipBoundary [ 0.4 0.6 0.4 0.6 ] }") + "\n"
)
SG = lambda tr, body: f"Transform {{ translation {tr} children ScreenGroup {{ children [ {body} ] }} }}"
SFS = lambda f: f"fontStyle ScreenFontStyle {{ {f} }}"
def stext(strings, f, mat="Material { emissiveColor 1 1 1 diffuseColor 0 0 0 }"):
    s = " ".join(f'"{x}"' for x in strings)
    return shape(f"Text {{ string [ {s} ] {SFS(f)} solid FALSE }}", f"Appearance {{ material {mat} }}")
worlds["vis-screen.wrl"] = (
    LAYOUT_HEAD + "Viewpoint { position 0 0 8 }\n" + BG + "\n" + SCENE + "\n"
    + SG("-2.5 2 0", rect(60, 40, "1 0.4 0.1")) + "\n"
    + SG("2.5 2 0", rect(60, 40, "0.2 0.9 0.4")) + "\n"
    + SG("0 0 2", rect(30, 30, "0.9 0.2 0.9")) + "\n"
    + SG("-2.5 -2 0", stext(["Screen"], "pointSize 24")) + "\n"
    + SG("2.5 -2 0", stext(["Screen", "Text"], 'pointSize 12 justify [ "MIDDLE" "MIDDLE" ] spacing 1.5')) + "\n"
    + SG("0 -2.8 0", stext(["Bold"], 'family "SANS" style "BOLD" pointSize 18')) + "\n"
    + "Transform { translation 0 2.8 -3 children ScreenGroup { children [ " + rect(60, 40, "0.5 0.5 0.5") + " ] } }\n"
)

# --- ScreenFontStyle layout and ScreenGroup in other views ----------------------------------------
STX = lambda strings, f: SG("0 0 0", stext(strings, f))
worlds["vis-screen-text.wrl"] = world([
    STX(["Abc"], "pointSize 8"),
    STX(["Abc"], "pointSize 36"),
    STX(["Ab", "Cd"], "pointSize 16 horizontal FALSE"),
    STX(["Ab", "Cd"], "pointSize 16 horizontal FALSE leftToRight FALSE topToBottom FALSE"),
    STX(["Ab", "Cd"], 'pointSize 14 justify [ "END" "END" ]'),
    STX(["Ab", "Cdefg"], 'pointSize 14 justify [ "BEGIN" "FIRST" ] spacing 1.4'),
    STX(["Abcd"], "pointSize 14").replace("solid FALSE", "length [ 90 ] solid FALSE"),
    STX(["Abcdefgh"], "pointSize 14").replace("solid FALSE", "maxExtent 40 solid FALSE"),
    STX(["Mono"], 'family "TYPEWRITER" pointSize 16'),
    STX(["Italic"], 'family "SERIF" style "ITALIC" pointSize 20'),
    STX(["Sans"], 'family [ "NoSuchFont" "SANS" ] style "BOLDITALIC" pointSize 16'),
    STX(["A", "", "B"], "pointSize 14 spacing 2"),
], head=LAYOUT_HEAD, vp="Viewpoint { position 0 0 12 }")
worlds["vis-screen-ortho.wrl"] = (
    "#X3D V4.1 utf8\nPROFILE Full\nCOMPONENT Layout:2\nOrthoViewpoint { position 0 0 10 fieldOfView -5 -4 5 4 }\n" + BG + "\n" + SCENE + "\n"
    + SG("-2.5 2 0", rect(60, 40, "1 0.4 0.1")) + "\n"
    + SG("2.5 2 -3", rect(60, 40, "0.2 0.9 0.4")) + "\n"
    + SG("-2.5 -2 0", stext(["Screen"], "pointSize 24")) + "\n"
    + SG("2.5 -2 0", stext(["Ortho", "Text"], 'pointSize 14 justify [ "MIDDLE" "MIDDLE" ]')) + "\n"
)
worlds["vis-screen-view.wrl"] = (
    LAYOUT_HEAD + "Viewpoint { position 1 0.5 9 orientation 0 1 0 0.2 fieldOfView 0.6 }\n" + BG + "\n" + SCENE + "\n"
    + "Transform { translation -2 1.5 0 rotation 0 0 1 0.5 scale 2 1 1 children ScreenGroup { children [ " + rect(60, 40, "1 0.4 0.1") + " ] } }\n"
    + "Transform { translation 2 1.5 -2 rotation 0 1 0 0.8 children ScreenGroup { children [ " + rect(60, 40, "0.2 0.9 0.4") + " ] } }\n"
    + "Transform { translation -2 -2 0 rotation 0 0 1 0.4 children ScreenGroup { children [ " + stext(["Turned"], "pointSize 20") + " ] } }\n"
    + "Transform { translation 2 -2 -4 children ScreenGroup { children [ " + stext(["Far", "away"], "pointSize 14") + " ] } }\n"
    + "Transform { translation 0 0 3 children ScreenGroup { children [ Transform { translation 40 0 0 children " + rect(20, 20, "0.9 0.2 0.9") + " } ] } }\n"
)

# --- LocalFog, TextureBackground -----------------------------------------------------------------
BOX3 = "".join(
    f"Transform {{ translation {dx} 0 {z} children " + shape("Sphere { radius 0.45 }", "Appearance { material Material { diffuseColor 0.9 0.6 0.4 } }") + " }\n"
    for dx, z in ((-0.7, 0), (0, -4), (0.7, -8)))
def fogcell(fog):
    return f"Group {{ children [ {fog}\n{BOX3} ] }}"
LF = lambda f: f"LocalFog {{ {f} }}"
worlds["vis-localfog.wrl"] = world([
    fogcell(""),
    fogcell(LF("color 0.9 0.2 0.2 visibilityRange 6")),
    fogcell(LF("color 0.2 0.9 0.4 visibilityRange 12")),
    fogcell(LF("color 0.9 0.9 0.9 visibilityRange 6 fogType \"EXPONENTIAL\"")),
    fogcell(LF("color 0.2 0.4 1 visibilityRange 12 fogType \"EXPONENTIAL\"")),
    fogcell(LF("color 1 1 0 visibilityRange 0")),
    fogcell(LF("color 1 0 1 visibilityRange 6 enabled FALSE")),
    fogcell(LF("color 0.9 0.2 0.2 visibilityRange 3")),
    fogcell(LF("color 0.5 0.5 0.5 visibilityRange 10 fogType \"EXPONENTIAL2\"")),
    fogcell(LF("color 0 1 1 visibilityRange 20 fogType \"LINEAR\"")),
    f"Group {{ children [ Group {{ children [ {LF('color 1 0.5 0 visibilityRange 6')} ] }}\n{BOX3} ] }}",
    fogcell(LF("color 0.9 0.2 0.2 visibilityRange 100")),
], head=HEADF, vp="Viewpoint { position 0 0 14 fieldOfView 0.9 }")

def tbg(fields, vp="Viewpoint { position 0 0 5 }", tex_sides="front back left right top bottom"):
    sides = " ".join(f'{s}Texture ImageTexture {{ url "{PBR}env-{s}.png" }}' for s in tex_sides.split())
    return (HEADF + vp + "\n" + f"TextureBackground {{ {sides} {fields} }}\n"
            + shape("Sphere { radius 0.8 }", "Appearance { material Material { diffuseColor 0.8 0.8 0.8 } }") + "\n")
worlds["vis-texturebackground-front.wrl"] = tbg("")
worlds["vis-texturebackground-corner.wrl"] = tbg("", "Viewpoint { position 0 0 5 orientation 0.3 0.9 0.1 0.9 }")
worlds["vis-texturebackground-sky.wrl"] = tbg("skyColor [ 0.1 0.2 0.8, 0.9 0.9 0.6 ] skyAngle 1.4 groundColor [ 0.3 0.2 0.1, 0.5 0.4 0.3 ] groundAngle 1.4 transparency 0.5",
                                              "Viewpoint { position 0 0 5 orientation 1 0 0 0.3 }")
worlds["vis-texturebackground-partial.wrl"] = tbg("skyColor 0.9 0.4 0.1", "Viewpoint { position 0 0 5 orientation 0 1 0 0.6 }", "front top")

# --- OrthoViewpoint (X3D 4.0 MFFloat, 4.1 SFVec4f) -----------------------------------------------
DOTS = "".join(
    "Transform { translation %d %d 0 children Shape { appearance Appearance { material Material { diffuseColor %s } } geometry Box { size 1.2 1.2 1.2 } } }\n"
    % (x, y, "0.9 0.2 0.2" if (x, y) == (0, 0) else ("0.3 0.5 0.9" if (x + y) % 4 == 0 else "0.7 0.7 0.7"))
    for x in range(-6, 7, 2) for y in range(-6, 7, 2))
def ortho(version, fov, extra=""):
    if fov is None:
        field = ""
    elif version == "4.0":
        field = "fieldOfView [ %s ]" % " ".join(map(str, fov))
    else:
        field = "fieldOfView %s" % " ".join(map(str, fov))
    return f"#X3D V{version} utf8\nPROFILE Full\nOrthoViewpoint {{ position 0 0 10 {field} {extra} }}\n{BG}\n{DOTS}"
for v in ("4.0", "4.1"):
    t = v.replace(".", "")
    worlds[f"vis-ortho-{t}-default.wrl"] = ortho(v, None)
    worlds[f"vis-ortho-{t}-wide.wrl"] = ortho(v, (-8, -4, 8, 4))
    worlds[f"vis-ortho-{t}-tall.wrl"] = ortho(v, (-3, -8, 3, 8))
    worlds[f"vis-ortho-{t}-offset.wrl"] = ortho(v, (0, -2, 8, 6))
    worlds[f"vis-ortho-{t}-rotated.wrl"] = ortho(v, (-5, -5, 5, 5), "orientation 0 0 1 0.5")

if __name__ == "__main__":
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for name, text in worlds.items():
        open(os.path.join(out, name), "w").write(text)
    print(len(worlds), "worlds ->", out)
