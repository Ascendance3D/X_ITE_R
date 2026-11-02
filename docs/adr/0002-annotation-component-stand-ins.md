# 0002. Annotation component: registered here, stand-ins where X_ITE cannot initialize

Status: **Accepted** (owner decision 2026-10-17, "Do both").

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

X_ITE 16.5.0 ships the Annotation classes but does not register the component (`COMPONENT Annotation` fails there). Observed by REA batch 04: the dormant classes cannot initialize in two cases. `AnnotationLayer`: `X3DLayerNode` reads `_hidden`, `_visible` and `_children`, which it lacks. `IconAnnotation`: `X3DUrlObject` reads `_load`, which it lacks (the ledger cites a probe, `evidence/boundary/annotation-layer-x_ite-probe.json`, which is not in this tree).

Ledger entries: `node:AnnotationLayer`, `node:AnnotationTarget`, `node:GroupAnnotation`, `node:IconAnnotation`, `node:TextAnnotation`, `node:URLAnnotation`.

## Decision

The Rust engine registers the Annotation component (`crates/x3d-scene/data/x_ite-nodes-annotation.json`, dumped from the dormant classes by `oracle/x_ite-nodes-annotation.mjs`). `AnnotationTarget`, `GroupAnnotation`, `TextAnnotation` and `URLAnnotation` behave as the dormant X_ITE classes (compared exactly). `AnnotationLayer` is an empty layer and `IconAnnotation` an url object with an empty `requestImmediateLoad`, like `GeoMetadata`, because the originals cannot run; these two are compared against named stand-ins (`oracle/rea-b04-baseline/standins.json`).

## Consequences

A world that uses an Annotation node loads here and fails in X_ITE 16.5.0. The browser build's `supportedComponents` has Annotation after X_ITE's 39 (see ADR 0001). If X_ITE later registers the component, the stand-ins must be re-compared.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: `oracle/synthetic-rea-b04/annotation-layer.wrl`, `icon-annotation.wrl` against the stand-ins (`rea_batch04.rs`, MSAA 0 and 4); scene text, log, poses and images exact against the dormant classes for the other four nodes.
- REA (pinned source, summary view `ev_4d1ca1620d9e97171af5274ab4e6d2599ad81c271b4c468fe9ead30fe03c3a2a`, parent `ev_c5b6bf2ca9b385b835dde280e6f4d578813404fa264eafb80778f8d5b551fb83`): `Annotation/AnnotationLayer.js`, `Annotation/IconAnnotation.js`, `Annotation/X3DAnnotationNode.js`. The graph is partial; relationships through unselected sources are unknown.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).
