# 0010. Pointing sensors: the hit point comes from a float32 pointing buffer

Status: **Superseded** (owner approval 2026-10-31, issue #67): the four sensors left `DIFFERENT` and are `COMPLETE`; this record keeps the history.

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

X_ITE takes the sensor hit point from a GPU `RGBA32F` pointing buffer (an interpolated float32 eye-space position). The Rust engine first intersected in f64, so values differed in the 6th to 7th digit; the entries were `DIFFERENT`.

Ledger entries: `node:CylinderSensor`, `node:PlaneSensor`, `node:SphereSensor`, `node:TouchSensor`.

## Decision

Re-measured on the GPU pointing path with `x3d-drive` against pinned X_ITE recordings of `oracle/synthetic-rea-verify/drag-sensors.wrl` (PlaneSensor, SphereSensor, CylinderSensor) and `touch-sensor.wrl`: scene text, poses and log are identical in all 60 frames. The owner approved the move off `DIFFERENT`.

## Consequences

With a pointing GPU the sensors match X_ITE. Without one, the CPU stand-in still intersects in f64; that path was not measured. The verifier keeps these four IDs in `LEFT_DIFFERENT`.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: `oracle/synthetic-rea-verify/{touch-sensor,drag-sensors}.wrl`; `crates/x3d-viewer/tests/rea_batch09_inputs.rs` (`sensors-drag`, `sensors-touch`; `oracle/rea-b09-inputs-baseline`, 3 identical runs each).
- REA (pinned source, summary view `ev_f2e88d1f7d56eac0d4fc3a1cccb59ed51ea3ec7cb7f942b459c330971cdf498c`, parent `ev_d8842d44ae46dcde1f0df664afdf5b8fed08fd69646d84c0b3778a46cf51bb14`): `PointingDeviceSensor/X3DPointingDeviceSensorNode.js`, `X3DDragSensorNode.js`, `PlaneSensor.js`. The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
