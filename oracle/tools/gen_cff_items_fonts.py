#!/usr/bin/env python3
"""Authors the synthetic CFF / CFF2 test fonts of oracle/synthetic-rea-t-cff-items (REA node:FontLibrary, sub:text.fonts, the UNVERIFIED
items of the CFF lane). Every outline and every byte of the CFF / CFF2 / fvar / avar / HVAR tables is written here: no third-party font data.
fontTools (tested with 4.46) writes only the SFNT shell (head, hhea, maxp, hmtx, cmap, name, OS/2, post); the output is deterministic.
The helpers (charstring and INDEX / DICT writers) come from oracle/tools/gen_cff_cid_fonts.py.

  python3 -I oracle/tools/gen_cff_items_fonts.py [OUTPUT_DIR]      (default oracle/synthetic-rea-t-cff-items)

Fonts (units per em 1000; every glyph has the hmtx advance 600 unless it says otherwise):
  bias-1239 bias-1240 bias-33899 bias-33900   plain CFF with N local and N global subrs (N in the name). Subr k draws a line whose size depends on k,
        local and global differ. Glyph A calls local subrs 0 and N-1, B global 0 and N-1, O the local and global subr N/2, H calls subr number N
        (out of range: nothing is drawn). The Type 2 bias is 107 below 1240 subrs, 1131 below 33900, 32768 from there (TN5177 section 4.7).
  depth               plain CFF, local and global chains of 13 subrs (each draws a line and calls the next) and subrs that call themselves.
        A..E start a local chain of 9, 10, 11, 12, 13 subrs; F, G a global chain of 10 and 11; H, I a chain that alternates local and global of
        10 and 11 subrs; O and V call themselves (local, global). The Type 2 limit is a nesting depth of 10 (TN5177 appendix B).
  seac-plain seac-cid seac-cid-fd   endchar with four arguments (adx ady bchar achar, the deprecated `seac`): plain CFF, CID-keyed CFF with one
        FD, and CID-keyed CFF with three FDs. C = A + B shifted, D has a leading width argument, E names a glyph the font does not have.
  var-default var-regular var-avar var-2axes  CFF2 with an fvar table (and avar, HVAR) and `blend`: default coordinates without a named
        instance; a named instance "Regular" at a non-default coordinate; the same with an avar map; two axes, two regions and an HVAR table.
"""
import os
import struct
import sys

here = os.path.dirname(os.path.abspath(__file__))
sys.path.insert(0, here)
from fontTools.fontBuilder import FontBuilder  # noqa: E402
from fontTools.ttLib import newTable  # noqa: E402
from fontTools.ttLib.tables.DefaultTable import DefaultTable  # noqa: E402

import gen_cff_cid_fonts as B  # noqa: E402
from gen_cff_cid_fonts import cs, index, dict_int, dict_small, dict_op, fdselect3  # noqa: E402

STD_SID = {"space": 1, **{chr(65 + i): 34 + i for i in range(26)}}


def sub_local(k):
    return [(k % 89) + 11, (k * 7) % 61 + 5, "rlineto", "return"]


def sub_global(k):
    return [-((k % 83) + 13), (k * 5) % 53 + 7, "rlineto", "return"]


