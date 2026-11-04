#!/usr/bin/env python3
"""Generate the end-to-end CID-keyed CFF and CFF2 worlds oracle/synthetic-rea-t-cff-cid/tc-*.wrl (REA node:FontLibrary, sub:text.fonts).
Authored; synthetic inputs only. The fonts are those of oracle/tools/gen_cff_cid_fonts.py (run it first: the worlds name them). As in
oracle/tools/gen_rea_p_cff_runtime.py, a Script reads origin, textBounds and lineBounds of every Text after the fonts have arrived and
writes them into its `out` field, so the scene text of the dumped frame carries them. The runtime, not Font::parse, is what runs.

  python3 -I oracle/tools/gen_rea_t_cff_cid.py
"""
import json, os, sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
import gen_rea_b09_script as G  # noqa: E402

out_dir = os.path.join(here, "..", "synthetic-rea-t-cff-cid")
G.out_dir = out_dir
G.HEAD = "#X3D V4.1 utf8\nPROFILE Full\n"

READ = 40  # the frame the Script reads the outputs in: every font has arrived long before
cases = []


def fs(family, extra=""):
    return f'FontStyle {{ family {family} {extra} }}'


def text(name, strings, family, extra="", tfields=""):
    s = " ".join('"%s"' % x for x in strings)
    return f'Shape {{ geometry DEF {name} Text {{ string [ {s} ] {tfields} fontStyle {fs(family, extra)} }} }}\n'


def world(name, nodes, texts):
    body = "  if (F_ === %d) { %s flush_ (); }" % (READ, " ".join(f"RD ('{t}');" for t in texts))
    pre = ("var F_ = 0;\nfunction N (n) { return Browser .currentScene .getNamedNode (n); }\n"
           "function RD (t) { ['origin', 'textBounds', 'lineBounds'] .forEach (function (f) { T (t + '.' + f, function () { return N (t) [f]; }); }); }\n")
    after = "function prepareEvents () {\n  ++ F_;\n" + body + "\n}\n"
    G.register(name, [], pre=pre, after=after, nodes=nodes)
    cases.append((name, READ + 4))


def lib(fam, font):
    return f'DEF L_{fam} FontLibrary {{ family "{fam}" url "{font}.otf" }}\n'


# 1. CID-keyed CFF through FontLibrary: three FDs with their own Private DICT widths and local subrs (cid-fd3, FDSelect 3), and two FDs, one
# without a Private DICT (cid-fd0, FDSelect 0). The advances come from the charstrings once a glyph's path exists (hmtx before), per FD.
LAY = [
    (["AB OH", "FWV"], "", ""),
    (["BOHF", "WV AB"], "justify [ \"MIDDLE\" \"MIDDLE\" ]", ""),
    (["ABOH", "FWV"], "size 1.5 spacing 0.7", ""),
    (["ABO", "HFWV"], "horizontal FALSE size 0.9", ""),
    (["ABO", "HFWV"], "leftToRight FALSE", ""),
    (["ABOH FWV"], "", "maxExtent 3"),
    (["ABOH", "FWV"], "", "length [ 3 2 ]"),
    (["AZB"], "", ""),
]


def layouts(prefix, fam):
    out, names = "", []
    for i, (s, f, t) in enumerate(LAY, 1):
        out += text(f"{prefix}{i}", s, f'"{fam}"', f, t)
        names.append(f"{prefix}{i}")
    return out, names


n1, t1 = layouts("A", "CidFd3")
n2, t2 = layouts("B", "CidFd0")
world("tc-cid", lib("CidFd3", "cid-fd3") + lib("CidFd0", "cid-fd0") + n1 + n2, t1 + t2)

# 2. CFF2 through FontLibrary: FDSelect 3 and 4, one FD without FDSelect, a VariationStore with `blend`; the hmtx advance stays (CFF2
# charstrings carry no width).
n1, t1 = layouts("A", "C2Fd3")
n2, t2 = layouts("B", "C2Fd4")
n3, t3 = layouts("C", "C2One")
world("tc-cff2", lib("C2Fd3", "cff2-fd3") + lib("C2Fd4", "cff2-fd4") + lib("C2One", "cff2-single") + n1 + n2 + n3, t1 + t2 + t3)

# 3. The deprecated route: the font file named by `family`, and a list that falls through to it.
nodes = (
    text("T1", ["ABOH", "FWV"], '"cid-fd3.otf"')
    + text("T2", ["ABOH", "FWV"], '"cid-fd0.otf"', "size 2")
    + text("T3", ["ABOH", "FWV"], '[ "NOSUCH" "cff2-fd3.otf" ]')
    + text("T4", ["AB OH"], '"cff2-single.otf"', "spacing 1.5")
)
world("tc-family-url", nodes, ["T1", "T2", "T3", "T4"])

