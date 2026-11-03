#!/usr/bin/env python3
"""Ledger step after #371 and #377 (issues #90, #369): python3 -I ledger_i369.py check|apply

Four entries, text and the stale missing fields only: sub:parse.gltf2 and sub:parse.glb2 (the decoder gaps listed
there are closed by #73, the notices and the KTX2 guard by #377), node:MovieTexture (VP8, video sound, seeking, ISO
timing, MPEG-1 video) and node:AudioClip (AAC excluded by owner decision). No status changes."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i369", "ledger.json")
SNAP_SHA256 = "c9b84489770fc131e5701f8ec6fab36924ec4a8e373b4abc0b778f5b1485a3da"

def add(text):
    return lambda old: old + "; " + text
GLTF_TESTS = "gltf_decoders_73 (Draco and meshopt compared with pinned X_ITE 16.5.0, oracle/gltf-decoders-73-baseline); x3d-render texture::ktx_tests (KTX2/BasisU, WebP, AVIF decode and malformed-input rejection)"
C = {
 "sub:parse.gltf2": (None, {
   "missing": lambda old: "EXT_texture_video runtime not exercised; buffer-fetch console text differs in 2 Inline cases; stepped animation cases run only by x10_compare.py",
   "tests": lambda old: old + [GLTF_TESTS]},
  "Issue #369 2026-11-03: the missing field no longer lists Draco, meshopt, BasisU, AVIF and WebP as undecoded. KHR_draco_mesh_compression and EXT_meshopt_compression decode and are compared with pinned X_ITE (#73, gltf_decoders_73, an input X_ITE rejects is rejected too); KHR_texture_basisu, EXT_texture_webp and EXT_texture_avif decode (x3d-render texture.rs). PR #377: the decoder crates' license notices are in THIRD_PARTY_NOTICES.md, and a KTX2 header with dimensions whose pixel count overflows is rejected (checked arithmetic, test truncated_ktx2_and_corrupt_webp_are_rejected). Status stays PARTIAL for the rows that remain."),
 "sub:parse.glb2": (None, {
   "missing": lambda old: "none beyond the glTF rows (sub:parse.gltf2); GLB shares the decoders",
   "tests": lambda old: old + [GLTF_TESTS]},
  "Issue #369 2026-11-03: the decoder gaps (Draco, meshopt, BasisU, AVIF/WebP) named before are closed, see sub:parse.gltf2. Status stays PARTIAL."),
 "node:MovieTexture": (None, {"missing": lambda old: "frames of AV1 and MP4 urls are not decoded natively (PR #363 is a draft); MPEG-1: video is decoded natively with pl_mpeg, the Systems audio and the browser <video> integration are open and ISO/IEC 11172 is not in the local library, so conformance is not claimed; VP9 WebM is 8-bit only and assumes BT.709 whatever the stream's colour tags say"},
  "Issue #90 2026-11-03: VP8 WebM frames decode natively; the Vorbis track of a WebM is decoded and equals the reference decode (the_vorbis_track_of_a_webm_equals_the_reference_decode); a seek request and its answer are carried between the runtime and the page (VideoOp::Seek, Runtime::video_seeked), and an unusable answer is asked again (PR #371, a_seek_answered_without_a_picture_is_asked_again_at_the_same_time); the timing of ISO/IEC 19775-1 18.4.2 (movie_time.rs, tests movie_iso_time) is the default and the pinned X_ITE timing stays an option (X3D_MOVIE_TIMING=pinned, page attribute movietiming=pinned). The browser module carries no native video decoder (tools/check_no_native_video.py). Status stays PARTIAL."),
 "node:AudioClip": (None, {"missing": lambda old: old.replace("AAC/M4A decode differs from Chromium (not implemented: symphonia does not expose the edit list that trims the priming frames, the frame count rule for the 48 kHz file is not derived, and the default moov-last layout is not seekable in the web-audio-api reader; the existing AAC patent exposure is an owner decision)", "AAC/M4A is excluded from every build by owner decision 2026-11-02 (PR #357, tools/check_no_aac.py), so it is unsupported and differs from Chromium by design")},
  "Issue #90 2026-11-03: AAC is excluded from every build (vendored web-audio-api patch, PR #357); the earlier wording that the AAC exposure awaits an owner decision is replaced by the exclusion. Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i369.py check|apply")
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
