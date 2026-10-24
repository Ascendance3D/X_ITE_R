#!/usr/bin/env python3
"""Batch 19 ledger update: python3 -I ledger_b19.py check|apply

Starts from ledger/before-b19/ledger.json (the authoritative ledger at d4cd4d5: 249 COMPLETE, 70 PARTIAL, 25 DIFFERENT, 2 MISSING).
Only node:ParticleSystem changes, and it stays PARTIAL. Its batch 13 missing list named particle picking, particle collision and
thick LINE particles; PR #24 (B14 picking), #26 (B15 avatar collision), #29 (B18 wide lines) and #32 (B19 CPU collision) did
that work, and #28 (B17) changed VolumeEmitter emission positions. The entry gains that evidence, and its missing list keeps
only the gaps that remain."""
import json, os, sys, collections
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-b19", "ledger.json")
B14 = ("crates/x3d-viewer/tests/rea_b14_particle_pick.rs::rea_b14_particle_pick_matches_x_ite (oracle/synthetic-rea-b14-particle-pick, "
       "22 cases, 3,108 probes at frames 15 / 30 / 45; KNOWN_GAPS pp-row-line, pp-row-point-props, pp-slant-point-props); "
       "crates/x3d-viewer/tests/particle_pick_viewers.rs (3)")
B15 = ("crates/x3d-viewer/tests/rea_b15_particle_collision.rs::rea_b15_particle_collision_matches_x_ite and "
       "rea_b15_particle_collision_cpu_matches_x_ite (oracle/synthetic-rea-b15-particle-collision, 14 cases, GPU depth pass and CPU "
       "stand-in; KNOWN_GAPS pc-line frame 45, CPU_TRIANGLES); crates/x3d-viewer/tests/particle_collision_viewers.rs (3)")
B17 = ("crates/x3d-viewer/tests/rea_b17_emitter_hits.rs (oracle/synthetic-rea-b17-emitter-hits, 20 cases of 31 to 128 VolumeEmitter "
       "ray hits, plus independent_viewers)")
B18 = ("crates/x3d-viewer/tests/rea_b18_particle_lines.rs::rea_b18_particle_line_cases_match_x_ite (oracle/synthetic-rea-b18-particle-lines, "
       "12 cases: LINE particles at widths 2 / 6 / 10 / 11 / 24, dashed, scale ramp, mirror, and InstancedShape lines; MSAA 0 and 4)")
RUST = ["crates/x3d-viewer/src/walk.rs", "crates/x3d-viewer/src/pointer.rs", "crates/x3d-viewer/src/collision.rs",
        "crates/x3d-render/src/depth_gpu.rs", "crates/x3d-render/src/gpu.rs"]
NOTE = "Batch 19 2026-10-24: "
C = {
 "node:ParticleSystem": ("PARTIAL", RUST, [B14, B15, B17, B18],
   "shadow-pass culling of the particle bbox (the shadow pass draws every particle, including off-screen systems); wide LINE "
   "particles are 1 px in the shadow, depth (collision) and pointing passes (PR #29 changed the color pass only); POINT "
   "particles with PointProperties are 1 px in the collision depth pass (no getPointSize or round mask); the CPU stand-ins: "
   "pointing without a pointing GPU approximates POINT and LINE particles at edges (377 probes in pp-row-line, 28 to 36 in the "
   "point-props cases), and collision without a depth GPU samples triangle types at pixel centers with geometric normals "
   "(CPU_TRIANGLES value gaps, about 1e-5), compares depth in f32, and covers exact-edge POINT pixels by one Vulkan driver's "
   "measured rule; particle systems in Inline and Script-created render cases (the seed recorder handles one system per "
   "world); GEOMETRY with a texCoord ramp has no oracle case; one draw call per particle (instance data through per-draw "
   "uniforms) and one pointer ShapeInst per particle per frame; KNOWN_GAPS: pr-line MSAA 4 frame 45 0.581% of the pixels "
   "(line-edge antialiasing), pc-line frame 45 (a line leaving the collision buffer through its right edge, 1.7e-5 in depth), "
   "pp-row-line up to 3 ulps and the point-props cases 1 ulp in gl_PointCoord",
   " + pinned X_ITE 16.5.0 pointer probes (oracle/rea-batch14-particle-pick-baseline), collision probes "
   "(oracle/rea-batch15-particle-collision-baseline), VolumeEmitter particle records (oracle/rea-batch17-emitter-hits-baseline) "
   "and LINE particle records and images (oracle/rea-batch18-particle-lines-baseline), each recorded three times identically "
   "with x3d-oracle particles",
   NOTE + "stays PARTIAL. Done since batch 13, each compared with pinned X_ITE 16.5.0: "
   "B14 picking, PR #24 (lane XITE_R_PARTICLE_PICKING_01, merge 51dd084): particles are pointing shapes of the pointer walk; "
   "the hit shape matches at every probe of 22 cases, and with X_ITE's records in place 19 cases are bit-exact and 3 differ "
   "by 1 to 3 ulps (KNOWN_GAPS); TouchSensor events, two viewers, reloads and unloads are covered by particle_pick_viewers. "
   "B15 avatar collision, PR #26 (merge d2666bd): an active system is a collision shape in getClosestObject, "
   "constrainTranslation, gravity and stepping, and Collision isActive / collideTime, through the 16 x 16 GPU depth buffer; "
   "13 of 14 cases match everywhere, pc-line frame 45 is the KNOWN_GAP. "
   "B17 emitter behavior, PR #28 (merge 3c04cae): VolumeEmitter counts every ray hit past ARRAY_SIZE 32 with clamped indices "
   "as X_ITE does under WebGL; 20 cases from 31 to 128 hits match within the batch 06R tolerance (the clamp was measured on "
   "one GPU / ANGLE; BVH_STACK_SIZE stays capped at 32). "
   "B18 wide lines, PR #29 (merge 348aea5): wide and stippled LINE particles apply the instance matrix once, before the "
   "window-space width, and instanced dashed lines up to width 10 draw solid as in X_ITE; failing image passes went from 28 "
   "of 72 to 0 of 72, every particle record matches. "
   "B19 CPU collision, PR #32 (lane XITE_R_PARTICLE_CPU_COLLISION_01, merge d4cd4d5): the CPU depth stand-in fills its own "
   "16 x 16 buffer (LEQUAL, f32 window z, readDepth's first smallest pixel), draws POINT particles and merges LINE particles "
   "as the GPU does, and reports the hit shape, so Collision nodes activate without a depth GPU; all 14 B15 cases match "
   "the hit states, shapes, poses, positionOffset and Collision isActive / collideTime exactly. "
   "The B16 BoundedPhysicsModel replay (PR #27) belongs to node:BoundedPhysicsModel and is not recorded here."),
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
    for k, b in snap.items():
        a = cur[k]
        if k not in C and a != b: bad.append(f"unrelated entry changed: {k}")
        if b["status"] == "DIFFERENT" and a != b: bad.append(f"DIFFERENT changed: {k}")
        if k in C and (a["status"] != (C[k][0] or b["status"]) or not a["notes"].startswith(b["notes"])): bad.append(f"{k}: status or notes")
    c = collections.Counter(i["status"] for i in d["items"]); print(dict(c))
    print("\n".join(bad) or "OK"); return not bad
if __name__ == "__main__":
    if sys.argv[1] == "apply":
        d = build(); json.dump(d, open(AUTH, "w"), indent=1); sys.exit(0 if check(load(AUTH)) else 1)
    sys.exit(0 if check(load(AUTH)) else 1)
