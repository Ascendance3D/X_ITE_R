# 0003. NURBS: ISO/IEC 19775-1 clause 27 where X_ITE differs

Status: **Accepted** (owner decision 2026-10-17, "ISO everywhere").

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

REA batch 04 found places where X_ITE 16.5.0 departs from ISO/IEC 19775-1 clause 27. The ledger names them I1 rational weights, I2 weight count, I3 the seam-weight closed rule, I4 clockwise trimming loops, I8 `NurbsTextureCoordinate` weights, and I9 `NurbsSet` `bboxDisplay` TRUE.

Ledger entries: `node:NurbsCurve`, `node:NurbsCurve2D`, `node:NurbsOrientationInterpolator`, `node:NurbsPatchSurface`, `node:NurbsPositionInterpolator`, `node:NurbsSet`, `node:NurbsSurfaceInterpolator`, `node:NurbsSweptSurface`, `node:NurbsSwungSurface`, `node:NurbsTextureCoordinate`, `node:NurbsTrimmedSurface`.

## Decision

The Rust engine follows the ISO rules in those places. For unweighted input it equals X_ITE.

## Consequences

A weighted or trimmed NURBS world can render differently from X_ITE. Ten `iso_*` geometry cases differ by design; 58 shared cases are exact.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: `oracle/synthetic-rea-b04/geometry-cases.json` (58 shared cases exact, 10 `iso_*` cases differ by design; `rea_b04_geometry.rs`), `crates/x3d-render/tests/rea_b04_nurbs_iso.rs` (an independent ISO reference), `render_rea_b04.rs`, `rea_batch04.rs` nurbs-runtime MSAA 0 and 4.
- REA (pinned source, summary view `ev_d36c876b1ec49cc42cb9f909fdc487ad14731f7d53fcdd8070b42bcc7d98d3d4`, parent `ev_827b64f4dd2a74a3d752b84c069b9336820900d7ed91e23c79cbdc7afcd1173f`): `NURBS/NurbsCurve.js`, `NURBS/NurbsSet.js`, `NURBS/NurbsTextureCoordinate.js` (the other eight NURBS nodes of the family were not parsed; the unselected files are unknowns). The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
