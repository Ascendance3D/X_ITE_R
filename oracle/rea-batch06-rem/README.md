# Batch 06R particle cases kept for the remediation lane

`volume-offset` (VolumeEmitter with its volume at x 2.5..3.5) and `bounded-small` (PointEmitter with
spherical velocity and a 1x1x1 BoundedPhysicsModel). `js.json` / `js.f32` are the saved pinned X_ITE 16.5.0
output of the 06R closure (`/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-06r-qa-closure/particles/out/static-*`;
a second X_ITE run was bit-identical). `world.x3dv` / `script.json` are the closure's inputs
(`root/worlds/geom-*.x3dv`, `root/scripts/static-*.json`). Nothing here is Rust output.
Tests: `crates/x3d-viewer/tests/particles_b06_rem.rs`.
