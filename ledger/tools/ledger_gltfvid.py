#!/usr/bin/env python3
"""Ledger step of the glTF video pictures (2026-11-11): EXT_texture_video textures against the pinned page.
python3 -I ledger/tools/ledger_gltfvid.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-gltfvid", "ledger.json")
SNAP_SHA256 = "33ff2b88048848b35e32a132cb0ba3a67c4451789039df3bfc98fe9f75e841ae"

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
 "sub:parse.gltf2": (None, {
   "tests": append('crates/x3d-viewer/tests/movie_anim.rs gltf_video_frames_match_x_ite (oracle/gltf-video-baseline: gv-uri, gv-data, gv-glb at MSAA 0 and 4, pinned X_ITE 16.5.0, five identical recordings; scene text, log and 24 pictures at four movie times, 0 pixels more than 8 off)'),
   "missing": swap("EXT_texture_video: scene construction and load-failure diagnostics are compared (gltf_fetch gf-video), playback frames of a glTF video are not compared;", "EXT_texture_video: scene construction and load-failure diagnostics are compared (gltf_fetch gf-video), and the pictures of VP8 and VP9 video textures at four movie times are compared (gltf_video_frames_match_x_ite: external files, data URLs and GLB buffer views, MSAA 0 and 4); not compared: free-running playback between seeks (the page's video element runs on the wall clock), MP4 sources (not decoded, node:MovieTexture);"),
  }, "2026-11-11 (glTF video pictures): three authored glTF files whose four textures have only an EXT_texture_video source (external VP8 and VP9 WebM files, data URLs, buffer views of a GLB; oracle/tools/gen_gltf_video.py) are recorded at four movie times (movieSeek of the recorder) and compared. The comparison found three defects of the port, now fixed: an UnlitMaterial did not draw a MovieTexture as its emissiveTexture (every glTF video quad was white); a video in a data URL did not load because the type was read from the file name only (the pinned parser gives a GLB buffer view to the node as a data URL too); x3d-drive's movieSeek did not pause the element as the recorder does, so a node at speed 1 (a glTF file cannot set speed) showed a later picture. Before: 19.1% of each picture more than 8 off; now 0 pixels, largest difference 1. Pinned: a file without an images array does not load (TypeError), so the worlds carry an empty one. The codecs are VP8 and VP9 only."),
 "sub:parse.glb2": (None, {
   "tests": append('crates/x3d-viewer/tests/movie_anim.rs gltf_video_frames_match_x_ite (oracle/gltf-video-baseline: gv-uri, gv-data, gv-glb at MSAA 0 and 4, pinned X_ITE 16.5.0, five identical recordings; scene text, log and 24 pictures at four movie times, 0 pixels more than 8 off)'),
  }, "2026-11-11 (glTF video pictures): a video in a buffer view of a GLB is compared (gv-glb), see sub:parse.gltf2."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_gltfvid.py check|apply")
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
