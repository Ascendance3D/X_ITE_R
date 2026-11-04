#!/usr/bin/env python3
"""Generate the ScreenFontStyle worlds oracle/synthetic-rea-q-screen-cff/qs-*.wrl (REA node:FontLibrary, sub:text.fonts).
Authored; synthetic inputs only. The font is the authored OpenType-CFF font of oracle/synthetic-rea-b24-cff (gen_cff_fonts.py): W and V have a
charstring width that differs from their hmtx advance. opentype.js replaces a glyph's advanceWidth by the charstring width when the glyph's path is
first built, ScreenText.build does that for every glyph it draws, and X_ITE builds a Text a second time when its scene goes live (the scene load
waits for the FontStyle's font), so a loaded ScreenText is laid out with the charstring widths. A Script reads origin, textBounds and lineBounds
of every Text after the fonts have arrived and writes them into its `out` field, so the scene text of the dumped frame carries them.

  python3 -I oracle/tools/gen_rea_q_screen_cff.py
"""
import json, os, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import gen_rea_b09_script as G  # noqa: E402

out_dir = os.path.join(here, "..", "synthetic-rea-q-screen-cff")
G.out_dir = out_dir
G.HEAD = "#X3D V4.1 utf8\nPROFILE Full\n"

BASIC = "../synthetic-rea-b24-cff/cff-basic.otf"
READ = 40  # the frame the Script reads the outputs in: every font has arrived long before
cases = []


def strs(strings):
    return " ".join('"%s"' % x for x in strings)


def stext(name, strings, family, extra="", tfields=""):
    """A Text with a ScreenFontStyle."""
    return (f'Shape {{ geometry DEF {name} Text {{ string [ {strs(strings)} ] {tfields} '
            f'fontStyle ScreenFontStyle {{ family {family} {extra} }} }} }}\n')


def ftext(name, strings, family, extra="", tfields=""):
    """A Text with a FontStyle (the polygon text)."""
    return (f'Shape {{ geometry DEF {name} Text {{ string [ {strs(strings)} ] {tfields} '
            f'fontStyle FontStyle {{ family {family} {extra} }} }} }}\n')


def world(name, nodes, texts, steps=None):
    """texts: names of Text nodes; steps: {frame: js} run before the final read."""
    steps = dict(steps or {})
    steps[READ] = steps.get(READ, "") + " ".join(f"RD ('{t}', '');" for t in texts) + " flush_ ();"
    body = "\n".join(f"  if (F_ === {f}) {{ {js} }}" for f, js in sorted(steps.items()))
    pre = ("var F_ = 0;\nfunction N (n) { return Browser .currentScene .getNamedNode (n); }\n"
           "function RD (t, tag) { ['origin', 'textBounds', 'lineBounds'] .forEach (function (f) { T (t + tag + '.' + f, function () { return N (t) [f]; }); }); }\n")
    after = "function prepareEvents () {\n  ++ F_;\n" + body + "\n}\n"
    G.register(name, [], pre=pre, after=after, nodes=nodes)
    cases.append((name, READ + 4))


