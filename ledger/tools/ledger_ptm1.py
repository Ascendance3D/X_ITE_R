#!/usr/bin/env python3
"""Ledger step of the Apple M1 touch recording (2026-11-11): the pinned page's touch history on ANGLE Metal, and the port on Metal
against it.
python3 -I ledger/tools/ledger_ptm1.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-ptm1", "ledger.json")
SNAP_SHA256 = "be0421cb4643f1146123c50be669fbfcc2b42e21e23d411762a952faed134ce2"

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
 "sub:pointer.picking": (None, {
   "tests": append("oracle/rea-pointer-touch-apple-m1-baseline (the 6 cases of rea_pointer_touch recorded on the physical Apple M1, Chrome 156 ANGLE Metal, three identical runs each; read with X3D_REFERENCE_PLATFORM=apple-m1)"),
  }, "2026-11-11 (macOS rea_pointer_touch): the hosted macOS run of PR #541 failed all 6 cases against the NVIDIA recording (10 lines, hit numbers 1 to 2 float32 ulps off, for example hitTexCoord y 0.4083078 / 0.4083079 and hitPoint y -0.1467075 / -0.1467073). Cause: PointingBuffer.getHit reads the point, normal and texCoord with gl.readPixels (RGBA, FLOAT) from three RGBA32F attachments and sets them into the hit with no arithmetic (oracle/x_ite-pin/src/x_ite/Rendering/PointingBuffer.js), so the numbers are the GPU's interpolated varyings; the port reads its pointing sample the same way (crates/x3d-viewer/src/pointer.rs gpu_hit). The pinned page on the physical M1 (ANGLE Metal) gives 0.4083079 and -0.1467073 itself, in three identical runs, and all 6 cases differ from the NVIDIA recording in the scene text only. The port on the M1 (wgpu Metal) fails against the NVIDIA recording (20 differing lines) and passes against the M1 recording with the same rules (rea_pointer_touch 1/1 with X3D_REFERENCE_PLATFORM=apple-m1: scene text exact in five cases, and in cancel the history before the first touchcancel and the last tap). No Rust change and no tolerance: the M1 recording is the reference on that hardware (owner decision 2026-11-04, issues #69 and #89). Status stays PARTIAL: the line and point ulp bounds."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_ptm1.py check|apply")
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
