# 0007. Script: no DOM globals, no stand-in objects

Status: **Accepted** (owner decision 2026-10-20, "Decision 1").

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

A Script in X_ITE runs in the page and can touch `window`, `document`, `XMLHttpRequest` and `fetch`. The native host's Script engine (QuickJS with X_ITE's own prelude generated from the pin) has no page.

Ledger entries: `node:Script`.

## Decision

No DOM and no stand-in objects: the page's own globals are not provided. The ledger lists the open clauses `scr-classes-open`, `scr-url-order-open` (the frame a fetched candidate completes in) and `scr-browser-open` (X3DConstants order of external-component node types). A related decision of the same date: a field callback that throws is reported at error level and ends the callbacks of that event (as X_ITE's `processEvent` loop); the runtime goes on where X_ITE's browser stops.

## Consequences

A Script that needs the DOM fails natively. The browser build is a different case: its Scripts run in the page's own JavaScript engine (`crates/x3d-web/web/API.md`; not checked for a Script section in this record).

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: `oracle/synthetic-rea/script-reload.wrl` (432 checks exact), `crates/x3d-script/tests/script.rs`, `crates/x3d-viewer/tests/rea_batch09_sai.rs` (`scr-globals`, `scr-classes`, `scr-url-order-sync`, `scr-url-order-micro`, `scr-image-events`).
- REA (pinned source, summary view `ev_ec960639a62b471207862ceb7d38ffebf6dc8db07f2caa784c9949a905b357e2`, parent `ev_d7d23bea4f1bfdae01f4a9105c88840cac1b2839de2af840244036bfeaa79e82`): `Scripting/Script.js`, `Scripting/X3DScriptNode.js`. The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
