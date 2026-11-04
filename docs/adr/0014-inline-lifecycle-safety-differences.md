# 0014. Inline lifecycle differences from X_ITE: c09, c29, c35 kept, c07 partly fixed

Status: **Accepted (decision of the repository owner, 2026-11-04).** Attribution: the owner's written instruction "OWNER DECISIONS: INLINE AND FILELOADER" and its follow-up of 2026-11-04, given to the coordinating agent of the REA campaign (the issue #89 and #90 lane). The partial fix of c07 (section 4) is a decision of the repository owner of 2026-11-04, given to the same coordinating agent. A change to c09, c29 or c35 needs a new decision of the repository owner, recorded the same way (a dated ADR change).

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). Standard: ISO/IEC 19775-1 V4.0, Part 1, 9.3.2 (X3DUrlObject) and 9.4.2 (Inline), read in `new-items/reference/iso-iec-19775-1-v4.0-x3d/markdown/Part01/components/networking.md`. The ISO/IEC 14772-1 4.12.3 clause on `shutdown ()` is not used by any decision of this record (it concerns the `shutdown ()` lines, decided in the ledger entry `node:Inline`); it is only named in the known-difference table.

Cases: `oracle/synthetic-inline-reload/` (scenarios), `oracle/synthetic-inline-reload-baseline.json` (three identical pinned recordings), `crates/x3d-script/tests/synthetic-inline-reload-known.tsv` (the pinned differences), `crates/x3d-script/tests/inline_reload_safety.rs` (the Rust behavior, asserted directly), `crates/x3d-script/tests/inline_reload_shutdown.rs` (the comparison with the recording).

## Context

Four scenarios of `synthetic-inline-reload` differ from pinned X_ITE for a reason that is not the `shutdown ()` call. The ledger entry `node:Inline` stays PARTIAL. This record keeps the differences visible. None of the four is an exact match, and none is called one. After the partial fix of section 4, c07 differs in step 5 only.

Each entry gives: input, pinned behavior, Rust behavior, reason, intent, consequence, decision.

## Decisions

### 1. c09 `disable-while-loading`: keep (owner approved)

Input: `main-one.wrl`; frame 4 `Inl.url = ["content-b.wrl"]`; frame 5 `Inl.load = FALSE`. Authority in the scenario file: IMPLEMENTATION_SAFETY.

Pinned: the request finishes after `load` became FALSE. Steps 5 to 8: `Inl` is COMPLETE with `load` false, B's nodes are live, B's clock and Script keep running ("B fraction 0, 2, 4, 5").

Rust: step 5 on: `Inl` is NOT_STARTED, empty, `load` false. B never initializes after step 4 and its late result is dropped. The scene A is disposed at step 5 (native build: "A shutdown").

Reason: a request that was current when `load` became FALSE is cancelled and the Inline unloads. A late result finds no current request and is dropped (`inline_request_is_current` in `crates/x3d-runtime/src/inline.rs`).

Standard: 9.4.2: "If a FALSE event is sent to the load field of a previously loaded Inline, the contents of the Inline will be unloaded from the scene graph." The standard does not say what happens to a request in flight; the rule above extends the unload to it. This extension is the decision.

Intent: deliberate. Consequence (lifecycle safety): X_ITE shows a scene and runs its Scripts in an Inline that the author switched off. Rust cannot.

Decision: keep cancellation and unloading when `load` becomes FALSE. Test: `disabled_inline_drops_the_late_result_and_recovers` (a later TRUE loads again).

### 2. c29 `script-interests`: keep (owner approved)

Input: `main-prep.wrl`; the Inline shows `content-prep.wrl` (a Script with `prepareEvents` and `eventsProcessed`); frame 5 `Inl.url = ["content-b.wrl"]`. Authority: IMPLEMENTATION_SAFETY.

