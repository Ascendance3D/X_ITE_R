#!/usr/bin/env python3
"""Generate the Script / MetadataSet / field-math probe worlds oracle/synthetic-rea-b09-sai/scr-*.wrl, meta-*.wrl, fld-*.wrl
(REA batch 09, step 3, items 1-3). Authored; synthetic inputs only.

A probe world holds one Script `P`. Each probe is a JavaScript function body run inside `T (label, fn)`, which appends
`NNN label = <result>` to the MFString field `out`; thrown values are recorded as `ERR name: message`. The scene text of the
pinned X_ITE and of the port (`toVRMLString`, every frame) is compared exactly, together with the sorted console log.

  python3 -I oracle/tools/gen_rea_b09_script.py            # write all worlds
  python3 -I oracle/tools/gen_rea_b09_script.py NAME ...   # only those
"""
import json, os, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-b09-sai")
HEAD = "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\n"


def vs(js):
    return js.replace("\\", "\\\\").replace('"', '\\"')


COMMON = r"""
var R_ = [];
function strip (s) { return String (s) .replace (/https?:\/\/[^\/]+\//g, ''); }
function show (v, d) {
  if (typeof v === 'number') return Object.is (v, -0) ? 'n:-0' : 'n:' + v;
  if (typeof v === 'string') return 's:' + JSON.stringify (strip (v));
  if (v === null || v === undefined) return String (v);
  if (typeof v === 'function') return 'fn';
  if (typeof v === 'boolean') return 'b:' + v;
  if (Array.isArray (v)) return 'A[' + v .map (function (x) { return show (x); }) .join (',') + ']';
  var c = v .constructor;
  return (c && c .name || '?') + ':' + String (v) + raw (v);
}
// The components of a field object at full double precision (String () rounds floats).
function raw (v) {
  if (typeof v .getTypeName !== 'function' || !v [Symbol .iterator] || v .getTypeName () .indexOf ('MF') === 0 && !v .length) return '';
  try { var a = Array .from (v, function (x) { return typeof x === 'object' && x && x [Symbol .iterator] ? '(' + Array .from (x, function (y) { return typeof y === 'object' && y && y [Symbol .iterator] ? '(' + Array .from (y) .join (' ') + ')' : y; }) .join (' ') + ')' : x; }); return ' raw=' + a .join (' '); } catch (e) { return ' raw=ERR'; }
}
function errText (e) {
  var n = e && e .name;
  if (n === 'TypeError' || n === 'ReferenceError' || n === 'SyntaxError') return 'ERR ' + n + ' (engine wording)';
  return 'ERR ' + n + ': ' + strip (e && e .message);
}
function T (n, f) {
  var r;
  try { r = show (f ()); } catch (e) { r = errText (e); }
  R_ .push ((R_ .length < 10 ? '00' : R_ .length < 100 ? '0' : '') + R_ .length + ' ' + n + ' = ' + r);
}
function flush_ () { out = new MFString (... R_); }
"""


def script_node(name, fields, js, extra=""):
    return f'DEF {name} Script {{\n{fields}\n{extra}  url "ecmascript:\n{vs(js)}\n  "\n}}\n'


def battery(name, title, probes, fields="", pre="", after="", nodes="", flush_each=False):
    sel = os.environ.get("PROBES")  # debugging: "a:b" runs probes a..b-1 only
    if sel:
        a, b = sel.split(":")
        probes = probes[int(a or 0):int(b or len(probes))]
    import re
    body = "\n".join(f"  T ({lab!r}, function () {{ {re.sub(r'(?<![.\w])this(?!\w)', 'SELF', code)} }});" for lab, code in probes)
    js = f"{COMMON}\nvar SELF;\n{pre}\nfunction initialize () {{\n  SELF = this;\n{body}\n  flush_ ();\n}}\n{after}"
    fields = "  inputOutput MFString out [ ]\n" + fields
    text = HEAD + f'META "title" "{title}"\n' + nodes + script_node("P", fields, js)
    return text


defs = {}  # world name -> dict (see register)


def register(name, probes, open_prefixes=(), **kw):
    """A probe world. Probes whose label starts with one of `open_prefixes` still differ from X_ITE: they are written to
    NAME-open.wrl, which is generated but never baselined."""
    defs[name] = dict(probes=probes, open=tuple(open_prefixes), kw=kw)


def custom(name):
    """A hand-written world: the function returns the complete text."""
    def deco(fn):
        defs[name] = dict(custom=fn)
        return fn
    return deco


# ---------------------------------------------------------------------------------------------------------------------------
# 1. Script field access
# ---------------------------------------------------------------------------------------------------------------------------

ALL_FIELDS = """
  initializeOnly SFBool iBool TRUE
  initializeOnly SFInt32 iInt 7
  initializeOnly SFFloat iFloat 1.5
  initializeOnly SFDouble iDouble 2.5
  initializeOnly SFTime iTime 3
  initializeOnly SFString iString "str"
  initializeOnly SFVec2f iV2f 1 2
  initializeOnly SFVec3f iV3f 1 2 3
  initializeOnly SFVec3d iV3d 4 5 6
  initializeOnly SFVec4f iV4f 1 2 3 4
  initializeOnly SFColor iCol 0.1 0.2 0.3
  initializeOnly SFColorRGBA iColA 0.1 0.2 0.3 0.4
  initializeOnly SFRotation iRot 0 1 0 1.5
  initializeOnly SFMatrix3f iM3 1 0 0 0 1 0 0 0 1
  initializeOnly SFMatrix4f iM4 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1
  initializeOnly SFNode iNode NULL
  initializeOnly SFImage iImg 1 1 3 0x112233
  initializeOnly MFBool mBool [TRUE FALSE]
  initializeOnly MFInt32 mInt [1 2 3]
  initializeOnly MFFloat mFloat [1.5 2.5]
  initializeOnly MFDouble mDouble [1.5]
  initializeOnly MFTime mTime [1 2]
  initializeOnly MFString mString ["a" "b"]
  initializeOnly MFVec2f mV2f [1 2, 3 4]
  initializeOnly MFVec3f mV3f [1 2 3, 4 5 6]
  initializeOnly MFVec4f mV4f [1 2 3 4]
  initializeOnly MFColor mCol [1 0 0]
  initializeOnly MFColorRGBA mColA [1 0 0 1]
  initializeOnly MFRotation mRot [0 0 1 1]
  initializeOnly MFMatrix3f mM3 [1 0 0 0 1 0 0 0 1]
  initializeOnly MFMatrix4f mM4 [1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1]
  initializeOnly MFNode mNode [ ]
  initializeOnly MFImage mImg [ ]
"""

ACCESS_FIELDS = """
  inputOnly SFFloat evIn
  inputOnly MFString evInM
  outputOnly SFFloat evOut
  outputOnly SFVec3f evOutV
  outputOnly MFInt32 evOutM
  initializeOnly SFFloat init 5
  inputOutput SFFloat io 6
  inputOutput MFFloat ioM [1 2]
  inputOutput SFVec3f ioV 1 2 3
  inputOutput SFString ioS "s"
  inputOutput SFNode ioN NULL
  inputOutput SFBool ioB FALSE
  inputOutput SFInt32 ioI 0
  inputOutput SFTime ioT 0
  inputOutput SFRotation ioR 0 0 1 0
  inputOutput SFColor ioC 0 0 0
  inputOutput MFNode ioMN [ ]
  inputOutput MFVec3f ioMV [ ]
  inputOutput MFString ioMS [ ]
  inputOutput SFDouble ioD 0
"""

P_ACCESS = [
    ("typeof evIn", "return typeof evIn;"),
    ("typeof evInM", "return typeof evInM;"),
    ("read evOut", "return evOut;"),
    ("read evOutV", "return evOutV;"),
    ("read evOutM", "return evOutM;"),
    ("read init", "return init;"),
    ("read io", "return io;"),
    ("read io_changed", "return io_changed;"),
    ("typeof set_io", "return typeof set_io;"),
    ("read ioM", "return ioM;"),
    ("read ioM_changed", "return ioM_changed;"),
    ("ioM length", "return ioM .length;"),
    ("ioM[1]", "return ioM [1];"),
    ("read ioV", "return ioV;"),
    ("ioV same object", "return ioV === ioV;"),
    ("ioM same object", "return ioM === ioM;"),
    ("ioV vs ioV_changed", "return ioV === ioV_changed;"),
    ("write evOut", "evOut = 3; return evOut;"),
    ("write evOutV", "evOutV = new SFVec3f (1, 2, 3); return evOutV;"),
    ("write evOutV member", "evOutV .x = 9; return evOutV;"),
    ("write evOutM", "evOutM = new MFInt32 (1, 2, 3); return evOutM;"),
    ("write evOutM array", "evOutM = [4, 5]; return evOutM;"),
    ("write init", "init = 8; return init;"),
    ("write io", "io = 9; return io;"),
    ("write io_changed", "io_changed = 10; return io;"),
    ("write ioV", "ioV = new SFVec3f (4, 5, 6); return ioV;"),
    ("write ioV member", "ioV .y = 50; return ioV;"),
    ("write ioV array", "ioV = [7, 8, 9]; return ioV;"),
    ("write ioV SFVec3d", "ioV = new SFVec3d (1, 1, 1); return ioV;"),
    ("write ioV SFVec2f", "ioV = new SFVec2f (1, 1); return ioV;"),
    ("write ioV number", "ioV = 5; return ioV;"),
    ("write ioV string", "ioV = 'x'; return ioV;"),
    ("write ioV null", "ioV = null; return ioV;"),
    ("write ioV undefined", "ioV = undefined; return ioV;"),
    ("write ioM array", "ioM = [3, 4, 5]; return ioM;"),
    ("write ioM MFFloat", "ioM = new MFFloat (7, 8); return ioM;"),
    ("write ioM MFDouble", "ioM = new MFDouble (7, 8); return ioM;"),
    ("write ioM number", "ioM = 5; return ioM;"),
    ("write ioM string", "ioM = 'abc'; return ioM;"),
    ("write ioM null", "ioM = null; return ioM;"),
    ("ioM push", "ioM .push (9); return ioM;"),
    ("ioM index write", "ioM [0] = 11; return ioM;"),
    ("ioM length write", "ioM .length = 2; return ioM;"),
    ("ioM index past end", "ioM [5] = 1; return ioM;"),
    ("ioS = number", "ioS = 5; return ioS;"),
    ("ioS = null", "ioS = null; return ioS;"),
    ("ioS = undefined", "ioS = undefined; return ioS;"),
    ("ioS = object", "ioS = {}; return ioS;"),
    ("ioS = SFVec3f", "ioS = new SFVec3f (1, 2, 3); return ioS;"),
    ("ioS = true", "ioS = true; return ioS;"),
    ("io = string num", "io = '2.5'; return io;"),
    ("io = string bad", "io = 'abc'; return io;"),
    ("io = null", "io = null; return io;"),
    ("io = undefined", "io = undefined; return io;"),
    ("io = true", "io = true; return io;"),
    ("io = object", "io = {}; return io;"),
    ("io = NaN", "io = NaN; return io;"),
    ("io = Infinity", "io = Infinity; return io;"),
    ("io = float rounding", "io = 0.1; return io;"),
    ("io = big", "io = 1e40; return io;"),
    ("ioD = 0.1", "ioD = 0.1; return ioD;"),
    ("ioI = 3.7", "ioI = 3.7; return ioI;"),
    ("ioI = -3.7", "ioI = -3.7; return ioI;"),
    ("ioI = '5'", "ioI = '5'; return ioI;"),
    ("ioI = big", "ioI = 4294967297; return ioI;"),
    ("ioI = 2^31", "ioI = 2147483648; return ioI;"),
    ("ioI = NaN", "ioI = NaN; return ioI;"),
    ("ioI = null", "ioI = null; return ioI;"),
    ("ioI = true", "ioI = true; return ioI;"),
    ("ioB = 1", "ioB = 1; return ioB;"),
    ("ioB = 0", "ioB = 0; return ioB;"),
    ("ioB = 'false'", "ioB = 'false'; return ioB;"),
    ("ioB = ''", "ioB = ''; return ioB;"),
    ("ioB = null", "ioB = null; return ioB;"),
    ("ioB = {}", "ioB = {}; return ioB;"),
    ("ioT = '4'", "ioT = '4'; return ioT;"),
    ("ioT = null", "ioT = null; return ioT;"),
    ("ioT = Date", "ioT = 1.5; return ioT;"),
    ("ioN = null", "ioN = null; return ioN;"),
    ("ioN = this", "ioN = this; return ioN === this;"),
    ("ioN = number", "ioN = 5; return ioN;"),
    ("ioN = {}", "ioN = {}; return ioN;"),
    ("ioN = undefined", "ioN = undefined; return ioN;"),
    ("ioN = SFVec3f", "ioN = new SFVec3f (); return ioN;"),
    ("ioN = NULL", "ioN = NULL; return ioN;"),
    ("ioMN = [this]", "ioMN = [this]; return ioMN .length;"),
    ("ioMN = [null]", "ioMN = [null]; return ioMN .length + ':' + ioMN [0];"),
    ("ioMN = [1]", "ioMN = [1]; return ioMN .length;"),
    ("ioMN = this", "ioMN = this; return ioMN;"),
    ("ioMN = new MFNode", "ioMN = new MFNode (this, this); return ioMN .length + ':' + (ioMN [0] === this);"),
    ("ioMV = [SFVec3f]", "ioMV = [new SFVec3f (1, 2, 3), new SFVec3f (4, 5, 6)]; return ioMV;"),
    ("ioMV = [arrays]", "ioMV = [[1, 2, 3]]; return ioMV;"),
    ("ioMV = [1,2,3]", "ioMV = [1, 2, 3]; return ioMV;"),
    ("ioMV = [SFVec2f]", "ioMV = [new SFVec2f (1, 2)]; return ioMV;"),
    ("ioMV [0] member", "ioMV = [new SFVec3f (1, 2, 3)]; ioMV [0] .x = 9; return ioMV;"),
    ("ioMV [0] replace", "ioMV [0] = new SFVec3f (5, 5, 5); return ioMV;"),
    ("ioMV [3] gap", "ioMV [3] = new SFVec3f (7, 7, 7); return ioMV;"),
    ("ioMV length", "return ioMV .length;"),
    ("ioMS = [1, null]", "ioMS = [1, null, undefined, true, 'x']; return ioMS;"),
    ("ioMS = 'abc'", "ioMS = 'abc'; return ioMS;"),
    ("ioMS = new MFString", "ioMS = new MFString ('a', 'b'); return ioMS;"),
    ("ioMS toString", "return String (ioMS);"),
    ("ioMS join", "return ioMS .join ('+');"),
    ("ioR = SFVec4", "ioR = new SFVec4f (0, 0, 1, 1); return ioR;"),
    ("ioR = array", "ioR = [0, 1, 0, 1]; return ioR;"),
    ("ioR = SFRotation", "ioR = new SFRotation (0, 1, 0, 1); return ioR;"),
    ("ioC = SFVec3f", "ioC = new SFVec3f (1, 1, 1); return ioC;"),
    ("ioC = SFColor", "ioC = new SFColor (0.5, 0.5, 0.5); return ioC;"),
    ("ioC = SFColorRGBA", "ioC = new SFColorRGBA (0.5, 0.5, 0.5, 1); return ioC;"),
    ("ioC member r", "ioC .r = 0.25; return ioC;"),
    ("ioC member x", "ioC .x = 1; return ioC + ':' + ioC .x;"),
    ("this", "return this;"),
    ("nested this", "return (function () { return this; }) ();"),
    ("this ctor", "return this .constructor .name;"),
    ("this typeof", "return typeof this;"),
    ("this.getNodeName", "return this .getNodeName ();"),
    ("this.getNodeTypeName", "return this .getNodeTypeName ();"),
    ("this.getNodeType", "return this .getNodeType ();"),
    ("this.getType", "return this .getType ();"),
    ("this.getTypeName", "return this .getTypeName ();"),
    ("this.getFieldDefinitions length", "return this .getFieldDefinitions () .length;"),
    ("this.getFieldDefinitions names", "return Array .from (this .getFieldDefinitions (), function (d) { return d .name + '/' + d .accessType + '/' + d .dataType; });"),
    ("this.io", "return this .io;"),
    ("this.io_changed", "return this .io_changed;"),
    ("this.set_io", "return this .set_io;"),
    ("this.evIn", "return this .evIn;"),
    ("this.evOut", "return this .evOut;"),
    ("this.init", "return this .init;"),
    ("this.io = ", "this .io = 21; return io + ':' + this .io;"),
    ("this.evIn =", "this .evIn = 22; return this .evIn;"),
    ("this.nonexistent", "return this .nonexistent;"),
    ("this.nonexistent =", "this .nonexistent = 3; return this .nonexistent;"),
    ("this.getField io", "var f = this .getField ('io'); return f .getName () + '/' + f .getType () + '/' + f .getAccessType () + '/' + f .getValue ();"),
    ("this.getField evIn", "var f = this .getField ('evIn'); return f .getName () + '/' + f .getType () + '/' + f .getAccessType ();"),
    ("this.getField evOut", "var f = this .getField ('evOut'); return f .getName () + '/' + f .getAccessType ();"),
    ("this.getField missing", "return this .getField ('nope');"),
    ("this.getField io_changed", "return this .getField ('io_changed') .getName ();"),
    ("this.getField set_io", "return this .getField ('set_io') .getName ();"),
    ("this.hasField", "return [this .hasField ('io'), this .hasField ('nope'), this .hasField ('io_changed')];"),
    ("getField isReadable", "var f = this .getField ('evIn'); return [f .isReadable (), f .isWritable (), f .isInput (), f .isOutput ()];"),
    ("getField isReadable io", "var f = this .getField ('io'); return [f .isReadable (), f .isWritable (), f .isInput (), f .isOutput ()];"),
    ("getField isReadable init", "var f = this .getField ('init'); return [f .isReadable (), f .isWritable (), f .isInput (), f .isOutput ()];"),
    ("getField isReadable out", "var f = this .getField ('evOut'); return [f .isReadable (), f .isWritable (), f .isInput (), f .isOutput ()];"),
    ("getField setValue", "var f = this .getField ('ioS'); f .setValue ('zz'); return ioS + ':' + f .getValue ();"),
    ("getField evIn setValue", "var f = this .getField ('evIn'); f .setValue (4); return f .getValue ();"),
    ("getField evOut getValue", "var f = this .getField ('evOut'); return f .getValue ();"),
    ("getField init setValue", "var f = this .getField ('init'); f .setValue (4); return init + ':' + f .getValue ();"),
    ("getField valueOf", "var f = this .getField ('ioV'); return [typeof f .valueOf (), f .valueOf () === f];"),
    ("getField equals", "var f = this .getField ('ioV'); return [f .equals (new SFVec3f (7, 8, 9)), f .equals (f)];"),
    ("getField toString", "return this .getField ('ioV') .toString ();"),
    ("getField toVRMLString", "return this .getField ('ioV') .toVRMLString ();"),
    ("getField toXMLString", "return this .getField ('ioV') .toXMLString ();"),
    ("getField toJSONString", "return this .getField ('ioV') .toJSONString ();"),
    ("getField clone", "var f = this .getField ('ioV'); var c = f .copy (); return [c === f, String (c)];"),
    ("getField isDefaultValue", "return [this .getField ('ioV') .isDefaultValue (), this .getField ('io') .isDefaultValue ()];"),
    ("getField getTainted", "return typeof this .getField ('io') .getTainted;"),
    ("getField setName", "return typeof this .getField ('io') .setName;"),
    ("field proto names", "return Object .getOwnPropertyNames (Object .getPrototypeOf (this .getField ('ioV'))) .sort () .join (',');"),
    ("fieldDefinitions", "var d = this .getFieldDefinitions () [0]; return [d .name, d .accessType, d .dataType, d .value, typeof d .getName];"),
]


