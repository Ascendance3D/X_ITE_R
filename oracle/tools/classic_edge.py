#!/usr/bin/env python3
"""Classic-encoding edge-case oracle baseline (REA ledger entry sub:parse.vrml-classic; pinned X_ITE in headless Chrome, no rendering).

  python3 -I oracle/tools/classic_edge.py cases
      writes the authored cases to /tmp/classic-edge/cases.json (for inspection).
  python3 -I oracle/tools/classic_edge.py xite
      one X_ITE run into /tmp/classic-edge/xite.json.
  python3 -I oracle/tools/classic_edge.py record
      records the cases THREE times with oracle/x_ite-classic-edge.mjs, requires the three recordings to be identical, and writes the
      first, with the input of each case, as oracle/classic-edge-baseline/classic-edge.json.

Every input is authored here (synthetic; no historical Cybertown or blaxxun content). A case is either a string (`text`, handed to
Browser.createX3DFromString: the parser's own behavior, with the page URL as its world URL) or the exact bytes (`b64`, served as a file
and loaded with Browser.createX3DFromURL: byte order marks, encodings, line endings, gzip). The recorder keeps the printed scene
(toVRMLString, toXMLString, toJSONString), the encoding, specification version, profile, components, units, meta data, the exact rejection
text and the console output of every level in print order. crates/x3d-parse/tests/classic_edge.rs compares the port with it.
Needs the ignored oracle/x_ite-pin/node_modules link and XITE_TESTS (oracle/README.md, "Recording from a worktree").
"""
import base64, gzip, hashlib, json, os, re, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
tmp = "/tmp/classic-edge"

V = "#VRML V2.0 utf8\n"
X = "#X3D V3.3 utf8\nPROFILE Full\n"
I = "#X3D V3.3 utf8\nPROFILE Immersive\n"

cases = []
seen = set()


def T(name, text):
    assert name not in seen, name
    seen.add(name)
    cases.append({"name": name, "text": text})


def B(name, data):
    assert name not in seen, name
    seen.add(name)
    cases.append({"name": name, "b64": base64.b64encode(data).decode()})


# --- headers and header statements -----------------------------------------------------------------------------------------------
for i, (h, body) in enumerate([
    ("#VRML V2.0 utf8", "Group { }"),
    ("#VRML V2.0 utf8 and a comment", "Group { }"),
    ("#VRML V2.0 UTF8", "Group { }"),
    ("#VRML V2.0 ascii", "Group { }"),
    ("#VRML V2.0", "Group { }"),
    ("#VRML V1.0 ascii", "Group { }"),
    ("#VRML V3.0 utf8", "Group { }"),
    ("#VRML  V2.0   utf8", "Group { }"),
    ("#vrml V2.0 utf8", "Group { }"),
    ("# VRML V2.0 utf8", "Group { }"),
    (" #VRML V2.0 utf8", "Group { }"),
    ("#VRML V2.0 utf8\t", "Group { }"),
    ("#X3D V3.0 utf8", "Group { }"),
    ("#X3D V3.1 utf8", "Group { }"),
    ("#X3D V3.2 utf8", "Group { }"),
    ("#X3D V3.3 utf8", "Group { }"),
    ("#X3D V4.0 utf8", "Group { }"),
    ("#X3D V4.1 utf8", "Group { }"),
    ("#X3D V9.9 utf8", "Group { }"),
    ("#X3D V3.3 ascii", "Group { }"),
    ("#X3D V3.3", "Group { }"),
    ("#X3D V3.3 utf8 trailing words", "Group { }"),
    ("#x3d V3.3 utf8", "Group { }"),
    ("#X3D v3.3 utf8", "Group { }"),
    ("#X3D V3 utf8", "Group { }"),
    ("#X3D V3.3.1 utf8", "Group { }"),
    ("#X3D  V3.3  utf8", "Group { }"),
    ("#X3D V3.3 utf8 X_ITE V16.5.0", "Group { }"),
    ("#X3D V3.3 utf8\r", "Group { }"),
    ("#VRML V2.0 utf8\r", "Group { }"),
]):
    T(f"hdr-{i:02d}", h + "\n" + body)

T("hdr-none", "Group { }")
T("hdr-none-comment", "# just a comment\nGroup { }")
T("hdr-crlf", "#VRML V2.0 utf8\r\nGroup {\r\n  children [ Group { } ]\r\n}\r\n")
T("hdr-cr-only", "#VRML V2.0 utf8\rGroup {\r  children [ Group { } ]\r}\r")
T("hdr-only", "#VRML V2.0 utf8")
T("hdr-only-nl", "#VRML V2.0 utf8\n")
T("hdr-x3d-only", "#X3D V3.3 utf8\nPROFILE Core")
T("hdr-empty", "")
T("hdr-space", "   \n\t\n  ")
T("hdr-comment-only", "# only a comment")
T("hdr-second-line-header", "\n#VRML V2.0 utf8\nGroup { }")
T("hdr-bom-text", "﻿#VRML V2.0 utf8\nGroup { }")
T("hdr-bom-x3d-text", "﻿#X3D V3.3 utf8\nPROFILE Core\nGroup { }")
T("hdr-header-in-the-middle", "Group { }\n#VRML V2.0 utf8\nGroup { }")

# --- PROFILE, COMPONENT, UNIT, META --------------------------------------------------------------------------------------------------
for p in ["Core", "Interchange", "CADInterchange", "Interactive", "MPEG-4", "Immersive", "Full", "Nonsense", "core", "IMMERSIVE", "MPEG4"]:
    T(f"prof-{p}", f"#X3D V3.3 utf8\nPROFILE {p}\nGroup {{ }}")

T("prof-missing", "#X3D V3.3 utf8\nGroup { }")
T("prof-twice", "#X3D V3.3 utf8\nPROFILE Immersive\nPROFILE Core\nGroup { }")
T("prof-late", "#X3D V3.3 utf8\nGroup { }\nPROFILE Immersive")
T("prof-bare", "#X3D V3.3 utf8\nPROFILE\nGroup { }")
T("prof-quoted", '#X3D V3.3 utf8\nPROFILE "Core"\nGroup { }')
T("prof-vrml-header", "#VRML V2.0 utf8\nPROFILE Core\nGroup { }")
T("prof-core-box", "#X3D V3.3 utf8\nPROFILE Core\nShape { geometry Box { } }")
T("prof-core-text", "#X3D V3.3 utf8\nPROFILE Core\nShape { geometry Text { string \"a\" } }")
T("prof-core-script", "#X3D V3.3 utf8\nPROFILE Core\nScript { url \"ecmascript:\" }")
T("prof-interchange-sound", "#X3D V3.3 utf8\nPROFILE Interchange\nSound { }")
T("prof-interchange-sphere-grid", "#X3D V3.3 utf8\nPROFILE Interchange\nShape { geometry Sphere { } }\nBox { }")
T("prof-immersive-nurbs", "#X3D V3.3 utf8\nPROFILE Immersive\nNurbsCurve { }")
T("prof-core-nurbs-component", "#X3D V3.3 utf8\nPROFILE Core\nCOMPONENT NURBS:1\nNurbsCurve { }")
T("prof-full-nurbs", "#X3D V3.3 utf8\nPROFILE Full\nNurbsCurve { }")

for i, c in enumerate([
    "COMPONENT Geospatial:1", "COMPONENT Geospatial:9", "COMPONENT Geospatial:0", "COMPONENT Geospatial:-1", "COMPONENT Geospatial:1.5",
    "COMPONENT Geospatial : 1", "COMPONENT Geospatial :1", "COMPONENT Geospatial: 1", "COMPONENT Geospatial", "COMPONENT Geospatial:",
    "COMPONENT :1", "COMPONENT Nonsense:1", "COMPONENT Nonsense:7", "COMPONENT geospatial:1", "COMPONENT Geospatial:1\nCOMPONENT Navigation:2",
    "COMPONENT Navigation:2\nCOMPONENT Geospatial:1", "COMPONENT Geospatial:1\nCOMPONENT Geospatial:2", "COMPONENT Geospatial:2\nCOMPONENT Geospatial:1",
    "COMPONENT Core:2", "COMPONENT Core:1\nCOMPONENT Shape:1", "COMPONENT Shape:3\nCOMPONENT Text:2", "COMPONENT \"Geospatial\":1", "COMPONENT Geospatial:0x1",
    "COMPONENT Geospatial:1e0", "COMPONENT Geospatial:+1", "COMPONENT HAnim:1", "COMPONENT X_ITE:1", "COMPONENT Texturing3D:2",
    "COMPONENT Scripting:1\nCOMPONENT Scripting:1", "COMPONENT Geospatial:1,Navigation:1",
]):
    T(f"comp-{i:02d}", f"#X3D V3.3 utf8\nPROFILE Core\n{c}\nGroup {{ }}")

T("comp-before-profile", "#X3D V3.3 utf8\nCOMPONENT Geospatial:1\nPROFILE Core\nGroup { }")
T("comp-no-profile", "#X3D V3.3 utf8\nCOMPONENT Geospatial:1\nGroup { }")
T("comp-after-node", "#X3D V3.3 utf8\nPROFILE Core\nGroup { }\nCOMPONENT Geospatial:1")
T("comp-vrml-header", "#VRML V2.0 utf8\nCOMPONENT Geospatial:1\nGroup { }")
T("comp-geo-node", "#X3D V3.3 utf8\nPROFILE Core\nCOMPONENT Geospatial:1\nGeoLocation { geoCoords 1 2 3 }")
T("comp-geo-node-missing", "#X3D V3.3 utf8\nPROFILE Core\nGeoLocation { geoCoords 1 2 3 }")

for i, u in enumerate([
    "UNIT length foot 0.3048", "UNIT angle degree 0.0174532925199433", "UNIT force newton 2", "UNIT mass gram 0.001", "UNIT nothing x 3",
    "UNIT length foot", "UNIT length foot abc", "UNIT length foot -1", "UNIT length foot 0", "UNIT length foot 1e-3", "UNIT length foot 1.5e400",
    "UNIT length \"foot\" 0.3048", "UNIT length foot 0.3048\nUNIT length metre 1", "UNIT length foot 0.3048\nUNIT angle degree 0.01745329251994329",
    "UNIT length metre 1", "UNIT angle radian 1", "UNIT LENGTH foot 0.3048", "UNIT length foot 0x10", "UNIT length foot NaN", "UNIT length foot .5", "UNIT length foot +2",
    "UNIT length foot 0.3048,", "UNIT", "UNIT length",
]):
    T(f"unit-{i:02d}", f"#X3D V3.3 utf8\nPROFILE Immersive\n{u}\nTransform {{ translation 1 2 3 rotation 0 0 1 90 }}")

T("unit-after-meta", "#X3D V3.3 utf8\nPROFILE Immersive\nMETA \"a\" \"b\"\nUNIT length foot 0.3048\nGroup { }")
T("unit-before-component", "#X3D V3.3 utf8\nPROFILE Immersive\nUNIT length foot 0.3048\nCOMPONENT Scripting:1\nGroup { }")
T("unit-before-profile", "#X3D V3.3 utf8\nUNIT length foot 0.3048\nPROFILE Immersive\nGroup { }")

for i, m in enumerate([
    'META "a" "b"', 'META "a" "b"\nMETA "a" "c"', 'META "" "d"', 'META "a" ""', 'META "a"', 'META a b', 'META "a" "b" "c"', 'META "a\\"q" "b\\\\c"',
    'META "é" "€"', 'META "line" "one\ntwo"', 'META "a" "b"\nMETA "b" "a"', 'META "created" "2026-01-01"', 'META', 'META "a" 5', 'META 5 "a"',
    'META "a" "b" META "c" "d"', 'META "x" "\\n"',
]):
    T(f"meta-{i:02d}", f"#X3D V3.3 utf8\nPROFILE Immersive\n{m}\nGroup {{ }}")

T("meta-vrml-header", '#VRML V2.0 utf8\nMETA "a" "b"\nGroup { }')
T("meta-after-node", '#X3D V3.3 utf8\nPROFILE Immersive\nGroup { }\nMETA "a" "b"')
T("meta-order-all", '#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\nUNIT length foot 0.3048\nMETA "a" "b"\nGroup { }')
T("meta-order-bad", '#X3D V3.3 utf8\nPROFILE Immersive\nMETA "a" "b"\nCOMPONENT Scripting:1\nGroup { }')

