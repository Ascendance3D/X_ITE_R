#!/usr/bin/env python3
"""Issue #75 ledger update (sub:parse.svg): python3 -I ledger_i75.py check|apply

Starts from ledger/before-i75/ledger.json (sha256 f4975062bcaa45df6189dc824f04b1ca9514326cf1efedce11d0076e0840d1ee). hwb(), lab(), lch(), oklab(), oklch() and color() now parse
to what pinned X_ITE 16.5.0 derives from Chrome's computed value (rea_b24_css4_color: 3,233 values against headless Chrome, svg_rea: four
SVG files against the pinned scene). The entry stays PARTIAL: relative color syntax, the radial-focal pixel and the SVG quality thread-local
remain. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i75", "ledger.json")
SNAP_SHA256 = "f4975062bcaa45df6189dc824f04b1ca9514326cf1efedce11d0076e0840d1ee"

T = ["crates/x3d-parse/tests/rea_b24_css4_color.rs (3,233 CSS Color 4 values vs headless Chrome through convertColor: hwb, lab, lch, oklab, oklch, color; 85 hwb() half-level ties within one 8-bit level, proven exact ties)",
     "crates/x3d-parse/tests/svg_rea.rs svg_css4_colors_match_x_ite (4 SVG files, oracle/synthetic-rea-b24-css4/svg-baseline.json, incl. the gradient stop X_ITE rejects)"]
MISSING = "relative color syntax (lab(from red l a b)) not parsed; 43 hsl() and 85 hwb() values one level off Chrome (half-level ties, owner decision #55); one radial-focal gradient pixel one level off; SVG quality is a thread-local not reset for a top-level world"
FIELDS = {"tests": lambda old: old + T, "missing": MISSING}
NOTE = "Issue #75 2026-10-31: Chrome computes only hwb() to rgb(); lab(), lch(), oklab(), oklch() and color() stay as such a string, which X3DParser.convertColor reads as rgb(): the first channel is NaN and the rest shift one place (emissiveColor NaN ...). A NaN gradient stop color makes addColorStop throw and X_ITE reject the file; the port does the same. Baselines: oracle/synthetic-rea-b24-css4 (5 identical recordings)."
C = {"sub:parse.svg": (None, FIELDS, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i75.py check|apply")
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
