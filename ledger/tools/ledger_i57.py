#!/usr/bin/env python3
"""Issue #57 ledger note (sub:fields.types): python3 -I ledger_i57.py check|apply

The clause fldx-print-count-open in the `missing` text of sub:fields.types is stale: the case no longer exists in the suite, the checkpoint after 804802d converts a raw
component when it is printed (as X_ITE does) and fldx-count (toString / valueOf calls on Script fields equal X_ITE's) passes. The clause is removed and a note says so.
Status stays PARTIAL: the X3DConstants order of external-component node types varies run to run in X_ITE (recorded as untestable) and the IterableWeakSet limit remain
(QuickJS). No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i57", "ledger.json")
SNAP_SHA256 = "89ec8de1708cac5b97b2310c86cf3850b56c53f594330c005538b84f399e7ddd"

OLD = "; the toString / valueOf calls a node's own math makes on a raw component (fldx-print-count-open: Transform 3, TextureTransform 2 when the value arrives; the port's runtime uses NaN for an object component and converts it only where it prints it)"
NOTE = "Issue #57 2026-11-01: the clause fldx-print-count-open is removed from missing: the case is gone from the suite, a raw component is converted when it is printed in print order as X_ITE does, and fldx-count (the toString / valueOf calls on Script fields) equals the pinned X_ITE recording. The X3DConstants order of external-component node types varies run to run in X_ITE and is recorded as untestable; the IterableWeakSet limit stays (QuickJS)."
FIELDS = {"missing": lambda old: old.replace(OLD, "") if OLD in old else old}
C = {"sub:fields.types": (None, FIELDS, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i57.py check|apply")
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
