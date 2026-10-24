#!/usr/bin/env python3
"""Two-layer wide LINE particle shadow baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU).

  python3 -I -B oracle/tools/rea_b22_two_layer_shadows.py record NAME [X3D_ORACLE]

runs `x3d-oracle particles` (default target/release/x3d-oracle) three times on
oracle/synthetic-rea-b22-two-layer-shadows/NAME.x3dv with scripts/NAME.json and `--png-dir`, as
rea_b21_wide_lines.py does. The three js.json and js.f32 must be identical and every PNG must decode to the same pixels
(rea_b18_particle_lines.pixels). The first run is kept as oracle/rea-batch22-two-layer-shadows-baseline/NAME/ (js.json,
js.f32, png/). A baseline is never taken from Rust.
"""
import filecmp, os, shutil, subprocess, sys

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
from rea_b18_particle_lines import pixels  # noqa: E402

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))


def main():
    mode, name = sys.argv[1:3]
    assert mode == "record"
    oracle = sys.argv[3] if len(sys.argv) > 3 else os.path.join(root, "target", "release", "x3d-oracle")
    world = os.path.join(root, "oracle", "synthetic-rea-b22-two-layer-shadows", name + ".x3dv")
    script = os.path.join(root, "oracle", "synthetic-rea-b22-two-layer-shadows", "scripts", name + ".json")
    runs = []
    for i in (1, 2, 3):
        out = f"/tmp/rea-b22-two-layer-shadows-record/{name}-{i}"
        shutil.rmtree(out, ignore_errors=True)
        os.makedirs(out)
        subprocess.run([oracle, "particles", world, script, "--out", os.path.join(out, "js.json"),
                        "--png-dir", os.path.join(out, "png")],
                       check=True, cwd=root, timeout=300, stdout=subprocess.DEVNULL)
        runs.append(out)
    a = runs[0]
    pngs = sorted(os.listdir(os.path.join(a, "png")))
    same = bool(pngs) and all(
        filecmp.cmp(os.path.join(a, f), os.path.join(b, f), shallow=False) for b in runs[1:] for f in ("js.json", "js.f32")
    ) and all(
        sorted(os.listdir(os.path.join(b, "png"))) == pngs and
        pixels(os.path.join(a, "png", p)) == pixels(os.path.join(b, "png", p))
        for b in runs[1:] for p in pngs)
    print(f"{name}: three X_ITE runs {'identical' if same else 'DIFFER'} ({len(pngs)} images)")
    if not same:
        sys.exit(1)
    dest = os.path.join(root, "oracle", "rea-batch22-two-layer-shadows-baseline", name)
    shutil.rmtree(dest, ignore_errors=True)
    os.makedirs(dest)
    for f in ("js.json", "js.f32"):
        shutil.copy(os.path.join(a, f), dest)
    shutil.copytree(os.path.join(a, "png"), os.path.join(dest, "png"))


if __name__ == "__main__":
    main()
