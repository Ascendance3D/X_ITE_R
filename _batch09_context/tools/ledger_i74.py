#!/usr/bin/env python3
"""Issue #74 ledger update (node:FontLibrary, sub:text.fonts): python3 -I ledger_i74.py check|apply

x3d_text::Font now loads OpenType-CFF (Type 2 charstring) outlines and matches pinned X_ITE 16.5.0 (opentype.js 2.0.0) on every recorded case (rea_b24_cff: metrics,
kerning, 14 glyph paths bit-exact, libtess triangles, 12 text layouts; 5 identical recordings). Both entries stay PARTIAL: the runtime FontLibrary and FontStyle
paths were not run end to end with a CFF font, and CID-keyed CFF, CFF2 and ScreenText are not compared. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i74", "ledger.json")
SNAP_SHA256 = "38a75ac42ed4d28d4d53b0d2c4d54a381bf1a3885f709c3f0bdbdb6c75e4c50b"

T = ["crates/x3d-text/tests/rea_b24_cff.rs (cff_font_metrics_kerning_and_glyph_outlines, cff_text_layouts_match_x_ite, bare_cff_and_non_font_data_do_not_parse; authored OpenType-CFF font with hint masks, subroutines, the flex operators, seac-style endchar and the opentype.js advance quirk)"]
ORACLE = "pinned X_ITE 16.5.0 (34cdf9f97): oracle/synthetic-rea-b24-cff-baseline.json (glyph paths, libtess triangles, 12 layouts; 5 identical recordings), authored fonts in oracle/synthetic-rea-b24-cff"
FL = {"tests": lambda old: old + T, "oracle": lambda old: (old + "; " if old else "") + ORACLE,
    "missing": "no end-to-end runtime FontLibrary or FontStyle run with a CFF font (Font::parse is compared); two libraries with one family not pinned (order-dependent in X_ITE); CID-keyed CFF and CFF2 not recorded; a bare CFF is rejected like X_ITE"}
TF = {"tests": lambda old: old + T, "oracle": lambda old: (old + "; " if old else "") + ORACLE,
    "missing": "only the 12 fonts of the answer key and one authored CFF font are compared; glyph vertices of 171 of 228 glyphs by count and area only; ScreenText with a CFF font not compared (cubics flattened into 16 lines)"}
NOTE_FL = "Issue #74 2026-10-31: CFF outlines are interpreted by ttf-parser (no new dependency). A glyph's advance follows the charstring width once its path is built, like opentype.js; they can differ only for a malformed font."
NOTE_TF = "Issue #74 2026-10-31: one authored OpenType-CFF font is compared in layouts (justify, size, spacing, vertical, right-to-left, length, maxExtent, .notdef, kerning)."
C = {"node:FontLibrary": (None, FL, NOTE_FL), "sub:text.fonts": (None, TF, NOTE_TF)}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, note = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            if note: i["notes"] = (i["notes"] + " | " if i["notes"] else "") + note
    return d
def check(d):
    bad, snap = [], {i["id"]: i for i in load(SNAP)["items"]}
    cur = {i["id"]: i for i in d["items"]}
    if set(cur) != set(snap) or len(d["items"]) != 346: bad.append("ID set or total changed")
    if [i["id"] for i in d["items"]] != [i["id"] for i in load(SNAP)["items"]]: bad.append("ID order changed")
    want = {i["id"]: i for i in build()["items"]}
    for k, b in snap.items():
        a = cur[k]
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i74.py check|apply")
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
