#!/usr/bin/env python3
"""Audio repair lane (XITE_R_AUDIO_ORACLE_REPAIR_01): VRML97 Sound / AudioClip oracle worlds.

  python3 -I oracle/tools/gen_rea_audio_repair.py

Writes oracle/synthetic-rea-b05/{clip-*,sound-*,buffer-fa*-decode}.wrl and scripts/NAME.json. The baselines are made
only from the pinned X_ITE (`python3 -I oracle/tools/rea_b05.py baseline NAME`) and checked with
`rea_b05.py check NAME` or the cargo test `vrml_audio_oracle`.

Every world puts the viewer at the origin, looking down -Z. A Script starts the clips at the 12th
TimeSensor tick (the rea_b05 pattern), so a start never depends on the load time. Media are in
oracle/synthetic-rea-b05/media: tone48k-stereo.wav (0.25 s), tone22k-mono.wav, and bad.mid /
bad2.mid (small Standard MIDI Files that no decodeAudioData accepts).
"""
import json, os, re

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out = os.path.join(root, "oracle", "synthetic-rea-b05")

VRML = "#VRML V2.0 utf8\n"
X3D = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT Sound:2\n"
VIEW = "Viewpoint { position 0 0 0 }\n"
WAV = '"media/tone48k-stereo.wav"'
BIG = "minFront 20 minBack 20 maxFront 40 maxBack 40"


def start(*targets, tick=12):
    routes = "".join(f"ROUTE S.start TO {t}.startTime\n" for t in targets)
    return f"""DEF T TimeSensor {{ loop TRUE cycleInterval 0.5 }}
DEF S Script {{
  eventIn SFTime set_time
  eventOut SFTime start
  field SFInt32 n 0
  url "javascript:
function set_time (value) {{ if (++ n === {tick}) start = value; }}
"
}}
ROUTE T.time TO S.set_time
{routes}"""


def watch(node, elapsed=False, paused=False):
    """Prints of the AudioClip's output events, with their time stamps."""
    fields = [("isActive", "SFBool", "a", "active"), ("duration_changed", "SFTime", "d", "duration")]
    if elapsed:
        fields.append(("elapsedTime", "SFTime", "e", "elapsed"))
    if paused:
        fields.append(("isPaused", "SFBool", "p", "paused"))
    decl = " ".join(f"eventIn {t} {f}" for _, t, f, _ in fields)
    fns = "".join(f"function {f} (v, t) {{ print ('{node} {label} ' + v + ' ' + t); }}\n" for _, _, f, label in fields)
    routes = "".join(f"ROUTE {node}.{name} TO P{node}.{f}\n" for name, _, f, _ in fields)
    return f'DEF P{node} Script {{ {decl} url "javascript:\n{fns}" }}\n{routes}'


def load_sensor(node):
    """X3D only: a LoadSensor over the clip, every output printed."""
    fields = [("isActive", "SFBool", "a"), ("isLoaded", "SFBool", "l"), ("progress", "SFFloat", "p"), ("loadTime", "SFTime", "t")]
    decl = " ".join(f"inputOnly {t} {f}" for _, t, f in fields)
    fns = "".join(f"function {f} (v, t) {{ print ('L {name} ' + v + ' ' + t); }}\n" for name, _, f in fields)
    routes = "".join(f"ROUTE L.{name} TO PL.{f}\n" for name, _, f in fields)
    return f'DEF L LoadSensor {{ children USE {node} }}\nDEF PL Script {{ {decl} url "ecmascript:\n{fns}" }}\n{routes}'


def x3d_start(*targets, tick=12):
    return start(*targets, tick=tick).replace("eventIn", "inputOnly").replace("eventOut", "outputOnly").replace("field SFInt32", "initializeOnly SFInt32")


def x3d_watch(node, **kw):
    return watch(node, **kw).replace("eventIn", "inputOnly")


# A clip that starts at load (loop TRUE, startTime 0) sends isActive with the first frame of the page,
# and the fetch / decode results arrive before or after that frame depending on real time (the
# order changed between a serial and a parallel X_ITE run). Cases that print the clip's events
# therefore start it from the Script only: stopTime 1.
ST = "stopTime 1 "


