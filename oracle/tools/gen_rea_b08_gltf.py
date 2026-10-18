#!/usr/bin/env python3
"""REA batch 08: authored glTF 2.0 / GLB inputs for the GLTF2Parser / GLB2Parser port.

  python3 -I oracle/tools/gen_rea_b08_gltf.py oracle/synthetic-rea-b08-gltf

Everything here is newly authored synthetic data (geometry, PNG images, JSON); no third-party
models or assets. Output is deterministic. Inputs X_ITE is expected to reject are in `invalid/`.
"""
import base64, json, math, os, struct, sys, zlib

OUT = sys.argv[1]
os.makedirs(OUT, exist_ok=True)

# --- PNG -------------------------------------------------------------------------------------------


def png(w, h, pixel):
    raw = b"".join(b"\0" + bytes(c for x in range(w) for c in pixel(x, y)) for y in range(h))

    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xFFFFFFFF)

    return b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")


def checker(x, y):
    on = ((x // 4) + (y // 4)) % 2 == 0
    return (230, 60, 40, 255) if on else (40, 80, 230, 255)


def gradient(x, y):
    return (x * 8 % 256, y * 8 % 256, 128, 255)


def alpha_png(x, y):
    return (255, 255, 255, 64 + 6 * x)


PNG_A = png(32, 32, checker)
PNG_B = png(16, 16, gradient)
PNG_C = png(8, 8, lambda x, y: (200, 200, 40 + 20 * y, 255))
FACE = [png(8, 8, lambda x, y, c=c: c + (255,)) for c in [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 0), (255, 0, 255), (0, 255, 255)]]


def data_uri(data, mime="application/octet-stream"):
    return f"data:{mime};base64," + base64.b64encode(data).decode()


# --- builder ---------------------------------------------------------------------------------------------

CT = {5120: "b", 5121: "B", 5122: "h", 5123: "H", 5125: "I", 5126: "f", 5124: "i", 5130: "d"}
NC = {"SCALAR": 1, "VEC2": 2, "VEC3": 3, "VEC4": 4, "MAT2": 4, "MAT3": 9, "MAT4": 16}


def f16(v):
    return struct.unpack("<H", struct.pack("<e", v))[0]


class G:
    def __init__(self):
        self.buf = bytearray()
        self.views = []
        self.accessors = []
        self.d = {"asset": {"version": "2.0", "generator": "rea-b08"}}

    def view(self, data, target=None, stride=None):
        while len(self.buf) % 4:
            self.buf.append(0)
        v = {"buffer": 0, "byteOffset": len(self.buf), "byteLength": len(data)}
        if target:
            v["target"] = target
        if stride:
            v["byteStride"] = stride
        self.buf += data
        self.views.append(v)
        return len(self.views) - 1

    def acc(self, values, ct, typ, normalized=False, minmax=False, stride=None, offset=0, target=None):
        n = NC[typ]
        flat = [x for v in values for x in (v if isinstance(v, (list, tuple)) else [v])]
        count = len(flat) // n
        if ct == 5131:
            data = b"".join(struct.pack("<H", f16(x)) for x in flat)
        else:
            data = struct.pack("<" + CT[ct] * len(flat), *flat)
        if stride:
            esize = n * (2 if ct == 5131 else struct.calcsize(CT[ct]))
            rows = [data[i * esize:(i + 1) * esize] for i in range(count)]
            data = b"".join(r + b"\0" * (stride - esize) for r in rows)
        data = b"\0" * offset + data
        bv = self.view(data, target, stride)
        a = {"bufferView": bv, "componentType": ct, "count": count, "type": typ}
        if offset:
            a["byteOffset"] = offset
        if normalized:
            a["normalized"] = True
        if minmax:
            cols = [[flat[i * n + c] for i in range(count)] for c in range(n)]
            a["min"] = [min(c) for c in cols]
            a["max"] = [max(c) for c in cols]
        self.accessors.append(a)
        return len(self.accessors) - 1

    def add(self, key, item):
        self.d.setdefault(key, []).append(item)
        return len(self.d[key]) - 1

    def finish(self):
        d = dict(self.d)
        if self.views:
            d["bufferViews"] = self.views
        if self.accessors:
            d["accessors"] = self.accessors
        return d

    def write(self, name, mode="embed"):
        d = self.finish()
        path = os.path.join(OUT, name)
        os.makedirs(os.path.dirname(path), exist_ok=True)
        buf = bytes(self.buf)
        if mode == "glb":
            write_glb(path + ".glb", d, buf)
            return
        if buf:
            if mode == "external":
                d["buffers"] = [{"byteLength": len(buf), "uri": os.path.basename(name) + ".bin"}]
                open(path + ".bin", "wb").write(buf)
            else:
                d["buffers"] = [{"byteLength": len(buf), "uri": data_uri(buf)}]
        open(path + ".gltf", "w").write(json.dumps(d, indent=1))


def write_glb(path, d, buf, extra_chunks=(), raw_json=None, length_delta=0):
    js = (raw_json if raw_json is not None else json.dumps(d, separators=(",", ":"))).encode()
    while len(js) % 4:
        js += b" "
    out = struct.pack("<II", len(js), 0x4E4F534A) + js
    if buf is not None and (len(buf) or d.get("buffers")):
        b = bytes(buf)
        while len(b) % 4:
            b += b"\0"
        d.setdefault("buffers", [{"byteLength": len(b)}])
        out = struct.pack("<II", len(js), 0x4E4F534A) + js
        out += struct.pack("<II", len(b), 0x004E4942) + b
    for t, data in extra_chunks:
        out += struct.pack("<II", len(data), t) + data
    total = 12 + len(out) + length_delta
    open(path, "wb").write(struct.pack("<III", 0x46546C67, 2, total) + out)


def glb_with(g, name, **kw):
    d = g.finish()
    path = os.path.join(OUT, name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    buf = bytes(g.buf)
    if buf:
        d["buffers"] = [{"byteLength": len(buf)}]
    write_glb(path + ".glb", d, buf if buf else None, **kw)


# --- shared geometry --------------------------------------------------------------------------------------

QUAD_P = [(-1, -1, 0), (1, -1, 0), (1, 1, 0), (-1, 1, 0)]
QUAD_N = [(0, 0, 1)] * 4
QUAD_UV = [(0, 1), (1, 1), (1, 0), (0, 0)]
QUAD_I = [0, 1, 2, 0, 2, 3]


def basic(g, prim_extra=None, attrs=None, indices=True, mesh_extra=None, node_extra=None, material=None):
    """One quad mesh on one node in one scene. Returns the primitive dict."""
    pos = g.acc(QUAD_P, 5126, "VEC3", minmax=True)
    prim = {"attributes": {"POSITION": pos}}
    if attrs:
        prim["attributes"].update(attrs)
    if indices:
        prim["indices"] = g.acc(QUAD_I, 5123, "SCALAR")
    if material is not None:
        prim["material"] = material
    if prim_extra:
        prim.update(prim_extra)
    mesh = {"primitives": [prim]}
    if mesh_extra:
        mesh.update(mesh_extra)
    m = g.add("meshes", mesh)
    node = {"mesh": m}
    if node_extra:
        node.update(node_extra)
    n = g.add("nodes", node)
    g.d["scenes"] = [{"nodes": [n]}]
    g.d["scene"] = 0
    return prim


def with_view(g, name, mode="embed"):
    g.write(name, mode)


def solid_material(g, **kw):
    m = {"pbrMetallicRoughness": {"baseColorFactor": [0.8, 0.4, 0.2, 1.0], "metallicFactor": 0.2, "roughnessFactor": 0.6}}
    m.update(kw)
    return g.add("materials", m)


def image_texture(g, uri=None, data=None, mime="image/png", sampler=None, name=None, image_name=None):
    if uri is not None:
        img = {"uri": uri}
    else:
        bv = g.view(data)
        img = {"bufferView": bv}
        if mime:
            img["mimeType"] = mime
    if image_name:
        img["name"] = image_name
    i = g.add("images", img)
    t = {"source": i}
    if sampler is not None:
        t["sampler"] = sampler
    if name:
        t["name"] = name
    return g.add("textures", t)


# =============================================================================================================
# A. accessors and primitive modes
# =============================================================================================================

for name, ct, vals in [("u8", 5121, QUAD_I), ("u16", 5123, QUAD_I), ("u32", 5125, QUAD_I)]:
    g = G()
    pos = g.acc(QUAD_P, 5126, "VEC3", minmax=True)
    idx = g.acc(vals, ct, "SCALAR")
    m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}, "indices": idx}]})
    n = g.add("nodes", {"mesh": m})
    g.d["scenes"] = [{"nodes": [n]}]
    g.write(f"accessor/indices-{name}", "embed")

# every primitive mode, indexed and not
for mode, label in enumerate(["points", "lines", "line-loop", "line-strip", "triangles", "triangle-strip", "triangle-fan"]):
    for indexed in (False, True):
        g = G()
        pts = [(-1, -1, 0), (1, -1, 0), (1, 1, 0), (-1, 1, 0), (0, 2, 0)]
        pos = g.acc(pts, 5126, "VEC3", minmax=True)
        prim = {"attributes": {"POSITION": pos}, "mode": mode}
        if indexed:
            prim["indices"] = g.acc([0, 1, 2, 3, 4, 0] if mode in (1, 4) else [0, 1, 2, 3, 4], 5123, "SCALAR")
        m = g.add("meshes", {"primitives": [prim]})
        n = g.add("nodes", {"mesh": m})
        g.d["scenes"] = [{"nodes": [n]}]
        g.write(f"mode/{label}-{'indexed' if indexed else 'plain'}", "embed")

# mode omitted and unusual values
for v in (7, -1, 4.5):
    g = G()
    pos = g.acc(QUAD_P, 5126, "VEC3")
    m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}, "mode": v}]})
    n = g.add("nodes", {"mesh": m})
    g.d["scenes"] = [{"nodes": [n]}]
    g.write(f"mode/odd-{str(v).replace('.', '_').replace('-', 'm')}", "embed")

# vertex attributes
g = G()
basic(g, attrs={
    "NORMAL": g.acc(QUAD_N, 5126, "VEC3"),
    "TANGENT": g.acc([(1, 0, 0, 1)] * 4, 5126, "VEC4"),
    "TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2"),
    "COLOR_0": g.acc([(1, 0, 0), (0, 1, 0), (0, 0, 1), (1, 1, 1)], 5126, "VEC3"),
}, material=solid_material(g, pbrMetallicRoughness={"baseColorTexture": {"index": 0}}))
image_texture(g, data=PNG_A)
g.write("attr/normal-tangent-uv-color", "embed")

for label, ct, typ, vals, norm in [
    ("color-vec4-u8", 5121, "VEC4", [(255, 0, 0, 255), (0, 255, 0, 128), (0, 0, 255, 64), (255, 255, 255, 255)], True),
    ("color-vec4-u16", 5123, "VEC4", [(65535, 0, 0, 65535), (0, 65535, 0, 32768), (0, 0, 65535, 16384), (65535, 65535, 65535, 65535)], True),
    ("color-vec3-u8", 5121, "VEC3", [(255, 0, 0), (0, 255, 0), (0, 0, 255), (255, 255, 255)], True),
    ("color-vec4-f32", 5126, "VEC4", [(1, 0, 0, 1), (0, 1, 0, 0.5), (0, 0, 1, 0.25), (1, 1, 1, 1)], False),
]:
    for alpha in ("OPAQUE", "BLEND"):
        g = G()
        mat = solid_material(g, alphaMode=alpha)
        basic(g, attrs={"COLOR_0": g.acc(vals, ct, typ, normalized=norm)}, material=mat)
        g.write(f"attr/{label}-{alpha.lower()}", "embed")

for label, ct, vals in [
    ("uv-u8", 5121, [(0, 255), (255, 255), (255, 0), (0, 0)]),
    ("uv-u16", 5123, [(0, 65535), (65535, 65535), (65535, 0), (0, 0)]),
    ("uv-i8", 5120, [(0, 127), (127, 127), (127, 0), (-127, -128)]),
    ("uv-i16", 5122, [(0, 32767), (32767, 32767), (32767, 0), (-32767, -32768)]),
]:
    g = G()
    mat = solid_material(g, pbrMetallicRoughness={"baseColorTexture": {"index": 0}})
    basic(g, attrs={"TEXCOORD_0": g.acc(vals, ct, "VEC2", normalized=True)}, material=mat)
    image_texture(g, data=PNG_A)
    g.write(f"attr/{label}", "embed")

