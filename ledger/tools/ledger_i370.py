#!/usr/bin/env python3
"""Ledger step after the navigation touch and press suites (issue #370): python3 -I ledger_i370.py check|apply

Four sub:nav entries, text only, no status change. The browser lane (headless Chromium, DevTools touch and pointer events)
now compares the cases the item lists named as missing: E15 (#372), F16 (#373, #386), P09 and L06 (#374, #382, #389), E16
(#375); #395 makes a partial move of a two-finger gesture wait for the rest, as X_ITE's touchmove has every finger. What stays
open is kept in `missing`."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i370", "ledger.json")
SNAP_SHA256 = "cace1323f7fc134a2f9e9db304fce0b861d527122a7361c8c47ac8719de5a8c4"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f
def addlist(*texts):
    return lambda old: old + list(texts)

ORACLE = "oracle/web-browser-nav-0{n}-baseline (pinned X_ITE 16.5.0, 34cdf9f97, three identical recordings; recorder oracle/x_ite-web-runtime-02.mjs --suite nav0{n})"
COMMON = "Pose numbers are compared at 1e-3, fixed before the first run. The input is synthetic DevTools touch and pointer events in headless Chromium; physical touch screens stay in #143 and macOS scroll factors in #88. The cases are in the browser lane (crates/x3d-web) only, there is no x3d-viewer test for them. #395 (touchsnapshot01.test.mjs): the page glue held a move of some fingers until the rest moved, as X_ITE's touchmove has every finger; before it the hosted runner saw the first finger of a PLANE zoom step moved and the second not (x = -0.003346939 where the pinned x is 0), after it the case ends where a pinch with both fingers moving ends; pointer-only input (no touch events) is unchanged."

C = {
 "sub:nav.examine": (None, {
   "tests": addlist("crates/x3d-web/tests/browser/nav01.test.mjs (nav01_case.mjs: pan, panMode, doubleTap, slowTaps, doubleTapEmpty, against " + ORACLE.format(n=1) + ")",
                    "crates/x3d-web/tests/browser/nav04.test.mjs (nav04_case.mjs: a press, drag, wheel, touch drag and pinch outside and inside a Viewport, against " + ORACLE.format(n=4) + ")",
                    "crates/x3d-web/tests/browser/touchsnapshot01.test.mjs"),
   "missing": swap("not compared: two-finger touch pan and double tap (E15), a press outside the viewport (E16), the macOS scroll factor (E17); E14",
                   "not compared: the macOS scroll factor (E17); E14")},
  "Issue #370 2026-11-03 (PRs #372, #375, #395): E15 (two-finger touch pan, touch mode lock, double tap look at, slow taps, empty space) and E16 (a press, wheel and touch outside a layer's Viewport) are compared against pinned X_ITE in the browser lane; no implementation defect, no engine change. " + COMMON),
 "sub:nav.walk-fly": (None, {
   "tests": addlist("crates/x3d-web/tests/browser/nav02.test.mjs (nav02_case.mjs: WALK and FLY one-finger forward, back, turn, diagonal and two-finger look-around, against " + ORACLE.format(n=2) + ")"),
   "missing": swap("not compared: Rubberband overlay (F15), touch (F16), ContentScale (F17)",
                   "not compared: Rubberband overlay (F15), ContentScale (F17)")},
  "Issue #370 2026-11-03 (PRs #373, #386, #395): F16 (touch) is compared in the browser lane. Defect found and fixed: X3DFlyViewer.touchstart, touchmove and touchend were not ported, so a finger did nothing in WALK and FLY; one finger now flies or walks, two fingers look around, a third ends it. X_ITE walks over real frame times, so a hold cannot be compared pose for pose: the cases compare the direction of travel and the speed and turn rate over the measured hold. " + COMMON),
 "sub:nav.plane": (None, {
   "tests": addlist("crates/x3d-web/tests/browser/nav03.test.mjs (nav03_case.mjs navPlane: move, zoom, mode, oneFinger, doubleTap, slowTaps, against " + ORACLE.format(n=3) + ")"),
   "missing": swap("not compared: touch (P09), macOS scroll factor (P10)", "not compared: macOS scroll factor (P10)")},
  "Issue #370 2026-11-03 (PRs #374, #382, #389, #395): P09 (two-finger pan, pinch zoom, double tap) is compared in the browser lane, all 6 navPlane cases with none skipped on the hosted runner (the page clock is held across the taps of a double tap). PLANE zoom changes fieldOfViewScale, which the Viewpoint node does not show, so the zoom cases also compare the picture. " + COMMON),
 "sub:nav.lookat": (None, {
   "tests": addlist("crates/x3d-web/tests/browser/nav03.test.mjs (nav03_case.mjs navLookAt: move, zoom, mode, oneFinger, doubleTap, slowTaps, against " + ORACLE.format(n=3) + ")"),
   "missing": swap("not compared: touch (L06), macOS scroll factor (L07)", "not compared: macOS scroll factor (L07)")},
  "Issue #370 2026-11-03 (PRs #374, #382, #389, #395): L06 (two-finger pan, pinch, double tap with DBL_TAP_TIME) is compared in the browser lane, all 6 navLookAt cases with none skipped on the hosted runner. " + COMMON),
}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i370.py check|apply")
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