def clip(url, extra=""):
    return f"DEF O AudioClip {{ url {url} loop TRUE {extra}}}".replace("  }", " }")


CASES = {}

# --- Defect A: URL fallback after a decode failure, failure states, LoadSensor ---------------------

CASES["clip-fallback-decode"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip('[ "media/bad.mid" ' + WAV + ' ]', ST)} }}
""" + watch("O") + start("O")

CASES["clip-fail"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip('"media/bad.mid"', ST)} }}
""" + watch("O") + start("O")

CASES["clip-fail-all-decode"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip('[ "media/bad.mid" "media/bad2.mid" ]', ST)} }}
""" + watch("O") + start("O")

CASES["clip-fail-missing"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip('[ "media/missing-a.wav" "media/missing-b.wav" ]', ST)} }}
""" + watch("O") + start("O")

CASES["clip-fallback-missing"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip('[ "media/missing-a.wav" ' + WAV + ' ]', ST)} }}
""" + watch("O") + start("O")

CASES["clip-empty-url"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip('[ ]', ST)} }}
""" + watch("O") + start("O")

CASES["clip-loadsensor-fallback"] = X3D + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip('[ "media/bad.mid" ' + WAV + ' ]', ST)} }}
""" + load_sensor("O") + x3d_watch("O") + x3d_start("O")

CASES["clip-loadsensor-fail"] = X3D + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip('[ "media/bad.mid" "media/bad2.mid" ]', ST)} }}
""" + load_sensor("O") + x3d_watch("O") + x3d_start("O")