# --- comments and whitespace ---------------------------------------------------------------------------------------------------------
cm = [
    "Group { # c\n children [ ] }",
    "Group { children # c\n [ ] }",
    "Group { children [ # c\n ] }",
    "Group { children [ ] # c\n }",
    "Group # c\n { }",
    "Group #c\n#d\n { }",
    "#c\nGroup { }",
    "Group { }#c",
    "Group { }\n#c",
    "Group { }\n# c without newline at the end of the file",
    "Group { #}\n }",
    "Group { children [ Group { } # ]\n ] }",
    "DEF # c\n A Group { }",
    "DEF A # c\n Group { }",
    "DEF A Group { }\nUSE # c\n A",
    "Transform { translation 1 # c\n 2 3 }",
    "Transform { translation 1#c\n 2 3 }",
    "Transform { translation 1 2 3# c\n }",
    "Transform { translation # c\n 1 2 3 }",
    "Transform { # c\n translation 1 2 3 # d\n scale 1 1 1 }",
    'WorldInfo { title "a # not a comment" }',
    'WorldInfo { title "a" # comment "b"\n }',
    'WorldInfo { info [ "a" # "b"\n "c" ] }',
    'WorldInfo { title "# at start" info [ "#", "##" ] }',
    "Group { children [ Group { } # first\n , # between\n Group { } ] }",
    "Group { children [ , Group { } ] }",
    "Group { children [ Group { }, ] }",
    "Group { children [ Group { } , , Group { } ] }",
    "Group { , }",
    "Group { children [ ] , }",
    ",Group { }",
    "Group { },",
    "Group { } , Group { }",
    "Group\t{\tchildren\t[\t]\t}",
    "Group\r\n{\r\nchildren\r\n[\r\n]\r\n}",
    "Group\r{\rchildren\r[\r]\r}",
    "Group\f{\fchildren\f[\f]\f}",
    "Group\v{ }",
    "Group { }",
    "Group { }",
    "Group　{ }",
    "Group ﻿{ }",
    "Group​{ }",
    "Group { }\u0085",
    "Group { } Group { }",
    "Group { } Group { }",
    "Group { }\x0b",
    "Group { }\x1a",
    "Group{children[Group{}Group{}]}",
    "Group{children[Group{},Group{}]}",
    "Group{children Group{}}",
    "Transform{translation 1 2 3}",
    "Transform{translation 1,2,3}",
    "Transform{translation 1 ,2 ,3}",
    "Transform{translation 1,,2 3}",
    "Group {\n#\n#\n#\n}",
    "Group {\n#\n}",
    "#\n#\n#\nGroup { }",
]
for i, c in enumerate(cm):
    T(f"cmt-{i:02d}", V + c)

T("cmt-x3d-header-comment-line", "#X3D V3.3 utf8\n# comment between header and profile\nPROFILE Core\n# comment after\nGroup { }")
T("cmt-in-profile", "#X3D V3.3 utf8\nPROFILE # c\n Core\nGroup { }")
T("cmt-in-component", "#X3D V3.3 utf8\nPROFILE Core\nCOMPONENT Geospatial # c\n :1\nGroup { }")
T("cmt-in-unit", "#X3D V3.3 utf8\nPROFILE Core\nUNIT length # c\n foot 0.3048\nGroup { }")
T("cmt-in-meta", '#X3D V3.3 utf8\nPROFILE Core\nMETA "a" # c\n "b"\nGroup { }')
T("cmt-in-route", V + "DEF A Transform { }\nDEF B Transform { }\nROUTE # c\nA.translation_changed # d\nTO # e\nB.set_translation")
T("cmt-in-proto", V + "PROTO # c\nP # d\n[ # e\n inputOutput # f\n SFFloat # g\n f # h\n 1 # i\n] # j\n{ # k\n Group { } # l\n} # m\nP { }")
T("cmt-in-script", V + 'Script { # c\n url # d\n "ecmascript:" # e\n }')
T("cmt-hash-in-id", V + "DEF A#B Group { }")
T("cmt-after-number-no-space", V + "Transform { scale 1#c\n1 1 }")
T("cmt-block-style", V + "/* block */ Group { }")
T("cmt-cpp-style", V + "// line\nGroup { }")
T("cmt-block-hash", V + "#/* block */# Group { }\nGroup { }")

# --- numbers -------------------------------------------------------------------------------------------------------------------------
tr = [
    "1 2 3", "+1 +2 +3", "-1 -2 -3", ".5 -.5 +.5", "1. 2. 3.", "-1. +2. .3", "1e3 1E3 1e+3", "1e-3 1E-3 -1e-3", "1e 2 3", "1e+ 2 3", "1e1.5 2 3",
    "0x10 0X1f 0xFF", "-0x10 +0x10 0x-1", "0x 1 2", "0xG 1 2", "0x1.8 1 2", "0x7fffffff 0xffffffff 0x100000000", "1.5.5 1 2", "1..2 3 4", "1 2 3 4",
    "1 2", "1", "", "NaN NaN NaN", "nan 0 0", "Infinity -Infinity 0", "inf 0 0", "-inf 0 0", "INF 0 0", "1_000 0 0", "1,2,3", "1 , 2 , 3",
    "00012 007 0", "012 08 09", "1e400 -1e400 1e-400", "1e308 -1e308 1e-308", "3.4028235e38 3.5e38 1e39", "1.4e-45 1e-46 1e-50",
    "0.1 0.2 0.3", "16777217 16777216 16777215", "0.30000000000000004 0.1 0.7", "123456789 1234567890 12345678901", "1234567.125 0.0001 0.000001",
    "-0 -0.0 +0", "0 -0 0", "1e21 1e22 1e-7", "9007199254740993 0 0", "2.5e-7 0.00000025 2.5E-7", "1.0000001 1.00000001 0.99999999",
    "100000000 1000000000 10000000000", "123.456 78.9 0.0", "5e-324 0 0", "1.7976931348623157e308 0 0", "1e-6 1e-5 1e-4", "1e5 1e6 1e7",
    "0b101 0 0", "0o7 0 0", "١ ٢ ٣", "1 2 abc", "abc 1 2", "1abc 2 3", "1 2 3abc", "1e5e5 0 0", "++1 0 0", "--1 0 0", "+-1 0 0", "1-2 3 4", "1+2 3 4",
    '"1" "2" "3"', "TRUE 0 0", "[1 2 3]", "[ 1 2 3 ]", "( 1 2 3 )", "{ 1 2 3 }", "1 2 3 NULL", "1 2 3", "1\t2\t3", "1\n2\n3", "1\r2\r3",
]
for i, t in enumerate(tr):
    T(f"num-sfvec3f-{i:02d}", V + f"Transform {{ translation {t} }}")

for i, t in enumerate([
    "1", "+1", "-1", "0", "-0", "1.5", "1e2", "1E+2", "1e-2", ".5", "5.", "0x10", "0X1F", "-0x10", "+0x10", "0x", "0xFFFFFFFF", "0x7FFFFFFF", "0x80000000", "-0x80000000",
    "2147483647", "2147483648", "-2147483648", "-2147483649", "4294967295", "4294967296", "99999999999", "1e10", "1e400", "NaN", "Infinity", "abc", "'1'", '"1"',
    "TRUE", "1 2", "[ 1 ]", "[ 1, ]", "[ 1.5 ]", "[ 1e2 ]", "00", "007", "1,", "1_0", "0.0", "-0.0", "1.0e0",
]):
    T(f"num-sfint32-{i:02d}", V + f"Switch {{ whichChoice {t} }}")

for i, t in enumerate([
    "[ ]", "[ 0 ]", "[ 0 1 2 -1 ]", "[ 0, 1, 2, -1 ]", "[ 0,1,2,-1 ]", "[ 0 1 2 -1, 3 4 5 -1 ]", "[ 0 1 2 -1 , ]", "[ , 0 ]", "[ 0 , , 1 ]", "0", "0 1 2",
    "[ 0x1 0x2 ]", "[ 1.5 ]", "[ 1e2 ]", "[ +1 ]", "[ -1 ]", "[ 2147483648 ]", "[ 1 2 3", "1 2 3 ]", "[ 1 2 3 ] ]", "[[ 1 ]]", "[ [ 1 ] ]", "[ abc ]", "[ 1 abc 2 ]",
    "[ NULL ]", '[ "1" ]', "[ TRUE ]", "[ 1 2 # c\n 3 ]", "[ 1 # c\n , 2 ]",
]):
    T(f"num-mfint32-{i:02d}", V + f"IndexedFaceSet {{ coordIndex {t} }}")

for i, t in enumerate([
    "TRUE", "FALSE", "true", "false", "True", "False", "1", "0", "\"TRUE\"", "\"true\"", "TRUE,", "[ TRUE ]", "yes", "ON", "NULL", "-1", "TRUE FALSE", "T", "TRUEFALSE",
    "TRUE#c", "0x1", "1.0",
]):
    T(f"num-sfbool-{i:02d}", V + f"DirectionalLight {{ on {t} }}")

for i, t in enumerate([
    "0.5", "1", "1e0", ".5", "-1", "2", "1e400", "NaN", "0x1", "+1", "5.", "0.1", "1e-50", "3.4028236e38", "16777217", "0.30000001192092896", "123456789.123",
]):
    T(f"num-sffloat-{i:02d}", V + f"NavigationInfo {{ speed {t} }}")

for i, t in enumerate([
    "0 0 1 0", "0 0 0 1", "0 0 0 0", "1 0 0 1.5707963267948966", "0 1 0 -1", "0 1 0 6.283185307179586", "0 1 0 7", "1 1 1 1", "2 0 0 1", "0 0 1", "0 0 1 0 0",
    "0 0 1 90", "0 0 1 1e10", "0 0 1 1e-10", "0x1 0 0 0x1", "NaN 0 0 0", "0 0 -1 3.14159", "0.7071067811865476 0 0.7071067811865476 3.141592653589793",
    "1e-9 0 0 1", "0.57735 0.57735 0.57735 2.0943951", "0.1 0.2 0.3 0.4",
]):
    T(f"num-sfrotation-{i:02d}", V + f"Transform {{ rotation {t} }}")

for i, t in enumerate([
    "1 0 0", "2 -1 0.5", "0x00ff00 0 0", "1 0", "1 0 0 0", "[ 1 0 0 ]", "1,0,0", "NaN 0 0", "1e400 0 0", ".5 .5 .5", "0.1 0.2 0.3", "1 1 1 1",
]):
    T(f"num-sfcolor-{i:02d}", V + f"Material {{ diffuseColor {t} }}")

for i, t in enumerate([
    "[ ]", "[ 1 0 0 ]", "[ 1 0 0, 0 1 0 ]", "[ 1 0 0 0 1 0 ]", "[ 1 0 0 0 1 ]", "[ 1 0 0, ]", "[ , 1 0 0 ]", "1 0 0", "[ 1,0,0, 0,1,0 ]", "[ 1 0 0 0 ]",
    "[ 0.1 0.2 0.3 0.4 0.5 0.6 ]", "[ 1e400 0 0 ]", "[ 1 0 0 NaN 0 0 ]", "[ 0x1 0 0 ]",
]):
    T(f"num-mfvec3f-{i:02d}", V + f"Coordinate {{ point {t} }}")

for i, t in enumerate([
    "[ ]", "[ 1 0 ]", "[ 1 0, 0 1 ]", "[ 1 0 0 1 ]", "[ 1 0 0 ]", "1 0", "[ 1 ]",
]):
    T(f"num-mfvec2f-{i:02d}", V + f"TextureCoordinate {{ point {t} }}")

for i, t in enumerate([
    "0", "1", "-1", "1.5", "1e21", "1e-7", "1e22", "123456789012345680000", "0.000001", "5e-324", "1.7976931348623157e308", "1e309", "NaN", "0x10", "0xFFFFFFFFFF",
    "0.1", "0.30000000000000004", "1000000", "9007199254740993", "-0", "+1",
]):
    T(f"num-sftime-{i:02d}", V + f"TimeSensor {{ cycleInterval {t} }}")

for i, t in enumerate([
    "[ ]", "[ 1 ]", "[ 1 2 3 ]", "[ 1, 2, 3 ]", "[ 0.1 1e21 1e-7 ]", "[ 1e400 ]", "[ NaN ]", "[ 0x10 ]", "1", "[ -0 ]", "[ 123456789012345680000 ]",
]):
    T(f"num-mfdouble-{i:02d}", V + f"MetadataDouble {{ name \"d\" value {t} }}")

