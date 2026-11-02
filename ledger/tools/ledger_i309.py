#!/usr/bin/env python3
"""Ledger step after #309, #321 and #322: python3 -I ledger_i309.py check|apply

node:GaussianSplats (#69: sort timing recorded, log-depth MSAA 4 compared, page timeout explained). Stays PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i309", "ledger.json")
SNAP_SHA256 = "e62b529d7c3ac996ce423bceba8dcb71a0c5e12b9e91dc5676f3e46a624b1cbe"

R = [
 ("sort synchronous (X_ITE async; owner decision)", "sort synchronous by engineering decision (#309): the settled pictures equal X_ITE's (flip, burst of ten changes, turn there and back), but X_ITE draws the frame of a view change with the old order and again after its worker answers, a transient difference that is recorded (KNOWN_DIFFERENCES splatSort.flip) and not emulated"),
 ("the pinned particles page times out on worlds with explicit bboxSize/bboxCenter or visible FALSE (the bbox cull rule rests on source text)", "the pinned page times out (45 s) on worlds whose splat node is never displayed (gs-empty, gs-visible-false, gs-bbox with bboxCenter 100 0 0), because GaussianSplatsShape adds a loading object on the worker's ready and removes it only on the first sorted, which only display() asks for (#322; explicit bbox fields are not the cause); X_ITE_R finishes loading, a documented known difference (web/API.md) that is not emulated"),
 (", the log-depth pass has no MSAA 4 comparison", ""),
]
def F(old):
    for a, b in R:
        assert a in old, a
        old = old.replace(a, b)
    return old
NOTE = "Issue #69 2026-11-02 (PRs #309, #321, #322): the sort timing is recorded against pinned X_ITE (5 identical runs, splatsort01: pictures equal, one transient frame differs, decided to keep the synchronous sort); log depth at MSAA 4 is compared (35 worlds, 0 pixels over 8, max channel difference 1 to 2; 21 of 35 recordings byte-identical, 14 differ by 1 level in single channels); the page timeout is explained by the pinned source and measured (#322). Open: the log-depth pick pass, the addCollisionShape sphere test and WALK gravity, following the castShadow field. ISO/IEC 19775-1 V4.0 has no GaussianSplats text (X3D 4.1); 19775-2 unverified. Status stays PARTIAL."
C = {"node:GaussianSplats": (None, {"missing": F}, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i309.py check|apply")
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
