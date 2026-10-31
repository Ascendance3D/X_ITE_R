#!/usr/bin/env python3
"""Issue #82 ledger update (WindPhysicsModel wind speed unit conversion): python3 -I ledger_i82.py check|apply

Starts from ledger/before-i82/ledger.json (the authoritative ledger at origin/master e67034c9, sha256 269f3e06...28a3).
node:WindPhysicsModel said "Wind speed unit conversion not modeled". That text is stale: the Rust force applies the
conversion of pinned X_ITE 16.5.0 (oracle/x_ite-pin, 34cdf9f97, WindPhysicsModel.js line 47):
pressure = 10 ** (2 * Math.log (speed)) * 0.64615, with the same clamps and the same Math.random draw order
(crates/x3d-runtime/src/nodes/particle_systems.rs). Only the owner-approved TOL_STRONG remains. The owner confirmed on
2026-10-31 that TOL_STRONG stays. The entry stays PARTIAL, and its open item names that decision. One entry changes;
no status changes; no DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i82", "ledger.json")
SNAP_SHA256 = "269f3e06e4c601d25e50fdd550e5388086a807a3a5b8ff0f659dd6227bd428a3"  # ledger.json at origin/master e67034c9
NOTE = ("Issue #82 2026-10-31: the unit conversion is modeled (Rust pressure = 10**(2*log(speed))*0.64615 equals "
        "WindPhysicsModel.js line 47 of the pin, same clamps and Math.random draw order); the old open item 'Wind speed unit "
        "conversion not modeled' was stale. Owner decision 2026-10-31: TOL_STRONG stays.")
OLD_TAIL = " Wind speed unit conversion not modeled."
NEW_TAIL = (" Owner decision 2026-10-31: TOL_STRONG stays, because the cause is the GPU's float32 turbulence, not the unit"
            " conversion. The unit conversion is modeled and equals pinned X_ITE 16.5.0.")

def fix_missing(old):
    if not old.endswith(OLD_TAIL): raise SystemExit(f"unexpected missing text: {old!r}")
    return old[: -len(OLD_TAIL)] + NEW_TAIL

# id: (status or None, {field: value or function of the old value}, note). The verifier reads the first element.
C = {"node:WindPhysicsModel": (None, {"missing": fix_missing}, NOTE)}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, note = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            i["notes"] = (i["notes"] + " | " if i["notes"] else "") + note
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i82.py check|apply")
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