for i, t in enumerate([
    "1 1 1", "1 1 4", "2 2 3 0xFF0000 0x00FF00 0x0000FF 0xFFFFFF", "2 1 4 0xFF0000FF 0x00FF00FF", "1 1 1 255", "1 1 2 0xFFFF", "0 0 0", "1 1 1 0x1ff", "1 1 1 -1",
    "1 1 5 0", "2 2 1 1 2 3", "1 1 3 0xFFFFFF 0x0", "1 1 1", "1 1 3 16777215", "1 2 1 0 1 2", "-1 1 1 0", "1.5 1 1 0", "1 1 1 0x", "1 1 1 NaN",
]):
    T(f"num-sfimage-{i:02d}", V + f"PixelTexture {{ image {t} }}")

# --- strings -------------------------------------------------------------------------------------------------------------------------
st = [
    '"a"', '""', '"a b"', '"a\\"b"', '"a\\\\b"', '"a\\nb"', '"a\\tb"', '"a\\rb"', '"a\\/b"', '"a\\u0041b"', '"a\\x41b"', '"a\\0b"', '"a\\\n b"', '"\\"', '"\\\\"', '"a\\\\"',
    '"multi\nline"', '"multi\r\nline"', '"multi\rline"', '"tab\there"', '"é€"', '"\U0001F600"', '"é€\U0001F600"', '"a\u0000b"', '"a b"', '"a​b"',
    '"a﻿b"', '"퟿"', '""', '"#"', '"# comment-like"', '"{ }"', '"[ ]"', '"a", "b"', '"a" "b"', '"a""b"', '"a"b', "'a'", "'a b'", "a", "\"a",
    '"a" b', "\"", '"unterminated\nWorldInfo { }', '"' + "x" * 300 + '"', '"a\\"', '"\\\\\\""', '"\\\\\\\\"', '"\u0001"', '"\x7f"', '" "',
]
for i, s in enumerate(st):
    T(f"str-sfstring-{i:02d}", V + f"WorldInfo {{ title {s} }}")

for i, s in enumerate([
    "[ ]", '[ "a" ]', '[ "a" "b" ]', '[ "a", "b" ]', '[ "a" , "b" , ]', '[ , "a" ]', '[ "a" , , "b" ]', '"a"', '"a" "b"', '[ "a" "b"', '"a" "b" ]', '[ "a" ] ]', '[[ "a" ]]',
    '[ "a" 1 ]', '[ 1 ]', '[ a ]', '[ NULL ]', '[ "" "" ]', '[ "" , "" ]', '[ "a" # c\n "b" ]', '[ "a\\"b" "c\\\\d" ]', '[ "é" "€" ]', '[ "a",\n"b",\n"c"\n]', '["a""b"]', '["a","b"]',
    '[ "a" "b" "c" "d" "e" "f" "g" "h" "i" "j" ]', '[ \'a\' ]', '[ "a" TRUE ]', '[ TRUE ]', '[ "unterminated ]',
]):
    T(f"str-mfstring-{i:02d}", V + f"WorldInfo {{ info {s} }}")

for i, s in enumerate([
    '"a"', '[ "a" ]', '[ "x.wrl" "y.wrl" ]', '[ "a b" "c d" ]', '"data:text/plain,a%20b"', '[ "x.wrl#v" ]', '""', '[ ]',
]):
    T(f"str-url-{i:02d}", V + f"Anchor {{ url {s} }}")

T("str-text-fontstyle", V + 'Text { string [ "a" "b" ] fontStyle FontStyle { family [ "SERIF" "SANS" ] justify [ "MIDDLE" "BEGIN" ] style "BOLD" } }')
T("str-sfstring-enum-case", V + 'NavigationInfo { type [ "walk" "ANY" "Examine" ] }')
T("str-script-url-multiline", V + 'Script { url "ecmascript:\nfunction f ()\n{\n}\n" }')
T("str-script-url-escapes", V + 'Script { url "ecmascript:function f () { return \\"x\\\\y\\"; }" }')
T("str-script-url-list", V + 'Script { url [ "ecmascript:a", "javascript:b", "x.js" ] }')
T("str-unicode-id", V + "DEF é Group { }\nUSE é")

# --- node lists and brackets ---------------------------------------------------------------------------------------------------------
for i, c in enumerate([
    "children [ ]", "children [ Group { } ]", "children [ Group { }, Group { } ]", "children [ Group { } Group { } ]", "children [ Group { },, Group { } ]",
    "children [ Group { } , ]", "children [ , Group { } ]", "children Group { }", "children Group { } Group { }", "children NULL", "children [ NULL ]", "children [ NULL NULL ]",
    "children [ NULL, Group { } ]", "children [ Group { } NULL ]", "children [", "children [ Group { }", "children ]", "children [ ] ]", "children [[ ]]", "children [ [ ] ]",
    "children 5", "children \"a\"", "children [ 5 ]", "children [ \"a\" ]", "children [ Material { } ]", "children [ Box { } ]", "children [ Shape { } ]",
    "children [ Group { children [ Group { children [ Group { } ] } ] } ]", "children [ Transform { } Transform { } ]", "children [ DEF A Group { } USE A ]",
    "children [ USE A ]", "children [ DEF A Group { } , USE A , USE A ]", "children [ ROUTE A.b TO C.d ]", "children [ PROTO P [ ] { Group { } } ]",
    "children [ PROTO P [ ] { Group { } } P { } ]", "children [ Group { } PROTO P [ ] { Group { } } P { } ]", "children [ EXTERNPROTO E [ ] \"x.wrl\" ]",
    "children [ DEF A Group { } ROUTE A.children_changed TO A.set_children ]", "children [ IMPORT A.b ]", "children [ EXPORT A ]", "children [ PROFILE Core ]",
    "children [ COMPONENT Core:1 ]", "children [ META \"a\" \"b\" ]", "children [ UNIT length foot 1 ]", "children [ Group { } } ]", "children [ { } ]", "children { }",
    "children [ children ]", "children [ Group ]", "children [ Group { children } ]",
]):
    T(f"mf-children-{i:02d}", V + f"Group {{ {c} }}")

T("mf-switch-choice", V + "Switch { whichChoice 1 choice [ Group { } Transform { } ] }")
T("mf-switch-children-alias", V + "Switch { whichChoice 0 children [ Group { } ] }")
T("mf-switch-children-set", V + "Switch { set_whichChoice 0 }")
T("mf-sfnode-field-list", V + "Shape { appearance [ Appearance { } ] }")
T("mf-sfnode-field-two", V + "Shape { appearance Appearance { } Appearance { } }")
T("mf-sfnode-null", V + "Shape { appearance NULL geometry NULL }")
T("mf-sfnode-wrong-type", V + "Shape { appearance Box { } }")
T("mf-sfnode-use", V + "DEF A Appearance { }\nShape { appearance USE A }\nShape { appearance USE A }")
T("mf-sfnode-use-wrong", V + "DEF A Appearance { }\nShape { geometry USE A }")
T("mf-nested-null-metadata", V + "Group { metadata NULL }")
T("mf-metadata-set", V + 'MetadataSet { name "s" value [ MetadataString { name "a" value [ "x" "y" ] } MetadataFloat { name "f" value [ 1 2 ] } MetadataBoolean { name "b" value [ TRUE FALSE ] } ] }')
T("mf-metadata-wrong-child", V + 'MetadataSet { value [ Group { } ] }')
T("mf-containerfield-alias", V + "Transform { children [ Shape { geometry Box { } } ] }")
T("mf-root-bare-nodes", V + "Group { }\nGroup { }\nTransform { }")
T("mf-root-null", V + "NULL")
T("mf-root-null-then-node", V + "NULL\nGroup { }\nNULL")
T("mf-root-comma", V + "Group { } , Group { } ,")
T("mf-root-brackets", V + "[ Group { } ]")
T("mf-root-string", V + '"text"')
T("mf-root-number", V + "5")
T("mf-root-brace", V + "{ }")
T("mf-root-keyword-only", V + "DEF")
T("mf-root-lowercase-node", V + "group { }")
T("mf-root-misspelled-node", V + "Grup { }")
T("mf-root-deep", V + "Group { children [ " * 60 + "Group { } " + "] } " * 60)
T("mf-root-many", V + "\n".join(f"Transform {{ translation {i} 0 0 }}" for i in range(120)))

# --- DEF / USE -----------------------------------------------------------------------------------------------------------------------
for i, d in enumerate([
    "DEF A Group { }", "DEF A_B-C Group { }", "DEF A.B Group { }", "DEF A:B Group { }", "DEF a1 Group { }", "DEF 1A Group { }", "DEF _A Group { }", "DEF +A Group { }", "DEF -A Group { }",
    "DEF A+ Group { }", "DEF A-1 Group { }", "DEF \"A\" Group { }", "DEF 'A' Group { }", "DEF A,B Group { }", "DEF A[B Group { }", "DEF A]B Group { }", "DEF A{B Group { }",
    "DEF A}B Group { }", "DEF A\\B Group { }", "DEF A'B Group { }", "DEF A\"B Group { }", "DEF A/B Group { }", "DEF A@B Group { }", "DEF A$B Group { }", "DEF A&B Group { }",
    "DEF A B Group { }", "DEF A B Group { }", "DEF é Group { }", "DEF 世界 Group { }", "DEF \U0001F600 Group { }", "DEF Group Group { }", "DEF DEF Group { }",
    "DEF USE Group { }", "DEF NULL Group { }", "DEF TRUE Group { }", "DEF FALSE Group { }", "DEF PROTO Group { }", "DEF ROUTE Group { }", "DEF IS Group { }", "DEF TO Group { }",
    "DEF EXTERNPROTO Group { }", "DEF IMPORT Group { }", "DEF AS Group { }", "DEF children Group { }", "DEF A DEF B Group { }", "DEF A USE A", "DEF A NULL", "DEF A",
    "DEF", "DEF Group { }", "DEF A Nonsense { }", "DEF A 5", "DEF A \"x\"", "DEF A [ Group { } ]", "DEF  A   Group   {   }", "DEF A\tGroup { }", "DEF A\nGroup { }",
    "DEF " + "A" * 200 + " Group { }", "DEF A Group { } DEF A Transform { }", "DEF A Group { } DEF B Group { } DEF A Group { }", "DEF A Group { children DEF B Group { } }",
    "DEF A Group { children DEF A Group { } }", "DEF A Group { children USE A }", "DEF A Group { children DEF B Group { children USE A } }",
    "DEF A Group { } USE A USE A", "DEF A Group { } Group { children [ USE A USE A ] }", "DEF A Group { } USE B", "USE A", "USE", "USE 5", "USE \"A\"", "USE A.b",
    "DEF A Group { } USE A { }", "DEF A Group { } USE A children [ ]", "DEF A Group { } Group { children USE A }", "DEF A Transform { } Group { children USE A }",
    "DEF A Group { } Shape { geometry USE A }", "DEF A Box { } Shape { geometry USE A }", "DEF A Box { } Shape { geometry USE A } Shape { geometry USE A }",
    "DEF A Material { } Shape { appearance Appearance { material USE A } }", "DEF A Group { metadata DEF M MetadataString { } }", "DEF A Group { metadata USE A }",
    "DEF A Group { } DEF A2 USE A", "DEF A Group { } Group { children [ DEF B USE A ] }",
]):
    T(f"def-{i:02d}", V + d)

T("def-use-before-def", V + "USE A\nDEF A Group { }")
T("def-redefine-then-route", V + "DEF A Transform { }\nDEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed TO B.set_translation")
T("def-redefine-use-new", V + "DEF A Group { }\nDEF A Transform { }\nGroup { children USE A }")
T("def-in-proto-body-visible-outside", V + "PROTO P [ ] { DEF Inner Group { } }\nP { }\nUSE Inner")
T("def-outside-visible-in-proto", V + "DEF Outer Group { }\nPROTO P [ ] { USE Outer }\nP { }")
T("def-proto-instance-def", V + "PROTO P [ ] { Group { } }\nDEF X P { }\nUSE X")
T("def-in-proto-field-value", V + "PROTO P [ inputOutput SFNode n NULL ] { Group { children IS n } }\nP { n DEF A Group { } }\nUSE A")
T("def-vrml-header-x3d-keywords", V + "DEF A Group { }\nIMPORT A.b AS c")
T("def-name-reuse-in-two-protos", V + "PROTO P [ ] { DEF A Group { } }\nPROTO Q [ ] { DEF A Transform { } }\nP { }\nQ { }")

