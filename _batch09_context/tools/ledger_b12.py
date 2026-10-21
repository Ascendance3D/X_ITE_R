#!/usr/bin/env python3
"""Batch 12 ledger update: python3 -I ledger_b12.py check|apply

Starts from ledger/before-b12/ledger.json (the authoritative ledger at f9b490d: 248 COMPLETE, 71 PARTIAL, 25 DIFFERENT, 2 MISSING).
Only the entries of C change. node:OrthoViewpoint is promoted; node:Text, sub:fields.math and sub:browser.sai-api keep PARTIAL
with updated evidence and a narrower missing list; the COMPLETE entries whose code changed gain the new evidence."""
import json, os, sys, collections
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-b12", "ledger.json")
B12 = "crates/x3d-viewer/tests/rea_batch12.rs::rea_batch12_cases_match_x_ite"
VW = B12 + " (vw-ortho, vw-persp, vw-geo, vw-layers, MSAA 0/4, 3 identical X_ITE recordings each)"
BB = B12 + " (bbb-static, bbb-camera, bbb-dynamic, bbb-shadow, MSAA 0/4, 3 identical X_ITE recordings each)"
GEOM = "crates/x3d-render/tests/geometry_rea_b12.rs::rea_b12_triangulation_matches_x_ite (oracle/synthetic-rea-b12-geom, 9 cases)"
WARM = "crates/x3d-viewer/tests/rea_batch12_text_warmup.rs::rea_batch12_text_first_build_has_no_font"
NOTE = "Batch 12 2026-10-21: "
PK = B12 + " (pk-points, pk-overlap, pk-events exact; pk-points-style and pk-lines KNOWN_GAPS; MSAA 0/4, 3 identical X_ITE recordings each; pickProbes bit for bit)"
NAV = "crates/x3d-viewer/tests/rea_batch09_nav.rs (geo-touch, touch-hit-events, pick-hits, pick-instanced; bit-exact pickProbes)"
C = {
 "node:OrthoViewpoint": ("COMPLETE", [VW], "",
   NOTE + "PASS. Browser.viewAll in every argument form (no argument, a time, 0, a Layer node, a non-layer node, junk), the "
   "StraightenHorizon option on and off (setting it TRUE starts X3DLayerNode.straightenView), run-time orientation and parent "
   "Transform changes, LookAt on a shape and with Alt, for OrthoViewpoint, Viewpoint, GeoViewpoint and a two-layer LayerSet: "
   "per-frame poses (1e-4), ProximitySensor position / orientation / centerOfRotation events and isBound / bindTime stamped per "
   "frame, images. Rust defects fixed: Browser.viewAll was a no-op; the StraightenHorizon option did not straighten; Browser "
   "requests from key and pointer events waited one frame; viewAll / LookAt used a per-shape axis-aligned box where X_ITE nests "
   "oriented Box3 boxes (X3DLayerNode.getBBox)."),
 "node:Viewpoint": (None, [VW], None, NOTE + "Browser.viewAll, StraightenHorizon and LookAt variants match X_ITE (vw-persp)."),
 "node:GeoViewpoint": (None, [VW], None, NOTE + "Browser.viewAll, StraightenHorizon and LookAt variants match X_ITE (vw-geo)."),
 "node:LayerSet": (None, [VW], None, NOTE + "Browser.viewAll on a Layer node and on the active layer matches X_ITE (vw-layers)."),
 "node:Billboard": (None, [BB], None,
   NOTE + "bboxDisplay (all axisOfRotation kinds, transformed parents, empty, explicit bboxSize, nested, under a Group), camera "
   "moves, run-time changes and the SHADOW pass match X_ITE. Rust defect fixed: Billboard.getBBox and the SHADOW pass used no "
   "rotation; X_ITE keeps the rotation of the last DISPLAY traversal (Billboard.matrix). A negative check (identity matrix) fails "
   "bbb-static, bbb-camera and bbb-shadow."),
 "node:Extrusion": (None, [GEOM], None,
   NOTE + "Rust defect fixed: non-convex caps used ear clipping; X_ITE uses Triangle3.triangulatePolygon (libtess, odd winding). "
   "Concave, clockwise, star, self-intersecting and open caps now match element by element; the old code fails 5 of the 9 cases."),
 "node:IndexedFaceSet": (None, [GEOM], None, NOTE + "convex FALSE polygons (concave, non-planar, self-intersecting, collinear, texture coordinates) match X_ITE."),
 "sub:fields.math": (None, [GEOM], "toString methods of the math classes are not ported",
   NOTE + "Triangle3.triangulatePolygon is now compared directly through convex FALSE IndexedFaceSet and Extrusion caps."),
 "node:Text": (None, [WARM], None,
   NOTE + "HOLD stays. Rust defect fixed: the first build had the font already (the default FontStyle and a world FontStyle "
   "loaded before the first frame); X_ITE's FontStyle load starts with `await $.sleep (0)` and the world does not wait for it, "
   "so the first build at frame 1 has no font in every X_ITE run (42 of 42). Rust now takes the same steps. The later warm-up "
   "frames still differ per X_ITE run (oracle/rea-batch12-baseline/text-warmup/patterns.txt); the arrival frame is not chosen."),
 "node:GeoTouchSensor": ("COMPLETE", [NAV, PK], "",
   NOTE + "PASS. The two inherited gaps of the old missing text are closed: the hit values come from the GPU pointing pass and "
   "are bit-exact (since batch 09), and the hit events re-sent on a press and a release at the same position match X_ITE "
   "(geo-touch, touch-hit-events, pk-events: press and release without movement, repeated presses, nested and stacked sensors, "
   "a drag off the shape, exact event order). hitGeoCoord_changed in GD and UTM with and without a GeoOrigin matches (geo-touch). "
   "The stale test note 'geo-touch.wrl (FAILS, kept as is)' predates batch 09: the case runs in rea_batch09_nav and passes. "
   "A GeoTouchSensor over lines or PointProperties points inherits the sub:pointer.picking ulp gaps listed there."),
 "sub:pointer.picking": (None, [NAV, PK],
   "lines: every probe hits the same line, but the hit point differs from X_ITE by up to 63 float32 ulps on aliased lines and up "
   "to 398 ulps on transformed thick lines (width above 10; X_ITE makes those triangles by transform feedback in float32, Rust on "
   "the CPU in double) (pk-lines, KNOWN_GAP); PointProperties points: gl_PointCoord (hit texCoord) differs by 1 to 3 ulps at "
   "some pixels (pk-points-style, KNOWN_GAP); an appearance's DepthMode is not applied in the pointing pass; touch input "
   "(touchstart / touchend) is not driven by the harness",
   NOTE + "Rust defects fixed: points and lines were never picked (the ray cast skipped geometry types 0 and 1); a point or line "
   "in front of a shape did not hide it; the pointing pass drew only the ray cast's nearest shape, so coincident faces went to "
   "the wrong shape (X_ITE draws every pointing shape in order into DEPTH_COMPONENT24 with LEQUAL: the last drawn wins a tie); "
   "fractional scripted mouse positions reached the viewer unrounded (Chrome gives MouseEvent pageX / pageY whole pixels, cut "
   "toward zero; X_ITE reads those). Measured on the oracle GPU and reproduced: gl_PointSize in 1/32 px steps rounded down, "
   "point centres on a 1/256 px grid, GL line varyings at the fragment's major-axis coordinate. pk-points (plain, colored, "
   "transformed, with normals, ClipPlane), pk-overlap (coplanar boxes, points on a face, USE at two depths, a ring with a hole, "
   "ClipPlane) and pk-events (TouchSensor order) are exact; X_ITE's picks are identical at MSAA 0 and 4. The old missing text "
   "(~1e-5 values, no re-sent hit events) was closed in batch 09 (GPU pointing pass, touch-hit-events)."),
 "node:PointSet": (None, [PK], None,
   NOTE + "Rust defect fixed: the Normal node was not read (PointSet.build: normalNode.addVectors); hit normals now match "
   "(pk-points). Points are picked like X_ITE (pk-points exact; PointProperties texCoord ulps in pk-points-style). The visual "
   "evidence item stays open."),
 "node:Normal": (None, [PK], None, NOTE + "PointSet and IndexedLineSet now read their Normal node (pk-points, pk-lines normals)."),
 "node:IndexedLineSet": (None, [PK], None,
   NOTE + "Rust defect fixed: the Normal node was not read (IndexedLineSet.build adds normalNode.addVector (coordIndex) per "
   "vertex, normalIndex and normalPerVertex are not read); lines are picked (pk-lines: same line at every probe, hit point "
   "ulps listed under sub:pointer.picking)."),
 "node:PointProperties": (None, [PK], None,
   NOTE + "The pointing pass uses it like X_ITE: gl_PointSize = max (getPointSize, 2), the round point mask without textures, "
   "gl_PointCoord texCoord (pk-points-style: ids, points and normals exact; texCoord 1 to 3 ulps at some pixels, KNOWN_GAP)."),
 "node:LineProperties": (None, [PK], None,
   NOTE + "The pointing pass uses the aliased width (gl.lineWidth) and the transformed thick-line triangles above width 10 "
   "(pk-lines: same line at every probe; hit point ulps under sub:pointer.picking)."),
 "node:ClipPlane": (None, [PK], None, NOTE + "Clip planes discard point and line fragments in the pointing pass (pk-points exact, pk-lines)."),
 "sub:browser.sai-api": (None, [
   "crates/x3d-viewer/tests/rea_batch09_sai.rs (scr-browser, brx-api, brx-call, sai-create, sai-scenes, bind-browser)", VW],
   "getBrowserProperty / getRenderingProperty only for a missing name and Initialized; currentSpeed / currentFrameRate only in a "
   "frozen scene; beginUpdate / endUpdate only as calls (that rendering stops is not compared)",
   NOTE + "Missing list reviewed against the batch 09 SAI worlds: createX3DFromString/URL, loadURL, replaceWorld, importDocument, "
   "importJS, browser options, changeViewpoint and addBrowserCallback have exact evidence there; Browser.viewAll is now ported "
   "and compared (vw-*)."),
}
def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, tests, missing, note = C[i["id"]]
            if s: i["status"] = s
            if missing is not None: i["missing"] = missing
            i["tests"] = i["tests"] + tests
            i["notes"] = (i["notes"] + " " if i["notes"] else "") + note
    return d
def check(d):
    bad, snap = [], {i["id"]: i for i in load(SNAP)["items"]}
    cur = {i["id"]: i for i in d["items"]}
    if set(cur) != set(snap) or len(d["items"]) != 346: bad.append("ID set or total changed")
    for k, b in snap.items():
        a = cur[k]
        if k not in C and a != b: bad.append(f"unrelated entry changed: {k}")
        if b["status"] == "DIFFERENT" and a != b: bad.append(f"DIFFERENT changed: {k}")
        if k in C and (a["status"] != (C[k][0] or b["status"]) or not a["notes"].startswith(b["notes"])): bad.append(f"{k}: status or notes")
    c = collections.Counter(i["status"] for i in d["items"]); print(dict(c))
    print("\n".join(bad) or "OK"); return not bad
if __name__ == "__main__":
    if sys.argv[1] == "apply":
        d = build(); json.dump(d, open(AUTH, "w"), indent=1); sys.exit(0 if check(load(AUTH)) else 1)
    sys.exit(0 if check(load(AUTH)) else 1)
