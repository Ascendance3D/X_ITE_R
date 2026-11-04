#!/usr/bin/env python3
"""Authors the synthetic CID-keyed CFF and CFF2 test fonts of oracle/synthetic-rea-t-cff-cid (REA node:FontLibrary, sub:text.fonts).

Every outline and every byte of the CFF / CFF2 tables is written here: no third-party font data. The CFF tables are assembled by hand
(INDEX, DICT, charset, FDSelect, FDArray, Private DICT, local and global subrs, VariationStore) so that a font can carry exactly the
structure a case needs, including broken structure. fontTools (tested with 4.46) writes only the SFNT shell (head, hhea, maxp, hmtx,
cmap, name, OS/2, post); the output is deterministic (fixed timestamps).

  python3 -I oracle/tools/gen_cff_cid_fonts.py [OUTPUT_DIR]      (default oracle/synthetic-rea-t-cff-cid)

Fonts (glyph order .notdef A B space O H F W V, characters A B space O H F W V; units per em 1000):
  cid-fd3.otf          CID-keyed CFF: ROS Adobe-Identity-0, charset format 2 (CIDs 1-3 5-7 20-21), FDSelect format 3, three Font DICTs. Each
                       FD has its own Private DICT (defaultWidthX, nominalWidthX) and its own local subrs: callsubr 0 draws a different shape per FD.
  (widths) opentype.js takes a glyph's advance from the charstring only when the charstring has a width argument or a stem hint, using the
                       nominalWidthX / defaultWidthX of the glyph's own FD; B (rmoveto only) keeps its hmtx advance, V (hstem, no width) takes its FD's
                       defaultWidthX, A / O / H / F / W / space carry explicit widths.
  cid-fd0.otf          CID-keyed CFF: charset format 0, FDSelect format 0, two FDs; FD 1 has no Private DICT (size 0), so its widths are 0 and
                       callsubr finds no subr.
  (CFF2)               CFF2 charstrings have no width, but a stem hint still sets opentype.js's `haveWidth` with defaultWidthX 0: H (hstem) gets
                       the advance 0.
  cff2-fd3.otf         CFF2: two FDs with local subrs (no `return`), FDSelect format 3, a VariationStore with one region and one
                       ItemVariationData, `vsindex` and `blend` in a glyph; no `endchar`, no widths, no `hmtx` replacement.
  cff2-fd4.otf         CFF2: the same with FDSelect format 4 (32-bit ranges).
  cff2-single.otf      CFF2: one FD, no FDSelect.
  cff2-novstore.otf    CFF2 without a VariationStore (opentype.js dereferences it for every glyph path).
  cff2-legacy-ops.otf  CFF2 whose charstrings use `return` and `endchar` (CFF2 forbids both; opentype.js logs and goes on).
  cid-bad-fdindex.otf  FDSelect names FD 3 of 3.
  cid-no-fdselect.otf  ROS without FDSelect.
  cid-bad-first.otf    FDSelect format 3 whose first range does not start at GID 0.
  cff2-no-fdarray.otf  CFF2 without FDArray.
"""
import os
import struct
import sys

from fontTools.fontBuilder import FontBuilder
from fontTools.ttLib.tables.DefaultTable import DefaultTable

OPS = {
    "hstem": 1, "vstem": 3, "vmoveto": 4, "rlineto": 5, "hlineto": 6, "vlineto": 7, "rrcurveto": 8, "callsubr": 10, "return": 11,
    "endchar": 14, "vsindex": 15, "blend": 16, "hstemhm": 18, "hintmask": 19, "cntrmask": 20, "rmoveto": 21, "hmoveto": 22,
    "vstemhm": 23, "rcurveline": 24, "rlinecurve": 25, "vvcurveto": 26, "hhcurveto": 27, "callgsubr": 29, "vhcurveto": 30,
    "hvcurveto": 31,
}
BIAS = 107


