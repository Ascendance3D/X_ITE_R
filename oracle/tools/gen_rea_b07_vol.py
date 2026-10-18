#!/usr/bin/env python3
"""REA batch 07 (VolumeRendering) synthetic worlds.

  python3 -I oracle/tools/gen_rea_b07_vol.py

Writes oracle/synthetic-rea-b07-vol/*.wrl (static pictures, one per case), the stepped-case worlds
and their event scripts (*.json), all from voxels computed here (clean-room: PixelTexture3D only).
The pinned X_ITE draws the baselines (oracle/tools/rea_b07_vol.py); nothing here is a reference.
"""
import json, math, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out = os.path.join(root, "oracle", "synthetic-rea-b07-vol", "stepped")
out_static = os.path.join(root, "oracle", "synthetic-rea-b07-vol", "static")
os.makedirs(out, exist_ok=True)
os.makedirs(out_static, exist_ok=True)

N = 16  # voxels per axis (X3DTexture3DNode: below 16 the texture is NEAREST, from 16 linear)


# --- voxel data ---------------------------------------------------------------------------------

def density(x, y, z):
    """Two blobs and a shell, x y z in [-1, 1]; asymmetric so orientation errors show."""
    d1 = math.dist((x, y, z), (-0.2, 0.1, 0.0))
    d2 = math.dist((x, y, z), (0.45, 0.3, 0.4))
    v = max(0.0, 1.0 - d1 / 0.9) + 0.6 * max(0.0, 1.0 - d2 / 0.5)
    shell = max(0.0, 1.0 - abs(math.dist((x, y, z), (0, 0, 0)) - 0.8) / 0.12)
    return min(1.0, v + 0.35 * shell)


def grid(f):
    for k in range(N):
        for j in range(N):
            for i in range(N):
                yield f(2.0 * (i + 0.5) / N - 1.0, 2.0 * (j + 0.5) / N - 1.0, 2.0 * (k + 0.5) / N - 1.0)


def vox_lum():
    return [round(255 * density(*p)) for p in grid(lambda x, y, z: (x, y, z))]


def vox_lum2():
    # luminance + alpha (components 2): alpha shrinks toward +x
    return [(round(255 * density(x, y, z)) << 8) | round(255 * (0.3 + 0.7 * (1 - (x + 1) / 2))) for x, y, z in grid(lambda x, y, z: (x, y, z))]


def vox_rgb():
    return [(round(255 * density(x, y, z) * (x + 1) / 2) << 16) | (round(255 * density(x, y, z) * (y + 1) / 2) << 8) | round(255 * density(x, y, z) * (z + 1) / 2) for x, y, z in grid(lambda x, y, z: (x, y, z))]


def vox_rgba():
    return [(round(255 * density(x, y, z) * (x + 1) / 2) << 24) | (round(255 * density(x, y, z) * (y + 1) / 2) << 16) | (round(255 * density(x, y, z) * (z + 1) / 2) << 8) | round(255 * min(1.0, density(x, y, z) * 1.5)) for x, y, z in grid(lambda x, y, z: (x, y, z))]


def vox_blend():
    # the second volume of BlendedVolumeStyle: a bar along y and a ring
    def f(x, y, z):
        bar = max(0.0, 1.0 - math.hypot(x - 0.1, z + 0.1) / 0.45) if abs(y) < 0.9 else 0.0
        ring = max(0.0, 1.0 - abs(math.hypot(x, y) - 0.6) / 0.15) if abs(z) < 0.35 else 0.0
        return round(255 * min(1.0, bar + ring))
    return list(grid(f))


def vox_normals():
    # outward normal of the density (-gradient), magnitude by gradient strength: RGB = n * 0.5 + 0.5
    e = 2.0 / N
    out = []
    for x, y, z in grid(lambda x, y, z: (x, y, z)):
        g = [(density(x + e, y, z) - density(x - e, y, z)), (density(x, y + e, z) - density(x, y - e, z)), (density(x, y, z + e) - density(x, y, z - e))]
        n = [-v for v in g]
        s = min(1.0, math.sqrt(sum(v * v for v in n)) * 2.0)
        l = math.sqrt(sum(v * v for v in n)) or 1.0
        c = [round(255 * (0.5 + 0.5 * s * v / l)) for v in n]
        out.append((c[0] << 16) | (c[1] << 8) | c[2])
    return out


def vox_segments():
    # segment identifiers (red channel of a luminance texture): 0 for x < -0.15, 1 for the rest, 2 for z > 0.35
    def f(x, y, z):
        if z > 0.35:
            return 2
        return 0 if x < -0.15 else 1
    return list(grid(f))


def img(comp, data):
    return f"image [ {comp} {N} {N} {N} " + " ".join(str(v) for v in data) + " ]"


DATA = {
    "lum": (1, vox_lum), "lum2": (2, vox_lum2), "rgb": (3, vox_rgb), "rgba": (4, vox_rgba), "blend": (1, vox_blend),
    "normals": (3, vox_normals), "segments": (1, vox_segments),
}
_cache = {}


class World:
    def __init__(self, head=""):
        self.defs = set()
        self.head = head

    def tex3d(self, kind, repeat=""):
        name = "T_" + kind.upper() + repeat
        if name in self.defs:
            return f"USE {name}"
        self.defs.add(name)
        if kind not in _cache:
            comp, f = DATA[kind]
            _cache[kind] = img(comp, f())
        return f"DEF {name} PixelTexture3D {{ {_cache[kind]} {'repeatS TRUE repeatT TRUE repeatR TRUE' if repeat else ''} }}"


def tf2d():
    # an 8 x 1 RGBA transfer function: dark blue -> cyan -> yellow -> red, alpha ramp
    cols = [0x0000FF00, 0x0050FF30, 0x00C0FF60, 0x40FFA090, 0xA0FF40B0, 0xFFE000D0, 0xFF8000F0, 0xFF2020FF]
    return "PixelTexture { image 8 1 4 " + " ".join(f"0x{c:08X}" for c in cols) + " }"


def tf3d():
    vals = []
    for k in range(4):
        for j in range(4):
            for i in range(4):
                vals.append((round(255 * i / 3) << 24) | (round(255 * j / 3) << 16) | (round(255 * k / 3) << 8) | round(255 * (0.15 + 0.85 * (i + j + k) / 9)))
    return "PixelTexture3D { image [ 4 4 4 4 " + " ".join(f"0x{c:08X}" for c in vals) + " ] }"


def weight_table():
    return "PixelTexture { image 4 4 1 " + " ".join(str(round(255 * (i / 3.0 * 0.3 + j / 3.0 * 0.7))) for j in range(4) for i in range(4)) + " }"


# --- viewpoints ---------------------------------------------------------------------------------

def look(pos, target=(0, 0, 0), up=(0, 1, 0)):
    d = [t - p for t, p in zip(target, pos)]
    l = math.sqrt(sum(v * v for v in d))
    z = [-v / l for v in d]
    x = [up[1] * z[2] - up[2] * z[1], up[2] * z[0] - up[0] * z[2], up[0] * z[1] - up[1] * z[0]]
    xl = math.sqrt(sum(v * v for v in x))
    x = [v / xl for v in x]
    y = [z[1] * x[2] - z[2] * x[1], z[2] * x[0] - z[0] * x[2], z[0] * x[1] - z[1] * x[0]]
    # rotation matrix columns x y z -> axis angle
    m = [[x[0], y[0], z[0]], [x[1], y[1], z[1]], [x[2], y[2], z[2]]]
    tr = m[0][0] + m[1][1] + m[2][2]
    a = math.acos(max(-1.0, min(1.0, (tr - 1) / 2)))
    if a < 1e-6:
        return f"position {pos[0]} {pos[1]} {pos[2]}"
    s = 2 * math.sin(a)
    ax = [(m[2][1] - m[1][2]) / s, (m[0][2] - m[2][0]) / s, (m[1][0] - m[0][1]) / s]
    return f"position {pos[0]} {pos[1]} {pos[2]} orientation {ax[0]:.6f} {ax[1]:.6f} {ax[2]:.6f} {a:.6f}"


