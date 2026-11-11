#!/usr/bin/env python3
"""Ledger step of PR #543 (2026-11-10): AudioClip startTime, stopTime, pauseTime and resumeTime in the future compared with
pinned X_ITE under the recorder's virtual timers; the elapsed-time start of a timer start fixed.
python3 -I ledger/tools/ledger_audio543.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-audio543", "ledger.json")
SNAP_SHA256 = "5fdcc059d988eecd30f07ee3b7cbc91c6c5ac92f3fd10117ffb852cdb7dc6991"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f

def swap_in_list(old_text, new_text):
    def f(old):
        assert sum(s.count(old_text) for s in old) == 1, old_text
        return [s.replace(old_text, new_text) for s in old]
    return f

def append(*extra):
    return lambda old: list(old) + list(extra)

OLD = "startTime in the future is not recorded (X_ITE arms a real-time setTimeout, so the frame depends on the wall clock);"
NEW = "startTime, stopTime, pauseTime and resumeTime in the future are compared (14 clip-future worlds: 0.5, 1 and 2 s ahead, one-shot and loop, stopTime before and after the start, a stopTime or startTime change while waiting, pauseTime with and without resumeTime; isActive, duration_changed, elapsedTime and isPaused events and every offline-rendered sample exact; PR #543) under the recorder's virtual setTimeout queue (anim.html), which makes the pinned page deterministic (three identical recordings); a real-time browser fires the timer late by its own lateness, which is not compared;"
NOTE = "PR #543 (2026-11-10): one Rust fix, red first (9 of 14 clip-future worlds failed in prints: the first elapsedTime after a timer start was 0 in the port and 0.012 s in X_ITE). X3DTimeDependentNode processTimeout runs do_start at the timer's due time (getCurrentTime inside the timer is max(previous frame, due)), so resetElapsedTime counts from startTime; the port counted from the frame time. Now the start timeout of a non-TimeSensor node sets the elapsed start to max(previous frame, due), capped at the frame time, while event time stamps stay at the frame time as in X_ITE (crates/x3d-runtime/src/nodes/time.rs). Tests: vrml_audio_oracle 3/3; rea_b05.py check 126 pass, 1 known (spatial-hrtf), 2 blocked as before; x3d-runtime and the audio, sound, movie, time, animation and multi-viewer suites 440 passed. Evidence read in the pinned source (X3DTimeDependentNode.js addTimeout, processTimeout, do_start, resetElapsedTime; anim.html runTimers). Status stays PARTIAL: HRTF panning and real devices."
C = {
 "node:AudioClip": (None, {"missing": swap(OLD, NEW), "tests": append("crates/x3d-viewer/tests/vrml_audio_oracle.rs clip-future-* (oracle/rea-b05-baseline, three identical recordings; PR #543)")}, NOTE),
 "node:Sound": (None, {"missing": swap(OLD, NEW)}, NOTE),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_audio543.py check|apply")
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
