#!/usr/bin/env python3
"""Batch 20 ledger update: python3 -I ledger_b20.py check|apply

Starts from ledger/before-b20/ledger.json (the authoritative ledger at b5f1301: 249 COMPLETE, 70 PARTIAL, 25 DIFFERENT, 2 MISSING).
Only node:VolumeEmitter and node:BoundedPhysicsModel change, and both stay PARTIAL. Their batch 09 missing text ("replays of the
pinned-X_ITE cases were not run") and the batch 06R gap list ("> 32 BVH hits dropped", "PolylineEmitter, WindPhysicsModel not
ported", "no renderer output", Group removal untested) predate PR #27 (B16 BoundedPhysicsModel replay), PR #28 (B17 VolumeEmitter
hits past ARRAY_SIZE) and batches 07 and 13. Each entry gains that evidence; its missing text keeps only what is still unverified.
Earlier notes stay as a prefix, so the correction of the 06R list is appended."""
import json, os, sys, collections
HERE = os.path.dirname(os.path.abspath(__file__))
ROOT = os.path.abspath(os.path.join(HERE, "..", ".."))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-b20", "ledger.json")
B16 = ("crates/x3d-viewer/tests/rea_b16_bounded_physics.rs (oracle/synthetic-rea-b16-bounded, 26 cases replayed with the batch 06R "
       "tolerances: 31 to 128 ray hits above / below / mixed / floor-last, zero damping on a floor, a box and mixed geometry, damping "
       "change, enabled toggle and late enable, interpolated normals on a quad and a sphere, tilted and transformed bounds, Coordinate "
       "rebuild and geometry replacement; plus independent_viewers, 27 tests); oracle/tools/rea_b16_bounded.py record (three identical "
       "X_ITE runs per case)")
B17 = ("crates/x3d-viewer/tests/rea_b17_emitter_hits.rs (oracle/synthetic-rea-b17-emitter-hits, 20 cases replayed with the batch 06R "
       "tolerances: emission lines through stacks of 31, 32, 33, 34, 40, 48, 64 and 128 quads, 33 / 40 / 64 / 128 also listed top-down, "
       "shuffled and unevenly spaced; plus independent_viewers, 21 tests); oracle/tools/rea_b17_emitter_hits.py record (three "
       "identical X_ITE runs per case)")
RUST = ["crates/x3d-runtime/src/particles.rs"]
PIN = ("pinned X_ITE 16.5.0 (oracle/x_ite-pin, PINNED_COMMIT 34cdf9f97; every baseline's scene text reads 'X_ITE V16.5.0')")
SHARED = ("Unverified, shared with the other entry: the clamp past ARRAY_SIZE was measured on one GPU and driver (RTX 3060 Ti, "
          "Chrome / ANGLE) and GLSL leaves out-of-range array writes undefined, so other WebGL backends may differ; X_ITE pushes "
          "onto its BVH_STACK_SIZE 32 traversal stack without a bound check while the port stops pushing at 32, and no case "
          "reaches that depth; results are within the batch 06R tolerances (velocity 3e-6 + 1e-6 |v|, position 5e-6 + 2e-6 |p|), "
          "not bit-exact; a bare Runtime without the viewer's geometry provider has empty emitter and bounded geometry "
          "(it warns once; crates/x3d-runtime/tests/geometry_provider_bare.rs).")
CORR = ("Corrections to the Batch 06R gap list above: '>32 BVH hits dropped' no longer holds (B16 for the bounce, B17 for "
        "emission); PolylineEmitter and WindPhysicsModel are ported (node:PolylineEmitter, node:WindPhysicsModel, batch 07); "
        "'no renderer output' no longer holds (ParticleSystem rendering, batch 13, rea_b13_particles.rs); ParticleSystem "
        "removal from a live Group is tested (particles_b07.rs, Group removal 50 cycles). The batch 09 owner decision "
        "('replays of the pinned-X_ITE cases were not run') is answered for this node by the replay above; the status "
        "stays PARTIAL until the owner decides otherwise.")
