#!/usr/bin/env python3
"""Ledger step of PR #535 (2026-11-10): the OrderIndependentTransparency option compared in the transparency reference
worlds. python3 -I ledger/tools/ledger_oit535.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-oit535", "ledger.json")
SNAP_SHA256 = "1332a09c9adf876f302d8f1da01acd523ee546ac0a88453a733a36880fe1c96a"

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
 "sub:render.transparency-sorting": (None, {
   "missing": swap("OIT is not compared in the reference worlds;", "OIT in the reference worlds is compared at MSAA 0 in all 8 (i92-tr-many, -nested, -opaque, -order, -ortho, -points-lines, -textured, i92-fg-transparent) and at MSAA 4 in 6; i92-tr-many and i92-tr-nested have no OIT MSAA 4 reference (X_ITE's three runs differ, msaa4/unstable.json), so MSAA 4 OIT parity is not claimed for them; i92-fg-transparent has one transparent layer per pixel and cannot tell OIT from sorted blending (negative control);"),
   "tests": append("crates/x3d-render/tests/render_rea_oit_i92_items.rs (OIT on against oracle/rea-oit-i92-render-items-baseline, pinned X_ITE 16.5.0, three identical recordings per pass; 14 shots within 8 / 0.5%; negative control: with the option off 12 of 14 shots differ by 2.1% to 34%, the two i92-fg-transparent shots are pinned by name; PR #535)"),
  }, "PR #535 (2026-11-10): the transparency reference worlds recorded with the OrderIndependentTransparency option on (oracle/tools/rea_i92_render_items.py shots --oit-i92) and replayed by render_rea_oit_i92_items: all 14 shots within the bound (largest i92-tr-opaque 0.362% at MSAA 0, 0.336% at MSAA 4; i92-tr-points-lines 0.174% at MSAA 0), no Rust change needed. REA: analysis ev_bdb28dc35b50b73af2c0a0f9bae3faab1c34b450a2bd853bcfaf96bfa886153e covers X3DRenderObject.js (drawShapes, the OIT accumulation path; partial graph). The MSAA 4 references of tr-many, tr-ortho and fg-transparent without OIT, and the two OIT MSAA 4 references above, stay open: PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_oit535.py check|apply")
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
