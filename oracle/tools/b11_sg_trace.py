#!/usr/bin/env python3
"""REA batch 11 lane b11-4-fog-screen: the ScreenGroup matrix feedback of the pinned X_ITE, traced per traversal.

  python3 -I oracle/tools/b11_sg_trace.py [OUT.json]

Works on a COPY of oracle/x_ite-pin in /tmp/b11-sg-trace (the pin is never edited): ScreenGroup.traverse logs the traverse type, the model-view matrix
and its own matrix (the one the CAMERA traversal reuses from the last DISPLAY traversal) into window.__tr, and a copy of x_ite-anim.mjs writes the log as
`trace` into the result. Runs oracle/synthetic-rea-b10-stepped/sg-viewpoint-degenerate twice and requires identical traces. The trace shows the cause of
the black frames: a Viewpoint inside a ScreenGroup sits ~5 pixels from its origin, so every frame scales the group's matrix by about 5 * 2 tan (fov / 2) /
512 until Matrix4.inverse meets a zero determinant (it returns the ZERO matrix), the camera becomes ZERO, Background.display takes the rotation of a zero
matrix (Matrix4.factor: NaN) and draws a NaN sphere over the (0, 0, 0, 0) clear.
"""
import json, os, shutil, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
work = "/tmp/b11-sg-trace"
out = sys.argv[1] if len(sys.argv) > 1 else os.path.join(work, "trace.json")
shutil.rmtree(work, ignore_errors=True)
o = os.path.join(work, "w", "o", "oracle")
os.makedirs(o)
shutil.copytree(os.path.join(root, "oracle", "x_ite-pin", "src"), os.path.join(o, "x_ite-pin", "src"))
shutil.copy(os.path.join(root, "oracle", "x_ite-pin", "package.json"), os.path.join(o, "x_ite-pin"))
os.symlink(os.path.realpath(os.path.join(root, "oracle", "x_ite-pin", "node_modules")), os.path.join(o, "x_ite-pin", "node_modules"))
# x_ite-anim.mjs resolves playwright from ../../../x_ite-tests of its own directory.
os.symlink("/home/ryan/Projects/cybertown/X_ITE/x_ite-tests", os.path.join(work, "x_ite-tests"))
shutil.copytree(os.path.join(root, "oracle", "synthetic-rea-b10-stepped"), os.path.join(o, "synthetic-rea-b10-stepped"))
shutil.copy(os.path.join(root, "oracle", "x_ite-anim.mjs"), o)
p = os.path.join(o, "x_ite-pin/src/x_ite/Components/Layout/ScreenGroup.js")
s = open(p).read()
needle = "      const modelViewMatrix = renderObject .getModelViewMatrix ();\n"
assert needle in s
s = s.replace(needle, needle + "      (window .__tr ??= []) .push ({ type, mv: Array .from (modelViewMatrix .get ()), m: Array .from (this .matrix), vp: renderObject .getViewpoint () ?.getName () });\n", 1)
open(p, "w").write(s)
p = os.path.join(o, "x_ite-anim.mjs")
s = open(p).read()
needle = "result .log = (await page"
assert needle in s
s = s.replace(needle, "result .trace = await page .evaluate (() => window .__tr ?? [ ]);\n   " + needle, 1)
open(p, "w").write(s)
runs = []
for i in (1, 2):
    r = os.path.join(work, f"run{i}.json")
    subprocess.run(["node", "x_ite-anim.mjs", "synthetic-rea-b10-stepped/sg-viewpoint-degenerate.wrl", "synthetic-rea-b10-stepped/sg-viewpoint-degenerate.json", "--out", r], check=True, cwd=o, timeout=300)
    runs.append(json.load(open(r))["trace"])
same = runs[0] == runs[1]
print(f"{len(runs[0])} ScreenGroup traversals, two X_ITE runs {'identical' if same else 'DIFFER'}")
if not same:
    sys.exit(1)
# (a ScreenGroup starts as Matrix4 (0) until its first DISPLAY traversal.)
live = next(i for i, e in enumerate(runs[0]) if any(e["m"]))
zero = next(i for i, e in enumerate(runs[0]) if i > live and not any(e["m"]))
print(f"first live matrix at traversal {live}, first zero matrix after it at traversal {zero} of {len(runs[0])} (3 ScreenGroups x CAMERA and DISPLAY per frame)")
json.dump(runs[0], open(out, "w"))
