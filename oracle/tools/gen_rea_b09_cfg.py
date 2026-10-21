#!/usr/bin/env python3
"""Generate the configuration worlds oracle/synthetic-rea-b09-sai/cfg-*.wrl (REA batch 09, step 3, item 4: profiles, components, units).
Authored; synthetic inputs only.

cfg-comp probes the component state X3DScene.addComponent / updateComponent / removeComponent leave (any name and level, missing,
null and unknown arguments). cfg-scene-odd (documented open, not in cases.txt) shows the TypeError text of addComponent () /
addComponent (null) / updateComponent (): V8 words it "Cannot read properties of undefined (reading 'name')", QuickJS "cannot read
property 'name' of undefined"; the state those calls leave is in cfg-comp.

Each world runs a list of probes (functions returning a string, or a promise of one) from its Script's `initialize` and writes them in
order to the MFString field `out` ("label = value", `...` while pending), so the scene text of every dumped frame carries them. URLs are
shown without the page's host and without the directory of the script (`assets/components/XComponent.js`), because the oracle serves
X_ITE from a host of its own; `https://create3000.github.io/x_ite/` (the provider URL of the built-in components) is kept as it is.
"""
import os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-b09-sai")

HEAD = "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\n"
STEP = "DEF STEP TimeSensor { cycleInterval 0.05 loop TRUE }\nROUTE STEP.cycleTime TO OUT.cycle\n"


def vs(js):
    return js.replace("\\", "\\\\").replace('"', '\\"')


COMMON = """
function U (s) { return String (s) .replace (/^.*?(assets\\/components\\/)/, '$1'); }
function PI (p) { return p .name + '|' + p .title + '|' + U (p .providerURL) + '|' + p .components .length + '|' + Object .keys (p) .join (','); }
function CI (c) { return c .name + ':' + c .level + '|' + c .title + '|' + U (c .providerURL) + '|' + c .external + '|' + (c .dependencies ? c .dependencies .join ('+') : c .dependencies) + '|' + Object .keys (c) .join (','); }
function CL (a) { var r = []; for (var i = 0; i < a .length; ++ i) r .push (a [i] .name + ':' + a [i] .level); return r .join (','); }
function T (f) { try { return String (f ()); } catch (e) { return 'ERR:' + String (e && e .message) .replace (/http:\\/\\/[^\\/]+/g, 'H'); } }
function AT (f) { try { return Promise .resolve (f ()) .then (function (v) { return String (v); }, function (e) { return 'REJ:' + String (e && e .message); }); } catch (e) { return 'THROW:' + String (e && e .message); } }
function VL (s) { var t; try { t = s .toVRMLString (); } catch (e) { return 'PRINT ' + e .message; } return t .split ('\\n') .filter (function (l) { return /^(PROFILE|COMPONENT|UNIT|META|#)/ .test (l); }) .join ('|'); }
"""


def world(title, probes):
    names = [p[0] for p in probes]
    bodies = ",\n".join(f"function () {{ {code} }}" for _, code in probes)
    js = COMMON + f"""
var LABELS = {names!r};
var PROBES = [
{bodies}
];
var res = [];
var step = 0;
function put (i, v) {{ res [i] = LABELS [i] + ' = ' + v; }}
function flush () {{ var o = []; for (var k = 0; k < LABELS .length; ++ k) o .push (k in res ? res [k] : LABELS [k] + ' = ...'); out = new MFString (... o); }}
function cycle () {{ step = step + 1; if (step == 12) flush (); }}
function initialize () {{
  PROBES .forEach (function (f, i) {{
    var v;
    try {{ v = f (); }} catch (e) {{ v = 'THROW ' + e .message; }}
    if (v && typeof v .then === 'function') v .then (function (x) {{ put (i, x); }}, function (e) {{ put (i, 'REJECT ' + (e && e .message)); }});
    else put (i, v);
  }});
}}"""
    return (HEAD + f'META "title" "{title}"\n' + f'DEF OUT Script {{\n  inputOutput MFString out [ ]\n  inputOnly SFTime cycle\n  url "ecmascript:\n{vs(js)}\n  "\n}}\n' + STEP)


PROFILES = ["CADInterchange", "Core", "Full", "Immersive", "Interactive", "Interchange", "MedicalInterchange", "MPEG4Interactive"]

