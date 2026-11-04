#!/usr/bin/env python3
"""Issue #95 ShaderPart lane baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU).

  python3 -I oracle/tools/rea_b24_shaderpart.py record NAME [RUNS] [--msaa 4]
      Runs oracle/x_ite-anim.mjs (MSAA 0) or oracle/x_ite-anim-msaa.mjs --ms N (MSAA N) RUNS times (default 5; the
      MSAA 4 baseline of issue #95 used 3) on oracle/synthetic-rea-b24-shaderpart/NAME.wrl with NAME.json. Only if every
      run gives the same js.json and the same PNG hashes is the first run kept and the case added to cases.txt;
      otherwise the case is reported UNSTABLE and nothing is written. A baseline is written only from X_ITE runs.
      MSAA 0: oracle/rea-b24-shaderpart-baseline/NAME-msaa0/{js.json,js/*.png}.
      MSAA N: oracle/rea-b24-shaderpart-baseline/NAME-msaaN.json (flat, so that the review gate leaves the record out)
      and the pictures in NAME-msaaN/js/*.png.
"""
import hashlib, json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
msaa = args[args.index("--msaa") + 1] if "--msaa" in args else "0"
if "--msaa" in args:
    del args[args.index("--msaa"):args.index("--msaa") + 2]
name = args[1]
runs = int(args[2]) if len(args) > 2 else 5
world = f"oracle/synthetic-rea-b24-shaderpart/{name}.wrl"
script = f"oracle/synthetic-rea-b24-shaderpart/{name}.json"
tmp = f"/tmp/lane95-rec/{name}-msaa{msaa}"
shutil.rmtree(tmp, ignore_errors=True)
sums = []
for i in range(runs):
    out = f"{tmp}/run{i}"
    os.makedirs(out)
    cmd = ["node", "oracle/x_ite-anim.mjs" if msaa == "0" else "oracle/x_ite-anim-msaa.mjs", world, script, "--out", f"{out}/js.json", "--png-dir", f"{out}/js"]
    if msaa != "0":
        cmd += ["--ms", msaa]
    subprocess.run(cmd, cwd=root, check=True, timeout=240, capture_output=True)
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
base = os.path.join(root, "oracle", "rea-b24-shaderpart-baseline")
dest = os.path.join(base, f"{name}-msaa{msaa}")
shutil.rmtree(dest, ignore_errors=True)
os.makedirs(dest)
if msaa == "0":
    shutil.copy(f"{tmp}/run0/js.json", dest)
else:
    shutil.copy(f"{tmp}/run0/js.json", f"{dest}.json")
shutil.copytree(f"{tmp}/run0/js", f"{dest}/js")
cases = os.path.join(root, "oracle", "rea-b24-shaderpart-baseline", "cases.txt")
lines = open(cases).read().splitlines() if os.path.exists(cases) else []
lines = [l for l in lines if l.split()[:1] != [name] or l.split()[3:4] != [msaa]] + [f"{name} {world} {script} {msaa}"]
open(cases, "w").write("\n".join(sorted(lines)) + "\n")
print(f"STABLE {name}: {runs} runs identical (sha256 of js.json {sums[0]['js.json']})")
