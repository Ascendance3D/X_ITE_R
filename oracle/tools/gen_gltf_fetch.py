#!/usr/bin/env python3
"""Generate oracle/synthetic-rea-gltf-fetch/: tiny authored glTF / GLB files whose buffers, images and EXT_texture_video sources
fail to load, and the worlds that load them through Browser.createX3DFromURL (REA ledger entries sub:parse.gltf2 and sub:parse.glb2,
pinned GLTF2Parser.js / GLB2Parser.js at X_ITE 16.5.0).

Every file is written here from first principles (one triangle, a 2 x 2 PNG made with zlib); nothing is copied from a third-party
asset. A world `gf-*.wrl` runs one Script that loads the URL lists of its group one after the other with
`createX3DFromURL (new MFString (...))` and records, per list, the scene head and `toVRMLString`, or the rejection text, in the
MFString field `out` (the same world shape as oracle/tools/gen_golden_gate_binary.py). A list that never settles stays `k: ...`
in `out`; the baseline keeps that too. The loaders' diagnostics are console messages.

  python3 -I oracle/tools/gen_gltf_fetch.py
"""
import base64, json, os, struct, zlib

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-gltf-fetch")


def put(rel, data):
    path = os.path.join(out_dir, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data if isinstance(data, bytes) else data.encode("utf-8"))


def j(obj):
    return json.dumps(obj, separators=(",", ":"))


def b64(data, mime="application/octet-stream"):
    return f"data:{mime};base64," + base64.b64encode(data).decode("ascii")


def png(w, h, rgba):
    def chunk(tag, body):
        return struct.pack(">I", len(body)) + tag + body + struct.pack(">I", zlib.crc32(tag + body) & 0xFFFFFFFF)
    raw = b"".join(b"\0" + bytes(rgba) * w for _ in range(h))
    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def glb(json_obj, bin_chunk=None):
    js = j(json_obj).encode("utf-8")
    js += b" " * (-len(js) % 4)
    chunks = struct.pack("<II", len(js), 0x4E4F534A) + js
    if bin_chunk is not None:
        b = bin_chunk + b"\0" * (-len(bin_chunk) % 4)
        chunks += struct.pack("<II", len(b), 0x004E4942) + b
    return struct.pack("<III", 0x46546C67, 2, 12 + len(chunks)) + chunks


POS = struct.pack("<9f", 0, 0, 0, 1, 0, 0, 0, 1, 0)
UV = struct.pack("<6f", 0, 0, 1, 0, 0, 1)
IDX = struct.pack("<3H", 0, 1, 2)
BIN = POS + IDX + b"\0\0"  # 36 + 6 + 2 padding
BIN_UV = POS + UV + IDX + b"\0\0"  # 36 + 24 + 6 + 2


