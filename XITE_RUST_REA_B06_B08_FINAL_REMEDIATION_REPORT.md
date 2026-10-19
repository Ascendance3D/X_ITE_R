# XITE_RUST_REA_B06_B08_FINAL_REMEDIATION_REPORT

Lane `XITE_RUST_REA_B06_B08_FINAL_REMEDIATION`. Worktree `integration-b06-b08`, branch `integration/xite-rust-b06-b08-final`, started at `d6355339`. Reference: pinned X_ITE 16.5.0 (`34cdf9f97`). Nothing was pushed, merged or tagged. Batch 09 was not started. CodeMirror is outside the X_ITE port and was not run or counted.

## 1. Verdict
**XITE_RUST_REA_B06_B08_REMEDIATION_PARTIAL**
P1, P2, P3, P5 and `#include` are done. P4 (two sub-ulp particle cases) and P6 (rb-a MSAA4 multi-viewer frame 40) remain open, and the remaining Batch 08 entries are still PARTIAL.

## 2. Starting state
HEAD `d6355339` verified. The closure report's Section 15 was prose only; this run adds a real file-by-file restore comparison log (Section 18).

## 3. P1 physics picking: FIXED
Cause: Ammo's nested compound-compound dbvt walk culls a pair at the inner level because a non-uniformly scaled cylinder's AABB z-extent is narrower than its support radius. `PickShapes::manifold` now calls `Placed::trees_overlap` (`pick_shapes.rs`). Cases 4697, 5480 and 11878 now agree. 20,000-case sweep at seeds 20261018, 1, 2 and 3: 0 differences. New test `nested_compound_tree_culls_like_ammo` (3 vectors). Rigid-body frame-rate matrix (rates r24-r144, stacks r24-r144): all 47/47 and 11/11 pass.

## 4. P2 shader GL-state: FIXED
Ported X_ITE's texture-unit residency (ordered Set, weak per-unit bindings, default units, shared programs, relink reset). `textures-runtime` is 9/9 and the new `textures-shared` 9/9 at MSAA 0 and 4 (18/18 stepped each). Covered: null SFNode sampler, replacement texture, shared program, relink, group removal, unload, reused IDs, runtime field changes. Unit tests in `shader_gpu.rs`.

## 5. Shader lifecycle and `#include`: IMPLEMENTED
`#include <Name>` follows X_ITE's ShaderRegistry (26 files, `#line` numbering) with literal tests. Worlds include-unknown/utils/tonemapping pass at MSAA 0 and 4. 15 of 26 includes link standalone; 11 need macro/uniform context. A `const in` rewrite was added for naga. Shader entries stay PARTIAL (transformFeedbackVaryings not ported, 11 includes only unit-tested).

## 6. P3 volume initialization: PASS
IsoSurfaceVolumeData and SegmentedVolumeData, route-before/route-after/direct-after, 12 runs at MSAA 0 and 4. Pinned X_ITE ignores late `voxels` writes; Rust matches. A control showed the test is not vacuous.

## 7. P4 particle numerics: NOT RECOVERED
VolumeEmitter offset (1.72e-5) reproduces exactly: an origin ulp amplified by a grazing ray (one particle, frame 20). BoundedPhysicsModel small box (6.68e-6 vs 5.96e-6 limit): velocity already differs by 9.5e-7 at birth (PointEmitter spherical sin/cos/sqrt). A fused surface point fixed the first (8e-7) but pushed `volume-twocubes-stat` to 2.1e-4 against 1e-4; a 192-pattern grid found no formula satisfying both, so it was reverted. Limits not widened. Both are pinned as permanent tests in `particles_b06_rem.rs` (`oracle/rea-batch06-rem/`). Particle oracles: b06 37/37, b07 103/103.

## 8. P5 volume style lifecycle: PASS
Eight styles (boundary, cartoon, composed, edge, opacitymap, projection, silhouette, tonemapped), three rounds of update/clear/assign/remove/add, then unload; resources release to zero. 16 X_ITE parity runs pass. Test `rea_rem_volume_styles.rs`.

