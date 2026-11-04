#!/usr/bin/env python3
"""Order-independent transparency worlds (sub:render.transparency-sorting): the `orderIndependentTransparency` option.

  python3 -I oracle/tools/gen_rea_oit_render_items.py oracle/synthetic-rea-oit-render-items

Writes oit-*.wrl next to nothing else: the directory is new. Each world is a grid of cells (the helpers of
gen_rea_b09_visual.py and the transparent-box helpers of gen_rea_i92_render_items.py) that vary one thing side by side.
Newly authored; no historical content. The worlds are recorded with the pinned X_ITE 16.5.0 with the browser option on
(oracle/tools/rea_i92_render_items.py shots --oit).

  oit-order       listing order of three overlapping boxes (the weighted blend does not depend on it), opacity, equal depths
  oit-opaque      opaque shapes before, between and after the transparent ones, intersecting an opaque shape
  oit-intersect   depth-intersecting transparent shapes (crossing boxes, overlapping spheres, a pierced sphere)
  oit-textured    texture alpha (PixelTexture, ImageTexture) with and without a Material transparency
  oit-physical    PhysicalMaterial transparency, with a texture, emissive, metallic, two layers, an opaque neighbor
  oit-alpha       Material transparency from 0.02 to 0.98 over an opaque plane (the weight function's alpha term)
  oit-many        ten, sixteen and twenty layers (the accumulation buffers; a sum stays below the RGBA16F maximum)
  oit-primitives  transparent points, lines, a two-sided face, ColorRGBA, a Fog, a unlit shape
  oit-ortho       three layers under an OrthoViewpoint
  oit-layers      two Layers of a LayerSet, each with transparent shapes
"""
import os, sys
sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from gen_rea_b09_visual import BG, CHECKER, cell, world, shape, UNLIT
from gen_rea_i92_render_items import tq, tball, stack, OPQ, POINTS, LINESET, R, G, B, Y, TEXA

HEAD4 = "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT Shape:4\nCOMPONENT Lighting:3\nCOMPONENT Layering:1\n"
PLANE = "Box { size 1.6 1.6 0.05 }"
DEEP = "Box { size 1.6 1.6 0.8 }"
worlds = {}

def rot(axis_angle, *children):
    return f"Transform {{ rotation {axis_angle} children [ " + "\n".join(children) + " ] }"

def pbr(z, color, t=0.5, size="1.3 1.3 0.05", x=0.0, y=0.0, extra="metallic 0 roughness 0.5", geo=None):
    g = geo or f"Box {{ size {size} }}"
    return (f"Transform {{ translation {x} {y} {z} children Shape {{ appearance Appearance {{ material PhysicalMaterial {{ "
            f"baseColor {color} transparency {t} {extra} }} }} geometry {g} }} }}")

def alpha_plane(t, color="1 1 0"):
    return shape("Box { size 1.6 1.6 0.05 }", f"Appearance {{ material Material {{ diffuseColor {color} emissiveColor {color} transparency {t} }} }}")

# --- listing order, opacity, equal depths ------------------------------------------------------------
worlds["oit-order.wrl"] = world([
    stack((0, 1, 2)), stack((2, 1, 0)), stack((1, 0, 2)), stack((0, 2, 1)), stack((2, 0, 1)), stack((1, 2, 0)),
    stack((0, 1, 2), t=0.2), stack((2, 1, 0), t=0.8),
    stack((0, 1, 2), zs=(0, 0, 0)), stack((2, 1, 0), zs=(0, 0, 0)),
    stack((0, 1, 2), zs=(-0.02, 0, 0.02)), stack((2, 1, 0), zs=(-0.02, 0, 0.02)),
])

# --- opaque shapes in between ------------------------------------------------------------------------
SPH = lambda: shape("Sphere { radius 0.9 }", "Appearance { material Material { diffuseColor 0.9 0.9 0.2 } }")
worlds["oit-opaque.wrl"] = world([
    OPQ(-0.8) + "\n" + tq(0, R, 0.5),
    OPQ(0.8) + "\n" + tq(0, R, 0.5),
    tq(0, R, 0.5) + "\n" + OPQ(-0.8),
    tq(0, R, 0.5) + "\n" + OPQ(0.8),
    OPQ(0) + "\n" + tq(-0.6, R, 0.5) + "\n" + tq(0.6, B, 0.5),
    tq(-0.6, R, 0.5) + "\n" + OPQ(0) + "\n" + tq(0.6, B, 0.5),
    tq(0.6, B, 0.5) + "\n" + tq(-0.6, R, 0.5) + "\n" + OPQ(0),
    # a transparent plane crossing an opaque cube (no coplanar faces, which would z-fight)
    rot("0 1 0 0.7", shape("Box { size 1.4 1.4 1.4 }", "Appearance { material Material { diffuseColor 0.9 0.9 0.9 } }")) + "\n" + rot("0 1 0 1.3", tq(0, G, 0.5, 1.8)),
    SPH() + "\n" + tq(0.3, B, 0.6, 1.6),
    tq(0.3, B, 0.6, 1.6) + "\n" + SPH(),
    tq(0, R, 1, 1.8) + "\n" + OPQ(-0.5),
    tq(0, R, 0.999, 1.8) + "\n" + OPQ(-0.5),
])

