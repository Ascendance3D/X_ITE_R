#!/usr/bin/env python3
"""Ledger step of the cubic edge walk of the canvas scan converter (Phase 7B, 2026-11-12): the 6 CFF canvases with cubic counters.
python3 -I ledger/tools/ledger_cffwalk.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-cffwalk", "ledger.json")
SNAP_SHA256 = "420d8c4faa35cdf8d9ea20403219d52968ac4450b4c4772b31660e13815bd42e"

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
   "missing": swap("glyphs of lines within 0.03% of the canvas, 5 of 11 canvases within 0.5%; open: 6 canvases of glyphs with cubic counters at 0.71% to 1.24% (Chrome has such an edge about 0.1 pixel further left; the rule is not known); ", "all 17 canvases within 0.5% (CFF 0.00% to 0.43%, with Skia's walk of a cubic edge); "),
   "tests": lambda old: append('crates/x3d-text/src/skia_edge.rs tests::the_walk_adds_the_truncated_steps_of_each_row_stop, tests::a_cubic_line_starts_where_the_walk_of_the_previous_line_ended (the cubic edge walk of SkScan_AAAPath: keepContinuous and goY (y, yShift))')(swap_in_list('crates/x3d-render/tests/render_rea_cff_screen_raster.rs (6 worlds of oracle/synthetic-rea-cff-screen-raster against oracle/rea-cff-screen-raster-baseline, pinned X_ITE 16.5.0, three identical recordings; 0.001% to 0.079% of the picture more than 8 off)', 'crates/x3d-render/tests/render_rea_cff_screen_raster.rs (6 worlds of oracle/synthetic-rea-cff-screen-raster against oracle/rea-cff-screen-raster-baseline, pinned X_ITE 16.5.0, three identical recordings; 0.000% to 0.011% of the picture more than 8 off)')(swap_in_list("crates/x3d-text/tests/rea_screen_canvas.rs (17 canvases of 6 worlds: the pinned page's own canvas calls given to x3d_text::skia_edge, alpha against the page's getImageData; at most 0.5% of the canvas' pixels more than 8 off; 6 CFF canvases pinned by name as over the bound)", "crates/x3d-text/tests/rea_screen_canvas.rs (17 canvases of 6 worlds: the pinned page's own canvas calls given to x3d_text::skia_edge, alpha against the page's getImageData; at most 0.5% of the canvas' pixels more than 8 off; every canvas within the bound, KNOWN_GAPS empty)")(old))),
  }, "2026-11-12 (Phase 7B, cubic edge walk): the 6 CFF canvases with cubic counters had Chrome's edge about 0.1 pixel further left. Skia's non-convex scan (SkScan_AAAPath.cpp aaa_walk_edges, Skia main) calls SkAnalyticCubicEdge::keepContinuous before every updateCubic: the next line of a cubic starts at the X where the walk of the previous line ended (goY (y, yShift) adds fDX >> yShift per quarter, half or full row step between the line ends of the path), so the truncation adds up along the curve; quadratic edges take each new X from fQx, and the convex walk does not call keepContinuous. skia_edge.rs (walk) does the same. Pixels more than 8 off on the 6 canvases: 206, 5, 17, 26, 43, 22 (0.71% to 1.24%) before, 30, 0, 0, 1, 5, 0 after; all 11 CFF canvases 338 before, 47 after (one justify canvas 5 to 6, within the bound); the 4 TrueType canvases are unchanged."),
 "sub:text.fonts": (None, {
   "missing": swap("the raster is compared on the page's canvas (rea_screen_canvas: TrueType within 0.11% of the canvas, CFF lines within 0.03%; 6 CFF canvases with cubic counters at 0.71% to 1.24% stay open, see node:FontLibrary)", "the raster is compared on the page's canvas (rea_screen_canvas: all 17 canvases within the bound, TrueType 0.00% to 0.11%, CFF 0.00% to 0.43%)"),
   "tests": lambda old: swap_in_list('crates/x3d-render/tests/render_rea_cff_screen_raster.rs (6 worlds of oracle/synthetic-rea-cff-screen-raster against oracle/rea-cff-screen-raster-baseline, pinned X_ITE 16.5.0, three identical recordings; 0.001% to 0.079% of the picture more than 8 off)', 'crates/x3d-render/tests/render_rea_cff_screen_raster.rs (6 worlds of oracle/synthetic-rea-cff-screen-raster against oracle/rea-cff-screen-raster-baseline, pinned X_ITE 16.5.0, three identical recordings; 0.000% to 0.011% of the picture more than 8 off)')(swap_in_list("crates/x3d-text/tests/rea_screen_canvas.rs (17 canvases of 6 worlds: the pinned page's own canvas calls given to x3d_text::skia_edge, alpha against the page's getImageData; at most 0.5% of the canvas' pixels more than 8 off; 6 CFF canvases pinned by name as over the bound)", "crates/x3d-text/tests/rea_screen_canvas.rs (17 canvases of 6 worlds: the pinned page's own canvas calls given to x3d_text::skia_edge, alpha against the page's getImageData; at most 0.5% of the canvas' pixels more than 8 off; every canvas within the bound, KNOWN_GAPS empty)")(old)),
  }, "2026-11-12 (Phase 7B): the 6 CFF canvases with cubic counters are within the bound with Skia's cubic edge walk; see node:FontLibrary."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_cffwalk.py check|apply")
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
