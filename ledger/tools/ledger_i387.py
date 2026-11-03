#!/usr/bin/env python3
"""Ledger step after #381, #384 and #387 (issue #90) and #337 (issue #69): python3 -I ledger_i387.py check|apply

Two entries, no status change. node:MovieTexture: ISO sound and pause/stop picture (#384), MPEG-1 Systems audio (#381) and
WebM pictures at fixed movie times against pinned X_ITE (#387); the stale missing text about the MPEG-1 Systems audio is
corrected. node:GaussianSplats: the equal-value castShadow event is resolved (#337 evidence), the sort-timing text is kept.
node:Appearance is COMPLETE and unchanged."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i387", "ledger.json")
SNAP_SHA256 = "a8b7c74f0a597d9842d3a195bd20b6775135d80a776a35e0ba0913a230140467"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f
def add(text):
    return lambda old: old + [text]
C = {
 "node:MovieTexture": (None, {
   "missing": swap("MPEG-1: video is decoded natively with pl_mpeg, the Systems audio and the browser <video> integration are open and ISO/IEC 11172 is not in the local library, so conformance is not claimed",
     "MPEG-1: video is decoded natively with pl_mpeg and the Systems audio (Layer II and III) natively with symphonia (#381), the browser <video> integration and the audio/video start-offset alignment are open and ISO/IEC 11172 is not in the local library, so conformance is not claimed; the browser <video> element in ISO timing mode is only seeked and is silent (#384, not verified, no browser in that run); APNG frames between the first and the last are not recordable against the pinned page (apng-js runs on the wall clock), so the animated-media rendering comparison covers GIF, APNG first and last frames and VP8/VP9 WebM, not AV1 (#363) and not those APNG frames (an owner record is pending)"),
   "tests": lambda old: old + [
     "crates/x3d-viewer/tests/movie_anim.rs movie_video_frames_match_x_ite (WebM VP8/VP9 pictures at fixed movie times against pinned X_ITE, 5 frames per MSAA, oracle/synthetic-movie-video, MSAA 0 and 4)",
     "crates/x3d-runtime/tests/movie_iso_audio.rs (6 cases: ISO sound rate, backwards from the end for a negative speed, resume at the movie time, pinned timing unchanged, offline render of a falling ramp)",
     "crates/x3d-runtime/tests/movie_iso_time.rs (pause and stop keep the picture of the instant)",
     "crates/x3d-runtime/tests/mpeg1_audio.rs (15 cases: MPEG-1 Systems Layer II and III audio equals ffmpeg within 2e-4, truncation and mutation)"],
   "oracle": lambda old: old + "; oracle/movie-video-baseline: WebM VP8/VP9 pictures after inter frames, backward seeks and seeks into a group of pictures (oracle/synthetic-movie-video, MSAA 0 and 4, five identical recordings)"},
  "Issue #90 2026-11-03 (PRs #381, #384, #387): (1) ISO timing, sound (#384): the sound of a movie plays at the signed rate speed * max(pitch, 0), so a negative speed reads the buffer backwards from the end, starts at the movie time of the first picture, resumes at the engine's movie time after a pause, and a pause or stop keeps the picture of the movie time at that instant (ISO/IEC 19775-1 18.4.2: the frame at the time the node became inactive remains); the pinned timing is unchanged and kept by a test (movie_iso_audio 6 cases, movie_iso_time 2 added, 309 passed in x3d-runtime). Not verified: the browser <video> element in ISO mode. (2) MPEG-1 Systems audio (#381): Layer II and III decode natively and equal ffmpeg 6.1.1 within 2e-4 per sample on four synthetic streams (worst measured 5.3e-7); Layer I is reported unsupported; ISO/IEC 11172 is not in the local library, so no conformance claim. (3) Pictures (#387): WebM VP8 and VP9 frames at fixed movie times (0.25, 0.55 s, a backward seek to 0.05 s, a seek back into a group of pictures to 0.35 s, and before the start) match pinned X_ITE 16.5.0 at MSAA 0 and MSAA 4: 5 frames per MSAA, five identical pinned recordings, 0 pixels over 8, max delta 1; a control with one changed seek time fails (6.35% over 8). The comparison puts the pinned element at a media time with the harness's own pause and seek (movieSeek), so it compares the picture of a media time, not the pinned playback timing. ISO/IEC 19775-1 V4.0 MovieTexture clause number unverified (no local clause text; 18.4.2 is the timing clause named above); 19775-2 unverified. Status stays PARTIAL: AV1 decode awaits #363, the pinned APNG middle frames and MPEG-1 (the pinned Chrome cannot play it) need owner records, and the browser <video> audio in ISO mode is unverified."),
 "node:GaussianSplats": (None, {
   "missing": swap("both follow the field after an event (#327; whether an event with an equal value counts is unverified)",
     "both follow the field after an event (#327), and an event whose value equals the field's current value counts (#337: gs-shadow-event-false sets castShadow FALSE from a Script at initialize on a node whose SFBool field is already FALSE, and the pinned picture, rgba sha256 94cb5628e3f4, has no shadow, whereas the same world with castShadow FALSE written in the file (gs-shadow-field-false, 13729bbf0660) still casts one; X_ITE_R matches both at MSAA 0 and 4)"),
   "tests": lambda old: old + ["crates/x3d-render/tests/render_b69_gs_shadow.rs (16 shots, castShadow event worlds incl. gs-shadow-event-false and gs-shadow-field-false; oracle/render-b69-gs-shadow-baseline, oracle/render-msaa4-b69-gs-shadow-baseline)"]},
  "Issue #69 2026-11-03 (wording, PRs #327 and #337): the earlier statement that it is unverified whether a castShadow event with a value equal to the field's value counts is resolved. Pinned X_ITE 16.5.0 (three identical recordings): gs-shadow-event-false (Script sets castShadow FALSE at initialize; the SFBool field was already FALSE) renders without the shadow (rgba sha256 94cb5628e3f4), gs-shadow-field-false (FALSE written in the file) renders with it (13729bbf0660, equal to gs-shadow-event-true), so the file value is not honored and an event of equal value is. Rust matches at MSAA 0 and 4 (render_b69_gs_shadow; MSAA 4 max channel difference 1). The sort-timing text (splatSort.flip, one transient frame, synchronous sort by decision #309) is unchanged and no new sort measurement is cited. Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i387.py check|apply")
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
