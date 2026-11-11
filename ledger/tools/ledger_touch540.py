#!/usr/bin/env python3
"""Ledger step of PR #540 (2026-11-10): touch input into the pointing device sensors compared with pinned X_ITE; the touch
release fix.
python3 -I ledger/tools/ledger_touch540.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-touch540", "ledger.json")
SNAP_SHA256 = "397f649c01f5304f590d105b8c30aef6383c0d048f26a7a070f9eebf32379762"

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
 "sub:pointer.picking": (None, {
   "missing": swap("; touch input (touchstart / touchend) is not driven by the harness", "; touch input (DevTools touch events into TouchSensor, PlaneSensor, CylinderSensor, SphereSensor and an Anchor: a tap, a drag, leave and enter, a second finger, a tap on empty space) is compared exactly (rea_pointer_touch, PR #540); touchcancel keeps the pressed sensor active in X_ITE and ends it in the port (a kept difference, ADR 0001 row 1, pinned by name in case cancel)"),
   "tests": append("crates/x3d-viewer/tests/rea_pointer_touch.rs (oracle/rea-pointer-touch-baseline: 6 cases, three identical pinned X_ITE 16.5.0 recordings; Script event history of every frame exact, poses 1e-4; PR #540)"),
  }, "PR #540 (2026-11-10): one Rust fix, red first. Pinned PointingDevice.touchend (and touchstart with two touches) releases through mouseup with a copied TouchEvent that has no pageX or pageY, so the release pointer is NaN: the release does not pick again and the motion check after it hits nothing (isOver FALSE at touchend). The port released at the touch position, kept isOver TRUE and sent hitPoint, hitNormal and hitTexCoord again (57, 82, 45 and 59 differing frames in tap-each, drag-each, leave-and-enter and second-finger). Now touch_end and the two-finger touch_start release with NaN, and pointing_up checks the motion after every release, as mouseup does (crates/x3d-viewer/src/viewer.rs; the browser build shares it). Tests: rea_pointer_touch 1/1; pointing subset (rea_batch09_nav, rea_batch12, rea_b24_touch, rea_b24_nav, pointer_slot_reuse, rea_b24_anchor_collision_instanced) 81 passed, 0 skipped. Evidence read in the pinned source (PointingDevice.js touchend, mouseup). Status stays PARTIAL: the line and point ulp bounds."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_touch540.py check|apply")
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
