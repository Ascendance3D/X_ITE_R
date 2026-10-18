#!/usr/bin/env python3
"""REA batch 03 RenderedTexture worlds (rt-*.wrl) for oracle/synthetic-rea-b03.

  python3 -I oracle/tools/gen_rea_b03_rendered.py oracle/synthetic-rea-b03

X_ITE 16.5.0's RenderedTexture (experimental there): width / height (non-square, below 16 texels,
0 clears), the layer's viewpoint or a Viewpoint / OrthoViewpoint child, Background, Fog and
NavigationInfo children, a Group of children or the whole layer, depthMap, repeat and
TextureProperties; with MultiTexture, PhysicalMaterial textures, a TextureProjector, a
GeneratedCubeMapTexture, LayerSet with Viewport and LayoutLayer, PickableGroup; a stepped runtime
case (singleFrame, updateInterval, field changes, aspectRatio of a projector).
"""
import json, os, sys

HEAD = """#X3D V4.1 utf8
PROFILE Interchange
COMPONENT Texturing:4
COMPONENT TextureProjection:2
COMPONENT CubeMapTexturing:3
COMPONENT Layering:1
COMPONENT Layout:2
COMPONENT Lighting:3
COMPONENT EnvironmentalEffects:4
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
COMPONENT Picking:3
Viewpoint { position 0 0 14 }
"""

SCENE = "\n".join([
    "Transform { translation -3 2 -2 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.3 0.2 } } geometry Sphere { radius 1 } } }",
    "Transform { translation 0 2.5 -3 rotation 1 1 0 0.5 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.8 0.3 } } geometry Box { size 1.6 1.6 1.6 } } }",
    "Transform { translation 3 2 -2 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.4 0.9 } } geometry Cone { bottomRadius 1 height 2 } } }",
])

def quad(tex, x, y, w=2.6, h=2.6, mat="Material { diffuseColor 1 1 1 }", extra=""):
    return (f"Transform {{ translation {x} {y} 0 children Shape {{ appearance Appearance {{ material {mat} texture {tex} {extra} }} "
            f"geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ {-w/2} {-h/2} 0, {w/2} {-h/2} 0, {w/2} {h/2} 0, {-w/2} {h/2} 0 ] }} coordIndex [ 0 1 2 3 -1 ] "
            f"texCoord TextureCoordinate {{ point [ 0 0, 1 0, 1 1, 0 1 ] }} }} }} }}")

CHILD_SCENE = ("Transform { rotation 0 1 0 0.6 children [ Shape { appearance Appearance { material Material { diffuseColor 1 0.8 0.2 } } geometry Box { size 1.2 1.2 1.2 } } "
               "Transform { translation 1.6 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.7 1 } } geometry Sphere { radius 0.6 } } } ] }")

worlds, scripts = {}, {}
worlds["rt-layer.wrl"] = HEAD + "\n".join([
    "Background { skyColor [ 0.1 0.1 0.2 ] }",
    SCENE,
    quad("RenderedTexture { width 128 height 128 }", -3.2, -2),
    quad("RenderedTexture { width 200 height 100 repeatS FALSE }", 1.2, -2, w=4, h=2),
    quad("RenderedTexture { width 8 height 8 }", 4.6, 1, w=1.6, h=1.6),
]) + "\n"
worlds["rt-children.wrl"] = HEAD + "\n".join([
    "Background { skyColor [ 0.15 0.15 0.15 ] }",
    SCENE,
    quad(f"RenderedTexture {{ width 128 height 128 children [ Viewpoint {{ position 0 0 5 }} Background {{ skyColor [ 0.2 0.3 0.6 ] }} {CHILD_SCENE} ] }}", -4, -2.2),
    quad(f"RenderedTexture {{ width 128 height 64 children [ OrthoViewpoint {{ position 0 0 5 fieldOfView -2 -1 2 1 }} NavigationInfo {{ headlight FALSE }} DirectionalLight {{ direction -1 -1 -1 }} {CHILD_SCENE} ] }}", 0, -2.2, w=3.6, h=1.8),
    quad(f"RenderedTexture {{ width 96 height 96 children [ Viewpoint {{ position 0 0 6 fieldOfView 0.6 }} Fog {{ color 0.8 0.8 0.8 visibilityRange 8 }} Background {{ skyColor [ 0.8 0.8 0.8 ] }} {CHILD_SCENE} ] textureProperties TextureProperties {{ magnificationFilter \"NEAREST_PIXEL\" minificationFilter \"NEAREST_PIXEL\" }} }}", 4, -2.2),
]) + "\n"
worlds["rt-depth.wrl"] = HEAD + "\n".join([
    SCENE,
    quad(f"RenderedTexture {{ depthMap TRUE width 128 height 128 children [ Viewpoint {{ position 0 0 4 }} NavigationInfo {{ visibilityLimit 8 }} {CHILD_SCENE} ] }}", -3.2, -2, mat="UnlitMaterial { }"),
    quad("RenderedTexture { depthMap TRUE width 128 height 128 }", 0, -2, mat="UnlitMaterial { }"),
    quad("RenderedTexture { width 0 height 64 }", 3.2, -2),
]) + "\n"
worlds["rt-cross.wrl"] = HEAD + "\n".join([
    "Background { skyColor [ 0.1 0.1 0.1 ] }",
    SCENE,
    "DEF RT RenderedTexture { width 128 height 128 children [ Viewpoint { position 0 0 5 } Background { skyColor [ 0.3 0.1 0.1 ] } " + CHILD_SCENE + " ] }",
    # MultiTexture, a PhysicalMaterial base texture, a TextureProjector.
    quad('MultiTexture { mode [ "MODULATE" "ADD" ] texture [ USE RT PixelTexture { image 2 2 3 0x202020 0x404040 0x404040 0x202020 } ] }', -4.5, -2.2),
    quad("USE RT", -1.5, -2.2, mat="PhysicalMaterial { baseColor 1 1 1 metallic 0 roughness 0.6 baseTexture USE RT }"),
    "TextureProjector { location 1.5 -2.2 4 direction 0 0 -1 upVector 0 1 0 fieldOfView 0.5 texture USE RT }",
    "Transform { translation 1.5 -2.2 -0.5 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } } geometry Box { size 2.6 2.6 0.2 } } }",
    "Transform { translation 4.5 -2.2 0 children Shape { appearance Appearance { material Material { diffuseColor 0 0 0 specularColor 0.2 0.2 0.2 } texture GeneratedCubeMapTexture { update \"ALWAYS\" size 64 } } geometry Sphere { radius 1.2 } } }",
    "DEF PG PickableGroup { children [ " + quad("USE RT", 4.5, 2.2, w=1.5, h=1.5) + " ] }",
    "LayerSet { order [ 0 1 2 ] layers [",
    "  Layer { viewport Viewport { clipBoundary [ 0.7 1 0.7 1 ] } children [ Viewpoint { position 0 0 5 } Background { skyColor [ 0.2 0.2 0.2 ] } " + quad("RenderedTexture { width 64 height 64 }", 0, 0) + " Transform { translation 0 0 -2 children Shape { appearance Appearance { material Material { diffuseColor 1 0 1 } } geometry Sphere { } } } ] }",
    "  LayoutLayer { layout Layout { size [ 0.25 0.25 ] sizeUnits [ \"FRACTION\" \"FRACTION\" ] align [ \"LEFT\" \"TOP\" ] } children [ Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture USE RT } geometry Rectangle2D { size 0.25 0.25 } } ] }",
    "] }",
]) + "\n"

