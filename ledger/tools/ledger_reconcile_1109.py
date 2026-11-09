#!/usr/bin/env python3
"""Ledger step of the 2026-11-09 reconciliation: python3 -I ledger_reconcile_1109.py check|apply

Every entry was mapped to the tests that ran in the strict suite at origin/master d79abe4c (audit_1109.py, evidence in
evidence/ledger-audit-2026-11-09/). Four entries change:
- sub:browser.webxr MISSING -> PARTIAL: the browser WebXR path of the web build (issue #83, PRs #513, #515) is implemented and
  compared with the pinned X3DWebXRContext (Node: 28 of 28; Rust: xr_views, xr_gamepads, xr_pointers, xr_ray_pick). The
  native OpenXR session is not started (owner decision 2026-11-08, item 2), and no headset has run it. The ledger has exactly 346 IDs (verify_ledger.py), so native XR stays a gap of this entry, not a new one.
- sub:render.transparency-sorting: text. The two oit01 todo cases pass since PR #499 (issue #480); the gap text said they differ.
- node:Collision: text. The gap named only the first paths; the recorded coverage is wider, only set_live__ is open.
- sub:parse.vrml-classic: text. The 6 resource-load log differences of the parse-only test equal pinned X_ITE in the
  runtime (new test crates/x3d-viewer/tests/classic_edge_runtime.rs). The status stays PARTIAL: the notes leave a status
  change to the owner while the Annotation decision stands.

No DIFFERENT entry is touched. Counts: 284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING before, 284 / 41 / 21 / 0 after,
total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-reconcile-1109", "ledger.json")
SNAP_SHA256 = "b5bc71269e9d2e1a5c186f24d343a90d1ebd2bbbc7bd2c0000c89fbef85157d8"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f

def swap_in_list(old_text, new_text):
    def f(old):
        assert sum(s.count(old_text) for s in old) == 1, old_text
        return [s.replace(old_text, new_text) for s in old]
    return f

def append(*extra):
    return lambda old: list(old) + list(extra)

C = {
 "sub:browser.webxr": ("PARTIAL", {
   "rust": lambda old: ["crates/x3d-web/web/xr_session.mjs", "crates/x3d-web/web/xr_layer.mjs", "crates/x3d-web/src/wasm.rs",
                        "crates/x3d-render/src/xr.rs", "crates/x3d-viewer/src/viewer.rs", "crates/x3d-viewer/src/nav.rs"],
   "behavior": lambda old: "browser WebXR of the web build after X3DWebXRContext.js: session start and end, the mode from XRSessionMode, the local reference space, session events, input sources and gamepads, the viewer pose and per-view matrices, drawing each view, gamepad navigation, target-ray picking and pointing sensor events",
   "tests": lambda old: [
     "crates/x3d-web/tests/xr/pinned_compare.test.mjs (the pinned X3DWebXRContext and the port driven by one emulated device: requests to navigator.xr, reference spaces, session events, render state, gamepads, framebuffer reshapes, viewer pose and every view matrix exact)",
     "crates/x3d-web/tests/xr/xr_session.test.mjs, crates/x3d-web/tests/xr/gamepads_pinned.test.mjs (Node, 28 tests with the pinned comparison)",
     "crates/x3d-render/tests/xr_views.rs (12 cases against pinned pictures oracle/xr-shot-baseline and -msaa4, MSAA 0 and 4)",
     "crates/x3d-viewer/tests/xr_gamepads.rs (crates/x3d-viewer/tests/data/xr_gamepads_pinned.json), crates/x3d-viewer/tests/xr_pointers.rs, crates/x3d-viewer/tests/xr_ray_pick.rs",
     "crates/x3d-web/tests/browser/xrsession01.test.mjs, crates/x3d-web/tests/browser/xrdraw01.test.mjs (headless Chromium, WebGL2, emulated session and layer)"],
   "missing": lambda old: "native OpenXR session not started (owner decision 2026-11-08, item 2: OpenXR for native applications on the shared engine; needs an OpenXR runtime and the wgpu Vulkan interop, issue #83); drawing into a session on WebGL2 only (XRGPUBinding is not in the stable browser), so on the WebGPU backend the XR button stays hidden; the input rays and hit points of the XR input sources are not drawn (X_ITE draws them with ScreenLine and ScreenPoint in xrUpdatePointers; the pick and the sensor events are implemented); no physical headset or real runtime has run any of it, the device and makeXRCompatible are emulated, so no hardware support is claimed; a lost pose keeps the last good pose where X_ITE would throw (deliberate, docs/webxr-runtime-options.md)",
   "oracle": lambda old: "the pinned X3DWebXRContext in Node with an emulated device; pinned pictures of oracle/xr-shot.html",
  }, "Reconciliation 2026-11-09 (origin/master d79abe4c): the entry was MISSING with no rust or tests, written before issue #83. PRs #513 and #515 (merged) implement the browser WebXR path; docs/webxr-runtime-options.md records the owner decision of 2026-11-08 and the limits. Evidence: node --test tests/xr/*.test.mjs 28 of 28 pass at d79abe4c; the strict suite at d79abe4c runs xr_views, xr_gamepads, xr_pointers and xr_ray_pick (counts in evidence/ledger-audit-2026-11-09/REPORT.md). getRenderingProperty XRSession is TRUE while a session runs and the XR button follows XRSessionMode (web/x_ite_r.mjs, web/API.md). PARTIAL, not COMPLETE: native XR and real devices are open. Native OpenXR stays a gap of this entry because the ledger has exactly 346 IDs; it is not a failure of the browser path."),
 "sub:render.transparency-sorting": (None, {
   "missing": swap(", and a world Script's option in the next world differs from X_ITE (two todo cases)", "; a world Script's option stays for the next world as in X_ITE (PR #499, issue #480; the two former todo cases pass)"),
   "tests": swap_in_list("and a todo case for a world Script's option in the next world", "and a world Script's option in the next world (todo until PR #499)"),
  }, "Reconciliation 2026-11-09: the gap text still listed the two oit01 todo cases; PR #499 (merged at 3534b792, required checks Rust tests result and Web tests result SUCCESS at its head) made them pass, recorded then under sub:browser.options only. Text only: MSAA 4 references and OIT in the reference worlds stay open, so the entry stays PARTIAL."),
 "sub:parse.vrml-classic": (None, {
   "missing": swap("open: 6 resource-load log differences (runtime side) and the Annotation owner decision", "the 6 resource-load log differences of the parse-only test equal pinned X_ITE in the runtime (classic_edge_runtime: first line of each message; the multi-line tail of one fallback summary is console text and is not compared); open: the Annotation owner decision"),
   "tests": append("crates/x3d-viewer/tests/classic_edge_runtime.rs (the 6 log rows of crates/x3d-parse/tests/classic-edge-known.tsv run through x3d-drive at the recorder's base URL; sorted first lines equal the pinned log)"),
  }, "Reconciliation 2026-11-09: the 6 log rows were never run in the runtime (PR #427: the runtime side was not re-verified). classic_edge_runtime runs them through x3d-drive: Inline empty url, AudioClip 404, the two Script url aliases, the IMPORT route of an Inline with no url and the Script url list all print X_ITE's lines. They do not change scene loading, event order or the API: each is the console report of a failed resource load, which the runtime already does. The status stays PARTIAL; the notes leave a change to the owner while the Annotation decision stands."),
 "node:Collision": (None, {
   "missing": lambda old: "not compared: a Collision whose execution context leaves the live state while it is active or enabled (Collision.set_live__ removes it from the browser's collisions: an unloaded Inline scene, a replaced world); everything else of Collision.js is compared with pinned X_ITE (see notes)",
  }, "Reconciliation 2026-11-09: the gap text named only the binding, static load and children paths and #408; it was not updated when the other paths were recorded. Compared with pinned X_ITE in the tests that ran at d79abe4c: isActive and collideTime for a wall, a graze, an edge, nested Collision nodes, enabled FALSE at load and enabled toggled at runtime, visible FALSE, a Shape proxy and an empty Group proxy, the collisionRadius * sqrt 2 threshold (rea_collision_time, oracle/synthetic-rea-collision-time/cases.txt; rea_batch09_inputs collision-runtime; rea_b24_anchor_collision_instanced cl-events, cl-render with bboxDisplay; rea_b11 bb-others bboxDisplay; rea_verify collision-children). Only set_live__ has no case, so the entry stays PARTIAL."),
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
    bad += [f"{k}: not in the snapshot" for k in sorted(set(C) - set(snap))]
    built = build()
    if {k: v for k, v in d.items() if k != "items"} != {k: v for k, v in built.items() if k != "items"}: bad.append("top-level fields differ from what apply writes")
    want = {i["id"]: i for i in built["items"]}
    for k, b in snap.items():
        a = cur.get(k)
        if a is None: continue  # reported above: ID set or total changed
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_reconcile_1109.py check|apply")
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
