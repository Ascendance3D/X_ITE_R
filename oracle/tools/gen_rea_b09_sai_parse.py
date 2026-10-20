#!/usr/bin/env python3
"""Generate the parser edge-case worlds oracle/synthetic-rea-b09-sai/parse-*.wrl (REA batch 09, scene-sai lane).

Each world feeds a list of source snippets to Browser.createX3DFromString and records, per snippet, the printed scene
(`toVRMLString`) or the rejection message in the MFString field `out`, so pinned X_ITE and the port are compared on the
parser (VRMLParser / GoldenGate dispatch) with the same Script code. Authored; synthetic inputs only.
"""
import json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-b09-sai")

H = "#X3D V3.3 utf8\nPROFILE Immersive\n"
H4 = "#X3D V4.0 utf8\nPROFILE Full\n"
V = "#VRML V2.0 utf8\n"

groups = {}

groups["parse-header"] = [
    V + "Group { }",
    "#VRML V2.0 utf8 comment text\nGroup { }",
    "#VRML V2.0 ascii\nGroup { }",
    "#VRML V1.0 ascii\nSeparator { }",
    "#VRML V3.0 utf8\nGroup { }",
    "#X3D V3.0 utf8\nPROFILE Core\nGroup { }",
    "#X3D V3.2 utf8\nPROFILE Interchange\nGroup { }",
    "#X3D V4.0 utf8\nPROFILE Full\nGroup { }",
    "#X3D V4.1 utf8\nPROFILE Full\nGroup { }",
    "#X3D V5.0 utf8\nPROFILE Full\nGroup { }",
    "#X3D V3.3 utf8\nGroup { }",
    "#X3D V3.3 utf8\nPROFILE Nonsense\nGroup { }",
    "#X3D V3.3 utf8\nPROFILE Core\nCOMPONENT Geospatial:1\nGroup { }",
    "#X3D V3.3 utf8\nPROFILE Core\nCOMPONENT Geospatial:9\nGroup { }",
    "#X3D V3.3 utf8\nPROFILE Core\nCOMPONENT Nonsense:1\nGroup { }",
    "#X3D V3.3 utf8\nPROFILE Core\nCOMPONENT Geospatial : 1\nCOMPONENT Navigation : 2\nGroup { }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nUNIT length foot 0.3048\nUNIT angle degree 0.0174532925199433\nTransform { translation 1 2 3 rotation 0 0 1 90 }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nUNIT force newton 2\nUNIT nothing x 3\nGroup { }",
    "#X3D V3.3 utf8\nPROFILE Immersive\nMETA \"a\" \"b\"\nMETA \"a\" \"c\"\nMETA \"\" \"d\"\nGroup { }",
    "﻿#VRML V2.0 utf8\nGroup { }",
    "# only a comment",
    "#VRML V2.0 utf8",
    "#X3D V3.3 utf8\nPROFILE Immersive",
    "Group { }",
    "  \n\t Group { }  # trailing\n",
    "DEF A Group { }\nUSE A",
    "PROTO P [ ] { Group { } }\nP { }",
    "ROUTE A.x TO B.y",
    "#VRML V2.0 utf8\r\nGroup {\r\n}\r\n",
    "#VRML V2.0 utf8\rGroup { }\r",
    "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\nGroup { }\nCOMPONENT Navigation:1\n",
    "#X3D V3.3 utf8\nUNIT length foot 0.3048\nPROFILE Immersive\nGroup { }",
]

