#!/usr/bin/env python3
"""REA batch 02: authored LayerSet / Layer / Viewport / LayoutLayer / Layout worlds (no archive
content).

  python3 -I oracle/tools/gen_rea_b02_layers.py oracle/synthetic-rea-b02
"""
import json, os, sys

out = sys.argv[1]

HEAD = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Layering:1
COMPONENT Layout:2
COMPONENT PointingDeviceSensor:1
COMPONENT Scripting:1
"""

def shape(geometry, color, x=0, y=0, z=0, extra="", defname=None, material=None):
    mat = material or f"Material {{ diffuseColor {color} }}"
    d = f"DEF {defname} " if defname else ""
    return f"Transform {{ translation {x} {y} {z} children Shape {{ appearance Appearance {{ material {d}{mat} }} geometry {geometry} }} {extra}}}"

# Layer 0: the root nodes.
LAYER0 = """Viewpoint { position 0 0 8 description "zero" }
Background { skyColor [ 0.15 0.2 0.35, 0.5 0.55 0.6 ] skyAngle 1.5 groundColor [ 0.25 0.2 0.15, 0.4 0.35 0.3 ] groundAngle 1.5 }
""" + "\n".join([
    shape("Box { size 2 2 2 }", "0.8 0.2 0.2", -2, 0),
    shape("Sphere { radius 1.2 }", "0.2 0.7 0.3", 1.5, 0.5),
    shape("Cone { }", "0.9 0.8 0.2", 0, -2, -1),
])

def layer(body, viewport=None, extra=""):
    vp = f" viewport Viewport {{ clipBoundary [ {viewport} ] }}" if viewport else ""
    return f"Layer {{{vp}{extra} children [\n{body}\n] }}"

L1 = """Viewpoint { position 0 1 6 orientation 1 0 0 -0.15 description "one" }
Background { skyColor 0.6 0.2 0.5 }
""" + "\n".join([
    shape("Box { size 1.5 1.5 1.5 }", "0.2 0.4 0.9", 0, 0),
    shape("Sphere { radius 0.5 }", "1 1 1", 1.5, 1),
])

L2 = """NavigationInfo { headlight FALSE }
DirectionalLight { direction 0 -1 -0.5 color 1 0.9 0.6 }
""" + "\n".join([
    shape("Cylinder { }", "0.9 0.5 0.1", 0, 0),
    shape("Box { size 3 0.2 3 }", "0.5 0.5 0.5", 0, -1.2),
])

def layerset(layers, order, active=0, extra=""):
    return f"LayerSet {{ activeLayer {active} order [ {order} ]{extra} layers [\n" + "\n".join(layers) + "\n] }"

def layout(align=("CENTER", "CENTER"), offset_units=("WORLD", "WORLD"), offset=(0, 0), size_units=("WORLD", "WORLD"), size=(1, 1), scale=("NONE", "NONE")):
    q = lambda v: " ".join(f'"{s}"' for s in v)
    n = lambda v: " ".join(str(s) for s in v)
    return f"Layout {{ align [ {q(align)} ] offsetUnits [ {q(offset_units)} ] offset [ {n(offset)} ] sizeUnits [ {q(size_units)} ] size [ {n(size)} ] scaleMode [ {q(scale)} ] }}"

def layout_layer(lay, body, viewport=None, extra=""):
    vp = f" viewport Viewport {{ clipBoundary [ {viewport} ] }}" if viewport else ""
    return f"LayoutLayer {{ layout {lay}{vp}{extra} children [\n{body}\n] }}"

def layout_group(lay, body, viewport=None):
    vp = f" viewport Viewport {{ clipBoundary [ {viewport} ] }}" if viewport else ""
    return f"LayoutGroup {{ layout {lay}{vp} children [\n{body}\n] }}"

RECT = lambda c, w=1, h=1, z=0: f"Transform {{ translation 0 0 {z} children Shape {{ appearance Appearance {{ material Material {{ emissiveColor {c} diffuseColor 0 0 0 }} }} geometry IndexedFaceSet {{ coord Coordinate {{ point [ -{w/2} -{h/2} 0, {w/2} -{h/2} 0, {w/2} {h/2} 0, -{w/2} {h/2} 0 ] }} coordIndex [ 0 1 2 3 -1 ] }} }} }}"
PIX = lambda c, w, h, z=0: RECT(c, w, h, z)

worlds = {}
worlds["layers-basic.wrl"] = HEAD + LAYER0 + "\n" + layerset([layer(L1, "0.55 0.95 0.55 0.95"), layer(L2, "0.05 0.45 0.05 0.4")], "0 1 2")
worlds["layers-order.wrl"] = HEAD + LAYER0 + "\n" + layerset([layer(L1, "0.3 0.9 0.2 0.8"), layer(L2, "0.1 0.6 0.1 0.6"), layer(L1.replace('"one"', '"hidden"'), "0 1 0 1", ' visible FALSE')], "2 0 1 3")
worlds["layers-no-layer0.wrl"] = HEAD + LAYER0 + "\n" + layerset([layer(L1, "0.25 0.75 0.25 0.75"), layer(L2)], "1")
worlds["layers-default-order.wrl"] = HEAD + LAYER0 + "\n" + layerset([layer(L1, "0.55 0.95 0.55 0.95")], "0")
worlds["layers-full-overlap.wrl"] = HEAD + LAYER0 + "\n" + layerset([layer(L2)], "0 1")
worlds["layers-default-viewpoint.wrl"] = HEAD + LAYER0 + "\n" + layerset([layer("\n".join([shape("Box { }", "0.3 0.8 0.8", 0, 0)]), "0.5 1 0 0.5")], "0 1")
worlds["layers-clip-out-of-range.wrl"] = HEAD + LAYER0 + "\n" + layerset([layer(L1, "-0.5 0.4 0.6 1.5"), layer(L2, "0.7 0.2 0 1")], "0 1 2")

# Viewport nodes inside the scene graph (X3DViewportNode as a grouping node).
NESTED = """Viewport { clipBoundary [ 0.5 1 0.5 1 ] children [
  Transform { translation 0 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.5 1 } } geometry Sphere { radius 2 } } }
  Viewport { clipBoundary [ 0 0.5 0 0.5 ] children [
    Shape { appearance Appearance { material Material { diffuseColor 1 0.4 0.1 } } geometry Box { size 3 3 3 } }
  ] }
] }
Viewport { clipBoundary [ 0 0.4 0 0.3 ] children [ Shape { appearance Appearance { material Material { diffuseColor 0.9 0.9 0.2 } } geometry Cone { bottomRadius 3 height 4 } } ] }
"""
worlds["viewport-nested.wrl"] = HEAD + LAYER0 + "\n" + NESTED
worlds["viewport-in-layer.wrl"] = HEAD + LAYER0 + "\n" + layerset([layer(L1 + "\n" + NESTED, "0.1 0.9 0.1 0.9")], "0 1")

# Layout.
LAYOUTS = [
    layout_layer(layout(("LEFT", "TOP"), ("PIXEL", "PIXEL"), (10, -10), ("PIXEL", "PIXEL"), (120, 80), ("PIXEL", "PIXEL")), PIX("1 0.3 0.3", 120, 80) + "\n" + PIX("1 1 1", 40, 40, 0.1)),
    layout_layer(layout(("RIGHT", "BOTTOM"), ("FRACTION", "FRACTION"), (-0.05, 0.05), ("FRACTION", "FRACTION"), (0.3, 0.2), ("FRACTION", "FRACTION")), RECT("0.3 1 0.3")),
    layout_layer(layout(("CENTER", "CENTER"), ("WORLD", "WORLD"), (0, 0), ("FRACTION", "FRACTION"), (0.5, 0.25), ("STRETCH", "STRETCH")), RECT("0.3 0.3 1", 1, 1, -0.5)),
    layout_layer(layout(("RIGHT", "TOP"), ("PIXEL", "PIXEL"), (0, 0), ("PIXEL", "PIXEL"), (101, 61), ("PIXEL", "STRETCH")), PIX("1 1 0.2", 101, 61)),
]
worlds["layout-units.wrl"] = HEAD + LAYER0 + "\n" + layerset(LAYOUTS, "0 1 2 3 4")
INNER = layout_group(layout(("CENTER", "CENTER"), ("PIXEL", "PIXEL"), (5, 5), ("PIXEL", "PIXEL"), (30, 20), ("PIXEL", "PIXEL")), PIX("1 1 1", 30, 20, 0.2))
MIDDLE = layout_group(layout(("RIGHT", "TOP"), size_units=("FRACTION", "FRACTION"), size=(0.5, 0.5), scale=("FRACTION", "FRACTION")), RECT("0.9 0.6 0.2", 1, 1, 0) + "\n" + INNER)
worlds["layout-nested.wrl"] = HEAD + LAYER0 + "\n" + layerset([
    layout_layer(layout(("LEFT", "BOTTOM"), size_units=("FRACTION", "FRACTION"), size=(0.5, 0.5), scale=("FRACTION", "FRACTION")), RECT("0.2 0.2 0.6", 1, 1, -0.5) + "\n" + MIDDLE),
    layout_layer(layout(("CENTER", "TOP"), size_units=("FRACTION", "PIXEL"), size=(0.8, 40), scale=("FRACTION", "NONE")), RECT("0.6 0.1 0.6", 1, 0.3), "0.5 1 0 1"),
], "0 1 2")
worlds["layout-default-ortho.wrl"] = HEAD + LAYER0 + "\n" + layerset([layout_layer("NULL", shape("Box { size 0.5 0.5 0.5 }", "0.9 0.9 0.9", 0.5, 0.5) + "\n" + shape("Sphere { radius 0.3 }", "0.9 0.2 0.9", -0.6, -0.6))], "0 1")
worlds["layout-group-root.wrl"] = HEAD + LAYER0 + "\n" + layout_group(layout(("RIGHT", "TOP"), ("PIXEL", "PIXEL"), (-10, -10), ("PIXEL", "PIXEL"), (100, 50), ("PIXEL", "PIXEL")), PIX("1 0.5 0", 100, 50), "0 1 0 1")

STYLES = """Fog { color 0.6 0.6 0.7 visibilityRange 14 }
Background { skyColor 0.2 0.25 0.2 }
Transform { translation -1.5 1 0 children Shape { appearance Appearance { material Material { emissiveColor 1 0.8 0.2 } pointProperties PointProperties { pointSizeScaleFactor 6 pointSizeMinValue 1 pointSizeMaxValue 20 } } geometry PointSet { coord Coordinate { point [ 0 0 0, 0.5 0 0, 1 0 0, 0 0.5 0, 0.5 0.5 0 ] } } } }
Transform { translation 0.5 1 0 children Shape { appearance Appearance { material Material { emissiveColor 0.2 0.9 1 } lineProperties LineProperties { linewidthScaleFactor 4 linetype 3 } } geometry IndexedLineSet { coord Coordinate { point [ 0 0 0, 1.5 0.3 0, 0.4 -0.8 0 ] } coordIndex [ 0 1 2 0 -1 ] } } }
Transform { translation 0 -0.8 0.5 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.2 0.6 transparency 0.5 } } geometry Box { size 2 1 1 } } }
Transform { translation 0.5 -0.8 -1.5 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.9 0.3 } } geometry Sphere { radius 1 } } }
"""
worlds["layers-styles.wrl"] = HEAD + LAYER0 + "\n" + layerset([layer(STYLES, "0.33 0.97 0.03 0.61")], "0 1")
worlds["layout-odd.wrl"] = HEAD + LAYER0 + "\n" + layerset([
    layout_layer(layout(("CENTER", "CENTER"), ("PIXEL", "PIXEL"), (0, 0), ("PIXEL", "PIXEL"), (51, 33), ("PIXEL", "PIXEL")), PIX("1 0.4 0.1", 51, 33), "0.1 0.533 0.2 0.799"),
    layout_layer(layout(("LEFT", "CENTER"), ("FRACTION", "PIXEL"), (0.1, 7), ("PIXEL", "FRACTION"), (40, 0.3), ("PIXEL", "FRACTION")), PIX("0.2 0.9 0.9", 40, 1), "0.55 0.95 0.1 0.9"),
], "0 1 2")

# Stepped: pointer hits across layers, pointerEvents, order, viewport, visible and Layout changes.
RUNTIME = HEAD + """Viewpoint { position 0 0 8 }
Background { skyColor 0.2 0.25 0.3 }
DEF TS0 TouchSensor { }
Shape { appearance Appearance { material DEF M0 Material { diffuseColor 0.8 0.3 0.2 } } geometry Box { size 12 12 0.2 } }
DEF LS LayerSet { order [ 0 1 2 ] layers [
  DEF L1 Layer { viewport DEF V1 Viewport { clipBoundary [ 0.5 1 0.5 1 ] } children [
    Viewpoint { position 0 0 6 }
    Background { skyColor 0.3 0.1 0.4 }
    DEF TS1 TouchSensor { }
    Shape { appearance Appearance { material DEF M1 Material { diffuseColor 0.2 0.4 0.9 } } geometry Box { size 3 3 0.5 } }
  ] }
  DEF L2 LayoutLayer { layout DEF LAY Layout { align [ "LEFT" "BOTTOM" ] offsetUnits [ "PIXEL" "PIXEL" ] offset [ 20 20 ] sizeUnits [ "PIXEL" "PIXEL" ] size [ 100 100 ] scaleMode [ "PIXEL" "PIXEL" ] } children [
    DEF TS2 TouchSensor { }
    Shape { appearance Appearance { material DEF M2 Material { emissiveColor 0.9 0.9 0.2 diffuseColor 0 0 0 } } geometry IndexedFaceSet { coord Coordinate { point [ -50 -50 0, 50 -50 0, 50 50 0, -50 50 0 ] } coordIndex [ 0 1 2 3 -1 ] } }
  ] }
] }
DEF S Script {
  directOutput TRUE
  inputOnly SFTime t0
  inputOnly SFTime t1
  inputOnly SFTime t2
  inputOutput SFInt32 c0 0
  inputOutput SFInt32 c1 0
  inputOutput SFInt32 c2 0
  inputOutput SFInt32 step 0
  initializeOnly SFNode ls USE LS
  initializeOnly SFNode l1 USE L1
  initializeOnly SFNode l2 USE L2
  initializeOnly SFNode v1 USE V1
  initializeOnly SFNode lay USE LAY
  initializeOnly SFNode m0 USE M0
  initializeOnly SFNode m1 USE M1
  initializeOnly SFNode m2 USE M2
  url "javascript:
function t0 (value) { c0 = c0 + 1; m0 .diffuseColor = new SFColor (0.8, 0.3 + 0.15 * c0, 0.2); next (); }
function t1 (value) { c1 = c1 + 1; m1 .diffuseColor = new SFColor (0.2 + 0.3 * c1, 0.4, 0.9); next (); }
function t2 (value) { c2 = c2 + 1; m2 .emissiveColor = new SFColor (0.9, 0.9 - 0.3 * c2, 0.2); next (); }
function next ()
{
   step = step + 1;

   if (step == 1) { l1 .pointerEvents = false; }
   if (step == 2) { l1 .pointerEvents = true; ls .order = new MFInt32 (1, 0, 2); }
   if (step == 3) { ls .order = new MFInt32 (0, 1, 2); v1 .clipBoundary = new MFFloat (0, 0.5, 0.5, 1); }
   if (step == 4) { lay .size = new MFFloat (150, 60); lay .offset = new MFFloat (0, 0); lay .align = new MFString ('CENTER', 'BOTTOM'); }
   if (step == 5) { l1 .visible = false; }
   if (step == 6) { ls .order = new MFInt32 (2, 1); l1 .visible = true; }
   if (step == 7) { l1 .removeChildren = new MFNode (l1 .children [3]); }
   if (step == 8) { l1 .addChildren = new MFNode (l2 .children [1]); }
}
"
}
ROUTE TS0.touchTime TO S.t0
ROUTE TS1.touchTime TO S.t1
ROUTE TS2.touchTime TO S.t2
"""
worlds["layers-runtime.wrl"] = RUNTIME

def clicks(points, start=20, every=14):
    events, capture = [], []
    for i, (x, y) in enumerate(points):
        f = start + i * every
        events += [{"frame": f, "type": "mousemove", "x": x, "y": y},
                   {"frame": f, "type": "mousedown", "button": 0, "x": x, "y": y},
                   {"frame": f + 1, "type": "mouseup", "button": 0, "x": x, "y": y}]
        capture.append(f - 2)
    end = start + len(points) * every
    return events, capture + [end], end

# GL coordinates (y up). Top right is layer 1's first viewport, top left its second.
ev, cap, end = clicks([(400, 400), (400, 400), (400, 400), (100, 400), (70, 70), (256, 50), (100, 400), (256, 30), (100, 400), (400, 100)])
SCRIPTS = {"layers-runtime.json": {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": end, "capture": cap, "dump": [cap[0], end], "events": ev}}

# Stepped: navigation acts on the active layer only, inside its viewport; a KeySensor switches it.
NAV = HEAD.replace("COMPONENT Scripting:1", "COMPONENT Scripting:1\nCOMPONENT KeyDeviceSensor:1") + """Viewpoint { position 0 0 9 description "zero" }
Background { skyColor 0.15 0.2 0.25 }
""" + "\n".join([shape("Box { size 2 2 2 }", "0.8 0.2 0.2", -2, 0), shape("Sphere { radius 1.2 }", "0.2 0.7 0.3", 1.5, 0.5)]) + """
DEF LS LayerSet { activeLayer 1 order [ 0 1 ] layers [
  Layer { viewport Viewport { clipBoundary [ 0.4 1 0.3 1 ] } children [
    Viewpoint { position 0 0 6 description "one" }
    DEF VP1B Viewpoint { position 3 2 5 orientation 0 1 0 0.5 description "one-b" }
    NavigationInfo { type [ "EXAMINE" "ANY" ] transitionType "LINEAR" transitionTime 0.5 }
    Background { skyColor 0.35 0.2 0.4 }
    Shape { appearance Appearance { material Material { diffuseColor 0.2 0.4 0.9 } } geometry Box { size 2 1 1.5 } }
    Transform { translation 1.2 0.8 0 children Shape { appearance Appearance { material Material { diffuseColor 1 1 0.3 } } geometry Sphere { radius 0.4 } } }
  ] }
] }
DEF K KeySensor { }
DEF S Script {
  directOutput TRUE
  inputOnly SFString key
  inputOutput SFInt32 keys 0
  initializeOnly SFNode ls USE LS
  initializeOnly SFNode b USE VP1B
  url "javascript:
function key (value)
{
   keys = keys + 1;
   if (value == 'a') { ls .activeLayer = 0; }
   if (value == 'b') { ls .activeLayer = 1; b .set_bind = true; }
}
"
}
ROUTE K.keyPress TO S.key
"""
worlds["layers-nav.wrl"] = NAV

def drag(f, x0, y0, x1, y1, steps=6):
    out = [{"frame": f, "type": "mousemove", "x": x0, "y": y0}, {"frame": f, "type": "mousedown", "button": 0, "x": x0, "y": y0}]
    for i in range(1, steps + 1):
        out.append({"frame": f + i, "type": "mousemove", "x": x0 + (x1 - x0) * i // steps, "y": y0 + (y1 - y0) * i // steps})
    out.append({"frame": f + steps + 1, "type": "mouseup", "button": 0, "x": x1, "y": y1})
    return out

nav = drag(20, 380, 380, 440, 360)          # inside layer 1: rotates its view
nav += drag(34, 100, 60, 160, 90)           # outside the active viewport: nothing
nav += [{"frame": 48, "type": "key", "key": "a"}]
nav += drag(52, 100, 60, 150, 70)           # layer 0 is active now
nav += [{"frame": 66, "type": "wheel", "deltaY": -120, "x": 200, "y": 100}]
nav += [{"frame": 72, "type": "key", "key": "b"}]
nav += drag(90, 300, 300, 260, 330)         # layer 1 again, bound to its second viewpoint
SCRIPTS["layers-nav.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 110, "capture": [18, 32, 46, 64, 70, 88, 102, 110], "dump": [18, 110], "events": nav}

# Stepped: a drag keeps its layer (pointingLayer) outside the layer's viewport; Viewport nodes
# limit the pointer to their rectangle and pick through their own viewport.
DRAG = HEAD + """Viewpoint { position 0 0 10 }
Background { skyColor 0.2 0.22 0.25 }
DEF TS0 TouchSensor { }
Shape { appearance Appearance { material DEF M0 Material { diffuseColor 0.5 0.5 0.5 } } geometry Box { size 30 30 0.1 } }
Viewport { clipBoundary [ 0 0.4 0.6 1 ] children [
  DEF TSV TouchSensor { }
  Shape { appearance Appearance { material DEF MV Material { diffuseColor 0.9 0.6 0.1 } } geometry Sphere { radius 3 } }
] }
LayerSet { order [ 0 1 ] layers [
  Layer { viewport Viewport { clipBoundary [ 0.5 1 0 0.5 ] } children [
    Viewpoint { position 0 0 6 }
    DEF PS PlaneSensor { maxPosition 3 3 minPosition -3 -3 }
    DEF T Transform { children Shape { appearance Appearance { material DEF M1 Material { diffuseColor 0.2 0.5 1 } } geometry Box { size 1.5 1.5 0.5 } } }
  ] }
] }
DEF S Script {
  directOutput TRUE
  inputOnly SFTime t0
  inputOnly SFTime tv
  inputOnly SFBool over0
  inputOutput SFInt32 c0 0
  inputOutput SFInt32 cv 0
  inputOutput SFInt32 o0 0
  initializeOnly SFNode m0 USE M0
  initializeOnly SFNode mv USE MV
  url "javascript:
function t0 (value) { c0 = c0 + 1; m0 .diffuseColor = new SFColor (0.5, 0.5 + 0.2 * c0, 0.5); }
function tv (value) { cv = cv + 1; mv .diffuseColor = new SFColor (0.9, 0.6, 0.1 + 0.3 * cv); }
function over0 (value) { if (value) o0 = o0 + 1; }
"
}
ROUTE PS.translation_changed TO T.set_translation
ROUTE TS0.touchTime TO S.t0
ROUTE TSV.touchTime TO S.tv
ROUTE TS0.isOver TO S.over0
"""
DRAG = DRAG.replace("COMPONENT Scripting:1", "COMPONENT Scripting:1\nCOMPONENT KeyDeviceSensor:1").replace("LayerSet { order", "DEF LS LayerSet { order").replace("  Layer { viewport Viewport { clipBoundary [ 0.5 1 0 0.5 ] }", "  DEF L1 Layer { viewport Viewport { clipBoundary [ 0.5 1 0 0.5 ] }")
DRAG += """DEF K KeySensor { }
DEF S2 Script {
  directOutput TRUE
  inputOnly SFString key
  initializeOnly SFNode l1 USE L1
  initializeOnly SFNode ls USE LS
  initializeOnly SFNode t USE T
  initializeOnly SFNode ps USE PS
  url "javascript:
function key (value)
{
   if (value == 'r') { t .translation = new SFVec3f (0, 0, 0); ps .offset = new SFVec3f (0, 0, 0); }
   if (value == 'h') l1 .visible = false;
   if (value == 'v') l1 .visible = true;
   if (value == 'o') ls .order = new MFInt32 (0);
}
"
}
ROUTE K.keyPress TO S2.key
"""
worlds["layers-drag.wrl"] = DRAG
dr = drag(20, 384, 128, 200, 300, steps=10)          # PlaneSensor of layer 1, out into layer 0
dr += drag(36, 380, 90, 470, 40, steps=6)            # again, inside layer 1
ev, cap, end = clicks([(100, 450), (180, 450), (100, 300), (60, 330)], start=50, every=10)
# The layer that holds the pointer is hidden, then dropped from the order, during drags.
hide = [{"frame": end + 1, "type": "key", "key": "r"}] + drag(end + 2, 384, 128, 420, 150, steps=8)
hide.insert(6, {"frame": end + 5, "type": "key", "key": "h"})
hide += [{"frame": end + 14, "type": "key", "key": "v"}, {"frame": end + 15, "type": "key", "key": "r"}]
hide2 = drag(end + 18, 384, 128, 350, 100, steps=8)
hide2.insert(5, {"frame": end + 21, "type": "key", "key": "o"})
after, cap2, end2 = clicks([(470, 40)], start=end + 32, every=8)
SCRIPTS["layers-drag.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": end2, "capture": [18, 26, 31, 48] + cap + [end + 12, end + 28] + cap2, "dump": [18, end, end + 12, end + 28, end2], "events": dr + ev + hide + hide2 + after}

# Stepped (shared pointer repair): a shape gets the sensors of its innermost group that has
# enabled ones (renderObject.getSensors () .at (-1)), not every enclosing group's.
NESTED_SENSORS = HEAD + """Viewpoint { position 0 0 10 }
DEF TS0 TouchSensor { }
Shape { appearance Appearance { material DEF M0 Material { diffuseColor 0.5 0.5 0.5 } } geometry Box { size 30 30 0.1 } }
Transform { translation -2.5 0 0 children [
  DEF TSG TouchSensor { }
  Shape { appearance Appearance { material DEF MG Material { diffuseColor 1 0.2 0.2 } } geometry Sphere { radius 1.5 } }
  Transform { translation 0 -3 0 children [
    DEF TSI TouchSensor { }
    Shape { appearance Appearance { material Material { diffuseColor 0.2 1 0.2 } } geometry Box { size 1 1 1 } }
  ] }
] }
Transform { translation 2.5 0 0 children [
  DEF TSD TouchSensor { enabled FALSE }
  Shape { appearance Appearance { material Material { diffuseColor 0.2 0.2 1 } } geometry Box { size 2 2 2 } }
] }
DEF S Script {
  directOutput TRUE
  inputOnly SFTime t0
  inputOnly SFTime tg
  inputOnly SFTime ti
  inputOutput SFInt32 c0 0
  inputOutput SFInt32 cg 0
  inputOutput SFInt32 ci 0
  initializeOnly SFNode m0 USE M0
  initializeOnly SFNode mg USE MG
  url "javascript:
function t0 (v) { c0 = c0 + 1; m0 .diffuseColor = new SFColor (0.5, 0.5 + 0.1 * c0, 0.5); }
function tg (v) { cg = cg + 1; mg .diffuseColor = new SFColor (1, 0.2 + 0.3 * cg, 0.2); }
function ti (v) { ci = ci + 1; }
"
}
ROUTE TS0.touchTime TO S.t0
ROUTE TSG.touchTime TO S.tg
ROUTE TSI.touchTime TO S.ti
"""
worlds["pointer-nested-sensors.wrl"] = NESTED_SENSORS
ev, cap, end = clicks([(125, 256), (101, 71), (380, 256), (256, 450)], start=12, every=8)
SCRIPTS["pointer-nested-sensors.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": end, "capture": cap, "dump": [cap[0], end], "events": ev}

# Stepped (shared pointer repair): an Anchor's own TouchSensor is pushed before its children's
# sensors, so a drag sensor child replaces it; the outer sensor of the enclosing group does not
# apply to it. (A plain Anchor "#V2" click is not part: x3d-drive reports anchor activations and
# does not follow them.)
ANCHOR_SENSORS = HEAD + """DEF V1 Viewpoint { position 0 0 10 description "start" }
DEF V2 Viewpoint { position 0 1 12 description "anchored" }
Group { children [
  DEF OUTER PlaneSensor { }
  DEF A1 Anchor { description "drag anchor" url "#V2" children [
    DEF PS PlaneSensor { }
    Transform { translation -2 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.3 0.2 } } geometry Box { size 2 2 2 } } }
  ] }
  DEF A2 Anchor { description "plain anchor" url "#V2" children [
    Transform { translation 2.5 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.3 0.9 } } geometry Box { size 2 2 2 } } }
  ] }
  Transform { translation 0 -3 0 children Shape { appearance Appearance { material Material { diffuseColor 0.3 0.8 0.3 } } geometry Box { size 3 1 1 } } }
] }
DEF S Script {
  inputOnly SFBool outer
  inputOnly SFBool inner
  inputOutput SFInt32 o 0
  inputOutput SFInt32 i 0
  url "javascript:
function outer (v) { if (v) o = o + 1; }
function inner (v) { if (v) i = i + 1; }
"
}
ROUTE OUTER.isActive TO S.outer
ROUTE PS.isActive TO S.inner
"""
worlds["pointer-anchor-sensors.wrl"] = ANCHOR_SENSORS
ev = drag(12, 155, 256, 175, 270, steps=4)           # the drag anchor: its PlaneSensor, no jump
ev += drag(26, 256, 70, 280, 80, steps=4)            # the green box: the outer PlaneSensor
SCRIPTS["pointer-anchor-sensors.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 60, "capture": [10, 24, 38, 60], "dump": [10, 24, 38, 60], "events": ev}

# Still: X3DBoundedObject visible FALSE takes a Group and a Viewport out of the display.
worlds["viewport-visible.wrl"] = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Layering:1
Viewpoint { position 0 0 8 }
Group { visible FALSE children Shape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Box { } } }
Viewport { visible FALSE clipBoundary [ 0 0.5 0 1 ] children Shape { appearance Appearance { material Material { diffuseColor 0 1 0 } } geometry Sphere { radius 3 } } }
Transform { translation 2 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0 0 1 } } geometry Cone { } } }
"""

