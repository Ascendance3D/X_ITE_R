#!/usr/bin/env python3
"""Batch 11 ledger update: python3 -I ledger_b11.py check|apply

Starts from ledger/before-b11/ledger.json (the authoritative ledger at ddb2c4e: 246 COMPLETE, 73 PARTIAL, 25 DIFFERENT, 2 MISSING).
Only the entries of C change. node:LocalFog and node:ScreenGroup are promoted; node:OrthoViewpoint and node:Text keep PARTIAL with
updated evidence; node:Group and node:Transform stay COMPLETE and gain the bboxDisplay evidence."""
import json, os, sys, collections
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-b11", "ledger.json")
SCR = "crates/x3d-viewer/tests/rea_batch10_screen.rs::rea_batch10_screen_cases_match_x_ite"
BB = "crates/x3d-viewer/tests/rea_batch11_bbox.rs (bb-static, bb-others, bb-screen, bb-dynamic, MSAA 0/4)"
TXT = "crates/x3d-viewer/tests/rea_batch11_text.rs (tb-fallback, tb-layout, tb-align, tb-late)"
NOTE = "Batch 11 2026-10-21: "
C = {
 "node:LocalFog": ("COMPLETE", [SCR + " (lf-geom, lf-vol-types, lf-vol-global, lf-volume: color, visibilityRange, visibilityStart, fogType, enabled, nesting, VolumeData / IsoSurfaceVolumeData / SegmentedVolumeData, bound Fog, MSAA 0/4)"], "",
   NOTE + "PASS. Volume draws now use the render context's fog node (innermost LocalFog, else the layer's Fog) like other shapes; images against pinned X_ITE (recorded twice, identical; 0.5% limit unchanged), no KNOWN_GAPS left."),
 "node:ScreenGroup": ("COMPLETE", [BB, SCR + " (sg-bbox, sg-viewpoint-degenerate, MSAA 0/4; no KNOWN_GAPS left)"], "",
   NOTE + "PASS. bboxDisplay (default and explicit bbox, nested, rotated parent, empty, Text, perspective and ortho, ContentScale 2) matches X_ITE. A Viewpoint at the ScreenGroup's origin: X_ITE's screen-matrix feedback makes the camera matrix zero and the Background rotation NaN (traced per traversal with an instrumented copy of the pin, 2 identical runs); Background now takes the rotation with Matrix4.factor as getTransform does, so Rust draws the same black frames from the same cause."),
 "node:OrthoViewpoint": ("PARTIAL", [SCR + " (ov-view, MSAA 0/4)", SCR + " (ov-view2, ov-lookat, MSAA 0/4)"], "pose events (position_changed and similar) recorded three times; Browser.viewAll and layer-level viewAll through the SAI; the StraightenHorizon option; Geo / perspective LookAt variants",
   NOTE + "HOLD. Rendering and per-frame poses for viewAll (scaled and rotated parents, resize, fieldOfView change, retainUserOffsets, empty scene) and LookAt match X_ITE. Rust defect fixed: a run-time viewAll = true on a bound viewpoint did not send set_bind (X3DViewpointNode.set_viewAll__)."),
 "node:Text": (None, [TXT], None,
   NOTE + "HOLD stays. Run-time changes, family fallback, layout inputs, stale alignments and late-made Texts now have exact evidence (3 identical X_ITE runs each). Rust defects fixed: FontStyle alignments kept from setup / last justify change; font-file cache keyed by resolved file. Warm-up frames: 30 X_ITE runs gave several patterns (first build at frame 1 empty, font arrives frame 2-5, 1-3 rebuilds), so no exact evidence is possible."),
 "node:Group": (None, [BB], None, NOTE + "bboxDisplay matches X_ITE (bb-static, bb-others, bb-dynamic). Billboard's own bboxDisplay is not covered."),
 "node:Transform": (None, [BB], None, NOTE + "bboxDisplay matches X_ITE (bb-static, bb-dynamic)."),
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
