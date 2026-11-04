#!/usr/bin/env python3
"""Platform recordings of the pinned X_ITE 16.5.0 (owner decision 2026-11-04, issues #69 and #89).

The pinned page gives different pointing, collision and touch results on other GPUs and ANGLE backends (the physical
M1 Mac, ANGLE on Metal, differs from the NVIDIA host in pick probes, collision distances and touch text). For a base such
as oracle/rea-b09-nav-baseline this tool records every case again ON THE TARGET MACHINE, three times, and keeps an
override only when

  1. the three recordings are identical (`loadFrames`, which follows Chrome's I/O, is left out of the check), and
  2. they differ from the NVIDIA recording of the same case (the same keys, `loadFrames` left out).

The overrides go to oracle/<stem>-<platform>-baseline/<case>-msaa<N>.json with a cases.txt and a PROVENANCE.json. The NVIDIA
baseline is never written. A case whose three runs differ is listed in UNSTABLE.txt and kept nowhere (UNVERIFIED, never a
pass). A baseline is never taken from Rust. Tests choose the recording with X3D_REFERENCE_PLATFORM=<platform>
(crates/x3d-viewer/tests/common/reference.rs); a case without an override keeps the NVIDIA recording.

  XITE_CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" XITE_ANGLE_BACKEND=metal \\
  XITE_TESTS=<dir whose node_modules holds playwright> \\
  python3 -I -B oracle/tools/record_platform_baseline.py --platform apple-m1 --base rea-b09-nav-baseline \\
      --hardware "Apple M1, macOS 27.0.1" [--only NAME] [--names FILE]

The recorders are oracle/x_ite-anim.mjs (MSAA 0) and oracle/x_ite-anim-msaa.mjs (MSAA N), run from the repository root.
"""
import datetime, hashlib, json, os, shutil, subprocess, sys, tempfile

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
platform, base, hardware = opt("--platform"), opt("--base"), opt("--hardware", "")
only, names_file = opt("--only"), opt("--names")
assert platform and base, "--platform and --base are required"
assert os.environ.get("XITE_CHROME") and os.environ.get("XITE_ANGLE_BACKEND"), "set XITE_CHROME and XITE_ANGLE_BACKEND"

src = os.path.join(root, "oracle", base)
dest = os.path.join(root, "oracle", f"{base.removesuffix('-baseline')}-{platform}-baseline")
names = {l.strip() for l in open(names_file)} if names_file else None
rows = [l.split() for l in open(os.path.join(src, "cases.txt")).read().splitlines() if l.strip()]
rows = [r for r in rows if (not only or only in r[0]) and (names is None or r[0] in names)]
view = lambda p: {k: v for k, v in json.load(open(p)).items() if k != "loadFrames"}
sha = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest()
chrome = subprocess.run([os.environ["XITE_CHROME"], "--version"], capture_output=True, text=True).stdout.strip()
pinned = open(os.path.join(root, "oracle", "x_ite-pin", "PINNED_COMMIT")).read().strip()

tmp = tempfile.mkdtemp(prefix="platform-baseline-")
kept, same, unstable, lines, prov = [], [], [], {}, {}

for row in rows:
    name, world, script, msaa = row[0], row[1], row[2], row[3]
    outs = []
    for i in (1, 2, 3):
        out = os.path.join(tmp, f"{name}-msaa{msaa}-{i}.json")
        cmd = ["node", os.path.join(root, "oracle", "x_ite-anim.mjs" if msaa == "0" else "x_ite-anim-msaa.mjs"), world, script, "--out", out]
        if msaa != "0":
            cmd += ["--ms", msaa]
        subprocess.run(cmd, check=True, cwd=root, timeout=300, capture_output=True)
        outs.append(out)
    a, b, c = (view(o) for o in outs)
    key = f"{name}-msaa{msaa}"

    if not (a == b == c):
        diff = sorted({k for o in (b, c) for k in set(a) | set(o) if a.get(k) != o.get(k)})
        unstable.append(f"{key}: runs differ in {diff}")
        continue

    nv = view(os.path.join(src, key, "js.json"))
    keys = sorted(k for k in set(a) | set(nv) if a.get(k) != nv.get(k))

    if not keys:
        same.append(key)
        continue

    os.makedirs(dest, exist_ok=True)
    shutil.copyfile(outs[0], os.path.join(dest, key + ".json"))
    lines[key] = " ".join(row)
    prov[key] = dict(differs_from_nvidia_in=keys, run_sha256=[sha(o) for o in outs])
    kept.append(key)

if lines:
    # Merge with an existing table: a later run for other cases must not drop the earlier overrides.
    table = os.path.join(dest, "cases.txt")
    old = {}
    if os.path.exists(table):
        for l in open(table).read().splitlines():
            if l.strip():
                f = l.split(); old[f"{f[0]}-msaa{f[3]}"] = l
    old.update(lines)
    open(table, "w").write("".join(old[k] + "\n" for k in sorted(old)))
    pfile = os.path.join(dest, "PROVENANCE.json")
    cur = json.load(open(pfile)) if os.path.exists(pfile) else {"cases": {}}
    cur.update(platform=platform, hardware=hardware, chrome=chrome, angle_backend=os.environ["XITE_ANGLE_BACKEND"],
               pinned_x_ite="16.5.0 " + pinned, recorder="oracle/x_ite-anim.mjs, oracle/x_ite-anim-msaa.mjs",
               recorded=datetime.date.today().isoformat(), rule="3 identical runs; kept only where the NVIDIA recording differs")
    cur["cases"].update(prov)
    json.dump(cur, open(pfile, "w"), indent=1, sort_keys=True)

if unstable:
    with open(os.path.join(dest, "UNSTABLE.txt"), "a") as f:
        f.write("".join(u + "\n" for u in unstable))

shutil.rmtree(tmp, ignore_errors=True)
print(f"{base}/{platform}: {len(rows)} cases recorded three times; {len(kept)} overrides kept, {len(same)} equal to the NVIDIA recording, {len(unstable)} unstable")
for u in unstable:
    print("UNSTABLE", u)
for k in kept:
    print("override", k, prov[k]["differs_from_nvidia_in"])
