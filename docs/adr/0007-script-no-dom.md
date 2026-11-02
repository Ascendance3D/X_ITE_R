# 0007. Script: no DOM globals, no stand-in objects

Status: **Accepted** (owner decision 2026-10-20, "Decision 1").

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

A Script in X_ITE runs in the page and can touch `window`, `document`, `XMLHttpRequest` and `fetch`. The native host's Script engine (QuickJS with X_ITE's own prelude generated from the pin) has no page.

Ledger entries: `node:Script`.

## Decision

No DOM and no stand-in objects: the page's own globals are not provided. The ledger lists the open clauses `scr-classes-open`, `scr-url-order-open` (the frame a fetched candidate completes in) and `scr-browser-open` (X3DConstants order of external-component node types). A related decision of the same date: a field callback that throws is reported at error level and ends the callbacks of that event (as X_ITE's `processEvent` loop); the runtime goes on where X_ITE's browser stops.

### `scr-url-order-open`: the frame a fetched Script candidate completes in (issue #54)

Decided from recordings and the standard, not left open.

* Observed (pinned X_ITE 16.5.0 in headless Chrome 156.0.8078.47, `oracle/x_ite-anim.mjs`, 14 runs of `oracle/synthetic-rea-b09-sai/scr-url-fetch-frame.wrl`, `oracle/rea-b09-sai-baseline-variable/scr-url-fetch-frame/xite-runs.json`): a candidate that needs a fetch (a file, a base64 data URL) completes in Chrome's network time. In 11 of 14 runs all four fetched Scripts initialize in the second frame, in 1 run all in the first, in 2 runs some in each; the order of the completions had five different values (the base64 data URL came last in 10 of 14). Five identical runs do not exist, so no baseline can be recorded and "match X_ITE" has no single target.
* The standard: ISO/IEC 19775-1 V4.0 29.2.3: `initialize ()` "shall be invoked before the X3D browser presents the world to the user and before any events are processed by any nodes in the same X3D file" (29.3.1: "Sometime before a script receives the first event it shall be initialized"). X_ITE's second-frame completions are later than that; they are an upstream difference from the standard. ISO/IEC 19775-2 (the scene access interface) is not available locally and was not checked.
* Decision: X_ITE_R completes the fetches while the world loads, in request order (`Runtime::run_tasks`), so every Script has initialized before the first frame. This is the behavior the standard asks for, and it is one of the outcomes X_ITE produced (all in frame 1, request order). The difference stays (frame 1 where X_ITE mostly gives frame 2, and the base64 data URL in request order where X_ITE mostly gives it last); it is not made to follow one run.
* Test: `crates/x3d-viewer/tests/rea_b54_script_fetch_frame.rs` (every candidate in frame 1, request order, and that the outcome is one of the 14 recorded).

## Consequences

A Script that needs the DOM fails natively. The browser build was not examined for this record; it is out of scope here.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: `crates/x3d-viewer/tests/rea_b54_script_fetch_frame.rs` (issue #54), `oracle/synthetic-rea/script-reload.wrl` (432 checks exact), `crates/x3d-script/tests/script.rs`, `crates/x3d-viewer/tests/rea_batch09_sai.rs` (`scr-globals`, `scr-classes`, `scr-url-order-sync`, `scr-url-order-micro`, `scr-image-events`).
- REA (pinned source, summary view `ev_ec960639a62b471207862ceb7d38ffebf6dc8db07f2caa784c9949a905b357e2`, parent `ev_d7d23bea4f1bfdae01f4a9105c88840cac1b2839de2af840244036bfeaa79e82`): `Scripting/Script.js`, `Scripting/X3DScriptNode.js`. The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
