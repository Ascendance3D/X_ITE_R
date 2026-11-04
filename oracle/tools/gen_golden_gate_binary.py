#!/usr/bin/env python3
"""Generate oracle/synthetic-rea-golden-gate-binary/: tiny authored binary inputs (binary STL, binary PLY, GLB) and the worlds that load
them through Browser.createX3DFromURL (REA ledger entry sub:parse.golden-gate, pinned GoldenGate.js at X_ITE 16.5.0).

Every file is written here from first principles (a few triangles); nothing is copied from a third-party asset. The worlds
`gg-*.wrl` run one Script that loads the URL lists of its group one after the other with `createX3DFromURL (new MFString (...))`
and records, per list, the scene head (encoding, specification version, profile, node counts, world URL) and `toVRMLString`,
or the rejection text, in the MFString field `out`. The loader's own diagnostic (`Couldn't load URL ...`, with the parser's error)
is a console.error / console.warn line, which the pinned page and x3d-drive both collect in the sorted `log` of the result.

  python3 -I oracle/tools/gen_golden_gate_binary.py
"""
import gzip, json, os, struct

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-golden-gate-binary")


def put(rel, data):
    path = os.path.join(out_dir, rel)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "wb") as f:
        f.write(data)


# ---------------------------------------------------------------------------------------------------------------- binary STL

def stl(header, facets, count=None):
    """80 header bytes, uint32 facet count, then per facet 12 float32 (normal, three vertices) and a uint16 attribute."""
    head = header.ljust(80, b"\0")[:80] if isinstance(header, bytes) else header
    body = b"".join(struct.pack("<12fH", *n, *v1, *v2, *v3, 0) for n, v1, v2, v3 in facets)
    return head + struct.pack("<I", len(facets) if count is None else count) + body


TRI = [
    ((0, 0, 1), (0, 0, 0), (1, 0, 0), (0, 1, 0)),
    ((0, 0, -1), (0.1, 0.2, 0.3), (0.5, -0.25, 0.75), (-1.5, 2, 0.001)),
]
TRI0 = [(((0, 0, 0)),) + t[1:] for t in TRI]  # zero normals

put("stl/tri.stl", stl(b"synthetic binary STL, two facets", TRI))
put("stl/tri space.stl", stl(b"name with a space", TRI))
put("stl/tri-nohdr.stl", stl(b"", TRI0))
put("stl/tri-solid-header.stl", stl(b"solid synthetic (a binary file with a solid header)", TRI))
put("stl/tri-endsolid-header.stl", stl(b"solid synthetic endsolid", TRI))
put("stl/empty.stl", stl(b"zero facets", []))
put("stl/bad-count.stl", stl(b"count is one too many", TRI, count=3))
put("stl/bad-count-few.stl", stl(b"count is one too few", TRI, count=1))
put("stl/short.stl", stl(b"eighty-three bytes", [])[:83])
# Not a case: a NaN coordinate makes the pinned page log "RangeError: Maximum call stack size exceeded" (measured, see the PR); a finite extreme is.
put("stl/extreme.stl", stl(b"extreme finite values", [((0, 0, 1), (3.0e38, 0, 0), (1.0e-45, 1, 0), (0, 1, -0.0))]))
put("stl/header-utf8.stl", stl("synthetic é€ binary header".encode("utf-8"), TRI))
put("stl/header-nul.stl", stl(b"before\0after the NUL", TRI))

# ---------------------------------------------------------------------------------------------------------------- binary PLY

def ply(fmt, header_lines, elements, eol=b"\n"):
    """elements: list of byte strings, one per element, already packed."""
    head = eol.join([b"ply", b"format " + fmt + b" 1.0"] + [h for h in header_lines] + [b"end_header"]) + eol
    return head + b"".join(elements)


def pk(endian, spec, *vals):
    return struct.pack(endian + spec, *vals)


VERTS3 = [(0, 0, 0), (1, 0, 0), (0, 1, 0)]
VERTS4 = [(0, 0, 0), (1, 0, 0), (1, 1, 0), (0, 1, 0)]


def tri_ply(endian_char, fmt):
    e = endian_char
    return ply(fmt, [b"comment synthetic binary PLY", b"element vertex 3", b"property float x", b"property float y", b"property float z",
                     b"element face 1", b"property list uchar int vertex_indices"],
               [b"".join(pk(e, "fff", *v) for v in VERTS3), pk(e, "B3i", 3, 0, 1, 2)])


