# Batch 08 — selection record

Written 2026-10-18 before any Rust edit. Source: `ledger/before/ledger.json` (22 MISSING).
Worktree HEAD `03f7a28a`, 266 status rows. Start backup `backup/start`, trial restore 9,697 files, 0 differences.

## Reference pin

`oracle/x_ite-pin` is a plain directory (not a Git checkout; `git rev-parse` there returns the worktree HEAD).
Confirmed by `oracle/x_ite-pin/PINNED_COMMIT` = `34cdf9f97` and `oracle/x_ite-pin/package.json` `"version": "16.5.0"`.

## Probes made before selection (read only, no worktree edit)

- **Shaders.** naga 30.0.1 (already in `Cargo.lock` via wgpu) has a GLSL frontend behind feature `glsl-in`
  (dependency `pp-rs 0.2.1`, fetched from crates.io into the cargo registry; scratch crate `/tmp/b08-spike`).
  It accepts only `#version 440/450/460`. With a `#version 450` swap, an X_ITE-style GLSL ES 3.00 shader fails on:
  loose `uniform` outside a block ("uniform/buffer blocks require layout(binding=X)"), `in`/`out` without
  `layout(location)`, combined `uniform sampler2D` ("variable qualifier" not implemented), GLSL ES 1.00
  `attribute` / `varying` / `gl_FragColor` (parse error). `precision` statements are accepted. A block uniform +
  separate `texture2D` / `sampler` + `sampler2D(t, s)` + located in/out validates and writes WGSL.
  Lane spec: a GLSL ES 1.00/3.00 → GLSL 450 source rewriter, then naga, then wgpu. Adding the `glsl-in` feature
  adds `pp-rs` to `Cargo.lock`: **owner decision** (new dependency).
- **PackagedShader** is "THIS NODE IS NOT SUPPORTED" in the pin. **ProgramShader** is a 38-line stub. Parity =
  node and fields exist, `isValid`/`isSelected` as X_ITE sets them, the Appearance falls back to default shading,
  proven by a picture comparison.
- **DIS.** `ReceiverPdu`, `SignalPdu`, `TransmitterPdu`, `DISEntityManager`, `DISEntityTypeMapping` in the pin have
  fields, X3DNetworkSensorNode / bounded-object behavior, and no packet input or output. Parity = no packet IO.
  Real DIS PDU IO would be a standards-based DIFFERENT: **owner decision**. `EspduTransform` (NOT_TESTED) is not in
  this batch; its status must not change.
- **glTF.** `GLTF2Parser.js` (4,469 lines) handles 30 extensions. `KHR_draco_mesh_compression` (draco decoder
  library), `KHR_meshopt_compression` / `EXT_meshopt_compression` (meshopt decoder), `KHR_texture_basisu`,
  `EXT_texture_avif` / `EXT_texture_webp`, `EXT_texture_video` depend on external decoders.
- **GaussianSplats.** The node wraps `Browser/GaussianSplats/GaussianSplatsShape.js` (672 lines) with its own
  vertex/fragment shaders (sort + splat draw). Splat data comes from fields, from PLY (`PLYAParser.js`;
  `sub:parse.ply` is already COMPLETE) and from glTF `KHR_gaussian_splatting`. No other ledger entry is promoted
  by this work.
- **HTMLSupport.** `XMLParser.js` uses it to map upper-case element names to node type names and lower-case
  attribute names (initializeOnly fields and aliases only) to field names.
- **physics.engine.** Batch 06R ported Bullet 2.82 into `crates/x3d-runtime/src/physics/bt/` (engine vs Ammo
  66/66), but the ledger entry is MISSING with no `rust` reference and no inventory.

## The 20 entries

