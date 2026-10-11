#VRML V2.0 utf8
# Copyright (c) 1997 by blaxxun interactive Inc.
# The right to download, change and reuse this example is explicitly
# granted, provided proper credit is given.
#
# The arena that allows to display avatars from the avatar den
#
DEF _Background Background {
 groundAngle 1.5708
 groundColor [0.98 0.9 0.73,0.98 0.9 0.73]
 skyAngle 1.5708
 skyColor [0.46 0.38 0.88,0.8 0.7 0.94]
}
DEF _NavigationInfo NavigationInfo {
 headlight TRUE
 type "WALK"
 avatarSize [ 0.25 1.6 1 ]
}
DEF Background Group {
 children DEF _Shape Shape {
  appearance Appearance {
   material Material { 
    diffuseColor 0.98 0.9 0.73
    specularColor 0.2 0.2 0.2
    emissiveColor 0.9 0.8 0.6
   }
  }
  geometry DEF _IndexedFaceSet IndexedFaceSet {
   coord DEF BackgroundCoord Coordinate {
    point [ -32 0 -32,32 0 -32,32 0 32,-32 0 32 ]
   }
   ccw FALSE
   coordIndex [ 0,1,2,3, -1]
   normalPerVertex FALSE
  }
 }
}
DEF Viewpoint1 Viewpoint {
 fieldOfView 0.8
 orientation 0 1 0 0
 position 0 1.6 4
 description "Front"
}
DEF Viewpoint2 Viewpoint {
 fieldOfView 0.8
 orientation 0 1 0 1.57
 position 4 1.6 0
 description "Left"
}
DEF Viewpoint3 Viewpoint {
 fieldOfView 0.8
 orientation 0 1 0 3.14
 position 0 1.6 -4
 description "Back"
}
DEF Viewpoint4 Viewpoint {
 fieldOfView 0.8
 orientation 0 1 0 -1.57
 position -4 1.6 0
 description "Right"
}
DEF Viewpoint5 Viewpoint {
 fieldOfView 0.8
 orientation 0 1 0 0
 position 1 1.6 3.5
 description "Action buttons"
}
DEF Viewpoint6 Viewpoint {
 fieldOfView 0.8
 orientation 0 1 0 0
 position 0 0 6
 description "Below"
}
DEF Viewpoint7 Viewpoint {
 fieldOfView 0.8
 orientation 0 1 0 0
 position -2 1.6 1
 description "Stick"
}

# This avatar inlines its geometry from separate files.
# DO NOT use it with blaxxun Contact in multi-user worlds.
#
# The Protos needed according to the Human Animation Spec
# date 12.03.97. See http://ece.uwaterloo.ca:80/~h-anim/spec.html.
#
PROTO Joint [
 exposedField SFString name ""
 exposedField SFVec3f translation 0 0 0
 exposedField SFRotation rotation 0 0 1  0
 exposedField SFVec3f scale 1 1 1
 exposedField SFRotation scaleOrientation 0 0 1  0
 exposedField SFVec3f center 0 0 0
 exposedField MFNode children []
 exposedField MFFloat ulimit [ 0, 0, 0 ]
 exposedField MFFloat llimit [ 0, 0, 0 ]
]
{
 Transform {
  translation IS translation
  rotation IS rotation
  scale IS scale
  scaleOrientation IS scaleOrientation
  center IS center
  children IS children
 }
}
PROTO Segment [
 exposedField SFString name ""
 exposedField SFFloat mass 0
 exposedField SFVec3f centerOfMass 0 0 0
 exposedField SFVec3f bboxCenter 0 0 0
 exposedField SFVec3f bboxSize -1 -1 -1
 exposedField MFNode children []
 exposedField SFVec3f translation 0 0 0
 exposedField SFVec3f scale 1 1 1
 exposedField SFRotation rotation 0 0 1  0
]
{
 Transform {
  translation IS translation
  rotation IS rotation
  scale IS scale
  children IS children
 }
}
PROTO Humanoid [
 exposedField SFString version "1.0"
 exposedField MFNode joints []
 exposedField MFNode segments []
 field        SFString name ""
 field        MFString info []
 field        SFVec3f bboxCenter 0 0 0
 field        SFVec3f bboxSize -1 -1 -1
]
{
 WorldInfo {
  title IS name
  info  IS info
 }
}

