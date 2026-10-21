#!/usr/bin/env python3
"""Batch 09 ledger update. Edits ledger.json directly (never run build_ledger.py: status.json is stale).
    python3 -I ledger_b09.py apply     # reads ledger/before/ledger.json, writes the authoritative ledger.json
Each entry: new status, tests appended, missing, note appended. Old notes are kept as a prefix."""
import json, os, sys
HERE = os.path.dirname(os.path.abspath(__file__))
BEFORE = os.path.join(HERE, "..", "ledger", "before", "ledger.json")
AUTH = os.path.join(HERE, "..", "..", "xite-rust-full-port-rea-completion", "ledger", "ledger.json")
IN = "crates/x3d-render/tests/render_rea_b09_inputs.rs::rea_b09_inputs_renders_match_x_ite"
ST = "crates/x3d-viewer/tests/rea_batch09_inputs.rs::rea_batch09_inputs_cases_match_x_ite"
NAV = "crates/x3d-viewer/tests/rea_batch09_nav.rs::rea_batch09_nav_shard0..3_match_x_ite"
SH = "crates/x3d-viewer/tests/rea_batch08_shaders.rs::rea_batch08_shader_cases_match_x_ite (stubs-runtime, MSAA 0/4)"
# id: (status, tests, missing, note)
P = {
 "node:FogCoordinate": ("COMPLETE", [IN + " (fogcoord-* 9 worlds, MSAA 0/4)"], "", "Batch 09 d8cce9b. fogcoord-geometries MSAA0 89 px (<0.5%)."),
 "node:Tangent": ("COMPLETE", [IN + " (tangent-* 10 worlds, attrib-tangent-fogdepth)"], "", "Batch 09 d8cce9b. All tangent worlds 0 px."),
 "node:Matrix4VertexAttribute": ("COMPLETE", [IN + " (attrib-matrix4, attrib-dup-name)"], "", "Batch 09 d8cce9b. Matrix3VertexAttribute and FloatVertexAttribute stay PARTIAL (no Matrix3 world; stale-after-shrink elements not modelled)."),
 "node:MultiTextureCoordinate": ("COMPLETE", [IN + " (multitex-channels, texcoord-mapping, phong-textures-mapping)"], "", "Batch 09 d8cce9b."),
 "node:TextureCoordinateGenerator": ("COMPLETE", [IN + " (texgen-modes/params/prims)"], "", "Batch 09 d8cce9b. Owner-accepted non-blocking difference: texgen-modes NOISE-eye MSAA0 404 px (0.154%), fma residue in the Perlin hash."),
 "node:Fog": ("COMPLETE", [ST + " (fog-runtime)", IN + " (fogcoord-*)"], "", "Batch 09 d8cce9b/d44ed6f."),
 "node:Background": ("COMPLETE", [ST + " (background-runtime)"], "", "Batch 09 d8cce9b. TextureBackground stays NOT_TESTED."),
 "node:Viewpoint": ("COMPLETE", [ST + " (viewpoint-runtime)", NAV + " (transition worlds)"], "", "Batch 09 d8cce9b."),
 "node:Billboard": ("COMPLETE", [ST + " (billboard-runtime)"], "", "Batch 09 d8cce9b."),
 "sub:nav.none": ("COMPLETE", [NAV + " (NONE cases)"], "", "Batch 09 4146c68. NoneViewer has no touch handlers in X_ITE."),
 "sub:nav.transitions": ("COMPLETE", [NAV + " (12 transition worlds x2, interruption cases)"], "", "Batch 09 4146c68."),
 "node:ShaderProgram": ("COMPLETE", [SH], "", "Batch 09 12d0e46. X_ITE has no behavior; stubs-runtime cases match."),
 "node:ProgramShader": ("COMPLETE", [SH], "", "Batch 09 12d0e46. X_ITE 38-line stub; stubs-runtime cases match."),
 "node:PackagedShader": ("COMPLETE", [SH], "", "Batch 09 12d0e46. Owner decision 2026-10-19: the write-stop quirk is accepted as a difference."),
 "node:VolumeEmitter": ("PARTIAL", [], "Owner decision 2026-10-18/19: replays of the pinned-X_ITE cases were not run; stays PARTIAL.", "Batch 09 downgrade per owner decision."),
 "node:BoundedPhysicsModel": ("PARTIAL", [], "Owner decision 2026-10-18/19: replays of the pinned-X_ITE cases were not run; stays PARTIAL.", "Batch 09 downgrade per owner decision."),
}
def apply():
    d = json.load(open(BEFORE))
    for i in d["items"]:
        p = P.get(i["id"])
        if not p: continue
        st, tests, missing, note = p
        i["status"] = st
        i["tests"] = list(i.get("tests") or []) + tests
        i["missing"] = missing
        i["notes"] = (i.get("notes", "") + " | " if i.get("notes") else "") + note
    json.dump(d, open(AUTH, "w"), indent=1)
if __name__ == "__main__" and sys.argv[1:] == ["apply"]:
    apply()
