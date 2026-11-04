# 0014. Inline lifecycle differences from X_ITE: c09, c29, c35 kept, c07 to be fixed

Status: **Accepted (owner decision, 2026-11-04).**

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). Standard: ISO/IEC 19775-1 V4.0, Part 1, 9.3.2 (X3DUrlObject) and 9.4.2 (Inline), read in `new-items/reference/iso-iec-19775-1-v4.0-x3d/markdown/Part01/components/networking.md`. ISO/IEC 14772-1 4.12.3 is quoted from `crates/x3d-script/tests/synthetic-inline-reload-known.tsv`; it was not re-read for this record.

Cases: `oracle/synthetic-inline-reload/` (scenarios), `oracle/synthetic-inline-reload-baseline.json` (three identical pinned recordings), `crates/x3d-script/tests/synthetic-inline-reload-known.tsv` (the pinned differences), `crates/x3d-script/tests/inline_reload_safety.rs` (the Rust behavior, asserted directly), `crates/x3d-script/tests/inline_reload_shutdown.rs` (the comparison with the recording).

## Context

Four scenarios of `synthetic-inline-reload` differ from pinned X_ITE for a reason that is not the `shutdown ()` call. The ledger entry `node:Inline` stays PARTIAL. This record keeps the differences visible. None of the four is an exact match, and none is called one.

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

Pinned: the replaced scene is disposed, its Script is not. The Script keeps receiving `prepareEvents` for ever ("P prepare 10" at step 6, then 11, 12, ... to the last step).

Rust: "P prepare 10" at step 6 only. The interests (`prepareEvents`, `eventsProcessed`) of a Script of a disposed scene are removed with the scene.

Reason: the Rust port removes a Script's event interests when its scene is disposed.

Intent: deliberate. Consequence (lifecycle safety): in X_ITE a Script of a replaced scene runs user code every frame until the page ends (a leak that can also change nodes with `directOutput`). Rust stops it.

Decision: keep cancellation of Script interests when the owning Inline scene is disposed. Test: `disposed_scene_cancels_nested_loads_and_script_interests`.

### 3. c35 `auto-refresh-after-failure`: keep (owner approved)

Input: `main-late-refresh.wrl` (X3D 4.0): `Inline { url "late.wrl" autoRefresh 0.5 }`; the file `late.wrl` does not exist, and appears at frame 4 (a copy of `content-b.wrl`). Authority: X3D_REQUIRED.

Pinned: `Inl` is FAILED at steps 3 and 4, IN_PROGRESS at step 5, COMPLETE at step 6: X_ITE refreshes after the failure and loads the file.

Rust: `Inl` stays FAILED (`X3D_REQUIRED`) at every step. An explicit new request (`url` or `load`) loads it.

Standard: 9.3.2: "If the preceding file loading fails or the load field is FALSE, no automatic refresh is performed."

Intent: deliberate, by the standard. Consequence: a failed Inline does not retry by itself, so no timer keeps running for a failed load.

Decision: keep the X3D 4.0 behavior. Test: `no_auto_refresh_after_a_failed_load`.

### 4. c07 `overlap-frames`: not accepted; fix queued (owner decision)

Input: `main-one.wrl`; frame 4 `Inl.url = ["content-b.wrl"]`; frame 5 `Inl.url = ["content-c.wrl"]`. Authority: XITE_COMPATIBILITY.

Pinned: step 5: C is COMPLETE, `loadCount` 0; console "B fraction 0, parent tick 2, A fraction 5, C initialize". Step 6: "C fraction 0, B fraction 2".

Rust (browser build): step 5: `Inl` IN_PROGRESS, `loadCount` 1, console as pinned. Step 6: COMPLETE, console "C fraction 0, A fraction 6" (the pinned page prints "B fraction 2" there). The native build adds `shutdown` lines.

Reason (verified in the Rust code, not yet in the pinned source): every request has a generation number; a result of an older generation is dropped. B is dropped, so C completes one frame later. X_ITE applies B's result before C's.

Decision: this is not a permanent difference. Fix: apply the result of an older request when it arrives before a newer request completes, where the pinned behavior is safe; never let an older request replace a newer completed scene; do not weaken the generation guard; find the cause of the one-frame completion difference. Use the existing cases and strict negative controls. The difference stays in `synthetic-inline-reload-known.tsv` until the fix merges.

The EXTERNPROTO disposed-context behavior (issue #58) is a different case and already matches X_ITE 16.5.0.

## Consequences

The known-difference table keeps all four cases. The ledger status of `node:Inline` and `sub:proto.proto` does not change. A change to c09, c29 or c35 needs a new owner decision.
