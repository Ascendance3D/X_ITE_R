#!/usr/bin/env python3
"""Generate the Text layout output worlds oracle/synthetic-rea-b09-sai/text-*.wrl (REA batch 09 cloud: node:Text, node:FontStyle).
Authored; synthetic inputs only. Uses the probe harness of gen_rea_b09_script.py.

Every event of Text's outputs (origin, textBounds, lineBounds) and of the inputs that change is logged with the frame it reached
the Script in (the callbacks run during the frame's event processing; `F_` counts prepareEvents calls), so the order of rebuilds
relative to field changes and to the font loads is compared, not only the final values.

  python3 -I oracle/tools/gen_rea_b09_text_cloud.py
"""
import os, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import gen_rea_b09_script as G  # noqa: E402

WATCH = ["origin", "textBounds", "lineBounds", "string", "length", "maxExtent", "fontStyle"]

PRE = """
var F_ = 0, LOG = [];
function watch (name, n) {
  %s .forEach (function (f) { n .addFieldCallback ('w', f, function (v) { LOG .push ('F' + F_ + ' ' + name + '.' + f + '=' + String (v) .replace (/\\s+/g, ' ')); }); });
}
""" % repr(WATCH).replace("'", '"')


def world(name, nodes, steps, last):
    """steps: {frame: js}; the callbacks are added at frame W (after the oracle's warm-up frames, see below); the log is
    written at frame `last`."""
    steps = dict(steps)
    steps[W] = "watch ('T1', Browser .currentScene .getNamedNode ('T1'));" + steps.get(W, "")
    body = "\n".join(f"  if (F_ === {f}) {{ {js} }}" for f, js in sorted(steps.items()))
    after = ("function prepareEvents () {\n  ++ F_;\n" + body +
             f"\n  if (F_ === {last}) {{ T ('log', function () {{ return LOG; }}); flush_ (); }}\n}}\n")
    G.register(name, [], pre=PRE, after=after, nodes=nodes)


# The oracle page (anim.html) loads the world on real animation frames at wall-clock speed, then tops up to the warm-up count
# with frames at the frozen time; Scripts see all of them as prepareEvents. A font the default FontStyle or a FontStyle given in
# those frames loads arrives in whichever of them the wall clock allows (node-text-outputs-open). From frame W on the frames are
# the harness's (virtual clock, `settle`): every event below happens there.
W = 10
SCENE = 'Shape { geometry DEF T1 Text { string "ab" } }\n'
GET = "var t = Browser .currentScene .getNamedNode ('T1');"

# Changes of Text's own fields.
world("text-events", SCENE, {
    12: GET + " t .string = new MFString ('x', 'yy');",
    14: GET + " t .maxExtent = 0.5;",
    16: GET + " t .length = new MFFloat (1, 2);",
    18: GET + " t .string = new MFString ('x', 'yy'); t .maxExtent = 0.5;",
    20: GET + " t .string = new MFString ();",
    22: GET + " t .string = new MFString ('a', '', 'bc');",
}, 25)

# A FontStyle given later, changed, replaced; a Text made later.
world("text-fontstyle", SCENE, {
    12: GET + " var f = Browser .currentScene .createNode ('FontStyle'); f .size = 2; t .fontStyle = f;",
    15: GET + " t .fontStyle .spacing = 1.5;",
    17: GET + " t .fontStyle .justify = new MFString ('MIDDLE', 'MIDDLE');",
    19: GET + " t .fontStyle .horizontal = false;",
    21: GET + " t .fontStyle .leftToRight = false; t .fontStyle .topToBottom = false;",
    23: GET + " t .fontStyle = null;",
    25: "var s = Browser .currentScene .createNode ('Shape'), t2 = Browser .currentScene .createNode ('Text'); t2 .string = new MFString ('q');"
        " watch ('T2', t2); s .geometry = t2; Browser .currentScene .addRootNode (s);",
}, 29)

# Families that need another font file.
world("text-family", SCENE, {
    12: GET + " var f = Browser .currentScene .createNode ('FontStyle'); f .family = new MFString ('SANS'); t .fontStyle = f;",
    20: GET + " t .fontStyle .family = new MFString ('TYPEWRITER'); t .fontStyle .style = 'BOLD';",
    30: GET + " t .fontStyle .family = new MFString ('SANS'); t .fontStyle .style = 'ITALIC';",
}, 40)

# A ScreenFontStyle (ScreenText: the layout in pixels, the box rounded to whole pixels), changed, replaced.
world("text-screen", SCENE, {
    12: GET + " var f = Browser .currentScene .createNode ('ScreenFontStyle'); f .pointSize = 24; t .fontStyle = f;",
    15: GET + " t .fontStyle .spacing = 1.5;",
    17: GET + " t .fontStyle .justify = new MFString ('MIDDLE', 'MIDDLE');",
    19: GET + " t .string = new MFString ('Ab', 'cde');",
    21: GET + " t .fontStyle .horizontal = false;",
    23: GET + " t .fontStyle .leftToRight = false; t .fontStyle .topToBottom = false;",
    25: GET + " t .fontStyle .pointSize = 10;",
    27: GET + " t .fontStyle .family = new MFString ('SANS'); t .fontStyle .style = 'BOLD';",
    32: GET + " t .fontStyle = null;",
}, 36)

if __name__ == "__main__":
    sys.argv = [sys.argv[0]] + [n for n in G.defs if n.startswith("text-")]
    G.main()
    # ScreenFontStyle is a Layout node: text-screen needs the component (the other worlds are the cloud's, unchanged).
    p = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b09-sai", "text-screen.wrl")
    text = open(p).read()
    if "COMPONENT Layout:2" not in text:
        open(p, "w").write(text.replace("COMPONENT Scripting:1\n", "COMPONENT Layout:2\nCOMPONENT Scripting:1\n", 1))
