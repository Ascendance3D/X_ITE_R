# 0002. Annotation component: registered here, stand-ins where X_ITE cannot initialize

Status: **Accepted** (owner decision 2026-10-17, "Do both").

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

X_ITE 16.5.0 ships the Annotation classes but does not register the component. Observed by REA batch 04: the dormant classes cannot initialize in two cases. `AnnotationLayer`: `X3DLayerNode` reads `_hidden`, `_visible` and `_children`, which it lacks. `IconAnnotation`: `X3DUrlObject` reads `_load`, which it lacks (the ledger cites a probe, `evidence/boundary/annotation-layer-x_ite-probe.json`, which is not in this tree).

Ledger entries: `node:AnnotationLayer`, `node:AnnotationTarget`, `node:GroupAnnotation`, `node:IconAnnotation`, `node:TextAnnotation`, `node:URLAnnotation`.

## Decision

The Rust engine registers the Annotation component (`crates/x3d-scene/data/x_ite-nodes-annotation.json`, dumped from the dormant classes by `oracle/x_ite-nodes-annotation.mjs`). `AnnotationTarget`, `GroupAnnotation`, `TextAnnotation` and `URLAnnotation` behave as the dormant X_ITE classes (compared exactly). `AnnotationLayer` is an empty layer and `IconAnnotation` an url object with an empty `requestImmediateLoad`, like `GeoMetadata`, because the originals cannot run; these two are compared against named stand-ins (`oracle/rea-b04-baseline/standins.json`).

## Consequences

A world that uses an Annotation node loads here and fails in X_ITE 16.5.0. The browser build's `supportedComponents` has Annotation after X_ITE's 39 (see ADR 0001). If X_ITE later registers the component, the stand-ins must be re-compared.

The ledger (`ledger/ledger.json`) is not changed by this record; it keeps its status and notes. The record states the decision once so that no implementation ticket reopens it (issue #63).

## Evidence

- Tests and recordings: `oracle/synthetic-rea-b04/annotation-layer.wrl`, `icon-annotation.wrl` against the stand-ins (`rea_batch04.rs`, MSAA 0 and 4); scene text, log, poses and images exact against the dormant classes for the other four nodes.
- REA (pinned source, summary view `ev_4d1ca1620d9e97171af5274ab4e6d2599ad81c271b4c468fe9ead30fe03c3a2a`, parent `ev_c5b6bf2ca9b385b835dde280e6f4d578813404fa264eafb80778f8d5b551fb83`): `Annotation/AnnotationLayer.js`, `Annotation/IconAnnotation.js`, `Annotation/X3DAnnotationNode.js`. The graph is partial; relationships through unselected sources are unknown.
- Classic encoding (2026-11-10, `crates/x3d-parse/tests/classic_annotation.rs`): the port accepts `COMPONENT Annotation:1` and `:2`, where the pinned parser throws (cases `sweep-annotation-1` and `-2` of `oracle/classic-edge-baseline`). In an `#X3D V4.0` world the six nodes load with the fields of the dormant classes; in the recorded `#X3D V3.3` cases they stay unknown, because the classes are `from: "4.0"` and X_ITE's version rule applies to them as to every registered node. Without the component statement they are unknown, as in X_ITE. A level above 2 is clamped to 2, as `X3DBrowser.getComponent` clamps every supported component. REA: analysis `ev_bdb28dc35b50b73af2c0a0f9bae3faab1c34b450a2bd853bcfaf96bfa886153e` (summary view `ev_ed4b3ac34d8319f9e34fba3a8f7f7e0c246d369387be082d7aceae25d7d17e3e`) of `Parser/VRMLParser.js`, `Browser/X3DBrowser.js`, `Configuration/SupportedComponents.js` and `SupportedProfiles.js`; the graph is partial.
- Format: as ADR 0001 (Status, Context, Decision, Consequences, Evidence).

## Amendment 2026-11-12: IconAnnotation loads its image

Owner decision 2026-11-12: a bounded IconAnnotation enhancement beyond pinned X_ITE. The rest of this record is unchanged.

- IconAnnotation is an X3DUrlObject that fetches `url` like an ImageTexture: the FileLoader fallback skips a candidate that does not fetch or that the ImageTexture check (`check_image`) rejects, the first candidate that passes sets COMPLETE_STATE, and no candidate left sets FAILED_STATE. The check is the one of ImageTexture: data in no image format fails, an SVG document passes, and the pixels are not decoded. So a damaged file with a valid image header passes, as it does for ImageTexture. A LoadSensor reports it. An empty `url` does not start a load. Nothing decoded is kept, because the node has no display.
- The class has no `load` field, so the node loads as with the X3DUrlObject default, TRUE.
- Not done: a default icon, placement and display (`displayPolicy`, `annotationGroupID`). The adopted ISO/IEC 19775-1:2023 text has no Annotation clause. The Web3D X3D 4 working draft WD2 (clause 43) requires JPEG and PNG support and a default icon, and gives no normative layout. It is supporting evidence only; no ISO compliance is claimed.
- Reference: `icon-annotation.wrl` now has an ImageTexture stand-in (`icon-annotation-ref.wrl`, `standins.json`) instead of GeoMetadata. Its url starts empty, and its script settles loads (`"settle": true`). Tests: `crates/x3d-runtime/tests/icon_annotation_load.rs` (PNG, JPEG, fallback over a missing candidate and data in no image format, failure, empty url) and `rea_batch04.rs` (MSAA 0 and 4, exact against pinned X_ITE's ImageTexture).
