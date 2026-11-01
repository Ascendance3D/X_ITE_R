#!/usr/bin/env python3
"""Authors the synthetic OpenType-CFF test fonts of oracle/synthetic-rea-b24-cff (REA issue 74).

Every outline is drawn here: no third-party font data. Usage: gen_cff_fonts.py OUTPUT_DIR
Needs fontTools (tested with 4.46). The output is deterministic (fixed timestamps).

Writes cff-basic.otf (OpenType-CFF, OTTO) and cff-bare.cff (the same CFF table, no SFNT wrapper).
"""
import os
import sys
from types import SimpleNamespace

from fontTools.cffLib import SubrsIndex
from fontTools.feaLib.builder import addOpenTypeFeaturesFromString
from fontTools.fontBuilder import FontBuilder
from fontTools.misc.psCharStrings import T2CharString
from fontTools.pens.t2CharStringPen import T2CharStringPen

UPM = 1000
PRIV = SimpleNamespace(defaultWidthX=400, nominalWidthX=600)


def pen_glyph(width, draw, tolerance=0.5):
    # The charstring stores the width minus nominalWidthX, or nothing when it is defaultWidthX.
    stored = None if width == PRIV.defaultWidthX else width - PRIV.nominalWidthX
    pen = T2CharStringPen(stored, None, roundTolerance=tolerance)
    draw(pen)
    return pen.getCharString(private=PRIV)


def poly(pen, pts):
    pen.moveTo(pts[0])
    for p in pts[1:]:
        pen.lineTo(p)
    pen.closePath()


def circle(pen, cx, cy, r, ccw=True):
    k = 0.5523 * r
    pen.moveTo((cx + r, cy))
    if ccw:
        pen.curveTo((cx + r, cy + k), (cx + k, cy + r), (cx, cy + r))
        pen.curveTo((cx - k, cy + r), (cx - r, cy + k), (cx - r, cy))
        pen.curveTo((cx - r, cy - k), (cx - k, cy - r), (cx, cy - r))
        pen.curveTo((cx + k, cy - r), (cx + r, cy - k), (cx + r, cy))
    else:
        pen.curveTo((cx + r, cy - k), (cx + k, cy - r), (cx, cy - r))
        pen.curveTo((cx - k, cy - r), (cx - r, cy - k), (cx - r, cy))
        pen.curveTo((cx - r, cy + k), (cx - k, cy + r), (cx, cy + r))
        pen.curveTo((cx + k, cy + r), (cx + r, cy + k), (cx + r, cy))
    pen.closePath()


def notdef(p):
    poly(p, [(50, 0), (50, 700), (450, 700), (450, 0)])
    poly(p, [(100, 50), (400, 50), (400, 650), (100, 650)])


def glyph_a(p):  # lines only, with a hole
    poly(p, [(0, 0), (300, 700), (600, 0), (480, 0), (300, 480), (120, 0)])
    poly(p, [(200, 180), (400, 180), (350, 280), (250, 280)])


def glyph_b(p):  # cubic curves and a counter
    p.moveTo((50, 0))
    p.lineTo((50, 700))
    p.lineTo((300, 700))
    p.curveTo((450, 700), (450, 400), (300, 380))
    p.curveTo((520, 350), (520, 0), (300, 0))
    p.closePath()
    poly(p, [(130, 80), (280, 80), (280, 300), (130, 300)])


def glyph_o(p):  # four-arc ring: outer ccw, inner cw
    circle(p, 300, 350, 300, True)
    circle(p, 300, 350, 180, False)


def glyph_s(p):  # quadratic segments, converted to cubic by the pen
    p.moveTo((40, 40))
    p.qCurveTo((300, -60), (560, 200))
    p.qCurveTo((300, 330), (560, 560))
    p.lineTo((560, 640))
    p.qCurveTo((300, 760), (40, 520))
    p.closePath()


def glyph_i(p):  # stem and dot: two contours
    poly(p, [(100, 0), (100, 450), (220, 450), (220, 0)])
    poly(p, [(100, 520), (100, 640), (220, 640), (220, 520)])


def glyph_frac(p):  # fractional coordinates (16.16 numbers)
    p.moveTo((10.5, 0.25))
    p.lineTo((10.5, 600.75))
    p.lineTo((350.125, 600.75))
    p.closePath()


