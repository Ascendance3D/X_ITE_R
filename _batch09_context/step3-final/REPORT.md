# Batch 09 step 3 final: node:Script, node:MetadataSet, sub:fields.types, sub:fields.math

Cloud session, branch `feature/xite-rust-rea-b09-cloud` from `f008c67`. Reference: pinned X_ITE 16.5.0 @ `34cdf9f97`
(`oracle/x_ite-pin`), driven headless by `oracle/x_ite-anim.mjs`. Every new X_ITE baseline below was recorded three times and
kept only when the three were identical; the case tool (`oracle/tools/rea_b09_sai.py case`) recorded a fourth and stored it only
when it equaled them and the Rust side matched (scene text at every frame and the sorted log with level, exact; poses at 1e-4).

This report replaces the missing report for commits `4a1c230` / `ca75e0a`. It does not trust their PASS statement: the open
probes those commits left in `*-open.wrl` were run against X_ITE again, the reasons they differed were found, and the owner
decided (2026-10-20) that the field model is to match X_ITE 1:1, including its quirks and reflection.

## What changed underneath (why the batteries now match)

The Script prelude no longer models fields with its own classes. It runs X_ITE's own field object model, generated verbatim from
the pin by `crates/x3d-script/tools/gen_prelude_xite.py`:

- `prelude/math.js`: `standard/Math` (Algorithm, Vector2/3/4, Color3/4, Matrix2/3/4, Quaternion, Rotation4). `Math.*` in the Script
  realm is V8's (jsmath: now also `atan`, `pow`, `hypot`, `sqrt`).
- `prelude/fields.js`: X3DObject, X3DChildObject, X3DField, X3DArrayField, X3DTypedArrayField, X3DObjectArrayField, X3DInfoArray,
  X3DFieldDefinition, FieldDefinitionArray, every SF and MF class, Events, IterableWeakSet, X3DConstants, the VRML/XML/JSON
  generators, Features, helper. Each module is a block in X_ITE's ES evaluation order; the patches are listed in the generator
  (Intl formatting through x3d-fields `format.rs`, the Rotation4 state, field callbacks, a QuickJS heap-corruption workaround in
  IterableWeakSet, the allocation guard, V8's `this` for calls through `with`).
- `prelude/node_types.js`: X3DBaseNode.getType () of every node class and X3DConstants, recorded from the pin
  (`oracle/x_ite-node-types.mjs`, `oracle/x_ite-component-constants.mjs`, each 3 identical recordings).
- `prelude/bridge.js`: the boundary (owner decision: the SAI-visible equivalent where X_ITE hands out an engine object): X3DNodeRef
  stands for the X3DNode (the value of an SFNode, the parent of its fields); a node field is one X_ITE field object per field for
  the node's lifetime, mirrored with the Rust runtime (refreshed before the Script runs from the runtime's dirty list, sent with an
  event on X3DChildObject.addEvent, sent without one after the Script ran when it changed the value in place).

Values cross as numbers through `data_number`, which keeps `-0` (rquickjs `Value::new_number` stores every integral double as
an int; REA batch 02 picking-sweep and batch 04 nurbs-runtime print `-0` components and caught it).

Supporting Rust: `Runtime::field_from_string` / `Parser::parse_field_into` (X3DField.fromVRMLString / fromXMLString with a scene),
the mirror dirty list in `Runtime::set_raw`, `Rotation4::state`, `Color3/4::from_raw`, `jsmath::JS_UNDEFINED / JS_NULL` (an SF
component a Script left undefined or null prints as X_ITE prints it), field callbacks (`Runtime::field_observed`).

## node:Script

- Status: PARTIAL. Proposed: **PARTIAL** (unchanged).
- Tests: `crates/x3d-viewer/tests/rea_batch09_sai.rs` cases scr-lifecycle, scr-direct-output, scr-self-events, scr-browser,
  scr-errors, scr-timers, scr-url, scr-classes, scr-field-access, scr-field-defaults, scr-image-events, fcb-basic,
  fcb-routes, fcb-reentry, fcb-script-fields; `crates/x3d-script/tests/script.rs` (6); `script_shutdown_kept_difference.rs` (4).
  Generators: `oracle/tools/gen_rea_b09_script.py`, `gen_rea_b09_fieldcb.py`.
- Verified against X_ITE: lifecycle and callback order, direct output, own-field events, the Browser object subset in scr-browser,
  error reporting, timers, url fallback and reload; the global classes a Script sees (scr-classes, 166 probes); field access and
  conversion on every field type (scr-field-access, 159 probes, including the `undefined`/`NaN` components X_ITE keeps after a
  wrong-shaped assignment); default values (scr-field-defaults, 223 probes); `set_field__` now reuses one copy of the field per
  callback, as Script.js does; field callbacks on the Script's own fields (fcb-script-fields).
