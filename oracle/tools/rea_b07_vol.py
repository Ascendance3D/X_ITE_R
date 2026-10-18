#!/usr/bin/env python3
"""REA batch 07 VolumeRendering oracle baselines (pinned X_ITE, headless Chrome on the GPU).

  python3 -I oracle/tools/rea_b07_vol.py shots [--only SUBSTRING]
      oracle/synthetic-rea-b07-vol/static/*.wrl -> oracle/render-rea-b07-vol-baseline (x_ite-shot.mjs) and
      oracle/render-msaa4-rea-b07-vol-baseline (x_ite-shot-msaa.mjs, 4 samples).
  python3 -I oracle/tools/rea_b07_vol.py case NAME WORLD SCRIPT MSAA [TEXT_TOL] [--target DIR]
      x10_compare.py against the release build in DIR (default /tmp/b07-vol-target/release); on success the
      X_ITE side (js.json, js/*.png) is kept as oracle/rea-batch07-vol-baseline/NAME-msaaMSAA and the case line
      is added to oracle/rea-batch07-vol-baseline/cases.txt.

The worlds come from oracle/tools/gen_rea_b07_vol.py. A baseline is written only from an X_ITE run; the Rust
result is compared, never copied.
"""
import os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]

if args[0] == "shots":
    only = args[args.index("--only") + 1] if "--only" in args else None
    for tool, out in [("x_ite-shot.mjs", "render-rea-b07-vol-baseline"), ("x_ite-shot-msaa.mjs", "render-msaa4-rea-b07-vol-baseline")]:
        cmd = ["node", os.path.join(root, "oracle", tool), os.path.join(root, "oracle", "synthetic-rea-b07-vol", "static"), os.path.join(root, "oracle", out)]
        if only:
            cmd += ["--only", only]
        subprocess.run(cmd, check=True, cwd=root, timeout=1800)
elif args[0] == "case":
    target = args[args.index("--target") + 1] if "--target" in args else "/tmp/b07-vol-target/release"
    pos = [a for i, a in enumerate(args[1:], 1) if not a.startswith("--") and args[i - 1] != "--target"]
    name, world, script, msaa = pos[:4]
    tol = pos[4] if len(pos) > 4 else None
    out = f"/tmp/rea-b07-vol-case/{name}-msaa{msaa}"
    shutil.rmtree(out, ignore_errors=True)
    cmd = ["python3", os.path.join(root, "oracle", "tools", "x10_compare.py"), world, script, out, "--target", target]
    if msaa != "0":
        cmd += ["--msaa", msaa]
    if tol:
        cmd += ["--text-tol", tol]
    r = subprocess.run(cmd, cwd=root, capture_output=True, text=True, timeout=900)
    print("\n".join(l for l in r.stdout.splitlines() if l.startswith("FAIL") or "within tolerance, " in l or l.startswith("PASS both")))
    print(f"exit {r.returncode}")
    if r.returncode == 0:
        # A baseline is only worth keeping when X_ITE ran the script without errors and the captured
        # frames are not all the same picture (a Script that fails in both browsers would pass).
        import json, hashlib
        js = json.load(open(os.path.join(out, "js.json")))
        if js.get("log"):
            print("REJECTED: X_ITE logged", js["log"][:4])
            sys.exit(3)
        pngs = sorted(os.listdir(os.path.join(out, "js"))) if os.path.isdir(os.path.join(out, "js")) else []
        hashes = {hashlib.sha256(open(os.path.join(out, "js", f), "rb").read()).hexdigest() for f in pngs}
        if len(pngs) > 1 and len(hashes) < min(len(pngs), 3):
            print("REJECTED: the captured X_ITE frames are nearly all the same picture", len(hashes), "of", len(pngs))
            sys.exit(4)
        dest = os.path.join(root, "oracle", "rea-batch07-vol-baseline", f"{name}-msaa{msaa}")
        shutil.rmtree(dest, ignore_errors=True)
        os.makedirs(dest)
        shutil.copy(os.path.join(out, "js.json"), dest)
        if os.path.isdir(os.path.join(out, "js")):
            shutil.copytree(os.path.join(out, "js"), os.path.join(dest, "js"))
        cases = os.path.join(root, "oracle", "rea-batch07-vol-baseline", "cases.txt")
        line = " ".join([name, world, script, msaa] + ([tol] if tol else []))
        lines = open(cases).read().splitlines() if os.path.exists(cases) else []
        lines = [l for l in lines if l.split()[:1] != [name] or l.split()[3:4] != [msaa]] + [line]
        open(cases, "w").write("\n".join(lines) + "\n")
    sys.exit(r.returncode)
