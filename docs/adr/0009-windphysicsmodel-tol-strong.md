# 0009. WindPhysicsModel: TOL_STRONG stays

Status: **Accepted** (owner decision 2026-10-31, issue #82).

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

Three strong-wind worlds (`ps-wind-strong`, `-r24`, `-r144`) pass only with `TOL_STRONG = 1e-4 + 2e-5*|ref|` (encoded in `rea_b07_ps.py` and `particles_b07.rs`). Against the stricter 06R limit, 311 of 2007, 35 of 666 and 341 of 4878 velocity values fail. The cause was measured: the GPU turbulence normal deviates from float32 per step (`turbulence-step-noise.log`). The unit conversion is modeled and equals the pinned source (`WindPhysicsModel.js` line 47; same clamps and `Math.random` draw order).

Ledger entries: `node:WindPhysicsModel`.

## Decision

`TOL_STRONG` stays, because the cause is the GPU's float32 turbulence, not the unit conversion.

## Consequences

Strong-wind particle velocities are compared with a looser tolerance than the 38 ordinary wind worlds, which pass the unchanged 06R limit.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: `oracle/tools/rea_b07_ps.py check` (103 cases, per-particle replay of X_ITE's `Math.random`), `crates/x3d-viewer/tests/particles_b07.rs`, `crates/x3d-runtime/tests/particles_b07_units.rs`.
- REA (pinned source, summary view `ev_2c6ba8bd8508b3c4ea75d9c6e548f53d97c914565b798695a9718ce7ff61aab0`, parent `ev_760503342553755880db97d85131a8205e18c5aac965e10a69494093cfe0b633`): `ParticleSystems/WindPhysicsModel.js`. The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
