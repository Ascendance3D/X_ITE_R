#!/usr/bin/env python3
"""Generate the Text event worlds oracle/synthetic-rea-b11-text/tb-*.wrl (REA batch 11 lane b11-2-text: node:Text).
Authored; synthetic inputs only. Same idiom as gen_rea_b09_text_cloud.py (probe harness of gen_rea_b09_script.py): every event
of Text's outputs (origin, textBounds, lineBounds) and of its input fields is logged with the frame it reached the Script in.

Callbacks start at frame W = 10, after the oracle's warm-up frames (the default font's arrival frame is Chrome's I/O,
see warm-stamp.wrl and the b11-2-text report). The font family warnings reach the compared sorted log (anim.html).

Only the last frame is dumped: the Script's frame count F_ runs ahead of the harness frame by the frames X_ITE traverses before the
world counts as current (Chrome's load timing), so a per-frame dump of script-changed fields would compare that offset, not Text.

  python3 -I oracle/tools/gen_rea_b11_text.py
"""
import json, os, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import gen_rea_b09_script as G  # noqa: E402

out_dir = os.path.join(here, "..", "synthetic-rea-b11-text")
G.out_dir = out_dir

WATCH = ["origin", "textBounds", "lineBounds", "string", "length", "maxExtent", "fontStyle"]
PRE = """
var F_ = 0, LOG = [];
function watch (name, n) {
  %s .forEach (function (f) { n .addFieldCallback ('w', f, function (v) { LOG .push ('F' + F_ + ' ' + name + '.' + f + '=' + String (v) .replace (/\\\\s+/g, ' ')); }); });
}
""" % repr(WATCH).replace("'", '"')
W = 10
GET = "var t = Browser .currentScene .getNamedNode ('T1');"
NEWFS = " var f = Browser .currentScene .createNode ('FontStyle');"
cases = []


def world(name, scene, steps, last):
    steps = dict(steps)
    steps[W] = "watch ('T1', Browser .currentScene .getNamedNode ('T1'));" + steps.get(W, "")
    body = "\n".join(f"  if (F_ === {f}) {{ {js} }}" for f, js in sorted(steps.items()))
    after = ("function prepareEvents () {\n  ++ F_;\n" + body +
             f"\n  if (F_ === {last}) {{ T ('log', function () {{ return LOG; }}); flush_ (); }}\n}}\n")
    G.register(name, [], pre=PRE, after=after, nodes=scene)
    cases.append((name, last + 4))


SCENE = 'Shape { geometry DEF T1 Text { string [ "ab", "c" ] } }\n'

# A family that does not exist, a list that ends in one that does, an empty list, lower case, and the default again.
world("tb-fallback", SCENE, {
    12: GET + NEWFS + " f .family = new MFString ('NOSUCH'); t .fontStyle = f;",
    16: GET + " t .fontStyle .family = new MFString ('NOSUCH', 'SANS');",
    20: GET + " t .fontStyle .family = new MFString ('NOSUCH2', 'TYPEWRITER'); t .fontStyle .style = 'BOLD';",
    24: GET + " t .fontStyle .family = new MFString ();",
    28: GET + " t .fontStyle .family = new MFString ('sans');",
    32: GET + " t .fontStyle .family = new MFString ('NOSUCH', 'ALSO_MISSING'); t .fontStyle .style = 'NOSUCHSTYLE';",
    36: GET + " t .fontStyle .style = 'ITALIC'; t .fontStyle .family = new MFString ('SERIF');",
}, 42)

# Layout inputs: size, spacing, justify (all 4x4 corners that matter), directions, per-line length, maxExtent, empty lines.
world("tb-layout", 'Shape { geometry DEF T1 Text { string [ "Hello", "wide line here", "", "x" ] } }\n', {
    12: GET + NEWFS + " f .size = 0.5; t .fontStyle = f;",
    14: GET + " t .fontStyle .spacing = 2;",
    16: GET + " t .fontStyle .justify = new MFString ('END');",
    18: GET + " t .fontStyle .justify = new MFString ('MIDDLE', 'BEGIN');",
    20: GET + " t .fontStyle .justify = new MFString ('BEGIN', 'END');",
    22: GET + " t .fontStyle .justify = new MFString ('FIRST', 'FIRST');",
    24: GET + " t .fontStyle .justify = new MFString ();",
    26: GET + " t .length = new MFFloat (1, 0, 3);",
    28: GET + " t .maxExtent = 2;",
    30: GET + " t .maxExtent = 0.5; t .length = new MFFloat ();",
    32: GET + " t .fontStyle .size = 0.25;",
    34: GET + " t .fontStyle .size = -1;",
    36: GET + " t .fontStyle .size = 2; t .fontStyle .spacing = 0.5;",
    38: GET + " t .fontStyle .horizontal = false;",
    40: GET + " t .fontStyle .leftToRight = false;",
    42: GET + " t .fontStyle .topToBottom = false; t .fontStyle .justify = new MFString ('MIDDLE', 'MIDDLE');",
    44: GET + " t .fontStyle .horizontal = true; t .string = new MFString ('A', 'B');",
}, 50)

