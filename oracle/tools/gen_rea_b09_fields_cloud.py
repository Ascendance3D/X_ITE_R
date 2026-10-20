#!/usr/bin/env python3
"""Generate the field API survey worlds oracle/synthetic-rea-b09-sai/fldx-*.wrl (REA batch 09 cloud, sub:fields.types).
Authored; synthetic inputs only. Uses the probe harness of gen_rea_b09_script.py (`T (label, fn)` into the MFString `out`).

The worlds measure what the pinned X_ITE exposes on every field class a Script sees: the own properties of the constructor and of
its prototype chain, a call without `new`, the static constructors (fromValue, fromString, ...), X3DField.fromString and its
siblings on detached and on node fields, and getValue () of the SF classes (the internal Vector2/3/4, Rotation4, Quaternion,
Matrix3/4 and Color3/4 objects).

  python3 -I oracle/tools/gen_rea_b09_fields_cloud.py
"""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_rea_b09_script as G  # noqa: E402

SF = ["SFBool", "SFColor", "SFColorRGBA", "SFDouble", "SFFloat", "SFImage", "SFInt32", "SFMatrix3d", "SFMatrix3f", "SFMatrix4d",
      "SFMatrix4f", "SFNode", "SFQuaternion", "SFRotation", "SFString", "SFTime", "SFVec2d", "SFVec2f", "SFVec3d", "SFVec3f",
      "SFVec4d", "SFVec4f", "VrmlMatrix"]
MF = ["MFBool", "MFColor", "MFColorRGBA", "MFDouble", "MFFloat", "MFImage", "MFInt32", "MFMatrix3d", "MFMatrix3f", "MFMatrix4d",
      "MFMatrix4f", "MFNode", "MFQuaternion", "MFRotation", "MFString", "MFTime", "MFVec2d", "MFVec2f", "MFVec3d", "MFVec3f",
      "MFVec4d", "MFVec4f"]
ALL = SF + MF

# Own property names of an object and of each prototype up to (not including) Object.prototype, sorted per level.
CHAIN = ("function chain (o) { var r = []; var seen = 0; while (o && o !== Object .prototype && o !== Function .prototype && seen ++ < 8) "
         "{ r .push (Object .getOwnPropertyNames (o) .concat (Object .getOwnPropertySymbols (o) .map (String)) .sort () .join (',')); "
         "o = Object .getPrototypeOf (o); } return r; }")

P_API = []
for t in ALL:
    P_API.append((f"{t} statics", f"return Object .getOwnPropertyNames ({t}) .sort ();"))
    P_API.append((f"{t} static values", f"return [typeof {t} .type, {t} .type, {t} .typeName, typeof {t} .fromValue, typeof {t} .fromString, typeof {t} .fromVRMLString, typeof {t} .fromXMLString, typeof {t} .fromJSONString, {t} .length, {t} .name];"))
    if t != "SFNode":
        P_API.append((f"{t} proto chain", f"return chain (Object .getPrototypeOf (new {t} ()));"))
P_API.append(("SFNode proto chain", "return chain (Object .getPrototypeOf (new SFNode ('Group { }')));"))
P_API.append(("X3DField proto", "return Object .getOwnPropertyNames (X3DField .prototype) .sort ();"))
P_API.append(("X3DArrayField proto", "return Object .getOwnPropertyNames (X3DArrayField .prototype) .sort ();"))
P_API.append(("X3DField statics", "return Object .getOwnPropertyNames (X3DField) .sort ();"))
P_API.append(("X3DArrayField statics", "return Object .getOwnPropertyNames (X3DArrayField) .sort ();"))

P_CALL = []
for t in ALL:
    arg = "'Group { }'" if t == "SFNode" else ""
    P_CALL.append((f"{t} call without new", f"var r = {t} ({arg}); return [typeof r, r === undefined];"))

