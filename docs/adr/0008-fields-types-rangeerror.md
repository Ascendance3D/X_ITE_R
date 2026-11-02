# 0008. Field types: a huge length or index is a RangeError

Status: **Accepted** (owner decision 2026-10-20, "approved safety difference", kept within `sub:fields.types`).

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

Script field objects are X_ITE's own classes (a prelude generated from the pin). For a length or index near 2^32, X_ITE allocates and runs out of memory.

Ledger entries: `sub:fields.types`.

## Decision

A length or index near 2^32 is a `RangeError` and the field keeps its prior value. A second decision of the same date ("Decision 2"): there is no string-to-NaN difference; the raw components are kept with the value and along routes and printed as X_ITE does.

## Consequences

A world that asks for a 2^32-size array fails with a `RangeError` instead of running out of memory. Open items in the ledger: X3DConstants order of external-component node types (varies run to run in X_ITE, untestable), the QuickJS `IterableWeakSet` limit (`weak_set_engine.rs`), engine-level `Intl` and `Error.stack`.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: `crates/x3d-script/tests/script.rs` (`huge_mf_length_or_index_is_a_range_error_not_an_allocation`, `huge_length_on_a_node_field_keeps_its_prior_value`), `crates/x3d-viewer/tests/rea_batch09_sai.rs` (`fld-*`, `fldx-*`, `fcb-*`), `weak_set_engine.rs`.
- REA (pinned source, summary view `ev_3e59115e4c21dec6ff6b1ae37bcae509a1474b222a276272e5ccf83ef071c948`, parent `ev_311c0a3284ef4862db8959d42537c80d993968a93ec1bdc067289e192e983b82`): `Fields/ArrayFields.js` (the SFVec classes of the same directory were not parsed). The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
