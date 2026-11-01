#!/usr/bin/env python3
"""Issue #95 ShaderPart lane worlds for oracle/synthetic-rea-b24-shaderpart.

  python3 -I oracle/tools/gen_rea_b24_shaderpart.py oracle/synthetic-rea-b24-shaderpart

Stepped worlds (a KeySensor key per step, a Script writes the ShaderPart / ComposedShader fields) for what
the batch 08 shaders lane did not step: url changes of a loaded ShaderPart (MFString fallback, an empty url,
load false/true around a url write, a missing file), vertex-stage ShaderPart url and error text, the
ShaderRegistry includes that need macro or uniform context used without that context, and a shader list whose
first ComposedShader becomes valid and invalid again. Every world and GLSL source is authored here.

A `data:` URL ends at a `#` (it is the fragment), so a preprocessor line in a data URL is written `%23`.
"""
import json, os, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "oracle/synthetic-rea-b24-shaderpart"
os.makedirs(OUT, exist_ok=True)

HEAD = "#X3D V4.0 utf8\nPROFILE Full\n"
worlds, scripts = {}, {}

VS = """attribute vec4 x3d_Vertex;
attribute vec3 x3d_Normal;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
uniform mat3 x3d_NormalMatrix;
varying vec3 vN;
void main()
{
   vN = normalize (x3d_NormalMatrix * x3d_Normal);
   gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;
}
"""

VS300 = """#version 300 es
in vec4 x3d_Vertex;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
void main()
{
   gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;
}
"""


def fs(r, g, b):
    return f"precision mediump float;\nvarying vec3 vN;\nvoid main() {{ gl_FragColor = vec4 (vec3 ({r}, {g}, {b}) * (0.4 + 0.6 * abs (vN.z)), 1.0); }}\n"


def durl(src, kind):
    enc = src.replace("%", "%25").replace("#", "%23")
    assert '"' not in enc and "\\" not in enc and "'" not in enc
    return f"data:x-shader/x-{kind},\n{enc}" if not enc.startswith("%23version") else f"data:x-shader/x-{kind},{enc}"


def jsurl(src, kind="fragment"):
    return durl(src, kind).replace("\n", "%0A")


def world(*parts, vp="Viewpoint { position 0 0 5 }"):
    return HEAD + vp + "\nNavigationInfo { headlight FALSE }\n" + "\n".join(parts) + "\n"


