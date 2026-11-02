#!/usr/bin/env python3
"""Ledger step after #316: python3 -I ledger_i316.py check|apply

node:ParticleSystem (#261, #262: the particle drive tool mirrors the MSAA sample pattern, the pr-line MSAA 4 gap is gone). Stays PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i316", "ledger.json")
SNAP_SHA256 = "50bb52405a15d0ad18e60a7de864883565e668afdc94a04d52b9838ee854e309"

OLD = "KNOWN_GAPS: pr-line MSAA 4 frame 45 0.581% of the pixels (line-edge antialiasing), pc-line frame 45"
NEW = "KNOWN_GAPS: pc-line frame 45"
NOTE = "Issues #261 and #262 2026-11-02 (PR #316): x3d-particles-drive never called set_gl_sample_orientation(true), so its MSAA 4 sample pattern was mirrored vertically against X_ITE. With the call, the rea_b13_particles MSAA 4 pictures differ by 0 to 1 pixel and the pr-line MSAA 4 gap (0.581%) is gone; KNOWN_GAPS of the MSAA 4 test is empty. Limits unchanged (0.5% of pixels more than 8 levels off). The two-canvas 96x72 particle pair (29 to 41 single-sample pixels at MSAA 4) is not explained by this and stays ungated. Status stays PARTIAL."
C = {"node:ParticleSystem": (None, {"missing": lambda old: old.replace(OLD, NEW)}, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i316.py check|apply")
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