VIEWS = {
    "front": "position 0 0 4.5",
    "obl": look((3.1, 2.2, 3.4)),
    "obl2": look((-3.3, -1.8, 2.9)),
    "top": look((0.3, 4.6, 0.8), up=(0, 0, -1)),
    "far30": "position 0 0 30",
    "behind": "position 0 0 -30 orientation 0 1 0 3.14159265",
    "farobl": look((18, 12, 18)),
}
VIEWPOINT_NODES = {"ortho": "OrthoViewpoint { position 3.1 2.2 3.4 orientation %s fieldOfView [ -2.5 -2.5 2.5 2.5 ] }"}

BACKGROUND = "Background { skyColor [ 0.12 0.16 0.3 ] groundColor [ 0.12 0.16 0.3 ] }"


def world(body, view="front", head="", headlight=False, extra=""):
    return (
        f"#X3D {'V3.3' if view == 'ortho' else 'V4.1'} utf8\nPROFILE Full\nCOMPONENT Texturing3D:2\nCOMPONENT VolumeRendering:4\n"
        f"META \"generator\" \"oracle/tools/gen_rea_b07_vol.py\"\n"
        f"NavigationInfo {{ type [ \"NONE\" ] headlight {'TRUE' if headlight else 'FALSE'} }}\n{BACKGROUND}\n"
        f"{VIEWPOINT_NODES[view] % look((3.1, 2.2, 3.4)).split('orientation ')[1] if view in VIEWPOINT_NODES else 'Viewpoint { ' + VIEWS[view] + ' }'}\n{extra}{body}\n"
    )


CASES = {}  # name -> text


def add(name, body, view="front", headlight=False, extra="", pre=None):
    w = World()
    CASES[name] = world(body(w) if callable(body) else body, view, headlight=headlight, extra=extra)


D = "dimensions 2.4 2.4 2.4"


def vd(w, style="", fields="", dims=D, voxels="lum", node="VolumeData"):
    s = f"renderStyle {style}" if style else ""
    v = f"voxels {w.tex3d(voxels)}" if voxels else ""
    return f"{node} {{ {dims} {fields} {s} {v} }}"


def vdm(w, styles, fields="", dims=D, voxels="lum", node="IsoSurfaceVolumeData"):
    return f"{node} {{ {dims} {fields} renderStyle [ {' '.join(styles)} ] voxels {w.tex3d(voxels)} }}"


# --- VolumeData -----------------------------------------------------------------------------------

add("vd-default", "VolumeData { }", "front")
add("vd-default-obl", "VolumeData { dimensions 2 1 1.5 }", "obl")
for v in ("front", "obl", "obl2", "top"):
    add(f"vd-voxels-{v}", lambda w: vd(w), v)
add("vd-aniso", lambda w: vd(w, dims="dimensions 3.4 1.2 2"), "obl")
add("vd-lum-alpha", lambda w: vd(w, voxels="lum2"), "obl")
add("vd-rgb", lambda w: vd(w, voxels="rgb"), "obl")
add("vd-rgba", lambda w: vd(w, voxels="rgba"), "obl")
add("vd-voxels-repeat", lambda w: f"VolumeData {{ {D} voxels {w.tex3d('lum', 'r')} }}", "obl")
BOX = "Shape {{ appearance Appearance {{ material Material {{ diffuseColor {c} emissiveColor {e} }} }} geometry Box {{ size {s} }} }}"
add("vd-opaque-behind", lambda w: "Transform { translation 0.5 0.2 -0.9 children " + BOX.format(c="0.9 0.3 0.2", e="0.4 0.1 0.05", s="1.6 1.6 0.4") + " }\n" + vd(w), "obl")
add("vd-opaque-through", lambda w: "Transform { translation 0.2 -0.2 0.2 rotation 0.3 1 0.2 0.6 children " + BOX.format(c="0.2 0.8 0.3", e="0.05 0.35 0.1", s="1.1 1.1 1.1") + " }\n" + vd(w), "obl2")
add("vd-opaque-front", lambda w: vd(w) + "\nTransform { translation -0.6 0.3 1.8 children " + BOX.format(c="0.9 0.8 0.2", e="0.4 0.35 0.05", s="0.9 0.9 0.5") + " }", "front")
add("vd-transparent-sort", lambda w: "Transform { translation 0.3 0 -1.3 children Shape { appearance Appearance { material Material { diffuseColor 1 0.2 0.2 emissiveColor 0.5 0.1 0.1 transparency 0.5 } } geometry Sphere { radius 0.9 } } }\n" + vd(w) + "\nTransform { translation -0.4 0.2 1.5 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.5 1 emissiveColor 0.1 0.2 0.5 transparency 0.5 } } geometry Sphere { radius 0.7 } } }", "obl")
add("vd-transform", lambda w: "Transform { translation 0.3 -0.2 0 rotation 0 1 0 0.7 scale 0.8 1.2 0.8 children " + vd(w) + " }", "obl")
add("vd-transform-nested", lambda w: "Transform { rotation 1 0 0 0.5 children Transform { translation 0 0.3 0 scale 1 0.6 1 children " + vd(w) + " } }", "obl2")
add("vd-visible-false", lambda w: vd(w, fields="visible FALSE") + "\n" + BOX.format(c="0.3 0.3 0.9", e="0.1 0.1 0.4", s="0.4 0.4 0.4"), "obl")
add("vd-bbox", lambda w: vd(w, fields="bboxSize 2.4 2.4 2.4 bboxCenter 0.1 0 0"), "obl")
add("vd-fog", lambda w: vd(w), "obl", extra="Fog { color 0.9 0.5 0.2 fogType \"LINEAR\" visibilityRange 7 }\n")
add("vd-fog-exp", lambda w: vd(w), "obl2", extra="Fog { color 0.2 0.8 0.5 fogType \"EXPONENTIAL\" visibilityRange 6 }\n")
add("vd-clipplane", lambda w: "Group { children [ ClipPlane { plane 1 0.3 0 0.1 }\n" + vd(w) + " ] }", "obl")
add("vd-two", lambda w: "Transform { translation -1.2 0 0.4 children " + vd(w, dims="dimensions 1.8 1.8 1.8") + " }\nTransform { translation 1.2 0.2 -0.4 rotation 0 1 0 1 children " + vd(w, voxels="rgba", dims="dimensions 1.8 1.8 1.8") + " }", "obl")
add("vd-no-voxels-dims", "VolumeData { dimensions 2 3 1 }", "obl")
add("vd-tiny-dims", lambda w: vd(w, dims="dimensions 0.3 0.3 0.3"), "obl")
add("vd-meta", lambda w: vd(w, fields="metadata MetadataString { name \"m\" value \"x\" }"), "front")

# --- scene integration ---------------------------------------------------------------------------