# 4. Fonts opentype.js rejects while it parses the CFF table, each with its own message (the library registers nothing, the family falls back
# to the default font), and a CFF2 font whose charstrings use `return` and `endchar` (opentype.js logs each use and goes on).
bad = ["cid-bad-fdindex", "cid-no-fdselect", "cid-bad-first", "cff2-no-fdarray", "cff2-legacy-ops"]
nodes = "".join(lib("Bad%d" % i, f) for i, f in enumerate(bad)) + "".join(
    text(f"T{i}", ["ABOH", "FWV"], f'"Bad{i}"') for i in range(len(bad)))
world("tc-bad", nodes, [f"T{i}" for i in range(len(bad))])

# 4b. A CFF2 font without a VariationStore parses, but every glyph path throws a TypeError, and the load of the whole world never completes in X_ITE
# (the recording times out). Not compared; see KNOWN below and crates/x3d-viewer/tests/rea_t_cff_cid.rs.
KNOWN = []
world("tc-novstore", lib("NoVs", "cff2-novstore") + text("T0", ["ABOH", "FWV"], '"NoVs"'), ["T0"])
KNOWN.append(cases.pop())

# 5. ScreenFontStyle (ScreenText) with a CID-keyed and a CFF2 family: the layout in scene units from the charstring advances (the canvas raster is not
# compared).
nodes = (
    lib("CidFd3", "cid-fd3") + lib("C2Fd3", "cff2-fd3")
    + "".join(
        f'Shape {{ geometry DEF T{i} Text {{ string [ {s} ] {tf} fontStyle ScreenFontStyle {{ family "{fam}" {fx} }} }} }}\n'
        for i, (fam, s, fx, tf) in enumerate([
            ("CidFd3", '"ABOH" "FWV"', "pointSize 24", ""),
            ("CidFd3", '"BOHF" "WV AB"', 'pointSize 20 justify [ "MIDDLE" "MIDDLE" ]', ""),
            ("CidFd3", '"ABO" "HFWV"', "pointSize 12 leftToRight FALSE", ""),
            ("C2Fd3", '"ABOH" "FWV"', "pointSize 24", ""),
            ("C2Fd3", '"ABOH FWV"', "pointSize 16", "maxExtent 0.5"),
            ("C2Fd3", '"ABO" "HFWV"', "pointSize 18 horizontal FALSE", ""),
        ], 1)
    )
)
world("tc-screen", nodes, [f"T{i}" for i in range(1, 7)])

# 6. Changes after the load: a CID family and a CFF2 family arrive at run time, the library family changes, size / spacing change.
nodes = (
    lib("CidFd3", "cid-fd3") + lib("C2Fd3", "cff2-fd3")
    + text("T1", ["ABOH FWV", "AB"], '"SERIF"')
    + text("T2", ["ABOH"], '"C2Fd3"')
)
G_steps = {
    12: "N ('T1') .fontStyle .family = new MFString ('CidFd3');",
    16: "N ('T1') .fontStyle .size = 2; N ('T1') .fontStyle .spacing = 1.5;",
    20: "N ('T2') .fontStyle .family = new MFString ('CidFd3'); N ('T1') .string = new MFString ('HF', 'W', '', 'V');",
    24: "N ('T2') .fontStyle .family = new MFString ('C2Fd3');",
}


def dynamic():
    steps = dict(G_steps)
    reads = {14: ("T1", "@14"), 18: ("T1", "@18"), 22: ("T1", "@22"), 26: ("T2", "@26")}
    for f, (t, tag) in reads.items():
        steps[f] = steps.get(f, "") + f" RD ('{t}', '{tag}');"
    steps[READ] = " ".join(f"RD ('{t}', '');" for t in ("T1", "T2")) + " flush_ ();"
    body = "\n".join(f"  if (F_ === {f}) {{ {js} }}" for f, js in sorted(steps.items()))
    pre = ("var F_ = 0;\nfunction N (n) { return Browser .currentScene .getNamedNode (n); }\n"
           "function RD (t, tag) { ['origin', 'textBounds', 'lineBounds'] .forEach (function (f) { T (t + tag + '.' + f, function () { return N (t) [f]; }); }); }\n")
    after = "function prepareEvents () {\n  ++ F_;\n" + body + "\n}\n"
    G.register("tc-dynamic", [], pre=pre, after=after, nodes=nodes)
    cases.append(("tc-dynamic", READ + 4))


dynamic()

if __name__ == "__main__":
    sys.argv = [sys.argv[0]] + [n for n, _ in cases + KNOWN]
    os.makedirs(out_dir, exist_ok=True)
    G.main()
    for n, frames in cases + KNOWN:
        json.dump({"size": [512, 512], "t0": 946684800, "dt": 1 / 60, "pinFrameRate": True, "settle": True, "frames": frames, "capture": [],
                   "dump": [frames], "events": []},
                  open(os.path.join(out_dir, n + ".json"), "w"))
