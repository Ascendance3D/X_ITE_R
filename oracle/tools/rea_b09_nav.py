#!/usr/bin/env python3
"""REA batch 09 navigation and pointer-picking oracle baselines (pinned X_ITE, headless Chrome on the GPU).

  python3 -I oracle/tools/rea_b09_nav.py list
      print the case table (name, world, script, msaa, text_tol).
  python3 -I oracle/tools/rea_b09_nav.py run [--only SUBSTRING] [--jobs N] [--target DIR]
      for every case: x10_compare.py against the release build in DIR (default target-viewer/release);
      on success the X_ITE side (js.json, js/*.png) is kept as
      oracle/rea-batch09-nav-baseline/NAME-msaaM and the case line is written to cases.txt.

A baseline is written only from an X_ITE run; the Rust result is compared, never copied. Navigation
poses and scene text do not depend on the multisampling level; the pictures of IMAGE_CASES are
compared at MSAA 0 and 4 (the other cases run without pictures).
"""
import json, os, re, shutil, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
nav = "oracle/synthetic-rea-b09-nav"
x10 = "oracle/x10-scripts"

# Cases whose captured frames are compared as pictures too, at MSAA 0 and 4.
IMAGE_CASES = {
    "walk-forward", "fly-turn-right", "walk-fall", "walk-steps", "examine-spin", "examine-wheel", "trans-linear-walk",
    "lookat-b09-drag", "plane-b09-pan", "viewer-switch", "touch-hit-events", "sensors", "examine-dblclick-alt", "lookat-click-alt", "examine-instanced-dblclick",
}


def cases():
    out = []  # (name, world, script, text_tol)
    for line in open(os.path.join(root, nav, "cases.txt")):
        if line.strip():
            n, w, s = line.split()
            out.append((n, w, s, None))
    # the pointer worlds
    out.append(("touch-hit-events", f"{nav}/touch-hit-events.wrl", f"{nav}/scripts/touch-hit-events.json", None))
    out.append(("geo-touch", "oracle/synthetic-rea/geo-touch.wrl", "oracle/synthetic-rea/scripts/geo-touch.json", None))
    out.append(("sensors", f"{x10}/sensors.wrl", f"{x10}/sensors.json", None))
    out.append(("pick-hits", "oracle/synthetic-rea/pick-hits.wrl", "oracle/synthetic-rea/scripts/pick-hits.json", None))
    out.append(("pick-instanced", "oracle/synthetic-rea/pick-instanced.wrl", "oracle/synthetic-rea/scripts/pick-instanced.json", None))
    # the earlier x10 viewer scripts on their worlds
    for f in sorted(os.listdir(os.path.join(root, x10))):
        if not f.endswith(".json"):
            continue
        n = f[:-5]
        world = None
        if n.startswith("plane-ortho"):
            world = "plane-ortho"
        elif n.startswith("plane-"):
            world = "plane-nav"
        elif n.startswith("lookat-ortho"):
            world = "lookat-ortho"
        elif n.startswith("lookat-"):
            world = "lookat-nav"
        elif n.startswith("examine-dblclick"):
            world = "examine-nav"
        if world:
            out.append((f"x10-{n}", f"{x10}/{world}.wrl", f"{x10}/{f}", None))
    return out


def run_case(case, msaa, target, images):
    name, world, script, tol = case
    out = f"/tmp/rea-b09-case/{name}-msaa{msaa}"
    shutil.rmtree(out, ignore_errors=True)
    cmd = ["python3", "-I", os.path.join(root, "oracle", "tools", "x10_compare.py"), world, script, out, "--target", target]
    if not images:
        cmd.append("--no-images")
    if msaa != "0":
        cmd += ["--msaa", msaa]
    if tol:
        cmd += ["--text-tol", tol]
    r = subprocess.run(cmd, cwd=root, capture_output=True, text=True, timeout=900)
    text = r.stdout + r.stderr
    fails = [l for l in text.splitlines() if l.startswith("FAIL")]
    worst = max([float(x) for t in re.findall(r"\|dpos\| (\S+) \|dcenter\| (\S+) dangle (\S+)", text) for x in t] or [0.0])
    return name, msaa, r.returncode == 0, fails, worst, out, text


def main():
    args = sys.argv[1:]
    opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
    target = opt("--target", os.path.join(root, "target-viewer", "release"))
    only = opt("--only")
    jobs = int(opt("--jobs", "6"))
    table = [c for c in cases() if not only or only in c[0]]
    if args[0] == "list":
        for c in table:
            print(" ".join(str(x) for x in c if x))
        return 0
    work = []
    for c in table:
        if c[0] in IMAGE_CASES:
            work += [(c, "0", True), (c, "4", True)]
        else:
            work.append((c, "0", False))
    with ThreadPoolExecutor(jobs) as ex:
        results = list(ex.map(lambda w: (w, run_case(w[0], w[1], target, w[2])), work))
    bdir = os.path.join(root, "oracle", "rea-batch09-nav-baseline")
    os.makedirs(bdir, exist_ok=True)
    lines = {}
    cpath = os.path.join(bdir, "cases.txt")
    if os.path.exists(cpath):
        for l in open(cpath).read().splitlines():
            f = l.split()
            lines[(f[0], f[3])] = l
    bad = 0
    for (c, msaa, images), (name, _, ok, fails, worst, out, text) in results:
        print(("PASS " if ok else "FAIL ") + f"{name} msaa{msaa}{' images' if images else ''} max pose delta {worst:.2e}")
        if not ok:
            bad += 1
            print("\n".join("    " + l for l in fails[:6]))
            continue
        dest = os.path.join(bdir, f"{name}-msaa{msaa}")
        shutil.rmtree(dest, ignore_errors=True)
        os.makedirs(dest)
        shutil.copy(os.path.join(out, "js.json"), dest)
        if images and os.path.isdir(os.path.join(out, "js")):
            shutil.copytree(os.path.join(out, "js"), os.path.join(dest, "js"))
        lines[(name, msaa)] = " ".join([name, c[1], c[2], msaa] + ([c[3]] if c[3] else []))
    open(cpath, "w").write("\n".join(lines[k] for k in sorted(lines)) + "\n")
    print(f"{len(results) - bad} of {len(results)} runs pass")
    return 1 if bad else 0


sys.exit(main())
