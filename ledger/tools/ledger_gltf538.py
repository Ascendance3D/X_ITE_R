#!/usr/bin/env python3
"""Ledger step of PR #538 (2026-11-10): the glTF animation cases (STEP, LINEAR, CUBICSPLINE, pointer) in cargo tests against
pinned X_ITE; STEP morph weights hold (an upstream defect not copied, ADR 0001 row 15).
python3 -I ledger/tools/ledger_gltf538.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-gltf538", "ledger.json")
SNAP_SHA256 = "afd06161dcff3a334b4971a2971b8696ab2593248d625572464bb22df80c259f"

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
 "sub:parse.gltf2": (None, {
   "missing": swap("stepped animation cases run only by x10_compare.py", "the animation cases run in cargo tests (gltf_anim_step, 32 cases, MSAA 0 and 4, PR #538) with one kept difference: STEP morph-target weights hold their keyframe value, where pinned X_ITE interpolates over the segment (ADR 0001 row 15); at MSAA 4 the pinned page records no console and no Inline text, so only scene text, poses and pictures are compared there; pointer/roughness-step.gltf fails to load in both (KHR_materials_emissive_strength on the material: Cannot set properties of null), so two plain-material STEP pointer cases were added"),
   "tests": append("crates/x3d-viewer/tests/gltf_anim_step.rs (translation, rotation, scale, weights with STEP, LINEAR and CUBICSPLINE, STEP pointer roughness and base color, and the three driver cases of oracle/rea-b08-gltf-baseline; oracle/rea-gltf-anim-baseline, pinned X_ITE 16.5.0, three identical recordings per case; scene text, Inline text, log, poses 1e-4, pictures 8 / 0.5%; PR #538)"),
  }, "PR #538 (2026-11-10): the hand-run x10 animation comparison is now a cargo test with a committed pinned baseline (30 new cases, all reproducible). One Rust change, red first: the port had copied X_ITE's STEP weights (keys t0, t1, t1, ... with values v0, v1, v1, ...: linear over each segment); it now holds (crates/x3d-parse/src/gltf2/animation.rs create_array_interpolator), after which gltf-anim-weights-step and animation/weights-step.gltf differ from the pinned page exactly where pinned does not hold; both are pinned by name (KNOWN, KNOWN_STEP_WEIGHTS) with the held values asserted. REA: analysis of GLTF2Parser.js ev_222019cb812d18eaea4fce15828db0937af498e6e1ec918354b892e8d4902bf0 (summary ev_574936dd6a3d55cca00f913b7fd62ff0257f6465eb8eaccbafbad9a720f24a7c), partial graph. Video playback frames stay open: PARTIAL."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_gltf538.py check|apply")
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
