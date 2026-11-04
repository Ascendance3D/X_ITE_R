#!/usr/bin/env python3
"""Platform recordings of the pinned X_ITE for the ParticleSystem pointing and collision tests (rea_b14, rea_b15, rea_b21).

Like record_platform_baseline.py (read it for the rules), for the baselines recorded by `x3d-oracle particles`: every case of the base is
recorded three times ON THE TARGET MACHINE; `js.json` and `js.f32` of the three runs must be identical (else the case is listed in
UNSTABLE.txt, UNVERIFIED, and kept nowhere); a recording is kept only where it differs from the NVIDIA one, as
oracle/<stem>-<platform>-baseline/<case>.json and <case>.f32 (flat: one directory level, as the review gate expects). The NVIDIA baseline is never written; a baseline is never taken from Rust. Tests choose it with
X3D_REFERENCE_PLATFORM=<platform> (crates/x3d-viewer/tests/common/reference.rs, `pair_dir`).

  X3D_ORACLE_CHROME="/Applications/Google Chrome.app/Contents/MacOS/Google Chrome" X3D_ORACLE_ANGLE=metal \\
  python3 -I -B oracle/tools/record_platform_particles.py --platform apple-m1 --base rea-b14-particle-pick-baseline \\
      --hardware "Apple M1, macOS 27.0.1" [--oracle target/release/x3d-oracle] [--only NAME]
"""
import datetime, filecmp, hashlib, json, os, shutil, subprocess, sys, tempfile

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rea_b18_particle_lines import pixels  # noqa: E402


def over_8(a, b):
    """The picture rule of rea_b18_particle_lines / rea_b21_wide_lines: pixels with a channel more than 8 apart, of all pixels."""
    (w, h, bpp, x), (w2, h2, bpp2, y) = pixels(a), pixels(b)
    if (w, h, bpp) != (w2, h2, bpp2):
        return 1.0
    n = sum(1 for i in range(0, len(x), bpp) if any(abs(x[i + c] - y[i + c]) > 8 for c in range(3)))
    return n / (w * h)


root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]
opt = lambda n, d=None: args[args.index(n) + 1] if n in args else d
platform, base, hardware, only = opt("--platform"), opt("--base"), opt("--hardware", ""), opt("--only")
oracle = opt("--oracle", os.path.join(root, "target", "release", "x3d-oracle"))
SYNTH = {"rea-b14-particle-pick-baseline": "synthetic-rea-b14-particle-pick", "rea-b15-particle-collision-baseline": "synthetic-rea-b15-particle-collision",
         "rea-b21-wide-lines-baseline": "synthetic-rea-b21-wide-lines"}
assert platform and base in SYNTH, "--platform and --base (one of %s) are required" % sorted(SYNTH)
assert os.environ.get("X3D_ORACLE_CHROME") and os.environ.get("X3D_ORACLE_ANGLE"), "set X3D_ORACLE_CHROME and X3D_ORACLE_ANGLE"

src, dest = os.path.join(root, "oracle", base), os.path.join(root, "oracle", f"{base.removesuffix('-baseline')}-{platform}-baseline")
synth = os.path.join(root, "oracle", SYNTH[base])
names = sorted(n for n in os.listdir(src) if os.path.isdir(os.path.join(src, n)) and (not only or only in n))
sha = lambda p: hashlib.sha256(open(p, "rb").read()).hexdigest()
tmp = tempfile.mkdtemp(prefix="platform-particles-")
kept, same, unstable, prov, pictures = [], [], [], {}, []

for name in names:
    world, script = os.path.join(synth, name + ".x3dv"), os.path.join(synth, "scripts", name + ".json")
    runs = []
    for i in (1, 2, 3):
        out = os.path.join(tmp, f"{name}-{i}")
        os.makedirs(out)
        subprocess.run([oracle, "particles", world, script, "--out", os.path.join(out, "js.json"), "--png-dir", os.path.join(out, "png")],
                       check=True, cwd=root, timeout=300, stdout=subprocess.DEVNULL)
        runs.append(out)
    if not all(filecmp.cmp(os.path.join(runs[0], f), os.path.join(b, f), shallow=False) for b in runs[1:] for f in ("js.json", "js.f32")):
        unstable.append(f"{name}: the three runs differ")
        continue
    # The pictures (the png/ directory of the case, wide-line cases only): the platform picture of a frame is kept, as
    # <case>.<frame>.png beside the json, only when its three runs decode to the same pixels and it differs from the NVIDIA picture by
    # more than the test's rule (0.5% of the pixels more than 8 apart). The test then compares Rust with it instead.
    if os.path.isdir(os.path.join(src, name, "png")):
        for png in sorted(os.listdir(os.path.join(runs[0], "png"))):
            mine = os.path.join(runs[0], "png", png)
            if not all(pixels(mine) == pixels(os.path.join(b, "png", png)) for b in runs[1:]):
                unstable.append(f"{name}/{png}: the three pictures differ")
            elif over_8(os.path.join(src, name, "png", png), mine) > 0.005:
                os.makedirs(dest, exist_ok=True)
                shutil.copyfile(mine, os.path.join(dest, f"{name}.{png}"))
                prov.setdefault(name, {})[png] = sha(mine)
                pictures.append(f"{name}.{png}")
    if all(filecmp.cmp(os.path.join(runs[0], f), os.path.join(src, name, f), shallow=False) for f in ("js.json", "js.f32")):
        same.append(name)
        continue
    os.makedirs(dest, exist_ok=True)
    for f in ("js.json", "js.f32"):
        shutil.copyfile(os.path.join(runs[0], f), os.path.join(dest, name + os.path.splitext(f)[1]))
    prov.setdefault(name, {}).update({f: sha(os.path.join(runs[0], f)) for f in ("js.json", "js.f32")})
    kept.append(name)

if kept or pictures:
    pfile = os.path.join(dest, "PROVENANCE.json")
    cur = json.load(open(pfile)) if os.path.exists(pfile) else {"cases": {}}
    pinned = open(os.path.join(root, "oracle", "x_ite-pin", "PINNED_COMMIT")).read().strip()
    chrome = subprocess.run([os.environ["X3D_ORACLE_CHROME"], "--version"], capture_output=True, text=True).stdout.strip()
    cur.update(platform=platform, hardware=hardware, chrome=chrome, angle_backend=os.environ["X3D_ORACLE_ANGLE"], pinned_x_ite="16.5.0 " + pinned,
               recorder="x3d-oracle particles", recorded=datetime.date.today().isoformat(),
               rule="3 identical runs; kept only where the NVIDIA recording differs (a picture: where it differs by more than 0.5% of the pixels over 8)")
    cur["cases"].update(prov)
    json.dump(cur, open(pfile, "w"), indent=1, sort_keys=True)
if unstable:
    os.makedirs(dest, exist_ok=True)
    with open(os.path.join(dest, "UNSTABLE.txt"), "a") as f:
        f.write("".join(u + "\n" for u in unstable))
shutil.rmtree(tmp, ignore_errors=True)
print(f"{base}/{platform}: {len(names)} cases recorded three times; {len(kept)} overrides kept, {len(same)} equal to the NVIDIA recording, {len(unstable)} unstable")
for u in unstable:
    print("UNSTABLE", u)
for k in kept:
    print("override", k)
for k in pictures:
    print("picture", k)