def runtime_json(keys, start=20, step=8):
    frames = start + step * len(keys) + 8
    events = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507},
              {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
    caps = []
    for i, k in enumerate(keys):
        f = start + i * step
        events.append({"frame": f, "type": "key", "key": k})
        caps.append(f + 4)
    return {"settle": True, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames,
            "capture": [12] + caps, "dump": [10] + caps + [frames - 1], "events": events}


def script(nodes, steps):
    """A Script with the nodes `nodes` (name list, DEF'd in the world) and one JS statement per key."""
    decl = "\n".join(f"  initializeOnly SFNode {n.lower()} USE {n}" for n in nodes)
    body = "\n".join(f"   if (k == '{k}') {js}" for k, js in steps)
    return f"""DEF K KeySensor {{ }}
DEF R Script {{
  directOutput TRUE
  inputOnly SFString on_key
{decl}
  url "javascript:
function on_key (k)
{{
{body}
}}
"
}}
ROUTE K.keyPress TO R.on_key"""


def sphere(shaders):
    return "Transform { children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.5 0.3 } shaders [ " + shaders + " ] } geometry Sphere { radius 1.2 } } }"


def setf(node, kind, src):
    return f"{node} .url = new MFString ('{jsurl(src, kind)}');"


GOOD_R, GOOD_G, GOOD_B = fs(1.0, 0.2, 0.2), fs(0.2, 1.0, 0.3), fs(0.2, 0.4, 1.0)
BAD = "precision mediump float;\nvoid main() { gl_FragColor = vec4 (1.0) oops; }\n"
BAD2 = "precision mediump float;\nvoid main() { gl_FragColor = nothing (1.0); }\n"

# ---------------------------------------------------------------------------------------------
# 1. url of a loaded fragment ShaderPart

url_steps = [
    ("a", setf("fp", "fragment", GOOD_B)),
    ("b", f"fp .url = new MFString ('{jsurl(BAD)}', '{jsurl(GOOD_G)}');"),
    ("c", f"fp .url = new MFString ('{jsurl(BAD)}', '{jsurl(BAD2)}');"),
    ("d", "fp .url = new MFString ();"),
    ("e", setf("fp", "fragment", GOOD_G)),
    ("f", f"fp .url = new MFString ('missing-24-part.frag', '{jsurl(GOOD_R)}');"),
    ("g", "fp .url = new MFString ('missing-24-part.frag');"),
    ("h", setf("fp", "fragment", GOOD_R)),
    ("i", "fp .load = false;"),
    ("j", setf("fp", "fragment", GOOD_B)),
    ("k", "fp .load = true;"),
    ("l", "fp .url = fp .url;"),
    ("m", setf("fp", "fragment", GOOD_G)),
    ("n", f"fp .url = new MFString ('{jsurl(BAD)}');"),
    ("o", "fp .load = false;"),
    ("p", "fp .load = true;"),
]
worlds["url-steps.wrl"] = world(
    sphere("DEF CS ComposedShader { language \"GLSL\" parts [ ShaderPart { type \"VERTEX\" url \"" + durl(VS, "vertex") + "\" } DEF FP ShaderPart { type \"FRAGMENT\" url \"" + durl(GOOD_R, "fragment") + "\" } ] }"),
    script(["FP"], url_steps))
scripts["url-steps"] = runtime_json([k for k, _ in url_steps])

# ---------------------------------------------------------------------------------------------
# 2. url of the vertex ShaderPart: errors in the vertex stage, a stage that stops matching the fragment stage

VS_OK2 = VS.replace("gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;", "gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * (x3d_Vertex * vec4 (0.6, 1.0, 1.0, 1.0));")
VS_B = VS.replace("x3d_Vertex;", "x3d_Vertex", 1)  # the semicolon of the attribute declaration is missing
VS_K = VS.replace("void main()", "float f (vec2 a) { return a; }\nvoid main()")
vs_steps = [
    ("a", setf("vp", "vertex", VS_OK2)),
    ("b", setf("vp", "vertex", VS.replace("x3d_Vertex;", "x3d_Vertexx;"))),
    ("c", setf("vp", "vertex", VS)),
    ("d", setf("vp", "vertex", "void main() { }\n")),
    ("e", setf("vp", "vertex", VS.replace("varying vec3 vN;", "varying vec4 vN;").replace("vN = normalize", "vN = vec4 (0.0) + vec4 (normalize").replace("x3d_Normal);", "x3d_Normal), 0.0);"))),
    ("f", setf("vp", "vertex", VS)),
    ("g", "vp .url = new MFString ();"),
    ("h", setf("vp", "vertex", VS_OK2)),
    ("i", setf("vp", "vertex", "precision mediump float;\nvoid main() { gl_FragColor = vec4 (1.0); }\n")),
]
vertex_world = lambda steps: world(
    sphere("ComposedShader { language \"GLSL\" parts [ DEF VP ShaderPart { type \"VERTEX\" url \"" + durl(VS, "vertex") + "\" } ShaderPart { type \"FRAGMENT\" url \"" + durl(GOOD_B, "fragment") + "\" } ] }"),
    script(["VP"], steps))
worlds["vertex-steps.wrl"] = vertex_world(vs_steps)
scripts["vertex-steps"] = runtime_json([k for k, _ in vs_steps])
# KNOWN differences of the Rust log (see rea_b24_shaderpart.rs KNOWN): the position of a syntax error after a line
# end, and a type error inside a function.
vk_steps = [("a", setf("vp", "vertex", VS_B)), ("b", setf("vp", "vertex", VS)), ("c", setf("vp", "vertex", VS_K)), ("d", setf("vp", "vertex", VS))]
worlds["vertex-known.wrl"] = vertex_world(vk_steps)
scripts["vertex-known"] = runtime_json([k for k, _ in vk_steps])

# ---------------------------------------------------------------------------------------------
# 3. ShaderRegistry includes used without the macro / uniform context they need (fragment stage, GLSL ES 3.00)

PRECISION = "precision highp int;\nprecision highp float;\nprecision highp sampler2D;\nprecision highp sampler3D;\nprecision highp samplerCube;\n"
NEEDS_CONTEXT = ["Fog", "Fragment", "Material", "Normal", "PointSize", "Stipple", "Texture", "Unlit", "Vertex", "MaterialInfo", "PBR"]


def inc_src(name, prelude=""):
    return f"#version 300 es\n{prelude}{PRECISION}#include <{name}>\nout vec4 fragColor;\nvoid main() {{ fragColor = vec4 (0.3, 0.7, 0.9, 1.0); }}\n"


FS300_OK = "#version 300 es\nprecision mediump float;\nout vec4 fragColor;\nvoid main() { fragColor = vec4 (0.9, 0.7, 0.2, 1.0); }\n"


def include_world(steps):
    return world(
        sphere("ComposedShader { language \"GLSL\" parts [ ShaderPart { type \"VERTEX\" url \"" + durl(VS300, "vertex") + "\" } DEF FP ShaderPart { type \"FRAGMENT\" url \"" + durl(FS300_OK, "fragment") + "\" } ] }"),
        script(["FP"], steps))


def furl(src):
    return f"fp .url = new MFString ('{jsurl(src, 'fragment')}');"


# one world per include: without its macros the include fails to compile in X_ITE, and the log names the include
for name in NEEDS_CONTEXT:
    steps = [("a", furl(inc_src(name))), ("b", furl(FS300_OK))]
    worlds[f"include-{name}.wrl"] = include_world(steps)
    scripts[f"include-{name}"] = runtime_json([k for k, _ in steps])

# includes that compile: a source without an include, Utils (no context needed), an unknown include (empty file),
# and Fog given the macros it needs
steps = [("a", furl(FS300_OK)), ("b", furl(inc_src("Utils"))), ("c", furl(inc_src("Nope"))),
         ("d", furl(inc_src("Fog", "#define X3D_FOG" + chr(10) + "#define X3D_FOG_LINEAR" + chr(10)))), ("e", furl(FS300_OK))]
worlds["include-steps.wrl"] = include_world(steps)
scripts["include-steps"] = runtime_json([k for k, _ in steps])

# ---------------------------------------------------------------------------------------------
# 4. Shader list: the first ComposedShader is invalid, the second valid; the first becomes valid, invalid, valid.

sel_steps = [
    ("a", setf("f1", "fragment", GOOD_R)),
    ("b", f"f1 .url = new MFString ('{jsurl(BAD)}');"),
    ("c", setf("f1", "fragment", GOOD_B)),
    ("d", "f2 .load = false;"),
    ("e", f"f1 .url = new MFString ('{jsurl(BAD2)}');"),
    ("f", "f2 .load = true;"),
    ("g", "f1 .load = false;"),
    ("h", setf("f1", "fragment", GOOD_R)),
    ("i", "f1 .load = true;"),
]
worlds["select-steps.wrl"] = world(
    sphere("ComposedShader { language \"GLSL\" parts [ ShaderPart { type \"VERTEX\" url \"" + durl(VS, "vertex") + "\" } DEF F1 ShaderPart { type \"FRAGMENT\" url \"" + durl(BAD, "fragment") + "\" } ] } ComposedShader { language \"GLSL\" parts [ ShaderPart { type \"VERTEX\" url \"" + durl(VS, "vertex") + "\" } DEF F2 ShaderPart { type \"FRAGMENT\" url \"" + durl(GOOD_G, "fragment") + "\" } ] }"),
    script(["F1", "F2"], sel_steps))
scripts["select-steps"] = runtime_json([k for k, _ in sel_steps])

for name, text in worlds.items():
    with open(os.path.join(OUT, name), "w") as f:
        f.write(text)

for name, data in scripts.items():
    with open(os.path.join(OUT, name + ".json"), "w") as f:
        json.dump(data, f, indent=1)

print(f"{len(worlds)} worlds, {len(scripts)} scripts -> {OUT}")