# X_ITE keeps JavaScript `undefined` / NaN in a vector, color or array that is given a value of the wrong shape; these assignments are
# not modeled (a number cannot hold undefined). They run in scr-field-access-open, which is never baselined.
register("scr-field-access", P_ACCESS, fields=ACCESS_FIELDS,
         open_prefixes=("write ioV array", "write ioV SFVec2f", "write ioV number", "write ioV string", "write ioM number", "ioC = SFVec3f", "ioC = SFColorRGBA",
                        "ioC member", "getField toString", "getField toVRMLString", "getField toXMLString", "getField toJSONString", "getField clone",
                        "field proto names", "fieldDefinitions", "this.getNodeType", "getField setName"))


# typed defaults of all fields, plus per-type read forms
P_DEFAULTS = []
for n in ["iBool", "iInt", "iFloat", "iDouble", "iTime", "iString", "iV2f", "iV3f", "iV3d", "iV4f", "iCol", "iColA", "iRot", "iM3", "iM4", "iNode", "iImg",
          "mBool", "mInt", "mFloat", "mDouble", "mTime", "mString", "mV2f", "mV3f", "mV4f", "mCol", "mColA", "mRot", "mM3", "mM4", "mNode", "mImg"]:
    P_DEFAULTS.append((f"{n}", f"return {n};"))
    P_DEFAULTS.append((f"{n} typeof", f"return typeof {n};"))
    P_DEFAULTS.append((f"{n} getType", f"return this .getField ('{n}') .getType ();"))
    P_DEFAULTS.append((f"{n} typeName", f"return this .getField ('{n}') .getTypeName ();"))
    P_DEFAULTS.append((f"{n} json", f"return this .getField ('{n}') .toJSONString ();"))
    P_DEFAULTS.append((f"{n} xml", f"return this .getField ('{n}') .toXMLString ();"))
    P_DEFAULTS.append((f"{n} isDefault", f"return this .getField ('{n}') .isDefaultValue ();"))


register("scr-field-defaults", P_DEFAULTS, fields=ALL_FIELDS)


CLASS_NAMES = ["SFBool", "SFColor", "SFColorRGBA", "SFDouble", "SFFloat", "SFImage", "SFInt32", "SFMatrix3d", "SFMatrix3f", "SFMatrix4d", "SFMatrix4f",
               "SFNode", "SFQuaternion", "SFRotation", "SFString", "SFTime", "SFVec2d", "SFVec2f", "SFVec3d", "SFVec3f", "SFVec4d", "SFVec4f", "VrmlMatrix",
               "MFBool", "MFColor", "MFColorRGBA", "MFDouble", "MFFloat", "MFImage", "MFInt32", "MFMatrix3d", "MFMatrix3f", "MFMatrix4d", "MFMatrix4f",
               "MFNode", "MFQuaternion", "MFRotation", "MFString", "MFTime", "MFVec2d", "MFVec2f", "MFVec3d", "MFVec3f", "MFVec4d", "MFVec4f",
               "X3DField", "X3DArrayField", "X3DConstants", "X3DBrowser", "X3DExecutionContext", "X3DScene", "X3DBaseNode", "X3DRoute", "Browser", "NULL", "TRUE", "FALSE",
               "print", "trace", "Math", "Date", "console", "window", "document", "globalThis", "setTimeout", "clearTimeout", "setInterval", "XMLHttpRequest", "fetch"]

P_CLASSES = []
for _n in CLASS_NAMES:
    P_CLASSES.append((f"{_n} typeof", f"return typeof {_n};"))
    P_CLASSES.append((f"{_n} name", f"var C = {_n}; return [C && C .name, C && C .typeName, C && C .length, typeof C];"))
    P_CLASSES.append((f"{_n} instance", f"var i = new {_n} (); return [i .constructor .name, i instanceof {_n}, i .getTypeName (), i .getType (), String (i)];"))


OPEN_GLOBALS = ("X3DBrowser", "X3DExecutionContext", "X3DScene", "X3DBaseNode", "X3DRoute", "window", "document", "globalThis name", "setTimeout",
                "clearTimeout", "setInterval", "XMLHttpRequest", "fetch", "print name", "trace name", "SFNode name", "X3DField name", "X3DField instance",
                "X3DArrayField name", "X3DArrayField instance")
register("scr-classes", P_CLASSES, open_prefixes=OPEN_GLOBALS)



# ---------------------------------------------------------------------------------------------------------------------------
# 1b. Script lifecycle, events, directOutput / mustEvaluate. Several Scripts append to one list shared through `globalThis .LOG`; the
#     reporter Script copies it to its `out` field every tick, so the scene text carries the order in which things ran.
# ---------------------------------------------------------------------------------------------------------------------------

LOG_JS = r"""
function log (s) { var L = globalThis .LOG = globalThis .LOG || []; L .push (NAME + '.' + s); }
function first (key, n) { var C = globalThis .CNT = globalThis .CNT || {}; C [NAME + key] = (C [NAME + key] || 0) + 1; return C [NAME + key] <= n; }
"""

REPORTER = """
DEF R Script {
  inputOutput MFString out [ ]
  inputOnly SFTime tick
  url "ecmascript:
    function tick (v, t) { var L = globalThis .LOG || []; out = new MFString (... L .map (function (s, i) { return i + ' ' + s; })); }
  "
}
DEF STEP TimeSensor { cycleInterval 0.05 loop TRUE }
"""

# The same, with the list sorted (for worlds whose Scripts finish loading in an order the port does not reproduce).
REPORTER_SORTED = REPORTER.replace("var L = globalThis .LOG || [];", "var L = (globalThis .LOG || []) .slice () .sort ();")


def lscript(name, fields, js, extra=""):
    return script_node(name, fields, f"var NAME = '{name}';\n{LOG_JS}\n{js}", extra)


def lifecycle_world(title, scripts, routes, nodes="", sort=False):
    return HEAD + f'META "title" "{title}"\n' + nodes + "".join(scripts) + (REPORTER_SORTED if sort else REPORTER) + routes + "ROUTE STEP.cycleTime TO R.tick\n"


@custom("scr-lifecycle")
def _():
    a = lscript("A", """  inputOnly SFTime tick
  inputOnly SFInt32 evIn
  outputOnly SFInt32 evOut
  inputOutput SFInt32 io 0
  initializeOnly SFInt32 init 4
  outputOnly SFTime t_out""", r"""
var n = 0;
function initialize (t) {
  log ('initialize init=' + init + ' io=' + io + ' typeof t=' + typeof t + ' this=' + (this && this .getNodeTypeName ()));
  evOut = 1;
  io = 2;
  log ('initialize end');
}
function prepareEvents (t) { if (first ('pre', 3)) log ('prepareEvents'); }
function eventsProcessed (t) { if (first ('ep', 8)) log ('eventsProcessed'); }
function tick (v, t) {
  n++;
  if (n > 8) return;
  log ('tick ' + n + ' arg=' + typeof v + '/' + typeof t);
  if (n == 2) { evOut = 20; log ('after evOut=20'); }
  if (n == 3) { io = 30; log ('after io=30'); }
  if (n == 4) { evOut = 41; evOut = 42; log ('after evOut 41 42'); }
  if (n == 5) { io = 50; io = 50; log ('after io=50 twice'); }
  if (n == 6) { t_out = t; }
}
function evIn (v, t) { log ('evIn ' + v + ' ' + typeof t); }
function set_io (v, t) { log ('set_io ' + v); }
function shutdown (t) { log ('shutdown'); }
""")
    b = lscript("B", """  inputOnly SFInt32 evIn
  inputOnly SFTime tick
  outputOnly SFInt32 evOut
  inputOutput SFInt32 io 0""", r"""
function initialize () { log ('initialize io=' + io); }
function prepareEvents () { if (first ('pre', 3)) log ('prepareEvents'); }
function eventsProcessed () { if (first ('ep', 8)) log ('eventsProcessed'); }
function evIn (v, t) { log ('evIn ' + v); evOut = v + 100; }
function set_io (v, t) { log ('set_io ' + v); }
function tick (v, t) { if (first ('tick', 3)) log ('tick'); }
""")
    c = lscript("C", """  inputOnly SFInt32 evIn
  inputOnly SFInt32 other""", r"""
function initialize () { log ('initialize'); }
function eventsProcessed () { if (first ('ep', 8)) log ('eventsProcessed'); }
function evIn (v) { log ('evIn ' + v); }
function other (v) { log ('other ' + v); }
""")
    routes = """ROUTE A.evOut TO B.evIn
ROUTE B.evOut TO C.evIn
ROUTE A.io_changed TO B.set_io
ROUTE A.io_changed TO C.other
ROUTE STEP.cycleTime TO A.tick
ROUTE STEP.cycleTime TO B.tick
"""
    return lifecycle_world("scr lifecycle", [a, b, c], routes)


@custom("scr-self-events")
def _():
    # Does a Script's own assignment run its own handler? Event coalescing, order, reentrancy.
    a = lscript("A", """  inputOnly SFTime tick
  inputOutput SFInt32 io 0
  inputOutput SFFloat f 0
  inputOutput MFFloat mf [ ]
  inputOutput SFVec3f v 0 0 0
  inputOutput SFNode nd NULL
  outputOnly SFInt32 outI
  outputOnly SFBool outB
  inputOnly SFInt32 inI
  inputOnly MFString inMS
  inputOnly SFVec3f inV
  inputOnly SFRotation inR
  inputOnly SFNode inN
  inputOnly MFNode inMN""", r"""
var n = 0;
function initialize () { log ('initialize'); }
function set_io (v, t) { log ('set_io ' + v + ' (' + typeof v + ')'); if (v == 1) { io = 2; log ('set_io reassign'); } }
function set_f (v) { log ('set_f ' + v); }
function set_mf (v) { log ('set_mf ' + v .length + ' ' + (v === mf)); }
function set_v (v) { log ('set_v ' + v + ' ' + (v === this .v) + ' ' + v .constructor .name); }
function set_nd (v) { log ('set_nd ' + (v ? v .getNodeTypeName () : v)); }
function inI (v) { log ('inI ' + v); }
function inMS (v) { log ('inMS ' + v .length + ' ' + v .constructor .name); }
function inV (v) { log ('inV ' + v + ' ' + v .constructor .name); v .x = 99; log ('inV after write ' + v); }
function inR (v) { log ('inR ' + v); }
function inN (v) { log ('inN ' + (v ? v .getNodeTypeName () + ' ' + v .getNodeName () : v)); }
function inMN (v) { log ('inMN ' + v .length + ' ' + v .constructor .name); }
function tick (v, t) {
  n++;
  if (n == 2) { io = 1; log ('assigned io=1'); }
  if (n == 3) { io = 5; io = 6; io = 7; log ('assigned io 5 6 7'); }
  if (n == 4) { f = 1.5; f = 2.5; log ('assigned f'); }
  if (n == 5) { mf = [1, 2, 3]; log ('assigned mf'); }
  if (n == 6) { v = new SFVec3f (1, 2, 3); log ('assigned v'); }
  if (n == 7) { nd = Browser .currentScene .getNamedNode ('T1'); log ('assigned nd'); }
  if (n == 8) { outI = 1; outB = true; log ('assigned outs'); }
  if (n == 9) { mf .push (4); mf [0] = 9; log ('mf push + index'); }
  if (n == 10) { v .x = 5; log ('v.x = 5'); }
  if (n == 11) { io = io; log ('io = io'); }
}
""")
    d = lscript("D", """  inputOnly SFTime tick
  outputOnly SFInt32 outI
  outputOnly MFString outMS
  outputOnly SFVec3f outV
  outputOnly SFRotation outR
  outputOnly SFNode outN
  outputOnly MFNode outMN""", r"""
var n = 0;
function tick () {
  n++;
  if (n == 2) { outI = 7.9; log ('outI'); }
  if (n == 3) { outMS = ['a', 'b']; log ('outMS'); }
  if (n == 4) { outV = new SFVec3f (1, 2, 3); log ('outV'); }
  if (n == 5) { outR = new SFRotation (0, 1, 0, 1); log ('outR'); }
  if (n == 6) { outN = Browser .currentScene .getNamedNode ('T1'); log ('outN'); }
  if (n == 7) { outMN = [Browser .currentScene .getNamedNode ('T1'), Browser .currentScene .getNamedNode ('T2')]; log ('outMN'); }
  if (n == 9) { outI = 8; outI = 9; log ('outI twice'); }
}
""")
    routes = """ROUTE D.outI TO A.inI
ROUTE D.outMS TO A.inMS
ROUTE D.outV TO A.inV
ROUTE D.outR TO A.inR
ROUTE D.outN TO A.inN
ROUTE D.outMN TO A.inMN
ROUTE STEP.cycleTime TO A.tick
ROUTE STEP.cycleTime TO D.tick
"""
    return lifecycle_world("scr self events", [a, d], routes, nodes="DEF T1 Transform { }\nDEF T2 Transform { translation 1 0 0 }\n")


