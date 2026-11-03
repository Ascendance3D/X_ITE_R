#!/usr/bin/env python3
"""Ledger step after #388 and #392 (issues #90, #69): python3 -I ledger_i388.py check|apply

Three entries, text only, no status change. node:AudioClip and node:Sound: the audio-output comparison of #388 (14 pinned
16.5.0 worlds through the offline render) and what stays open. node:GaussianSplats: the sort transient is measured at
2, 441, 3969 and 30000 splats (#392). #383 is not merged and is not cited."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i388", "ledger.json")
SNAP_SHA256 = "ffd093f4e47cc25aaf8937a8a365bcb111daf0d8099f6bec80da0e4d40efad85"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f
def addtext(text):
    return lambda old: old + "; " + text
def addlist(text):
    return lambda old: old + [text]
AUDIO_TEST = "crates/x3d-viewer/tests/vrml_audio_oracle.rs (vrml_audio_matches_x_ite, 47 cases incl. 14 added by #388: clip-gain, clip-gain-runtime, clip-gain-pitch-intensity, sound-intensity-runtime, clip-pitch-edges, clip-enabled, sound-enabled, clip-loop-off, clip-stop-in-loop, clip-restart, sound-location-runtime and others; vrml_audio_oracle_keys_are_present; vrml_audio_oracle_keys_have_the_recorded_shape) against oracle/rea-b05-baseline"
AUDIO_ORACLE = "oracle/rea-b05-baseline (offline render of pinned X_ITE 16.5.0 (oracle/x_ite-pin, 34cdf9f97), sample for sample, tolerance 0; the 14 keys of #388 from oracle/synthetic-rea-b05, three identical recordings plus the writing one)"
AUDIO_MISSING = "real-device audio output is not compared (issue #88: CoreAudio, PulseAudio/PipeWire, WASAPI; only the offline render of both sides is); startTime in the future is not recorded (X_ITE arms a real-time setTimeout, so the frame depends on the wall clock); HRTF panning (spatial-hrtf) stays a known difference (rea_b05.py KNOWN)"
AUDIO_NOTE = "Issue #90 2026-11-03 (PR #388): audio output is compared with the pinned offline render for @@FIELDS@@. 14 synthetic worlds (oracle/tools/gen_rea_audio_repair.py), recorded with oracle/x_ite-anim.mjs: three recordings plus the one that wrote the key, scene text, log, Script prints, suspend frames and every sample identical in all four. Rust against the keys: 14 of 14 bit-identical (max difference 0 in both channels), the engine needed no change; the keys are not silent (vrml_audio_oracle_keys_have_the_recorded_shape pins frame count, silent frames and peak). Control: limiting the AudioClip gain callback to 1.0 fails clip-gain, clip-gain-runtime and clip-gain-pitch-intensity. No world set these fields before. ISO/IEC 19775-1 V4.0 clause numbers for the fields unverified (no local clause text); 19775-2 unverified. Hosted macOS skips vrml_audio_oracle by name (docs/gpu-failure-classification.md), so the new cases run on Linux only. Status stays PARTIAL for the items in missing."
C = {
 "node:AudioClip": (None, {"tests": addlist(AUDIO_TEST), "oracle": addtext(AUDIO_ORACLE), "missing": addtext(AUDIO_MISSING)},
  AUDIO_NOTE.replace("@@FIELDS@@", "AudioClip.gain (static and changed at run time, negative gain), pitch 0, negative, 4 and 0.25, enabled, loop switched off while playing, stopTime inside a loop, startTime while active and after the end, a mono clip at the side, and a turned Viewpoint (listener orientation)")),
 "node:Sound": (None, {"tests": addlist(AUDIO_TEST), "oracle": addtext(AUDIO_ORACLE), "missing": addtext(AUDIO_MISSING)},
  AUDIO_NOTE.replace("@@FIELDS@@", "Sound.intensity changed at run time and its 0..1 clamp, enabled, location, direction and range changes at run time, priority, and the listener orientation of a turned Viewpoint")),
 "node:GaussianSplats": (None, {
   "missing": swap("a transient difference that is recorded (KNOWN_DIFFERENCES splatSort.flip) and not emulated;",
     "a transient difference that is recorded (KNOWN_DIFFERENCES splatSort.flip) and not emulated; the transient is measured at 2, 441, 3969 and 30000 splats (#392): in every count one draw with the old order, then the worker's answer and a second draw, the worker answering within that one frame on the RTX 3060 Ti; above 30000 splats or on a slower machine it may be longer (not measured, the worker's run time is wall-clock);")},
  "Issue #69 2026-11-03 (PR #392): the sort-timing transient is measured against pinned X_ITE 16.5.0 (oracle/x_ite-splat-sort-trace.mjs, oracle/web-browser-splat-sort-01-baseline/splatsort01-trace-counts.json) with a synthetic cloud turned by a half turn at N = 2, 441, 3969 and 30000, three identical runs each: the worker messages and the instanced draws are `post sort`, `draw N` (old order), `recv sorted`, `draw N` at every count. The test b69_gs_sort_trace (data only, no GPU) fails when the recording is absent or different; X_ITE_R has no worker, so there is no Rust side to compare. The sort timing stays a recorded difference by the engineering decision of #309 (synchronous sort); whether the async sort or the two-frame WALK start lag are to be emulated is an owner decision, not made here. Open: counts above 30000 and other machines for the trace; the macOS Metal backend (needs a Mac). Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i388.py check|apply")
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