add("vx-layers", lambda w: "Transform { translation 0 0 -0.5 children " + BOX.format(c="0.9 0.3 0.2", e="0.4 0.1 0.05", s="1.2 1.2 1.2") + " }\nLayerSet { order [ 0 1 2 ] layers [ Layer { children [ Viewpoint { position 1.5 1 4 orientation 0 1 0 0.3 } " + vd(w, dims="dimensions 1.6 1.6 1.6") + " ] } Layer { viewport Viewport { clipBoundary [ 0.55 1 0 0.45 ] } children [ Viewpoint { position 0 0 3 } " + vd(w, "ToneMappedVolumeStyle { }", dims="dimensions 1.6 1.6 1.6") + " ] } ] }", "front", headlight=True)
add("vx-proto", lambda w: "PROTO Vol [ initializeOnly SFVec3f dims 1 1 1 inputOutput SFNode style NULL ] { VolumeData { dimensions IS dims renderStyle IS style voxels " + w.tex3d("lum") + " } }\nVol { dims 2.4 2.4 2.4 style CartoonVolumeStyle { colorSteps 3 } }", "obl")
add("vx-switch", lambda w: "Switch { whichChoice 1 children [ " + BOX.format(c="0.9 0.3 0.2", e="0.4 0.1 0.05", s="1 1 1") + " " + vd(w) + " ] }", "obl")
add("vx-billboard", lambda w: "Billboard { axisOfRotation 0 1 0 children [ " + vd(w, "EdgeEnhancementVolumeStyle { }") + " ] }", "obl")
add("vx-local-light", lambda w: "Group { children [ PointLight { global FALSE location 1.5 1.5 2 radius 20 } " + vd(w, "ShadedVolumeStyle { lighting TRUE material Material { diffuseColor 0.9 0.8 0.3 } }") + " ] }", "obl", headlight=False)
add("vx-ortho", lambda w: vd(w), "ortho")
add("vx-shared", lambda w: "Transform { translation -1.2 0 0 children " + vd(w, "DEF SH CartoonVolumeStyle { colorSteps 3 }", dims="dimensions 1.8 1.8 1.8") + " }\nTransform { translation 1.2 0 0 rotation 0 1 0 0.8 children VolumeData { dimensions 1.8 1.8 1.8 renderStyle USE SH voxels USE T_LUM } }", "obl")
add("vx-negative-scale", lambda w: "Transform { scale -1 1 1 children [ " + vd(w) + " ] }", "obl")
add("vx-outside-sensor", lambda w: "VolumeData { dimensions 0.2 0.2 0.2 voxels " + w.tex3d("lum") + " }", "far30")
add("vx-outside-behind", lambda w: "VolumeData { dimensions 0.2 0.2 0.2 voxels " + w.tex3d("lum") + " }", "behind")
add("vx-outside-far-obl", lambda w: "VolumeData { dimensions 0.2 0.2 0.2 voxels " + w.tex3d("lum") + " renderStyle EdgeEnhancementVolumeStyle { } }", "farobl")
add("vx-inside-rotated-parent", lambda w: "Transform { rotation 0 0 1 1.0 children Transform { rotation 1 0 0 0.6 translation 0.2 0 0 children " + vd(w, "EdgeEnhancementVolumeStyle { }") + " } }", "obl2")
add("vx-opaque-all-styles", lambda w: "Transform { translation 0 0 -1.2 children " + BOX.format(c="0.9 0.3 0.2", e="0.4 0.1 0.05", s="3 3 0.3") + " }\n" + vd(w, "ComposedVolumeStyle { renderStyle [ BoundaryEnhancementVolumeStyle { } ToneMappedVolumeStyle { } ] }"), "obl2", headlight=True)

# --- OpacityMapVolumeStyle ------------------------------------------------------------------------

add("om-default", lambda w: vd(w, "OpacityMapVolumeStyle { }"), "obl")
add("om-tf2d", lambda w: vd(w, f"OpacityMapVolumeStyle {{ transferFunction {tf2d()} }}"), "obl")
add("om-tf2d-front", lambda w: vd(w, f"OpacityMapVolumeStyle {{ transferFunction {tf2d()} }}", voxels="rgba"), "front")
add("om-tf3d", lambda w: vd(w, f"OpacityMapVolumeStyle {{ transferFunction {tf3d()} }}", voxels="rgb"), "obl2")
add("om-disabled", lambda w: vd(w, f"OpacityMapVolumeStyle {{ enabled FALSE transferFunction {tf2d()} }}"), "obl")
add("om-tf-invalid", lambda w: vd(w, "OpacityMapVolumeStyle { transferFunction Box { } }"), "obl")

# --- BoundaryEnhancementVolumeStyle ---------------------------------------------------------------

add("bd-default", lambda w: vd(w, "BoundaryEnhancementVolumeStyle { }"), "obl")
add("bd-params", lambda w: vd(w, "BoundaryEnhancementVolumeStyle { retainedOpacity 0.05 boundaryOpacity 1 opacityFactor 5 }"), "obl2")
add("bd-flat", lambda w: vd(w, "BoundaryEnhancementVolumeStyle { retainedOpacity 0.6 boundaryOpacity 0.1 opacityFactor 0.5 }"), "front")
add("bd-disabled", lambda w: vd(w, "BoundaryEnhancementVolumeStyle { enabled FALSE }"), "obl")

# --- CartoonVolumeStyle ---------------------------------------------------------------------------

add("ct-default", lambda w: vd(w, "CartoonVolumeStyle { }"), "obl")
add("ct-steps2", lambda w: vd(w, "CartoonVolumeStyle { colorSteps 2 orthogonalColor 1 0.4 0.1 1 parallelColor 0.1 0.2 0.9 1 }"), "obl2")
add("ct-steps8", lambda w: vd(w, "CartoonVolumeStyle { colorSteps 8 orthogonalColor 0.9 0.9 0.2 1 parallelColor 0.8 0.1 0.6 1 }"), "obl")
add("ct-steps0", lambda w: vd(w, "CartoonVolumeStyle { colorSteps 0 orthogonalColor 0.2 0.9 0.2 1 parallelColor 0.1 0.1 0.5 1 }"), "front")
add("ct-normals", lambda w: vd(w, f"CartoonVolumeStyle {{ colorSteps 3 surfaceNormals {w.tex3d('normals')} }}"), "obl")
add("ct-disabled", lambda w: vd(w, "CartoonVolumeStyle { enabled FALSE }"), "obl")

# --- EdgeEnhancementVolumeStyle -------------------------------------------------------------------

add("ed-default", lambda w: vd(w, "EdgeEnhancementVolumeStyle { }"), "obl")
add("ed-params", lambda w: vd(w, "EdgeEnhancementVolumeStyle { edgeColor 1 0.2 0.2 1 gradientThreshold 1.1 }"), "obl2")
add("ed-normals", lambda w: vd(w, f"EdgeEnhancementVolumeStyle {{ edgeColor 0.2 1 0.2 1 gradientThreshold 0.9 surfaceNormals {w.tex3d('normals')} }}"), "obl")
add("ed-disabled", lambda w: vd(w, "EdgeEnhancementVolumeStyle { enabled FALSE }"), "obl")

# --- SilhouetteEnhancementVolumeStyle -------------------------------------------------------------

add("sl-default", lambda w: vd(w, "SilhouetteEnhancementVolumeStyle { }"), "obl")
add("sl-params", lambda w: vd(w, "SilhouetteEnhancementVolumeStyle { silhouetteRetainedOpacity 0.3 silhouetteBoundaryOpacity 1 silhouetteSharpness 3 }"), "obl2")
add("sl-normals", lambda w: vd(w, f"SilhouetteEnhancementVolumeStyle {{ silhouetteRetainedOpacity 0.5 silhouetteBoundaryOpacity 0.8 silhouetteSharpness 1.5 surfaceNormals {w.tex3d('normals')} }}"), "obl")
add("sl-disabled", lambda w: vd(w, "SilhouetteEnhancementVolumeStyle { enabled FALSE }"), "obl")

# --- ProjectionVolumeStyle ------------------------------------------------------------------------

for t in ("MAX", "MIN", "AVERAGE"):
    for v in ("front", "obl"):
        add(f"pr-{t.lower()}-{v}", lambda w, t=t: vd(w, f"ProjectionVolumeStyle {{ type \"{t}\" }}"), v)
    add(f"pr-{t.lower()}-threshold", lambda w, t=t: vd(w, f"ProjectionVolumeStyle {{ type \"{t}\" intensityThreshold 0.35 }}"), "obl2")
