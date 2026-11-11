#!/usr/bin/env python3
"""Ledger step of PR #542 (2026-11-10): viewers on separate OS threads and a second particle pair, compared with pinned X_ITE;
LogarithmicDepthBuffer switch made per thread.
python3 -I ledger/tools/ledger_mv542.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-mv542", "ledger.json")
SNAP_SHA256 = "fd92da9cf7ba426c02f54defe4c35396bcaa7723bd44111ad82b7c77d942c3f7"

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
 "sub:browser.multi-viewer-thread": (None, {
   "missing": swap("viewers on different threads, particle pictures, sound, second device not tested; process-global geometry provider", "viewers on separate OS threads (a per-frame Barrier so the draws overlap; script, rigid-body, particle and sound pairs and a six-thread mix, MSAA 0 and 4) equal the single-thread run exactly and the pinned two-canvas recordings within 0.5%; particle pictures with two viewers are compared (pairs p-a/p-b and the new p-c/p-d: LINE, QUAD, TRIANGLE and POINT particles, Cone, Polyline and Volume emitters, WindPhysicsModel, ColorRamp; PR #542); the geometry provider is one process-wide stateless function (thread-safe); not tested: audible sound of two viewers (sound is compared through the SAI state only), a second GPU device (max_line_width and metal_native_lines are process-global per backend); observed but not tracked here: an ExplosionEmitter with LINE particles differs from pinned X_ITE by 0.738% at MSAA 0 with one viewer (node:ParticleSystem)"),
   "tests": append("crates/x3d-viewer/tests/multi_viewer_b08.rs threads_* (OS threads) and x_ite_two_canvases_particles_more_msaa0/4 (oracle/multiview-b08-baseline p-c, p-d, p-c+p-d, three identical recordings; PR #542); crates/x3d-render unit test logarithmic_depth_switch_is_per_thread"),
  }, "PR #542 (2026-11-10): one Rust fix, red first: the LogarithmicDepthBuffer switch was a process-wide AtomicBool (crates/x3d-render/src/traverse.rs), so two viewers on two threads with the option on and off overwrote each other's switch (logarithmic_depth_switch_is_per_thread failed: the other thread's switch is seen); it is now thread-local and travels in BrowserGlobals like the browser qualities. Tests: multi_viewer_b08 41 passed, 1 skipped; x3d-render and x3d-viewer 751 of 752 (the one failure, backend_gpu gl_backend_cannot_run_the_renderer_and_says_so, fails on master on this host: no GL device). Status stays PARTIAL: sound output and a second device."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_mv542.py check|apply")
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