# --- depth-intersecting shapes -------------------------------------------------------------------------
worlds["oit-intersect.wrl"] = world([
    rot("0 1 0 1.5708", tq(0, R, 0.5, 1.6)) + "\n" + tq(0, B, 0.5, 1.6),
    tq(0, B, 0.5, 1.6) + "\n" + rot("0 1 0 1.5708", tq(0, R, 0.5, 1.6)),
    tball(-0.3, R, 0.5, 0.8, -0.25) + tball(0.3, B, 0.5, 0.8, 0.25),
    tball(0.3, B, 0.5, 0.8, 0.25) + tball(-0.3, R, 0.5, 0.8, -0.25),
    rot("0 1 0 0.7", tq(0, R, 0.5, 1.6)) + rot("1 0 0 0.7", tq(0, G, 0.5, 1.6)) + rot("0 0 1 0.7", tq(0, B, 0.5, 1.6)),
    tball(0, B, 0.4, 0.9) + "\n" + rot("0 1 0 0.5", tq(0, R, 0.5, 2.0)),
    rot("1 0 0 0.6", OPQ(0, "0.2 0.5 0.9", 2.0)) + "\n" + tball(0, R, 0.5, 0.8),
    tball(0, R, 0.5, 0.8) + "\n" + rot("1 0 0 0.6", OPQ(0, "0.2 0.5 0.9", 2.0)),
    shape(DEEP, "Appearance { material Material { diffuseColor 0.9 0.2 0.2 transparency 0.5 } }") + "\n"
        + rot("0 1 0 0.8", shape(DEEP, "Appearance { material Material { diffuseColor 0.2 0.2 0.9 transparency 0.5 } }")),
    shape("Cone { height 1.8 bottomRadius 0.8 }", "Appearance { material Material { diffuseColor 0.9 0.5 0.2 transparency 0.4 } }") + "\n"
        + rot("0 0 1 1.5708", shape("Cylinder { height 1.8 radius 0.4 }", "Appearance { material Material { diffuseColor 0.2 0.7 0.5 transparency 0.4 } }")),
    tball(-0.4, R, 0.6, 0.7, -0.5) + tball(0.4, G, 0.6, 0.7, 0) + tball(0, B, 0.6, 0.7, 0.5),
    tball(0.4, G, 0.6, 0.7, 0) + tball(-0.4, R, 0.6, 0.7, -0.5) + tball(0, B, 0.6, 0.7, 0.5),
])

# --- textures -------------------------------------------------------------------------------------------
BOXT = lambda app: shape(PLANE, app)
TEXM = lambda tex, t=0: f"Appearance {{ material Material {{ diffuseColor 1 1 1 transparency {t} }} texture {tex} }}"
CHK = f"ImageTexture {{ url \"{CHECKER}\" }}"
worlds["oit-textured.wrl"] = world([
    BOXT(TEXM(TEXA)),
    BOXT(TEXM(TEXA, 0.5)),
    OPQ(-0.8) + "\n" + BOXT(TEXM(TEXA)),
    BOXT("Appearance { texture " + TEXA + " }"),
    shape("Sphere { radius 0.9 }", TEXM(TEXA)),
    OPQ(0.8) + "\n" + BOXT(TEXM(TEXA)),
    tq(-0.5, R, 0.5) + "\n" + BOXT(TEXM(TEXA)),
    BOXT(TEXM(TEXA)) + "\n" + tq(0.5, R, 0.5),
    BOXT(TEXM(CHK)),
    BOXT(TEXM(CHK, 0.5)),
    rot("0 1 0 0.8", BOXT(TEXM(TEXA))) + "\n" + BOXT(TEXM(CHK, 0.3)),
    BOXT(TEXM(TEXA)) + "\n" + OPQ(-0.8),
])