@custom("scr-image-events-open")
def _():
    # X_ITE delivers an SFImage event twice when a Script assigns an outputOnly SFImage (the image's MFInt32 array reports to its parent as
    # well); the port delivers once. Not baselined.
    d = lscript("D", """  outputOnly SFImage outImg
  inputOnly SFTime tick""", r"""
var n = 0;
function tick () { n++; if (n == 2) { outImg = new SFImage (2, 1, 3, new MFInt32 (1, 2)); log ('outImg'); } }
""")
    p = lscript("P", "  inputOnly SFImage inImg", "function inImg (v) { log ('inImg ' + v .width + ' ' + v .array .length); }")
    return lifecycle_world("scr image events", [d, p], "ROUTE STEP.cycleTime TO D.tick\nROUTE D.outImg TO P.inImg\n")


@custom("scr-direct-output")
def _():
    # directOutput / mustEvaluate are parsed and ignored by X_ITE: a Script writes node fields and reads them with and without them.
    mk = lambda name, flags: lscript(name, f"""  inputOnly SFTime tick
  initializeOnly SFNode target NULL
  {flags}""", r"""
var n = 0;
function initialize () { log ('initialize directOutput=' + this .directOutput + ' mustEvaluate=' + this .mustEvaluate + ' target=' + (target ? target .getNodeTypeName () : target)); }
function tick () {
  n++;
  if (n == 2 && target) { target .translation = new SFVec3f (n, n, n); log ('wrote translation'); }
  if (n == 3 && target) { log ('read ' + target .translation + ' ' + target .children .length); }
  if (n == 4) { try { this .directOutput = !this .directOutput; log ('wrote directOutput ' + this .directOutput); } catch (e) { log ('ERR ' + e .name); } }
}
""")
    scripts = [mk("A", "directOutput TRUE\n  mustEvaluate TRUE\n  initializeOnly SFNode other NULL"),
               mk("B", "directOutput FALSE\n  mustEvaluate FALSE\n  initializeOnly SFNode other NULL"),
               mk("C", "initializeOnly SFNode other NULL")]
    text = lifecycle_world("scr direct output", scripts, "ROUTE STEP.cycleTime TO A.tick\nROUTE STEP.cycleTime TO B.tick\nROUTE STEP.cycleTime TO C.tick\n",
                           nodes="DEF TA Transform { }\nDEF TB Transform { }\nDEF TC Transform { }\n")
    # target fields are set with `initializeOnly SFNode target USE TA`
    text = text.replace("DEF A Script {\n  inputOnly SFTime tick\n  initializeOnly SFNode target NULL", "DEF A Script {\n  inputOnly SFTime tick\n  initializeOnly SFNode target USE TA")
    text = text.replace("DEF B Script {\n  inputOnly SFTime tick\n  initializeOnly SFNode target NULL", "DEF B Script {\n  inputOnly SFTime tick\n  initializeOnly SFNode target USE TB")
    text = text.replace("DEF C Script {\n  inputOnly SFTime tick\n  initializeOnly SFNode target NULL", "DEF C Script {\n  inputOnly SFTime tick\n  initializeOnly SFNode target USE TC")
    return text




# ---------------------------------------------------------------------------------------------------------------------------
# 1c. Script errors and console levels. The page keeps the first line of every console.warn / console.error, sorted (`log`); each Script
#     also records into the shared list what ran after the error.
# ---------------------------------------------------------------------------------------------------------------------------

ERR_CASES = [
    # (DEF name, fields, js)
    ("E01", "", "function initialize () { throw new Error ('boom 1'); }"),
    ("E02", "", "function initialize () { throw 'a string'; }"),
    ("E03", "", "function initialize () { throw 42; }"),
    ("E04", "", "function initialize () { throw { a: 1 }; }"),
    ("E05", "", "function initialize () { throw null; }"),
    ("E06", "", "function initialize () { throw undefined; }"),
    ("E07", "", "function initialize () { var e = new Error ('named'); e .name = 'CustomError'; throw e; }"),
    ("E08", "", "function initialize () { throw new RangeError ('rng'); }"),
    ("E09", "", "function initialize () { undefinedFunction (); }"),
    ("E10", "", "function initialize () { var x = undefinedVariable + 1; }"),
    ("E11", "", "function initialize () { var o = null; o .x; }"),
    ("E12", "", "function initialize () { var o; o .x = 1; }"),
    ("E13", "", "function initialize () { var o = {}; o .f (); }"),
    ("E14", "", "function initialize () { new SFVec3f (1, 2, 3) .nope (); }"),
    ("E15", "", "function initialize () { Browser .nope (); }"),
    ("E16", "", "function initialize () { JSON .parse ('{bad'); }"),
    ("E17", "", "function initialize () { new Array (-1); }"),
    ("E18", "", "function initialize () { decodeURIComponent ('%'); }"),
    ("E19", "", "function initialize () { x = 1; throw new Error ('after leak'); }"),
    ("E20", "", "function initialize () { log ('before'); throw new Error ('with log'); }\nfunction log (s) { (globalThis .LOG = globalThis .LOG || []) .push ('E20.' + s); }"),
    ("E21", "  inputOnly SFInt32 evIn\n  inputOnly SFTime tick", """
var n = 0;
function evIn (v) { n++; (globalThis .LOG = globalThis .LOG || []) .push ('E21.evIn ' + v); if (v == 1) throw new Error ('handler ' + v); }
function tick () { n++; if (n == 2) throw new Error ('tick handler'); }"""),
    ("E22", "", """
var n = 0;
function prepareEvents () { n++; if (n == 3) throw new Error ('prepareEvents 3'); }
function eventsProcessed () { n++; if (n == 4) throw new Error ('eventsProcessed 4'); }"""),
    ("E23", "", "function initialize () { return Promise .reject (new Error ('rejected')); }"),
    ("E24", "", "function initialize () { return Promise .resolve (1) .then (function () { throw new Error ('late'); }); }"),
    ("E25", "", "var a = ;"),
    ("E26", "", "function initialize () { }\nfunction ("),
    ("E27", "", "throw new Error ('top level');"),
    ("E28", "", "var x = notDefinedAtTop;"),
    ("E29", "", "function initialize () { console .error (new Error ('console error object')); console .warn ('a', 1, true, null, undefined); console .error ('multi\\nline'); console .log ('plain log'); }"),
    ("E30", "", "function initialize () { console .info ('info'); console .debug ('debug'); console .assert (false, 'assert'); console .trace ('trace'); console .dir ({}); console .table ([1]); }"),
    ("E31", "", "function initialize () { print ('p1'); trace ('t1'); Browser .print ('b1', 2); Browser .println ('b2', 3); }"),
    ("E32", "", "function initialize () { setTimeout; }"),
]


@custom("scr-errors")
def _():
    scripts = []
    for name, fields, js in ERR_CASES:
        scripts.append(script_node(name, fields, js))
    ev = """ROUTE STEP.cycleTime TO E21.tick
"""
    # a driver that sends events to E21.evIn (1 throws, 2 does not)
    drv = lscript("DRV", """  inputOnly SFTime tick
  outputOnly SFInt32 outI""", r"""
var n = 0;
function tick () { n++; if (n == 3) { outI = 1; log ('sent 1'); } if (n == 4) { outI = 2; log ('sent 2'); } }
""")
    return lifecycle_world("scr errors", scripts + [drv], ev + "ROUTE DRV.outI TO E21.evIn\nROUTE STEP.cycleTime TO DRV.tick\n")





# ---------------------------------------------------------------------------------------------------------------------------
# 4. Browser surface seen from a Script: member kinds, simple getters, argument errors.
# ---------------------------------------------------------------------------------------------------------------------------

BROWSER_MEMBERS = """name version currentSpeed currentFrameRate description supportedComponents supportedProfiles currentScene baseURL
getName getVersion getCurrentSpeed getCurrentFrameRate getDescription setDescription getSupportedProfiles getSupportedComponents
getSupportedProfile getSupportedComponent addSupportedProfile updateSupportedProfile removeSupportedProfile addSupportedComponent
updateSupportedComponent removeSupportedComponent getProfile getComponent loadComponents createScene replaceWorld loadURL importDocument importJS
createX3DFromString createX3DFromURL createVrmlFromString createVrmlFromURL getBrowserOption setBrowserOption getBrowserProperty
getRenderingProperty getExecutionContext firstViewpoint lastViewpoint nextViewpoint previousViewpoint changeViewpoint bindViewpoint
viewAll beginUpdate endUpdate print println trace getScriptNode addBrowserCallback removeBrowserCallback getBrowserCallbacks addRoute deleteRoute
getCurrentTime getCurrentViewer getActiveViewpoint getActiveNavigationInfo getActiveLayer getWorldURL getBaseURL getBrowserOptions
getBrowserProperties getRenderingProperties getAvailableViewers getViewer setViewer initialized shutdown resize
isContextLost getPrintBuffer getCanvas getElement getName2 setBackgroundColor setDescription2 getLocalStorage getNotification getContextMenu
getBrowserTime getBrowserTimings getFrameRate getEnvironmentTexture dispose toString toVRMLString toXMLString toJSONString addEvent
getTypeName getType getId getNodeName getFieldDefinitions getField hasField getFields getUserDefinedFields""".split()

P_BROWSER = [(f"typeof Browser.{n}", f"return typeof Browser .{n};") for n in BROWSER_MEMBERS]
P_BROWSER += [
    ("name", "return Browser .name;"),
    ("version", "return Browser .version;"),
    ("getName()", "return Browser .getName ();"),
    ("getVersion()", "return Browser .getVersion ();"),
    ("description", "return Browser .description;"),
    ("setDescription", "Browser .description = 'abc'; var a = Browser .description; Browser .setDescription (5); return [a, Browser .getDescription (), Browser .description];"),
    ("description set undefined", "Browser .description = undefined; return Browser .description;"),
    ("currentSpeed", "return [Browser .currentSpeed, Browser .getCurrentSpeed ()];"),
    ("currentFrameRate", "return [Browser .currentFrameRate, Browser .getCurrentFrameRate ()];"),
    ("getCurrentTime type", "return typeof Browser .getCurrentTime ();"),
    ("getCurrentTime stable", "var a = Browser .getCurrentTime (), b = Browser .getCurrentTime (); return a === b;"),
    ("baseURL", "return Browser .baseURL .length > 0;"),
    ("currentScene", "return [Browser .currentScene .constructor .name, Browser .currentScene .isScene];"),
    ("currentScene eq", "return Browser .currentScene === Browser .currentScene;"),
    ("getExecutionContext", "return Browser .getExecutionContext () === Browser .currentScene;"),
    ("getScriptNode", "return Browser .getScriptNode () === SELF;"),
    ("getScriptNode name", "return Browser .getScriptNode () .getNodeName ();"),
    ("print returns", "return Browser .print ();"),
    ("println returns", "return Browser .println ();"),
    ("assign Browser.name", "Browser .name = 'x'; return Browser .name;"),
    ("assign Browser.foo", "Browser .foo = 1; return Browser .foo;"),
    ("delete Browser.foo", "delete Browser .foo; return typeof Browser .foo;"),
    ("Browser typeof", "return [typeof Browser, Browser === Browser, Browser === globalThis .Browser];"),
    ("Browser ctor", "return Browser .constructor .name;"),
    ("Browser toString", "return String (Browser);"),
    ("createVrmlFromString", "var n = Browser .createVrmlFromString ('Transform { translation 1 2 3 } Group { }'); return [n .constructor .name, n .length, n [0] .getNodeTypeName (), n [0] .translation];"),
    ("createVrmlFromString empty", "return Browser .createVrmlFromString ('') .length;"),
    ("createVrmlFromString bad", "return Browser .createVrmlFromString ('Nonsense { }') .length;"),
    ("createVrmlFromString number", "return Browser .createVrmlFromString (5) .length;"),
    ("createVrmlFromString null", "return Browser .createVrmlFromString (null) .length;"),
    ("createVrmlFromString undefined", "return Browser .createVrmlFromString () .length;"),
    ("createVrmlFromString DEF", "var n = Browser .createVrmlFromString ('DEF Q Transform { }'); return [n [0] .getNodeName (), Browser .currentScene .getNamedNode ('Q') ? 'named in scene' : 'not named'];"),
    ("createVrmlFromString proto", "var n = Browser .createVrmlFromString ('PROTO PP [ ] { Group { } } PP { }'); return n .length + ':' + n [0] .getNodeTypeName ();"),
    ("createVrmlFromString v2 header", "var n = Browser .createVrmlFromString ('#VRML V2.0 utf8\\nGroup { }'); return n .length;"),
    ("createVrmlFromString x3d header", "var n = Browser .createVrmlFromString ('#X3D V3.3 utf8\\nPROFILE Immersive\\nGroup { }'); return n .length;"),
    ("createVrmlFromString route", "var n = Browser .createVrmlFromString ('DEF A Transform { } DEF B Transform { } ROUTE A.translation_changed TO B.set_translation'); return n .length + ':' + Browser .currentScene .routes .length;"),
    ("createVrmlFromString script", "var n = Browser .createVrmlFromString ('Script { inputOutput SFInt32 x 5 url \"ecmascript:\" }'); return n [0] .x;"),
    ("createX3DFromString type", "return Browser .createX3DFromString ('Group { }') .constructor .name;"),
    ("createX3DFromURL args", "return Browser .createX3DFromURL ();"),
    ("createX3DFromURL string", "return Browser .createX3DFromURL ('x.wrl');"),
    ("createVrmlFromURL args", "return Browser .createVrmlFromURL ();"),
    ("createVrmlFromURL badnode", "return Browser .createVrmlFromURL (new MFString ('x.wrl'), null, 'set_children');"),
    ("createVrmlFromURL badevent", "return Browser .createVrmlFromURL (new MFString ('x.wrl'), SELF, 'nope');"),
    ("createVrmlFromURL notinput", "return Browser .createVrmlFromURL (new MFString ('x.wrl'), SELF, 'out');"),
    ("createVrmlFromURL wrongtype", "return Browser .createVrmlFromURL (new MFString ('x.wrl'), SELF, 'evIn');"),
    ("getBrowserOption missing", "return Browser .getBrowserOption ('Nope');"),
    ("setBrowserOption missing", "Browser .setBrowserOption ('Nope', 1); return 'ok';"),
    ("getBrowserProperty missing", "return Browser .getBrowserProperty ('Nope');"),
    ("getRenderingProperty missing", "return Browser .getRenderingProperty ('Nope');"),
    ("getBrowserOption Shading", "return Browser .getBrowserOption ('Shading');"),
    ("setBrowserOption Shading", "Browser .setBrowserOption ('Shading', 'FLAT'); var r = Browser .getBrowserOption ('Shading'); Browser .setBrowserOption ('Shading', 'GOURAUD'); return r;"),
    ("setBrowserOption bad value", "Browser .setBrowserOption ('Shading', 5); return Browser .getBrowserOption ('Shading');"),
    ("getBrowserProperty Initialized", "return typeof Browser .getBrowserProperty ('Initialized');"),
    ("getRenderingProperty Renderer", "return typeof Browser .getRenderingProperty ('Renderer');"),
    ("getProfile missing", "return Browser .getProfile ('Nope');"),
    ("getProfile Full", "return Browser .getProfile ('Full') .name;"),
    ("getComponent missing", "return Browser .getComponent ('Nope');"),
    ("getComponent Core", "var c = Browser .getComponent ('Core'); return c .name + ':' + c .level;"),
    ("getComponent level", "var c = Browser .getComponent ('Core', 1); return c .name + ':' + c .level;"),
    ("getComponent bad level", "return Browser .getComponent ('Core', 9);"),
    ("changeViewpoint missing", "Browser .changeViewpoint ('Nope'); return 'ok';"),
    ("firstViewpoint", "Browser .firstViewpoint (); return 'ok';"),
    ("beginUpdate/endUpdate", "Browser .beginUpdate (); Browser .endUpdate (); return 'ok';"),
    ("viewAll", "return Browser .viewAll ();"),
    ("addRoute args", "Browser .addRoute (); return 'ok';"),
    ("addRoute node", "Browser .addRoute (SELF, 'out', SELF, 'out'); return 'ok';"),
    ("print nested", "return typeof print;"),
    ("NULL TRUE FALSE", "return [NULL, TRUE, FALSE];"),
    ("X3DConstants misc", "return [X3DConstants .inputOnly, X3DConstants .outputOnly, X3DConstants .initializeOnly, X3DConstants .inputOutput, X3DConstants .SFNode, X3DConstants .MFNode, X3DConstants .SFTime, X3DConstants .X3DNode, X3DConstants .Script, X3DConstants .Transform, X3DConstants .X3DUrlObject];"),
    ("X3DConstants keys", "return Object .keys (X3DConstants) .length;"),
    ("X3DConstants states", "return [X3DConstants .NOT_STARTED_STATE, X3DConstants .IN_PROGRESS_STATE, X3DConstants .COMPLETE_STATE, X3DConstants .FAILED_STATE, X3DConstants .INITIALIZED_EVENT, X3DConstants .SHUTDOWN_EVENT, X3DConstants .CONNECTION_ERROR, X3DConstants .BROWSER_EVENT, X3DConstants .INITIALIZED_ERROR];"),
    ("X3DConstants node types", "return [X3DConstants .X3DChildNode, X3DConstants .X3DBindableNode, X3DConstants .X3DSensorNode, X3DConstants .X3DScriptNode, X3DConstants .X3DExecutionContext, X3DConstants .X3DScene, X3DConstants .X3DProtoDeclaration];"),
    ("X3DConstants frozen", "X3DConstants .foo = 1; return [typeof X3DConstants .foo, Object .isFrozen (X3DConstants)];"),
]

