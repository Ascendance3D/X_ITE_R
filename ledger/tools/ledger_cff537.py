#!/usr/bin/env python3
"""Ledger step of PR #537 (2026-11-10): the UNVERIFIED CFF items of node:FontLibrary compared with pinned X_ITE (subroutine
bias and depth, seac, variable CFF2 default instance); two Rust fixes.
python3 -I ledger/tools/ledger_cff537.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-cff537", "ledger.json")
SNAP_SHA256 = "a78b0972f3e0c1c0aa45e5b3b8cb7ecc605bb1527315f1ab48ae3d72feedd122"

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
   "missing": swap("UNVERIFIED: variable CFF2 fonts (fvar, avar, HVAR, blend with coordinates), seac inside a CID-keyed font, subroutine depth over 10, subroutine bias for 1240 or more subrs;", "compared at the glyph level with 12 authored fonts (rea_t_cff_items, PR #537): subroutine bias at 1239/1240/33899/33900 local and global subrs, subroutine depth 9 to 13 (10 nested calls, the 11th skipped with X_ITE's warning), seac in plain and CID-keyed CFF (X_ITE throws a TypeError for every CID-keyed seac glyph, which the port mirrors; a CID-keyed seac world was not recorded), variable CFF2 at X_ITE's default instance (fvar, avar, a Regular instance off the axis defaults, two axes; X_ITE sets no coordinates and applies no HVAR advance); not covered: name records other than en, an fvar or avar that opentype.js rejects, gvar fonts;"),
   "tests": append("crates/x3d-text/tests/rea_t_cff_items.rs (12 authored fonts from oracle/tools/gen_cff_items_fonts.py against oracle/rea-t-cff-items-baseline/fonts.json, pinned X_ITE 16.5.0 opentype.js, three identical recordings: every glyph's path vertex for vertex, a check that each font discriminates its item, damaged bytes without panic; PR #537)"),
  }, "PR #537 (2026-11-10): two fixes, each red first. Variable CFF2: the port ignored fvar (var-2axes A vertex 0: [0.42, 0] against X_ITE's [0.478, 0]); it now picks opentype.js's default instance (the instance at the axis defaults, else the one named Regular, else the defaults), applies avar and the charstring blend, and rounds the path like Math.round (crates/x3d-text/src/variation.rs, cff.rs). seac in a plain CFF with a missing accent glyph: X_ITE throws TypeError: Cannot read properties of undefined (reading 'path'), the port drew an empty glyph; now it throws the same (font.rs). Equal without change: subroutine bias and depth. REA: analysis of the pinned opentype.js 2.0.0 dist ev_7ca7a943bce4b1cd5f0645464833e791086003efd7ea6d9b224e233143811da6 (summary ev_2f7e8243ac98d128a4a16fb67269742d0be5200dfe54f8ba4b814fd83ed190bb), partial graph. The canvas raster and the two-library order stay open: PARTIAL."),
 "sub:text.fonts": (None, {
   "missing": swap("with the open items of node:FontLibrary", "with the open items of node:FontLibrary; subroutine bias and depth, seac and variable CFF2 at the default instance are compared glyph by glyph (rea_t_cff_items, PR #537)"),
  }, "PR #537 (2026-11-10): see node:FontLibrary (variable CFF2 and seac fixes, 12 authored fonts). Status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_cff537.py check|apply")
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
