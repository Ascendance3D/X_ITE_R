#!/usr/bin/env python3
"""Ledger step of the arm64 oscillator lookup and the M1 audible run (2026-11-12), and the BufferAudioSource text after #347.
python3 -I ledger/tools/ledger_soundneon.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-soundneon", "ledger.json")
SNAP_SHA256 = "1a67f93037c019b2d2de7171a898e234ed9eae0d78dda8fc3cdcc6422467a33e"

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

C = {
 "sub:sound.engine": (None, {
   "missing": lambda old: swap("audible realtime output (cpal) is not verified on any host; on Apple M1 (macOS 27.2.1) the engine builds and the CoreAudio output opens and selects its device, but", "audible realtime output is verified on Apple M1 only (see the notes) and on no Linux host; on Apple M1 (macOS 27.2.1) the engine builds and the CoreAudio output opens and selects its device, but")(swap("33 of the 61 match the Mac Chrome recording exactly, and 28 (biquad-lowpass, the convolver and conv-delta worlds, waveshaper-2x and -4x, the rt-* automation worlds, enabled-toggle, remove-children, selector, splitter-outputs) differ from it by at most 4.4e-6; the port follows Chromium's x86 paths (SSE and AVX kernels and reduction order, scalar biquad: biquad.rs, convolver.rs, oscillator.rs), and which arm64 macOS path (vDSP, NEON) makes each of the 28 differences is not identified", "37 of the 61 match the Mac Chrome recording exactly, and 24 (biquad-lowpass, the convolver and conv-delta worlds, waveshaper-2x and -4x, the rt-* automation worlds) differ from it by at most 4.4e-6; the oscillator's a-rate table lookup follows Chromium's NEON arithmetic on arm64 (oscillator.rs); for the rest the port follows Chromium's x86 paths (SSE and AVX kernels and reduction order, scalar biquad: biquad.rs, convolver.rs), and Chromium's macOS build uses Accelerate there (vDSP_deq22D in biquad.cc, mac/vector_math_mac.h for the vector operations), which the port does not call")(old)),
   "tests": append("crates/x3d-runtime/src/sound/oscillator.rs tests::the_a_rate_table_lookup_follows_the_target_arithmetic (the a-rate lookup uses the reciprocal form of Chromium's CPU_ARM_NEON build on arm64 and the division form elsewhere)"),
  }, "2026-11-12 (arm64 oscillator lookup and M1 audible run): Chromium's four-frequency WaveDataForFundamentalFrequency multiplies by float reciprocals on NEON and divides on x86; with the NEON form on arm64, the selector, enabled-toggle, remove-children and splitter-outputs worlds equal pinned X_ITE in Chrome on the same Apple M1 sample for sample (they were 35 to 70 samples off in the first quantum, the a-rate start), and the other worlds are unchanged; Linux x86-64 is unchanged (rea_b05 126 pass, 1 known, 2 blocked). Realtime on the M1 (web-audio-api AudioContext, MacBook Pro Speakers, 48 kHz): play, suspend (clock stops), resume and close each changed the context state and the CoreAudio device running state, and the owner heard both tones; the microphone capture of the SSH session got no input, so no recording of the sound exists."),
 "node:BufferAudioSource": (None, {
   "missing": lambda old: "a resampled file (WAV, MP3, FLAC, Ogg Vorbis, A-law, mu-law: decode.rs ports Chromium's SincResampler) has Chromium's length but is within 5e-5 of Chromium, not exact; Opus: Chromium decodes it, the port rejects it; AAC in M4A is excluded by owner decision (issue #90) and Chromium decodes it, so its parity is UNSUPPORTED",
   "tests": swap_in_list("known_ogg_48000_chromium_drops_the_last_64_frames, known_resampled_non_wav_differs_from_chromium, known_opus_is_rejected_by_the_port", "ogg_48000_has_chromiums_length_and_samples, resampled_compressed_files_match_chromium, more_rates_and_g711_match_chromium, aac_is_excluded_and_chromium_parity_is_unsupported, known_opus_is_rejected_by_the_port"),
  }, "2026-11-12 (text after #347): the gap text described the state before #347; decode.rs decodes MP3, FLAC and Ogg Vorbis at their own rate and resamples them with the SincResampler port, and the rea_b24_audio_decode tests (8 of 8 pass) measure Chromium's length and at most 5e-5 for every resampled file, and Chromium's samples for Ogg at 48 kHz (14336 frames)."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_soundneon.py check|apply")
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
