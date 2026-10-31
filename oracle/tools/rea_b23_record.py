#!/usr/bin/env python3
"""DepthMode pointing baseline (pinned X_ITE 16.5.0, headless Chrome on the GPU) for crates/x3d-viewer/tests/rea_b23_depthmode_pick.rs.

  python3 -I -B oracle/tools/rea_b23_record.py record [X3D_ORACLE]

runs `x3d-oracle particles` (default target/release/x3d-oracle) FIVE times on oracle/synthetic-rea-b23-depthmode-pick/dm-pick.x3dv
with scripts/dm-pick.json. `--png-dir` is always passed so the page reshapes its drawing buffer to 512 x 512 (without it
`browser .touch` has a 1 x 1 viewport). The five js.json and js.f32 must be byte-identical; only then is the first kept as
oracle/rea-batch23-depthmode-pick-baseline/dm-pick/. The WebGL renderer string is kept beside it as renderer.json
(oracle/tools/webgl_renderer_probe.mjs, needs $XITE_TESTS). A baseline is never taken from Rust. Staging is /tmp/rea-b23-record.
"""
import filecmp, hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
assert sys.argv[1] == "record"
oracle = sys.argv[2] if len(sys.argv) > 2 else os.path.join(root, "target", "release", "x3d-oracle")
world = os.path.join(root, "oracle", "synthetic-rea-b23-depthmode-pick", "dm-pick.x3dv")
script = os.path.join(root, "oracle", "synthetic-rea-b23-depthmode-pick", "scripts", "dm-pick.json")
stage = "/tmp/rea-b23-record"
shutil.rmtree(stage, ignore_errors=True)
runs = []
for i in range(1, 6):
    out = f"{stage}/run{i}"
    os.makedirs(out)
    subprocess.run([oracle, "particles", world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "png")],
                   check=True, cwd=root, timeout=300, stdout=subprocess.DEVNULL)
    runs.append(out)
a = runs[0]
same = all(filecmp.cmp(os.path.join(a, f), os.path.join(b, f), shallow=False) for b in runs[1:] for f in ("js.json", "js.f32"))
sha = hashlib.sha256(open(os.path.join(a, "js.json"), "rb").read()).hexdigest()
print(f"dm-pick: five X_ITE runs {'identical' if same else 'DIFFER'}, js.json sha256 {sha}")
if not same:
    sys.exit(1)
probe = subprocess.run(["node", os.path.join(root, "oracle", "tools", "webgl_renderer_probe.mjs")], check=True, capture_output=True, text=True, timeout=60).stdout
dest = os.path.join(root, "oracle", "rea-batch23-depthmode-pick-baseline", "dm-pick")
shutil.rmtree(dest, ignore_errors=True)
os.makedirs(dest)
for f in ("js.json", "js.f32"):
    shutil.copy(os.path.join(a, f), dest)
open(os.path.join(dest, "renderer.json"), "w").write(json.dumps(json.loads(probe), indent=1) + "\n")
