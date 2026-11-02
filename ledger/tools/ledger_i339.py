#!/usr/bin/env python3
"""Ledger step after #339 (issue #69): python3 -I ledger_i339.py check|apply

One entry, node:GaussianSplats: GPU-only pointing and collision (ADR 0013) and collision at MSAA 4. Status stays PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i339", "ledger.json")
SNAP_SHA256 = "9437bfb83d1e3839788e3bfc14b717b15e67645f018ee4150d0feffa5d3ca6db"

def setm(new):
    return lambda old: new
C = {
 "node:GaussianSplats": (None, {}, "Issue #69 2026-11-02 (PR #339, ADR 0013 Accepted): pointing and collision for splats are GPU-only, as in pinned X_ITE 16.5.0 (it renders ids into PointingBuffer and depth into depthBuffer for every shape; no CPU path). The viewer's --cpu-pointing path skips splats (pointing and collision give 0 hits there); test b69_gs_cpu_standin pins this and the GPU result (217 pick hits and 7 collision hits, equal to the pinned baselines). ISO/IEC 19775-1 V4.0 defines the pointing and collision results (collision 23.2.4), not the method; 19775-2 unverified. Collision at MSAA 4 is recorded (13 worlds, five identical pinned runs each; collisionProbes equal the MSAA 0 baseline in all 13, including the WALK fall; collision_probes_do_not_depend_on_msaa). Still open for #69: the macOS Metal backend needs a Mac (every GPU result is Vulkan on the RTX 3060 Ti); the sort timing and the WALK start lag are recorded differences, not parity. Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i339.py check|apply")
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
