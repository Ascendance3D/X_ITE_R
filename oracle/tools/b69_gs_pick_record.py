#!/usr/bin/env python3
"""GaussianSplats pointing baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU) for crates/x3d-viewer/tests/b69_gs_pick.rs.

  python3 -I -B oracle/tools/b69_gs_pick_record.py record NAME MSAA [X3D_ORACLE]

runs `x3d-oracle particles` (default target/debug/x3d-oracle) FIVE times on oracle/synthetic-b69-gs-pick/NAME.x3dv with
scripts/NAME.json; MSAA is 0 (the page's default canvas) or 4 (`--aa --ms 4`: the display is multisampled; the pointing buffer
is not, so the picks must not change). `--png-dir` is always passed so the page reshapes its drawing buffer to 512 x 512 (without it
`browser .touch` has a 1 x 1 viewport). The five js.json and js.f32 must be byte-identical; only then is the first kept as
oracle/b69-gs-pick-baseline/msaaMSAA/NAME/. A baseline is never taken from Rust. Staging is /tmp/b69-gs-pick-record.
"""
import filecmp, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
assert sys.argv[1] == "record"
name, msaa = sys.argv[2], sys.argv[3]
oracle = sys.argv[4] if len(sys.argv) > 4 else os.path.join(root, "target", "debug", "x3d-oracle")
# LOG=1 in the environment: LogarithmicDepthBuffer on (X3D_ORACLE_LOG_DEPTH=1 for x3d-oracle); baselines go to b69-gs-pick-log-baseline.
log = os.environ.get("LOG") == "1"
if log:
    os.environ["X3D_ORACLE_LOG_DEPTH"] = "1"
world = os.path.join(root, "oracle", "synthetic-b69-gs-pick", name + ".x3dv")
script = os.path.join(root, "oracle", "synthetic-b69-gs-pick", "scripts", name + ".json")
extra = ["--aa", "--ms", "4"] if msaa == "4" else []
runs = []
for i in range(1, 6):
    out = f"/tmp/b69-gs-pick-record/{name}-ms{msaa}{"-log" if log else ""}-{i}"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    subprocess.run([oracle, "particles", world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "png")] + extra,
                   check=True, cwd=root, timeout=300, stdout=subprocess.DEVNULL)
    runs.append(out)
a = runs[0]
same = all(filecmp.cmp(os.path.join(a, f), os.path.join(b, f), shallow=False) for b in runs[1:] for f in ("js.json", "js.f32"))
print(f"{name} msaa {msaa}: five X_ITE runs {'identical' if same else 'DIFFER'}")
if not same:
    sys.exit(1)
dest = os.path.join(root, "oracle", "b69-gs-pick-log-baseline" if log else "b69-gs-pick-baseline", f"msaa{msaa}", name)
shutil.rmtree(dest, ignore_errors=True)
os.makedirs(dest)
for f in ("js.json", "js.f32"):
    shutil.copy(os.path.join(a, f), dest)