register("scr-browser", P_BROWSER, open_prefixes=("typeof Browser.", "X3DConstants", "Browser ctor", "Browser toString", "createVrmlFromURL", "createX3DFromURL", "getBrowserProperty Initialized", "getRenderingProperty Renderer", "getExecutionContext", "getScriptNode"))




# ---------------------------------------------------------------------------------------------------------------------------
# 1d. Timers and the clock seen by a Script. Timers are armed from tick handlers only: the oracle page installs its virtual timer queue
#     after the world has loaded, and a timer armed earlier would be a real one.
# ---------------------------------------------------------------------------------------------------------------------------


@custom("scr-timers")
def _():
    a = lscript("A", """  inputOnly SFTime tick
  outputOnly SFInt32 outI
  inputOnly SFInt32 inI""", r"""
var n = 0, ids = [];
function stamp () { return Math .round (Date .now () - globalThis .T0) + 'ms/' + Math .round ((Browser .getCurrentTime () - globalThis .T0 / 1000) * 1000) + 'ms'; }
function tick (v, t) {
  n++;
  if (n == 2) {
    globalThis .T0 = Date .now ();
    ids .push (setTimeout (function () { log ('t30 ' + stamp ()); }, 30));
    ids .push (setTimeout (function (a, b) { log ('t10 args ' + a + ',' + b + ' ' + stamp ()); }, 10, 'x', 2));
    ids .push (setTimeout (function () { log ('t0 ' + stamp ()); }));
    ids .push (setTimeout (function () { log ('t10b ' + stamp ()); }, 10));
    ids .push (setTimeout (function () { log ('cleared'); }, 20));
    clearTimeout (ids [4]);
    clearTimeout (999);
    clearTimeout (undefined);
    clearTimeout ('x');
    log ('ids ' + ids .join (','));
    log ('typeof id ' + typeof ids [0]);
  }
  if (n == 3) {
    setTimeout (function () {
      log ('outer ' + stamp ());
      setTimeout (function () { log ('inner ' + stamp ()); }, 5);
      outI = 7;
    }, 40);
    setTimeout (function () { throw new Error ('timer boom'); }, 15);
    setTimeout (function () { log ('after throw ' + stamp ()); }, 16);
  }
  if (n == 5) {
    setTimeout (function () { log ('negative ' + stamp ()); }, -50);
    setTimeout (function () { log ('NaN ' + stamp ()); }, NaN);
    setTimeout (function () { log ('string delay ' + stamp ()); }, '25');
    setTimeout (function () { log ('big ' + stamp ()); }, 1e9);
  }
  if (n == 7) { log ('now ' + stamp ()); }
}
function inI (v) { log ('inI ' + v + ' ' + stamp ()); }
""")
    return lifecycle_world("scr timers", [a], "ROUTE A.outI TO A.inI\nROUTE STEP.cycleTime TO A.tick\n")





# ---------------------------------------------------------------------------------------------------------------------------
# 1e. Script url forms and fallback. Each source appends to the shared list directly (it is its own global code).
# ---------------------------------------------------------------------------------------------------------------------------

def L(msg):
    return "(globalThis .LOG = globalThis .LOG || []) .push ('" + msg + "');"


GOOD = lambda n: f"{L(n + ' top')} function initialize () {{ {L(n + ' init')} }} function shutdown () {{ {L(n + ' shutdown')} }}"
B64 = lambda s: __import__("base64").b64encode(s.encode()).decode()
PCT = lambda s: __import__("urllib.parse", fromlist=["quote"]).quote(s, safe="")

URL_CASES = [
    ("U01", f'"javascript:{GOOD("U01")}"', ""),
    ("U02", f'"vrmlscript:{GOOD("U02")}"', ""),
    ("U03", f'"ecmascript:{GOOD("U03")}"', ""),
    ("U04", f'"data:text/javascript,{PCT(GOOD("U04"))}"', ""),
    ("U05", f'"data:application/javascript;base64,{B64(GOOD("U05"))}"', ""),
    ("U06", f'[ "assets/scr-missing.js" "ecmascript:{GOOD("U06")}" ]', ""),
    ("U07", f'[ "ecmascript:var = (" "ecmascript:{GOOD("U07")}" ]', ""),
    ("U08", '"assets/scr-ok.js"', ""),
    ("U09", f'[ "assets/scr-syntax.js" "ecmascript:{GOOD("U09")}" ]', ""),
    ("U10", "[ ]", ""),
    ("U11", '"ecmascript:"', ""),
    ("U12", f'[ "foo:bar" "ecmascript:{GOOD("U12")}" ]', ""),
    ("U13", f'[ "ecmascript:{GOOD("U13a")}" "ecmascript:{GOOD("U13b")}" ]', ""),
    ("U14", '"ecmascript: // comment only"', ""),
    ("U15", f'[ "ecmascript:{L("U15 a")} throw new Error (\'u15\');" "ecmascript:{GOOD("U15b")}" ]', ""),
    ("U16", f'[ "assets/scr-throw.js" "ecmascript:{GOOD("U16")}" ]', ""),
    ("U17", f'"ecmascript:{GOOD("U17a")}"', "  inputOnly SFTime tick\n"),
    ("U18", f'"ecmascript:{GOOD("U18")}"', "  inputOnly SFTime tick\n  load FALSE\n"),
    ("U19", f'"ecmascript:{GOOD("U19")} // trailing comment"', ""),
    ("U20", f'"ecmascript:\n{GOOD("U20")}\n"', ""),
    ("U21", f'"  ecmascript:{GOOD("U21")}"', ""),
    ("U22", f'"ECMAScript:{GOOD("U22")}"', ""),
    ("U23", f'"ecmascript: {GOOD("U23")}"', ""),
    ("U24", f'[ "ecmascript:{GOOD("U24a")}" "assets/scr-ok.js" ]', ""),
    ("U25", f'"data:,{PCT(GOOD("U25"))}"', ""),
    ("U26", f'"data:text/plain,{PCT(GOOD("U26"))}"', ""),
    ("U27", '"assets/scr-ok.js#frag"', ""),
    ("U28", f'[ "javascript:{GOOD("U28")}" "assets/scr-ok.js" ]', ""),
]

URL_DRIVER = r"""
var n = 0;
function tick () {
  n++;
  var u = Browser .currentScene .getNamedNode ('U17');
  var w = Browser .currentScene .getNamedNode ('U18');
  if (n == 3) { u .url = new MFString ("ecmascript:" + "(globalThis .LOG = globalThis .LOG || []) .push ('U17b top'); function initialize () { (globalThis .LOG = globalThis .LOG || []) .push ('U17b init'); }"); log ('U17 url set'); }
  if (n == 5) { w .load = true; log ('U18 load true'); }
  if (n == 7) { u .url = new MFString (); log ('U17 url emptied'); }
  if (n == 9) { u .url = new MFString ("ecmascript:" + "(globalThis .LOG = globalThis .LOG || []) .push ('U17c init'); throw 1;", "ecmascript:" + "function initialize () { (globalThis .LOG = globalThis .LOG || []) .push ('U17d init'); }"); log ('U17 url fallback'); }
}
"""


def url_world(title, skip, sort):
    scripts = []
    for name, url, extra in URL_CASES:
        if name in skip:
            continue

        flds = extra.replace("load FALSE\n", "")
        body = f'DEF {name} Script {{\n{flds}  url {url}\n'
        if "load FALSE" in extra:
            body += "  load FALSE\n"
        body += "}\n"
        scripts.append(body)
    drv = lscript("DRV", "  inputOnly SFTime tick", URL_DRIVER)
    return lifecycle_world(title, scripts + [drv], "ROUTE STEP.cycleTime TO DRV.tick\n", sort=sort)


# Content only (sorted): which candidate won, top-level and initialize runs, shutdown on url change. Syntax errors and urls that need a fetch (files,
# base64 data urls: X_ITE finishes them frames after the port does) are in the -open world
# (V8 and QuickJS word them differently).
@custom("scr-url")
def _():
    return url_world("scr url", ("U05", "U06", "U07", "U08", "U09", "U16", "U27"), True)


@custom("scr-url-order-open")
def _():
    return url_world("scr url order", (), False)





# ---------------------------------------------------------------------------------------------------------------------------
# 2. MetadataSet and the X3DMetadataObject nodes (MetadataBoolean, MetadataDouble, MetadataFloat, MetadataInteger, MetadataString, MetadataSet).
# ---------------------------------------------------------------------------------------------------------------------------

META_TYPES = ["MetadataBoolean", "MetadataDouble", "MetadataFloat", "MetadataInteger", "MetadataString", "MetadataSet"]

META_NODES = """
DEF G Group {
  metadata DEF MS MetadataSet {
    name "root"
    reference "http://example.org/ref"
    value [
      DEF MB MetadataBoolean { name "b" reference "rb" value [ TRUE FALSE TRUE ] }
      DEF MD MetadataDouble { name "d" value [ 1.5 2.25 -0.125 1e300 ] }
      DEF MF MetadataFloat { name "f" value [ 0.1 0.2 3.4e38 ] }
      DEF MI MetadataInteger { name "i" value [ 1 -2 2147483647 ] }
      DEF MST MetadataString { name "s" value [ "hello" "wor\\"ld" "" ] }
      DEF MNEST MetadataSet {
        name "nested"
        value [
          DEF MNB MetadataBoolean { name "nb" value TRUE }
          USE MB
          MetadataSet { name "empty" }
        ]
        metadata MetadataString { name "meta of meta" value "m" }
      }
    ]
  }
}
DEF T Transform { metadata USE MS }
DEF W WorldInfo { title "t" metadata MetadataString { name "w" value [ "x" ] } }
"""

