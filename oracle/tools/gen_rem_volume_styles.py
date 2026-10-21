#!/usr/bin/env python3
"""Worlds for the remediation lane's volume style lifecycle matrix (crates/x3d-viewer/tests/rea_rem_volume_styles.rs):
one VolumeData in a live Group, one style per world, assigned at the start. Keys: b update a style field,
c clear the style from the volume, a assign it again, d remove the volume from the Group, e add it again."""
import math, os

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b07-vol", "styles")
N = 8
voxels = []
for z in range(N):
    for y in range(N):
        for x in range(N):
            r = math.sqrt(((x - 3.5) / 3.5) ** 2 + ((y - 3.5) / 3.5) ** 2 + ((z - 3.5) / 3.5) ** 2)
            voxels.append(max(0, min(255, int(255 * (1 - r)))))
IMAGE = "image [ 1 %d %d %d %s ]" % (N, N, N, " ".join(map(str, voxels)))
TEX = "PixelTexture3D { %s }" % IMAGE

STYLES = {
    "boundary": ("BoundaryEnhancementVolumeStyle { boundaryOpacity 0.2 retainedOpacity 0.5 opacityFactor 1 }", "boundaryOpacity = 0.9;"),
    "cartoon": ("CartoonVolumeStyle { colorSteps 3 orthogonalColor 1 0 0 1 parallelColor 0 0 1 1 }", "colorSteps = 6;"),
    "composed": ("ComposedVolumeStyle { renderStyle [ CartoonVolumeStyle { colorSteps 2 } ] }", "enabled = false;"),
    "edge": ("EdgeEnhancementVolumeStyle { edgeColor 0 1 0 1 gradientThreshold 0.2 }", "edgeColor = new SFColorRGBA (1, 0, 0, 1);"),
    "opacitymap": ("OpacityMapVolumeStyle { type \"DEFAULT\" }", "type = 'MAX';"),
    "projection": ("ProjectionVolumeStyle { type \"MAX\" }", "type = 'MIN';"),
    "silhouette": ("SilhouetteEnhancementVolumeStyle { silhouetteBoundaryOpacity 0.2 silhouetteRetainedOpacity 0.5 silhouetteSharpness 1 }", "silhouetteSharpness = 4;"),
    "tonemapped": ("ToneMappedVolumeStyle { coolColor 0 0 1 1 warmColor 1 1 0 1 }", "coolColor = new SFColorRGBA (0, 1, 0, 1);"),
}

for name, (style, update) in STYLES.items():
    world = f"""#X3D V4.1 utf8
PROFILE Full
COMPONENT Texturing3D:2
COMPONENT VolumeRendering:4
META "generator" "oracle/tools/gen_rem_volume_styles.py"
NavigationInfo {{ type [ "NONE" ] headlight FALSE }}
Background {{ skyColor [ 0.12 0.16 0.3 ] }}
Viewpoint {{ position 0 0 4 }}
DEF G Group {{ children [ DEF VD VolumeData {{ dimensions 2 2 2 renderStyle DEF S {style} voxels {TEX} }} ] }}

DEF K KeySensor {{ }}
DEF R Script {{
  directOutput TRUE
  inputOnly SFString on_key
  initializeOnly SFNode g USE G
  initializeOnly SFNode vd USE VD
  initializeOnly SFNode s USE S
  url "javascript:
function on_key (k)
{{
   if (k == 'a') vd .renderStyle = new MFNode (s);
   if (k == 'b') s .{update}
   if (k == 'c') vd .renderStyle = new MFNode ();
   if (k == 'd') g .removeChildren = new MFNode (vd);
   if (k == 'e') g .addChildren = new MFNode (vd);
}}
"
}}
ROUTE K.keyPress TO R.on_key
"""
    open(os.path.join(out, f"style-{name}.wrl"), "w").write(world)
print(len(STYLES), "worlds")
