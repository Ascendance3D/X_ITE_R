#!/usr/bin/env python3
"""Generate the Browser.loadURL / Browser.replaceWorld worlds oracle/synthetic-rea-b09-sai/loadurl-*.wrl, replaceworld-*.wrl and
the worlds they load (assets/lu-*.wrl) (REA batch 09, step 3). Authored; synthetic inputs only.

Every world A calls one of the two functions from a Script at its third TimeSensor cycle. What A sees is logged with console.log
(the sai-console.json run records the console in order, with levels); what the *new* world sees is written into its own `hist`
field, so the scene text of the dumped frames carries it. The harness serves the world from a host with a random port, so URLs
are printed without their scheme and host (`H`).

A's Scripts never define shutdown (), prepareEvents () or eventsProcessed (): the port calls shutdown () of a replaced world's
Scripts (owner-approved kept difference, crates/x3d-script/tests/script_shutdown_kept_difference.rs), X_ITE does not.
"""
import os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out_dir = os.path.join(root, "oracle", "synthetic-rea-b09-sai")
asset_dir = os.path.join(out_dir, "assets")

HEAD = "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\n"


def vs(js):
    """JavaScript source as the body of a VRML string: \\ and \" are the only escapes the lexer reads."""
    return js.replace("\\", "\\\\").replace('"', '\\"')


def script(name, fields, js):
    return f'DEF {name} Script {{\n{fields}  url "ecmascript:\n{vs(js)}\n  "\n}}\n'


# URL without scheme and host.
H = """function H (s) { s = String (s); var i = s .indexOf ('://'); if (i < 0) return s; var j = s .indexOf ('/', i + 3); return 'H' + (j < 0 ? '' : s .substring (j)); }"""

# L (text): the log of a Script. A run that records the console (sai-console.json) must not produce warnings or errors the
# harness page swallows (X_ITE's console.warn / error go to the page's `log`; only console.log reaches the console), so the
# cases that fail a load log into `hist` instead (sai.json).
def log_fn(mode):
    return "function L (s) { console .log (s); }" if mode == "console" else "function L (s) { hist = hist + s + ';'; }"


TICK = "DEF STEP TimeSensor { cycleInterval 0.05 loop TRUE }\n"

# The callbacks A registers (they stay registered after A's world is gone, in both browsers).
A_CALLBACKS = """
function initialize () {
  var f = function (e) { L ('A: cb ' + e); };
  Browser .addBrowserCallback ('a', X3DConstants .INITIALIZED_EVENT, f);
  Browser .addBrowserCallback ('a', X3DConstants .SHUTDOWN_EVENT, f);
  Browser .addBrowserCallback ('a', X3DConstants .INITIALIZED_ERROR, f);
@INIT@
}"""


def world_a(title, body, mode="console", init=""):
    """A world with a Script AS that runs go (step) at every cycle of its TimeSensor."""
    js = f"""{H}
{log_fn(mode)}
{A_CALLBACKS.replace("@INIT@", init)}
function cycle () {{
  step = step + 1;
  go (step);
}}
{body}"""
    fields = "  inputOutput SFString hist \"\"\n  inputOutput SFInt32 step 0\n  inputOnly SFTime cycle\n"
    return (HEAD + f'''META "title" "{title}"
DEF WA WorldInfo {{ title "A" }}
DEF NA NavigationInfo {{ type "FLY" headlight FALSE }}
DEF BGA Background {{ skyColor 1 0 0 }}
DEF VPA Viewpoint {{ description "vpA" position 0 0 10 }}
DEF VPA2 Viewpoint {{ description "vpA2" position 4 0 10 }}
''' + script("AS", fields, js) + TICK + "ROUTE STEP.cycleTime TO AS.cycle\n")


# What A logs for a promise.
def track(expr, label):
    return f"""(function () {{
  var p = {expr};
  L ('A: {label} ret ' + typeof p + ' ' + (p instanceof Promise));
  p .then (function (v) {{ L ('A: {label} resolved ' + typeof v + ' cs=' + H (Browser .currentScene .worldURL) + ' ec=' + H (Browser .getExecutionContext () .worldURL)); }},
           function (e) {{ L ('A: {label} rejected ' + (e && e .message)); }});
}}) ();"""


