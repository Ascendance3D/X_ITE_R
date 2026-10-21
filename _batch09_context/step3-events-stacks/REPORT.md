# Batch 09 step 3.2: context events, context API, import/export, bindable stacks, configuration

Worktree `.worktrees/rust-port-private/integration-b06-b08`, branch `integration/xite-rust-b06-b08-final`.
Base `0b31c24`. Three local commits (nothing pushed, no ledger file touched, no tolerance widened):

- `c44e0e1` scene, parse: component/profile aliases and titles, PROFILE statement flag
- `6d1072b` runtime, script: execution context change events, config and import/export API
- `a42f33a` oracle: probes, generators and exact X_ITE baselines (47 new cases; `cases.txt` 29 -> 76)

Final `cargo test --workspace --release -j 12 --no-fail-fast`: 994 passed / 0 failed / 1 ignored (baseline 994/0/1;
`rea_batch09_sai_cases_match_x_ite` now covers 76 cases). Commits 2 and 3 are not split further because the
event, configuration and import/export hunks share `engine.rs`, `prelude.js`, `ctx_api.rs`, `lib.rs`, `browser_load.rs`.

## What the pinned X_ITE (16.5.0) actually has

- `X3DExecutionContext` declares only `sceneGraph_changed` and `bbox_changed` (SFTime, set to `Date.now()/1000`).
  `X3DScene` adds `profile_changed` and `metadata_changed` (and `loadCount`). They are not fields a Script can
  look up: `getField` throws `Unknown field`; they are reachable only as properties
  (`scene.sceneGraph_changed.addFieldCallback (key, fn)`, also under a `_` prefix). Callbacks receive the time.
- `_namedNodes_changed` is a plain number property. `_importedNodes_changed`, `_exportedNodes_changed`, `_protos_changed`,
  `_externprotos_changed`, `_rootNodes_changed`, `_routes_changed` do not exist; `_components_changed` / `_units_changed`
  are assigned but never declared. There is no separate browser-level `sceneGraph_changed`.
- A scene is not an X3DNode: `addRoute (scene, 'sceneGraph_changed', ...)` and `addRoute (node, field, scene, ...)` throw
  (probe `ctxev-routes`, baselined).

Measured firing rules (all baselined in `ctxev-*`), the sceneGraph_changed (SG) count per action:

| action | SG |
| --- | --- |
| imported add/update/rename/remove(existing); exported add/update/remove(existing) | 1 |
| proto/externproto add/update; remove (even a missing name) | 1 |
| several sets in one script call | coalesced into 1 |
| `createNode`; assigning a node-valued field | 1 |
| `updateNamedNode`/`removeNamedNode`, route add/delete, `addComponent`, `updateUnit` | 0 |
| set/add/remove root nodes on the world scene | 4 (X3DWorld/layer cascade; modeled as the constant `root_nodes_rounds`, even for the same value) |
| same on a `createScene` / `createX3DFromString` scene | 1 |
| `setMetaData`/`addMetaData`/`removeMetaData` | `metadata_changed` x1 (always, even missing name) |
| `setProfile` | `profile_changed` x1 |

`createX3DFromString` scene changes also fire on the world scene; `createScene` scenes do not forward.
Startup of the first world: 1 round right after Script initialize, then 3 in the first frame (+1 per Inline load,
+1 per PROTO instance). A world installed by loadURL: PROFILE/META/SG once at the Script's first events (before any
TimeSensor), 3 rounds in the next frame; the old world's scene fires 1 round at the switch.

## Implementation

- `crates/x3d-runtime/src/ctx_events.rs`: pending/deferred/parsed queues, forwarding along the context parent chain
  (`no_forward` for `createScene`), `ctxObserve` registration, delivery from `process_events` and at finishEvents.
  Compared with the old design nothing is evaluated unless a Script registered a callback.
- Parent diffs come from `set_raw` (X3DBaseNode.parentsChanged), Inline scene attach, root node edits, `createNode`.
- prelude.js: field objects (`addFieldCallback`/`removeFieldCallback`/`getName`/`getValue`/`valueOf`), `getField` throwing
  the X_ITE message, `_namedNodes_changed`, `rootNodes` setter.
