#!/usr/bin/env python3
"""Worlds and event scripts for EXT_texture_video frames at fixed movie times (ledger sub:parse.gltf2).

  python3 -I oracle/tools/gen_gltf_video.py

Writes oracle/synthetic-gltf-video/: three glTF worlds that show the same four video pictures on four quads, and one event
script (the same for all three, gv.json). The videos are the authored WebM streams of oracle/synthetic-movie-codecs/media (ffmpeg
test patterns, no third-party media): vp8-ramp, vp8-testsrc, vp8-parts and vp9-vorbis, 10 pictures per second.

- gv-uri.gltf: the videos are external files (`uri` relative to the glTF file).
- gv-data.gltf: the videos are base64 data URLs in `uri`.
- gv-glb.glb: the videos are bufferViews of the BIN chunk (the pinned parser makes a data URL of the blob, GLTF2Parser.imageObject).

Each quad has an unlit material (KHR_materials_unlit) with a baseColorTexture whose texture has only the EXT_texture_video source.
The pinned parser makes a MovieTexture of it with `loop TRUE` and `colorSpaceConversion FALSE`; the file cannot set `speed`, so the
node plays at speed 1 from the load. The event script (as in gen_movie_video.py) seeks each element (`movieSeek`: the harness pauses the
element and sets currentTime, x3d-drive calls Runtime::movie_seek) to the middle of a picture and captures three frames later, so the
captured picture does not depend on the wall clock.
"""
import base64, json, os, struct

HERE = os.path.dirname(os.path.abspath(__file__))
OUT = os.path.normpath(os.path.join(HERE, "..", "synthetic-gltf-video"))
MEDIA = os.path.normpath(os.path.join(HERE, "..", "synthetic-movie-codecs", "media"))
os.makedirs(OUT, exist_ok=True)

CLIPS = ["vp8-ramp", "vp8-testsrc", "vp8-parts", "vp9-vorbis"]
TIMES = [0.25, 0.55, 0.05, 0.35]
T0, DT, FIRST = 946684800, 1 / 32, 8
SEEK_AT = [FIRST + 8 * i for i in range(len(TIMES))]
CAPTURE = [f + 3 for f in SEEK_AT]
FRAMES = CAPTURE[-1] + 2
EXT = "EXT_texture_video"


def read(clip):
    return open(os.path.join(MEDIA, clip + ".webm"), "rb").read()


def pad4(b, fill=b"\0"):
    return b + fill * (-len(b) % 4)


# one quad: 4 vertices, uv, 6 indices
POS = struct.pack("<12f", -0.9, -0.9, 0, 0.9, -0.9, 0, 0.9, 0.9, 0, -0.9, 0.9, 0)
UV = struct.pack("<8f", 0, 1, 1, 1, 1, 0, 0, 0)
IDX = struct.pack("<6H", 0, 1, 2, 0, 2, 3)
GEOM = POS + UV + IDX  # 48 + 32 + 12 = 92 bytes


def doc(videos, buffers, views):
    """`videos`: the EXT_texture_video list; `buffers`/`views` the buffer list and the bufferViews that follow the geometry."""
    n = len(CLIPS)
    return {
        "asset": {"version": "2.0"},
        "extensionsUsed": [EXT, "KHR_materials_unlit"],
        "extensionsRequired": [],
        "scene": 0,
        "scenes": [{"nodes": list(range(n))}],
        "nodes": [{"name": f"Q{i}", "mesh": i, "translation": [-1.1 + 2.2 * (i % 2), 1.1 - 2.2 * (i // 2), 0]} for i in range(n)],
        "meshes": [{"primitives": [{"attributes": {"POSITION": 0, "TEXCOORD_0": 1}, "indices": 2, "material": i}]} for i in range(n)],
        "materials": [{"name": f"M{i}", "pbrMetallicRoughness": {"baseColorTexture": {"index": i}},
                       "extensions": {"KHR_materials_unlit": {}}} for i in range(n)],
        # The pinned parser reads `this .images [texture .source]` for every texture, so a file without an images array does not load.
        "images": [],
        "textures": [{"name": f"V{i}", "extensions": {EXT: {"source": i}}} for i in range(n)],
        "accessors": [
            {"bufferView": 0, "componentType": 5126, "count": 4, "type": "VEC3", "min": [-0.9, -0.9, 0], "max": [0.9, 0.9, 0]},
            {"bufferView": 1, "componentType": 5126, "count": 4, "type": "VEC2"},
            {"bufferView": 2, "componentType": 5123, "count": 6, "type": "SCALAR"},
        ],
        "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 48}, {"buffer": 0, "byteOffset": 48, "byteLength": 32},
                        {"buffer": 0, "byteOffset": 80, "byteLength": 12}] + views,
        "buffers": buffers,
        "extensions": {EXT: {"videos": videos}},
    }


def j(o):
    return json.dumps(o, separators=(",", ":"))


def data_url(b):
    return "data:video/webm;base64," + base64.b64encode(b).decode("ascii")


def main():
    geom_url = "data:application/octet-stream;base64," + base64.b64encode(GEOM).decode("ascii")

    uri = doc([{"uri": f"../synthetic-movie-codecs/media/{c}.webm", "mimeType": "video/webm"} for c in CLIPS],
              [{"uri": geom_url, "byteLength": len(GEOM)}], [])
    open(os.path.join(OUT, "gv-uri.gltf"), "w").write(j(uri))

    data = doc([{"uri": data_url(read(c)), "mimeType": "video/webm"} for c in CLIPS], [{"uri": geom_url, "byteLength": len(GEOM)}], [])
    open(os.path.join(OUT, "gv-data.gltf"), "w").write(j(data))

    blob, views, videos = GEOM, [], []
    for c in CLIPS:
        b = read(c)
        views.append({"buffer": 0, "byteOffset": len(blob), "byteLength": len(b)})
        videos.append({"bufferView": 3 + len(videos), "mimeType": "video/webm"})
        blob += b + b"\0" * (-len(b) % 4)
    glb = doc(videos, [{"byteLength": len(blob)}], views)
    js = pad4(j(glb).encode(), b" ")
    chunks = struct.pack("<II", len(js), 0x4E4F534A) + js + struct.pack("<II", len(blob), 0x004E4942) + blob
    open(os.path.join(OUT, "gv-glb.glb"), "wb").write(struct.pack("<III", 0x46546C67, 2, 12 + len(chunks)) + chunks)

    events = [{"frame": f, "type": "movieSeek", "node": f"V{i}", "time": t} for f, t in zip(SEEK_AT, TIMES) for i in range(len(CLIPS))]
    json.dump({"settle": True, "size": [512, 512], "t0": T0, "dt": DT, "frames": FRAMES, "capture": CAPTURE, "dump": [FRAMES],
               "events": events}, open(os.path.join(OUT, "gv.json"), "w"), indent=1)


main()