def main(out):
    os.makedirs(out, exist_ok=True)
    names = [".notdef", "A", "B", "O", "S", "i", "space", "H", "F", "Eacute", "Q", "frac", "W", "V"]
    cmap = {0x41: "A", 0x42: "B", 0x4F: "O", 0x53: "S", 0x69: "i", 0x20: "space", 0x48: "H",
            0x46: "F", 0xC9: "Eacute", 0x51: "Q", 0x66: "frac", 0x57: "W", 0x56: "V"}
    adv = {".notdef": 500, "A": 640, "B": 560, "O": 600, "S": 600, "i": 320, "space": 300,
           "H": 700, "F": 500, "Eacute": 560, "Q": 600, "frac": 400, "W": 500, "V": 480}
    cs = {
        ".notdef": pen_glyph(500, notdef),
        "A": pen_glyph(640, glyph_a),
        "B": pen_glyph(560, glyph_b),
        "O": pen_glyph(600, glyph_o),
        "S": pen_glyph(600, glyph_s),
        "i": pen_glyph(320, glyph_i),
        "space": pen_glyph(300, lambda p: None),
        "frac": pen_glyph(400, glyph_frac, tolerance=0),
        # W: the charstring says 650, hmtx says 500 (opentype.js takes the charstring width once the
        # glyph's path has been built).
        "W": pen_glyph(650, lambda p: poly(p, [(50, 0), (50, 500), (450, 500), (450, 0)])),
    }

    def hand(program):
        return T2CharString(program=program, private=PRIV)

    # H: hstem + vstem + hintmask, hlineto alternation, an implicit close, vmoveto, rlineto.
    # width 700 is stored as 700 - nominalWidthX(600) = 100 before the first stem operator.
    cs["H"] = hand([
        100, 0, 100, 600, 100, "hstem", 0, 100, 500, 100, "vstem",
        "hintmask", b"\xf0",
        0, 0, "rmoveto",
        100, 300, 400, -300, 100, 700, -100, -300, -400, 300, -100, "hlineto",
        100, "vmoveto",
        100, 0, 0, 100, -100, 0, "rlineto",
        "endchar",
    ])
    # F: width 500 stored as -100; main calls local subr 0 and global subr 0 (bias 107).
    cs["F"] = hand([-100, 0, 0, "rmoveto", -107, "callsubr", -107, "callgsubr", "endchar"])
    # Eacute: endchar with four arguments (adx ady bchar achar): base 'B' (66), accent 'S' (83).
    cs["Eacute"] = hand([-40, 100, 120, 66, 83, "endchar"])
    # V: hstem without a width argument: the charstring width is defaultWidthX (400), hmtx says 480.
    cs["V"] = hand([0, 100, "hstem", 50, 0, "rmoveto", 200, 0, 0, 500, -200, 0, "rlineto", "endchar"])
    # Q: rmoveto, hflex, hflex1, flex1, flex, closed by endchar.
    cs["Q"] = hand([
        0, 50, 50, "rmoveto",
        100, 100, 50, 100, 100, 100, 100, "hflex",
        50, -20, 50, 40, 50, 50, 50, -20, 50, "hflex1",
        30, 10, 30, 10, 30, 10, -30, -10, -30, -10, -30, "flex1",
        20, 20, 20, 20, 20, 20, -20, -20, -20, -20, -20, -20, 50, "flex",
        "endchar",
    ])

    fb = FontBuilder(UPM, isTTF=False)
    fb.setupGlyphOrder(names)
    fb.setupCharacterMap(cmap)
    fb.setupCFF("SyntheticCff-Regular", {"FullName": "Synthetic CFF Regular"}, cs,
                {"defaultWidthX": 400, "nominalWidthX": 600})
    fb.setupHorizontalMetrics({n: (adv[n], 0) for n in names})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable({"familyName": "Synthetic CFF", "styleName": "Regular"})
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=900, usWinDescent=250)
    fb.setupPost()
    fb.font["head"].created = 3600000000
    fb.font["head"].modified = 3600000000

    top = fb.font["CFF "].cff.topDictIndex[0]
    cff = fb.font["CFF "].cff
    priv = top.Private

    def sub(program):
        t = T2CharString(program=program, private=priv, globalSubrs=cff.GlobalSubrs)
        return t

    priv.Subrs = SubrsIndex()
    priv.Subrs.append(sub([100, 300, 300, 100, -300, 200, "hlineto", "return"]))
    cff.GlobalSubrs.append(sub([400, 100, -500, "hlineto", "return"]))
    for name in names:
        c = top.CharStrings[name]
        c.private = priv
        c.globalSubrs = cff.GlobalSubrs

    addOpenTypeFeaturesFromString(
        fb.font, "languagesystem DFLT dflt; feature kern { pos A B -60; pos B A -35; pos O S 40; } kern;")

    fb.save(os.path.join(out, "cff-basic.otf"))
    raw = fb.font.getTableData("CFF ")
    with open(os.path.join(out, "cff-bare.cff"), "wb") as f:
        f.write(raw)


if __name__ == "__main__":
    main(sys.argv[1])