def build_cff1(glyphs, lsubrs, gsubrs, cid=None):
    """glyphs: [(name, program)] with .notdef first. cid: None for a plain font, else [(cid, fd)] per glyph (FDs: 1 or 3 identical-shaped FDs)."""
    n = len(glyphs)
    strings = [b"Adobe", b"Identity"] if cid else []
    sids = []
    for name, _p in glyphs[1:]:
        if name in STD_SID:
            sids.append(STD_SID[name])
        else:
            strings.append(name.encode())
            sids.append(390 + len(strings))
    charstrings = [cs(p) for _n, p in glyphs]
    cs_idx = index(charstrings)
    gsub_idx = index([cs(s) for s in gsubrs])
    if cid:
        charset = b"\x00" + b"".join(struct.pack(">H", c) for c, _fd in cid[1:])
        ranges, prev = [], None
        for gid, (_c, fd) in enumerate(cid):
            if fd != prev:
                ranges.append((gid, fd))
                prev = fd
        fdsel = fdselect3(ranges, n)
        nfd = max(fd for _c, fd in cid) + 1
    else:
        charset = b"\x00" + b"".join(struct.pack(">H", s) for s in sids)
        fdsel, nfd = b"", 1
    name_idx = index([b"SynthItems"])
    str_idx = index(strings)
    header = bytes([1, 0, 4, 4])

    def top(charset_off, fdsel_off, cs_off, fda_off, priv_size, priv_off):
        t = b""
        if cid:
            t += b"".join(dict_small(v) for v in (391, 392, 0)) + dict_op(1230) + dict_small(n) + dict_op(1234)
        t += dict_int(charset_off) + dict_op(15)
        if cid:
            t += dict_int(fdsel_off) + dict_op(1237)
        t += dict_int(cs_off) + dict_op(17)
        if cid:
            t += dict_int(fda_off) + dict_op(1236)
        else:
            t += dict_int(priv_size) + dict_int(priv_off) + dict_op(18)
        return t

    top_len = len(index([top(0, 0, 0, 0, 0, 0)]))
    pos = len(header) + len(name_idx) + top_len + len(str_idx) + len(gsub_idx)
    charset_off = pos
    pos += len(charset)
    fdsel_off = pos
    pos += len(fdsel)
    cs_off = pos
    pos += len(cs_idx)
    fda_off = pos
    fd_dict_len = len(dict_int(0) * 2 + dict_op(18))
    fda_len = len(index([b"\x00" * fd_dict_len] * nfd)) if cid else 0
    pos += fda_len
    sub_idx = index([cs(s) for s in lsubrs])
    priv = dict_int(5 + 1) + dict_op(19)  # Subrs: right behind the Private DICT
    priv_block = priv + sub_idx
    fda = b""
    if cid:
        fdicts = []
        privs = b""
        for _fd in range(nfd):
            fdicts.append(dict_int(len(priv)) + dict_int(pos + len(privs)) + dict_op(18))
            privs += priv_block
        fda = index(fdicts)
        assert len(fda) == fda_len
        tail = fda + privs
        topd = index([top(charset_off, fdsel_off, cs_off, fda_off, 0, 0)])
    else:
        tail = priv_block
        topd = index([top(charset_off, fdsel_off, cs_off, fda_off, len(priv), pos)])
    assert len(topd) == top_len
    return header + name_idx + topd + str_idx + gsub_idx + charset + fdsel + cs_idx + tail


def shell(tag, data, path, glyphs, extra=None, names=None):
    """glyphs: [(name, char or None, advance)]."""
    fb = FontBuilder(1000, isTTF=False)
    fb.setupGlyphOrder([g[0] for g in glyphs])
    fb.setupCharacterMap({g[1]: g[0] for g in glyphs if g[1] is not None})
    fb.setupHorizontalMetrics({g[0]: (g[2], 0) for g in glyphs})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    nm = {"familyName": "Synthetic Items", "styleName": "Regular"}
    nm.update(names or {})
    fb.setupNameTable(nm, mac=False)
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=900, usWinDescent=250)
    fb.setupPost()
    font = fb.font
    font.recalcBBoxes = False
    font["head"].xMin, font["head"].yMin, font["head"].xMax, font["head"].yMax = 0, 0, 700, 800
    t = DefaultTable(tag)
    t.data = data
    font[tag] = t
    m = newTable("maxp")
    m.tableVersion = 0x00005000
    m.numGlyphs = len(glyphs)
    font["maxp"] = m
    for k, v in (extra or {}).items():
        t = DefaultTable(k)
        t.data = v
        font[k] = t
    font["head"].created = 3600000000
    font["head"].modified = 3600000000
    font.save(path)



def bias(count):
    return 107 if count < 1240 else 1131 if count < 33900 else 32768


def cl(k, count):
    return [k - bias(count), "callsubr"]


def cg(k, count):
    return [k - bias(count), "callgsubr"]


# --------------------------------------------------------------------------------------------------------------------------- bias
def bias_font(count):
    lsubrs = [sub_local(k) for k in range(count)]
    gsubrs = [sub_global(k) for k in range(count)]
    mid = count // 2
    glyphs = [
        (".notdef", [50, 0, "rmoveto", 400, 700, -400, "hlineto", "endchar"]),
        ("A", [30, 0, "rmoveto"] + cl(0, count) + cl(count - 1, count) + [-60, 0, "rlineto", "endchar"]),
        ("B", [30, 0, "rmoveto"] + cg(0, count) + cg(count - 1, count) + [60, 0, "rlineto", "endchar"]),
        ("O", [30, 0, "rmoveto"] + cl(mid, count) + cg(mid, count) + [0, -60, "rlineto", "endchar"]),
        # H: subr number N is out of range (nothing is drawn); the operand does not fit a short for N = 33899, which takes N - 2 instead
        ("H", [30, 0, "rmoveto"] + cl(min(count, count if count != 33899 else count - 2), count) + cg(min(count, count if count != 33899 else count - 2), count)
         + [100, 0, 100, 100, 0, 200, "rlineto", "endchar"]),
    ]
    chars = [None, 0x41, 0x42, 0x4F, 0x48]
    return glyphs, chars, lsubrs, gsubrs