worlds["rt-runtime.wrl"] = HEAD + "\n".join([
    "Background { skyColor [ 0.1 0.1 0.2 ] }",
    "DEF SPIN Transform { translation 0 2.5 -2 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.6 0.1 } } geometry Box { size 1.5 1.5 1.5 } } }",
    "DEF TS TimeSensor { cycleInterval 4 loop TRUE }",
    "DEF OI OrientationInterpolator { key [ 0 0.5 1 ] keyValue [ 0 1 0 0, 0 1 0 3.14159, 0 1 0 6.28318 ] }",
    "ROUTE TS.fraction_changed TO OI.set_fraction",
    "ROUTE OI.value_changed TO SPIN.set_rotation",
    "DEF RT RenderedTexture { width 128 height 128 singleFrame FALSE updateInterval 0.2 }",
    quad("USE RT", -3, -2),
    "DEF RT2 RenderedTexture { width 96 height 96 children [ DEF V2 Viewpoint { position 0 0 6 } DEF B2 Background { skyColor [ 0.2 0.5 0.2 ] } USE SPIN ] }",
    quad("USE RT2", 0.5, -2),
    "DEF P TextureProjector { location 3.6 -2 4 direction 0 0 -1 upVector 0 1 0 fieldOfView 0.5 texture USE RT2 }",
    "Transform { translation 3.6 -2 -0.5 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } } geometry Box { size 2.4 2.4 0.2 } } }",
    "DEF K KeySensor { }",
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOnly SFFloat on_aspect
  inputOutput SFString log ""
  initializeOnly SFNode rt USE RT
  initializeOnly SFNode rt2 USE RT2
  initializeOnly SFNode b2 USE B2
  url "javascript:
function on_aspect (value) { log = log + 'A' + value .toFixed (4) + ' '; }
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') rt2 .singleFrame = false;
   if (k == 'b') { rt2 .singleFrame = true; rt2 .width = 160; rt2 .height = 64; }
   if (k == 'c') rt .singleFrame = true;
   if (k == 'd') b2 .transparency = 0.5;
   if (k == 'e') rt2 .children = new MFNode ();
   if (k == 'f') rt2 .depthMap = true;
   if (k == 'g') rt2 .width = 0;
   if (k == 'h') { rt2 .depthMap = false; rt2 .width = 64; rt2 .height = 128; }
   if (k == 'i') rt .updateInterval = 0;
   if (k == 'j') rt .singleFrame = false;
}
"
}""",
    "ROUTE K.keyPress TO R.on_key",
    "ROUTE P.aspectRatio TO R.on_aspect",
]) + "\n"
FOCUS = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507}, {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
keys = [(20 + 12 * i, k) for i, k in enumerate("abcdefghij")]
after = [f + 7 for f, _ in keys] + [f + 10 for f, _ in keys]
scripts["rt-runtime.json"] = {"settle": True, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 145, "capture": sorted([12, 16] + after), "dump": [10, 16] + [f + 10 for f, _ in keys] + [144], "events": FOCUS + [{"frame": f, "type": "key", "key": k} for f, k in keys]}

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
for name, text in worlds.items():
    with open(os.path.join(out, name), "w") as f:
        f.write(text)
for name, script in scripts.items():
    with open(os.path.join(out, name), "w") as f:
        json.dump(script, f, indent=1)
print(len(worlds), "worlds,", len(scripts), "scripts")
