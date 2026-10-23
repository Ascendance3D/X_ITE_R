#!/usr/bin/env python3
"""ParticleSystem pointing baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU) for crates/x3d-viewer/tests/rea_b14_particle_pick.rs.

  python3 -I -B oracle/tools/rea_b14_pick.py record NAME [X3D_ORACLE]

runs `x3d-oracle particles` (default target/release/x3d-oracle) three times on oracle/synthetic-rea-b14-particle-pick/NAME.x3dv
with scripts/NAME.json. `--png-dir` is always passed: it makes the page reshape its drawing buffer to the 512 x 512
canvas, without which browser .touch has a 1 x 1 viewport; the images themselves are not kept. The three js.json and
js.f32 must be identical; the first is kept as oracle/rea-batch14-particle-pick-baseline/NAME/. A baseline is never
taken from Rust.
"""
import filecmp, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
mode, name = sys.argv[1:3]
assert mode == "record"
oracle = sys.argv[3] if len(sys.argv) > 3 else os.path.join(root, "target", "release", "x3d-oracle")
world = os.path.join(root, "oracle", "synthetic-rea-b14-particle-pick", name + ".x3dv")
script = os.path.join(root, "oracle", "synthetic-rea-b14-particle-pick", "scripts", name + ".json")
runs = []
for i in (1, 2, 3):
    out = f"/tmp/rea-b14-pick-record/{name}-{i}"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    subprocess.run([oracle, "particles", world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "png")],
                   check=True, cwd=root, timeout=300, stdout=subprocess.DEVNULL)
    runs.append(out)
a = runs[0]
same = all(filecmp.cmp(os.path.join(a, f), os.path.join(b, f), shallow=False) for b in runs[1:] for f in ("js.json", "js.f32"))
print(f"{name}: three X_ITE runs {'identical' if same else 'DIFFER'}")
if not same:
    sys.exit(1)
dest = os.path.join(root, "oracle", "rea-batch14-particle-pick-baseline", name)
shutil.rmtree(dest, ignore_errors=True)
os.makedirs(dest)
for f in ("js.json", "js.f32"):
    shutil.copy(os.path.join(a, f), dest)
