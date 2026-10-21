# X_ITE Rust port — Batch 09 hand-off

Branch: `integration/xite-rust-b06-b08-final`. Remote: `DJAscendance/X_ITE_R` (private).
Scope: X_ITE 16.5.0 to native Rust only. No CodeMirror 5, Windows, mobile or WRL Forge work.

## Rules that still apply

- Do not widen any comparison limit. Poses stay at 1e-4. Scene text and the sorted console log (with level) stay exact.
- Do not change the 25 DIFFERENT entries without owner approval.
- Do not promote an entry without full evidence. An unresolved behavior keeps it PARTIAL.
- Do not add pub modules to `x3d-scene` or `x3d-fields`. The `vrml97` `api_surface` test pins that list.
- Headless browsers only.
- Do not push to anything but `X_ITE_R`. Do not tag or release.

## Read this before you run anything (memory)

A test run used 54 GB of RAM on Oct 20, 2026 at 00:46 and the kernel killed the user session. Cap every run:

```
systemd-run --user --scope -p MemoryMax=24G -p MemorySwapMax=0 cargo test --workspace --release -j 12 --no-fail-fast
systemd-run --user --scope -p MemoryMax=8G  -p MemorySwapMax=0 timeout 120 target/release/x3d-drive ...
```

The cause was never found. The agent that ran it was killed before it could say. Every test now passes under the cap, so the cause was probably an uncommitted probe that is no longer in the tree. If one case grows without limit again, run the cases one at a time under the cap and find it.

## State

- Last full run (capped, all work in this branch): **995 passed, 0 failed, 1 ignored**.
- Authoritative ledger: `_batch09_context/ledger/ledger.json`. Totals read from it: COMPLETE 231, PARTIAL 55, DIFFERENT 25, NOT_TESTED 33, MISSING 2. Total 346.
- `status.json` and `summary.txt` are stale. **Never run `build_ledger.py`.** It rebuilds from the stale file and drops Batch 08 and 09 changes.
- Ledger tools: `_batch09_context/tools/ledger_b09.py` (the dict `P` holds the promotions and downgrades; `apply` reads `ledger/before/ledger.json` and writes `ledger.json`) and `verify_b09.py` (checks the ID set, the 25 DIFFERENT entries and that nothing outside `P` changed). Run `python3 -I verify_b09.py` after every ledger edit.
- The ledger has NOT been updated for the Step 3 work below. No Step 3 entry was promoted.

## Done this batch (commits on the branch)

- `4aaafb1` Scene error level, XML unknown-node baseline, Script shutdown kept-difference tests. Report: `step2-errors/REPORT.md`.
- `0b31c24` `Browser.loadURL` and `replaceWorld` switch worlds in place. Anchor click loads the world. Report: `step3-loadurl/REPORT.md`.
- `c44e0e1`, `6d1072b`, `a42f33a` Execution-context change events, config API, import/export checks, bindable stacks. 47 new oracle cases. Report: `step3-events-stacks/REPORT.md`.
- `4a1c230`, `ca75e0a` Script runtime battery (`scr-*`), MetadataSet battery (`meta-*`), field type and math battery (`fld-*`, 29 cases), plus a quiet internal `getField` so `createVrmlFromURL` does not log the deprecation warning that X_ITE only logs for user calls. **No report was written for these and no ledger decision was made.** The cases pass, but nobody checked them against the ledger "missing" text.
  - Check the `scr-browser` baseline first. It was stale and was re-recorded from pinned X_ITE 3 times (identical).

## Owner decisions on record

- VolumeEmitter, BoundedPhysicsModel and WindPhysicsModel stay PARTIAL.
- PackagedShader write-stop quirk accepted. ShaderProgram, ProgramShader and PackagedShader are COMPLETE. ComposedShader and ShaderPart stay PARTIAL.
- Render-inputs DIFFERENT candidates 1 (texgen NOISE-eye fma, 0.154%) and 2 (aliased lines MSAA0) are non-blocking.
- The multiview 0.65% limit is rejected.
- `importDocument` and `window.open` are browser DOM functions and are outside native scope. They are not marked DIFFERENT.
- Rust keeps Script shutdown on dispose. This differs from X_ITE. Tests: `crates/x3d-script/tests/script_shutdown_kept_difference.rs`. Do not change a status for this alone.

## Step 3 candidates that need an owner or reviewer decision

