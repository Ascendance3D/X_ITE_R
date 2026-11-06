#!/usr/bin/env python3
"""Platform pictures of the pinned X_ITE 16.5.0 for the x3d-render picture tests (owner decision 2026-11-04, issues #69, #89 and
#461). For a picture key such as oracle/render-lines-edge-baseline (made by oracle/x_ite-shot.mjs from a corpus directory) this tool
renders the corpus again ON THE TARGET MACHINE, three times, and keeps a picture only when
  1. the three pictures are pixel-identical, and
  2. they differ in a pixel from the NVIDIA picture of the same file.
The pictures go to oracle/<stem>-<platform>-baseline/<file> with a PROVENANCE.json. The NVIDIA key is never written. A file whose
three runs differ is listed in UNSTABLE.txt and kept nowhere. A picture is never taken from Rust. Tests choose the picture with
X3D_REFERENCE_PLATFORM=<platform> (crates/x3d-render/tests/common/reference.rs); a file without a platform picture keeps the NVIDIA one.

  XITE_CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" XITE_ANGLE_BACKEND=metal \\
  XITE_TESTS=<dir whose node_modules holds playwright> \\
  python3 -I -B oracle/tools/record_platform_pictures.py --platform apple-m1 --key render-lines-edge-baseline \\
      --corpus synthetic-lines-edge --size 256x256 [--ms 4] --hardware "Apple M1, macOS 27.0.1"

The recorder is oracle/x_ite-shot.mjs, or oracle/x_ite-shot-msaa.mjs with --ms N; run from the repository root.
"""
import datetime, hashlib, importlib.util, json, os, shutil, subprocess, sys, tempfile

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
platform, key, corpus, size, ms, hardware = opt("--platform"), opt("--key"), opt("--corpus"), opt("--size", "256x256"), opt("--ms"), opt("--hardware", "")
assert platform and key and corpus, "--platform, --key and --corpus are required"
assert os.environ.get("XITE_CHROME") and os.environ.get("XITE_ANGLE_BACKEND"), "set XITE_CHROME and XITE_ANGLE_BACKEND"

spec = importlib.util.spec_from_file_location("png_pixels", os.path.join(os.path.dirname(os.path.abspath(__file__)), "png_pixels.py"))
png_pixels = importlib.util.module_from_spec(spec)
spec.loader.exec_module(png_pixels)
pixels = png_pixels.pixels
sha = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest()

src = os.path.join(root, "oracle", key)
dest = os.path.join(root, "oracle", f"{key.removesuffix('-baseline')}-{platform}-baseline")
files = sorted(f for f in os.listdir(src) if f.endswith(".png"))
chrome = subprocess.run([os.environ["XITE_CHROME"], "--version"], capture_output=True, text=True).stdout.strip()
pinned = open(os.path.join(root, "oracle", "x_ite-pin", "PINNED_COMMIT")).read().strip()

tmp = tempfile.mkdtemp(prefix="platform-pictures-")
runs = []
for i in (1, 2, 3):
    out = os.path.join(tmp, str(i))
    cmd = ["node", os.path.join(root, "oracle", "x_ite-shot.mjs" if not ms else "x_ite-shot-msaa.mjs"), os.path.join("oracle", corpus), out, "--size", size]
    if ms:
        cmd += ["--ms", ms]
    subprocess.run(cmd, check=True, cwd=root, timeout=600, capture_output=True)
    runs.append(out)

index = json.load(open(os.path.join(runs[0], "index.json")))
kept, same, unstable = {}, [], []
for f in files:
    made = [os.path.join(r, f) for r in runs]
    if not all(os.path.exists(m) for m in made):
        unstable.append(f"{f}: not rendered")
        continue
    p = [pixels(m) for m in made]
    if not (p[0] == p[1] == p[2]):
        unstable.append(f"{f}: the three runs differ")
        continue
    if p[0] == pixels(os.path.join(src, f)):
        same.append(f)
        continue
    os.makedirs(dest, exist_ok=True)
    shutil.copyfile(made[0], os.path.join(dest, f))
    kept[f] = dict(png=sha(os.path.join(dest, f)), runs_png_sha256=[sha(m) for m in made])

if kept:
    gl = index.get("gl", {})
    prov = dict(platform=platform, hardware=hardware, pinned=pinned, chrome=chrome, gl_renderer=gl.get("renderer"),
                angle_backend=os.environ["XITE_ANGLE_BACKEND"], recorder="oracle/x_ite-shot.mjs" if not ms else "oracle/x_ite-shot-msaa.mjs",
                recorder_args=f"oracle/{corpus} <out> --size {size}" + (f" --ms {ms}" if ms else ""), runs=3,
                rule=f"kept only where the three runs are pixel-identical and differ from the NVIDIA picture in oracle/{key}",
                nvidia_reference=f"oracle/{key} (unchanged)", recorded=datetime.date.today().isoformat(), kept=kept, same_as_nvidia=same)
    with open(os.path.join(dest, "PROVENANCE.json"), "w") as fp:
        json.dump(prov, fp, indent=1, sort_keys=True)
        fp.write("\n")
if unstable:
    os.makedirs(dest, exist_ok=True)
    with open(os.path.join(dest, "UNSTABLE.txt"), "a") as fp:
        fp.write("".join(u + "\n" for u in unstable))

shutil.rmtree(tmp, ignore_errors=True)
print(f"{key}/{platform}: {len(files)} pictures recorded three times; {len(kept)} kept, {len(same)} equal to the NVIDIA picture, {len(unstable)} unstable")
for u in unstable:
    print("UNSTABLE", u)
for f in kept:
    print("kept", f)
