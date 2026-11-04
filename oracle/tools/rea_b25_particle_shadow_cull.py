#!/usr/bin/env python3
"""ParticleSystem shadow-pass culling baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU).

  python3 -I -B oracle/tools/rea_b25_particle_shadow_cull.py record NAME [X3D_ORACLE]

runs `x3d-oracle particles` (default target/release/x3d-oracle) three times per image pass on
oracle/synthetic-rea-b25-particle-shadow-cull/NAME.x3dv with scripts/NAME.json: `png` (antialiased="false") and `msaa4`
(--ms 4), as rea_b13_particles.py does. Within a pass the three js.json and js.f32 must be identical and every PNG must
decode to the same pixels. The first run of each pass is kept as oracle/rea-b25-particle-shadow-cull-baseline/NAME.js.json
and NAME/ (js.f32, png/, msaa4/). A baseline is never taken from Rust.
"""
import filecmp, os, shutil, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rea_b13_particles import pixels  # noqa: E402

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))


def main():
    mode, name = sys.argv[1:3]
    assert mode == "record"
    oracle = sys.argv[3] if len(sys.argv) > 3 else os.path.join(root, "target", "release", "x3d-oracle")
    world = os.path.join(root, "oracle", "synthetic-rea-b25-particle-shadow-cull", name + ".x3dv")
    script = os.path.join(root, "oracle", "synthetic-rea-b25-particle-shadow-cull", "scripts", name + ".json")
    kept = {}

    for image_pass, extra in (("png", []), ("msaa4", ["--ms", "4"])):
        runs = []
        for i in (1, 2, 3):
            out = f"/tmp/rea-b25-particle-shadow-cull-record/{name}-{image_pass}-{i}"
            shutil.rmtree(out, ignore_errors=True)
            os.makedirs(out)
            subprocess.run([oracle, "particles", world, script, "--out", os.path.join(out, "js.json"),
                            "--png-dir", os.path.join(out, image_pass)] + extra,
                           check=True, cwd=root, timeout=300, stdout=subprocess.DEVNULL)
            runs.append(out)
        a = runs[0]
        pngs = sorted(os.listdir(os.path.join(a, image_pass)))
        same = bool(pngs) and all(
            filecmp.cmp(os.path.join(a, f), os.path.join(b, f), shallow=False) for b in runs[1:] for f in ("js.json", "js.f32")
        ) and all(
            sorted(os.listdir(os.path.join(b, image_pass))) == pngs and
            pixels(os.path.join(a, image_pass, p)) == pixels(os.path.join(b, image_pass, p))
            for b in runs[1:] for p in pngs)
        print(f"{name} {image_pass}: three X_ITE runs {'identical' if same else 'DIFFER'} ({len(pngs)} images)")
        if not same:
            sys.exit(1)
        kept[image_pass] = a

    if not all(filecmp.cmp(os.path.join(kept["png"], f), os.path.join(kept["msaa4"], f), shallow=False) for f in ("js.json", "js.f32")):
        print(f"{name}: records differ between the png and msaa4 passes")
        sys.exit(1)

    dest = os.path.join(root, "oracle", "rea-b25-particle-shadow-cull-baseline", name)
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest)
    if os.path.exists(dest + ".js.json"):
        os.remove(dest + ".js.json")
    shutil.copy(os.path.join(kept["png"], "js.f32"), dest)
    # NAME.js.json sits beside NAME/ so the Review gate's one-level baseline exclusion covers it.
    shutil.copy(os.path.join(kept["png"], "js.json"), dest + ".js.json")
    for image_pass, src in kept.items():
        shutil.copytree(os.path.join(src, image_pass), os.path.join(dest, image_pass))


if __name__ == "__main__":
    main()