# --- PhysicalMaterial -----------------------------------------------------------------------------------
LIGHT = "DirectionalLight { direction -0.4 -0.6 -1 intensity 1.2 }"
SPH_PBR = lambda t, extra="metallic 0 roughness 0.5", color="0.8 0.3 0.2": pbr(0, color, t, extra=extra, geo="Sphere { radius 0.9 }")
worlds["oit-physical.wrl"] = world([
    SPH_PBR(0.5),
    SPH_PBR(0.2),
    SPH_PBR(0.8),
    SPH_PBR(0.5, "metallic 1 roughness 0.2"),
    SPH_PBR(0.5, "metallic 0.5 roughness 0.8", "0.2 0.6 0.9"),
    SPH_PBR(0.5, "metallic 0 roughness 0.5 emissiveColor 0.5 0.3 0.0"),
    pbr(-0.6, "0.9 0.2 0.2", 0.5, "1.5 1.5 0.05") + "\n" + pbr(0.6, "0.2 0.2 0.9", 0.5, "1.5 1.5 0.05"),
    pbr(0.6, "0.2 0.2 0.9", 0.5, "1.5 1.5 0.05") + "\n" + pbr(-0.6, "0.9 0.2 0.2", 0.5, "1.5 1.5 0.05"),
    OPQ(-0.8) + "\n" + pbr(0, "0.9 0.2 0.2", 0.5, "1.6 1.6 0.05"),
    pbr(0, "0.9 0.2 0.2", 0.5, "1.6 1.6 0.05") + "\n" + OPQ(0.8),
    shape(PLANE, f"Appearance {{ material PhysicalMaterial {{ baseColor 1 1 1 transparency 0.4 metallic 0 roughness 0.6 baseTexture {CHK} }} }}"),
    rot("0 1 0 0.8", pbr(0, "0.9 0.2 0.2", 0.5, "1.6 1.6 0.05")) + "\n" + pbr(0, "0.2 0.2 0.9", 0.5, "1.6 1.6 0.05"),
], head=HEAD4, extra=LIGHT)

# --- alpha range ----------------------------------------------------------------------------------------
BACKDROP = "Transform { translation 0 0 -1 children " + shape("Box { size 2.0 2.0 0.05 }", "Appearance { material Material { diffuseColor 0.1 0.6 0.9 emissiveColor 0.1 0.6 0.9 } }") + " }"
worlds["oit-alpha.wrl"] = world([
    BACKDROP + "\n" + alpha_plane(t) for t in (0.02, 0.05, 0.1, 0.2, 0.35, 0.5, 0.65, 0.8, 0.9, 0.95, 0.98, 0.999)
])

# --- many layers ----------------------------------------------------------------------------------------
def colour(k):
    return f"{(k % 3) / 2} {((k + 1) % 3) / 2} {((k + 2) % 3) / 2}"
worlds["oit-many.wrl"] = world([
    "\n".join(tball(-0.9 + 0.2 * k, colour(k), 0.6, 0.55, -0.4 + 0.1 * k) for k in range(10)),
    "\n".join(tball(0.9 - 0.2 * k, colour(k), 0.6, 0.55, -0.4 + 0.1 * k) for k in range(10)),
    "\n".join(tq(-0.9 + 0.2 * k, colour(k), 0.7, 1.6, -0.4 + 0.1 * k, 0.05 * k) for k in range(10)),
    "\n".join(tq(0.9 - 0.2 * k, colour(k), 0.7, 1.6, -0.4 + 0.1 * k, 0.05 * k) for k in range(10)),
    "\n".join(tq(-0.95 + 0.1 * k, colour(k), 0.8, 1.6, 0, 0) for k in range(20)),
    "\n".join(tq(0.95 - 0.1 * k, colour(k), 0.8, 1.6, 0, 0) for k in range(20)),
    "\n".join(tq(-0.95 + 0.1 * k, colour(k), 0.5, 1.6, 0, 0) for k in range(16)),
    "\n".join(tq(0.55 - 0.1 * k, colour(k), 0.5, 1.6, 0, 0) for k in range(16)),
])

# --- other primitives and states ---------------------------------------------------------------------------
TWOSIDED = lambda t: shape("IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0.4, 0.9 0.9 0, -0.9 0.9 -0.4 ] } coordIndex [ 0 1 2 3 -1 ] }",
                           f"Appearance {{ material Material {{ diffuseColor 0.9 0.7 0.2 transparency {t} }} }}")
RGBA = shape("IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] } coordIndex [ 0 1 2 3 -1 ] "
             "color ColorRGBA { color [ 1 0 0 1, 0 1 0 0.6, 0 0 1 0.3, 1 1 0 0.1 ] } }", "Appearance { material Material { diffuseColor 1 1 1 } }")