groups["parse-values"] = [
    H + "Transform { translation 1e3 -.5 +2 }",
    H + "Transform { translation 1. .5e-3 -0.0 }",
    H + "Transform { translation 1,2,3 }",
    H + "Transform { translation 1 2 }",
    H + "Transform { translation 1 2 3 4 }",
    H + "Transform { translation 0x10 0Xff -0x1 }",
    H + "Transform { translation NaN Infinity -Infinity }",
    H + "Transform { translation 1e400 -1e400 1e-400 }",
    H + "Transform { rotation 0 0 0 1 }",
    H + "Transform { rotation 0 1 0 1e10 }",
    H + "Transform { scale 0 0 0 }",
    H + "Transform { center 1.23456789012345678 2.5e-7 123456789.123456789 }",
    H + "Material { shininess 0.5 transparency 2 diffuseColor 2 -1 .5 }",
    H + "Shape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Box { size 1 2 3 } }",
    H + "IndexedFaceSet { coordIndex [0 1 2 -1, 3 4 5 -1] }",
    H + "IndexedFaceSet { coordIndex [0, 1, 2, -1 3 4 5 -1,] }",
    H + "IndexedFaceSet { coordIndex 0x1 }",
    H + "IndexedFaceSet { coordIndex [ 1.5 ] }",
    H + "IndexedFaceSet { coordIndex [ 0 1 2 # comment\n -1 ] }",
    H + "Coordinate { point [ 0 0 0, 1 0 0, 0 1 0 ] }",
    H + "Coordinate { point 0 0 0 }",
    H + "Coordinate { point [ 0 0 0 1 ] }",
    H + "Coordinate { point [ ] }",
    H + "Coordinate { point [ , ] }",
    H + "Text { string [ \"a\" \"b\", \"c\" ] }",
    H + "Text { string \"a\\\"b\\\\c\\nd\" }",
    H + "Text { string \"multi\nline\" }",
    H + "Text { string \"é€\U0001F600\" }",
    H + "Text { string [ ] }",
    H + "Text { string a }",
    H + "WorldInfo { title \"x\" info [ \"a\" \"b\" ] }",
    H + "WorldInfo { info [ \"a\" \"b\" \"c\" ] title \"t\" }",
    H + "Switch { whichChoice -1 choice [ Group { } Group { } ] }",
    H + "Switch { whichChoice 0 children Group { } }",
    H + "Switch { whichChoice 1.5 }",
    H + "Group { children [ ] }",
    H + "Group { children [ Group { }, Group { } ] }",
    H + "Group { children Group { } Group { } }",
    H + "Group { children [ NULL ] }",
    H + "Group { children NULL }",
    H + "Group { bboxSize -1 -1 -1 bboxCenter 0 0 0 }",
    H + "Group { metadata MetadataString { name \"n\" value [ \"v\" ] } }",
    H + "MetadataSet { name \"s\" value [ MetadataDouble { name \"d\" value [ 1 2 ] } MetadataInteger { name \"i\" value 5 } ] }",
    H + "Anchor { url [ \"a.wrl\" \"b.wrl\" ] description \"d\" parameter [ \"target=x\" ] }",
    H + "NavigationInfo { type [ \"EXAMINE\" \"ANY\" ] avatarSize [ 0.25 1.6 0.75 ] }",
    H + "NavigationInfo { type \"FLY\" speed 5 headlight FALSE visibilityLimit 0 }",
    H + "Viewpoint { position 0 0 10 orientation 0 1 0 0.5 fieldOfView 1 description \"v\" }",
    H + "Inline { url [ \"x.wrl\" ] bboxSize 1 1 1 }",
    H + "DirectionalLight { direction 0 -1 0 on TRUE intensity 0.5 }",
    H + "DirectionalLight { on true }",
    H + "DirectionalLight { on 1 }",
    H + "TimeSensor { cycleInterval 2 loop TRUE startTime 1e9 }",
    H + "ImageTexture { url \"a.png\" repeatS FALSE }",
    H + "PixelTexture { image 2 2 3 0xFF0000 0x00FF00 0x0000FF 0xFFFFFF }",
    H + "PixelTexture { image 0 0 0 }",
    H + "PixelTexture { image 1 1 1 255 }",
    H + "Color { color [ 1 0 0, 0 1 0 ] }",
    H + "ColorInterpolator { key [ 0 1 ] keyValue [ 1 0 0, 0 1 0 ] }",
    H + "OrientationInterpolator { key [ 0 1 ] keyValue [ 0 1 0 0, 0 1 0 3.14 ] }",
    H + "Script { url \"ecmascript:function f(){}\" }",
    H + "Script { field SFFloat x 1.5 eventIn SFBool b eventOut SFInt32 o url \"ecmascript:\" }",
    H + "Script { initializeOnly SFFloat x 1.5 inputOnly SFBool b outputOnly SFInt32 o inputOutput MFString s [ \"a\" ] url [\"ecmascript:\" \"b.js\"] }",
    H + "Script { inputOutput SFNode n NULL inputOutput SFNode m Group { } inputOutput MFNode k [ ] }",
    H + "Script { inputOutput SFImage i 1 1 1 0x01 inputOutput SFRotation r 0 1 0 1 }",
    H + "Script { directOutput TRUE mustEvaluate TRUE url \"ecmascript:\" }",
    H + "Script { inputOutput SFBool b TRUE inputOutput SFBool b FALSE }",
    H + "Script { inputOnly SFTime \"quoted\" }",
]