browser = [
    ("sp.length", "return Browser .supportedProfiles .length + ':' + Browser .supportedComponents .length + ':' + Browser .supportedProfiles .constructor .typeName + ':' + Browser .supportedComponents .constructor .typeName;"),
    ("sp.names", "var r = []; for (var i = 0; i < Browser .supportedProfiles .length; ++ i) r .push (Browser .supportedProfiles [i] .name); return r .join (',');"),
    ("sc.names", "var r = []; for (var i = 0; i < Browser .supportedComponents .length; ++ i) r .push (Browser .supportedComponents [i] .name); return r .join (',');"),
    ("same", "return (Browser .supportedProfiles === Browser .getSupportedProfiles ()) + ':' + (Browser .supportedComponents === Browser .getSupportedComponents ()) + ':' + (Browser .getProfile ('Core') === Browser .supportedProfiles .get ('Core'));"),
]
for p in PROFILES:
    browser.append((f"profile.{p}", f"var p = Browser .getProfile ('{p}'); return PI (p) + '|' + CL (p .components);"))
    browser.append((f"profile.{p}.comp0", f"var p = Browser .getProfile ('{p}'); return CI (p .components [0]);"))

browser += [
    ("profile.alias", "return T (function () { return Browser .getProfile ('MPEG-4') .name + ':' + (Browser .getProfile ('MPEG-4') === Browser .getProfile ('MPEG4Interactive')) + ':' + Browser .supportedProfiles .has ('MPEG-4') + ':' + Browser .supportedProfiles .length; });"),
    ("component.alias", "return ['H-Anim', 'TextureProjector', 'ProjectiveTextureMapping'] .map (function (n) { return T (function () { var c = Browser .getComponent (n, 9); return c .name + ':' + c .level + ':' + Browser .supportedComponents .has (n); }); }) .join (' ');"),
    ("profile.err.nope", "return T (function () { return Browser .getProfile ('Nope'); });"),
    ("profile.err.empty", "return T (function () { return Browser .getProfile (''); });"),
    ("profile.err.null", "return T (function () { return Browser .getProfile (null); });"),
    ("profile.err.undefined", "return T (function () { return Browser .getProfile (); });"),
    ("profile.err.number", "return T (function () { return Browser .getProfile (42); });"),
    ("profile.err.case", "return T (function () { return Browser .getProfile ('full'); });"),
    ("supported.profile.err", "return T (function () { return Browser .getSupportedProfile ('Nope'); });"),
    ("supported.profile.ok", "return T (function () { return Browser .getSupportedProfile ('Full') .name; });"),
    ("supported.component.err", "return T (function () { return Browser .getSupportedComponent ('Nope'); });"),
    ("supported.component.ok", "return T (function () { return CI (Browser .getSupportedComponent ('Layout')); });"),
    ("supported.component.alias", "return T (function () { return Browser .getSupportedComponent ('H-Anim') .name + ':' + Browser .supportedComponents .has ('H-Anim') + ':' + Browser .supportedComponents .has ('Nope') + ':' + Browser .supportedComponents .get ('Nope'); });"),
]

# Every supported component, in order.
browser.append(("sc.all", "var r = []; for (var i = 0; i < Browser .supportedComponents .length; ++ i) r .push (CI (Browser .supportedComponents [i])); return r .join (' ;; ');"))

# getComponent (name, level): the level is clamped to 1 .. supported.
for name in ["Core", "Geometry3D", "Interpolation", "H-Anim", "Layout", "Annotation", "WebXR", "X_ITE"]:
    for level in ["undefined", "0", "1", "2", "3", "5", "6", "99", "-1", "-99", "'2'", "'x'", "1.7", "NaN", "null", "true", "[2]"]:
        pass

