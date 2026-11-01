#!/usr/bin/env python3
"""Issue #81 ledger update (node:AudioDestination): python3 -I ledger_i81.py check|apply

mediaDeviceID now selects the native output device (web_audio_api set_sink_id_sync) with X_ITE's rules: the current sink and "" are left alone or select the default,
an unknown id changes nothing and logs nothing (recorded, 3 identical runs). The entry stays PARTIAL for the platform limits. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i81", "ledger.json")
SNAP_SHA256 = "912609594f6f2897ce45c08cf46d5cab9a5d958b05b5eeb335911f38dd6aaa88"

T = ["crates/x3d-runtime/src/sound/sink.rs tests (known_device_is_selected, unknown_id_leaves_the_output_alone, empty_id_is_the_default_device, current_sink_is_not_opened_again, id_match_is_exact, device_that_will_not_open_keeps_the_output, follows_the_recorded_baseline; native_context_selects_a_listed_device is #[ignore], Linux only)"]
FIELDS = {"tests": lambda old: old + T, "oracle": "pinned X_ITE 16.5.0 (34cdf9f97): oracle/web-audio-sink-baseline/sink-baseline.json (3 identical runs)",
    "missing": "macOS (CoreAudio) not run; one native context serves all AudioDestination nodes, so the last mediaDeviceID set decides the device (X_ITE: one audio element per node); native ids are hashed names, so Chrome's \"default\" alias is unknown natively; the headless host has one output, so a real device id is not in the baseline"}
NOTE = "Issue #81 2026-10-31: select_sink (sound/sink.rs) mirrors set_mediaDeviceID_impl__: equal to current sinkId is a no-op, empty selects the default, an unknown id or one that fails to open leaves the output where it was with no log (X_ITE catches the setSinkId rejection). Only AudioDestination acts; StreamAudioDestination is skipped. Mutations (prefix match, no empty-id case) are caught."
C = {"node:AudioDestination": (None, FIELDS, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i81.py check|apply")
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