add("pr-default", lambda w: vd(w, "ProjectionVolumeStyle { }"), "obl")
add("pr-bad-type", lambda w: vd(w, "ProjectionVolumeStyle { type \"BOGUS\" }"), "obl")
add("pr-disabled", lambda w: vd(w, "ProjectionVolumeStyle { enabled FALSE }"), "obl")

# --- ShadedVolumeStyle ----------------------------------------------------------------------------

add("sh-default", lambda w: vd(w, "ShadedVolumeStyle { }"), "obl")
add("sh-material", lambda w: vd(w, "ShadedVolumeStyle { material Material { diffuseColor 0.9 0.4 0.1 transparency 0.3 } }"), "obl")
LIT = "ShadedVolumeStyle { lighting TRUE %s }"
add("sh-lit-headlight", lambda w: vd(w, LIT % ""), "obl", headlight=True)
add("sh-lit-material", lambda w: vd(w, LIT % "material Material { diffuseColor 0.9 0.5 0.2 ambientIntensity 0.3 specularColor 1 1 1 shininess 0.4 emissiveColor 0.02 0.02 0.05 transparency 0.2 }"), "obl", headlight=True)
add("sh-lit-dir", lambda w: vd(w, LIT % "material Material { diffuseColor 0.4 0.7 0.9 specularColor 0.8 0.8 0.8 shininess 0.6 }"), "obl2", extra="DirectionalLight { direction -0.5 -0.6 -0.6 intensity 0.9 ambientIntensity 0.2 }\n")
add("sh-lit-point", lambda w: vd(w, LIT % "material Material { diffuseColor 0.9 0.9 0.9 specularColor 1 1 1 shininess 0.8 }"), "obl", extra="PointLight { location 2 2 3 radius 20 attenuation 1 0.05 0 color 1 0.8 0.6 intensity 1.2 }\n")
add("sh-lit-spot", lambda w: vd(w, LIT % "material Material { diffuseColor 0.9 0.9 0.9 }"), "obl", extra="SpotLight { location 2 2.5 3 direction -0.5 -0.6 -0.7 radius 30 cutOffAngle 0.6 beamWidth 0.3 intensity 1.4 }\n")
add("sh-lit-two", lambda w: vd(w, LIT % "material Material { diffuseColor 0.8 0.8 0.8 emissiveColor 0.03 0 0 }"), "obl", extra="DirectionalLight { direction -1 -0.3 -0.5 color 1 0.3 0.3 }\nPointLight { location -2 1 2 radius 20 color 0.3 0.3 1 }\n")
add("sh-normals", lambda w: vd(w, f"ShadedVolumeStyle {{ lighting TRUE surfaceNormals {w.tex3d('normals')} }}"), "obl", headlight=True)
add("sh-shadows", lambda w: vd(w, LIT % "shadows TRUE"), "obl", headlight=True)
add("sh-disabled", lambda w: vd(w, "ShadedVolumeStyle { enabled FALSE }"), "obl")

# --- ToneMappedVolumeStyle ------------------------------------------------------------------------

add("tm-default", lambda w: vd(w, "ToneMappedVolumeStyle { }"), "obl", headlight=True)
add("tm-colors", lambda w: vd(w, "ToneMappedVolumeStyle { coolColor 0.1 0.8 0.3 1 warmColor 0.9 0.2 0.5 1 }"), "obl2", headlight=True)
add("tm-nolight", lambda w: vd(w, "ToneMappedVolumeStyle { }"), "obl")
add("tm-lights", lambda w: vd(w, "ToneMappedVolumeStyle { }"), "obl", extra="DirectionalLight { direction -1 -0.4 -0.5 }\nPointLight { location -2 2 2 radius 20 }\n")
add("tm-normals", lambda w: vd(w, f"ToneMappedVolumeStyle {{ surfaceNormals {w.tex3d('normals')} }}"), "obl", headlight=True)
add("tm-disabled", lambda w: vd(w, "ToneMappedVolumeStyle { enabled FALSE }"), "obl", headlight=True)

# --- ComposedVolumeStyle --------------------------------------------------------------------------

add("cm-empty", lambda w: vd(w, "ComposedVolumeStyle { }"), "obl")
add("cm-boundary-cartoon", lambda w: vd(w, "ComposedVolumeStyle { renderStyle [ BoundaryEnhancementVolumeStyle { } CartoonVolumeStyle { colorSteps 3 } ] }"), "obl")
add("cm-cartoon-boundary", lambda w: vd(w, "ComposedVolumeStyle { renderStyle [ CartoonVolumeStyle { colorSteps 3 } BoundaryEnhancementVolumeStyle { } ] }"), "obl")
add("cm-edge-silhouette", lambda w: vd(w, "ComposedVolumeStyle { renderStyle [ EdgeEnhancementVolumeStyle { edgeColor 1 0 0 1 } SilhouetteEnhancementVolumeStyle { silhouetteBoundaryOpacity 0.9 } ] }"), "obl2")
add("cm-opacity-tone", lambda w: vd(w, f"ComposedVolumeStyle {{ renderStyle [ OpacityMapVolumeStyle {{ transferFunction {tf2d()} }} ToneMappedVolumeStyle {{ }} ] }}"), "obl", headlight=True)
add("cm-shaded-edge", lambda w: vd(w, "ComposedVolumeStyle { renderStyle [ ShadedVolumeStyle { lighting TRUE } EdgeEnhancementVolumeStyle { } ] }"), "obl", headlight=True)
add("cm-nested", lambda w: vd(w, "ComposedVolumeStyle { renderStyle [ ComposedVolumeStyle { renderStyle [ CartoonVolumeStyle { colorSteps 2 } ] } SilhouetteEnhancementVolumeStyle { silhouetteRetainedOpacity 0.4 silhouetteBoundaryOpacity 1 } ] }"), "obl")
add("cm-with-projection", lambda w: vd(w, "ComposedVolumeStyle { renderStyle [ ProjectionVolumeStyle { type \"MAX\" } CartoonVolumeStyle { } ] }"), "obl")
add("cm-disabled", lambda w: vd(w, "ComposedVolumeStyle { enabled FALSE renderStyle [ CartoonVolumeStyle { } ] }"), "obl")
add("cm-inner-disabled", lambda w: vd(w, "ComposedVolumeStyle { renderStyle [ CartoonVolumeStyle { enabled FALSE } BoundaryEnhancementVolumeStyle { } ] }"), "obl")

# --- BlendedVolumeStyle ---------------------------------------------------------------------------

def bl(w, fields="", inner="", voxels=True, view=None):
    v = f"voxels {w.tex3d('blend')}" if voxels else ""
    r = f"renderStyle {inner}" if inner else ""
    return vd(w, f"BlendedVolumeStyle {{ {fields} {v} {r} }}")


