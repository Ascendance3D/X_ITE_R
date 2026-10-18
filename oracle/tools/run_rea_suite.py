#!/usr/bin/env python3
"""Run the REA full-port oracle worlds (oracle/synthetic-rea/ by default, --dir to change) and summarize.

  python3 -I oracle/tools/run_rea_suite.py [--dir oracle/synthetic-rea] [--jobs 8] [--out /tmp/rea-suite] [--target target-viewer/release]
         [--no-images] [world ...]

For each world, runs oracle/tools/x10_compare.py (pinned X_ITE in headless GPU Chrome against
x3d-drive: scene text exact at every dumped frame, poses 1e-4, images threshold 8 / 0.5%) and prints
one line: PASS/FAIL, the number of passed and failed checks, the largest pose delta, and the first
failing diff lines. Exit status 1 if any world fails. Tolerances are x10_compare's own.
"""
import os, re, subprocess, sys
from concurrent.futures import ThreadPoolExecutor

root = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
jobs = int(opt("--jobs", "8"))
out = opt("--out", "/tmp/rea-suite")
target = opt("--target", os.path.join(root, "target-viewer", "release"))
images = "--no-images" not in args
worlds = [a for i, a in enumerate(args) if not a.startswith("--") and not (i and args[i - 1] in ("--jobs", "--out", "--target", "--dir"))]
d = os.path.join(root, opt("--dir", os.path.join("oracle", "synthetic-rea")))
worlds = worlds or sorted(f[:-4] for f in os.listdir(d) if f.endswith(".wrl"))


def run(w):
    o = os.path.join(out, w)
    cmd = ["python3", "-I", os.path.join(root, "oracle", "tools", "x10_compare.py"), os.path.join(d, w + ".wrl"), os.path.join(d, "scripts", w + ".json"), o, "--target", target]
    if not images:
        cmd.append("--no-images")
    try:
        r = subprocess.run(cmd, capture_output=True, text=True, cwd=root, timeout=900)
        text = r.stdout + r.stderr
    except subprocess.TimeoutExpired:
        return w, False, "TIMEOUT"
    passed = len(re.findall(r"^PASS ", text, re.M))
    failed = len(re.findall(r"^FAIL ", text, re.M))
    worst = max([float(x) for t in re.findall(r"\|dpos\| (\S+) \|dcenter\| (\S+) dangle (\S+)", text) for x in t] or [0.0])
    first = "\n".join(l for l in text.splitlines() if l.startswith("FAIL") or l.startswith("    "))[:1200]
    return w, r.returncode == 0 and failed == 0, f"{passed} checks passed, {failed} failed, max pose delta {worst:.2e}" + (("\n" + first) if failed else "")


with ThreadPoolExecutor(jobs) as ex:
    results = list(ex.map(run, worlds))

bad = 0
for w, ok, msg in results:
    print(("PASS " if ok else "FAIL ") + w + ": " + msg)
    bad += 0 if ok else 1
print(f"suite {os.path.basename(d)}: {len(results) - bad} of {len(results)} worlds pass")
sys.exit(1 if bad else 0)