Group {
 children [

  # The geometry of the avatars from the avatar den
  #

    # The joint model used for avatars of the avatar den.
    #
    # The joint model is conform to the "VRML Humanoid proposal" Version "1.0",
    # date 12.03.97. See http://ece.uwaterloo.ca:80/~h-anim/spec.html.
    #
    DEF hanim_HumanoidRoot Joint { name "HumanoidRoot"
     center 0 1.05 0
     children [
      # lower body 
      DEF hanim_sacroiliac Joint { name "sacroiliac"
       center 0 1 0
       children [
        # pelvis
        DEF hanim_pelvis Segment { name "pelvis"
         translation 0 1 0
         children [
          Inline { url "/vrml/avatarden/lbodies/bluecyl.wrl" }
         ]
        }
        # right leg
        DEF hanim_r_hip Joint { name "r_hip"
         center -0.125 0.9 0
         children [
          DEF hanim_r_thigh Segment { name "r_thigh"
           translation -0.125 0.7 0
           children [
            Inline { url "/vrml/avatarden/rulegs/bluecyl.wrl" }
           ]
          }
          DEF hanim_r_knee Joint { name "r_knee"
           center -0.125 0.5 0
           children [
            DEF hanim_r_calf Segment { name "r_calf"
             translation -0.125 0.3 0
             children [
              Inline { url "/vrml/avatarden/rllegs/bluecyl.wrl" }
             ]
            }
            DEF hanim_r_ankle Joint { name  "r_ankle"
             center -0.125 0.1 0
             children [
              DEF hanim_r_hindfoot Segment { name "r_hindfoot"
               translation -0.125 0.045 0.1
               children [
                Inline { url "/vrml/avatarden/rfeet/bluecyl.wrl" }
               ]
              }
             ]
            }
           ]
          }
         ]
        }
        # left leg
        DEF hanim_l_hip Joint { name "l_hip"
         center 0.125 0.9 0
         children [
          DEF hanim_l_thigh Segment { name "l_thigh"
           translation 0.125 0.7 0
           children [
            Inline { url "/vrml/avatarden/lulegs/bluecyl.wrl" }
           ]
          }
          DEF hanim_l_knee Joint { name "l_knee"
           center 0.125 0.5 0
           children [
            DEF hanim_l_calf Segment { name "l_calf"
             translation 0.125 0.3 0
             children [
              Inline { url "/vrml/avatarden/lllegs/bluecyl.wrl" }
             ]
            }
            DEF hanim_l_ankle Joint { name  "l_ankle"
             center 0.125 0.1 0
             children [
              DEF hanim_l_hindfoot Segment { name "l_hindfoot"
               translation 0.125 0.045 0.1
               children [
                Inline { url "/vrml/avatarden/lfeet/bluecyl.wrl" }
               ]
              }
             ]
            }
           ]
          }
         ]
        }
       ]
      }
      # upper body, arms and head
      DEF hanim_vl5 Joint { name "vl5"
       center 0 1.1 0
       children [
        DEF hanim_vc7 Joint { name "vc7"
         center 0 1.51 0
         children [
          # upper body
          DEF hanim_c7 Segment { name "c7"
           translation 0 1.35 0
           children [
            Inline { url "/vrml/avatarden/ubodies/bluecyl.wrl" }
           ]
          }
          # right arm
          DEF hanim_r_shoulder Joint { name "r_shoulder"
           center -0.25 1.56 0
           children [
            DEF hanim_r_upperarm Segment { name "r_upperarm" 
             translation -0.25 1.4 0
             children [
              Inline { url "/vrml/avatarden/ruarms/bluecyl.wrl" }
             ]
            }
            DEF hanim_r_elbow Joint { name "r_elbow"
             center -0.25 1.22 0
             children [
              DEF hanim_r_forearm Segment { name "r_forearm"
               translation -0.25 1.11 0
               children [
                Inline { url "/vrml/avatarden/rlarms/bluecyl.wrl" }
               ]
              }
              DEF hanim_r_wrist Joint { name "r_wrist"
               center -0.25 1 0
               children [
                DEF hanim_r_hand Segment { name "r_hand"
                 translation -0.25 0.93 0
                 children [
                  Inline { url "/vrml/avatarden/rhands/bluecyl.wrl" }
                 ]
                }
               ]
              }
             ]
            }
           ]
          }
          # left arm
          DEF hanim_l_shoulder Joint { name "l_shoulder"
           center 0.25 1.56 0
           children [
            DEF hanim_l_upperarm Segment { name "l_upperarm" 
             translation 0.25 1.4 0
             children [
              Inline { url "/vrml/avatarden/luarms/bluecyl.wrl" }
             ]
            }
            DEF hanim_l_elbow Joint { name "l_elbow"
             center 0.25 1.22 0
             children [
              DEF hanim_l_forearm Segment { name "l_forearm"
               translation 0.25 1.11 0
               children [
                Inline { url "/vrml/avatarden/llarms/bluecyl.wrl" }
               ]
              }
              DEF hanim_l_wrist Joint { name "l_wrist"
               center 0.25 1 0
               children [
                DEF hanim_l_hand Segment { name "l_hand"
                 translation 0.25 0.93 0
                 children [
                  Inline { url "/vrml/avatarden/lhands/bluecyl.wrl" }
                 ]
                }
               ]
              }
             ]
            }
           ]
          }
          # head
          DEF hanim_vc4 Joint { name "vc4"
           center 0 1.6 0
           children [
            # neck
            DEF hanim_c4 Segment { name "c4"
             translation 0 1.61 0
             children [
              #Inline { url "/vrml/avatarden/lhands/bluecyl.wrl" }
             ]
            }
            DEF hanim_skullbase Joint { name "skullbase"
             center 0 1.62 0
             children [
              DEF hanim_skull Segment { name "skull"
               translation 0 1.71 0
               children [
                Inline { url "/vrml/avatarden/heads/bluecyl.wrl" }
               ]
              }
             ]
            }
           ]
          }
         ]
        }
       ]
      }
     ]
    }
 ]
}

