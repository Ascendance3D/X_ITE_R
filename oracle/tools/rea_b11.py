#!/usr/bin/env python3
"""REA batch 11 lane b11-1-bbox baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU).

  python3 -I oracle/tools/rea_b11.py record NAME WORLD SCRIPT MSAA
      X_ITE side only, run twice; the two runs (js.json and every PNG) must be identical, then the first is kept as
      oracle/rea-batch11-baseline/NAME-msaaMSAA and the case line is added to cases.txt. A baseline is never taken from Rust.
"""
import filecmp, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
mode, name, world, script, msaa = sys.argv[1:6]
assert mode == "record"
cases_dir = os.path.join(root, "oracle", "rea-batch11-baseline")
os.makedirs(cases_dir, exist_ok=True)
anim = "x_ite-anim.mjs" if msaa == "0" else "x_ite-anim-msaa.mjs"
runs = []
for i in (1, 2):
    out = f"/tmp/rea-b11-record/{name}-msaa{msaa}-{i}"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    cmd = ["node", os.path.join(root, "oracle", anim), world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "js")]
    if msaa != "0":
        cmd += ["--ms", msaa]
    subprocess.run(cmd, check=True, cwd=root, timeout=300)
    runs.append(out)
a, b = runs
pngs = sorted(os.listdir(os.path.join(a, "js")))
same = pngs == sorted(os.listdir(os.path.join(b, "js"))) and filecmp.cmp(os.path.join(a, "js.json"), os.path.join(b, "js.json"), shallow=False) \
    and all(filecmp.cmp(os.path.join(a, "js", f), os.path.join(b, "js", f), shallow=False) for f in pngs)
print(f"{name} MSAA {msaa}: two X_ITE runs {'identical' if same else 'DIFFER'} ({len(pngs)} images)")
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