groups["parse-def-proto"] = [
    H + "DEF A Group { }\nDEF A Transform { }\nUSE A",
    H + "USE Missing",
    H + "DEF A Group { children USE A }",
    H + "DEF A Group { }\nGroup { children [ USE A USE A ] }",
    H + "DEF A_B-C Group { }",
    H + "DEF 1A Group { }",
    H + "DEF \"A\" Group { }",
    H + "DEF A.B Group { }",
    H + "DEF Group Group { }",
    H + "DEF NULL Group { }",
    H + "DEF TRUE Group { }",
    H + "DEF A NULL",
    H + "DEF A Nonsense { }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Transform { translation IS f } }\nP { f 2 }",
    H + "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { }",
    H + "PROTO P [ inputOutput SFVec3f t 1 2 3 ] { Transform { translation IS t } }\nP { t 4 5 6 }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Transform { translation IS f } }\nP { }",
    H + "PROTO P [ inputOnly SFBool b outputOnly SFInt32 o initializeOnly SFString s \"x\" ] { Group { } }\nP { s \"y\" }",
    H + "PROTO P [ field SFFloat f 1 eventIn SFBool b ] { Group { } }\nP { }",
    H + "PROTO P [ ] { Group { } Group { } }\nP { }",
    H + "PROTO P [ ] { }\nP { }",
    H + "PROTO P [ ] { Group { } }\nPROTO P [ ] { Transform { } }\nP { }",
    H + "PROTO P [ ] { Group { } }\nPROTO Q [ ] { P { } }\nQ { }",
    H + "PROTO P [ ] { P { } }\nP { }",
    H + "PROTO P [ ] { DEF A Group { } }\nP { }\nP { }",
    H + "PROTO P [ inputOutput SFNode n NULL ] { Group { children IS n } }\nP { n Transform { } }",
    H + "PROTO P [ inputOutput MFNode c [ ] ] { Group { children IS c } }\nP { c [ Transform { } Group { } ] }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Script { inputOutput SFFloat g IS f url \"ecmascript:\" } }\nP { }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Transform { translation IS g } }\nP { }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Transform { scale IS f } }\nP { }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Transform { translation 1 1 1 IS f } }\nP { }",
    H + "PROTO P [ inputOutput SFFloat f 1 inputOutput SFFloat f 2 ] { Group { } }\nP { }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Group { } ROUTE X.a TO Y.b }\nP { }",
    H + "PROTO P [ ] { DEF A Transform { } DEF B Transform { } ROUTE A.translation_changed TO B.set_translation }\nP { }",
    H + "PROTO P [ inputOutput SFVec3f t 0 0 0 ] { DEF A Transform { translation IS t } }\nP { t 1 1 1 }",
    H + "PROTO P [ ] { PROTO Inner [ ] { Group { } } Inner { } }\nP { }",
    H + "PROTO P [ ] { EXTERNPROTO E [ ] \"e.wrl\" Group { } }\nP { }",
    H + "EXTERNPROTO E [ inputOutput SFFloat f ] \"missing.wrl#E\"\nE { f 1 }",
    H + "EXTERNPROTO E [ inputOutput SFFloat f ] [ \"a.wrl\" \"b.wrl\" ]\nE { }",
    H + "EXTERNPROTO E [ inputOutput SFFloat f ] \"\"\nE { }",
    H + "EXTERNPROTO E [ ] \"missing.wrl\"\nEXTERNPROTO E [ ] \"missing2.wrl\"\nE { }",
    H + "EXTERNPROTO E [ SFFloat f ] \"missing.wrl\"",
    H + "EXTERNPROTO E [ inputOutput SFFloat f ] \"missing.wrl\"\nPROTO E [ ] { Group { } }\nE { }",
    H + "PROTO Group [ ] { Transform { } }\nGroup { }",
    H + "PROTO Delay [ ] { Transform { } }\nDelay { }",
    H + "DEF G Group { }\nPROTO P [ ] { USE G }\nP { }",
    H + "PROTO P [ ] { Group { } }\nDEF X P { }\nUSE X",
    H + "DEF A Group { }\nIMPORT A.b AS c",
    H + "DEF I Inline { url \"missing.wrl\" }\nIMPORT I.E1\nEXPORT I",
    H + "DEF I Inline { }\nIMPORT I.E1 AS X\nGroup { children USE X }",
    H + "DEF I Inline { }\nIMPORT I.E1\nIMPORT I.E1",
    H + "DEF I Inline { }\nIMPORT I.E1 AS X\nIMPORT I.E2 AS X",
    H + "IMPORT Missing.E1",
    H + "DEF A Group { }\nEXPORT A AS B\nEXPORT A AS B",
    H + "DEF A Group { }\nEXPORT A\nEXPORT B",
    H + "DEF A Group { }\nEXPORT A AS",
]

