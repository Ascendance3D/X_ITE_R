#!/usr/bin/env python3
"""The EXAMINE drag / zoom clipping regression suite (permanent check).

  python3 -I oracle/tools/run_x10_clip_suite.py [--jobs 4] [--out /tmp/x10-clip-suite] [--target dir]

Runs oracle/tools/x10_compare.py (X_ITE in headless GPU Chrome is the truth) on:
  * chronos-orrery with all 12 clip-*.json scripts (512x512, 1024x768, 1024x768 with six mouse
    moves per frame; rotate, zoom in, zoom out, middle/left drags);
  * nine other render-items and 43-villasantzip / 07-bookcase with the four 1024x768 multi-move
    scripts.
Each case compares poses (1e-4) at the captured frames and the PNGs (threshold 8, 0.5% of pixels).
KNOWN lists the cases that differ for a documented reason; they are reported as XFAIL and do not
fail the suite (an XFAIL that starts passing is reported as XPASS).
Exit status 1 when an unexpected failure remains.
"""
import concurrent.futures, os, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
args = sys.argv[1:]
opt = lambda n, d: args[args.index(n) + 1] if n in args else d
jobs = int(opt("--jobs", "4"))
out = opt("--out", "/tmp/x10-clip-suite")
target = opt("--target", os.path.join(root, "target-viewer", "release"))

ORRERY = "oracle/render-items/antiques/chronos-orrery/chronos-orrery.wrl"
BASES = ["clip-rotate-large", "clip-zoom-in-rotate", "clip-zoom-out-rotate", "clip-pan-zoom-drag"]
MULTI = [b + "-1024-multi" for b in BASES]
OTHERS = {
    "glass-cathedral": "oracle/render-items/art/glass-cathedral/glass-cathedral.wrl",
    "mall-promenade": "oracle/render-items/art/mall-promenade/mall-promenade.wrl",
    "smartcar-lite": "oracle/render-items/vehicles/smartcar/smartcar-lite.wrl",
    "advenbed2-qa-hero": "oracle/render-items/furniture/advenbed2/qa/qa-hero.wrl",
    "43-villasantzip": "oracle/corpus-x0/43-villasantzip.wrl",
    "07-bookcase": "oracle/corpus-x0/07-bookcase.wrl",
    "echo-cabinet": "oracle/render-items/antiques/echo-cabinet/echo-cabinet.wrl",
    "signal-storm-globe": "oracle/render-items/electronics/signal-storm-globe/signal-storm-globe.wrl",
    "chronos-orrery-v2": "oracle/render-items/antiques/chronos-orrery-v2/chronos-orrery-v2.wrl",
}
KNOWN = {
    ("smartcar-lite", "clip-zoom-in-rotate-1024-multi"):
        "camera inside the model: one huge near triangle shades differently (6.7% of frame 80); not near-plane clipping",
}

cases = [("chronos-orrery", ORRERY, s + v) for s in BASES for v in ("", "-1024", "-1024-multi")]
cases += [(name, path, s) for name, path in OTHERS.items() for s in MULTI]

def run(case):
    name, world, script = case
    d = os.path.join(out, f"{name}-{script}")
    r = subprocess.run([sys.executable, "-I", os.path.join(root, "oracle/tools/x10_compare.py"), world, os.path.join(root, f"oracle/x10-scripts/{script}.json"), d, "--target", target],
                       capture_output=True, text=True, cwd=root, timeout=300)
    open(d + ".txt", "w").write(r.stdout + r.stderr)
    return case, r.returncode == 0

bad = 0
with concurrent.futures.ThreadPoolExecutor(jobs) as ex:
    for (name, world, script), ok in ex.map(run, cases):
        known = KNOWN.get((name, script))
        status = ("PASS" if ok else "FAIL") if not known else ("XPASS" if ok else "XFAIL")
        bad += status == "FAIL"
        print(f"{status:5} {name:20} {script}" + (f"   ({known})" if known and not ok else ""))

print(f"\n{len(cases)} cases, {bad} unexpected failure(s); details in {out}/*.txt")
sys.exit(1 if bad else 0)
