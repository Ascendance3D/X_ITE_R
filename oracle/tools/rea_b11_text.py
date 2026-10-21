#!/usr/bin/env python3
"""REA batch 11 lane b11-2-text: Text event cases (oracle/synthetic-rea-b11-text, rea_batch11_text.rs).

  python3 -I oracle/tools/rea_b11_text.py record NAME WORLD SCRIPT
      X_ITE 16.5.0 three times; the compared view (scene text at every dumped frame, poses, log, console, ok, warmupFrames) must be
      identical in all three, then the first js.json is kept as oracle/rea-batch11-baseline/NAME and the case line is added to
      oracle/rea-batch11-baseline/text-cases.txt. (`loadFrames` is the frame the load finished in; it follows Chrome's I/O and is not
      compared.) A baseline is never taken from Rust.
"""
import json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
mode, name, world, script = sys.argv[1:5]
assert mode == "record"
base = os.path.join(root, "oracle", "rea-batch11-baseline")
os.makedirs(base, exist_ok=True)
runs = []
for i in (1, 2, 3):
    out = f"/tmp/rea-b11-text-record/{name}-{i}"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world, script, "--out", os.path.join(out, "js.json")], check=True, cwd=root, timeout=300)
    runs.append(os.path.join(out, "js.json"))
keys = ("vrml", "poses", "log", "console", "ok", "warmupFrames")
view = lambda f: {k: v for k, v in json.load(open(f)).items() if k in keys}
same = all(view(runs[0]) == view(r) for r in runs[1:])
print(f"{name}: three X_ITE runs {'identical' if same else 'DIFFER'}")
if not same:
    sys.exit(1)
dest = os.path.join(base, name)
shutil.rmtree(dest, ignore_errors=True)
os.makedirs(dest)
shutil.copy(runs[0], dest)
cases = os.path.join(base, "text-cases.txt")
lines = open(cases).read().splitlines() if os.path.exists(cases) else []
lines = [l for l in lines if l.split()[:1] != [name]] + [" ".join([name, world, script])]
open(cases, "w").write("\n".join(lines) + "\n")