browser.append(("getComponent.levels", "var r = []; [undefined, 0, 1, 2, 3, 4, 5, 6, 99, -1, -99, '2', 'x', 1.7, 2.9, NaN, null, true, [2], {}] .forEach (function (l) { r .push (T (function () { var c = Browser .getComponent ('Geometry3D', l); return c .name + ':' + c .level; })); }); return r .join (' ');"))
browser.append(("getComponent.names", "var r = []; ['Core', 'Layout', 'H-Anim', 'HAnim', 'core', 'Nope', '', 'Annotation', 'WebXR', 'X_ITE'] .forEach (function (n) { r .push (n + '=' + T (function () { var c = Browser .getComponent (n, 2); return c .name + ':' + c .level; })); }); return r .join (' ');"))
browser.append(("getComponent.nolevel", "var r = []; ['Core', 'Layout', 'Interpolation', 'Followers'] .forEach (function (n) { r .push (n + '=' + T (function () { return CI (Browser .getComponent (n)); })); }); return r .join (' ;; ');"))
browser.append(("getComponent.err", "return [T (function () { return Browser .getComponent ('Nope', 3); }), T (function () { return Browser .getComponent (); }), T (function () { return Browser .getComponent (null, 1); }), T (function () { return Browser .getComponent ('Nope', 'x'); })] .join (' | ');"))
browser.append(("info.frozen", "var c = Browser .getComponent ('Core', 1); var p = Browser .getProfile ('Core'); var r = []; r .push (Object .isFrozen (c), Object .isFrozen (p), Object .isFrozen (p .components)); c .name = 'x'; c .level = 9; p .name = 'y'; r .push (c .name, c .level, p .name); try { 'use strict'; delete c .name; } catch (e) { r .push ('del'); } return r .join (',') + ' ' + JSON .stringify (c) + ' ' + JSON .stringify (p) .substring (0, 90);"))
browser.append(("info.string", "var c = Browser .getComponent ('Core', 1); var p = Browser .getProfile ('Core'); return Object .prototype .toString .call (c) + ' ' + Object .prototype .toString .call (p) + ' ' + String (c) + ' ' + String (p) + ' ' + c .providerUrl + ' ' + p .providerUrl + ' ' + (c instanceof Object) + ' ' + typeof c .toVRMLString + ' ' + (typeof c .toVRMLString === 'function' ? c .toVRMLString () : '') + ' ' + (typeof p .toVRMLString === 'function' ? p .toVRMLString () : '');"))
browser.append(("info.xml", "var c = Browser .getComponent ('Core', 1); return typeof c .toXMLString + ' ' + (typeof c .toXMLString === 'function' ? c .toXMLString () : '') + ' ' + (typeof c .toJSONString === 'function' ? c .toJSONString () : '');"))
browser.append(("array.api", "var a = Browser .supportedComponents; return [typeof a .add, typeof a .get, typeof a .has, typeof a .remove, typeof a .update, typeof a .forEach, typeof a .map, typeof a .filter, typeof a .find, a .length, a [0] .name, a [39] .name, a [40], a .get (3) , a .get ('Core') .name].join (',');"))
browser.append(("array.mutate", "var a = Browser .supportedComponents; var n = a .length; return T (function () { a .add ('Foo', { name: 'Foo', level: 1 }); return a .length + ':' + a .get ('Foo') .name; }) + ' ' + T (function () { a .remove ('Foo'); return a .length + ':' + (a .length === n); });"))
browser.append(("add.supported", "return T (function () { Browser .addSupportedComponent (Browser .getComponent ('Core', 1)); return 'added'; }) + ' ' + T (function () { Browser .removeSupportedComponent ('Nope'); return 'rm' + Browser .supportedComponents .length; }) + ' ' + T (function () { Browser .updateSupportedComponent (Browser .getComponent ('Core', 1)); return 'upd' + Browser .supportedComponents .length; });"))
browser.append(("browser.keys", "return ['supportedProfiles', 'supportedComponents', 'getProfile', 'getComponent', 'getSupportedProfile', 'getSupportedProfiles', 'getSupportedComponent', 'getSupportedComponents', 'addSupportedProfile', 'updateSupportedProfile', 'removeSupportedProfile', 'addSupportedComponent', 'updateSupportedComponent', 'removeSupportedComponent', 'loadComponents', 'createScene'] .map (function (k) { return k + ':' + typeof Browser [k]; }) .join (',');"))
browser.append(("loadComponents", "return AT (function () { return Browser .loadComponents ('Geospatial') .then (function (v) { return 'ok ' + typeof v + ' ' + Array .isArray (v); }); }) ;"))
browser.append(("loadComponents.err", "return AT (function () { return Browser .loadComponents ('Nope') .then (function (v) { return 'ok'; }); });"))
browser.append(("loadComponents.mixed", "return AT (function () { return Browser .loadComponents (Browser .getProfile ('Core'), Browser .getComponent ('Layout', 1), 'Text', 42, null) .then (function (v) { return 'ok'; }); });"))

