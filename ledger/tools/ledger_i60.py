#!/usr/bin/env python3
"""Issue #60 ledger note (sub:browser.multi-viewer-thread, sub:physics.engine): python3 -I ledger_i60.py check|apply

The 0.579% at frame 40 of x_ite_two_canvases_rigid_bodies_msaa4 was not a float32 ulp at the substep boundary. Commit 510f48fc pinned the harness frame rate to 1/DT; since then rb-a and rb-b are 0.00000 at frames 5, 20 and 40 for all four comparisons (rerun 2026-11-01). The `missing` clauses that blame the ulp are replaced; statuses stay PARTIAL (other clauses remain)."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i60", "ledger.json")
SNAP_SHA256 = "3e67875cee77f6a36808ae69ed39f24b2d674e68f4ee507cbe82f0eb67dda3b8"

OLD1 = "x_ite_two_canvases_rigid_bodies_msaa4 f40 is 0.579% (single-viewer edge difference); "
NEW1 = "x_ite_two_canvases_rigid_bodies_msaa4 is 0.00000 at frames 5, 20 and 40 (the earlier 0.579% came from the harness frame rate, fixed in 510f48fc; issue #60); "
OLD2 = "rb-a (box falling from rest, 8 substeps/frame) takes one substep fewer than pinned X_ITE at frame 8 when sum/fixedTimeStep lands one ulp below an integer; rounding the quotient to float32 fixes it but regresses the rate/stack corpus, so it is not applied (x_ite_two_canvases_rigid_bodies_msaa4 f40 0.579% stays open)"
NEW2 = "the earlier rb-a substep difference at frame 8 was the harness measuring the frame rate from clock deltas; with currentFrameRate pinned to 1/DT (510f48fc) rb-a and rb-b equal pinned X_ITE at MSAA 4 (issue #60)"
NOTE = "Issue #60 2026-11-01: x_ite_two_canvases_rigid_bodies_msaa4 rerun on master 7375acdd (RTX 3060 Ti, Vulkan): 1 passed; X_ITE pair vs alone, Rust pair vs X_ITE pair, Rust alone vs X_ITE alone and Rust pair vs Rust alone are 0.00000 at frames 5, 20 and 40 for rb-a and rb-b. The float32 ulp explanation of the 0.579% was wrong. Multi-viewer threads, particles and sound cases are in PR #260. Status stays PARTIAL."
F1 = {"missing": lambda old: old.replace(OLD1, NEW1)}
F2 = {"missing": lambda old: old.replace(OLD2, NEW2)}
C = {"sub:browser.multi-viewer-thread": (None, F1, NOTE), "sub:physics.engine": (None, F2, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i60.py check|apply")
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
