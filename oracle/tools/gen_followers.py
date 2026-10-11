#!/usr/bin/env python3
"""Generate oracle/synthetic-followers/ (worlds + animation scripts for the Followers component).

New files only. Everything is synthetic (no Mall or archive content). Each world drives followers
(Color/Coordinate/Orientation/Position/Position2D/Scalar/TexCoord2D x Chaser/Damper) with
 * interpolators fed by TimeSensor bursts (the destination changes every frame, mid-flight),
 * a Script ("DRV") that counts frames from a looping TimeSensor and sends set_value jumps and
   set_destination events at fixed frames,
and routes every value_changed to visible geometry (Transform, Material, TextureTransform,
Coordinate in a PointSet, TextureCoordinate in a textured quad). A tracer Script per follower
records every event it receives (count, a running full-precision checksum, and the frames at
which isActive changed), so the scene text carries values the printed fields round away.

Run (per world):  python3 -I oracle/tools/x10_compare.py oracle/synthetic-followers/<w>.wrl \
                      oracle/synthetic-followers/scripts/<w>.json /tmp/fol/<w> --target target-viewer/release
"""
import json, os

out = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-followers")
os.makedirs(os.path.join(out, "scripts"), exist_ok=True)
T0 = 946684800

# --- kinds -------------------------------------------------------------------------------------
# sf: field type; dflt: initialValue / initialDestination default as VRML text; interp: destination
# interpolator (node, keyValue); tgt: how the value is shown.
KINDS = {
    "Position": dict(sf="SFVec3f", dflt="0 0 0", interp="PositionInterpolator", tgt="translation"),
    "Position2D": dict(sf="SFVec2f", dflt="0 0", interp="PositionInterpolator2D", tgt="tt"),
    "Orientation": dict(sf="SFRotation", dflt="0 1 0 0", interp="OrientationInterpolator", tgt="rotation"),
    "Color": dict(sf="SFColor", dflt="0.8 0.8 0.8", interp="ColorInterpolator", tgt="diffuse"),
    "Scalar": dict(sf="SFFloat", dflt="0", interp="ScalarInterpolator", tgt="shininess"),
    "Coordinate": dict(sf="MFVec3f", dflt="0 0 0", interp="CoordinateInterpolator", tgt="coord"),
    "TexCoord2D": dict(sf="MFVec2f", dflt="0 0", interp="CoordinateInterpolator2D", tgt="texcoord"),
}
KEY = [0, 0.3, 0.6, 1]
KEYVALUE = {
    "Position": "0 0 0, 1.2 0.8 0, -1.2 0.8 0.5, 0 0 0",
    "Position2D": "0 0, 0.5 0.25, -0.25 0.5, 0 0",
    "Orientation": "0 1 0 0, 0 1 0 1.5, 1 0 0 3, 0 0.6 0.8 0.5",
    "Color": "1 0 0, 0 1 0, 0.2 0.3 1, 1 1 0",
    "Scalar": "0 1 0.2 0.8",
    "Coordinate": "-1 -1 0 1 -1 0 1 1 0 -1 1 0, -1.5 -0.5 0 1 -1.5 0 0.5 1 0 -1 1.5 0, -0.5 -1 0 1.5 -1 0 1 0.5 0 -1.5 1 0, -1 -1 0 1 -1 0 1 1 0 -1 1 0",
    "TexCoord2D": "0 0 1 0 1 1 0 1, 0.25 0 1.25 0 1.25 1 0.25 1, -0.25 0.1 0.8 0.1 0.8 0.9 -0.25 0.9, 0 0 1 0 1 1 0 1",
}


def vec_list(text):
    return [[float(x) for x in item.split()] for item in text.split(",")]


def js_value(kind, v):
    """JavaScript expression for the value v (a list of numbers) of a kind."""
    sf = KINDS[kind]["sf"]
    if sf == "SFFloat":
        return repr(v[0])
    if sf == "SFVec3f":
        return "new SFVec3f (%s)" % ", ".join(map(repr, v))
    if sf == "SFVec2f":
        return "new SFVec2f (%s)" % ", ".join(map(repr, v))
    if sf == "SFRotation":
        return "new SFRotation (%s)" % ", ".join(map(repr, v))
    if sf == "SFColor":
        return "new SFColor (%s)" % ", ".join(map(repr, v))
    n = 3 if sf == "MFVec3f" else 2
    el = "SFVec3f" if n == 3 else "SFVec2f"
    pts = [v[i:i + n] for i in range(0, len(v), n)]
    return "new %s (%s)" % (sf, ", ".join("new %s (%s)" % (el, ", ".join(map(repr, p))) for p in pts))