SAMPLE = {
    "SFBool": ["TRUE", "FALSE", "true", "1", "x"], "SFDouble": ["1.5", "-2e3", "x", ""], "SFFloat": ["0.1", "1e40", "x"],
    "SFInt32": ["12", "0x1F", "1.5", "x"], "SFString": ['"abc"', "abc", '"a\\"b"', ""], "SFTime": ["123.5", "x"],
    "SFColor": ["1 0 0", "0.1 0.2", "1 0 0 1", "x"], "SFColorRGBA": ["1 0 0 0.5", "1 0 0"],
    "SFVec2f": ["1 2", "1", "1 2 3", "1,2", "x"], "SFVec2d": ["1 2"], "SFVec3f": ["1 2 3", "1 2", "1e3 -2 .5", "x"], "SFVec3d": ["1 2 3"],
    "SFVec4f": ["1 2 3 4"], "SFVec4d": ["1 2 3 4"], "SFRotation": ["0 1 0 1.5", "0 0 0 0", "1 2"], "SFQuaternion": ["0 0 0 1"],
    "SFMatrix3f": ["1 0 0 0 1 0 0 0 1", "2 0 0 0 2 0 0 0 1", "1 2"], "SFMatrix3d": ["1 0 0 0 1 0 0 0 1"],
    "SFMatrix4f": ["1 0 0 0 0 1 0 0 0 0 1 0 1 2 3 1"], "SFMatrix4d": ["1 0 0 0 0 1 0 0 0 0 1 0 1 2 3 1"],
    "SFImage": ["1 1 1 0xFF", "2 1 3 0xFF0000 0x00FF00", "1 1 1", "0 0 0"], "SFNode": ["NULL", "Group { }", "Transform { translation 1 2 3 }", "x"],
    "MFBool": ["[TRUE FALSE]", "TRUE", "[]"], "MFInt32": ["[1 2 3]", "[1, 2, 3]", "4"], "MFFloat": ["[1 2.5]", "[]"],
    "MFDouble": ["[1 2.5]"], "MFTime": ["[1 2]"], "MFString": ['["a" "b"]', '"one"', "[]", "[a]"],
    "MFVec2f": ["[1 2, 3 4]", "[1 2 3]"], "MFVec2d": ["[1 2]"], "MFVec3f": ["[1 2 3, 4 5 6]", "[1 2 3 4]", "1 2 3"], "MFVec3d": ["[1 2 3]"],
    "MFVec4f": ["[1 2 3 4]"], "MFVec4d": ["[1 2 3 4]"], "MFColor": ["[1 0 0, 0 1 0]"], "MFColorRGBA": ["[1 0 0 1]"],
    "MFRotation": ["[0 1 0 1]"], "MFQuaternion": ["[0 0 0 1]"], "MFMatrix3f": ["[1 0 0 0 1 0 0 0 1]"], "MFMatrix3d": ["[1 0 0 0 1 0 0 0 1]"],
    "MFMatrix4f": ["[1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1]"], "MFMatrix4d": ["[1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1]"], "MFImage": ["[1 1 1 0xFF]"],
    "MFNode": ["[Group { } Transform { }]", "[]", "Group { }"],
}

P_FROM = []
for t, samples in SAMPLE.items():
    for i, s in enumerate(samples):
        js = repr(s)
        mk = f"new {t} ('Group {{ }}')" if t == "SFNode" else f"new {t} ()"
        P_FROM.append((f"{t} fromString {i}", f"var f = {mk}; var r = f .fromString ({js}); return [r, f];"))
        P_FROM.append((f"{t} static fromString {i}", f"return {t} .fromString ({js});"))
    P_FROM.append((f"{t} fromVRMLString", f"var f = {t} .fromVRMLString ({repr(samples[0])}); return f;"))
    P_FROM.append((f"{t} fromJSONString", f"var f = {t} .fromJSONString ({repr(samples[0])}); return f;"))
    P_FROM.append((f"{t} fromValue", f"var f = {t} .fromValue ? 1 : 0; return f;"))
for t, s in [("SFVec3f", "1 2 3"), ("SFBool", "true"), ("SFString", "abc"), ("MFInt32", "1 2 3"), ("MFString", '"a" "b"'), ("SFColor", "1 0 0"),
             ("SFRotation", "0 1 0 1"), ("MFVec3f", "1 2 3 4 5 6"), ("SFNode", "<Group/>")]:
    mk = f"new {t} ('Group {{ }}')" if t == "SFNode" else f"new {t} ()"
    P_FROM.append((f"{t} fromXMLString", f"var f = {mk}; f .fromXMLString ({s!r}); return f;"))
    P_FROM.append((f"{t} static fromXMLString", f"return {t} .fromXMLString ({s!r});"))