scenes = [
    ("create.none", "return AT (function () { return Browser .createScene () .then (function (s) { return [s .encoding, s .specificationVersion, s .profile, s .components .length, s .units .length, s .hasComponent ('Core'), s .hasComponent ('Geospatial'), s .hasComponent ('Geospatial', 1), s .hasComponent ('Core', 3), VL (s)] .join('|'); }); });"),
    ("create.profile", "return AT (function () { return Browser .createScene (Browser .getProfile ('Interchange')) .then (function (s) { return [s .profile .name, s .components .length, s .hasComponent ('Geometry3D', 2), s .hasComponent ('Geometry3D', 3), s .hasComponent ('Layout'), VL (s)] .join ('|'); }); });"),
    ("create.profile.components", "return AT (function () { return Browser .createScene (Browser .getProfile ('Core'), Browser .getComponent ('Geospatial', 1), Browser .getComponent ('Layout', 2), Browser .getComponent ('Geospatial', 2)) .then (function (s) { return [s .profile .name, CL (s .components), s .hasComponent ('Geospatial', 2), s .hasComponent ('Layout', 3), s .hasComponent ('Core', 1), s .hasComponent ('Core', 2), VL (s)] .join ('|'); }); });"),
    ("create.full", "return AT (function () { return Browser .createScene (Browser .getProfile ('Full')) .then (function (s) { return [s .profile .name, s .components .length, s .hasComponent ('Scripting'), s .hasComponent ('Annotation'), s .hasComponent ('WebXR'), VL (s)] .join ('|'); }); });"),
    ("create.err.string", "return AT (function () { return Browser .createScene ('Full'); });"),
    ("create.err.null", "return AT (function () { return Browser .createScene (null); });"),
    ("create.err.undefined", "return AT (function () { return Browser .createScene (undefined); });"),
    ("create.err.component", "return AT (function () { return Browser .createScene (Browser .getProfile ('Core'), 'Geospatial'); });"),
    ("create.err.component2", "return AT (function () { return Browser .createScene (Browser .getProfile ('Core'), Browser .getComponent ('Layout', 1), {}); });"),
    ("create.err.component3", "return AT (function () { return Browser .createScene (Browser .getProfile ('Core'), null); });"),
    ("create.err.plain", "return AT (function () { return Browser .createScene ({ name: 'Core', title: 't', providerURL: 'u', components: [] }); });"),
    ("create.identity", "return AT (function () { return Browser .createScene () .then (function (a) { return Browser .createScene () .then (function (b) { return (a === b) + ' ' + (a .components === b .components) + ' ' + a .worldURL .replace (/http:\\/\\/[^\\/]+/, 'H') + ' ' + a .baseURL .replace (/http:\\/\\/[^\\/]+/, 'H'); }); }); });"),
    ("create.many", "return AT (function () { return Browser .createScene (Browser .getProfile ('Core'), Browser .getComponent ('VolumeRendering', 1), Browser .getComponent ('Picking', 1)) .then (function (s) { return CL (s .components) + ' ' + VL (s) + ' ' + s .hasComponent ('VolumeRendering'); }); });"),
    ("scene.addComponent", """return AT(function () { return Browser .createScene (Browser .getProfile ('Core')) .then (function (s) {
      var r = [];
      r .push (T (function () { s .addComponent (Browser .getComponent ('Geospatial', 1)); return CL (s .components); }));
      r .push (T (function () { s .addComponent (Browser .getComponent ('Geospatial', 2)); return CL (s .components); }));
      r .push (T (function () { s .addComponent (Browser .getComponent ('Layout')); return CL (s .components); }));
      r .push (T (function () { s .addComponent ({ name: 'Foo', level: 3 }); return CL (s .components); }));
      r .push (T (function () { s .addComponent ({ name: 'Foo', level: 3 }); return CL (s .components); }));
      r .push (T (function () { return s .hasComponent ('Foo', 3) + ',' + s .hasComponent ('Foo', 4); }));
      r .push (VL (s));
      return r .join (' ## ');
    }); });"""),
    ("scene.updateComponent", """return AT(function () { return Browser .createScene (Browser .getProfile ('Core')) .then (function (s) {
      var r = [];
      r .push (T (function () { s .updateComponent (Browser .getComponent ('Geospatial', 1)); return CL (s .components); }));
      r .push (T (function () { s .updateComponent (Browser .getComponent ('Geospatial', 2)); return CL (s .components); }));
      r .push (T (function () { s .updateComponent (Browser .getComponent ('Layout', 1)); return CL (s .components); }));
      r .push (T (function () { s .removeComponent ('Geospatial'); return CL (s .components); }));
      r .push (T (function () { s .removeComponent ('Geospatial'); return CL (s .components); }));
      r .push (T (function () { s .removeComponent ('Nope'); return CL (s .components); }));
      r .push (T (function () { s .removeComponent (); return CL (s .components); }));
      r .push (VL (s));
      return r .join (' ## ');
    }); });"""),
    ("scene.setProfile", """return AT(function () { return Browser .createScene () .then (function (s) {
      var r = [];
      r .push (T (function () { s .setProfile (Browser .getProfile ('Interchange')); return s .profile .name + ' ' + s .hasComponent ('Geometry3D', 2); }));
      r .push (T (function () { s .setProfile (null); return s .profile + ' ' + s .hasComponent ('Geometry3D', 4); }));
      r .push (T (function () { s .setProfile ('Core'); return String (s .profile); }));
      r .push (T (function () { s .setProfile (); return String (s .profile); }));
      r .push (T (function () { s .setProfile (Browser .getProfile ('Core')); return VL (s); }));
      r .push (T (function () { return s .getProfile () === s .profile; }));
      return r .join (' ## ');
    }); });"""),
    ("scene.hasComponent", """return AT(function () { return Browser .createScene (Browser .getProfile ('Interactive'), Browser .getComponent ('Layout', 1)) .then (function (s) {
      var r = [];
      [['Core', 1], ['Core', 2], ['Core', 3], ['Navigation', 1], ['Navigation', 2], ['Layout', 1], ['Layout', 2], ['Text', 1], ['Nope', 1], ['', 0], ['Core'], ['Core', 0], ['Core', -1], ['Core', 'x'], ['H-Anim', 1], ['HAnim', 1]] .forEach (function (a) {
        r .push (a .join ('.') + '=' + T (function () { return s .hasComponent .apply (s, a); }));
      });
      r .push (T (function () { return s .hasComponent (Browser .getComponent ('Layout', 2)); }));
      r .push (T (function () { return s .hasComponent (Browser .getComponent ('Layout', 1)); }));
      r .push (T (function () { return s .hasComponent ({ name: 'Layout', level: 1 }); }));
      r .push (T (function () { return s .hasComponent (); }));
      return r .join (' ');
    }); });"""),
    ("scene.print", """return AT(function () { return Browser .createScene (Browser .getProfile ('Immersive'), Browser .getComponent ('Geospatial', 2), Browser .getComponent ('Layout', 1)) .then (function (s) {
      var vrml = s .toVRMLString () .split ('\\n') .join ('|');
      var xml = s .toXMLString () .split ('\\n') .join ('|');
      var json = s .toJSONString () .split ('\\n') .join ('|') .replace (/ +/g, ' ');
      return vrml + ' ## ' + xml + ' ## ' + json;
    }); });"""),
    ("scene.print.none", """return AT(function () { return Browser .createScene () .then (function (s) {
      return s .toVRMLString () .split ('\\n') .join ('|') + ' ## ' + s .toXMLString () .split ('\\n') .join ('|') + ' ## ' + s .toJSONString () .split ('\\n') .join ('|') .replace (/ +/g, ' ');
    }); });"""),
    ("scene.units", """return AT(function () { return Browser .createScene (Browser .getProfile ('Full')) .then (function (s) {
      var r = [];
      for (var i = 0; i < s .units .length; ++ i) r .push (s .units [i] .category + ':' + s .units [i] .name + ':' + s .units [i] .conversionFactor);
      r .push (T (function () { return s .getUnit ('length') .name + s .getUnit ('nope'); }));
      r .push (T (function () { s .updateUnit ('length', 'foot', 0.3048); return s .getUnit ('length') .name + ':' + s .getUnit ('length') .conversionFactor + ' ' + s .fromUnit ('length', 1) + ' ' + s .toUnit ('length', 1) + ' ' + VL (s); }));
      r .push (T (function () { s .updateUnit ('nope', 'x', 2); return s .units .length; }));
      r .push (T (function () { s .updateUnit ('angle', 'degree', '0.0174532925199433'); return s .getUnit ('angle') .conversionFactor + ' ' + s .fromUnit ('angle', 90); }));
      r .push (T (function () { s .updateUnit ('mass', 5, 'x'); return s .getUnit ('mass') .name + ':' + s .getUnit ('mass') .conversionFactor; }));
      r .push (T (function () { return s .fromUnit ('nope', 5) + ':' + s .toUnit ('nope', 5) + ':' + s .fromUnit ('speed', 5) + ':' + s .fromUnit ('area', 2); }));
      return r .join (' ## ');
    }); });"""),
]