# 32-bit and 64-bit and half float accessors
g = G()
basic(g, attrs={"_UNUSED": g.acc([1, 2, 3, 4], 5125, "SCALAR")})
g.write("accessor/unused-attribute", "embed")
g = G()
pos = g.acc(QUAD_P, 5130, "VEC3")
idx = g.acc(QUAD_I, 5125, "SCALAR")
m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}, "indices": idx}]})
n = g.add("nodes", {"mesh": m})
g.d["scenes"] = [{"nodes": [n]}]
g.write("accessor/float64", "embed")
g = G()
pos = g.acc(QUAD_P, 5131, "VEC3")
idx = g.acc(QUAD_I, 5123, "SCALAR")
m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}, "indices": idx}]})
n = g.add("nodes", {"mesh": m})
g.d["scenes"] = [{"nodes": [n]}]
g.write("accessor/float16", "embed")
g = G()
basic(g, attrs={"COLOR_0": g.acc([(2147483647, 0, 0, 2147483647), (0, 2147483647, 0, 1073741824), (0, 0, 2147483647, 0), (-2147483648, 1, 1, 1)], 5124, "VEC4", normalized=True)})
g.write("accessor/normalized-i32", "embed")
g = G()
basic(g, attrs={"COLOR_0": g.acc([(4294967295, 0, 0, 4294967295), (0, 2147483648, 0, 2147483647), (0, 0, 1000000000, 0), (1, 1, 1, 1)], 5125, "VEC4", normalized=True)})
g.write("accessor/normalized-u32", "embed")

# interleaved, offsets and strides
g = G()
inter = []
for p, n_, uv in zip(QUAD_P, QUAD_N, QUAD_UV):
    inter += list(p) + list(n_) + list(uv)
data = struct.pack("<" + "f" * len(inter), *inter)
bv = g.view(data, stride=32)
g.accessors.append({"bufferView": bv, "byteOffset": 0, "componentType": 5126, "count": 4, "type": "VEC3", "min": [-1, -1, 0], "max": [1, 1, 0]})
g.accessors.append({"bufferView": bv, "byteOffset": 12, "componentType": 5126, "count": 4, "type": "VEC3"})
g.accessors.append({"bufferView": bv, "byteOffset": 24, "componentType": 5126, "count": 4, "type": "VEC2"})
idx = g.acc(QUAD_I, 5123, "SCALAR")
mat = solid_material(g, pbrMetallicRoughness={"baseColorTexture": {"index": 0}})
m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": 0, "NORMAL": 1, "TEXCOORD_0": 2}, "indices": idx, "material": mat}]})
n = g.add("nodes", {"mesh": m})
g.d["scenes"] = [{"nodes": [n]}]
image_texture(g, data=PNG_A)
g.write("accessor/interleaved", "embed")

g = G()
pos = g.acc(QUAD_P, 5126, "VEC3", stride=16)
idx = g.acc(QUAD_I, 5123, "SCALAR", stride=4)
m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}, "indices": idx}]})
n = g.add("nodes", {"mesh": m})
g.d["scenes"] = [{"nodes": [n]}]
g.write("accessor/stride-padded", "embed")

g = G()
pos = g.acc(QUAD_P, 5126, "VEC3", offset=8)
idx = g.acc(QUAD_I, 5123, "SCALAR", offset=4)
m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}, "indices": idx}]})
n = g.add("nodes", {"mesh": m})
g.d["scenes"] = [{"nodes": [n]}]
g.write("accessor/byte-offset", "embed")

# sparse accessors
for label, with_base in (("with-base", True), ("no-base", False)):
    g = G()
    base_pts = [(-1, -1, 0), (1, -1, 0), (1, 1, 0), (-1, 1, 0)]
    pos_view = g.view(struct.pack("<12f", *[c for p in base_pts for c in p]))
    ind_view = g.view(struct.pack("<2H", 1, 3))
    val_view = g.view(struct.pack("<6f", 2, -2, 0, -2, 2, 0))
    a = {"componentType": 5126, "count": 4, "type": "VEC3",
         "sparse": {"count": 2, "indices": {"bufferView": ind_view, "componentType": 5123}, "values": {"bufferView": val_view}}}
    if with_base:
        a["bufferView"] = pos_view
    g.accessors.append(a)
    idx = g.acc(QUAD_I, 5123, "SCALAR")
    m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": 0}, "indices": idx}]})
    n = g.add("nodes", {"mesh": m})
    g.d["scenes"] = [{"nodes": [n]}]
    g.write(f"accessor/sparse-{label}", "embed")

# =============================================================================================================
# B. nodes, scenes, cameras, metadata
# =============================================================================================================

g = G()
prim = basic(g, node_extra={"name": "Quad Node", "translation": [1, 2, 3], "rotation": [0, 0.38268343, 0, 0.92387953], "scale": [2, 2, 2]}, mesh_extra={"name": "QuadMesh"})
g.write("node/trs-names", "embed")

g = G()
basic(g, node_extra={"matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 4, 5, 6, 1]})
g.write("node/matrix-translation", "embed")
g = G()
c, s = math.cos(0.5), math.sin(0.5)
basic(g, node_extra={"matrix": [2 * c, 2 * s, 0, 0, -3 * s, 3 * c, 0, 0, 0, 0, 4, 0, 1, 2, 3, 1]})
g.write("node/matrix-trs", "embed")
g = G()
basic(g, node_extra={"matrix": [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], "translation": [9, 9, 9]})
g.write("node/matrix-identity-wins", "embed")

g = G()
pos = g.acc(QUAD_P, 5126, "VEC3", minmax=True)
idx = g.acc(QUAD_I, 5123, "SCALAR")
m = g.add("meshes", {"name": "Shared", "primitives": [{"attributes": {"POSITION": pos}, "indices": idx}]})
a = g.add("nodes", {"name": "A", "mesh": m, "translation": [-3, 0, 0]})
b = g.add("nodes", {"name": "B", "mesh": m, "translation": [3, 0, 0], "children": [a]})
root = g.add("nodes", {"name": "Root", "children": [b, a], "rotation": [0, 0, 0.7071068, 0.7071068]})
g.d["scenes"] = [{"name": "Main", "nodes": [root]}]
g.write("node/hierarchy-shared-mesh", "embed")

g = G()
pos = g.acc(QUAD_P, 5126, "VEC3")
m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}}]})
for i in range(3):
    g.add("nodes", {"mesh": m, "translation": [i, 0, 0]})
g.d["scenes"] = [{"nodes": [0]}, {"nodes": [1, 2]}, {"name": "Third", "nodes": [2]}]
g.d["scene"] = 1
g.write("scene/multiple-default-1", "embed")
g.d["scene"] = 0
g.write("scene/multiple-default-0", "embed")
del g.d["scene"]
g.write("scene/multiple-no-default", "embed")
g.d["scenes"] = [{"nodes": [0]}]
g.d["scene"] = 2
g.write("scene/one-scene-default-2", "embed")
g.d["scenes"] = [{"nodes": []}, {"nodes": [1]}]
g.d["scene"] = 1
g.write("scene/empty-and-one", "embed")
g.d["scenes"] = [{"nodes": [0, 1, 2]}]
g.d.pop("scene", None)
g.write("scene/group-of-three", "embed")

# empty nodes and an optimizer workout
g = G()
pos = g.acc(QUAD_P, 5126, "VEC3")
m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}}]})
e1 = g.add("nodes", {"name": "Empty1"})
e2 = g.add("nodes", {"name": "Empty2", "children": [e1], "translation": [0, 1, 0]})
leaf = g.add("nodes", {"mesh": m, "scale": [1, 2, 1]})
wrap = g.add("nodes", {"children": [leaf], "translation": [1, 1, 1]})
wrap2 = g.add("nodes", {"children": [wrap]})
root = g.add("nodes", {"children": [e2, wrap2, e1]})
g.d["scenes"] = [{"nodes": [root]}]
g.write("node/optimizer-empty-chains", "embed")

# cameras
g = G()
basic(g, node_extra={"name": "Mesh"})
cam1 = g.add("cameras", {"name": "Perspective_Cam", "type": "perspective", "perspective": {"yfov": 0.6, "znear": 0.05, "zfar": 200, "aspectRatio": 1.5}})
cam2 = g.add("cameras", {"type": "orthographic", "orthographic": {"xmag": 3, "ymag": 2, "znear": 0.2, "zfar": 100}})
cam3 = g.add("cameras", {"type": "perspective", "perspective": {"yfov": 0.8}})
cam4 = g.add("cameras", {"type": "orthographic", "orthographic": {}})
cam5 = g.add("cameras", {"type": "bogus"})
n1 = g.add("nodes", {"camera": cam1, "translation": [0, 0, 5]})
n2 = g.add("nodes", {"camera": cam2, "translation": [5, 0, 0], "rotation": [0, 0.7071068, 0, 0.7071068]})
n3 = g.add("nodes", {"camera": cam3})
n4 = g.add("nodes", {"camera": cam4, "translation": [0, 3, 0]})
n5 = g.add("nodes", {"camera": cam5})
g.d["scenes"] = [{"nodes": [0, n1, n2, n3, n4, n5]}]
g.write("camera/all-kinds", "embed")

g = G()
basic(g)
cam = g.add("cameras", {"type": "perspective", "perspective": {"yfov": 0.7}})
n1 = g.add("nodes", {"camera": cam, "translation": [0, 0, 5]})
g.d["scenes"] = [{"nodes": [0, n1]}]
g.d["nodes"][0]["translation"] = [10, 0, 0]
g.write("camera/center-of-rotation", "embed")

# metadata
g = G()
basic(g)
g.d["asset"] = {"version": "2.0", "generator": "rea-b08 generator", "copyright": "synthetic", "minVersion": "2.0",
                "extras": {"author": "someone", "count": 3, "tag": "t"}, "title": "A Title"}
g.write("asset/metadata", "embed")
g = G()
basic(g)
g.d["extensionsUsed"] = ["KHR_xmp_json_ld"]
g.d["asset"]["extensions"] = {"KHR_xmp_json_ld": {"packet": 0}}
g.d["extensions"] = {"KHR_xmp_json_ld": {"packets": [{"dc:creator": {"@list": ["a", "b"]}, "dc:title": {"rdf:_1": {"@value": "T"}}, "xmp:CreatorTool": "tool", "plain": "x", "dc:rights": {"@set": ["r1", "r2", 3]}, "xmp:n": 5}]}}
g.write("asset/xmp", "embed")
g = G()
basic(g)
g.d["extensions"] = {"VRM": {"meta": {"title": "vrm title", "author": "vrm author", "version": 1}}}
g.d["extensionsUsed"] = ["VRM"]
g.write("asset/vrm-meta", "embed")
g = G()
basic(g)
g.d["asset"]["version"] = "2.1"
g.write("asset/version-2-1", "embed")
g = G()
basic(g)
g.d["asset"]["version"] = 2
g.write("asset/version-number", "embed")

# =============================================================================================================
# C. materials and textures
# =============================================================================================================

for label, mat in [
    ("factor", {"pbrMetallicRoughness": {"baseColorFactor": [0.1, 0.5, 0.9, 0.5], "metallicFactor": 0.0, "roughnessFactor": 0.3}}),
    ("blend", {"alphaMode": "BLEND", "pbrMetallicRoughness": {"baseColorFactor": [1, 0.5, 0.25, 0.4]}}),
    ("mask", {"alphaMode": "MASK", "alphaCutoff": 0.3, "pbrMetallicRoughness": {"baseColorFactor": [1, 1, 1, 0.6]}}),
    ("double-sided", {"doubleSided": True, "name": "Two Sided", "pbrMetallicRoughness": {}}),
    ("emissive-factor", {"emissiveFactor": [0.2, 0.4, 0.6], "pbrMetallicRoughness": {"metallicFactor": 1.0}}),
    ("no-pbr", {"name": "Nothing"}),
    ("odd-types", {"alphaMode": 5, "alphaCutoff": "x", "pbrMetallicRoughness": {"metallicFactor": "high", "roughnessFactor": None, "baseColorFactor": [1, 2, 3]}}),
    ("clamped", {"pbrMetallicRoughness": {"baseColorFactor": [2, -1, 0.5, 3]}, "emissiveFactor": [5, -1, 0.5]}),
]:
    g = G()
    m = g.add("materials", mat)
    basic(g, material=m)
    g.write(f"material/{label}", "embed")