P_META = [
    ("type names", "var o = []; ['MB', 'MD', 'MF', 'MI', 'MST', 'MNEST', 'MS', 'MNB'] .forEach (function (n) { var x = Browser .currentScene .getNamedNode (n); o .push (n + ':' + x .getNodeTypeName ()); }); return o;"),
    ("MB fields", "var x = Browser .currentScene .getNamedNode ('MB'); return [x .name, x .reference, x .value, x .value .length, x .value [1], x .value .constructor .name, x .metadata];"),
    ("MD fields", "var x = Browser .currentScene .getNamedNode ('MD'); return [x .name, x .reference, x .value, x .value [3], x .value .length];"),
    ("MF fields", "var x = Browser .currentScene .getNamedNode ('MF'); return [x .value [0], x .value [1], x .value [2]];"),
    ("MI fields", "var x = Browser .currentScene .getNamedNode ('MI'); return [x .value [0], x .value [1], x .value [2]];"),
    ("MST fields", "var x = Browser .currentScene .getNamedNode ('MST'); return [x .value [0], x .value [1], x .value [2], x .value .length];"),
    ("MS fields", "var x = Browser .currentScene .getNamedNode ('MS'); return [x .name, x .reference, x .value .length, x .value [0] .getNodeTypeName (), x .value [5] .getNodeName (), x .value .constructor .name];"),
    ("MNEST value", "var x = Browser .currentScene .getNamedNode ('MNEST'); return [x .value .length, x .value [0] .getNodeName (), x .value [1] .getNodeName (), x .value [1] === Browser .currentScene .getNamedNode ('MB'), x .value [2] .name, x .metadata .name, x .metadata .value];"),
    ("MNB single value", "return Browser .currentScene .getNamedNode ('MNB') .value;"),
    ("T.metadata", "var x = Browser .currentScene .getNamedNode ('T'); return [x .metadata === Browser .currentScene .getNamedNode ('MS'), x .metadata .name];"),
    ("W.metadata", "var x = Browser .currentScene .getNamedNode ('W'); return [x .metadata .getNodeTypeName (), x .metadata .name, x .metadata .value, x .title];"),
    ("G.metadata", "var x = Browser .currentScene .getNamedNode ('G'); return x .metadata .value .length;"),
    ("field defs", "var o = []; META_TYPES .forEach (function (n) { var x = Browser .currentScene .createNode (n); o .push (n + ':' + Array .from (x .getFieldDefinitions (), function (d) { return d .name + '/' + d .accessType + '/' + d .dataType; }) .join (',')); }); return o;"),
    ("defaults", "var o = []; META_TYPES .forEach (function (n) { var x = Browser .currentScene .createNode (n); o .push (n + ':' + [JSON .stringify (x .name), JSON .stringify (x .reference), x .value .length, x .metadata].join ('|')); }); return o;"),
    ("toString defaults", "var o = []; META_TYPES .forEach (function (n) { var x = Browser .currentScene .createNode (n); o .push (x .toString () + '/' + x .toVRMLString () .replace (/\\s+/g, ' ')); }); return o;"),
    ("xml defaults", "var o = []; META_TYPES .forEach (function (n) { var x = Browser .currentScene .createNode (n); o .push (x .toXMLString () .replace (/\\s+/g, ' ')); }); return o;"),
    ("json defaults", "var o = []; META_TYPES .forEach (function (n) { var x = Browser .currentScene .createNode (n); o .push (x .toJSONString () .replace (/\\s+/g, ' ')); }); return o;"),
    ("xml MS", "return Browser .currentScene .getNamedNode ('MS') .toXMLString ();"),
    ("json MS", "return Browser .currentScene .getNamedNode ('MS') .toJSONString ();"),
    ("vrml MS", "return Browser .currentScene .getNamedNode ('MS') .toVRMLString ();"),
    ("vrml T", "return Browser .currentScene .getNamedNode ('T') .toVRMLString ();"),
    ("xml T", "return Browser .currentScene .getNamedNode ('T') .toXMLString ();"),
    ("set MB.value array", "var x = Browser .currentScene .getNamedNode ('MB'); x .value = [true, false]; return [x .value, x .value .length];"),
    ("set MB.value numbers", "var x = Browser .currentScene .getNamedNode ('MB'); x .value = [0, 1, 2, -1, NaN]; return x .value;"),
    ("set MB.value new MFBool", "var x = Browser .currentScene .getNamedNode ('MB'); x .value = new MFBool (true, false); return x .value;"),
    ("MB push", "var x = Browser .currentScene .getNamedNode ('MB'); x .value .push (true, 0, 'x'); return x .value;"),
    ("set MD.value", "var x = Browser .currentScene .getNamedNode ('MD'); x .value = [0.1, 1/3, NaN, Infinity, -0]; return x .value;"),
    ("set MF.value", "var x = Browser .currentScene .getNamedNode ('MF'); x .value = [0.1, 1/3, 1e39, -1e-50]; return x .value;"),
    ("set MI.value", "var x = Browser .currentScene .getNamedNode ('MI'); x .value = [1.9, -1.9, 4294967296, 2147483648, '12', 'x', null, true]; return x .value;"),
    ("set MST.value", "var x = Browser .currentScene .getNamedNode ('MST'); x .value = ['a', 1, null, undefined, true, {}]; return x .value;"),
    ("set MST.value single", "var x = Browser .currentScene .getNamedNode ('MST'); x .value = new MFString ('only'); return [x .value, x .value .length];"),
    ("set name", "var x = Browser .currentScene .getNamedNode ('MB'); x .name = 'renamed'; x .reference = 5; return [x .name, x .reference, x .getNodeName ()];"),
    ("set name undefined", "var x = Browser .currentScene .getNamedNode ('MB'); x .name = undefined; return JSON .stringify (x .name);"),
    ("MS.value reassign", "var x = Browser .currentScene .getNamedNode ('MS'); var a = x .value [0]; x .value = [x .value [1], a]; return [x .value .length, x .value [0] .name, x .value [1] .name];"),
    ("MS.value push new", "var x = Browser .currentScene .getNamedNode ('MS'); var n = Browser .createVrmlFromString ('MetadataInteger { name \"pushed\" value 5 }') [0]; x .value .push (n); return [x .value .length, x .value [x .value .length - 1] .name, x .value [x .value .length - 1] .value];"),
    ("MS.value non-metadata node", "var x = Browser .currentScene .getNamedNode ('MS'); var n = Browser .currentScene .getNamedNode ('T'); x .value .push (n); return [x .value .length, x .value [x .value .length - 1] .getNodeTypeName ()];"),
    ("MS.value null", "var x = Browser .currentScene .getNamedNode ('MS'); x .value .push (null); return [x .value .length, x .value [x .value .length - 1]];"),
    ("MS.value remove", "var x = Browser .currentScene .getNamedNode ('MS'); x .value .length = 2; return [x .value .length, x .value [1] .name];"),
    ("MS.value clear", "var x = Browser .currentScene .getNamedNode ('MS'); x .value = []; return x .value .length;"),
    ("cycle", "var x = Browser .currentScene .getNamedNode ('MNEST'); x .value .push (x); return [x .value .length, x .value [x .value .length - 1] === x];"),
    ("cycle print", "return Browser .currentScene .getNamedNode ('MNEST') .toVRMLString () .length > 0;"),
    ("scene print", "return Browser .currentScene .toVRMLString () .length;"),
    ("T.metadata = other", "var x = Browser .currentScene .getNamedNode ('T'); x .metadata = Browser .currentScene .getNamedNode ('MST'); return [x .metadata .name, x .metadata === Browser .currentScene .getNamedNode ('MST')];"),
    ("T.metadata = Transform", "var x = Browser .currentScene .getNamedNode ('T'); x .metadata = Browser .currentScene .getNamedNode ('G'); return x .metadata .getNodeTypeName ();"),
    ("T.metadata = null", "var x = Browser .currentScene .getNamedNode ('T'); x .metadata = null; return x .metadata;"),
    ("new MetadataString via createVrml", "var n = Browser .createVrmlFromString ('MetadataString { name \"n\" value [\"a\" \"b\"] }') [0]; return [n .name, n .value .length, n .getNodeTypeName (), n .toString ()];"),
    ("createVrml with metadata", "var n = Browser .createVrmlFromString ('Group { metadata MetadataSet { name \"k\" value MetadataBoolean { name \"z\" value TRUE } } }') [0]; return [n .metadata .name, n .metadata .value .length, n .metadata .value [0] .value];"),
    ("createX3D with metadata 3.3", "var s = Browser .createX3DFromString ? 1 : 0; return s;"),
    ("MetadataReference option", "return typeof Browser .getBrowserOption ('MetadataReference');"),
    ("MetadataReference value", "return JSON .stringify (Browser .getBrowserOption ('MetadataReference'));"),
    ("scene setMetaData string", "var s = Browser .currentScene; s .setMetaData ('t/k', 'v'); return [s .getMetaData ('t/k'), s .getMetaDatas () .length];"),
    ("this.metadata", "return SELF .metadata;"),
    ("getField metadata", "return SELF .getField ('metadata') .getTypeName ();"),
    ("instanceof", "var x = Browser .currentScene .getNamedNode ('MB'); return [x instanceof SFNode, x .getNodeType () .length > 0];"),
    ("getNodeType values", "return Browser .currentScene .getNamedNode ('MB') .getNodeType ();"),
    ("USE count", "var s = Browser .currentScene; return s .getNamedNode ('MS') === s .getNamedNode ('T') .metadata;"),
    ("name conflicts", "var n = Browser .createVrmlFromString ('DEF MB MetadataBoolean { }'); return [n .length, n [0] .getNodeName ()];"),
]


class _Meta:  # the META_TYPES list is a JavaScript global of the probes
    pass


register("meta-nodes", P_META, open_prefixes=("instanceof", "getNodeType values"), pre="var META_TYPES = " + str(META_TYPES) + ";", nodes=META_NODES)


# getNodeType () returns X3DConstants numbers that depend on the order in which X_ITE registered the node types while loading components.
OPEN_META = ("instanceof", "getNodeType values")





# Metadata through the parsers and printers: each snippet goes through Browser.createX3DFromString; `out` lists its VRML, XML and JSON texts.
META_SNIPPETS = [
    "#X3D V3.3 utf8\nPROFILE Immersive\nGroup { metadata MetadataSet { name \"a\" value [ MetadataString { name \"s\" value \"x\" } MetadataInteger { name \"i\" value [1 2] } ] } }",
    "#X3D V4.0 utf8\nPROFILE Full\nGroup { metadata MetadataSet { name \"a\" value [ MetadataString { name \"s\" value \"x\" } ] } }",
    "#X3D V4.1 utf8\nPROFILE Full\nGroup { metadata MetadataSet { name \"a\" value [ MetadataBoolean { name \"b\" value [ TRUE ] } MetadataFloat { name \"f\" value 1.5 } ] } }",
    "#VRML V2.0 utf8\nGroup { metadata MetadataSet { name \"a\" } }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nDEF M MetadataSet { name \"top\" }\nGroup { metadata USE M }\nTransform { metadata USE M }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nGroup { metadata MetadataSet { value [ MetadataSet { value [ MetadataSet { name \"deep\" value MetadataDouble { name \"d\" value [ 1e-7 123456789.123 ] } } ] } ] } }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nMetadataSet { name \"root level\" }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nMetadataString { name \"n\" value [ \"a\" \"b\" \"c\" ] }\nMetadataInteger { name \"m\" }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nGroup { metadata MetadataString { name \"x\" value [ \"line1\\nline2\" \"tab\\t\" \"quote\\\"\" \"back\\\\slash\" ] } }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nWorldInfo { metadata MetadataSet { name \"w\" } title \"t\" info [ \"i\" ] }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nMetadataBoolean { name \"b\" value [ TRUE, FALSE, TRUE ] reference \"r\" metadata MetadataString { name \"m\" } }",
    "<X3D profile='Immersive' version='3.3'><Scene><Group><MetadataSet name='a'><MetadataString name='s' value='\"x\" \"y\"'/><MetadataInteger name='i' value='1 2 3'/></MetadataSet></Group></Scene></X3D>",
    "<X3D profile='Full' version='4.0'><Scene><Group><MetadataSet containerField='metadata' name='a'><MetadataBoolean containerField='value' name='b' value='true false'/></MetadataSet></Group></Scene></X3D>",
    "<X3D profile='Immersive' version='3.3'><Scene><MetadataSet name='sceneLevel'/></Scene></X3D>",
    "<X3D profile='Immersive' version='3.3'><head><meta name='k' content='v'/></head><Scene><Group><MetadataSet name='a' reference='r' DEF='D'/></Group><Transform><MetadataSet USE='D'/></Transform></Scene></X3D>",
    "{\"X3D\":{\"@profile\":\"Full\",\"@version\":\"4.0\",\"Scene\":{\"-children\":[{\"Group\":{\"-metadata\":{\"MetadataSet\":{\"@name\":\"a\",\"-value\":[{\"MetadataString\":{\"@name\":\"s\",\"@value\":[\"x\"]}},{\"MetadataDouble\":{\"@name\":\"d\",\"@value\":[1.5,2]}}]}}}}]}}}",
    "#X3D V3.3 utf8\nPROFILE Immersive\nGroup { metadata MetadataSet { name \"a\" value [ DEF D MetadataString { name \"dup\" } USE D USE D ] } }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nGroup { metadata MetadataString { name \"s\" value [ ] } }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nGroup { metadata MetadataDouble { name \"d\" value [ NaN Infinity -Infinity ] } }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nGroup { metadata MetadataInteger { name \"i\" value [ 0x10 -0x1 2147483648 ] } }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nGroup { metadata MetadataFloat { name \"f\" value [ 1e39 1e-46 -0 ] } }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nGroup { metadata Transform { } }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nMetadataSet { value [ Group { } ] }",
    "#X3D V3.3 utf8\nPROFILE Core\nGroup { metadata MetadataSet { name \"core only\" } }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nPROTO P [ ] { MetadataSet { name \"in proto\" value MetadataString { name \"x\" } } }\nP { }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nDEF A MetadataString { name \"a\" }\nDEF T Group { }\nROUTE A.value_changed TO T.set_children",
    "#X3D V3.3 utf8\nPROFILE Immersive\nDEF A MetadataString { name \"a\" }\nDEF B MetadataString { name \"b\" }\nROUTE A.value_changed TO B.set_value",
    "#X3D V3.3 utf8\nPROFILE Immersive\nDEF A MetadataInteger { name \"a\" }\nDEF B MetadataString { name \"b\" }\nROUTE A.value_changed TO B.set_value",
    "#X3D V3.3 utf8\nPROFILE Immersive\nDEF A MetadataSet { name \"a\" }\nDEF B MetadataSet { name \"b\" }\nROUTE A.value_changed TO B.set_value\nROUTE A.metadata_changed TO B.set_metadata",
]


META_PARSE_SCRIPT = r"""DEF OUT Script {
  inputOutput MFString out [ ]
  url "ecmascript:
    var SNIPPETS = @SNIPPETS@;
    var res = [];
    function strip (s) { return String (s) .replace (/https?:\\/\\/[^\\/]+\\//g, ''); }
    function put (i, v) { res [i] = i + ': ' + v; var o = []; for (var k = 0; k < SNIPPETS .length; ++ k) o .push (k in res ? res [k] : k + ': ...'); out = new MFString (... o); }
    function initialize () {
      SNIPPETS .forEach (function (src, i) {
        var p;
        try { p = Browser .createX3DFromString (src); } catch (e) { put (i, 'THROW ' + strip (e .message)); return; }
        p .then (function (sc) {
          var a, b, c;
          try { a = sc .toVRMLString (); } catch (e) { a = 'VRML ERR ' + e .message; }
          try { b = sc .toXMLString (); } catch (e) { b = 'XML ERR ' + e .message; }
          try { c = sc .toJSONString (); } catch (e) { c = 'JSON ERR ' + e .message; }
          put (i, a + '\\n--XML--\\n' + b + '\\n--JSON--\\n' + c);
        }, function (e) { put (i, 'REJ ' + strip (e && e .message)); });
      });
    }
  "
}
"""


# X_ITE names the node a PROTO instance's field value holds (`meta DEF _1 MetadataString`) when the proto body refers to the field; the port prints it
# without a name (the printer-names difference of parse-def-proto in the scene-sai report).
META_OPEN_SNIPPETS = [
    "#X3D V3.3 utf8\nPROFILE Immersive\nPROTO P [ inputOutput SFNode meta NULL ] { Group { metadata IS meta } }\nP { meta MetadataString { name \"pm\" } }",
]


@custom("meta-parse-open")
def _():
    return meta_parse_world(META_OPEN_SNIPPETS)


@custom("meta-parse")
def _():
    return meta_parse_world(META_SNIPPETS)


def meta_parse_world(snips):
    js = json.dumps(snips, ensure_ascii=True)
    snippets = js.replace("\\", "\\\\").replace('"', '\\"')
    return HEAD + "META \"title\" \"meta parse\"\n" + META_PARSE_SCRIPT.replace("@SNIPPETS@", snippets)




@custom("meta-events")
def _():
    nodes = """DEF MB MetadataBoolean { name "b" }
DEF MD MetadataDouble { name "d" }
DEF MF MetadataFloat { name "f" }
DEF MI MetadataInteger { name "i" }
DEF MS MetadataString { name "s" }
DEF MSET MetadataSet { name "set" }
DEF MSET2 MetadataSet { name "set2" value [ DEF MC MetadataString { name "child" value "c0" } ] }
DEF HOLDER Group { metadata USE MSET }
"""
    d = lscript("D", """  inputOnly SFTime tick
  outputOnly MFBool outB
  outputOnly MFDouble outD
  outputOnly MFFloat outF
  outputOnly MFInt32 outI
  outputOnly MFString outS
  outputOnly MFNode outN
  outputOnly SFNode outMeta""", r"""
var n = 0;
function tick () {
  n++;
  var sc = Browser .currentScene;
  if (n == 2) { outB = [true, false]; outD = [1.5, 2.5]; outF = [0.1]; outI = [7, 8, 9]; outS = ['x', 'y']; log ('sent values'); }
  if (n == 3) { outN = [sc .getNamedNode ('MC')]; outMeta = sc .getNamedNode ('MC'); log ('sent nodes'); }
  if (n == 4) { sc .getNamedNode ('MS') .value = ['direct']; sc .getNamedNode ('MB') .name = 'renamed'; log ('direct writes'); }
  if (n == 5) { sc .getNamedNode ('MSET2') .value = []; log ('MSET2 cleared'); }
  if (n == 6) { sc .getNamedNode ('MI') .value .push (10); sc .getNamedNode ('MSET2') .value .push (sc .getNamedNode ('MS')); log ('pushes'); }
  if (n == 7) { sc .getNamedNode ('HOLDER') .metadata = sc .getNamedNode ('MSET2'); log ('holder metadata'); }
}
""")
    a = lscript("A", """  inputOnly MFBool inB
  inputOnly MFDouble inD
  inputOnly MFFloat inF
  inputOnly MFInt32 inI
  inputOnly MFString inS
  inputOnly MFNode inSetValue
  inputOnly SFNode inMeta
  inputOnly SFString inName
  inputOnly SFString inRef""", r"""
function inB (v) { log ('inB ' + v .length + ' ' + v); }
function inD (v) { log ('inD ' + v); }
function inF (v) { log ('inF ' + v); }
function inI (v) { log ('inI ' + v); }
function inS (v) { log ('inS ' + v); }
function inSetValue (v) { log ('inSetValue ' + v .length + ' ' + (v .length ? v [0] .name : '')); }
function inMeta (v) { log ('inMeta ' + (v ? v .name : v)); }
function inName (v) { log ('inName ' + v); }
function inRef (v) { log ('inRef ' + v); }
""")
    routes = """ROUTE D.outB TO MB.set_value
ROUTE D.outD TO MD.set_value
ROUTE D.outF TO MF.set_value
ROUTE D.outI TO MI.set_value
ROUTE D.outS TO MS.set_value
ROUTE D.outN TO MSET.set_value
ROUTE D.outMeta TO MSET.set_metadata
ROUTE MB.value_changed TO A.inB
ROUTE MD.value_changed TO A.inD
ROUTE MF.value_changed TO A.inF
ROUTE MI.value_changed TO A.inI
ROUTE MS.value_changed TO A.inS
ROUTE MSET.value_changed TO A.inSetValue
ROUTE MSET2.value_changed TO A.inSetValue
ROUTE MSET.metadata_changed TO A.inMeta
ROUTE MB.name_changed TO A.inName
ROUTE MB.reference_changed TO A.inRef
ROUTE STEP.cycleTime TO D.tick
"""
    return lifecycle_world("meta events", [d, a], routes, nodes=nodes)





