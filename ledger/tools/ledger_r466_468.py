#!/usr/bin/env python3
"""Ledger step for PRs #466 and #468: python3 -I ledger_r466_468.py check|apply

Two text-only changes.
- sub:proto.externproto: PR #466 adds case externproto-from-url (createX3DFromURL with an EXTERNPROTO after load) to
  crates/x3d-viewer/tests/rea_batch09_sai.rs. createVrmlFromURL and loadURL with an EXTERNPROTO after load are not recorded.
- sub:nav.walk-fly: PR #468 compares the rubber band at Multisampling 4 (14 cases). The ContentScale and browser-option
  cases at Multisampling 4 and Viewport nodes stay open.

No status changes. No DIFFERENT entry is touched. Counts: 284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING before and
after, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-r466-468", "ledger.json")
SNAP_SHA256 = "eb77dbd7ce07481b119f4941ed3aa40d514f2a54878fba8834a392f5e56e8298"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f
def chain(*fs):
    def f(old):
        for g in fs: old = g(old)
        return old
    return f
def addtext(text):
    return lambda old: old + "; " + text
def addlist(*texts):
    return lambda old: old + list(texts)
def note(text):
    return lambda old: (old + " | " if old else "") + text

C = {
 "sub:proto.externproto": (None, {
   "missing": addtext("createVrmlFromURL and loadURL with an EXTERNPROTO after load are not recorded (only createX3DFromURL, case externproto-from-url)"),
   "tests": lambda old: [t[:-1] + "; case externproto-from-url: createX3DFromURL after load for a file with an EXTERNPROTO next to the scene, a missing url, a scene-cache hit and the same file twice; cache hits resolve at once, at frames 14 and 17 as in X_ITE)" if t.startswith("crates/x3d-viewer/tests/rea_batch09_sai.rs") else t for t in old],
  }, "Issue #51 2026-11-06: PR #466 (747e3e96). Case externproto-from-url (oracle/synthetic-rea-b09-sai/externproto-from-url.wrl, pinned X_ITE 16.5.0, three identical recordings, oracle/rea-b09-sai-baseline/externproto-from-url/js.json): the values were equal, but the port resolved the two scene-cache hits one frame late (frames 15 and 18; X_ITE 14 and 17). The deferred hand-over of #458 now applies only to scenes made from a string; a createX3DFromURL scene takes a Ready hit at once. Text only: createVrmlFromURL and loadURL are not recorded, so the entry stays PARTIAL."),
 "sub:nav.walk-fly": (None, {
   "missing": swap("MSAA above 0 and Viewport nodes for the rubber band", "Viewport nodes for the rubber band, and the ContentScale and browser-option rubber band cases at MSAA 4 (the MSAA recorder has neither key)"),
   "tests": addlist("crates/x3d-viewer/tests/rea_nav_overlay.rs (rea_nav_overlay_rubber_band_msaa4_matches_x_ite: 14 rubber band cases at MSAA 4, x3d-drive --msaa 4 --gl-samples, poses within 1e-4, pixels within 1; oracle/rea-nav-overlay-msaa4-baseline and oracle/rea-nav-overlay-msaa4-apple-m1-baseline, three identical recordings each)"),
  }, "Issue #51 2026-11-06: PR #468 (55b0c529). At Multisampling above 0 X_ITE draws the rubber band into its multisampled framebuffer before the blit (X3DBrowserContext.js:264-269); the port drew it into the resolved target (red: rubber-walk-up-right, 270 of 304 pixels over 1). The last pass now keeps the samples when a band follows, and the band pass resolves into the target. 14 cases at MSAA 4 match on Linux NVIDIA and, with the apple-m1 references, on the physical M1. Text only: Viewport nodes and the ContentScale and browser-option cases at MSAA 4 stay open, so the entry stays PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_r466_468.py check|apply")
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
