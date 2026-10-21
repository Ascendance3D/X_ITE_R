# Batch 09 scene/SAI lane report

Worktree `.../integration-b06-b08`, branch `integration/xite-rust-b06-b08-final`, base `d44ed6f`. Local commits only.
Final gate: `cargo test --workspace --release -j 12` = 990 passed, 0 failed, 1 ignored (`gen_oracle_worlds`).

## Test added
`crates/x3d-viewer/tests/rea_batch09_sai.rs::rea_batch09_sai_cases_match_x_ite` (passes). It runs `x3d-drive` against pinned X_ITE baselines
(`oracle/rea-batch09-sai-baseline/`, listed in `cases.txt`) and compares dumped scene text exactly, the sorted console log, and poses. No numeric tolerance changed.
Cases (each is a Script probe writing its results to its own field, so results are in the scene text):
sai-create, sai-context, sai-scenes (121 checks pass / 0 fail each in x10_compare), parse-header (32 snippets), parse-values (67), parse-routes (34), parse-errors (74).
Generator: `oracle/tools/gen_rea_b09_sai_parse.py`; recorder: `oracle/tools/rea_b09_sai.py`.
Not baselined because they still differ (documented below): parse-def-proto (1 of 56), parse-formats (1 of 50 plus one log level), parse-loads (timing).

## Implemented
- Browser: createX3DFromString/URL, createVrmlFromURL, createScene, importJS, X3DScene identity and serialization (XML/JSON of any scene), console global in Scripts.
- Execution context write API: named nodes, imported/exported nodes, proto and externproto declarations, routes, rootNodes, unique-name helpers, live X3DInfoArray objects.
- VRML parser: the thrown error text now has the line/caret form of VRMLParser.getError; unsupported component message carries the level; empty META key rejected; XML parser name prefix "XML Parser"; XML IS-type-mismatch warning carries X_ITE's actual TypeError text; createX3DFromString of a string never matches the binary parsers (GLB, STL-B, PLY-B).
- Files: `crates/x3d-runtime/src/{browser_load.rs (new),lib.rs,inline.rs,release.rs,script.rs,loader.rs}`, `crates/x3d-script/src/{engine.rs,engine/ctx_api.rs (new),prelude.js}`, `crates/x3d-scene/src/{scene.rs,xml.rs,json.rs}` (no new pub modules), `crates/x3d-parse/src/{lib.rs,parser.rs,xml.rs}`, oracle worlds under `oracle/synthetic-rea-b09-sai/`.

## Recommended status (all stay PARTIAL; none meets the COMPLETE bar)
- sub:browser.sai-api: PARTIAL. Done: scene creation/import, console. Missing: Browser.loadURL/replaceWorld (needs in-place world switch, designed, not built), importDocument (needs a DOM), change-event fields (_namedNodes_changed, _sceneGraph_changed), currentFrameRate/changeViewpoint/viewAll/getRenderingProperty/addBrowserCallback not probed.
- sub:exec.context-api: PARTIAL. Named/imported/exported/proto/route/rootNodes write API verified by sai-context; change events and V8-worded TypeErrors from typed-array copies missing.
- sub:exec.import-export: PARTIAL. Parser IMPORT/EXPORT error paths covered in parse-def-proto; chains through fields/routes not probed.
- sub:parse.vrml-classic: PARTIAL. 207 edge snippets match (headers, values, routes, errors); open gaps: printer names (`DEF _1`) nodes of an MFNode field value held by a PROTO instance body (parse-def-proto snippet 26).
- sub:parse.golden-gate: PARTIAL. Dispatch by string verified for VRML/XML/JSON/OBJ/STL/PLY/glTF/SVG; gaps: SVG viewpoint fieldOfView last-digit rounding (-0.0005644444 vs ...45), XML "Unknown node type" is a warning in Rust but console.error in X_ITE (scene has no error channel).
- sub:proto.proto, sub:proto.externproto, sub:io.file-loader, node:Inline, node:InlineGeometry: PARTIAL, only parse-level coverage added. parse-loads shows Rust settles Inline/EXTERNPROTO loads of missing and present files 1-2 frames before X_ITE (real HTTP latency in the oracle) with identical end state.
- node:Script, node:MetadataSet, sub:fields.types, sub:fields.math, sub:config.components-profiles, sub:exec.bindable-stacks: PARTIAL, not advanced beyond what existed (no probe batteries written).

## DIFFERENT candidates (evidence in the baselines/logs)
1. dispose/replace: Rust `dispose_scene` calls Script shutdown; X_ITE does not on replaceWorld (verified with console.log).
2. Concurrent createX3DFromString promises settle in X_ITE by loadComponents dependency depth; Rust settles in call order within a frame.
3. V8-specific TypeError wording from info-array map/filter/slice and getNodeOrImportedNode(...).getNodeName.
4. X_ITE extern fallback with a missing first URL logs `TypeError: Cannot read properties of null (reading 'setWorldURL')` where Rust logs `Error: Not Found` (parse-loads).
5. Load timing of network fetches (above).

## Owner decisions needed
- importDocument: needs a DOM; out of native scope. Recommend rejecting with an error (current behavior) and marking DIFFERENT.
- loadURL with a foreign target (window.open) is out of native scope.
- Change-event fields on the execution context: implement or accept as DIFFERENT?
- Whether to add an error-level channel to the scene warning list (needed for the XML parser log-level gap).
