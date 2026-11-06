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

The recorders are oracle/x_ite-anim.mjs (MSAA 0, 512x512), oracle/x_ite-anim-sized.mjs (MSAA 0, another script size) and
oracle/x_ite-anim-msaa.mjs (MSAA N), run from the repository root.

Bases with another layout (issue #461) name it; the defaults are the layout above:
  --cases PATH      the case table, relative to oracle/ (default <base>/cases.txt); `name world script [msaa]`, msaa 0 when absent
  --key T           the case key (default {name}-msaa{msaa}); the override is <key>.json
  --json T          the NVIDIA recording of a case, relative to oracle/ (default {base}/{key}/js.json)
  --pictures T      the NVIDIA pictures of a case, relative to oracle/ (e.g. {base}/{key}/js). The pictures are recorded too
                    (--png-dir), must be identical in the three runs, and a picture that differs from the NVIDIA picture in
                    any pixel is kept as <key>.<file> (rea_b21_wide_lines' layout). The JSON override is kept only when the
                    JSON differs.
"""
import datetime, hashlib, json, os, shutil, subprocess, sys, tempfile

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
platform, base, hardware = opt("--platform"), opt("--base"), opt("--hardware", "")
only, names_file = opt("--only"), opt("--names")
cases_t, key_t, json_t, pictures_t = opt("--cases", "{base}/cases.txt"), opt("--key", "{name}-msaa{msaa}"), opt("--json", "{base}/{key}/js.json"), opt("--pictures")
assert platform and base, "--platform and --base are required"
assert os.environ.get("XITE_CHROME") and os.environ.get("XITE_ANGLE_BACKEND"), "set XITE_CHROME and XITE_ANGLE_BACKEND"

src = os.path.join(root, "oracle", base)
dest = os.path.join(root, "oracle", f"{base.removesuffix('-baseline')}-{platform}-baseline")
names = {l.strip() for l in open(names_file)} if names_file else None
rows = [l.split() for l in open(os.path.join(root, "oracle", cases_t.format(base=base))).read().splitlines() if l.strip()]
rows = [r if len(r) > 3 else r + ["0"] for r in rows]
rows = [r for r in rows if (not only or only in r[0]) and (names is None or r[0] in names)]
view = lambda p: {k: v for k, v in json.load(open(p)).items() if k != "loadFrames"}
sha = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest()


def _load_png_pixels():
    import importlib.util
    spec = importlib.util.spec_from_file_location("png_pixels", os.path.join(os.path.dirname(os.path.abspath(__file__)), "png_pixels.py"))
    module = importlib.util.module_from_spec(spec)
    spec.loader.exec_module(module)
    return module.pixels


pixels = _load_png_pixels()
chrome = subprocess.run([os.environ["XITE_CHROME"], "--version"], capture_output=True, text=True).stdout.strip()
pinned = open(os.path.join(root, "oracle", "x_ite-pin", "PINNED_COMMIT")).read().strip()

tmp = tempfile.mkdtemp(prefix="platform-baseline-")
kept, same, unstable, lines, prov = [], [], [], {}, {}

for row in rows:
    name, world, script, msaa = row[0], row[1], row[2], row[3]
    key = key_t.format(name=name, msaa=msaa)
    outs, pngs = [], []
    for i in (1, 2, 3):
        out = os.path.join(tmp, f"{key}-{i}.json")
        # x_ite-anim.mjs draws on the fixed 512x512 canvas of anim.html; a script of another size was recorded with the sized
        # variant (anim-sized.html), as the NVIDIA rea-b01 baselines at 384 and 256 were.
        sized = json.load(open(os.path.join(root, script))).get("size", [512, 512]) != [512, 512]
        anim = "x_ite-anim-msaa.mjs" if msaa != "0" else "x_ite-anim-sized.mjs" if sized else "x_ite-anim.mjs"
        cmd = ["node", os.path.join(root, "oracle", anim), world, script, "--out", out]
        if msaa != "0":
            cmd += ["--ms", msaa]
        if pictures_t:
            pngs.append(os.path.join(tmp, f"{key}-{i}-png"))
            cmd += ["--png-dir", pngs[-1]]
        subprocess.run(cmd, check=True, cwd=root, timeout=300, capture_output=True)
        outs.append(out)
    a, b, c = (view(o) for o in outs)

    if not (a == b == c):
        diff = sorted({k for o in (b, c) for k in set(a) | set(o) if a.get(k) != o.get(k)})
        unstable.append(f"{key}: runs differ in {diff}")
        continue

    nv = view(os.path.join(root, "oracle", json_t.format(base=base, key=key, name=name, msaa=msaa)))
    keys = sorted(k for k in set(a) | set(nv) if a.get(k) != nv.get(k))
    differing_pictures = []

    nv_dir = os.path.join(root, "oracle", pictures_t.format(base=base, key=key, name=name, msaa=msaa)) if pictures_t else None

    # A case without NVIDIA pictures (no js/ directory) compares its JSON only, as the tests do.
    if nv_dir and os.path.isdir(nv_dir):
        files = sorted(f for f in os.listdir(nv_dir) if f.endswith(".png"))
        made = [sorted(f for f in os.listdir(d) if f.endswith(".png")) for d in pngs]
        if any(m != files for m in made):
            unstable.append(f"{key}: the pictures {made[0]} are not the NVIDIA pictures {files}")
            continue
        if any(pixels(os.path.join(pngs[0], f)) != pixels(os.path.join(d, f)) for d in pngs[1:] for f in files):
            unstable.append(f"{key}: the pictures of the runs differ")
            continue
        differing_pictures = [f for f in files if pixels(os.path.join(pngs[0], f)) != pixels(os.path.join(nv_dir, f))]

    if not keys and not differing_pictures:
        same.append(key)
        continue

    os.makedirs(dest, exist_ok=True)
    entry = dict(run_sha256=[sha(o) for o in outs])
    if keys:
        shutil.copyfile(outs[0], os.path.join(dest, key + ".json"))
        entry["differs_from_nvidia_in"] = keys
    for f in differing_pictures:
        shutil.copyfile(os.path.join(pngs[0], f), os.path.join(dest, f"{key}.{f}"))
    if differing_pictures:
        entry["pictures_differ_from_nvidia"] = {f: sha(os.path.join(dest, f"{key}.{f}")) for f in differing_pictures}
    lines[key] = " ".join(row)
    prov[key] = entry
    kept.append(key)

if lines:
    # Merge with an existing table: a later run for other cases must not drop the earlier overrides.
    table = os.path.join(dest, "cases.txt")
    old = {}
    if os.path.exists(table):
        for l in open(table).read().splitlines():
            if l.strip():
                f = l.split(); old[key_t.format(name=f[0], msaa=f[3] if len(f) > 3 else "0")] = l
    old.update(lines)
    open(table, "w").write("".join(old[k] + "\n" for k in sorted(old)))
    pfile = os.path.join(dest, "PROVENANCE.json")
    cur = json.load(open(pfile)) if os.path.exists(pfile) else {"cases": {}}
    cur.update(platform=platform, hardware=hardware, chrome=chrome, angle_backend=os.environ["XITE_ANGLE_BACKEND"],
               pinned_x_ite="16.5.0 " + pinned, recorder="oracle/x_ite-anim.mjs, oracle/x_ite-anim-sized.mjs, oracle/x_ite-anim-msaa.mjs",
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
    print("override", k, prov[k].get("differs_from_nvidia_in", []), sorted(prov[k].get("pictures_differ_from_nvidia", {})))
