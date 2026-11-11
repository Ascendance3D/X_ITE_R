#!/usr/bin/env python3
"""Ledger step of PR #541 (2026-11-10): F16 touch navigation of WALK and FLY compared natively with pinned X_ITE.
python3 -I ledger/tools/ledger_navtouch541.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-navtouch541", "ledger.json")
SNAP_SHA256 = "9d35f0804420f97f18f2a9b925a60039838afaca5d83c31395c42a545e507792"

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
   "missing": swap("not compared: F16 (touch: one finger drives, two fingers look around) in x3d-viewer; it is compared in the browser lane only;", "F16 (touch: one finger drives, two fingers look around) is compared in x3d-viewer too (rea_nav_touch, 24 cases in WALK with gravity and collision, WALK with Gravity 0 and FLY, PR #541; the WALK and Gravity 0 poses are equal in the pinned page, so the Gravity 0 world is not shown to differ); touchcancel during a move: X_ITE has no touchcancel handler and keeps driving, the port stops (a kept difference, ADR 0001 row 1, pinned by name in the *-cancel-during-move cases);"),
   "tests": append("crates/x3d-viewer/tests/rea_nav_touch.rs (oracle/rea-nav-touch-baseline: 24 cases, three identical pinned X_ITE 16.5.0 recordings; poses 1e-4 and the Script history exact at every frame; PR #541)"),
  }, "PR #541 (2026-11-10): F16 touch navigation compared natively with no Rust change (21 of 24 cases equal at once; the 3 touchcancel cases are the kept difference, with the frames before the cancel equal, the pinned drive continuing at frames 20 to 29, the port holding the frame-19 pose and the next drive equal). Evidence read in the pinned source (X3DFlyViewer.js touch handlers). Status stays PARTIAL: the entry's behavior field is empty, so a promotion needs that evidence first."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_navtouch541.py check|apply")
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