# --- IS, fields, access types, field-name aliases --------------------------------------------------------------------------------------
for i, p in enumerate([
    "PROTO P [ inputOutput SFFloat f 1 ] { Transform { scale 1 1 1 translation 0 0 0 } }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { t 4 5 6 }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { translation 4 5 6 }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { set_t 4 5 6 }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { t_changed 4 5 6 }",
    "PROTO P [ exposedField SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { }",
    "PROTO P [ field SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { }",
    "PROTO P [ eventIn SFVec3f t ] { Transform { set_translation IS t } }\nP { }",
    "PROTO P [ eventOut SFVec3f t ] { Transform { translation_changed IS t } }\nP { }",
    "PROTO P [ inputOnly SFVec3f t ] { Transform { set_translation IS t } }\nP { }",
    "PROTO P [ outputOnly SFVec3f t ] { Transform { translation_changed IS t } }\nP { }",
    "PROTO P [ initializeOnly SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { set_translation IS t } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation_changed IS t } }\nP { }",
    "PROTO P [ inputOnly SFVec3f t ] { Transform { translation IS t } }\nP { }",
    "PROTO P [ outputOnly SFVec3f t ] { Transform { translation IS t } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t scale IS t } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t } Transform { translation IS t } }\nP { t 7 8 9 }",
    "PROTO P [ inputOutput SFFloat t 1 ] { Transform { translation IS t } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS u } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS 5 } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t, } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS # c\n t } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation 1 1 1 IS t } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t translation 1 1 1 } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { IS t } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { IS t }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Group { } }\nP { t IS t }",
    "Transform { translation IS t }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nTransform { translation IS t }",
    "PROTO P [ inputOutput SFBool b TRUE ] { DirectionalLight { on IS b } }\nP { b FALSE }",
    "PROTO P [ inputOutput SFString s \"x\" ] { WorldInfo { title IS s } }\nP { s \"y\" }",
    "PROTO P [ inputOutput MFString s [ \"a\" \"b\" ] ] { WorldInfo { info IS s } }\nP { }",
    "PROTO P [ inputOutput MFString s [ \"a\" \"b\" ] ] { WorldInfo { info IS s } }\nP { s \"c\" }",
    "PROTO P [ inputOutput MFNode c [ ] ] { Group { children IS c } }\nP { c Group { } }",
    "PROTO P [ inputOutput MFNode c [ ] ] { Group { children IS c } }\nP { c [ Group { } Transform { } ] }",
    "PROTO P [ inputOutput MFNode c [ ] ] { Group { children IS c } }\nP { c NULL }",
    "PROTO P [ inputOutput MFNode c [ Group { } ] ] { Group { children IS c } }\nP { }",
    "PROTO P [ inputOutput SFNode n NULL ] { Shape { appearance IS n } }\nP { n Appearance { } }",
    "PROTO P [ inputOutput SFNode n NULL ] { Shape { appearance IS n } }\nP { n Box { } }",
    "PROTO P [ inputOutput SFNode n Appearance { } ] { Shape { appearance IS n } }\nP { }",
    "PROTO P [ inputOutput SFNode n DEF D Appearance { } ] { Shape { appearance IS n } }\nP { }\nUSE D",
    "PROTO P [ inputOutput SFNode n USE X ] { Shape { appearance IS n } }\nP { }",
    "PROTO P [ inputOutput SFInt32 i 1.5 ] { Switch { whichChoice IS i } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1e400 ] { Fog { visibilityRange IS f } }\nP { }",
    "PROTO P [ inputOutput SFRotation r 0 0 1 0 ] { Transform { rotation IS r } }\nP { r 0 1 0 1 }",
    "PROTO P [ inputOutput SFColor c 1 0 0 ] { Material { diffuseColor IS c } }\nP { c 0 1 0 }",
    "PROTO P [ inputOutput SFTime t 1 ] { TimeSensor { cycleInterval IS t } }\nP { }",
    "PROTO P [ inputOutput SFVec2f v 1 2 ] { TextureTransform { translation IS v } }\nP { }",
    "PROTO P [ inputOutput MFFloat f [ 1 2 ] ] { ScalarInterpolator { key IS f } }\nP { }",
    "PROTO P [ inputOutput MFInt32 f [ 1 2 ] ] { IndexedLineSet { coordIndex IS f } }\nP { f [ 3 4 ] }",
    "PROTO P [ inputOutput SFBool b 5 ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFBool b TRUE ] { Group { } }\nP { b 5 }",
    "PROTO P [ inputOutput SFBool b TRUE ] { Group { } }\nP { b }",
    "PROTO P [ inputOutput SFBool b ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1 inputOutput SFFloat f 2 ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1 inputOutput SFString f \"x\" ] { Group { } }\nP { }",
    "PROTO P [ inputOutput NoType f 1 ] { Group { } }\nP { }",
    "PROTO P [ bogus SFFloat f 1 ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat ] { Group { } }\nP { }",
    "PROTO P [ inputOutput ] { Group { } }\nP { }",
    "PROTO P [ SFFloat f 1 ] { Group { } }\nP { }",
    "PROTO P [ f ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat 1 ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1, ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1 , inputOutput SFFloat g 2 ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1\n inputOutput SFFloat g 2 ] { Group { } }\nP { g 5 }",
    "PROTO P [ inputOutput SFFloat F 1 inputOutput SFFloat f 2 ] { Group { } }\nP { F 5 f 6 }",
    "PROTO P [ inputOutput SFFloat set_f 1 ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat f_changed 1 ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat metadata 1 ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat 1f 1 ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat \"f\" 1 ] { Group { } }\nP { }",
    "PROTO P [ inputOutput SFFloat f.g 1 ] { Group { } }\nP { }",
    "PROTO P [ initializeOnly SFFloat f 1 inputOnly SFFloat i outputOnly SFFloat o ] { Group { } }\nP { f 2 }",
    "PROTO P [ initializeOnly SFFloat f 1 ] { Group { } }\nP { set_f 2 }",
    "PROTO P [ inputOnly SFFloat i ] { Group { } }\nP { i 2 }",
    "PROTO P [ outputOnly SFFloat o ] { Group { } }\nP { o 2 }",
    "PROTO P [ eventIn SFFloat i ] { Group { } }\nP { i 2 }",
    "PROTO P [ eventOut SFFloat o ] { Group { } }\nP { o 2 }",
]):
    T(f"is-{i:02d}", V + p)

for i, f in enumerate([
    "Transform { set_translation 1 2 3 }", "Transform { translation_changed 1 2 3 }", "Transform { translation 1 2 3 translation 4 5 6 }",
    "Transform { translation 1 2 3 set_translation 4 5 6 }", "Transform { Translation 1 2 3 }", "Transform { TRANSLATION 1 2 3 }", "Transform { children_changed [ ] }",
    "Transform { set_children [ ] }", "Transform { addChildren [ ] }", "Transform { removeChildren [ ] }", "Transform { bboxSize 1 1 1 }", "Transform { bboxSize -1 -1 -1 }",
    "Transform { bboxSize 0 0 0 }", "Transform { bboxSize_changed 1 1 1 }", "Transform { metadata_changed NULL }", "Transform { set_metadata NULL }", "Transform { metadata NULL }",
    "Transform { DEF A Group { } }", "Transform { USE A }", "Transform { children [ ] children [ ] }", "Transform { nope 1 }", "Transform { nope }", "Transform { nope [ ] }",
    "Transform { nope Group { } }", "Transform { 5 }", "Transform { \"a\" }", "Transform { [ ] }", "Transform { translation }", "Transform { translation scale }",
    "Transform { scale 1 1 1 translation }", "Transform { translation 1 2 3 nope }", "Transform { nope 1 translation 1 2 3 }", "Transform { isActive TRUE }",
    "Transform { renderable FALSE }", "Transform { visible FALSE }", "Transform { bboxDisplay TRUE }", "Transform { scaleOrientation 0 0 1 0 }",
    "Transform { scaleOrientation 0 0 0 0 }", "Transform { center 1 2 3 }",
    "TimeSensor { isActive TRUE }", "TimeSensor { fraction_changed 0.5 }", "TimeSensor { set_startTime 1 }", "TimeSensor { startTime_changed 1 }", "TimeSensor { enabled FALSE }",
    "TimeSensor { set_enabled FALSE }", "TimeSensor { enabled_changed FALSE }", "TimeSensor { cycleTime 0 }", "TimeSensor { time 0 }", "TimeSensor { elapsedTime 0 }",
    "Group { children [ ] set_children [ ] }", "Script { url \"ecmascript:\" nope 1 }", "Script { }", "Script { directOutput TRUE mustEvaluate TRUE url \"ecmascript:\" }",
    "Script { directOutput 1 }", "Script { set_url \"x\" }", "Script { url_changed \"x\" }", "Script { metadata NULL }", "Script { children [ ] }",
    "Script { eventIn SFBool b url \"ecmascript:\" }", "Script { eventIn SFBool b eventIn SFBool b }", "Script { field SFInt32 i 5 eventOut SFInt32 o url \"ecmascript:\" }",
    "Script { exposedField SFInt32 i 5 url \"ecmascript:\" }", "Script { inputOutput SFInt32 i 5 url \"ecmascript:\" }", "Script { initializeOnly SFInt32 i 5 url \"ecmascript:\" }",
    "Script { inputOnly SFInt32 i url \"ecmascript:\" }", "Script { outputOnly SFInt32 o url \"ecmascript:\" }", "Script { inputOnly SFInt32 i 5 }", "Script { outputOnly SFInt32 o 5 }",
    "Script { field SFInt32 i }", "Script { field SFInt32 }", "Script { field }", "Script { field Nope i 5 }", "Script { field SFNode n NULL }", "Script { field SFNode n Group { } }",
    "Script { field MFNode n [ Group { } Group { } ] }", "Script { field MFNode n [ ] }", "Script { field MFNode n NULL }", "Script { field SFNode n USE A }",
    "Script { field SFString s \"x\" field MFString m [ \"a\" \"b\" ] }", "Script { field SFImage i 1 1 3 0xFFFFFF }", "Script { field SFColor c 1 2 3 }", "Script { field SFColorRGBA c 1 2 3 4 }",
    "Script { field SFMatrix3f m 1 0 0 0 1 0 0 0 1 }", "Script { field SFMatrix4f m 1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1 }", "Script { field SFVec4d v 1 2 3 4 }",
    "Script { field MFVec3d v [ 1 2 3, 4 5 6 ] }", "Script { field SFDouble d 1e-7 }", "Script { field MFBool b [ TRUE FALSE true ] }", "Script { field MFBool b [ TRUE, FALSE ] }",
    "Script { field MFRotation r [ 0 0 1 1, 0 1 0 2 ] }", "Script { field MFTime t [ 1 2 3 ] }", "Script { field MFMatrix3d m [ ] }", "Script { field MFImage i [ 1 1 1 0, 2 1 1 0 1 ] }",
    "Script { field SFBool b FALSE field SFBool b TRUE }", "Script { field SFFloat f 1 field SFString f \"x\" }",
]):
    T(f"field-{i:03d}", V + f)