add("bl-no-voxels", lambda w: bl(w, voxels=False), "obl")
add("bl-default", lambda w: bl(w), "obl")
add("bl-weights", lambda w: bl(w, "weightConstant1 0.9 weightConstant2 0.4"), "obl2")
add("bl-alpha0", lambda w: bl(w, "weightFunction1 \"ALPHA0\" weightFunction2 \"ONE_MINUS_ALPHA0\""), "obl")
# ALPHA1 and ONE_MINUS_ALPHA1: X_ITE 16.5.0 writes `blendColor_ <id> .a` (invalid GLSL) and the page never
# finishes a frame (x_ite-shot.mjs: TIMEOUT 45s), so there is no picture to compare: see unverifiable/.
UNVERIFIABLE = {
    "bl-alpha1": lambda w: bl(w, "weightFunction1 \"ALPHA1\" weightFunction2 \"CONSTANT\""),
    "bl-one-minus-alpha1": lambda w: bl(w, "weightFunction1 \"CONSTANT\" weightFunction2 \"ONE_MINUS_ALPHA1\""),
}
add("bl-table", lambda w: bl(w, f"weightFunction1 \"TABLE\" weightFunction2 \"TABLE\" weightTransferFunction1 {weight_table()} weightTransferFunction2 {weight_table()}"), "obl")
add("bl-table-no-tf", lambda w: bl(w, "weightFunction1 \"TABLE\" weightConstant1 0.7 weightFunction2 \"TABLE\" weightConstant2 0.2"), "obl")
add("bl-bad-function", lambda w: bl(w, "weightFunction1 \"BOGUS\" weightConstant1 0.8"), "obl")
add("bl-inner-edge", lambda w: bl(w, inner="EdgeEnhancementVolumeStyle { edgeColor 1 1 0 1 }"), "obl")
add("bl-inner-cartoon", lambda w: bl(w, inner="CartoonVolumeStyle { colorSteps 2 }"), "obl2")
add("bl-inner-opacity", lambda w: bl(w, inner=f"OpacityMapVolumeStyle {{ transferFunction {tf2d()} }}"), "obl")
add("bl-outer-edge", lambda w: vd(w, f"ComposedVolumeStyle {{ renderStyle [ EdgeEnhancementVolumeStyle {{ }} BlendedVolumeStyle {{ voxels {w.tex3d('blend')} }} ] }}"), "obl")
add("bl-disabled", lambda w: bl(w, "enabled FALSE"), "obl")

# --- IsoSurfaceVolumeData -------------------------------------------------------------------------

def iso(w, fields="", styles=(), voxels="lum", dims=D):
    s = f"renderStyle [ {' '.join(styles)} ]" if styles else ""
    return f"IsoSurfaceVolumeData {{ {dims} {fields} {s} voxels {w.tex3d(voxels)} }}"


add("is-single", lambda w: iso(w, "surfaceValues [ 0.4 ]"), "obl")
add("is-single-front", lambda w: iso(w, "surfaceValues [ 0.25 ]"), "front")
add("is-two", lambda w: iso(w, "surfaceValues [ 0.3 0.65 ]"), "obl2")
add("is-three", lambda w: iso(w, "surfaceValues [ 0.2 0.45 0.7 ]"), "obl")
add("is-contour", lambda w: iso(w, "surfaceValues [ 0.45 ] contourStepSize 0.15"), "obl")
add("is-contour-negative", lambda w: iso(w, "surfaceValues [ 0.5 ] contourStepSize -0.2"), "obl2")
add("is-tolerance", lambda w: iso(w, "surfaceValues [ 0.35 ] surfaceTolerance 0.12"), "obl")
add("is-empty", lambda w: iso(w), "obl")
add("is-high", lambda w: iso(w, "surfaceValues [ 0.97 ]"), "obl")
add("is-gradients", lambda w: iso(w, f"surfaceValues [ 0.35 ] surfaceTolerance 0.2 gradients {w.tex3d('normals')}"), "obl")
add("is-gradients-styles", lambda w: iso(w, f"surfaceValues [ 0.2 0.45 0.7 ] surfaceTolerance 0.15 gradients {w.tex3d('normals')}", ["CartoonVolumeStyle { colorSteps 3 }", "ToneMappedVolumeStyle { }"]), "obl", headlight=True)
add("is-gradients-multi", lambda w: iso(w, f"surfaceValues [ 0.2 0.45 0.7 ] surfaceTolerance 0.15 gradients {w.tex3d('normals')}"), "obl")
add("is-style-shaded", lambda w: iso(w, "surfaceValues [ 0.4 ]", ["ShadedVolumeStyle { lighting TRUE material Material { diffuseColor 0.9 0.6 0.2 specularColor 1 1 1 shininess 0.5 } }"]), "obl", headlight=True)
add("is-style-cartoon", lambda w: iso(w, "surfaceValues [ 0.4 ]", ["CartoonVolumeStyle { colorSteps 3 }"]), "obl2")
add("is-styles-multi", lambda w: iso(w, "surfaceValues [ 0.2 0.45 0.7 ]", ["ToneMappedVolumeStyle { }", "CartoonVolumeStyle { colorSteps 2 }", "EdgeEnhancementVolumeStyle { edgeColor 1 0 0 1 }"]), "obl", headlight=True)
add("is-styles-fewer", lambda w: iso(w, "surfaceValues [ 0.2 0.45 0.7 ]", ["CartoonVolumeStyle { colorSteps 2 orthogonalColor 1 0 0 1 }"]), "obl")
add("is-style-composed", lambda w: iso(w, "surfaceValues [ 0.4 ]", ["ComposedVolumeStyle { renderStyle [ CartoonVolumeStyle { colorSteps 3 } SilhouetteEnhancementVolumeStyle { } ] }"]), "obl")
add("is-style-projection", lambda w: iso(w, "surfaceValues [ 0.4 ]", ["ProjectionVolumeStyle { type \"MAX\" }"]), "obl")
add("is-style-disabled", lambda w: iso(w, "surfaceValues [ 0.4 ]", ["CartoonVolumeStyle { enabled FALSE }"]), "obl")
add("is-default-dims", "IsoSurfaceVolumeData { surfaceValues [ 0.5 ] }", "front")

# --- SegmentedVolumeData --------------------------------------------------------------------------

def seg(w, fields="", styles=(), ids=True, voxels="lum", dims=D):
    s = f"renderStyle [ {' '.join(styles)} ]" if styles else ""
    i = f"segmentIdentifiers {w.tex3d('segments')}" if ids else ""
    return f"SegmentedVolumeData {{ {dims} {fields} {s} {i} voxels {w.tex3d(voxels)} }}"


SEG_STYLES = ["CartoonVolumeStyle { colorSteps 3 orthogonalColor 1 0.5 0.1 1 parallelColor 0.2 0.1 0.6 1 }", "EdgeEnhancementVolumeStyle { edgeColor 0.1 1 0.2 1 gradientThreshold 1.2 }", "BoundaryEnhancementVolumeStyle { boundaryOpacity 1 retainedOpacity 0.1 }"]
add("sg-default", "SegmentedVolumeData { }", "front")
add("sg-no-ids", lambda w: seg(w, ids=False), "obl")
add("sg-ids-no-style", lambda w: seg(w), "obl")
add("sg-ids-styles", lambda w: seg(w, styles=SEG_STYLES), "obl")
add("sg-ids-styles2", lambda w: seg(w, styles=SEG_STYLES[:2]), "obl2")
add("sg-enabled", lambda w: seg(w, "segmentEnabled [ TRUE FALSE TRUE ]", SEG_STYLES), "obl")
add("sg-enabled-short", lambda w: seg(w, "segmentEnabled [ FALSE ]", SEG_STYLES), "obl2")
add("sg-no-ids-styles", lambda w: seg(w, styles=SEG_STYLES, ids=False), "obl")
add("sg-composed", lambda w: seg(w, styles=["ComposedVolumeStyle { renderStyle [ CartoonVolumeStyle { colorSteps 2 } SilhouetteEnhancementVolumeStyle { } ] }", "ToneMappedVolumeStyle { }"]), "obl", headlight=True)
add("sg-shaded", lambda w: seg(w, styles=["ShadedVolumeStyle { lighting TRUE material Material { diffuseColor 1 0.3 0.3 } }", "ShadedVolumeStyle { lighting TRUE material Material { diffuseColor 0.3 0.3 1 } }", "OpacityMapVolumeStyle { }"]), "obl", headlight=True)
add("sg-projection", lambda w: seg(w, styles=["ProjectionVolumeStyle { type \"MAX\" }", "CartoonVolumeStyle { }"]), "obl")

# --- stepped cases (key events drive Scripts; oracle/tools/rea_b07_vol.py case) -----------------

STEPPED = {}  # name -> (world text, script dict)


