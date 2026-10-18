#!/usr/bin/env python3
"""REA batch 08 (glTF lane) oracle baselines (pinned X_ITE, headless Chrome on the GPU).

  python3 -I oracle/tools/rea_b08_gltf.py parse
      oracle/synthetic-rea-b08-gltf/**/*.gltf|glb -> oracle/rea-batch08-gltf-baseline/gltf-baseline.json (x_ite-gltf.mjs)
  python3 -I oracle/tools/rea_b08_gltf.py shots
      oracle/synthetic-rea-b08-gltf/render/*.gltf -> oracle/render-rea-b08-gltf-baseline (x_ite-shot-gltf.mjs) and
      oracle/render-msaa4-rea-b08-gltf-baseline (x_ite-shot-msaa-gltf.mjs, 4 samples).
  python3 -I oracle/tools/rea_b08_gltf.py case NAME WORLD SCRIPT MSAA [TEXT_TOL]
      x10_compare.py against the release build in $B08_GLTF_TARGET (default /tmp/b08/gltf/target/release); on success the
      X_ITE side (js.json, js/*.png) is kept as oracle/rea-batch08-gltf-baseline/NAME-msaaMSAA and the case line is
      added to oracle/rea-batch08-gltf-baseline/cases.txt.

A baseline is written only from an X_ITE run; the Rust result is compared, never copied.
"""
import os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
BASE = "rea-batch08-gltf-baseline"

if args[0] == "parse":
    subprocess.run(["node", "x_ite-gltf.mjs", "synthetic-rea-b08-gltf", f"{BASE}/gltf-baseline.json"], check=True, cwd=os.path.join(root, "oracle"), timeout=900)
elif args[0] == "shots":
    for tool, out in [("x_ite-shot-gltf.mjs", "render-rea-b08-gltf-baseline"), ("x_ite-shot-msaa-gltf.mjs", "render-msaa4-rea-b08-gltf-baseline")]:
        subprocess.run(["node", tool, "synthetic-rea-b08-gltf", out, "--only", "render/", "--jobs", "4"], check=True, cwd=os.path.join(root, "oracle"), timeout=900)
elif args[0] == "case":
    name, world, script, msaa = args[1:5]
    tol = args[5] if len(args) > 5 else None
    out = f"/tmp/rea-b08-gltf-case/{name}-msaa{msaa}"
    shutil.rmtree(out, ignore_errors=True)
    cmd = ["python3", os.path.join(root, "oracle", "tools", "x10_compare.py"), world, script, out, "--target", os.environ.get("B08_GLTF_TARGET", "/tmp/b08/gltf/target/release")]
    if msaa != "0":
        cmd += ["--msaa", msaa]
    if tol:
        cmd += ["--text-tol", tol]
    r = subprocess.run(cmd, cwd=root, capture_output=True, text=True, timeout=600)
    print("\n".join(l for l in r.stdout.splitlines() if l.startswith("FAIL") or "within tolerance, " in l or l.startswith("PASS both")))
    print(f"exit {r.returncode}")
    if r.returncode == 0:
        dest = os.path.join(root, "oracle", BASE, f"{name}-msaa{msaa}")
        shutil.rmtree(dest, ignore_errors=True)
        os.makedirs(dest)
        shutil.copy(os.path.join(out, "js.json"), dest)
        if os.path.isdir(os.path.join(out, "js")):
            shutil.copytree(os.path.join(out, "js"), os.path.join(dest, "js"))
        cases = os.path.join(root, "oracle", BASE, "cases.txt")
        line = " ".join([name, world, script, msaa] + ([tol] if tol else []))
        lines = open(cases).read().splitlines() if os.path.exists(cases) else []
        lines = [l for l in lines if l.split()[:1] != [name] or l.split()[3:4] != [msaa]] + [line]
        open(cases, "w").write("\n".join(lines) + "\n")
    sys.exit(r.returncode)