# --- ROUTE ---------------------------------------------------------------------------------------------------------------------------
TT = V + "DEF A Transform { }\nDEF B Transform { }\n"
for i, r in enumerate([
    "ROUTE A.translation_changed TO B.set_translation",
    "ROUTE A.translation TO B.translation",
    "ROUTE A.set_translation TO B.set_translation",
    "ROUTE A.translation_changed TO B.translation_changed",
    "ROUTE A.translation_changed TO B.translation",
    "ROUTE A.translation TO B.set_translation",
    "ROUTE A.translation TO B.scale",
    "ROUTE A.translation_changed TO B.set_scale",
    "ROUTE A.translation_changed TO B.set_rotation",
    "ROUTE A.nope TO B.set_translation",
    "ROUTE A.translation_changed TO B.nope",
    "ROUTE A.set_nope TO B.set_translation",
    "ROUTE A.nope_changed TO B.set_translation",
    "ROUTE C.translation_changed TO B.set_translation",
    "ROUTE A.translation_changed TO C.set_translation",
    "ROUTE C.translation_changed TO D.set_translation",
    "ROUTE A.translation_changed TO A.set_translation",
    "ROUTE A.translation_changed TO B.set_translation\nROUTE A.translation_changed TO B.set_translation",
    "ROUTE A.translation_changed TO B.set_translation ROUTE A.scale_changed TO B.set_scale",
    "ROUTE A . translation_changed TO B . set_translation",
    "ROUTE A .translation_changed TO B .set_translation",
    "ROUTE A. translation_changed TO B. set_translation",
    "ROUTE A.translation_changed to B.set_translation",
    "ROUTE A.translation_changed To B.set_translation",
    "ROUTE A.translation_changed IN B.set_translation",
    "ROUTE A.translation_changed B.set_translation",
    "ROUTE A.translation_changed TO",
    "ROUTE A.translation_changed",
    "ROUTE A.",
    "ROUTE A",
    "ROUTE",
    "ROUTE .translation_changed TO B.set_translation",
    "ROUTE A.translation_changed TO .set_translation",
    "ROUTE A.translation_changed TO B.",
    "ROUTE A,translation_changed TO B,set_translation",
    "ROUTE A.translation_changed, TO B.set_translation",
    "ROUTE A.translation_changed TO B.set_translation,",
    "ROUTE \"A\".translation_changed TO B.set_translation",
    "ROUTE A.\"translation_changed\" TO B.set_translation",
    "ROUTE A.translation_changed TO B.set_translation.x",
    "ROUTE A.A.translation_changed TO B.set_translation",
    "ROUTE A.translation_changed TO B.set_translation\nROUTE B.translation_changed TO A.set_translation",
    "ROUTE A.children_changed TO B.addChildren",
    "ROUTE A.children_changed TO B.set_children",
    "ROUTE A.children_changed TO B.removeChildren",
    "ROUTE A.metadata_changed TO B.set_metadata",
    "ROUTE A.bboxSize TO B.set_bboxSize",
    "ROUTE A.bboxSize_changed TO B.set_bboxSize",
    "ROUTE A.metadata TO B.metadata",
    "ROUTE A.bboxCenter TO B.bboxSize",
    "ROUTE A.translation_changed TO B.set_translation # c\n",
    "ROUTE A.translation_changed\nTO\nB.set_translation",
    "ROUTE\tA.translation_changed\tTO\tB.set_translation",
    "ROUTE A.translation_changed TO B.set_translation\nUSE A",
    "ROUTE A.translation_changed TO B.set_translation\nDEF A Group { }",
    "DEF C Group { }\nROUTE A.translation_changed TO C.set_translation",
    "DEF C Group { }\nROUTE C.children_changed TO A.set_children",
    "ROUTE A.isActive TO B.set_translation",
    "ROUTE route.translation_changed TO B.set_translation",
]):
    T(f"route-{i:02d}", TT + r)

TS = V + "DEF T TimeSensor { }\nDEF P PositionInterpolator { key [ 0 1 ] keyValue [ 0 0 0 1 1 1 ] }\nDEF C Transform { }\nDEF S Script { inputOnly SFTime t outputOnly SFBool b field SFFloat f 1 inputOutput SFFloat g 2 url \"ecmascript:\" }\n"
for i, r in enumerate([
    "ROUTE T.fraction_changed TO P.set_fraction\nROUTE P.value_changed TO C.set_translation",
    "ROUTE T.fraction_changed TO C.set_translation",
    "ROUTE T.fraction_changed TO P.fraction",
    "ROUTE T.cycleTime TO S.t",
    "ROUTE T.cycleTime TO S.set_t",
    "ROUTE S.b TO T.set_enabled",
    "ROUTE S.b TO T.enabled",
    "ROUTE S.b_changed TO T.set_enabled",
    "ROUTE T.isActive TO S.t",
    "ROUTE T.cycleTime TO S.f",
    "ROUTE T.fraction_changed TO S.f",
    "ROUTE T.fraction_changed TO S.g",
    "ROUTE T.fraction_changed TO S.set_g",
    "ROUTE S.g_changed TO T.set_cycleInterval",
    "ROUTE S.g TO T.set_cycleInterval",
    "ROUTE S.f_changed TO T.set_cycleInterval",
    "ROUTE S.t TO T.set_enabled",
    "ROUTE T.cycleTime TO S.nope",
    "ROUTE T.fraction_changed TO P.set_key",
    "ROUTE P.value_changed TO P.set_fraction",
    "ROUTE T.isActive TO T.set_enabled",
    "ROUTE T.isActive TO T.set_loop",
    "ROUTE T.time TO T.set_startTime",
    "ROUTE T.fraction_changed TO C.set_scale",
    "ROUTE P.value_changed TO C.translation",
    "ROUTE P.value TO C.set_translation",
    "ROUTE P.value_changed TO C.set_translation\nROUTE P.value_changed TO C.set_scale",
    "ROUTE P.keyValue_changed TO C.set_translation",
]):
    T(f"route-script-{i:02d}", TS + r)

T("route-in-proto-body", V + "PROTO P [ ] { DEF A Transform { } DEF B Transform { } ROUTE A.translation_changed TO B.set_translation }\nP { }")
T("route-in-proto-bad", V + "PROTO P [ ] { DEF A Transform { } ROUTE A.translation_changed TO B.set_translation }\nP { }")
T("route-in-proto-outer-node", V + "DEF A Transform { }\nPROTO P [ ] { DEF B Transform { } ROUTE A.translation_changed TO B.set_translation }\nP { }")
T("route-in-proto-is", V + "PROTO P [ inputOutput SFVec3f t 0 0 0 ] { DEF A Transform { translation IS t } DEF B Transform { } ROUTE A.translation_changed TO B.set_translation }\nP { t 1 1 1 }")
T("route-proto-instance-field", V + "DEF A Transform { }\nPROTO P [ inputOnly SFVec3f v ] { Group { } }\nDEF Q P { }\nROUTE A.translation_changed TO Q.v")
T("route-proto-instance-set-alias", V + "DEF A Transform { }\nPROTO P [ inputOutput SFVec3f v 0 0 0 ] { Group { } }\nDEF Q P { }\nROUTE A.translation_changed TO Q.set_v")
T("route-proto-instance-changed-alias", V + "DEF A Transform { }\nPROTO P [ outputOnly SFVec3f v ] { Group { } }\nDEF Q P { }\nROUTE Q.v_changed TO A.set_translation")
T("route-proto-instance-out", V + "DEF A Transform { }\nPROTO P [ outputOnly SFVec3f v ] { Group { } }\nDEF Q P { }\nROUTE Q.v TO A.set_translation")
T("route-inside-node", V + "DEF A Transform { }\nGroup { children [ DEF B Transform { } ROUTE A.translation_changed TO B.set_translation ] }")
T("route-inside-node-bad-target", V + "DEF A Transform { }\nGroup { children [ ROUTE A.translation_changed TO Z.set_translation DEF Z Transform { } ] }")
T("route-before-def", V + "ROUTE A.translation_changed TO B.set_translation\nDEF A Transform { }\nDEF B Transform { }")
T("route-inside-field", V + "DEF A Transform { }\nTransform { translation 0 0 0 ROUTE A.translation_changed TO A.set_translation }")
T("route-eventin-spelling", V + "DEF A Script { eventIn SFVec3f i eventOut SFVec3f o url \"ecmascript:\" }\nDEF B Transform { }\nROUTE A.o TO B.set_translation\nROUTE B.translation_changed TO A.i")
T("route-exposed-spelling", V + "DEF A Script { exposedField SFVec3f e 0 0 0 url \"ecmascript:\" }\nDEF B Transform { }\nROUTE A.e_changed TO B.set_translation\nROUTE B.translation_changed TO A.set_e")
T("route-import-export", V + "DEF A Transform { }\nEXPORT A AS Z\nIMPORT A.Y AS W")
T("route-import-inline", V + "DEF I Inline { }\nIMPORT I.E1 AS X\nDEF B Transform { }\nROUTE X.translation_changed TO B.set_translation")

# --- PROTO and EXTERNPROTO syntax ---------------------------------------------------------------------------------------------------
for i, p in enumerate([
    "PROTO P [ ] { Group { } }\nP { }",
    "PROTO P [ ] { Group { } Group { } }\nP { }",
    "PROTO P [ ] { }\nP { }",
    "PROTO P [ ] { # only a comment\n }\nP { }",
    "PROTO P [ ] { Group { } }",
    "PROTO P [ ] { Group { } }\nP { }\nP { }",
    "PROTO P [ ] { Group { } }\nPROTO P [ ] { Transform { } }\nP { }",
    "PROTO P [ ] { Group { } }\nPROTO Q [ ] { P { } }\nQ { }",
    "PROTO Q [ ] { P { } }\nPROTO P [ ] { Group { } }\nQ { }",
    "PROTO P [ ] { P { } }\nP { }",
    "PROTO P [ ] { Q { } }\nPROTO Q [ ] { P { } }\nP { }",
    "PROTO P [ ] { PROTO Inner [ ] { Group { } } Inner { } }\nP { }",
    "PROTO P [ ] { PROTO Inner [ ] { Group { } } Inner { } }\nInner { }",
    "PROTO P [ ] { PROTO Inner [ ] { Group { } } }\nP { }",
    "PROTO P [ ] { DEF A Group { } USE A }\nP { }\nP { }",
    "PROTO P [ ] { USE A }\nP { }",
    "PROTO P [ ] { ROUTE A.b TO C.d Group { } }\nP { }",
    "PROTO P [ ] { Group { } ROUTE A.b TO C.d }\nP { }",
    "PROTO P [ ] { DEF A Group { } Group { } }\nP { }",
    "PROTO P [ ] { NULL }\nP { }",
    "PROTO P [ ] { NULL Group { } }\nP { }",
    "PROTO P [ ] { 5 }\nP { }",
    "PROTO P [ ] { \"x\" }\nP { }",
    "PROTO P [ ] { Group { } \"x\" }\nP { }",
    "PROTO P [ ] { Script { url \"ecmascript:\" } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Script { inputOutput SFFloat g IS f url \"ecmascript:\" } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Script { field SFFloat g IS f url \"ecmascript:\" } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Script { field SFString g IS f url \"ecmascript:\" } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Script { inputOnly SFFloat g IS f url \"ecmascript:\" } }\nP { }",
    "PROTO Group [ ] { Transform { } }\nGroup { }",
    "PROTO Transform [ ] { Group { } }\nTransform { }",
    "PROTO Box [ ] { Sphere { } }\nShape { geometry Box { } }",
    "PROTO Delay [ ] { Transform { } }\nDelay { }",
    "PROTO group [ ] { Transform { } }\ngroup { }",
    "PROTO P [ ] { Group { } }\nP { }\nPROTO P [ ] { Transform { } }\nP { }",
    "PROTO P-Q [ ] { Group { } }\nP-Q { }",
    "PROTO P.Q [ ] { Group { } }\nP.Q { }",
    "PROTO 1P [ ] { Group { } }\n1P { }",
    "PROTO \"P\" [ ] { Group { } }",
    "PROTO é [ ] { Group { } }\né { }",
    "PROTO P { Group { } }",
    "PROTO P [ ] Group { }",
    "PROTO P ] [ { }",
    "PROTO P [ ]",
    "PROTO P [",
    "PROTO P",
    "PROTO",
    "PROTO P [ ] {",
    "PROTO P [ ] { Group { }",
    "PROTO P [ ] { Group { } } }",
    "PROTO P [ ] { Group { } }}",
    "PROTO P [ ] { Group { } } P",
    "PROTO P [ ] { Group { } } P {",
    "PROTO P [ ] { Group { } } P { } }",
    "PROTO P [ ] { Group { } }\nDEF X P { }\nUSE X\nUSE X",
    "PROTO P [ ] { Group { } }\nGroup { children [ P { } P { } ] }",
    "PROTO P [ ] { Group { } }\nShape { geometry P { } }",
    "PROTO P [ ] { Box { } }\nShape { geometry P { } }",
    "PROTO P [ ] { Box { } Sphere { } }\nShape { geometry P { } }",
    "PROTO P [ ] { Appearance { } }\nShape { appearance P { } }",
    "PROTO P [ inputOutput SFNode a NULL ] { Shape { appearance IS a } }\nP { a P { } }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nP { f 2 f 3 }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nP { f }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nP { g 1 }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nP { f \"x\" }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nP { f 1 2 }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nP { f [ 1 ] }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nP { f IS f }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nPROTO Q [ inputOutput SFFloat g 2 ] { P { f IS g } }\nQ { g 5 }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nPROTO Q [ inputOutput SFFloat g 2 ] { P { f IS h } }\nQ { }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nPROTO Q [ inputOutput SFString g \"s\" ] { P { f IS g } }\nQ { }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Transform { scale IS f } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Transform { rotation IS f } }\nP { }",
    "PROTO P [ inputOutput SFInt32 f 1 ] { Switch { whichChoice IS f } }\nP { f 2 }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Switch { whichChoice IS f } }\nP { }",
    "PROTO P [ inputOutput MFString s [ ] ] { WorldInfo { title IS s } }\nP { }",
    "PROTO P [ inputOutput SFString s \"\" ] { WorldInfo { info IS s } }\nP { }",
]):
    T(f"proto-{i:02d}", V + p)

