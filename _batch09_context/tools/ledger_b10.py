#!/usr/bin/env python3
"""Batch 10 ledger update: python3 -I ledger_b10.py check|apply

Starts from ledger/before-b10/ledger.json (the authoritative ledger at e17ee7d: 245 COMPLETE, 74 PARTIAL, 25 DIFFERENT, 2 MISSING),
so the ten Batch 09 promotions are kept. `apply` rebuilds the three reviewed entries from the snapshot and writes ledger.json.
Only the entries of C change; node:ScreenFontStyle is promoted, node:Text and node:ScreenGroup keep PARTIAL (HOLD) with their notes
and `missing` text updated."""
import json, os, sys, collections
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-b10", "ledger.json")
SCR = "crates/x3d-viewer/tests/rea_batch10_screen.rs::rea_batch10_screen_cases_match_x_ite"
SAI = "crates/x3d-viewer/tests/rea_batch10_sai.rs::rea_batch10_sai_cases_match_x_ite"
VIS = "crates/x3d-render/tests/render_rea_b09_visual.rs::rea_b09_visual_renders_match_x_ite"
NOTE = "Batch 10 review 2026-10-20: "
C = {
 "node:ScreenFontStyle": ("COMPLETE", [VIS + " (vis-screen, vis-screen-text, vis-screen-view, vis-screen-ortho, MSAA 0/4)", SCR + " (sfs-runtime, sfs-plain, sg-resize, sg-camera: FontStyle fields, camera / viewpoint changes, ContentScale 0.5 / 1 / 2, resize, MSAA 0/4)", SAI + " (text-screen-scale: origin / textBounds / lineBounds events through ContentScale changes)"], "",
   NOTE + "PASS. family / style / size / spacing / justify / direction / length / maxExtent (vis-screen-text), parent transforms and camera types (vis-screen-view, vis-screen-ortho, sg-camera), run-time field changes (sfs-runtime), Text without a ScreenGroup (sfs-plain), ContentScale 0.5 / 1 / 2 and canvas resize (sfs-runtime, sfs-plain, sg-resize), output events (text-screen, text-screen-scale) all against pinned X_ITE (images recorded twice, events three times, identical; 0.5% limit unchanged). Rust defects fixed: ContentScale changes did not rebuild the Text geometry (cache key, rebuild event), frames were built at the canvas size not the drawing-buffer size. Browser-dependent: a ContentScale change in the frame a font file arrives delays that arrival one frame in X_ITE (not modelled; the cases avoid the coincidence)."),
 "node:Text": ("PARTIAL", [SCR + " (text-runtime: FontStyle / Text field changes, family fallback, MSAA 0/4, settle)", SAI + " (text-screen-scale)"], "origin / textBounds / lineBounds during the oracle's warm-up frames (node-text-outputs-open): X_ITE itself is not repeatable there (one or two default-font rebuilds, depending on the machine's load; 3 quiet runs gave one pair, a fourth quiet run two, runs under CPU load one pair and a later load frame), so it cannot be exact evidence",
   NOTE + "HOLD. Rendering (vis-text-font, vis-text-layout), run-time FontStyle and Text changes with pictures (text-runtime), events (text-events, text-fontstyle, text-family, text-screen-scale) are exact. The warm-up frames stay open because the oracle is not deterministic there."),
 "node:ScreenGroup": ("PARTIAL", [SCR + " (sg-runtime, sg-viewpoint, sg-resize, sg-camera, MSAA 0/4; sg-bbox and sg-viewpoint-degenerate are KNOWN_GAPS)"], "bboxDisplay of a ScreenGroup is not drawn (the renderer draws bboxDisplay only for the DIS PDU nodes; the Group / Transform bboxDisplay lane is unscheduled); a Viewpoint at the ScreenGroup's origin (X_ITE's matrix feedback goes singular and draws black)",
   NOTE + "HOLD. Verified against X_ITE: screen-space scaling, translation / rotation / scale of parents, child geometry changes, nesting, Viewpoint and OrthoViewpoint and GeoViewpoint, Viewpoints inside (also nested) a ScreenGroup (poses and pictures), ContentScale 0.5 / 1 / 2, canvas resize, visible. Rust defects fixed: the viewer did not walk ScreenGroup children (Viewpoints never bound), the previous frame's screen matrix did not feed the CAMERA pass, frames were not built at drawing-buffer size. Open: bboxDisplay."),
}
def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, tests, missing, note = C[i["id"]]
            i["status"], i["missing"] = s, missing
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
        if k in C and (a["status"] != C[k][0] or not a["notes"].startswith(b["notes"])): bad.append(f"{k}: status or notes")
    c = collections.Counter(i["status"] for i in d["items"]); print(dict(c))
    print("\n".join(bad) or "OK"); return not bad
if __name__ == "__main__":
    if sys.argv[1] == "apply":
        d = build(); json.dump(d, open(AUTH, "w"), indent=1); sys.exit(0 if check(load(AUTH)) else 1)
    sys.exit(0 if check(load(AUTH)) else 1)
