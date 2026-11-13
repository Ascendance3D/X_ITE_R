#!/usr/bin/env python3
"""Ledger step of the LoadSensor first events (PR #589, 2026-11-12): an ImageTexture is COMPLETE_STATE after its image decode, so a LoadSensor
reports isActive TRUE, progress 0 first, as pinned X_ITE 16.5.0 (recorded with Chrome 156.0.8078.47). Evidence only: no status changes.
python3 -I ledger/tools/ledger_loadsensor.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-loadsensor", "ledger.json")
SNAP_SHA256 = "f1c605cd517a63ccb88358db0de0c8bca55043f6a0b178e9e249effc0f07bed6"

def append(*extra):
    return lambda old: list(old) + list(extra)

C = {
 "node:LoadSensor": (None, {
   "rust": append("crates/x3d-runtime/src/nodes/load_sensor.rs"),
   "tests": append('crates/x3d-viewer/tests/load_sensor_first_events.rs (an ImageTexture, an Inline and an AudioClip loaded at world load, each with its own LoadSensor: the events of each LoadSensor equal pinned X_ITE 16.5.0, oracle/synthetic-loadsensor-baseline, at 7 frames)'),
  }, '2026-11-12 (PR #589): a LoadSensor that watches an url object loaded at world load gives isActive TRUE, progress 0 at its initialize, then isActive FALSE, isLoaded TRUE, progress 1, loadTime, as pinned X_ITE 16.5.0 (recorded with Chrome 156.0.8078.47, the reference browser; Chrome 157.0.8103.78 gives the same events of each LoadSensor in another order between them). Before, an ImageTexture was COMPLETE_STATE when its fetch ended, in the cascade of the initialize, and its LoadSensor gave only isActive FALSE, progress 1, isLoaded TRUE. ImageTexture.setImage sets COMPLETE_STATE after the decode (ImageTexture.js:78-129); the port now does so in apply_decoded_images.'),
 "node:ImageTexture": (None, {
   "tests": append('crates/x3d-viewer/tests/load_sensor_first_events.rs (an ImageTexture, an Inline and an AudioClip loaded at world load, each with its own LoadSensor: the events of each LoadSensor equal pinned X_ITE 16.5.0, oracle/synthetic-loadsensor-baseline, at 7 frames)'),
  }, '2026-11-12 (PR #589): a LoadSensor that watches an url object loaded at world load gives isActive TRUE, progress 0 at its initialize, then isActive FALSE, isLoaded TRUE, progress 1, loadTime, as pinned X_ITE 16.5.0 (recorded with Chrome 156.0.8078.47, the reference browser; Chrome 157.0.8103.78 gives the same events of each LoadSensor in another order between them). Before, an ImageTexture was COMPLETE_STATE when its fetch ended, in the cascade of the initialize, and its LoadSensor gave only isActive FALSE, progress 1, isLoaded TRUE. ImageTexture.setImage sets COMPLETE_STATE after the decode (ImageTexture.js:78-129); the port now does so in apply_decoded_images.'),
}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, nt = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            if nt: i["notes"] = (i["notes"] + " | " if i["notes"] else "") + nt
    return d
def check(d):
    bad, snap = [], {i["id"]: i for i in load(SNAP)["items"]}
    cur = {i["id"]: i for i in d["items"]}
    if set(cur) != set(snap) or len(d["items"]) != 346: bad.append("ID set or total changed")
    if [i["id"] for i in d["items"]] != [i["id"] for i in load(SNAP)["items"]]: bad.append("ID order changed")
    bad += [f"{k}: not in the snapshot" for k in sorted(set(C) - set(snap))]
    built = build()
    if {k: v for k, v in d.items() if k != "items"} != {k: v for k, v in built.items() if k != "items"}: bad.append("top-level fields differ from what apply writes")
    want = {i["id"]: i for i in built["items"]}
    for k, b in snap.items():
        a = cur.get(k)
        if a is None: continue  # reported above: ID set or total changed
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_loadsensor.py check|apply")
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
