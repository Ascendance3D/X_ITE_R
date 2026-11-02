# 0006. DOM integration: not ported to the native host

Status: **Proposed.** The ledger note says only "DOM-only feature; native host has no DOM"; no dated owner decision was found for it. The browser build has its own differences (ADR 0001, group on `importDocument (htmlElement)` and issue #146).

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

`DOMIntegration.js` reads the `<X3D>` element of an HTML page (an HTMLParser path, attribute and element changes). The native host has no DOM.

Ledger entries: `sub:browser.dom-integration`.

## Decision

Not ported to the native host. Whether the browser build should port it is issue #146's question, not this record's.

## Consequences

The native host loads files and strings, never an HTML element. The question to the owner: confirm that a native build without DOM integration is the decision, and that the browser-side gap is tracked in #146.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: Entry `sub:browser.dom-integration` (`DIFFERENT`, note "DOM-only feature; native host has no DOM"); no test.
- REA (pinned source, summary view `ev_537f8ca3dfe9bde6e6887f475f4420020d32b62af8858080807c0223243ea80d`, parent `ev_1c09119a37d7bfc3f3f8fb85d5aed52b88ad7d4fa2c9f9698d9920a46bbc7b08`): `Browser/DOMIntegration.js`. The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
