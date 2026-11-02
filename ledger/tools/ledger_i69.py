#!/usr/bin/env python3
"""Issue #69 ledger note (node:GaussianSplats): python3 -I ledger_i69.py check|apply

PRs #271 (logarithmic depth), #272 (castShadow shadow pass), #275 (pointerEvents picking) and #277 (collision) port those passes, each compared with pinned X_ITE 16.5.0 (REA IDs in the PR bodies). bboxDisplay and sphere culling were already in master (#243). The `missing` text keeps only the synchronous sort (owner decision) and the unknowns the PRs list. Status stays PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i69", "ledger.json")
SNAP_SHA256 = "7fc2a0f6a8105009d00a5f6510548f38391bf16b84da5df2dd7c25504c54a028"

OLD = "castShadow shadow pass, pointerEvents picking, collision, bboxDisplay, sphere culling, reversed/log depth; sort synchronous (X_ITE async)"
NEW = "sort synchronous (X_ITE async; owner decision); unknowns: the pinned particles page times out on worlds with explicit bboxSize/bboxCenter or visible FALSE (the bbox cull rule rests on source text), the pick pass has no logarithmic depth mode, the addCollisionShape sphere test and WALK gravity with splats are not reproduced, castShadow is always TRUE as in the pinned shape (the field is not followed), the log-depth pass has no MSAA 4 comparison"
NOTE = "Issue #69 2026-11-01: castShadow shadow pass (#272, 5 synthetic worlds, MSAA 0 and 4, 0 pixels over 8), pointerEvents picking (#275, 6 worlds, 2646 probes, hit or miss identical, values within 7.1e-5), collision (#277, 6 worlds, hit state exact, distances within 1e-6) and logarithmic depth (#271, 35 worlds) match pinned X_ITE 16.5.0. pointerEvents defaults to FALSE and is copied once at initialize; the hit normal is always (0, 0, 1); collision does not depend on pointerEvents. bboxDisplay and sphere culling were already in master (#243), so the earlier missing text was stale. Status stays PARTIAL."
F = {"missing": lambda old: old.replace(OLD, NEW)}
C = {"node:GaussianSplats": (None, F, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i69.py check|apply")
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
