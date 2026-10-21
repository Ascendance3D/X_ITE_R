# Proposed REA ledger changes, B06-B08 final lane

The authoritative ledger was NOT changed. Totals if accepted: COMPLETE 215, PARTIAL 71, DIFFERENT 25, NOT_TESTED 33, MISSING 2 = 346.

1. Apply Batch 08's `_batch08_context/reports/ledger/proposed/CHANGES.md` (20 entries: 5 DIS COMPLETE, 15 PARTIAL) with `set_status.py`; DIS suites passed in this lane's run.
2. Then downgrade COMPLETE to PARTIAL: `node:VolumeEmitter`, `node:BoundedPhysicsModel` (replays not run), `node:IsoSurfaceVolumeData`, `node:SegmentedVolumeData` (pinned-X_ITE ordering cases not written).
3. Unchanged: `sub:physics.engine` PARTIAL (3 cylinder picking cases, 20,000-case sweep reproduced), `node:WindPhysicsModel` PARTIAL, all five shader entries PARTIAL, eight volume style entries (no change; per-style combined test not added), WebXR and keyboard UI MISSING, all 25 DIFFERENT.
4. Run `ledger_verify.py` after applying; the 215/71 arithmetic is not yet checked by it.
