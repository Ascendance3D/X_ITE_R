# render2 lane report (GaussianSplats, VolumeScatterMaterialExtension)

Worktree: .worktrees/rust-port-private/r0-3-e-retention-cleanup (WT). Build env: CARGO_TARGET_DIR=/tmp/b08/render2/target, CARGO_BUILD_BUILD_DIR=/tmp/b08/render2/build.

## Recommended status
- node:GaussianSplats: PARTIAL. Items 1,2,3,4,5,7 met. Missing: castShadow shadow pass, pointerEvents picking, collision, bboxDisplay, shape sphere culling, reversed/log depth modes.
- node:VolumeScatterMaterialExtension: COMPLETE candidate (items 1-5 and 7; item 6 n/a), with item 5 read as the contract says (assigned/cleared on a live material, Shape removed from live Group). Owner may reject that reading.

## Files
New: crates/x3d-render/src/{splats.rs,splats.wgsl,splats_gpu.rs,traverse_splats.rs,scatter_gpu.rs}; oracle/tools/{gen_rea_b08_gs.py,gen_rea_b08_vs.py,rea_b08_render2.py}; tests x3d-render/tests/render_rea_b08_{gs,vs}.rs, x3d-viewer/tests/rea_batch08_{gs,vs}.rs, rea_b08_gs_reload.rs, rea_b08_vs_reload.rs; corpora oracle/synthetic-rea-b08-{gs,vs}; baselines oracle/render-rea-b08-{gs,vs}-baseline, render-msaa4-rea-b08-{gs,vs}-baseline, rea-batch08-{gs,vs}-baseline (X_ITE runs only).
Shared hunks: lib.rs (3 mods); traverse.rs (DrawItem.splats, RenderCache.splats, GaussianSplats arm, EXT_VOLUME_SCATTER, multiscatter_color, EXTENSION_TEXTURES 15->16, MATERIAL_EXTENSIONS, volume_scatter()); gpu.rs (Source::Splats, splat prepare/draw/prune, scatter pass, frame group bindings 8/9, FrameU.inv_projection and pass_flags.y, DrawU.ext 10, MAT_KINDS 21, FLOAT32_BLENDABLE feature requested, scatter_resources/splat_resources); x3d.wgsl (scatter block, EXT_VOLUME_SCATTER, bindings 8/9, 21 mats).

## Results vs X_ITE
GaussianSplats: 35 static worlds x MSAA 0 and 4: max delta 2, 0 pixels over 8. Stepped gs-runtime (21 frames) and gs-route (3 frames) at MSAA 0 and 4 pass. X_ITE stepped runs repeat byte-identically (checked twice).
VolumeScatter: 19 worlds x MSAA 0/4 (38 shots) pass under the 0.5% bound. Worst: vs-lights MSAA 4 0.225%, vs-transparent-ibl 0.055% (max 118, edges), vs-point-lines 3 px. vs-runtime (16 frames, 15 keys) passes at MSAA 0 and 4; X_ITE repeats identically.
Lifecycle: gs reload test (9 worlds x 5 reloads, same picture, resources not growing, [0,0,0,0] after unload) and gs removal/Script-created/emptied test pass. vs reload test (8 worlds x 5 reloads) and extension-cleared/Shape-removed test pass; scatter buffer is 1 while used, 0 after KEEP_FRAMES and after unload. x3d-render full test suite passed (incl. render_rea_b07_mat). x3d-viewer: only my tests plus rea_batch08 ones run; full viewer suite not run.

Commands: cargo test --release -j 8 -p x3d-render --test render_rea_b08_gs / render_rea_b08_vs; -p x3d-viewer --test rea_batch08_gs / rea_batch08_vs / rea_b08_gs_reload / rea_b08_vs_reload. Regenerate: python3 -I oracle/tools/gen_rea_b08_{gs,vs}.py oracle/synthetic-rea-b08-{gs,vs}; baselines: python3 -I oracle/tools/rea_b08_render2.py {gs|vs} shots | case.

## X_ITE quirks / hangs (recorded, not reproduced further)
- GaussianSplats never displayed (empty, or visible FALSE) never finishes loading (first sort never arrives): cases removed.
- Inner shape castShadow starts TRUE; degree -1 without SH gives gray 0.5; SH texture stale across rebuilds; sort is async (lag); orientations/opacities/SH half-rounded.
- VolumeScatter: scatterStrength/scatterAnisotropy unused by shaders; with scatterStrengthTexture the world never loads (hang), left out. TransmissionMaterialExtension plus scatter without an EnvironmentLight never loads (hang; scatter shader calls getDiffuseLight without IBL), only covered with IBL (vs-combined-ibl). Two materials per world needed to stay under the harness 45 s, so combined/textures worlds are split a/b. Without a Volume extension attenuationDistance is undefined in GLSL (vs-missing-ibl matches on this GPU only).
- A root ClipPlane also clips the Background in X_ITE; Rust does not (not this node); vs-clip uses a Group-local plane. Appearance.alphaMode is not read by the Rust scene, so not used in worlds.

## Known limits
Splat sort is synchronous in Rust. Splat shadows, picking, collision, bboxDisplay, sphere culling, reversed/log depth unimplemented. Scatter buffer read-back uses storage buffers + compute depth copy (no sampled texture binding left); reversed/log depth modes and XR untested for scatter. scatterStrengthTexture parsed no further (no shader reads it).

## Owner decisions
1. Accept contract item 5 reading for extensions. 2. Accept GaussianSplats as PARTIAL, or request shadows/picking. 3. OrthoViewpoint fieldOfView is typed MFFloat in Rust scene vs SFVec4f in X_ITE (pre-existing). 4. Async sort lag vs synchronous sort. 5. The two X_ITE load hangs are upstream bugs; not reported upstream.
