#!/usr/bin/env python3
"""REA batch 09 lane scene-sai oracle baselines (pinned X_ITE in headless Chrome; no rendering is compared).

  python3 -I oracle/tools/rea_b09_sai.py case NAME WORLD SCRIPT
      oracle/tools/x10_compare.py --no-images against the release build in $B09_TARGET (default target-viewer/release);
      on success the X_ITE side (js.json) is kept as oracle/rea-batch09-sai-baseline/NAME and the case line
      `NAME WORLD SCRIPT` is added to cases.txt. WORLD and SCRIPT are paths relative to the repository root.
  python3 -I oracle/tools/rea_b09_sai.py xite NAME WORLD SCRIPT
      the X_ITE side only, into /tmp/rea-b09-sai-xite/NAME/js.json (run it again to see that it repeats).

The worlds (oracle/synthetic-rea-b09-sai) record what a Script saw in a field of its own, so the scene text carries it.
A baseline is written only from an X_ITE run; the Rust result is compared, never copied.
"""
import os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
mode, args = sys.argv[1], sys.argv[2:]
baseline = os.path.join(root, "oracle", "rea-batch09-sai-baseline")

if mode == "xite":
    name, world, script = args[:3]
    out = f"/tmp/rea-b09-sai-xite/{name}"
    shutil.rmtree(out, ignore_errors=True)
    os.makedirs(out)
    subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world, script, "--out", os.path.join(out, "js.json")], check=True, cwd=root, timeout=300)
elif mode == "case":
    name, world, script = args[:3]
    out = f"/tmp/rea-b09-sai-case/{name}"
    shutil.rmtree(out, ignore_errors=True)
    cmd = ["python3", os.path.join(root, "oracle", "tools", "x10_compare.py"), world, script, out, "--no-images",
           "--target", os.environ.get("B09_TARGET", os.path.join(root, "target-viewer", "release"))]
    r = subprocess.run(cmd, cwd=root, capture_output=True, text=True, timeout=600)
    print("\n".join(l for l in r.stdout.splitlines() if l.startswith("FAIL") or l.startswith("PASS both")))
    print(f"exit {r.returncode}")
    if r.returncode == 0:
        dest = os.path.join(baseline, name)
        shutil.rmtree(dest, ignore_errors=True)
        os.makedirs(dest)
        shutil.copy(os.path.join(out, "js.json"), dest)
        cases = os.path.join(baseline, "cases.txt")
        lines = open(cases).read().splitlines() if os.path.exists(cases) else []
        lines = [l for l in lines if l.split()[:1] != [name]] + [" ".join([name, world, script])]
        open(cases, "w").write("\n".join(lines) + "\n")
    sys.exit(r.returncode)
