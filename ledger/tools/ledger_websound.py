#!/usr/bin/env python3
"""Ledger step of Phase 9B (WebM and Ogg sound in the browser through the engine's graph, Phase 9A, PR #573), 2026-11-12.
python3 -I ledger/tools/ledger_websound.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-websound", "ledger.json")
SNAP_SHA256 = "cf2da25176fb18ab0bcf1a4dec52aeae44cd0102b424b5e873c196d9d9cee073"

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
   "missing": lambda old: swap("the browser <video> element of a WebM or Ogg video in ISO timing mode is only seeked and is silent (#384, not verified, no browser in that run), and in pinned timing its sound plays through the element, not through a Sound node", "in the browser the sound of a WebM or Ogg file (Opus, Vorbis) plays through a Sound node in ISO and pinned timing, with the leading silence from the container within one audio block (about 20 ms; Phase 9A, measured in headless Chromium 151), and Chromium 151 decodes no Theora picture, so an Ogg video there has its sound and no picture")(swap("the browser's MP4 sound path (decodeAudioData, through a Sound node) is not compared with a pinned recording, and the browser chooses that path from the url (an .mp4, .m4v or .m4a path or a video/mp4 or audio/mp4 data: url), so an MP4 under another name plays its sound through the element", "the browser's MP4, WebM and Ogg sound path (decodeAudioData, through a Sound node) is not compared with a pinned recording, and the browser chooses that path from the url (an .mp4, .m4v, .m4a, .webm, .ogv or .ogg path or a video/mp4, audio/mp4, video/webm or video/ogg data: url), so a file under another name plays its sound through the element")(old)),
   "rust": append("crates/x3d-runtime/src/ebml.rs", "crates/x3d-runtime/src/oggtime.rs"),
   "tests": append(
     "crates/x3d-web/tests/browser/webmsound01.test.mjs (16 cases, ISO and pinned timing: VP8+Opus and VP8+Vorbis WebM and Theora+Vorbis and Theora+Opus Ogg heard through a Sound node with the element muted on a blob: URL, the WebM picture 64 x 48 and the Ogg element 0 x 0; silent without a Sound node; pauseTime, resumeTime, stopTime and startTime act on the sound; scene replacement WebM to Ogg to an empty world: only the new world is heard and the old elements are released)",
     "crates/x3d-runtime/src/ebml.rs tests (first block times of video and audio; a file without audio; cut, damaged and short-block data, a cluster time above i64::MAX, a track without a TrackNumber and an empty block never panic or set a time)",
     "crates/x3d-runtime/src/oggtime.rs tests (Theora and Vorbis or Opus start times against ffprobe; an offset file whose first Theora granule is not 0; not Ogg; cut and damaged data never panic)",
     "crates/x3d-runtime/tests/movie_iso_time.rs the_sound_lead_comes_from_the_container, a_webm_or_ogg_video_sound_follows_start_pause_resume_and_stop"),
  }, "2026-11-12 (Phase 9A, #573): WebM and Ogg sound in the browser through the engine's graph. The element of a WebM or Ogg file is muted as for an MP4; the page decodes the sound with decodeAudioData and the engine plays it through the node's audio source on the movie time in both timings, so only a Sound node whose source is the MovieTexture makes it heard (pinned MovieTexture.js:118-119, Sound.js:180-209); in ISO timing it was not heard before (#384 note). Measured in headless Chromium 151: VP8 WebM pictures play and Opus and Vorbis decode; Theora is not decoded (canPlayType empty, element 0 x 0) while the Ogg audio plays and decodes; decodeAudioData drops a leading 0.5 s of the late files and the element plays that sound from 0.499 to 0.519 s, and the engine restores the lead from the WebM block times and the Ogg granule positions. No native Opus decoder (check_no_opus.py). Stays PARTIAL: no pinned recording of this sound, the url dispatch, no Theora picture in Chromium, and the other gaps above remain."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_websound.py check|apply")
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