# material shared by two primitives; unused; named twice
g = G()
m1 = g.add("materials", {"name": "Same", "pbrMetallicRoughness": {"baseColorFactor": [1, 0, 0, 1]}})
m2 = g.add("materials", {"name": "Same", "pbrMetallicRoughness": {"baseColorFactor": [0, 1, 0, 1]}})
m3 = g.add("materials", {"name": "Unused"})
pos = g.acc(QUAD_P, 5126, "VEC3")
mesh = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}, "material": m1}, {"attributes": {"POSITION": pos}, "material": m1}, {"attributes": {"POSITION": pos}, "material": m2}, {"attributes": {"POSITION": pos}, "material": 7}]})
n = g.add("nodes", {"mesh": mesh})
g.d["scenes"] = [{"nodes": [n]}]
g.write("material/shared-and-missing", "embed")

# textures: sources, samplers
SAMPLERS = [
    {"magFilter": 9728, "minFilter": 9728, "wrapS": 33071, "wrapT": 33071},
    {"magFilter": 9729, "minFilter": 9729, "wrapS": 33648, "wrapT": 10497},
    {"magFilter": 9729, "minFilter": 9984},
    {"minFilter": 9985},
    {"minFilter": 9986, "name": "sampler name"},
    {"minFilter": 9987, "wrapS": 1, "wrapT": 2},
    {},
]
for i, smp in enumerate(SAMPLERS):
    g = G()
    sm = g.add("samplers", smp)
    mat = solid_material(g, pbrMetallicRoughness={"baseColorTexture": {"index": 0}})
    basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
    image_texture(g, uri=data_uri(PNG_A, "image/png"), sampler=sm)
    g.write(f"texture/sampler-{i}", "embed")

g = G()
mat = solid_material(g, pbrMetallicRoughness={"baseColorTexture": {"index": 0}})
basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
image_texture(g, uri="textures/external.png", name="External Tex", image_name="ext image")
os.makedirs(os.path.join(OUT, "texture/textures"), exist_ok=True)
open(os.path.join(OUT, "texture/textures/external.png"), "wb").write(PNG_B)
g.write("texture/external-uri", "embed")

for label, mime in (("mime-png", "image/png"), ("mime-upper", "IMAGE/PNG"), ("mime-none", None), ("mime-odd", "image/png; x=1"), ("mime-jpeg", "image/jpeg")):
    g = G()
    mat = solid_material(g, pbrMetallicRoughness={"baseColorTexture": {"index": 0}})
    basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
    image_texture(g, data=PNG_A, mime=mime, image_name="from view")
    g.write(f"texture/view-{label}", "embed")

# texture reuse and the cache key
g = G()
mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}, "metallicRoughnessTexture": {"index": 1}}, "emissiveTexture": {"index": 2}, "occlusionTexture": {"index": 0, "strength": 0.5}, "normalTexture": {"index": 3, "scale": 2.0}, "emissiveFactor": [1, 1, 1]})
basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
g.add("samplers", {"minFilter": 9728})
im0 = g.add("images", {"uri": data_uri(PNG_A, "image/png"), "name": "img zero"})
im1 = g.add("images", {"uri": data_uri(PNG_B, "image/png")})
g.add("textures", {"source": im0})
g.add("textures", {"source": im0, "sampler": 0, "name": "tex one"})
g.add("textures", {"source": im1})
g.add("textures", {"source": im0, "name": "same as zero"})
g.write("texture/reuse-and-cache", "embed")

# two UV sets
g = G()
mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorTexture": {"index": 0, "texCoord": 0}, "metallicRoughnessTexture": {"index": 1, "texCoord": 1}}, "emissiveTexture": {"index": 1, "texCoord": 1}, "emissiveFactor": [1, 1, 1]})
basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2"), "TEXCOORD_1": g.acc([(0, 0), (1, 0), (1, 1), (0, 1)], 5126, "VEC2")}, material=mat)
image_texture(g, uri=data_uri(PNG_A, "image/png"))
image_texture(g, uri=data_uri(PNG_B, "image/png"))
g.write("texture/two-uv-sets", "embed")
g = G()
mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorTexture": {"index": 0, "texCoord": 1}}})
basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2"), "TEXCOORD_1": g.acc([(0, 0), (1, 0), (1, 1), (0, 1)], 5126, "VEC2")}, material=mat)
image_texture(g, uri=data_uri(PNG_A, "image/png"))
g.write("texture/only-uv-set-1", "embed")
g = G()
mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorTexture": {"index": 0, "texCoord": 3}}})
basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
image_texture(g, uri=data_uri(PNG_A, "image/png"))
g.write("texture/missing-uv-set", "embed")
g = G()
mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}})
basic(g, material=mat)
image_texture(g, uri=data_uri(PNG_A, "image/png"))
g.write("texture/no-texcoords", "embed")
g = G()
mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorTexture": {"index": 5}}})
basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
image_texture(g, uri=data_uri(PNG_A, "image/png"))
g.write("texture/index-out-of-range", "embed")

