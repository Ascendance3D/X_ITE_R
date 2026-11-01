#!/usr/bin/env python3
"""Issue #95 ShaderPart lane baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU).

  python3 -I oracle/tools/rea_b24_shaderpart.py record NAME [RUNS]
      Runs oracle/x_ite-anim.mjs RUNS times (default 5) on oracle/synthetic-rea-b24-shaderpart/NAME.wrl with
      NAME.json. Only if every run gives the same js.json and the same PNG hashes is the first run kept as
      oracle/rea-b24-shaderpart-baseline/NAME-msaa0 and the case added to cases.txt; otherwise the case is
      reported UNSTABLE and nothing is written. A baseline is written only from X_ITE runs.
"""
import hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
name = sys.argv[2]
runs = int(sys.argv[3]) if len(sys.argv) > 3 else 5
world = f"oracle/synthetic-rea-b24-shaderpart/{name}.wrl"
script = f"oracle/synthetic-rea-b24-shaderpart/{name}.json"
tmp = f"/tmp/lane95-rec/{name}"
shutil.rmtree(tmp, ignore_errors=True)
sums = []
for i in range(runs):
    out = f"{tmp}/run{i}"
    os.makedirs(out)
    subprocess.run(["node", "oracle/x_ite-anim.mjs", world, script, "--out", f"{out}/js.json", "--png-dir", f"{out}/js"], cwd=root, check=True, timeout=240, capture_output=True)
    h = {"js.json": hashlib.sha256(open(f"{out}/js.json", "rb").read()).hexdigest()}
    for p in sorted(os.listdir(f"{out}/js")):
        h[p] = hashlib.sha256(open(f"{out}/js/{p}", "rb").read()).hexdigest()
    sums.append(h)
    print(f"run {i}: json {h['js.json'][:12]} {len(h) - 1} pictures")
if any(s != sums[0] for s in sums):
    for k in sums[0]:
        if len({s.get(k) for s in sums}) > 1:
            print("DIFFERS:", k)
    print(f"UNSTABLE {name}")
    sys.exit(2)
dest = os.path.join(root, "oracle", "rea-b24-shaderpart-baseline", f"{name}-msaa0")
shutil.rmtree(dest, ignore_errors=True)
os.makedirs(dest)
shutil.copy(f"{tmp}/run0/js.json", dest)
shutil.copytree(f"{tmp}/run0/js", f"{dest}/js")
cases = os.path.join(root, "oracle", "rea-b24-shaderpart-baseline", "cases.txt")
lines = open(cases).read().splitlines() if os.path.exists(cases) else []
lines = [l for l in lines if l.split()[:1] != [name]] + [f"{name} {world} {script} 0"]
open(cases, "w").write("\n".join(sorted(lines)) + "\n")
print(f"STABLE {name}: {runs} runs identical (sha256 of js.json {sums[0]['js.json']})")