def gltf(buffers, textured=None, extra=None, images=None, textures=None, extensions_used=None, with_uv=False):
    """One triangle; `buffers` replaces the buffer list (the first buffer holds the geometry)."""
    prim = {"attributes": {"POSITION": 0}, "indices": 1 + (1 if with_uv else 0)}
    accessors = [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]}]
    views = [{"buffer": 0, "byteOffset": 0, "byteLength": 36}]
    if with_uv:
        prim["attributes"]["TEXCOORD_0"] = 1
        accessors.append({"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC2"})
        views.append({"buffer": 0, "byteOffset": 36, "byteLength": 24})
        accessors.append({"bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR"})
        views.append({"buffer": 0, "byteOffset": 60, "byteLength": 6})
    else:
        accessors.append({"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"})
        views.append({"buffer": 0, "byteOffset": 36, "byteLength": 6})
    doc = {
        "asset": {"version": "2.0"},
        "scene": 0,
        "scenes": [{"nodes": [0]}],
        "nodes": [{"mesh": 0}],
        "meshes": [{"primitives": [prim]}],
        "accessors": accessors,
        "bufferViews": views,
        "buffers": buffers,
    }
    if textured is not None:
        prim["material"] = 0
        doc["materials"] = [{"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}}]
    if images is not None:
        doc["images"] = images
    if textures is not None:
        doc["textures"] = textures
    if extensions_used:
        doc["extensionsUsed"] = extensions_used
    if extra:
        doc.update(extra)
    return doc


# ------------------------------------------------------------------------------------------------------------------ buffers

put("buf/tri.bin", BIN)
put("buf/tri space.bin", BIN)
G = lambda buffers, **kw: j(gltf(buffers, **kw))
put("buf/ok-external.gltf", G([{"uri": "tri.bin", "byteLength": 42}]))
put("buf/ok-data.gltf", G([{"uri": b64(BIN), "byteLength": 42}]))
put("buf/ok-encoded.gltf", G([{"uri": "tri%20space.bin", "byteLength": 42}]))
put("buf/missing.gltf", G([{"uri": "missing.bin", "byteLength": 42}]))
put("buf/missing-subdir.gltf", G([{"uri": "nodir/missing.bin", "byteLength": 42}]))
put("buf/missing-dotdot.gltf", G([{"uri": "../nothing-here/missing.bin", "byteLength": 42}]))
put("buf/bad-base64.gltf", G([{"uri": "data:application/octet-stream;base64,@@@@", "byteLength": 42}]))
put("buf/data-short.gltf", G([{"uri": b64(BIN[:20]), "byteLength": 42}]))
put("buf/data-empty.gltf", G([{"uri": "data:application/octet-stream;base64,", "byteLength": 42}]))
put("buf/uri-empty.gltf", G([{"uri": "", "byteLength": 42}]))
put("buf/uri-null.gltf", G([{"uri": None, "byteLength": 42}]))
put("buf/uri-number.gltf", G([{"uri": 7, "byteLength": 42}]))
put("buf/uri-invalid.gltf", G([{"uri": "http://[bad", "byteLength": 42}]))
put("buf/two-second-missing.gltf", G([{"uri": "tri.bin", "byteLength": 42}, {"uri": "missing-b.bin", "byteLength": 4}]))
put("buf/two-first-missing.gltf", G([{"uri": "missing-a.bin", "byteLength": 42}, {"uri": "tri.bin", "byteLength": 42}]))
put("buf/two-both-missing.gltf", G([{"uri": "missing-a.bin", "byteLength": 42}, {"uri": "missing-b.bin", "byteLength": 4}]))
put("buf/two-same-missing.gltf", G([{"uri": "missing-a.bin", "byteLength": 42}, {"uri": "missing-a.bin", "byteLength": 42}]))
put("buf/view-of-missing.gltf", G([{"uri": "missing.bin", "byteLength": 42}]))  # accessors read the failed buffer
put("buf/not-a-bin-dir.gltf", G([{"uri": "dir-not-file/", "byteLength": 42}]))

# GLB: buffer 0 is the BIN chunk (no uri); a further buffer names a file.
glb_doc = lambda buffers: gltf(buffers)
put("buf/glb-ok.glb", glb(glb_doc([{"byteLength": 42}]), BIN))
put("buf/glb-second-ok.glb", glb(glb_doc([{"byteLength": 42}, {"uri": "tri.bin", "byteLength": 42}]), BIN))
put("buf/glb-second-missing.glb", glb(glb_doc([{"byteLength": 42}, {"uri": "missing-b.bin", "byteLength": 4}]), BIN))
put("buf/glb-first-uri-missing.glb", glb(glb_doc([{"uri": "missing.bin", "byteLength": 42}]), BIN))
put("buf/glb-first-uri-ok.glb", glb(glb_doc([{"uri": "tri.bin", "byteLength": 42}]), BIN))
put("buf/glb-nobin-uri-missing.glb", glb(glb_doc([{"uri": "missing.bin", "byteLength": 42}])))
put("buf/glb-nobin-uri-ok.glb", glb(glb_doc([{"uri": "tri.bin", "byteLength": 42}])))
put("buf/glb-nobin-no-uri.glb", glb(glb_doc([{"byteLength": 42}])))
put("buf/glb-data-bad.glb", glb(glb_doc([{"uri": "data:application/octet-stream;base64,@@@@", "byteLength": 42}]), BIN))

# ------------------------------------------------------------------------------------------------------------------ images

PNG = png(2, 2, (255, 0, 0, 255))
put("img/red.png", PNG)
put("img/not-a-png.png", b"this is not an image\n")
TEX = lambda src=0, **kw: [dict({"source": src}, **kw)]
BUF_DATA = lambda data: [{"uri": b64(data), "byteLength": len(data)}]
T = dict(textured=True, with_uv=True)


def GI(buffers, images, textures, **kw):
    return j(gltf(buffers, images=images, textures=textures, **dict(T, **kw)))


put("img/ok-external.gltf", GI(BUF_DATA(BIN_UV), [{"uri": "red.png"}], TEX()))
put("img/ok-data.gltf", GI(BUF_DATA(BIN_UV), [{"uri": b64(PNG, "image/png")}], TEX()))
put("img/missing.gltf", GI(BUF_DATA(BIN_UV), [{"uri": "missing.png"}], TEX()))
put("img/missing-subdir.gltf", GI(BUF_DATA(BIN_UV), [{"uri": "nodir/missing.png"}], TEX()))
put("img/not-an-image.gltf", GI(BUF_DATA(BIN_UV), [{"uri": "not-a-png.png"}], TEX()))
put("img/data-garbage.gltf", GI(BUF_DATA(BIN_UV), [{"uri": b64(b"not an image at all", "image/png")}], TEX()))
put("img/data-bad-base64.gltf", GI(BUF_DATA(BIN_UV), [{"uri": "data:image/png;base64,@@@@"}], TEX()))
put("img/data-empty.gltf", GI(BUF_DATA(BIN_UV), [{"uri": "data:image/png;base64,"}], TEX()))
put("img/texture-no-source.gltf", GI(BUF_DATA(BIN_UV), [{"uri": "red.png"}], [{}]))
put("img/source-out-of-range.gltf", GI(BUF_DATA(BIN_UV), [{"uri": "red.png"}], TEX(5)))
put("img/image-no-uri-no-view.gltf", GI(BUF_DATA(BIN_UV), [{"mimeType": "image/png"}], TEX()))
put("img/uri-invalid.gltf", GI(BUF_DATA(BIN_UV), [{"uri": "http://[bad"}], TEX()))
# an image kept in a bufferView of the (data URL) buffer: the parser turns it into a data URL itself
for name, data, mime in (("bufferview-ok", PNG, "image/png"), ("bufferview-garbage", b"garbage bytes, not an image", "image/png"),
                         ("bufferview-nomime", PNG, None)):
    blob = BIN_UV + data
    doc = gltf(BUF_DATA(blob), images=[({"bufferView": 3, "mimeType": mime} if mime else {"bufferView": 3})], textures=TEX(), **T)
    doc["bufferViews"].append({"buffer": 0, "byteOffset": len(BIN_UV), "byteLength": len(data)})
    put(f"img/{name}.gltf", j(doc))
# GLB with the image in the BIN chunk, and a bufferView that points past the buffer
for name, data in (("glb-image-ok", PNG), ("glb-image-garbage", b"garbage bytes, not an image")):
    blob = BIN_UV + data
    doc = gltf([{"byteLength": len(blob)}], images=[{"bufferView": 3, "mimeType": "image/png"}], textures=TEX(), **T)
    doc["bufferViews"].append({"buffer": 0, "byteOffset": len(BIN_UV), "byteLength": len(data)})
    put(f"img/{name}.glb", glb(doc, blob))
# Texture extension sources: a first candidate that fails, then the fallback source
for ext in ("EXT_texture_webp", "EXT_texture_avif", "KHR_texture_basisu"):
    images = [{"uri": "missing-ext.bin"}, {"uri": "red.png"}]
    textures = [{"source": 1, "extensions": {ext: {"source": 0}}}]
    put(f"img/{ext}-missing-then-png.gltf", GI(BUF_DATA(BIN_UV), images, textures, extensions_used=[ext]))
    put(f"img/{ext}-unused.gltf", GI(BUF_DATA(BIN_UV), images, textures))  # not in extensionsUsed: the source is ignored

# EXT_lights_image_based: the first face image has neither uri nor bufferView (its url is the string "undefined"), the others load
doc = gltf(BUF_DATA(BIN_UV), images=[{"mimeType": "image/png"}] + [{"uri": "red.png"}] * 5, extensions_used=["EXT_lights_image_based"], **T)
doc["textures"] = TEX()
doc["extensions"] = {"EXT_lights_image_based": {"lights": [{"specularImages": [[0, 1, 2, 3, 4, 5]]}]}}
doc["scenes"] = [{"nodes": [0], "extensions": {"EXT_lights_image_based": {"light": 0}}}]
put("img/ibl-face-without-uri.gltf", j(doc))

# ------------------------------------------------------------------------------------------------------------------ EXT_texture_video

VIDEO_EXT = "EXT_texture_video"


def GV(videos, source=0, used=True, images=({"uri": "red.png"},), **kw):
    images = list(images) if images is not None else None
    textures = [{"extensions": {VIDEO_EXT: {"source": source}}}]
    doc = gltf(BUF_DATA(BIN_UV), images=images, textures=textures, extensions_used=[VIDEO_EXT] if used else None, **T)
    doc.update(kw)
    doc["extensions"] = {VIDEO_EXT: {"videos": videos}}
    return j(doc)


put("vid/missing.gltf", GV([{"uri": "missing.webm"}]))
put("vid/no-images-array.gltf", GV([{"uri": "missing.webm"}], images=None))  # the pinned parser reads `this .images [texture .source]`
put("vid/ok-data-image-present.gltf", GV([{"uri": b64(PNG, "video/webm")}]))
put("vid/two-videos.gltf", GV([{"uri": "missing.webm"}, {"uri": "missing-b.webm"}], source=1))
put("vid/video-no-uri.gltf", GV([{"mimeType": "video/webm"}]))
put("vid/missing-mp4.gltf", GV([{"uri": "missing.mp4", "mimeType": "video/mp4"}]))
put("vid/data-garbage.gltf", GV([{"uri": b64(b"not a video", "video/webm")}]))
put("vid/data-bad-base64.gltf", GV([{"uri": "data:video/webm;base64,@@@@"}]))
put("vid/not-used.gltf", GV([{"uri": "missing.webm"}], used=False))
put("vid/source-out-of-range.gltf", GV([{"uri": "missing.webm"}], source=3))
put("vid/with-image-fallback.gltf", j(dict(json.loads(GV([{"uri": "missing.webm"}])),
                                           textures=[{"source": 0, "extensions": {VIDEO_EXT: {"source": 0}}}])))
# a video that exists: an authored file of oracle/synthetic-movie-codecs, reached by a relative url (nothing is copied)
put("vid/ok-external.gltf", GV([{"uri": "../../synthetic-movie-codecs/media/vp8-parts.webm", "mimeType": "video/webm"}]))
put("vid/glb-missing.glb", glb(json.loads(GV([{"uri": "missing.webm"}]))))

# ------------------------------------------------------------------------------------------------------------------ worlds

PREAMBLE = """#X3D V3.3 utf8
PROFILE Immersive
COMPONENT Scripting:1
# Synthetic REA oracle world (generated by oracle/tools/gen_gltf_fetch.py): %(title)s.
# Each URL list goes through Browser.createX3DFromURL, one after the other; `out` lists the scene head and printed scene or the rejection of each.
# The loads start from the first event of a TimeSensor, after the world itself finished loading (the harness freezes time at 946684800 for the load; `go` waits for the first frame after 946684800.1): a glTF buffer that fails to load is never
# removed from the loading objects of the world in the pinned X_ITE, so the world's own loadURL would never settle (oracle/README.md).
DEF CLOCK TimeSensor { loop TRUE cycleInterval 1000 }
DEF OUT Script {
  eventIn SFTime go
  inputOutput MFString out [ ]
  url "ecmascript:
    var CASES = %(cases)s;
    var res = [], started = false;
    function strip (s) { return String (s) .replace (/https?:\\\\/\\\\/[^\\\\/]+\\\\//g, ''); }
    function put (i, v) { res [i] = i + ': ' + v; var o = []; for (var k = 0; k < CASES .length; ++ k) o .push (k in res ? res [k] : k + ': ...'); out = new MFString (... o); }
    function run (i) {
      if (i >= CASES .length) return;
      var p;
      try { p = Browser .createX3DFromURL (new MFString (... CASES [i])); } catch (e) { put (i, 'THROW ' + strip (e .message)); run (i + 1); return; }
      p .then (function (sc) {
        var head = [sc .encoding, sc .specificationVersion, sc .profile && sc .profile .name, sc .rootNodes .length, sc .protos .length, sc .externprotos .length, sc .routes .length, sc .importedNodes .length, sc .exportedNodes .length, strip (sc .worldURL)] .join ('/');
        var text; try { text = sc .toVRMLString (); } catch (e) { text = 'PRINT ERR ' + e .message; }
        put (i, 'RES ' + head + '\\n' + text);
        run (i + 1);
      }, function (e) { put (i, 'REJ ' + strip (e && e .message)); run (i + 1); });
    }
    function go (t) { if (started || t < 946684800.1) return; started = true; run (0); }
  "
}
ROUTE CLOCK.time TO OUT.go
"""



def listing(sub):
    """Every glTF / GLB file of a directory, one URL list each (in name order); the data files are no cases."""
    return [[f"{sub}/{n}"] for n in sorted(os.listdir(os.path.join(out_dir, sub))) if n.endswith((".gltf", ".glb"))]


groups = {
    "gf-buffer": listing("buf") + [
        ["buf/missing.gltf", "buf/ok-external.gltf"],
        ["buf/missing.gltf", "buf/missing-subdir.gltf", "buf/ok-data.gltf"],
        ["buf/glb-second-missing.glb", "buf/glb-ok.glb"],
        ["buf/ok-external.gltf", "buf/missing.gltf"],
        # A glTF whose buffer failed rejects with no error, which FileLoader logs nothing for and does not count as an attempt.
        ["buf/missing.gltf", "buf/nonexistent.wrl"],
        ["buf/nonexistent.wrl", "buf/missing.gltf"],
        ["buf/missing.gltf", "buf/nonexistent.wrl", "buf/ok-external.gltf"],
        ["buf/nonexistent.wrl", "buf/missing.gltf", "buf/nonexistent-2.wrl", "buf/ok-external.gltf"],
        ["buf/missing.gltf", "buf/two-both-missing.gltf"],
        ["buf/uri-invalid.gltf", "buf/missing.gltf", "buf/ok-data.gltf"],
    ],
    "gf-image": listing("img") + [["img/missing.gltf", "img/ok-data.gltf"], ["img/glb-image-garbage.glb", "img/glb-image-ok.glb"]],
    "gf-video": listing("vid") + [["vid/missing.gltf", "img/ok-data.gltf"]],
}

for name, cases in groups.items():
    js = json.dumps(cases, ensure_ascii=True)
    # The case array sits inside a VRML string: \\ and \" are the only escapes the lexer reads.
    vrml = js.replace("\\", "\\\\").replace('"', '\\"')
    text = PREAMBLE % {"title": name, "cases": vrml}
    # `strip`'s regular expression went through the same escaping by hand above (four backslashes become two in the file).
    text = text.replace("\\\\\\\\", "\\\\")
    with open(os.path.join(out_dir, name + ".wrl"), "w") as f:
        f.write(text)
    print(name, len(cases))

# The harness script (the format x3d-drive and oracle/x_ite-anim.mjs read): settle, ordered console, a few dumped frames.
script = {"size": [512, 512], "t0": 946684800, "dt": 1 / 60, "pinFrameRate": True, "settle": True, "frames": 120,
          "capture": [], "dump": [1, 2, 3, 5, 10, 20, 60, 120], "events": []}
with open(os.path.join(out_dir, "gf.json"), "w") as f:
    json.dump(script, f)
