#!/usr/bin/env python3
"""Batch 13 ledger update: python3 -I ledger_b13.py check|apply

Starts from ledger/before-b13/ledger.json (the authoritative ledger at bde3e2f: 250 COMPLETE, 69 PARTIAL, 25 DIFFERENT, 2 MISSING).
Only node:ParticleSystem changes. PR #21 (fb929a8) added the ParticleSystem renderer; the entry was COMPLETE although nothing was
drawn, and it goes to PARTIAL with the rendering evidence and the gaps PR #21 left open as its missing list."""
import json, os, sys, collections
HERE = os.path.dirname(os.path.abspath(__file__))
AUTH = os.path.join(HERE, "..", "ledger", "ledger.json")
SNAP = os.path.join(HERE, "..", "ledger", "before-b13", "ledger.json")
B13 = ("crates/x3d-viewer/tests/rea_b13_particles.rs::rea_b13_particle_cases_match_x_ite (oracle/synthetic-rea-b13-particles, "
       "35 cases, frames 15 / 25 / 45: particle records and 512 x 512 images at MSAA 0 and 4; KNOWN_GAPS pr-line msaa4)")
B07 = "crates/x3d-viewer/tests/particles_b07.rs, particles_b06_exact.rs, particles_b06_stat.rs, particles_b06_events.rs (simulation unchanged by the renderer)"
RUST = ["crates/x3d-runtime/src/particles.rs", "crates/x3d-runtime/src/nodes/particle_systems.rs", "crates/x3d-render/src/particles.rs",
        "crates/x3d-render/src/traverse.rs"]
NOTE = "Batch 13 2026-10-23: "
C = {
 "node:ParticleSystem": ("PARTIAL", RUST, [B13, B07],
   "particle picking (the pointing pass and walk.rs do not see particles); particle collision; shadow-pass culling of the "
   "particle bbox (the shadow pass draws every particle, including off-screen systems); thick LINE particles (LineProperties "
   "linewidthScaleFactor > 1); particle systems in Inline and Script-created render cases (the seed recorder handles one system "
   "per world); GEOMETRY with a texCoord ramp has no oracle case; one draw call per particle (instance data through per-draw "
   "uniforms); pr-line MSAA 4 frame 45 differs by 0.581% of the pixels (line-edge antialiasing, KNOWN_GAP)",
   " + pinned X_ITE 16.5.0 particle records and 512 x 512 images (anim.html, anim-msaa.html multisampling 4) captured with "
   "x3d-oracle particles (oracle/rea-batch13-particles-baseline)",
   NOTE + "PARTIAL: rendering added by PR #21 (lane XITE_R_PARTICLE_RENDER_01, merge fb929a8). The earlier COMPLETE rated the "
   "simulation only; the port drew nothing. Rendered and compared with X_ITE (rea_b13_particles, 35 cases): POINT, LINE, "
   "TRIANGLE, QUAD, SPRITE and GEOMETRY geometry types; particleSize and the scale ramp; RGB and RGBA color ramps (and no "
   "color node); texCoord ramps; Appearance, textures, the default appearance and unlit particles; run-time field changes "
   "(enabled, maxParticles, particleSize, visible); bboxSize, bounded-physics and emitter bounding boxes and view-volume "
   "culling; bboxDisplay; castShadow on and off. Every particle record matches; 209 of 210 image comparisons pass. Known gap: "
   "pr-line MSAA 4 frame 45, 0.581% (the same 45 segments as a static IndexedLineSet differ alike: 1524 px vs 1523, so it is "
   "line-edge antialiasing, not particle code). The simulation is unchanged (rs.f32 identical with and without image capture; "
   "B06 and B07 pass)."),
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