P_FROM.append(("fromString scene arg", "var f = new SFVec3f (); f .fromString ('1 2 3', Browser .currentScene); return f;"))
P_FROM.append(("fromString node field", "var t = Browser .currentScene .getNamedNode ('T'); var e = []; t .addFieldCallback ('k', 'translation', function (v) { e .push (String (v)); }); t .translation .fromString ('4 5 6'); return [t .translation, e .length];"))
P_FROM.append(("fromString SFNode scene names", "var f = SFNode .fromString ('DEF FROMSTR Group { }', Browser .currentScene); return [f .getNodeName (), Browser .currentScene .getNamedNodes ? 1 : 0];"))
P_FROM.append(("fromString error name", "var t = Browser .currentScene .getNamedNode ('T'); try { t .translation .fromString ('x'); return 'no error'; } catch (e) { return e .name + ': ' + e .message; }"))

P_VALUE = []
for t in ["SFVec2f", "SFVec2d", "SFVec3f", "SFVec3d", "SFVec4f", "SFVec4d", "SFColor", "SFColorRGBA", "SFRotation", "SFQuaternion",
          "SFMatrix3f", "SFMatrix3d", "SFMatrix4f", "SFMatrix4d", "VrmlMatrix", "SFImage", "SFBool", "SFDouble", "SFFloat", "SFInt32",
          "SFString", "SFTime"]:
    P_VALUE.append((f"{t} getValue type", f"var f = new {t} (); var g = f .getValue (); return [typeof g, g === f, g === f .getValue (), g && g .constructor && g .constructor .name, g && Object .getPrototypeOf (g) === Object .getPrototypeOf (f)];"))
    P_VALUE.append((f"{t} getValue chain", f"var g = new {t} () .getValue (); return g !== null && typeof g === 'object' ? chain (g) : typeof g;"))
for t, n in [("Vector2", "SFVec2f (1, 2)"), ("Vector3", "SFVec3f (1, 2, 3)"), ("Vector4", "SFVec4f (1, 2, 3, 4)")]:
    P_VALUE.append((f"{t} statics", f"var C = new {n} .getValue () .constructor; return Object .getOwnPropertyNames (C) .sort ();"))
    P_VALUE.append((f"{t} static values", f"var C = new {n} .getValue () .constructor; return Object .getOwnPropertyNames (C) .sort () .map (function (k) {{ var v = C [k]; return k + '=' + (typeof v === 'object' && v ? String (v) : typeof v); }});"))
    P_VALUE.append((f"{t} own keys", f"var g = new {n} .getValue (); return [Object .keys (g), JSON .stringify (g), String (g), g .length, Array .from (g), g [0], g .x];"))
    P_VALUE.append((f"{t} live", f"var f = new {n}; var g = f .getValue (); g .x = 9; return [f .x, String (f)];"))

# MF lengths that are not array lengths (crates/x3d-script/tests/script.rs). A length or index near 2^32 is left out: the pinned X_ITE
# allocates the typed array and the page runs out of memory (the port throws a RangeError instead, a kept safety difference).
P_EDGE = [
    ("length -1", "var a = new MFInt32 (1, 2, 3); try { a .length = -1; return ['no error', a .length, String (a)]; } catch (e) { return [e .name, a .length, String (a)]; }"),
    ("length 1.5", "var a = new MFInt32 (1, 2, 3); try { a .length = 1.5; return ['no error', a .length, String (a), Array .from (a)]; } catch (e) { return [e .name, a .length, String (a)]; }"),
    ("length 1.5 then push", "var a = new MFInt32 (1, 2, 3); a .length = 1.5; try { a .push (7); return [a .length, String (a)]; } catch (e) { return [e .name]; }"),
    ("vec length 2.5", "var v = new MFVec3f (new SFVec3f (1, 2, 3)); try { v .length = 2.5; return ['no error', v .length, String (v)]; } catch (e) { return [e .name, v .length]; }"),
    ("string length 1.5", "var a = new MFString ('a', 'b', 'c'); try { a .length = 1.5; return ['no error', a .length, String (a)]; } catch (e) { return [e .name, a .length]; }"),
]