put("ply/tri-le.ply", tri_ply("<", b"binary_little_endian"))
put("ply/tri-be.ply", tri_ply(">", b"binary_big_endian"))
put("ply/tri-le-crlf.ply", ply(b"binary_little_endian", [b"element vertex 3", b"property float x", b"property float y", b"property float z",
                                                         b"element face 1", b"property list uchar int vertex_indices"],
                               [b"".join(pk("<", "fff", *v) for v in VERTS3), pk("<", "B3i", 3, 0, 1, 2)], eol=b"\r\n"))
put("ply/quad-le.ply", ply(b"binary_little_endian", [b"element vertex 4", b"property float x", b"property float y", b"property float z",
                                                     b"element face 1", b"property list uchar uint vertex_indices"],
                           [b"".join(pk("<", "fff", *v) for v in VERTS4), pk("<", "B4I", 4, 0, 1, 2, 3)]))
put("ply/two-faces-le.ply", ply(b"binary_little_endian", [b"element vertex 4", b"property float x", b"property float y", b"property float z",
                                                          b"element face 2", b"property list uchar int vertex_indices"],
                                [b"".join(pk("<", "fff", *v) for v in VERTS4), pk("<", "B3i", 3, 0, 1, 2) + pk("<", "B3i", 3, 0, 2, 3)]))
put("ply/color-normal-le.ply", ply(b"binary_little_endian", [b"element vertex 3", b"property float x", b"property float y", b"property float z",
                                                             b"property float nx", b"property float ny", b"property float nz",
                                                             b"property uchar red", b"property uchar green", b"property uchar blue",
                                                             b"element face 1", b"property list uchar int vertex_indices"],
                                   [b"".join(pk("<", "ffffffBBB", *v, 0, 0, 1, c[0], c[1], c[2])
                                             for v, c in zip(VERTS3, [(255, 0, 0), (0, 255, 0), (0, 0, 255)])),
                                    pk("<", "B3i", 3, 0, 1, 2)]))
put("ply/color-alpha-le.ply", ply(b"binary_little_endian", [b"element vertex 3", b"property double x", b"property double y", b"property double z",
                                                            b"property uchar red", b"property uchar green", b"property uchar blue", b"property uchar alpha",
                                                            b"element face 1", b"property list uchar int vertex_indices"],
                                  [b"".join(pk("<", "dddBBBB", *v, c[0], c[1], c[2], c[3])
                                            for v, c in zip(VERTS3, [(255, 0, 0, 255), (0, 255, 0, 128), (0, 0, 255, 0)])),
                                   pk("<", "B3i", 3, 0, 1, 2)]))
put("ply/points-le.ply", ply(b"binary_little_endian", [b"element vertex 3", b"property float x", b"property float y", b"property float z"],
                             [b"".join(pk("<", "fff", *v) for v in VERTS3)]))
put("ply/int-types-le.ply", ply(b"binary_little_endian", [b"element vertex 3", b"property short x", b"property ushort y", b"property int8 z",
                                                          b"element face 1", b"property list uint8 uint16 vertex_indices"],
                                [b"".join(pk("<", "hHb", v[0] * 100, v[1] * 200, v[2] - 5) for v in [(1, 0, 0), (0, 1, 0), (1, 1, 1)]),
                                 pk("<", "B3H", 3, 0, 1, 2)]))
put("ply/texcoord-le.ply", ply(b"binary_little_endian", [b"element vertex 3", b"property float x", b"property float y", b"property float z",
                                                         b"property float s", b"property float t",
                                                         b"element face 1", b"property list uchar int vertex_indices"],
                               [b"".join(pk("<", "fffff", *v, v[0], v[1]) for v in VERTS3), pk("<", "B3i", 3, 0, 1, 2)]))
put("ply/blender-le.ply", ply(b"binary_little_endian", [b"comment Created by Blender synthetic", b"element vertex 3", b"property float x",
                                                        b"property float y", b"property float z",
                                                        b"element face 1", b"property list uchar int vertex_indices"],
                              [b"".join(pk("<", "fff", *v) for v in [(0, 0, 0), (1, 0, 0), (0, 0, 1)]), pk("<", "B3i", 3, 0, 1, 2)]))
