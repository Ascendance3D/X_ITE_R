# Batch 08 lane `shaders`: report

Written 2026-10-18 in the cloud recovery session (repo `DJAscendance/X_ITE_R`, branch `batch-08-snapshot`).
The original shader agent stopped (out of credits) before writing a report. This report covers what it left on
disk, what the recovery session changed, and what was run here. The container has no GPU. GPU checks ran on
lavapipe (Mesa software Vulkan) for information only. They are **not** the GPU gate.

Entries: `node:ComposedShader`, `node:ShaderPart`, `node:ShaderProgram`, `node:ProgramShader`, `node:PackagedShader`.

## 1. Recovered implementation state

The saved code was complete enough to build and run. No `TODO`, `todo!` or `unimplemented!` remained.
Recovered pieces:

| Piece | File | State found |
| --- | --- | --- |
| Runtime: ShaderPart load and compile, ComposedShader private LoadSensor, link, `isValid`, `activate`, `language`, Appearance selection, `isSelected` counting | `crates/x3d-runtime/src/nodes/shaders.rs` (546 lines) | complete |
| GLSL ES 1.00 / 3.00 → GLSL 450 rewriter: preprocessor (`#if`/`#ifdef`/macros, `x3d_MaxLights`), X_ITE defines and custom options, uniform block packing, attribute and varying locations, sampler split, ES 1.00 → 3.00 forms, line-number shift for errors, link checks, deprecation warnings | `crates/x3d-render/src/glsl_compat.rs` (1,945 lines), `glsl_types.glsl` | complete |
| GPU: compiler installed on the runtime, built-in uniforms per draw, one pipeline per program and draw state, vertex buffers per geometry and attribute set, texture bind groups, 120-frame age-out | `crates/x3d-render/src/shader_gpu.rs` (786 lines) | complete |
| Traversal: user fields → uniforms (SF/MF, matrices, rotations, images), SFNode/MFNode textures → samplers | `crates/x3d-render/src/traverse_shaders.rs` (195 lines) | complete |
| Tests | `glsl_compat_b08.rs`, `render_rea_b08_shaders.rs`, `rea_b08_shaders_reload.rs`, `rea_batch08_shaders.rs` | see 6–9 |
| Corpus and baselines | `oracle/synthetic-rea-b08-shaders/` (88 worlds, 9 event scripts), `oracle/render-rea-b08-shaders-baseline/`, `oracle/render-msaa4-rea-b08-shaders-baseline/` (85 X_ITE shots each) | static baselines present; **stepped baseline `oracle/rea-batch08-shaders-baseline/` was never made** |

Defects found in the recovered state and fixed here:

1. `rea_b08_shaders_reload.rs` checked "an invalid shader drew" by counting GPU objects. The previous world's
   objects stay for `KEEP_FRAMES` = 120 frames, so `bad-syntax.wrl` failed with `[1,1,2,2,1]` although it drew
   nothing. The test now checks the custom draws of the last frame (`Renderer::shader_draws`). Valid worlds now
   need both a pipeline and a draw. No tolerance changed.
2. The runtime shader tables (`Runtime::shaders`: parts, composed, selection counts, appearances) were never
   pruned. NodeIds are reused, so a new node in a released slot could inherit an old selection or source.
   `release.rs` now drops them with the node. New test `rea_b08_release_tables.rs`. It fails without the fix
   (`[2,1,1,1]` kept after unload) and passes with it.
3. `rea_batch08_shaders.rs` had a header copied from Batch 07 (it named DispersionMaterialExtension and others)
   and a test named `rea_batch08_mat_cases_match_x_ite`. Header rewritten and test renamed
   `rea_batch08_shader_cases_match_x_ite`. It now fails with "baseline missing … comparison NOT RUN" instead of
   a bare `unwrap` panic.

## 2. Source files changed (lane total, start → now)

New: `crates/x3d-runtime/src/nodes/shaders.rs`, `crates/x3d-render/src/{glsl_compat.rs,glsl_types.glsl,shader_gpu.rs,traverse_shaders.rs}`,
tests `crates/x3d-render/tests/{glsl_compat_b08.rs,render_rea_b08_shaders.rs}`,
`crates/x3d-viewer/tests/{rea_b08_shaders_reload.rs,rea_batch08_shaders.rs,rea_b08_release_tables.rs}`,
`oracle/tools/{gen_rea_b08_shaders.py,rea_b08_shaders.py}`, the corpus and the two static baselines.

