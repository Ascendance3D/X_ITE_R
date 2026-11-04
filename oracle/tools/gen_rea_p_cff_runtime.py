#!/usr/bin/env python3
"""Generate the end-to-end CFF font worlds oracle/synthetic-rea-p-cff-runtime/pc-*.wrl (REA node:FontLibrary, sub:text.fonts).
Authored; synthetic inputs only. The font is the authored OpenType-CFF font of oracle/synthetic-rea-b24-cff (gen_cff_fonts.py), and the CID-keyed
variant of oracle/synthetic-rea-p-cff-runtime (gen_cff_cid_font.py). A Script reads the output events of every Text (origin, textBounds,
lineBounds) after the fonts have arrived, and writes them into its `out` field, so the scene text of
the dumped frame carries them. The runtime, not Font::parse, is what runs: URL resolution, FileLoader, FontLibrary registration,
FontStyle lookup, Text layout.

  python3 -I oracle/tools/gen_rea_p_cff_runtime.py
"""
import json, os, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import gen_rea_b09_script as G  # noqa: E402

out_dir = os.path.join(here, "..", "synthetic-rea-p-cff-runtime")
G.out_dir = out_dir
G.HEAD = "#X3D V4.1 utf8\nPROFILE Full\n"

BASIC = "../synthetic-rea-b24-cff/cff-basic.otf"
BARE = "../synthetic-rea-b24-cff/cff-bare.cff"
CID = "cff-cid.otf"
READ = 40  # the frame the Script reads the outputs in: every font has arrived long before
cases = []


def fs(family, extra=""):
    return f'FontStyle {{ family {family} {extra} }}'


def text(name, strings, family, extra="", tfields=""):
    s = " ".join('"%s"' % x for x in strings)
    return f'Shape {{ geometry DEF {name} Text {{ string [ {s} ] {tfields} fontStyle {fs(family, extra)} }} }}\n'


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


# 1. FontLibrary with the CFF font: the registered name is the `family` field, the FontStyle lookup is case insensitive. The layouts of the
# b24 baseline (kerning pairs, the opentype.js advance quirk, justify, size and spacing, vertical, right to left, length, maxExtent, .notdef).
nodes = (
    f'DEF L1 FontLibrary {{ family "CffLib" url "{BASIC}" }}\n'
    + text("T1", ["ABAB BA OS", "AB"], '"CffLib"')
    + text("T2", ["WVWV", "AWV V"], '"cfflib"')
    + text("T3", ["AB OS", "HiF", "QÉ"], '"CffLib"', 'justify [ "MIDDLE" "MIDDLE" ]')
    + text("T4", ["ABO", "SiH"], '"CffLib"', "size 1.5 spacing 0.7")
    + text("T5", ["ABO", "SiH"], '"CffLib"', "horizontal FALSE size 0.9")
    + text("T6", ["ABO", "SiHW"], '"CffLib"', "leftToRight FALSE")
    + text("T7", ["ABOS", "WV"], '"CffLib"', "", "length [ 3 1 ]")
    + text("T8", ["ABOS HFQ"], '"CffLib"', "", "maxExtent 4")
    + text("T9", ["AZЖB"], '"CffLib"')
    + text("T10", ["ABOS", "HFQ"], '"CffLib"', "horizontal FALSE", "maxExtent 2")
    + text("T11", ["ABOS HFQ", "WV"], '"CffLib"', "", "maxExtent 4.5 length [ 0 2 ]")
)
world("pc-library", nodes, [f"T{i}" for i in range(1, 12)])

# 2. The deprecated route: the font file named by `family` (a url relative to the FontStyle's world), and a list that falls through to it.
nodes = (
    text("T1", ["ABAB BA OS", "AB"], f'"{BASIC}"')
    + text("T2", ["WVWV", "AWV V"], f'[ "NOSUCH" "{BASIC}" ]', "size 2")
    + text("T3", ["ABO"], f'"{BARE}"')
)
world("pc-family-url", nodes, ["T1", "T2", "T3"])

