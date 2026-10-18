#!/usr/bin/env python3
"""REA batch 08 fixtures for sub:parse.html-support, in oracle/synthetic-rea-html.

  python3 -I oracle/tools/gen_rea_b08_html.py oracle/synthetic-rea-html

HTMLSupport.js is used by the XMLParser only when its input is an HTML element (X_ITE's DOM
integration, `browser .importDocument (element)` and the <x3d-canvas> children). The *.html files
here are fragments assigned to `div .innerHTML` and the first <x3d> element is imported; the
xml/*.x3d files are the same text as XML documents, which X_ITE reads without HTMLSupport.
All files are synthetic.
"""
import os, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "oracle/synthetic-rea-html"
os.makedirs(os.path.join(OUT, "xml"), exist_ok=True)
html = {}
xml = {}


def both(name, body, profile="Full", extra=""):
    html[name] = f'<x3d profile="{profile}" version="4.0" {extra}>\n{body}\n</x3d>\n'


# --- node names ---------------------------------------------------------------------------------
both("names-upper", "<SCENE><TRANSFORM TRANSLATION='1 2 3'><SHAPE><APPEARANCE><MATERIAL DIFFUSECOLOR='1 0 0'></MATERIAL></APPEARANCE><BOX SIZE='2 2 2'></BOX></SHAPE></TRANSFORM></SCENE>")
both("names-lower", "<scene><transform translation='1 2 3'><shape><appearance><material diffusecolor='1 0 0'></material></appearance><box size='2 2 2'></box></shape></transform></scene>")
both("names-camel", "<Scene><Transform translation='1 2 3'><Shape><Appearance><Material diffuseColor='1 0 0'></Material></Appearance><Box size='2 2 2'></Box></Shape></Transform></Scene>")
both("names-mixed-case", "<Scene><tRaNsFoRm><sHaPe><bOx></bOx></sHaPe></tRaNsFoRm></Scene>")
both("names-components", "<Scene><NURBSCURVE ORDER='3'></NURBSCURVE><GEOCOORDINATE></GEOCOORDINATE><PARTICLESYSTEM></PARTICLESYSTEM><HANIMHUMANOID></HANIMHUMANOID><VOLUMEDATA></VOLUMEDATA></Scene>", extra="")
both("names-unknown", "<Scene><FooBar></FooBar><TRANSFORM><BAZ></BAZ></TRANSFORM></Scene>")
both("names-null", "<Scene><SHAPE><NULL></NULL></SHAPE><Group><NULL></NULL></Group></Scene>")
both("names-no-scene-element", "<TRANSFORM><BOX></BOX></TRANSFORM>")
# --- field names -----------------------------------------------------------------------------------
both("fields-initializeonly", "<Scene><GROUP BBOXSIZE='1 2 3' BBOXCENTER='0 1 0'></GROUP><SWITCH WHICHCHOICE='1'><BOX></BOX><SPHERE></SPHERE></SWITCH><TIMESENSOR LOOP='true' CYCLEINTERVAL='3' STARTTIME='5'></TIMESENSOR></Scene>")
both("fields-inputoutput", "<Scene><MATERIAL AMBIENTINTENSITY='0.5' SHININESS='0.3' TRANSPARENCY='0.1'></MATERIAL><DIRECTIONALLIGHT ON='false' INTENSITY='0.5' DIRECTION='1 0 0'></DIRECTIONALLIGHT><NAVIGATIONINFO TYPE='\"WALK\"' SPEED='2'></NAVIGATIONINFO></Scene>")
both("fields-inputonly-outputonly", "<Scene><TRANSFORM SET_TRANSLATION='5 5 5' TRANSLATION_CHANGED='7 7 7' ADDCHILDREN='1' ISACTIVE='true'></TRANSFORM><TIMESENSOR ELAPSEDTIME='9' FRACTION_CHANGED='0.5' SET_STARTTIME='3' CYCLETIME='1'></TIMESENSOR></Scene>")
both("fields-inputonly-camel", "<Scene><Transform set_translation='5 5 5' translation_changed='7 7 7' addChildren='1' removeChildren='2'></Transform><Viewpoint set_bind='true' bindTime='5' isBound='true' position='1 1 1'></Viewpoint></Scene>")
both("fields-wrong-node", "<Scene><BOX WHICHCHOICE='3' TRANSLATION='1 1 1' SIZE='1 2 3'></BOX><SPHERE SIZE='5' RADIUS='2'></SPHERE></Scene>")
both("fields-unknown", "<Scene><BOX FOO='1' BAR='2' SIZE='3 3 3'></BOX></Scene>")
both("fields-alias-depthmode", "<Scene><DEPTHMODE DEPTHFUNC='GREATER' DEPTHFUNCTION='LESS'></DEPTHMODE><DEPTHMODE depthFunction='GREATER'></DEPTHMODE></Scene>")
both("fields-aliases-vrml2", "<Scene><SWITCH CHOICE='1'><BOX></BOX></SWITCH><COLLISION COLLIDE='false'></COLLISION><LOD LEVEL='1'></LOD><LAYER ISPICKABLE='false'></LAYER></Scene>")
both("fields-metadata-sfnode", "<Scene><TRANSFORM><METADATASTRING NAME='n' VALUE='\"v\"'></METADATASTRING></TRANSFORM><WORLDINFO TITLE='t' INFO='\"a\", \"b\"'></WORLDINFO></Scene>")
both("fields-reserved-attrs", "<Scene><TRANSFORM DEF='A' CONTAINERFIELD='children'><BOX></BOX></TRANSFORM><TRANSFORM USE='A'></TRANSFORM></Scene>")
both("fields-value-types", "<Scene><SHAPE><INDEXEDFACESET COORDINDEX='0 1 2 -1' SOLID='false' CREASEANGLE='0.5'><COORDINATE POINT='0 0 0, 1 0 0, 0 1 0'></COORDINATE></INDEXEDFACESET></SHAPE></Scene>")
both("fields-entities", "<Scene><WORLDINFO TITLE='a &amp; b &lt; c &#65;' INFO='\"&quot;x&quot;\"'></WORLDINFO></Scene>")
both("fields-unquoted", "<Scene><TRANSFORM TRANSLATION=1 CENTER=0></TRANSFORM><BOX SIZE=2></BOX></Scene>")
both("fields-duplicate-attr", "<Scene><BOX SIZE='1 1 1' size='5 5 5'></BOX></Scene>")
# --- DEF, USE, ROUTE -----------------------------------------------------------------------------------
both("defuse", "<Scene><SHAPE DEF='S'><BOX></BOX></SHAPE><TRANSFORM><SHAPE USE='S'></SHAPE></TRANSFORM><GROUP><TRANSFORM USE='S'></TRANSFORM><FOO USE='S'></FOO></GROUP></Scene>")
both("defuse-lower", "<scene><shape def='S'><box></box></shape><shape use='S'></shape></scene>")
both("route", "<Scene><TIMESENSOR DEF='T' LOOP='true'></TIMESENSOR><POSITIONINTERPOLATOR DEF='P' KEY='0 1' KEYVALUE='0 0 0, 1 1 1'></POSITIONINTERPOLATOR><TRANSFORM DEF='X'></TRANSFORM><ROUTE FROMNODE='T' FROMFIELD='fraction_changed' TONODE='P' TOFIELD='set_fraction'></ROUTE><ROUTE fromNode='P' fromField='value_changed' toNode='X' toField='set_translation'></ROUTE></Scene>")
both("container-field", "<Scene><SHAPE><APPEARANCE><MATERIAL></MATERIAL></APPEARANCE><BOX></BOX></SHAPE><TRANSFORM><SHAPE CONTAINERFIELD='children'><SPHERE CONTAINERFIELD='geometry'></SPHERE></SHAPE></TRANSFORM></Scene>")
# --- head statements ------------------------------------------------------------------------------------
both("head-statements", "<head><component name='Geometry3D' level='2'><meta name='title' content='t'><unit category='length' name='ft' conversionFactor='0.3048'></head><Scene><BOX></BOX></Scene>")
both("head-statements-direct", "<component name='Shape' level='3'></component><meta name='author' content='a'><unit category='angle' name='deg' conversionFactor='0.0174532925199433'></unit><Scene><BOX></BOX></Scene>")
both("head-upper", "<COMPONENT NAME='Shape' LEVEL='3'></COMPONENT><META NAME='k' CONTENT='v'><Scene><BOX></BOX></Scene>")
both("profile-core", "<Scene><BOX></BOX><BACKGROUND SKYCOLOR='1 0 0'></BACKGROUND></Scene>", profile="Core")
both("profile-bad", "<Scene><BOX></BOX></Scene>", profile="Nonsense")
# --- PROTO -----------------------------------------------------------------------------------------------
PROTO = "<PROTODECLARE NAME='MyProto'><PROTOINTERFACE><FIELD NAME='size' TYPE='SFVec3f' ACCESSTYPE='inputOutput' VALUE='1 1 1'></FIELD><FIELD NAME='myColor' TYPE='SFColor' ACCESSTYPE='initializeOnly' VALUE='1 0 0'></FIELD></PROTOINTERFACE><PROTOBODY><SHAPE><APPEARANCE><MATERIAL DEF='M'></MATERIAL></APPEARANCE><BOX></BOX></SHAPE><IS><CONNECT NODEFIELD='diffuseColor' PROTOFIELD='myColor'></CONNECT></IS></PROTOBODY></PROTODECLARE>"
both("proto-instance-name", f"<Scene>{PROTO}<PROTOINSTANCE NAME='MyProto'><FIELDVALUE NAME='size' VALUE='2 2 2'></FIELDVALUE></PROTOINSTANCE></Scene>")
both("proto-instance-upper", f"<Scene>{PROTO}<MYPROTO SIZE='3 3 3' MYCOLOR='0 1 0'></MYPROTO></Scene>")
both("proto-instance-lower", f"<Scene>{PROTO}<myproto size='3 3 3' mycolor='0 1 0'></myproto></Scene>")
both("proto-instance-camel", f"<Scene>{PROTO}<MyProto size='4 4 4' myColor='0 0 1'></MyProto></Scene>")
both("proto-instance-mixed", f"<Scene>{PROTO}<mYpRoTo size='4 4 4' MYCOLOR='0 0 1'></mYpRoTo></Scene>")
both("proto-before-declaration", "<Scene><MYPROTO></MYPROTO><PROTODECLARE NAME='MyProto'><PROTOBODY><BOX></BOX></PROTOBODY></PROTODECLARE><MYPROTO></MYPROTO></Scene>")
both("proto-use-reserved", f"<Scene>{PROTO}<MYPROTO DEF='P' SIZE='1 2 3'></MYPROTO><MYPROTO USE='P'></MYPROTO></Scene>")
both("proto-field-names-builtin", f"<Scene>{PROTO}<MYPROTO TRANSLATION='1 1 1' WHICHCHOICE='2'></MYPROTO></Scene>")
both("proto-extern", "<Scene><EXTERNPROTODECLARE NAME='Ext' URL='\"nowhere.x3d#Ext\"'><FIELD NAME='f' TYPE='SFFloat' ACCESSTYPE='inputOutput'></FIELD></EXTERNPROTODECLARE><EXT F='3'></EXT></Scene>")
both("proto-nested-instance", "<Scene><PROTODECLARE NAME='Inner'><PROTOINTERFACE><FIELD NAME='x' TYPE='SFFloat' ACCESSTYPE='inputOutput' VALUE='1'></FIELD></PROTOINTERFACE><PROTOBODY><BOX></BOX></PROTOBODY></PROTODECLARE><PROTODECLARE NAME='Outer'><PROTOBODY><INNER X='5'></INNER><Inner x='6'></Inner></PROTOBODY></PROTODECLARE><OUTER></OUTER></Scene>")
# --- script and other special HTML elements --------------------------------------------------------------
both("script-cdata", "<Scene><SCRIPT DEF='S' DIRECTOUTPUT='true'><field name='f' type='SFFloat' accessType='inputOutput' value='1'></field><![CDATA[ecmascript:\nfunction initialize () { }\n]]></SCRIPT></Scene>")
both("script-three-children", "<Scene><SCRIPT DEF='S'><field name='a' type='SFFloat' accessType='inputOutput' value='1'></field><field name='b' type='SFFloat' accessType='inputOutput' value='2'></field><field name='c' type='SFFloat' accessType='inputOutput' value='3'></field><![CDATA[ecmascript:\nfunction initialize () { }\n]]></SCRIPT></Scene>")
both("script-spaced-children", "<Scene><SCRIPT DEF='S'>\n<field name='a' type='SFFloat' accessType='inputOutput' value='1'></field>\n<field name='b' type='SFFloat' accessType='inputOutput' value='2'></field>\n<![CDATA[ecmascript:\nfunction initialize () { }\n]]>\n</SCRIPT></Scene>")
both("script-plain-text", "<Scene><Script><field name='f' type='SFFloat' accessType='inputOutput'></field>ecmascript: function initialize () { }</Script></Scene>")
both("script-invalid-xml", "<Scene><Script> if (a < b) { } </Script><BOX></BOX></Scene>")
both("script-url", "<Scene><SCRIPT URL='\"ecmascript:function initialize () { }\"'></SCRIPT></Scene>")
both("fields-user-defined", "<Scene><Script><FIELD NAME='myField' TYPE='SFInt32' ACCESSTYPE='inputOutput' VALUE='7'></FIELD></Script></Scene>")
both("comments-text", "<Scene>text<!-- comment --><BOX></BOX>more</Scene>")
both("inline", "<Scene><INLINE URL='\"a.x3d\"' LOAD='false' BBOXSIZE='1 1 1'></INLINE></Scene>")
both("anchor-lod", "<Scene><ANCHOR DESCRIPTION='d' URL='\"http://x.org\"' PARAMETER='\"target=_blank\"'></ANCHOR><LOD RANGE='1 2' CENTER='0 1 0'><BOX></BOX></LOD></Scene>")
both("geo", "<Scene><GEOVIEWPOINT GEOSYSTEM='\"GD\"' POSITION='1 2 3' NAVTYPE='\"EXAMINE\"' HEADLIGHT='false'></GEOVIEWPOINT></Scene>")
both("two-x3d", "<Scene><BOX></BOX></Scene></x3d><x3d><Scene><SPHERE></SPHERE></Scene>")