cases = {"cfg-browser": world("cfg browser", browser), "cfg-scene": world("cfg scene", scenes)}

# Documented-open (not in cases.txt): the engine's TypeError text (see the header).
odd = [
    ("odd.addComponent.undefined", "return AT (function () { return Browser .createScene (Browser .getProfile ('Core')) .then (function (s) { return T (function () { s .addComponent (); return CL (s .components); }); }); });"),
    ("odd.addComponent.null", "return AT (function () { return Browser .createScene (Browser .getProfile ('Core')) .then (function (s) { return T (function () { s .addComponent (null); return CL (s .components); }); }); });"),
    ("odd.addComponent.string", "return AT (function () { return Browser .createScene (Browser .getProfile ('Core')) .then (function (s) { return T (function () { s .addComponent ('Core'); return CL (s .components) + ' ' + VL (s); }); }); });"),
    ("odd.updateComponent.undefined", "return AT (function () { return Browser .createScene (Browser .getProfile ('Core')) .then (function (s) { return T (function () { s .updateComponent (); return CL (s .components); }); }); });"),
]
cases["cfg-scene-odd"] = world("cfg scene odd", odd)


# Component state of a scene (owner decision 2026-10-20: X_ITE's observable component behavior, the engine's TypeError text aside).
# Each probe makes a Core scene, runs one call and shows: the outcome (a thrown error by its class and, unless it is a TypeError
# whose text is the engine's, its message), the components (name:level:typeof level), hasComponent, and the COMPONENT lines of the
# VRML, XML and JSON printouts.
COMP = """
function E (e) { return e instanceof TypeError ? 'TypeError' : (e && e .constructor && e .constructor .name) + ':' + String (e && e .message); }
function CT (a) { var r = []; for (var i = 0; i < a .length; ++ i) r .push (a [i] .name + ':' + a [i] .level + ':' + typeof a [i] .level + ':' + typeof a [i] .name); return r .join (','); }
function XL (s) { var t; try { t = s .toXMLString (); } catch (e) { return 'PRINT ' + E (e); } return t .split ('\\n') .filter (function (l) { return /<component/ .test (l); }) .map (function (l) { return l .trim (); }) .join ('|'); }
function JL (s) { var t; try { t = s .toJSONString (); } catch (e) { return 'PRINT ' + E (e); } var m = t .replace (/\\s+/g, ' ') .match (/"component": \\[[^\\]]*\\]/); return m ? m [0] : 'none'; }
function C (op) {
  return AT (function () { return Browser .createScene (Browser .getProfile ('Core')) .then (function (s) {
    var r; try { r = 'ok:' + op (s); } catch (e) { r = 'ERR:' + E (e); }
    return [r, CT (s .components), T (function () { return s .hasComponent ('Geometry3D') + '/' + s .hasComponent ('Geometry3D', 3) + '/' + s .hasComponent ('Nope'); }), VL (s), XL (s), JL (s)] .join (' ## ');
  }); });
}
"""
comp = [
    ("add.none", "return C (function (s) { return s .addComponent (); });"),
    ("add.null", "return C (function (s) { return s .addComponent (null); });"),
    ("add.string", "return C (function (s) { return s .addComponent ('Core'); });"),
    ("add.number", "return C (function (s) { return s .addComponent (5); });"),
    ("add.info", "return C (function (s) { return s .addComponent (Browser .getComponent ('Geometry3D', 2)); });"),
    ("add.info.twice", "return C (function (s) { s .addComponent (Browser .getComponent ('Geometry3D', 2)); return s .addComponent (Browser .getComponent ('Geometry3D', 3)); });"),
    ("add.info.max", "return C (function (s) { return s .addComponent (Browser .getComponent ('Geometry3D')); });"),
    ("add.obj", "return C (function (s) { return s .addComponent ({ name: 'Geometry3D', level: 2 }); });"),
    ("add.obj.nolevel", "return C (function (s) { return s .addComponent ({ name: 'Geometry3D' }); });"),
    ("add.obj.float", "return C (function (s) { return s .addComponent ({ name: 'Geometry3D', level: 2.5 }); });"),
    ("add.obj.strlevel", "return C (function (s) { return s .addComponent ({ name: 'Geometry3D', level: '3' }); });"),
    ("add.obj.neg", "return C (function (s) { return s .addComponent ({ name: 'Geometry3D', level: -1 }); });"),
    ("add.obj.nan", "return C (function (s) { return s .addComponent ({ name: 'Geometry3D', level: NaN }); });"),
    ("add.obj.unknown", "return C (function (s) { return s .addComponent ({ name: 'Nope', level: 2 }); });"),
    ("add.obj.noname", "return C (function (s) { return s .addComponent ({ level: 2 }); });"),
    ("add.obj.numname", "return C (function (s) { return s .addComponent ({ name: 7, level: 1 }); });"),
    ("add.core", "return C (function (s) { return s .addComponent (Browser .getComponent ('Core', 1)); });"),
    ("upd.none", "return C (function (s) { return s .updateComponent (); });"),
    ("upd.null", "return C (function (s) { return s .updateComponent (null); });"),
    ("upd.string", "return C (function (s) { return s .updateComponent ('Core'); });"),
    ("upd.obj", "return C (function (s) { return s .updateComponent ({ name: 'Geometry3D', level: 2 }); });"),
    ("upd.info.new", "return C (function (s) { return s .updateComponent (Browser .getComponent ('Geometry3D', 2)); });"),
    ("upd.info.replace", "return C (function (s) { s .addComponent (Browser .getComponent ('Geometry3D', 2)); s .addComponent (Browser .getComponent ('Grouping', 1)); return s .updateComponent (Browser .getComponent ('Geometry3D', 4)); });"),
    ("rem.name", "return C (function (s) { s .addComponent (Browser .getComponent ('Geometry3D', 2)); return s .removeComponent ('Geometry3D'); });"),
    ("rem.unknown", "return C (function (s) { s .addComponent (Browser .getComponent ('Geometry3D', 2)); return s .removeComponent ('Nope'); });"),
    ("rem.none", "return C (function (s) { s .addComponent (Browser .getComponent ('Geometry3D', 2)); return s .removeComponent (); });"),
    ("rem.null", "return C (function (s) { s .addComponent (Browser .getComponent ('Geometry3D', 2)); return s .removeComponent (null); });"),
    ("rem.info", "return C (function (s) { s .addComponent (Browser .getComponent ('Geometry3D', 2)); return s .removeComponent (Browser .getComponent ('Geometry3D', 2)); });"),
    ("get.unknown", "return T (function () { return CT ([Browser .getComponent ('Nope', 2)]); });"),
    ("get.nolevel", "return T (function () { return CT ([Browser .getComponent ('Geometry3D')]); });"),
    ("get.toolevel", "return T (function () { return CT ([Browser .getComponent ('Geometry3D', 9)]); });"),
    ("get.strlevel", "return T (function () { return CT ([Browser .getComponent ('Geometry3D', '2')]); });"),
    ("get.none", "return T (function () { return CT ([Browser .getComponent ()]); });"),
]
cases["cfg-comp"] = world("cfg comp", comp).replace("function U (s)", vs(COMP) + "function U (s)", 1)