HEAD_NODES = "DEF T Transform { }\n"
G.register("fldx-api", P_API, pre=CHAIN, nodes=HEAD_NODES)
G.register("fldx-call", P_CALL, pre=CHAIN, nodes=HEAD_NODES)
G.register("fldx-from", P_FROM, pre=CHAIN, nodes=HEAD_NODES)
G.register("fldx-value", P_VALUE, pre=CHAIN, nodes=HEAD_NODES)
G.register("fldx-edge", P_EDGE, pre=CHAIN, nodes=HEAD_NODES)


# Values that are not numbers in a numeric field component (owner decision 2026-10-20: X_ITE's representation, compared): each value
# goes in through a path, at frame 3; at frame 6 the Script reads the field back, and the scene text (dumped every frame) shows what
# the node holds and what reached the routed destination.
VALUES = [("num", "2.5"), ("numstr", "'2'"), ("str", "'abc'"), ("empty", "''"), ("null", "null"), ("undef", "undefined"),
          ("nan", "NaN"), ("true", "true"), ("obj", "({ })"), ("expstr", "'1e3'")]
PATHS = [
    # SFVec3f: the internal Vector3 (getValue), then an event so the value is sent on.
    ("vec-internal", "Transform", "translation", "var f = N .translation; f .getValue () .x = V; N .translation = f;"),
    # SFVec3f: the component setter.
    ("vec-setter", "Transform", "translation", "var f = N .translation; f .x = V; N .translation = f;"),
    # SFVec3f: the constructor.
    ("vec-new", "Transform", "translation", "N .translation = new SFVec3f (V, 1, 1);"),
    # SFColor: the internal Color3 (getValue).
    ("color-internal", "Material", "diffuseColor", "var f = N .diffuseColor; f .getValue () .r = V; N .diffuseColor = f;"),
    # SFRotation: the internal Rotation4 axis component through its setter.
    ("rot-setter", "Transform", "rotation", "var f = N .rotation; f .angle = V; N .rotation = f;"),
    # MFFloat element (typed array), MFVec3f element's internal value.
    ("mffloat-index", "ScalarInterpolator", "key", "var f = new MFFloat (0, 1); f [1] = V; N .key = f;"),
    ("mfvec-internal", "Coordinate", "point", "var f = new MFVec3f (new SFVec3f (1, 1, 1)); f [0] .getValue () .y = V; N .point = f;"),
    # SFFloat node field.
    ("sffloat", "Material", "transparency", "N .transparency = V;"),
]
FI_NODES, FI_SET, FI_READ = [], [], []
for pk, ptype, pfield, code in PATHS:
    for vk, v in VALUES:
        a, b = f"A_{pk}_{vk}".replace("-", "_"), f"B_{pk}_{vk}".replace("-", "_")
        FI_NODES.append(f"DEF {a} {ptype} {{ }}\nDEF {b} {ptype} {{ }}\nROUTE {a}.{pfield}_changed TO {b}.set_{pfield}\n")
        FI_SET.append(f"  T ('{pk} {vk} set', function () {{ var N = Browser .currentScene .getNamedNode ('{a}'); var V = {v}; {code} return 'ok'; }});")
        FI_READ.append(f"  T ('{pk} {vk} read', function () {{ var N = Browser .currentScene .getNamedNode ('{a}'), M = Browser .currentScene .getNamedNode ('{b}'); var f = N .{pfield}, g = M .{pfield}; "
                       f"var c = f .getValue ? f .getValue () : f; var first = typeof f === 'object' && f .length !== undefined && f [Symbol .iterator] ? f [f .length - 1] : f; "
                       f"return [String (f), String (g), f .toVRMLString ? f .toVRMLString () : String (f), JSON .stringify (Array .from (typeof first === 'object' && first && first [Symbol .iterator] ? first : [first]) .map (function (x) {{ return typeof x + ':' + String (x); }}))]; }});")
