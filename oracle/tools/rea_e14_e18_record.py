#!/usr/bin/env python3
"""Record pinned X_ITE 16.5.0 (34cdf9f97) for the E14 and E18 navigation cases of sub:nav.examine (new baseline directories only).

  python3 -I -B oracle/tools/rea_e14_e18_record.py record --cases CASES.txt --dest DIR [--only SUBSTRING] [--jobs N] [--pictures]

CASES.txt lines are `name world script msaa` (paths from the repository root). For every case the X_ITE side is recorded three
times; the recorder depends on the script and the MSAA setting, like oracle/tools/x10_compare.py:
  msaa 0, size 512x512 -> oracle/x_ite-anim.mjs;  msaa 0, other size -> oracle/x_ite-anim-sized.mjs;
  msaa N > 0           -> oracle/x_ite-anim-msaa.mjs --ms N (the script's size).
The three js.json (without `loadFrames`, which follows Chrome's I/O) and, with --pictures, the three sets of PNGs must be identical;
the first is kept as DIR/NAME-msaaN/js.json (+ js/*.png). A case that differs is listed in DIR/UNSTABLE.txt and not kept. The check
that oracle/x_ite-pin/PINNED_COMMIT is 34cdf9f97 runs first. DIR/cases.txt is the list of the kept cases and DIR/SHA256SUMS the
sha256 of every kept file. A baseline is never taken from Rust.
"""
import hashlib, json, os, shutil, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
assert args and args[0] == "record"
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
only, jobs, pictures = opt("--only"), int(opt("--jobs", "4")), "--pictures" in args
dest_dir = os.path.join(root, opt("--dest"))
table = [l.split() for l in open(os.path.join(root, opt("--cases"))).read().splitlines() if l.strip()]
table = [c for c in table if not only or only in c[0]]
pin = open(os.path.join(root, "oracle", "x_ite-pin", "PINNED_COMMIT")).read().strip()
assert pin == "34cdf9f97", f"the pin is {pin}"
os.makedirs(dest_dir, exist_ok=True)
tmp = "/tmp/xiter-e14e18/record"


def record(case):
    name, world, script, msaa = case[0], case[1], case[2], int(case[3])
    size = json.load(open(os.path.join(root, script))).get("size", [512, 512])
    anim = "x_ite-anim.mjs" if list(size) == [512, 512] else "x_ite-anim-sized.mjs"
    extra = []
    if msaa:
        anim, extra = "x_ite-anim-msaa.mjs", ["--ms", str(msaa)]
    runs = []
    for i in (1, 2, 3):
        out = f"{tmp}/{name}-msaa{msaa}-{i}"
        shutil.rmtree(out, ignore_errors=True)
        os.makedirs(out)
        cmd = ["node", os.path.join(root, "oracle", anim), world, script, "--out", os.path.join(out, "js.json")] + extra
        if pictures:
            cmd += ["--png-dir", os.path.join(out, "js")]
        subprocess.run(cmd, check=True, cwd=root, timeout=300, capture_output=True)
        runs.append(out)
    view = lambda d: {k: v for k, v in json.load(open(os.path.join(d, "js.json"))).items() if k != "loadFrames"}
    pngs = lambda d: {f: hashlib.sha256(open(os.path.join(d, "js", f), "rb").read()).hexdigest() for f in sorted(os.listdir(os.path.join(d, "js")))} if pictures else {}
    a, pa = view(runs[0]), pngs(runs[0])
    bad = sorted({k for o in runs[1:] for k in set(a) | set(view(o)) if a.get(k) != view(o).get(k)})
    bad += sorted({"png:" + f for o in runs[1:] for f in set(pa) | set(pngs(o)) if pa.get(f) != pngs(o).get(f)})
    return case, runs[0], bad, a.get("ok"), a.get("touchSplit", 0)


with ThreadPoolExecutor(jobs) as ex:
    results = list(ex.map(record, table))
cpath, upath = os.path.join(dest_dir, "cases.txt"), os.path.join(dest_dir, "UNSTABLE.txt")
lines = {l.split()[0]: l for l in open(cpath).read().splitlines()} if os.path.exists(cpath) else {}
unstable = {}
for case, out, bad, ok, split in results:
    name, msaa = case[0], int(case[3])
    print(f"{name} msaa {msaa}: three X_ITE runs {'identical' if not bad else 'DIFFER in ' + ','.join(bad)}; ok={ok}; touch split {split}")
    if bad or not ok or split:
        unstable[name] = f"{name} differs in {','.join(bad) or 'nothing'}; ok={ok}; touch split {split}"
        continue
    d = os.path.join(dest_dir, f"{name}-msaa{msaa}")
    shutil.rmtree(d, ignore_errors=True)
    os.makedirs(d)
    shutil.copy(os.path.join(out, "js.json"), d)
    if pictures and os.path.isdir(os.path.join(out, "js")):
        shutil.copytree(os.path.join(out, "js"), os.path.join(d, "js"))
    lines[name] = " ".join(case[:4])
open(cpath, "w").write("\n".join(lines[k] for k in sorted(lines)) + "\n")
if unstable:
    open(upath, "w").write("\n".join(unstable[k] for k in sorted(unstable)) + "\n")
elif os.path.exists(upath):
    os.remove(upath)
sums = []
for dp, _, fs in os.walk(dest_dir):
    for f in fs:
        if f != "SHA256SUMS":
            p = os.path.join(dp, f)
            sums.append(f"{hashlib.sha256(open(p, 'rb').read()).hexdigest()}  {os.path.relpath(p, dest_dir)}")
open(os.path.join(dest_dir, "SHA256SUMS"), "w").write("\n".join(sorted(sums, key=lambda l: l.split('  ')[1])) + "\n")
shutil.rmtree(tmp, ignore_errors=True)
sys.exit(1 if unstable else 0)