put("ply/unknown-element-le.ply", ply(b"binary_little_endian", [b"element vertex 3", b"property float x", b"property float y", b"property float z",
                                                                b"element edge 2", b"property int vertex1", b"property int vertex2",
                                                                b"property list uchar short extra",
                                                                b"element face 1", b"property list uchar int vertex_indices"],
                                      [b"".join(pk("<", "fff", *v) for v in VERTS3),
                                       pk("<", "iiB2h", 0, 1, 2, 7, 8) + pk("<", "iiB", 1, 2, 0),
                                       pk("<", "B3i", 3, 0, 1, 2)]))
put("ply/truncated-le.ply", tri_ply("<", b"binary_little_endian")[:-9])
put("ply/no-elements-le.ply", ply(b"binary_little_endian", [], []))
put("ply/zero-vertices-le.ply", ply(b"binary_little_endian", [b"element vertex 0", b"property float x", b"property float y", b"property float z"], []))
put("ply/bad-format.ply", ply(b"binary_middle_endian", [b"element vertex 1", b"property float x"], [pk("<", "f", 1)]))
put("ply/bad-version.ply", b"ply\nformat binary_little_endian 2.0\nelement vertex 1\nproperty float x\nend_header\n" + pk("<", "f", 1))
put("ply/no-end-header.ply", b"ply\nformat binary_little_endian 1.0\nelement vertex 1\nproperty float x\n" + pk("<", "f", 1))

# ---------------------------------------------------------------------------------------------------------------- GLB

def glb(json_obj, bin_chunk=None, version=2, length=None, extra=b"", json_text=None):
    js = (json_text if json_text is not None else json.dumps(json_obj, separators=(",", ":"))).encode("utf-8")
    js += b" " * (-len(js) % 4)
    chunks = struct.pack("<II", len(js), 0x4E4F534A) + js
    if bin_chunk is not None:
        b = bin_chunk + b"\0" * (-len(bin_chunk) % 4)
        chunks += struct.pack("<II", len(b), 0x004E4942) + b
    chunks += extra
    total = 12 + len(chunks)
    return struct.pack("<III", 0x46546C67, version, total if length is None else length) + chunks


POS = struct.pack("<9f", 0, 0, 0, 1, 0, 0, 0, 1, 0)
IDX = struct.pack("<3H", 0, 1, 2)
BIN = POS + IDX  # 36 + 6 bytes
G_MIN = {
    "asset": {"version": "2.0"},
    "scene": 0,
    "scenes": [{"nodes": [0]}],
    "nodes": [{"mesh": 0}],
    "meshes": [{"primitives": [{"attributes": {"POSITION": 0}, "indices": 1}]}],
    "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]},
                  {"bufferView": 1, "componentType": 5123, "count": 3, "type": "SCALAR"}],
    "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 36}, {"buffer": 0, "byteOffset": 36, "byteLength": 6}],
    "buffers": [{"byteLength": 42}],
}
put("glb/tri.glb", glb(G_MIN, BIN))