# World B: what a world loaded by loadURL records about itself.
def world_b(mode):
    return WORLD_B_TEMPLATE.replace("@LOG@", log_fn(mode))


WORLD_B_TEMPLATE = HEAD + 'META "title" "lu world B"\nDEF WB WorldInfo { title "B" }\nDEF VPB Viewpoint { description "vpB" position 0 0 7 }\nDEF VPB2 Viewpoint { description "vpB2" position 2 0 7 }\n' + script(
    "BS",
    "  inputOutput SFString hist \"\"\n  inputOutput SFInt32 step 0\n  inputOnly SFTime cycle\n  inputOnly SFBool bound\n  inputOnly SFBool bound2\n  inputOnly SFTime bindtime\n",
    H + """
@LOG@
var seen = '';
function initialize () {
  var f = function (e) { L ('B: cb ' + e); };
  Browser .addBrowserCallback ('b', X3DConstants .INITIALIZED_EVENT, f);
  Browser .addBrowserCallback ('b', X3DConstants .SHUTDOWN_EVENT, f);
  Browser .addBrowserCallback ('b', X3DConstants .INITIALIZED_ERROR, f);
  var named;
  try { named = Browser .currentScene .getNamedNode ('VPA') .getNodeName (); } catch (e) { named = 'none'; }
  L ('B: initialize desc=[' + Browser .description + '] initialized=' + Browser .initialized () .getValue ());
  hist = hist + 'init wurl=' + H (Browser .worldURL) + ' get=' + H (Browser .getWorldURL ()) + ' cs=' + H (Browser .currentScene .worldURL)
    + ' ec=' + H (Browser .getExecutionContext () .worldURL) + ' roots=' + Browser .currentScene .rootNodes .length
    + ' vpb=' + Browser .currentScene .getNamedNode ('VPB') .getNodeName () + ' vpa=' + named
    + ' leak=' + (typeof leak === 'undefined' ? 'undef' : leak) + ' name=' + Browser .name
    + ' enc=' + Browser .currentScene .encoding + ' profile=' + Browser .currentScene .profile .name;
}
function bound (v) { seen = seen + 'b' + (v ? 1 : 0) + '@' + step + ' '; L ('B: VPB isBound ' + v + ' desc=[' + Browser .description + ']'); }
function bound2 (v) { seen = seen + 'c' + (v ? 1 : 0) + '@' + step + ' '; L ('B: VPB2 isBound ' + v); }
function bindtime (t) { seen = seen + 't@' + step + ' '; }
function cycle () {
  step = step + 1;
  if (step == 2)
    hist = hist + ' | step2 desc=[' + Browser .description + '] init=' + Browser .initialized () .getValue () + ' shut=' + Browser .shutdown () .getValue () + ' seen=' + seen;
}""") + TICK + "ROUTE STEP.cycleTime TO BS.cycle\nROUTE VPB.isBound TO BS.bound\nROUTE VPB2.isBound TO BS.bound2\nROUTE VPB.bindTime TO BS.bindtime\n"

# A world with an Inline that must load before the world is replaced.
INLINE_LEAF = HEAD + "DEF IL Transform { translation 1 2 3 }\n"
WORLD_B_INLINE = HEAD + 'META "title" "lu world B inline"\nDEF VPI Viewpoint { description "vpI" position 0 0 6 }\nDEF II Inline { url [ "lu-inl.wrl" ] }\n' + script(
    "IS",
    "  inputOutput SFString hist \"\"\n  inputOnly SFTime cycle\n  initializeOnly SFNode inl USE II\n",
    H + """
function initialize () { hist = 'init inl=' + inl .getNodeName () + ' loadstate=' + inl .loadState; }
function cycle () { if (hist .indexOf ('step') < 0) hist = hist + ' | step loadstate=' + inl .loadState + ' roots=' + Browser .currentScene .rootNodes .length; }"""
) + TICK + "ROUTE STEP.cycleTime TO IS.cycle\n"

