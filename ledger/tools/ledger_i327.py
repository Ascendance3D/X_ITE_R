#!/usr/bin/env python3
"""Ledger step after #325 and #327 (issue #69): python3 -I ledger_i327.py check|apply

One entry, node:GaussianSplats: collision sphere test reproduced, castShadow follows the field after an event, the WALK gravity two-frame lag stays listed as unexplained. Status stays PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i327", "ledger.json")
SNAP_SHA256 = "bc5b9d04fc3398e9eb3853df5d1dd7f97454cc6a7f52d351b3b94b866537e541"

def setm(new):
    return lambda old: new
OLD1 = "the addCollisionShape sphere test and WALK gravity with splats are not reproduced, castShadow is always TRUE as in the pinned shape (the field is not followed)"
NEW1 = "WALK gravity with splats matches pinned X_ITE except for an unexplained two-frame lag (#325; the cause is not identified, so this is a named difference and not parity), castShadow written FALSE in a file is never honored by the pinned shape and X_ITE_R matches that; both follow the field after an event (#327; whether an event with an equal value counts is unverified)"
def fix(old):
    assert OLD1 in old, "expected text not found"
    return old.replace(OLD1, NEW1)
C = {
 "node:GaussianSplats": (None, {"missing": fix}, "Issue #69 2026-11-02 (PRs #325, #327): the addCollisionShape sphere test is reproduced against pinned X_ITE (#325). WALK gravity with splats matches with a two-frame lag whose cause is unknown (assigned, open). castShadow follows the field after an event as in the pinned shape (#327: Runtime::splat_cast_shadow). MSAA 4 is not recorded for the castShadow and pick-under-log-depth worlds. ISO/IEC 19775-1 V4.0 has no GaussianSplats text; 19775-2 unverified. Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i327.py check|apply")
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
