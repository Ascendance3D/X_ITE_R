#!/usr/bin/env python3
"""REA batch 08 lane render2 oracle baselines (pinned X_ITE, headless Chrome on the GPU).

SUITE is `gs` (GaussianSplats) or `vs` (VolumeScatterMaterialExtension).

  python3 -I oracle/tools/rea_b08_render2.py SUITE shots [--only SUBSTRING]
      oracle/synthetic-rea-b08-SUITE/*.wrl -> oracle/render-rea-b08-SUITE-baseline (x_ite-shot.mjs) and
      oracle/render-msaa4-rea-b08-SUITE-baseline (x_ite-shot-msaa.mjs, 4 samples).
  python3 -I oracle/tools/rea_b08_render2.py SUITE case NAME WORLD SCRIPT MSAA [TEXT_TOL]
      x10_compare.py against the release build in $B08_TARGET (default /tmp/b08/render2/target/release);
      on success the X_ITE side (js.json, js/*.png) is kept as
      oracle/rea-batch08-SUITE-baseline/NAME-msaaMSAA and the case line is added to cases.txt.
  python3 -I oracle/tools/rea_b08_render2.py SUITE xite-case NAME WORLD SCRIPT MSAA
      the X_ITE side only (x_ite-anim.mjs / x_ite-anim-msaa.mjs), into /tmp/rea-b08-SUITE-xite/NAME-msaaMSAA
      (to check that a stepped X_ITE run repeats).

A baseline is written only from an X_ITE run; the Rust result is compared, never copied.
"""
import os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
suite, mode, args = args[0], args[1], args[2:]
assert suite in ("gs", "vs")
corpus = f"synthetic-rea-b08-{suite}"
shots = [("x_ite-shot.mjs", f"render-rea-b08-{suite}-baseline"), ("x_ite-shot-msaa.mjs", f"render-msaa4-rea-b08-{suite}-baseline")]
cases_dir = os.path.join(root, "oracle", f"rea-batch08-{suite}-baseline")

if mode == "shots":
    only = args[args.index("--only") + 1] if "--only" in args else None
    for tool, out in shots:
        cmd = ["node", os.path.join(root, "oracle", tool), os.path.join(root, "oracle", corpus), os.path.join(root, "oracle", out)]
        if only:
            cmd += ["--only", only]
        subprocess.run(cmd, check=True, cwd=root, timeout=1800)
elif mode == "xite-case":
    name, world, script, msaa = args[:4]
    out = f"/tmp/rea-b08-{suite}-xite/{name}-msaa{msaa}"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    anim = "x_ite-anim.mjs" if msaa == "0" else "x_ite-anim-msaa.mjs"
    cmd = ["node", os.path.join(root, "oracle", anim), world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "js")]
    if msaa != "0":
        cmd += ["--ms", msaa]
    subprocess.run(cmd, check=True, cwd=root, timeout=300)
elif mode == "case":
    name, world, script, msaa = args[:4]
    tol = args[4] if len(args) > 4 else None
    out = f"/tmp/rea-b08-{suite}-case/{name}-msaa{msaa}"
    shutil.rmtree(out, ignore_errors=True)
    cmd = ["python3", os.path.join(root, "oracle", "tools", "x10_compare.py"), world, script, out, "--target", os.environ.get("B08_TARGET", "/tmp/b08/render2/target/release")]
    if msaa != "0":
        cmd += ["--msaa", msaa]
    if tol:
        cmd += ["--text-tol", tol]
    r = subprocess.run(cmd, cwd=root, capture_output=True, text=True, timeout=600)
    print("\n".join(l for l in r.stdout.splitlines() if l.startswith("FAIL") or "within tolerance, " in l or l.startswith("PASS both")))
    print(f"exit {r.returncode}")
    if r.returncode == 0:
        dest = os.path.join(cases_dir, f"{name}-msaa{msaa}")
        shutil.rmtree(dest, ignore_errors=True)
        os.makedirs(dest)
        shutil.copy(os.path.join(out, "js.json"), dest)
        if os.path.isdir(os.path.join(out, "js")):
            shutil.copytree(os.path.join(out, "js"), os.path.join(dest, "js"))
        cases = os.path.join(cases_dir, "cases.txt")
        line = " ".join([name, world, script, msaa] + ([tol] if tol else []))
        lines = open(cases).read().splitlines() if os.path.exists(cases) else []
        lines = [l for l in lines if l.split()[:1] != [name] or l.split()[3:4] != [msaa]] + [line]
        open(cases, "w").write("\n".join(lines) + "\n")
    sys.exit(r.returncode)
