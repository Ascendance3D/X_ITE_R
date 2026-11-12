#!/usr/bin/env python3
"""Ledger step of the scripted frame rate (2026-11-11): the Walk resting height after a fall with gravity.
python3 -I ledger/tools/ledger_walkrate.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-walkrate", "ledger.json")
SNAP_SHA256 = "759f2e0fd0d2cbece8df5117af90f7f616489425b9152389424063e171df3824"

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
 "sub:nav.walk-fly": (None, {
   "tests": [
     swap_in_list("poses 1e-4 and the Script history exact at every frame; PR #541)", "poses 1e-4 and the Script history exact at every frame; PR #541; with the scripted frame rate the count PX_n of ProximitySensor events is compared in every case, the gravity WALK cases too, and every pose is equal to X_ITE's on the RTX 3060 Ti) and crates/x3d-viewer/src/viewer.rs (a_scripted_frame_rate_replaces_the_rate_measured_from_the_clock)"),
   ],
   "missing": [
     swap("open: after landing with gravity the port's resting height wobbles at the f32 depth-read noise (about 2e-7, World::gravitate_on) and sends about one more ProximitySensor position_changed than X_ITE, whose height stays 1.6000000758, so the event count PX_n is not compared in the gravity WALK cases (poses within 1e-4 and the rest of the history are));", "the resting height after a fall with gravity is X_ITE's now, 1.6000000758, and the event count PX_n is compared in every case (the difference was the frame rate of the test driver, not the depth read: see the 2026-11-11 note); the pose bound stays 1e-4 for GPUs other than the RTX 3060 Ti, whose depth read is not measured);"),
   ],
  }, "2026-11-11 (scripted frame rate): the cause of the resting-height difference is proved, and it is not the depth read. At the landing frame X_ITE and the port subtract the same distance from the same depth value (0.0924416874521797). The fall before it differs: the recorder pages replace `getCurrentFrameRate` by `() => RATE` with RATE = 1 / dt (oracle/x_ite-pin/src/anim.html), so `X3DRenderObject.gravitate` falls with exactly 30; the driver x3d-drive let `Viewer3d::step` measure the rate from the script's clock, and at the epoch time of the scripts (946684800 s) a double has steps of 1.2e-7 s, so the measured rate was 30.0000286 or 29.9999213. The port then rested 2e-7 off X_ITE's height and moved by that much from frame to frame, and each move sent a ProximitySensor position_changed (walk-look-around: X_ITE 2 events, the port 55). Fix: `Viewer3d::scripted_frame_rate`, set by x3d-drive to 1 / dt; a viewer with a real clock (`None`) measures as before, as X3DTimeContext.advanceTime does. Result: in all 24 cases of rea_nav_touch every pose is equal to X_ITE's (distance 0) and the scene text with PX_n is equal at every frame; the strict suite passes (1724 passed, 0 failed, 10 skipped). REA (static analysis of the pinned JavaScript, not of Rust; partial graph): ev_9d08d991bad1a772f0bbe844bafa454cd5018ce4d036aa56ecff228516484dc3, summary ev_04a8ca05259dd6aaa65d8ce961ffe2f826b3cf12d7057fdd6a5fd3189254df6b (X3DTimeContext.js, X3DRenderObject.js; 2 files, 4922 AST nodes, 0 parse failures). Status stays PARTIAL: the entry's behavior field is still empty, and the touchcancel difference is kept."),
}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, nt = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items():
                for g in (v if isinstance(v, list) else [v]): i[f] = g(i[f]) if callable(g) else g
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_walkrate.py check|apply")
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
