# 0012. Decisions of issue #68: Text warm-up, volume styles, html-support, InlineGeometry, microphone

Status: **Accepted (2026-11-02).** Routine technical decisions under the standing rule: match pinned X_ITE 16.5.0 unless it violates the applicable ISO standard (then implement the standard and record the clause, the difference and a regression test); where the standard has no text, keep pinned X_ITE behavior where possible. None of the seven is an owner gate (no history rewrite, licensing, spending or architecture change).

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope. Standard: ISO/IEC 19775-1 V4.0 (`new-items/reference/iso-iec-19775-1-v4.0-x3d`, read only). ISO/IEC 19775-2 (SAI) is not local: unverified.

## Context

Seven ledger entries wait for a decision: `node:Text`, `node:BlendedVolumeStyle`, `node:ShadedVolumeStyle`, `node:VolumeScatterMaterialExtension`, `sub:parse.html-support`, `node:InlineGeometry`, `node:MicrophoneSource` (`sub:physics.engine` is related only). Each item below states the decision, its evidence, its consequence and the ledger effect.

## Decisions

### 1. `node:Text`: warm-up frames (decision: ALT COMPLETE)

X_ITE's own output is not repeatable for `origin`, `textBounds` and `lineBounds` in the warm-up frames: one or two default-font rebuilds depending on load (3 quiet runs gave one pair, a fourth quiet run two, loaded runs one pair and a later load frame; ledger `missing`). A baseline that the oracle cannot repeat is not evidence (the 5-run rule), so exactness cannot be asked there. After the warm-up everything is exact (`rea_batch09_sai` node-text, text-events, text-fontstyle, text-family; `rea_batch10_screen`; `rea_batch11_text`), and the first build without a font is pinned by `rea_batch12_text_warmup`. Decision: the warm-up counts are outside what is compared; the ledger entry reads as ALT COMPLETE. Ledger effect: `node:Text` PARTIAL to COMPLETE (ALT). No code.

### 2. `node:BlendedVolumeStyle`: ALPHA1 hang (decision: follow the standard; code and test in this PR)

Observations. Pinned `BlendedVolumeStyle.js` (lines 154 to 222) writes `blendColor_ <id> .a` for `ALPHA1` and `ONE_MINUS_ALPHA1` (a space after the underscore: invalid GLSL), so the page never draws: `oracle/x_ite-shot.mjs` gives `TIMEOUT 45s` in 5 of 5 runs (`oracle/synthetic-rea-b68-owner/xite-runs.json`, world `bl-iso1`). The same source knows `ALPHA0` / `ONE_MINUS_ALPHA0` (original opacity) and takes `ALPHA2` for the `default:` case, CONSTANT: `bl-iso2` and `bl-const` give the same picture hash in 5 of 5 runs.

Standard. ISO/IEC 19775-1 V4.0 41.4.1 (BlendedVolumeStyle), Table 41.3 defines `CONSTANT`, `ALPHA1` (Ov, the original opacity), `ALPHA2` (Oblend, the opacity of the blend voxels), `TABLE`, `ONE_MINUS_ALPHA1`, `ONE_MINUS_ALPHA2`. X_ITE violates it twice: its `ALPHA1` means Oblend, not Ov, and `ALPHA2` is not recognized. `ALPHA0` is not in the standard; X_ITE draws it (recorded baseline `bl-alpha0`, in `render_rea_b07_vol`), so Rust keeps it.

Decision. `crates/x3d-render/src/volume.rs`: `ALPHA1` and `ONE_MINUS_ALPHA1` use the original opacity (the picture of `ALPHA0`), `ALPHA2` and `ONE_MINUS_ALPHA2` the blend opacity. Before this PR Rust copied X_ITE's meaning of `ALPHA1` (blend opacity), which no X_ITE picture could confirm. Regression test: `crates/x3d-render/tests/render_b68_owner.rs` (`blended_alpha_weight_functions_follow_the_standard`, and the X_ITE record test). Consequence: the ALPHA1 / ALPHA2 difference is "X_ITE violates ISO/IEC 19775-1 V4.0 41.4.1 Table 41.3, the standard is implemented"; the `unverifiable/` worlds `bl-alpha1` and `bl-one-minus-alpha1` stay as the evidence that X_ITE cannot draw them. Ledger effect: `node:BlendedVolumeStyle` PARTIAL to DIFFERENT (standard followed).

### 3. `node:ShadedVolumeStyle`: non-Material `material` (decision: keep the difference; test in this PR)

Observations. `ShadedVolumeStyle.js` accepts any `X3DMaterialNode` (line 33) and its `addShaderFields` copies `ambientIntensity`, `diffuseColor`, ... of it (lines 47 to 54). A `PhysicalMaterial` has none of them, so X_ITE throws and the page never draws: `TIMEOUT 45s`, 5 of 5 runs (`sh-physicalmaterial`), while `sh-material` (a Material) draws in 5 of 5 (`xite-runs.json`). Rust draws the volume without the material and lists `PhysicalMaterial` as unsupported (`volume.rs`, `shaded`).

