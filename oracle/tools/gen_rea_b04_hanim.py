#!/usr/bin/env python3
"""REA batch 04: oracle/synthetic-rea-b04/hanim-humanoid.{wrl,json} (python3 -I oracle/tools/gen_rea_b04_hanim.py oracle/synthetic-rea-b04/hanim-humanoid)."""
import json, sys
out = sys.argv[1]
# Skin: a square column of 4 levels (y = 0, 0.8, 1.6, 2.4), 4 corners each: points 0..15.
pts = []
for lv in range(4):
    y = lv * 0.8
    for (x, z) in [(-0.3, -0.3), (0.3, -0.3), (0.3, 0.3), (-0.3, 0.3)]:
        pts.append(f"{x} {y:.1f} {z}")
faces = []
for lv in range(3):
    b = lv * 4
    for k in range(4):
        a0, a1 = b + k, b + (k + 1) % 4
        faces.append(f"{a0} {a1} {a1 + 4} {a0 + 4} -1")
faces.append("3 2 1 0 -1")
faces.append("12 13 14 15 -1")
# Segment shape: a small quad on the forearm with its own coord.
world = f'''#X3D V4.1 utf8
PROFILE Interchange
COMPONENT HAnim:3
COMPONENT KeyDeviceSensor:1
COMPONENT PointingDeviceSensor:1
COMPONENT Scripting:1
Viewpoint {{ position 0 1.2 6 }}
DEF H HAnimHumanoid {{
  name "test"
  translation -0.5 0 0
  skeleton DEF ROOT HAnimJoint {{
    name "humanoid_root"
    skinCoordIndex [0 1 2 3 4 5 6 7]
    skinCoordWeight [1 1 1 1 0.5 0.5 0.5 0.5]
    children [
      DEF ELBOW HAnimJoint {{
        name "l_elbow"
        center 0 0.8 0
        skinCoordIndex [4 5 6 7 8 9 10 11 12 13 14 15]
        skinCoordWeight [0.5 0.5 0.5 0.5 1 1 1 1 1 1 1 1]
        displacers DEF D HAnimDisplacer {{ name "l_elbow_bulge" coordIndex [9 10] displacements [0.3 0 0, 0.3 0 0] weight 0.5 }}
        children DEF SEG HAnimSegment {{
          name "l_forearm"
          coord DEF SC Coordinate {{ point [0.6 1.0 0, 1.0 1.0 0, 1.0 1.4 0, 0.6 1.4 0] }}
          displacers DEF SD HAnimDisplacer {{ coordIndex [2] displacements [0 0.3 0] weight 1 }}
          children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.9 0.8 0.2 }} }} geometry IndexedFaceSet {{ solid FALSE coord USE SC coordIndex [0 1 2 3 -1] }} }}
        }}
      }}
    ]
  }}
  joints [ USE ROOT USE ELBOW ]
  skinCoord DEF SKIN Coordinate {{ point [ {", ".join(pts)} ] }}
  skin [
    DEF TS TouchSensor {{ }}
    Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.3 0.6 0.9 }} }} geometry IndexedFaceSet {{ coord USE SKIN coordIndex [ {" ".join(faces)} ] }} }}
  ]
  motionsEnabled [TRUE FALSE]
  motions [
    DEF M HAnimMotion {{
      enabled FALSE
      joints "humanoid_root, l_elbow"
      channels "6 Xposition Yposition Zposition Zrotation Xrotation Yrotation 3 Zrotation Xrotation Yrotation"
      values [ 0 0 0 0 0 0 0 0 0,
               0.5 0 0 0 0 10 30 0 0,
               0.5 0.5 0 0 0 20 60 0 0,
               0 0 0 0 0 0 90 0 0 ]
      frameDuration 0.5
      loop TRUE
    }}
    DEF M2 HAnimMotion {{ joints "l_elbow" channels "3 Xrotation Yrotation Zrotation" values [0 0 0, 0 90 0] }}
  ]
  children DEF P HAnimPose {{
    transitionDuration 1
    children HAnimJoint {{ name "l_elbow" rotation 0 0 1 -0.8 }}
  }}
}}
DEF K KeySensor {{ }}
DEF R Script {{
  directOutput TRUE
  inputOnly SFString on_key
  inputOnly SFInt32 on_frameCount
  inputOnly SFInt32 on_frameIndex
  inputOnly SFBool on_poseActive
  inputOnly SFBool on_touch
  inputOutput SFString log ""
  initializeOnly SFNode m USE M
  initializeOnly SFNode p USE P
  initializeOnly SFNode d USE D
  initializeOnly SFNode elbow USE ELBOW
  initializeOnly SFNode root USE ROOT
  initializeOnly SFNode h USE H
  url "javascript:
function initialize () {{ log = 'I ' + m .frameCount + ' ' + m .endFrame + ' '; }}
function on_frameCount (v) {{ log = log + 'C' + v + ' '; }}
function on_frameIndex (v) {{ log = log + 'F' + v + ' '; }}
function on_poseActive (v) {{ log = log + 'P' + v + ' '; }}
function on_touch (v) {{ log = log + 'T' + v + ' '; }}
function on_key (k)
{{
   log = log + 'K' + k + ' ' + elbow .rotation + ' / ' + root .translation + ' ';
   if (k == 'a') m .enabled = true;
   if (k == 'b') m .enabled = false;
   if (k == 'c') m .next = true;
   if (k == 'd') m .previous = true;
   if (k == 'e') m .frameIndex = 2;
   if (k == 'f') p .commencePose = true;
   if (k == 'g') p .resetAllJoints = true;
   if (k == 'h') p .set_fraction = 0.5;
   if (k == 'i') d .weight = 1;
   if (k == 'j') {{ m .startFrame = 1; m .endFrame = 2; }}
   if (k == 'l') elbow .name = 'IGNORED';
   if (k == 'm') {{ elbow .name = 'l_elbow'; h .motionsEnabled = new MFBool (false, true); }}
}}
"
}}
ROUTE K.keyPress TO R.on_key
ROUTE M.frameCount TO R.on_frameCount
ROUTE M.frameIndex TO R.on_frameIndex
ROUTE P.isActive TO R.on_poseActive
ROUTE TS.isActive TO R.on_touch
'''
open(out + ".wrl", "w").write(world)
ev = []
f = 10
def key(k, gap=8):
    global f
    ev.append({"frame": f, "type": "key", "key": k}); f += gap
key("a", 40)   # motion runs 1.5 s at dt 1/30 = 45 frames; 40 frames in
key("b")
key("c"); key("c"); key("d"); key("e")
key("j"); key("c"); key("c")
key("f", 40)   # pose transition 1 s = 30 frames
key("i")
key("g", 40)
key("h")
key("l"); key("c")
key("m"); key("a", 30); key("b")
# click on the skinned elbow region
ev.append({"frame": f, "type": "mousemove", "x": 230, "y": 200})
ev.append({"frame": f, "type": "mousedown", "button": 0, "x": 230, "y": 200})
ev.append({"frame": f + 2, "type": "mouseup", "button": 0, "x": 230, "y": 200})
f += 6
frames = f + 4
cap = sorted(set([9] + [e["frame"] + 3 for e in ev if e["type"] == "key"] + [30, 45, 120, 140, frames - 2]))
json.dump({"size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames, "capture": cap, "dump": cap, "events": ev}, open(out + ".json", "w"), indent=1)
print(frames, len(cap))