FI_AFTER = ("var F_ = 0;\nfunction prepareEvents () {\n  ++ F_;\n  if (F_ === 3) {\n" + "\n".join(FI_SET) + "\n  }\n  if (F_ === 6) {\n" + "\n".join(FI_READ) + "\n    flush_ ();\n  }\n}\n")
# A Script SFFloat field and a Script SFVec3f field.
FI_FIELDS = "  inputOutput SFFloat sf 1\n  inputOutput SFVec3f sv 1 1 1\n"
FI_SCRIPT = [("script sffloat " + vk, f"sf = {v}; return [typeof sf, String (sf)];") for vk, v in VALUES] + \
            [("script sfvec internal " + vk, f"sv = new SFVec3f (1, 1, 1); sv .getValue () .z = {v}; return [typeof sv .z, String (sv .z), String (sv)];") for vk, v in VALUES]
G.register("fldx-internal", FI_SCRIPT, fields=FI_FIELDS, pre=CHAIN, after=FI_AFTER, nodes="".join(FI_NODES))

# Raw components in the scene's VRML, XML and JSON text (checkpoint after b2e5b9f): node fields with a unit category (Transform
# translation, length) and without one (SFVec2f, SFVec4f, SFColor, SFColorRGBA), and Script fields of the six vector and color types.
# MO's text changes after it was assigned (frame 5): X_ITE takes String (raw) when it prints. CO counts its toString calls.
FP_VALUES = [("str", "'abc'"), ("xml", "'a<b&\"c\\'d'"), ("expstr", "'1e3'"), ("null", "null"), ("true", "true"), ("obj", "({ })"),
             ("arr", "[1, 2]"), ("valueof", "({ valueOf: function () { return 7; } })"), ("mut", "MO")]
FP_NODES = [("Transform", "translation", "x"), ("TextureTransform", "translation", "x"), ("ClipPlane", "plane", "x"),
            ("Material", "diffuseColor", "r"), ("TextureProperties", "borderColor", "r")]
FP_SCRIPT_FIELDS = [("p3f", "SFVec3f", "1 1 1", "x"), ("p2d", "SFVec2d", "1 1", "x"), ("p3d", "SFVec3d", "1 1 1", "x"),
                    ("p4d", "SFVec4d", "1 1 1 1", "x"), ("pc", "SFColor", "1 1 1", "r"), ("pca", "SFColorRGBA", "1 1 1 1", "r")]
FP_PRE = ("var MO = { t: 'p', toString: function () { return this .t; } };\n"
          "var CO = { n: 0, toString: function () { return 'c' + (++ this .n); } };")


def fp_world(name, values):
    fields = "".join(f"  inputOutput {t} {n}_{vk} {d}\n" for n, t, d, _ in FP_SCRIPT_FIELDS for vk, _ in values)
    nodes, setp, read = [], [], []
    for ptype, pfield, comp in FP_NODES:
        for vk, v in values:
            a = f"P_{ptype}_{vk}"
            nodes.append(f"DEF {a} {ptype} {{ }}\n")
            setp.append(f"  T ('{ptype} {vk} set', function () {{ var N = Browser .currentScene .getNamedNode ('{a}'); var f = N .{pfield}; f .getValue () .{comp} = {v}; N .{pfield} = f; return 'ok'; }});")
            read.append(f"  T ('{ptype} {vk} text', function () {{ var N = Browser .currentScene .getNamedNode ('{a}'); return [N .toVRMLString (), N .toXMLString (), N .toJSONString (), N .{pfield} .toVRMLString (), N .{pfield} .toXMLString (), N .{pfield} .toJSONString ? N .{pfield} .toJSONString () : 'none']; }});")
    for n, t, d, comp in FP_SCRIPT_FIELDS:
        for vk, v in values:
            setp.append(f"  T ('{n} {vk} set', function () {{ var f = new {t} (); f .getValue () .{comp} = {v}; {n}_{vk} = f; return 'ok'; }});")
            read.append(f"  T ('{n} {vk} text', function () {{ return [{n}_{vk} .toVRMLString (), {n}_{vk} .toXMLString (), {n}_{vk} .toJSONString ? {n}_{vk} .toJSONString () : 'none', String ({n}_{vk})]; }});")
    after = ("var F_ = 0;\nfunction prepareEvents () {\n  ++ F_;\n  if (F_ === 3) {\n" + "\n".join(setp) + "\n  }\n  if (F_ === 5) { MO .t = 'q'; }\n"
             "  if (F_ === 7) {\n" + "\n".join(read) +
             "\n  T ('scene xml', function () { return Browser .currentScene .toXMLString (); });"
             "\n  T ('scene json', function () { return Browser .currentScene .toJSONString (); });"
             "\n  T ('count calls', function () { return CO .n; });"
             "\n    flush_ ();\n  }\n}\n")
    G.register(name, [], fields=fields, pre=CHAIN + "\n" + FP_PRE, after=after, nodes="".join(nodes))


