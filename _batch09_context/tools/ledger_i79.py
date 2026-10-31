#!/usr/bin/env python3
"""Ledger step for issue #79, recorded after the fact: python3 -I ledger_i79.py check|apply

PRs for #79 (0e73f029 GPU pointing, bb3c07f5 CPU ray cast, "Closes #79") edited node:DepthMode in ledger.json without a
HISTORY step, so verify_ledger.py failed on master (entry outside the change set changed: node:DepthMode). This step starts
from ledger/before-i79/ledger.json (the ledger at origin/master e67034c9 with node:DepthMode as the #90 merge 9df5bce5 left
it, sha256 4efc4d14aec9f657679c84c98d5716ca328d23d344909ace6f077cc4bc32c12d) and reproduces the edit: the values below are the ones origin/master holds. The #140 edits
of node:AudioClip and node:Sound are inside the change set of ledger_i90.py and stay in the snapshot. Status stays
PARTIAL. No DIFFERENT entry is in the change set."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i79", "ledger.json")
SNAP_SHA256 = "4efc4d14aec9f657679c84c98d5716ca328d23d344909ace6f077cc4bc32c12d"
FIELDS = {
 "rust": [
  "crates/x3d-render/src/traverse.rs",
  "crates/x3d-render/src/pointing_gpu.rs",
  "crates/x3d-render/src/pointing.wgsl"
 ],
 "tests": [
  "crates/x3d-render/tests/render_rea2.rs",
  "crates/x3d-viewer/tests/rea_b23_depthmode_pick.rs::depthmode_pick_matches_pinned_x_ite (58 cells, adapter name and backend printed)",
  "crates/x3d-viewer/tests/rea_b23_depthmode_pick_cpu.rs::depthmode_cpu_pick_matches_pinned_x_ite (the same 58 cells on the CPU ray cast, no adapter needed)",
  "crates/x3d-render/tests/pointing_depthmode.rs",
  "crates/x3d-viewer/tests/depthmode_pick.rs"
 ],
 "missing": "the main color pass is not compared for a reversed depthRange (WebGL ignores near > far; only the pointing pass applies that rule)",
 "oracle": "x_ite-shot.mjs / x_ite-shot-msaa.mjs baselines oracle/render-rea2-baseline and render-msaa4-rea2-baseline: 10 worlds within threshold 8 / 0.5% at MSAA 0 and MSAA 4; oracle/rea-batch23-depthmode-pick-baseline/dm-pick (pinned 16.5.0, WebGL 2 on ANGLE/OpenGL, RTX 3060 Ti; 58 pointing cells: default, depthFunction x8, depthMask, depthTest, per-shape state, depthRange, polygonOffset; 5 identical recordings, then 58 of 58 match wgpu/Vulkan on the same GPU)"
}
NOTE = '; Wave 3 2026-10-31 (#79): GPU pointing applies DepthMode (X3DRenderObject.pointing, depthModeNode.enable). Rust defect fixed: a reversed depthRange (near > far) was applied; WebGL raises INVALID_OPERATION and keeps the default, so X_ITE ignores it. Negative checks: ignoring depthRange fails 4 cells and a unit test; applying a reversed range fails 3 cells and a unit test. Wave 3 2026-10-31 (#79, CPU path): the CPU ray cast runs its hits through a 24-bit depth buffer in drawing order (depth function, mask, test, range, polygon offset) when a candidate has a DepthMode; without one the nearest hit still wins in double. 58 of 58 cells match; ignoring DepthMode in the CPU path fails the cells.'

# id: (status or None, {field: value}, note). The verifier reads the first element.
C = {"node:DepthMode": ('PARTIAL', FIELDS, NOTE)}

def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, fields, note = C[i["id"]]
            if s: i["status"] = s
            for f, v in fields.items(): i[f] = v(i[f]) if callable(v) else v
            i["notes"] = i["notes"] + note
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i79.py check|apply")
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