def stepped(name, body, keys, view="obl", headlight=False, extra="", script="", routes="", after=6, first=16, gap=10, frames=None):
    """A world with a KeySensor/Script (`script` is the JavaScript body of on_key, `body` the nodes) and an
    event script that presses `keys` one per `gap` frames and captures `after` frames later."""
    text = world(body + "\nDEF K KeySensor { }\n" + script + "\nROUTE K.keyPress TO R.on_key\n" + routes, view, headlight=headlight, extra=extra)
    events, capture = [], []
    f = first
    for k in keys:
        events.append({"frame": f, "type": "key", "key": k})
        capture.append(f + after)
        f += gap
    capture = [first - 2] + capture
    total = frames or (f + 2)
    STEPPED[name] = (text, {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": total, "capture": capture, "dump": capture, "events": events})


def script_node(fields, body):
    return "DEF R Script {\n  directOutput TRUE\n  inputOnly SFString on_key\n" + "".join(f"  initializeOnly SFNode {n.lower()} USE {n}\n" for n in fields) + '  url "javascript:\n' + body + '\n"\n}\n'


def rt_world(name, node, fields, body, keys, **kw):
    """`node` text contains DEF names; `fields` are the DEF names the Script needs (lower case in the script)."""
    stepped(name, node, keys, script=script_node(fields, "function on_key (k)\n{\n" + body + "\n}"), **kw)


_w = World()
VOX = _w.tex3d("lum")
NRM = "DEF NRM PixelTexture3D { " + _cache["normals"] + " }" if "normals" in _cache else ""


def pre(w, *kinds):
    return [w.tex3d(k) for k in kinds]


def rt_style(name, style, fields, body, keys, vox="lum", extra_nodes="", headlight=False, view="obl", **kw):
    w = World()
    v = w.tex3d(vox)
    node = f"DEF VD VolumeData {{ {D} renderStyle {style} voxels {v} }}\n{extra_nodes}"
    rt_world(name, node, ["VD"] + fields, body, keys, headlight=headlight, view=view, **kw)


# VolumeData: every inputOutput field, the voxels and style swapped, removal from a live Group, a Script-created instance.
def vol_fields():
    w = World()
    v = w.tex3d("lum")
    other = w.tex3d("rgb")
    other_plain = other.replace("DEF T_RGB ", "")
    node = (
        f"DEF G Group {{ children [ DEF VD VolumeData {{ {D} voxels {v} }} ] }}\n"
        f"DEF OTHER {other_plain}\nDEF CART CartoonVolumeStyle {{ colorSteps 3 }}\nDEF BOUND BoundaryEnhancementVolumeStyle {{ }}\n"
    )
    body = """
   if (k == 'a') vd .dimensions = new SFVec3f (1.6, 2.4, 2.4);
   if (k == 'b') vd .voxels = other;
   if (k == 'c') vd .renderStyle = cart;
   if (k == 'd') vd .renderStyle = bound;
   if (k == 'e') vd .renderStyle = null;
   if (k == 'f') vd .visible = false;
   if (k == 'g') vd .visible = true;
   if (k == 'h') g .removeChildren = new MFNode (vd);
   if (k == 'i') g .addChildren = new MFNode (vd);
   if (k == 'j') { var n = Browser .currentScene .createNode ('VolumeData'); n .dimensions = new SFVec3f (1.2, 1.2, 1.2); n .voxels = other; n .renderStyle = cart; g .addChildren = new MFNode (n); }
   if (k == 'k') vd .voxels = null;
   if (k == 'l') vd .dimensions = new SFVec3f (2.4, 2.4, 2.4);"""
    rt_world("rt-volumedata", node, ["VD", "G", "OTHER", "CART", "BOUND"], body, list("abcdefghijkl"), gap=9)


vol_fields()


def rt_all():
    # BoundaryEnhancementVolumeStyle
    rt_style("rt-boundary", "DEF S BoundaryEnhancementVolumeStyle { }", ["S"], """
   if (k == 'a') s .retainedOpacity = 0.05;
   if (k == 'b') s .boundaryOpacity = 1;
   if (k == 'c') s .opacityFactor = 5;
   if (k == 'd') s .enabled = false;
   if (k == 'e') s .enabled = true;
   if (k == 'f') { s .retainedOpacity = 0.5; s .boundaryOpacity = 0.3; s .opacityFactor = 1; }""", list("abcdef"))
    # CartoonVolumeStyle
    w = World(); n = w.tex3d("normals")
    rt_style("rt-cartoon", "DEF S CartoonVolumeStyle { }", ["S", "NRM"], """
   if (k == 'a') s .colorSteps = 2;
   if (k == 'b') s .orthogonalColor = new SFColorRGBA (1, 0.3, 0.1, 1);
   if (k == 'c') s .parallelColor = new SFColorRGBA (0.1, 0.2, 0.9, 1);
   if (k == 'd') s .surfaceNormals = nrm;
   if (k == 'e') s .enabled = false;
   if (k == 'f') { s .enabled = true; s .colorSteps = 7; }
   if (k == 'g') s .surfaceNormals = null;""", list("abcdefg"), extra_nodes=n.replace("DEF T_NORMALS", "DEF NRM"))
    # EdgeEnhancementVolumeStyle
    w = World(); n = w.tex3d("normals")
    rt_style("rt-edge", "DEF S EdgeEnhancementVolumeStyle { }", ["S", "NRM"], """
   if (k == 'a') s .edgeColor = new SFColorRGBA (1, 0.2, 0.2, 1);
   if (k == 'b') s .gradientThreshold = 1.1;
   if (k == 'c') s .surfaceNormals = nrm;
   if (k == 'd') s .enabled = false;
   if (k == 'e') s .enabled = true;""", list("abcde"), extra_nodes=n.replace("DEF T_NORMALS", "DEF NRM"))
    # SilhouetteEnhancementVolumeStyle
    rt_style("rt-silhouette", "DEF S SilhouetteEnhancementVolumeStyle { }", ["S"], """
   if (k == 'a') s .silhouetteRetainedOpacity = 0.3;
   if (k == 'b') s .silhouetteBoundaryOpacity = 1;
   if (k == 'c') s .silhouetteSharpness = 3;
   if (k == 'd') s .enabled = false;
   if (k == 'e') s .enabled = true;""", list("abcde"))
    # ProjectionVolumeStyle
    rt_style("rt-projection", "DEF S ProjectionVolumeStyle { }", ["S"], """
   if (k == 'a') s .type = 'MIN';
   if (k == 'b') s .type = 'AVERAGE';
   if (k == 'c') { s .type = 'MAX'; s .intensityThreshold = 0.35; }
   if (k == 'd') s .intensityThreshold = 0;
   if (k == 'e') s .enabled = false;
   if (k == 'f') s .enabled = true;
   if (k == 'g') s .type = 'MIN';
   if (k == 'h') s .intensityThreshold = 0.35;""", list("abcdefgh"))
    # ShadedVolumeStyle
    rt_style("rt-shaded", "DEF S ShadedVolumeStyle { material DEF M Material { diffuseColor 0.9 0.5 0.2 } }", ["S", "M"], """
   if (k == 'a') s .lighting = true;
   if (k == 'b') m .diffuseColor = new SFColor (0.3, 0.8, 0.4);
   if (k == 'c') m .specularColor = new SFColor (1, 1, 1);
   if (k == 'd') m .shininess = 0.7;
   if (k == 'e') m .emissiveColor = new SFColor (0.05, 0.0, 0.1);
   if (k == 'f') m .transparency = 0.4;
   if (k == 'g') m .ambientIntensity = 0.6;
   if (k == 'h') s .material = null;
   if (k == 'i') s .enabled = false;
   if (k == 'j') { s .enabled = true; s .lighting = false; }""", list("abcdefghij"), headlight=True)
    # ToneMappedVolumeStyle
    rt_style("rt-tonemapped", "DEF S ToneMappedVolumeStyle { }", ["S"], """
   if (k == 'a') s .coolColor = new SFColorRGBA (0.1, 0.8, 0.3, 1);
   if (k == 'b') s .warmColor = new SFColorRGBA (0.9, 0.2, 0.5, 1);
   if (k == 'c') s .enabled = false;
   if (k == 'd') s .enabled = true;""", list("abcd"), headlight=True)
    # OpacityMapVolumeStyle
    rt_style("rt-opacity", "DEF S OpacityMapVolumeStyle { }", ["S", "TF", "TF3"], """
   if (k == 'a') s .transferFunction = tf;
   if (k == 'b') s .transferFunction = tf3;
   if (k == 'c') s .enabled = false;
   if (k == 'd') s .enabled = true;
   if (k == 'e') s .transferFunction = null;""", list("abcde"), vox="rgb", extra_nodes=f"DEF TF {tf2d()}\nDEF TF3 {tf3d()}\n")
    # ComposedVolumeStyle
    rt_style("rt-composed", "DEF S ComposedVolumeStyle { renderStyle [ DEF C CartoonVolumeStyle { colorSteps 3 } DEF B BoundaryEnhancementVolumeStyle { } ] }", ["S", "C", "B", "E"], """
   if (k == 'a') s .renderStyle = new MFNode (b, c);
   if (k == 'b') s .renderStyle = new MFNode (c, e, b);
   if (k == 'c') c .colorSteps = 2;
   if (k == 'd') s .enabled = false;
   if (k == 'e') s .enabled = true;
   if (k == 'f') s .renderStyle = new MFNode ();
   if (k == 'g') s .renderStyle = new MFNode (e);""", list("abcdefg"), extra_nodes="DEF E EdgeEnhancementVolumeStyle { edgeColor 1 0 0 1 }\n")
    # BlendedVolumeStyle
    w = World(); n = w.tex3d("blend")
    rt_style("rt-blended", "DEF S BlendedVolumeStyle { }", ["S", "BV", "E", "TFW"], """
   if (k == 'a') s .voxels = bv;
   if (k == 'b') s .weightConstant1 = 0.9;
   if (k == 'c') s .weightConstant2 = 0.3;
   if (k == 'd') s .weightFunction1 = 'ALPHA0';
   if (k == 'e') s .weightFunction2 = 'ONE_MINUS_ALPHA0';
   if (k == 'f') { s .weightFunction1 = 'TABLE'; s .weightTransferFunction1 = tfw; }
   if (k == 'g') s .renderStyle = e;
   if (k == 'h') s .enabled = false;
   if (k == 'i') s .enabled = true;
   if (k == 'j') s .voxels = null;""", list("abcdefghij"), extra_nodes=n.replace("DEF T_BLEND", "DEF BV") + "DEF E EdgeEnhancementVolumeStyle { }\nDEF TFW " + weight_table() + "\n")
    # IsoSurfaceVolumeData
    w = World(); v = w.tex3d("lum"); n = w.tex3d("normals")
    node = f"DEF VD IsoSurfaceVolumeData {{ {D} surfaceValues [ 0.4 ] voxels {v} }}\nDEF CART CartoonVolumeStyle {{ colorSteps 3 }}\nDEF TONE ToneMappedVolumeStyle {{ }}\n{n.replace('DEF T_NORMALS', 'DEF GR')}\nDEF OTHER {w.tex3d('rgb').replace('DEF T_RGB ', '')}\n"
    rt_world("rt-iso", node, ["VD", "CART", "TONE", "GR", "OTHER"], """
   if (k == 'a') vd .surfaceValues = new MFFloat (0.25);
   if (k == 'b') vd .surfaceValues = new MFFloat (0.2, 0.45, 0.7);
   if (k == 'c') vd .contourStepSize = 0.1;
   if (k == 'd') vd .surfaceValues = new MFFloat (0.45);
   if (k == 'e') vd .surfaceTolerance = 0.15;
   if (k == 'f') vd .renderStyle = new MFNode (cart, tone);
   if (k == 'g') vd .gradients = gr;
   if (k == 'h') { vd .contourStepSize = 0; vd .surfaceValues = new MFFloat (0.2, 0.6); }
   if (k == 'i') vd .surfaceValues = new MFFloat ();
   if (k == 'j') vd .dimensions = new SFVec3f (1.6, 2.4, 2.4);
   if (k == 'k') vd .voxels = other;
   if (k == 'l') vd .surfaceValues = new MFFloat (0.3, 0.5);""", list("abcdefghijkl"), gap=9)
    # SegmentedVolumeData
    w = World(); v = w.tex3d("lum"); n = w.tex3d("segments")
    node = f"DEF VD SegmentedVolumeData {{ {D} voxels {v} segmentIdentifiers {n} renderStyle [ DEF A CartoonVolumeStyle {{ colorSteps 3 }} DEF B BoundaryEnhancementVolumeStyle {{ boundaryOpacity 1 }} DEF C EdgeEnhancementVolumeStyle {{ edgeColor 0 1 0 1 }} ] }}\n"
    rt_world("rt-segmented", node, ["VD", "A", "B", "C"], """
   if (k == 'a') vd .segmentEnabled = new MFBool (true, false, true);
   if (k == 'b') vd .segmentEnabled = new MFBool (false);
   if (k == 'c') vd .segmentEnabled = new MFBool ();
   if (k == 'd') vd .renderStyle = new MFNode (a, c);
   if (k == 'e') vd .renderStyle = new MFNode (c, b, a);
   if (k == 'f') vd .segmentIdentifiers = null;
   if (k == 'g') vd .renderStyle = new MFNode ();
   if (k == 'h') a .colorSteps = 2;""", list("abcdefgh"), gap=9)


rt_all()


# Item 5 for IsoSurfaceVolumeData and SegmentedVolumeData: removal from a live Group, re-adding, and a
# Script-created instance that is added and then removed.
def rt_lifecycle():
    w = World(); v = w.tex3d("lum")
    node = (f"DEF G Group {{ children [ DEF VD IsoSurfaceVolumeData {{ {D} surfaceValues [ 0.2 0.45 0.7 ] "
            f"renderStyle [ DEF CART CartoonVolumeStyle {{ colorSteps 3 }} DEF TONE ToneMappedVolumeStyle {{ }} ] voxels {v} }} ] }}\n"
            f"DEF OTHER {w.tex3d('rgb').replace('DEF T_RGB ', '')}\n")
    rt_world("rt-iso-life", node, ["VD", "G", "CART", "TONE", "OTHER"], """
   if (k == 'a') g .removeChildren = new MFNode (vd);
   if (k == 'b') g .addChildren = new MFNode (vd);
   if (k == 'c') { made = Browser .currentScene .createNode ('IsoSurfaceVolumeData'); made .dimensions = new SFVec3f (1.2, 1.2, 1.2); made .surfaceValues = new MFFloat (0.35, 0.6); made .voxels = other; g .addChildren = new MFNode (made); }
   if (k == 'd') g .removeChildren = new MFNode (made);
   if (k == 'e') g .removeChildren = new MFNode (vd);
   if (k == 'f') g .addChildren = new MFNode (made);
   if (k == 'g') g .addChildren = new MFNode (vd);
   if (k == 'h') g .removeChildren = new MFNode (made);""", list("abcdefgh"), gap=9)
    w = World(); v = w.tex3d("lum"); sg = w.tex3d("segments")
    node = (f"DEF G Group {{ children [ DEF VD SegmentedVolumeData {{ {D} segmentIdentifiers {sg} segmentEnabled [ true false true ] "
            f"renderStyle [ DEF A CartoonVolumeStyle {{ colorSteps 3 }} DEF B BoundaryEnhancementVolumeStyle {{ boundaryOpacity 1 }} DEF C EdgeEnhancementVolumeStyle {{ edgeColor 0 1 0 1 }} ] voxels {v} }} ] }}\n"
            f"DEF OTHER {w.tex3d('rgb').replace('DEF T_RGB ', '')}\n")
    rt_world("rt-segmented-life", node, ["VD", "G", "A", "B", "C", "OTHER", "T_SEGMENTS"], """
   if (k == 'a') g .removeChildren = new MFNode (vd);
   if (k == 'b') g .addChildren = new MFNode (vd);
   if (k == 'c') { made = Browser .currentScene .createNode ('SegmentedVolumeData'); made .dimensions = new SFVec3f (1.2, 1.2, 1.2); made .segmentIdentifiers = t_segments; made .renderStyle = new MFNode (c, b, a); made .voxels = other; g .addChildren = new MFNode (made); }
   if (k == 'd') g .removeChildren = new MFNode (made);
   if (k == 'e') g .removeChildren = new MFNode (vd);
   if (k == 'f') g .addChildren = new MFNode (made);
   if (k == 'g') g .addChildren = new MFNode (vd);
   if (k == 'h') g .removeChildren = new MFNode (made);""", list("abcdefgh"), gap=9)


rt_lifecycle()

# ROUTE driven: a TimeSensor and interpolators change style fields and the view (the ProximitySensor follows).
def rt_route():
    w = World(); v = w.tex3d("lum")
    node = (
        f"DEF VD VolumeData {{ {D} voxels {v} renderStyle DEF S BoundaryEnhancementVolumeStyle {{ }} }}\n"
        "DEF T TimeSensor { cycleInterval 2 loop TRUE }\n"
        "DEF I ScalarInterpolator { key [ 0 0.5 1 ] keyValue [ 0.1 1 0.1 ] }\n"
        "DEF F ScalarInterpolator { key [ 0 1 ] keyValue [ 0.5 6 ] }\n"
    )
    routes = "ROUTE T.fraction_changed TO I.set_fraction\nROUTE I.value_changed TO S.set_boundaryOpacity\nROUTE T.fraction_changed TO F.set_fraction\nROUTE F.value_changed TO S.set_opacityFactor\n"
    text = world(node + routes + "DEF K KeySensor { }\nDEF R Script { inputOnly SFString on_key url \"javascript:function on_key (k) { }\" }\n", "obl")
    frames = 70
    STEPPED["rt-route"] = (text, {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames, "capture": [10, 25, 40, 55, 69], "dump": [10, 25, 40, 55, 69], "events": []})

    # An orbiting viewpoint: the planes follow the view one frame late (ProximitySensor).
    w = World(); v = w.tex3d("lum")
    node = (
        f"DEF VD VolumeData {{ {D} voxels {v} renderStyle EdgeEnhancementVolumeStyle {{ }} }}\n"
        "DEF T TimeSensor { cycleInterval 1.2 loop TRUE }\n"
        "DEF O OrientationInterpolator { key [ 0 0.5 1 ] keyValue [ 0 1 0 0, 0 1 0 3.1416, 0 1 0 6.2832 ] }\n"
        "DEF P PositionInterpolator { key [ 0 0.25 0.5 0.75 1 ] keyValue [ 0 0 4.5, 4.5 0 0, 0 0 -4.5, -4.5 0 0, 0 0 4.5 ] }\n"
        "DEF VP2 Viewpoint { position 0 0 4.5 }\n"
    )
    routes = "ROUTE T.fraction_changed TO O.set_fraction\nROUTE T.fraction_changed TO P.set_fraction\nROUTE O.value_changed TO VP2.set_orientation\nROUTE P.value_changed TO VP2.set_position\n"
    STEPPED["rt-orbit"] = (world(node + routes + "DEF K KeySensor { }\n", "front").replace("Viewpoint { position 0 0 4.5 }\nDEF", "DEF"), {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 100, "capture": [8, 14, 20, 26, 32, 40, 50, 60, 75, 90], "dump": [8, 40, 90], "events": []})


rt_route()


# TextureQuality (a browser option the Script sets) changes the number of planes: 200 / 400 / 600.
def rt_quality():
    w = World(); v = w.tex3d("lum")
    node = f"DEF VD IsoSurfaceVolumeData {{ {D} surfaceValues [ 0.4 ] voxels {v} }}\nDEF VD2 VolumeData {{ dimensions 1.2 1.2 1.2 voxels USE T_LUM renderStyle CartoonVolumeStyle {{ }} }}\n"
    body = """
   if (k == 'a') Browser .setBrowserOption ('TextureQuality', 'LOW');
   if (k == 'b') Browser .setBrowserOption ('TextureQuality', 'HIGH');
   if (k == 'c') Browser .setBrowserOption ('TextureQuality', 'MEDIUM');
   if (k == 'd') Browser .setBrowserOption ('QualityWhenMoving', 'LOW');
   if (k == 'e') Browser .setBrowserOption ('TextureQuality', 'LOW');"""
    rt_world("rt-quality", node, ["VD", "VD2"], body, list("abcde"), gap=12)


rt_quality()


# Inline: reload by load FALSE / TRUE, and by changing the url.
def rt_inline():
    for name, style in (("a", "CartoonVolumeStyle { colorSteps 3 }"), ("b", "BoundaryEnhancementVolumeStyle { }")):
        w = World(); v = w.tex3d("lum")
        text = f"#X3D V4.1 utf8\nPROFILE Full\nCOMPONENT Texturing3D:2\nCOMPONENT VolumeRendering:4\nVolumeData {{ {D} renderStyle {style} voxels {v} }}\n"
        with open(os.path.join(out, f"inline-{name}.x3dv"), "w") as f:
            f.write(text)
    node = "DEF IN Inline { url \"inline-a.x3dv\" }\n"
    body = """
   if (k == 'a') in_ .load = false;
   if (k == 'b') in_ .load = true;
   if (k == 'c') in_ .url = new MFString ('inline-b.x3dv');
   if (k == 'd') in_ .url = new MFString ('inline-a.x3dv');
   if (k == 'e') in_ .load = false;
   if (k == 'f') in_ .url = new MFString ('inline-b.x3dv');
   if (k == 'g') in_ .load = true;"""
    text = world(node + "DEF K KeySensor { }\nDEF R Script {\n directOutput TRUE\n inputOnly SFString on_key\n initializeOnly SFNode in_ USE IN\n url \"javascript:\nfunction on_key (k)\n{\n" + body.replace("in_ ", "in_ ") + "\n}\n\"\n}\nROUTE K.keyPress TO R.on_key\n", "obl")
    text = text.replace("initializeOnly SFNode in_ USE IN", "initializeOnly SFNode in_ USE IN")
    keys = list("abcdefg")
    events, capture, f = [], [], 16
    for k in keys:
        events.append({"frame": f, "type": "key", "key": k}); capture.append(f + 30); f += 40
    capture = [12] + capture
    STEPPED["rt-inline"] = (text, {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": f + 2, "capture": capture, "dump": capture, "events": events})


rt_inline()

# --- write the static worlds ---------------------------------------------------------------------

for name, text in CASES.items():
    with open(os.path.join(out_static, name + ".wrl"), "w") as f:
        f.write(text)

out_unverifiable = os.path.join(root, "oracle", "synthetic-rea-b07-vol", "unverifiable")
os.makedirs(out_unverifiable, exist_ok=True)

for name, body in UNVERIFIABLE.items():
    w = World()
    with open(os.path.join(out_unverifiable, name + ".wrl"), "w") as f:
        f.write(world(body(w), "obl"))

for name, (text, script) in STEPPED.items():
    with open(os.path.join(out, name + ".wrl"), "w") as f:
        f.write(text)
    with open(os.path.join(out, name + ".json"), "w") as f:
        json.dump(script, f, indent=1)

print(len(CASES), "static worlds,", len(STEPPED), "stepped worlds ->", out)