groups["parse-routes"] = [
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed TO B.set_translation",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation TO B.translation",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.set_translation TO B.set_translation",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed TO B.translation_changed",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed TO B.set_scale",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.nope TO B.set_translation",
    H + "DEF A Transform { }\nROUTE A.translation_changed TO B.set_translation",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed TO B.set_translation\nROUTE A.translation_changed TO B.set_translation",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed TO B.set_translation ROUTE A.scale_changed TO B.set_scale",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A . translation_changed TO B . set_translation",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed IN B.set_translation",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed to B.set_translation",
    H + "DEF A TimeSensor { }\nDEF B PositionInterpolator { key [ 0 1 ] keyValue [ 0 0 0 1 1 1 ] }\nDEF C Transform { }\nROUTE A.fraction_changed TO B.set_fraction\nROUTE B.value_changed TO C.set_translation",
    H + "DEF A TimeSensor { }\nDEF C Transform { }\nROUTE A.fraction_changed TO C.set_translation",
    H + "DEF A TimeSensor { }\nDEF C Transform { }\nROUTE A.cycleTime TO C.metadata",
    H + "DEF S Script { inputOnly SFTime t outputOnly SFBool b url \"ecmascript:\" }\nDEF T TimeSensor { }\nROUTE T.cycleTime TO S.t\nROUTE S.b TO T.set_enabled",
    H + "DEF S Script { eventIn SFTime t eventOut SFBool b url \"ecmascript:\" }\nDEF T TimeSensor { }\nROUTE T.cycleTime TO S.t\nROUTE S.b TO T.set_enabled",
    H + "DEF S Script { inputOutput SFFloat f 1 url \"ecmascript:\" }\nDEF T TimeSensor { }\nROUTE T.fraction_changed TO S.f\nROUTE S.f_changed TO T.set_cycleInterval",
    H + "DEF S Script { inputOutput SFFloat f 1 url \"ecmascript:\" }\nDEF T TimeSensor { }\nROUTE T.fraction_changed TO S.set_f",
    H + "DEF A Transform { }\nPROTO P [ ] { Group { } }\nROUTE A.translation_changed TO P.set_translation",
    H + "DEF A Transform { }\nPROTO P [ inputOnly SFVec3f v ] { Group { } }\nDEF Q P { }\nROUTE A.translation_changed TO Q.v",
    H + "DEF A Transform { }\nPROTO P [ inputOutput SFVec3f v 0 0 0 ] { Group { } }\nDEF Q P { }\nROUTE A.translation_changed TO Q.set_v",
    H + "DEF A Transform { }\nPROTO P [ outputOnly SFVec3f v ] { Group { } }\nDEF Q P { }\nROUTE Q.v TO A.set_translation",
    H + "DEF A Transform { }\nPROTO P [ outputOnly SFVec3f v ] { Group { } }\nDEF Q P { }\nROUTE Q.v_changed TO A.set_translation",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed TO B.set_translation\nDEF A Group { }",
    H + "DEF A Transform { }\nROUTE A.translation_changed TO A.set_translation",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.children_changed TO B.addChildren",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.children_changed TO B.set_children",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.metadata_changed TO B.set_metadata",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.bboxSize TO B.set_bboxSize",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed TO",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed",
    H + "ROUTE",
    H + "DEF A Transform { }\nDEF B Transform { }\nROUTE A.translation_changed TO B.set_translation\nROUTE B.translation_changed TO A.set_translation",
]

