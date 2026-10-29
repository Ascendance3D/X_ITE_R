#!/usr/bin/env python3
"""Issue #90 ledger update (item-level desktop GPU visual comparison): python3 -I ledger_i90.py check|apply

Starts from ledger/before-i90/ledger.json (the authoritative ledger at origin/master af312ac, sha256 41cd5e2d...66ef:
249 COMPLETE, 71 PARTIAL, 25 DIFFERENT, 1 MISSING). The 18 PARTIAL entries of issue #90 had one open item each, "visual
evidence (rendering) not compared: needs the desktop GPU run". Each gains its still-image comparison against pinned X_ITE
16.5.0 (oracle/x_ite-pin, 34cdf9f97) in headless Chrome on the RTX 3060 Ti, at MSAA 0 and 4
(crates/x3d-render/tests/render_rea_b09_visual.rs, bound: at most 0.5% of the pixels more than 8 off).

Every reference was recorded three times with identical RGBA (index-items.json beside the baselines; the older vis-*
references were re-recorded three times to /tmp and reproduced byte for byte, not overwritten).

14 entries become COMPLETE: the comparison passes and no other item was open. 4 stay PARTIAL with the visual item replaced
by what the comparison found or cannot show: Appearance (alphaMode / alphaCutoff not applied: vis-items-alpha is a known gap),
MovieTexture (frames not decoded: vis-items-movie is a known gap), AudioClip and Sound (they draw nothing in X_ITE or Rust,
vis-items-sound matches; audio output has no oracle). No DIFFERENT entry is in the change set. Earlier notes stay as a
prefix. Result: 263 COMPLETE, 57 PARTIAL, 25 DIFFERENT, 1 MISSING."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i90", "ledger.json")
SNAP_SHA256 = "41cd5e2d319e54762dbbd2d5675cccdc50cb2ef52ab958cf5459bff9be0466ef"  # ledger.json at origin/master af312ac
NOTE = "Issue #90 2026-10-29 (desktop GPU run, RTX 3060 Ti): "
TEST = "crates/x3d-render/tests/render_rea_b09_visual.rs"
GEN = "oracle/tools/gen_rea_b09_visual_items.py"
PIN = "pinned X_ITE 16.5.0 (oracle/x_ite-pin, 34cdf9f97), headless Chrome 156 / ANGLE on the RTX 3060 Ti, three identical recordings"
OPEN = "visual evidence (rendering) not compared: needs the desktop GPU run"

def worlds(*w):
    return f"{TEST} " + ", ".join(w) + " (oracle/synthetic-rea-b09-visual; MSAA 0 and 4)"

def oracle(*w):
    return "oracle/render-rea-b09-visual-baseline and oracle/render-msaa4-rea-b09-visual-baseline: " + ", ".join(w) + " (" + PIN + ")"

def result(text):
    return NOTE + text

def drop_open(old):
    if old != OPEN: raise SystemExit(f"unexpected missing text: {old!r}")
    return ""

def replace_open(new):
    return lambda old: (drop_open(old), new)[1]

def add(v):
    return lambda old: old + [v]

def add_oracle(v):
    return lambda old: (old + "; " if old else "") + v

def entry(status, ws, missing, note):
    return (status, {"tests": add(worlds(*ws)), "oracle": add_oracle(oracle(*ws)), "missing": missing}, result(note))

ZERO = "0 pixels over 8 at MSAA 0 and 4"
# id: (status or None, {field: value or function of the old value}, note). The verifier reads the first element.
C = {
 "node:EspduTransform": entry("COMPLETE", ["vis-items-espdu"], drop_open,
    f"vis-items-espdu {ZERO}. Rust defect fixed: EspduTransform drew nothing ('not rendered'); it is an X3DGroupingNode "
    "(DIS/EspduTransform.js, no getMatrix or traverse override), so its children are drawn like a Group's and its translation, "
    "rotation, scale and center are DIS state, not a matrix (crates/x3d-render/src/traverse.rs GROUPS, sg.rs static shapes, "
    "crates/x3d-viewer/src/walk.rs GROUPS). Cells: defaults, translation, rotation + scale, center, enabled FALSE, "
    "linearVelocity + deadReckoning, nested Transform, visible FALSE, a scaled parent."),
 "node:HAnimJoint": entry("COMPLETE", ["vis-items-hanim", "vis-items-hanim-seg"], drop_open,
    f"vis-items-hanim and vis-items-hanim-seg {ZERO}: center, rotation, translation, scale, scaleOrientation, nested joints, "
    "skinCoordIndex / skinCoordWeight skinning of two joints, a joint displacer, the humanoid's own transform."),
 "node:HAnimSegment": entry("COMPLETE", ["vis-items-hanim", "vis-items-hanim-seg"], drop_open,
    f"vis-items-hanim and vis-items-hanim-seg {ZERO}: children, bboxCenter / bboxSize, and the segment's coord with displacers "
    "(none, one, two, weight 0.5 and 0, a CoordinateDouble coord). Rust defect fixed: a CoordinateDouble segment coord (or "
    "humanoid skinCoord) had length 0, because its MFVec3d point was read as MFVec3f, so no displacement was applied "
    "(crates/x3d-render/src/hanim.rs coord_length)."),
 "node:HAnimSite": entry("COMPLETE", ["vis-items-hanim"], drop_open,
    f"vis-items-hanim {ZERO}: a site under a joint (translation, rotation) and a site directly in the skeleton (scale)."),
 "node:LayoutGroup": entry("COMPLETE", ["vis-layout"], drop_open,
    f"vis-layout {ZERO}: align, offsetUnits, sizeUnits, scaleMode PIXEL / FRACTION / STRETCH, nested content, a Viewport; "
    "with the b02 stills (layout-group-root, layout-nested) and the stepped b02 cases."),
 "node:DirectionalLight": entry("COMPLETE", ["vis-light-dir", "vis-items-lights"], drop_open,
    f"vis-light-dir and vis-items-lights {ZERO} (max channel difference 1): direction, color, intensity 0.4 / 2, "
    "ambientIntensity, on FALSE, global TRUE / FALSE, two and nine lights on one shape, a light under a rotated Transform, "
    "mixed with Point and Spot lights. Shadows and scope: sub:render.shadows and oracle/synthetic-rea-b03 (batch 03)."),
 "node:PointLight": entry("COMPLETE", ["vis-light-point", "vis-items-lights"], drop_open,
    f"vis-light-point and vis-items-lights {ZERO} (max channel difference 1): location, radius cut-off, attenuation, intensity "
    "0.4 / 2 / 3, color, ambientIntensity, on FALSE, a light behind the plane, global TRUE / FALSE, a light under a translated "
    "and scaled Transform."),
 "node:SpotLight": entry("COMPLETE", ["vis-light-spot", "vis-items-lights"], drop_open,
    "vis-items-lights 0 pixels over 8 at MSAA 0 and 4; vis-light-spot 2 pixels over 8 (0.001%, bound 0.5%) at MSAA 0 and 4, "
    "max 131: two isolated pixels of the cell beamWidth 1.2 > cutOffAngle 0.5, both at the hard cone edge (angle 0.493 and "
    "0.505 rad from the axis), where X_ITE draws gray and Rust white; a float rounding of the cut-off test, not a field "
    "difference. Covered: location, direction, radius, beamWidth below / above cutOffAngle, attenuation, intensity, color, "
    "ambientIntensity, on FALSE, global TRUE / FALSE, a light under a rotated Transform, mixed light types."),
 "node:CoordinateDouble": entry("COMPLETE", ["vis-items-coord", "vis-rendering"], drop_open,
    "vis-items-coord 46 pixels over 8 at MSAA 0 (0.018%), 0 at MSAA 4; consumers IndexedLineSet, PointSet, TriangleSet, "
    "IndexedTriangleSet, IndexedFaceSet (textured, creaseAngle), IndexedQuadSet; HAnimSegment coord in vis-items-hanim-seg."),
 "node:Normal": entry("COMPLETE", ["vis-items-coord", "vis-rendering"], drop_open,
    "vis-items-coord 46 pixels over 8 at MSAA 0 (0.018%), 0 at MSAA 4: per-vertex and per-face normals on TriangleSet, "
    "IndexedTriangleSet, ElevationGrid, IndexedFaceSet with normalIndex, IndexedQuadSet."),
 "node:PointSet": entry("COMPLETE", ["vis-items-pointset", "vis-items-coord", "vis-rendering"], drop_open,
    "vis-items-pointset 56 pixels over 8 at MSAA 0 (0.021%), 0 at MSAA 4: emissive, Color, ColorRGBA, lit Material, "
    "transparency, a scaled parent, LineProperties / FillProperties (ignored), empty coord and no coord, a texture (ignored)."),
 "node:Appearance": entry(None, ["vis-items-appearance", "vis-items-alpha"],
    replace_open("alphaMode and alphaCutoff are not applied by the renderer: vis-items-alpha is a KNOWN_GAPS world "
                 "(OPAQUE still blends the texture and material alpha; MASK neither cuts at alphaCutoff nor draws the kept "
                 "pixels opaque)"),
    "vis-items-appearance 10 pixels over 8 at MSAA 0 (0.004%), 0 at MSAA 4: texture alone, with Material, with emissive, "
    "TextureTransform, LineProperties (applied, linetype, width), FillProperties (filled, hatched, hatchStyle, hatchColor). "
    "vis-items-alpha (alphaMode AUTO / OPAQUE / MASK + alphaCutoff / BLEND) fails: 59894 pixels over 8 at MSAA 0, so the "
    "entry stays PARTIAL."),
 "node:Shape": entry("COMPLETE", ["vis-items-shape", "vis-items-shape-bbox", "vis-shape"], drop_open,
    "vis-items-shape 2 pixels over 8 at MSAA 0 (0.001%), 0 at MSAA 4; vis-items-shape-bbox 0. Rust defect fixed: a non-default "
    "bboxSize is the shape's bbox in X3DShapeNode.set_bbox__, and addDisplayShape culls a shape whose box is outside the view "
    "volume and sorts by its center; the renderer drew it anyway (crates/x3d-render/src/traverse.rs shape, declared_bbox). "
    "Cells: no geometry, visible FALSE, bboxDisplay, no appearance, mirrored, empty Appearance, Switch, LOD, two shapes, "
    "transparency, emissive."),
 "node:AudioClip": entry(None, ["vis-items-sound"],
    replace_open("audio output (playback, pitch, loop, timing) is not compared: there is no audio oracle"),
    "vis-items-sound 0 pixels over 8 at MSAA 0 and 4: an AudioClip source draws nothing and leaves its siblings unchanged."),
 "node:Sound": entry(None, ["vis-items-sound"],
    replace_open("audio output (intensity, spatialize, ellipsoid attenuation, direction) is not compared: there is no audio "
                 "oracle"),
    "vis-items-sound 0 pixels over 8 at MSAA 0 and 4: Sound draws nothing (with and without source, under a scaled Transform)."),
 "node:ImageTexture": entry("COMPLETE", ["vis-items-textures", "vis-items-appearance", "vis-texturing"], drop_open,
    f"vis-items-textures {ZERO}: Cylinder, Cone, ElevationGrid, Extrusion mapping, url fallback after a missing file, "
    "modulation with Material, an alpha texture with repeatS / repeatT FALSE."),
 "node:MovieTexture": entry(None, ["vis-items-movie"],
    replace_open("frames are not decoded: vis-items-movie is a KNOWN_GAPS world (animated GIF, APNG and WebM urls draw "
                 "untextured white; x3d-shot: 'not rendered: MovieTexture')"),
    "vis-items-movie 78930 pixels over 8 at MSAA 0 (30.1%), 79936 at MSAA 4: stays PARTIAL."),
 "node:PixelTexture": entry("COMPLETE", ["vis-items-textures", "vis-items-pointset", "vis-texturing"], drop_open,
    f"vis-items-textures {ZERO}: RGB 2x2 on Box, Sphere, Cylinder, repeatS / repeatT FALSE outside 0..1, a 4x4 intensity "
    "image modulated by Material."),
}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, note = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            i["notes"] = (i["notes"] + " | " if i["notes"] else "") + note
    return d
def check(d):
    bad, snap = [], {i["id"]: i for i in load(SNAP)["items"]}
    cur = {i["id"]: i for i in d["items"]}
    if set(cur) != set(snap) or len(d["items"]) != 346: bad.append("ID set or total changed")
    if [i["id"] for i in d["items"]] != [i["id"] for i in load(SNAP)["items"]]: bad.append("ID order changed")
    want = {i["id"]: i for i in build()["items"]}
    for k, b in snap.items():
        a = cur[k]
        if k not in C and a != b: bad.append(f"unrelated entry changed: {k}")
        if b["status"] == "DIFFERENT" and a != b: bad.append(f"DIFFERENT changed: {k}")
        if k in C and (a["status"] != (C[k][0] or b["status"]) or not a["notes"].startswith(b["notes"])): bad.append(f"{k}: status or notes")
        if k in C and a != want[k]: bad.append(f"{k}: differs from what apply writes")
    c = collections.Counter(i["status"] for i in d["items"]); print(dict(sorted(c.items())))
    print("\n".join(bad) or "OK"); return not bad
def sha256(p):
    with open(p, "rb") as f: return hashlib.sha256(f.read()).hexdigest()
def snap_ok():
    got = sha256(SNAP)
    if got != SNAP_SHA256: print(f"snapshot sha256 {got} is not the expected {SNAP_SHA256}: refused"); return False
    return True
if __name__ == "__main__":
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i90.py check|apply")
    if not snap_ok(): sys.exit(1)
    if sys.argv[1:] == ["apply"]:
        d = build()
        try: same = sha256(AUTH) == SNAP_SHA256 or load(AUTH) == d
        except ValueError: same = False
        if not same:
            print("ledger.json is neither the snapshot nor the result of this change set: refused"); sys.exit(1)
        # Write a temporary file in the same directory, then replace: an interrupted write leaves ledger.json whole.
        tmp = AUTH + ".tmp"
        with open(tmp, "w") as f:
            json.dump(d, f, indent=1); f.flush(); os.fsync(f.fileno())
        os.replace(tmp, AUTH); sys.exit(0 if check(load(AUTH)) else 1)
    sys.exit(0 if check(load(AUTH)) else 1)
