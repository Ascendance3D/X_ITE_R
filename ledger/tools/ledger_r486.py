#!/usr/bin/env python3
"""Ledger step for PR #486 and the #469 texgen measurement: python3 -I ledger_r486.py check|apply

Two text-only changes.
- sub:nav.examine: PR #486 proves the cause of the remaining touchsnapshot01 drift (#472, line 66): the same split
  two-finger touchmove as the line-88 drift, at the second pinch step. The hold of PR #482 covers it; #472 is closed.
- node:TextureCoordinateGenerator: the accepted texgen-modes 404 px (NOISE eye, MSAA 0) is not the Perlin hash: it is
  the same with dot and with fma on every measured path (#469).

No status changes. No DIFFERENT entry is touched. Counts: 284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING before and
after, total 346."""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger.json")
SNAP = os.path.join(HERE, "..", "snapshots", "before-r486", "ledger.json")
SNAP_SHA256 = "ea2bc34a69420bb43ee996d9fe66956105b2c707334c1648ce92e8833814a6b2"

def swap(old_text, new_text):
    def f(old):
        assert old.count(old_text) == 1, old_text
        return old.replace(old_text, new_text)
    return f
def addlist(*texts):
    return lambda old: old + list(texts)

C = {
 "sub:nav.examine": (None, {
   "missing": swap("; open: one touchsnapshot01 pinch drift (#472, line 66, x = -0.0036) has no proven cause, and the CI failure rate is not measured after PR #482", ""),
  }, "Issue #472 2026-11-06: PR #486 (test only, merged at 8d547828). The line-66 drift (x = -0.0036 on CI, -0.003572172 locally) has the cause of the line-88 drift: a two-finger touchmove that arrives as two DOM events with a frame between the halves, here at the second pinch step (160/240 -> 150/250); the reverse finger order gives +0.003572172. A split of the first step does not move the camera. Fault matrix without the hold: 141 runs, 48 forced-split failures with exactly these values and 1 natural step-2 failure. With the hold of PR #482: 10 of 10 pass with the step-1 fault, 10 of 10 fail without it. touchsnapshot01 now sends the second step as two halves in both finger orders inside the hold (red with the hold removed: 3 of 3 runs fail). The CI failure rate is not measured (owner decision 2026-11-06); #472 is closed. Text only: the entry stays PARTIAL."),
 "node:TextureCoordinateGenerator": (None, {}, "Issue #469 2026-11-06: measured. Pinned X_ITE's NOISE hash is dot (Perlin2.glsl.js), and its rounding follows the shader compiler: on one NVIDIA RTX 3060 Ti, ANGLE on OpenGL gives fma(y, b, x * a) and ANGLE on Vulkan fma(x, a, y * b); Apple M1 Metal gives the first. texgen-params of pinned X_ITE differs between ANGLE OpenGL and ANGLE Vulkan on the same GPU (5,665 px). The accepted texgen-modes 404 px (NOISE eye, MSAA 0) is the same with dot and with fma on every measured path, so it is not the Perlin hash; its cause is not known. The x3d.wgsl change to dot and the two re-recorded NVIDIA texgen-params pictures are PR #487, which waits for owner review. Text only: the entry stays COMPLETE."),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_r486.py check|apply")
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