groups["parse-errors"] = [
    H + "Group {",
    H + "Group { children [ }",
    H + "Group { children [ Group { } }",
    H + "Group { } }",
    H + "Group { children ] }",
    H + "Group { nope 1 }",
    H + "Group { children 5 }",
    H + "Transform { translation a b c }",
    H + "Transform { translation \"1\" \"2\" \"3\" }",
    H + "Transform { translation [ 1 2 3 ] }",
    H + "Transform { translation 1 2 3 translation 4 5 6 }",
    H + "Text { string \"unterminated }",
    H + "Text { string \"a\" \"b\" }",
    H + "Text { string [ \"a\" \"b\" }",
    H + "Material { diffuseColor 1 0 }",
    H + "Material { diffuseColor 1 0 0 0 }",
    H + "Shape { geometry Material { } }",
    H + "Shape { appearance Box { } }",
    H + "Shape { geometry Box { } geometry Sphere { } }",
    H + "Nonsense { }",
    H + "nonsense",
    H + "{ }",
    H + "Group Group",
    H + "Group { } Nonsense",
    H + "Group { } 5",
    H + "Group { } \"text\"",
    H + "Group { } ,",
    H + "Group { , }",
    H + "Group { children [ , ] }",
    H + "Group { children [ Group { } , , Group { } ] }",
    H + "Script { url \"ecmascript:\" nope 1 }",
    H + "Script { inputOnly SFFloat }",
    H + "Script { inputOnly NoType x }",
    H + "Script { inputOnly SFFloat 5 }",
    H + "Script { inputOutput SFFloat f \"x\" }",
    H + "Script { inputOutput SFInt32 i 1.5 }",
    H + "Script { inputOutput SFBool b 1 }",
    H + "Script { inputOutput SFBool b yes }",
    H + "PROTO",
    H + "PROTO P",
    H + "PROTO P [",
    H + "PROTO P [ ]",
    H + "PROTO P [ ] {",
    H + "PROTO P [ inputOutput SFFloat ] { }",
    H + "PROTO P [ inputOutput NoType f 1 ] { Group { } }",
    H + "PROTO P [ bogus SFFloat f 1 ] { Group { } }",
    H + "PROTO P [ inputOutput SFFloat f ] { Group { } }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nP { f }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Group { } }\nP { g 1 }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Transform { translation IS } }\nP { }",
    H + "PROTO P [ inputOutput SFFloat f 1 ] { Transform { translation IS 5 } }\nP { }",
    H + "EXTERNPROTO",
    H + "EXTERNPROTO E",
    H + "EXTERNPROTO E [ ]",
    H + "EXTERNPROTO E [ ] 5",
    H + "DEF",
    H + "DEF A",
    H + "USE",
    H + "COMPONENT",
    H + "PROFILE",
    H + "IMPORT",
    H + "EXPORT",
    H + "META",
    H + "UNIT",
    "#X3D V3.3 utf8\nPROFILE Immersive\nPROFILE Core\nGroup { }",
    "#X3D V3.3 utf8\nGroup { }\nPROFILE Immersive",
    "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Core:1\nUNIT length foot 0.3\nMETA \"a\" \"b\"\nCOMPONENT Core:2",
    "\u0000#VRML V2.0 utf8\nGroup { }",
    "#VRML V2.0 utf8\nGroup { } \u0000",
    H + "Group { children [ " + "Group { " * 40 + "} " * 40 + "] }",
    H + "Group { } # c1 # c2\n# c3\nGroup { }",
    H + "#/* block */# Group { }",
    H + "Group { children [ #/* block */# Group { } ] }",
    H + "Group { children [ Group { } #/* unterminated ] }",
]

