#!/usr/bin/env python3
"""Merge three identical recordings of x_ite-shot.mjs / x_ite-shot-msaa.mjs into one baseline directory.

  python3 -I oracle/tools/merge_repeat_runs.py <run1> <run2> <run3> <out>

<out> must not exist. It gets the PNGs and the index.json of run 1 plus a `repeatRuns` field (count, identical,
rgbaSha256 per file). Exits 1 and writes nothing when the three runs differ in any picture.
"""
import json, os, shutil, sys

runs = sys.argv[1:4]
out = sys.argv[4]
idx = [json.load(open(os.path.join(r, "index.json"))) for r in runs]
hashes = [{x["file"]: x["rgbaSha256"] for x in i["results"]} for i in idx]
identical = hashes[0] == hashes[1] == hashes[2]
if not identical:
    sys.exit("the three runs differ")
os.mkdir(out)
for f in os.listdir(runs[0]):
    if f.endswith(".png"):
        shutil.copy(os.path.join(runs[0], f), os.path.join(out, f))
idx[0]["repeatRuns"] = {"count": 3, "identical": True, "rgbaSha256": dict(sorted(hashes[0].items()))}
with open(os.path.join(out, "index.json"), "w") as f:
    json.dump(idx[0], f, indent=1)
print("ok", len(hashes[0]), "files")