# alternative values used for jumps and destinations; MF kinds take a point count.
def alt(kind, which, count=4):
    sf = KINDS[kind]["sf"]
    A = {
        "SFVec3f": [[2.0, -1.0, 0.5], [-2.5, 1.5, -0.5], [0.0, 0.0, 0.0]],
        "SFVec2f": [[0.6, -0.4], [-0.3, 0.7], [0.0, 0.0]],
        "SFRotation": [[0, 0, 1, 2.2], [1, 0, 0, -2.9], [0, 1, 0, 0]],
        "SFColor": [[0.1, 0.9, 0.5], [0.9, 0.2, 0.7], [0.8, 0.8, 0.8]],
        "SFFloat": [[0.9], [0.1], [0.0]],
    }
    if sf in A:
        return A[sf][which % 3]
    n = 3 if sf == "MFVec3f" else 2
    pts = []
    for i in range(count):
        if n == 3:
            pts += [round(-1.5 + 0.7 * i + 0.3 * which, 3), round(((-1) ** i) * (0.4 + 0.15 * i), 3), round(0.1 * which, 3)]
        else:
            pts += [round(0.15 * i + 0.1 * which, 3), round(0.9 - 0.2 * i - 0.05 * which, 3)]
    return pts


def last_js(kind):
    """JavaScript that stores the exact components of the last event value in the string `last`."""
    sf = KINDS[kind]["sf"]
    if sf == "SFFloat":
        return "last = '' + v;"
    if sf in ("SFVec3f",):
        return "last = v.x + ' ' + v.y + ' ' + v.z;"
    if sf == "SFVec2f":
        return "last = v.x + ' ' + v.y;"
    if sf == "SFRotation":
        return "last = v.x + ' ' + v.y + ' ' + v.z + ' ' + v.angle;"
    if sf == "SFColor":
        return "last = v.r + ' ' + v.g + ' ' + v.b;"
    comp = "v[i].x + ' ' + v[i].y + ' ' + v[i].z" if sf == "MFVec3f" else "v[i].x + ' ' + v[i].y"
    return "last = ''; for (var i = 0; i < v.length; ++ i) last = last + (i ? ', ' : '') + %s;" % comp


def tracer_js(kind):
    sf = KINDS[kind]["sf"]
    if sf == "SFFloat":
        return "acc = acc * 1.0000001 + v;"
    if sf == "SFVec3f":
        return "acc = acc * 1.0000001 + v.x + 2 * v.y + 3 * v.z;"
    if sf == "SFVec2f":
        return "acc = acc * 1.0000001 + v.x + 2 * v.y;"
    if sf == "SFRotation":
        return "acc = acc * 1.0000001 + v.x + 2 * v.y + 3 * v.z + 4 * v.angle;"
    if sf == "SFColor":
        return "acc = acc * 1.0000001 + v.r + 2 * v.g + 3 * v.b;"
    comp = "v[i].x + 2 * v[i].y + 3 * v[i].z" if sf == "MFVec3f" else "v[i].x + 2 * v[i].y"
    return "len = v.length; for (var i = 0; i < v.length; ++ i) acc = acc * 1.0000001 + %s;" % comp


class F:
    """One follower. kind: Position ...; typ: Chaser | Damper."""
    def __init__(self, kind, typ, name, params="", init=None, dest=None, auto=True, show=True):
        self.kind, self.typ, self.name, self.params = kind, typ, name, params
        self.init, self.dest, self.auto, self.show = init, dest, auto, show

    @property
    def node(self):
        return self.kind + self.typ + ("2D" if self.kind in ("Position2D", "TexCoord2D") and False else "")

    @property
    def type_name(self):
        k = self.kind
        if k == "Position2D":
            return "PositionChaser2D" if self.typ == "Chaser" else "PositionDamper2D"
        if k == "TexCoord2D":
            return "TexCoordChaser2D" if self.typ == "Chaser" else "TexCoordDamper2D"
        return k + self.typ


