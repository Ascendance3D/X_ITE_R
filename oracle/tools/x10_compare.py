#!/usr/bin/env python3
"""Run one X10 case in both implementations and compare.

  python3 oracle/tools/x10_compare.py <world inside oracle/> <script.json> <out-dir>
         [--pose-tol 1e-4] [--rust-only-bin DIR] [--no-images] [--msaa N]

Runs oracle/x_ite-anim.mjs (pinned X_ITE, headless Chrome on the GPU) and crates/x3d-viewer's
x3d-drive with the same script. Compares: scene text (toVRMLString) at the dumped frames (exact),
user position / orientation / centerOfRotation of the bound viewpoint at every recorded frame
(numeric, with a tolerance), the bound viewpoint's name, and the PNGs (x3d-shot compare,
threshold 8, at most 0.5% of pixels over). Prints one summary line per check and exits 1 if any
check fails.
"""
import json, math, os, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(__file__), "..", ".."))
args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
pos = [a for i, a in enumerate(args) if not a.startswith("--") and not (i and args[i - 1].startswith("--"))]
world, script, out = pos[:3]
tol = float(opt("--pose-tol", "1e-4"))
target = opt("--target", os.path.join(root, "target-viewer", "release"))  # release: a stale debug build gave a false GPU-layout crash
os.makedirs(out, exist_ok=True)
js_json, rs_json = os.path.join(out, "js.json"), os.path.join(out, "rs.json")
env = dict(os.environ, CARGO_TARGET_DIR=os.path.join(root, "target-viewer"))
imgs = "--no-images" not in args

# Scripts whose size is not 512x512 need the sized oracle page (anim.html is fixed at 512x512).
size = json.load(open(script)).get("size", [512, 512])
anim = "x_ite-anim.mjs" if list(size) == [512, 512] else "x_ite-anim-sized.mjs"
msaa = opt("--msaa")  # additive: X_ITE antialiased with N samples vs Rust N samples (offscreen, GL sample positions)
cmd = ["node", os.path.join(root, "oracle", anim if not msaa else "x_ite-anim-msaa.mjs"), world, script, "--out", js_json] + (["--ms", msaa] if msaa else [])
if imgs: cmd += ["--png-dir", os.path.join(out, "js")]
subprocess.run(cmd, check=False, timeout=180, cwd=root)
cmd = [os.path.join(target, "x3d-drive"), world, script, "--oracle", os.path.join(root, "oracle"), "--out", rs_json, "--warmup-from", js_json]
if msaa: cmd += ["--msaa", msaa, "--gl-samples"]
if imgs: cmd += ["--png-dir", os.path.join(out, "rs")]
subprocess.run(cmd, check=False, timeout=180, cwd=root)

js, rs = json.load(open(js_json)), json.load(open(rs_json))
fails = 0
def report(ok, what):
    global fails
    print(("PASS " if ok else "FAIL ") + what)
    fails += 0 if ok else 1

report(js.get("ok") and rs.get("ok"), f"both ran (js ok={js.get('ok')} {js.get('error','')}, rs ok={rs.get('ok')} {rs.get('error','')}) warmup frames {js.get('warmupFrames')}")
if not (js.get("ok") and rs.get("ok")): sys.exit(1)

import re
text_tol = float(opt("--text-tol", "0"))
NUM = re.compile(r"-?\d+(?:\.\d+)?(?:[eE][-+]?\d+)?")

def text_equal(a, b):
    """(equal, max numeric difference). With text_tol 0 the texts must be identical; otherwise
    they must be identical after replacing every number, and the numbers within text_tol
    (absolute below magnitude 1, relative above)."""
    if a == b: return True, 0.0
    if not text_tol or b is None: return False, None
    if NUM.sub("#", a) != NUM.sub("#", b): return False, None
    worst = 0.0
    for x, y in zip(NUM.findall(a), NUM.findall(b)):
        d = abs(float(x) - float(y)) / max(1.0, abs(float(x)))
        worst = max(worst, d)
    return worst <= text_tol, worst

for k, a in js["vrml"].items():
    b = rs["vrml"].get(k)
    eq, worst = text_equal(a, b)
    report(eq, f"scene text equal at frame {k}" + ("" if not worst else f" (numbers within {worst:.1e})"))
    if not eq:
        import difflib
        d = list(difflib.unified_diff(a.split("\n"), b.split("\n") if b else [], lineterm="", n=0))
        print("\n".join("    " + l for l in d[:20]))

def quat(r):
    x, y, z, a = r
    n = math.sqrt(x*x + y*y + z*z) or 1
    s = math.sin(a / 2) / n
    return (x*s, y*s, z*s, math.cos(a / 2))

worst = {}
for k, a in js["poses"].items():
    b = rs["poses"].get(k)
    if b is None: report(False, f"pose at frame {k} missing"); continue
    dp = max(abs(p - q) for p, q in zip(a["position"], b["position"]))
    dc = max(abs(p - q) for p, q in zip(a["centerOfRotation"], b["centerOfRotation"]))
    qa, qb = quat(a["orientation"]), quat(b["orientation"])
    dot = abs(sum(p * q for p, q in zip(qa, qb)))
    do = 2 * math.acos(min(1.0, dot))
    ok = dp <= tol and dc <= tol and do <= tol and a["viewer"] == b["viewer"] and (a["viewpoint"] == b["viewpoint"] or b["viewpoint"] == "<default>")
    # fieldOfViewScale (PLANE zoom) and, for an OrthoViewpoint, fieldOfViewOffset (both sides report them).
    extra = ""
    if "fieldOfViewScale" in a or "fieldOfViewScale" in b:
        ds = abs(a.get("fieldOfViewScale", math.nan) - b.get("fieldOfViewScale", math.nan))
        ok = ok and ds <= tol
        extra = f" |dfovScale| {ds:.2e}"
    if a.get("fieldOfViewOffset") is not None or b.get("fieldOfViewOffset") is not None:
        ao, bo = a.get("fieldOfViewOffset"), b.get("fieldOfViewOffset")
        dfo = max(abs(p - q) for p, q in zip(ao, bo)) if ao is not None and bo is not None else math.nan
        ok = ok and dfo <= tol
        extra += f" |dfovOffset| {dfo:.2e}"
    report(ok, f"pose frame {k}: |dpos| {dp:.2e} |dcenter| {dc:.2e} dangle {do:.2e}{extra} viewer {a['viewer']}/{b['viewer']} viewpoint {a['viewpoint']!r}/{b['viewpoint']!r}")

if imgs:
    r = subprocess.run([os.path.join(target, "x3d-shot"), "compare", os.path.join(out, "js"), os.path.join(out, "rs"), "--threshold", "8", "--max-pct", "0.5"], capture_output=True, text=True, cwd=root)
    print(r.stdout.rstrip())
    # x3d-shot compare exits 0 even for "over" or SIZE-mismatch files; read its summary line.
    ok_imgs = r.returncode == 0 and ", 0 over, 0 missing" in r.stdout and "SIZE" not in r.stdout
    report(ok_imgs, "images within tolerance (threshold 8, max 0.5%)")
sys.exit(1 if fails else 0)