# EXTERNPROTO: only forms that do not need to load anything, or whose load result does not change the parse (no instance).
for i, p in enumerate([
    "EXTERNPROTO E [ ]",
    "EXTERNPROTO E",
    "EXTERNPROTO",
    "EXTERNPROTO E [ ] 5",
    "EXTERNPROTO E [ ] [ ]",
    "EXTERNPROTO E [ ] { }",
    "EXTERNPROTO E [ ] \"x.wrl\" extra",
    "EXTERNPROTO E [ inputOutput SFFloat ] \"x.wrl\"",
    "EXTERNPROTO E [ inputOutput NoType f ] \"x.wrl\"",
    "EXTERNPROTO E [ bogus SFFloat f ] \"x.wrl\"",
    "EXTERNPROTO E [ SFFloat f ] \"x.wrl\"",
    "EXTERNPROTO E [ inputOutput SFFloat f 1 ] \"x.wrl\"",
    "EXTERNPROTO E [ inputOutput SFFloat f inputOutput SFFloat f ] \"x.wrl\"",
    "EXTERNPROTO E [ inputOutput SFFloat f, inputOutput SFFloat g ] \"x.wrl\"",
    "EXTERNPROTO E [ ] [ \"a.wrl\" \"b.wrl\" ] extra",
    "PROTO P [ ] { EXTERNPROTO E [ ] \"x.wrl\" Group { } }\nP { }",
    "Group { children [ EXTERNPROTO E [ ] \"x.wrl\" ] }",
    "EXTERNPROTO E [ ] \"x.wrl\"\nPROTO E [ ] { Group { } }\nE { }",
    "PROTO E [ ] { Group { } }\nEXTERNPROTO E [ ] \"x.wrl\"",
]):
    T(f"extern-{i:02d}", V + p)

# --- IMPORT / EXPORT / misc statements ---------------------------------------------------------------------------------------------------
for i, s in enumerate([
    "DEF A Group { }\nEXPORT A", "DEF A Group { }\nEXPORT A AS B", "DEF A Group { }\nEXPORT A AS B\nEXPORT A AS B", "DEF A Group { }\nEXPORT A AS", "DEF A Group { }\nEXPORT B",
    "EXPORT", "DEF A Group { }\nEXPORT A AS B C", "DEF A Group { }\nEXPORT A\nEXPORT A", "DEF A Group { }\nDEF B Group { }\nEXPORT A AS Z\nEXPORT B AS Z",
    "IMPORT", "IMPORT A", "IMPORT A.", "IMPORT A.b", "IMPORT A.b AS", "IMPORT A.b AS c", "DEF A Group { }\nIMPORT A.b AS c", "DEF I Inline { }\nIMPORT I.E1",
    "DEF I Inline { }\nIMPORT I.E1 AS X", "DEF I Inline { }\nIMPORT I.E1 AS X\nIMPORT I.E2 AS X", "DEF I Inline { }\nIMPORT I.E1\nIMPORT I.E1",
    "DEF I Inline { }\nIMPORT I.E1 AS X\nGroup { children USE X }", "DEF I Inline { }\nIMPORT I . E1 AS X", "DEF I Inline { }\nIMPORT I.E1 as X", "DEF I Inline { }\nimport I.E1",
    "IMPORT Missing.E1", "DEF I Inline { }\nIMPORT I.E1 AS", "DEF I Inline { }\nEXPORT I AS E1", "DEF I Inline { }\nEXPORT I\nIMPORT I.I",
]):
    T(f"impexp-{i:02d}", V + s)

# --- errors: unterminated and malformed input ------------------------------------------------------------------------------------------
for i, e in enumerate([
    "Group {", "Group { children [ }", "Group { children [ Group { } }", "Group { } }", "Group { children ] }", "Group { nope 1 }", "Group { children 5 }",
    "Transform { translation a b c }", "Transform { translation \"1\" \"2\" \"3\" }", "Transform { translation [ 1 2 3 ] }", "Transform { translation 1 2 3 translation 4 5 6 }",
    "Transform { translation 1 2 3 ", "Transform { translation 1 2", "Transform { translation 1 2 3 }}", "Transform { translation 1 2 3 } }",
    'Text { string "unterminated }', 'Text { string "a" "b" }', 'Text { string [ "a" "b" }', 'Text { string [ "a" "b" ] ', 'Text { string "a',
    "Material { diffuseColor 1 0 }", "Material { diffuseColor 1 0 0 0 }", "Shape { geometry Material { } }", "Shape { appearance Box { } }",
    "Shape { geometry Box { } geometry Sphere { } }", "Shape { geometry Box { } appearance Appearance { } geometry Box { } }", "Nonsense { }", "nonsense", "{ }", "Group Group",
    "Group { } Nonsense", "Group { } 5", 'Group { } "text"', "Group { } ,", "Group { , }", "Group { children [ , ] }", "Group { children [ Group { } , , Group { } ] }",
    "Script { url \"ecmascript:\" nope 1 }", "Script { inputOnly SFFloat }", "Script { inputOnly NoType x }", "Script { inputOnly SFFloat 5 }",
    "Script { inputOutput SFFloat f \"x\" }", "Script { inputOutput SFInt32 i 1.5 }", "Script { inputOutput SFBool b 1 }", "Script { inputOutput SFBool b yes }",
    "Group { children [ Group { children [ Group { children [ Group {", "Group { children [ Group { children [ Group { } ] } ", "}", "]", "[", "[ ]", ",", ".", "IS", "TO", "NULL NULL",
    "Group { children [ Nonsense { } ] }", "Group { children [ Group { } Nonsense { } Group { } ] }", "Transform { children [ Nonsense { children [ Group { } ] } ] }",
    "Group { nope { } }", "Group { nope [ ] }", "Group { nope Group { } }", "Group { nope 1 2 3 }", "Group { nope \"a\" }", "Group { nope TRUE }", "Group { nope NULL }",
    "Group { nope [ 1 2 3 ] }", "Group { nope [ Group { } ] }", "Group { bboxCenter nope }", "Group { bboxCenter 1 2 nope }", "Group { bboxCenter 1 2 3 nope 5 }",
    "Nonsense { nope 1 }", "Nonsense { children [ Group { } ] }", "Nonsense { Nonsense2 { } }", "Nonsense", "Nonsense Nonsense", "Nonsense { } { }", "Nonsense { [ ] }",
    "Group { children [ Nonsense ] }", "Group { children Nonsense }", "DEF A Nonsense { }\nUSE A", "DEF A Nonsense { children [ Group { } ] }\nGroup { }",
    "Shape { geometry Nonsense { } }", "Shape { geometry Nonsense { } appearance Appearance { } }", "Shape { geometry IndexedFaceSet { coordIndex [ 0 1 2 ] coord Nonsense { } } }",
    "IndexedFaceSet { coord Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] }",
    "IndexedFaceSet { coord Coordinate { point [ 0 0 0 ] } coordIndex [ 0 1 2 -1 ] color Color { color [ 1 0 0 ] } colorIndex [ 0 ] }",
    "LOD { range [ 10, 20 ] level [ Group { } Group { } Group { } ] }", "LOD { range [ 10 20 ] level [ ] center 1 2 3 }",
    "Inline { url [ ] }", "Inline { url \"\" }", "Inline { load FALSE url \"missing.wrl\" }", "Inline { bboxSize 1 1 1 bboxCenter 0 0 0 load FALSE }",
    "Anchor { url [ ] children [ Group { } ] description \"d\" parameter [ \"target=_blank\" ] }",
    "Billboard { axisOfRotation 0 0 0 children [ Group { } ] }", "Collision { collide FALSE children [ Group { } ] proxy Group { } }",
    "Background { skyColor [ 0 0 1, 1 1 1 ] skyAngle [ 1.5 ] groundColor [ ] }", "Background { skyColor [ 0 0 1 ] skyAngle [ 1 2 ] }",
    "Fog { color 1 0 0 fogType \"EXPONENTIAL\" visibilityRange 10 }", "Fog { fogType \"nonsense\" }", "NavigationInfo { type [ \"EXAMINE\" \"ANY\" ] avatarSize [ 0.25 1.6 0.75 ] }",
    "NavigationInfo { type \"FLY\" speed 5 headlight FALSE visibilityLimit 0 }", "Viewpoint { position 0 0 10 orientation 0 1 0 0.5 fieldOfView 1 description \"v\" }",
    "Viewpoint { fieldOfView 0 }", "Viewpoint { fieldOfView 4 }", "Viewpoint { jump FALSE }", "OrthoViewpoint { fieldOfView [ -1 -1 1 1 ] }",
    "Sound { source AudioClip { url \"a.wav\" } minFront 1 }", "Shape { geometry Text { string [ \"a\" \"b\" ] length [ 1 2 ] maxExtent 3 } }",
    "ElevationGrid { xDimension 2 zDimension 2 height [ 0 1 2 3 ] }", "ElevationGrid { xDimension 2 zDimension 2 height [ 0 1 2 ] }", "Extrusion { crossSection [ 1 1, 1 -1, -1 -1, -1 1, 1 1 ] spine [ 0 0 0, 0 1 0 ] }",
    "Cone { bottomRadius 2 height 3 side TRUE bottom FALSE }", "Cylinder { radius 2 height 3 top FALSE }", "Sphere { radius -1 }", "Box { size 0 0 0 }", "Box { size -1 -1 -1 }",
    "PointLight { location 0 0 0 radius 10 attenuation 1 0 0 }", "SpotLight { cutOffAngle 0.5 beamWidth 1 }", "DirectionalLight { direction 0 0 0 }",
    "TouchSensor { enabled FALSE }", "PlaneSensor { minPosition 0 0 maxPosition -1 -1 }", "CylinderSensor { diskAngle 0.2 }", "SphereSensor { }", "ProximitySensor { size 1 1 1 }",
    "VisibilitySensor { size 1 1 1 }", "CollisionSensor { }", "Billboard { }", "ColorInterpolator { key [ 0 1 ] keyValue [ 1 0 0 0 1 0 ] }", "OrientationInterpolator { key [ 0 1 ] keyValue [ 0 1 0 0, 0 1 0 3 ] }",
    "NormalInterpolator { key [ 0 1 ] keyValue [ 0 1 0 0 1 0 ] }", "CoordinateInterpolator { key [ 0 1 ] keyValue [ 0 0 0 1 1 1 ] }", "ScalarInterpolator { key [ 0 1 ] keyValue [ 0 1 ] }",
    "PositionInterpolator { key [ 0 1 ] keyValue [ 0 0 0 1 1 1 ] }", "PositionInterpolator { key [ 1 0 ] keyValue [ 0 0 0 1 1 1 ] }", "PositionInterpolator { key [ 0 1 2 ] keyValue [ 0 0 0 1 1 1 ] }",
]):
    T(f"err-{i:03d}", V + e)

T("err-header-then-garbage", "#VRML V2.0 utf8\n@@@@")
T("err-binary-garbage", V + "\x01\x02\x03\x04")
T("err-nul-before-header", "\u0000#VRML V2.0 utf8\nGroup { }")
T("err-nul-after", V + "Group { }\u0000")
T("err-nul-in-middle", V + "Group {\u0000 }")
T("err-long-line", V + "WorldInfo { title \"" + "x" * 5000 + "\" nope }")
T("err-error-on-line-40", V + "\n" * 38 + "Group { nope 1 }")
T("err-error-in-last-line-no-newline", V + "Group { children [ Group { } ]")
T("err-error-with-tabs", V + "Group {\n\t\tchildren [\n\t\t\tNonsense { }\n\t\t]\n}")
T("err-error-with-crlf", V + "Group {\r\n  children [\r\n    Nonsense { }\r\n  ]\r\n}\r\n")
T("err-error-with-cr", V + "Group {\r  children [\r    Nonsense { }\r  ]\r}\r")
T("err-error-unicode-line", V + "WorldInfo { title \"é€\U0001F600\" }\nGroup { nope 1 }")
T("err-error-in-x3d-header", "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:\nGroup { }")
T("err-two-errors", V + "Group { nope 1 }\nGroup { nope2 2 }")
T("err-warn-then-error", V + "Group { nope 1 }\nNonsense { }")
T("err-many-warnings", V + "\n".join(f"Group {{ nope{i} {i} }}" for i in range(12)))