fp_world("fldx-print", FP_VALUES)
# Documented open: X_ITE calls a raw component's toString once per print of it; the port takes the texts of all raw values of a
# Script when its code has run and before it prints, so a toString with side effects (a counter) is called a different number of times.
fp_world("fldx-print-count-open", [("count", "CO")])

# Units in standalone prints (checkpoint after b2e5b9f): X3DObject.toXXXString pushes a `null` execution context, so ToUnit leaves
# values as they are; the scene's own toXXXString converts them. A scene from createX3DFromString with length and angle units.
FU_SRC = ("#X3D V3.3 utf8\\nPROFILE Immersive\\nUNIT length km 1000\\nUNIT angle deg 0.017453292519943295\\n"
          "DEF U Transform { translation 1000 2000 3000 rotation 0 1 0 1.5 }\\nDEF V Viewpoint { position 10 20 30 }\\nDEF M Material { diffuseColor 0.5 0.5 0.5 }")
FU_READ = [
    ("node vrml", "return u .toVRMLString ();"), ("node xml", "return u .toXMLString ();"), ("node json", "return u .toJSONString ();"),
    ("viewpoint vrml", "return v .toVRMLString ();"), ("viewpoint xml", "return v .toXMLString ();"), ("viewpoint json", "return v .toJSONString ();"),
    ("field vrml", "return [u .translation .toVRMLString (), u .rotation .toVRMLString (), String (u .translation)];"),
    ("field xml", "return [u .translation .toXMLString (), u .rotation .toXMLString ()];"),
    ("field json", "return [u .translation .toJSONString (), u .rotation .toJSONString ()];"),
    ("raw node vrml", "var f = u .translation; f .getValue () .x = 'abc'; u .translation = f; return [u .toVRMLString (), u .toXMLString (), u .toJSONString ()];"),
    ("raw color vrml", "var f = m .diffuseColor; f .getValue () .r = '1e3'; m .diffuseColor = f; return [m .toVRMLString (), m .toXMLString (), m .toJSONString ()];"),
    ("scene vrml", "return s .toVRMLString ();"), ("scene xml", "return s .toXMLString ();"), ("scene json", "return s .toJSONString ();"),
]
FU_JS = ("Browser .createX3DFromString ('" + FU_SRC + "') .then (function (s) {\n"
         "    var u = s .getNamedNode ('U'), v = s .getNamedNode ('V'), m = s .getNamedNode ('M');\n" +
         "\n".join(f"    T ({lab!r}, function () {{ {code} }});" for lab, code in FU_READ) + "\n    flush_ ();\n  });\n  return 'ok';")
G.register("fldx-units", [("start", FU_JS)], pre=CHAIN, nodes=HEAD_NODES)

# A raw value one Script writes, read by another (both Scripts share X_ITE's page realm; the port's Scripts have a realm each).
FC_NODES = ("DEF CT Transform { }\nDEF CW Script {\n  inputOutput SFNode t USE CT\n  url \"ecmascript:\n"
            "function initialize () { var f = t .translation; f .getValue () .x = 'abc'; f .getValue () .y = null; t .translation = f; }\n\"\n}\n")
FC_AFTER = ("var F_ = 0;\nfunction prepareEvents () {\n  ++ F_;\n  if (F_ === 3) {\n"
            "  T ('other script raw', function () { var f = Browser .currentScene .getNamedNode ('CT') .translation; "
            "return [typeof f .getValue () .x, String (f .getValue () .x), typeof f .getValue () .y, String (f), f .toVRMLString ()]; });\n"
            "    flush_ ();\n  }\n}\n")
G.register("fldx-cross", [], pre=CHAIN, after=FC_AFTER, nodes=FC_NODES)

if __name__ == "__main__":
    sys.argv = [sys.argv[0]] + [n for n in G.defs if n.startswith("fldx-")]
    G.main()
