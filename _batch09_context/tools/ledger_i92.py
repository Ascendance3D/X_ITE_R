#!/usr/bin/env python3
"""Issue #92 ledger update (render core, lighting, transparency, headlight, fog, fonts): python3 -I ledger_i92.py check|apply

Starts from ledger/before-i92/ledger.json (sha256 b9ef79f44968116be079f20eaab12bb06d606b916f0f90af2250b2a09916b6c4). Item lists and evidence against pinned X_ITE 16.5.0 (34cdf9f97):
crates/x3d-render/tests/render_rea_i92_items.rs replays 46 synthetic worlds (oracle/synthetic-rea-i92-render-items: headlight 8, lighting model 5,
transparency sorting 7, fog 26) against oracle/rea-i92-render-items-baseline (three identical X_ITE recordings per world, MSAA 0 and 4). Three worlds
(fg-transparent, tr-many, tr-ortho at MSAA 4) have no MSAA 4 reference because X_ITE's own output changes from run to run. The x3d-text oracle
runs with oracle/x_ite-text.json present (228 glyphs, 57 with every vertex, 41 layouts, 12 fonts) and fails when the key is absent. The fog.rs tests
fail when oracle/x_ite-fog.json is absent (they passed silently before). Headlight, lighting model and fog become COMPLETE; render core stays PARTIAL
(two Items2 worlds over the bound, NPOT mipmaps), transparency sorting stays PARTIAL (no MSAA 4 reference for three worlds), text fonts stays PARTIAL.
No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i92", "ledger.json")
SNAP_SHA256 = "b9ef79f44968116be079f20eaab12bb06d606b916f0f90af2250b2a09916b6c4"
SUB = ("oracle/synthetic-rea-i92-render-items replayed by crates/x3d-render/tests/render_rea_i92_items.rs against "
       "oracle/rea-i92-render-items-baseline (pinned X_ITE 16.5.0, three identical recordings per world, MSAA 0 and 4; bound 8 / 0.5%)")
NOTES = {
 "sub:render.core": ("Issue #92 2026-10-31: Items2 compare 294 within tolerance, 2 over (art/rainbow-room 2.54%, art/djascendance-live 1.85%: "
        "NPOT mipmaps, recorded in x3d-script/tests/items2-known.tsv) at MSAA 0 and MSAA 4; the 46 render-i92 worlds pass at MSAA 0, and at MSAA 4 wherever X_ITE is reproducible (3 worlds are not)."),
 "sub:render.transparency-sorting": ("Issue #92 2026-10-31 items: tr-order, tr-opaque, tr-nested (LOD cell), tr-points-lines, tr-textured, tr-many, tr-ortho, fg-transparent; "
        "all within the bound at MSAA 0. No MSAA 4 reference for tr-many, tr-ortho and fg-transparent: X_ITE's multisampled output differs between "
        "its own runs (oracle/rea-i92-render-items-baseline/msaa4/unstable.json). " + SUB + "."),
 "sub:render.lighting-model": ("Issue #92 2026-10-31 items: lm-normals, lm-sides, lm-material, lm-color-material, lm-attenuation, fg-specular, fg-emissive, fg-unlit; "
        + SUB + "; all within the bound at MSAA 0 and 4."),
 "sub:render.headlight": ("Issue #92 2026-10-31 items: hl-default, hl-on-light, hl-off-light, hl-off, hl-ortho, hl-rotated-viewpoint, hl-tilted-nav-type, "
        "hl-two-navinfo, fg-nofog-headlight; " + SUB + "; all within the bound at MSAA 0 and 4."),
 "sub:render.fog": ("Issue #92 2026-10-31 items: fg-linear, fg-linear-short, fg-exp, fg-exp2, fg-range0, fg-range-neg, fg-unknown-type, fg-unbound, fg-two-fogs, "
        "fg-localfog, fg-coordinate, fg-in-transform, fg-scaled-view, fg-rotated-view, fg-ortho, fg-points, fg-lines, fg-textured, fg-text, fg-color-bg, fg-red; "
        + SUB + "; all within the bound at MSAA 0 and 4. fog.rs oracle tests (x_ite-fog.json) now fail when the file is absent."),
 "sub:text.fonts": ("Issue #92 2026-10-31: x3d-text oracle with oracle/x_ite-text.json present: 228 glyphs checked (counts, area), 57 with every vertex, "
        "41 layouts, 12 fonts in the key; with an empty key directory the layouts test fails (never passes)."),
}
TESTS = {
 "sub:render.core": "crates/x3d-render/tests/render_rea_i92_items.rs (46 synthetic worlds, MSAA 0+4)",
 "sub:render.transparency-sorting": "crates/x3d-render/tests/render_rea_i92_items.rs (tr-* and fg-transparent, MSAA 0; MSAA 4 where X_ITE is reproducible)",
 "sub:render.lighting-model": "crates/x3d-render/tests/render_rea_i92_items.rs (lm-*, MSAA 0+4); crates/x3d-render/tests/render_rea_b09_visual.rs",
 "sub:render.headlight": "crates/x3d-render/tests/render_rea_i92_items.rs (hl-*, MSAA 0+4)",
 "sub:render.fog": "crates/x3d-render/tests/render_rea_i92_items.rs (fg-*, MSAA 0+4); crates/x3d-render/tests/fog.rs (x_ite-fog.json)",
 "sub:text.fonts": "crates/x3d-text/tests/oracle.rs (228 glyphs, 57 full vertex, 41 layouts)",
}
NEWMISSING = {
 "sub:render.core": "art/rainbow-room and art/djascendance-live exceed the bound (non-power-of-two texture mipmaps, X8 gap)",
 "sub:render.transparency-sorting": "MSAA 4 of tr-many, tr-ortho and fg-transparent has no X_ITE reference (not reproducible); OIT is not compared",
 "sub:text.fonts": "only the 12 fonts of the answer key are compared; glyph vertices of 171 of 228 glyphs by count and area only",
}
OLD_FOG = "no subsystem-level test listed; node:Fog, node:LocalFog and node:FogCoordinate are COMPLETE"
def fix_missing(k):
    def f(old):
        if k == "sub:render.fog" and old != OLD_FOG: raise SystemExit(f"unexpected missing text: {old!r}")
        return NEWMISSING.get(k, "")
    return f
def add_test(k): return lambda old: old + [TESTS[k]]

# id: (status or None, {field: value or function of the old value}, note). The verifier reads the first element.
C = {k: (("COMPLETE" if k in ("sub:render.lighting-model", "sub:render.headlight", "sub:render.fog") else None),
         {"missing": fix_missing(k), "tests": add_test(k)}, NOTES[k]) for k in NOTES}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, note = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            i["notes"] = (i["notes"] + " | " if i["notes"] else "") + note
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i92.py check|apply")
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