# Statements of the three encodings.
STRINGS = {
    "vrml-profile": ["#X3D V3.3 utf8\\nPROFILE %s\\nGroup { }" % p for p in PROFILES + ["MPEG-4", "Nope", "full", ""]],
    "vrml-component": ["#X3D V3.3 utf8\\nPROFILE Core\\nCOMPONENT %s\\nGroup { }" % c for c in
                       ["Geospatial:1", "Geospatial:2", "Geospatial:3", "Geospatial:0", "Geospatial:-1", "Geospatial:x", "Geospatial", "Geospatial :", "Nope:1", "H-Anim:1", "HAnim:3", "HAnim:4", "Annotation:1", "Annotation:2", "Annotation:3", "WebXR:1", "X_ITE:1", "Core:3", "TextureProjector:1", "ProjectiveTextureMapping:2", "TextureProjection:2", "Core:1\\nCOMPONENT Core:2", "Layout:1\\nCOMPONENT Layout:2", "CADGeometry:2\\nCOMPONENT Geospatial:1"]],
    "vrml-misc": ["#X3D V3.3 utf8\\nPROFILE Core\\nUNIT length foot 0.3048\\nGroup { }", "#X3D V3.3 utf8\\nPROFILE Core\\nUNIT length foot x\\nGroup { }",
                  "#X3D V3.3 utf8\\nPROFILE Core\\nUNIT length foot\\nGroup { }", "#X3D V3.3 utf8\\nPROFILE Core\\nUNIT length\\nGroup { }", "#X3D V3.3 utf8\\nPROFILE Core\\nUNIT\\nGroup { }",
                  "#X3D V3.3 utf8\\nPROFILE Core\\nUNIT angle degree 0.5\\nUNIT angle radian 1\\nGroup { }", "#X3D V3.3 utf8\\nPROFILE Core\\nUNIT acceleration x 2\\nUNIT speed y 3\\nGroup { }",
                  "#X3D V3.3 utf8\\nPROFILE Core\\nPROFILE Full\\nGroup { }", "#X3D V3.3 utf8\\nGroup { }\\nPROFILE Core\\n", "#X3D V3.3 utf8\\nCOMPONENT Core:1\\nPROFILE Core\\nGroup { }",
                  "#X3D V3.3 utf8\\nPROFILE\\nGroup { }", "#X3D V3.3 utf8\\nPROFILE Core\\nCOMPONENT\\nGroup { }", "#X3D V3.3 utf8\\nPROFILE Core\\nCOMPONENT Core:\\nGroup { }"],
    "xml": [
        '<?xml version="1.0"?><X3D profile="%s" version="3.3"><Scene><Group/></Scene></X3D>' % p for p in ["Full", "Core", "Immersive", "MPEG-4", "Nope", ""]
    ] + [
        '<?xml version="1.0"?><X3D profile="Core" version="3.3"><head><component name="%s" level="%s"/></head><Scene><Group/></Scene></X3D>' % c for c in
        [("Geospatial", "1"), ("Geospatial", "9"), ("Nope", "1"), ("Geospatial", "x"), ("", "1"), ("Geospatial", ""), ("H-Anim", "2"), ("TextureProjector", "1")]
    ] + [
        '<?xml version="1.0"?><X3D profile="Core" version="3.3"><head><unit category="length" name="foot" conversionFactor="0.3048"/><unit category="nope" name="x" conversionFactor="2"/><unit category="angle" name="d" conversionFactor="x"/></head><Scene><Group/></Scene></X3D>',
        '<?xml version="1.0"?><X3D version="3.3"><Scene><Group/></Scene></X3D>',
        '<?xml version="1.0"?><X3D profile="Core"><Scene><Group/></Scene></X3D>',
    ],
    "json": [
        '{"X3D":{"@profile":"%s","@version":"3.3","Scene":{"-children":[]}}}' % p for p in ["Full", "Core", "Nope"]
    ] + [
        '{"X3D":{"@profile":"Core","@version":"3.3","head":{"component":[{"@name":"%s","@level":%s}]},"Scene":{"-children":[]}}}' % c for c in [("Geospatial", "1"), ("Geospatial", "9"), ("Nope", "1")]
    ] + [
        '{"X3D":{"@profile":"Core","@version":"3.3","head":{"unit":[{"@category":"length","@name":"foot","@conversionFactor":0.3048}]},"Scene":{"-children":[]}}}',
    ],
}