# The touch sensors to start the animations
DEF GestureButtons Transform {
 translation 1 0 1
 children [
  # The touch sensors to start the animations

  Transform {
   translation 0 2 0
   children [
    # a cube used as button
    DEF arButton Shape {
     appearance DEF blackA Appearance { material Material { diffuseColor 0 0 0 } }
     geometry Box { size 0.37 0.13 0.08 }
    }

    # the text
    Transform {
     translation -0.16 -0.02 0.05
     scale 0.07 0.07 1
     children [
      Shape {
       appearance DEF whiteT Appearance {
        material Material { diffuseColor 1 1 1 }
       }
       geometry Text { string [ "Walk" ] }
      }
     ]
    }

    # the sensor
    DEF TouchWalk TouchSensor { },

    # animation clock
    DEF TimeWalk TimeSensor {
     cycleInterval 1
     loop TRUE
     startTime 1
     stopTime 2
    }
   ]
  }

  Transform {
   translation 0 1.85 0
   children [
    USE arButton
    Transform {
     translation -0.16 -0.02 0.05
     scale 0.07 0.07 1
     children [
      Shape {
       appearance USE whiteT
       geometry Text { string [ "Stop" ] }
      }
     ]
    }
    DEF TouchStop TouchSensor { },
    DEF TimeStop TimeSensor { cycleInterval 0.1 }
   ]
  }

  Transform {
   translation 0 1.7 0
   children [
    USE arButton
    Transform {
     translation -0.16 -0.02 0.05
     scale 0.07 0.07 1
     children [
      Shape {
       appearance USE whiteT
       geometry Text { string [ "Hello" ] }
      }
     ]
    }
    DEF Touch_1 TouchSensor { },
    DEF Time_1 TimeSensor { cycleInterval 2.0 }
   ]
  }

  Transform {
   translation 0 1.55 0
   children [
    USE arButton
    Transform {
     translation -0.16 -0.02 0.05
     scale 0.07 0.07 1
     children [
      Shape {
       appearance USE whiteT
       geometry Text { string [ "Hey" ] }
      }
     ]
    }
    DEF Touch_2 TouchSensor { },
    DEF Time_2 TimeSensor { cycleInterval 2.0 }
   ]
  }

  Transform {
   translation 0 1.4 0
   children [
    USE arButton
    Transform {
     translation -0.16 -0.02 0.05
     scale 0.07 0.07 1
     children [
      Shape {
       appearance USE whiteT
       geometry Text { string [ "Yes" ] }
      }
     ]
    }
    DEF Touch_3 TouchSensor { },
    DEF Time_3 TimeSensor { cycleInterval 2.0 }
   ]
  }

  Transform {
   translation 0 1.25 0
   children [
    USE arButton
    Transform {
     translation -0.16 -0.02 0.05
     scale 0.07 0.07 1
     children [
      Shape {
       appearance USE whiteT
       geometry Text { string [ "Smile" ] }
      }
     ]
    }
    DEF Touch_4 TouchSensor { },
    DEF Time_4 TimeSensor { cycleInterval 2.0 }
   ]
  }

  Transform {
   translation 0 1.1 0
   children [
    USE arButton
    Transform {
     translation -0.16 -0.02 0.05
     scale 0.07 0.07 1
     children [
      Shape {
       appearance USE whiteT
       geometry Text { string [ "Frown" ] }
      }
     ]
    }
    DEF Touch_5 TouchSensor { },
    DEF Time_5 TimeSensor { cycleInterval 2.0 }
   ]
  }

  Transform {
   translation 0 0.95 0
   children [
    USE arButton
    Transform {
     translation -0.16 -0.02 0.05
     scale 0.07 0.07 1
     children [
      Shape {
       appearance USE whiteT
       geometry Text { string [ "No" ] }
      }
     ]
    }
    DEF Touch_6 TouchSensor { },
    DEF Time_6 TimeSensor { cycleInterval 2.0 }
   ]
  }

  Transform {
   translation 0 0.8 0
   children [
    USE arButton
    Transform {
     translation -0.16 -0.02 0.05
     scale 0.07 0.07 1
     children [
      Shape {
       appearance USE whiteT
       geometry Text { string [ "Not!" ] }
      }
     ]
    }
    DEF Touch_7 TouchSensor { },
    DEF Time_7 TimeSensor { cycleInterval 2.0 }
   ]
  }

  Transform {
   translation 0 0.65 0
   children [
    USE arButton
    Transform {
     translation -0.16 -0.02 0.05
     scale 0.07 0.07 1
     children [
      Shape {
       appearance USE whiteT
       geometry Text { string [ "Bye" ] }
      }
     ]
    }
    DEF Touch_8 TouchSensor { },
    DEF Time_8 TimeSensor { cycleInterval 2.0 }
   ]
  }
 ]
}