# ---------------------------------------------------------------------------------------------------------------------------
# 3. Field types and field math. Every probe builds its operands itself; results show the field's text and its full-precision components.
# ---------------------------------------------------------------------------------------------------------------------------

def vec_probes(T, n):
    comps = "xyzw"[:n]
    ones = ", ".join(str(i + 1) for i in range(n))
    twos = ", ".join(str((i + 1) * 2.5) for i in range(n))
    P = []
    p = P.append
    p((f"{T} new ()", f"return new {T} ();"))
    p((f"{T} new (1)", f"return new {T} (1);"))
    p((f"{T} new (all)", f"return new {T} ({ones});"))
    p((f"{T} new (extra)", f"return new {T} ({ones}, 99);"))
    p((f"{T} new (strings)", f"return new {T} ({', '.join(repr(str(i + 1) + '.5') for i in range(n))});"))
    p((f"{T} new (bad)", f"return new {T} ('a', null, undefined, true);"))
    p((f"{T} new (NaN)", f"return new {T} (NaN, Infinity, -Infinity, -0);"))
    p((f"{T} new (object)", f"return new {T} ({{ x: 1 }});"))
    p((f"{T} call without new", f"return {T} ({ones});"))
    p((f"{T} float rounding", f"var v = new {T} (0.1, 0.2, 0.3, 0.4); return [v .x, v [0], String (v), v .toString (), v .toVRMLString ()];"))
    p((f"{T} big", f"return new {T} (1e300, 1e-300, 123456789.123456789, 5e-324);"))
    for i, c in enumerate(comps):
        p((f"{T} get {c}", f"return [new {T} ({ones}) .{c}, new {T} ({ones}) [{i}]];"))
        p((f"{T} set {c}", f"var v = new {T} ({ones}); v .{c} = 7.5; v [{(i + 1) % n}] = 8.25; return v;"))
    p((f"{T} set string", f"var v = new {T} ({ones}); v .x = '9'; v [1] = 'abc'; return v;"))
    p((f"{T} set null/undefined", f"var v = new {T} ({ones}); v .x = null; v [1] = undefined; return v;"))
    p((f"{T} index out", f"var v = new {T} ({ones}); return [v [{n}], v [-1], v ['1'], v ['x'], v .nope, v [1.5]];"))
    p((f"{T} index write out", f"var v = new {T} ({ones}); v [{n}] = 5; v .nope = 6; v [-1] = 7; return [v, v [{n}], v .nope, v [-1]];"))
    p((f"{T} keys", f"var v = new {T} ({ones}); return [Object .keys (v), JSON .stringify (v), Object .getOwnPropertyNames (v) .length, 'x' in v, 0 in v, {n} in v];"))
    p((f"{T} iterate", f"var v = new {T} ({ones}); var o = []; for (var x of v) o .push (x); return [o, Array .from (v), [... v], Math .max (... v)];"))
    p((f"{T} for in", f"var v = new {T} ({ones}); var o = []; for (var k in v) o .push (k); return o;"))
    p((f"{T} concat", f"var v = new {T} ({ones}); return ['' + v, v + '', `${{v}}`, v + 1, v * 2, +v, String (v), v .valueOf () === v, typeof v];"))
    p((f"{T} identity", f"var v = new {T} ({ones}); var w = v .copy (); return [v === w, v .equals (w), v .equals (v), w .constructor === v .constructor, v instanceof {T}, v instanceof X3DField];"))
    p((f"{T} equals other type", f"var v = new {T} ({ones}); return v .equals (new {T.replace('f', 'd') if T.endswith('f') else T.replace('d', 'f')} ({ones}));"))
    p((f"{T} getName/Type", f"var v = new {T} ({ones}); return [v .getTypeName (), v .getType (), v .getName (), v .getAccessType (), v .isDefaultValue (), new {T} () .isDefaultValue ()];"))
    p((f"{T} toStrings", f"var v = new {T} ({twos}); return [v .toString (), v .toVRMLString (), v .toXMLString (), v .toJSONString ()];"))
    p((f"{T} toStrings nan", f"var v = new {T} (NaN, Infinity, -Infinity, 1e21); return [v .toString (), v .toXMLString (), v .toJSONString ()];"))
    p((f"{T} fromVRMLString", f"var v = new {T} (); return typeof v .fromString;"))
    # math
    A = f"new {T} ({ones})"
    B = f"new {T} ({twos})"
    Z = f"new {T} ()"
    for m in ["add", "subtract", "multVec", "divVec", "min", "max", "reflect", "distance", "dot"]:
        p((f"{T} {m}", f"return {A} .{m} ({B});"))
        p((f"{T} {m} zero", f"return {A} .{m} ({Z});"))
        p((f"{T} {m} nan", f"return {A} .{m} (new {T} (NaN));"))
        p((f"{T} {m} missing", f"return {A} .{m} ();"))
        p((f"{T} {m} wrongtype", f"return {A} .{m} (5);"))
    for m in ["abs", "inverse", "length", "negate", "normalize"]:
        p((f"{T} {m}", f"return {A} .{m} ();"))
        p((f"{T} {m} zero", f"return {Z} .{m} ();"))
        p((f"{T} {m} neg", f"return new {T} ({', '.join(('-' if i % 2 == 0 else '') + str(i + 1.5) for i in range(n))}) .{m} ();"))
        p((f"{T} {m} nan", f"return new {T} (NaN, 1, 2, 3) .{m} ();"))
    for m in ["multiply", "divide"]:
        p((f"{T} {m}", f"return {A} .{m} (2.5);"))
        p((f"{T} {m} zero", f"return {A} .{m} (0);"))
        p((f"{T} {m} string", f"return {A} .{m} ('2');"))
        p((f"{T} {m} nan", f"return {A} .{m} (NaN);"))
        p((f"{T} {m} missing", f"return {A} .{m} ();"))
        p((f"{T} {m} vector", f"return {A} .{m} ({B});"))
    p((f"{T} clamp", f"return {A} .clamp (new {T} (0.5), new {T} (1.5));"))
    p((f"{T} clamp swapped", f"return {A} .clamp (new {T} (1.5), new {T} (0.5));"))
    p((f"{T} clamp missing", f"return {A} .clamp (new {T} (0.5));"))
    p((f"{T} lerp", f"return {A} .lerp ({B}, 0.25);"))
    p((f"{T} lerp t=0/1/2", f"return [{A} .lerp ({B}, 0), {A} .lerp ({B}, 1), {A} .lerp ({B}, 2), {A} .lerp ({B}, -1)];"))
    p((f"{T} lerp nan", f"return {A} .lerp ({B}, NaN);"))
    p((f"{T} lerp missing t", f"return {A} .lerp ({B});"))
    if n == 3:
        p((f"{T} cross", f"return {A} .cross ({B});"))
        p((f"{T} cross axes", f"return new {T} (1, 0, 0) .cross (new {T} (0, 1, 0));"))
        p((f"{T} cross parallel", f"return {A} .cross ({A});"))
        p((f"{T} cross missing", f"return {A} .cross ();"))
    else:
        p((f"{T} cross", f"return typeof {A} .cross;"))
    p((f"{T} chain", f"return {A} .add ({B}) .multiply (0.5) .normalize () .negate ();"))
    p((f"{T} non mutating", f"var v = {A}; var w = v .add ({B}); return [v, w, v === w];"))
    p((f"{T} result type", f"var r = [{A} .add ({B}), {A} .abs (), {A} .copy ()]; return r .map (function (x) {{ return x .constructor .name + '/' + x .getTypeName (); }});"))
    p((f"{T} add other precision", f"return {A} .add (new {T.replace('f', 'd') if T.endswith('f') else T.replace('d', 'f')} ({twos}));"))
    p((f"{T} add array", f"return {A} .add ([{twos}]);"))
    p((f"{T} add object", f"return {A} .add ({{ x: 1, y: 2, z: 3, w: 4 }});"))
    p((f"{T} getValue", f"var v = {A}; var g = v .getValue (); return [typeof g, g === v, g && g .constructor && g .constructor .name];"))
    p((f"{T} setValue", f"var v = {A}; v .setValue ({B}); return v;"))
    p((f"{T} assign", f"var v = {A}; v .assign ({B}); return v;"))
    p((f"{T} setValue other precision", f"var v = {A}; v .setValue (new {T.replace('f', 'd') if T.endswith('f') else T.replace('d', 'f')} ({twos})); return v;"))
    return P


VEC_OPEN = ()
for _T, _n in [("SFVec2f", 2), ("SFVec3f", 3), ("SFVec4f", 4), ("SFVec2d", 2), ("SFVec3d", 3), ("SFVec4d", 4)]:
    # Open: a class called without `new` returns undefined in X_ITE (a TypeError here); `fromString` and the internal `Vector` class of getValue are not modeled.
    register(f"fld-{_T.lower()}", vec_probes(_T, _n), open_prefixes=tuple(f"{_T} {s}" for s in ("call without new", "fromVRMLString", "getValue")))





def color_probes(T, n):
    comps = "rgba"[:n]
    P = []
    p = P.append
    vals = "0.2, 0.4, 0.6, 0.8"[: 5 * n - 2]
    A = f"new {T} ({vals})"
    p((f"{T} new ()", f"return new {T} ();"))
    p((f"{T} new (vals)", f"return {A};"))
    p((f"{T} new (out of range)", f"return new {T} (2, -1, 1e10, NaN);"))
    p((f"{T} new (strings)", f"return new {T} ('0.5', 'x', null, undefined);"))
    for i, c in enumerate(comps):
        p((f"{T} get/set {c}", f"var v = {A}; v .{c} = 0.9; v [{(i + 1) % n}] = -3; return [v .{c}, v [{i}], v];"))
    p((f"{T} index out", f"var v = {A}; return [v [{n}], v ['r'], v .nope, v .x];"))
    p((f"{T} keys", f"var v = {A}; return [Object .keys (v), JSON .stringify (v), Array .from (v), [... v]];"))
    p((f"{T} for in", f"var o = []; for (var k in {A}) o .push (k); return o;"))
    p((f"{T} equals/copy", f"var v = {A}; var w = v .copy (); return [v .equals (w), v === w, w .constructor .name, v .isDefaultValue (), new {T} () .isDefaultValue ()];"))
    p((f"{T} toStrings", f"var v = {A}; return [v .toString (), v .toVRMLString (), v .toXMLString (), v .toJSONString ()];"))
    p((f"{T} getHSV", f"return [{A} .{'getHSV' if n == 3 else 'getHSVA'} (), new {T} (1, 0, 0{', 1' if n == 4 else ''}) .{'getHSV' if n == 3 else 'getHSVA'} (), new {T} (0, 0, 0{', 0' if n == 4 else ''}) .{'getHSV' if n == 3 else 'getHSVA'} (), new {T} (0.5, 0.5, 0.5{', 0.5' if n == 4 else ''}) .{'getHSV' if n == 3 else 'getHSVA'} ()];"))
    setter = "setHSV" if n == 3 else "setHSVA"
    p((f"{T} setHSV", f"var v = new {T} (); v .{setter} (1, 0.5, 0.5); return v;"))
    p((f"{T} setHSV edge", f"var v = new {T} (); v .{setter} (7, 2, -1); var w = new {T} (); w .{setter} (0, 0, 0.7); var u = new {T} (); u .{setter} (NaN, 0.5, 0.5); return [v, w, u];"))
    p((f"{T} setHSV partial", f"var v = new {T} (); v .{setter} (1); return v;"))
    p((f"{T} setHSV roundtrip", f"var o = []; for (var h = 0; h < 6.3; h += 0.7) {{ var v = new {T} (); v .{setter} (h, 0.6, 0.8); o .push (v .toString ()); }} return o;"))
    p((f"{T} lerp", f"return [{A} .lerp (new {T} (0.9, 0.1, 0.5{', 0.3' if n == 4 else ''}), 0.3), {A} .lerp ({A}, 0.5), {A} .lerp (new {T} (), 1)];"))
    p((f"{T} lerp missing", f"return {A} .lerp ();"))
    p((f"{T} lerp t range", f"var b = new {T} (0.9, 0.1, 0.5{', 0.3' if n == 4 else ''}); return [{A} .lerp (b, -1), {A} .lerp (b, 2), {A} .lerp (b, NaN)];"))
    p((f"{T} linearToSRGB", f"return [{A} .linearToSRGB (), new {T} (0, 1, 2{', 0.5' if n == 4 else ''}) .linearToSRGB ()];"))
    p((f"{T} sRGBToLinear", f"return [{A} .sRGBToLinear (), new {T} (0, 1, 2{', 0.5' if n == 4 else ''}) .sRGBToLinear ()];"))
    p((f"{T} setValue", f"var v = new {T} (); v .setValue ({A}); return v;"))
    p((f"{T} setValue other", f"var v = new {T} (); v .setValue (new {'SFColorRGBA' if n == 3 else 'SFColor'} (0.1, 0.2, 0.3, 0.4)); return v;"))
    return P


# Open: X_ITE stores JavaScript undefined for a color set from the other color class.
register("fld-sfcolor", color_probes("SFColor", 3), open_prefixes=("SFColor setValue other",))
register("fld-sfcolorrgba", color_probes("SFColorRGBA", 4), open_prefixes=("SFColorRGBA setValue other",))


