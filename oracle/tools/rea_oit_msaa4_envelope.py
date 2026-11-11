#!/usr/bin/env python3
"""The multisampled OIT pictures of the worlds whose X_ITE output is not the same in every run.

oracle/rea-oit-i92-render-items-baseline/msaa4/unstable.json lists the worlds for which three runs of the pinned X_ITE 16.5.0 with
`orderIndependentTransparency` and Multisampling 4 gave three different pictures, so they have no single reference there. This tool
measures how far the runs are apart and, when they are close, keeps the envelope of the runs:

  XITE_PLAYWRIGHT=<x_ite-tests>/node_modules python3 -I -B oracle/tools/rea_oit_msaa4_envelope.py record [runs]

shoots each listed world `runs` times (default 8, x_ite-shot-msaa.mjs --oit) and writes, for each world, the lowest and the highest
value of every channel of every pixel over the runs (<world>.min.png, <world>.max.png) and index.json (the hash of every run, the
number of pixels that differ between runs, the largest spread) into oracle/rea-oit-i92-render-items-baseline/msaa4-envelope/.

It refuses when a run fails or logs an error, when the spread of a channel is over MAX_SPREAD levels, or when more than
MAX_DIFFERING of the pixels differ between runs: such a world has no usable reference. It never overwrites a baseline, and it does
not change msaa4/unstable.json. A baseline is never taken from Rust."""
import json, os, shutil, subprocess, sys
from PIL import Image

MAX_SPREAD = 2          # levels of 255
MAX_DIFFERING = 0.001   # of the pixels

root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
base = os.path.join(root, "oracle", "rea-oit-i92-render-items-baseline")
dest = os.path.join(base, "msaa4-envelope")
tmp = "/tmp/rea-oit-msaa4-envelope"

def main():
    if sys.argv[1:2] != ["record"]:
        sys.exit(__doc__)
    runs = int(sys.argv[2]) if len(sys.argv) > 2 else 8
    if runs < 5:
        sys.exit("at least 5 runs")
    if os.path.exists(dest):
        sys.exit(f"{dest} exists; a baseline is not overwritten")
    worlds = json.load(open(os.path.join(base, "msaa4", "unstable.json")))
    shutil.rmtree(tmp, ignore_errors=True)
    indexes = []
    for i in range(1, runs + 1):
        out = os.path.join(tmp, f"run-{i}")
        cmd = ["node", os.path.join(root, "oracle", "x_ite-shot-msaa.mjs"), os.path.join(root, "oracle", "synthetic-rea-i92-render-items"), out,
               "--only", ",".join(worlds), "--oit"]
        subprocess.run(cmd, check=True, cwd=root, timeout=1800, stdout=subprocess.DEVNULL)
        indexes.append(json.load(open(os.path.join(out, "index.json"))))
    report, bad = {}, []
    os.makedirs(os.path.join(tmp, "out"))
    for world in worlds:
        results = [next(r for r in ix["results"] if r["file"] == world) for ix in indexes]
        for i, r in enumerate(results):
            errors = [l for l in r.get("log", []) if "error" in l.lower()]
            if not r.get("ok") or errors:
                bad.append(f"{world} run {i + 1}: ok {r.get('ok')}, errors {errors}")
        images = [Image.open(os.path.join(tmp, f"run-{i}", world + ".png")).convert("RGB") for i in range(1, runs + 1)]
        size = images[0].size
        data = [im.tobytes() for im in images]
        lo = bytes(min(c) for c in zip(*data))
        hi = bytes(max(c) for c in zip(*data))
        differing = sum(1 for p in range(0, len(lo), 3) if lo[p:p + 3] != hi[p:p + 3])
        spread = max(h - l for l, h in zip(lo, hi))
        pixels = size[0] * size[1]
        if spread > MAX_SPREAD or differing > pixels * MAX_DIFFERING:
            bad.append(f"{world}: {differing} of {pixels} pixels differ between runs, largest spread {spread}")
        Image.frombytes("RGB", size, lo).save(os.path.join(tmp, "out", world + ".min.png"))
        Image.frombytes("RGB", size, hi).save(os.path.join(tmp, "out", world + ".max.png"))
        report[world] = {"runs": [r["rgbaSha256"] for r in results], "distinctRuns": len({r["rgbaSha256"] for r in results}),
                         "differingPixels": differing, "largestSpread": spread, "width": size[0], "height": size[1]}
        print(f"{world}: {runs} runs, {report[world]['distinctRuns']} distinct, {differing} pixels differ, largest spread {spread}")
    if bad:
        sys.exit("\n".join(bad) + f"\nnothing written (see {tmp})")
    head = {k: v for k, v in indexes[0].items() if k != "results"}
    json.dump({**head, "runs": runs, "maxSpread": MAX_SPREAD, "maxDiffering": MAX_DIFFERING, "worlds": report},
              open(os.path.join(tmp, "out", "index.json"), "w"), indent=1)
    shutil.copytree(os.path.join(tmp, "out"), dest)
    shutil.rmtree(tmp)
    print(f"wrote {dest}")

main()