From `step3-events-stacks/REPORT.md`. Recommendation only:

- `sub:exec.import-export`: candidate COMPLETE.
- `sub:exec.bindable-stacks`: candidate COMPLETE.
- `sub:config.components-profiles`: candidate COMPLETE except `cfg-scene-odd` (V8-worded TypeErrors, `addComponent('Core')` stores an undefined name).
- `sub:exec.context-api`: stays PARTIAL until the open items in that report are accepted.
- `sub:browser.sai-api`: stays PARTIAL. `addFieldCallback` on ordinary node and field objects is a no-op. V8 error wording differs.

Open differences with first differing lines are in that report: `ctxev-inline` (frame 11 vs 9-10), `ctxev-startup-proto` (one extra event at frame 2), `ie-err-string-frames` (one frame), `ctxev-loadurl` TimeSensor phase after a world switch (cycles at 6 and 7 in X_ITE, 6 and 9 in Rust).

Also open: IsoSurfaceVolumeData and SegmentedVolumeData are still COMPLETE but their ordering cases were never written. Re-check them.

## Work left, hardest first

1. Turn the `scr-*`, `meta-*`, `fld-*` batteries into ledger evidence. Decide `node:Script`, `node:MetadataSet`, `sub:fields.types`, `sub:fields.math`. Write the missing report.
2. GPU render lane (`sub:render.core`, `render.transparency-sorting`, `render.lighting-model`, `render.headlight`, `render.fog`): needs a GPU adapter in the test environment. Depth ulp noise decision is open (nav-pointer decisions 1 and 3).
3. `sub:pointer.picking`: point and line GPU picking.
4. Touch harness: `sub:nav.examine`, `nav.plane`, `nav.lookat`, `nav.walk-fly`, and `nav.collision-gravity`.
5. `node:GaussianSplats` lane and Group/Transform `bboxDisplay` lane. Owner has not scheduled them. Do not mark them COMPLETE.
6. Physics: `sub:physics.engine`, VolumeEmitter, BoundedPhysicsModel, WindPhysicsModel (stay PARTIAL by owner decision).
7. Parsers: `sub:parse.vrml-classic`, `parse.gltf2`, `parse.glb2`, `parse.svg`, `parse.html-support`, `parse.golden-gate`. Also `sub:proto.proto`, `proto.externproto`, `sub:io.file-loader`.
8. Audio and text: `node:AudioDestination`, `BufferAudioSource`, `MicrophoneSource`, `sub:sound.engine`, `node:FontLibrary`, `sub:text.fonts` (real CFF fonts is an open Batch 08 decision).
9. 33 NOT_TESTED entries (lights, geometry, texture and text nodes, HAnim, layout, EspduTransform). Real DIS I/O is an open Batch 08 decision.
10. MISSING: `sub:browser.keyboard-ui`, `sub:browser.webxr`.

The full PARTIAL / NOT_TESTED / MISSING ID lists are in `ledger.json` (filter on `status`).

## How to run the Batch 09 checks

```
cargo test --release -p x3d-viewer --test rea_batch09_sai      # 114 scene/SAI/Script cases. REA_ONLY=<substring> picks cases
cargo test --release -p x3d-viewer --test rea_batch09_inputs
cargo test --release -p x3d-viewer --test rea_batch09_nav
python3 -I oracle/tools/rea_b09_sai.py xite NAME WORLD SCRIPT   # X_ITE side only, into /tmp/rea-b09-sai-xite/NAME/js.json
python3 -I oracle/tools/rea_b09_sai.py case NAME WORLD SCRIPT   # record a baseline and add it to cases.txt
```

Record each new X_ITE baseline 3 times and keep it only if all 3 are identical. A baseline is written from X_ITE only. The Rust side is compared and never copied. If you edit a probe world after recording, record it again (this caused the `scr-browser` failure).

## What is not in this repo

Several large oracle baselines are symlinks to a local scratch tree and are ignored by git (`oracle/items-baseline.json`, `oracle/render-*-baseline`, `oracle/x1-baseline.json` and others, see `.gitignore` and `oracle/README.md`). Tests that need them will fail or skip in a fresh checkout until you regenerate them. The Batch 09 SAI baselines (`oracle/rea-batch09-sai-baseline/`) are tracked and need only `oracle/x_ite-pin` plus `npm install` there.