# KHR_texture_transform
def tt_case(label, mats, textures=2, uv1=False):
    g = G()
    mids = [g.add("materials", m) for m in mats]
    attrs = {"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}
    if uv1:
        attrs["TEXCOORD_1"] = g.acc([(0, 0), (1, 0), (1, 1), (0, 1)], 5126, "VEC2")
    pos = g.acc(QUAD_P, 5126, "VEC3")
    prims = []
    for m in mids:
        a = dict(attrs)
        a["POSITION"] = pos
        prims.append({"attributes": a, "material": m})
    mesh = g.add("meshes", {"primitives": prims})
    n = g.add("nodes", {"mesh": mesh})
    g.d["scenes"] = [{"nodes": [n]}]
    for i in range(textures):
        image_texture(g, uri=data_uri([PNG_A, PNG_B, PNG_C][i % 3], "image/png"))
    g.d["extensionsUsed"] = ["KHR_texture_transform"]
    g.d["extensionsRequired"] = ["KHR_texture_transform"]
    g.write(f"transform/{label}", "embed")

T = lambda **kw: {"extensions": {"KHR_texture_transform": kw}}
tt_case("offset", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(offset=[0.25, 0.5]))}}])
tt_case("rotation", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(rotation=0.7))}}])
tt_case("scale", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(scale=[2, 3]))}}])
tt_case("all", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(offset=[0.1, 0.2], rotation=0.5, scale=[1.5, 0.5]))}}])
tt_case("empty", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T())}}])
tt_case("identical-twice", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(offset=[0.3, 0.3])), "metallicRoughnessTexture": dict(index=1, **T(offset=[0.3, 0.3]))}}])
tt_case("different-two", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(offset=[0.3, 0.3])), "metallicRoughnessTexture": dict(index=1, **T(rotation=1.0))}}])
tt_case("mixed-with-plain", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(offset=[0.3, 0.3])), "metallicRoughnessTexture": {"index": 1}}}])
tt_case("texcoord-override", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(texCoord=1, offset=[0.5, 0]))}}], uv1=True)
tt_case("two-materials", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(offset=[0.2, 0.1]))}}, {"pbrMetallicRoughness": {"baseColorTexture": dict(index=1, **T(offset=[0.2, 0.1]))}}])
tt_case("three-transforms", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(offset=[0.1, 0])), "metallicRoughnessTexture": dict(index=1, **T(offset=[0, 0.1]))}, "emissiveTexture": dict(index=0, **T(scale=[2, 2])), "emissiveFactor": [1, 1, 1]}])
tt_case("bad-vectors", [{"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(offset=[0.1], scale="x", rotation="y"))}}])
tt_case("extensions-without-transform", [{"pbrMetallicRoughness": {"baseColorTexture": {"index": 0, "extensions": {"OTHER": {}}}}}])

# without the extension declared
g = G()
mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorTexture": dict(index=0, **T(offset=[0.3, 0.3]))}})
basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
image_texture(g, uri=data_uri(PNG_A, "image/png"))
g.write("transform/not-declared", "embed")

# unlit, spec/gloss
g = G()
mat = g.add("materials", {"name": "Unlit", "pbrMetallicRoughness": {"baseColorFactor": [0.9, 0.3, 0.1, 0.8], "baseColorTexture": {"index": 0}}, "normalTexture": {"index": 0, "scale": 0.5}, "extensions": {"KHR_materials_unlit": {}}})
basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
image_texture(g, uri=data_uri(PNG_A, "image/png"))
g.d["extensionsUsed"] = ["KHR_materials_unlit"]
g.write("material/unlit", "embed")

g = G()
mat = g.add("materials", {"extensions": {"KHR_materials_unlit": {}}})
basic(g, material=mat)
g.d["extensionsUsed"] = ["KHR_materials_unlit"]
g.write("material/unlit-plain", "embed")

for label, sg in [
    ("full", {"diffuseFactor": [0.8, 0.2, 0.1, 0.7], "specularFactor": [0.5, 0.6, 0.7], "glossinessFactor": 0.4, "diffuseTexture": {"index": 0}, "specularGlossinessTexture": {"index": 0}}),
    ("defaults", {}),
    ("no-specular", {"diffuseFactor": [1, 1, 1, 1]}),
]:
    g = G()
    mat = g.add("materials", {"emissiveFactor": [0.1, 0.1, 0.1], "occlusionTexture": {"index": 0}, "extensions": {"KHR_materials_pbrSpecularGlossiness": sg}})
    basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
    image_texture(g, uri=data_uri(PNG_A, "image/png"))
    g.d["extensionsUsed"] = ["KHR_materials_pbrSpecularGlossiness"]
    g.write(f"material/specgloss-{label}", "embed")

g = G()
mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorFactor": [1, 0, 0, 1]}, "extensions": {"KHR_materials_pbrSpecularGlossiness": {"diffuseFactor": [0, 1, 0, 1]}}})
basic(g, material=mat)
g.d["extensionsUsed"] = ["KHR_materials_pbrSpecularGlossiness"]
g.write("material/specgloss-and-metal", "embed")

# every KHR_materials_* extension
EXTS = {
    "anisotropy": {"anisotropyStrength": 0.7, "anisotropyRotation": 0.5, "anisotropyTexture": {"index": 0}},
    "clearcoat": {"clearcoatFactor": 0.8, "clearcoatRoughnessFactor": 0.1, "clearcoatTexture": {"index": 0}, "clearcoatRoughnessTexture": {"index": 0}, "clearcoatNormalTexture": {"index": 0, "scale": 1.5}},
    "diffuse_transmission": {"diffuseTransmissionFactor": 0.4, "diffuseTransmissionColorFactor": [0.9, 0.5, 0.2], "diffuseTransmissionTexture": {"index": 0}, "diffuseTransmissionColorTexture": {"index": 0}},
    "dispersion": {"dispersion": 0.3},
    "emissive_strength": {"emissiveStrength": 4.5},
    "ior": {"ior": 1.8},
    "iridescence": {"iridescenceFactor": 0.9, "iridescenceIor": 1.5, "iridescenceThicknessMinimum": 120, "iridescenceThicknessMaximum": 500, "iridescenceTexture": {"index": 0}, "iridescenceThicknessTexture": {"index": 0}},
    "retroreflection": {"retroreflectionFactor": 0.6, "retroreflectionTexture": {"index": 0}},
    "sheen": {"sheenColorFactor": [0.4, 0.5, 0.6], "sheenRoughnessFactor": 0.35, "sheenColorTexture": {"index": 0}, "sheenRoughnessTexture": {"index": 0}},
    "specular": {"specularFactor": 0.7, "specularColorFactor": [0.8, 0.4, 0.2], "specularTexture": {"index": 0}, "specularColorTexture": {"index": 0}},
    "transmission": {"transmissionFactor": 0.9, "transmissionTexture": {"index": 0}},
    "volume": {"thicknessFactor": 2.5, "attenuationDistance": 8, "attenuationColor": [0.5, 0.7, 0.9], "thicknessTexture": {"index": 0}},
    "scatter": {"scatterStrengthFactor": 0.45, "scatterAnisotropy": 0.2, "multiscatterColorFactor": [0.3, 0.6, 0.9], "scatterStrengthTexture": {"index": 0}, "multiscatterColorTexture": {"index": 0}, "scatterTexture": {"index": 0}},
    "volume_scatter": {"scatterStrengthFactor": 0.45, "multiscatterColorFactor": [0.3, 0.6, 0.9]},
    "specular-black": {"specularColorFactor": [0, 0, 0]},
}
for label, ext in EXTS.items():
    for with_tex in (True, False):
        if label == "dispersion" or label == "emissive_strength" or label == "ior" or label == "volume_scatter" or label == "specular-black":
            if with_tex:
                continue
        g = G()
        key = label.split("-")[0]
        e = ext if with_tex else {k: v for k, v in ext.items() if not (isinstance(v, dict) and "index" in v)}
        mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorFactor": [0.8, 0.8, 0.8, 1], "metallicFactor": 0.1}, "extensions": {f"KHR_materials_{key}": e}})
        basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
        image_texture(g, uri=data_uri(PNG_A, "image/png"))
        g.d["extensionsUsed"] = [f"KHR_materials_{key}"]
        g.write(f"extension/{label}-{'tex' if with_tex else 'plain'}", "embed")

# several extensions on one material, in unusual key order, and with null
g = G()
mat = g.add("materials", {"extensions": {"KHR_materials_volume": {"thicknessFactor": 1}, "KHR_materials_clearcoat": {"clearcoatFactor": 1}, "KHR_materials_transmission": {"transmissionFactor": 0.5}, "KHR_materials_ior": {"ior": 1.4}, "KHR_materials_sheen": {}}})
basic(g, material=mat)
g.d["extensionsUsed"] = ["KHR_materials_volume", "KHR_materials_clearcoat", "KHR_materials_transmission", "KHR_materials_ior", "KHR_materials_sheen"]
g.write("extension/many-order", "embed")
g = G()
mat = g.add("materials", {"extensions": {"KHR_materials_clearcoat": None, "KHR_materials_sheen": 5, "KHR_materials_unknown": {}}})
basic(g, material=mat)
g.write("extension/non-object-values", "embed")
g = G()
mat = g.add("materials", {"extensions": {"KHR_materials_ior": None}})
basic(g, material=mat)
g.write("extension/ior-null", "embed")

# material variants
g = G()
m0 = g.add("materials", {"name": "Red", "pbrMetallicRoughness": {"baseColorFactor": [1, 0, 0, 1]}})
m1 = g.add("materials", {"name": "Green", "pbrMetallicRoughness": {"baseColorFactor": [0, 1, 0, 1]}})
m2 = g.add("materials", {"name": "Blue", "pbrMetallicRoughness": {"baseColorFactor": [0, 0, 1, 1]}})
basic(g, material=m0, prim_extra={"extensions": {"KHR_materials_variants": {"mappings": [{"material": m1, "variants": [0]}, {"material": m2, "variants": [1, 2]}]}}})
g.d["extensions"] = {"KHR_materials_variants": {"variants": [{"name": "green_variant"}, {"name": "blue variant"}, {"name": "third"}]}}
g.d["extensionsUsed"] = ["KHR_materials_variants"]
g.write("variants/basic", "embed")
g = G()
basic(g, material=m0 if False else solid_material(g), prim_extra={"extensions": {"KHR_materials_variants": {"mappings": [{"material": 0, "variants": [2]}]}}})
g.d["extensions"] = {"KHR_materials_variants": {"variants": [{"name": "a"}, {"name": "b"}, {"name": "c"}]}}
g.write("variants/sparse-index", "embed")
g = G()
basic(g, material=solid_material(g), prim_extra={"extensions": {"KHR_materials_variants": {"mappings": [{"material": 9, "variants": [0]}]}}})
g.d["extensions"] = {"KHR_materials_variants": {"variants": [{"name": "a"}]}}
g.write("variants/missing-material", "embed")
g = G()
basic(g, material=solid_material(g), prim_extra={"extensions": {"KHR_materials_variants": {"mappings": []}}})
g.d["extensions"] = {"KHR_materials_variants": {"variants": [{"name": "a"}]}}
g.write("variants/empty-mappings", "embed")
g = G()
m0 = g.add("materials", {"pbrMetallicRoughness": {"baseColorFactor": [1, 0, 0, 1]}})
m1 = g.add("materials", {"pbrMetallicRoughness": {"baseColorFactor": [0, 1, 0, 1]}})
pos = g.acc(QUAD_P, 5126, "VEC3")
ext = {"extensions": {"KHR_materials_variants": {"mappings": [{"material": m1, "variants": [0]}]}}}
mesh = g.add("meshes", {"primitives": [dict({"attributes": {"POSITION": pos}, "material": m0}, **ext), dict({"attributes": {"POSITION": pos}, "material": m0}, **ext)]})
n = g.add("nodes", {"mesh": mesh})
g.d["scenes"] = [{"nodes": [n]}]
g.d["extensions"] = {"KHR_materials_variants": {"variants": [{"name": "v"}]}}
g.write("variants/two-primitives", "embed")
g = G()
basic(g, material=solid_material(g), prim_extra={"extensions": {"KHR_materials_variants": {"mappings": [{"material": 0, "variants": [0]}]}}})
g.d["extensions"] = {"KHR_materials_variants": {"variants": [{}]}}
g.write("variants/nameless", "embed")

# =============================================================================================================
# D. lights, visibility, instancing, splats
# =============================================================================================================

g = G()
basic(g)
g.d["extensionsUsed"] = ["KHR_lights_punctual"]
g.d["extensions"] = {"KHR_lights_punctual": {"lights": [
    {"type": "directional", "name": "Sun", "color": [1, 0.9, 0.8], "intensity": 3},
    {"type": "spot", "color": [0.5, 0.5, 1], "intensity": 50, "range": 12, "spot": {"innerConeAngle": 0.2, "outerConeAngle": 0.9}, "innerConeAngle": 0.2, "outerConeAngle": 0.9},
    {"type": "point", "intensity": 10, "range": 0},
    {"type": "point", "name": "Lamp"},
    {"type": "bogus"},
    {"type": "spot"},
]}}
ns = [g.add("nodes", {"extensions": {"KHR_lights_punctual": {"light": i}}, "translation": [i, 0, 0]}) for i in range(7)]
g.d["scenes"][0]["nodes"] += ns
g.write("light/punctual", "embed")

# EXT_lights_image_based
g = G()
basic(g)
g.d["extensionsUsed"] = ["EXT_lights_image_based"]
imgs = [g.add("images", {"uri": data_uri(f, "image/png"), "name": f"face{i}"} if i != 3 else {"uri": data_uri(f, "image/png")}) for i, f in enumerate(FACE)]
g.d["extensions"] = {"EXT_lights_image_based": {"lights": [
    {"intensity": 1.5, "irradianceCoefficients": [[0.1 * i, 0.2, 0.3] for i in range(9)], "specularImageSize": 8, "specularImages": [imgs] + [imgs]},
    {"specularImages": [imgs[:4]]},
    {},
]}}
g.d["scenes"] = [{"nodes": [0], "extensions": {"EXT_lights_image_based": {"light": 0}}}, {"nodes": [0], "extensions": {"EXT_lights_image_based": {"light": 1}}}, {"nodes": [0], "extensions": {"EXT_lights_image_based": {"light": 2}}}]
g.d["scene"] = 0
g.write("light/image-based", "embed")

# visibility
g = G()
pos = g.acc(QUAD_P, 5126, "VEC3")
m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}}]})
a = g.add("nodes", {"mesh": m, "extensions": {"KHR_node_visibility": {"visible": False}}})
b = g.add("nodes", {"mesh": m, "translation": [3, 0, 0], "extensions": {"KHR_node_visibility": {}}})
c = g.add("nodes", {"mesh": m, "translation": [6, 0, 0], "extensions": {"KHR_node_visibility": {"visible": True}}})
d = g.add("nodes", {"mesh": m, "translation": [9, 0, 0], "extensions": {"KHR_node_visibility": 5}})
g.d["scenes"] = [{"nodes": [a, b, c, d]}]
g.d["extensionsUsed"] = ["KHR_node_visibility"]
g.write("visibility/basic", "embed")

# EXT_mesh_gpu_instancing
def inst(label, t=None, r=None, s=None, extra_nodes=0, two_nodes=False):
    g = G()
    pos = g.acc(QUAD_P, 5126, "VEC3", minmax=True)
    idx = g.acc(QUAD_I, 5123, "SCALAR")
    m = g.add("meshes", {"name": "Instanced", "primitives": [{"attributes": {"POSITION": pos}, "indices": idx}]})
    attrs = {}
    if t:
        attrs["TRANSLATION"] = g.acc(t, 5126, "VEC3")
    if r:
        attrs["ROTATION"] = g.acc(r, 5126, "VEC4")
    if s:
        attrs["SCALE"] = g.acc(s, 5126, "VEC3")
    nodes = [g.add("nodes", {"mesh": m, "extensions": {"EXT_mesh_gpu_instancing": {"attributes": attrs}}})]
    if two_nodes:
        nodes.append(g.add("nodes", {"mesh": m, "translation": [0, 5, 0], "extensions": {"EXT_mesh_gpu_instancing": {"attributes": attrs}}}))
        nodes.append(g.add("nodes", {"mesh": m, "translation": [0, 9, 0]}))
    g.d["scenes"] = [{"nodes": nodes}]
    g.d["extensionsUsed"] = ["EXT_mesh_gpu_instancing"]
    g.write(f"instancing/{label}", "embed")

inst("translations", t=[(0, 0, 0), (3, 0, 0), (0, 3, 0)])
inst("all", t=[(0, 0, 0), (3, 0, 0)], r=[(0, 0, 0, 1), (0, 0.7071068, 0, 0.7071068)], s=[(1, 1, 1), (2, 0.5, 1)])
inst("rotation-only", r=[(0, 0, 0, 1), (0, 0, 0.7071068, 0.7071068), (0.1, 0.2, 0.3, 0.9)])
inst("scale-only", s=[(1, 1, 1), (2, 2, 2)])
inst("empty")
inst("two-nodes-one-mesh", t=[(0, 0, 0), (3, 0, 0)], two_nodes=True)

# Gaussian splats
g = G()
pos = g.acc([(0, 0, 0), (1, 0, 0), (0, 1, 0)], 5126, "VEC3", minmax=True)
rot = g.acc([(0, 0, 0, 1)] * 3, 5126, "VEC4")
sc = g.acc([(0.1, 0.1, 0.1)] * 3, 5126, "VEC3")
op = g.acc([0.5, 0.8, 1.0], 5126, "SCALAR")
sh0 = g.acc([(0.5, 0.2, 0.1)] * 3, 5126, "VEC3")
m = g.add("meshes", {"primitives": [{"mode": 0, "attributes": {"POSITION": pos, "KHR_gaussian_splatting:ROTATION": rot, "KHR_gaussian_splatting:SCALE": sc, "KHR_gaussian_splatting:OPACITY": op, "KHR_gaussian_splatting:SH_DEGREE_0_COEF_0": sh0}, "extensions": {"KHR_gaussian_splatting": {"colorSpace": "srgb_rec709_display"}}}]})
n = g.add("nodes", {"mesh": m})
g.d["scenes"] = [{"nodes": [n]}]
g.d["extensionsUsed"] = ["KHR_gaussian_splatting"]
g.write("splat/basic", "embed")
g = G()
pos = g.acc([(0, 0, 0), (1, 0, 0)], 5126, "VEC3")
m = g.add("meshes", {"primitives": [{"mode": 0, "attributes": {"POSITION": pos}, "extensions": {"KHR_gaussian_splatting": {}}}]})
n = g.add("nodes", {"mesh": m})
g.d["scenes"] = [{"nodes": [n]}]
g.d["extensionsUsed"] = ["KHR_gaussian_splatting"]
g.write("splat/minimal", "embed")

