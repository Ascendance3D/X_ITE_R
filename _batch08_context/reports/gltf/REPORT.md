# glTF lane report (Batch 08)

## Recommended status
- sub:parse.gltf2: PARTIAL. All of GLTF2Parser.js is ported except the external decoders.
- sub:parse.glb2: PARTIAL. GLB2Parser is complete; it inherits the missing decoders of glTF.
- Not COMPLETE because Draco, BasisU, AVIF/WebP and meshopt are not decoded. Meshopt is a stub.

## Contract items met
- Same X3D scene graph as X_ITE: node types, DEF names, fields, ROUTEs, Scripts (byte-exact ecmascript sources), optimizer, export groups.
- Dispatch: GoldenGate order JSON, glTF, GLB. glTF is detected by JSON keys plus asset.version; GLB by magic, version 2 and length.
- Runtime: Inline, loaders and render load paths fetch glTF buffers through the runtime fetcher (data:, files, http://oracle/).
- Invalid inputs: same accept/reject decision and same error text as X_ITE.
- Render at MSAA 0 and 4, stepped animation, Inline load/reload/unload: see below.

## Files changed
- New: crates/x3d-parse/src/gltf2/ (mod, util, access, scenegraph, geometry, material, animation, bbox, glb, meshopt, scripts).
- Small hunks: crates/x3d-parse/src/lib.rs (mod, dispatch, fetcher re-exports); crates/x3d-runtime/src/loader.rs (parse_bytes_with_fetcher), inline.rs (use it, glTF error text), lib.rs (re-export); crates/x3d-render/src/load.rs (use it).
- New tests: crates/x3d-parse/tests/gltf_rea.rs; crates/x3d-render/tests/render_rea_b08_gltf.rs; crates/x3d-script/tests/gltf_inline_b08.rs and gltf_inline_common/; crates/x3d-viewer/tests/rea_b08_gltf_reload.rs.
- Oracle: oracle/x_ite-gltf.mjs, x_ite-shot-gltf.mjs, x_ite-shot-msaa-gltf.mjs, tools/gen_rea_b08_gltf.py, tools/rea_b08_gltf.py, synthetic-rea-b08-gltf/ (and anim, inline, probe, probe2 inside), synthetic-rea-b08-gltf-inline/, rea-batch08-gltf-baseline/, render-rea-b08-gltf-baseline/, render-msaa4-rea-b08-gltf-baseline/.
- probe2/pm.wrl is a stray probe and can be deleted.

## Commands (export CARGO_TARGET_DIR=/tmp/b08/gltf/target CARGO_BUILD_BUILD_DIR=/tmp/b08/gltf/build; add -j 8 --release)
- python3 -I oracle/tools/gen_rea_b08_gltf.py oracle/synthetic-rea-b08-gltf
- python3 -I oracle/tools/rea_b08_gltf.py parse | shots ; case NAME WORLD SCRIPT MSAA
- (cd oracle; node x_ite-inline.mjs synthetic-rea-b08-gltf-inline rea-batch08-gltf-baseline/inline-baseline.json)
- cargo test -p x3d-parse --test gltf_rea
- cargo test -p x3d-render --test render_rea_b08_gltf
- cargo test -p x3d-script --test gltf_inline_b08
- cargo test -p x3d-viewer --test rea_b08_gltf_reload
- Stepped animation: cases.txt in oracle/rea-batch08-gltf-baseline lists each x10 case (python3 oracle/tools/x10_compare.py WORLD SCRIPT OUT --target ...).

## X_ITE comparison numbers
- Parse: 287 synthetic inputs (about 240 valid, 49 invalid). 980 text checks (VRML, XML, JSON text equal, encoding, error text): all pass, 0 failed, except Draco (below).
- Render: 16 worlds at MSAA 0 and 4, 32 shots, bound 0.5% over 8 levels. Worst: tex-nearest 0.017% (MSAA 0), 0.021% (MSAA 4); all others 0.000%.
- Stepped animation (step, linear, cubic-spline, driven through a TouchSensor and an imported Timer): scene text, Inline scene text and poses equal at all dump frames; images max delta 0 at MSAA 0 and 4.
- Inline scenarios: 18 (url switch to GLB, load toggle, fallback list, loadNow/unloadNow, remove, variants, skin animation, 10 invalid files): load count, inline state and scene text equal at every step; console equal except two cases (below).

## Lifecycle and memory
- rea_b08_gltf_reload: 7 worlds (glTF, GLB, skin, morph, animation Inline, variants), 5 reloads each draw the same picture (0 pixels over 8); resource counts never grow; all geometry, texture and cube map counts are 0 after unload.

## Known limits
- Draco: X_ITE loads draco_decoder_gltf.js. Rust skips it. With garbage Draco data X_ITE rejects the file ("Error: Unsupported geometry type."); Rust loads the scene without decoding. Skipped in gltf_rea.
- BasisU, AVIF and WebP: no decoder. Images keep their URIs; the texture node loading fails at runtime as in X_ITE without the codec.
- Meshopt: stub. Undeclared use produces X_ITE's TypeError text; declared use is not decoded.
- Buffer fetch failures inside an Inline (missing .bin, bad base64 data URL): X_ITE logs the buffer loader's own message and the Inline logs nothing; Rust logs the Inline message with the same error text. Console text differs in those two cases only (allowed in gltf_inline_b08.rs).
- No Khronos sample models were used; all fixtures are synthetic.
- Inline error mapping is by message prefix and file extension (.gltf/.glb) in inline.rs.

## X_ITE quirks reproduced
- Scene name ignored; EnvironmentLight rotation always 0 0 1 pi.
- Missing images/textures arrays throw TypeError; undefined texture mapping prints "undefined".
- Material extension node absent (version gating) throws "Cannot set properties of null".
- Humanoid bounding box is empty while the parser runs, so camera centerOfRotation ignores skinned meshes (verified against X_ITE).
- Sparse/normalized/Float16 accessors, typed-array RangeError texts, JS truthiness and ?? semantics.
- Interpolator keys keep the times array element type.
- Material variant names without name print "undefined".
- Falsy-throw and null-camera export TypeErrors.

## Owner decisions
- Port Draco and meshopt natively (new crates are disallowed in this batch)?
- Should buffer fetch failures log like X_ITE (nested loader message, silent Inline)? Needs a logging channel from the parser to the runtime.
- Add Khronos sample models with SHA-256 provenance as optional extra evidence?