def cs_num(v):
    if isinstance(v, float) and v != int(v):
        return b"\xff" + struct.pack(">i", round(v * 65536))
    v = int(v)
    if -107 <= v <= 107:
        return bytes([v + 139])
    if 108 <= v <= 1131:
        return bytes([(v - 108) // 256 + 247, (v - 108) % 256])
    if -1131 <= v <= -108:
        return bytes([(-v - 108) // 256 + 251, (-v - 108) % 256])
    return b"\x1c" + struct.pack(">h", v)


def cs(program):
    out = b""
    for t in program:
        if isinstance(t, bytes):
            out += t
        elif isinstance(t, str):
            out += bytes([OPS[t]])
        else:
            out += cs_num(t)
    return out


def dict_int(v):
    """A DICT operand in the fixed five-byte form, so that offsets do not change the size of a DICT."""
    return b"\x1d" + struct.pack(">i", v)


def dict_small(v):
    if -107 <= v <= 107:
        return bytes([v + 139])
    if 108 <= v <= 1131:
        return bytes([(v - 108) // 256 + 247, (v - 108) % 256])
    if -1131 <= v <= -108:
        return bytes([(-v - 108) // 256 + 251, (-v - 108) % 256])
    return b"\x1c" + struct.pack(">h", v)


def dict_op(op):
    return bytes([op]) if op < 1200 else bytes([12, op - 1200])


def index(items, cff2=False):
    head = struct.pack(">I" if cff2 else ">H", len(items))
    if not items:
        return head
    offs, pos = [1], 1
    for it in items:
        pos += len(it)
        offs.append(pos)
    size = 1 if pos < 256 else 2 if pos < 65536 else 4
    return head + bytes([size]) + b"".join(o.to_bytes(size, "big") for o in offs) + b"".join(items)


def fdselect3(ranges, n_glyphs, first_override=None):
    """ranges: [(first_gid, fd)] ascending; format 3."""
    out = b"\x03" + struct.pack(">H", len(ranges))
    for i, (first, fd) in enumerate(ranges):
        out += struct.pack(">HB", first if not (i == 0 and first_override is not None) else first_override, fd)
    return out + struct.pack(">H", n_glyphs)


def fdselect4(ranges, n_glyphs):
    out = b"\x04" + struct.pack(">I", len(ranges))
    for first, fd in ranges:
        out += struct.pack(">IH", first, fd)
    return out + struct.pack(">I", n_glyphs)


def fdselect0(fds):
    return b"\x00" + bytes(fds)


# ---------------------------------------------------------------------------------------------------------------------------
# The glyphs. Each glyph: (character, hmtx advance, FD index, CFF1 program, CFF2 program).
# Subroutine 0 of an FD is reached with -107 (bias 107), subroutine 1 with -106.
# ---------------------------------------------------------------------------------------------------------------------------
GLYPHS = [
    # name, char, hmtx advance, FD (3-FD fonts)
    (".notdef", None, 500, 0),
    ("A", 0x41, 640, 0),
    ("B", 0x42, 560, 1),
    ("space", 0x20, 300, 0),
    ("O", 0x4F, 600, 1),
    ("H", 0x48, 700, 2),
    ("F", 0x46, 500, 2),
    ("W", 0x57, 500, 1),
    ("V", 0x56, 480, 2),
]
CIDS = [0, 1, 2, 3, 5, 6, 7, 20, 21]

# FD private data: (defaultWidthX, nominalWidthX, [CFF1 local subrs], [CFF2 local subrs]).
# A CFF1 subr ends with `return`; a CFF2 subr ends where its data ends.
FDS = [
    (400, 600, [[300, 300, -300, "hlineto"], [0, 100, "rlineto", 100, 0, "rlineto"]]),
    (500, 300, [[200, 400, -200, "hlineto"], [100, 100, "rlineto", -100, 100, "rlineto"]]),
    (250, 100, [[100, 500, -100, "hlineto"], [50, 50, "rlineto", 50, -50, "rlineto"]]),
]
GSUBR = [150, 150, -150, "hlineto"]


def cff1_program(name, legacy=False):
    """CFF1 charstrings (CID fonts): the width is stored minus the FD's nominalWidthX, or left out when it is defaultWidthX."""
    P = {
        ".notdef": [-100, 50, 0, "rmoveto", 400, 700, -400, "hlineto", "endchar"],
        "A": [40, 20, 0, "rmoveto", -107, "callsubr", 350, 0, "rmoveto", -107, "callgsubr", "endchar"],
        "B": [30, 0, "rmoveto", -107, "callsubr", 250, -50, "rmoveto", -106, "callsubr", "endchar"],
        "space": [-300, "endchar"],
        "O": [300, 40, 0, "rmoveto", 500, 700, -500, "hlineto", 130, 130, "rmoveto", -106, "callsubr", "endchar"],
        "H": [600, 0, 100, "hstem", "hintmask", b"\x80", 50, 0, "rmoveto", -107, "callsubr", 300, 0, "rmoveto", -107, "callsubr",
              "endchar"],
        "F": [400, 20, 0, "rmoveto", -107, "callsubr", 100, 100, "rmoveto", -107, "callgsubr", "endchar"],
        "W": [350, 50, 0, "rmoveto", 400, 500, -400, "hlineto", "endchar"],
        # V: an hstem without a width argument: the width is the FD's defaultWidthX (250 in FD 2), whatever hmtx says (480)
        "V": [0, 100, "hstem", 50, 0, "rmoveto", 200, 500, -200, "hlineto", -106, "callsubr", "endchar"],
    }
    return P[name]


def cff2_program(name):
    """CFF2 charstrings: no width, no endchar; a Font DICT's subrs have no `return`."""
    P = {
        ".notdef": [50, 0, "rmoveto", 400, 700, -400, "hlineto"],
        "A": [20, 0, "rmoveto", -107, "callsubr", 350, 0, "rmoveto", -107, "callgsubr"],
        "B": [30, 0, "rmoveto", -107, "callsubr", 250, -50, "rmoveto", -106, "callsubr"],
        "space": [],
        # vsindex 0, then "100 0 1 blend": one default value and one delta set of one region; a static font keeps the default.
        "O": [0, "vsindex", 40, 0, "rmoveto", 500, 5, 1, "blend", 700, -500, "hlineto", 130, 130, "rmoveto", -106, "callsubr"],
        "H": [0, 100, "hstem", "hintmask", b"\x80", 50, 0, "rmoveto", -107, "callsubr", 300, 0, "rmoveto", -107, "callsubr"],
        "F": [20, 0, "rmoveto", -107, "callsubr", 100, 100, "rmoveto", -107, "callgsubr"],
        "W": [50, 0, "rmoveto", 400, 500, -400, "hlineto"],
        "V": [50, 0, "rmoveto", 200, 500, -200, "hlineto", -106, "callsubr"],
    }
    return P[name]


def private1(default_w, nominal_w, subrs_len):
    """Private DICT of a CFF1 font; Subrs is the last entry and the fixed-size operand makes its own offset (the DICT size) known."""
    d = b""
    if default_w:
        d += dict_small(default_w) + dict_op(20)
    if nominal_w:
        d += dict_small(nominal_w) + dict_op(21)
    return d + dict_int(len(d) + 5 + 1) + dict_op(19)


def vstore():
    """VariationStore: length, ItemVariationStore (format 1, region list at 12, one ItemVariationData at 20)."""
    region_list = struct.pack(">HH", 1, 1) + struct.pack(">hhh", 0, 0x4000, 0x4000)  # 1 axis, 1 region: start 0, peak 1, end 1
    ivd = struct.pack(">HHH", 0, 0, 1) + struct.pack(">H", 0)  # itemCount 0, wordDeltaCount 0, regionIndexCount 1, region 0
    store = struct.pack(">HIH", 1, 12, 1) + struct.pack(">I", 12 + len(region_list))
    store += region_list + ivd
    return struct.pack(">H", len(store)) + store


def build_cff1_cid(variant):
    n = len(GLYPHS)
    legacy = False
    fd_of = [g[3] for g in GLYPHS]
    gl_fd = fd_of
    if variant == "fd0":
        # two FDs: FD 0 as above, FD 1 with no Private DICT. B, O, W, V move to FD 1; H and F to FD 0.
        gl_fd = [0, 0, 1, 0, 1, 0, 0, 1, 1]
        fds = [FDS[0], None]
    else:
        fds = list(FDS)
    charstrings = []
    for (name, _c, _w, _fd), fd in zip(GLYPHS, gl_fd):
        prog = list(cff1_program(name))
        if variant == "fd0" and fd == 1:
            # no Private DICT: nominalWidthX and defaultWidthX are 0, so a stored width is the width itself
            if name == "B":
                prog = [560, 30, 0, "rmoveto", 200, 400, -200, "hlineto", -107, "callsubr", "endchar"]
            elif name == "O":
                prog = [600, 40, 0, "rmoveto", 500, 700, -500, "hlineto", "endchar"]
            elif name == "W":
                prog = [650, 50, 0, "rmoveto", 400, 500, -400, "hlineto", "endchar"]
            elif name == "V":
                prog = [0, 100, "hstem", 50, 0, "rmoveto", 200, 500, -200, "hlineto", "endchar"]
        elif variant == "fd0" and fd == 0:
            if name == "H":
                prog = [100, 0, 100, "hstem", "hintmask", b"\x80", 50, 0, "rmoveto", -107, "callsubr", "endchar"]
            elif name == "F":
                prog = [-100, 20, 0, "rmoveto", -107, "callsubr", 100, 100, "rmoveto", -107, "callgsubr", "endchar"]
        charstrings.append(cs(prog))
    gsubrs = [cs(GSUBR + ["return"])]
    strings = [b"Adobe", b"Identity"]
    cids = CIDS
    if variant == "fd0":
        cids = list(range(n))
    # charset
    if variant == "fd0":
        charset = b"\x00" + b"".join(struct.pack(">H", c) for c in cids[1:])
    else:
        charset = b"\x02"
        for first, nleft in ((1, 2), (5, 2), (20, 1)):
            charset += struct.pack(">HH", first, nleft)
    # FDSelect
    if variant == "fd0":
        fdsel = fdselect0(gl_fd)
    else:
        ranges, prev = [], None
        for gid, fd in enumerate(gl_fd):
            if fd != prev:
                ranges.append((gid, fd))
                prev = fd
        first = 0
        if variant == "bad-first":
            first = 1
        fdsel = fdselect3(ranges, n, first_override=first if variant == "bad-first" else None)
        if variant == "bad-fdindex":
            ranges = [(r[0], 3 if r[0] == 5 else r[1]) for r in ranges]
            fdsel = fdselect3(ranges, n)
    name_idx = index([b"SynthCid"])
    str_idx = index(strings)
    gsub_idx = index(gsubrs)
    cs_idx = index(charstrings)
    header = bytes([1, 0, 4, 4])

    def top_dict(charset_off, fdsel_off, cs_off, fda_off):
        t = b"".join(dict_small(v) for v in (391, 392, 0)) + dict_op(1230)  # ROS
        t += dict_small(n) + dict_op(1234)  # CIDCount
        t += dict_int(charset_off) + dict_op(15)
        if variant != "no-fdselect":
            t += dict_int(fdsel_off) + dict_op(1237)
        t += dict_int(cs_off) + dict_op(17)
        t += dict_int(fda_off) + dict_op(1236)
        return t

    top_len = len(index([top_dict(0, 0, 0, 0)]))
    pos = len(header) + len(name_idx) + top_len + len(str_idx) + len(gsub_idx)
    charset_off = pos
    pos += len(charset)
    fdsel_off = pos
    pos += len(fdsel)
    cs_off = pos
    pos += len(cs_idx)
    fda_off = pos
    # FDArray: one Font DICT per FD; Private (size, offset) in fixed-size operands.
    fd_dict_len = len(dict_int(0) * 2 + dict_op(18))
    fda_len = len(index([b"\x00" * fd_dict_len] * len(fds)))
    pos += fda_len
    privs, fdicts = b"", []
    for fd in fds:
        if fd is None:
            fdicts.append(dict_int(0) + dict_int(0) + dict_op(18))
            continue
        default_w, nominal_w, subrs = fd
        p = private1(default_w, nominal_w, 0)
        sub_idx = index([cs(s + ["return"]) for s in subrs])
        fdicts.append(dict_int(len(p)) + dict_int(pos + len(privs)) + dict_op(18))
        privs += p + sub_idx
    fda = index(fdicts)
    assert len(fda) == fda_len
    topd = index([top_dict(charset_off, fdsel_off, cs_off, fda_off)])
    assert len(topd) == top_len
    return header + name_idx + topd + str_idx + gsub_idx + charset + fdsel + cs_idx + fda + privs


def build_cff2(variant):
    n = len(GLYPHS)
    legacy = variant == "legacy-ops"
    single = variant == "single"
    gl_fd = [0] * n if single else [g[3] if g[3] < 2 else 1 for g in GLYPHS]
    fds = [FDS[0]] if single else [FDS[0], FDS[1]]
    charstrings = []
    for (name, _c, _w, _fd) in GLYPHS:
        prog = list(cff2_program(name))
        if legacy:
            prog = prog + ["endchar"]
        charstrings.append(cs(prog))
    gsubrs = [cs(GSUBR + (["return"] if legacy else []))]
    cs_idx = index(charstrings, True)
    gsub_idx = index(gsubrs, True)
    # FDSelect: 0 = FD 0 (A, space, .notdef), 1 = the rest of the FD-1 glyphs.
    ranges, prev = [], None
    for gid, fd in enumerate(gl_fd):
        if fd != prev:
            ranges.append((gid, fd))
            prev = fd
    if variant == "fd4":
        fdsel = fdselect4(ranges, n)
    else:
        fdsel = fdselect3(ranges, n)
    vs = vstore()

    def top_dict(cs_off, fda_off, fdsel_off, vs_off):
        t = dict_int(cs_off) + dict_op(17)
        if variant != "no-fdarray":
            t += dict_int(fda_off) + dict_op(1236)
        if not single:
            t += dict_int(fdsel_off) + dict_op(1237)
        if variant != "novstore":
            t += dict_int(vs_off) + dict_op(24)
        return t

    top_len = len(top_dict(0, 0, 0, 0))
    header_len = 5
    pos = header_len + top_len + len(gsub_idx)
    cs_off = pos
    pos += len(cs_idx)
    fdsel_off = pos
    pos += 0 if single else len(fdsel)
    vs_off = pos
    pos += 0 if variant == "novstore" else len(vs)
    fda_off = pos
    fd_dict_len = len(dict_int(0) * 2 + dict_op(18))
    fda_len = len(index([b"\x00" * fd_dict_len] * len(fds), True))
    pos += fda_len
    privs, fdicts = b"", []
    for _fd, (default_w, nominal_w, subrs) in enumerate(fds):
        # CFF2 Private DICT: vsindex 0 (a non-default entry is not needed), Subrs offset (the DICT size)
        p = dict_int(5 + 1) + dict_op(19)
        sub_idx = index([cs(s + (["return"] if legacy else [])) for s in subrs], True)
        fdicts.append(dict_int(len(p)) + dict_int(pos + len(privs)) + dict_op(18))
        privs += p + sub_idx
    fda = index(fdicts, True)
    assert len(fda) == fda_len
    topd = top_dict(cs_off, fda_off, fdsel_off, vs_off)
    assert len(topd) == top_len
    header = bytes([2, 0, header_len]) + struct.pack(">H", top_len)
    return (header + topd + gsub_idx + cs_idx + (b"" if single else fdsel) + (b"" if variant == "novstore" else vs) + fda + privs)


def shell(cff_tag, cff_data, path):
    names = [g[0] for g in GLYPHS]
    fb = FontBuilder(1000, isTTF=False)
    fb.setupGlyphOrder(names)
    fb.setupCharacterMap({g[1]: g[0] for g in GLYPHS if g[1] is not None})
    fb.setupHorizontalMetrics({g[0]: (g[2], 0) for g in GLYPHS})
    fb.setupHorizontalHeader(ascent=800, descent=-200)
    fb.setupNameTable({"familyName": "Synthetic CID", "styleName": "Regular"})
    fb.setupOS2(sTypoAscender=800, sTypoDescender=-200, usWinAscent=900, usWinDescent=250)
    fb.setupPost()
    font = fb.font
    font.recalcBBoxes = False  # head would read the bbox from fontTools' own CFF object
    font["head"].xMin, font["head"].yMin, font["head"].xMax, font["head"].yMax = 0, 0, 700, 800
    t = DefaultTable(cff_tag)
    t.data = cff_data
    font[cff_tag] = t
    font["maxp"] = _maxp(len(names))
    font["head"].created = 3600000000
    font["head"].modified = 3600000000
    font.save(path)


def _maxp(n):
    from fontTools.ttLib import newTable
    m = newTable("maxp")
    m.tableVersion = 0x00005000
    m.numGlyphs = n
    return m


FONTS = {
    "cid-fd3": ("CFF ", lambda: build_cff1_cid("fd3")),
    "cid-fd0": ("CFF ", lambda: build_cff1_cid("fd0")),
    "cid-bad-fdindex": ("CFF ", lambda: build_cff1_cid("bad-fdindex")),
    "cid-no-fdselect": ("CFF ", lambda: build_cff1_cid("no-fdselect")),
    "cid-bad-first": ("CFF ", lambda: build_cff1_cid("bad-first")),
    "cff2-fd3": ("CFF2", lambda: build_cff2("fd3")),
    "cff2-fd4": ("CFF2", lambda: build_cff2("fd4")),
    "cff2-single": ("CFF2", lambda: build_cff2("single")),
    "cff2-novstore": ("CFF2", lambda: build_cff2("novstore")),
    "cff2-legacy-ops": ("CFF2", lambda: build_cff2("legacy-ops")),
    "cff2-no-fdarray": ("CFF2", lambda: build_cff2("no-fdarray")),
}


def main(out):
    os.makedirs(out, exist_ok=True)
    for name, (tag, make) in FONTS.items():
        shell(tag, make(), os.path.join(out, name + ".otf"))


if __name__ == "__main__":
    here = os.path.dirname(os.path.abspath(__file__))
    main(sys.argv[1] if len(sys.argv) > 1 else os.path.join(here, "..", "synthetic-rea-t-cff-cid"))
