#!/usr/bin/env python3
"""Ledger step of Phase 8F (MP4 video and audio through platform decoders, ADR 0015, PRs #567-#570), 2026-11-12.
python3 -I ledger/tools/ledger_mp4.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-mp4", "ledger.json")
SNAP_SHA256 = "dc147a2a8131d7ac4adc8a6a654f4466b634776f150adcd7260c037fce337aa1"

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
 "node:MovieTexture": (None, {
   "missing": lambda old: swap("the browser <video> element in ISO timing mode is only seeked and is silent (#384, not verified, no browser in that run)", "the browser <video> element of a WebM or Ogg video in ISO timing mode is only seeked and is silent (#384, not verified, no browser in that run), and in pinned timing its sound plays through the element, not through a Sound node")(swap("frames of MP4 urls are not decoded natively", "MP4 (ADR 0015) is decoded only by the platform: GStreamer with the non-default system-video-gst feature (Linux) and AVFoundation with the non-default system-video-avf feature (macOS); a build without them fails an MP4 with X_ITE's messages, Windows has no backend (no approved test environment), AV1 in MP4 is Unsupported on both measured hosts (GStreamer 1.24.2 qtdemux not-negotiated; Apple M1 Cannot Decode), the decoded media are held in memory under the 512 MiB limit (about 53 s at 320x240, 14 s at 640x480, 4.6 s at 1280x720), the browser's MP4 sound path (decodeAudioData, through a Sound node) is not compared with a pinned recording, and the browser chooses that path from the url (an .mp4, .m4v or .m4a path or a video/mp4 or audio/mp4 data: url), so an MP4 under another name plays its sound through the element")(old)),
   "rust": append("crates/x3d-runtime/src/mp4probe.rs", "crates/x3d-runtime/src/sysvideo/mod.rs", "crates/x3d-runtime/src/sysvideo/gst.rs", "crates/x3d-runtime/src/sysvideo/avf.rs", "crates/x3d-runtime/src/nodes/movie_texture/platform_media.rs", "crates/x3d-web/web/video_mirror.mjs"),
   "tests": append(
     "crates/x3d-runtime/tests/mp4_probe.rs (6 cases: codec, size, rate, time scale and empty edits of each synthetic fixture; damaged data never panics; unknown codec and damaged data fail like the element; the next candidate loads)",
     "crates/x3d-runtime/tests/mp4_platform.rs (13 cases + 1 ignored, features system-video-gst or system-video-avf: decode against the known pictures and tones; HEVC; AV1 names its codec when unsupported; late audio; video-only and audio-only; sync on the engine's offline audio render through a Sound node over loop, pause/resume, seek, speed 0.5 and 2, ISO and pinned, largest lag 0.030 s; drift correction, red without it; limit; unload; no sound without a Sound node; Linux GStreamer 1.24.2 and CI, Apple M1 AVFoundation)",
     "crates/x3d-web/tests/browser/mp4sound01.test.mjs (3 cases: an MP4's sound heard through a Sound node with the element muted; a replacing world is heard and the old element is released; silent without a Sound node)",
     "crates/x3d-runtime/tests/movie_iso_time.rs an_mp4_video_asks_the_page_for_its_sound_and_plays_it_on_the_timeline"),
  }, "2026-11-12 (Phase 8, ADR 0015, #566-#570): MP4 video and audio through platform decoders. Linux GStreamer (gstreamer-rs, decodebin3) and macOS AVFoundation (AVAssetReader) decode the pictures and the sound with their presentation times; the engine puts both on one timeline from the container times and plays them as one media; the movie time is the clock and the sound is moved back to it when it drifts by more than max(2 picture periods, 40 ms). The sound is heard only through a Sound node whose source is the MovieTexture (pinned MovieTexture.js:118-119, Sound.js:180-209). Measured: Linux (nvh264dec, avdec_h265, avdec_aac; CI avdec_h264) and Apple M1 (VideoToolbox, AudioToolbox): 22 of 22 MP4 tests each at the 8C and 8D heads (the later no-Sound-node case ran on Linux only); real output recorded on Linux (the 8 tones in order) and heard by the owner on Linux and on the M1. Browser: an MP4 element is muted and its sound, decoded by decodeAudioData, plays through the engine's graph on the movie time. Stays PARTIAL: AV1 in MP4, Windows, the memory limit, WebM element sound and the other gaps above remain."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_mp4.py check|apply")
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
