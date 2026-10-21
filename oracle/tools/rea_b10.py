#!/usr/bin/env python3
"""REA batch 10 lane Text / ScreenGroup / ScreenFontStyle stepped oracle baselines (pinned X_ITE, headless Chrome on the GPU).

SUITE is `stepped` (oracle/synthetic-rea-b10-stepped, made by gen_rea_b10_screen.py).

  python3 -I oracle/tools/rea_b10.py SUITE case NAME WORLD SCRIPT MSAA [TEXT_TOL]
      x10_compare.py against the release build in $B09_TARGET (default /tmp/b10/target/release);
      on success the X_ITE side (js.json, js/*.png) is kept as
      oracle/rea-batch10-baseline/NAME-msaaMSAA and the case line is added to cases.txt.
  python3 -I oracle/tools/rea_b10.py SUITE xite-case NAME WORLD SCRIPT MSAA
      the X_ITE side only (x_ite-anim.mjs / x_ite-anim-msaa.mjs), into /tmp/rea-b10-SUITE-xite/NAME-msaaMSAA
      (to check that a stepped X_ITE run repeats).

  python3 -I oracle/tools/rea_b10.py stepped record-sai NAME WORLD SCRIPT
      event cases (oracle/synthetic-rea-b10-sai, rea_batch10_sai.rs): X_ITE three times, identical js.json, kept in
      oracle/rea-batch10-sai-baseline/NAME.
  python3 -I oracle/tools/rea_b10.py stepped record NAME WORLD SCRIPT MSAA
      X_ITE side only, run twice; the two runs (js.json and every PNG) must be identical, then the first is kept as
      oracle/rea-batch10-baseline/NAME-msaaMSAA and the case line is added to cases.txt (so a Rust defect can be
      recorded before it is fixed; the baseline is never taken from Rust).

A baseline is written only from an X_ITE run; the Rust result is compared, never copied.
"""
import os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
suite, mode, args = args[0], args[1], args[2:]
assert suite in ("stepped",)
corpus = f"synthetic-rea-b10-{suite}"
shots = []
cases_dir = os.path.join(root, "oracle", f"rea-batch10-baseline")

if mode == "shots":
    only = args[args.index("--only") + 1] if "--only" in args else None
    for tool, out in shots:
        cmd = ["node", os.path.join(root, "oracle", tool), os.path.join(root, "oracle", corpus), os.path.join(root, "oracle", out)]
        if only:
            cmd += ["--only", only]
        subprocess.run(cmd, check=True, cwd=root, timeout=1800)
elif mode == "xite-case":
    name, world, script, msaa = args[:4]
    out = f"/tmp/rea-b10-{suite}-xite/{name}-msaa{msaa}"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    anim = "x_ite-anim.mjs" if msaa == "0" else "x_ite-anim-msaa.mjs"
    cmd = ["node", os.path.join(root, "oracle", anim), world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "js")]
    if msaa != "0":
        cmd += ["--ms", msaa]
    subprocess.run(cmd, check=True, cwd=root, timeout=300)
elif mode == "record-sai":
    # Event cases (no images): X_ITE three times, every js.json identical, kept in oracle/rea-batch10-sai-baseline/NAME.
    name, world, script = args[:3]
    runs = []
    for i in (1, 2, 3):
        out = f"/tmp/rea-b10-sai-record/{name}-{i}"
        shutil.rmtree(out, ignore_errors=True)
        os.makedirs(out)
        subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world, script, "--out", os.path.join(out, "js.json")], check=True, cwd=root, timeout=300)
        runs.append(os.path.join(out, "js.json"))
    import json
    # What the test compares: scene text at every dumped frame, poses, log, console. (`loadFrames`, the frame a load finished in, is
    # load-time metadata that varies with Chrome's I/O and is not compared.)
    keys = ("vrml", "poses", "log", "console", "ok", "warmupFrames")
    view = lambda f: {k: v for k, v in json.load(open(f)).items() if k in keys}
    same = all(view(runs[0]) == view(r) for r in runs[1:])
    print(f"{name}: three X_ITE runs {'identical' if same else 'DIFFER'}")
    if not same:
        sys.exit(1)
    dest = os.path.join(root, "oracle", "rea-batch10-sai-baseline", name)
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest)
    shutil.copy(runs[0], dest)
    cases = os.path.join(root, "oracle", "rea-batch10-sai-baseline", "cases.txt")
    lines = open(cases).read().splitlines() if os.path.exists(cases) else []
    lines = [l for l in lines if l.split()[:1] != [name]] + [" ".join([name, world, script])]
    open(cases, "w").write("\n".join(lines) + "\n")
elif mode == "record":
    import filecmp
    name, world, script, msaa = args[:4]
    anim = "x_ite-anim.mjs" if msaa == "0" else "x_ite-anim-msaa.mjs"
    runs = []
    for i in (1, 2):
        out = f"/tmp/rea-b10-{suite}-record/{name}-msaa{msaa}-{i}"
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
elif mode == "case":
    name, world, script, msaa = args[:4]
    tol = args[4] if len(args) > 4 else None
    out = f"/tmp/rea-b10-{suite}-case/{name}-msaa{msaa}"
    shutil.rmtree(out, ignore_errors=True)
    cmd = ["python3", os.path.join(root, "oracle", "tools", "x10_compare.py"), world, script, out, "--target", os.environ.get("B09_TARGET", "/tmp/b10/target/release")]
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