NOTE = "Batch 20 2026-10-24: "
C = {
 "node:VolumeEmitter": ("PARTIAL", RUST, [B17],
   "Not replayed: hits past 32 with a nonzero speed or variation (the B17 cases emit with speed 0 from stacked parallel quads), "
   "and BVH traversal deeper than 32 entries. " + SHARED,
   " + " + PIN + " VolumeEmitter particle records for 31 to 128 ray hits (oracle/rea-batch17-emitter-hits-baseline, 20 cases, "
   "every frame's transform feedback buffer; three runs byte-identical); max position error 2.9e-6",
   NOTE + "stays PARTIAL. B17, PR #28 (lane XITE_R_PARTICLE_EMITTER_HIT_LIMIT_01, head 7edfae9, merge 3c04cae), compared "
   "with " + PIN + ": X_ITE's volume getIntersections writes points [count ++] with no ARRAY_SIZE check, and the port "
   "stopped storing and counting at 32 hits, so emission positions were off by up to 1.17 units once a line met 33 or more "
   "triangles. get_intersections now returns the true count, hits past the 31st overwrite the last entry, and comb_sort and "
   "random_volume_position sort and pick among the whole count with clamped indices. The 18 cases of 33 to 128 hits failed "
   "before and pass after; 31 and 32 hits were unchanged; two viewers stepped in turn equal each alone, bit for bit. "
   "get_intersections is shared with BoundedPhysicsModel (PR #27), and both suites pass together. " + CORR),
 "node:BoundedPhysicsModel": ("PARTIAL", RUST, [B16],
   "Not replayed: a geometry change at a time a TimeSensor-started Script chooses (that Script did not run under the oracle "
   "protocol in X_ITE or Rust, so bp-rebuild and bp-replace are driven by a field event), and BVH traversal deeper than 32 "
   "entries. " + SHARED,
   " + " + PIN + " BoundedPhysicsModel particle records (oracle/rea-batch16-bounded-baseline, 26 cases, every frame's "
   "transform feedback buffer; three runs byte-identical)",
   NOTE + "stays PARTIAL. B16, PR #27 (lane XITE_R_BOUNDED_PHYSICS_REPLAY_01, head c65657a, merge dad5100), compared with "
   + PIN + ": 25 of 26 cases matched the port as it was; X_ITE's bounded getIntersections writes points [count] / "
   "normals [count] past ARRAY_SIZE 32, and the port dropped every hit after the 32nd, so bp-hits-64-below and "
   "bp-hits-64-floor-last bounced on the floor instead of at y = -0.55 / -0.60. Hits past the 31st now overwrite the last "
   "entry (the last hit in BVH order stays) and all 26 match. Recorded X_ITE behavior that the port now matches: zero damping "
   "puts zero normals in the BVH and particles pass through, with no non-finite values; the bounce runs in the "
   "ParticleSystem's local space (tilted floor, rotated and non-uniformly scaled Transform); per-vertex normals are "
   "interpolated, their length is the damping and the normalized normal reflects; enabled and damping events take effect on "
   "the next frame; a Coordinate change and a geometry replacement rebuild the bounded BVH. Two viewers stepped in turn "
   "equal each alone, bit for bit. " + CORR),
}
def load(p): return json.load(open(p))
def build():
    d = load(SNAP)
    for i in d["items"]:
        if i["id"] in C:
            s, rust, tests, missing, oracle, note = C[i["id"]]
            if s: i["status"] = s
            i["rust"] = i["rust"] + [r for r in rust if r not in i["rust"]]
            if missing is not None: i["missing"] = missing
            if oracle: i["oracle"] = i["oracle"] + oracle
            i["tests"] = i["tests"] + tests
            i["notes"] = (i["notes"] + " " if i["notes"] else "") + note
    return d
def check(d):
    bad, snap = [], {i["id"]: i for i in load(SNAP)["items"]}
    cur = {i["id"]: i for i in d["items"]}
    if set(cur) != set(snap) or len(d["items"]) != 346: bad.append("ID set or total changed")
    want = {i["id"]: i for i in build()["items"]}
    for k, b in snap.items():
        a = cur[k]
        if k not in C and a != b: bad.append(f"unrelated entry changed: {k}")
        if b["status"] == "DIFFERENT" and a != b: bad.append(f"DIFFERENT changed: {k}")
        if k in C and (a["status"] != (C[k][0] or b["status"]) or not a["notes"].startswith(b["notes"])): bad.append(f"{k}: status or notes")
        if k in C and a != want[k]: bad.append(f"{k}: differs from what apply writes")
    c = collections.Counter(i["status"] for i in d["items"]); print(dict(c))
    print("\n".join(bad) or "OK"); return not bad
if __name__ == "__main__":
    if sys.argv[1] == "apply":
        d = build(); json.dump(d, open(AUTH, "w"), indent=1); sys.exit(0 if check(load(AUTH)) else 1)
    sys.exit(0 if check(load(AUTH)) else 1)
