#!/usr/bin/env python3
"""Ledger step of the ScreenText raster (2026-11-11): the canvas of CFF and TrueType text against the pinned page's canvas.
python3 -I ledger/tools/ledger_cffsr.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-cffsr", "ledger.json")
SNAP_SHA256 = "811970038cc526bc162bd8955113807a9c10153930e2a9ec4c199ac3dd5b8c72"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f

def swap_in_list(old_text, new_text):
    def f(old):
        assert sum(s.count(old_text) for s in old) == 1, old_text
        return [s.replace(old_text, new_text) for s in old]
    return f

def append(*extra):
    return lambda old: list(old) + list(extra)

C = {
 "node:FontLibrary": (None, {
   "tests": append("crates/x3d-text/tests/rea_screen_canvas.rs (17 canvases of 6 worlds: the pinned page's own canvas calls given to x3d_text::skia_edge, alpha against the page's getImageData; at most 0.5% of the canvas' pixels more than 8 off; 6 CFF canvases pinned by name as over the bound)", 'crates/x3d-render/tests/render_rea_cff_screen_raster.rs (6 worlds of oracle/synthetic-rea-cff-screen-raster against oracle/rea-cff-screen-raster-baseline, pinned X_ITE 16.5.0, three identical recordings; 0.001% to 0.079% of the picture more than 8 off)'),
   "missing": swap("open: the canvas raster of CFF glyphs (cubics flattened into 16 lines);", "the canvas raster of CFF glyphs is compared with the page's canvas (rea_screen_canvas, render_rea_cff_screen_raster): glyphs of lines within 0.03% of the canvas, 5 of 11 canvases within 0.5%; open: 6 canvases of glyphs with cubic counters at 0.71% to 1.24% (Chrome has such an edge about 0.1 pixel further left; the rule is not known);"),
  }, "2026-11-11 (ScreenText raster): the pinned page's canvas is recorded (x_ite-shot.mjs --canvas: the calls and the alpha of every 2D canvas ScreenText fills; 6 authored worlds, 17 canvases, three identical runs). The canvas fills polylines: Skia's analytic anti-aliasing cuts a curve at its Y extrema and walks each part by forward differences in 16.16 fixed point, with every Y on a quarter pixel and a line's X truncated to 1/256; the canvas holds the text upside down. crates/x3d-text/src/skia_edge.rs does the same and ScreenText uses it for every glyph (before: exact curve points, 16 lines for each CFF cubic, and the rows counted from the other end). Canvas pixels more than 8 off: TrueType 0.54% to 2.98% before, 0.00% to 0.11% now; CFF lines-only glyphs 0.60% and 0.68% before, 0.02% and 0.03% now; CFF with curves 1.68% to 7.36% before, 0.21% to 1.24% now, of which 6 canvases stay over 0.5%. Open: where a cubic bulges into the filled span Chrome's edge is about 0.1 pixel further left than the polylines (a quadratic, and a cubic that bulges away, are not; measured with single arcs on Chrome 156); the rule is not known and is not copied. Also pinned: with this CFF font a ScreenText of one line draws nothing in X_ITE and in the port. The entries stay PARTIAL."),
 "sub:text.fonts": (None, {
   "tests": append("crates/x3d-text/tests/rea_screen_canvas.rs (17 canvases of 6 worlds: the pinned page's own canvas calls given to x3d_text::skia_edge, alpha against the page's getImageData; at most 0.5% of the canvas' pixels more than 8 off; 6 CFF canvases pinned by name as over the bound)", 'crates/x3d-render/tests/render_rea_cff_screen_raster.rs (6 worlds of oracle/synthetic-rea-cff-screen-raster against oracle/rea-cff-screen-raster-baseline, pinned X_ITE 16.5.0, three identical recordings; 0.001% to 0.079% of the picture more than 8 off)'),
   "missing": swap("the raster not compared (cubics flattened into 16 lines);", "the raster is compared on the page's canvas (rea_screen_canvas: TrueType within 0.11% of the canvas, CFF lines within 0.03%; 6 CFF canvases with cubic counters at 0.71% to 1.24% stay open, see node:FontLibrary);"),
  }, "2026-11-11 (ScreenText raster): see node:FontLibrary. The default TrueType font's ScreenText canvas is compared too (ttf-sr-default, 4 canvases)."),
}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, nt = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            if nt: i["notes"] = (i["notes"] + " | " if i["notes"] else "") + nt
    return d
def check(d):
    bad, snap = [], {i["id"]: i for i in load(SNAP)["items"]}
    cur = {i["id"]: i for i in d["items"]}
    if set(cur) != set(snap) or len(d["items"]) != 346: bad.append("ID set or total changed")
    if [i["id"] for i in d["items"]] != [i["id"] for i in load(SNAP)["items"]]: bad.append("ID order changed")
    bad += [f"{k}: not in the snapshot" for k in sorted(set(C) - set(snap))]
    built = build()
    if {k: v for k, v in d.items() if k != "items"} != {k: v for k, v in built.items() if k != "items"}: bad.append("top-level fields differ from what apply writes")
    want = {i["id"]: i for i in built["items"]}
    for k, b in snap.items():
        a = cur.get(k)
        if a is None: continue  # reported above: ID set or total changed
        if k not in C and a != b: bad.append(f"unrelated entry changed: {k}")
        if b["status"] == "DIFFERENT" and a != b: bad.append(f"DIFFERENT changed: {k}")
        if k in C and (a["status"] != (C[k][0] or b["status"]) or not a["notes"].startswith(b["notes"])): bad.append(f"{k}: status or notes")
        if k in C and a != want[k]: bad.append(f"{k}: differs from what apply writes")
    c = collections.Counter(i["status"] for i in d["items"]); print(dict(sorted(c.items())))
    print("\n".join(bad) or "OK"); return not bad
def sha256(p):
    with open(p, "rb") as f: return hashlib.sha256(f.read()).hexdigest()
def snap_ok():
    got = sha256(SNAP)
    if got != SNAP_SHA256: print(f"snapshot sha256 {got} is not the expected {SNAP_SHA256}: refused"); return False
    return True
if __name__ == "__main__":
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_cffsr.py check|apply")
    if not snap_ok(): sys.exit(1)
    if sys.argv[1:] == ["apply"]:
        d = build()
        try: same = sha256(AUTH) == SNAP_SHA256 or load(AUTH) == d
        except ValueError: same = False
        if not same:
            print("ledger.json is neither the snapshot nor the result of this change set: refused"); sys.exit(1)
        tmp = AUTH + ".tmp"
        with open(tmp, "w") as f:
            json.dump(d, f, indent=1); f.flush(); os.fsync(f.fileno())
        os.replace(tmp, AUTH); sys.exit(0 if check(load(AUTH)) else 1)
    sys.exit(0 if check(load(AUTH)) else 1)