JS_PARSE = """
var step = 0;
function put (i, v) { res [i] = i + ': ' + v; }
function flush () { var o = []; for (var k = 0; k < SNIPPETS .length; ++ k) o .push (k in res ? res [k] : k + ': ...'); out = new MFString (... o); }
function cycle () { step = step + 1; if (step == 12) flush (); }
function strip (s) { return String (s) .replace (/http:\\/\\/[^\\/]+/g, 'H'); }
"""


def parse_world(name, snippets):
    import json
    js = f"""var SNIPPETS = {json.dumps([s.replace(chr(92) + 'n', chr(10)) for s in snippets])};
var res = [];
{JS_PARSE}
function initialize () {{
  SNIPPETS .forEach (function (src, i) {{
    var p;
    try {{ p = Browser .createX3DFromString (src); }} catch (e) {{ put (i, 'THROW ' + strip (e .message)); return; }}
    p .then (function (sc) {{
      var comps = []; for (var k = 0; k < sc .components .length; ++ k) comps .push (sc .components [k] .name + ':' + sc .components [k] .level + ':' + sc .components [k] .title);
      var units = []; for (var k = 0; k < sc .units .length; ++ k) units .push (sc .units [k] .category + ':' + sc .units [k] .name + ':' + sc .units [k] .conversionFactor);
      var text; try {{ text = sc .toVRMLString () .split ('\\n') .join ('|'); }} catch (e) {{ text = 'PRINT ERR ' + e .message; }}
      put (i, [sc .profile && (sc .profile .name + '/' + sc .profile .title + '/' + sc .profile .components .length), comps .join (','), units .join (','), sc .hasComponent ('Geospatial'), sc .hasComponent ('Core', 2), text] .join (' ## '));
    }}, function (e) {{ put (i, 'REJ ' + strip (e && e .message) .split ('\\n') .join ('|')); }});
  }});
}}"""
    return HEAD + f'META "title" "{name}"\nDEF OUT Script {{\n  inputOutput MFString out [ ]\n  inputOnly SFTime cycle\n  url "ecmascript:\n{vs(js)}\n  "\n}}\n' + STEP


for k, v in STRINGS.items():
    cases["cfg-parse-" + k] = parse_world("cfg parse " + k, v)

if __name__ == "__main__":
    os.makedirs(out_dir, exist_ok=True)

    for name, text in cases.items():
        with open(os.path.join(out_dir, name + ".wrl"), "w") as f:
            f.write(text)

    print(len(cases), "worlds")