# A url change while the world runs: the fetch and the decode happen during the frames.
CASES["clip-url-change"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip(WAV, ST)} }}
DEF K Script {{ directOutput TRUE field SFNode o USE O eventIn SFTime set_time field SFInt32 n 0 url "javascript:
function set_time (value) {{ ++ n;
   if (n === 20) o.url = new MFString ('media/bad.mid', 'media/tone22k-mono.wav');
}}
" }}
ROUTE T.time TO K.set_time
""" + watch("O") + start("O")

# BufferAudioSource shares the loader: the same fallback after a decode failure.
BUFFER_PRINTS = """DEF P Script { inputOnly SFBool a inputOnly SFInt32 nc inputOnly SFTime bd url "ecmascript:
function a (v, t) { print ('active ' + v + ' ' + t); }
function nc (v, t) { print ('channels ' + v + ' ' + t); }
function bd (v, t) { print ('duration ' + v + ' ' + t); }
" }
ROUTE O.isActive TO P.a
ROUTE O.numberOfChannels TO P.nc
ROUTE O.bufferDuration TO P.bd
"""

CASES["buffer-fallback-decode"] = X3D + f"""AudioDestination {{ children DEF O BufferAudioSource {{ url [ "media/bad.mid" {WAV} ] }} }}
""" + BUFFER_PRINTS + load_sensor("O") + x3d_start("O")

CASES["buffer-fail-decode"] = X3D + f"""AudioDestination {{ children DEF O BufferAudioSource {{ url [ "media/bad.mid" "media/bad2.mid" ] }} }}
""" + BUFFER_PRINTS + load_sensor("O") + x3d_start("O")

# --- Defect B: a PROTO instance as Sound.source -----------------------------------------------------

PROTO = f"""PROTO MyClip [
  exposedField SFTime startTime 0
  exposedField MFString url {WAV}
  exposedField SFBool loop TRUE
  exposedField SFTime stopTime 1
  eventOut SFBool isActive
  eventOut SFTime duration_changed
] {{
  AudioClip {{ url IS url loop IS loop startTime IS startTime stopTime IS stopTime isActive IS isActive duration_changed IS duration_changed }}
}}
"""

CASES["sound-proto-source"] = VRML + VIEW + PROTO + f"""Sound {{ {BIG} location 3 0 0 source DEF O MyClip {{ }} }}
""" + watch("O") + start("O")

CASES["sound-proto-fallback"] = VRML + VIEW + PROTO + f"""Sound {{ {BIG} location -3 0 0 source DEF O MyClip {{ url [ "media/bad.mid" {WAV} ] }} }}
""" + watch("O") + start("O")

# A source that is not an X3DSoundSourceNode, and a PROTO whose first root is no sound source.
CASES["sound-invalid-source"] = VRML + VIEW + f"""PROTO NoClip [ exposedField SFTime startTime 0 ] {{
  Group {{ }}
  AudioClip {{ url {WAV} loop TRUE startTime IS startTime }}
}}
Sound {{ {BIG} location 0 0 -5 source Group {{ }} }}
Sound {{ {BIG} location 3 0 0 source DEF O NoClip {{ }} }}
""" + start("O")

# Source change while the world runs: a direct clip, then a PROTO clip.
CASES["sound-source-change"] = VRML + VIEW + PROTO + f"""DEF SND Sound {{ {BIG} location 3 0 0 source {clip(WAV)} }}
DEF Q MyClip {{ }}
DEF K Script {{ directOutput TRUE field SFNode s USE SND field SFNode q USE Q eventIn SFTime set_time field SFInt32 n 0 url "javascript:
function set_time (value) {{ ++ n;
   if (n === 24) s.source = q;
}}
" }}
ROUTE T.time TO K.set_time
""" + start("O", "Q")

# --- Playback, stereo placement, attenuation ---------------------------------------------------------

# stopTime 1: the clip does not start at load (an elapsedTime from a start at load holds the page's wall clock).
CASES["clip-play"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip(WAV, "stopTime 1 ")} }}
""" + watch("O", elapsed=True) + start("O")

CASES["clip-loop-end"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source DEF O AudioClip {{ url {WAV} }} }}
""" + watch("O", elapsed=True) + start("O")

CASES["clip-pitch"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source DEF O AudioClip {{ url {WAV} pitch 2 }} }}
DEF K Script {{ directOutput TRUE field SFNode o USE O eventIn SFTime set_time field SFInt32 n 0 url "javascript:
function set_time (value) {{ ++ n;
   if (n === 26) o.pitch = 0.5;
}}
" }}
ROUTE T.time TO K.set_time
""" + watch("O", elapsed=True) + start("O")

CASES["clip-pause-resume"] = X3D + VIEW + f"""Sound {{ {BIG} location 0 0 -5 source {clip(WAV, "stopTime 1 ")} }}
DEF K Script {{ directOutput TRUE initializeOnly SFNode o USE O inputOnly SFTime set_time initializeOnly SFInt32 n 0 url "ecmascript:
function set_time (value) {{ ++ n;
   if (n === 20) o.pauseTime = value;
   if (n === 26) o.resumeTime = value;
   if (n === 32) o.stopTime = value;
}}
" }}
ROUTE T.time TO K.set_time
""" + x3d_watch("O", elapsed=True, paused=True) + x3d_start("O")

CASES["sound-pan-left"] = VRML + VIEW + f"""Sound {{ {BIG} location -3 0 0 source {clip(WAV)} }}
""" + start("O")

CASES["sound-pan-right"] = VRML + VIEW + f"""Sound {{ {BIG} location 3 0 0 source {clip(WAV)} }}
""" + start("O")

CASES["sound-pan-center"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -3 source {clip(WAV)} }}
""" + start("O")

CASES["sound-nospatialize"] = VRML + VIEW + f"""Sound {{ {BIG} location 3 0 0 spatialize FALSE source {clip(WAV)} }}
""" + start("O")

CASES["sound-intensity"] = VRML + VIEW + f"""Sound {{ {BIG} location 0 0 -5 intensity 0.25 source {clip(WAV)} }}
Sound {{ {BIG} location 3 0 0 intensity 2 source DEF O2 AudioClip {{ url {WAV} loop TRUE }} }}
""" + start("O", "O2")

# Direction not +Z: the back channels and Rotation4.straighten.
CASES["sound-direction"] = VRML + VIEW + f"""Sound {{ {BIG} location 3 0 0 direction 1 0 0 source {clip(WAV)} }}
Sound {{ {BIG} location 0 0 -4 direction 0 0 -1 source DEF O2 AudioClip {{ url {WAV} loop TRUE }} }}
""" + start("O", "O2")

# Asymmetric ellipsoids: the viewer moves through min, between, and outside max.
CASES["sound-ellipsoid-asym"] = VRML + f"""DEF V Viewpoint {{ position 0 0 0 }}
Sound {{ minBack 2 minFront 8 maxBack 6 maxFront 20 direction 0.6 0 -0.8 location 1 0 -2 source {clip(WAV)} }}
DEF M Script {{ eventIn SFTime set_time eventOut SFVec3f p field SFInt32 n 0 url "javascript:
function set_time (value) {{ ++ n; if (n > 12) p = new SFVec3f (0.2 * (n - 12), 0, 1.0 * (n - 12)); }}
" }}
ROUTE T.time TO M.set_time
ROUTE M.p TO V.position
""" + start("O")

CASES["sound-zero-axis"] = VRML + VIEW + f"""Sound {{ minBack 0 minFront 4 maxBack 20 maxFront 20 location 0 0 -8 source {clip(WAV)} }}
Sound {{ minBack 1 minFront 1 maxBack 30 maxFront 0 location 2 0 -2 source DEF O2 AudioClip {{ url {WAV} loop TRUE }} }}
""" + start("O", "O2")

CASES["sound-transform"] = VRML + VIEW + f"""Transform {{ translation 1 0 -2 rotation 0 1 0 0.7 scale 2 1 0.5 children Sound {{ minFront 4 minBack 1 maxFront 12 maxBack 6 location 1 0 0 direction 1 0 0 source {clip(WAV)} }} }}
""" + start("O")

CASES["sound-two-nodes"] = VRML + VIEW + f"""Sound {{ minBack 0 minFront 3 maxBack 8 maxFront 9 location 2 0 -1 source {clip(WAV)} }}
Sound {{ minBack 1 minFront 2 maxBack 14 maxFront 30 location -4 0 -6 direction 1 0 0 source DEF O2 AudioClip {{ url "media/tone22k-mono.wav" loop TRUE }} }}
""" + start("O", "O2")

CASES["sound-children"] = X3D + VIEW + f"""Sound {{ {BIG} location 3 0 0 source {clip(WAV)} children Gain {{ gain 0.5 children DEF O2 OscillatorSource {{ frequency 220 }} }} }}
""" + x3d_start("O", "O2")

# Scene removal: the Sound leaves its Group at tick 24 and comes back at tick 32.
CASES["sound-lifecycle"] = VRML + VIEW + f"""DEF G Group {{ children DEF SND Sound {{ {BIG} location 3 0 0 source {clip(WAV, ST)} }} }}
DEF K Script {{ directOutput TRUE field SFNode g USE G field SFNode s USE SND eventIn SFTime set_time field SFInt32 n 0 url "javascript:
function set_time (value) {{ ++ n;
   if (n === 24) g.removeChildren = new MFNode (s);
   if (n === 32) g.addChildren = new MFNode (s);
}}
" }}
ROUTE T.time TO K.set_time
""" + watch("O") + start("O")

SCRIPT = {"dt": 0.016, "frames": 40, "warmup": 8, "dump": [40], "audio": {"sampleRate": 48000, "channels": 2}, "console": True}

def order(text):
    """Moves the start block (TimeSensor T, Script S, their ROUTEs) before the first DEF K / DEF M Script."""
    m = re.search(r"DEF T TimeSensor.*?ROUTE T\.time TO S\.set_time\n(?:ROUTE S\.start TO \S+\n)*", text, re.S)
    k = re.search(r"DEF [KM] Script", text)
    if not m or not k or k.start() > m.start():
        return text
    block = m.group(0)
    text = text[:m.start()] + text[m.end():]
    return text[:k.start()] + block + text[k.start():]


for name, text in CASES.items():
    text = order(text)
    open(os.path.join(out, name + ".wrl"), "w").write(text)
    json.dump(SCRIPT, open(os.path.join(out, "scripts", name + ".json"), "w"))

print(f"{len(CASES)} worlds written to {os.path.relpath(out, root)}")