# XMLParser.nodeElement prints "Unknown node type" with console.error (the log level is compared: sai-parse-console.json).
X = "<X3D profile=\"Full\" version=\"4.0\"><Scene>"
groups["parse-xml-unknown"] = [
    X + "<Bogus/><Transform DEF=\"T\" translation=\"1 2 3\"><Nonsense2 size=\"1\"/><Group DEF=\"G\"/></Transform><Shape><Box/></Shape></Scene></X3D>",
    X + "<Shape><Bogus3/><Box size=\"1 2 3\"/></Shape><Group DEF=\"K\"/></Scene></X3D>",
    X + "<Bogus4><Group DEF=\"inside\"/></Bogus4><Group DEF=\"after\"/></Scene></X3D>",
    X + "<Group><Bogus5/><Bogus6/></Group><Transform DEF=\"X\"/></Scene></X3D>",
    X + "<Group USE=\"Nope\"/><Bogus7/><Group DEF=\"ok\"/></Scene></X3D>",
]

groups["parse-formats"] = [
    "<?xml version=\"1.0\"?><X3D profile=\"Full\" version=\"4.0\"><Scene><Group DEF=\"G\"/></Scene></X3D>",
    "<X3D profile=\"Immersive\" version=\"3.3\"><head><component name=\"Scripting\" level=\"1\"/><meta name=\"a\" content=\"b\"/><unit category=\"length\" name=\"foot\" conversionFactor=\"0.3048\"/></head><Scene><Transform translation=\"1 2 3\"/></Scene></X3D>",
    "<X3D><Scene><Group/></Scene></X3D>",
    "<X3D profile=\"Core\" version=\"3.3\"><Scene><Nonsense/></Scene></X3D>",
    "<X3D profile=\"Core\" version=\"3.3\"><Scene><Group><Shape/></Group></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene><ProtoDeclare name=\"P\"><ProtoInterface><field name=\"f\" type=\"SFFloat\" accessType=\"inputOutput\" value=\"1\"/></ProtoInterface><ProtoBody><Transform><IS><connect nodeField=\"scale\" protoField=\"f\"/></IS></Transform></ProtoBody></ProtoDeclare><ProtoInstance name=\"P\"/></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene><Group DEF=\"A\"/><Group USE=\"A\"/></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene><Group USE=\"Missing\"/></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene><Transform DEF=\"A\"/><Transform DEF=\"B\"/><ROUTE fromNode=\"A\" fromField=\"translation_changed\" toNode=\"B\" toField=\"set_translation\"/></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene><Script DEF=\"S\"><field name=\"x\" type=\"SFFloat\" accessType=\"inputOutput\" value=\"2\"/><![CDATA[ecmascript:function f(){}]]></Script></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene><Shape><Appearance><Material diffuseColor=\"1 0 0\"/></Appearance><Box size=\"1 2 3\"/></Shape></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene><Shape><Box containerField=\"geometry\"/></Shape></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene><MetadataSet name=\"s\"><MetadataString name=\"m\" value='\"a\" \"b\"'/></MetadataSet></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene><Group bboxSize=\"abc\"/></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene><Group nope=\"1\"/></Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\"><Scene></X3D>",
    "<X3D profile=\"Full\" version=\"4.0\">",
    "<Scene><Group/></Scene>",
    "<svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"4\"><rect width=\"2\" height=\"2\" fill=\"red\"/></svg>",
    "<html><body/></html>",
    "<notx3d/>",
    "{\"X3D\":{\"@profile\":\"Full\",\"@version\":\"4.0\",\"Scene\":{\"-children\":[{\"Group\":{\"@DEF\":\"G\"}}]}}}",
    "{\"X3D\":{\"@profile\":\"Full\",\"@version\":\"4.0\",\"Scene\":{\"-children\":[{\"Transform\":{\"@translation\":[1,2,3]}}]}}}",
    "{\"X3D\":{\"Scene\":{\"-children\":[{\"Group\":{}}]}}}",
    "{\"X3D\":{\"@profile\":\"Full\",\"@version\":\"4.0\",\"Scene\":{\"-children\":[{\"Nonsense\":{}}]}}}",
    "{\"X3D\":{\"@profile\":\"Full\",\"@version\":\"4.0\",\"Scene\":{\"-children\":[{\"Group\":{\"@USE\":\"M\"}}]}}}",
    "{\"X3D\":{\"@profile\":\"Full\",\"@version\":\"4.0\",\"Scene\":{\"-children\":[{\"Script\":{\"field\":[{\"@name\":\"f\",\"@type\":\"SFFloat\",\"@accessType\":\"inputOutput\",\"@value\":3}],\"@url\":[\"ecmascript:\"]}}]}}}",
    "{\"X3D\":{\"@profile\":\"Full\",\"@version\":\"4.0\",\"Scene\":{\"-children\":[{\"Shape\":{\"-geometry\":{\"Box\":{\"@size\":[1,2,3]}}}}]}}}",
    "{\"X3D\":{\"@profile\":\"Full\",\"@version\":\"4.0\",\"Scene\":{\"-children\":[{\"Group\":{\"@bboxSize\":\"abc\"}}]}}}",
    "{\"other\":1}",
    "{\"X3D\":",
    "{ \"asset\": { \"version\": \"2.0\" }, \"scenes\": [ { \"nodes\": [] } ], \"scene\": 0 }",
    "{ \"asset\": { \"version\": \"2.0\" }, \"scenes\": [ { \"nodes\": [ 0 ] } ], \"scene\": 0, \"nodes\": [ { \"name\": \"n\", \"translation\": [ 1, 2, 3 ] } ] }",
    "v 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\n",
    "v 0 0 0\nv 1 0 0\nv 0 1 0\nvn 0 0 1\nvt 0 0\nf 1/1/1 2/1/1 3/1/1\n",
    "o thing\nv 0 0 0\nv 1 0 0\nv 0 1 0\nf 1 2 3\nusemtl m\n",
    "# only a comment\nv 0 0 0\n",
    "v 0 0 0\n",
    "solid t\nfacet normal 0 0 1\nouter loop\nvertex 0 0 0\nvertex 1 0 0\nvertex 0 1 0\nendloop\nendfacet\nendsolid t\n",
    "solid t\nendsolid t\n",
    "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nelement face 1\nproperty list uchar int vertex_indices\nend_header\n0 0 0\n1 0 0\n0 1 0\n3 0 1 2\n",
    "ply\nformat ascii 1.0\nelement vertex 3\nproperty float x\nproperty float y\nproperty float z\nproperty uchar red\nproperty uchar green\nproperty uchar blue\nend_header\n0 0 0 255 0 0\n1 0 0 0 255 0\n0 1 0 0 0 255\n",
    "ply\nformat binary_little_endian 1.0\nend_header\n",
    "plain text that is nothing",
    "   ",
    "",
    "\n",
    "12345",
    "null",
    "[]",
]

