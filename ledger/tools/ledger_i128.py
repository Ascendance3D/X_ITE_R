#!/usr/bin/env python3
"""Issues #128, #130, #131 ledger update (node:Inline, sub:parse.gltf2): python3 -I ledger_i128.py check|apply

Notes only, no status change. gltfworld02 gains 3 cases against pinned X_ITE 16.5.0 (5 identical recordings): an Inline glTF with one failed and one held buffer now rejects at the first failure as X_ITE does (fixed); two followed failures log in arrival order and a newer loadURL does not abort followed buffers (no difference)."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i128", "ledger.json")
SNAP_SHA256 = "c0aaeca87a4ec4ab48e145d019cfda3f45ffcf2ecae966b03b041f82f9eaa4dc"

T = ["crates/x3d-web/tests/browser/gltfworld02_case.mjs (inlineFailThenHeld, followedReverse, supersededFollowed; oracle/web-browser-gltf-world-02-baseline, 5 identical recordings)"]
FIELDS = {"tests": lambda old: old + T}
NOTE = "Issues #128/#130/#131 2026-11-01: an Inline glTF rejects at the first failed buffer and follows the others (fixed, matches X_ITE); buffer failures log in arrival order and a newer loadURL does not abort followed buffers (no difference). Only the leaked loading object (fallback.gltfFailed) remains."
C = {"node:Inline": (None, FIELDS, NOTE), "sub:parse.gltf2": (None, FIELDS, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i128.py check|apply")
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
