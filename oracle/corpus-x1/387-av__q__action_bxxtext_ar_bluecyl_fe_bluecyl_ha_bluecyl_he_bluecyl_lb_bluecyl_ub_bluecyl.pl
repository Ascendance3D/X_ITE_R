#VRML V2.0 utf8
# Copyright (c) 1997 by blaxxun interactive Inc.
# The right to download, change and reuse this example is explicitly
# granted, provided proper credit is given.
#

# The proto that allows to use avatars from the Avatar Den in blaxxun
# communities with blaxxun Contact. The walk animation and gestures are enabled.
#
PROTO Avatar [
 exposedField SFTime gesture1 0
 exposedField SFTime gesture2 0
 exposedField SFTime gesture3 0
 exposedField SFTime gesture4 0
 exposedField SFTime gesture5 0
 exposedField SFTime gesture6 0
 exposedField SFTime gesture7 0
 exposedField SFTime gesture8 0
 exposedField SFBool isAvatar TRUE
 exposedField SFRotation rotation 0 1 0 0
 exposedField SFInt32 whichChoice 0
 exposedField SFBool isOver FALSE
 exposedField SFTime touchTime 0
 eventIn SFVec3f set_position
]
{
Switch {
whichChoice IS whichChoice
choice [
DEF WalkingAvatar Transform {
 rotation IS rotation
 children [

  # The real avatar starts here
  #
  Transform {
   translation 0 -1.6 0
   children [

    # The joint model of the avatar goes here
    Transform {
      ...
    }


   ]
  }

  # time sensors for the gestures
  #
  DEF TimeMove TimeSensor { enabled FALSE loop FALSE cycleInterval 1.4 }
  DEF TimeStop TimeSensor { cycleInterval 1 startTime 0 stopTime 0 }
  DEF TimeWalk TimeSensor { cycleInterval 0.7 startTime 0 }
  DEF Time_1 TimeSensor { cycleInterval 4 startTime IS gesture1 }
  DEF Time_2 TimeSensor { cycleInterval 2 startTime IS gesture2 }
  DEF Time_3 TimeSensor { cycleInterval 4 startTime IS gesture3 }
  DEF Time_4 TimeSensor { cycleInterval 4 startTime IS gesture4 }
  DEF Time_5 TimeSensor { cycleInterval 4 startTime IS gesture5 }
  DEF Time_6 TimeSensor { cycleInterval 4 startTime IS gesture6 }
  DEF Time_7 TimeSensor { cycleInterval 4 startTime IS gesture7 }
  DEF Time_8 TimeSensor { cycleInterval 4 startTime IS gesture8 }

  DEF AvatarInterpolator PositionInterpolator {
   key [ 0 1 ]
   keyValue [0 0 0, 0 0 0]
  }
  DEF AvatarScript Script {

   eventIn SFVec3f set_position IS set_position
   eventIn SFFloat set_fraction

   field SFNode  TimeMove USE TimeMove
   field MFVec3f keyValues[]
   field SFFloat curFraction 0
   field SFFloat posFraction 0
   field SFBool  TimeMoveEnabled FALSE
   field SFNode  AvatarTransform USE WalkingAvatar
   directOutput TRUE

   eventOut SFTime  startTime
   eventOut SFTime  stopTime
   eventOut MFVec3f values_changed
   eventOut SFFloat fraction_changed

   url "vrmlscript:
    function initialize() {
     TimeMoveEnabled = TimeMove.set_enabled = FALSE;
    }
    function set_fraction (value, timestamp) {
     if( TimeMoveEnabled )
     {
      curFraction = value;
      if( value > posFraction )
      {
       fraction_changed = value - posFraction;
      }
      else
      {
       fraction_changed = value + ( 1 - posFraction );
      }
     }
    }
    function set_position (value, timestamp) {
     if(( AvatarTransform.translation[0] == value[0] )
      &&( AvatarTransform.translation[1] == value[1] )
      &&( AvatarTransform.translation[2] == value[2] ))
     {
      return;
     }

     // Browser.print( 'fraction = ' + fraction_changed );
     posFraction = curFraction;

     TimeMoveEnabled = TimeMove.set_enabled = FALSE;
     stopTime = timestamp;
     keyValues[0] = AvatarTransform.translation_changed;
     keyValues[1] = value;
     values_changed = keyValues;
     startTime = timestamp;
     stopTime = timestamp + 1.4;
     TimeMoveEnabled = TimeMove.set_enabled = TRUE;
    }
   "
  }
 ]
 }
 ]
}


ROUTE TimeMove.fraction_changed TO AvatarScript.set_fraction
ROUTE AvatarScript.fraction_changed TO AvatarInterpolator.set_fraction

ROUTE AvatarScript.startTime TO TimeMove.set_startTime
ROUTE AvatarScript.startTime TO TimeWalk.set_startTime
ROUTE AvatarScript.stopTime  TO TimeStop.set_startTime
ROUTE AvatarScript.values_changed TO AvatarInterpolator.set_keyValue

ROUTE AvatarInterpolator.value_changed TO WalkingAvatar.set_translation

}
Avatar { }