- Config: `supportedProfiles`/`supportedComponents` (alias aware, titles, providerURL, `add`/`update`/`remove`/`clear`),
  `ComponentInfo`/`ProfileInfo` with `toVRMLString`/`toXMLString`/`toJSONString`, scene `setProfile`, `hasComponent`,
  `addComponent`/`updateComponent`/`removeComponent`, `updateUnit`, `createScene` checks, `H-Anim`->`HAnim`,
  `TextureProjector`/`ProjectiveTextureMapping`->`TextureProjection`, `MPEG-4`->`MPEG4Interactive`, `scene.worldURL` of a
  created scene = page URL.
- Import/export: imported route ends now run X_ITE's `getRoute`/`$.try (getField)`/`checkFields`; a failed reconnect logs
  `warn: AggregateError: Couldn't connect route.`. `EXPORT` of an imported node is rejected, as in X_ITE.
- `x_ite-nodes*.json` got `title` fields via `oracle/tools/gen_rea_b09_cfg_titles.py` (idempotent). If a generator ever
  regenerates those JSON files, rerun it.

## Evidence per ledger entry (recommendation only)

- `sub:browser.sai-api` -> stays PARTIAL. New evidence: `cfg-browser`, `bind-browser`, `ctxev-*` (14 cases). Unresolved:
  `addFieldCallback` on ordinary node/field objects is still a no-op in the port; V8-worded TypeErrors (see below);
  `importDocument`/`window.open` are out of native scope and were not touched.
- `sub:exec.context-api` -> candidate COMPLETE for the scene/context API tested (named, imported, exported, protos, routes,
  roots, metadata, scene creation, events), PARTIAL until the open items below are accepted: `ctxev-inline`,
  `ctxev-startup-proto`, `cfg-scene-odd`, `ie-err-string-frames`.
- `sub:exec.import-export` -> candidate COMPLETE: `ie-chain`, `ie-rename`, `ie-direct`, nine `ie-err-*`, `ie-err-string`
  (import -> local node -> export chains, duplicate names, missing exports, route-before-load, failed load, self import).
- `sub:exec.bindable-stacks` -> candidate COMPLETE: `bind-{Viewpoint,OrthoViewpoint,GeoViewpoint,NavigationInfo,Background,
  TextureBackground,Fog,route,inline,read,browser,layers,switch}`, exact scene text at every frame and poses at 1e-4.
- `sub:config.components-profiles` -> candidate COMPLETE for statements (`cfg-parse-{vrml-profile,vrml-component,vrml-misc,xml,json}`),
  `cfg-browser`, `cfg-scene`; PARTIAL only through `cfg-scene-odd`.

## Documented open (not baselined, each with first difference)

1. `ctxev-inline`: after `url` is set, X_ITE delivers the Inline-load SG at frame 11, Rust at frame 9-10
   (`10: xite "4:set=ok"  rust "4:set=ok SG@4"`). Depends on real fetch latency. Counts match.
2. `ctxev-startup-proto`: X_ITE delivers one extra SG a frame after the first frame (frame 2); Rust delivers only the
   frame-1 rounds (`2: xite "... SG@1 x5" rust "... SG@1 x4"`). Not per instance (two instances: still one extra).
3. `ie-err-string-frames`: X_ITE resolves a `createX3DFromString`/`createScene` promise one frame later than Rust
   (except in frame 1). `ie-err-string` covers the same values at a fixed step.
4. `cfg-scene-odd`: `addComponent ()` / `addComponent (null)` / `updateComponent ()` throw V8-worded TypeErrors;
   `addComponent ('Core')` stores undefined name/level. Wording is engine specific (QuickJS differs).
5. Root-node cascade constant 4 and the startup 1+3 rounds are modeled as constants (X3DWorld, layer0 cascade is not ported).
6. `setProfile` oddities (profile with no known components) and `loadComponents` for external components are not probed.
7. The Rust-only difference that scripts of the old world get `shutdown ()` is pre-existing and owner-approved.

## Method notes

- All 47 new X_ITE recordings were repeated three times and were identical. Rust runs with `B09_TARGET=$PWD/target/release`.
- Probe Scripts record into their own fields (`hist`/`out`), the clock value is never compared.
- `ctxev-loadurl` B-world probes do not use a TimeSensor: its phase after a world switch differs between X_ITE and Rust
  (cycles at frames 6,7 versus 6,9), an unrelated pre-existing timing difference that is not part of this lane.
