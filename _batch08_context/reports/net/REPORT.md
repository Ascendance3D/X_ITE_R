# Batch 08 lane net report

## Recommended status
- DISEntityManager, DISEntityTypeMapping, ReceiverPdu, SignalPdu, TransmitterPdu: COMPLETE for the X_ITE behavior (fields, defaults, legacy defaults, mapping alias, bbox, visible, bboxDisplay, no packet IO). Packet IO is not implemented. That is an owner decision.
- InlineGeometry: COMPLETE for the tested contract. Known gaps are listed below.
- FontLibrary: COMPLETE for url fallback, family registration, per-context tables, FontStyle lookup and load errors. WOFF1 is converted in x3d-parse. CFF fonts stay unsupported (no new crates).

Ledger is untouched. The lead sets it.

## Evidence (pinned X_ITE vs Rust)
- 33 synthetic worlds in oracle/synthetic-rea-b08-net (generator oracle/tools/gen_rea_b08_net.py).
- 32 stepped cases, each at MSAA 0 and MSAA 4 (64 cases in oracle/rea-batch08-net-baseline/cases.txt): scene text, poses, sorted warn/error log and pictures all match (at most 0.5% of pixels over 8).
- Static shots: oracle/render-rea-b08-net-baseline and render-msaa4-rea-b08-net-baseline. All pass. dis-viewall worlds are excluded from stills because viewAll animates. Stepped cases cover them.
- Field changes via ROUTE and Script, removal from a live Group, Script-created instances: dis-runtime, dis-route, ig-runtime, ig-remove, fl-runtime.
- UDP: loopback sender (127.0.0.1 only) with positive control. Both X_ITE and Rust leave the ports closed and send nothing. Checked in rea_batch08_net.
- Lifecycle: reload five times keeps pictures and resources bounded. Unload gives resources 0 and no pending scenes, loads, timers, tasks, InlineGeometry state or waiting FontStyle loops.
- Regression: x3d-script, x3d-runtime, x3d-text, x3d-parse: 220 passed, 0 failed. Batch 01-07 render and case tests, embed: all pass.

## Files
New: x3d-render/src/bbox_display.rs, x3d-runtime/src/inline_geometry.rs, x3d-runtime/src/font_library.rs, x3d-parse/src/woff1.rs (a pub mod in x3d-parse only), oracle/tools/gen_rea_b08_net.py, oracle/tools/rea_b08_net.py, oracle/synthetic-rea-b08-net/ (fonts/PROVENANCE.txt), oracle/rea-batch08-net-baseline, the two render baselines, tests x3d-render/tests/render_rea_b08_net.rs, x3d-viewer/tests/rea_batch08_net.rs, x3d-viewer/tests/rea_b08_net_reload.rs, x3d-script/tests/rea_b08_net_unload.rs.
Shared hunks: x3d-render lib.rs, traverse.rs (PDU bbox branch, Text font epoch), sg.rs, text.rs (library font lookup); x3d-runtime lib.rs, nodes/mod.rs, release.rs, inline.rs, loader.rs (FontStyle load is now a task with family/style reload, data url text), nodes/load_sensor.rs.

## Re-run
Set CARGO_TARGET_DIR=/tmp/b08/net/target, CARGO_BUILD_BUILD_DIR=/tmp/b08/net/build.
- cargo test --release -j 8 -p x3d-render --test render_rea_b08_net
- cargo test --release -j 8 -p x3d-viewer --test rea_batch08_net --test rea_b08_net_reload
- cargo test --release -j 8 -p x3d-script --test rea_b08_net_unload
- New baseline: python3 -I oracle/tools/rea_b08_net.py case NAME WORLD SCRIPT MSAA [--udp]; shots: ... shots

## X_ITE quirks reproduced
- creaseAngle early return in InlineGeometry smooth/solid forwarding.
- PDU nodes do not add to a parent bbox.
- FontStyle keeps the font found at its last load; a library that registers later changes nothing until the next load. FontStyle waits for libraries still loading.
- A library never unregisters a family. Unparsable font: node stays IN_PROGRESS; text of the error matches.

## Known limits
- Unreproduced: X_ITE leaks a stale bbox node after Group removal. Extra-frame timing for geometry fallback and data urls.
- X_ITE hangs when two nodes share a missing url inside fallback lists. Recorded, avoided (not reproduced).
- Two libraries with the same family: X_ITE result depends on load order (differed between runs), so no world depends on it.
- ig-select contains an animated scene, so the reload picture test allows phase differences there.
- Per-case tolerances were not widened.

## Owner decisions
- Real DIS PDU network IO (standards based, different work).
- CFF/OpenType fonts need a new dependency.
- Whether the two-libraries-one-family order should be pinned.
