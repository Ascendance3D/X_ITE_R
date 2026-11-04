#!/usr/bin/env python3
"""Ledger step after the audit of the 44 PARTIAL entries (issue #51): python3 -I ledger_i452.py check|apply

Two promotions, PARTIAL to COMPLETE. In each entry the `missing` text names one gap only, and that gap is closed on master;
the entry stayed PARTIAL with no other gap listed. Both were re-run on master 59d7a31b with X3D_EVIDENCE=strict before this
step (NVIDIA GeForce RTX 3060 Ti, Vulkan; reference ANGLE OpenGL ES, Chrome 156):

- node:DepthMode: the gap "the main color pass is not compared for a reversed depthRange" was closed by PR #407 (9b3774eb):
  render_depthmode_reversed, control and reversed worlds 0 pixels over 8 at MSAA 0 and 4. Also passed: render_rea2,
  pointing_depthmode (7), depthmode_pick, rea_b23_depthmode_pick and rea_b23_depthmode_pick_cpu (58 cells each).
- sub:physics.engine: the gap "rb-a takes one substep fewer than pinned X_ITE at frame 8" was closed by PR #8 (510f48fc,
  a7f1b787; issue #60): the harness measured the frame rate from clock deltas. physics_rem_fall covers all 45 frames within
  2e-6 with no ignore; multi_viewer_b08 x_ite_two_canvases_rigid_bodies_msaa4 is 0.00000 at frames 5, 20, 40. Also passed:
  bullet_b08_engine (37), bullet_dyn_scenes (66), physics_b08_lifecycle (5), physics_b06_baseline (48),
  physics_b06_lifecycle (6), rea_b03_pick_vectors (2), oracle/tools/rea_b06.py check (47 of 47 worlds).

No DIFFERENT entry is touched. Counts: 280 COMPLETE / 44 PARTIAL / 21 DIFFERENT / 1 MISSING before,
282 COMPLETE / 42 PARTIAL / 21 DIFFERENT / 1 MISSING after, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-i452", "ledger.json")
SNAP_SHA256 = "c42d6ce0f38571f79c0fa631e4795f691d2d126aed8f0534b37092807cab454d"

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
 "node:DepthMode": ("COMPLETE", {
   "missing": swap('main color pass: a reversed depthRange (near > far) is ignored like WebGL (INVALID_OPERATION, the range stays the default), measured against the pinned page at MSAA 0 and 4 with 0 pixels over 8 (render_depthmode_reversed)', ""),
  }, "Issue #51 2026-11-04 (ledger audit): promoted PARTIAL to COMPLETE. The only listed gap, the main color pass with a reversed depthRange, was closed by PR #407 (9b3774eb): render_depthmode_reversed 0 pixels over 8 at MSAA 0 and 4. Re-run on master 59d7a31b with X3D_EVIDENCE=strict on the RTX 3060 Ti (Vulkan): render_depthmode_reversed, render_rea2, pointing_depthmode (7), depthmode_pick, rea_b23_depthmode_pick and rea_b23_depthmode_pick_cpu (58 cells each) pass."),
 "sub:physics.engine": ("COMPLETE", {
   "missing": swap('the earlier rb-a substep difference at frame 8 was the harness measuring the frame rate from clock deltas; with currentFrameRate pinned to 1/DT (510f48fc) rb-a and rb-b equal pinned X_ITE at MSAA 4 (issue #60)', ""),
   "tests": lambda old: [("physics_rem_fall (45 frames within 2e-6, no ignore: P6 closed, a7f1b787)" if x == "physics_rem_fall (frames 1-7; full run ignored, open)" else x) for x in old],
  }, "Issue #51 2026-11-04 (ledger audit): promoted PARTIAL to COMPLETE. The only listed gap, rb-a one substep short at frame 8, was the harness frame rate (PR #8: 510f48fc pins currentFrameRate, a7f1b787 gates all 45 frames; issue #60). Re-run on master 59d7a31b with X3D_EVIDENCE=strict: bullet_b08_engine (37), bullet_dyn_scenes (66), physics_b08_lifecycle (5), physics_b06_baseline (48), physics_b06_lifecycle (6), rea_b03_pick_vectors (2), physics_rem_fall (45 frames), multi_viewer_b08 x_ite_two_canvases_rigid_bodies_msaa4 pass; oracle/tools/rea_b06.py check 47 of 47 worlds."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i452.py check|apply")
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
