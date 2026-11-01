#!/usr/bin/env python3
"""REA batch 24 navigation baselines (issue #91) (pinned X_ITE 16.5.0, headless Chrome on the GPU).

  python3 -I -B oracle/tools/rea_b24_nav.py record [--only SUBSTRING] [--jobs N]
      For every case of oracle/synthetic-rea-b24-nav/cases.txt (made by gen_rea_b24_nav.py): record the X_ITE side three
      times (x_ite-anim.mjs). The three js.json must be identical (`loadFrames`, which follows Chrome's I/O, is left out of the
      check; rea_b24_nav.rs does not compare it). The first is kept as oracle/rea-batch24-nav-baseline/NAME-msaa0 and the case
      line is written to cases.txt there. A case whose recordings differ is listed in UNSTABLE.txt with the differing keys and
      is not kept. A baseline is never taken from Rust. No pictures: the navigation result is the camera pose.
"""
import json, os, shutil, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
assert args and args[0] == "record"
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
only, jobs = opt("--only"), int(opt("--jobs", "6"))
src = os.path.join(root, "oracle", "synthetic-rea-b24-nav", "cases.txt")
dest_dir = os.path.join(root, "oracle", "rea-batch24-nav-baseline")
os.makedirs(dest_dir, exist_ok=True)
table = [l.split() for l in open(src).read().splitlines() if l.strip()]
table = [c for c in table if not only or only in c[0]]


def run(case):
    name, world, script = case
    outs = []
    for i in (1, 2, 3):
        out = f"/tmp/lane91-record/{name}-{i}"
        shutil.rmtree(out, ignore_errors=True)
        os.makedirs(out)
        subprocess.run(["node", os.path.join(root, "oracle", "x_ite-anim.mjs"), world, script, "--out", os.path.join(out, "js.json")],
                       check=True, cwd=root, timeout=300, capture_output=True)
        outs.append(out)
    view = lambda d: {k: v for k, v in json.load(open(os.path.join(d, "js.json"))).items() if k != "loadFrames"}
    a = view(outs[0])
    bad = sorted({k for o in outs[1:] for k in set(a) | set(view(o)) if a.get(k) != view(o).get(k)})
    return case, outs[0], bad, a.get("ok")


with ThreadPoolExecutor(jobs) as ex:
    results = list(ex.map(run, table))
cpath, upath = os.path.join(dest_dir, "cases.txt"), os.path.join(dest_dir, "UNSTABLE.txt")
lines = {l.split()[0]: l for l in open(cpath).read().splitlines()} if os.path.exists(cpath) else {}
unstable = {l.split()[0]: l for l in open(upath).read().splitlines()} if os.path.exists(upath) else {}
for (name, world, script), out, bad, ok in results:
    print(f"{name}: three X_ITE runs {'identical' if not bad else 'DIFFER in ' + ','.join(bad)}; ok={ok}")
    if bad or not ok:
        unstable[name] = f"{name} differs in {','.join(bad) or 'nothing'}; ok={ok}"
        continue
    unstable.pop(name, None)
    d = os.path.join(dest_dir, f"{name}-msaa0")
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    shutil.copy(os.path.join(out, "js.json"), d)
    lines[name] = " ".join([name, world, script, "0"])
open(cpath, "w").write("\n".join(lines[k] for k in sorted(lines)) + "\n")
if unstable:
    open(upath, "w").write("\n".join(unstable[k] for k in sorted(unstable)) + "\n")
elif os.path.exists(upath):
    os.remove(upath)
sys.exit(1 if any(b or not o for _, _, b, o in results) else 0)
