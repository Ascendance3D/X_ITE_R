#!/usr/bin/env python3
"""The reference of the multisampled worlds whose X_ITE picture is not the same in every run: the stored runs and their envelope.

msaa4/unstable.json of oracle/rea-oit-i92-render-items-baseline (option `orderIndependentTransparency` on) and of
oracle/rea-i92-render-items-baseline (option off, `--i92`) lists the worlds for which runs of the pinned X_ITE 16.5.0 with
Multisampling 4 gave different pictures, so they have no single reference. Owner decision 2026-11-11: the reference of such a
world is the interval between the lowest and the highest value of every channel of every pixel over stored, repeated runs.

  XITE_TESTS=<x_ite-tests> python3 -I -B oracle/tools/rea_oit_msaa4_envelope.py record [runs] [--i92]
  python3 -I -B oracle/tools/rea_oit_msaa4_envelope.py build [--i92]
  python3 -I -B oracle/tools/rea_oit_msaa4_envelope.py check [--i92]

`record` shoots each listed world `runs` times (default 8; x_ite-shot-msaa.mjs, with --oit unless --i92) and stores every picture
as the recorder wrote it, <base>/msaa4-envelope-v1/raw/run-<n>/<world>.png, and manifest.json: the recorder's own description
(pin, Chrome, GL renderer, size, Multisampling), and for each world the scene file with its SHA-256, its viewpoint statement, and
the SHA-256 of every stored file and of its pixels. Then it runs `build`.

`build` reads only the stored pictures. It checks each against manifest.json and writes <world>.min.png and <world>.max.png (the
lowest and the highest value of each channel; no margin) and envelope.json (the number of pixels that differ between runs, the
largest spread, the SHA-256 of the two pictures). It never overwrites: when the files exist they must be equal to its output.

`check` verifies the stored pictures against manifest.json, builds the envelope twice into memory, and requires both results to
be equal, byte for byte, to each other and to the stored files.

A run that fails or logs an error stops `record`, and nothing is written. So does a spread over MAX_SPREAD levels or more than
MAX_DIFFERING of the pixels differing: such a world has no usable reference. No run is left out of an envelope. A version
directory is never changed: a new recording is a new version (VERSION). msaa4/ and unstable.json do not change. A reference is
never taken from Rust. Prepare the worktree as in oracle/README.md, "Recording from a worktree"."""
import hashlib, io, json, os, shutil, subprocess, sys, zlib
import PIL
from PIL import Image

VERSION = "v1"
MAX_SPREAD = 2          # levels of 255
MAX_DIFFERING = 0.001   # of the pixels
MIN_RUNS = 5

root = os.path.dirname(os.path.dirname(os.path.dirname(os.path.abspath(__file__))))
oit = "--i92" not in sys.argv
if not oit:
    sys.argv.remove("--i92")
name = "rea-oit-i92-render-items-baseline" if oit else "rea-i92-render-items-baseline"
base = os.path.join(root, "oracle", name)
dest = os.path.join(base, f"msaa4-envelope-{VERSION}")
scenes = "synthetic-rea-i92-render-items"
tmp = "/tmp/rea-oit-msaa4-envelope"

def sha(data):
    return hashlib.sha256(data).hexdigest()

def png(size, rgb):
    out = io.BytesIO()
    Image.frombytes("RGB", size, rgb).save(out, format="PNG", optimize=False, compress_level=9)
    return out.getvalue()

def envelope(directory):
    """The files of the envelope of the stored pictures of `directory`, {name: bytes}, after the manifest check."""
    manifest = json.load(open(os.path.join(directory, "manifest.json")))
    files, report = {}, {}
    for world, entry in manifest["worlds"].items():
        data = []
        for capture in entry["captures"]:
            raw = open(os.path.join(directory, capture["file"]), "rb").read()
            if sha(raw) != capture["fileSha256"]:
                sys.exit(f"{capture['file']}: the stored picture is not the one of manifest.json")
            image = Image.open(io.BytesIO(raw))
            if list(image.size) != [entry["width"], entry["height"]]:
                sys.exit(f"{capture['file']}: size {image.size}")
            if sha(image.convert("RGBA").tobytes()) != capture["rgbaSha256"]:
                sys.exit(f"{capture['file']}: the pixels are not the ones of manifest.json")
            data.append(image.convert("RGB").tobytes())
        if len(data) != manifest["runs"] or len(data) < MIN_RUNS:
            sys.exit(f"{world}: {len(data)} stored pictures, manifest.json says {manifest['runs']} runs (at least {MIN_RUNS})")
        size = (entry["width"], entry["height"])
        lo = bytes(min(c) for c in zip(*data))
        hi = bytes(max(c) for c in zip(*data))
        files[world + ".min.png"] = png(size, lo)
        files[world + ".max.png"] = png(size, hi)
        report[world] = {
            "captures": len(data), "distinctCaptures": len(set(data)),
            "differingPixels": sum(1 for p in range(0, len(lo), 3) if lo[p:p + 3] != hi[p:p + 3]),
            "differingChannels": sum(1 for l, h in zip(lo, hi) if l != h),
            "largestSpread": max(h - l for l, h in zip(lo, hi)),
            "minSha256": sha(files[world + ".min.png"]), "maxSha256": sha(files[world + ".max.png"]),
            "minRgbSha256": sha(lo), "maxRgbSha256": sha(hi)}
    files["envelope.json"] = (json.dumps({"version": VERSION, "baseline": name, "generator": "oracle/tools/rea_oit_msaa4_envelope.py build",
                                          "pillow": PIL.__version__, "zlib": zlib.ZLIB_RUNTIME_VERSION, "margin": 0, "worlds": report},
                                         indent=1, sort_keys=True) + "\n").encode()
    return files, report