# a square of boxes around the whole thing
DEF squareWall Transform {
 children [
  Transform {
   translation -0.4 0 -10.4
   children [
    DEF Wall Transform {
     children [
      Transform {
       translation -9.36 0.25 0
       children [
        # a black border cube
        DEF blackBox Shape {
         appearance USE blackA
         geometry Box { size 2.08 0.5 0.8 }
        }
       ]
      }
      Transform {
       translation -7.28 0.25 0
       children [
        # a white border cube 
        DEF whiteBox Shape {
         appearance DEF whiteA Appearance {
          material Material {
           diffuseColor 1 1 1
           emissiveColor 0.5 0.5 0.5
          }
         }
         geometry Box { size 2.08 0.5 0.8 }
        }
       ]
      }
      Transform { translation -5.2 0.25 0 children [ USE blackBox ] } 
      Transform { translation -3.12 0.25 0 children [ USE whiteBox ] }
      Transform { translation -1.04 0.25 0 children [ USE blackBox ] }
      Transform { translation 1.04 0.25 0 children [ USE whiteBox ] }
      Transform { translation 3.12 0.25 0 children [ USE blackBox ] }
      Transform { translation 5.2 0.25 0 children [ USE whiteBox ] }
      Transform { translation 7.28 0.25 0 children [ USE blackBox ] }
      Transform { translation 9.36 0.25 0 children [ USE whiteBox ] }
     ]
    }
   ]
  }
  
  Transform {
   translation -10.4 0 0.4
   rotation 0 1 0 1.57
   children [ USE Wall ]
  }

  Transform {
   translation 0.4 0 10.4
   rotation 0 1 0 3.14
   children [ USE Wall ]
  }

  Transform {
   translation 10.4 0 -0.4
   rotation 0 1 0 4.71
   children [ USE Wall ]
  }
 ]
}


# a meter stick to show the size
DEF meterStick Transform {
 translation -2 0 -2
 children [
  Transform {
   children [
    DEF Stick Group {
     children [
      Transform {
       translation 0 0.25 0
       children [
        # a red stick cube
        DEF redstick Shape {
         appearance DEF redA Appearance {
          material Material {
           diffuseColor 1 0 0
           emissiveColor 1 0.5 0.5
          }
         }
         geometry Box { size 0.05 0.5 0.05 }
        }
       ]
      }
      Transform {
       translation 0 0.75 0
       children [
        # a white stick cube 
        DEF whiteStick Shape {
         appearance USE whiteA
         geometry Box { size 0.05 0.5 0.05 }
        }
       ]
      }
     ]
    }
   ]
  }
  Transform { translation 0 1 0 children [ USE Stick ] }
  Transform { translation 0 2 0 children [ USE Stick ] }
  # text for the stick
  Transform {
   translation -0.6 1 0
   scale 0.1 0.1 0.1
   children [
    DEF Panel Transform {
     translation 2.4 0.5 -0.3
     children [
      Shape {
       appearance USE redA
       geometry Box { size 6 1 0.5 }
      }
     ]
    }
    Shape {
     appearance USE blackA
     geometry Text { string [ "1 meter" ] }
    }
   ]
  }
  Transform {
   translation -0.6 2 0
   scale 0.1 0.1 0.1
   children [
    USE Panel
    Shape {
     appearance USE blackA
     geometry Text { string [ "2 meters" ] }
    }
   ]
  }
  Transform {
   translation -0.6 3 0
   scale 0.1 0.1 0.1
   children [
    USE Panel
    Shape {
     appearance USE blackA
     geometry Text { string [ "3 meters" ] }
    }
   ]
  }
 ]
}

# Touch sensors for the Arena
ROUTE TouchStop.touchTime TO TimeWalk.set_stopTime
ROUTE TouchStop.touchTime TO TimeStop.set_startTime
ROUTE TouchWalk.touchTime TO TimeWalk.set_startTime
ROUTE Touch_1.touchTime  TO Time_1.set_startTime
ROUTE Touch_2.touchTime  TO Time_2.set_startTime
ROUTE Touch_3.touchTime  TO Time_3.set_startTime
ROUTE Touch_4.touchTime  TO Time_4.set_startTime
ROUTE Touch_5.touchTime  TO Time_5.set_startTime
ROUTE Touch_6.touchTime  TO Time_6.set_startTime
ROUTE Touch_7.touchTime  TO Time_7.set_startTime
ROUTE Touch_8.touchTime  TO Time_8.set_startTime