# World B that loads C from its own Script (a chain of switches).
WORLD_B_CHAIN = HEAD + 'META "title" "lu world B chain"\nDEF VPC1 Viewpoint { description "vpBchain" position 0 0 5 }\n' + script(
    "CS",
    "  inputOutput SFString hist \"\"\n  inputOutput SFInt32 step 0\n  inputOnly SFTime cycle\n",
    H + """
function initialize () { hist = 'chain B ' + H (Browser .worldURL); }
function cycle () {
  step = step + 1;
  if (step == 2)
    Browser .loadURL (new MFString ('lu-c.wrl')) .then (function () { console .log ('B: loadURL C resolved ec=' + H (Browser .getExecutionContext () .worldURL)); });
}"""
) + TICK + "ROUTE STEP.cycleTime TO CS.cycle\n"

WORLD_C = HEAD + 'META "title" "lu world C"\nDEF VPC Viewpoint { description "vpC" position 0 0 4 }\nDEF NC NavigationInfo { type "FLY" }\n' + script(
    "CC",
    "  inputOutput SFString hist \"\"\n  inputOutput SFInt32 step 0\n  inputOnly SFTime cycle\n",
    H + """
function initialize () {
  var vpb; try { vpb = Browser .currentScene .getNamedNode ('VPB') .getNodeName (); } catch (e) { vpb = 'none'; }
  hist = 'C init ' + H (Browser .worldURL) + ' vpb=' + vpb;
}
function cycle () { step = step + 1; }"""
) + TICK + "ROUTE STEP.cycleTime TO CC.cycle\n"

BAD = "#VRML V2.0 utf8\nGroup { children [ Transform { translation 1 2 \n"

cases = {}


def load(name, urls, extra="", params=None, at=3, mode="console"):
    p = f", new MFString ({', '.join(repr(x) for x in params)})" if params is not None else ""
    u = ", ".join(repr(x) for x in urls)
    cases[name] = world_a("lu " + name, f"""function go (s) {{
  if (s == {at}) {track(f"Browser .loadURL (new MFString ({u}){p})", "load")}
{extra}}}""", mode)


def again(step, label="load2", url="assets/lu-b.wrl"):
    return f"  if (s == {step}) " + track(f"Browser .loadURL (new MFString ('{url}'))", label) + "\n"


load("loadurl-ok", ["assets/lu-b.wrl"])
load("loadurl-hash-start", ["assets/lu-b.wrl#VPB2"])
load("loadurl-fallback", ["assets/lu-missing.wrl", "assets/lu-bh.wrl"], mode="hist")
load("loadurl-fail", ["assets/lu-missing.wrl"], extra=again(7, url="assets/lu-bh.wrl"), mode="hist")
load("loadurl-fail2", ["", "assets/lu-m1.wrl", "assets/lu-m2.wrl"], extra=again(7, url="assets/lu-bh.wrl"), mode="hist")
load("loadurl-empty", [], extra=again(7))
load("loadurl-badscene", ["assets/lu-bad.wrl"], extra=again(7, url="assets/lu-bh.wrl"), mode="hist")
load("loadurl-inline", ["assets/lu-b-inline.wrl"])
load("loadurl-chain", ["assets/lu-b-chain.wrl"])
load("loadurl-param", ["assets/lu-b.wrl"], params=["foo=bar", "target=_self", "target=a=b"])
load("loadurl-viewpoint", ["#VPA2"], extra=again(5, "load2", "#nosuch") + again(6, "load3"))

cases["loadurl-badargs"] = world_a("lu loadurl-badargs", f"""function go (s) {{
  if (s == 3) {{
    {track("Browser .loadURL ('assets/lu-b.wrl')", "string")}
    {track("Browser .loadURL (new MFString ('assets/lu-b.wrl'), 'target=_blank')", "parameter")}
    {track("Browser .loadURL ()", "none")}
    {track("Browser .loadURL (null)", "null")}
    {track("Browser .loadURL (new MFString ('assets/lu-b.wrl'), new SFString ('x'))", "sfstring")}
  }}
}}""")

cases["loadurl-twice"] = world_a("lu loadurl-twice", f"""function go (s) {{
  if (s == 3) {{
    {track("Browser .loadURL (new MFString ('assets/lu-bh.wrl'))", "first")}
    {track("Browser .loadURL (new MFString ('assets/lu-c.wrl'))", "second")}
  }}
}}""", "hist")

cases["loadurl-then-replace"] = world_a("lu loadurl-then-replace", f"""function go (s) {{
  if (s == 3) {{
    {track("Browser .loadURL (new MFString ('assets/lu-bh.wrl'))", "load")}
    {track("Browser .replaceWorld (null)", "replace")}
  }}
}}""", "hist")

