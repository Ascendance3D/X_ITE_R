#!/usr/bin/env python3
"""Ledger step after #312 and #314: python3 -I ledger_i312.py check|apply

node:Script (#54, fetched-candidate frame decided) and node:BoundedPhysicsModel / node:VolumeEmitter (#89, SwiftShader measurement). All three stay PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i312", "ledger.json")
SNAP_SHA256 = "ec5a8733603f9dbd7eccc4fa54b2906649b1ab5a145e9e2dc5d9fc92e988d387"

OLD_S = "scr-url-order-open: the frame a fetched Script candidate (a file, a base64 data URL) completes in, and that Chrome completes a base64 data URL's fetch after the files' (network completion in the oracle page; the microtask order now matches, scr-url-order-micro); "
OLD_P = "was measured on one GPU and driver (RTX 3060 Ti, Chrome / ANGLE) and"
NEW_P = "was measured on one GPU and driver (RTX 3060 Ti, Chrome / ANGLE); the pinned X_ITE on SwiftShader, a software ANGLE backend and so a second implementation but not a second GPU, gave identical js.json and float differences from the RTX baseline (53 replays: 36 within the 06R tolerance, 17 over it, worst 2.4e-3 in position components, no sign of a different clamp; #314), and no Metal run exists (#89); and"
NOTE_S = "Issue #54 2026-11-02 (PR #312): the fetched-candidate frame is decided. Pinned X_ITE has no single frame (14 runs: all four candidates in frame 2 in 11 runs, in frame 1 in 1, split in 2; no 5-identical-run baseline), so a deterministic match is impossible. X_ITE_R completes the fetches while the world loads, in request order, so every Script initializes before the first frame (ISO/IEC 19775-1 V4.0 29.2.3), one of the recorded X_ITE outcomes; pinned by rea_b54_script_fetch_frame.rs, ADR 0007. scr-classes-open and scr-browser-open remain. ISO/IEC 19775-2 unverified. Status stays PARTIAL."
NOTE_P = "Issue #89 2026-11-02 (PR #314): X3D_BACKEND selects the renderer backend (metal, dx12, gl, vulkan) and X3D_ORACLE_ANGLE the X_ITE replay backend. The pinned particle replays on SwiftShader are measured (see missing). The Metal run on macOS 15+ is not done: it needs a second machine. Status stays PARTIAL."
C = {
    "node:Script": (None, {"missing": lambda old: old.replace(OLD_S, "")}, NOTE_S),
    "node:BoundedPhysicsModel": (None, {"missing": lambda old: old.replace(OLD_P, NEW_P)}, NOTE_P),
    "node:VolumeEmitter": (None, {"missing": lambda old: old.replace(OLD_P, NEW_P)}, NOTE_P),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i312.py check|apply")
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