# --------------------------------------------------------------------------------------------------------------------------- depth
def depth_font():
    L = [None] * 27
    G = [None] * 27
    for i in range(13):
        nxt = cl(i + 1, 27) if i < 12 else []
        L[i] = [i + 10, 5, "rlineto"] + nxt + ["return"]
        nxt = cg(i + 1, 27) if i < 12 else []
        G[i] = [-(i + 10), 6, "rlineto"] + nxt + ["return"]
    # alternating chain: LM_i = L[13 + i] calls GM_i = G[13 + i]; GM_i calls LM_(i+1)
    for i in range(13):
        L[13 + i] = [i + 12, -4, "rlineto"] + cg(13 + i, 27) + ["return"]
        G[13 + i] = [-(i + 12), 4, "rlineto"] + (cl(13 + i + 1, 27) if i < 12 else []) + ["return"]
    L[26] = [20, 3, "rlineto"] + cl(26, 27) + ["return"]
    G[26] = [-20, 3, "rlineto"] + cg(26, 27) + ["return"]
    start = [30, 0, "rmoveto"]
    glyphs = [(".notdef", [50, 0, "rmoveto", 400, 700, -400, "hlineto", "endchar"])]
    chars = [None]
    for ch, prog in [
        ("A", cl(4, 27)), ("B", cl(3, 27)), ("C", cl(2, 27)), ("D", cl(1, 27)), ("E", cl(0, 27)),
        ("F", cg(3, 27)), ("G", cg(2, 27)),
        ("H", cl(13 + 8, 27)), ("I", cg(13 + 7, 27)),
        ("O", cl(26, 27)), ("V", cg(26, 27)),
    ]:
        glyphs.append((ch, start + prog + ["endchar"]))
        chars.append(ord(ch))
    return glyphs, chars, L, G


# --------------------------------------------------------------------------------------------------------------------------- seac
def seac_font(kind):
    sq = lambda w, h: [w, 0, "rlineto", 0, h, "rlineto", -w, 0, "rlineto"]
    glyphs = [
        (".notdef", [50, 0, "rmoveto", 400, 700, -400, "hlineto", "endchar"]),
        ("A", [20, 0, "rmoveto"] + sq(300, 500) + ["endchar"]),
        ("B", [10, 600, "rmoveto"] + sq(120, 90) + ["endchar"]),
        # C: A with B 300 right and 200 up (65 = A, 66 = B in the standard encoding). adx ady bchar achar endchar.
        ("C", [300, 200, 65, 66, "endchar"]),
        # D: the same behind a width argument
        ("D", [650, 300, 200, 65, 66, "endchar"]),
        # E: names a character (90 = Z) the font does not have
        ("E", [300, 200, 65, 90, "endchar"]),
    ]
    chars = [None, 0x41, 0x42, 0x43, 0x44, 0x45]
    lsubrs = [[10, 0, "rlineto", "return"]]
    gsubrs = [[0, 10, "rlineto", "return"]]
    return glyphs, chars, lsubrs, gsubrs


# --------------------------------------------------------------------------------------------------------------------------- variable CFF2
def vstore(axes, regions, subtables):
    """regions: [[(start, peak, end) per axis]]; subtables: [[region indexes]]. VariationStore: length, ItemVariationStore."""
    f = lambda v: struct.pack(">h", round(v * 16384))
    rl = struct.pack(">HH", axes, len(regions))
    for r in regions:
        for s, p, e in r:
            rl += f(s) + f(p) + f(e)
    ivds = [struct.pack(">HHH", 0, 0, len(ix)) + b"".join(struct.pack(">H", i) for i in ix) for ix in subtables]
    head = struct.pack(">HIH", 1, 8 + 4 * len(subtables), len(subtables))
    off = 8 + 4 * len(subtables) + len(rl)
    offs = b""
    for d in ivds:
        offs += struct.pack(">I", off)
        off += len(d)
    store = head + offs + rl + b"".join(ivds)
    return struct.pack(">H", len(store)) + store