Standard. ISO/IEC 19775-1 V4.0 41.4.10 (ShadedVolumeStyle) declares `SFNode material [X3DMaterialNode]` and defines the lighting from the `ambientIntensity`, `diffuseColor`, `emissiveColor`, `specularColor`, `shininess` and `transparency` of "X3DMaterialNode"; a node without those fields has no defined result. No picture can be required of either side. Decision: keep Rust (a drawn, reported fallback beats a page that never draws). Test: `shaded_style_with_a_non_material_is_drawn_and_reported`. Consequence: approved difference, X_ITE throws. Ledger effect: `node:ShadedVolumeStyle` PARTIAL to DIFFERENT.

### 4. `node:VolumeScatterMaterialExtension`: contract item 5 for attached nodes (decision: the clarified reading holds)

The Batch 07 and 08 completion contract (`docs/reports/XITE_RUST_REA_B06_B08_FINAL_REMEDIATION_REPORT.md`, "owner reading of item 5") is not in the repository as text; the reading is "attached-node removal and Script-created evidence, as for the eight COMPLETE volume style rows". The scatter entry has `render_rea_b08_vs` (19 x MSAA 0/4), `rea_batch08_vs` (16 frames) and `rea_b08_vs_reload`, the same shape of evidence. Decision: the reading applies to scatter as it already does to the eight COMPLETE volume styles. Ledger effect: `node:VolumeScatterMaterialExtension` PARTIAL to COMPLETE (ALT). No code. Unknown: the contract's own wording, which is not in the repository.

### 5. `sub:parse.html-support` (decision: judged by the parser contract)

Name and field maps equal X_ITE on 53 HTML inputs and 12 XML twins (`html_support_rea`, 325 checks). The part that needs a DOM (live attributes, `importDocument`) is decided separately: not ported to the native host (ADR 0006) and ported in the browser build by #146 (`importDocument`, element changes), tested against pinned X_ITE (`tests/browser/dom01`, `dom02`). Decision: the native parser is judged by that contract; browser DOM work is tracked in #146. Ledger effect: `sub:parse.html-support` PARTIAL to COMPLETE (ALT). No code.

### 6. `node:InlineGeometry`: routes of a second geometry with the same URL (decision: approved difference)

Observation. `InlineGeometry.js` lines 140 to 146: when the geometry node is already in use (`[_lock]`), X_ITE copies it with `copy ()` and leaves `// TODO: add routes.`; routes inside the loaded scene are lost for the second instance. Rust keeps the routes (`crates/x3d-runtime/src/inline_geometry.rs`). The standard (ISO/IEC 19775-1 V4.0 has no InlineGeometry node; it is an X_ITE node) cannot decide it; the X_ITE comment states the loss is a known gap. Unknown: no recording of the second-instance routes exists and the `ig-select` reload picture check is still disabled; both stay open if the owner wants evidence. Decision: keep Rust, which is the intended behavior (X_ITE's own TODO). Ledger effect: `node:InlineGeometry` PARTIAL to DIFFERENT. The ISO text search covered the whole local V4.0 Part 01 markdown.

### 7. `node:MicrophoneSource`: live capture (decision: ALT COMPLETE)

The oracle host has no input device, so X_ITE raises no events there and neither does Rust (`oracle/synthetic-rea-b05/microphone.wrl`, 48 kHz offline co-simulation, tolerance 0; `rea_b05_reload`). Live capture needs `getUserMedia` and a device; it cannot be compared and it is not a CI-testable feature. Standard: ISO/IEC 19775-1 V4.0 16.4.14 (MicrophoneSource) defines live input and notes that enabling it has privacy and permission prerequisites; it defines no result for a host without an input device, so pinned X_ITE's behavior (no events) is kept. Decision: the no-device behavior is the contract; capture is opened as a ticket only if a device becomes available. Ledger effect: `node:MicrophoneSource` PARTIAL to COMPLETE (ALT).

## Consequences

The ledger (`ledger/ledger.json`) is not changed by this record; the per-entry effects above are applied by the serialized ledger step. `sub:physics.engine` is related only, with no effect.

## Evidence

* Tests and recordings: `crates/x3d-render/tests/render_b68_owner.rs`; `oracle/synthetic-rea-b68-owner/` (worlds and `xite-runs.json`, 5 identical runs, Chrome 156.0.8078.47, RTX 3060 Ti); the existing suites named above.
* REA (pinned source, summary views; graphs are partial, relationships through unselected sources are unknown): `Components/VolumeRendering` `ev_6be33ea65ce48b83b1ea8bff5b23f88e81482e9f63067134d7ec476248749bf4` (parent `ev_72acf6c9e16a5f09563055aa524c4ed8f28a00c328e56e45ddafdb92cff1d458`); `Components/Networking` `ev_759ba22085dbd4c38f69a3a5d1fb9ef1be24285eb7527a2df0879819711e0ef7` (parent `ev_d25938d10c86432a9418435b9eb59d39641c5e1aa793016afbac944adc88db82`).
* Format: as ADR 0001.