# Copyright (c) 1997 by blaxxun interactive Inc.
# The right to download, change and reuse this example is explicitly
# granted, provided proper credit is given.
#
# Interpolators for the avatars of the avatar den
#
DEF Animations Group {
 children [
  DEF WalkAnimation Group {
   children [
    DEF r_shoulderRotWalk OrientationInterpolator {
     key [ 0, 0.25, 0.5, 0.75, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 -0.4,
      1 0 0 0,
      1 0 0 0.4,
      1 0 0 0
     ]
    }
     
    DEF r_hipRotWalk OrientationInterpolator {
     key [ 0, 0.25, 0.5, 0.75, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 0.5,
      1 0 0 0,
      1 0 0 -0.5,
      1 0 0 0
     ]
    }
    
    DEF r_kneeRotWalk OrientationInterpolator {
     key [ 0, 0.15, 0.25, 0.5, 0.75, 0.85, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 0,
      1 0 0 0.6,
      1 0 0 1,
      1 0 0 0,
      1 0 0 0.2,
      1 0 0 0
     ]
    }
    
    DEF r_elbowRotWalk OrientationInterpolator {
     key [ 0, 0.25, 0.5, 1 ]
     keyValue [
      1 0 0 -0.4,
      1 0 0 -1,
      1 0 0 0,
      1 0 0 0
     ]
    }
    
    DEF l_shoulderRotWalk OrientationInterpolator {
     key [ 0, 0.25, 0.5, 0.75, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 0.4,
      1 0 0 0,
      1 0 0 -0.4,
      1 0 0 0
     ]
    }
    
    DEF l_hipRotWalk OrientationInterpolator {
     key [ 0, 0.25, 0.5, 0.75, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 -0.5,
      1 0 0 0,
      1 0 0 0.5,
      1 0 0 0
     ]
    }
    
    DEF l_kneeRotWalk OrientationInterpolator {
     key [ 0, 0.25, 0.35, 0.5, 0.65, 0.75, 1 ]
     keyValue [
      1 0 0 1,
      1 0 0 0,
      1 0 0 0.2,
      1 0 0 0,
      1 0 0 0,
      1 0 0 0.6,
      1 0 0 1
     ]
    }
    
    DEF l_elbowRotWalk OrientationInterpolator {
     key [ 0, 0.25, 0.5, 0.75, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 0,
      1 0 0 -0.4,
      1 0 0 -1,
      1 0 0 0
     ]
    }
    
    DEF vl5RotWalk OrientationInterpolator {
     key [ 0, 0.25, 0.5, 0.75, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 0.1,
      1 0 0 0,
      1 0 0 0.1,
      1 0 0 0
     ]
    }
    
    DEF HumanoidRootTransWalk PositionInterpolator {
     key [ 0, 0.25, 0.5, 0.75, 1 ]
     keyValue [ 0 0 0, 0 -0.05 0, 0 0 0, 0 -0.05 0, 0 0 0 ]
    }
   ]
  }
    
  DEF StopAnimation Group {
   children [
    DEF jointTransStop PositionInterpolator {
     key [ 0, 1 ]
     keyValue [ 0 0 0, 0 0 0 ]
    }
    
    DEF jointRotStop OrientationInterpolator {
     key [ 0, 1 ]
     keyValue [ 1 0 0 0, 1 0 0 0 ]
    }
   ]
  }
 
  DEF NoAnimation Group {
   children [
 
    DEF r_shoulderRotNo OrientationInterpolator {
     key [ 0, 0.05, 0.95, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 -0.7,
      1 0 0 -0.7,
      1 0 0 0
     ]
    }
    
    DEF r_elbowRotNo OrientationInterpolator {
     key [ 0, 0.05, 0.95, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 -0.8,
      1 0 0 -0.8
      1 0 0 0
     ]
    }
    
    DEF r_wristRotNo OrientationInterpolator {
     key [ 0, 0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8, 0.9, 1 ]
     keyValue [
      0 0 1 0,
      1 0 1 -0.8,
      1 0 -1 -0.8,
      1 0 1 -0.8,
      1 0 -1 -0.8,
      1 0 1 -0.8,
      1 0 -1 -0.8,
      1 0 1 -0.8,
      1 0 -1 -0.8,
      1 0 1 -0.8,
      0 0 1 0
     ]
    }
    
    DEF skullbaseRotNo OrientationInterpolator {
     key [ 0, 0.125, 0.25, 0.375, 0.50, 0.625, 0.75, 0.875, 1 ]
     keyValue [
      0 1 0 0,
      0 1 0 0.3,
      0 1 0 0,
      0 1 0 -0.3,
      0 1 0 0,
      0 1 0 0.3,
      0 1 0 0,
      0 1 0 -0.3,
      0 1 0 0
     ]
    }
   ]
  }

  DEF HiAnimation Group {
   children [
 
    DEF vl5RotHi OrientationInterpolator {
     key [ 0, 0.25, 0.75, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 -0.2,
      1 0 0 -0.2,
      1 0 0 0
     ]
    }
    
    DEF l_shoulderRotHi OrientationInterpolator {
     key [ 0, 0.2, 0.4, 0.6, 0.8, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 -0.5,
      1 0 0 -1,
      1 0 0 -1,
      1 0 0 -0.5,
      1 0 0 0
     ]
    }
    
    DEF l_elbowRotHi OrientationInterpolator {
     key [ 0, 0.2, 0.4, 0.6, 0.8, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 -0.8,
      1 0 0 -1.8
      1 0 0 -1.8
      1 0 0 -0.8
      1 0 0 0
     ]
    }
    
    DEF l_wristRotHi OrientationInterpolator {
     key [ 0, 0.4, 0.45, 0.5, 0.55, 0.6, 1 ]
     keyValue [
      0 0 1 0,
      0 0 1 0,
      0 0 1 0.8,
      0 0 1 0,
      0 0 1 -0.8,
      0 0 1 0,
      0 0 1 0
     ]
    }
   ]
  }

  DEF HeyAnimation Group {
   children [
    
    DEF HumanoidRootTransHey PositionInterpolator {
     key [ 0, 0.25, 0.4 0.5, 0.6, 0.72, 1 ]
     keyValue [ 0 0 0, 0 -0.26 0, 0 0.3 0, 0 0.5 0, 0 0.3 0, 0 -0.27 0, 0 0 0 ]
    }

    DEF HumanoidRootRotHey OrientationInterpolator {
     key [ 0, 0.38, 0.47, 0.57, 0.66, 0.75, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 0,
      1 0 0 -1.57
      1 0 0 -3.14,
      1 0 0 -4.71,
      1 0 0 0,
      1 0 0 0
     ]
    }
    
    DEF skullbaseRotHey OrientationInterpolator {
     key [ 0, 0.25, 0.5, 0.75, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 0.3,
      1 0 0 0,
      1 0 0 0.3,
      1 0 0 0
     ]
    }

    DEF shoulderRotHey OrientationInterpolator {
     key [ 0, 0.12, 0.25, 0.375, 0.7, 0.8, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 1,
      1 0 0 0,
      1 0 0 -0.8,
      1 0 0 0,
      1 0 0 -1.2,
      1 0 0 0
     ]
    }

    DEF elbowRotHey OrientationInterpolator {
     key [ 0, 0.44, 0.5, 0.75, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 0,
      1 0 0 -1.6,
      1 0 0 -1.2,
      1 0 0 0
     ]
    }
    
    DEF hipRotHey OrientationInterpolator {
     key [ 0, 0.25, 0.34, 0.37, 0.5, 0.65, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 -1.3,
      1 0 0 0,
      1 0 0 0,
      1 0 0 -2.5,
      1 0 0 -1.5,
      1 0 0 0
     ]
    }
    
    DEF kneeRotHey OrientationInterpolator {
     key [ 0, 0.25, 0.34, 0.37, 0.5, 0.81, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 2,
      1 0 0 0,
      1 0 0 0,
      1 0 0 2.5,
      1 0 0 1.6,
      1 0 0 0
     ]
    }

    DEF vl5RotHey OrientationInterpolator {
     key [ 0, 0.12, 0.5, 0.64, 0.78, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 1.4,
      1 0 0 0,
      1 0 0 1.5,
      1 0 0 1.6,
      1 0 0 0
     ]
    }

   ]
  }

  DEF YesAnimation Group {
   children [
    
    DEF l_elbowRotYes OrientationInterpolator {
     key [ 0, 0.1, 0.9, 1 ]
     keyValue [
      1 0 1 0,
      1 0 1 -2.5,
      1 0 1 -2.5,
      1 0 1 0
     ]
    }
    
    DEF l_shoulderRotYes OrientationInterpolator {
     key [ 0, 0.1, 0.9, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 -0.4,
      1 0 0 -0.4,
      1 0 0 0
     ]
    }
    
    DEF vc4RotYes OrientationInterpolator {
     key [ 0, 0.1, 0.9, 1 ]
     keyValue [
      0 0 1 0,
      0 0 1 -0.3,
      0 0 1 -0.3,
      0 0 1 0
     ]
    }
   ]
  }

  DEF SmileAnimation Group {
   children [
    
    #
    # This is for smile 
    #
    # tilt head
    #
    # and wave as in hi
    
   ]
  }

  DEF FrownAnimation Group {
   children [

    #
    # This is for disapprove
    #
    # shake the head
    #

    # tilt head
    #

    DEF l_shoulderRotFrown OrientationInterpolator {
     key [ 0, 0.1, 0.2, 0.8, 0.9, 1 ]
     keyValue [
      0 0 1 0,
      0 0 1 0.3,
      0 0 1 0.6,
      0 0 1 0.6,
      0 0 1 0.3,
      0 0 1 0
     ]
    }
    
    DEF l_elbowRotFrown OrientationInterpolator {
     key [ 0, 0.1, 0.2, 0.8, 0.9, 1 ]
     keyValue [
      0 0 1 0,
      0 0 1 -0.6,
      0 0 1 -1.2,
      0 0 1 -1.2,
      0 0 1 -0.6,
      0 0 1 0
     ]
    }
    
    DEF r_shoulderRotFrown OrientationInterpolator {
     key [ 0, 0.1, 0.2, 0.8, 0.9, 1 ]
     keyValue [
      0 0 1 0,
      0 0 1 -0.3,
      0 0 1 -0.6,
      0 0 1 -0.6,
      0 0 1 -0.3,
      0 0 1 0
     ]
    }
    
    DEF r_elbowRotFrown OrientationInterpolator {
     key [ 0, 0.1, 0.2, 0.8, 0.9, 1 ]
     keyValue [
      0 0 1 0,
      0 0 1 0.6,
      0 0 1 1.2,
      0 0 1 1.2,
      0 0 1 0.6,
      0 0 1 0
     ]
    }
   ]
  }
    
  DEF NotAnimation Group {
   children [

    #
    # This is for Not! Rejects
    #
    #
    # shake the head
    #
    # tilt head
    #
    # wave right hand
    #
    # lift left elbow
    #
    
   ]
  }
    
  DEF ByeAnimation Group {
   children [
    #
    # This is for waves goodbye, we just bow for now
    #
    
    DEF vl5RotBye OrientationInterpolator {
     key [ 0, 0.5, 1 ]
     keyValue [
      1 0 0 0,
      1 0 0 1.7,
      1 0 0 0
     ]
    }
   ]
  }
 ]
}
DEF Humanoid Humanoid {
 version "1.0"
 name "humanoid, from the blaxxun Avatar Den"
 info [
  "authorName=Peter Graf",
  "authorEmail=support@blaxxun.com",
  "copyright=1997, blaxxun interactive Inc.",
  "creationDate=December 3, 1997",
  "usageRestrictions=Usage is allowed, if proper credit is given."
 ]
 joints [
  USE hanim_HumanoidRoot,
  USE hanim_sacroiliac,
  USE hanim_r_hip,
  USE hanim_r_knee,
  USE hanim_r_ankle,
  USE hanim_l_hip,
  USE hanim_l_knee,
  USE hanim_l_ankle,
  USE hanim_vl5,
  USE hanim_vc7,
  USE hanim_r_shoulder,
  USE hanim_r_elbow,
  USE hanim_r_wrist,
  USE hanim_l_shoulder,
  USE hanim_l_elbow,
  USE hanim_l_wrist,
  USE hanim_vc4,
  USE hanim_skullbase
 ]
 segments [
  USE hanim_pelvis,
  USE hanim_l_thigh,
  USE hanim_l_calf,
  USE hanim_l_hindfoot,
  USE hanim_r_thigh,
  USE hanim_r_calf,
  USE hanim_r_hindfoot,
  USE hanim_c7,
  USE hanim_l_upperarm,
  USE hanim_l_forearm,
  USE hanim_l_hand,
  USE hanim_r_upperarm,
  USE hanim_r_forearm,
  USE hanim_r_hand,
  USE hanim_c4,
  USE hanim_skull
 ]
}

