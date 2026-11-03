#!/usr/bin/env python3
"""Ledger step after #346 and #347 (issue #90): python3 -I ledger_i347.py check|apply

Two entries, text only: node:MovieTexture (#346, native WebM decode limits and no panic) and node:AudioClip (#347,
decodeAudioData parity for MP3, FLAC and Ogg Vorbis, A-law and mu-law). Both stay PARTIAL."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i347", "ledger.json")
SNAP_SHA256 = "90bdae17826024add0f2271424f9edd661b3622ea8d3916c06d3aea1252fb64b"

def add(text):
    return lambda old: old + "; " + text
C = {
 "node:MovieTexture": (None, {}, "Issue #90 2026-11-02 (PR #346): the native WebM decode refuses pictures above 35 651 584 luma samples and stops at a 512 MiB budget per frame, and a crafted WebM no longer panics (mutation tests). No pinned behavior changed. Status stays PARTIAL."),
 "node:AudioClip": (None, {"missing": add("AAC/M4A decode differs from Chromium (not implemented: symphonia does not expose the edit list that trims the priming frames, the frame count rule for the 48 kHz file is not derived, and the default moov-last layout is not seekable in the web-audio-api reader; the existing AAC patent exposure is an owner decision); Opus is rejected natively")},
  "Issue #90 2026-11-02 (PR #347): decodeAudioData parity. MP3, FLAC and Ogg Vorbis are decoded with symphonia at the file's own rate and resampled with the Chromium SincResampler port that the WAV path already used; A-law and mu-law WAV are decoded. Measured against the pinned recordings (rea_b24_audio_decode.rs; oracle/rea-b24-audio-decode-2-baseline, 13 clean-room files, five identical Chromium recordings of the pinned X_ITE 16.5.0 (34cdf9f97)). The AAC difference is pinned by known_aac_differs_from_chromium. node:Sound: no change. Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i347.py check|apply")
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
