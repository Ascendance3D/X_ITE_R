#!/usr/bin/env python3
"""Issue #78 ledger update (sub:proto.proto): python3 -I ledger_i78.py check|apply

The SAI proto API is ported and compared with pinned X_ITE 16.5.0 (rea_b24_proto_sai: 5 cases, 3 identical recordings): createProto, the declaration methods, declaration
field updates (add, replace, remove, updateInstances, requestUpdateInstances) and the instances that follow them. The entry stays PARTIAL for the differences listed. No
DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i78", "ledger.json")
SNAP_SHA256 = "f002f14a7abb6edfdaf217e894a4f1b44abbd3eeec05e2936625707cb55e0dcf"

T = ["crates/x3d-script/tests/rea_b24_proto_sai.rs (rea_b24_proto_sai_matches_x_ite: createProto found, unknown and EXTERNPROTO, declaration surface, field add/replace/remove, late and held instances, requestUpdateInstances timing, nested PROTO context, instance with IS fields; console compared as one list)",
     "crates/x3d-viewer/tests/rea_batch09_sai.rs rea_batch09_sai_cases_match_x_ite (member listing unchanged)"]
FIELDS = {"tests": lambda old: old + T, "oracle": "pinned X_ITE 16.5.0 (34cdf9f97): oracle/synthetic-rea-b24-proto-sai-baseline.json (5 cases, 3 identical recordings)",
    "missing": "corpus-exact; a body Script's shutdown () runs when an update disposes the body (X_ITE prints nothing); an update that disposes and rebuilds a body logs initialize once (X_ITE twice); getBody on an EXTERNPROTO, createInstance with the non-public setup argument and live X3DField objects from getUserDefinedFields not ported; addUserDefinedField on an EXTERNPROTO not probed; field objects a Script holds across a layout change stop refreshing; page load runs two more frames in X_ITE than the port, so consoles are compared as one list"}
NOTE = "Issue #78 2026-10-31: createProto already matched; added the declaration methods (getName, getType, getBody, getUserDefinedFields, getFieldDefinitions, newInstance, createInstance), the declaration field updates and instance updates through the generic extern_instance_update, an instance's getFieldDefinitions following its declaration, and the drop of a stale JS field mirror (it panicked after a layout shrink). Mutations (update propagation skipped, add not replacing) are caught."
C = {"sub:proto.proto": (None, FIELDS, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i78.py check|apply")
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