def build_cff2(programs, store):
    n = len(programs)
    cs_idx = index([cs(p) for p in programs], True)
    gsub_idx = index([], True)
    top_len = len(dict_int(0) * 3 + dict_op(17) + dict_op(1236) + dict_op(24))
    header_len = 5
    pos = header_len + top_len + len(gsub_idx)
    cs_off = pos
    pos += len(cs_idx)
    vs_off = pos
    pos += len(store)
    fda_off = pos
    fd_dict_len = len(dict_int(0) * 2 + dict_op(18))
    pos += len(index([b"\x00" * fd_dict_len], True))
    p = dict_int(5 + 1) + dict_op(19)
    sub_idx = index([cs([10, 0, "rlineto"])], True)
    fda = index([dict_int(len(p)) + dict_int(pos) + dict_op(18)], True)
    top = dict_int(cs_off) + dict_op(17) + dict_int(fda_off) + dict_op(1236) + dict_int(vs_off) + dict_op(24)
    assert len(top) == top_len
    return bytes([2, 0, header_len]) + struct.pack(">H", top_len) + top + gsub_idx + cs_idx + store + fda + p + sub_idx


def fvar_table(axes, instances):
    """axes: [(tag, min, default, max, nameID)]; instances: [(nameID, [coords])]. Fixed 16.16."""
    fx = lambda v: struct.pack(">i", round(v * 65536))
    isz = 4 + 4 * len(axes)
    out = struct.pack(">IHHHHHH", 0x10000, 16, 2, len(axes), 20, len(instances), isz)
    for tag, lo, d, hi, nid in axes:
        out += tag.encode() + fx(lo) + fx(d) + fx(hi) + struct.pack(">HH", 0, nid)
    for nid, co in instances:
        out += struct.pack(">HH", nid, 0) + b"".join(fx(c) for c in co)
    return out


def avar_table(maps):
    f = lambda v: struct.pack(">h", round(v * 16384))
    out = struct.pack(">HHHH", 1, 0, 0, len(maps))
    for m in maps:
        out += struct.pack(">H", len(m)) + b"".join(f(a) + f(b) for a, b in m)
    return out


def hvar_table(store_bytes_no_len):
    return struct.pack(">HHIII", 1, 0, 20, 0, 0) + struct.pack(">I", 0) + store_bytes_no_len


def var_font(kind):
    # glyph order: .notdef A B space O H F W V
    fx = lambda v: b"\xff" + struct.pack(">i", round(v * 65536))
    P = {
        ".notdef": [50, 0, "rmoveto", 400, 700, -400, "hlineto"],
        # A: a triangle whose second point carries one blend: 400 + 100 * scalar
        "A": [0, "vsindex", 20, 0, "rmoveto", 400, 100, 1, "blend", 0, "rlineto", -200, 300, "rlineto"],
        # B: fractional coordinates (16.16): 10.5 and -10.5 and 0.25 are rounded by opentype.js once a variation exists
        # B: fractional coordinates (16.16): -10.5, 0.5, 10.5 and 30.25 end up on .5 and .25 and are rounded by opentype.js once a variation exists
        # (Math.round: -10.5 -> -10, 40.5 -> 41, 0.5 -> 1)
        "B": [fx(-10.5), fx(0.5), "rmoveto", fx(10.5), 40, "rlineto", fx(-10.5), 40, "rlineto", fx(30.25), -80, "rlineto"],
        "space": [],
        "O": [0, "vsindex", 40, 0, "rmoveto"] + [500, 5, 1, "blend", 700, -500, "hlineto"],
        "H": [60, 0, "rmoveto", 300, 0, "rlineto", 0, 300, 150, 1, "blend", "rlineto"],
        "F": [0, "vsindex", 20, 0, "rmoveto", 300, 200, 100, 2, "blend", 0, "rlineto", -300, 400, "rlineto"],
        "W": [20, 20, "rmoveto", 100, 7, -3, 2, "blend", 200, 4, 1, "blend", 50, 100, "rlineto"],
        "V": [10, 10, "rmoveto", -1000, 3, 1, "blend", 100, "hlineto", 100, 100, "rlineto"],
    }
    if kind in ("2axes",):
        # blend with two regions: the stack takes n default values then n * 2 deltas
        P["A"] = [0, "vsindex", 20, 0, "rmoveto", 400, 100, -60, 1, "blend", 0, "rlineto", -200, 300, "rlineto"]
        P["O"] = [0, "vsindex", 40, 0, "rmoveto", 500, 40, 80, 1, "blend", 700, -500, "hlineto"]
        P["H"] = [60, 0, "rmoveto", 300, 0, "rlineto", 0, 300, 150, 90, 1, "blend", "rlineto"]
        P["F"] = [0, "vsindex", 20, 0, "rmoveto", 300, 200, 100, -50, 20, 2, "blend", 0, "rlineto", -300, 400, "rlineto"]
        P["W"] = [20, 20, "rmoveto", 100, 7, -3, 2, 5, 11, 2, "blend", 200, 4, 1, 1, 1, "blend", 50, 100, "rlineto"]
        P["V"] = [10, 10, "rmoveto", -1000, 3, 9, 1, "blend", 100, "hlineto", 100, 100, "rlineto"]
    names = [".notdef", "A", "B", "space", "O", "H", "F", "W", "V"]
    progs = [P[x] for x in names]
    if kind == "2axes":
        store = vstore(2, [[(0, 1, 1), (0, 0, 0)], [(0, 0, 0), (-1, -1, 0)]], [[0, 1], [1]])
    else:
        store = vstore(1, [[(0, 1, 1)]], [[0]])
    # a second subtable with one region for the glyphs that blend 2 values with one region: not needed (vsindex 0 only)
    extra, nm = {}, {}
    if kind == "default":
        axes = [("wght", 400, 400, 900, 257)]
        inst = []
    elif kind in ("regular", "avar"):
        axes = [("wght", 400, 400, 900, 257)]
        inst = [(256, [700])]
    else:
        axes = [("wght", 400, 400, 900, 257), ("wdth", 50, 100, 150, 258)]
        inst = [(259, [900, 100]), (256, [700, 75])]
    nm = {256: "Regular", 257: "Weight", 258: "Width", 259: "Bold"}
    extra["fvar"] = fvar_table(axes, inst)
    if kind == "avar":
        extra["avar"] = avar_table([[(-1, -1), (0, 0), (0.5, 0.8), (1, 1)]])
    if kind == "2axes":
        extra["avar"] = avar_table([[(-1, -1), (0, 0), (0.25, 0.5), (1, 1)], [(-1, -1), (-0.5, -0.25), (0, 0), (1, 1)]])
        hs = vstore(2, [[(0, 1, 1), (0, 0, 0)], [(0, 0, 0), (-1, -1, 0)]], [[0, 1]])[2:]
        extra["HVAR"] = hvar_table(hs)
    return progs, extra, nm


