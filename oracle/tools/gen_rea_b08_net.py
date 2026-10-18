#!/usr/bin/env python3
"""REA batch 08 lane `net` worlds (DIS nodes, InlineGeometry, FontLibrary) for oracle/synthetic-rea-b08-net.

  python3 -I oracle/tools/gen_rea_b08_net.py oracle/synthetic-rea-b08-net

Every file is synthetic. Stepped runtime cases are the *-runtime.wrl worlds with their *.json scripts
(one click to focus the KeySensor, then one key per `step` frames).
"""
import json, os, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "oracle/synthetic-rea-b08-net"
os.makedirs(OUT, exist_ok=True)

HEAD = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Shape:4
COMPONENT Networking:4
COMPONENT DIS:2
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
COMPONENT EventUtilities:1
COMPONENT Text:2
COMPONENT X_ITE:1
"""
LIGHTS = "DirectionalLight { direction -0.4 -0.6 -0.7 intensity 0.9 }"
BG = "Background { skyColor 0.15 0.15 0.18 }"

worlds, scripts = {}, {}


HEAD41 = HEAD.replace("V4.0", "V4.1")


def world(*parts, vp="Viewpoint { position 0 0 8 }", head=HEAD):
    return head + vp + "\n" + "\n".join(parts) + "\n"


def runtime_json(frames, keys, start=20, step=6, console=False, extra_events=()):
    events = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507},
              {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
    caps = []
    for i, k in enumerate(keys):
        f = start + i * step
        events.append({"frame": f, "type": "key", "key": k})
        caps.append(f + 4)
    events += list(extra_events)
    s = {"settle": True, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames,
         "capture": [12] + caps, "dump": [10] + caps + [frames - 1], "events": events}
    if console:
        s["console"] = True
    return s


# ---------------------------------------------------------------------------------------------
# DIS. Fields, defaults, legacy defaults (specification <= 3.3), the `mapping` alias, bboxes.
PORT_R, PORT_W = 41234, 41236  # the loopback sender / listener of oracle/tools/rea_b08_net.py

dis_full = f"""
Group {{ children [
  DEF EM DISEntityManager {{ address "127.0.0.1" applicationID 3 port {PORT_R} siteID 4
    children [ DEF TM DISEntityTypeMapping {{ url "mapping.xml" category 1 country 225 domain 2 extra 3 kind 4 specific 5 subcategory 6 description "types" }} ] }}
  DEF RX ReceiverPdu {{ address "127.0.0.1" port {PORT_R} networkMode "networkReader" applicationID 5 entityID 6 radioID 7 siteID 8
    transmitterApplicationID 9 transmitterEntityID 10 transmitterRadioID 11 transmitterSiteID 12
    receivedPower 1.5 receiverState 2 rtpHeaderExpected TRUE readInterval 0.25 writeInterval 2 whichGeometry 0
    multicastRelayHost "relay.invalid" multicastRelayPort 99 geoCoords 1 2 3 geoSystem [ "UTM" "Z10" "N" ] description "rx" }}
  DEF SG SignalPdu {{ address "127.0.0.1" port {PORT_W} networkMode "networkWriter" applicationID 5 data [ 1 2 3 4 ] dataLength 4
    encodingScheme 6 entityID 7 radioID 8 sampleRate 8000 samples 4 siteID 9 tdlType 10 whichGeometry -1 description "sig" }}
  DEF TX TransmitterPdu {{ address "127.0.0.1" port {PORT_W} networkMode "networkWriter" antennaLocation 1 2 3 antennaPatternLength 4 antennaPatternType 5
    applicationID 6 cryptoKeyID 7 cryptoSystem 8 entityID 9 frequency 10 inputSource 11 lengthOfModulationParameters 12 modulationTypeDetail 13
    modulationTypeMajor 14 modulationTypeSpreadSpectrum 15 modulationTypeSystem 16 power 17.5 radioEntityTypeCategory 18 radioEntityTypeCountry 19
    radioEntityTypeDomain 20 radioEntityTypeKind 21 radioEntityTypeNomenclature 22 radioEntityTypeNomenclatureVersion 23 radioID 24
    relativeAntennaLocation 4 5 6 siteID 25 transmitFrequencyBandwidth 26.5 transmitState 27 description "tx" }}
] }}
DEF DRX ReceiverPdu {{ }}
DEF DSG SignalPdu {{ }}
DEF DTX TransmitterPdu {{ }}
DEF DEM DISEntityManager {{ }}
DEF DTM DISEntityTypeMapping {{ }}
Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.7 0.3 0.2 }} }} geometry Box {{ size 1 1 1 }} }}
"""
worlds["dis-fields.wrl"] = world(BG, LIGHTS, dis_full)
scripts["dis-fields.json"] = runtime_json(60, [])

# Old specification versions: ReceiverPdu applicationID / transmitterApplicationID 1, SignalPdu and
# TransmitterPdu applicationID 1, DISEntityManager `children` also named `mapping`.
legacy_body = """
DEF RX ReceiverPdu { }
DEF SG SignalPdu { }
DEF TX TransmitterPdu { }
DEF EM DISEntityManager { mapping [ DEF TM DISEntityTypeMapping { description "via mapping" } ] }
DEF EM2 DISEntityManager { }
DEF RX2 ReceiverPdu { applicationID 0 transmitterApplicationID 0 }
Shape { appearance Appearance { material Material { diffuseColor 0.2 0.5 0.7 } } geometry Sphere { radius 0.6 } }
"""
for ver in ("3.3", "3.2", "4.0", "4.1"):
    h = HEAD.replace("V4.0", "V" + ver)
    worlds[f"dis-legacy-{ver.replace('.', '')}.wrl"] = world(BG, LIGHTS, legacy_body, head=h)
    scripts[f"dis-legacy-{ver.replace('.', '')}.json"] = runtime_json(40, [])
worlds["dis-legacy-vrml.wrl"] = ("#VRML V2.0 utf8\nViewpoint { position 0 0 8 }\n" + legacy_body.replace("mapping [", "children ["))
scripts["dis-legacy-vrml.json"] = runtime_json(40, [])

# bboxes: getBBox of a PDU is its bboxSize / bboxCenter (empty at the default -1 -1 -1), but a PDU does not
# call setBoundedObject, so no parent adds it. bboxDisplay draws X_ITE's bbox shape (white lines).
bb = f"""
Group {{ bboxSize 20 20 20 children [
  ReceiverPdu {{ bboxSize 2 1 1 bboxCenter -3 2 0 bboxDisplay TRUE }}
  SignalPdu {{ bboxSize 1 2 1 bboxCenter 0 2 0 bboxDisplay TRUE visible FALSE }}
  TransmitterPdu {{ bboxSize 1 1 3 bboxCenter 3 2 0 bboxDisplay TRUE }}
  ReceiverPdu {{ bboxDisplay TRUE }}
  ReceiverPdu {{ bboxSize 1 1 1 bboxCenter -3 -2 0 }}
  SignalPdu {{ bboxSize 2 2 2 bboxCenter 0 -2 0 bboxDisplay TRUE enabled FALSE }}
  Transform {{ translation 3 -2 0 rotation 0 1 0 0.6 scale 1 2 1 children [
    TransmitterPdu {{ bboxSize 1 1 1 bboxDisplay TRUE }}
    Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.3 0.8 0.3 }} }} geometry Box {{ size 0.5 0.5 0.5 }} }} ] }}
] }}
"""
worlds["dis-bbox.wrl"] = world(BG, LIGHTS, bb)
scripts["dis-bbox.json"] = runtime_json(30, [])

# bboxDisplay of PDUs below other parents, with odd sizes: Switch (chosen child only), LOD, Billboard, Anchor, Collision,
# StaticGroup, Inline, PROTO body, a scale 0 Transform, a negative bboxSize, a zero bboxSize.
worlds["dis-bbox-parents.wrl"] = world(BG, LIGHTS, """
PROTO P [ field SFVec3f c 0 0 0 ] { Group { children ReceiverPdu { bboxSize 0.8 0.8 0.8 bboxCenter IS c bboxDisplay TRUE } } }
Transform { translation -3 3 0 children Switch { whichChoice 1 choice [ ReceiverPdu { bboxSize 1 1 1 bboxDisplay TRUE } SignalPdu { bboxSize 1.2 0.6 0.6 bboxDisplay TRUE } TransmitterPdu { bboxSize 0.5 0.5 0.5 bboxDisplay TRUE } ] } }
Transform { translation 0 3 0 children LOD { range [ 100 ] level [ ReceiverPdu { bboxSize 1 0.5 1 bboxDisplay TRUE } SignalPdu { bboxSize 2 2 2 bboxDisplay TRUE } ] } }
Transform { translation 3 3 0 children Billboard { axisOfRotation 0 0 0 children SignalPdu { bboxSize 1 1 0.2 bboxDisplay TRUE } } }
Transform { translation -3 1 0 children Anchor { url "x.wrl" children TransmitterPdu { bboxSize 1 1 1 bboxDisplay TRUE } } }
Transform { translation 0 1 0 children Collision { children ReceiverPdu { bboxSize 1 1 1 bboxDisplay TRUE } } }
Transform { translation 3 1 0 children StaticGroup { children [ ReceiverPdu { bboxSize 1 1 1 bboxDisplay TRUE } Shape { geometry Sphere { radius 0.3 } } ] } }
Transform { translation -3 -1 0 children Inline { url "dis-bbox-inline.wrl" } }
Transform { translation 0 -1 0 children P { c 0 0.5 0 } }
Transform { translation 3 -1 0 scale 0 1 1 children ReceiverPdu { bboxSize 1 1 1 bboxDisplay TRUE } }
Transform { translation -3 -3 0 children ReceiverPdu { bboxSize -1 1 1 bboxDisplay TRUE } }
Transform { translation 0 -3 0 children SignalPdu { bboxSize 0 0 0 bboxDisplay TRUE } }
Transform { translation 3 -3 0 children TransmitterPdu { bboxSize 1 1 -2 bboxCenter 0 0.5 0 bboxDisplay TRUE } }
Transform { translation 0 -4.3 0 rotation 1 0 0 0.5 children TransmitterPdu { bboxSize 1 0 1 bboxDisplay TRUE } }
""", vp="Viewpoint { position 0 0 12 }")
worlds["dis-bbox-inline.wrl"] = "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT DIS:2\nSignalPdu { bboxSize 1 1 1 bboxDisplay TRUE }\n"
scripts["dis-bbox-parents.json"] = runtime_json(30, [])

# viewAll frames the Layer's bbox: PDUs with a large bboxSize must not move the view (X_ITE: they are not bounded
# objects of their parent), a Box with the same bboxSize does.
for name, node in (("pdu", "ReceiverPdu { bboxSize 40 40 40 }"), ("box", "Shape { geometry Box { size 40 40 40 } }"), ("none", "")):
    worlds[f"dis-viewall-{name}.wrl"] = world(BG, LIGHTS,
        "Shape { appearance Appearance { material Material { diffuseColor 0.9 0.6 0.1 } } geometry Sphere { radius 1 } }", node,
        vp="Viewpoint { position 0 0 8 viewAll TRUE }")
    scripts[f"dis-viewall-{name}.json"] = runtime_json(30, [])

# Run-time: field changes by Script, removal from a live Group, Script-created instance.
dis_rt = f"""
DEF G Group {{ children [
  DEF RX ReceiverPdu {{ bboxSize 2 1 1 bboxCenter -2.5 2 0 bboxDisplay TRUE address "127.0.0.1" port {PORT_R} networkMode "networkReader" }}
  DEF SG SignalPdu {{ bboxSize 1 2 1 bboxCenter 0 2 0 bboxDisplay TRUE address "127.0.0.1" port {PORT_W} networkMode "networkWriter" }}
  DEF TX TransmitterPdu {{ bboxSize 1 1 3 bboxCenter 2.5 2 0 bboxDisplay TRUE }}
  DEF EM DISEntityManager {{ address "127.0.0.1" port {PORT_R} children [ DEF TM DISEntityTypeMapping {{ }} ] }}
]}}
DEF G2 Group {{ children [ Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.7 0.3 0.2 }} }} geometry Box {{ size 0.8 0.8 0.8 }} }} ] }}
DEF K KeySensor {{ }}
DEF R Script {{
  directOutput TRUE
  inputOnly SFString on_key
  inputOutput SFString log ""
  initializeOnly SFNode rx USE RX
  initializeOnly SFNode sg USE SG
  initializeOnly SFNode tx USE TX
  initializeOnly SFNode em USE EM
  initializeOnly SFNode tm USE TM
  initializeOnly SFNode g USE G
  initializeOnly SFNode g2 USE G2
  url "javascript:
var made = [ ];
var got = '';
function on_key (k)
{{
   log = log + 'K' + k + ' ';
   if (k == 'a') rx .enabled = false;
   if (k == 'b') rx .enabled = true;
   if (k == 'c') rx .visible = false;
   if (k == 'd') rx .visible = true;
   if (k == 'e') rx .bboxDisplay = false;
   if (k == 'f') rx .bboxDisplay = true;
   if (k == 'g') rx .description = 'changed';
   if (k == 'h') {{ rx .port = 5000; rx .address = 'localhost'; rx .networkMode = 'networkWriter'; rx .whichGeometry = 2; }}
   if (k == 'i') {{ sg .data = new MFInt32 (9, 8, 7); sg .dataLength = 3; sg .sampleRate = 44100; sg .samples = 3; }}
   if (k == 'j') {{ tx .power = 55.5; tx .frequency = 123456; tx .transmitState = 2; tx .antennaLocation = new SFVec3f (7, 8, 9); }}
   if (k == 'k') {{ em .siteID = 11; em .applicationID = 12; em .port = 6000; em .children = new MFNode (); }}
   if (k == 'l') {{ tm .url = new MFString ('a.xml', 'b.xml'); tm .load = false; tm .description = 'mapped'; }}
   if (k == 'm') {{ rx .bboxDisplay = false; sg .bboxDisplay = false; }}
   if (k == 'n') g .removeChildren = new MFNode (rx, sg);
   if (k == 'o') g .addChildren = new MFNode (rx, sg);
   if (k == 'p') {{ rx .bboxDisplay = true; sg .bboxDisplay = true; }}
   if (k == 'q') {{ made = [ Browser .currentScene .createNode ('ReceiverPdu'), Browser .currentScene .createNode ('SignalPdu'), Browser .currentScene .createNode ('TransmitterPdu'), Browser .currentScene .createNode ('DISEntityManager'), Browser .currentScene .createNode ('DISEntityTypeMapping') ];
                   made [0] .bboxSize = new SFVec3f (1, 1, 1); made [0] .bboxCenter = new SFVec3f (0, -2, 0); made [0] .bboxDisplay = true;
                   made [1] .bboxSize = new SFVec3f (1, 1, 1); made [1] .bboxCenter = new SFVec3f (-2, -2, 0); made [1] .bboxDisplay = true;
                   made [2] .bboxSize = new SFVec3f (1, 1, 1); made [2] .bboxCenter = new SFVec3f (2, -2, 0); made [2] .bboxDisplay = true;
                   g2 .addChildren = new MFNode (made [0], made [1], made [2], made [3]); }}
   if (k == 'r') {{ made [0] .enabled = false; made [1] .visible = false; made [2] .bboxSize = new SFVec3f (2, 2, 2); made [3] .children = new MFNode (made [4]); }}
   if (k == 's') {{ made [0] .bboxDisplay = false; made [1] .bboxDisplay = false; made [2] .bboxDisplay = false; }}
   if (k == 't') g2 .removeChildren = new MFNode (made [0], made [1], made [2], made [3]);
   if (k == 'u') {{ got = rx .isActive + ' ' + rx .isNetworkReader + ' ' + rx .isNetworkWriter + ' ' + rx .isStandAlone + ' ' + rx .isRtpHeaderHeard + ' ' + rx .timestamp + ' ' + rx .receivedPower + ' ' + rx .getTypeName (); rx .description = got; }}
}}
"
}}
ROUTE K.keyPress TO R.on_key
"""
worlds["dis-runtime.wrl"] = world(BG, LIGHTS, dis_rt, vp="Viewpoint { position 0 0 9 }")
scripts["dis-runtime.json"] = runtime_json(210, list("abcdefghijklmnopqrstu"), start=16, step=8)

# ROUTEs: enabled / visible / description / bboxDisplay set by ROUTE from a TimeSensor-driven BooleanToggle,
# outputs of the PDU routed to the Script log. Neither isActive nor any other output may fire.
dis_route = f"""
DEF RX ReceiverPdu {{ bboxSize 2 2 2 bboxDisplay TRUE address "127.0.0.1" port {PORT_R} networkMode "networkReader" }}
DEF SG SignalPdu {{ bboxSize 1 1 1 bboxCenter 2 0 0 bboxDisplay TRUE address "127.0.0.1" port {PORT_W} networkMode "networkWriter" }}
DEF EM DISEntityManager {{ address "127.0.0.1" port {PORT_R} }}
DEF K KeySensor {{ }}
DEF T BooleanToggle {{ }}
DEF R Script {{
  directOutput TRUE
  inputOnly SFString on_key
  inputOnly SFBool seen
  inputOnly SFTime seen_time
  inputOnly SFInt32 seen_int
  inputOnly MFNode seen_nodes
  outputOnly SFBool t_set
  inputOutput SFString log ""
  url "javascript:
function on_key (k) {{ log = log + 'K' + k + ' '; t_set = true; }}
function seen (v, t) {{ log = log + 'seen:' + v + '@' + t + ' '; }}
function seen_time (v, t) {{ log = log + 'time:' + v + ' '; }}
function seen_int (v, t) {{ log = log + 'int:' + v + ' '; }}
function seen_nodes (v, t) {{ log = log + 'nodes:' + v .length + ' '; }}
"
}}
ROUTE K.keyPress TO R.on_key
ROUTE R.t_set TO T.set_boolean
ROUTE T.toggle TO RX.set_bboxDisplay
ROUTE T.toggle TO SG.set_enabled
ROUTE T.toggle TO RX.set_visible
ROUTE RX.isActive TO R.seen
ROUTE RX.isNetworkReader TO R.seen
ROUTE RX.isNetworkWriter TO R.seen
ROUTE RX.isStandAlone TO R.seen
ROUTE RX.isRtpHeaderHeard TO R.seen
ROUTE RX.timestamp TO R.seen_time
ROUTE SG.isActive TO R.seen
ROUTE SG.isNetworkWriter TO R.seen
ROUTE SG.timestamp TO R.seen_time
ROUTE EM.addedEntities TO R.seen_nodes
ROUTE EM.removedEntities TO R.seen_nodes
ROUTE RX.enabled_changed TO R.seen
ROUTE RX.visible_changed TO R.seen
ROUTE SG.enabled_changed TO R.seen
"""
worlds["dis-route.wrl"] = world(BG, LIGHTS, dis_route, vp="Viewpoint { position 0 0 8 }")
scripts["dis-route.json"] = runtime_json(120, list("aaaa"), start=20, step=20)

# ---------------------------------------------------------------------------------------------
# InlineGeometry. Scenes the node loads (X3D classic encoding, VRML97 and XML), each with a geometry in a different place.
IGDIR = "ig"
files = {}   # relative path (below OUT) -> text/bytes


def x3dv(*parts, comps="Geometry3D:2"):
    return "#X3D V4.1 utf8\nPROFILE Interchange\nCOMPONENT Geometry3D:2\nCOMPONENT Geometry2D:2\nCOMPONENT Text:1\n" + "\n".join(parts) + "\n"


IFS = ("IndexedFaceSet { coord Coordinate { point [ -1 -1 -1, 1 -1 -1, 1 -1 1, -1 -1 1, 0 1 0, 0.8 0.2 0.6 ] }"
       " coordIndex [ 0 1 4 -1 1 2 4 -1 2 3 4 -1 3 0 4 -1 3 2 1 0 -1 ] creaseAngle 0.5 solid TRUE }")
files[IGDIR + "/box.x3dv"] = x3dv("Group { children Shape { geometry Box { size 1.4 1 0.8 } } }")
files[IGDIR + "/sphere.x3dv"] = x3dv("Shape { geometry Sphere { radius 0.7 } }")
files[IGDIR + "/cone.x3dv"] = x3dv("Shape { geometry Cone { height 1.6 bottomRadius 0.6 } }")
files[IGDIR + "/cylinder.x3dv"] = x3dv("Shape { geometry Cylinder { height 1.4 radius 0.5 } }")
files[IGDIR + "/ifs.x3dv"] = x3dv(f"Shape {{ geometry DEF IFS {IFS} }}")
files[IGDIR + "/extrusion.x3dv"] = x3dv("Shape { geometry Extrusion { crossSection [ 1 1, 1 -1, -1 -1, -1 1, 1 1 ] spine [ 0 -0.8 0, 0 0 0, 0 0.8 0 ] scale [ 0.6 0.6, 0.3 0.3, 0.6 0.6 ] creaseAngle 0.3 solid TRUE } }")
files[IGDIR + "/grid.x3dv"] = x3dv("Shape { geometry ElevationGrid { xDimension 4 zDimension 4 xSpacing 0.5 zSpacing 0.5 height [ 0 0.3 0.1 0, 0.2 0.5 0.4 0.1, 0.1 0.4 0.6 0.2, 0 0.1 0.2 0 ] creaseAngle 0.5 solid TRUE } }")
files[IGDIR + "/rect.x3dv"] = x3dv("Shape { geometry Rectangle2D { size 1.5 1 } }")
files[IGDIR + "/disk.x3dv"] = x3dv("Shape { geometry Disk2D { innerRadius 0.2 outerRadius 0.7 } }")
files[IGDIR + "/lines.x3dv"] = x3dv("Shape { geometry IndexedLineSet { coord Coordinate { point [ -1 -1 0, 1 -1 0, 0 1 0 ] } coordIndex [ 0 1 2 0 -1 ] } }")
files[IGDIR + "/points.x3dv"] = x3dv("Shape { geometry PointSet { coord Coordinate { point [ -1 -1 0, 1 -1 0, 0 1 0, 0 0 0.5 ] } } }")
files[IGDIR + "/triangles.x3dv"] = x3dv("Shape { geometry IndexedTriangleSet { coord Coordinate { point [ -1 -1 0, 1 -1 0, 0 1 0, 0 0 1 ] } index [ 0 1 2, 0 3 1, 1 3 2, 2 3 0 ] solid TRUE } }")
files[IGDIR + "/ifs-normal.x3dv"] = x3dv("Shape { geometry IndexedFaceSet { coord Coordinate { point [ -1 -1 0, 1 -1 0, 0 1 0 ] } coordIndex [ 0 1 2 -1 ] normal Normal { vector [ 0 0 1, 0 0 1, 0 0 1 ] } creaseAngle 0.5 solid TRUE } }")
files[IGDIR + "/text.x3dv"] = x3dv('Shape { geometry Text { string "Ab" fontStyle FontStyle { size 1 } solid TRUE } }')
# depth-first search: the first geometry found through SFNode / MFNode fields (Transform > Switch > Shape ...), skipping non-geometry first
files[IGDIR + "/nested.x3dv"] = x3dv("WorldInfo { title \"first\" }", "Transform { translation 1 0 0 children [ Group { children [ Viewpoint { } Switch { whichChoice 0 children [ Shape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Cone { height 1.2 bottomRadius 0.5 } } ] } ] } ] }", "Shape { geometry Sphere { radius 2 } }")
files[IGDIR + "/multi.x3dv"] = x3dv("Shape { geometry Box { size 1 1 1 } }", "Shape { geometry DEF G2 Cylinder { height 1.6 radius 0.4 } }", "DEF G3 Shape { geometry DEF G3G Cone { height 1.6 bottomRadius 0.5 } }", "DEF NOGEO WorldInfo { }")
files[IGDIR + "/none.x3dv"] = x3dv("Viewpoint { }", "WorldInfo { title \"no geometry\" }")
files[IGDIR + "/broken.x3dv"] = "#X3D V4.0 utf8\nPROFILE Interchange\nShape { geometry Box { size 1 1 1 \n"
files[IGDIR + "/text.txt"] = "this is not a scene\n"
files[IGDIR + "/box.wrl"] = "#VRML V2.0 utf8\nShape { geometry Box { size 1.4 1 0.8 } }\n"
files[IGDIR + "/box.x3d"] = '<?xml version="1.0" encoding="UTF-8"?>\n<X3D profile="Interchange" version="4.0"><Scene><Shape><Box size="1.4 1 0.8"/></Shape></Scene></X3D>\n'
files[IGDIR + "/script.x3dv"] = x3dv("COMPONENT Scripting:1", "DEF S Script { url \"javascript:function initialize () { Browser .print ('ig-script-init'); }\" }", "Shape { geometry Sphere { radius 0.5 } }", comps="")
files[IGDIR + "/inline-inside.x3dv"] = x3dv("Inline { url \"box.x3dv\" }", "Shape { geometry Cone { } }")
files[IGDIR + "/proto.x3dv"] = x3dv("PROTO PB [ field SFVec3f size 1 1 1 ] { Shape { geometry Box { size IS size } } }", "PB { size 0.6 1.4 0.6 }")
files[IGDIR + "/routes.x3dv"] = x3dv("Shape { geometry IndexedFaceSet { coord DEF CO Coordinate { point [ -0.5 -0.5 0, 0.5 -0.5 0, 0 0.5 0 ] } coordIndex [ 0 1 2 -1 ] solid FALSE } }", "DEF TS TimeSensor { loop TRUE cycleInterval 2 }", "DEF CI CoordinateInterpolator { key [ 0 1 ] keyValue [ -0.5 -0.5 0, 0.5 -0.5 0, 0 0.5 0, -1 -1 0, 1 -1 0, 0 1 0 ] }", "ROUTE TS.fraction_changed TO CI.set_fraction", "ROUTE CI.value_changed TO CO.set_point")
files[IGDIR + "/dup.x3dv"] = x3dv("Shape { geometry DEF D1 Box { size 1.2 1.2 1.2 } }")

# Four rows of the same geometries (lit, red material): default solid FALSE / smooth TRUE; solid TRUE; smooth FALSE; both set.
names = ["box", "sphere", "cone", "cylinder", "ifs", "extrusion", "grid", "rect", "disk", "lines", "points", "triangles", "ifs-normal", "text"]
mat = "appearance Appearance { material Material { diffuseColor 0.8 0.3 0.2 specularColor 0.6 0.6 0.6 shininess 0.4 } }"
rows = []
for r, extra in enumerate(("", "solid TRUE ", "smooth FALSE ", "solid TRUE smooth FALSE ")):
    for i, n in enumerate(names):
        x, y = (i % 7 - 3) * 1.45, 3.2 - (r * 2 + i // 7) * 1.1
        rows.append(f'Transform {{ translation {x:.3f} {y:.3f} 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry InlineGeometry {{ url "{IGDIR}/{n}.x3dv" {extra}}} }} }}')
worlds["ig-kinds.wrl"] = world(BG, LIGHTS, "PointLight { location 0 0 10 intensity 0.6 }", *rows, vp="Viewpoint { position 0 0 14 }", head=HEAD41)
scripts["ig-kinds.json"] = runtime_json(30, [])

# Which geometry: hash, depth first, formats, nested Inline / PROTO / Script / ROUTEs inside the loaded scene.
sel = [
    ("nested.x3dv", ""), ("multi.x3dv", ""), ("multi.x3dv#G2", ""), ("multi.x3dv#G3G", ""), ("multi.x3dv#G3", ""), ("multi.x3dv#NOGEO", ""),
    ("multi.x3dv#MISSING", ""), ("none.x3dv", ""), ("box.wrl", ""), ("box.x3d", ""), ("inline-inside.x3dv", ""), ("proto.x3dv", ""), ("routes.x3dv", ""),
    ("script.x3dv", ""), ("broken.x3dv", ""), ("text.txt", ""), ("missing.x3dv", ""), ("dup.x3dv", "solid TRUE"), ("dup.x3dv", ""),
]
rows = []
for i, (n, extra) in enumerate(sel):
    x, y = (i % 7 - 3) * 1.45, 3.0 - (i // 7) * 1.5
    rows.append(f'Transform {{ translation {x:.3f} {y:.3f} 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry DEF I{i} InlineGeometry {{ url "{IGDIR}/{n}" {extra} }} }} }}')
worlds["ig-select.wrl"] = world(BG, LIGHTS, "PointLight { location 0 0 10 intensity 0.6 }", *rows, vp="Viewpoint { position 0 0 12 }", head=HEAD41)
scripts["ig-select.json"] = runtime_json(60, [])

# url lists, load FALSE, empty url, data URLs, a url that fails first, LoadSensor watching the nodes.
dataurl = "data:model/x3d+vrml,%23VRML%20V2.0%20utf8%0AShape%20%7B%20geometry%20Sphere%20%7B%20radius%200.8%20%7D%20%7D"
dataurl2 = "data:model/x3d+vrml,%23VRML%20V2.0%20utf8%0AShape%20%7B%20geometry%20Cone%20%7B%20%7D%20%7D%0AShape%20%7B%20geometry%20Box%20%7B%20%7D%20%7D"
loads = f"""
Transform {{ translation -3 2 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry DEF L0 InlineGeometry {{ url [ "{IGDIR}/missing.x3dv" "{IGDIR}/box.x3dv" ] }} }} }}
Transform {{ translation -1.5 2 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry DEF L1 InlineGeometry {{ url [ "{IGDIR}/none.x3dv" "{IGDIR}/cone.x3dv" ] }} }} }}
Transform {{ translation 0 2 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry DEF L2 InlineGeometry {{ url [ "{IGDIR}/broken.x3dv" "{IGDIR}/sphere.x3dv" ] }} }} }}
Transform {{ translation 1.5 2 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry DEF L3 InlineGeometry {{ load FALSE url "{IGDIR}/box.x3dv" }} }} }}
Transform {{ translation 3 2 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry DEF L4 InlineGeometry {{ }} }} }}
Transform {{ translation -3 0 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry DEF L5 InlineGeometry {{ url "{dataurl}" }} }} }}
Transform {{ translation -1.5 0 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry DEF L6 InlineGeometry {{ url "{dataurl2}" }} }} }}
Transform {{ translation 0 0 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry DEF L7 InlineGeometry {{ url [ "" "{IGDIR}/box.x3dv" ] }} }} }}
Transform {{ translation 1.5 0 0 scale 0.5 0.5 0.5 children Shape {{ {mat} geometry DEF L8 InlineGeometry {{ url [ "{IGDIR}/text.txt" "{IGDIR}/missing2.x3dv" ] }} }} }}
"""
worlds["ig-loads.wrl"] = world(BG, LIGHTS, "PointLight { location 0 0 10 intensity 0.6 }", loads, vp="Viewpoint { position 0 0 10 }", head=HEAD41)
scripts["ig-loads.json"] = runtime_json(60, [])

# LoadSensor and InlineGeometry: X3DCast (X3DUrlObject, node) goes through getInnerNode, so the sensor never watches an
# InlineGeometry; its outputs fire only because the node's setInternalScene sends an event to the fields that hold it
# (X3DChildObject.addEvent): the sensor's `children`, which resets (and counts nothing).
LSLOG = """
DEF LS LoadSensor { children [ %s ] }
DEF LOG Script { inputOnly SFBool a  inputOnly SFBool l  inputOnly SFFloat p  inputOnly SFTime t  inputOutput SFString log ""
  url "javascript:
function a (v) { log = log + 'A' + v + ' '; }
function l (v) { log = log + 'L' + v + ' '; }
function p (v) { log = log + 'P' + v + ' '; }
function t (v) { log = log + 'T '; }
" }
ROUTE LS.isActive TO LOG.a
ROUTE LS.isLoaded TO LOG.l
ROUTE LS.progress TO LOG.p
ROUTE LS.loadTime TO LOG.t
"""
for name, nodes in {"ok": ["box.x3dv"], "fail": ["missing3.x3dv"], "two": ["box.x3dv", "sphere.x3dv"], "nourl": [None], "mixed": ["box.x3dv", "missing4.x3dv", None]}.items():
    body = "\n".join(f'Shape {{ {mat} geometry DEF A{i} InlineGeometry {{ {("url " + chr(34) + IGDIR + "/" + n + chr(34)) if n else ""} }} }}' for i, n in enumerate(nodes))
    worlds[f"ig-ls-{name}.wrl"] = world(BG, LIGHTS, body, LSLOG % " ".join(f"USE A{i}" for i in range(len(nodes))), "DEF IL Inline { url \"ig/box.x3dv\" }", head=HEAD41)
    scripts[f"ig-ls-{name}.json"] = runtime_json(40, [])

# Run-time: url / load / solid / smooth changes by Script, the Shape's geometry field, Group removal, a Script-created
# InlineGeometry, a url that fails after one that worked, autoRefresh.
DU = "data:model/x3d+vrml,%23VRML%20V2.0%20utf8%0AShape%20%7B%20geometry%20Sphere%20%7B%20radius%200.8%20%7D%20%7D"
DU2 = "data:model/x3d+vrml,%23VRML%20V2.0%20utf8%0AShape%20%7B%20geometry%20Cone%20%7B%20height%201.4%20%7D%20%7D"
ig_rt = f"""
PointLight {{ location 0 0 10 intensity 0.6 }}
DEF G Group {{ children [
  Transform {{ translation -2.5 1.5 0 children DEF S1 Shape {{ {mat} geometry DEF I1 InlineGeometry {{ url "{IGDIR}/box.x3dv" }} }} }}
  Transform {{ translation 0 1.5 0 children DEF S2 Shape {{ {mat} geometry DEF I2 InlineGeometry {{ url "{IGDIR}/ifs.x3dv" }} }} }}
  Transform {{ translation 2.5 1.5 0 rotation 0 1 0 3.14159265 children DEF S3 Shape {{ {mat} geometry DEF I3 InlineGeometry {{ url "{IGDIR}/rect.x3dv" }} }} }}
  Transform {{ translation 2.5 -0.2 0 children DEF S4 Shape {{ {mat} geometry DEF I4 InlineGeometry {{ url "{IGDIR}/grid.x3dv" }} }} }}
]}}
DEF G2 Group {{ }}
Transform {{ translation -2.5 -1.5 0 children DEF S5 Shape {{ {mat} geometry DEF I5 InlineGeometry {{ url "{IGDIR}/extrusion.x3dv" smooth FALSE }} }} }}
DEF K KeySensor {{ }}
DEF R Script {{
  directOutput TRUE
  inputOnly SFString on_key
  inputOutput SFString log ""
  initializeOnly SFNode i1 USE I1
  initializeOnly SFNode i2 USE I2
  initializeOnly SFNode i3 USE I3
  initializeOnly SFNode i4 USE I4
  initializeOnly SFNode i5 USE I5
  initializeOnly SFNode s1 USE S1
  initializeOnly SFNode s5 USE S5
  initializeOnly SFNode g USE G
  initializeOnly SFNode g2 USE G2
  url "javascript:
var made = null, made_shape = null;
function on_key (k)
{{
   log = log + 'K' + k + ' ';
   if (k == 'a') i1 .url = new MFString ('{IGDIR}/sphere.x3dv');
   if (k == 'b') i1 .url = new MFString ('{IGDIR}/missing.x3dv');
   if (k == 'c') i1 .url = new MFString ('{IGDIR}/cone.x3dv#MISSING', '{IGDIR}/multi.x3dv#G2');
   if (k == 'd') i1 .load = false;
   if (k == 'e') i1 .load = true;
   if (k == 'f') i3 .solid = true;
   if (k == 'g') i3 .solid = false;
   if (k == 'h') i2 .smooth = false;
   if (k == 'i') i2 .smooth = true;
   if (k == 'j') {{ i4 .smooth = false; i5 .smooth = true; }}
   if (k == 'k') {{ i4 .smooth = true; i5 .solid = true; }}
   if (k == 'l') i2 .url = new MFString ();
   if (k == 'm') i2 .url = new MFString ('{IGDIR}/ifs-normal.x3dv');
   if (k == 'n') i4 .url = new MFString ('{DU}');
   if (k == 'o') s1 .geometry = i4;
   if (k == 'p') s1 .geometry = null;
   if (k == 'q') s1 .geometry = i1;
   if (k == 'r') g .removeChildren = new MFNode (s1);
   if (k == 's') g .addChildren = new MFNode (s1);
   if (k == 't') {{ made = Browser .currentScene .createNode ('InlineGeometry'); made .url = new MFString ('{DU2}'); made .solid = true; made_shape = Browser .currentScene .createNode ('Shape'); made_shape .appearance = s1 .appearance; made_shape .geometry = made; var t = Browser .currentScene .createNode ('Transform'); t .translation = new SFVec3f (0, -1.5, 0); t .children = new MFNode (made_shape); g2 .addChildren = new MFNode (t); }}
   if (k == 'u') {{ made .url = new MFString ('{IGDIR}/cylinder.x3dv'); made .smooth = false; }}
   if (k == 'v') g2 .children = new MFNode ();
   if (k == 'w') i5 .autoRefresh = 0.5;
   if (k == 'x') {{ i5 .autoRefresh = 0; i5 .url = new MFString ('{IGDIR}/ifs.x3dv'); i5 .url = new MFString ('{IGDIR}/sphere.x3dv'); }}
   if (k == 'y') {{ i5 .loadNow (); }}
}}
"
}}
ROUTE K.keyPress TO R.on_key
"""
# Removal from a live Group: here the Shapes are direct children of the Group (in ig-runtime they sit under Transforms,
# so `removeChildren` of the Group is a no-op there). 'x' removes S1 and S2 (two InlineGeometry Shapes), 'y' adds them back,
# 't' adds a Script-created InlineGeometry Shape to G2 and 'v' clears G2. The keys avoid the letters the windowed viewer uses.
ig_rm = f"""
DEF G Group {{ children [
  DEF S1 Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.8 0.3 0.2 }} }} geometry DEF I1 InlineGeometry {{ url "{IGDIR}/sphere.x3dv" }} }}
  Transform {{ translation 2.5 0 0 children DEF S2 Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.2 0.6 0.8 }} }} geometry DEF I2 InlineGeometry {{ url "{IGDIR}/box.x3dv" }} }} }}
]}}
DEF G2 Group {{ }}
DEF K KeySensor {{ }}
DEF R Script {{
  directOutput TRUE
  inputOnly SFString on_key
  initializeOnly SFNode s1 USE S1
  initializeOnly SFNode g USE G
  initializeOnly SFNode g2 USE G2
  url "javascript:
var made = null, made_shape = null;
function on_key (k)
{{
   if (k == 'x') g .removeChildren = new MFNode (s1);
   if (k == 'y') g .addChildren = new MFNode (s1);
   if (k == 't') {{ made = Browser .currentScene .createNode ('InlineGeometry'); made .url = new MFString ('{DU2}'); made_shape = Browser .currentScene .createNode ('Shape'); made_shape .appearance = s1 .appearance; made_shape .geometry = made; g2 .addChildren = new MFNode (made_shape); }}
   if (k == 'v') g2 .children = new MFNode ();
}}
"
}}
ROUTE K.keyPress TO R.on_key
"""
worlds["ig-remove.wrl"] = world(BG, LIGHTS, ig_rm, vp="Viewpoint { position 0 0 10 }", head=HEAD41)
scripts["ig-remove.json"] = runtime_json(80, list("xytv"), start=16, step=12)

worlds["ig-runtime.wrl"] = world(BG, LIGHTS, ig_rt, vp="Viewpoint { position 0 0 10 }", head=HEAD41)
scripts["ig-runtime.json"] = runtime_json(260, list("abcdefghijklmnopqrstuvwx"), start=16, step=8)

# ---------------------------------------------------------------------------------------------
# FontLibrary (X3D 4.1): url fallback, the registered name is the `family` field, FontStyle `family` finds it (case
# insensitive), one library table per execution context, the registration is never removed.
# Fonts: PT Sans (SIL OFL 1.1, ParaType) and Droid Serif (Apache 2.0) from oracle/x_ite-pin/src/assets/fonts; the .ttf and
# .woff files in fonts/ are conversions of those woff2 files made with fontTools (the same outlines).
POS = [(-1.6, 1.3), (1.6, 1.3), (-1.6, -1.3), (1.6, -1.3), (0, 0)]


def text(label, family, i, extra="", style="PLAIN", size=1.3, name=""):
    x, y = POS[i]
    fam = f"family {family}" if family else ""
    return (f'Transform {{ translation {x} {y} 0 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.9 0.9 0.4 }} }}'
            f' geometry Text {{ string "{label}" fontStyle {"DEF " + name if name else ""} FontStyle {{ {fam} style "{style}" size {size} justify "MIDDLE" {extra} }} }} }} }}')


def fl_world(name, libs, texts, extra=""):
    worlds[name + ".wrl"] = world(BG, LIGHTS, libs, "\n".join(text("Ag", f, i, **kw) for i, (f, kw) in enumerate(texts)), extra, vp="Viewpoint { position 0 0 6 }", head=HEAD41)
    scripts[name + ".json"] = runtime_json(40, [])


fl_world("fl-basic-a", """
FontLibrary { family "PTTTF" url "fonts/PTSans-Regular.ttf" }
FontLibrary { family "PTW2" url "fonts/PTSans-Regular.woff2" }
FontLibrary { family "PTBOLD" url [ "fonts/missing.ttf" "fonts/PTSans-Bold.ttf" ] }
FontLibrary { family "FAILED" url [ "fonts/missing1.ttf" "fonts/missing2.ttf" ] }
""", [('"ptttf"', {}), ('[ "NOPE" "PTW2" ]', {}), ('"PTBOLD"', {}), ('[ "FAILED" "PTTTF" ]', {})])
fl_world("fl-basic-b", """
FontLibrary { family "FAILED" url [ "fonts/missing1.ttf" "fonts/missing2.ttf" ] }
FontLibrary { url "fonts/DroidSerif-Italic.ttf" }
FontLibrary { family "TWICE" url "fonts/PTSans-Regular.ttf" }
FontLibrary { family "TWICE" url "fonts/PTSans-Bold.ttf" }
FontLibrary { family "NOLOAD" load FALSE url "fonts/PTSans-Regular.ttf" }
""", [('"FAILED"', {}), ('"Droid Serif Italic"', {}), ('"SERIF"', {}), ('"NOLOAD"', {})])
# Two libraries with one family (TWICE): which font a FontStyle gets depends on the order the two loads finish
# (X_ITE got the first one in one run and the second in another), so no Text uses TWICE; only the registration runs.
fl_world("fl-basic-c", """
FontLibrary { family "PTTTF" url "fonts/PTSans-Regular.ttf" }
FontLibrary { family "PTW2" url "fonts/PTSans-Regular.woff2" }
""", [('"PTTTF"', {"style": "BOLD"}), ('"PTW2"', {"style": "ITALIC"}), ('"fonts/PTSans-Regular.ttf"', {}), ('[ "PTTTF.ttf" "SANS" ]', {})])
fl_world("fl-basic-d", """
FontLibrary { family "ptttf" url "fonts/PTSans-Regular.ttf" }
FontLibrary { family "Ünï" url "fonts/PTSans-Bold.ttf" }
""", [('"PTTTF"', {}), ('"ünï"', {}), ('"SERIF"', {}), ('"TYPEWRITER"', {})])
fl_world("fl-woff1", """
FontLibrary { family "PTW1" url "fonts/PTSans-Regular.woff" }
""", [('"PTW1"', {}), ('"SERIF"', {}), ('"SERIF"', {}), ('"SERIF"', {})])
# a file that is not a font: through a FontLibrary (alone, then before a good url) and through the deprecated family url
fl_world("fl-bad", """
FontLibrary { family "BAD" url "fonts/notfont.ttf" }
FontLibrary { family "BAD2" url [ "fonts/notfont.ttf" "fonts/PTSans-Bold.ttf" ] }
""", [('"BAD"', {}), ('"BAD2"', {}), ('"fonts/notfont.ttf"', {}), ('"SERIF"', {})])
# order: the FontStyle before the library, and a FontStyle waiting for a slow library behind a failing one
fl_world("fl-order", "", [('"LATER"', {}), ('"LATER" ', {}), ('[ "NONE" "LATER" ]', {}), ('"SERIF"', {})],
         extra='FontLibrary { family "LATER" url "fonts/PTSans-Bold.ttf" }')
# scopes: an Inline scene and a PROTO body are execution contexts of their own
open(os.path.join(OUT, "fl-inline.x3dv"), "w").write(x3dv('FontLibrary { family "INL" url "fonts/PTSans-Bold.ttf" }', text("Ag", '"INL"', 4, size=1.0)).replace("V4.0", "V4.1").replace("COMPONENT Geometry3D:2", "COMPONENT Geometry3D:2\nCOMPONENT Text:2"))
worlds["fl-scopes.wrl"] = world(BG, LIGHTS, f"""
PROTO PText [ field MFString family "X" ] {{ Shape {{ geometry Text {{ string "Ag" fontStyle FontStyle {{ family IS family size 1.2 justify "MIDDLE" }} }} }} }}
PROTO PLib [ ] {{ Group {{ children [ FontLibrary {{ family "INPROTO" url "fonts/PTSans-Regular.ttf" }} {text("Ag", '"INPROTO"', 3, size=1.0)} ] }} }}
FontLibrary {{ family "ROOTLIB" url "fonts/PTSans-Bold.ttf" }}
Transform {{ translation -1.6 1.3 0 children Inline {{ url "fl-inline.x3dv" }} }}
{text("Ag", '"INL"', 1)}
Transform {{ translation 0 -0.2 0 children PText {{ family "ROOTLIB" }} }}
PLib {{ }}
{text("Ag", '"INPROTO"', 2)}
""", vp="Viewpoint { position 0 0 6 }", head=HEAD41)
scripts["fl-scopes.json"] = runtime_json(40, [])

# Run-time. A FontStyle resolves its font in loadData (initialize, `family` / `style` change), a library that appears later is
# not looked at again; a FontLibrary registers `family` whenever its font or `family` changes and never unregisters.
fl_rt = """
DEF G Group { children [
  DEF LA FontLibrary { family "RTA" url "fonts/PTSans-Regular.ttf" }
  DEF LB FontLibrary { family "RTB" url "fonts/PTSans-Bold.ttf" }
] }
DEF G2 Group { }
""" + "\n".join([
    text("Ag", '"RTA"', 0, name="FS1"), text("Ag", '"RTB"', 1, name="FS2"), text("Ag", '"LATE"', 2, name="FS3"), text("Ag", '"RTNEW"', 3, name="FS4"),
]) + """
DEF K KeySensor { }
DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOutput SFString log ""
  initializeOnly SFNode la USE LA
  initializeOnly SFNode lb USE LB
  initializeOnly SFNode g USE G
  initializeOnly SFNode g2 USE G2
  initializeOnly SFNode fs1 USE FS1
  initializeOnly SFNode fs2 USE FS2
  initializeOnly SFNode fs3 USE FS3
  initializeOnly SFNode fs4 USE FS4
  url "javascript:
var made = null, mt = null;
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') la .url = new MFString ('fonts/PTSans-Bold.ttf');
   if (k == 'b') fs1 .family = new MFString ('RTA');
   if (k == 'c') la .family = 'RTNEW';
   if (k == 'd') fs4 .family = new MFString ('RTNEW');
   if (k == 'e') la .url = new MFString ('fonts/missing.ttf');
   if (k == 'f') fs1 .family = new MFString ('RTA', 'x');
   if (k == 'g') la .load = false;
   if (k == 'h') { made = Browser .currentScene .createNode ('FontLibrary'); made .family = 'LATE'; made .url = new MFString ('fonts/DroidSerif-Italic.ttf'); g2 .addChildren = new MFNode (made); }
   if (k == 'i') fs3 .family = new MFString ('LATE');
   if (k == 'j') g .removeChildren = new MFNode (la, lb);
   if (k == 'k') fs2 .style = 'BOLD';
   if (k == 'l') lb .url = new MFString ('fonts/PTSans-Regular.ttf');
   if (k == 'm') fs2 .style = 'PLAIN';
   if (k == 'n') g2 .removeChildren = new MFNode (made);
   if (k == 'o') { lb .family = ''; lb .url = new MFString ('fonts/PTSans-Bold.ttf'); }
   if (k == 'p') fs2 .family = new MFString ('rtb');
   if (k == 'q') fs3 .family = new MFString ('SANS');
}
"
}
ROUTE K.keyPress TO R.on_key
"""
worlds["fl-runtime.wrl"] = world(BG, LIGHTS, fl_rt, vp="Viewpoint { position 0 0 6 }", head=HEAD41)
scripts["fl-runtime.json"] = runtime_json(180, list("abcdefghijklmnopq"), start=16, step=8)

if __name__ == "__main__":
    for n, t in worlds.items():
        open(os.path.join(OUT, n), "w").write(t)
    for n, t in files.items():
        os.makedirs(os.path.dirname(os.path.join(OUT, n)), exist_ok=True)
        open(os.path.join(OUT, n), "w").write(t)
    for n, s in scripts.items():
        open(os.path.join(OUT, n), "w").write(json.dumps(s, indent=1))
    print(len(worlds), "worlds", len(scripts), "scripts")
