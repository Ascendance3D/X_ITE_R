#!/usr/bin/env python3
"""Ledger step for PRs #474, #475 and #476: python3 -I ledger_r474_476.py check|apply

Three text-only changes.
- sub:nav.walk-fly: PR #474 compares Viewport nodes for the rubber band (9 cases at MSAA 0 and 4) and the ContentScale and
  browser-option cases at MSAA 4 (4 cases). F16 (touch in WALK and FLY) stays open in x3d-viewer.
- sub:nav.examine: PR #475 compares E14 (touch rotate and pinch, 7 cases) and E18 (X10 stress, 28 cases, synthetic world)
  natively. Physical touch screens (#143) and the intermittent touchsnapshot01 drift (#472) stay open.
- node:GaussianSplats: PR #476 measures the sort worker to 300000 splats on three machines and compares the depth order.

No status changes. No DIFFERENT entry is touched. Counts: 284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING before and
after, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-r474-476", "ledger.json")
SNAP_SHA256 = "fb0d019d91ec9190100812e1099a5ebe7096cd9151c93387e54cd08d37564683"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f
def chain(*fs):
    def f(old):
        for g in fs: old = g(old)
        return old
    return f
def addtext(text):
    return lambda old: old + "; " + text
def addlist(*texts):
    return lambda old: old + list(texts)
def note(text):
    return lambda old: (old + " | " if old else "") + text

C = {
 "sub:nav.walk-fly": (None, {
   "missing": swap("Viewport nodes for the rubber band, and the ContentScale and browser-option rubber band cases at MSAA 4 (the MSAA recorder has neither key)", "F16 (touch: one finger drives, two fingers look around) in x3d-viewer; it is compared in the browser lane only"),
   "tests": addlist("crates/x3d-viewer/tests/rea_nav_overlay.rs (rea_nav_overlay_viewport_matches_x_ite and rea_nav_overlay_viewport_msaa4_matches_x_ite: 9 Viewport cases each, a LayerSet layer with a Viewport clipBoundary and a Viewport grouping node, a press inside and outside, a drag that leaves the rectangle, ContentScale 2 and 0.5, MSAA 0 and 4; rea_nav_overlay_rubber_band_options_msaa4_matches_x_ite: the option-off and ContentScale 2, 1.5, 0.5 cases at MSAA 4; oracle/rea-nav-overlay-viewport-baseline, -viewport-msaa4-baseline, -options-msaa4-baseline and their apple-m1 references, three identical recordings each)"),
  }, "Issue #51 2026-11-06: PR #474. X_ITE draws the rubber band with gl.viewport and gl.scissor set to browser.getViewport(), the whole drawing buffer (ScreenLine.display); the active layer's Viewport only gates the press (X3DViewer.isPointerInRectangle). The port clipped the band to the layer rectangle (red: 7 of 9 layer cases, for example 444 of 446 pixels over 1). The band now uses the whole buffer. 22 cases match on Linux NVIDIA and, with the apple-m1 references (sky color one level off only), on the physical M1. Text only: F16 is not compared in x3d-viewer, so the entry stays PARTIAL."),
 "sub:nav.examine": (None, {
   "missing": swap("not compared: E14 (touch rotate and pinch) only in the x3d-web browser lane, E18 only through x10_compare.py on a private world", "not compared: physical touch screens (#143), the E14 cases use DevTools touch events in headless Chrome; open: touchsnapshot01 has an intermittent pinch drift under CPU load (#472, 1 of 20 runs with 24 busy loops, 0 of 20 without)"),
   "tests": addlist("crates/x3d-viewer/tests/rea_b24_touch.rs (rea_b24_touch_match_x_ite: E14, 7 touch cases, one-finger rotate and spin, pinch in, out and slanted, a finger added during a pinch, rotate then pinch; x3d-drive touchstart, touchmove and touchend; poses within 1e-4; oracle/rea-b24-nav-touch-baseline, three identical recordings, touch delivery split 0)", "crates/x3d-viewer/tests/rea_x10_synthetic.rs (shard0 to shard3: E18, 28 cases of the oracle/x10-scripts clip, rotate-then-zoom, rotate-during-zoom and msaa-orrery scripts on the clean-room world oracle/synthetic-rea-b24-nav/x10-stress.wrl, MSAA 0 and 4, 512x512 and 1024x768; poses within 1e-4, pictures threshold 8 / 0.5%; oracle/rea-x10-synthetic-baseline, three identical recordings)"),
  }, "Issue #51 2026-11-06: PR #475. New native compatibility test coverage: E14 and E18 are compared natively against pinned X_ITE 16.5.0. The base test driver x3d-drive had no touch events (26 pose failures in the 7 touch cases with it: a driver gap, not an engine defect). No engine change was made: nav.rs matches ExamineViewer.js (MOVE_ANGLE, ZOOM_ANGLE, zoomFactor, the touch mode lock, the spin rule). E18 shows no difference. Text only: physical touch screens are not compared and #472 is open, so the entry stays PARTIAL."),
 "node:GaussianSplats": (None, {
   "missing": swap("above 30000 splats or on a slower machine it may be longer (not measured, the worker's run time is wall-clock)", "PR #476 measured it to 300000 splats on the RTX 3060 Ti (ANGLE GL), SwiftShader and the Apple M1 (ANGLE Metal), Chrome 156 at about 60 Hz, five runs and three camera steps per count: no step was unanswered, the answer took 2.8 to 12.4 ms median, and two draws with the old order occurred only when the answer or the request took about one frame (SwiftShader 30000 and 100000, M1 300000); the depth order after the answer equals X_ITE_R's sort at every count on all three machines (order checksums); not measured: low-power machines, displays above 60 Hz, SH degree above 0"),
   "tests": addlist("crates/x3d-render/tests/gs_sort_order_pinned.rs (sort_indices with the recorded model-view matrices gives the order checksums of the pinned sort worker at 2, 441, 3969, 30000, 100000 and 300000 splats; oracle/gs-sort-order-baseline/order.json, byte-identical from three machines)", "crates/x3d-render/tests/render_gs_worker_timing.rs (441 splats turned by pi about Y and 3969 splats turned by pi/2 about X after the worker answer; oracle/render-gs-worker-timing-baseline, three identical runs; threshold 8 / 0.5%)"),
  }, "Issue #392 2026-11-06: PR #476. The worker timing and the depth order are measured apart (oracle/x_ite-splat-sort-timing.mjs). X_ITE posts at most one sort request at a time from display(); the sorted answer uploads the indices and the next draw uses them, so the view-change frame has the old order. That is a timing difference, not an order difference, and it is not emulated (KNOWN_DIFFERENCES splatSort.flip). Text only: the other gaps of this entry stay, so it stays PARTIAL."),
}



def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, nt = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            if nt: i["notes"] = (i["notes"] + " | " if i["notes"] else "") + nt
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_r474_476.py check|apply")
    if not snap_ok(): sys.exit(1)
    if sys.argv[1:] == ["apply"]:
        d = build()
        try: same = sha256(AUTH) == SNAP_SHA256 or load(AUTH) == d
        except ValueError: same = False
        if not same:
            print("ledger.json is neither the snapshot nor the result of this change set: refused"); sys.exit(1)
        tmp = AUTH + ".tmp"
        with open(tmp, "w") as f:
            json.dump(d, f, indent=1); f.flush(); os.fsync(f.fileno())
        os.replace(tmp, AUTH); sys.exit(0 if check(load(AUTH)) else 1)
    sys.exit(0 if check(load(AUTH)) else 1)
