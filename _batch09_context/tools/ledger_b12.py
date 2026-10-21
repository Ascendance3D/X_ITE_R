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