NORMALS = struct.pack("<9f", 0, 0, 1, 0, 0, 1, 0, 0, 1)
BIN2 = POS + NORMALS + IDX
G_MAT = {
    "asset": {"version": "2.0", "generator": "synthetic generator"},
    "scene": 0,
    "scenes": [{"name": "the scene", "nodes": [0]}],
    "nodes": [{"name": "Tri Node", "mesh": 0, "translation": [1, 2, 3], "scale": [2, 2, 2]}],
    "meshes": [{"name": "Tri Mesh", "primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1}, "indices": 2, "material": 0}]}],
    "materials": [{"name": "Red", "pbrMetallicRoughness": {"baseColorFactor": [1, 0, 0, 1], "metallicFactor": 0.25, "roughnessFactor": 0.75}}],
    "accessors": [{"bufferView": 0, "componentType": 5126, "count": 3, "type": "VEC3", "min": [0, 0, 0], "max": [1, 1, 0]},
                  {"bufferView": 1, "componentType": 5126, "count": 3, "type": "VEC3"},
                  {"bufferView": 2, "componentType": 5123, "count": 3, "type": "SCALAR"}],
    "bufferViews": [{"buffer": 0, "byteOffset": 0, "byteLength": 36}, {"buffer": 0, "byteOffset": 36, "byteLength": 36},
                    {"buffer": 0, "byteOffset": 72, "byteLength": 6}],
    "buffers": [{"byteLength": 78}],
}
put("glb/tri-material.glb", glb(G_MAT, BIN2))
put("glb/tri-unknown-chunk.glb", glb(G_MIN, BIN, extra=struct.pack("<II", 4, 0x58585858) + b"abcd"))
put("glb/tri-extra-json.glb", glb(G_MIN, BIN, extra=struct.pack("<II", 4, 0x4E4F534A) + b"{}  "))
put("glb/no-bin.glb", glb({"asset": {"version": "2.0"}, "scene": 0, "scenes": [{"nodes": [0]}], "nodes": [{"name": "empty node"}]}))
put("glb/bad-version.glb", glb(G_MIN, BIN, version=1))
put("glb/bad-version3.glb", glb(G_MIN, BIN, version=3))
put("glb/length-long.glb", glb(G_MIN, BIN, length=len(glb(G_MIN, BIN)) + 4))
put("glb/length-short.glb", glb(G_MIN, BIN, length=len(glb(G_MIN, BIN)) - 4))
put("glb/truncated.glb", glb(G_MIN, BIN)[:-8])
put("glb/header-only.glb", struct.pack("<III", 0x46546C67, 2, 12))
put("glb/short.glb", struct.pack("<II", 0x46546C67, 2))
put("glb/bad-json.glb", glb(None, BIN, json_text="{\"asset\": "))
put("glb/no-asset.glb", glb({"scenes": [{"nodes": []}], "scene": 0}))
put("glb/bin-short.glb", glb(dict(G_MIN, buffers=[{"byteLength": 42}]), BIN[:20]))

# ---------------------------------------------------------------------------------------------------------------- dispatch

put("dispatch/stl-as-wrl.wrl", stl(b"binary STL with a .wrl name", TRI))
put("dispatch/stl-as-txt.txt", stl(b"binary STL with a .txt name", TRI))
put("dispatch/stl-as-json.json", stl(b"binary STL with a .json name", TRI))
put("dispatch/stl-no-extension", stl(b"binary STL without an extension", TRI))
put("dispatch/ply-as-wrl.wrl", tri_ply("<", b"binary_little_endian"))
put("dispatch/ply-as-stl.stl", tri_ply("<", b"binary_little_endian"))
put("dispatch/glb-as-wrl.wrl", glb(G_MIN, BIN))
put("dispatch/glb-as-gltf.gltf", glb(G_MIN, BIN))
put("dispatch/glb-as-stl.stl", glb(G_MIN, BIN))
put("dispatch/stl-as-glb.glb", stl(b"binary STL with a .glb name", TRI))
put("dispatch/stl-gz.stl", gzip.compress(stl(b"gzipped binary STL", TRI), mtime=0))
put("dispatch/glb-gz.glb", gzip.compress(glb(G_MIN, BIN), mtime=0))
put("dispatch/ply-gz.ply", gzip.compress(tri_ply("<", b"binary_little_endian"), mtime=0))
put("dispatch/unknown-high.bin", bytes([0xFF, 0xFE, 0x80, 0x81, 0x00, 0x01, 0x02, 0xC3, 0x28, 0xA0, 0xA1]) * 12)
put("dispatch/unknown-nul.bin", bytes(100))
put("dispatch/unknown-ff.bin", bytes([0xFF]) * 120)
put("dispatch/unknown-ascii.bin", b"not a model, just a few words\n")
put("dispatch/empty.bin", b"")
put("dispatch/one-byte.bin", b"\x00")
put("dispatch/stl-valid-size-but-text.bin", b"A" * 80 + struct.pack("<I", 0))  # 84 bytes, zero facets, header is plain text
put("dispatch/magic-ply-text-header.bin", b"ply\nformat binary_little_endian 1.0\nend_header\n")
put("dispatch/glb-magic-only.bin", struct.pack("<I", 0x46546C67) + bytes(8))

# ---------------------------------------------------------------------------------------------------------------- worlds

PREAMBLE = """#X3D V3.3 utf8
PROFILE Immersive
COMPONENT Scripting:1
# Synthetic REA oracle world (generated by oracle/tools/gen_golden_gate_binary.py): %(title)s.
# Each URL list goes through Browser.createX3DFromURL, one after the other; `out` lists the scene head and printed scene or the rejection of each.
DEF OUT Script {
  inputOutput MFString out [ ]
  url "ecmascript:
    var CASES = %(cases)s;
    var res = [];
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
    function initialize () { run (0); }
  "
}
"""

groups = {
    "gg-stl": [
        ["stl/tri.stl"],
        ["stl/tri%20space.stl"],
        ["stl/tri-nohdr.stl"],
        ["stl/tri-solid-header.stl"],
        ["stl/tri-endsolid-header.stl"],
        ["stl/empty.stl"],
        ["stl/bad-count.stl"],
        ["stl/bad-count-few.stl"],
        ["stl/short.stl"],
        ["stl/extreme.stl"],
        ["stl/header-utf8.stl"],
        ["stl/header-nul.stl"],
        ["stl/missing.stl"],
    ],
    "gg-ply": [
        ["ply/tri-le.ply"],
        ["ply/tri-be.ply"],
        ["ply/tri-le-crlf.ply"],
        ["ply/quad-le.ply"],
        ["ply/two-faces-le.ply"],
        ["ply/color-normal-le.ply"],
        ["ply/color-alpha-le.ply"],
        ["ply/points-le.ply"],
        ["ply/int-types-le.ply"],
        ["ply/texcoord-le.ply"],
        ["ply/blender-le.ply"],
        ["ply/unknown-element-le.ply"],
        ["ply/truncated-le.ply"],
        ["ply/no-elements-le.ply"],
        ["ply/zero-vertices-le.ply"],
        ["ply/bad-format.ply"],
        ["ply/bad-version.ply"],
        ["ply/no-end-header.ply"],
    ],
    "gg-glb": [
        ["glb/tri.glb"],
        ["glb/tri-material.glb"],
        ["glb/tri-unknown-chunk.glb"],
        ["glb/tri-extra-json.glb"],
        ["glb/no-bin.glb"],
        ["glb/bad-version.glb"],
        ["glb/bad-version3.glb"],
        ["glb/length-long.glb"],
        ["glb/length-short.glb"],
        ["glb/truncated.glb"],
        ["glb/header-only.glb"],
        ["glb/short.glb"],
        ["glb/bad-json.glb"],
        ["glb/no-asset.glb"],
        ["glb/bin-short.glb"],
    ],
    "gg-dispatch": [
        ["dispatch/stl-as-wrl.wrl"],
        ["dispatch/stl-as-txt.txt"],
        ["dispatch/stl-as-json.json"],
        ["dispatch/stl-no-extension"],
        ["dispatch/ply-as-wrl.wrl"],
        ["dispatch/ply-as-stl.stl"],
        ["dispatch/glb-as-wrl.wrl"],
        ["dispatch/glb-as-gltf.gltf"],
        ["dispatch/glb-as-stl.stl"],
        ["dispatch/stl-as-glb.glb"],
        ["dispatch/stl-gz.stl"],
        ["dispatch/glb-gz.glb"],
        ["dispatch/ply-gz.ply"],
        ["dispatch/unknown-high.bin"],
        ["dispatch/unknown-nul.bin"],
        ["dispatch/unknown-ff.bin"],
        ["dispatch/unknown-ascii.bin"],
        ["dispatch/empty.bin"],
        ["dispatch/one-byte.bin"],
        ["dispatch/stl-valid-size-but-text.bin"],
        ["dispatch/magic-ply-text-header.bin"],
        ["dispatch/glb-magic-only.bin"],
        ["dispatch/unknown-high.bin", "stl/tri.stl"],
        ["dispatch/unknown-high.bin", "dispatch/missing.glb"],
        ["dispatch/unknown-high.bin", "dispatch/unknown-ff.bin", "glb/tri.glb"],
        ["glb/bad-version.glb", "ply/tri-le.ply"],
        ["stl/bad-count.stl", "stl/tri.stl", "ply/tri-le.ply"],
    ],
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

# The harness script (the format x3d-drive and oracle/x_ite-anim.mjs read): settle, a few dumped frames.
script = {"size": [512, 512], "t0": 946684800, "dt": 1 / 60, "pinFrameRate": True, "settle": True, "frames": 60,
          "capture": [], "dump": [1, 2, 3, 5, 10, 20, 40, 60], "events": []}
with open(os.path.join(out_dir, "gg.json"), "w") as f:
    json.dump(script, f)
