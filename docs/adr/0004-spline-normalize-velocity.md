# 0004. Spline interpolators: normalizeVelocity TRUE keeps the math

Status: **Accepted** (owner decision 2026-10-16).

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

On every tested path the Rust interpolators equal X_ITE. With `normalizeVelocity` TRUE and a non-empty `keyVelocity`, X_ITE throws `value.norm is not a function` (the SFVec fields have no `norm`) and the scene load fails (upstream bug, per the ledger note).

Ledger entries: `node:SplinePositionInterpolator`, `node:SplinePositionInterpolator2D`.

## Decision

Keep the math: Rust computes the normalized velocity. The difference stays listed.

## Consequences

Such a world loads here and fails to load in X_ITE 16.5.0. A fix upstream would remove the difference.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: `oracle/synthetic-rea/interp-ext.wrl` (exact on all tested paths).
- REA (pinned source, summary view `ev_08803dec981a4071a5ed35b07971222707855796b281d7e566557f7bab4f24bb`, parent `ev_46f6d4f5054ce4c93052078c3e86b7ca08db4073101413494e2f6fb56738b1c5`): `Interpolation/SplinePositionInterpolator.js`, `Interpolation/SplinePositionInterpolator2D.js`. The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
