#!/usr/bin/env python3
"""Issue #91 ledger update (the five navigation subsystems): python3 -I ledger_i91.py check|apply

Each entry has an item list derived from X_ITE 16.5.0's Browser/Navigation sources (docs/reports/NAV_ITEM_LISTS.md): examine 18 items, walk-fly 18, plane 10, lookat 7,
collision-gravity 19. 40 new cases (rea_b24_nav, 3 identical recordings each) match pinned X_ITE; one Rust bug was fixed (avatarSize[0] = 0). The entries stay PARTIAL for the
items listed as missing (touch, macOS scroll factor, Rubberband, ContentScale, two-layer gravity). No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i91", "ledger.json")
SNAP_SHA256 = "a5b2bc67192d27be3180a42be596c4afa1857106779afca091e1c5ddefb7709e"

T = ["crates/x3d-viewer/tests/rea_b24_nav.rs (rea_b24_nav_shard0_match_x_ite, shard1, shard2: 40 cases, pose tolerance 1e-4, probe distance 1e-5; item lists in docs/reports/NAV_ITEM_LISTS.md)"]
ORACLE = "pinned X_ITE 16.5.0 (34cdf9f97): oracle/rea-batch24-nav-baseline (40 cases, 3 identical recordings), oracle/synthetic-rea-b24-nav"
def f(missing, note):
    return ({"tests": lambda old: old + T, "oracle": ORACLE, "missing": missing}, note)
NOTE = "Issue #91 2026-10-31: item list in docs/reports/NAV_ITEM_LISTS.md."
EX = f("not compared: two-finger touch pan and double tap (E15), a press outside the viewport (E16), the macOS scroll factor (E17); E14 (touch rotate and pinch) only in the x3d-web browser lane, E18 only through x10_compare.py on a private world", NOTE + " 13 of 18 items compared in committed tests; new cases: centerOfRotation, OrthoViewpoint, trackball rotation and spin, spin stopped by a wheel step.")
WF = f("not compared: Rubberband overlay (F15), touch (F16), ContentScale (F17), macOS roll angle (F18)", NOTE + " 14 of 18 items compared; new cases: StraightenHorizon false, FLY look-around past vertical, speed 0, 1 and 16, walk inside a tilted Transform.")
PL = f("not compared: touch (P09), macOS scroll factor (P10)", NOTE + " 8 of 10 items compared; new cases: zoom-in limit (225 wheel steps), double click, Alt double click.")
LA = f("not compared: touch (L06), macOS scroll factor (L07)", NOTE + " 5 of 7 items compared; new case: lookat around a Viewpoint centerOfRotation.")
CG = f("not compared: active-layer gravity with two Layers (C17); C18 only indirectly through the trans-*-walk poses; probe distance differs from X_ITE by up to 1.3e-6 (5.7e-6 on a slanted wall, where the shape id also differs) so the new cases allow 1e-5", NOTE + " Non-particle cases C01-C16 compared: ceiling collision, step-up threshold (stepHeight 0, 0.75, 3), avatarHeight, collision radius 0 and 1, avatarSize defaults, speed, Gravity option. Fixed: avatarSize[0] = 0 makes X_ITE's collision depth NaN, so the avatar cannot move; collision_queries now returns NaN for radius 0.")
C = {"sub:nav.examine": (None,)+EX[:1]+(EX[1],), "sub:nav.walk-fly": (None,)+WF[:1]+(WF[1],), "sub:nav.plane": (None,)+PL[:1]+(PL[1],), "sub:nav.lookat": (None,)+LA[:1]+(LA[1],), "sub:nav.collision-gravity": (None,)+CG[:1]+(CG[1],)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i91.py check|apply")
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