## 9. P6 `x_ite_two_canvases_rigid_bodies_msaa4`: OPEN (still fails, limit not relaxed)
Frame 40 is 0.579% against the 0.5% limit. Not a multi-viewer defect: pair equals alone in both engines. Cause: from frame 8 on, rb-a takes one Bullet substep fewer than X_ITE where `sum / fixedTimeStep` is one ulp under an integer (9.999999999999998). Rounding the quotient to float32 repairs rb-a exactly, but every float32 variant tried (quotient, residue, entry rounding, full-float32) regresses the rate corpus, and center-of-mass step 3 has the same boundary value yet X_ITE takes 9 there. The shipped `world.rs` is unchanged. Regression: `physics_rem_fall.rs` gates frames 1-7 exactly; the full 45-frame run is `#[ignore]` with the reason. Needs a faithful model of the asm.js float state at that step.

## 10. P7 remaining Batch 08 entries
All Batch 08 suites pass (net render+viewer+reload+script unload, glTF, GaussianSplats, VolumeScatter, SVG, shaders, CSS colors; `evidence/b06b08-rem/gates/b08-summary.txt`). No further code change. They stay PARTIAL for the documented gaps: InlineGeometry (copy() routes), FontLibrary (CFF), glTF/GLB (Draco, meshopt, BasisU, AVIF/WebP), GaussianSplats (shadow, picking, culling, async sort), VolumeScatter (owner reading of item 5), SVG (modern CSS colors, 43 hsl values), HTML parser (owner reading), multi-viewer (P6). Five DIS entries: COMPLETE (parity, no packet IO in X_ITE), evidence confirmed.

## 11. Workspace tests
`cargo build --release --workspace --locked` exit 0. `cargo test --release --workspace --no-fail-fast --locked`: 173 ok groups, 1 failing test (`x_ite_two_canvases_rigid_bodies_msaa4`, P6).

## 12. Oracle suites
items2 296/296, items 606/606, x0 47+3 expected, x1 470+25 expected+3 explained mismatch (0 unexplained), inline scripts 18/18, x6 8/8, EXTERNPROTO 12/12, followers 10/10 (649 checks), future-gate 12/12, reload pass, sound 65+1 known, physics 47/47, particles 37/37 and 103/103, pick vectors 4000/4000. Logs in `evidence/b06b08-rem/gates/`.

## 13. GPU MSAA 0 and 4
Adapter: NVIDIA GeForce RTX 3060 Ti, Vulkan, driver 595.91.07 (llvmpipe also present, not used); X_ITE side ANGLE on the same card. items2 shots: 294/296 within tolerance at MSAA 0 and at MSAA 4. The two over are `art/djascendance-live` (1.85%/1.84%) and `art/rainbow-room` (2.54%/2.55%), identical to the prior run, known corpus exceptions.

## 14. Ledger
Authoritative `ledger/ledger.json` updated from the proposal; `ledger_verify.py`: OK, 346 entries. Before: COMPLETE 214, PARTIAL 52, DIFFERENT 25, NOT_TESTED 33, MISSING 22. After: **COMPLETE 219, PARTIAL 67, DIFFERENT 25, NOT_TESTED 33, MISSING 2** (63.29%). The brief expected 215/71; the difference is the five DIS entries, promoted because their evidence was confirmed. DIFFERENT preserved (25); MISSING are WebXR and keyboard UI. Pre-remediation copies are in `~/backups/xite-rust-b06-b08/`. `status.json` was not regenerated.

## 15. Remaining PARTIAL and DIFFERENT
67 PARTIAL, 25 DIFFERENT, 33 NOT_TESTED (unchanged). `sub:physics.engine` stays PARTIAL on the P6 substep boundary. ComposedShader/ShaderPart stay PARTIAL.

## 16. Commits
See `git log d6355339..HEAD` (picking, GL-state, `#include`, volume init, particles pin, volume styles, P6, ledger).

## 17. Owner decisions needed
Accept or reject the P4 pins as known differences; whether P6 is worth a deeper asm.js float-state model; the VolumeScatter and HTML-parser contract readings.

## 18. Final backup and restore
See `~/backups/xite-rust-b06-b08/final-rem/` (`BACKUP_EVIDENCE.md`, `restore-compare.log`).

## 19. Not done
P4 and P6 fixes; transformFeedbackVaryings; context-dependent include linking; CodeMirror (out of scope).

## 20. Batch 09
Not started.