def world(name, fs, schedule, frames, dt=1 / 60, bursts=((10, 1.5), (170, 1.0)), capture=None, dump_every=1, title=""):
    """fs: followers; schedule: [(frame, follower name, 'set_value' | 'set_destination', value list)];
    bursts: TimeSensor bursts (start frame, seconds) feeding every auto interpolator."""
    L = []
    w = L.append
    w("#X3D V3.3 utf8")
    w("PROFILE Immersive")
    w("COMPONENT Followers:1")
    w("# Synthetic Followers world (%s), generated by oracle/tools/gen_followers.py. No archive or Mall content." % (title or name))
    w("META \"generator\" \"gen_followers.py\"")
    w("Viewpoint { position 0 0 %d description \"main\" }" % (10 + 3 * ((len(fs) + 3) // 4)))
    w("DEF TICK TimeSensor { cycleInterval 1 loop TRUE }")
    for i, (start, secs) in enumerate(bursts):
        w("DEF CLOCK%d TimeSensor { cycleInterval %s }" % (i + 1, secs))

    # followers, interpolators, targets
    cols = 4
    for idx, f in enumerate(fs):
        kd = KINDS[f.kind]
        x, y = (idx % cols) * 4 - 6, -(idx // cols) * 3.5 + 3
        mf = kd["sf"].startswith("MF")
        q = (lambda v: "[ %s ]" % v) if mf else (lambda v: v)
        isf = ""  # omitted: the node's own defaults
        if f.init is not None:
            isf += " initialValue %s" % q(f.init)
        if f.dest is not None:
            isf += " initialDestination %s" % q(f.dest)
        w("DEF %s %s { %s%s }" % (f.name, f.type_name, f.params, isf))
        t = kd["tgt"]
        shape_app = "appearance Appearance { material DEF %s_M Material { diffuseColor 0.8 0.5 0.2 } }" % f.name
        if t == "translation":
            w("Transform { translation %g %g 0 children DEF %s_X Transform { children Shape { %s geometry Box { size 0.6 0.6 0.6 } } } }" % (x, y, f.name, shape_app))
        elif t == "rotation":
            w("Transform { translation %g %g 0 children DEF %s_X Transform { children Shape { %s geometry Box { size 1.4 0.5 0.3 } } } }" % (x, y, f.name, shape_app))
        elif t == "diffuse":
            w("Transform { translation %g %g 0 children Shape { appearance Appearance { material DEF %s_M Material { diffuseColor 0.8 0.8 0.8 } } geometry Sphere { radius 0.7 } } }" % (x, y, f.name))
        elif t == "shininess":
            w("Transform { translation %g %g 0 children Shape { appearance Appearance { material DEF %s_M Material { diffuseColor 0.3 0.6 0.9 specularColor 1 1 1 shininess 0 } } geometry Sphere { radius 0.7 } } }" % (x, y, f.name))
        elif t == "tt":
            w("Transform { translation %g %g 0 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture PixelTexture { image 2 2 3 0xff0000 0x00ff00 0x0000ff 0xffff00 } textureTransform DEF %s_X TextureTransform { } } geometry Box { size 1.4 1.4 0.2 } } }" % (x, y, f.name))
        elif t == "coord":
            w("Transform { translation %g %g 0 children Shape { appearance Appearance { material Material { emissiveColor 1 0.8 0.2 } } geometry PointSet { coord DEF %s_X Coordinate { } } } }" % (x, y, f.name))
        elif t == "texcoord":
            w("Transform { translation %g %g 0 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture PixelTexture { image 2 2 3 0xff0000 0x00ff00 0x0000ff 0xffff00 } } geometry IndexedFaceSet { coord Coordinate { point [ -1 -1 0, 1 -1 0, 1 1 0, -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] texCoord DEF %s_X TextureCoordinate { } } } }" % (x, y, f.name))
        # tracer
        sf = kd["sf"]
        w("DEF %s_T Script {\n  eventIn SFBool act\n  eventIn %s val\n  field SFInt32 n 0\n  field SFInt32 len 0\n  field SFDouble acc 0\n  field SFString acts \"\"\n  field SFString last \"\"\n  url \"ecmascript:\n"
          "function act (v, ts) { acts = acts + Math.round ((ts - %d) / %r) + (v ? '+' : '-') + ' '; }\n"
          "function val (v, ts) { n = n + 1; %s %s }\n  \"\n}" % (f.name, sf, T0, dt, tracer_js(f.kind), last_js(f.kind)))
        if f.auto:
            w("DEF %s_I %s { key [ %s ] keyValue [ %s ] }" % (f.name, kd["interp"], " ".join(map(str, KEY)), KEYVALUE[f.kind]))

    # driver
    outs = sorted({(n, inp) for _, n, inp, _ in schedule})
    byname = {f.name: f for f in fs}
    w("DEF DRV Script {")
    w("  eventIn SFTime tick")
    for i in range(len(bursts)):
        w("  eventOut SFTime go%d" % (i + 1))
    for n, inp in outs:
        w("  eventOut %s %s_%s" % (KINDS[byname[n].kind]["sf"], n, inp))
    w("  field SFInt32 lastK -1")
    w("  url \"ecmascript:")
    w("function tick (value, ts) {")
    w("  var k = Math.round ((ts - %d) / %r);" % (T0, dt))
    w("  if (k == lastK) return;")
    w("  lastK = k;")
    for i, (start, secs) in enumerate(bursts):
        w("  if (k == %d) go%d = ts;" % (start, i + 1))
    for fr, n, inp, v in schedule:
        w("  if (k == %d) %s_%s = %s;" % (fr, n, inp, js_value(byname[n].kind, v)))
    w("}")
    w("  \"")
    w("}")

    # routes
    w("ROUTE TICK.time TO DRV.tick")
    for i in range(len(bursts)):
        w("ROUTE DRV.go%d TO CLOCK%d.startTime" % (i + 1, i + 1))
    for n, inp in outs:
        w("ROUTE DRV.%s_%s TO %s.%s" % (n, inp, n, inp))
    for f in fs:
        kd = KINDS[f.kind]
        t = kd["tgt"]
        target = {"translation": "%s_X.translation", "rotation": "%s_X.rotation", "diffuse": "%s_M.diffuseColor",
                  "shininess": "%s_M.shininess", "tt": "%s_X.translation", "coord": "%s_X.point", "texcoord": "%s_X.point"}[t] % f.name
        w("ROUTE %s.value_changed TO %s" % (f.name, target))
        w("ROUTE %s.value_changed TO %s_T.val" % (f.name, f.name))
        w("ROUTE %s.isActive TO %s_T.act" % (f.name, f.name))
        if f.auto:
            for i in range(len(bursts)):
                w("ROUTE CLOCK%d.fraction_changed TO %s_I.set_fraction" % (i + 1, f.name))
            w("ROUTE %s_I.value_changed TO %s.set_destination" % (f.name, f.name))

    open(os.path.join(out, name + ".wrl"), "w").write("\n".join(L) + "\n")
    cap = capture or [1, frames // 6, frames // 3, frames // 2, 2 * frames // 3, frames]
    dump = sorted(set(list(range(1, frames + 1, dump_every)) + [frames]))
    json.dump({"size": [512, 512], "t0": T0, "dt": dt, "pinFrameRate": True, "frames": frames, "capture": cap, "dump": dump, "events": []},
              open(os.path.join(out, "scripts", name + ".json"), "w"), indent=1)


def standard_schedule(fs, base=0):
    """set_value jumps and set_destination events in flight, in the idle phase, and equal to the current value."""
    S = []
    for j, f in enumerate(fs):
        a = alt(f.kind, 0)
        b = alt(f.kind, 1)
        c = alt(f.kind, 2)
        o = base + (j % 3)  # stagger by frame so followers do not all fire together
        S += [
            (30 + o, f.name, "set_value", a),  # jump while the burst moves the destination
            (46 + o, f.name, "set_destination", b),  # extra destination mid-flight
            (47 + o, f.name, "set_destination", a),  # and again one frame later
            (125 + o, f.name, "set_destination", c),  # idle phase: restarts the follower
            (131 + o, f.name, "set_destination", b),
            (150 + o, f.name, "set_value", b),  # jump while moving
            (235 + o, f.name, "set_value", c),  # jump in the idle phase (activates)
            (262 + o, f.name, "set_destination", c),  # destination equal to the current value
        ]
    return S


# --- 1. chasers: every node, several durations ---------------------------------------------------
ch = [
    F("Position", "Chaser", "PC1", "duration 0.5"), F("Position", "Chaser", "PC2", "duration 1.5"),
    F("Orientation", "Chaser", "OC1", "duration 0.8"), F("Orientation", "Chaser", "OC2", "duration 2.5"),
    F("Color", "Chaser", "CC1", "duration 0.3"), F("Color", "Chaser", "CC2", "duration 1"),
    F("Scalar", "Chaser", "SC1", "duration 0.25"), F("Scalar", "Chaser", "SC2", "duration 2"),
    F("Scalar", "Chaser", "SC3", "duration 0.05"),
    F("Position2D", "Chaser", "P2C1", "duration 0.6"), F("Position2D", "Chaser", "P2C2", "duration 1.2"),
    F("Coordinate", "Chaser", "CoC1", "duration 0.7"), F("Coordinate", "Chaser", "CoC2", "duration 1.6"),
    F("TexCoord2D", "Chaser", "TC1", "duration 0.7"), F("TexCoord2D", "Chaser", "TC2", "duration 1.6"),
]
world("chasers", ch, standard_schedule(ch), 320, title="chasers: all 7 node types, several durations", capture=[1, 20, 40, 60, 100, 160, 200, 300, 320])

# --- 2. dampers: every node, order 0..5, tau and tolerance variants -----------------------------
da = [
    F("Position", "Damper", "PD0", "order 0 tau 0.3"), F("Position", "Damper", "PD1", "order 1 tau 0.3"),
    F("Position", "Damper", "PD2", "order 2 tau 0.5"), F("Position", "Damper", "PD3", "order 3 tau 0.3"),
    F("Position", "Damper", "PD4", "order 4 tau 0.2"), F("Position", "Damper", "PD5", "order 5 tau 0.4"),
    F("Position", "Damper", "PD6", "order 3 tau 0.3 tolerance 0.05"),
    F("Orientation", "Damper", "OD3", "order 3 tau 0.4"), F("Orientation", "Damper", "OD1", "order 1 tau 0.15 tolerance 0.01"),
    F("Color", "Damper", "CD3", "order 3 tau 0.3"), F("Color", "Damper", "CD2", "order 2 tau 0.6 tolerance 0.02"),
    F("Scalar", "Damper", "SD0", "order 0 tau 0.25"), F("Scalar", "Damper", "SD1", "order 1 tau 0.5"), F("Scalar", "Damper", "SD2", "order 2 tau 0.1"),
    F("Scalar", "Damper", "SD3", "order 3 tau 0.3"), F("Scalar", "Damper", "SD4", "order 4 tau 1"), F("Scalar", "Damper", "SD5", "order 5 tau 0.3 tolerance 0.001"),
    F("Position2D", "Damper", "P2D3", "order 3 tau 0.3"), F("Position2D", "Damper", "P2D1", "order 1 tau 0.8 tolerance 0.005"),
    F("Coordinate", "Damper", "CoD3", "order 3 tau 0.3"), F("Coordinate", "Damper", "CoD1", "order 1 tau 0.6 tolerance 0.01"),
    F("TexCoord2D", "Damper", "TD3", "order 3 tau 0.3"), F("TexCoord2D", "Damper", "TD2", "order 2 tau 0.5 tolerance 0.01"),
    F("Position", "Damper", "PDtau0", "order 3 tau 0"),
]
world("dampers", da, standard_schedule(da), 320, title="dampers: all 7 node types, order 0..5, tau, tolerance", capture=[1, 20, 40, 60, 100, 160, 200, 300, 320])

# --- 3. initial state: initialValue != initialDestination (active at load) and equal ----------------
ia = [
    F("Position", "Chaser", "iPC", "duration 1", init="0 0 0", dest="1.5 1 0", auto=False),
    F("Position", "Damper", "iPD", "order 3 tau 0.3", init="0 0 0", dest="-1.5 1 0", auto=False),
    F("Orientation", "Chaser", "iOC", "duration 1", init="0 1 0 0", dest="0 0 1 2", auto=False),
    F("Orientation", "Damper", "iOD", "order 2 tau 0.3", init="0 1 0 0", dest="1 0 0 -2", auto=False),
    F("Color", "Chaser", "iCC", "duration 1", init="1 0 0", dest="0 0 1", auto=False),
    F("Color", "Damper", "iCD", "order 3 tau 0.3", init="0 1 0", dest="1 0 1", auto=False),
    F("Scalar", "Chaser", "iSC", "duration 1", init="0", dest="1", auto=False),
    F("Scalar", "Damper", "iSD", "order 3 tau 0.3", init="1", dest="0.2", auto=False),
    F("Position2D", "Chaser", "iP2C", "duration 1", init="0 0", dest="0.5 0.5", auto=False),
    F("Position2D", "Damper", "iP2D", "order 3 tau 0.3", init="0 0", dest="-0.5 0.2", auto=False),
    F("Coordinate", "Chaser", "iCoC", "duration 1", init="0 0 0, 1 0 0", dest="-1 -1 0, 1 -1 0, 1 1 0, -1 1 0", auto=False),
    F("Coordinate", "Damper", "iCoD", "order 3 tau 0.3", init="-1 -1 0, 1 -1 0, 1 1 0", dest="-1 -1 0, 1 -1 0", auto=False),
    F("TexCoord2D", "Chaser", "iTC", "duration 1", init="0 0, 1 0, 1 1, 0 1", dest="0.5 0.5, 1.5 0.5, 1.5 1.5, 0.5 1.5", auto=False),
    F("TexCoord2D", "Damper", "iTD", "order 3 tau 0.3", init="0 0, 1 0, 1 1, 0 1", dest="0.2 0, 1 0, 1 1, 0 0.8", auto=False),
    # equal: not active, value_changed set once at initialize
    F("Position", "Chaser", "ePC", "duration 1", init="1 2 3", dest="1 2 3", auto=False),
    F("Orientation", "Damper", "eOD", "order 3", init="0 0 1 1", dest="0 0 1 1", auto=False),
    F("Color", "Chaser", "eCC", "duration 1", init="0.2 0.4 0.6", dest="0.2 0.4 0.6", auto=False),
    F("Coordinate", "Damper", "eCoD", "order 3", init="1 2 3, 4 5 6", dest="1 2 3, 4 5 6", auto=False),
    F("TexCoord2D", "Chaser", "eTC", "duration 1", init="0 0, 1 1", dest="0 0, 1 1", auto=False),
    F("Scalar", "Chaser", "eSC", "duration 1", init="0.5", dest="0.5", auto=False),
]
world("initial", ia, [(40, "iPC", "set_destination", [-2.0, 0.0, 1.0]), (90, "ePC", "set_destination", [0.0, 3.0, 0.0]), (100, "eSC", "set_value", [0.9])], 200,
      bursts=(), title="initialValue != initialDestination, equal initial state", capture=[1, 5, 30, 60, 120, 200])

# --- 4. arrays: coordinate / texcoord lengths change ---------------------------------------------
ar = [
    F("Coordinate", "Chaser", "aCoC", "duration 0.8", auto=False),
    F("Coordinate", "Damper", "aCoD", "order 3 tau 0.2", auto=False),
    F("Coordinate", "Chaser", "aCoCi", "duration 0.8", auto=True),  # fixed-length 4 interpolator feeds it too
    F("TexCoord2D", "Chaser", "aTC", "duration 0.8", auto=False),
    F("TexCoord2D", "Damper", "aTD", "order 3 tau 0.2", auto=False),
    F("Coordinate", "Damper", "aCoDz", "order 0 tau 0.3", auto=False),
]
S = []
lens = [3, 6, 2, 0, 5, 4, 7, 1, 4]
for j, f in enumerate(ar):
    n3 = 3 if KINDS[f.kind]["sf"] == "MFVec3f" else 2
    for i, ln in enumerate(lens):
        S.append((20 + 25 * i + j, f.name, "set_destination", alt(f.kind, i, ln)))
    # set_value with a length that differs from the destination's; during motion
    S.append((70 + j, f.name, "set_value", alt(f.kind, 1, 5)))
    S.append((145 + j, f.name, "set_value", alt(f.kind, 0, 2)))
    S.append((190 + j, f.name, "set_value", alt(f.kind, 2, 0)))
    S.append((196 + j, f.name, "set_destination", alt(f.kind, 0, 3)))
world("arrays", ar, S, 300, bursts=((15, 2.0),), title="Coordinate/TexCoord2D followers with changing array lengths", capture=[1, 25, 50, 75, 100, 150, 200, 250, 300])

# --- 5. interest order: followers feeding each other and sharing a target -------------------------
# Two chasers write the same Transform.translation; their activation order decides which value wins.
# A chaser chain (A.value_changed -> B.set_destination) and a damper into a chaser.
od = []
L = ["#X3D V3.3 utf8", "PROFILE Immersive", "COMPONENT Followers:1", "# Synthetic Followers world (order): activation order and chains. No archive or Mall content.",
     "Viewpoint { position 0 0 16 }", "DEF TICK TimeSensor { cycleInterval 1 loop TRUE }"]
w = L.append
w("DEF A PositionChaser { duration 0.6 }")
w("DEF B PositionChaser { duration 1.4 }")
w("DEF D PositionDamper { order 2 tau 0.2 }")
w("DEF E PositionChaser { duration 0.4 }")
w("DEF SA ScalarChaser { duration 0.5 }")
w("DEF SB ScalarChaser { duration 0.5 }")
w("DEF SD ScalarDamper { order 1 tau 0.15 }")
w("Transform { translation -3 0 0 children DEF SHARED Transform { children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.3 0.2 } } geometry Box { size 0.8 0.8 0.8 } } } }")
w("Transform { translation 3 0 0 children DEF CHAINED Transform { children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.5 0.9 } } geometry Sphere { radius 0.5 } } } }")
w("Transform { translation 0 -3 0 children DEF DAMPED Transform { children Shape { appearance Appearance { material DEF SM Material { diffuseColor 0.3 0.8 0.4 } } geometry Cone { bottomRadius 0.5 height 1 } } } }")
w("DEF TR Script { eventIn SFBool aAct eventIn SFBool bAct eventIn SFBool dAct eventIn SFBool eAct eventIn SFBool saAct eventIn SFBool sbAct eventIn SFBool sdAct")
w("  field SFString acts \"\" url \"ecmascript:")
w("function mark (c, v, ts) { acts = acts + c + Math.round ((ts - %d) * 60) + (v ? '+' : '-') + ' '; }" % T0)
for c in ["a", "b", "d", "e", "sa", "sb", "sd"]:
    w("function %sAct (v, ts) { mark ('%s', v, ts); }" % (c, c.upper()))
w("\" }")
w("DEF DRV Script { eventIn SFTime tick eventOut SFVec3f aDest eventOut SFVec3f bDest eventOut SFVec3f dDest eventOut SFVec3f eVal eventOut SFFloat saDest eventOut SFFloat sbDest eventOut SFFloat sdDest eventOut SFVec3f bVal field SFInt32 lastK -1")
w("  url \"ecmascript:")
w("function tick (value, ts) {")
w("  var k = Math.round ((ts - %d) * 60);" % T0)
w("  if (k == lastK) return; lastK = k;")
sched = [
    (5, "aDest = new SFVec3f (2, 1, 0);"), (6, "bDest = new SFVec3f (-2, 1, 0);"),  # A then B: A activates first
    (60, "bDest = new SFVec3f (2, -1, 0);"), (62, "aDest = new SFVec3f (-2, -1, 0);"),  # B first, then A
    (120, "aDest = new SFVec3f (1, 1, 1);"), (120, "bDest = new SFVec3f (-1, -1, -1);"),
    (150, "bDest = new SFVec3f (0, 2, 0);"), (151, "aDest = new SFVec3f (0, -2, 0);"),  # while both still moving, B re-activated first
    (200, "dDest = new SFVec3f (1, 2, 3);"), (203, "eVal = new SFVec3f (-1, 0, 0);"),
    (240, "bVal = new SFVec3f (0, 0, 0);"),
    (10, "saDest = 1;"), (14, "sbDest = 0.5;"), (80, "sdDest = 1;"), (110, "saDest = 0;"), (112, "sdDest = 0;"), (170, "sbDest = 1;"), (230, "saDest = 0.5;"),
]
for fr, code in sched:
    w("  if (k == %d) { %s }" % (fr, code))
w("}\"")
w("}")
w("ROUTE TICK.time TO DRV.tick")
# A and B both drive SHARED; A.value_changed also chains into E; D drives CHAINED; B drives A's set_destination at the end
w("ROUTE DRV.aDest TO A.set_destination")
w("ROUTE DRV.bDest TO B.set_destination")
w("ROUTE DRV.bVal TO B.set_value")
w("ROUTE A.value_changed TO SHARED.set_translation")
w("ROUTE B.value_changed TO SHARED.set_translation")
w("ROUTE DRV.dDest TO D.set_destination")
w("ROUTE D.value_changed TO E.set_destination")
w("ROUTE DRV.eVal TO E.set_value")
w("ROUTE E.value_changed TO CHAINED.set_translation")
w("ROUTE DRV.saDest TO SA.set_destination")
w("ROUTE SA.value_changed TO SB.set_destination")
w("ROUTE DRV.sbDest TO SB.set_destination")
w("ROUTE SB.value_changed TO SD.set_destination")
w("ROUTE SD.value_changed TO SM.set_shininess")
w("ROUTE SA.value_changed TO SM.set_transparency")
w("ROUTE A.isActive TO TR.aAct")
w("ROUTE B.isActive TO TR.bAct")
w("ROUTE D.isActive TO TR.dAct")
w("ROUTE E.isActive TO TR.eAct")
w("ROUTE SA.isActive TO TR.saAct")
w("ROUTE SB.isActive TO TR.sbAct")
w("ROUTE SD.isActive TO TR.sdAct")
open(os.path.join(out, "order.wrl"), "w").write("\n".join(L) + "\n")
json.dump({"size": [512, 512], "t0": T0, "dt": 1 / 60, "pinFrameRate": True, "frames": 320, "capture": [1, 20, 70, 130, 160, 210, 320], "dump": list(range(1, 321)), "events": []},
          open(os.path.join(out, "scripts", "order.json"), "w"), indent=1)

# --- 6. parameters: tolerance / tau / order edge cases and field changes while running -----------
ed = [
    F("Scalar", "Damper", "Z0", "order 3 tau 0.3 tolerance 0"),  # tolerance 0: never settles
    F("Scalar", "Damper", "Zneg", "order 3 tau -0.3"),  # negative tau
    F("Scalar", "Damper", "Zbig", "order 9 tau 0.3"),  # order clamped to 5
    F("Scalar", "Damper", "Zlow", "order -2 tau 0.3"),  # order clamped to 0
    F("Scalar", "Damper", "Ztiny", "order 2 tau 0.0001"),  # alpha ~ 0
    F("Position", "Damper", "Zhuge", "order 2 tau 100 tolerance 5"),  # hits tolerance fast
    F("Orientation", "Damper", "ZO", "order 2 tau 0.3 tolerance 0"),
    F("Scalar", "Chaser", "Zdur", "duration 4"),
    F("Position", "Chaser", "ZP", "duration 0.01"),
    F("Orientation", "Chaser", "ZOC", "duration 0.01"),
    F("Scalar", "Chaser", "ZD0", "duration 0"),  # stepTime 0: fraction is NaN / Infinity
    F("Position", "Chaser", "ZPD0", "duration 0"),
    F("Scalar", "Chaser", "ZDneg", "duration -1"),
]
sch = []
for j, f in enumerate(ed):
    sch += [(20 + j, f.name, "set_destination", alt(f.kind, 0)), (90 + j, f.name, "set_value", alt(f.kind, 1)), (110 + j, f.name, "set_destination", alt(f.kind, 2)),
            (170 + j, f.name, "set_destination", alt(f.kind, 1))]
world("edge", ed, sch, 260, bursts=(), title="tolerance 0, negative tau, order clamp, tiny/huge tau and duration", capture=[1, 30, 60, 100, 160, 260])

# --- 7. other frame intervals: Damper tau uses currentFrameRate; Chaser steps jump several buffers ----
d30 = [F(k, "Damper", "D%d" % i, "order %d tau %s" % (i % 6, 0.15 + 0.1 * i)) for i, k in enumerate(["Position", "Orientation", "Color", "Scalar", "Position2D", "Coordinate", "TexCoord2D", "Scalar"])]
world("dampers30", d30, standard_schedule(d30)[:], 160, dt=1 / 30, bursts=((5, 1.0), (85, 0.5)), title="dampers at 30 frames per second", capture=[1, 30, 80, 160])
for dt, tag in ((1 / 24, "24"), (0.25, "quarter"), (0.07, "coarse")):
    cx = [F("Position", "Chaser", "A", "duration %s" % (dt * 60)), F("Position", "Chaser", "B", "duration %s" % (dt * 30)), F("Orientation", "Chaser", "C", "duration %s" % (dt * 61)),
          F("Color", "Chaser", "D", "duration %s" % (dt * 7)), F("Scalar", "Chaser", "E", "duration %s" % (dt * 2)), F("Scalar", "Chaser", "G", "duration %s" % (dt * 59.5)),
          F("Coordinate", "Chaser", "H", "duration %s" % (dt * 90)), F("Position2D", "Damper", "I", "order 2 tau %s" % (dt * 3))]
    sch = []
    for j, f in enumerate(cx):
        sch += [(8 + j, f.name, "set_destination", alt(f.kind, 0)), (14 + j, f.name, "set_value", alt(f.kind, 1)), (30 + j, f.name, "set_destination", alt(f.kind, 2)),
                (31 + j, f.name, "set_destination", alt(f.kind, 0)), (60 + j, f.name, "set_destination", alt(f.kind, 1))]
    world("frames-" + tag, cx, sch, 100, dt=dt, bursts=((3, dt * 20),), title="frame interval %s s: Chaser buffer steps larger than one" % dt, capture=[1, 20, 50, 100])