def rot_probes():
    T = "SFRotation"
    P = []
    p = P.append
    R = "new SFRotation (0, 1, 0, 1.2)"
    p(("SFRotation new ()", "return new SFRotation ();"))
    p(("SFRotation new (xyza)", f"return {R};"))
    p(("SFRotation new (partial)", "return [new SFRotation (1), new SFRotation (1, 2), new SFRotation (1, 2, 3)];"))
    p(("SFRotation new (unnormalized)", "return new SFRotation (0, 3, 4, 1);"))
    p(("SFRotation new (zero axis)", "return new SFRotation (0, 0, 0, 1);"))
    p(("SFRotation new (vec, angle)", "return new SFRotation (new SFVec3f (1, 0, 0), 0.5);"))
    p(("SFRotation new (vec, angle unnorm)", "return new SFRotation (new SFVec3f (0, 0, 2), 0.5);"))
    p(("SFRotation new (vec, vec)", "return new SFRotation (new SFVec3f (1, 0, 0), new SFVec3f (0, 1, 0));"))
    p(("SFRotation new (vec, vec parallel)", "return [new SFRotation (new SFVec3f (1, 0, 0), new SFVec3f (1, 0, 0)), new SFRotation (new SFVec3f (1, 0, 0), new SFVec3f (-1, 0, 0))]  ;"))
    p(("SFRotation new (vec, vec zero)", "return new SFRotation (new SFVec3f (), new SFVec3f (0, 1, 0));"))
    p(("SFRotation new (vec3d)", "return new SFRotation (new SFVec3d (0, 1, 0), new SFVec3d (0, 0, 1));"))
    p(("SFRotation new (matrix)", "return new SFRotation (new SFMatrix3f (0, 1, 0, -1, 0, 0, 0, 0, 1));"))
    p(("SFRotation new (matrix d)", "return new SFRotation (new SFMatrix3d (0, 1, 0, -1, 0, 0, 0, 0, 1));"))
    p(("SFRotation new (NaN)", "return new SFRotation (NaN, 0, 1, Infinity);"))
    p(("SFRotation new (strings)", "return new SFRotation ('0', '1', '0', '0.5');"))
    p(("SFRotation new (bad vec)", "return new SFRotation ({x: 1}, 2);"))
    for i, c in enumerate(["x", "y", "z", "angle"]):
        p((f"SFRotation set {c}", f"var v = {R}; v .{c} = 0.25; v [{(i + 1) % 4}] = 0.75; return [v, v .{c}, v [{i}]];"))
    p(("SFRotation index", f"var v = {R}; return [v [4], v .w, v .nope, Object .keys (v), JSON .stringify (v), Array .from (v)];"))
    p(("SFRotation toStrings", f"var v = {R}; return [v .toString (), v .toVRMLString (), v .toXMLString (), v .toJSONString ()];"))
    p(("SFRotation getAxis", f"return [{R} .getAxis (), new SFRotation (0, 3, 4, 1) .getAxis (), new SFRotation (0, 0, 0, 1) .getAxis ()];"))
    p(("SFRotation setAxis", f"var v = {R}; v .setAxis (new SFVec3f (0, 0, 2)); return v;"))
    p(("SFRotation setAxis bad", f"var v = {R}; v .setAxis ({{x: 1, y: 2, z: 3}}); return v;"))
    p(("SFRotation setAxis missing", f"var v = {R}; v .setAxis (); return v;"))
    p(("SFRotation getMatrix", f"return [{R} .getMatrix (), new SFRotation () .getMatrix (), new SFRotation (0, 0, 1, 3.14159265358979) .getMatrix ()];"))
    p(("SFRotation setMatrix", f"var v = new SFRotation (); v .setMatrix (new SFMatrix3f (0, 1, 0, -1, 0, 0, 0, 0, 1)); return v;"))
    p(("SFRotation setMatrix 4", f"var v = new SFRotation (); v .setMatrix (new SFMatrix4f ()); return v;"))
    p(("SFRotation setMatrix missing", f"var v = new SFRotation (); v .setMatrix (); return v;"))
    p(("SFRotation setMatrix zero", f"var v = new SFRotation (); v .setMatrix (new SFMatrix3f (0, 0, 0, 0, 0, 0, 0, 0, 0)); return v;"))
    p(("SFRotation setMatrix scaled", f"var v = new SFRotation (); v .setMatrix (new SFMatrix3f (2, 0, 0, 0, 2, 0, 0, 0, 2)); return v;"))
    p(("SFRotation getQuaternion", f"var q = {R} .getQuaternion (); return [q .constructor .name, q .toString (), q .x, q .w];"))
    p(("SFRotation setQuaternion", f"var v = new SFRotation (); v .setQuaternion (new SFQuaternion (0, 0.5, 0, 0.5)); return v;"))
    p(("SFRotation setQuaternion identity", f"var v = {R}; v .setQuaternion (new SFQuaternion (0, 0, 0, 1)); return v;"))
    p(("SFRotation setQuaternion missing", f"var v = new SFRotation (); v .setQuaternion (); return v;"))
    p(("SFRotation inverse", f"return [{R} .inverse (), new SFRotation () .inverse (), new SFRotation (0, 0, 0, 1) .inverse ()];"))
    p(("SFRotation multiply", f"return [{R} .multiply (new SFRotation (1, 0, 0, 0.7)), {R} .multiply ({R} .inverse ()), {R} .multiply (new SFRotation ())];"))
    p(("SFRotation multiply missing", f"return {R} .multiply ();"))
    p(("SFRotation multiply vec", f"return {R} .multiply (new SFVec3f (1, 2, 3));"))
    p(("SFRotation multVec", f"return [{R} .multVec (new SFVec3f (1, 2, 3)), {R} .multVec (new SFVec3d (1, 2, 3)), new SFRotation () .multVec (new SFVec3f (4, 5, 6))];"))
    p(("SFRotation multVec missing", f"return {R} .multVec ();"))
    p(("SFRotation multVec wrong", f"return {R} .multVec (new SFVec2f (1, 2));"))
    p(("SFRotation slerp", f"var b = new SFRotation (1, 0, 0, 2.5); return [{R} .slerp (b, 0), {R} .slerp (b, 0.5), {R} .slerp (b, 1), {R} .slerp (b, 2), {R} .slerp (b, -1), {R} .slerp (b, NaN)];"))
    p(("SFRotation slerp opposite", f"return [{R} .slerp (new SFRotation (0, -1, 0, -1.2), 0.5), {R} .slerp ({R} .inverse (), 0.5), {R} .slerp (new SFRotation (0, 1, 0, 1.2 + 6.283185307179586), 0.5)];"))
    p(("SFRotation slerp missing", f"return {R} .slerp ();"))
    p(("SFRotation slerp no t", f"return {R} .slerp (new SFRotation (1, 0, 0, 1));"))
    p(("SFRotation straighten", f"return [{R} .straighten (new SFVec3f (0, 1, 0)), new SFRotation (1, 0, 0, 0.3) .straighten (new SFVec3f (0, 1, 0)), new SFRotation (0, 0, 1, 1) .straighten (new SFVec3f (0, 0, 1))];"))
    p(("SFRotation straighten missing", f"return {R} .straighten ();"))
    p(("SFRotation equals/copy", f"var v = {R}; var w = v .copy (); return [v .equals (w), v .equals (new SFRotation (0, 1, 0, 1.2 + 1e-12)), v .isDefaultValue (), new SFRotation () .isDefaultValue (), new SFRotation (0, 0, 1, 0) .isDefaultValue (), new SFRotation (1, 0, 0, 0) .isDefaultValue ()];"))
    p(("SFRotation static", "return [typeof SFRotation .typeName, SFRotation .typeName, typeof SFRotation .fromMatrix];"))
    p(("SFRotation setValue", f"var v = new SFRotation (); v .setValue ({R}); return v;"))
    p(("SFRotation setValue quaternion", f"var v = new SFRotation (); v .setValue (new SFQuaternion (0, 0.5, 0, 0.5)); return v;"))
    return P


# Open: X_ITE builds new SFRotation (matrix) with NaN (it reads the matrix as the first number); setMatrix takes a 4x4 matrix too; straighten and multVec of a foreign vector are not ported.
register("fld-sfrotation", rot_probes(), open_prefixes=("SFRotation static", "SFRotation new (matrix", "SFRotation setMatrix 4", "SFRotation multVec wrong", "SFRotation straighten"))


def quat_probes():
    P = []
    p = P.append
    Q = "new SFQuaternion (0, 0.6, 0, 0.8)"
    p(("SFQuaternion new ()", "return new SFQuaternion ();"))
    p(("SFQuaternion new (xyzw)", f"return {Q};"))
    p(("SFQuaternion new (partial)", "return [new SFQuaternion (1), new SFQuaternion (1, 2), new SFQuaternion (1, 2, 3)];"))
    p(("SFQuaternion new (NaN)", "return new SFQuaternion (NaN, 1, 2, 3);"))
    for i, c in enumerate("xyzw"):
        p((f"SFQuaternion set {c}", f"var v = {Q}; v .{c} = 0.25; v [{(i + 1) % 4}] = 0.75; return [v, v .{c}, v [{i}]];"))
    p(("SFQuaternion aliases", f"var v = {Q}; return [v .i, v .j, v .k, v .real];"))
    p(("SFQuaternion keys", f"var v = {Q}; return [Object .keys (v), JSON .stringify (v), Array .from (v)];"))
    p(("SFQuaternion toStrings", f"var v = {Q}; return [v .toString (), v .toVRMLString (), v .toXMLString (), v .toJSONString ()];"))
    for m in ["add", "subtract", "dot", "multLeft", "multRight"]:
        p((f"SFQuaternion {m}", f"return {Q} .{m} (new SFQuaternion (0.1, 0.2, 0.3, 0.4));"))
        p((f"SFQuaternion {m} missing", f"return {Q} .{m} ();"))
    for m in ["conjugate", "inverse", "length", "negate", "normalize", "getMatrix"]:
        p((f"SFQuaternion {m}", f"return [{Q} .{m} (), new SFQuaternion () .{m} (), new SFQuaternion (1, 2, 3, 4) .{m} ()];"))
    for m in ["multiply", "divide"]:
        p((f"SFQuaternion {m}", f"return [{Q} .{m} (2), {Q} .{m} (0), {Q} .{m} ('3'), {Q} .{m} ()];"))
    p(("SFQuaternion multQuatVec", f"return [{Q} .multQuatVec (new SFVec3f (1, 2, 3)), {Q} .multQuatVec (new SFVec3d (1, 2, 3))];"))
    p(("SFQuaternion multVecQuat", f"return [{Q} .multVecQuat (new SFVec3f (1, 2, 3)), {Q} .multVecQuat (new SFVec3d (1, 2, 3))];"))
    p(("SFQuaternion multQuatVec missing", f"return {Q} .multQuatVec ();"))
    p(("SFQuaternion slerp", f"var b = new SFQuaternion (0.6, 0, 0, 0.8); return [{Q} .slerp (b, 0), {Q} .slerp (b, 0.5), {Q} .slerp (b, 1), {Q} .slerp (b, 2), {Q} .slerp (b, NaN)];"))
    p(("SFQuaternion slerp opposite", f"return {Q} .slerp ({Q} .negate (), 0.5);"))
    p(("SFQuaternion slerp zero", "return new SFQuaternion () .slerp (new SFQuaternion (), 0.5);"))
    p(("SFQuaternion setMatrix", f"var v = new SFQuaternion (); v .setMatrix (new SFMatrix3f (0, 1, 0, -1, 0, 0, 0, 0, 1)); return v;"))
    p(("SFQuaternion setMatrix 4", f"var v = new SFQuaternion (); v .setMatrix (new SFMatrix4f ()); return v;"))
    p(("SFQuaternion equals/copy", f"var v = {Q}; var w = v .copy (); return [v .equals (w), v === w, v .isDefaultValue (), new SFQuaternion () .isDefaultValue (), new SFQuaternion (0, 0, 0, 1) .isDefaultValue ()];"))
    p(("SFQuaternion setValue", f"var v = new SFQuaternion (); v .setValue ({Q}); return v;"))
    p(("SFQuaternion static", "return [typeof SFQuaternion .ZERO, typeof SFQuaternion .IDENTITY, typeof SFQuaternion .fromMatrix];"))
    return P


register("fld-sfquaternion", quat_probes(), open_prefixes=("SFQuaternion static", "SFQuaternion setMatrix 4"))