# 3. Failing and passing urls: the bare CFF is not a font (opentype.js rejects the signature), so a FontLibrary whose list ends in the OpenType
# font takes that one; a library with only the bare CFF registers nothing and its family falls back to the default font. Family lists.
nodes = (
    f'DEF L1 FontLibrary {{ family "BareFirst" url [ "{BARE}" "{BASIC}" ] }}\n'
    f'DEF L2 FontLibrary {{ family "BareOnly" url "{BARE}" }}\n'
    f'DEF L3 FontLibrary {{ family "Missing" url "../synthetic-rea-b24-cff/nofont.otf" }}\n'
    + text("T1", ["ABAB BA OS"], '"BareFirst"')
    + text("T2", ["ABAB"], '[ "BareOnly" "CffLib2" ]')
    + text("T3", ["ABAB"], '[ "Missing" "BareFirst" ]', "size 0.5")
    + text("T4", ["ABAB"], '"SERIF"')
)
world("pc-urls", nodes, ["T1", "T2", "T3", "T4"])

# 4. Changes after the load: the FontStyle gets the CFF family at run time, the library family is changed (X_ITE keeps the old registration), a
# second library is added, size / spacing / string change.
nodes = (
    f'DEF L1 FontLibrary {{ family "CffLib" url "{BASIC}" }}\n'
    + text("T1", ["ABAB BA OS", "AB"], '"SERIF"')
    + text("T2", ["WVWV"], '"CffLib"')
)
world("pc-dynamic", nodes, ["T1", "T2"], {
    12: "N ('T1') .fontStyle .family = new MFString ('CffLib');",
    14: "RD ('T1', '@14');",
    16: "N ('T1') .fontStyle .size = 2; N ('T1') .fontStyle .spacing = 1.5;",
    18: "RD ('T1', '@18');",
    20: "N ('T1') .string = new MFString ('HiF', 'QQ', '', 'S');",
    22: "RD ('T1', '@22');",
    24: "N ('L1') .family = 'CffLib3'; N ('T2') .fontStyle .family = new MFString ('CffLib3');",
    26: "RD ('T2', '@26');",
    28: "N ('T2') .fontStyle .family = new MFString ('CffLib');",
    30: "RD ('T2', '@30');",
    32: "N ('T1') .fontStyle .justify = new MFString ('END', 'MIDDLE'); N ('T1') .maxExtent = 3;",
})

# 5. ScreenFontStyle (ScreenText): the layout in scene units from the CFF font's advances (the canvas raster is not compared).
nodes = (
    f'DEF L1 FontLibrary {{ family "CffLib" url "{BASIC}" }}\n'
    + "".join(
        f'Shape {{ geometry DEF T{i} Text {{ string [ {s} ] {tf} fontStyle ScreenFontStyle {{ family "CffLib" {fx} }} }} }}\n'
        for i, (s, fx, tf) in enumerate([
            ('"ABAB BA OS" "AB"', "pointSize 24", ""),
            ('"HiFQ" "AB O"', "pointSize 16 spacing 1.5", ""),
            ('"AB OS" "HiF"', 'pointSize 20 justify [ "MIDDLE" "MIDDLE" ]', ""),
            ('"ABO" "SiHF"', "pointSize 12 leftToRight FALSE", ""),
            ('"ABOS HFQ"', "pointSize 24", "maxExtent 0.5"),
            ('"ABO" "SiH"', "pointSize 18 horizontal FALSE", ""),
        ], 1)
    )
)
world("pc-screen", nodes, [f"T{i}" for i in range(1, 7)])

# Open, generated and never baselined: the same ScreenFontStyle with W and V, the glyphs whose charstring width differs from hmtx (the
# opentype.js advance quirk, see gen_cff_fonts.py). X_ITE lays "WVWV" out with the charstring widths (textBounds 38 1, lineBounds 36.26667) because the
# glyph paths exist from an earlier build; the Rust layout, run once with the library font, still uses hmtx (33 1, 31.57333).
nodes = (
    f'DEF L1 FontLibrary {{ family "CffLib" url "{BASIC}" }}\n'
    'Shape { geometry DEF T1 Text { string [ "WVWV" ] fontStyle ScreenFontStyle { family "CffLib" pointSize 16 } } }\n'
)
world("pc-screen-quirk-open", nodes, ["T1"])
cases.pop()
OPEN = [("pc-screen-quirk-open", READ + 4)]

if __name__ == "__main__":
    sys.argv = [sys.argv[0]] + [n for n, _ in cases + OPEN]
    os.makedirs(out_dir, exist_ok=True)
    G.main()
    for n, frames in cases + OPEN:
        json.dump({"size": [512, 512], "t0": 946684800, "dt": 1 / 60, "pinFrameRate": True, "settle": True, "frames": frames, "capture": [],
                   "dump": [frames], "events": []},
                  open(os.path.join(out_dir, n + ".json"), "w"))