# Walk
ROUTE TimeWalk.fraction_changed TO HumanoidRootTransWalk.set_fraction
ROUTE TimeWalk.fraction_changed TO vl5RotWalk.set_fraction
ROUTE TimeWalk.fraction_changed TO r_shoulderRotWalk.set_fraction
ROUTE TimeWalk.fraction_changed TO r_elbowRotWalk.set_fraction
ROUTE TimeWalk.fraction_changed TO r_hipRotWalk.set_fraction
ROUTE TimeWalk.fraction_changed TO r_kneeRotWalk.set_fraction
ROUTE TimeWalk.fraction_changed TO l_shoulderRotWalk.set_fraction
ROUTE TimeWalk.fraction_changed TO l_elbowRotWalk.set_fraction
ROUTE TimeWalk.fraction_changed TO l_hipRotWalk.set_fraction
ROUTE TimeWalk.fraction_changed TO l_kneeRotWalk.set_fraction

ROUTE HumanoidRootTransWalk.value_changed TO hanim_HumanoidRoot.set_translation
ROUTE vl5RotWalk.value_changed TO hanim_vl5.set_rotation
ROUTE r_shoulderRotWalk.value_changed TO hanim_r_shoulder.set_rotation
ROUTE r_elbowRotWalk.value_changed TO hanim_r_elbow.set_rotation
ROUTE r_hipRotWalk.value_changed TO hanim_r_hip.set_rotation
ROUTE r_kneeRotWalk.value_changed TO hanim_r_knee.set_rotation
ROUTE l_shoulderRotWalk.value_changed TO hanim_l_shoulder.set_rotation
ROUTE l_elbowRotWalk.value_changed TO hanim_l_elbow.set_rotation
ROUTE l_hipRotWalk.value_changed TO hanim_l_hip.set_rotation
ROUTE l_kneeRotWalk.value_changed TO hanim_l_knee.set_rotation