| # | ID | Priority | Lane |
| --- | --- | --- | --- |
| 1 | sub:physics.engine | A | physics |
| 2 | node:ComposedShader | A | shaders |
| 3 | node:ShaderPart | A | shaders |
| 4 | node:ShaderProgram | A | shaders |
| 5 | node:ProgramShader | A | shaders |
| 6 | node:PackagedShader | A | shaders |
| 7 | sub:parse.gltf2 | C (hard) | gltf |
| 8 | sub:parse.glb2 | C (hard) | gltf |
| 9 | node:GaussianSplats | B | render2 |
| 10 | node:VolumeScatterMaterialExtension | C | render2 |
| 11 | node:DISEntityManager | B | net |
| 12 | node:DISEntityTypeMapping | B | net |
| 13 | node:ReceiverPdu | B | net |
| 14 | node:SignalPdu | B | net |
| 15 | node:TransmitterPdu | B | net |
| 16 | node:InlineGeometry | B | net |
| 17 | node:FontLibrary | C | net |
| 18 | sub:parse.svg | C | svg |
| 19 | sub:parse.html-support | C | svg |
| 20 | sub:browser.multi-viewer-thread | C | multiview |

## Deferred (2)

- **sub:browser.webxr.** X_ITE's WebXR code drives the browser WebXR session API. A native port needs an OpenXR
  runtime and a headset, and no headless oracle exists. Owner decision on OpenXR scope first.
- **sub:browser.keyboard-ui.** `ContextMenu` and `Notification` are DOM overlays, and the key handling is bound to
  the HTML canvas. No headless X_ITE oracle can compare them. SVG, by contrast, has a text oracle.

## Acceptance criteria (written before implementation)

Nodes: Batch 07 `CONTRACT.md` items 1–7 (copied to `selection/CONTRACT.md`).
Subsystems:

- **sub:physics.engine:** a coverage table of every `Ammo.*` symbol that X_ITE's `RigidBodyPhysics/*.js` and
  `Picking/*.js` call → the `physics/bt/` code that implements it → a test that runs it against Ammo.js
  (Bullet 2.82). Every row has a test, or a documented reason (X_ITE never reaches it). Capsule is excluded by
  X_ITE itself. Existing physics gates stay green.
- **sub:parse.gltf2 / glb2:** for the same input, the Rust parser's scene, written by the existing XML writer, is
  text-equal to pinned X_ITE's `scene.toXMLString()` (or a documented, normalized comparison); valid and invalid
  inputs; console error text equal; pictures within 0.5% / 8 at MSAA 0 and 4 for a render subset; animation and
  reload cases. Fixtures authored here (synthetic), or Khronos samples with provenance. Nothing from the archives.
- **sub:parse.svg:** same text-equal rule plus render subset; valid and invalid inputs.
- **sub:parse.html-support:** the XML parser maps upper-case element names and lower-case initializeOnly field
  names exactly as pinned X_ITE does (text oracle), plus negative cases (inputOutput lower-case, unknown names).
- **sub:browser.multi-viewer-thread:** two or more `EmbeddedViewer`s on one thread, each with its own world (incl.
  particles, physics, scripts, sound-free), render the same pictures as each alone, with no shared-state leak
  (geometry provider, particle RNG, script engine, caches), with independent reload and drop, and resources
  released per viewer.

## Expected PARTIAL (stated before coding)

- sub:parse.gltf2 / glb2: Draco, meshopt, BasisU, AVIF/WebP, video textures need external decoders. Unless a lane
  ports one, those inputs are listed as missing.
- ComposedShader / ShaderPart / ShaderProgram: X_ITE's built-in uniform surface (`x3d_*` lights, fog, texture
  matrices, clip planes) is large; any uniform not provided is listed as missing.
- sub:browser.multi-viewer-thread: the process-global geometry provider (06R, owner decision pending) may
  interfere. If a test shows interference, the entry stays PARTIAL with that reason; the lane does not change the
  provider architecture without owner approval.
- GaussianSplats sort order: X_ITE sorts on the CPU or GPU; float ties may differ.

## Rule

Any swap after implementation starts is documented below with the reason.

## Swaps

(none yet)