def build(directory):
    files, report = envelope(directory)
    for file, data in files.items():
        path = os.path.join(directory, file)
        if os.path.exists(path):
            if open(path, "rb").read() != data:
                sys.exit(f"{path} exists and is not what the stored pictures give; an envelope is not overwritten")
        else:
            open(path, "wb").write(data)
    return report

def check():
    first, report = envelope(dest)
    second, _ = envelope(dest)
    if first != second:
        sys.exit("two builds of the envelope differ")
    for file, data in first.items():
        if open(os.path.join(dest, file), "rb").read() != data:
            sys.exit(f"{file}: the stored file is not what the stored pictures give")
    stored = sorted(f for f in os.listdir(dest) if f != "raw")
    if stored != sorted(list(first) + ["manifest.json"]):
        sys.exit(f"{dest} holds {stored}")
    for world, r in report.items():
        print(f"{world}: {r['captures']} stored pictures, {r['distinctCaptures']} distinct, {r['differingPixels']} pixels differ, "
              f"largest spread {r['largestSpread']}")
    print(f"OK: {dest} (every stored picture has its SHA-256; two builds are equal to the stored envelope, byte for byte)")

def record():
    if not os.environ.get("XITE_TESTS"):
        sys.exit(__doc__)
    if not os.path.isdir(os.path.join(root, "oracle", "x_ite-pin", "node_modules", "libtess")):
        sys.exit("oracle/x_ite-pin/node_modules is not the pinned X_ITE's node_modules (oracle/README.md, \"Recording from a worktree\")")
    runs = int(sys.argv[2]) if len(sys.argv) > 2 else 8
    if runs < MIN_RUNS:
        sys.exit(f"at least {MIN_RUNS} runs")
    if os.path.exists(dest):
        sys.exit(f"{dest} exists; a recording is not overwritten (a new recording is a new VERSION)")
    worlds = json.load(open(os.path.join(base, "msaa4", "unstable.json")))
    shutil.rmtree(tmp, ignore_errors=True)
    out = os.path.join(tmp, "out")
    indexes = []
    for i in range(1, runs + 1):
        run = os.path.join(out, "raw", f"run-{i}")
        cmd = ["node", os.path.join(root, "oracle", "x_ite-shot-msaa.mjs"), os.path.join(root, "oracle", scenes), run,
               "--only", ",".join(worlds)] + (["--oit"] if oit else [])
        subprocess.run(cmd, check=True, cwd=root, timeout=1800, stdout=subprocess.DEVNULL)
        indexes.append(json.load(open(os.path.join(run, "index.json"))))
        os.remove(os.path.join(run, "index.json"))
    heads = [{k: v for k, v in ix.items() if k != "results"} for ix in indexes]
    if any(h != heads[0] for h in heads):
        sys.exit(f"the runs do not have one description; nothing written (see {tmp})")
    if bool(heads[0].get("oit")) != oit or str(heads[0].get("ms")) != "4":
        sys.exit(f"the runs say oit {heads[0].get('oit')}, Multisampling {heads[0].get('ms')}; nothing written")
    manifest, bad = {}, []
    for world in worlds:
        results = [next((r for r in ix["results"] if r["file"] == world), {}) for ix in indexes]
        for i, r in enumerate(results):
            errors = [l for l in r.get("log", []) if "error" in l.lower()]
            if not r.get("ok") or errors:
                bad.append(f"{world} run {i + 1}: ok {r.get('ok')}, error {r.get('error')}, log errors {errors}")
        if any(line.startswith(world + " run") for line in bad):
            continue
        scene = open(os.path.join(root, "oracle", scenes, world), "rb").read()
        viewpoints = [l.strip() for l in scene.decode().splitlines() if l.startswith(("Viewpoint", "OrthoViewpoint"))]
        captures = []
        for i, r in enumerate(results):
            file = f"raw/run-{i + 1}/{world}.png"
            raw = open(os.path.join(out, file), "rb").read()
            if sha(raw) != r["pngSha256"]:
                bad.append(f"{file}: not the picture the recorder hashed")
            captures.append({"run": i + 1, "file": file, "fileSha256": sha(raw), "rgbaSha256": r["rgbaSha256"], "log": r.get("log", [])})
        manifest[world] = {"scene": f"oracle/{scenes}/{world}", "sceneSha256": sha(scene), "viewpoint": viewpoints,
                           "width": results[0]["width"], "height": results[0]["height"], "captures": captures}
    if bad:
        sys.exit("\n".join(bad) + f"\nnothing written (see {tmp})")
    json.dump({"version": VERSION, "baseline": name, "recorder": "oracle/x_ite-shot-msaa.mjs" + (" --oit" if oit else ""),
               **heads[0], "oit": oit, "runs": runs, "worlds": manifest}, open(os.path.join(out, "manifest.json"), "w"), indent=1)
    report = build(out)
    for world, r in report.items():
        pixels = manifest[world]["width"] * manifest[world]["height"]
        print(f"{world}: {runs} runs, {r['distinctCaptures']} distinct, {r['differingPixels']} pixels differ, largest spread {r['largestSpread']}")
        if r["largestSpread"] > MAX_SPREAD or r["differingPixels"] > pixels * MAX_DIFFERING:
            bad.append(f"{world}: {r['differingPixels']} of {pixels} pixels differ between runs, largest spread {r['largestSpread']}")
    if bad:
        sys.exit("\n".join(bad) + f"\nnothing written (see {tmp})")
    shutil.copytree(out, dest)
    shutil.rmtree(tmp)
    print(f"wrote {dest}")

command = sys.argv[1:2]
if command == ["record"]:
    record()
elif command == ["build"]:
    build(dest)
    check()
elif command == ["check"]:
    check()
else:
    sys.exit(__doc__)
