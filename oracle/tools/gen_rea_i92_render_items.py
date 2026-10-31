#!/usr/bin/env python3
"""Render-core item worlds (issue #92): transparency sorting, lighting model, headlight and fog.

  python3 -I oracle/tools/gen_rea_i92_render_items.py oracle/synthetic-rea-i92-render-items

Writes i92-*.wrl next to nothing else: the directory is new. Each world is a grid of cells (the helpers of
gen_rea_b09_visual.py) that vary one thing side by side. Newly authored; no historical content.

  i92-hl-*      NavigationInfo headlight: default (TRUE), FALSE, with a DirectionalLight, a rotated viewpoint, an
                OrthoViewpoint, a second NavigationInfo, headlight under a Transform of the viewpoint
  i92-lm-*      lighting model: normals (smooth, flat, creaseAngle, per-face), two-sided faces, Material with Color /
                ColorRGBA, specular / shininess / emissive / ambient, point-light attenuation
  i92-tr-*      transparent sorting: listing order, depth, nesting, opaque in between, ties, texture alpha, ortho
  i92-fg-*      Fog: LINEAR, EXPONENTIAL, EXPONENTIAL2, visibilityRange 0, fogColor against the Background, lines,
                points, transparent and textured shapes, FogCoordinate, nested and local fog, ortho, Text
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_rea_b09_visual import (HEAD, BG, MAT, QUAD, SPHERE, CHECKER, cell, world, shape, ifs, NOHEAD, GRID, lit, UNLIT)

worlds = {}
SPEC = "Appearance { material Material { diffuseColor 0.8 0.5 0.3 specularColor 0.9 0.9 0.9 shininess 0.4 } }"
ROW = "Appearance { material Material { diffuseColor 0.9 0.9 0.9 specularColor 0.6 0.6 0.6 shininess 0.3 } }"
PLANE = ("Shape { appearance " + ROW + " geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -1 -1 0, 1 -1 0, 1 1 0, -1 1 0 ] } "
         "coordIndex [ 0 1 2 3 -1 ] normal Normal { vector [ 0 0 1 ] } } }")
CELLS = [
    shape("Sphere { radius 0.9 }", SPEC),
    shape("Box { size 1.4 1.4 1.4 }", SPEC),
    shape("Cone { height 1.6 bottomRadius 0.8 }", SPEC),
    shape("Cylinder { height 1.6 radius 0.7 }", SPEC),
    PLANE,
    f"Transform {{ rotation 0 1 0 0.8 children {PLANE} }}",
    f"Transform {{ rotation 1 0 0 -0.8 children {PLANE} }}",
    shape("Sphere { radius 0.9 }", "Appearance { }"),
    shape("Sphere { radius 0.9 }", "Appearance { material Material { diffuseColor 0 0 0 emissiveColor 0.2 0.7 0.4 } }"),
    shape("Sphere { radius 0.9 }", "Appearance { material Material { diffuseColor 0.2 0.6 0.9 transparency 0.5 specularColor 1 1 1 shininess 0.6 } }"),
    shape("Sphere { radius 0.9 }", f"Appearance {{ material Material {{ diffuseColor 0.9 0.9 0.9 }} texture ImageTexture {{ url \"{CHECKER}\" }} }}"),
    shape("Sphere { radius 0.9 }", UNLIT),
]
DIR = "DirectionalLight { direction 0.5 -0.6 -1 intensity 0.8 }"

# --- headlight --------------------------------------------------------------------------------
worlds["i92-hl-default.wrl"] = world(CELLS)
worlds["i92-hl-off.wrl"] = world(CELLS, nav=NOHEAD)
worlds["i92-hl-off-light.wrl"] = world(CELLS, nav=NOHEAD, extra=DIR)
worlds["i92-hl-on-light.wrl"] = world(CELLS, nav="NavigationInfo { headlight TRUE }", extra=DIR)
worlds["i92-hl-rotated-viewpoint.wrl"] = world(CELLS, vp="Viewpoint { position 6 4 10 orientation -0.45 0.85 0.27 0.55 }")
worlds["i92-hl-ortho.wrl"] = world(CELLS, vp="OrthoViewpoint { position 0 0 12 fieldOfView [ -6.5 -5 6.5 5 ] }")
worlds["i92-hl-two-navinfo.wrl"] = world(CELLS, nav="NavigationInfo { headlight FALSE }\nNavigationInfo { headlight TRUE }")
worlds["i92-hl-tilted-nav-type.wrl"] = world(CELLS, nav='NavigationInfo { type "NONE" headlight TRUE }',
                                             vp="Viewpoint { position 0 0 12 orientation 1 0 0 -0.35 }")

# --- lighting model -----------------------------------------------------------------------------
CREASE = "coord Coordinate { point [ -0.9 -0.9 0.5, 0.9 -0.9 0, 0.9 0.9 0.5, -0.9 0.9 0 ] } coordIndex [ 0 1 2 -1 0 2 3 -1 ]"
def ifsn(fields, appearance=ROW):
    return shape(f"IndexedFaceSet {{ solid FALSE {CREASE} {fields} }}", appearance)
LIGHT = lambda body: f"Group {{ children [ {DIR}\n{body} ] }}"
worlds["i92-lm-normals.wrl"] = world([LIGHT(c) for c in [
    shape("Sphere { radius 0.9 }", ROW),
    shape("Box { size 1.4 1.4 1.4 }", ROW),
    ifsn("creaseAngle 0"),
    ifsn("creaseAngle 3.14"),
    ifsn("creaseAngle 0.3"),
    ifsn("normalPerVertex TRUE normal Normal { vector [ -0.6 -0.6 0.5, 0.6 -0.6 0.5, 0.6 0.6 0.5, -0.6 0.6 0.5 ] } coordIndex [ 0 1 2 3 -1 ]"),
    ifsn("normalPerVertex FALSE normal Normal { vector [ 0 0.6 0.8, 0.6 0 0.8 ] }"),
    shape("Cone { height 1.6 bottomRadius 0.8 }", ROW),
    shape("Cylinder { height 1.6 radius 0.7 }", ROW),
    shape("Extrusion { crossSection [ 1 1, 1 -1, -1 -1, -1 1, 1 1 ] spine [ 0 -0.8 0, 0 0.8 0 ] scale [ 0.8 0.8, 0.4 0.4 ] creaseAngle 0 }", ROW),
    shape("ElevationGrid { xDimension 4 zDimension 4 xSpacing 0.6 zSpacing 0.6 height [ 0 0.3 0 0.3, 0.3 0 0.3 0, 0 0.3 0 0.3, 0.3 0 0.3 0 ] creaseAngle 0.6 solid FALSE }", ROW),
    shape("ElevationGrid { xDimension 4 zDimension 4 xSpacing 0.6 zSpacing 0.6 height [ 0 0.3 0 0.3, 0.3 0 0.3 0, 0 0.3 0 0.3, 0.3 0 0.3 0 ] creaseAngle 0 solid FALSE }", ROW),
]], nav=NOHEAD)
# solid TRUE / FALSE faces seen from behind, with and without a light on the back
BACKQUAD = lambda solid: ("IndexedFaceSet { solid " + solid + " coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] } coordIndex [ 3 2 1 0 -1 ] }")
worlds["i92-lm-sides.wrl"] = world([LIGHT(c) for c in [
    shape(BACKQUAD("TRUE"), ROW),
    shape(BACKQUAD("FALSE"), ROW),
    f"Group {{ children [ DirectionalLight {{ direction 0 0 1 }}\n{shape(BACKQUAD('FALSE'), ROW)} ] }}",
    f"Group {{ children [ DirectionalLight {{ direction 0 0 1 }}\n{shape(BACKQUAD('TRUE'), ROW)} ] }}",
    shape("Sphere { radius 0.9 solid FALSE }", ROW),
    shape("Box { size 1.4 1.4 1.4 solid FALSE }", ROW),
    "Transform { scale 1 1 -1 children " + shape("Sphere { radius 0.9 }", ROW) + " }",
    "Transform { scale -1 1 1 children " + shape("Cone { height 1.6 bottomRadius 0.8 }", ROW) + " }",
    shape("Sphere { radius 0.9 }", "Appearance { material TwoSidedMaterial { diffuseColor 0.8 0.4 0.2 backDiffuseColor 0.2 0.4 0.8 } }"),
    shape(BACKQUAD("FALSE"), "Appearance { material TwoSidedMaterial { diffuseColor 0.8 0.4 0.2 backDiffuseColor 0.2 0.4 0.8 separateBackColor TRUE } }"),
    shape(BACKQUAD("FALSE"), "Appearance { material TwoSidedMaterial { diffuseColor 0.8 0.4 0.2 backDiffuseColor 0.2 0.4 0.8 separateBackColor FALSE } }"),
    shape(BACKQUAD("TRUE"), "Appearance { material TwoSidedMaterial { diffuseColor 0.8 0.4 0.2 backDiffuseColor 0.2 0.4 0.8 separateBackColor TRUE } }"),
]], nav=NOHEAD, head="#X3D V4.0 utf8\nPROFILE Full\n")
def mcell(m, geo=SPHERE):
    return LIGHT(shape(geo, f"Appearance {{ material Material {{ {m} }} }}"))
worlds["i92-lm-material.wrl"] = world([
    mcell("diffuseColor 0.8 0.3 0.3"),
    mcell("diffuseColor 0.8 0.3 0.3 shininess 0 specularColor 1 1 1"),
    mcell("diffuseColor 0.8 0.3 0.3 shininess 1 specularColor 1 1 1"),
    mcell("diffuseColor 0.2 0.2 0.2 specularColor 1 0.2 0.2 shininess 0.5"),
    mcell("diffuseColor 0 0 0 emissiveColor 0.4 0.4 0.9"),
    mcell("diffuseColor 0.8 0.8 0.2 emissiveColor 0.3 0 0"),
    mcell("diffuseColor 0.5 0.5 0.5 ambientIntensity 1"),
    mcell("diffuseColor 0.5 0.5 0.5 ambientIntensity 0"),
    mcell("diffuseColor 1 1 1 transparency 0.3 specularColor 1 1 1 shininess 0.5"),
    mcell("diffuseColor 1 1 1 transparency 0.9"),
    mcell("diffuseColor 0.7 0.7 0.7 specularColor 0.5 0.5 0.5 shininess 0.2", "Box { size 1.4 1.4 1.4 }"),
    mcell("diffuseColor 0.7 0.7 0.7 specularColor 0.5 0.5 0.5 shininess 0.2", "Cylinder { height 1.6 radius 0.7 }"),
], nav=NOHEAD)
COLS = "coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] } coordIndex [ 0 1 2 3 -1 ] normal Normal { vector [ 0 0 1 ] }"
def colored(color, mat="Material { diffuseColor 0.4 0.4 0.4 specularColor 0.5 0.5 0.5 shininess 0.3 }", per="TRUE"):
    return LIGHT(shape(f"IndexedFaceSet {{ solid FALSE {COLS} colorPerVertex {per} {color} }}", f"Appearance {{ material {mat} }}"))
C4 = "color Color { color [ 1 0 0, 0 1 0, 0 0 1, 1 1 0 ] }"
C4A = "color ColorRGBA { color [ 1 0 0 1, 0 1 0 0.6, 0 0 1 0.3, 1 1 0 0.1 ] }"
worlds["i92-lm-color-material.wrl"] = world([
    colored(C4), colored(C4A), colored(C4, per="FALSE"),
    colored(C4, "Material { diffuseColor 0 0 0 emissiveColor 0.3 0.3 0.3 }"),
    colored(C4, "Material { diffuseColor 0.9 0.9 0.9 emissiveColor 0 0 0.3 transparency 0.5 }"),
    colored(C4A, "Material { diffuseColor 0.9 0.9 0.9 transparency 0.5 }"),
    colored(C4, "Material { }"),
    LIGHT(shape(f"IndexedFaceSet {{ solid FALSE {COLS} {C4} }}", UNLIT)),
    LIGHT(shape(f"IndexedFaceSet {{ solid FALSE {COLS} {C4} }}", "Appearance { }")),
    LIGHT(shape(f"IndexedFaceSet {{ solid FALSE {COLS} {C4} }}", f"Appearance {{ material Material {{ }} texture ImageTexture {{ url \"{CHECKER}\" }} }}")),
    colored(C4, "Material { diffuseColor 0.4 0.4 0.4 specularColor 1 1 1 shininess 0.9 }"),
    LIGHT(shape(f"IndexedFaceSet {{ solid FALSE {COLS} }}", "Appearance { material Material { diffuseColor 0.4 0.4 0.4 specularColor 1 1 1 shininess 0.9 } }")),
], nav=NOHEAD)
POINTGRID = lambda pl: lit(pl, GRID)
worlds["i92-lm-attenuation.wrl"] = world([
    POINTGRID(f"PointLight {{ location 0 0 0.8 radius 10 attenuation {a} }}") for a in
    ["1 0 0", "1 0.5 0", "1 0 0.5", "1 1 1", "0.5 0 0", "0 0 0", "2 0 0", "1 0.2 0.2"]
] + [
    POINTGRID("PointLight { location 0 0 0.8 radius 10 attenuation 1 0 0.5 intensity 1.5 }"),
    POINTGRID("PointLight { location 0 0 0.8 radius 1.5 attenuation 1 0 0.5 }"),
    lit("SpotLight { location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 0.4 cutOffAngle 0.8 attenuation 1 0.3 0.2 }", GRID),
    lit("SpotLight { location 0 0 1.5 direction 0 0 -1 radius 10 beamWidth 0.4 cutOffAngle 0.8 attenuation 1 0 0 }", GRID),
], nav=NOHEAD)

# --- transparency sorting -----------------------------------------------------------------------
def tq(z, color, transparency=0.5, size=1.3, x=0.0, y=0.0):
    return (f"Transform {{ translation {x} {y} {z} children Shape {{ appearance Appearance {{ material Material {{ diffuseColor {color} "
            f"emissiveColor {color} transparency {transparency} }} }} geometry Box {{ size {size} {size} 0.05 }} }} }}")
R, G, B, Y = "1 0 0", "0 1 0", "0 0 1", "1 1 0"
def stack(order, zs=(-0.6, 0, 0.6), colors=(R, G, B), t=0.5):
    items = [tq(zs[i], colors[i], t, x=0.25 * i - 0.25, y=0.15 * i - 0.15) for i in range(3)]
    return "\n".join(items[i] for i in order)
OPQ = lambda z, c="0.9 0.9 0.9", s=1.9: tq(z, c, 0, s)
worlds["i92-tr-order.wrl"] = world([
    stack((0, 1, 2)), stack((2, 1, 0)), stack((1, 0, 2)), stack((0, 2, 1)), stack((2, 0, 1)), stack((1, 2, 0)),
    stack((0, 1, 2), t=0.2), stack((2, 1, 0), t=0.8),
    stack((0, 1, 2), zs=(0, 0, 0)), stack((2, 1, 0), zs=(0, 0, 0)),
    stack((0, 1, 2), zs=(-0.02, 0, 0.02)), stack((2, 1, 0), zs=(-0.02, 0, 0.02)),
])
worlds["i92-tr-opaque.wrl"] = world([
    OPQ(-0.8) + "\n" + tq(0, R, 0.5),
    OPQ(0.8) + "\n" + tq(0, R, 0.5),
    tq(0, R, 0.5) + "\n" + OPQ(-0.8),
    tq(0, R, 0.5) + "\n" + OPQ(0.8),
    OPQ(0) + "\n" + tq(-0.6, R, 0.5) + "\n" + tq(0.6, B, 0.5),
    tq(-0.6, R, 0.5) + "\n" + OPQ(0) + "\n" + tq(0.6, B, 0.5),
    tq(0.6, B, 0.5) + "\n" + tq(-0.6, R, 0.5) + "\n" + OPQ(0),
    # an intersecting opaque and transparent box
    "Transform { rotation 0 1 0 0.7 children [ " + OPQ(0, s=1.4) + tq(0, G, 0.5, 1.8) + " ] }",
    shape("Sphere { radius 0.9 }", "Appearance { material Material { diffuseColor 0.9 0.9 0.2 } }") + "\n" + tq(0.3, B, 0.6, 1.6),
    tq(0.3, B, 0.6, 1.6) + "\n" + shape("Sphere { radius 0.9 }", "Appearance { material Material { diffuseColor 0.9 0.9 0.2 } }"),
    tq(0, R, 1, 1.8) + "\n" + OPQ(-0.5),
    tq(0, R, 0.999, 1.8) + "\n" + OPQ(-0.5),
])
def tball(z, color, t=0.5, r=0.7, x=0):
    return (f"Transform {{ translation {x} 0 {z} children Shape {{ appearance Appearance {{ material Material {{ diffuseColor {color} "
            f"transparency {t} }} }} geometry Sphere {{ radius {r} }} }} }}")
worlds["i92-tr-nested.wrl"] = world([
    "Transform { translation 0 0 -0.6 children [ " + tball(0, R) + " ] }\n" + tball(0.2, B, x=0.3),
    tball(0.2, B, x=0.3) + "\n" + "Transform { translation 0 0 -0.6 children [ " + tball(0, R) + " ] }",
    "Transform { scale 1 1 -1 children [ " + tball(0.7, R) + tball(-0.2, B, x=0.3) + " ] }",
    "Transform { rotation 0 1 0 1.5708 children [ " + tball(0.7, R) + tball(-0.2, B, x=0.3) + " ] }",
    "Group { children [ " + tball(-0.5, R) + tball(0.5, B, x=0.3) + " ] }",
    "Switch { whichChoice 0 children [ " + tball(-0.5, R) + tball(0.5, B, x=0.3) + " ] }",
    "Switch { whichChoice -1 children [ " + tball(-0.5, R) + " ] }" + tball(0, G),
    "Billboard { axisOfRotation 0 1 0 children [ " + tq(-0.3, R, 0.5) + tq(0.3, B, 0.5) + " ] }",
    "Collision { children [ " + tball(-0.5, R) + tball(0.5, B, x=0.3) + " ] }",
    "Anchor { children [ " + tball(-0.5, R) + tball(0.5, B, x=0.3) + " ] }",
    "LOD { range [ 20 ] children [ " + tball(-0.5, R) + tball(0.5, B, x=0.3) + " ] }",
    tball(-0.5, R, 0.5, 0.7, -0.5) + tball(0.5, G, 0.5, 0.7, 0) + tball(0, B, 0.5, 0.7, 0.5),
])
TEXA = ("PixelTexture { image 2 2 4 0xff0000ff 0xff000040 0x00ff0080 0x0000ffff }")
worlds["i92-tr-textured.wrl"] = world([
    shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {TEXA} }}"),
    shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 transparency 0.5 }} texture {TEXA} }}"),
    OPQ(-0.8) + "\n" + shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {TEXA} }}"),
    shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ texture {TEXA} }}"),
    shape("Sphere { radius 0.9 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {TEXA} }}"),
    OPQ(0.8) + "\n" + shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {TEXA} }}"),
    tq(-0.5, R, 0.5) + "\n" + shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {TEXA} }}"),
    shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {TEXA} }}") + "\n" + tq(0.5, R, 0.5),
    shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture ImageTexture {{ url \"{CHECKER}\" }} }}"),
    shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 transparency 0.5 }} texture ImageTexture {{ url \"{CHECKER}\" }} }}"),
    shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture PixelTexture {{ image 2 2 4 0xff0000ff 0xff000040 0x00ff0080 0x0000ffff textureProperties TextureProperties {{ boundaryModeS \"CLAMP_TO_EDGE\" }} }} }}"),
    shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {TEXA} }}") + "\n" + OPQ(-0.8),
])
POINTS = lambda c, t: shape("PointSet { coord Coordinate { point [ " + ", ".join(f'{-0.8 + 0.4 * i:.1f} {(-0.8 + 0.4 * j):.1f} 0' for i in range(5) for j in range(5)) + " ] } }",
                            f"Appearance {{ material Material {{ emissiveColor {c} transparency {t} }} }}")
LINESET = lambda c, t: shape("IndexedLineSet { coord Coordinate { point [ -0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0, 0.9 -0.9 0 ] } coordIndex [ 0 1 -1 2 3 -1 ] }",
                             f"Appearance {{ material Material {{ emissiveColor {c} transparency {t} }} }}")
worlds["i92-tr-points-lines.wrl"] = world([
    POINTS(R, 0.5) + "\n" + tq(0.3, B, 0.5, 1.6),
    tq(-0.3, B, 0.5, 1.6) + "\n" + POINTS(R, 0.5),
    LINESET(R, 0.5) + "\n" + tq(0.3, B, 0.5, 1.6),
    tq(-0.3, B, 0.5, 1.6) + "\n" + LINESET(R, 0.5),
    POINTS(R, 0) + "\n" + tq(0.3, B, 0.5, 1.6),
    LINESET(R, 0) + "\n" + tq(0.3, B, 0.5, 1.6),
    tq(-0.3, B, 0.5, 1.6) + "\n" + POINTS(R, 0.8),
    OPQ(-0.4) + "\n" + LINESET(R, 0.5),
    OPQ(0.4) + "\n" + LINESET(R, 0.5),
    "Transform { rotation 0 1 0 0.7 children [ " + LINESET(R, 0.5) + tq(0, B, 0.5, 1.6) + " ] }",
    "Transform { rotation 1 0 0 0.7 children [ " + POINTS(R, 0.5) + tq(0, B, 0.5, 1.6) + " ] }",
    tq(0, B, 0.5, 1.6) + "\n" + "Transform { translation 0 0 0.4 children [ " + POINTS(R, 0.5) + " ] }",
])
worlds["i92-tr-ortho.wrl"] = world([
    stack((0, 1, 2)), stack((2, 1, 0)), stack((1, 0, 2)),
    OPQ(-0.8) + "\n" + tq(0, R, 0.5), tq(0, R, 0.5) + "\n" + OPQ(0.8),
    stack((0, 1, 2), t=0.8), stack((2, 1, 0), zs=(0, 0, 0)),
    "Transform { rotation 0 1 0 0.7 children [ " + stack((0, 1, 2)) + " ] }",
    tball(-0.5, R) + tball(0.5, B, x=0.3), tball(0.5, B, x=0.3) + tball(-0.5, R),
    tball(-0.5, R, 0.5, 0.7, -0.5) + tball(0.5, G, 0.5, 0.7, 0) + tball(0, B, 0.5, 0.7, 0.5),
    tq(0, R, 0.5) + tq(0, G, 0.5, 1.0),
], vp="OrthoViewpoint { position 0 0 12 fieldOfView [ -6.5 -5 6.5 5 ] }")
worlds["i92-tr-many.wrl"] = world([
    "\n".join(tball(-0.9 + 0.2 * k, f"{(k % 3) / 2} {((k + 1) % 3) / 2} {((k + 2) % 3) / 2}", 0.6, 0.55, -0.4 + 0.1 * k) for k in range(10)),
    "\n".join(tball(0.9 - 0.2 * k, f"{(k % 3) / 2} {((k + 1) % 3) / 2} {((k + 2) % 3) / 2}", 0.6, 0.55, -0.4 + 0.1 * k) for k in range(10)),
    "\n".join(tq(-0.9 + 0.2 * k, f"{(k % 3) / 2} {((k + 1) % 3) / 2} {((k + 2) % 3) / 2}", 0.7, 1.6, -0.4 + 0.1 * k, 0.05 * k) for k in range(10)),
    "\n".join(tq(0.9 - 0.2 * k, f"{(k % 3) / 2} {((k + 1) % 3) / 2} {((k + 2) % 3) / 2}", 0.7, 1.6, -0.4 + 0.1 * k, 0.05 * k) for k in range(10)),
])

# --- fog ------------------------------------------------------------------------------------------
FOGBG = "Background { skyColor 0.5 0.5 0.6 }"
def corridor(n=6, geo="Sphere { radius 0.45 }", app="Appearance { material Material { diffuseColor 0.9 0.2 0.2 } }", dz=-2.5, y=0.0):
    return "\n".join(f"Transform {{ translation {-1.5 + 0.6 * k:.2f} {y} {dz * k} children Shape {{ appearance {app} geometry {geo} }} }}" for k in range(n))
def fogworld(fog, body=None, vp="Viewpoint { position 0 0 4 }", bg=FOGBG, nav="NavigationInfo { headlight FALSE }\nDirectionalLight { direction 0 0 -1 }", head=HEAD):
    return head + vp + "\n" + nav + "\n" + bg + "\n" + fog + "\n" + (body or corridor()) + "\n"
for name, fog in {
    "linear": "Fog { fogType \"LINEAR\" color 0.8 0.8 0.9 visibilityRange 12 }",
    "linear-short": "Fog { fogType \"LINEAR\" color 0.8 0.8 0.9 visibilityRange 5 }",
    "exp": "Fog { fogType \"EXPONENTIAL\" color 0.8 0.8 0.9 visibilityRange 12 }",
    "exp2": "Fog { fogType \"EXPONENTIAL2\" color 0.8 0.8 0.9 visibilityRange 12 }",
    "range0": "Fog { fogType \"LINEAR\" color 1 0 0 visibilityRange 0 }",
    "color-bg": "Fog { fogType \"LINEAR\" color 0.5 0.5 0.6 visibilityRange 12 }",
    "red": "Fog { fogType \"LINEAR\" color 1 0 0 visibilityRange 12 }",
    "range-neg": "Fog { fogType \"LINEAR\" color 0.8 0.8 0.9 visibilityRange -4 }",
    "unknown-type": "Fog { fogType \"BOGUS\" color 0.8 0.8 0.9 visibilityRange 12 }",
}.items():
    worlds[f"i92-fg-{name}.wrl"] = fogworld(fog)
LINEAR = "Fog { fogType \"LINEAR\" color 0.8 0.8 0.9 visibilityRange 12 }"
worlds["i92-fg-two-fogs.wrl"] = fogworld(LINEAR + "\nFog { fogType \"EXPONENTIAL\" color 1 0 0 visibilityRange 6 }")
worlds["i92-fg-unbound.wrl"] = fogworld("Fog { }")
worlds["i92-fg-in-transform.wrl"] = fogworld("Transform { translation 5 0 0 scale 3 3 3 children [ " + LINEAR + " ] }")
worlds["i92-fg-unlit.wrl"] = fogworld(LINEAR, corridor(app="Appearance { }"))
worlds["i92-fg-emissive.wrl"] = fogworld(LINEAR, corridor(app="Appearance { material Material { diffuseColor 0 0 0 emissiveColor 0.9 0.2 0.2 } }"))
worlds["i92-fg-textured.wrl"] = fogworld(LINEAR, corridor(geo="Box { size 0.8 0.8 0.8 }", app=f"Appearance {{ material Material {{ }} texture ImageTexture {{ url \"{CHECKER}\" }} }}"))
worlds["i92-fg-transparent.wrl"] = fogworld(LINEAR, corridor(app="Appearance { material Material { diffuseColor 0.9 0.2 0.2 transparency 0.5 } }"))
worlds["i92-fg-specular.wrl"] = fogworld(LINEAR, corridor(app="Appearance { material Material { diffuseColor 0.9 0.2 0.2 specularColor 1 1 1 shininess 0.5 } }"))
worlds["i92-fg-lines.wrl"] = fogworld(LINEAR, corridor(geo="IndexedLineSet { coord Coordinate { point [ -0.4 -0.4 0, 0.4 0.4 0, -0.4 0.4 0, 0.4 -0.4 0 ] } coordIndex [ 0 1 -1 2 3 -1 ] }",
                                                      app="Appearance { material Material { emissiveColor 1 1 0 } }"))
worlds["i92-fg-points.wrl"] = fogworld(LINEAR, corridor(geo="PointSet { coord Coordinate { point [ -0.4 -0.4 0, 0.4 0.4 0, -0.4 0.4 0, 0.4 -0.4 0 ] } }",
                                                       app="Appearance { material Material { emissiveColor 1 1 0 } }"))
worlds["i92-fg-ortho.wrl"] = fogworld(LINEAR, vp="OrthoViewpoint { position 0 0 4 fieldOfView [ -3 -2 3 2 ] }")
worlds["i92-fg-rotated-view.wrl"] = fogworld(LINEAR, vp="Viewpoint { position 3 2 5 orientation -0.4 0.9 0.2 0.6 }")
worlds["i92-fg-scaled-view.wrl"] = fogworld(LINEAR, "Transform { scale 2 2 2 children [ " + corridor() + " ] }")
worlds["i92-fg-text.wrl"] = fogworld(LINEAR, head="#X3D V4.0 utf8\nPROFILE Full\n", body="\n".join(
    f"Transform {{ translation -1.5 {0.6 - 0.0} {-2.5 * k} children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.9 0.2 0.2 }} }} geometry Text {{ string \"FOG{k}\" fontStyle FontStyle {{ size 0.6 }} }} }} }}" for k in range(5)))
FC = "FogCoordinate { depth [ 0, 0.5, 1, 0.2 ] }"
worlds["i92-fg-coordinate.wrl"] = fogworld(
    "Fog { fogType \"LINEAR\" color 0.8 0.8 0.9 visibilityRange 12 }",
    "\n".join(f"Transform {{ translation {-1.2 + 1.2 * k} 0 0 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.9 0.2 0.2 }} }} geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -0.5 -0.5 0, 0.5 -0.5 0, 0.5 0.5 0, -0.5 0.5 0 ] }} coordIndex [ 0 1 2 3 -1 ] {'fogCoord ' + FC if k else ''} }} }} }}" for k in range(3)),
    head="#X3D V4.0 utf8\nPROFILE Full\n")
worlds["i92-fg-localfog.wrl"] = fogworld(LINEAR, "\n".join(
    f"Transform {{ translation {-1.2 + 1.2 * k} 0 {-3 * k} children [ LocalFog {{ fogType \"{t}\" color 1 0.3 0.3 visibilityRange 8 enabled {e} }} Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.2 0.8 0.3 }} }} geometry Sphere {{ radius 0.5 }} }} ] }}"
    for k, (t, e) in enumerate([("LINEAR", "TRUE"), ("EXPONENTIAL", "TRUE"), ("EXPONENTIAL2", "FALSE")])), head="#X3D V4.0 utf8\nPROFILE Full\n")
worlds["i92-fg-nofog-headlight.wrl"] = fogworld(LINEAR, nav="NavigationInfo { headlight TRUE }")

if __name__ == "__main__":
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for name, text in sorted(worlds.items()):
        with open(os.path.join(out, name), "w") as f:
            f.write(text)
    print(f"{len(worlds)} worlds in {out}")