# Stepped: each layer has its own lights (a global PointLight of layer 1 does not light layer 0),
# its own Viewpoint stack and bind events, and a layer with pointerEvents FALSE (and the
# isPickable alias of pickable) lets the pointer through to the layer below.
BIND = HEAD.replace("COMPONENT Scripting:1", "COMPONENT Scripting:1\nCOMPONENT KeyDeviceSensor:1") + """DEF V0 Viewpoint { position 0 0 10 description "layer 0" }
NavigationInfo { headlight FALSE }
DirectionalLight { color 0.2 0.4 1 direction 0 0 -1 }
Transform { translation -2.5 0 0 children Shape { appearance Appearance { material Material { } } geometry Sphere { radius 1.5 } } }
LayerSet { order [ 0 1 2 ] layers [
  Layer { viewport Viewport { clipBoundary [ 0.5 1 0 1 ] } children [
    DEF V1A Viewpoint { position 0 0 10 description "layer 1 a" }
    DEF V1B Viewpoint { position 0 2 14 description "layer 1 b" }
    NavigationInfo { headlight FALSE }
    PointLight { global TRUE color 1 0.2 0.1 location 0 0 6 }
    DEF T1 TouchSensor { }
    Shape { appearance Appearance { material Material { } } geometry Box { size 3 3 3 } }
  ] }
  DEF L2 Layer { isPickable FALSE pointerEvents FALSE viewport Viewport { clipBoundary [ 0.5 1 0 1 ] } children [
    Viewpoint { position 0 0 10 }
    DEF T2 TouchSensor { }
    Transform { translation 0 -3 0 children Shape { appearance Appearance { material Material { diffuseColor 0.3 0.8 0.3 } } geometry Box { size 2 1 2 } } }
    Transform { scale 0.3 0.3 0.3 children Shape { appearance Appearance { material Material { diffuseColor 0.8 0.8 0.3 } } geometry Sphere { } } }
  ] }
] }
DEF K KeySensor { }
DEF S Script {
  directOutput TRUE
  inputOnly SFString key
  inputOnly SFBool b0
  inputOnly SFBool b1a
  inputOnly SFBool b1b
  inputOnly SFTime t1
  inputOnly SFTime t2
  inputOutput SFInt32 n0 0
  inputOutput SFInt32 n1a 0
  inputOutput SFInt32 n1b 0
  inputOutput SFInt32 c1 0
  inputOutput SFInt32 c2 0
  inputOutput SFString last ""
  initializeOnly SFNode l2 USE L2
  initializeOnly SFNode v1b USE V1B
  url "javascript:
function key (value)
{
   if (value == 'b') v1b .set_bind = true;
   if (value == 'u') v1b .set_bind = false;
   if (value == 'p') l2 .pointerEvents = true;
}
function b0 (v) { n0 = n0 + 1; last = last + ' 0' + v; }
function b1a (v) { n1a = n1a + 1; last = last + ' 1a' + v; }
function b1b (v) { n1b = n1b + 1; last = last + ' 1b' + v; }
function t1 () { c1 = c1 + 1; }
function t2 () { c2 = c2 + 1; }
"
}
ROUTE K.keyPress TO S.key
ROUTE V0.isBound TO S.b0
ROUTE V1A.isBound TO S.b1a
ROUTE V1B.isBound TO S.b1b
ROUTE T1.touchTime TO S.t1
ROUTE T2.touchTime TO S.t2
"""
worlds["layers-bind-light.wrl"] = BIND
ev, cap, end = clicks([(384, 256)], start=20)
ev += [{"frame": end + 2, "type": "key", "key": "b"}]
ev2, cap2, end2 = clicks([(384, 256)], start=end + 10)
ev2 += [{"frame": end2 + 2, "type": "key", "key": "p"}]
ev3, cap3, end3 = clicks([(384, 256), (384, 150)], start=end2 + 10)
ev3 += [{"frame": end3 + 2, "type": "key", "key": "u"}]
SCRIPTS["layers-bind-light.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": end3 + 12, "capture": [18] + cap + cap2 + cap3 + [end3 + 12], "dump": [18, end, end2, end3, end3 + 12], "events": ev + ev2 + ev3}