- The ledger's missing items:
  - SCRIPT_SHUTDOWN_ON_REMOVAL. New probe `oracle/synthetic-rea-b09-sai/scr-shutdown-inline.wrl` (3 X_ITE recordings, same
    content): X_ITE calls no `shutdown ()` when an Inline's url changes, when its `load` goes FALSE, or when a Script leaves the
    scene graph. Rust: removal calls none (matches); an Inline scene replaced and a world unloaded call it, the owner-approved
    "Script shutdown on dispose" difference. Not baselined: in this cloud the console capture also records SwiftShader GL driver
    messages with memory addresses, so the three recordings differ byte-wise.
  - JS_ONLY_NODE_REFERENCE_LIFETIME: closed. The X3DNodeRef keeps what X_ITE's node keeps (its field objects and their
    callbacks, its FieldDefinitionArray, its user data, and through X_ITE's own SFNodeCache WeakMap its one SFNode object), and
    stays pinned until the runtime releases the node. release.rs now applies every other keep rule first and asks the Script host
    only about the runtime-created nodes that would go; for those the pin is lifted, garbage is collected, and what survives is
    held (an SFNode object, an MFNode element, a field object). Tests: `retention_tables.rs`
    a_created_node_in_the_scene_keeps_its_objects_after_the_script_lets_go (fails if the host is asked about every created node:
    the callback and the property on the SFNode object are lost), script_wrappers_of_released_nodes_keep_their_value_and_then_go
    (the field's handle now goes with the node), `particles_b06_unreachable.rs` (11, including the MFNode case).
  - addUserDefinedField / removeUserDefinedField reload: not reachable from Script content (SFNode does not expose them). No
    observable behavior to compare.
- Remaining differences (unresolved):
  1. scr-browser-open: resolved except the 2 X3DConstants probes (external-component numbering, X_ITE's load order). The
     Browser is now an X3DBrowser (X3DBrowser → X3DBrowserContext → X3DBaseNode → X3DChildObject → X3DObject, each member on
     X_ITE's class), with the 30 members it lacked; getScriptNode returns the node, getExecutionContext is the page's first,
     empty world until the loaded world is installed. New baselined worlds brx-api, brx-call, brx-obj, brx-data and the
     re-recorded scr-browser (each 3 identical X_ITE recordings). Kept open as the owner's boundary: brx-*-open.
  2. scr-classes-open: 19 probes (globals Script.js defines that the port does not: ComponentInfoArray, X3DProtoDeclaration, …).
  3. scr-url-order-open: url candidates that need a fetch finish frames later in X_ITE (timing).
  4. A field callback that throws (owner decision 2026-10-20, approved with conditions): X_ITE lets the exception leave
     X3DField.processEvent, so the remaining callbacks of the event do not run, and its event loop does not catch it either: the
     page stops advancing (oracle world `fcb-throw.wrl`, 2 of 2 recordings hung until the harness timeout, so no baseline). The
     port ends the callbacks of that event at the same point, reports the error at error level (`error: Error: boom`), and goes
     on: the field keeps its value, its routes delivered the event, other fields' callbacks run, the next event runs them all
     again. Test: script.rs a_throwing_field_callback_ends_its_event_and_the_runtime_goes_on. Observable difference: after the
     throw X_ITE does nothing more; the port continues.
- Reason: items 2-3 are unresolved Script behavior. Ledger: PARTIAL kept, decision 4 recorded in its notes.

## node:MetadataSet

- Status: **COMPLETE** (applied 2026-10-20 after the owner's review; meta-parse, meta-events, meta-nodes and gltf_rea re-run
  and exact on the commit that applied it).
- Tests: rea_batch09_sai meta-parse, meta-events, meta-nodes (55 probes, now baselined: field values and events of all six metadata
  nodes, nesting, cycles, USE, printing in VRML/XML/JSON, `instanceof SFNode`, `getNodeType ()` against X_ITE's constants);
  `crates/x3d-parse/tests/gltf_rea.rs` (the 287 X_ITE glTF results, including KHR_materials_variants, whose Switch carries the
  MetadataSet that X3DNode.setMetaData makes, with X_ITE's unique names).
- The ledger's missing item, "X3DNode getMetaData/setMetaData helpers": internal; a Script cannot reach them (SFNode.valueOf ()
  returns the SFNode, not the node). Their only output is the glTF variants metadata, compared above.
- Remaining: meta-parse-open (a node in a PROTO interface field value printed with `DEF _1` by X_ITE) is the printer of PROTO
  field values, not MetadataSet behavior; it is listed under sub:proto.proto in OPEN_WORK.md.
- Reason: every MetadataSet behavior a Script or a parser can observe is compared and matches.

## sub:fields.types

- Status: **PARTIAL** (kept: items 1, 4, 5, 6 below are open).
- Tests: rea_batch09_sai fld-* (29 batteries, every former `-open` probe now in the base worlds), fldx-api (139 probes: the
  statics and the whole prototype chain of all 45 field classes, X3DField and X3DArrayField, against X_ITE's reflection),
  fldx-call (45: a call without `new`), fldx-from (342: fromString / fromVRMLString / fromXMLString / fromJSONString, static and
  on fields, with and without a scene, on a node field, errors and warnings), fldx-value (56: getValue () internals, the math
  classes and their statics), fldx-edge (MF lengths that are not lengths), fcb-* (field callbacks), scr-field-access,
  scr-field-defaults; `crates/x3d-script/tests/script.rs`.
- Verified: everything above matches X_ITE exactly.
- Remaining differences:
  1. X3DConstants of node types from external components: X_ITE numbers them in the order their modules finish loading. Three
     recordings of the Immersive, Interactive, MedicalInterchange and Full profiles gave three different orders; the port numbers
     them in the order X_ITE requests them. Not a parity target (the reference varies run to run).
  2. A length or index near 2^32 (`a .length = 4294967295`): X_ITE allocates the typed array and the page runs out of memory; the
     port throws a RangeError and the field keeps its prior value, in the runtime and in the Script's object. **Approved safety
     difference** (owner 2026-10-20), recorded in this entry's notes, no DIFFERENT status. Tests: script.rs
     huge_mf_length_or_index_is_a_range_error_not_an_allocation (Script fields), huge_length_on_a_node_field_keeps_its_prior_value
     (a Coordinate's point).
  3. A throwing field callback: decided, see node:Script item 4.
  4. IterableWeakSet holds its members strongly (QuickJS corrupts its heap when a WeakRef target in it is collected). Only the
     moment an otherwise unreachable member leaves a parent set differs; X_ITE's moment follows its garbage collector.
  5. An SF component that is neither a number, undefined nor null (a string assigned through an internal value object): the
     Script's object holds it exactly; the runtime stores NaN, so the scene text prints NaN.
  6. Engine level, not fields: QuickJS has no `Intl`; `Error.stack` frames are QuickJS's (the V8 header line is added);
     `Map.prototype.getOrInsert`, `Math.sumPrecise` and `Float16Array` come from X_ITE's polyfills (Features.js) unless the
     engine has them.
- Reason: 1, 4, 5 and 6 are open.

## sub:fields.math

- Status: PARTIAL. Proposed: **PARTIAL** (unchanged).
- Evidence: the Numbers classes a Script uses are X_ITE's own code now (prelude/math.js), checked by fldx-value and the fld-*
  batteries; `crates/r0-2-c-tests/tests/field_math.rs` and the x3d-fields oracle tests cover the Rust Numbers used elsewhere.
- Remaining: the ledger's item is the Rust Geometry math the renderer, picking and navigation use (Box2/Box3, Line3, Plane3,
  Triangle, ViewVolume, Spline in x3d-render / x3d-viewer / x3d-runtime). No direct comparison with X_ITE's standard/Math/Geometry
  exists yet; a node oracle can run those modules unchanged (they import only standard/Math).
- Reason: the Geometry numerics are not compared.

## Local hardware validation (owner decision 2026-10-20)

The 8 tests the cloud container cannot run (two corpus answer keys not in git, no Vulkan adapter, a UDP probe) were run by the
owner on a Linux desktop with an NVIDIA GeForce RTX 3060 Ti against pinned X_ITE 16.5.0 @ `34cdf9f97`, at commit
`0f3de084d8c0630018f8f15e65dd15c9ad0aa749`: 8 passed, 0 failed, 0 blocked. The owner accepted them as PASS. The historical x1
and items answer keys were used unchanged (hashes match). The three time-dependent x1 worlds (244-tardis666, 265-tardis666,
87-weaver_anime_final_fixed2, `new Date ()` in a Script) are documented in `_batch09_context/cloud/OPEN_WORK.md` and
`crates/x3d-script/tests/x1-known.tsv`; they are not Rust defects. Ledger unchanged: 346 entries, 232 COMPLETE, 54 PARTIAL,
25 DIFFERENT, 33 NOT_TESTED, 2 MISSING (231 → 232 COMPLETE is node:MetadataSet).
