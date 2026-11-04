#!/usr/bin/env python3
"""Collision collideTime / Billboard orientation baselines (ledger node:Collision, node:Anchor) (pinned X_ITE 16.5.0, headless Chrome
on the GPU).

  python3 -I -B oracle/tools/rea_collision_time.py record [--only SUBSTRING] [--jobs N]
      For every case of oracle/synthetic-rea-collision-time/cases.txt (made by gen_rea_collision_time.py) and MSAA 0 and 4: record
      the X_ITE side three times (x_ite-anim.mjs / x_ite-anim-msaa.mjs). All three runs (js.json and every PNG) must be identical
      (`loadFrames`, the frame the load finished in, follows Chrome's I/O and is left out of the check, as in rea_b24.py); the
      first is kept as oracle/rea-collision-time-baseline/NAME-msaaM and the case line is written to cases.txt there. A case whose
      runs differ is listed in UNSTABLE.txt and is not kept. A baseline is never taken from Rust.
      Needs XITE_TESTS (the x_ite-tests checkout with playwright) and oracle/x_ite-pin/node_modules (oracle/README.md).
"""
import filecmp, json, os, shutil, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
assert args and args[0] == "record"
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
only, jobs = opt("--only"), int(opt("--jobs", "6"))
src = os.path.join(root, "oracle", "synthetic-rea-collision-time", "cases.txt")
dest_dir = os.path.join(root, "oracle", "rea-collision-time-baseline")
os.makedirs(dest_dir, exist_ok=True)
table = [l.split() + [m] for l in open(src).read().splitlines() if l.strip() for m in ("0", "4")]
table = [c for c in table if not only or only in c[0]]
view = lambda d: {k: v for k, v in json.load(open(os.path.join(d, "js.json"))).items() if k != "loadFrames"}


def run(case):
    name, world, script, msaa = case
    anim = "x_ite-anim.mjs" if msaa == "0" else "x_ite-anim-msaa.mjs"
    runs = []
    for i in (1, 2, 3):
        out = f"/tmp/agentH-record/{name}-msaa{msaa}-{i}"
        shutil.rmtree(out, ignore_errors=True)
        os.makedirs(out)
        cmd = ["node", os.path.join(root, "oracle", anim), world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "js")]
        if msaa != "0":
            cmd += ["--ms", msaa]
        subprocess.run(cmd, check=True, cwd=root, timeout=300, capture_output=True)
        runs.append(out)
    a = runs[0]
    pngs = sorted(os.listdir(os.path.join(a, "js")))
    same = all(pngs == sorted(os.listdir(os.path.join(b, "js"))) and view(a) == view(b)
               and all(filecmp.cmp(os.path.join(a, "js", f), os.path.join(b, "js", f), shallow=False) for f in pngs) for b in runs[1:])
    return case, a, same, len(pngs), view(a).get("ok")


with ThreadPoolExecutor(jobs) as ex:
    results = list(ex.map(run, table))
cpath, upath = os.path.join(dest_dir, "cases.txt"), os.path.join(dest_dir, "UNSTABLE.txt")
lines = {tuple(l.split()[:1] + l.split()[3:4]): l for l in open(cpath).read().splitlines()} if os.path.exists(cpath) else {}
unstable = {l.split()[0]: l for l in open(upath).read().splitlines()} if os.path.exists(upath) else {}
for (name, world, script, msaa), out, same, n, ok in results:
    print(f"{name} MSAA {msaa}: three X_ITE runs {'identical' if same else 'DIFFER'} ({n} images); ok={ok}")
    if not same or not ok:
        unstable[f"{name}-msaa{msaa}"] = f"{name}-msaa{msaa} identical={same} ok={ok}"
        continue
    unstable.pop(f"{name}-msaa{msaa}", None)
    d = os.path.join(dest_dir, f"{name}-msaa{msaa}")
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    shutil.copy(os.path.join(out, "js.json"), d + ".json")
    shutil.copytree(os.path.join(out, "js"), os.path.join(d, "js"))
    lines[(name, msaa)] = " ".join([name, world, script, msaa])
open(cpath, "w").write("\n".join(lines[k] for k in sorted(lines)) + "\n")
if unstable:
    open(upath, "w").write("\n".join(unstable[k] for k in sorted(unstable)) + "\n")
elif os.path.exists(upath):
    os.remove(upath)
sys.exit(1 if any(not s or not o for _, _, s, _, o in results) else 0)
