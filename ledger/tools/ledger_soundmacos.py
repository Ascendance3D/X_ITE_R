#!/usr/bin/env python3
"""Ledger step of the sound engine on macOS (2026-11-12): Apple M1 build, CoreAudio device test and offline audio against pinned X_ITE in Chrome on the same Mac.
python3 -I ledger/tools/ledger_soundmacos.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-soundmacos", "ledger.json")
SNAP_SHA256 = "1687f690f5e68f0bba1c70b6bf14892b96a6804012a4c75b53dc2649bb047456"

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
   "missing": swap("built and run on Linux x86-64 only: the macOS 15+ build and the realtime output path (cpal) are not verified", "audible realtime output (cpal) is not verified on any host; on Apple M1 (macOS 27.2.1) the engine builds and the CoreAudio output opens and selects its device, but the offline audio of 61 of the 127 compared rea_b05 worlds differs from the x86-64 baselines by at most 3.6e-6, and pinned X_ITE in Chrome on the same Mac differs from those baselines in all 61 too (at most 4.0e-6): 33 of the 61 match the Mac Chrome recording exactly, and 28 (biquad-lowpass, the convolver and conv-delta worlds, waveshaper-2x and -4x, the rt-* automation worlds, enabled-toggle, remove-children, selector, splitter-outputs) differ from it by at most 4.4e-6; the port follows Chromium's x86 paths (SSE and AVX kernels and reduction order, scalar biquad: biquad.rs, convolver.rs, oscillator.rs), and which arm64 macOS path (vDSP, NEON) makes each of the 28 differences is not identified"),
  }, "2026-11-12 (sound on macOS, Apple M1, macOS 27.2.1): x3d-drive release build OK; rea_b05.py check against the x86-64 baselines: 65 pass, 1 known, 2 blocked, 61 fail, all 61 by at most 3.6e-6 (no shape, text, log, print or suspend difference). The 61 worlds were recorded again with pinned X_ITE 16.5.0 in Chrome 156 on the same Mac (oracle/x_ite-anim.mjs with the Mac Chrome path, ANGLE Metal): Chrome itself differs from the x86-64 baselines in all 61 (at most 4.0e-6); x3d-drive on the Mac equals that recording sample for sample in 33 worlds (gain, the oscillators, the spatial and listener worlds, delay, merger, analyser, compressor, biquad-highshelf and -peaking, waveshaper-none and others) and differs by at most 4.4e-6 in 28 (biquad-lowpass, convolver and conv-delta, waveshaper-2x and -4x, rt-* automation and four gain-routing worlds). The 65 passing worlds were not recorded on the Mac. The x86-64 baselines are not changed and no tolerance was added. x3d-runtime on the Mac: 361 of 363 pass; the 2 others are the known issue #88 tests (late_connected_oscillator_keeps_its_phase, same_rate_mp3_and_flac_are_bit_identical: about 1e-7 against x86-64 values). Also: glibc sinf and Apple sinf differ by 1 ulp at 3 of the 2048 web-audio-api sine table entries (glibc correctly rounded); a double-precision table did not change any compared world."),
 "node:AudioDestination": (None, {
   "missing": swap("macOS (CoreAudio) not run; ", ""),
   "tests": swap_in_list("native_context_selects_a_listed_device is #[ignore], Linux only)", "native_context_selects_a_listed_device is #[ignore]: run on Linux (ALSA) and on Apple M1, macOS 27.2.1 (CoreAudio, one output device), both pass)"),
  }, "2026-11-12 (CoreAudio): native_context_selects_a_listed_device run on Apple M1 (macOS 27.2.1, MacBook Pro Speakers the only output): the listed device is selected, an unknown id leaves the output alone, the empty id selects the default; PASS. sound:: unit tests on the Mac: 18 of 18 pass."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_soundmacos.py check|apply")
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