# =============================================================================================================
# E. skins, morph targets, animations
# =============================================================================================================

def skin_scene(g, joints_ct=5121, with_ibm=True, skeleton=None, two_prims=False, extra=None):
    pts = [(-1, 0, 0), (1, 0, 0), (-1, 1, 0), (1, 1, 0), (-1, 2, 0), (1, 2, 0)]
    pos = g.acc(pts, 5126, "VEC3", minmax=True)
    nrm = g.acc([(0, 0, 1)] * 6, 5126, "VEC3")
    idx = g.acc([0, 1, 3, 0, 3, 2, 2, 3, 5, 2, 5, 4], 5123, "SCALAR")
    jts = g.acc([(0, 0, 0, 0), (0, 0, 0, 0), (0, 1, 0, 0), (0, 1, 0, 0), (1, 0, 0, 0), (1, 0, 0, 0)], joints_ct, "VEC4")
    w = g.acc([(1, 0, 0, 0), (1, 0, 0, 0), (0.5, 0.5, 0, 0), (0.5, 0.5, 0, 0), (1, 0, 0, 0), (1, 0, 0, 0)], 5126, "VEC4")
    prim = {"attributes": {"POSITION": pos, "NORMAL": nrm, "JOINTS_0": jts, "WEIGHTS_0": w}, "indices": idx}
    prims = [prim]
    if two_prims:
        prims.append(dict(prim))
    m = g.add("meshes", {"primitives": prims})
    j0 = g.add("nodes", {"name": "Joint0", "children": [1], "translation": [0, 0, 0]})
    j1 = g.add("nodes", {"name": "Joint1", "translation": [0, 1, 0]})
    skin = {"joints": [j0, j1], "name": "Skin"}
    if with_ibm:
        skin["inverseBindMatrices"] = g.acc([[1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1], [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, -1, 0, 1]], 5126, "MAT4")
    if skeleton is not None:
        skin["skeleton"] = skeleton
    sk = g.add("skins", skin)
    mesh_node = g.add("nodes", {"name": "SkinnedMesh", "mesh": m, "skin": sk})
    g.d["scenes"] = [{"nodes": [j0, mesh_node]}]
    return j0, j1, mesh_node

g = G()
skin_scene(g)
g.write("skin/basic", "embed")
g = G()
skin_scene(g, joints_ct=5123)
g.write("skin/joints-u16", "embed")
g = G()
skin_scene(g, with_ibm=False)
g.write("skin/no-inverse-bind", "embed")
g = G()
skin_scene(g, skeleton=0)
g.write("skin/explicit-skeleton", "embed")
g = G()
skin_scene(g, two_prims=True)
g.write("skin/two-primitives", "embed")
g = G()
j0, j1, mn = skin_scene(g)
cam = g.add("cameras", {"type": "perspective", "perspective": {"yfov": 0.7}})
cn = g.add("nodes", {"camera": cam, "translation": [0, 1, 6]})
g.d["scenes"][0]["nodes"].append(cn)
g.write("skin/with-camera", "embed")
g = G()
skin_scene(g)
g.d["extensions"] = {"VRM": {"meta": {"title": "x"}}}
g.write("skin/vrm", "embed")


# a humanoid's bounding box is empty while the parser runs: the camera's center of rotation
# comes from the plain mesh only
g = G()
j0, j1, mn = skin_scene(g)
cam = g.add("cameras", {"type": "perspective", "perspective": {"yfov": 0.7}})
cn = g.add("nodes", {"camera": cam, "translation": [0, 1, 6]})
m2 = g.add("meshes", {"primitives": [{"attributes": {"POSITION": 0}}]})
pn = g.add("nodes", {"mesh": m2, "translation": [10, 0, 0]})
g.d["scenes"][0]["nodes"] += [cn, pn]
g.write("skin/camera-with-plain-mesh", "embed")
g = G()
j0, j1, mn = skin_scene(g)
m2 = g.add("meshes", {"primitives": [{"attributes": {"POSITION": 0}}]})
g.d["nodes"][j1]["mesh"] = m2
g.d["nodes"][j1]["translation"] = [10, 0, 0]
cam = g.add("cameras", {"type": "perspective", "perspective": {"yfov": 0.7}})
cn = g.add("nodes", {"camera": cam, "translation": [0, 1, 6]})
g.d["scenes"][0]["nodes"].append(cn)
g.write("skin/camera-joint-has-mesh", "embed")


# external .bin buffer, two buffers, percent-encoded names
g = G()
basic(g, attrs={"NORMAL": g.acc(QUAD_N, 5126, "VEC3")})
g.write("buffer/external-bin", "external")
g = G()
basic(g)
d = g.finish()
b = bytes(g.buf)
half = len(b) // 2
os.makedirs(os.path.join(OUT, "buffer"), exist_ok=True)
open(os.path.join(OUT, "buffer/two part.bin"), "wb").write(b)
d["buffers"] = [{"byteLength": len(b), "uri": "two%20part.bin"}]
open(os.path.join(OUT, "buffer/percent-name.gltf"), "w").write(json.dumps(d))
d["buffers"] = [{"byteLength": len(b), "uri": "two part.bin?x=1#frag"}]
open(os.path.join(OUT, "buffer/query-name.gltf"), "w").write(json.dumps(d))

# morph targets
def morph_scene(g, weights=(0.0, 0.0), with_normal=True, node_weights=None):
    pts = [(-1, -1, 0), (1, -1, 0), (1, 1, 0), (-1, 1, 0)]
    pos = g.acc(pts, 5126, "VEC3", minmax=True)
    nrm = g.acc(QUAD_N, 5126, "VEC3")
    idx = g.acc(QUAD_I, 5123, "SCALAR")
    t0 = {"POSITION": g.acc([(0, 0, 1), (0, 0, 1), (0, 0, -1), (0, 0, -1)], 5126, "VEC3")}
    t1 = {"POSITION": g.acc([(1, 0, 0), (1, 0, 0), (1, 0, 0), (1, 0, 0)], 5126, "VEC3")}
    if with_normal:
        t0["NORMAL"] = g.acc([(0, 0.5, 0)] * 4, 5126, "VEC3")
        t1["NORMAL"] = g.acc([(0.5, 0, 0)] * 4, 5126, "VEC3")
    prim = {"attributes": {"POSITION": pos}, "indices": idx, "targets": [t0, t1]}
    if with_normal:
        prim["attributes"]["NORMAL"] = nrm
    mesh = {"primitives": [prim], "weights": list(weights)}
    m = g.add("meshes", mesh)
    n = g.add("nodes", {"name": "Morph", "mesh": m})
    g.d["scenes"] = [{"nodes": [n]}]
    return n

g = G()
morph_scene(g, weights=(0.5, 0.25))
g.write("morph/static-weights", "embed")
g = G()
morph_scene(g, weights=(0.5, 0.25), with_normal=False)
g.write("morph/no-normals", "embed")
g = G()
morph_scene(g, weights=())
g.write("morph/empty-weights", "embed")

def anim(g, name, target_node, path, times, values, interp="LINEAR", vt="VEC3", extra_target=None, ct=5126):
    t = g.acc(times, 5126, "SCALAR", minmax=True)
    v = g.acc(values, ct, vt)
    s = {"input": t, "output": v, "interpolation": interp}
    tgt = {"node": target_node, "path": path}
    if extra_target:
        tgt.update(extra_target)
    return g.add("animations", {"name": name, "samplers": [s], "channels": [{"sampler": 0, "target": tgt}]})

for interp in ("LINEAR", "STEP", "CUBICSPLINE"):
    # translation
    g = G()
    n = morph_scene(g, weights=(0.0, 0.0))
    times = [0, 0.5, 1.0]
    if interp == "CUBICSPLINE":
        vals = [(0, 0, 0), (0, 0, 0), (1, 0, 0), (0, 0, 0), (1, 1, 0), (0, 0, 0), (0, 0, 0), (2, 0, 0), (0, 0, 0)]
    else:
        vals = [(0, 0, 0), (1, 1, 0), (2, 0, 0)]
    anim(g, "Move", n, "translation", times, vals, interp)
    g.write(f"animation/translation-{interp.lower()}", "embed")
    # rotation
    g = G()
    n = morph_scene(g, weights=(0.0, 0.0))
    q = [(0, 0, 0, 1), (0, 0.38268343, 0, 0.92387953), (0, 0.7071068, 0, 0.7071068)]
    if interp == "CUBICSPLINE":
        q = [(0, 0, 0, 0), (0, 0, 0, 1), (0, 0, 0, 0), (0, 0, 0, 0), (0, 0.38268343, 0, 0.92387953), (0, 0, 0, 0), (0, 0, 0, 0), (0, 0.7071068, 0, 0.7071068), (0, 0, 0, 0)]
    anim(g, "Spin", n, "rotation", times, q, interp, vt="VEC4")
    g.write(f"animation/rotation-{interp.lower()}", "embed")
    # scale
    g = G()
    n = morph_scene(g, weights=(0.0, 0.0))
    vals = [(1, 1, 1), (2, 2, 2), (1, 3, 1)]
    if interp == "CUBICSPLINE":
        vals = [(0, 0, 0), (1, 1, 1), (0, 0, 0), (0, 0, 0), (2, 2, 2), (0, 0, 0), (0, 0, 0), (1, 3, 1), (0, 0, 0)]
    anim(g, "Grow", n, "scale", times, vals, interp)
    g.write(f"animation/scale-{interp.lower()}", "embed")
    # weights
    g = G()
    n = morph_scene(g, weights=(0.0, 0.0))
    if interp == "CUBICSPLINE":
        wv = [0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0.5, 1, 0, 0, 0.5, 0, 0, 0]
        wv = [0, 0, 0, 0, 0, 0] + [0, 0, 1, 0, 0, 0] + [0, 0, 0.5, 1, 0, 0]
        wv = [0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 0, 0, 0, 0, 0.5, 1, 0, 0]
    else:
        wv = [0, 0, 1, 0, 0.5, 1]
    anim(g, "Morph", n, "weights", times, wv, interp, vt="SCALAR")
    g.write(f"animation/weights-{interp.lower()}", "embed")

# multiple channels, multiple animations, cycle interval and edge cases
g = G()
n = morph_scene(g, weights=(0.0, 0.0))
t = g.acc([0, 1, 2], 5126, "SCALAR", minmax=True)
t2 = g.acc([0, 3], 5126, "SCALAR")
v1 = g.acc([(0, 0, 0), (1, 0, 0), (2, 0, 0)], 5126, "VEC3")
v2 = g.acc([(1, 1, 1), (3, 3, 3)], 5126, "VEC3")
v3 = g.acc([(0, 0, 0, 1), (0, 0.7071068, 0, 0.7071068)], 5126, "VEC4")
g.add("animations", {"name": "A one", "samplers": [{"input": t, "output": v1}, {"input": t2, "output": v2}, {"input": t2, "output": v3}], "channels": [{"sampler": 0, "target": {"node": n, "path": "translation"}}, {"sampler": 1, "target": {"node": n, "path": "scale"}}, {"sampler": 2, "target": {"node": n, "path": "rotation"}}]})
g.add("animations", {"samplers": [{"input": t2, "output": v2}], "channels": [{"sampler": 0, "target": {"node": n, "path": "scale"}}]})
g.add("animations", {"samplers": [], "channels": []})
g.add("animations", {"name": "bad channel", "samplers": [{"input": t, "output": v1}], "channels": [{"sampler": 4, "target": {"node": n, "path": "translation"}}, {"sampler": 0, "target": {"node": 99, "path": "translation"}}, {"sampler": 0, "target": {"node": n, "path": "visibility"}}, {"sampler": 0}]})
g.write("animation/multiple", "embed")

