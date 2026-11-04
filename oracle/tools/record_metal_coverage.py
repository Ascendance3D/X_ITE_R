#!/usr/bin/env python3
"""Records the Metal coverage worlds (oracle/synthetic-metal-coverage, made by gen_metal_coverage.py) with the pinned X_ITE
16.5.0 ON THE TARGET MACHINE, three times each (issues #69 and #89).

Unlike record_platform_baseline.py there is no NVIDIA recording to differ from: these worlds exist to measure the rasterizer of the
target (how a 1 px line and a point sprite cover pixel centres), so every case that is stable is kept. A case is kept only if the
three recordings are identical (`loadFrames`, which follows Chrome's I/O, is left out of the check); otherwise it is listed in
UNSTABLE.txt and kept nowhere (UNVERIFIED, never a pass). A baseline is never taken from Rust.

Writes oracle/metal-coverage-<platform>-baseline/<case>-msaa<N>.json (flat: one directory level, as the review gate expects),
cases.txt and PROVENANCE.json. The pointing buffer is not multisampled, so the cases are recorded at MSAA 0; `--msaa 4 --only NAME`
adds the Multisampling 4 recording of a case, which shows that the probes do not depend on it.

  XITE_CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" XITE_ANGLE_BACKEND=metal \\
  XITE_TESTS=<dir whose node_modules holds playwright> \\
  python3 -I -B oracle/tools/record_metal_coverage.py --platform apple-m1 --hardware "Apple M1, macOS 27.0.1" [--msaa 4] [--only NAME]
"""
import datetime, hashlib, json, os, shutil, subprocess, sys, tempfile

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
platform, hardware, only, msaa = opt("--platform"), opt("--hardware", ""), opt("--only"), opt("--msaa", "0")
assert platform, "--platform is required"
assert os.environ.get("XITE_CHROME") and os.environ.get("XITE_ANGLE_BACKEND"), "set XITE_CHROME and XITE_ANGLE_BACKEND"

synth = os.path.join(root, "oracle", "synthetic-metal-coverage")
dest = os.path.join(root, "oracle", f"metal-coverage-{platform}-baseline")
names = sorted(f[:-4] for f in os.listdir(synth) if f.endswith(".wrl") and (not only or only in f))
view = lambda p: {k: v for k, v in json.load(open(p)).items() if k != "loadFrames"}
sha = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest()
chrome = subprocess.run([os.environ["XITE_CHROME"], "--version"], capture_output=True, text=True).stdout.strip()
pinned = open(os.path.join(root, "oracle", "x_ite-pin", "PINNED_COMMIT")).read().strip()
tmp = tempfile.mkdtemp(prefix="metal-coverage-")
os.makedirs(dest, exist_ok=True)
kept, unstable, lines, prov = [], [], {}, {}

for name in names:
    world, script = f"oracle/synthetic-metal-coverage/{name}.wrl", f"oracle/synthetic-metal-coverage/{name}.json"
    outs = []
    for i in (1, 2, 3):
        out = os.path.join(tmp, f"{name}-msaa{msaa}-{i}.json")
        cmd = ["node", os.path.join(root, "oracle", "x_ite-anim.mjs" if msaa == "0" else "x_ite-anim-msaa.mjs"), world, script, "--out", out]
        if msaa != "0":
            cmd += ["--ms", msaa]
        subprocess.run(cmd, check=True, cwd=root, timeout=600, capture_output=True)
        outs.append(out)
    key = f"{name}-msaa{msaa}"
    a, b, c = (view(o) for o in outs)
    if not (a == b == c):
        unstable.append(f"{key}: runs differ in {sorted({k for o in (b, c) for k in set(a) | set(o) if a.get(k) != o.get(k)})}")
        continue
    shutil.copyfile(outs[0], os.path.join(dest, key + ".json"))
    lines[key] = f"{name} {world} {script} {msaa}"
    prov[key] = dict(run_sha256=[sha(o) for o in outs], probes=len(a["pickProbes"]))
    kept.append(key)

if lines:
    table, old = os.path.join(dest, "cases.txt"), {}
    if os.path.exists(table):
        for l in open(table).read().splitlines():
            if l.strip():
                f = l.split()
                old[f"{f[0]}-msaa{f[3]}"] = l
    old.update(lines)
    open(table, "w").write("".join(old[k] + "\n" for k in sorted(old)))
    pfile = os.path.join(dest, "PROVENANCE.json")
    cur = json.load(open(pfile)) if os.path.exists(pfile) else {"cases": {}}
    cur.update(platform=platform, hardware=hardware, chrome=chrome, angle_backend=os.environ["XITE_ANGLE_BACKEND"],
               pinned_x_ite="16.5.0 " + pinned, recorder="oracle/x_ite-anim.mjs, oracle/x_ite-anim-msaa.mjs",
               generator="oracle/tools/gen_metal_coverage.py", recorded=datetime.date.today().isoformat(),
               rule="3 identical runs (loadFrames left out)")
    cur["cases"].update(prov)
    json.dump(cur, open(pfile, "w"), indent=1, sort_keys=True)

if unstable:
    with open(os.path.join(dest, "UNSTABLE.txt"), "a") as f:
        f.write("".join(u + "\n" for u in unstable))

shutil.rmtree(tmp, ignore_errors=True)
print(f"metal-coverage/{platform} MSAA {msaa}: {len(names)} cases recorded three times; {len(kept)} kept, {len(unstable)} unstable")
for u in unstable:
    print("UNSTABLE", u)
