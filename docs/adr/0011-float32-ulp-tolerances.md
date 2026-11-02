# 0011. Float32 ulp gaps of particles, PointSet and line picking: bounded tolerances, one fix

Status: **Accepted** (decided in issue #53 from recordings and the standard; the owner can still ask for a fix ticket for any gap).

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). 16.6.0 is out of scope.

## Context

Four cases were listed (a fifth, `pr-line` at MSAA 4, was the mirrored sample pattern and is closed on master by issue #261) as `KNOWN_GAPS` that "must keep failing", which guards nothing: a probe that moved further away still passed. Ledger entries: `node:ParticleSystem`, `node:PointSet`, `sub:pointer.picking`.

## Decision

Each gap now has a stated bound that the test enforces in both directions of the claim (the case passes inside the bound and fails outside it). The bounds are the largest differences the recordings show (3 identical X_ITE recordings per case).

| Gap | Test | Bound | Reason |
| --- | --- | --- | --- |
| PointProperties points, hit texCoord (`gl_PointCoord`) | `rea_batch12` `pk-points-style`; `rea_b14_particle_pick` `pp-row-point-props`, `pp-slant-point-props` | 3 float32 ulps; 1 ulp for the particle cases | The oracle GPU's point sprite coordinate is not reproduced to the last bit; ids, points and normals are bit-exact |
| Line hit points | `rea_batch12` `pk-lines`; `rea_b14` `pp-row-line` | 398 ulps (pk-lines: 63 on aliased lines, 398 on transformed thick lines); 3 ulps (pp-row-line) | GL line rasterization vs. the quad emulation; X_ITE builds thick-line triangles by transform feedback in float32, Rust on the CPU in double (Rust is the closer to the exact value) |
| `pc-line` collision depth, frame 45 | `rea_b15_particle_collision` | distance 2e-5 (measured 1.68e-5 along one direction) | Unexplained remainder at the right-edge pixel of one line (see Unknowns) |

One gap was an engine error, not noise, and is fixed: `line_fragments` (the collision depth pass's rasterization of 1 px LINE particles) clamped the interpolation parameter of a pixel centre to the segment, but GL does not clamp it, so the last pixel of the diamond-exit rule, which can lie past an end, extrapolates. The clamp gave the constrained translation 4 of `pc-line` frame 45 an error of 6e-3 (a branch of the collision response that the 1.7e-5 depth difference tipped); without it the translations match to 4.5e-8. The `pc-line` entry of `CPU_KNOWN_GAPS` is gone for the same reason.

## Standard

ISO/IEC 19775-1 V4.0 5.3.5 (SFFloat): "Implementation of these fields is targeted at the single precision floating point capabilities of processors" and a fixed-point implementation must keep "at least six decimal digits of precision". The standard states no accuracy for a computed hit point (`hitPoint_changed`, pointing device sensors), for GL's rasterization of lines or points, or for collision depth. A float32 ulp is about 1.2e-7 of the value, so 3 ulps are inside six digits; 63 and 398 ulps (up to about 4.7e-5 of the value) are not, but they are the pinned X_ITE's own float32 and rasterization behavior, which the engine matches within the bound above. ISO/IEC 19775-2 is not local and was not checked.

## Unknowns

* The 1.7e-5 distance remainder of `pc-line` direction 7 is not explained. X_ITE's depth there is farther than the engine's extrapolated line depth.

## Consequences

`node:PointSet` is `COMPLETE` in the ledger; the issue named it as PARTIAL, but its only gap (`pk-points-style`) is carried by `sub:pointer.picking`. The ledger (`ledger/ledger.json`) is not changed by this record; the missing texts of `node:ParticleSystem` and `sub:pointer.picking` can name these bounds instead of KNOWN_GAP.

## Evidence

* Tests: `crates/x3d-viewer/tests/rea_batch12.rs` (`PICK_ULPS`), `rea_b14_particle_pick.rs` (`PICK_ULPS`), `rea_b15_particle_collision.rs` (`PROBE_TOLERANCES`), `tests/common/ulps.rs`.
* REA (pinned source, summary views): `Browser/PointingDeviceSensor` `ev_e030ce0b38bc998607b1911e594e4a9328570db2035e85973b3babd2d90140f7`, `Components/ParticleSystems` `ev_b4a4029e9e11e554f6e929d3c75e0c65d5eacce220d03f3e15a284b2a1f7b1cc`. The graphs are partial.
* Format: as ADR 0001.