# --- byte-level cases (FileLoader path) ----------------------------------------------------------------------------------------------------
G = "#VRML V2.0 utf8\nWorldInfo { title \"é€\U0001F600\" }\n"
B("bytes-utf8", G.encode())
B("bytes-utf8-bom", b"\xef\xbb\xbf" + G.encode())
B("bytes-utf8-bom-x3d", b"\xef\xbb\xbf" + b"#X3D V3.3 utf8\nPROFILE Core\nGroup { }\n")
B("bytes-utf16le-bom", b"\xff\xfe" + G.encode("utf-16-le"))
B("bytes-utf16be-bom", b"\xfe\xff" + G.encode("utf-16-be"))
B("bytes-utf16le-nobom", G.encode("utf-16-le"))
B("bytes-latin1", "#VRML V2.0 utf8\nWorldInfo { title \"caf\xe9\" }\n".encode("latin-1"))
B("bytes-cp1252-euro", b"#VRML V2.0 utf8\nWorldInfo { title \"\x80\x93quoted\x94\" }\n")
B("bytes-invalid-utf8-lead", b"#VRML V2.0 utf8\nWorldInfo { title \"\xc3\" }\n")
B("bytes-invalid-utf8-cont", b"#VRML V2.0 utf8\nWorldInfo { title \"\x80\x80\" }\n")
B("bytes-overlong", b"#VRML V2.0 utf8\nWorldInfo { title \"\xc0\xaf\" }\n")
B("bytes-surrogate-utf8", b"#VRML V2.0 utf8\nWorldInfo { title \"\xed\xa0\x80\" }\n")
B("bytes-above-max", b"#VRML V2.0 utf8\nWorldInfo { title \"\xf4\x90\x80\x80\" }\n")
B("bytes-truncated-utf8", b"#VRML V2.0 utf8\nWorldInfo { title \"\xe2\x82")
B("bytes-crlf", b"#VRML V2.0 utf8\r\nGroup {\r\n children [\r\n Group { }\r\n ]\r\n}\r\n")
B("bytes-cr-only", b"#VRML V2.0 utf8\rGroup {\r children [\r Group { }\r ]\r}\r")
B("bytes-lf-cr", b"#VRML V2.0 utf8\n\rGroup { }\n\r")
B("bytes-nul-tail", b"#VRML V2.0 utf8\nGroup { }\x00\x00\x00")
B("bytes-nul-head", b"\x00#VRML V2.0 utf8\nGroup { }")
B("bytes-ctrl-z", b"#VRML V2.0 utf8\nGroup { }\x1a")
B("bytes-empty", b"")
B("bytes-bom-only", b"\xef\xbb\xbf")
B("bytes-newline-only", b"\n")
B("bytes-high-bytes-comment", b"#VRML V2.0 utf8\n# \xff\xfe comment\nGroup { }\n")
B("bytes-gzip", gzip.compress(G.encode(), mtime=0))
B("bytes-gzip-x3d", gzip.compress(b"#X3D V3.3 utf8\nPROFILE Immersive\nUNIT length foot 0.3048\nTransform { translation 1 2 3 }\n", mtime=0))
B("bytes-gzip-truncated", gzip.compress(G.encode(), mtime=0)[:-12])
B("bytes-gzip-empty", gzip.compress(b"", mtime=0))
B("bytes-gzip-garbage", b"\x1f\x8b\x08\x00garbage")
B("bytes-gzip-nested", gzip.compress(gzip.compress(G.encode(), mtime=0), mtime=0))
B("bytes-gzip-bom", gzip.compress(b"\xef\xbb\xbf" + G.encode(), mtime=0))
B("bytes-header-only", b"#VRML V2.0 utf8")
B("bytes-line-separator", "#VRML V2.0 utf8\nWorldInfo { title \"a b\" }\n".encode())
B("bytes-nbsp-token-gap", "#VRML V2.0 utf8\nGroup { }\n".encode())
B("bytes-big", ("#VRML V2.0 utf8\n" + "".join(f"Transform {{ translation {i} {i * 2} {i * 3} children [ Shape {{ geometry Box {{ }} }} ] }}\n" for i in range(300))).encode())
B("bytes-x3dv-xml-lookalike", b"#X3D V3.3 utf8\nPROFILE Core\nWorldInfo { title \"<X3D>\" }\n")
B("bytes-xml-prolog-then-vrml", b"<?xml version=\"1.0\"?>\n#VRML V2.0 utf8\nGroup { }\n")
B("bytes-json-looks", b"{ \"X3D\": { \"Scene\": { } } }")
B("bytes-vrml-in-html", b"<html>#VRML V2.0 utf8\nGroup { }</html>")


# --- keywords glued to their neighbors, more number forms, XML / JSON escaping, container fields -----------------------------------------------
for i, k in enumerate([
    "DEFA Group { }", "DEF A Group { }\nUSEA", "PROTOP [ ] { Group { } }\nP { }", "PROTO P[ ]{ Group{ } }\nP{ }", "DEF A Transform { }\nDEF B Transform { }\nROUTEA.translation_changed TO B.set_translation",
    "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed TOB.set_translation", "EXTERNPROTOE [ ] \"x.wrl\"", "IMPORTA.b", "EXPORTA", "DEF A Group { }\nEXPORTA",
    "NULLGroup { }", "Group { children [ NULLGroup { } ] }", "Group { children NULL, }", "Switch { whichChoice 0 }IS", "Group { }IS", "WorldInfo { title TRUE }",
    "DirectionalLight { on TRUEx }", "DirectionalLight { on FALSE1 }", "DirectionalLight { on TRUE\"x\" }", "DirectionalLight { on trueFALSE }", "Group{children[Group{}]}Group{}",
    "Group {children[]}", "Group{children[]bboxSize 1 1 1}", "Transform{translation 1 2 3rotation 0 0 1 1}", "Transform{translation 1 2 3 scale 1 1 1}",
    "Transform { translation 1 2 3scale 1 1 1 }", "WorldInfo { title \"a\"info \"b\" }", "WorldInfo { title\"a\" }", "WorldInfo{title\"a\"info[\"b\"\"c\"]}",
    "Switch { whichChoice-1 }", "Switch { whichChoice 1choice [ ] }", "DEF A Group { }USE A", "DEF A Group { }\nGroup { children [USE A,USE A] }",
]):
    T(f"kw-{i:02d}", V + k)

for i, t in enumerate([
    ".", "-", "+", "e5", "1.e5", ".e5", "-.5e-3", "+.5e+3", "0e0", "00x1", "0x00ff", "0xffffffffff", "0x1 0x2 0x3", "1e5 1e5 1e5", "-0x0", "0x0.1", "1.0e", "1e1e1",
    "5 5 5.", "5 5 .5", "5 5 -.5", "5 5 +.5", "5 5 .5.5", "5 5 5e5.5", "5 5 5 5", "1,.5,-.5", "1,+1,-1", "1..", "..1", "1.2.3", "٣", "1e０", "1\u2212 0 0", "１ ２ ３",
]):
    T(f"numx-{i:02d}", V + f"Transform {{ translation {t} }}")

for i, nm in enumerate(["A<B", "A>B", "A&B", "A&amp;B", "A;B", "A=B", "A?B", "A!B", "A%B", "A*B", "A(B)", "A|B", "A^B", "A~B", "A`B", "A:B", "A@B"]):
    T(f"def-xml-{i:02d}", V + f"DEF {nm} Group {{ }}\nGroup {{ children USE {nm} }}")

for i, t in enumerate(['a<b', 'a>b', 'a&b', 'a&amp;b', "a'b", 'a\\"b', 'a\tb', 'a\\nb', 'a\x01b', 'a\x08b', 'a\x1fb', 'a\x7fb', 'a\u0085b', 'a\u2028b', 'a\ud7ffb', 'a\ufffeb',
                       'a\uffffb', '\U0001F600', 'a]]>b', 'a-->b', '<!-- c -->']):
    T(f"esc-{i:02d}", V + f'WorldInfo {{ title "{t}" info [ "{t}" ] }}\nScript {{ field SFString s "{t}" url "ecmascript:" }}')
    T(f"esc-meta-{i:02d}", f'#X3D V3.3 utf8\nPROFILE Immersive\nMETA "{t}" "{t}"\nGroup {{ }}')

for i, c in enumerate([
    "Switch { choice [ Group { } ] }", "Switch { choice [ DEF A Group { } USE A ] }", "DEF A Group { }\nSwitch { choice [ USE A ] }", "LOD { level [ Group { } Group { } ] }",
    "Collision { proxy Shape { } }", "DEF A Shape { }\nCollision { proxy USE A children [ USE A ] }", "Shape { appearance Appearance { texture ImageTexture { } textureTransform TextureTransform { } } }",
    "Shape { geometry IndexedFaceSet { color Color { } coord Coordinate { } normal Normal { } texCoord TextureCoordinate { } } }",
    "Shape { geometry Text { fontStyle FontStyle { } } }", "Sound { source AudioClip { } }", "Anchor { children [ Group { } ] }",
    "PROTO P [ ] { Group { } }\nCollision { proxy P { } }", "PROTO P [ ] { Shape { } }\nSwitch { choice P { } }", "DEF I Inline { }\nIMPORT I.E1 AS X\nCollision { proxy USE X }",
    "DEF I Inline { }\nIMPORT I.E1 AS X\nSwitch { choice [ USE X ] }", "DEF I Inline { }\nIMPORT I.E1 AS X\nShape { geometry USE X }",
    "Group { metadata MetadataString { } }", "Group { metadata DEF M MetadataSet { value [ MetadataString { } USE M ] } }",
    "Viewpoint { metadata MetadataFloat { name \"f\" value 1 } }", "Script { metadata MetadataInteger { } field SFNode n Group { } field MFNode m [ Group { } ] url \"ecmascript:\" }",
    "Script { field SFNode n DEF D Group { } field SFNode u USE D url \"ecmascript:\" }",
]):
    T(f"xmlcf-{i:02d}", V + c)

# --- a value after IS (the node field and the PROTO field are interests of each other), legacy GeoViewpoint ----------------------------------------
for i, p in enumerate([
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t translation 4 5 6 } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation 4 5 6 translation IS t } }\nP { }",
    "PROTO P [ initializeOnly SFVec3f t 1 2 3 ] { Transform { center IS t center 4 5 6 } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t translation 4 5 6 translation 7 8 9 } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t translation 4 5 6 } Transform { translation IS t } }\nP { }\nP { t 9 9 9 }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t translation 1 2 3 } }\nP { }",
    "PROTO P [ inputOutput SFFloat f 1 ] { Script { inputOutput SFFloat g IS f g 5 url \"ecmascript:\" } }\nP { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t translation 4 5 6 } }\nDEF X P { }\nP { t 1 1 1 }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t translation 4 5 6 } }\nPROTO Q [ inputOutput SFVec3f u 0 0 0 ] { P { t IS u } }\nQ { }",
    "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { }\nPROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t translation 4 5 6 } }\nP { }",
    "PROTO P [ inputOutput MFString s [ \"a\" ] ] { WorldInfo { info IS s info [ \"b\" \"c\" ] } }\nP { }",
    "PROTO P [ inputOutput SFBool b TRUE ] { DirectionalLight { on IS b on FALSE } }\nP { }\nP { b FALSE }",
]):
    T(f"iswb-{i:02d}", V + p)

for i, g in enumerate([
    "GeoViewpoint { }", "GeoViewpoint { navType \"FLY\" }", "GeoViewpoint { navType [ \"WALK\" \"ANY\" ] headlight FALSE }", "GeoViewpoint { headlight FALSE }",
    "GeoViewpoint { set_navType \"NONE\" }", "GeoViewpoint { navType_changed \"NONE\" }", "GeoViewpoint { navType \"EXAMINE\" \"ANY\" }",
    "GeoViewpoint { navigationInfo NavigationInfo { type \"FLY\" } }", "DEF G GeoViewpoint { navType \"FLY\" }\nUSE G",
    "PROTO P [ ] { GeoViewpoint { navType \"FLY\" } }\nP { }", "Group { children [ GeoViewpoint { navType \"FLY\" } GeoViewpoint { headlight FALSE } ] }",
]):
    for v in ("3.0", "3.2", "3.3", "4.0"):
        T(f"geo-{i:02d}-{v}", f"#X3D V{v} utf8\nPROFILE Full\n{g}")

# --- where the parser stops: the error text shows the line, a caret, and cuts a line over 80 characters around the column ---------------------
for pad in [0, 1, 20, 38, 39, 40, 41, 42, 60, 70, 79, 80, 81, 100, 160]:
    for kind, lead in [("ascii", "x"), ("bmp", "\u00e9"), ("astral", "\U0001F600"), ("tab", "\t")]:
        T(f"errpos-{pad:03d}-{kind}", V + f'WorldInfo {{ title "{lead * pad}" nope 1 }}\n' + f'WorldInfo {{ title "{lead * pad}" Nonsense }}')
        T(f"errpos-eof-{pad:03d}-{kind}", V + f'WorldInfo {{ title "{lead * pad}" info [ "{lead * pad}"')