groups["parse-loads"] = [
    H + "Group { }",
    H + "Inline { url \"assets/missing.wrl\" }",
    H + "Inline { url \"assets/exp.wrl\" }",
    H + "Inline { url [ \"assets/missing.wrl\" \"assets/exp.wrl\" ] }",
    H + "Inline { url \"assets/exp.wrl\" load FALSE }",
    H + "Inline { url \"data:model/x3d+vrml,%23VRML%20V2.0%20utf8%0AGroup%20%7B%7D\" }",
    H + "EXTERNPROTO E [ ] \"assets/missing.wrl#E\"\nE { }",
    H + "EXTERNPROTO E [ ] \"assets/extern-p.wrl#P\"\nE { }",
    H + "EXTERNPROTO E [ ] \"assets/extern-p.wrl\"\nE { }\nE { }",
    H + "EXTERNPROTO E [ ] [ \"assets/missing.wrl\" \"assets/extern-p.wrl#P\" ]\nE { }",
    H + "Inline { url \"assets/nested.wrl\" }",
    H + "EXTERNPROTO E [ ] \"assets/extern-p.wrl#P\"\nInline { url \"assets/exp.wrl\" }\nE { }",
    H + "Script { url \"assets/script.js\" }",
    H + "Script { url \"assets/missing.js\" }",
    H + "ImageTexture { url \"assets/missing.png\" }",
    H + "AudioClip { url \"assets/missing.wav\" }",
]

