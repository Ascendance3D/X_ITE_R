#!/usr/bin/env python3
"""Issue #67 ledger update (the four pointing sensors on the GPU pointing path): python3 -I ledger_i67.py check|apply

Starts from ledger/before-i67/ledger.json (sha256 b9ef79f44968116be079f20eaab12bb06d606b916f0f90af2250b2a09916b6c4). Owner decision 2026-10-31: PlaneSensor, SphereSensor, CylinderSensor
and TouchSensor leave DIFFERENT. The GPU pointing path (x3d-drive, PointingGpu) now matches pinned X_ITE 16.5.0 in all 60 dumped
frames of both worlds, character for character (scene text, poses, log). These four are the only DIFFERENT entries in a change
set in the whole history; the other 21 stay frozen."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i67", "ledger.json")
SNAP_SHA256 = "b9ef79f44968116be079f20eaab12bb06d606b916f0f90af2250b2a09916b6c4"

TESTS = ["crates/x3d-viewer/tests/rea_batch09_inputs.rs (sensors-drag, sensors-touch; oracle/rea-batch09-inputs-baseline/sensors-drag-msaa0 and sensors-touch-msaa0, recorded from pinned X_ITE 16.5.0, 3 identical runs each)"]
NOTE = "Issue #67 2026-10-31 (owner approved the move off DIFFERENT): re-measured on the GPU pointing path with x3d-drive against pinned X_ITE recordings of oracle/synthetic-rea-verify/drag-sensors.wrl (PlaneSensor, SphereSensor, CylinderSensor) and touch-sensor.wrl: scene text, poses and log are identical in all 60 frames, so the earlier float32 note no longer applies to this path. Without a pointing GPU the CPU stand-in still intersects in f64 (not measured here)."
def upd(old): return old + TESTS
FIELDS = {"tests": upd, "oracle": "scene text, poses and log identical to pinned X_ITE in 60 frames of two worlds (GPU pointing path)"}
C = {k: ("COMPLETE", FIELDS, NOTE) for k in ("node:PlaneSensor", "node:SphereSensor", "node:CylinderSensor", "node:TouchSensor")}

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
        if b["status"] == "DIFFERENT" and k not in C and a != b: bad.append(f"DIFFERENT changed: {k}")
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i67.py check|apply")
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