for name, text in html.items():
    with open(os.path.join(OUT, name + ".html"), "w", encoding="utf-8", newline="") as f:
        f.write(text)

# --- XML twins: X_ITE reads XML without HTMLSupport --------------------------------------------------------
def x3d(body, extra=""):
    return f'<?xml version="1.0" encoding="UTF-8"?>\n<X3D profile="Full" version="4.0">\n<Scene>\n{body}\n</Scene>\n</X3D>\n'

xml["upper-element-names"] = x3d("<TRANSFORM translation='1 2 3'><SHAPE><BOX size='2 2 2'/></SHAPE></TRANSFORM>")
xml["lower-element-names"] = x3d("<transform translation='1 2 3'><shape><box size='2 2 2'/></shape></transform>")
xml["camel-element-names"] = x3d("<Transform translation='1 2 3'><Shape><Box size='2 2 2'/></Shape></Transform>")
xml["lower-attribute-names"] = x3d("<Switch whichchoice='1'><Box size='2 2 2'/><Sphere/></Switch><Group bboxsize='1 1 1'/><TimeSensor cycleinterval='3' loop='true'/>")
xml["upper-attribute-names"] = x3d("<Switch WHICHCHOICE='1'><Box SIZE='2 2 2'/><Sphere/></Switch>")
xml["camel-attribute-names"] = x3d("<Switch whichChoice='1'><Box size='2 2 2'/><Sphere/></Switch>")
xml["inputoutput-lowercase"] = x3d("<Material diffusecolor='1 0 0' shininess='0.2'/><DirectionalLight intensity='0.5' on='false'/>")
xml["upper-proto-instance"] = x3d("<ProtoDeclare name='P'><ProtoInterface><field name='f' type='SFFloat' accessType='inputOutput' value='1'/></ProtoInterface><ProtoBody><Box/></ProtoBody></ProtoDeclare><P f='2'/><p f='3'/><PROTOINSTANCE name='P'/><ProtoInstance name='P'><fieldValue name='f' value='4'/></ProtoInstance>")
xml["upper-use-def"] = x3d("<Shape DEF='S'><Box/></Shape><SHAPE USE='S'/><Shape use='S'/>")
xml["upper-head"] = '<?xml version="1.0"?>\n<X3D profile="Full" version="4.0"><HEAD><COMPONENT name="Shape" level="3"/><META name="a" content="b"/></HEAD><SCENE><Box/></SCENE></X3D>\n'
xml["lower-head"] = '<?xml version="1.0"?>\n<X3D profile="Full" version="4.0"><head><component name="Shape" level="3"/><meta name="a" content="b"/></head><Scene><Box/></Scene></X3D>\n'
xml["upper-route"] = x3d("<TimeSensor DEF='T'/><Transform DEF='X'/><ROUTE fromNode='T' fromField='fraction_changed' toNode='X' toField='set_rotation'/><route fromNode='T' fromField='time' toNode='X' toField='set_scale'/>")
for name, text in xml.items():
    with open(os.path.join(OUT, "xml", name + ".x3d"), "w", encoding="utf-8", newline="") as f:
        f.write(text)
print(len(html), "html,", len(xml), "xml")
