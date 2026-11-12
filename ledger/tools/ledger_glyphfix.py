#!/usr/bin/env python3
"""Ledger step of the glyph geometry fixture (2026-11-12): all 228 default-font glyphs vertex for vertex against pinned X_ITE.
python3 -I ledger/tools/ledger_glyphfix.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-glyphfix", "ledger.json")
SNAP_SHA256 = "19941aa91c41510e77dd02484e0280cfc14b0381f538d83d6b5c025faf265881"

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
 "sub:text.fonts": (None, {
   "missing": swap("glyph vertices of 171 of 228 glyphs by count and area only; ", ""),
   "tests": append("crates/x3d-text/tests/glyph_geometry.rs (every libtess triangle vertex of the 228 glyphs of the 12 default fonts, 174486 coordinates, bit for bit against the versioned fixture crates/x3d-text/tests/data/glyph-geometry from oracle/x_ite-text.mjs --all-glyphs and oracle/tools/glyph_fixture.py, pinned X_ITE 16.5.0, three identical recordings; font files checked by SHA-256; a negative control changes one expected bit and must fail)"),
  }, "2026-11-12 (glyph geometry fixture): pinned X_ITE 16.5.0 recorded all 12 default fonts in full (oracle/x_ite-text.mjs --all-glyphs, headless Chrome 156) three times with identical bytes (SHA-256 701edabe8b547ea6b4b6730355befb5ded7f94eaa8abf84256ad3378af9bd3e6); the 57 glyphs that the answer key holds in full are equal to it. x3d-text gives the same f64 bits for all 228 glyphs (171 of them were compared by count and area only before). The fixture is separate from the protected answer key oracle/x_ite-text.json, which is not changed."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_glyphfix.py check|apply")
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
