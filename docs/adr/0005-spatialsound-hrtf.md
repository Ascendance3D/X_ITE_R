# 0005. SpatialSound: the HRTF panner is not X_ITE's

Status: **Accepted** (owner decision 2026-10-17).

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

SpatialSound is exact on all tested paths except `enableHRTF` TRUE. The Rust HRTF `PannerNode` is the web-audio-api crate's, with its own HRIR set. Chromium's IRCAM Composite data (`platform/audio/resources`) has no stated license and is not ported. Measured maximum difference 0.126 and 0.141 (`spatial-hrtf.wrl`).

Ledger entries: `node:SpatialSound`.

## Decision

Accept the HRTF difference; it stays listed. Do not port the Chromium data.

## Consequences

HRTF-spatialized output differs in the audio samples; everything else compared is exact (equalpower, cone, moving, linear, exponential, no-spatialize). The sound engine entry (`sub:sound.engine`, `PARTIAL`) lists the HRTF data as open.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: `oracle/tools/rea_b05.py check` (`oracle/synthetic-rea-b05`, baselines `oracle/rea-batch05-baseline`, 48 kHz offline co-simulation, audio tolerance 0): the listed worlds exact; `spatial-hrtf.wrl` differs.
- REA (pinned source, summary view `ev_4eb27f085684d9f7cc4bc1601134e7f8f3afc2b1c8b329dc5acd48b3826becdf`, parent `ev_65d606873c92ef13414e39c7f11251ca6b1acadb4ab204a47e4112da8b959686`): `Sound/SpatialSound.js`. The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
