# Render-inputs lane (batch 09) — commits d8cce9b, d44ed6f (local, not pushed)

Workspace: 989 passed, 0 failed, 1 ignored (gen_oracle_worlds); lead re-ran and confirmed.
Report text saved by the lead from the agent hand-back (agent could not write the file).

## Tests (MSAA 0 and 4 vs pinned X_ITE, no tolerance widened)
- rea_b09_inputs_renders_match_x_ite (crates/x3d-render/tests/render_rea_b09_inputs.rs): 47 static worlds.
  Non-zero diffs, all < 0.5%: fogcoord-geometries MSAA0 89 px; gs-bboxdisplay MSAA0 17 px;
  texgen-modes MSAA0 404 px (0.154%); texgen-params 14/15 px; all others 0 px.
- rea_batch09_inputs_cases_match_x_ite (crates/x3d-viewer/tests/rea_batch09_inputs.rs): 8 stepped cases
  (fog, background, viewpoint, billboard, collision x2, anchor x2). Scene text exact, poses 1e-4, images 8 levels / 0.5%.
- Baselines: oracle/render-rea-b09-inputs-baseline, oracle/render-msaa4-rea-b09-inputs-baseline, oracle/rea-batch09-inputs-baseline.
- Worlds/generators: oracle/synthetic-rea-b09-inputs, oracle/synthetic-rea-b09-stepped; runner oracle/tools/rea_b09_inputs.py.

## Implemented
FogCoordinate, LocalFog (innermost wins), IndexedLineSet fog, Tangent, Float/Matrix3/Matrix4VertexAttribute,
MultiTextureCoordinate, TextureCoordinateGenerator, texcoord mapping clamp, Phong textures + normal maps,
TextureBackground set_bind, Collision proxy/isActive/collideTime/enabled, Anchor #viewpoint, GaussianSplats bboxDisplay + fog.

## Recommended status
COMPLETE: FogCoordinate, Tangent, Float/Matrix3/Matrix4VertexAttribute, MultiTextureCoordinate,
TextureCoordinateGenerator, Fog, Background, Viewpoint, Billboard, LocalFog (geometry only; not volume draws),
Collision (viewer-driven cases; CPU fallback untested), Anchor (same-document #viewpoint; non-hash urls only reported).
PARTIAL: GaussianSplats (no castShadow pass, picking, collision, culling, log depth; sort sync vs async).

## DIFFERENT candidates
1. texgen NOISE-eye at MSAA 0: 404 px (0.154%), NVIDIA/ANGLE fma residue in Perlin hash.
2. Aliased lines at MSAA 0 (wgpu vs ANGLE line rules).
3. Group/Transform bboxDisplay not drawn at all (outside this lane).

## Owner decisions
1. Accept DIFFERENT 1 and 2 as non-blocking. 2. Group/Transform bboxDisplay in a later batch?
3. Own lane for GaussianSplats picking/shadow/culling/log depth (new GPU passes)? 4. Docstring of rea_b09_inputs.py mentions rea-batch08.