def matrix_probes(T, n):
    size = n * n
    P = []
    p = P.append
    vals = ", ".join(str(round((i + 1) * 0.5 + (1 if i % 5 == 0 else 0), 3)) for i in range(size))
    ident = ", ".join("1" if i % (n + 1) == 0 else "0" for i in range(size))
    sing = ", ".join("0" for _ in range(size))
    sing2 = ", ".join(str(i % n + 1) for i in range(size))  # rows equal: singular
    A = f"new {T} ({vals})"
    I = f"new {T} ()"
    p((f"{T} new ()", f"return {I};"))
    p((f"{T} new (vals)", f"return {A};"))
    p((f"{T} new (partial)", f"return new {T} (5, 6);"))
    p((f"{T} new (bad)", f"return new {T} ('a', null, undefined, NaN);"))
    p((f"{T} new (matrix)", f"return new {T} (new {T} ({vals}));"))
    for i in (0, 1, size // 2, size - 1):
        p((f"{T} get/set [{i}]", f"var m = {A}; var old = m [{i}]; m [{i}] = 42.5; return [old, m [{i}], m];"))
    p((f"{T} index out", f"var m = {A}; return [m [{size}], m [-1], m ['0'], m .nope, m .length, Object .keys (m) .length];"))
    p((f"{T} index write out", f"var m = {A}; m [{size}] = 1; m .nope = 2; return [m, m [{size}], m .nope];"))
    p((f"{T} set string/null", f"var m = {A}; m [0] = 'x'; m [1] = null; m [2] = undefined; m [3] = true; return m;"))
    p((f"{T} iterate", f"var m = {A}; return [Array .from (m) .length, [... m] .join (), JSON .stringify (m) .length];"))
    p((f"{T} equals", f"var m = {A}; return [m .equals (m .copy ()), m .equals ({I}), {I} .isDefaultValue (), m .isDefaultValue (), m .copy () === m];"))
    p((f"{T} toStrings", f"var m = {A}; return [m .toString (), m .toVRMLString (), m .toXMLString (), m .toJSONString ()];"))
    p((f"{T} toStrings nan", f"var m = {A}; m [0] = NaN; m [1] = Infinity; m [2] = -0; m [3] = 1e21; return [m .toString (), m .toXMLString (), m .toJSONString ()];"))
    p((f"{T} determinant", f"return [{A} .determinant (), {I} .determinant (), new {T} ({sing}) .determinant (), new {T} ({sing2}) .determinant ()];"))
    p((f"{T} transpose", f"return [{A} .transpose (), {I} .transpose (), {A} .transpose () .transpose () .equals ({A})];"))
    p((f"{T} inverse", f"return [new {T} ({', '.join(str(x) for x in ([2,0,0,0,3,0,0,0,4] if n == 3 else [2,0,0,1,0,3,0,2,0,0,4,3,0,0,0,1]))}) .inverse (), {I} .inverse ()];"))
    p((f"{T} inverse singular", f"return [new {T} ({sing}) .inverse (), new {T} ({sing2}) .inverse ()];"))
    p((f"{T} inverse nan", f"var m = {A}; m [0] = NaN; return m .inverse ();"))
    p((f"{T} inverse product", f"var m = new {T} ({', '.join(str(x) for x in ([2,1,0,0,3,1,1,0,4] if n == 3 else [2,1,0,5,0,3,1,2,1,0,4,3,0,0,0,1]))}); return m .multLeft (m .inverse ());"))
    p((f"{T} multLeft", f"return [{A} .multLeft ({I}), {A} .multLeft ({A}), {I} .multLeft ({A})];"))
    p((f"{T} multRight", f"return [{A} .multRight ({I}), {A} .multRight ({A}), {I} .multRight ({A})];"))
    p((f"{T} mult missing", f"return {A} .multLeft ();"))
    p((f"{T} mult wrongtype", f"return {A} .multRight (new SFVec3f (1, 2, 3));"))
    p((f"{T} mult other precision", f"return {A} .multRight (new {T.replace('f', 'd') if T.endswith('f') else T.replace('d', 'f')} ({vals}));"))
    p((f"{T} mult nonassoc", f"var a = {A}; var b = new {T} ({ident}); b [1] = 2; return [a .multLeft (b), a .multRight (b)];"))
    if n == 3:
        p((f"{T} rotate", f"return [{I} .rotate (0.5), {I} .rotate (0), {A} .rotate (1), {I} .rotate (NaN), {I} .rotate ('1')];"))
        p((f"{T} skewX/Y", f"return [{I} .skewX (0.5), {I} .skewY (0.5), {A} .skewX (1), {I} .skewX (NaN)];"))
        p((f"{T} multVecMatrix", f"return [{A} .multVecMatrix (new SFVec2f (1, 2)), {A} .multMatrixVec (new SFVec2f (1, 2)), {A} .multVecMatrix (new SFVec2d (1, 2)), {A} .multDirMatrix (new SFVec2f (1, 2)), {A} .multMatrixDir (new SFVec2f (1, 2))];"))
        p((f"{T} multVecMatrix 3", f"return {A} .multVecMatrix (new SFVec3f (1, 2, 3));"))
        p((f"{T} multVecMatrix 3 missing", f"return {A} .multVecMatrix ();"))
        p((f"{T} translate", f"return typeof {A} .translate;"))
        p((f"{T} setTransform", f"var m = {I}; m .setTransform (new SFVec2f (3, 4), 0.5, new SFVec2f (2, 3), 0.25, new SFVec2f (1, 1)); return m;"))
        p((f"{T} setTransform partial", f"var m = {I}; m .setTransform (new SFVec2f (3, 4)); var n = {I}; n .setTransform (undefined, 1); var o = {I}; o .setTransform (); return [m, n, o];"))
        p((f"{T} setTransform scale only", f"var m = {I}; m .setTransform (undefined, undefined, new SFVec2f (2, 3)); return m;"))
        p((f"{T} getTransform", f"var m = {I}; m .setTransform (new SFVec2f (3, 4), 0.5, new SFVec2f (2, 3), 0.25, new SFVec2f (1, 1)); var t = new SFVec2f (), r = 0, s = new SFVec2f (), so = 0; return [m .getTransform (t, undefined, s), t, s];"))
    else:
        p((f"{T} translate", f"return [{I} .translate (new SFVec3f (1, 2, 3)), {A} .translate (new SFVec3f (1, 2, 3)), {I} .translate (new SFVec3d (1, 2, 3))];"))
        p((f"{T} rotate", f"return [{I} .rotate (new SFRotation (0, 1, 0, 0.5)), {A} .rotate (new SFRotation (1, 0, 0, 1)), {I} .rotate (new SFRotation ())];"))
        p((f"{T} scale", f"return [{I} .scale (new SFVec3f (2, 3, 4)), {A} .scale (new SFVec3f (2, 3, 4)), {I} .scale (new SFVec3f ())];"))
        p((f"{T} translate missing", f"return {I} .translate ();"))
        p((f"{T} rotate missing", f"return {I} .rotate ();"))
        p((f"{T} scale wrong", f"return {I} .scale (new SFVec2f (1, 2));"))
        p((f"{T} multVecMatrix", f"return [{A} .multVecMatrix (new SFVec3f (1, 2, 3)), {A} .multMatrixVec (new SFVec3f (1, 2, 3)), {A} .multDirMatrix (new SFVec3f (1, 2, 3)), {A} .multMatrixDir (new SFVec3f (1, 2, 3))];"))
        p((f"{T} multVecMatrix d", f"return [{A} .multVecMatrix (new SFVec3d (1, 2, 3)), {A} .multMatrixDir (new SFVec3d (1, 2, 3))];"))
        p((f"{T} multVecMatrix 4", f"return [{A} .multVecMatrix (new SFVec4f (1, 2, 3, 1)), {A} .multMatrixVec (new SFVec4f (1, 2, 3, 1)), {A} .multDirMatrix (new SFVec4f (1, 2, 3, 1))];"))
        p((f"{T} multVecMatrix missing", f"return {A} .multVecMatrix ();"))
        p((f"{T} multVecMatrix nan", f"return {A} .multVecMatrix (new SFVec3f (NaN, 0, 0));"))
        p((f"{T} perspective w", f"var m = {I}; m [11] = 1; m [15] = 0; return [m .multVecMatrix (new SFVec3f (1, 2, 3)), m .multMatrixVec (new SFVec3f (1, 2, 3))];"))
        p((f"{T} setTransform", f"var m = {I}; m .setTransform (new SFVec3f (1, 2, 3), new SFRotation (0, 1, 0, 0.5), new SFVec3f (2, 3, 4), new SFRotation (1, 0, 0, 0.25), new SFVec3f (1, 1, 1)); return m;"))
        p((f"{T} setTransform partial", f"var m = {I}; m .setTransform (new SFVec3f (1, 2, 3)); var n = {I}; n .setTransform (undefined, new SFRotation (0, 1, 0, 1)); var o = {I}; o .setTransform (); var q = {I}; q .setTransform (undefined, undefined, new SFVec3f (2, 3, 4)); return [m, n, o, q];"))
        p((f"{T} setTransform zero scale", f"var m = {I}; m .setTransform (undefined, undefined, new SFVec3f (0, 1, 1)); return m;"))
        p((f"{T} setTransform bad", f"var m = {I}; m .setTransform (5); return m;"))
        p((f"{T} getTransform", f"var m = {I}; m .setTransform (new SFVec3f (1, 2, 3), new SFRotation (0, 1, 0, 0.5), new SFVec3f (2, 3, 4), new SFRotation (1, 0, 0, 0.25), new SFVec3f (1, 1, 1)); var t = new SFVec3f (), r = new SFRotation (), s = new SFVec3f (), so = new SFRotation (); m .getTransform (t, r, s, so); return [t, r, s, so];"))
        p((f"{T} getTransform partial", f"var m = {A}; var t = new SFVec3f (); m .getTransform (t); var r = new SFRotation (); m .getTransform (undefined, r); var s = new SFVec3f (); m .getTransform (undefined, undefined, s); return [t, r, s];"))
        p((f"{T} getTransform singular", f"var m = new {T} ({sing}); var t = new SFVec3f (9, 9, 9), r = new SFRotation (), s = new SFVec3f (9, 9, 9); m .getTransform (t, r, s); return [t, r, s];"))
        p((f"{T} getTransform identity", f"var m = {I}; var t = new SFVec3f (9, 9, 9), r = new SFRotation (1, 0, 0, 1), s = new SFVec3f (9, 9, 9), so = new SFRotation (1, 0, 0, 1); m .getTransform (t, r, s, so); return [t, r, s, so];"))
        p((f"{T} getTransform center", f"var m = {I}; m .setTransform (new SFVec3f (1, 2, 3), new SFRotation (0, 1, 0, 0.5), new SFVec3f (2, 3, 4), new SFRotation (1, 0, 0, 0.25), new SFVec3f (1, 1, 1)); var t = new SFVec3f (), r = new SFRotation (), s = new SFVec3f (), so = new SFRotation (); m .getTransform (t, r, s, so, new SFVec3f (1, 1, 1)); return [t, r, s, so];"))
    p((f"{T} setValue", f"var m = {I}; m .setValue ({A}); return m;"))
    p((f"{T} setValue other precision", f"var m = {I}; m .setValue (new {T.replace('f', 'd') if T.endswith('f') else T.replace('d', 'f')} ({vals})); return m;"))
    p((f"{T} float rounding", f"var m = new {T} (0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9); return [m [0], m .toString (), m .toVRMLString ()];"))
    return P


for _T, _n in [("SFMatrix3f", 3), ("SFMatrix3d", 3), ("SFMatrix4f", 4), ("SFMatrix4d", 4)]:
    # Open: SFMatrix3.getTransform (Matrix3.factor) is not ported; a matrix multiplied by a foreign field gives NaN in X_ITE, a TypeError here.
    register(f"fld-{_T.lower()}", matrix_probes(_T, _n), open_prefixes=tuple(f"{_T} {s}" for s in ("mult wrongtype", "scale wrong") + (("getTransform",) if _n == 3 else ())))





MF_SAMPLES = {
    "MFBool": ("true", "false", "0", "'x'"),
    "MFInt32": ("1", "2", "3.7", "-4"),
    "MFFloat": ("1.5", "2.25", "0.1", "-4"),
    "MFDouble": ("1.5", "2.25", "0.1", "-4"),
    "MFTime": ("1.5", "2.25", "0.1", "-4"),
    "MFString": ("'a'", "'bb'", "''", "'d e'"),
    "MFVec2f": ("new SFVec2f (1, 2)", "new SFVec2f (3, 4)", "new SFVec2f (5, 6)", "new SFVec2f (7, 8)"),
    "MFVec3f": ("new SFVec3f (1, 2, 3)", "new SFVec3f (4, 5, 6)", "new SFVec3f (7, 8, 9)", "new SFVec3f (1, 0, 0)"),
    "MFVec3d": ("new SFVec3d (1, 2, 3)", "new SFVec3d (4, 5, 6)", "new SFVec3d (7, 8, 9)", "new SFVec3d (1, 0, 0)"),
    "MFVec4f": ("new SFVec4f (1, 2, 3, 4)", "new SFVec4f (5, 6, 7, 8)", "new SFVec4f (0, 0, 0, 1)", "new SFVec4f (1, 1, 1, 1)"),
    "MFColor": ("new SFColor (1, 0, 0)", "new SFColor (0, 1, 0)", "new SFColor (0, 0, 1)", "new SFColor (0.5, 0.5, 0.5)"),
    "MFColorRGBA": ("new SFColorRGBA (1, 0, 0, 1)", "new SFColorRGBA (0, 1, 0, 0.5)", "new SFColorRGBA (0, 0, 1, 0)", "new SFColorRGBA (1, 1, 1, 1)"),
    "MFRotation": ("new SFRotation (0, 1, 0, 1)", "new SFRotation (1, 0, 0, 2)", "new SFRotation ()", "new SFRotation (0, 0, 1, 3)"),
    "MFMatrix3f": ("new SFMatrix3f ()", "new SFMatrix3f (1, 2, 3, 4, 5, 6, 7, 8, 9)", "new SFMatrix3f (2, 0, 0, 0, 2, 0, 0, 0, 2)", "new SFMatrix3f ()"),
    "MFMatrix4f": ("new SFMatrix4f ()", "new SFMatrix4f (1, 2, 3, 4, 5, 6, 7, 8, 9, 10, 11, 12, 13, 14, 15, 16)", "new SFMatrix4f (2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 2, 0, 0, 0, 0, 1)", "new SFMatrix4f ()"),
}


def mf_probes(T):
    s = MF_SAMPLES[T]
    s0, s1, s2, s3 = s
    P = []
    p = P.append
    M = f"new {T} ({s0}, {s1}, {s2})"
    p((f"{T} new ()", f"return new {T} ();"))
    p((f"{T} new (elems)", f"return {M};"))
    p((f"{T} new (array)", f"return new {T} ([{s0}, {s1}]);"))
    p((f"{T} new (bad)", f"return new {T} (null, undefined, 'x', {{}});"))
    p((f"{T} length", f"var m = {M}; return [m .length, new {T} () .length];"))
    p((f"{T} index", f"var m = {M}; return [m [0], m [2], m [3], m [-1], m ['1'], m [1.5], m .nope];"))
    p((f"{T} index write", f"var m = {M}; m [1] = {s3}; return [m, m .length];"))
    p((f"{T} index write past end", f"var m = {M}; m [5] = {s3}; return [m, m .length];"))
    p((f"{T} index write bad", f"var m = {M}; m [1] = null; m [0] = undefined; return [m, m .length];"))
    p((f"{T} length set", f"var m = {M}; m .length = 1; var n = {M}; n .length = 5; var o = {M}; o .length = 0; return [m, n, o];"))
    p((f"{T} length set bad", f"var m = {M}; m .length = -1; return m .length;"))
    p((f"{T} length set nan", f"var m = {M}; m .length = 'x'; return m .length;"))
    p((f"{T} keys", f"var m = {M}; return [Object .keys (m), JSON .stringify (m), Array .from (m) .length, [... m] .length, Array .isArray (m), typeof m, m instanceof X3DArrayField];"))
    p((f"{T} for in", f"var o = []; for (var k in {M}) o .push (k); return o;"))
    p((f"{T} toStrings", f"var m = {M}; return [m .toString (), m .toVRMLString (), m .toXMLString (), m .toJSONString ()];"))
    p((f"{T} toStrings empty", f"var m = new {T} (); return [m .toString (), m .toVRMLString (), m .toXMLString (), m .toJSONString ()];"))
    p((f"{T} push", f"var m = {M}; var r = m .push ({s3}); var q = m .push ({s0}, {s1}); return [r, q, m];"))
    p((f"{T} push none", f"var m = {M}; return [m .push (), m .length];"))
    p((f"{T} pop", f"var m = {M}; var r = m .pop (); var e = new {T} () .pop (); return [r, e, m, m .length];"))
    p((f"{T} shift", f"var m = {M}; var r = m .shift (); var e = new {T} () .shift (); return [r, e, m, m .length];"))
    p((f"{T} unshift", f"var m = {M}; var r = m .unshift ({s3}, {s3}); return [r, m];"))
    p((f"{T} splice", f"var m = {M}; var r = m .splice (1, 1); return [r, m, m .length];"))
    p((f"{T} splice insert", f"var m = {M}; var r = m .splice (1, 0, {s3}, {s3}); return [r, m, m .length];"))
    p((f"{T} splice neg/over", f"var m = {M}; var r = m .splice (-1); var n = {M}; var q = n .splice (1, 100); var o = {M}; var u = o .splice (); return [r, m, q, n, u, o];"))
    p((f"{T} reverse", f"var m = {M}; var r = m .reverse (); return [r === m, m];"))
    p((f"{T} fill", f"var m = {M}; m .fill ({s3}); var n = {M}; n .fill ({s3}, 1, 2); return [m, n];"))
    p((f"{T} concat", f"var m = {M}; var r = m .concat (new {T} ({s3})); var q = m .concat ([{s3}, {s3}]); var o = m .concat ({s3}); return [r, q, o, m, r .constructor === m .constructor];"))
    p((f"{T} slice", f"var m = {M}; var r = m .slice (1); var q = m .slice (-1); var o = m .slice (0, 2); return [r, q, o, m, r === m, r .constructor === m .constructor];"))
    p((f"{T} indexOf", f"var m = {M}; return [m .indexOf ({s1}), m .indexOf ({s3}), m .includes ({s1}), m .lastIndexOf ({s0}), m .findIndex (function (x) {{ return true; }}), m .find (function (x) {{ return false; }})];"))
    p((f"{T} join", f"var m = {M}; return [m .join (), m .join ('-'), m .join ('')];"))
    p((f"{T} forEach", f"var m = {M}; var o = []; m .forEach (function (x, i, a) {{ o .push (i + ':' + (a === m) + ':' + typeof x); }}); return o;"))
    p((f"{T} map", f"var m = {M}; var r = m .map (function (x) {{ return x; }}); return [r, r .constructor .name];"))
    p((f"{T} filter", f"var m = {M}; var r = m .filter (function (x, i) {{ return i != 1; }}); return [r, r .length];"))
    p((f"{T} reduce", f"var m = {M}; return [m .reduce (function (a, x, i) {{ return a + i; }}, 0), m .every (function () {{ return true; }}, m .some (function () {{ return false; }}))];"))
    p((f"{T} sort", f"var m = {M}; m .sort (function (a, b) {{ return 0; }}); return m;"))
    p((f"{T} at/entries", f"var m = {M}; return [m .at (-1), m .at (5), Array .from (m .keys ()), Array .from (m .entries ()) .length];"))
    p((f"{T} equals/copy", f"var m = {M}; var c = m .copy (); return [m .equals (c), m === c, c .length, m .isDefaultValue (), new {T} () .isDefaultValue ()];"))
    p((f"{T} copy independent", f"var m = {M}; var c = m .copy (); c [0] = {s3}; c .push ({s0}); return [m, c];"))
    p((f"{T} setValue", f"var m = new {T} (); m .setValue ({M}); return m;"))
    p((f"{T} setValue array", f"var m = new {T} (); m .setValue ([{s0}, {s1}]); return m;"))
    p((f"{T} setValue empty", f"var m = {M}; m .setValue ([]); return m;"))
    p((f"{T} setValue self", f"var m = {M}; m .setValue (m); return m;"))
    p((f"{T} element alias", f"var m = {M}; var e = m [0]; if (e && typeof e === 'object' && e .getTypeName) {{ e .setValue ({s3}); }} return [e, m];"))
    p((f"{T} resize", f"var m = {M}; var r = typeof m .resize; return r;"))
    return P


for _T in MF_SAMPLES:
    # Open: garbage elements (null, wrong shape): X_ITE throws a TypeError or stores NaN, the port stores defaults; `length = 'x'` leaves the string in X_ITE (resize is handed the string); whether a retrieved element aliases the array (X_ITE returns the stored object; the port a view), and `resize`.
    register(f"fld-{_T.lower()}", mf_probes(_T), open_prefixes=(f"{_T} element alias", f"{_T} resize", f"{_T} length set nan", f"{_T} new (array)", f"{_T} new (bad)", f"{_T} index write bad"))




def main():
    names = sys.argv[1:] or list(defs)
    for n in names:
        d = defs[n]
        if "custom" in d:
            files = {n: d["custom"]()}
        else:
            op = [p for p in d["probes"] if d["open"] and p[0].startswith(d["open"])]
            base = [p for p in d["probes"] if p not in op]
            files = {n: battery(n, n.replace("-", " "), base, **d["kw"])}
            if op:
                files[n + "-open"] = battery(n + "-open", n.replace("-", " ") + " open", op, **d["kw"])
        for fname, text in files.items():
            open(os.path.join(out_dir, fname + ".wrl"), "w").write(text)
            print(fname, len(text))


if __name__ == "__main__":
    main()
