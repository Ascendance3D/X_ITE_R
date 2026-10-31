#!/usr/bin/env python3
"""Issue #93 ledger update (particle replay gaps, PR #208). Three entries change; no status changes; no DIFFERENT entry is in the change set.

Starts from ledger/before-i93/ledger.json (sha256 42e81e78b06bfe6798b92eb40b7d99d26b9577349bb8b9a22c8295cd92960815). python3 -I ledger_i93.py check|apply"""
import json, os, sys, collections, hashlib
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-i93", "ledger.json")
SNAP_SHA256 = "42e81e78b06bfe6798b92eb40b7d99d26b9577349bb8b9a22c8295cd92960815"

def sub(old, new):
    def f(text):
        if old not in text: raise SystemExit(f"text not found: {old!r}")
        return text.replace(old, new, 1)
    return f
def add(v):
    return lambda old: old + [v]

T13 = 'crates/x3d-viewer/tests/rea_b13_particles.rs (pr-script-created-node, pr-script-created-geometry, pr-inline, pr-inline-transform, pr-geometry-texture, pr-geometry-texramp, pr-geometry-texramp-rgb; oracle/rea-batch13-particles-baseline, 3 identical X_ITE runs)'
T17 = 'crates/x3d-viewer/tests/rea_b17_emitter_hits.rs (ve-speed-33/64/128, -down, -shuffle, -128-uneven, ve-speed-64-bounded: nonzero-speed hits past 32; oracle/rea-batch17-emitter-hits-baseline, 3 identical X_ITE runs)'
TBVH = 'crates/x3d-runtime/tests/particles_bvh_depth.rs (BVH depth up to 70000 triangles)'
BVH = 'BVH traversal deeper than 32 entries cannot be reached in X_ITE (BVH.js splits at the median: depth = ceil(log2 N), so 32 needs more than 2^31 triangles; pinned by crates/x3d-runtime/tests/particles_bvh_depth.rs)'
NOTE = "Issue #93 2026-10-31 (PR #208): nonzero-speed VolumeEmitter hits past 32, Script-created and Inline ParticleSystems (images and seeds) and a texCoord ramp on GEOMETRY systems recorded from pinned X_ITE (3 identical runs each). Rust defect fixed: a texCoord ramp on a GEOMETRY system gives every vertex the ramp's zero texel in X_ITE (texCoordCount is 0); Rust used the geometry's own coordinates (3.9 to 4.6% of the pixels)."
# id: (status or None, {field: value or function of the old value}, note). The verifier reads the first element.
C = {
 "node:BoundedPhysicsModel": (None, {"tests": add(TBVH), "missing": sub(", and BVH traversal deeper than 32 entries.", ". " + BVH + ".")}, NOTE),
 "node:VolumeEmitter": (None, {"tests": lambda old: old + [T17, TBVH],
    "missing": sub("Not replayed: hits past 32 with a nonzero speed or variation (the B17 cases emit with speed 0 from stacked parallel quads), and BVH traversal deeper than 32 entries.", BVH[0].upper() + BVH[1:] + ".")}, NOTE),
 "node:ParticleSystem": (None, {"tests": add(T13),
    "missing": sub("particle systems in Inline and Script-created render cases (the seed recorder handles one system per world); GEOMETRY with a texCoord ramp has no oracle case; ",
        "particle records of a system inside an Inline are not compared (images and seeds are: pr-inline, pr-inline-transform; the dumper looks up root-scene nodes only); a texCoord ramp on POINT or LINE systems is not modeled or recorded; ")}, NOTE),
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
    if sys.argv[1:] not in (["check"], ["apply"]): raise SystemExit("usage: ledger_i93.py check|apply")
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