# single key, unsorted, large
g = G()
n = morph_scene(g, weights=(0.0, 0.0))
anim(g, "OneKey", n, "translation", [0.0], [(1, 2, 3)])
g.write("animation/single-key", "embed")
g = G()
n = morph_scene(g, weights=(0.0, 0.0))
anim(g, "Long", n, "translation", [i * 0.1 for i in range(40)], [(i * 0.1, 0, 0) for i in range(40)], "CUBICSPLINE" if False else "LINEAR")
g.write("animation/many-keys", "embed")
g = G()
n = morph_scene(g, weights=(0.0, 0.0))
times = [0, 0.5, 1.0]
vals = [(0, 0, 0), (0, 0, 0), (1, 0, 0), (0, 0, 0), (1, 1, 0), (0, 0, 0), (0, 0, 0), (2, 0, 0), (0, 0, 0)]
anim(g, "Short", n, "translation", [0, 0.01, 0.02], [(0, 0, 0), (0, 0, 0), (1, 0, 0), (0, 0, 0), (1, 1, 0), (0, 0, 0), (0, 0, 0), (2, 0, 0), (0, 0, 0)], "CUBICSPLINE")
g.write("animation/cubic-short", "embed")

# animation on a skin
g = G()
j0, j1, mn = skin_scene(g)
anim(g, "Bend", j1, "rotation", [0, 1], [(0, 0, 0, 1), (0, 0, 0.3826834, 0.9238795)], vt="VEC4")
g.write("animation/skin-joint", "embed")

# KHR_animation_pointer
def ptr_case(label, setup, pointer, vt, values, times=(0, 1), interp="LINEAR", path_ok=True):
    g = G()
    setup(g)
    t = g.acc(list(times), 5126, "SCALAR", minmax=True)
    v = g.acc(values, 5126, vt)
    g.add("animations", {"name": label, "samplers": [{"input": t, "output": v, "interpolation": interp}],
                         "channels": [{"sampler": 0, "target": {"path": "pointer", "extensions": {"KHR_animation_pointer": {"pointer": pointer}}}}]})
    g.d["extensionsUsed"] = ["KHR_animation_pointer"]
    g.d["extensionsRequired"] = ["KHR_animation_pointer"]
    g.write(f"pointer/{label}", "embed")

def mat_scene(g):
    m = g.add("materials", {"name": "M", "pbrMetallicRoughness": {"baseColorFactor": [1, 1, 1, 1], "baseColorTexture": {"index": 0, "extensions": {"KHR_texture_transform": {"offset": [0, 0]}}}, "metallicFactor": 1, "roughnessFactor": 1}, "emissiveFactor": [0, 0, 0],
                             "extensions": {"KHR_materials_emissive_strength": {"emissiveStrength": 1}, "KHR_materials_clearcoat": {"clearcoatFactor": 0}}})
    basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=m)
    image_texture(g, uri=data_uri(PNG_A, "image/png"))
    g.d["extensionsUsed"] = ["KHR_texture_transform", "KHR_materials_emissive_strength", "KHR_materials_clearcoat"]

ptr_case("base-color", mat_scene, "/materials/0/pbrMetallicRoughness/baseColorFactor", "VEC4", [(1, 1, 1, 1), (1, 0, 0, 0.5)])
ptr_case("base-color-opaque", mat_scene, "/materials/0/pbrMetallicRoughness/baseColorFactor", "VEC4", [(1, 1, 1, 1), (1, 0, 0, 1)])
ptr_case("metallic", mat_scene, "/materials/0/pbrMetallicRoughness/metallicFactor", "SCALAR", [1, 0])
ptr_case("roughness-step", mat_scene, "/materials/0/pbrMetallicRoughness/roughnessFactor", "SCALAR", [1, 0, 0.5], times=(0, 1, 2), interp="STEP")
ptr_case("emissive", mat_scene, "/materials/0/emissiveFactor", "VEC3", [(0, 0, 0), (1, 0.5, 0)])
ptr_case("emissive-strength", mat_scene, "/materials/0/extensions/KHR_materials_emissive_strength/emissiveStrength", "SCALAR", [1, 5])
ptr_case("clearcoat", mat_scene, "/materials/0/extensions/KHR_materials_clearcoat/clearcoatFactor", "SCALAR", [0, 1])
ptr_case("alpha-cutoff", mat_scene, "/materials/0/alphaCutoff", "SCALAR", [0.1, 0.9])
ptr_case("tt-offset", mat_scene, "/materials/0/pbrMetallicRoughness/baseColorTexture/extensions/KHR_texture_transform/offset", "VEC2", [(0, 0), (0.5, 0.25)])
ptr_case("tt-rotation", mat_scene, "/materials/0/pbrMetallicRoughness/baseColorTexture/extensions/KHR_texture_transform/rotation", "SCALAR", [0, 1.5])
ptr_case("tt-scale", mat_scene, "/materials/0/pbrMetallicRoughness/baseColorTexture/extensions/KHR_texture_transform/scale", "VEC2", [(1, 1), (2, 3)])
ptr_case("bad-path", mat_scene, "/materials/0/nothing/here", "SCALAR", [0, 1])
ptr_case("no-path", mat_scene, "", "SCALAR", [0, 1])

def node_scene(g):
    basic(g, node_extra={"name": "N", "extensions": {"KHR_node_visibility": {"visible": True}}})
    g.d["extensionsUsed"] = ["KHR_node_visibility"]

ptr_case("node-translation", node_scene, "/nodes/0/translation", "VEC3", [(0, 0, 0), (1, 2, 3)])
ptr_case("node-rotation", node_scene, "/nodes/0/rotation", "VEC4", [(0, 0, 0, 1), (0, 0.7071068, 0, 0.7071068)])
ptr_case("node-scale", node_scene, "/nodes/0/scale", "VEC3", [(1, 1, 1), (2, 2, 2)])
ptr_case("node-weights", node_scene, "/nodes/0/weights", "SCALAR", [0, 1])
ptr_case("node-visible", node_scene, "/nodes/0/extensions/KHR_node_visibility/visible", "SCALAR", [1, 0])

def light_cam_scene(g):
    basic(g)
    cam = g.add("cameras", {"type": "perspective", "perspective": {"yfov": 0.7, "znear": 0.1, "zfar": 10}})
    ocam = g.add("cameras", {"type": "orthographic", "orthographic": {"xmag": 2, "ymag": 1, "znear": 0.1, "zfar": 10}})
    g.d["extensions"] = {"KHR_lights_punctual": {"lights": [{"type": "spot", "intensity": 5, "range": 10, "color": [1, 1, 1]}, {"type": "point", "intensity": 2}]}}
    n1 = g.add("nodes", {"camera": cam})
    n2 = g.add("nodes", {"camera": ocam})
    n3 = g.add("nodes", {"extensions": {"KHR_lights_punctual": {"light": 0}}})
    n4 = g.add("nodes", {"extensions": {"KHR_lights_punctual": {"light": 1}}})
    g.d["scenes"][0]["nodes"] += [n1, n2, n3, n4]
    g.d["extensionsUsed"] = ["KHR_lights_punctual"]

ptr_case("camera-yfov", light_cam_scene, "/cameras/0/perspective/yfov", "SCALAR", [0.5, 1.0])
ptr_case("camera-znear", light_cam_scene, "/cameras/0/perspective/znear", "SCALAR", [0.1, 0.5])
ptr_case("camera-ortho-xmag", light_cam_scene, "/cameras/1/orthographic/xmag", "SCALAR", [2, 4])
ptr_case("camera-ortho-zfar", light_cam_scene, "/cameras/1/orthographic/zfar", "SCALAR", [10, 20])
ptr_case("light-intensity", light_cam_scene, "/extensions/KHR_lights_punctual/lights/0/intensity", "SCALAR", [5, 50])
ptr_case("light-color", light_cam_scene, "/extensions/KHR_lights_punctual/lights/0/color", "VEC3", [(1, 1, 1), (1, 0, 0)])
ptr_case("light-range", light_cam_scene, "/extensions/KHR_lights_punctual/lights/1/range", "SCALAR", [1, 9])
ptr_case("light-spot-cone", light_cam_scene, "/extensions/KHR_lights_punctual/lights/0/spot/outerConeAngle", "SCALAR", [0.5, 1.0])

# =============================================================================================================
# F. GLB
# =============================================================================================================

g = G()
basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=solid_material(g, pbrMetallicRoughness={"baseColorTexture": {"index": 0}}))
image_texture(g, data=PNG_A, mime="image/png", image_name="glb image")
glb_with(g, "glb/textured")
g = G()
basic(g)
glb_with(g, "glb/plain")
g = G()
basic(g)
g.d["buffers"] = []
d = g.finish()
d["buffers"] = [{"byteLength": len(g.buf)}]
write_glb(os.path.join(OUT, "glb/json-only-no-bin.glb"), {"asset": {"version": "2.0"}, "scenes": [{"nodes": [0]}], "nodes": [{}]}, None)
g = G()
basic(g)
glb_with(g, "glb/unknown-chunk", extra_chunks=[(0x12345678, b"abcd")])
g = G()
basic(g)
d = g.finish()
buf = bytes(g.buf)
d["buffers"] = [{"byteLength": len(buf)}, {"byteLength": 8, "uri": data_uri(b"\0" * 8)}]
write_glb(os.path.join(OUT, "glb/two-buffers.glb"), d, buf)
g = G()
basic(g)
d = g.finish()
d["buffers"] = [{"byteLength": len(g.buf), "uri": data_uri(bytes(g.buf))}]
write_glb(os.path.join(OUT, "glb/uri-buffer.glb"), d, None)
g = G()
basic(g)
d = g.finish()
buf = bytes(g.buf)
d["buffers"] = [{"byteLength": len(buf)}]
os.makedirs(os.path.join(OUT, "glb"), exist_ok=True)
write_glb(os.path.join(OUT, "glb/two-json-chunks.glb"), d, buf, extra_chunks=[(0x4E4F534A, json.dumps({"asset": {"version": "2.0"}, "scenes": [{"nodes": [0]}], "nodes": [{}]}).encode().ljust(80, b" "))])
g = G()
basic(g)
d = g.finish()
buf = bytes(g.buf)
d["buffers"] = [{"byteLength": len(buf)}]
write_glb(os.path.join(OUT, "glb/invalid-first-json.glb"), d, buf, extra_chunks=[(0x4E4F534A, json.dumps({"asset": {"version": "1.0"}}).encode().ljust(40, b" "))])

# a glb with images and a sparse accessor and animation (everything in one binary)
g = G()
n = morph_scene(g, weights=(0.25, 0.5))
anim(g, "Move", n, "translation", [0, 1], [(0, 0, 0), (1, 1, 1)])
glb_with(g, "glb/animated-morph")

# =============================================================================================================
# G. decoders and formats X_ITE loads elsewhere
# =============================================================================================================

for ext, mime in (("KHR_texture_basisu", "image/ktx2"), ("EXT_texture_webp", "image/webp"), ("EXT_texture_avif", "image/avif")):
    for declared in (True, False):
        g = G()
        mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}})
        basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
        i0 = g.add("images", {"uri": data_uri(PNG_A, "image/png"), "name": "fallback"})
        i1 = g.add("images", {"uri": "tex." + mime.split("/")[1], "name": "modern", "mimeType": mime})
        g.add("textures", {"source": i0, "extensions": {ext: {"source": i1}}})
        if declared:
            g.d["extensionsUsed"] = [ext]
        g.write(f"codec/{ext.lower()}-{'declared' if declared else 'undeclared'}", "embed")