# Stop
ROUTE TimeStop.fraction_changed TO jointTransStop.set_fraction
ROUTE TimeStop.fraction_changed TO jointRotStop.set_fraction

ROUTE jointTransStop.value_changed TO hanim_HumanoidRoot.set_translation
ROUTE jointRotStop.value_changed TO hanim_vl5.set_rotation
ROUTE jointRotStop.value_changed TO hanim_r_shoulder.set_rotation
ROUTE jointRotStop.value_changed TO hanim_r_elbow.set_rotation
ROUTE jointRotStop.value_changed TO hanim_r_hip.set_rotation
ROUTE jointRotStop.value_changed TO hanim_r_knee.set_rotation
ROUTE jointRotStop.value_changed TO hanim_l_shoulder.set_rotation
ROUTE jointRotStop.value_changed TO hanim_l_elbow.set_rotation
ROUTE jointRotStop.value_changed TO hanim_l_hip.set_rotation
ROUTE jointRotStop.value_changed TO hanim_l_knee.set_rotation

# Hello
ROUTE Time_1.fraction_changed TO vl5RotHi.set_fraction
ROUTE Time_1.fraction_changed TO l_shoulderRotHi.set_fraction
ROUTE Time_1.fraction_changed TO l_elbowRotHi.set_fraction
ROUTE Time_1.fraction_changed TO l_wristRotHi.set_fraction

ROUTE vl5RotHi.value_changed TO hanim_vl5.set_rotation
ROUTE l_shoulderRotHi.value_changed TO hanim_l_shoulder.set_rotation
ROUTE l_elbowRotHi.value_changed TO hanim_l_elbow.set_rotation
ROUTE l_wristRotHi.value_changed TO hanim_l_wrist.set_rotation