# X3DFontStyleNode keeps its alignments (set_justify__) from setup and from the last change of `justify`: a later change of
# horizontal / leftToRight / topToBottom moves the glyphs but not the alignment. The FontStyle here is parsed with its flags set.
world("tb-align", 'Shape { geometry DEF T1 Text { string [ "ab", "cdef" ] fontStyle FontStyle { horizontal FALSE leftToRight FALSE } } }\n', {
    12: GET + " t .fontStyle .horizontal = true;",
    14: GET + " t .fontStyle .topToBottom = false;",
    16: GET + " t .fontStyle .leftToRight = true;",
    18: GET + " t .fontStyle .justify = new MFString ('BEGIN');",
    20: GET + " t .fontStyle .horizontal = false;",
    22: GET + " t .fontStyle .leftToRight = false; t .fontStyle .topToBottom = true;",
    24: GET + " t .fontStyle .justify = new MFString ('MIDDLE', 'END');",
    26: GET + " t .fontStyle .horizontal = true; t .fontStyle .leftToRight = true;",
    28: GET + " t .fontStyle .justify = new MFString ('END', 'MIDDLE');",
    30: GET + " t .fontStyle .topToBottom = true; t .fontStyle .horizontal = false;",
    32: GET + " t .fontStyle .justify = new MFString ();",
}, 38)

# Initialization and loading after the warm-up: Texts made at run time (default font cached, then FontStyles of fonts not
# cached yet, a FontStyle shared by two Texts, a Text whose FontStyle is replaced in the frame it is made).
world("tb-late", SCENE, {
    12: "var s = Browser .currentScene .createNode ('Shape'), t2 = Browser .currentScene .createNode ('Text'); t2 .string = new MFString ('q');"
        " watch ('T2', t2); s .geometry = t2; Browser .currentScene .addRootNode (s); Browser .currentScene .updateNamedNode ('T2', t2);",
    16: "var t2 = Browser .currentScene .getNamedNode ('T2'); t2 .maxExtent = 0.5; t2 .length = new MFFloat (0.5);",
    20: "var s = Browser .currentScene .createNode ('Shape'), t3 = Browser .currentScene .createNode ('Text'), f = Browser .currentScene .createNode ('FontStyle');"
        " f .family = new MFString ('TYPEWRITER'); f .style = 'ITALIC'; t3 .fontStyle = f; t3 .string = new MFString ('zz', 'y'); watch ('T3', t3); s .geometry = t3;"
        " Browser .currentScene .addRootNode (s); Browser .currentScene .updateNamedNode ('T3', t3);",
    26: "var t3 = Browser .currentScene .getNamedNode ('T3'), t = Browser .currentScene .getNamedNode ('T1'); t .fontStyle = t3 .fontStyle;",
    30: "var t3 = Browser .currentScene .getNamedNode ('T3'); t3 .fontStyle .size = 3;",
    34: "var t3 = Browser .currentScene .getNamedNode ('T3'), g = Browser .currentScene .createNode ('FontStyle'); g .family = new MFString ('SANS'); g .style = 'BOLD'; t3 .fontStyle = g;"
        " g .size = 2;",
}, 42)

# X_ITE 16.5.0 logs "error: RuntimeError: unreachable" (a WebAssembly trap in its geometry build) when a FontStyle's size is 0, after
# it has sent the same origin / textBounds / lineBounds as for any other size. Generated, never baselined: the engine's trap is not
# a Text output. (tb-layout uses 0.25.)
world("tb-size0-open", 'Shape { geometry DEF T1 Text { string [ "Hello", "x" ] } }\n', {
    12: GET + NEWFS + " f .size = 0.5; t .fontStyle = f;",
    14: GET + " t .fontStyle .size = 0;",
    16: GET + " t .fontStyle .size = 1;",
}, 20)
cases.pop()

if __name__ == "__main__":
    sys.argv = [sys.argv[0]] + [n for n, _ in cases] + ["tb-size0-open"]
    G.main()
    for n, frames in cases + [("tb-size0-open", 24)]:
        json.dump({"size": [512, 512], "t0": 946684800, "dt": 1 / 60, "pinFrameRate": True, "settle": True, "frames": frames, "capture": [],
                   "dump": [frames], "events": []},
                  open(os.path.join(out_dir, n + ".json"), "w"))