# Stepped: X3DLayerNode's legacy alias isPickable of pickable exists up to specification 3.3 (in the
# 4.0 world above it is an unknown field).
PICK33 = """#X3D V3.3 utf8
PROFILE Interchange
COMPONENT Layering:1
Viewpoint { position 0 0 10 }
LayerSet { order [ 0 1 2 ] layers [
  Layer { isPickable FALSE children Shape { appearance Appearance { material Material { diffuseColor 0.8 0.3 0.3 } } geometry Box { } } }
  LayoutLayer { isPickable FALSE }
] }
"""
worlds["layers-pickable-v33.wrl"] = PICK33
SCRIPTS["layers-pickable-v33.json"] = {"size": [256, 256], "t0": 946684800, "dt": 1 / 30, "frames": 12, "capture": [12], "dump": [12], "events": []}

# Stepped: WALK collision and gravity use the active layer's shapes only.
WALK = HEAD + """Viewpoint { position 0 1.6 8 }
Background { skyColor 0.2 0.2 0.2 }
Transform { translation 0 1.5 5 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.2 0.2 } } geometry Box { size 6 4 0.2 } } }
LayerSet { activeLayer 1 order [ 0 1 ] layers [
  Layer { viewport Viewport { clipBoundary [ 0.25 1 0.25 1 ] } children [
    Viewpoint { position 0 3 8 }
    NavigationInfo { type [ "WALK" "ANY" ] avatarSize [ 0.25 1.6 0.75 ] }
    Background { skyColor 0.3 0.35 0.5 }
    Transform { translation 0 -0.05 0 children Shape { appearance Appearance { material Material { diffuseColor 0.4 0.6 0.3 } } geometry Box { size 30 0.1 30 } } }
    Transform { translation 0 1.5 -2 children Shape { appearance Appearance { material Material { diffuseColor 0.8 0.8 0.2 } } geometry Box { size 6 3 0.3 } } }
  ] }
] }
"""
worlds["layers-walk.wrl"] = WALK
walk = [{"frame": 30, "type": "mousemove", "x": 320, "y": 250}, {"frame": 30, "type": "mousedown", "button": 0, "x": 320, "y": 250}]
walk += [{"frame": 30 + i, "type": "mousemove", "x": 320, "y": 250 + 30 * i} for i in range(1, 9)]
walk += [{"frame": 130, "type": "mouseup", "button": 0, "x": 320, "y": 490}]
SCRIPTS["layers-walk.json"] = {"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 150, "capture": [28, 60, 90, 130, 150], "dump": [28, 150], "events": walk}

for name, script in SCRIPTS.items():
    json.dump(script, open(os.path.join(out, name), "w"), indent=1)

for name, text in worlds.items():
    open(os.path.join(out, name), "w").write(text + "\n")