# The Texts of the library and family-url worlds: the same ScreenFontStyle texts through the two routes. Strings with W and V (charstring width
# differs from hmtx) and without; pointSize 12, 16 and 24; spacing; justify; right to left; vertical; length; maxExtent.
SET = [
    ("T1", ["WVWV"], "pointSize 16", ""),
    ("T2", ["ABAB BA OS", "AB"], "pointSize 16", ""),
    ("T3", ["WVWV", "AWV V"], "pointSize 12", ""),
    ("T4", ["WVWV"], "pointSize 24", ""),
    ("T5", ["VWAB", "WV"], "pointSize 16 spacing 1.5", ""),
    ("T6", ["WV W", "VW"], 'pointSize 16 justify [ "MIDDLE" "MIDDLE" ]', ""),
    ("T7", ["AWV", "VWO"], 'pointSize 20 justify [ "END" "END" ]', ""),
    ("T8", ["WVAB"], "pointSize 16 leftToRight FALSE", ""),
    ("T9", ["WV", "AB"], "pointSize 18 horizontal FALSE", ""),
    ("T10", ["WVWV", "ABOS"], "pointSize 16", "length [ 30 0 ]"),
    ("T11", ["OWVWVS"], "pointSize 16", "maxExtent 3"),
    ("T12", ["HiF", "QÉ"], "pointSize 14", ""),
    # maxExtent: `text ._textBounds` is a Vector2 of doubles, so the ceil of ScreenText.configure sees 10.00000026, not its float32 10.
    ("T13", ["OWVWVS"], "pointSize 16", "maxExtent 10"),
    ("T14", ["OWVWVS"], "pointSize 16", "maxExtent 7.3"),
    ("T15", ["WV", "ABO"], "pointSize 16 horizontal FALSE", "maxExtent 2"),
]


def set_nodes(family):
    return "".join(stext(n, s, family, fx, tf) for n, s, fx, tf in SET)


names = [n for n, *_ in SET]

# 1. FontLibrary: the registered name is the `family` field.
world("qs-library", f'DEF L1 FontLibrary {{ family "CffLib" url "{BASIC}" }}\n' + set_nodes('"CffLib"'), names)

# 2. The deprecated route: the font file named by `family`.
world("qs-family-url", set_nodes(f'"{BASIC}"'), names)

# 3. One font object, several Texts: a FontStyle (polygon) Text and a ScreenFontStyle Text of the same library, and a second library with the same
# file, which is its own font object.
nodes = (
    f'DEF L1 FontLibrary {{ family "CffA" url "{BASIC}" }}\n'
    f'DEF L2 FontLibrary {{ family "CffB" url "{BASIC}" }}\n'
    + ftext("T1", ["WVWV"], '"CffA"', "size 1")
    + stext("T2", ["WVWV"], '"CffA"', "pointSize 16")
    + stext("T3", ["WVWV"], '"CffB"', "pointSize 16")
    + stext("T4", ["WVWV", "VW"], '"CffA"', "pointSize 16 spacing 2")
)
world("qs-shared", nodes, ["T1", "T2", "T3", "T4"])

# 4. Changes after the load: the ScreenFontStyle gets the CFF family at run time (its font arrives after the scene went live, so the first layout
# uses hmtx and keeps it until the Text is laid out again), then string, pointSize and justify change.
nodes = (
    f'DEF L1 FontLibrary {{ family "CffLib" url "{BASIC}" }}\n'
    + stext("T1", ["WVWV"], '"SERIF"', "pointSize 16")
    + stext("T2", ["WVWV", "ABAB"], '"SERIF"', "pointSize 16")
    + stext("T3", ["ABOS"], '"CffLib"', "pointSize 16")
)
world("qs-late", nodes, ["T1", "T2", "T3"], {
    12: "N ('T1') .fontStyle .family = new MFString ('CffLib'); N ('T2') .fontStyle .family = new MFString ('CffLib');",
    16: "RD ('T1', '@16'); RD ('T2', '@16');",
    18: "N ('T1') .string = new MFString ('WVWV', 'VW');",
    22: "RD ('T1', '@22'); RD ('T2', '@22');",
    24: "N ('T2') .fontStyle .pointSize = 24;",
    28: "RD ('T2', '@28');",
    30: "N ('T2') .fontStyle .justify = new MFString ('END', 'MIDDLE');",
})

if __name__ == "__main__":
    sys.argv = [sys.argv[0]] + [n for n, _ in cases]
    os.makedirs(out_dir, exist_ok=True)
    G.main()
    for n, frames in cases:
        json.dump({"size": [512, 512], "t0": 946684800, "dt": 1 / 60, "pinFrameRate": True, "settle": True, "frames": frames, "capture": [],
                   "dump": [frames], "events": []},
                  open(os.path.join(out_dir, n + ".json"), "w"))