T("errpos-crlf-long", V.replace("\n", "\r\n") + "WorldInfo { title \"" + "x" * 100 + "\" }\r\nGroup { nope }")
T("errpos-first-line", "Group { nope }")
T("errpos-first-line-eof", "Group {")
T("errpos-second-line", "# c\nGroup {")
T("errpos-many-lines", "\n".join(["# comment"] * 30) + "\nGroup {")

# --- sweeps over the node table of the pinned source (profiles, components and levels, specification versions) ----------------------------
import re

PIN = os.path.join(root, "oracle", "x_ite-pin", "src", "x_ite", "Components")


def node_table():
    """(name, component, level, fromVersion) of every concrete node of the pinned source, in a stable order."""
    nodes = []
    for d, _, files in os.walk(PIN):
        for f in files:
            if not f.endswith(".js"):
                continue
            text = open(os.path.join(d, f), encoding="utf-8").read()
            for m in re.finditer(r'getStaticProperties \("(\w+)", "(\w+)", (\d+), "?(\w*)"?, "([\d.]+)"', text):
                if not m.group(1).startswith("X3D"):
                    nodes.append((m.group(1), m.group(2), int(m.group(3)), m.group(5)))
    return sorted(set(nodes))


NODES = node_table()
assert len(NODES) > 200, len(NODES)
ALL = "\n".join(f"{n} {{ }}" for n, _, _, _ in NODES)

for profile in ["Core", "Interchange", "CADInterchange", "Interactive", "MPEG-4", "Immersive", "Full"]:
    T(f"sweep-profile-{profile}", f"#X3D V3.3 utf8\nPROFILE {profile}\n{ALL}")

for version in ["3.0", "3.1", "3.2", "3.3", "4.0", "4.1", "5.0"]:
    T(f"sweep-version-{version}", f"#X3D V{version} utf8\nPROFILE Full\n{ALL}")

T("sweep-vrml97", V + ALL)

# The Annotation component is in the node table of the pinned source, but SupportedComponents.js has it commented out, so the pinned
# `COMPONENT Annotation:n` is an error; the port supports it (owner decision of REA batch 04): the case sweep-annotation-* pins that.
for component in sorted({c for _, c, _, _ in NODES} - {"Annotation"}):
    for level in sorted({l for _, c, l, _ in NODES if c == component}):
        mine = "\n".join(f"{n} {{ }}" for n, c, l, _ in NODES if c == component)
        T(f"sweep-component-{component}-{level}", f"#X3D V3.3 utf8\nPROFILE Core\nCOMPONENT {component}:{level}\n{mine}")

for level in (1, 2):
    T(f"sweep-annotation-{level}", f"#X3D V3.3 utf8\nPROFILE Core\nCOMPONENT Annotation:{level}\n" + "\n".join(f"{n} {{ }}" for n, c, _, _ in NODES if c == "Annotation"))
T("sweep-annotation-no-component", "#X3D V3.3 utf8\nPROFILE Full\n" + "\n".join(f"{n} {{ }}" for n, c, _, _ in NODES if c == "Annotation"))

# --- every field type, every list length: the value printers and their line breaks ------------------------------------------------------
FLOATS = ["0", "1", "-1.5", "0.001", "123456.789", "1e-7", "1e21", "0.1", "3.14159265358979", "-0", "100", "2.5e-10", "0.30000000000000004", "7"]


def values(ftype, count):
    """`count` values of one MF type's element, as source text."""
    out = []
    k = 0
    for i in range(count):
        def num():
            nonlocal k
            k += 1
            return FLOATS[(i * 3 + k) % len(FLOATS)]
        if ftype in ("MFBool",):
            out.append(["TRUE", "FALSE"][i % 2])
        elif ftype == "MFInt32":
            out.append(str(i * 7 % 11 - 1))
        elif ftype in ("MFFloat", "MFDouble", "MFTime"):
            out.append(num())
        elif ftype in ("MFVec2f", "MFVec2d"):
            out.append(" ".join(num() for _ in range(2)))
        elif ftype in ("MFVec3f", "MFVec3d", "MFColor"):
            out.append(" ".join(num() for _ in range(3)))
        elif ftype in ("MFVec4f", "MFVec4d", "MFRotation", "MFColorRGBA"):
            out.append(" ".join(num() for _ in range(4)))
        elif ftype in ("MFMatrix3f", "MFMatrix3d"):
            out.append(" ".join(num() for _ in range(9)))
        elif ftype in ("MFMatrix4f", "MFMatrix4d"):
            out.append(" ".join(num() for _ in range(16)))
        elif ftype == "MFString":
            out.append('"' + ["a", "b c", "d\\\"e", "", "long " * 12 + "text"][i % 5] + '"')
        elif ftype == "MFImage":
            out.append("1 1 3 0x" + "%06X" % (i * 99991 % 0xFFFFFF))
    return out


for ftype in ["MFBool", "MFInt32", "MFFloat", "MFDouble", "MFTime", "MFVec2f", "MFVec2d", "MFVec3f", "MFVec3d", "MFColor", "MFVec4f", "MFVec4d", "MFRotation",
              "MFColorRGBA", "MFMatrix3f", "MFMatrix3d", "MFMatrix4f", "MFMatrix4d", "MFString", "MFImage"]:
    for n in [0, 1, 2, 3, 4, 7, 10, 30]:
        vs = values(ftype, n)
        T(f"mfprint-{ftype}-{n:02d}", V + f"Script {{ field {ftype} f [ {' '.join(vs)} ] url \"ecmascript:\" }}")
        if n in (3, 10):
            T(f"mfprint-{ftype}-{n:02d}-commas", V + f"Script {{ field {ftype} f [ {', '.join(vs)} ] url \"ecmascript:\" }}")
        if n == 1:
            T(f"mfprint-{ftype}-{n:02d}-bare", V + f"Script {{ field {ftype} f {vs[0]} url \"ecmascript:\" }}")

for ftype, vs in [
    ("SFBool", ["TRUE", "FALSE"]), ("SFInt32", ["0", "-5", "2147483647"]), ("SFFloat", ["1.5", "-0", "1e-7"]), ("SFDouble", ["1.5", "1e21", "-0"]),
    ("SFTime", ["-1", "1e9", "0.5"]), ("SFString", ['""', '"a b"', '"q\\"x"', '"line\nbreak"']), ("SFColor", ["0 0 0", "1 0.5 0.25"]),
    ("SFColorRGBA", ["0 0 0 0", "1 .5 .25 .125"]), ("SFVec2f", ["1 2", "-0 1e-7"]), ("SFVec2d", ["1 2", "1e21 1e-7"]), ("SFVec3f", ["1 2 3", "0.1 0.2 0.3"]),
    ("SFVec3d", ["1 2 3", "1e21 -1e-7 0"]), ("SFVec4f", ["1 2 3 4"]), ("SFVec4d", ["1 2 3 4", "1e21 0 0 1"]), ("SFRotation", ["0 0 1 0", "0 1 0 3.14159265358979"]),
    ("SFMatrix3f", ["1 0 0 0 1 0 0 0 1", "1 2 3 4 5 6 7 8 9"]), ("SFMatrix3d", ["1 0 0 0 1 0 0 0 1"]),
    ("SFMatrix4f", ["1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1", "1 2 3 4 5 6 7 8 9 10 11 12 13 14 15 16"]), ("SFMatrix4d", ["1 0 0 0 0 1 0 0 0 0 1 0 0 0 0 1"]),
    ("SFImage", ["0 0 0", "1 1 1 0x80", "2 2 4 0xFF0000FF 0x00FF00FF 0x0000FFFF 0xFFFFFFFF"]), ("SFNode", ["NULL", "Group { }", "DEF D Group { }"]),
]:
    for j, v in enumerate(vs):
        T(f"sfprint-{ftype}-{j}", V + f"Script {{ field {ftype} f {v} url \"ecmascript:\" }}")
        T(f"sfprint-{ftype}-{j}-proto", V + f"PROTO P [ inputOutput {ftype} f {v} ] {{ Group {{ }} }}\nP {{ }}\nP {{ f {v} }}")

for n in [0, 1, 2, 5, 12]:
    T(f"mfprint-MFNode-{n:02d}", V + "Script { field MFNode f [ " + " ".join(f"Group {{ children [ Transform {{ translation {i} 0 0 }} ] }}" for i in range(n)) + " ] url \"ecmascript:\" }")
    T(f"mfprint-MFNode-{n:02d}-null", V + "Script { field MFNode f [ " + " ".join("NULL" for _ in range(n)) + " ] url \"ecmascript:\" }")
    T(f"mfprint-MFNode-{n:02d}-def", V + "Script { field MFNode f [ " + " ".join(f"DEF D{i} Group {{ }} USE D{i}" for i in range(n)) + " ] url \"ecmascript:\" }")

for depth in [1, 2, 5, 12, 25]:
    T(f"nest-{depth:02d}", V + "".join("Transform { translation 1 0 0 children [ " for _ in range(depth)) + "Shape { geometry Box { } }" + " ] }" * depth)
    T(f"nest-proto-{depth:02d}", V + "PROTO P0 [ ] { Group { } }\n" + "".join(f"PROTO P{i} [ ] {{ P{i - 1} {{ }} }}\n" for i in range(1, depth + 1)) + f"P{depth} {{ }}")

def write_cases(path):
    os.makedirs(os.path.dirname(path), exist_ok=True)
    with open(path, "w") as f:
        json.dump(cases, f)


def run(out, tag="cases"):
    os.makedirs(os.path.dirname(out), exist_ok=True)
    write_cases(f"{tmp}/{tag}.json")
    return subprocess.Popen(["node", os.path.join(root, "oracle", "x_ite-classic-edge.mjs"), f"{tmp}/{tag}.json", out], cwd=root)


LONE = re.compile("[\ud800-\udbff](?![\udc00-\udfff])|(?<![\ud800-\udbff])[\udc00-\udfff]")


def well_formed(value):
    """JSON text cannot carry a lone UTF-16 surrogate. X_ITE's error text can: it cuts a long line with substring, in the middle of a
    surrogate pair when the column is there. Each one is written as U+FFFD, which is what the port's UTF-16 conversion gives."""
    if isinstance(value, str):
        return LONE.sub("\ufffd", value)
    if isinstance(value, list):
        return [well_formed(v) for v in value]
    if isinstance(value, dict):
        return {k: well_formed(v) for k, v in value.items()}
    return value


def merged(recorded):
    """The recorder's results with the input of each case."""
    by_name = {c["name"]: c for c in cases}
    results = [r for r in recorded["results"] if r["name"] in by_name]
    assert len(results) == len(cases), "the recording lacks a case"
    return {
        "pinned": recorded["pinned"],
        "results": [well_formed({**by_name[r["name"]], **{k: v for k, v in r.items() if k != "name"}}) for r in results],
    }


def write_baseline(recorded, dest):
    os.makedirs(os.path.dirname(dest), exist_ok=True)
    with open(dest, "w") as f:
        json.dump(merged(recorded), f, indent=1, ensure_ascii=False)
        f.write("\n")


if __name__ == "__main__":
    mode = sys.argv[1] if len(sys.argv) > 1 else "cases"

    if mode == "cases":
        write_cases(f"{tmp}/cases.json")
        print(len(cases), "cases")
    elif mode == "xite":
        if run(f"{tmp}/xite.json").wait():
            sys.exit("the recorder failed")
    elif mode == "merge":
        # merge SRC DEST: the baseline file from one recorder output (for development; `record` is the only writer of the tracked file)
        write_baseline(json.load(open(sys.argv[2])), sys.argv[3])
    elif mode == "record":
        # Three recordings, side by side (each uses six browsers); they must be identical.
        procs = [(i, run(f"{tmp}/{i}/classic-edge.json", f"cases{i}")) for i in (1, 2, 3)]
        if any(p.wait() for _, p in procs):
            sys.exit("a recorder failed")
        runs = [open(f"{tmp}/{i}/classic-edge.json", "rb").read() for i in (1, 2, 3)]
        sums = [hashlib.sha256(r).hexdigest() for r in runs]
        print(len(cases), "cases", " ".join(s[:16] for s in sums), "identical" if len(set(sums)) == 1 else "DIFFERENT")
        if len(set(sums)) != 1:
            sys.exit("the three X_ITE recordings differ")
        write_baseline(json.loads(runs[0]), os.path.join(root, "oracle", "classic-edge-baseline", "classic-edge.json"))
        shutil.rmtree(tmp, ignore_errors=True)