GL = [(".notdef", None, 500), ("A", 0x41, 640), ("B", 0x42, 560), ("space", 0x20, 300), ("O", 0x4F, 600), ("H", 0x48, 700),
      ("F", 0x46, 500), ("W", 0x57, 500), ("V", 0x56, 480)]


def main(out):
    os.makedirs(out, exist_ok=True)
    save = lambda name, glyphs, chars, data, tag="CFF ", extra=None, names=None, adv=600: shell(
        tag, data, os.path.join(out, name + ".otf"),
        [(g[0], c, adv) for g, c in zip(glyphs, chars)] if isinstance(glyphs[0], tuple) and len(glyphs[0]) == 2 else glyphs, extra, names)
    for count in (1239, 1240, 33899, 33900):
        glyphs, chars, ls, gs = bias_font(count)
        save(f"bias-{count}", glyphs, chars, build_cff1(glyphs, ls, gs))
    glyphs, chars, ls, gs = depth_font()
    save("depth", glyphs, chars, build_cff1(glyphs, ls, gs))
    glyphs, chars, ls, gs = seac_font("plain")
    save("seac-plain", glyphs, chars, build_cff1(glyphs, ls, gs))
    glyphs, chars, ls, gs = seac_font("cid")
    cid = [(0, 0)] + [(i, 0) for i in range(1, len(glyphs))]
    save("seac-cid", glyphs, chars, build_cff1(glyphs, ls, gs, cid=cid))
    cid3 = [(0, 0)] + [(i, (i - 1) % 3) for i in range(1, len(glyphs))]
    save("seac-cid-fd", glyphs, chars, build_cff1(glyphs, ls, gs, cid=cid3))
    for kind in ("default", "regular", "avar", "2axes"):
        progs, extra, nm = var_font(kind)
        shell("CFF2", build_cff2(progs, vstore_of(kind)), os.path.join(out, f"var-{kind}.otf"), GL, extra, nm)


def vstore_of(kind):
    if kind == "2axes":
        return vstore(2, [[(0, 1, 1), (0, 0, 0)], [(0, 0, 0), (-1, -1, 0)]], [[0, 1], [1]])
    return vstore(1, [[(0, 1, 1)]], [[0]])


if __name__ == "__main__":
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, "..", "synthetic-rea-t-cff-items"))
