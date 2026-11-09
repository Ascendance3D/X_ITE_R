#!/usr/bin/env python3
"""Ledger step of PR #531 (2026-11-09): the input rays and hit points of the XR input sources are drawn as in X_ITE.
python3 -I ledger/tools/ledger_xr531.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-xr531", "ledger.json")
SNAP_SHA256 = "eea0c27aea5746db37535f850d1efe01c16eae70df13b086b0fe9b8970051a48"

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
 "sub:browser.webxr": (None, {
   "missing": swap("; the input rays and hit points of the XR input sources are not drawn (X_ITE draws them with ScreenLine and ScreenPoint in xrUpdatePointers; the pick and the sensor events are implemented)", ""),
   "tests": append("crates/x3d-render/tests/xr_views.rs stereo-pointers (five input sources: rays and hit points against pinned pictures of oracle/xr-shot.html, 3 identical recordings, MSAA 0 and 4, bound 0.05%; PR #531)",
                   "crates/x3d-viewer/tests/xr_rays.rs (the hit points and ray-space normals of the input sources equal pinned X_ITE within 1e-4; PR #531)"),
  }, "PR #531 (2026-11-09): X3DWebXRContext.js:346-520 with ScreenLine.js and ScreenPoint.js draws a ray for every active input source (white, or 0.5 0.75 1 while a button is pressed, cut at the hit) and a ring and disk at a pick hit, blended without depth test, per view. The port draws the same in x3d-render (xr.rs, xr_pointer.rs) for the web XR frame. Before: 1472 pixels (1.12%) over 8 at MSAA 0 with no rays; after: 14 pixels (0.011%) at MSAA 0 and 0 at MSAA 4. Limits: WebGL2 only, emulated device, no physical headset; the projection viewport is each view's own size (pinned: the last reshaped framebuffer, equal for equal views). Native OpenXR (#83) stays open, so the status stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_xr531.py check|apply")
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