# EXT_texture_video
g = G()
mat = g.add("materials", {"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}})
basic(g, attrs={"TEXCOORD_0": g.acc(QUAD_UV, 5126, "VEC2")}, material=mat)
g.d["extensions"] = {"EXT_texture_video": {"videos": [{"uri": "clip.mp4", "name": "clip", "mimeType": "video/mp4"}]}}
i0 = g.add("images", {"uri": data_uri(PNG_A, "image/png")})
g.add("textures", {"source": i0, "extensions": {"EXT_texture_video": {"source": 0}}})
g.d["extensionsUsed"] = ["EXT_texture_video"]
g.write("codec/texture-video", "embed")

# Draco: a primitive that carries the extension with bytes that are not a Draco stream
g = G()
pos = g.acc(QUAD_P, 5126, "VEC3")
bad = g.view(b"DRACO\0\0\0notreallydraco")
m = g.add("meshes", {"primitives": [{"attributes": {"POSITION": pos}, "extensions": {"KHR_draco_mesh_compression": {"bufferView": bad, "attributes": {"POSITION": 0}}}}]})
n = g.add("nodes", {"mesh": m})
g.d["scenes"] = [{"nodes": [n]}]
g.d["extensionsUsed"] = ["KHR_draco_mesh_compression"]
g.d["extensionsRequired"] = ["KHR_draco_mesh_compression"]
g.write("codec/draco-garbage", "embed")

# =============================================================================================================
# H. invalid inputs
# =============================================================================================================

def raw(name, text):
    path = os.path.join(OUT, name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "w").write(text)

good = G()
basic(good)
gd = good.finish()
gd["buffers"] = [{"byteLength": len(good.buf), "uri": data_uri(bytes(good.buf))}]
GOOD = json.dumps(gd)

raw("invalid/bad-json-truncated.gltf", GOOD[:-20])
raw("invalid/not-json.gltf", "this is not json at all")
raw("invalid/empty.gltf", "")
raw("invalid/version-1.gltf", GOOD.replace('"version": "2.0"', '"version": "1.0"').replace('"version":"2.0"', '"version":"1.0"'))
raw("invalid/no-asset.gltf", json.dumps({k: v for k, v in gd.items() if k != "asset"}))
raw("invalid/unknown-top-level-key.gltf", json.dumps(dict(gd, bogus=1)))
raw("invalid/array-root.gltf", "[1, 2, 3]")
raw("invalid/missing-bin.gltf", json.dumps(dict(gd, buffers=[{"byteLength": 100, "uri": "does-not-exist.bin"}])))
raw("invalid/bad-base64.gltf", json.dumps(dict(gd, buffers=[{"byteLength": 100, "uri": "data:application/octet-stream;base64,@@@@"}])))
raw("invalid/buffer-uri-garbage.gltf", json.dumps(dict(gd, buffers=[{"byteLength": 100, "uri": "http://[bad"}])))

def mutate(label, fn):
    d = json.loads(GOOD)
    fn(d)
    raw(f"invalid/{label}.gltf", json.dumps(d))

mutate("accessor-count-too-big", lambda d: d["accessors"][0].update(count=100000))
mutate("accessor-offset-past-end", lambda d: d["accessors"][0].update(byteOffset=100000))
mutate("accessor-offset-unaligned", lambda d: d["accessors"][0].update(byteOffset=2))
mutate("accessor-no-bufferview", lambda d: d["accessors"][0].pop("bufferView"))
mutate("accessor-bufferview-missing", lambda d: d["accessors"][0].update(bufferView=9))
mutate("accessor-bad-component-type", lambda d: d["accessors"][0].update(componentType=1234))
mutate("accessor-bad-type", lambda d: d["accessors"][0].update(type="VEC9"))
mutate("accessor-negative-count", lambda d: d["accessors"][0].update(count=-4))
mutate("accessor-string-count", lambda d: d["accessors"][0].update(count="4"))
mutate("bufferview-buffer-missing", lambda d: d["bufferViews"][0].update(buffer=4))
mutate("bufferview-too-long", lambda d: d["bufferViews"][0].update(byteLength=100000))
mutate("bufferview-offset-past-end", lambda d: d["bufferViews"][0].update(byteOffset=100000))
mutate("indices-accessor-missing", lambda d: d["meshes"][0]["primitives"][0].update(indices=7))
mutate("position-accessor-missing", lambda d: d["meshes"][0]["primitives"][0]["attributes"].update(POSITION=7))
mutate("mesh-primitives-not-array", lambda d: d["meshes"][0].update(primitives={}))
mutate("mesh-index-missing", lambda d: d["nodes"][0].update(mesh=5))
mutate("node-null", lambda d: d["nodes"].append(None))
mutate("node-null-in-scene", lambda d: (d["nodes"].append(None), d["scenes"][0]["nodes"].append(1)))
mutate("scene-nodes-bad-index", lambda d: d["scenes"][0].update(nodes=[0, 9, "x", None]))
mutate("texture-without-images", lambda d: (d.update(materials=[{"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}}], textures=[{"source": 0}]), d["meshes"][0]["primitives"][0].update(material=0)))
mutate("material-texture-without-textures", lambda d: (d.update(materials=[{"pbrMetallicRoughness": {"baseColorTexture": {"index": 0}}}]), d["meshes"][0]["primitives"][0].update(material=0)))
mutate("sparse-bad-indices-type", lambda d: d["accessors"][0].update(sparse={"count": 1, "indices": {"bufferView": 0, "componentType": 5126}, "values": {"bufferView": 0}}))
mutate("sparse-bufferview-missing", lambda d: d["accessors"][0].update(sparse={"count": 1, "indices": {"bufferView": 9, "componentType": 5123}, "values": {"bufferView": 0}}))
mutate("animation-bad-sampler-accessor", lambda d: d.update(animations=[{"samplers": [{"input": 9, "output": 9}], "channels": [{"sampler": 0, "target": {"node": 0, "path": "translation"}}]}]))
mutate("animation-sampler-null", lambda d: d.update(animations=[{"samplers": [None], "channels": []}]))
mutate("animation-out-of-range-output", lambda d: d.update(animations=[{"samplers": [{"input": 1, "output": 0}], "channels": [{"sampler": 0, "target": {"node": 0, "path": "translation"}}]}]))
mutate("skin-without-nodes", lambda d: d.update(skins=[{"joints": [0]}]))
mutate("skin-null-node", lambda d: (d.update(skins=[{"joints": [0]}]), d["nodes"].append(None)))
mutate("camera-null", lambda d: (d.update(cameras=[None]), d["nodes"][0].update(camera=0)))
mutate("light-index-bad", lambda d: (d.update(extensions={"KHR_lights_punctual": {"lights": []}}), d["nodes"][0].update(extensions={"KHR_lights_punctual": {"light": 4}})))
mutate("xmp-packets-missing", lambda d: (d["asset"].update(extensions={"KHR_xmp_json_ld": {"packet": 0}}), d.update(extensions={"KHR_xmp_json_ld": {}})))
mutate("meshopt-undeclared-view", lambda d: d["bufferViews"][0].update(extensions={"KHR_meshopt_compression": {"buffer": 0, "byteLength": 4, "byteStride": 12, "count": 1, "mode": "ATTRIBUTES"}}))

# invalid GLB
def glb_raw(name, data):
    path = os.path.join(OUT, name)
    os.makedirs(os.path.dirname(path), exist_ok=True)
    open(path, "wb").write(data)

g = G()
basic(g)
d = g.finish()
buf = bytes(g.buf)
d["buffers"] = [{"byteLength": len(buf)}]
write_glb(os.path.join(OUT, "glb/ok.glb"), d, buf)
ok = open(os.path.join(OUT, "glb/ok.glb"), "rb").read()
os.remove(os.path.join(OUT, "glb/ok.glb"))
glb_raw("invalid/glb-bad-magic.glb", b"XXXX" + ok[4:])
glb_raw("invalid/glb-version-1.glb", ok[:4] + struct.pack("<I", 1) + ok[8:])
glb_raw("invalid/glb-length-mismatch.glb", ok[:8] + struct.pack("<I", len(ok) + 4) + ok[12:])
glb_raw("invalid/glb-truncated.glb", ok[:len(ok) - 7])
glb_raw("invalid/glb-header-only.glb", ok[:12][:8] + struct.pack("<I", 12))
glb_raw("invalid/glb-tiny.glb", b"glTF")
def fixed(blob):
    return blob[:8] + struct.pack("<I", len(blob)) + blob[12:]
glb_raw("invalid/glb-chunk-too-long.glb", fixed(ok[:12] + struct.pack("<II", 0x7FFFFFF0, 0x4E4F534A) + b"{}"))
glb_raw("invalid/glb-trailing-bytes.glb", fixed(ok + b"\x01\x02\x03"))
glb_raw("invalid/glb-no-json-chunk.glb", fixed(ok[:12] + struct.pack("<II", 4, 0x004E4942) + b"\0\0\0\0"))
glb_raw("invalid/glb-json-garbage.glb", fixed(ok[:12] + struct.pack("<II", 8, 0x4E4F534A) + b"not json"))
glb_raw("invalid/glb-empty-json.glb", fixed(ok[:12] + struct.pack("<II", 4, 0x4E4F534A) + b"{}  "))
glb_raw("invalid/glb-bin-missing-buffer-uri.glb", fixed(ok[:12] + struct.pack("<II", len(json.dumps(d)) + (4 - len(json.dumps(d)) % 4) % 4, 0x4E4F534A) + json.dumps(d).encode().ljust(len(json.dumps(d)) + (4 - len(json.dumps(d)) % 4) % 4, b" ")))

# =============================================================================================================
# I. render subset (compared against X_ITE renders at MSAA 0 and 4)
# =============================================================================================================

def sphere(seg=24, rings=14, r=1.0):
    pos, nrm, uv, idx = [], [], [], []
    for j in range(rings + 1):
        v = j / rings
        th = v * math.pi
        for i in range(seg + 1):
            u = i / seg
            ph = u * 2 * math.pi
            x, y, z = math.sin(th) * math.cos(ph), math.cos(th), math.sin(th) * math.sin(ph)
            pos.append((x * r, y * r, z * r))
            nrm.append((x, y, z))
            uv.append((u, v))
    for j in range(rings):
        for i in range(seg):
            a = j * (seg + 1) + i
            b = a + seg + 1
            idx += [a, a + 1, b, a + 1, b + 1, b]
    return pos, nrm, uv, idx


def render_scene(g, material=None, extra_attrs=None, tex=False, cam=True, node_extra=None, camz=4.5, shape="sphere"):
    if shape == "sphere":
        pos, nrm, uv, idx = sphere()
    else:
        pos, nrm, uv, idx = QUAD_P, QUAD_N, QUAD_UV, QUAD_I
    a = {"POSITION": g.acc(pos, 5126, "VEC3", minmax=True), "NORMAL": g.acc(nrm, 5126, "VEC3")}
    if tex:
        a["TEXCOORD_0"] = g.acc(uv, 5126, "VEC2")
    if extra_attrs:
        a.update(extra_attrs(len(pos)))
    prim = {"attributes": a, "indices": g.acc(idx, 5125, "SCALAR")}
    if material is not None:
        prim["material"] = material
    m = g.add("meshes", {"primitives": [prim]})
    n = g.add("nodes", dict({"mesh": m}, **(node_extra or {})))
    nodes = [n]
    if cam:
        c = g.add("cameras", {"type": "perspective", "perspective": {"yfov": 0.8, "znear": 0.1, "zfar": 50}})
        nodes.append(g.add("nodes", {"camera": c, "translation": [0, 0, camz]}))
    g.d["scenes"] = [{"nodes": nodes}]
    return n


def light_nodes(g, lights):
    g.d["extensionsUsed"] = g.d.get("extensionsUsed", []) + ["KHR_lights_punctual"]
    g.d["extensions"] = {"KHR_lights_punctual": {"lights": lights}}
    out = []
    for i, (t, rot, pos) in enumerate(lights_pos(lights)):
        out.append(g.add("nodes", {"extensions": {"KHR_lights_punctual": {"light": i}}, "rotation": rot, "translation": pos}))
    g.d["scenes"][0]["nodes"] += out


def lights_pos(lights):
    for l in lights:
        yield l["type"], [0.0, 0.0, 0.0, 1.0], [2.0, 2.0, 3.0]


for label, mat in [
    ("pbr-metal", {"pbrMetallicRoughness": {"baseColorFactor": [0.9, 0.6, 0.2, 1], "metallicFactor": 1.0, "roughnessFactor": 0.35}}),
    ("pbr-rough", {"pbrMetallicRoughness": {"baseColorFactor": [0.2, 0.5, 0.9, 1], "metallicFactor": 0.0, "roughnessFactor": 0.9}}),
    ("emissive", {"pbrMetallicRoughness": {"baseColorFactor": [0.1, 0.1, 0.1, 1]}, "emissiveFactor": [0.9, 0.3, 0.1]}),
    ("blend", {"alphaMode": "BLEND", "pbrMetallicRoughness": {"baseColorFactor": [0.9, 0.2, 0.3, 0.5]}}),
    ("unlit", {"pbrMetallicRoughness": {"baseColorFactor": [0.3, 0.8, 0.4, 1]}, "extensions": {"KHR_materials_unlit": {}}}),
    ("clearcoat", {"pbrMetallicRoughness": {"baseColorFactor": [0.7, 0.1, 0.1, 1], "metallicFactor": 0.0, "roughnessFactor": 0.6}, "extensions": {"KHR_materials_clearcoat": {"clearcoatFactor": 1.0, "clearcoatRoughnessFactor": 0.05}}}),
]:
    g = G()
    mid = g.add("materials", mat)
    render_scene(g, material=mid)
    ex = [k for k in mat.get("extensions", {})]
    if ex:
        g.d["extensionsUsed"] = ex
    light_nodes(g, [{"type": "directional", "intensity": 3.0, "color": [1, 1, 1]}])
    g.d["scenes"][0]["nodes"]  # lights rotated below
    g.write(f"render/mat-{label}", "embed")

# directional light rotated toward the sphere
g = G()
mid = g.add("materials", {"pbrMetallicRoughness": {"baseColorFactor": [0.8, 0.8, 0.8, 1], "metallicFactor": 0.0, "roughnessFactor": 0.5}})
render_scene(g, material=mid)
g.d["extensionsUsed"] = ["KHR_lights_punctual"]
g.d["extensions"] = {"KHR_lights_punctual": {"lights": [{"type": "directional", "intensity": 3.0, "color": [1, 0.9, 0.8]}, {"type": "point", "intensity": 30.0, "color": [0.3, 0.5, 1.0]}]}}
g.d["scenes"][0]["nodes"] += [g.add("nodes", {"rotation": [-0.3, 0.4, 0.1, 0.86], "extensions": {"KHR_lights_punctual": {"light": 0}}}),
                              g.add("nodes", {"translation": [-2, 1, 2], "extensions": {"KHR_lights_punctual": {"light": 1}}})]
g.write("render/lights-directional-point", "embed")

# textured sphere, sampler variants and texture transform
for label, smp, tt in [("nearest", {"magFilter": 9728, "minFilter": 9728}, None), ("linear", {"magFilter": 9729, "minFilter": 9729, "wrapS": 33071, "wrapT": 33071}, None), ("mirror", {"wrapS": 33648, "wrapT": 33648}, None), ("transform", {}, {"offset": [0.25, 0.1], "rotation": 0.4, "scale": [2, 2]})]:
    g = G()
    ti = {"index": 0}
    if tt:
        ti["extensions"] = {"KHR_texture_transform": tt}
        g.d["extensionsUsed"] = ["KHR_texture_transform"]
    mid = g.add("materials", {"pbrMetallicRoughness": {"baseColorTexture": ti, "metallicFactor": 0.0, "roughnessFactor": 0.8}})
    render_scene(g, material=mid, tex=True)
    g.add("samplers", smp)
    image_texture(g, uri=data_uri(PNG_A, "image/png"), sampler=0)
    light_nodes(g, [{"type": "directional", "intensity": 2.5}])
    g.write(f"render/tex-{label}", "embed")

# vertex colors
g = G()
mid = g.add("materials", {"pbrMetallicRoughness": {"metallicFactor": 0.0, "roughnessFactor": 0.7}})
render_scene(g, material=mid, extra_attrs=lambda n: {"COLOR_0": g.acc([(math.sin(i * 0.37) * 0.5 + 0.5, math.cos(i * 0.21) * 0.5 + 0.5, (i % 7) / 7.0) for i in range(n)], 5126, "VEC3")})
light_nodes(g, [{"type": "directional", "intensity": 2.5}])
g.write("render/vertex-colors", "embed")

# double sided quad, rotated
g = G()
mid = g.add("materials", {"doubleSided": True, "pbrMetallicRoughness": {"baseColorFactor": [0.9, 0.7, 0.1, 1]}})
render_scene(g, material=mid, shape="quad", node_extra={"rotation": [0, 0.8, 0, 0.6]})
light_nodes(g, [{"type": "directional", "intensity": 2.0}])
g.write("render/double-sided-quad", "embed")

# skinned and morphed, static pose
g = G()
j0, j1, mn = skin_scene(g)
c = g.add("cameras", {"type": "perspective", "perspective": {"yfov": 0.8}})
g.d["scenes"][0]["nodes"].append(g.add("nodes", {"camera": c, "translation": [0, 1, 6]}))
g.d["nodes"][j1]["rotation"] = [0, 0, 0.3826834, 0.9238795]
g.write("render/skin-bent", "embed")
g = G()
n = morph_scene(g, weights=(0.6, 0.4))
c = g.add("cameras", {"type": "perspective", "perspective": {"yfov": 0.8}})
g.d["scenes"][0]["nodes"].append(g.add("nodes", {"camera": c, "translation": [0, 0, 5]}))
g.write("render/morph-static", "embed")

# stepped animation: one rotation channel, STEP interpolation, cycles on a 4 second interval
g = G()
mid = g.add("materials", {"pbrMetallicRoughness": {"baseColorFactor": [0.2, 0.7, 0.9, 1]}})
n = render_scene(g, material=mid, shape="quad", node_extra={"name": "Spinner"})
anim(g, "Step", n, "rotation", [0, 1, 2, 3], [(0, 0, 0, 1), (0, 0.38268343, 0, 0.92387953), (0, 0.7071068, 0, 0.7071068), (0, 0.92387953, 0, 0.38268343)], "STEP", vt="VEC4")
g.write("render/animation-step", "embed")

# =============================================================================================================
# J. runtime: a VRML wrapper that IMPORTs the exported Timer of a glTF Inline and starts it, and
#    wrappers that load glTF / GLB through Inline (load, reload, unload)
# =============================================================================================================

os.makedirs(os.path.join(OUT, "anim"), exist_ok=True)
# A click on the TouchSensor (a transparent box behind the glTF, away from the picture) starts the
# glTF's exported Timer1; a looping run is started by the same click through set_loop.
for label, gltf, loop in [("step", "../render/animation-step.gltf", "FALSE"), ("linear", "../animation/rotation-linear.gltf", "TRUE"), ("cubic", "../animation/translation-cubicspline.gltf", "FALSE")]:
    open(os.path.join(OUT, f"anim/{label}-driver.wrl"), "w").write("""#VRML V2.0 utf8
NavigationInfo { type "NONE" }
DEF Gltf Inline { url "%s" }
IMPORT Gltf.Timer1 AS Timer1
Transform {
  translation 0 0 -3
  children [
    DEF Touch TouchSensor { }
    Shape { appearance Appearance { material Material { transparency 0.9 } } geometry Box { size 100 100 0.1 } }
  ]
}
DEF Loop Script { eventIn SFTime touched eventOut SFBool loop_ url "ecmascript:
function touched (v) { loop_ = %s; }
" }
ROUTE Touch.touchTime TO Loop.touched
ROUTE Loop.loop_ TO Timer1.set_loop
ROUTE Touch.touchTime TO Timer1.set_startTime
""" % (gltf, loop.lower()))

# Inline load / switch / unload driven by keys: 'u' unloads (load FALSE), 'l' loads again, 'g' switches the
# url to a GLB, 'h' back to the glTF, 'm' sets an MFString with a missing first candidate (fallback).
os.makedirs(os.path.join(OUT, "inline"), exist_ok=True)
open(os.path.join(OUT, "inline/toggle.wrl"), "w").write("""#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
Viewpoint { position 0 0 5 }
Background { skyColor 0.15 0.15 0.18 }
DirectionalLight { direction -0.4 -0.6 -0.7 intensity 0.9 }
DEF Inl Inline { url "../render/tex-nearest.gltf" }
DEF K KeySensor { }
DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  initializeOnly SFNode inl USE Inl
  url "javascript:
function on_key (k)
{
   if (k == 'u') inl .load = false;
   if (k == 'l') inl .load = true;
   if (k == 'g') inl .url = new MFString ('../glb/textured.glb');
   if (k == 'h') inl .url = new MFString ('../render/tex-nearest.gltf');
   if (k == 'm') inl .url = new MFString ('../nothing/missing.gltf', '../render/mat-pbr-metal.gltf');
   if (k == 'n') inl .url = new MFString ('../invalid/version-1.gltf');
}
"
}
ROUTE K.keyPress TO R.on_key
""")

# Inline scenarios (oracle/x_ite-inline.mjs): the glTF files of OUT are named relative to the scenario dir.
INL = os.path.join(os.path.dirname(OUT.rstrip("/")), "synthetic-rea-b08-gltf-inline")
os.makedirs(INL, exist_ok=True)
G_REL = "../synthetic-rea-b08-gltf/"
open(os.path.join(INL, "main.wrl"), "w").write("""#VRML V2.0 utf8
# Synthetic, newly authored for REA batch 08 (glTF lane): one Inline inside a Group, and a parent clock whose
# routes must survive every reload of the Inline.
DEF Holder Group { children [ DEF Inl Inline { url "%srender/tex-nearest.gltf" } ] }
DEF ParentClock TimeSensor { cycleInterval 0.5 loop TRUE }
DEF ParentScript Script {
  eventIn SFTime tick
  field SFInt32 n 0
  url "javascript:
    function tick (t) { n = n + 1; print ('parent tick ' + n); }
  "
}
ROUTE ParentClock.cycleTime TO ParentScript.tick
""" % G_REL)

def scenario(name, actions, frames=14, covers="glTF/GLB through Inline"):
    open(os.path.join(INL, name + ".json"), "w").write(json.dumps({"world": "main.wrl", "authority": "XITE_COMPATIBILITY", "covers": [covers], "dt": 0.1, "frames": frames, "actions": actions}))

def url(frame, *u):
    return {"frame": frame, "type": "url", "name": "Inl", "url": [G_REL + x if not x.startswith("missing") else x for x in u]}

scenario("q01-initial", [])
scenario("q02-url-glb", [url(4, "glb/textured.glb"), url(8, "buffer/external-bin.gltf"), url(11, "render/tex-nearest.gltf")])
scenario("q03-load-toggle", [{"frame": 4, "type": "load", "name": "Inl", "value": False}, {"frame": 7, "type": "load", "name": "Inl", "value": True}, {"frame": 9, "type": "load", "name": "Inl", "value": False}, {"frame": 11, "type": "load", "name": "Inl", "value": True}])
scenario("q04-fallback", [url(4, "missing/gone.gltf", "invalid/version-1.gltf", "render/mat-pbr-metal.gltf")])
scenario("q05-reload", [{"frame": f, "type": t, "name": "Inl"} for f, t in [(3, "loadNow"), (5, "unloadNow"), (7, "loadNow"), (9, "loadNow"), (11, "unloadNow"), (12, "loadNow")]])
scenario("q06-remove", [{"frame": 5, "type": "remove", "name": "Inl", "group": "Holder"}])
scenario("q07-material-variants", [url(4, "variants/basic.gltf")])
scenario("q08-skin-animation", [url(4, "animation/skin-joint.gltf")])
for label, rel in [("version-1", "invalid/version-1.gltf"), ("not-json", "invalid/not-json.gltf"), ("missing-bin", "invalid/missing-bin.gltf"), ("bad-accessor", "invalid/accessor-count-too-big.gltf"),
                   ("node-null", "invalid/node-null.gltf"), ("glb-truncated", "invalid/glb-truncated.glb"), ("glb-trailing", "invalid/glb-trailing-bytes.glb"), ("bad-ref-view", "invalid/bufferview-buffer-missing.gltf"),
                   ("bad-base64", "invalid/bad-base64.gltf"), ("tt-pointer", "pointer/metallic.gltf")]:
    scenario("r-" + label, [url(4, rel), url(8, "render/tex-nearest.gltf")], frames=12, covers="invalid input")

print("done")
