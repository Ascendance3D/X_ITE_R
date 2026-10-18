# Batch 08 lane `physics` — sub:physics.engine (ammo.js backend)

Recommended status: COMPLETE for the dynamics engine (all 109 Ammo symbols that pinned X_ITE 16.5.0 reached have
a Rust implementation and a test against Ammo or X_ITE). One documented known limit remains in the picking
pipeline (3 of 20000 fresh random cylinder pairs, 0.015%, see Known limits). If the owner wants zero differences
in that sweep, keep the entry at PARTIAL for picking only.

## What was done

1. Coverage table: `coverage.tsv` (122 rows) built by `make_coverage.py` from `trace_union.json`. The union comes from
   52 pinned X_ITE runs (47 rea-b06 physics worlds + 5 vpick worlds) with an instrumented AmmoClass
   (`oracle/x_ite-anim-ammotrace.mjs`: Proxy on constructors, wrapped prototype methods and statics).
   The script stops if a traced symbol has no row, so the table cannot silently miss one.
   Sections: reached 109 (all with tests), static-only 3, never reached 10 (with the reason).
2. New Ammo-in-node vectors, generated from the pinned ammo.js by `oracle/tools/ammo_dyn_b08.mjs`
   into `crates/x3d-runtime/tests/data/bullet_b08` (16 scenes, 21 corpora). New test
   `crates/x3d-runtime/tests/bullet_b08_engine.rs` (37 tests), tolerance 0 everywhere:
   height field (6 scenes), convex vs BVH mesh and vs height field (pairs + solve, 5 shapes each), multi-collidable
   bodies, joint chains (ball/hinge/slider), DoubleAxisHinge frames, run-time ops (world rebuild, joint
   remove/add, pivot change, margin/gravity, field writes, sleeping body removed/re-added, body removed with joint).
3. Lifecycle: `crates/x3d-runtime/tests/physics_b08_lifecycle.rs` (5 tests): counting allocator, 100 cycles of
   world/body/shape/constraint create and destroy, structure counts via `World::stats()` flat, heap flat
   (50 vs 100 cycles equal; warm-up bound 1024 B, measured 64 to 736 B), scene reload.
4. Fixes in the engine:
   - `physics/bt/broadphase.rs`: proxy slots were never reused (real leak, grew every body re-add). Now a free list.
   - `physics/bt/world.rs`: added `WorldStats` / `World::stats()` (read only, for tests).
   - `bullet.rs` `Gjk::evaluate`: the ray vector is an asm.js local (unrounded double) inside the loop and is
     rounded only when stored to memory. Rust rounded it too early. Fixed with `ray_locals`. This removed the
     06R residue of 7/240 deep cone/cylinder cases and the two failures found by the new tests.

## Gates (final code, `-j 8 --release`, logs in `logs/`)

| gate | result |
| --- | --- |
| bullet_box_box | 1 pass |
| bullet_dyn_scenes | 66 pass |
| bullet_b08_engine (new) | 37 pass |
| physics_b08_lifecycle (new) | 5 pass |
| physics_b06_baseline | 48 pass |
| physics_b06_lifecycle | 6 pass |
| rea_b03_pick_vectors (4000 committed cases) | pass |
| `python3 -I oracle/tools/rea_b06.py check` | 47 pass, 0 fail, all deltas 0 |
| fresh 20000-case pick sweep (seed 20261018) | 20000 cases, 3 differ |

## Commands

```
export CARGO_TARGET_DIR=/tmp/b08/physics/target CARGO_BUILD_BUILD_DIR=/tmp/b08/physics/build
cargo test --release -j 8 -p x3d-runtime --test bullet_box_box --test bullet_dyn_scenes --test bullet_b08_engine --test physics_b08_lifecycle
cargo test --release -j 8 -p x3d-viewer --test physics_b06_baseline --test physics_b06_lifecycle --test rea_b03_pick_vectors
python3 -I oracle/tools/rea_b06.py check
node oracle/tools/ammo_dyn_b08.mjs [outDir] [scenes|corpora|names]     # regenerate vectors from pinned Ammo
node oracle/x_ite-anim-ammotrace.mjs ...                                # Ammo call trace (adds result.ammoTrace)
node oracle/tools/ammo_pick_vectors.mjs OUT.json 20000 20261018         # then PICK_VECTORS=OUT.json cargo test ... rea_b03_pick_vectors
python3 -I make_coverage.py                                             # in this report dir
```

## Files

New: `oracle/x_ite-anim-ammotrace.mjs`, `oracle/tools/ammo_dyn_b08.mjs`, `crates/x3d-runtime/tests/bullet_b08_engine.rs`,
`crates/x3d-runtime/tests/physics_b08_lifecycle.rs`, `crates/x3d-runtime/tests/data/bullet_b08/*.json` (37 files, from pinned Ammo),
and in this dir: REPORT.md, coverage.tsv, make_coverage.py, trace_union.json, residual_pick_cases.json, logs/.
Small hunks in existing (untracked, 06R-era) files: `physics/bt/broadphase.rs` (free list), `physics/bt/world.rs` (WorldStats),
`bullet.rs` (GJK ray locals). No baseline, existing test or other lane file was changed. The temporary debug hook
added to `pick_shapes.rs` was removed (byte-identical to before).

## Coverage notes

- Kinematic bodies: X_ITE never makes one (`fixed` = mass 0 static). Fixed bodies are covered in every scene with a floor.
- UniversalJoint and MotorJoint create no Ammo constraint in X_ITE; node fields only, covered by rea-b06 worlds.
- Capsule is excluded by X_ITE; the Rust engine has it anyway (dyn scene s13b).
- Hinge limit/motor calls are not forwarded by X_ITE, so they are not ported. Ray/sweep/ghost APIs are unused by X_ITE.
- Picking is separate code (`bullet.rs` + `pick_shapes.rs`), tested by pick vectors and the 5 vpick worlds.
- Float-store rounding: kept as in 06R (doubles for asm.js locals, `f()` at struct stores).

## Known limits

- Pick sweep: 3 of 20000 differ (cases 4697, 5480, 11878; data in `residual_pick_cases.json`, script
  `logs/pick_dbg_case_5480.mjs`). All are deeply overlapping scaled cylinder pairs (Rust depth 0.13, 0.87,
  0.44). Pinned Ammo reports no manifold. Facts measured on case 5480: a bare scaled cylinder pair in Ammo gives a
  contact of -0.8686 (equal to Rust); the same pair wrapped in the VolumePicker compounds gives none; AABBs
  agree to float precision. So the difference is inside the compound path (child transform rounding feeding GJK/EPA
  on a near-degenerate deep configuration). Root cause not proven; no tolerance was widened. The committed
  4000-case vector set passes at 0.
- Ammo's `getAabb`/`Ammo.destroy` details inside asm.js are only checked through behavior, not memory layout.

## X_ITE quirks reproduced

Slider frame typo (frameInA used for both), removal then re-add of all bodies in `set_dynamicsWorld__`, bounce set
only when |v| >= minBounceSpeed, compound recalculation on every `updateChildTransform`, picker compound local AABB
that only grows.

## Owner decisions

1. Accept the 0.015% pick residue as a known limit, or ask for a deeper trace with an instrumented ammo.js EPA/GJK
   build around the compound path (about a day).
2. Optional node-level ElevationGrid worlds with a new X_ITE baseline were not added (the engine level height field is
   covered at 0 and rea-b06 has shape-elevation worlds).
