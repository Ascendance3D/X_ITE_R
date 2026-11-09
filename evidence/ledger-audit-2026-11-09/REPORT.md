# Ledger audit 2026-11-09

Starting point: origin/master `d79abe4c`, ledger 284 COMPLETE / 40 PARTIAL / 21 DIFFERENT / 1 MISSING, total 346.
The audit maps every entry to the tests that ran; it changes no entry by itself.

## Runs

- Strict suite (`tools/strict-suite.sh`, X3D_EVIDENCE unset, answer keys present, RTX 3060 Ti, 6 test jobs) at `d79abe4c`:
  nextest 1666 tests run, 1666 passed, 10 skipped (the `#[ignore]` tests); doctests 5 passed.
- `node --test 'tests/xr/*.test.mjs'` in `crates/x3d-web` at `d79abe4c`: 28 tests, 28 passed.
- New `crates/x3d-viewer/tests/classic_edge_runtime.rs` with `crates/x3d-parse/tests/classic_edge.rs`: 2 passed.
  Negative control: comparing the whole multi-line text instead of the first line fails on `str-script-url-list` only.

## Method

`ledger/tools/audit_1109.py <nextest log>` writes `audit.tsv` (one row per entry).
Pass 1 matches the Rust test binaries and test functions that an entry names; pass 2 maps the oracle directories,
synthetic suites and corpus suites that an entry names to the Rust tests whose source reads them.
A file name or a test name alone is not taken as proof: the class says whether the matched tests ran and passed.

| status | RUST-PASS | TRACED-PASS | OTHER-EVIDENCE | NO-TRACE |
| --- | --- | --- | --- | --- |
| COMPLETE (284) | 194 | 82 | 4 | 4 |
| PARTIAL (41 after this step) | 41 | 0 | 0 | 0 |
| DIFFERENT (21) | 18 | 2 | 0 | 1 |

No matched Rust test failed or was skipped (a skipped match would be RUST-SKIP, not a pass; the strict-suite log has no
SKIP line). OTHER-EVIDENCE is a reference only: the audit does not run the named .mjs / .py checks; they are traced by hand
below. The ledger.json audited is the one after this step (sub:browser.webxr names its tests now).

## Manual traces (the 9 rows without a Rust match)

- `node:ConeEmitter`, `ExplosionEmitter`, `PointEmitter`, `SurfaceEmitter` (OTHER-EVIDENCE): the entries name
  `particles_b06_{exact,stat,events,retention}.rs` in brace form; those binaries ran and passed.
- `sub:routing.event-loop` (NO-TRACE): "followers, x6, inline scripts traces" = `x3d-script::corpus`
  `synthetic_followers_matches_x_ite` and `synthetic_x6_matches_x_ite`: passed.
- `sub:browser.dom-integration` (DIFFERENT, NO-TRACE): no Rust evidence by design. See the finding below.
- `node:Switch`, `node:ProximitySensor`, `node:VisibilitySensor` (COMPLETE, NO-TRACE): see the finding below.

## Findings

1. `sub:browser.webxr` was MISSING with no code and no tests. Browser WebXR is implemented (PRs #513, #515) and compared
   with the pinned X3DWebXRContext. Changed to PARTIAL in this step. The ledger has exactly 346 IDs (`verify_ledger.py`),
   so native OpenXR stays a gap of this entry; it is not a failure of the browser path.
2. `sub:render.transparency-sorting` still listed two oit01 todo cases that pass since PR #499. Text corrected.
3. `sub:parse.vrml-classic`: the 6 resource-load log differences equal pinned X_ITE in the runtime (new test). They are
   the console reports of failed loads; they do not change scene loading, event order or the API. Text corrected.
4. `node:Collision`: the gap text named only the first compared paths. Only `set_live__` has no case. Text corrected.
5. `docs/webxr-runtime-options.md` said the XRSession rendering property was not wired; it is (`web/x_ite_r.mjs`). Corrected.
6. `oracle/synthetic-rea-verify`: five of its seven worlds (switch-lod-billboard, proximity-visibility,
   collision-children, anchor-static, bind-stack) are named as evidence but no current test reads them, and no
   baseline for them is in the tree or in the private store. ProximitySensor and Switch are compared by other cases
   (rea_batch12 vw-*, many render and reload groups); VisibilitySensor has no other pinned comparison.
   Not downgraded (the code and the earlier comparison are not shown wrong); the comparison is being restored in a
   separate PR.
7. `sub:browser.dom-integration` is DIFFERENT with the note "DOM-only feature; native host has no DOM". The web build
   implements DOM integration (issue #146; `crates/x3d-web/tests/browser/dom01..03.test.mjs` against pinned X_ITE, one
   known difference; `web/API.md` "DOM integration"). The note is true for the native build only. A DIFFERENT status
   is frozen by `verify_ledger.py` and is the owner's decision, so it is reported here and not changed.

## Rationale spot checks of DIFFERENT entries (unchanged)

- Annotation (6): `SupportedComponents.js:5` still has the Annotation component commented out in the pin.
- SplinePositionInterpolator (2): `CatmullRomSplineInterpolatorTemplate.js:35` calls `value .norm ()`, which SFVec
  values lack: the upstream failure stands.
- NURBS (11) and SpatialSound: owner decisions ("ISO everywhere", HRTF data licensing); no new evidence.
