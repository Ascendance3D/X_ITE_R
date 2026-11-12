#!/usr/bin/env python3
"""Ledger step of the VP9 color description (2026-11-12): tagged VP9 streams against the pinned page.
python3 -I ledger/tools/ledger_vp9color.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-vp9color", "ledger.json")
SNAP_SHA256 = "233103a29d34de3b94df5aca5228c50382f19af8140a2d7176be0d0d25e3c565"

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
   "tests": append('crates/x3d-runtime/src/webm.rs the_color_description_is_read_from_real_key_frames, tagged_vp9_streams_decode_with_their_color_description, a_444_bt601_stream_keeps_the_standard_conversion (crates/x3d-runtime/tests/data/vp9-color: 11 authored VP9 streams, profiles 0 to 2, against the colors pinned X_ITE 16.5.0 drew in headless Chrome 156; 444-bt601-no-primaries shows that the 4:4:4 conversion matches without the primaries tag)'),
   "missing": swap("VP9 WebM is 8-bit only and assumes BT.709 whatever the stream's colour tags say", "VP9 WebM reads the color description of its key frames (BT.601, SMPTE-170, BT.709, SMPTE-240, BT.2020, sRGB; limited and full range) and decodes 10 and 12 bit streams; a 4:4:4 stream whose WebM container tags BT.470BG color primaries is a known difference (Chrome converts those primaries to sRGB on its 4:4:4 path; the port does not reproduce browser color management, as for tagged images: blue about 28 levels higher where U is far below 128; without the primaries tag, or with BT.709 primaries, the stream matches), and the WebM container Colour element is not read (the bitstream description is used)"),
  }, "2026-11-12 (VP9 color description): 11 authored VP9 streams (ffmpeg testsrc2, libvpx-vp9: untagged, BT.601, SMPTE-170, BT.709, BT.601 and BT.709 full range, 4:4:4 BT.601, 10 bit untagged and BT.709, 10 bit BT.2020, sRGB) were drawn by pinned X_ITE 16.5.0 (x_ite-shot, headless Chrome 156, ANGLE on NVIDIA) and by x3d-shot. X_ITE gives the video element to texImage2D with UNPACK_COLORSPACE_CONVERSION_WEBGL BROWSER_DEFAULT (X3DTexture2DNode.setTextureData; REA ev_ca3b36632662dd682dbd9e9cd8cfc93389da65dfcfa44ed3eda4564abc8aa013), so the browser chooses the matrix and range. Before: only untagged and BT.709 matched; BT.601 was up to 40 levels off, full range up to 32, 10 bit gave no picture, sRGB was converted as YUV. Now 10 of 11 are within the still bound (0.5% of pixels more than 8 off), sRGB exact; the 4:4:4 BT.601 stream is 7.4% over (was 28.6%). Six tag variants show the cause: only the container's BT.470BG primaries tag on a 4:4:4 stream makes the difference (Chrome's gamut conversion to sRGB); the same stream without it or with BT.709 primaries matches (max 1), so it is pinned as a known difference of browser color management, not a decoder defect."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_vp9color.py check|apply")
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
