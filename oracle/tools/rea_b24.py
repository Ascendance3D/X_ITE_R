#!/usr/bin/env python3
"""REA batch 24 baselines (issue #94) (pinned X_ITE 16.5.0, headless Chrome on the GPU),.

  python3 -I -B oracle/tools/rea_b24.py record NAME WORLD SCRIPT MSAA
      X_ITE side only, run three times: the event cases need three identical recordings and the images two, so every case is
      recorded three times. All runs (js.json and every PNG) must be identical, then the first is kept as
      oracle/rea-batch24-anchor-collision-instanced-baseline/NAME-msaaMSAA and the case line is added to cases.txt. A baseline is never taken from Rust.
      (`loadFrames`, the frame the load finished in, follows Chrome's I/O; like rea_b12.py, it is left out of
      the identity check, and rea_b24_anchor_collision_instanced.rs does not compare it.)
"""
import filecmp, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
mode, name, world, script, msaa = sys.argv[1:6]
assert mode == "record"
cases_dir = os.path.join(root, "oracle", "rea-batch24-anchor-collision-instanced-baseline")
os.makedirs(cases_dir, exist_ok=True)
anim = "x_ite-anim.mjs" if msaa == "0" else "x_ite-anim-msaa.mjs"
runs = []
for i in (1, 2, 3):
    out = f"/tmp/lane94-record/{name}-msaa{msaa}-{i}"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    cmd = ["node", os.path.join(root, "oracle", anim), world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "js")]
    if msaa != "0":
        cmd += ["--ms", msaa]
    subprocess.run(cmd, check=True, cwd=root, timeout=300)
    runs.append(out)
a = runs[0]
pngs = sorted(os.listdir(os.path.join(a, "js")))
view = lambda d: {k: v for k, v in json.load(open(os.path.join(d, "js.json"))).items() if k != "loadFrames"}
same = all(pngs == sorted(os.listdir(os.path.join(b, "js"))) and view(a) == view(b)
           and all(filecmp.cmp(os.path.join(a, "js", f), os.path.join(b, "js", f), shallow=False) for f in pngs) for b in runs[1:])
print(f"{name} MSAA {msaa}: three X_ITE runs {'identical' if same else 'DIFFER'} ({len(pngs)} images)")
if not same:
    sys.exit(1)
dest = os.path.join(cases_dir, f"{name}-msaa{msaa}")
shutil.rmtree(dest, ignore_errors=True)
os.makedirs(dest)
shutil.copy(os.path.join(a, "js.json"), dest)
shutil.copytree(os.path.join(a, "js"), os.path.join(dest, "js"))
cases = os.path.join(cases_dir, "cases.txt")
line = " ".join([name, world, script, msaa])
lines = open(cases).read().splitlines() if os.path.exists(cases) else []
lines = [l for l in lines if l.split()[:1] != [name] or l.split()[3:4] != [msaa]] + [line]
open(cases, "w").write("\n".join(lines) + "\n")
