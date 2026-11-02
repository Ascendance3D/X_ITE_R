#!/usr/bin/env python3
"""Ledger step after #334 (issue #69): python3 -I ledger_i334.py check|apply

One entry, node:GaussianSplats: the WALK start lag has a cause and is a named known difference. Status stays PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i334", "ledger.json")
SNAP_SHA256 = "a9ca8d6a87b604c8f3c6b2cba3e2f321399f8cc9ccc23d81d0435d1aca2b6359"

def setm(new):
    return lambda old: new
OLD1 = "WALK gravity with splats matches pinned X_ITE except for an unexplained two-frame lag (#325; the cause is not identified, so this is a named difference and not parity)"
NEW1 = "WALK gravity with splats matches pinned X_ITE except for a two-frame start lag (#325): the pinned page applies the NavigationInfo viewer only when its initialized event fires, that event waits for loadCount 0, and the splat worker's ready and first-sorted messages hold loadCount at 1 for two more traversed frames; X_ITE_R loads splats synchronously and does not emulate it, a named known difference (gaussian-splats-walk-start-lag, #334; docs/gaussian-splats-walk-start-lag.md; not parity)"
def fix(old):
    assert OLD1 in old, "expected text not found"
    return old.replace(OLD1, NEW1)
C = {
 "node:GaussianSplats": (None, {"missing": fix}, "Issue #69 2026-11-02 (PR #334): the two-frame WALK start lag has a cause. Pinned X_ITE, five byte-identical runs per variant: a no-splat WALK world falls with no lag; gc-walk-floor (49 splats), gc-walk-lag-n441 and gc-walk-lag-n3969 lag by exactly two frames, and the splat count does not change the lag. Tested by walk_start_lag_follows_the_splat_load (data only, runs on hosted CI). ISO/IEC 19775-1 V4.0 23.3.1 and 23.4.4 do not say when navigation starts relative to node loading, and GaussianSplats has no clause, so the pinned behavior is recorded and not emulated; 19775-2 unverified. Unproved: that each worker message costs exactly one frame turn, and in a real browser the delay is worker time, not a fixed frame count. Status stays PARTIAL."),
}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, note = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            if note: i["notes"] = (i["notes"] + " | " if i["notes"] else "") + note
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i334.py check|apply")
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
