#!/usr/bin/env python3
"""GaussianSplats avatar-collision baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU) for crates/x3d-viewer/tests/b69_gs_collision.rs.

  python3 -I -B oracle/tools/b69_gs_collision_record.py record NAME [X3D_ORACLE]

runs `x3d-oracle particles` (default target/debug/x3d-oracle) FIVE times on oracle/synthetic-b69-gs-collision/NAME.x3dv with
scripts/NAME.json. `--png-dir` is always passed, as for oracle/tools/rea_b15_collision.py. The five js.json and js.f32 must be
byte-identical; only then is the first kept as oracle/b69-gs-collision-baseline/NAME/. A baseline is never taken from Rust.
Staging is /tmp/b69-gs-collision-record.
"""
import filecmp, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
assert sys.argv[1] == "record"
name = sys.argv[2]
oracle = sys.argv[3] if len(sys.argv) > 3 else os.path.join(root, "target", "debug", "x3d-oracle")
world = os.path.join(root, "oracle", "synthetic-b69-gs-collision", name + ".x3dv")
script = os.path.join(root, "oracle", "synthetic-b69-gs-collision", "scripts", name + ".json")
runs = []
for i in range(1, 6):
    out = f"/tmp/b69-gs-collision-record/{name}-{i}"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    subprocess.run([oracle, "particles", world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "png")],
                   check=True, cwd=root, timeout=300, stdout=subprocess.DEVNULL)
    runs.append(out)
a = runs[0]
same = all(filecmp.cmp(os.path.join(a, f), os.path.join(b, f), shallow=False) for b in runs[1:] for f in ("js.json", "js.f32"))
print(f"{name}: five X_ITE runs {'identical' if same else 'DIFFER'}")
if not same:
    sys.exit(1)
dest = os.path.join(root, "oracle", "b69-gs-collision-baseline", name)
shutil.rmtree(dest, ignore_errors=True)
os.makedirs(dest)
for f in ("js.json", "js.f32"):
    shutil.copy(os.path.join(a, f), dest)
