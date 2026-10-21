#!/usr/bin/env python3
"""REA batch 09 still-image baselines (pinned X_ITE 16.5.0, headless Chrome on the GPU).

  python3 -I oracle/tools/rea_b09_visual.py shots [--only SUBSTRING]
      oracle/synthetic-rea-b09-visual/*.wrl -> oracle/render-rea-b09-visual-baseline (x_ite-shot.mjs) and
      oracle/render-msaa4-rea-b09-visual-baseline (x_ite-shot-msaa.mjs, 4 samples).

The worlds come from oracle/tools/gen_rea_b09_visual.py. A baseline is written only from an X_ITE run;
the Rust result is compared (crates/x3d-render/tests/render_rea_b09_visual.rs), never copied.
"""
import os, subprocess, sys

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
args = sys.argv[1:]

if args and args[0] == "shots":
    only = args[args.index("--only") + 1] if "--only" in args else None
    for tool, out in [("x_ite-shot.mjs", "render-rea-b09-visual-baseline"), ("x_ite-shot-msaa.mjs", "render-msaa4-rea-b09-visual-baseline")]:
        cmd = ["node", os.path.join(root, "oracle", tool), os.path.join(root, "oracle", "synthetic-rea-b09-visual"), os.path.join(root, "oracle", out)]
        if only:
            cmd += ["--only", only]
        subprocess.run(cmd, check=True, cwd=root, timeout=900)
else:
    sys.exit(__doc__)