Pinned: the replaced scene is disposed, its Script is not. The Script keeps receiving `prepareEvents` to the end of the recording ("P prepare 10" at step 6, then 11, 12, ... to step 12, the last step); the recording is 12 steps long, so "for ever" is the reading of the pinned source comment in the known-difference table ("the scene is disposed, the Script is not"), not a measurement.

Rust: "P prepare 10" at step 6 only. The interests (`prepareEvents`, `eventsProcessed`) of a Script of a disposed scene are removed with the scene.

Reason: the Rust port removes a Script's event interests when its scene is disposed.

Intent: deliberate. Consequence (lifecycle safety): in X_ITE a Script of a replaced scene runs user code every frame through the whole recording (12 steps, measured); it can also change nodes with `directOutput`. Whether it runs without a limit is not measured: it is read from the comment in the known-difference table, and no pinned source path was read for this record. Rust stops it.

Decision: keep cancellation of Script interests when the owning Inline scene is disposed. Test: `disposed_scene_cancels_nested_loads_and_script_interests`.

### 3. c35 `auto-refresh-after-failure`: keep (owner approved)

Input: `main-late-refresh.wrl` (X3D 4.0): `Inline { url "late.wrl" autoRefresh 0.5 }`; the file `late.wrl` does not exist, and appears at frame 4 (a copy of `content-b.wrl`). Authority: X3D_REQUIRED.

Pinned: `Inl` is FAILED at steps 3 and 4, IN_PROGRESS at step 5, COMPLETE at step 6: X_ITE refreshes after the failure and loads the file.

Rust: `Inl` stays FAILED (`X3D_REQUIRED`) at every step. An explicit new request (`url` or `load`) loads it.

Standard: 9.3.2: "If the preceding file loading fails or the load field is FALSE, no automatic refresh is performed."

The clause says that after a failed load no automatic refresh happens, and the recording shows X_ITE refreshing after the failure. So X_ITE does not follow this clause in this case, and Rust does. The decision follows the standard: it is not a compatibility difference that is kept for X_ITE's sake. (The clause says nothing that allows a refresh after a failure: the refresh rule for the `url` list in the same clause applies to a refresh that fails after a successful load.)

Intent: deliberate, by the standard. Consequence: a failed Inline does not retry by itself, so no timer keeps running for a failed load.

Decision: keep the X3D 4.0 behavior. Test: `no_auto_refresh_after_a_failed_load`.

### 4. c07 `overlap-frames`: partly fixed, step 5 kept (owner decision of 2026-11-04)

Input: `main-one.wrl`; frame 4 `Inl.url = ["content-b.wrl"]`; frame 5 `Inl.url = ["content-c.wrl"]`. Authority: XITE_COMPATIBILITY.

Pinned: step 5: C is COMPLETE, `loadCount` 0; console "B fraction 0, parent tick 2, A fraction 5, C initialize". Step 6: "C fraction 0, B fraction 2".

Cause (pinned source, read for this record). `FileLoader.js:146` (`set_loadCount__`) awaits `browser.nextFrame()` (`X3DBrowserContext.js:184-196`, resolved in `_finishEvents` at `:273`), so a parsed scene is applied at the end of the frame after it was parsed. At frame 5, `Inline.js:75` calls `fileLoader.abort()` on B's loader; `abort ()` only cancels the fetch (`FileLoader.js:74-78`). B's data had arrived, so B is applied at the end of frame 5 (`Inline.js:78-96`: the old scene A is disposed at `:81`, the state becomes COMPLETE at `:96`, the loading object is removed in `X3DUrlObject.js:59`). At frame 6 B is live ("B fraction 2") and C is applied at the end of frame 6. So C completes at step 6 in both engines. The earlier reading of this record, a one-frame completion difference of C, was wrong; the difference was B being dropped.

Rust before the fix: `inline_abort` discarded the scene of the older request that had arrived, and `inline_request_is_current` (same generation and IN_PROGRESS) dropped B's result at its two uses (`scene_load_count`, `finish_frame` in `crates/x3d-runtime/src/inline.rs`). Step 6 console was "C fraction 0, A fraction 6".

