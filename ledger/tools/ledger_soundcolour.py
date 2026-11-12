#!/usr/bin/env python3
"""Ledger step of Phase 7A (Linux realtime audio lifecycle, PR #560) and Phase 7C (WebM container Colour, PR #562), 2026-11-12.
python3 -I ledger/tools/ledger_soundcolour.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-soundcolour", "ledger.json")
SNAP_SHA256 = "11eeb285695d6ac7a8abca09f0837c2a591dccd2375c7dec207adfef38148e3f"

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
   "missing": swap("audible realtime output is verified on Apple M1 only (see the notes) and on no Linux host; ", "audible realtime output is verified on Apple M1 (CoreAudio) and on one Linux desktop (PipeWire 1.0.5 through cpal ALSA; see the notes), not on other Linux audio stacks (plain ALSA, PulseAudio, JACK) or on Wayland-only hosts; "),
   "tests": append("crates/x3d-viewer/tests/realtime_audio_lifecycle.rs (ignored, needs an output device: an OscillatorSource plays, pauses by pauseTime, resumes by resumeTime and the viewer is dropped, stepped on the wall clock; the event order, 2 s spacing and the running context clock are checked)"),
  }, "2026-11-12 (Phase 7A, Linux realtime): on the Linux desktop (PipeWire 1.0.5 with pipewire-pulse; cpal ALSA through the ALSA plug-in; default sink Starship/Matisse HD Audio analog stereo, ALC1220) realtime_audio_lifecycle passed, and the default sink monitor showed the 440 Hz tone (peak 0.1982 for gain 0.2) on within 29 ms of the play event, off within 10 ms of the pause, on within 5 ms of the resume and off within 2 ms of the drop; the PipeWire stream node was removed after the drop. The owner heard the tone in each run. The monitor is the signal at the sink, not an acoustic recording."),
 "node:MovieTexture": (None, {
   "missing": swap(", and the WebM container Colour element is not read (the bitstream description is used)", "; a valid WebM container Colour element (matrix, range, transfer and primaries all known, as Chromium's gfx::ColorSpace::IsValid requires) replaces the VP8 or VP9 bitstream description, as Chrome's VpxVideoDecoder does (#562; BT.601, BT.709, SMPTE-240 and BT.2020 matrices, limited and full range), while a container RGB, FCC, YCoCg or YDZDX matrix or a derived range keeps the bitstream description (not measured)"),
   "tests": append("crates/x3d-runtime/src/webm.rs tests the_colour_element_of_the_video_track_is_read, a_colour_config_needs_all_four_fields_known and a_valid_container_colour_wins_over_the_bitstream (8 authored streams in tests/data/webm-colour whose container Colour and bitstream disagree)"),
  }, "2026-11-12 (Phase 7C, WebM Colour, #562): 8 authored streams (ffmpeg testsrc2; vp9_metadata rewrites only the bitstream color bits) against pinned X_ITE 16.5.0 in headless Chrome 156: the 4 whose container and bitstream tags disagree (BT.601 and BT.709 both ways, full against limited range, VP8 with a BT.709 container) were 28% to 48% of the picture more than 8 off (max 40) and are within 3 levels after; a matrix-only Colour is not a valid config and keeps the bitstream description in Chrome and in the port."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_soundcolour.py check|apply")
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