RGBA_T = shape("IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] } coordIndex [ 0 1 2 3 -1 ] "
               "color ColorRGBA { color [ 1 0 0 1, 0 1 0 0.6, 0 0 1 0.3, 1 1 0 0.1 ] } }", "Appearance { material Material { diffuseColor 1 1 1 transparency 0.4 } }")
worlds["oit-primitives.wrl"] = world([
    POINTS(R, 0.5) + "\n" + tq(0.3, B, 0.5, 1.6),
    tq(-0.3, B, 0.5, 1.6) + "\n" + POINTS(R, 0.5),
    LINESET(R, 0.5) + "\n" + tq(0.3, B, 0.5, 1.6),
    tq(-0.3, B, 0.5, 1.6) + "\n" + LINESET(R, 0.5),
    OPQ(-0.4) + "\n" + LINESET(R, 0.5),
    TWOSIDED(0.4),
    TWOSIDED(0.4) + "\n" + tq(0, B, 0.5, 1.6),
    shape("Sphere { radius 0.9 solid FALSE }", "Appearance { material Material { diffuseColor 0.9 0.4 0.2 transparency 0.5 } }"),
    RGBA,
    RGBA_T,
    shape("Sphere { radius 0.9 }", "Appearance { material Material { diffuseColor 0 0 0 emissiveColor 0.9 0.2 0.2 transparency 0.5 } }") + "\n" + tq(0.5, B, 0.5, 1.6),
    shape("Sphere { radius 0.9 }", "Appearance { material Material { diffuseColor 0.9 0.9 0.9 specularColor 1 1 1 shininess 0.6 transparency 0.5 } }"),
], extra="DirectionalLight { direction 0.5 -0.6 -1 intensity 0.8 }")
FOGGED = "Fog { fogType \"LINEAR\" color 0.8 0.8 0.9 visibilityRange 40 }"
worlds["oit-fog.wrl"] = world([
    tball(-0.5, R, 0.5, 0.7, -0.5) + tball(0.5, B, 0.5, 0.7, 0),
    tq(0, R, 0.5) + "\n" + tq(0.8, B, 0.5),
], extra=FOGGED)

# --- ortho -------------------------------------------------------------------------------------------------
worlds["oit-ortho.wrl"] = world([
    stack((0, 1, 2)), stack((2, 1, 0)), stack((1, 0, 2)),
    OPQ(-0.8) + "\n" + tq(0, R, 0.5), tq(0, R, 0.5) + "\n" + OPQ(0.8),
    stack((0, 1, 2), t=0.8), stack((2, 1, 0), zs=(0, 0, 0)),
    rot("0 1 0 0.7", stack((0, 1, 2))),
    tball(-0.5, R) + tball(0.5, B, x=0.3), tball(0.5, B, x=0.3) + tball(-0.5, R),
    tball(-0.5, R, 0.5, 0.7, -0.5) + tball(0.5, G, 0.5, 0.7, 0) + tball(0, B, 0.5, 0.7, 0.5),
    tq(0, R, 0.5) + tq(0, G, 0.5, 1.0),
], vp="OrthoViewpoint { position 0 0 12 fieldOfView [ -6.5 -5 6.5 5 ] }")

# --- layers ------------------------------------------------------------------------------------------------
LAYER = lambda body: f"Layer {{ children [\n{body}\n] }}"
worlds["oit-layers.wrl"] = (HEAD4 + "Viewpoint { position 0 0 12 }\n" + BG + "\n" + "LayerSet { activeLayer 0 order [ 0 1 2 ] layers [\n"
    + LAYER("Viewpoint { position 0 0 12 }\n" + "\n".join(cell(i, c) for i, c in enumerate([
        stack((0, 1, 2)), stack((2, 1, 0)), OPQ(-0.8) + "\n" + tq(0, R, 0.5), tball(-0.5, R) + tball(0.5, B, x=0.3)])))
    + "\n" + LAYER("Viewpoint { position 0 0 12 }\n" + "\n".join(cell(i + 4, c) for i, c in enumerate([
        tq(0, R, 0.5) + "\n" + OPQ(0.8), tball(0.5, B, x=0.3) + tball(-0.5, R), stack((0, 1, 2), t=0.8),
        tball(-0.5, R, 0.5, 0.7, -0.5) + tball(0.5, G, 0.5, 0.7, 0) + tball(0, B, 0.5, 0.7, 0.5)])))
    + " ] }\n")

if __name__ == "__main__":
    out = sys.argv[1]
    os.makedirs(out, exist_ok=True)
    for name, text in sorted(worlds.items()):
        with open(os.path.join(out, name), "w") as f:
            f.write(text)
    print(f"{len(worlds)} worlds in {out}")
