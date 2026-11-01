#!/usr/bin/env python3
"""Issue #139 ledger note (node:MovieTexture, browser build): python3 -I ledger_i139.py check|apply

The browser build plays video through the page's <video> element and falls back to the next url when it does not play; GIF and APNG frames already ran through the
native decoder in wasm. 13 browser cases match the pinned X_ITE recording (3 identical runs). Status and the native `missing` text are left to the native entry; only the
evidence is added. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i139", "ledger.json")
SNAP_SHA256 = "3ecf06012763999ec61b13e19b06a72843b31e2a2636d60faa26883d9bd22c24"

T = ["crates/x3d-web/tests/browser/movie01.test.mjs (movieFrames.gif/apng/webm/mp4/gifData/gifOnce/gifSlow, movieFallback.brokenThenGif/brokenThenWebm/missingThenMp4/notGifThenWebm/firstPlays/noneplay; baseline oracle/web-browser-movie-01-baseline, 3 identical runs)",
     "crates/x3d-web/tests/async_loading.rs a_video_is_the_pages_element_and_a_video_that_fails_goes_on_with_the_next_candidate"]
FIELDS = {"tests": lambda old: old + T}
NOTE = "Issue #139 2026-10-31 (browser build): a non-GIF/PNG candidate goes to a page <video> element (video_mirror.mjs); a video that fails logs X_ITE's two messages, then the next url is requested. Frame colors over time (GIF, APNG, WebM, MP4, data-URL GIF, loop FALSE, speed 0.5), width, height, colorDepth, duration_changed and the fallback messages match. Browser differences: video sound plays through the element, not X_ITE's createMediaElementSource graph (gain, Mute, spatialization do not reach it); video ops flush only from the frame tick, so a video does not start while the frame loop is stopped; the WebM and MP4 frame cases (movieFrames.webm, movieFrames.mp4) are skipped on CI because the hosted runner's video decode exceeds the sampling window, so they run only locally."
C = {"node:MovieTexture": (None, FIELDS, NOTE)}

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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i139.py check|apply")
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