Decision (owner, 2026-11-04): the partial fix. The scene of an older request that has arrived is applied while a newer request is pending (rules 1 and 2 below), and the state of the newer request is not changed by it (rule 4). X_ITE sets the Inline COMPLETE and removes the loading object when it applies B (`Inline.js:96`, `X3DUrlObject.js:59`). Rust does not: C stays IN_PROGRESS, with its generation, and the `loadCount` of the parent and the load state belong to C until C completes. So step 5 differs in `loadCount` (X_ITE 0, Rust 1) and in the `loadState` of `inlines` (X_ITE COMPLETE, Rust IN_PROGRESS), and nothing else. This is a kept difference for step 5 only (`c07-overlap-frames  5  5  inlines,loadCount` in `synthetic-inline-reload-known.tsv`; the browser build equals X_ITE in every other field and step, the native build adds the `shutdown` lines of the other entries). The other steps of c07 equal the recording.

Invariant change. Before the fix every result of an older generation was dropped, whatever the state of the newer request. Now the generation alone does not decide: `inline_result_may_apply` accepts the current request (as before: same generation, IN_PROGRESS), and an Inline also accepts an older one when the Inline's request is newer and IN_PROGRESS (pending, not disabled). A newer request that completed, failed or was cancelled by `load` FALSE leaves no IN_PROGRESS state, so the older result is dropped. `inline_request_is_current` is unchanged and still guards every state write and a fetch that arrives after its request was replaced (an aborted fetch delivers nothing). `inline_abort` keeps the arrived scenes of an Inline only for a new request; `load` FALSE, a dispose of the container and an InlineGeometry discard them as before.

Rules the fix follows (each has a regression test in `crates/x3d-script/tests/inline_reload_safety.rs`; the generation guard is not weakened):

1. A result of an older request may be applied only while a newer request is still pending, and only if the Inline was not unloaded (`load` FALSE) and no newer request has completed or failed. Tests: `arrived_older_scene_is_shown_while_the_newer_request_is_pending` (c07 itself), `older_scene_with_a_nested_load_is_shown_while_the_newer_request_is_pending`.
2. An older scene that was applied in this way is disposed before the newer scene becomes current; it never becomes the scene that survives. Same two tests (B does not run after C became current; no orphan live scene).
3. A result of an older request that arrives after the newer request has completed, or after it has failed, is dropped. Test: `older_scene_after_the_newer_request_failed_is_dropped`.
4. An older result never changes the state (load state, load count, failure) that belongs to the newer request. `show_older_scene` writes no state. Tests: the first two above assert IN_PROGRESS and `loadCount` 1 while the older scene is shown.
5. `load` FALSE cancels every outstanding request, whatever its generation (c09 stays as decided above), and an older scene that was applied is unloaded with it. Test: `load_false_cancels_an_arrived_older_scene`.

Negative controls (run once, 2026-11-04): reverting the change in `inline.rs` fails `arrived_older_scene_is_shown_while_the_newer_request_is_pending`, `older_scene_with_a_nested_load_is_shown_while_the_newer_request_is_pending` and `synthetic_inline_reload_browser_build_matches_x_ite_shutdown`; removing the IN_PROGRESS test from `inline_result_may_apply` fails `older_scene_after_the_newer_request_failed_is_dropped`.

The decision on step 5 follows the safety reading of rule 4: the Inline does not report COMPLETE while the request that it reports on is still loading. A change of this difference needs a new decision of the repository owner.

The EXTERNPROTO disposed-context behavior (issue #58) is a different case and already matches X_ITE 16.5.0.

## Consequences

The known-difference table keeps c09, c29 and c35, and c07 for step 5 only. The ledger status of `node:Inline` and `sub:proto.proto` does not change. A change to c09, c29 or c35 needs a new owner decision.