for name, arg in [("replaceworld-null", "null"), ("replaceworld-none", ""), ("replaceworld-string", "'text'"), ("replaceworld-number", "42")]:
    cases[name] = world_a("lu " + name, f"""function go (s) {{
  if (s == 3) {{
    {track(f"Browser .replaceWorld ({arg})", "replace")}
  }}
}}""")

# The scene a Script makes with createX3DFromString: its own Script runs initialize () when the scene is made.
SCENE_SRC = (
    "#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Scripting:1\n"
    'DEF VPN Viewpoint { description "vpN" position 0 0 3 }\n'
    'DEF NS Script { inputOutput SFString hist "" inputOnly SFTime t url "ecmascript:'
    "function initialize () { var sp = String .fromCharCode (32); hist = Browser .currentScene .getNamedNode (String .fromCharCode (86, 80, 78)) .getNodeName () + sp + Browser .currentScene .rootNodes .length + sp + Browser .currentScene .encoding; console .log (String .fromCharCode (78, 58) + sp + 'initialize'); }"
    '" }\n'
    "DEF NT TimeSensor { loop TRUE cycleInterval 0.05 }\nROUTE NT.cycleTime TO NS.t\n"
)

cases["replaceworld-scene"] = world_a("lu replaceworld-scene", f"""var scene = null;
function go (s) {{
  if (s == 2) Browser .createX3DFromString ({SCENE_SRC!r}) .then (function (sc) {{ scene = sc; }}, function (e) {{ L ('A: create rejected ' + e .message); }});
  if (s == 3) {{
    L ('A: scene ' + scene .encoding + ' ' + scene .rootNodes .length);
    {track("Browser .replaceWorld (scene)", "replace")}
  }}
}}""")

cases["replaceworld-nodes"] = world_a("lu replaceworld-nodes", f"""function go (s) {{
  if (s == 3) {{
    var nodes = Browser .createVrmlFromString ('DEF NV Viewpoint {{ description "vpV" position 0 0 2 }} DEF NG Group {{ children [ DEF NW WorldInfo {{ title "nodes" }} ] }}');
    console .log ('A: nodes ' + nodes .length + ' ' + nodes [0] .getNodeName ());
    {track("Browser .replaceWorld (nodes)", "replace")}
  }}
}}""")

cases["replaceworld-twice"] = world_a("lu replaceworld-twice", f"""var one = null, two = null;
function go (s) {{
  if (s == 2) {{
    Browser .createX3DFromString ('#X3D V3.3 utf8\\nPROFILE Immersive\\nDEF ONE WorldInfo {{ title "one" }}') .then (function (sc) {{ one = sc; }});
    Browser .createX3DFromString ('#X3D V3.3 utf8\\nPROFILE Immersive\\nDEF TWO WorldInfo {{ title "two" }}') .then (function (sc) {{ two = sc; }});
  }}
  if (s == 3) {{
    {track("Browser .replaceWorld (one)", "first")}
    {track("Browser .replaceWorld (two)", "second")}
  }}
}}""")

assets = {
    "lu-b.wrl": world_b("console"),
    "lu-bh.wrl": world_b("hist"),
    "lu-c.wrl": WORLD_C,
    "lu-b-inline.wrl": WORLD_B_INLINE,
    "lu-inl.wrl": INLINE_LEAF,
    "lu-b-chain.wrl": WORLD_B_CHAIN,
    "lu-bad.wrl": BAD,
}

for name, text in assets.items():
    open(os.path.join(asset_dir, name), "w").write(text)

for name, text in cases.items():
    open(os.path.join(out_dir, name + ".wrl"), "w").write(text)

# `NAME WORLD SCRIPT` lines for cases.txt.
HIST = {"loadurl-fail", "loadurl-fail2", "loadurl-fallback", "loadurl-badscene", "loadurl-twice", "loadurl-then-replace"}

with open("/tmp/lu/cases-lines.txt", "w") as f:
    for name in sorted(cases):
        script = "oracle/synthetic-rea-b09-sai/sai.json" if name in HIST else "oracle/synthetic-rea-b09-sai/sai-console.json"
        f.write(f"{name} oracle/synthetic-rea-b09-sai/{name}.wrl {script}\n")

print(len(cases), "cases,", len(assets), "assets")
