#!/usr/bin/env python3
"""Ledger step after #363 and #397 (issue #90): python3 -I ledger_i363.py check|apply

One entry, no status change. node:MovieTexture: AV1 WebM pictures are decoded natively through rav1d (non-default `av1`
feature, #363) and compared with pinned X_ITE 16.5.0 at five fixed movie times, MSAA 0 and 4 (#397). The stale missing
text ("PR #363 is a draft", "not AV1") is corrected. The status stays PARTIAL: MP4 frames, the APNG middle frames and
MPEG-1 are unchanged."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i363", "ledger.json")
SNAP_SHA256 = "5ac61c8aae72a4838129683edb5a5a1b36e6ad73a4ca0b2d43ed4d8f63cda947"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f
C = {
 "node:MovieTexture": (None, {
   "missing": lambda old: swap(
     "VP8/VP9 WebM, not AV1 (#363) and not those APNG frames",
     "VP8/VP9 WebM and, with the non-default av1 feature, AV1 WebM (#397), not those APNG frames")(
     swap("frames of AV1 and MP4 urls are not decoded natively (PR #363 is a draft);",
          "frames of MP4 urls are not decoded natively; AV1 WebM is decoded natively through rav1d only when the non-default av1 feature is built (#363), and a build without it does not decode AV1;")(old)),
   "tests": lambda old: old + [
     "crates/x3d-runtime/src/webm.rs av1_pictures_equal_the_reference_decoder_plane_by_plane (6 synthetic AV1 streams, 8 and 10 bit, 4:2:0 and 4:4:4, tiles, a second encoder, film grain: every plane of every picture equals libaom; feature av1)",
     "crates/x3d-runtime/tests/av1_safety.rs (2 cases: damaged AV1 files in child processes never crash the process; feature av1)",
     "crates/x3d-viewer/tests/movie_anim.rs movie_video_av1_frames_match_x_ite and movie_video_av1_baseline_is_present (AV1 pictures at fixed movie times against pinned X_ITE, 5 frames per MSAA, oracle/synthetic-movie-video, MSAA 0 and 4; the first needs feature av1, the second fails when the baseline is missing)"],
   "oracle": lambda old: old + "; oracle/movie-video-av1-baseline: AV1 WebM pictures at the same five movie times (oracle/synthetic-movie-video/movie-video-av1.wrl, MSAA 0 and 4, five identical recordings each)"},
  "Issue #90 2026-11-03 (PRs #363 and #397): AV1 in WebM is decoded natively through rav1d 1.1.0 (BSD-2-Clause, vendored in vendor/rav1d with the one recorded change to src/decode.rs; the other vendored files are checked byte for byte by tools/check_vendored_rav1d.py). It is the non-default av1 feature and CI builds it and runs its tests; the comparison with pinned X_ITE needs a GPU adapter, so on the hosted runner (no adapter) it is recorded as an allowed skip, and the figures below come from strict runs on the RTX 3060 Ti. Pictures: all six synthetic streams equal the libaom reference decoder plane by plane (av1_pictures_equal_the_reference_decoder_plane_by_plane). Safety: damaged files never crash the process (av1_safety, child processes; a panic inside rav1d's C interface cannot unwind, so the tests, not catch_unwind, are the proof). Against pinned X_ITE 16.5.0 at five movie times (0.25, 0.55, 0.05, 0.35 s; frames 28, 39, 47, 55, 63): 0 pixels over 8 and a largest channel difference of 2 or 3 at MSAA 0 and at MSAA 4, five identical pinned recordings each (oracle/movie-video-av1-baseline/recordings.json). The first comparison showed 5.1 to 5.2% of the pixels over 8, all in the 10-bit and the 4:4:4 stream; two defects in the port were fixed, not the tolerance: the pinned page converts an untagged AV1 picture with the BT.601 matrix when it is not 8-bit 4:2:0 (the port used BT.709), and takes the nearest chroma sample for more than 8 bits (the port interpolated). The 8-bit 4:2:0 streams matched at once. Not measured: pictures tagged with matrix coefficients other than BT.601 and BT.709 (identity, BT.2020) are converted as BT.709, as the shared converter has two matrices. Status stays PARTIAL: MP4 frames are not decoded natively, the pinned APNG middle frames and MPEG-1 need owner records, and the browser <video> audio in ISO mode is unverified."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i363.py check|apply")
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
