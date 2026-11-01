#!/usr/bin/env python3
"""Issue #98 ledger update (node:BufferAudioSource): python3 -I ledger_i98.py check|apply

MP3, Ogg Vorbis, FLAC, Opus and WAV files are now compared with Chromium's decodeAudioData in pinned X_ITE 16.5.0 (rea_b24_audio_decode: 11 synthetic files,
5 identical recordings). A 48 kHz MP3 and a 48 kHz FLAC are bit-identical; resampled files differ. The entry stays PARTIAL with the measured differences. No
DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i98", "ledger.json")
SNAP_SHA256 = "dcb5a64d9393146204a377aaced61703e565cea9f9205430fed8db939f4930c3"

T = ["crates/x3d-runtime/tests/rea_b24_audio_decode.rs (same_rate_mp3_and_flac_are_bit_identical, resampled_wav_is_within_5e_5_of_chromium, known_ogg_48000_chromium_drops_the_last_64_frames, known_resampled_non_wav_differs_from_chromium, known_opus_is_rejected_by_the_port)"]
FIELDS = {"tests": lambda old: old + T,
    "oracle": lambda old: old + "; Chromium decodeAudioData of 11 synthetic files (3 MP3, 3 Ogg, 3 FLAC, 1 Opus, 1 WAV; oracle/rea-batch24-audio-decode-baseline, 5 identical recordings)",
    "missing": "non-WAV files whose rate differs from the 48 kHz context are resampled by web-audio-api (rubato), not like Chromium: max abs difference 0.33 to 0.46 and 14401 frames against Chromium's 14400 (MP3, FLAC) or 13931 and 14349 (Ogg); Ogg 48 kHz mono: Chromium returns 14336 frames and drops the last 64, the port 14400; Opus: Chromium decodes it, the port rejects it; a resampled WAV is within 5e-5 of Chromium, not exact"}
NOTE = "Issue #98 2026-10-31: exact for a 48 kHz MP3 and FLAC. The known differences are pinned by tests that fail once a file starts matching Chromium. A fix needs the file's native-rate frames, which web-audio-api does not expose."
C = {"node:BufferAudioSource": (None, FIELDS, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i98.py check|apply")
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
