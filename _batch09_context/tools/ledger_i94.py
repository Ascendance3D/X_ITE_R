#!/usr/bin/env python3
"""Issue #94 ledger update (Anchor, Collision and InstancedShape pointer paths, PR #209). Four entries change; no status changes; no DIFFERENT entry is in the change set.

Starts from ledger/before-i94/ledger.json (sha256 1fcae60a54dc5d50a8b089b0c772d96215017d2a4a6b824eb1dbf67156bcca3b). python3 -I ledger_i94.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i94", "ledger.json")
SNAP_SHA256 = "1fcae60a54dc5d50a8b089b0c772d96215017d2a4a6b824eb1dbf67156bcca3b"

def sub(old, new):
    def f(text):
        if old not in text: raise SystemExit(f"text not found: {old!r}")
        return text.replace(old, new, 1)
    return f
def add(v):
    return lambda old: old + [v]

T24 = 'crates/x3d-viewer/tests/rea_b24_anchor_collision_instanced.rs (ci-pick 2601 probes, ci-events, cl-render, cl-events, an-nested; MSAA 0/4, 3 identical X_ITE runs each; oracle/rea-batch24-anchor-collision-instanced-baseline)'
NOTE = "Issue #94 2026-10-31 (PR #209): Anchor, Collision and InstancedShape pointer paths recorded from pinned X_ITE (batch 24). Rust defects fixed: pointerEvents FALSE on a Shape or InstancedShape was ignored by the pointing pass; a fragment Anchor click bound its Viewpoint one frame later than X_ITE (owner decision 2026-10-31: match X_ITE)."
C = {
 "node:Collision": (None, {"tests": add(T24), "missing": sub("render and navigation detail covered by GPU and navigation suites but not item-by-item; Billboard orientation not exercised; Anchor click/load not exercised", "Billboard orientation not exercised")}, NOTE),
 "node:Anchor": (None, {"tests": add(T24), "missing": sub("render and navigation detail covered by GPU and navigation suites but not item-by-item; Collision collideTime path and Billboard orientation not exercised; Anchor click/load not exercised", "Collision collideTime path and Billboard orientation not exercised; an Anchor to a non-X3D page or another window cannot be observed by the headless runner")}, NOTE),
 "node:InstancedShape": (None, {"tests": add(T24), "missing": sub("Pointer picking (one candidate per instance matrix) has no X_ITE comparison; collision and volume picking use the base geometry only;", "Volume picking (PickableGroup) of instances is not compared (X_ITE uses the base geometry);")}, NOTE),
 "sub:pointer.picking": (None, {"tests": add(T24)}, NOTE),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i94.py check|apply")
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