PREAMBLE = """#X3D V3.3 utf8
PROFILE Immersive
COMPONENT Scripting:1
# Synthetic REA oracle world (generated by oracle/tools/gen_rea_b09_sai_parse.py): %(title)s.
# Each source snippet goes through Browser.createX3DFromString; `out` lists the printed scene or the rejection of each, in snippet order.
DEF OUT Script {
  inputOutput MFString out [ ]
  inputOutput SFInt32 done 0
  inputOnly SFTime cycle
  url "ecmascript:
    var SNIPPETS = %(snippets)s;
    var res = [];
    function strip (s) { return String (s) .replace (/https?:\\\\/\\\\/[^\\\\/]+\\\\//g, ''); }
    function put (i, v) { res [i] = i + ': ' + v; var o = []; for (var k = 0; k < SNIPPETS .length; ++ k) o .push (k in res ? res [k] : k + ': ...'); out = new MFString (... o); }
    function initialize () {
      SNIPPETS .forEach (function (src, i) {
        var p;
        try { p = Browser .createX3DFromString (src); } catch (e) { put (i, 'THROW ' + strip (e .message)); return; }
        p .then (function (sc) {
          var head = [sc .encoding, sc .specificationVersion, sc .profile && sc .profile .name, sc .rootNodes .length, sc .protos .length, sc .externprotos .length, sc .routes .length, sc .importedNodes .length, sc .exportedNodes .length] .join ('/');
          var text; try { text = sc .toVRMLString (); } catch (e) { text = 'PRINT ERR ' + e .message; }
          put (i, head + '\\n' + text);
        }, function (e) { put (i, 'REJ ' + strip (e && e .message)); });
      });
    }
  "
}
"""

import sys

# `--only GROUP INDEX OUT`: one snippet of a group into OUT (to find which snippet a log line belongs to).
only = sys.argv[sys.argv.index("--only") + 1:sys.argv.index("--only") + 4] if "--only" in sys.argv else None

for name, snippets in groups.items():
    if only:
        if name != only[0]:
            continue
        snippets = [snippets[int(only[1])]]
    js = json.dumps(snippets, ensure_ascii=True)
    # The snippet array sits inside a VRML string: \\ and \" are the only escapes the lexer reads.
    vrml = js.replace("\\", "\\\\").replace('"', '\\"')
    text = PREAMBLE % {"title": name, "snippets": vrml}
    # `strip`'s regular expression went through the same escaping by hand above (four backslashes become two in the file).
    text = text.replace("\\\\\\\\", "\\\\")
    open(only[2] if only else os.path.join(out_dir, name + ".wrl"), "w").write(text)
    print(name, len(snippets))
