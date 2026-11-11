# 0006. DOM integration: not ported to the native host

Status: **Accepted (2026-11-02), for the native host only.** The native host has no DOM, so DOM integration has nothing to read there; this follows from the architecture. The browser build ports it (issue #146; part 1 landed in #291) and keeps no difference for it once #146 closes.

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

`DOMIntegration.js` reads the `<X3D>` element of an HTML page (an HTMLParser path, attribute and element changes). The native host has no DOM.

Ledger entries: `sub:browser.dom-integration`.

## Decision

Not ported to the native host. The browser build ports it (issue #146): since the first part of #146 the canvas follows its `<X3D>` child and `importDocument (htmlElement)` reads an HTML `<X3D>` element, both compared with pinned X_ITE (`tests/browser/dom01.test.mjs`). The changes of the element's attributes, children and removed nodes are in part 2 (`tests/browser/dom02.test.mjs`); part 3 adds the Inline child elements, the load, error and field CustomEvents, the refresh of a user field object, and the disposal of a removed `Scene` element (`tests/browser/dom03.test.mjs`). One known difference is left (the order of the first load events of the Inline elements, see `web/API.md`), so the entry `sub:browser.dom-integration` stays `DIFFERENT` until the owner decides on the ledger status.

## Consequences

The native host loads files and strings, never an HTML element. The question to the owner: confirm that a native build without DOM integration is the decision, and that the browser-side gap is tracked in #146.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: Entry `sub:browser.dom-integration` (`DIFFERENT`, note "DOM-only feature; native host has no DOM"). The entry text is frozen with the other 20 DIFFERENT entries (`ledger/tools/verify_ledger.py`), so the platform evidence is recorded here.
- Native host (2026-11-10): no DOM, nothing to port; the native parser is judged by its contract (`sub:parse.html-support`, COMPLETE, ADR 0012 item 5).
- Browser build (2026-11-10, local run at master 1203bf40 plus this branch, headless Chromium, WebGPU adapter nvidia ampere): `tests/browser/dom01.test.mjs`, `dom02.test.mjs` and `dom03.test.mjs` pass 21 of 21 (0 fail, 0 skipped) against pinned X_ITE. They cover the canvas following its `<X3D>` child, `importDocument (htmlElement)`, attribute, child and removal changes, the Inline child elements, the load, error and field CustomEvents, a user field object refresh and the disposal of a removed `Scene` element. The code is `crates/x3d-web/src/{session,chain,wasm}.rs`, `crates/x3d-runtime/src/dom.rs` and `crates/x3d-web/web/x_ite_r.mjs`, and `crates/x3d-web/tests/dom_mutation.rs` tests the engine side of the element changes. One known difference stays: the order of the first load events of the Inline elements (`web/API.md`).
- Ledger status: unchanged (`DIFFERENT`). This record leaves the status to the owner, and the browser-side evidence does not change that.
- REA (pinned source, summary view `ev_537f8ca3dfe9bde6e6887f475f4420020d32b62af8858080807c0223243ea80d`, parent `ev_1c09119a37d7bfc3f3f8fb85d5aed52b88ad7d4fa2c9f9698d9920a46bbc7b08`): `Browser/DOMIntegration.js`. The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
