#!/usr/bin/env python3
"""REA batch 08 shaders lane (ComposedShader, ShaderPart) oracle baselines (pinned X_ITE, headless Chrome on the GPU).

  python3 -I oracle/tools/rea_b08_shaders.py shots [--only SUBSTRING]
      oracle/synthetic-rea-b08-shaders/*.wrl -> oracle/render-rea-b08-shaders-baseline (x_ite-shot.mjs) and
      oracle/render-msaa4-rea-b08-shaders-baseline (x_ite-shot-msaa.mjs, 4 samples).
  python3 -I oracle/tools/rea_b08_shaders.py case NAME WORLD SCRIPT MSAA [TEXT_TOL]
      x10_compare.py against the release build in $B08_SHADERS_TARGET (default /tmp/b08-shaders-target/release); on success the X_ITE
      side (js.json, js/*.png) is kept as oracle/rea-batch08-shaders-baseline/NAME-msaaMSAA and the case
      line is added to oracle/rea-batch08-shaders-baseline/cases.txt.

A baseline is written only from an X_ITE run; the Rust result is compared, never copied.
Still images of worlds in STEPPED_ONLY are removed after `shots`: their pictures depend on how many
real-time frames the shot page drew (mirrors that see each other), or on a rendering property a
Script sets (x3d-shot does not apply those), so only `case` compares them.
"""
import os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
STEPPED_ONLY = []  # worlds whose pictures depend on real-time frames

if args[0] == "shots":
    only = args[args.index("--only") + 1] if "--only" in args else None
    for tool, out in [("x_ite-shot.mjs", "render-rea-b08-shaders-baseline"), ("x_ite-shot-msaa.mjs", "render-msaa4-rea-b08-shaders-baseline")]:
        cmd = ["node", os.path.join(root, "oracle", tool), os.path.join(root, "oracle", "synthetic-rea-b08-shaders"), os.path.join(root, "oracle", out)]
        if only:
            cmd += ["--only", only]
        subprocess.run(cmd, check=True, cwd=root, timeout=900)
        for w in STEPPED_ONLY:
            p = os.path.join(root, "oracle", out, w + ".png")
            if os.path.exists(p):
                os.remove(p)
elif args[0] == "case":
    name, world, script, msaa = args[1:5]
    tol = args[5] if len(args) > 5 else None
    out = f"/tmp/b08/shaders/case/{name}-msaa{msaa}"
    shutil.rmtree(out, ignore_errors=True)
    cmd = ["python3", os.path.join(root, "oracle", "tools", "x10_compare.py"), world, script, out, "--target", os.environ.get("B08_SHADERS_TARGET", "/tmp/b08-shaders-target/release")]
    if msaa != "0":
        cmd += ["--msaa", msaa]
    if tol:
        cmd += ["--text-tol", tol]
    r = subprocess.run(cmd, cwd=root, capture_output=True, text=True, timeout=600)
    print("\n".join(l for l in r.stdout.splitlines() if l.startswith("FAIL") or "within tolerance, " in l or l.startswith("PASS both")))
    print(f"exit {r.returncode}")
    if r.returncode == 0:
        dest = os.path.join(root, "oracle", "rea-batch08-shaders-baseline", f"{name}-msaa{msaa}")
        shutil.rmtree(dest, ignore_errors=True)
        os.makedirs(dest)
        shutil.copy(os.path.join(out, "js.json"), dest)
        if os.path.isdir(os.path.join(out, "js")):
            shutil.copytree(os.path.join(out, "js"), os.path.join(dest, "js"))
        cases = os.path.join(root, "oracle", "rea-batch08-shaders-baseline", "cases.txt")
        line = " ".join([name, world, script, msaa] + ([tol] if tol else []))
        lines = open(cases).read().splitlines() if os.path.exists(cases) else []
        lines = [l for l in lines if l.split()[:1] != [name] or l.split()[3:4] != [msaa]] + [line]
        open(cases, "w").write("\n".join(lines) + "\n")
    sys.exit(r.returncode)