Shared hunks: `crates/x3d-render/Cargo.toml` (naga), `Cargo.lock`, `crates/x3d-render/src/{lib.rs,load.rs,sg.rs,traverse.rs,gpu.rs}`,
`crates/x3d-runtime/src/{lib.rs,nodes/mod.rs,loader.rs,release.rs}`, `crates/x3d-viewer/src/viewer.rs` (one line: `shader_gpu::install`).

## 3. Dependencies

Checked against the Batch 08 start (`backup/start/tracked.patch` applied to `03f7a28`):

- `crates/x3d-render/Cargo.toml`: `naga = { version = "=30.0.1", default-features = false, features = ["glsl-in", "wgsl-in", "wgsl-out"] }`.
  naga 30.0.1 was already in `Cargo.lock` through wgpu, at the same version. There is still one naga package.
- New packages in `Cargo.lock`: **`pp-rs 0.2.1`** (the approved one, needed by `glsl-in`) and **`unicode-xid 0.2.6`**
  (the small helper, a dependency of pp-rs). Nothing else was added or updated by this lane. (The svg lane
  added the internal path dependency `x3d-text` to `x3d-parse`. That is not an external crate.)
- Rule deviation: the lane rules name `glsl-in` (and `wgsl-in` if needed). The lane also turned on `wgsl-out`, a
  feature of the same crate with no new package. It is needed to write WGSL. Owner to confirm.
- Toolchain unchanged (no `rust-toolchain` file; container cargo 1.97.0).

## 4. Implemented behavior (as pinned X_ITE does it)

- ShaderPart: X3DUrlObject loading with url fallback, `data:` URLs (Chrome `fetch` rules, base64, percent
  escapes), `load`, `type` change (`set_type__` → reload). `getShaderType`: an unknown type is VERTEX.
  TESS_CONTROL, TESS_EVALUATION, GEOMETRY and COMPUTE fail with the `null` log, as WebGL 2 has no such
  types. Error text follows X_ITE: `Error in ShaderPart 'name' in URL 'file', line N, <log>`.
- Defines as `ShaderSource.getSource`: `X3D_VERTEX_SHADER`/`X3D_FRAGMENT_SHADER`, the custom options, `x3d_MaxLights 8`,
  the X_ITE types block, `#line` shift so error lines name the user line.
- ComposedShader: private LoadSensor over `parts`, `connectLoaded`, `set_loaded__` link when all parts are
  loaded and `language == "GLSL"`, `isValid`, `activate TRUE` relinks, `Couldn't initialize ComposedShader 'name': <log>`,
  deprecation warnings printed as X_ITE prints them.
- Appearance: the first valid shader in `shaders` (PROTO instances looked through), `isSelected` with a count
  (shared shader nodes), re-selection when `isValid` changes.
- Field-to-uniform: user-defined fields with X_ITE's class rules (a type mismatch leaves the uniform unchanged,
  MF fields need an array uniform), SFRotation/SFQuaternion → mat3, SFImage/MFImage → int arrays; SFNode/MFNode
  textures to samplers (an unset user `sampler2D` reads white; `samplerCube` and `sampler3D` read black, as
  measured against X_ITE).
- Built-in uniforms: projection, model view, normal, view, camera space, viewport, exposure, fog (new and legacy
  names), up to 8 lights (new and legacy names), 6 clip planes, materials (Unlit, Material, Physical,
  SpecularGlossiness), texture matrices, coordinate generators, multi-texture, point, line and fill properties,
  `x3d_AlphaCutoff`.
- Attributes: `x3d_Vertex`, `x3d_Normal`, `x3d_Color`, `x3d_TexCoord[0-3]`, `x3d_Tangent`, `x3d_FogDepth` and the
  Float/Matrix3/Matrix4 vertex attribute nodes.
- Pipeline selection per draw state: topology, cull, winding, blend and BlendMode, depth write, test and function,
  depth offset, MSAA, flipped targets, reversed depth.
- ShaderProgram, ProgramShader, PackagedShader: nodes and fields exist; X_ITE has no behavior for them
  (ProgramShader is a 38-line stub, PackagedShader prints "THIS NODE IS NOT SUPPORTED"). `isValid` stays FALSE
  and an Appearance never selects them. The shape draws with its default shading, as in X_ITE.
- An invalid or unsupported shader is **not** replaced silently. The shape falls back to default shading only
  where X_ITE does (no valid shader in `shaders`), and the compile or link error is logged in X_ITE's format.

## 5. Unimplemented or different behavior

- `#include <name>` of X_ITE's ShaderRegistry includes (`ShaderCompiler.process`) is not supported. A user
  shader that uses it fails to compile in Rust and works in X_ITE.
- `transformFeedbackVaryings` (`setTransformFeedbackVaryings`, used internally by X_ITE) is not ported.
- GLSL the rewriter or naga cannot express fails in Rust where a WebGL driver might accept it. The error is
  logged and the shape falls back as for any invalid shader. The size of this gap is unmeasured beyond the corpus.
