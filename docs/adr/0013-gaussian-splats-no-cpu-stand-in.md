# ADR 0013: GaussianSplats has no CPU pointing or collision stand-in

Status: Accepted. Refs #69, criterion 1 ("a CPU fallback where the viewer has one").

## Context

The viewer has two CPU paths, both behind `--cpu-pointing` (`x3d-view`, `x3d-drive`, `x3d-particles-drive`):

* a CPU ray cast for pointing (`crates/x3d-viewer/src/pointer.rs`, `cpu_candidates`), and
* a CPU depth stand-in for avatar collision (`crates/x3d-viewer/src/collision.rs`, `depth_cpu`).

They exist for two reasons stated in the code: a host with no adapter must still pick and collide, and the CPU path is an
independent check of the GPU path for triangles, points and lines (ADR 0010 notes the CPU path intersects in f64 and was not
measured against X_ITE). The pinned X_ITE has no CPU path. Its pointing renders ids into a `PointingBuffer` and reads the
pixel; its collision renders into `depthBuffer` and reads depth (`X3DPointingDeviceSensorContext.touch`,
`X3DRenderObject` depth pass, `TextureBuffer.readDepth`). Both are GPU-only for every shape.

A splat is not geometry that a CPU can test directly. Its picture is a screen-space quad of a projected 3D Gaussian with a
per-fragment alpha cut-off, sized from the display viewport and ordered by the sort. A CPU path would be a second copy of
that GPU program (about the splat vertex and fragment math) with its own rounding, for a case the pinned implementation does
not have.

## Decision

GaussianSplats has no CPU stand-in for pointing or collision. With `--cpu-pointing` a splat is not picked and does not
stop the avatar. The GPU path is the only path, as in the pinned X_ITE.

## Evidence

* Pinned X_ITE (REA, `oracle/x_ite-pin`): ev_7d2c269ce9768e2f04e4617fa7f07ad940c80a36301f8b8a3b3d0014850ea735 (summary of
  ev_d2e0ac37315cb9fdfd4d9b293ad984cf098ceb274851a2667a74ee7c2e83658a): `X3DPointingDeviceSensorContext.js`,
  `X3DRenderObject.js`, `TextureBuffer.js`, `GaussianSplatsShape.js`. No CPU intersection of splats exists in them. REA covers
  these four files only; the other files were read with `rg` (no `Raycast` or CPU hit test in Browser/PointingDeviceSensor).
* Standards: ISO/IEC 19775-1 V4.0 (local text) defines the result of picking (the sensor nearest to the pointer, the
  intersection point; pointingDeviceSensor.md) and of collision (23.2.4, navigation.md), and not the method. Nothing in the local
  standards library requires a CPU path. 19775-2 is not local: unverified.
* Test `crates/x3d-viewer/tests/b69_gs_cpu_standin.rs`: the GPU path hits the splats (217 pick hits and 7 closest-object
  hits, equal to the pinned baselines), `--cpu-pointing` has 0.

## Consequences

* Criterion 1 for GaussianSplats: GPU path only, by this decision.
* A host without an adapter cannot pick or collide with splats; it could not draw them either.
* Reversing this needs a new ADR and a change of the test above.