# Hey
ROUTE Time_2.fraction_changed TO HumanoidRootTransHey.set_fraction
ROUTE Time_2.fraction_changed TO skullbaseRotHey.set_fraction
ROUTE Time_2.fraction_changed TO shoulderRotHey.set_fraction
ROUTE Time_2.fraction_changed TO elbowRotHey.set_fraction
ROUTE Time_2.fraction_changed TO HumanoidRootRotHey.set_fraction
ROUTE Time_2.fraction_changed TO hipRotHey.set_fraction
ROUTE Time_2.fraction_changed TO kneeRotHey.set_fraction
ROUTE Time_2.fraction_changed TO vl5RotHey.set_fraction

ROUTE HumanoidRootTransHey.value_changed TO hanim_HumanoidRoot.set_translation
ROUTE skullbaseRotHey.value_changed TO hanim_skullbase.set_rotation
ROUTE shoulderRotHey.value_changed TO hanim_l_shoulder.set_rotation
ROUTE shoulderRotHey.value_changed TO hanim_r_shoulder.set_rotation
ROUTE elbowRotHey.value_changed TO hanim_l_elbow.set_rotation
ROUTE elbowRotHey.value_changed TO hanim_r_elbow.set_rotation
ROUTE HumanoidRootRotHey.value_changed TO hanim_HumanoidRoot.set_rotation
ROUTE hipRotHey.value_changed TO hanim_l_hip.set_rotation
ROUTE hipRotHey.value_changed TO hanim_r_hip.set_rotation
ROUTE kneeRotHey.value_changed TO hanim_l_knee.set_rotation
ROUTE kneeRotHey.value_changed TO hanim_r_knee.set_rotation
ROUTE vl5RotHey.value_changed TO hanim_vl5.set_rotation

# Yes
ROUTE Time_3.fraction_changed TO skullbaseRotHey.set_fraction
ROUTE Time_3.fraction_changed TO vc4RotYes.set_fraction
ROUTE Time_3.fraction_changed TO l_shoulderRotYes.set_fraction
ROUTE Time_3.fraction_changed TO l_elbowRotYes.set_fraction
ROUTE vc4RotYes.value_changed TO hanim_vc4.set_rotation
ROUTE l_elbowRotYes.value_changed TO hanim_l_elbow.set_rotation
ROUTE l_shoulderRotYes.value_changed TO hanim_l_shoulder.set_rotation

# Smile
ROUTE Time_4.fraction_changed TO vc4RotYes.set_fraction
ROUTE Time_4.fraction_changed TO vl5RotHi.set_fraction
ROUTE Time_4.fraction_changed TO l_shoulderRotHi.set_fraction
ROUTE Time_4.fraction_changed TO l_elbowRotHi.set_fraction
ROUTE Time_4.fraction_changed TO l_wristRotHi.set_fraction

# Frown
ROUTE Time_5.fraction_changed TO skullbaseRotNo.set_fraction
ROUTE Time_5.fraction_changed TO vc4RotYes.set_fraction
ROUTE Time_5.fraction_changed TO l_shoulderRotFrown.set_fraction
ROUTE Time_5.fraction_changed TO l_elbowRotFrown.set_fraction
ROUTE Time_5.fraction_changed TO r_shoulderRotFrown.set_fraction
ROUTE Time_5.fraction_changed TO r_elbowRotFrown.set_fraction
ROUTE l_shoulderRotFrown.value_changed TO hanim_l_shoulder.set_rotation
ROUTE l_elbowRotFrown.value_changed TO hanim_l_elbow.set_rotation
ROUTE r_shoulderRotFrown.value_changed TO hanim_r_shoulder.set_rotation
ROUTE r_elbowRotFrown.value_changed TO hanim_r_elbow.set_rotation

# No
ROUTE Time_6.fraction_changed TO skullbaseRotNo.set_fraction
ROUTE Time_6.fraction_changed TO r_shoulderRotNo.set_fraction
ROUTE Time_6.fraction_changed TO r_elbowRotNo.set_fraction
ROUTE Time_6.fraction_changed TO r_wristRotNo.set_fraction

ROUTE skullbaseRotNo.value_changed TO hanim_skullbase.set_rotation
ROUTE r_shoulderRotNo.value_changed TO hanim_r_shoulder.set_rotation
ROUTE r_elbowRotNo.value_changed TO hanim_r_elbow.set_rotation
ROUTE r_wristRotNo.value_changed TO hanim_r_wrist.set_rotation

# Not
ROUTE Time_7.fraction_changed TO skullbaseRotNo.set_fraction
ROUTE Time_7.fraction_changed TO vc4RotYes.set_fraction
ROUTE Time_7.fraction_changed TO r_shoulderRotNo.set_fraction
ROUTE Time_7.fraction_changed TO r_elbowRotNo.set_fraction
ROUTE Time_7.fraction_changed TO r_wristRotNo.set_fraction
ROUTE Time_7.fraction_changed TO l_shoulderRotFrown.set_fraction
ROUTE Time_7.fraction_changed TO l_elbowRotFrown.set_fraction

# Bye
ROUTE Time_8.fraction_changed TO vl5RotBye.set_fraction
ROUTE vl5RotBye.value_changed TO hanim_vl5.set_rotation