- Compile error **log text** comes from the Rust compiler, not ANGLE. The message frame and line number follow X_ITE,
  but the log body is not compared with X_ITE.
- Shadow uniforms are set to 0 (`shadowIntensity`); shadow maps are not bound for custom shaders.
- The three `.x3d` (XML) worlds in the corpus have event scripts but no render baseline.

## 6. X_ITE comparison results

- Static renders (`render_rea_b08_shaders`): 84 worlds × MSAA 0 and 4 = **168 shots against the X_ITE GPU
  baselines, all within 0.5% / 8 levels, on lavapipe**. Worst: `texture-cube.wrl` 0.077% (max 46) MSAA 0, 0.074%
  MSAA 4. The test also requires a custom draw in every world with a valid shader and none in the
  fallback worlds (`bad-*`, `select-all-invalid`, `select-none`, `created-runtime`). Log:
  `gates-cloud/shader_render_lvp.log`. **On the owner's GPU: NOT RUN.**
- Stepped run-time cases vs X_ITE (`rea_batch08_shader_cases_match_x_ite`, worlds `*-runtime.wrl` + `.json`):
  **NOT RUN, baseline never made.** The test fails with "baseline missing" until the local run creates it.

## 7. GPU results

Owner GPU, MSAA 0 and 4: **NOT RUN** (no GPU in the container). The lavapipe results above and in 9 are
evidence that the WGSL, pipelines and bind groups are valid and draw the expected pictures. They are not gate
results.

## 8. Runtime field tests

The worlds exist (`fields-runtime`, `parts-runtime`, `textures-runtime`, `events-runtime`, `created-runtime`,
`environment-runtime`, `xml-*`). Their X_ITE comparison is NOT RUN (section 6). On lavapipe:
`rea_b08_shader_removal_from_live_group_releases_program` (a shaded Transform removed from a live Group by
Script and added back) and `rea_b08_script_created_shader_draws_and_is_released` (a Script creates the
ComposedShader and its parts) pass.

## 9. Memory and reload results

- `rea_b08_shaders_survive_reload_and_are_released` (lavapipe): 12 worlds × 5 reloads, each reload draws the same
  picture (0 pixels over 8), shader resources never grow, and resources are `[0,0,0,0,0]` (modules, pipelines,
  vertex buffers, bind groups, uniform buffer) after unload + 240 frames. Example: `fog.wrl` `[2,2,10,4,1]` →
  `[1,1,2,2,1]` after aging out.
- `rea_b08_release_tables` (no GPU): 20 Inline swaps between a shader world and a FontLibrary world. Shader tables are
  equal in every shader cycle, and 0 in the font cycles and after the last unload.
- Compile and link cost: the 11 `glsl_compat_b08` compile and link tests run in 0.01 s total (release, CPU).
  Programs are memoized by source (`link_cached`). Pipeline creation cost on a real GPU and frame time: **NOT
  RUN.**

## 10. Recommended REA status

| Entry | Recommendation | Contract items with evidence | Missing |
| --- | --- | --- | --- |
| node:ComposedShader | **PARTIAL** | 1, 2 (static, lavapipe), 4, 5 (lavapipe) | 3 (stepped X_ITE comparison), 7 on GPU; `#include`; transform feedback |
| node:ShaderPart | **PARTIAL** | 1, 2 (static), 4 | 3, 7 on GPU, `#include`, log text body |
| node:ShaderProgram | **PARTIAL** | 1, 2 (stub-nodes static), 4 | 3 |
| node:ProgramShader | **PARTIAL** | 1, 2 (stub-nodes static), 4 | 3 |
| node:PackagedShader | **PARTIAL** | 1, 2 (stub-nodes static), 4 | 3 |

If the local run creates the stepped baseline, the stepped cases pass, and the GPU renders pass, the three stub nodes
can move to COMPLETE (X_ITE has no behavior beyond the fields). ComposedShader and ShaderPart stay PARTIAL
because of the `#include` gap.

## Commands

```
cargo test --release --locked -p x3d-render --test glsl_compat_b08 --test render_rea_b08_shaders -- --nocapture
cargo test --release --locked -p x3d-viewer --test rea_b08_shaders_reload --test rea_b08_release_tables --test rea_batch08_shaders -- --nocapture --test-threads=1
# stepped baselines (local, X_ITE + GPU; writes only from X_ITE runs):
B08_SHADERS_TARGET=target/release python3 -I oracle/tools/rea_b08_shaders.py case NAME WORLD SCRIPT MSAA
```
