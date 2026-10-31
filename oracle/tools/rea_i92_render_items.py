#!/usr/bin/env python3
"""Render-core item baselines (issue #92; pinned X_ITE 16.5.0, headless Chrome on the GPU).

  XITE_PLAYWRIGHT=<x_ite-tests>/node_modules python3 -I -B oracle/tools/rea_i92_render_items.py shots

shoots oracle/synthetic-rea-i92-render-items/*.wrl (oracle/tools/gen_rea_i92_render_items.py) three times per pass with
x_ite-shot.mjs (`png`, antialiased="false") and x_ite-shot-msaa.mjs (`msaa4`, Multisampling 4). Every world's raw RGBA
sha256 must be identical in the three runs, and no run may log an error; the first run is kept as
oracle/rea-i92-render-items-baseline/{png,msaa4}/ (with its index.json). A baseline is never taken from Rust.
"""
import json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
worlds = os.path.join(root, "oracle", "synthetic-rea-i92-render-items")
dest = os.path.join(root, "oracle", "rea-i92-render-items-baseline")
env = dict(os.environ, NODE_PATH=os.environ.get("XITE_PLAYWRIGHT", ""))
if sys.argv[1:2] != ["shots"] or not env["NODE_PATH"]:
    sys.exit(__doc__)
only = sys.argv[sys.argv.index("--only") + 1] if "--only" in sys.argv else None
bad = 0
for name, tool in (("png", "x_ite-shot.mjs"), ("msaa4", "x_ite-shot-msaa.mjs")):
    runs = []
    for i in (1, 2, 3):
        out = f"/tmp/lane92-record/{name}-{i}"
        shutil.rmtree(out, ignore_errors=True)
        os.makedirs(out)
        cmd = ["node", os.path.join(root, "oracle", tool), worlds, out] + (["--only", only] if only else [])
        subprocess.run(cmd, check=True, cwd=root, env=env, timeout=1800, stdout=subprocess.DEVNULL)
        runs.append(out)
    index = [{r["file"]: r for r in json.load(open(os.path.join(o, "index.json")))["results"]} for o in runs]
    unstable = []
    for f, r in index[0].items():
        same = all(x.get(f, {}).get("rgbaSha256") == r.get("rgbaSha256") and r.get("ok") for x in index)
        errs = [l for l in r.get("log", []) if "error" in l.lower()]
        if name == "msaa4" and not errs and r.get("ok") and not same:
            # X_ITE's own multisampled output differs from run to run (three runs, three results): no reference exists.
            unstable.append(f)
        elif not same or errs:
            bad += 1
            print(f"{name} {f}: {'runs differ or failed' if not same else ''} {errs}")
    print(f"{name}: {len(index[0])} worlds, three runs compared")
    if not bad and only and os.path.isdir(os.path.join(dest, name)):
        # --only on an existing baseline: replace just those worlds and their index entries.
        d = os.path.join(dest, name)
        idx = json.load(open(os.path.join(d, "index.json")))
        new = json.load(open(os.path.join(runs[0], "index.json")))
        mine = {r["file"] for r in new["results"]}
        idx["results"] = sorted([r for r in idx["results"] if r["file"] not in mine] + new["results"], key=lambda r: r["file"])
        for f in mine:
            shutil.copy(os.path.join(runs[0], f + ".png"), d)
        json.dump(idx, open(os.path.join(d, "index.json"), "w"), indent=1)
        old = os.path.join(d, "unstable.json")
        keep = [f for f in (json.load(open(old)) if os.path.exists(old) else []) if f not in mine]
        keep = sorted(set(keep) | set(unstable))
        for f in unstable:
            os.remove(os.path.join(d, f + ".png"))
        if keep:
            json.dump(keep, open(old, "w"), indent=1)
        elif os.path.exists(old):
            os.remove(old)
    elif not bad:
        shutil.rmtree(os.path.join(dest, name), ignore_errors=True)
        shutil.copytree(runs[0], os.path.join(dest, name))
        for f in unstable:
            os.remove(os.path.join(dest, name, f + ".png"))
        if unstable:
            json.dump(sorted(unstable), open(os.path.join(dest, name, "unstable.json"), "w"), indent=1)
            print(f"{name}: no reference for {unstable} (X_ITE differs between its own runs)")
sys.exit(1 if bad else 0)
