#!/usr/bin/env python3
"""Ledger step of Phase 7D (the macOS biquad feedback precision, PR #564), 2026-11-12.
python3 -I ledger/tools/ledger_biquadmac.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-biquadmac", "ledger.json")
SNAP_SHA256 = "46a3e5e7d8b3aa3d78b6af10be558b1480999fd43f0238cc4009f017d8bb49b1"

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
   "missing": swap("37 of the 61 match the Mac Chrome recording exactly, and 24 (biquad-lowpass, the convolver and conv-delta worlds, waveshaper-2x and -4x, the rt-* automation worlds) differ from it by at most 4.4e-6; the oscillator's a-rate table lookup follows Chromium's NEON arithmetic on arm64 (oscillator.rs); for the rest the port follows Chromium's x86 paths (SSE and AVX kernels and reduction order, scalar biquad: biquad.rs, convolver.rs)", "46 of the 61 match the Mac Chrome recording exactly, and 15 (the convolver and conv-delta worlds, waveshaper-2x and -4x, and rt-over and runtime, whose WaveShaper oversamples) differ from it by at most 5.4e-7; the oscillator's a-rate table lookup follows Chromium's NEON arithmetic on arm64 (oscillator.rs), and a biquad quantum with fixed coefficients keeps Chromium's macOS double feedback on macOS (biquad.rs, #564); for the rest the port follows Chromium's x86 paths (SSE and AVX kernels and reduction order: convolver.rs)"),
   "tests": append("crates/x3d-runtime/src/sound/biquad.rs tests the_macos_form_keeps_the_unrounded_feedback_of_fixed_coefficients, sample_accurate_coefficients_use_the_float_form_on_every_platform and only_macos_builds_keep_double_feedback"),
  }, "2026-11-12 (Phase 7D, #564): Chromium's Biquad::Process under BUILDFLAG(IS_MAC) runs fixed coefficients through vDSP_deq22D on double buffers and keeps y1/y2 as the unrounded doubles (not flushed); the other builds feed back the float output. With that form on macOS, biquad-lowpass and rt-curve, -delay, -fdetune, -gain, -osc, -q, -stop and -type equal pinned X_ITE in Chrome on the Apple M1 exactly (up to 4.4e-6 before); rt-over and runtime go from about 1.2e-6 to 1.19e-7; no other world changed. Linux x86-64 rea_b05 is unchanged (126 pass, 1 known, 2 blocked, 0 fail). Intel Macs are covered by the same cfg and were not measured."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_biquadmac.py check|apply")
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
