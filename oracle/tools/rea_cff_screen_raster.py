#!/usr/bin/env python3
"""ScreenText raster baseline (node:FontLibrary, sub:text.fonts; pinned X_ITE 16.5.0, headless Chrome on the GPU).

  XITE_TESTS=<x_ite-tests> python3 -I -B oracle/tools/rea_cff_screen_raster.py record

shoots oracle/synthetic-rea-cff-screen-raster/*.wrl three times with `x_ite-shot.mjs --canvas`. Every world's raw RGBA sha256
and the sha256 of its canvas record (the 2D canvas calls of ScreenText and the alpha of every canvas pixel) must be identical
in the three runs, and no run may log an error. The first run is then kept as oracle/rea-cff-screen-raster-baseline/ (PNG,
<world>.canvas.json, index.json). An existing baseline stops the tool: nothing is overwritten. A baseline is never taken
from Rust. Prepare the worktree as in oracle/README.md, "Recording from a worktree".
"""
import json, os, shutil, subprocess, sys, tempfile

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
worlds = os.path.join(root, "oracle", "synthetic-rea-cff-screen-raster")
dest = os.path.join(root, "oracle", "rea-cff-screen-raster-baseline")

if sys.argv[1:] != ["record"] or not os.environ.get("XITE_TESTS"):
    sys.exit(__doc__)
if os.path.lexists(dest):
    sys.exit(f"{dest} exists; a new recording goes to a new baseline directory")

tmp = tempfile.mkdtemp(prefix="cff-screen-raster-")
runs = []
for i in (1, 2, 3):
    out = os.path.join(tmp, f"run-{i}")
    subprocess.run(["node", os.path.join(root, "oracle", "x_ite-shot.mjs"), worlds, out, "--canvas"], check=True, cwd=root, timeout=900, stdout=subprocess.DEVNULL)
    runs.append({r["file"]: r for r in json.load(open(os.path.join(out, "index.json")))["results"]})

bad = []
for f, r in sorted(runs[0].items()):
    errors = [l for l in r.get("log", []) if "error" in l.lower()]
    same = all(x.get(f, {}).get(k) == r.get(k) for x in runs for k in ("rgbaSha256", "canvasSha256"))
    if not r.get("ok") or errors or not same or not r.get("canvases"):
        bad.append(f"{f}: ok {r.get('ok')}, same in three runs {same}, canvases {r.get('canvases')}, errors {errors}")
print(f"{len(runs[0])} worlds, three runs compared")
if bad:
    shutil.rmtree(tmp)
    sys.exit("\n".join(bad))
shutil.copytree(os.path.join(tmp, "run-1"), dest)
shutil.rmtree(tmp)
print(f"wrote {dest}")
