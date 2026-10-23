#!/usr/bin/env python3
"""Regenerates src/page_js.rs and src/particles_js.rs: the page-side JavaScript of the Node oracle tools, copied
byte for byte so x3d-oracle sends the browser exactly what oracle/x_ite-anim*.mjs and oracle/tools/x_ite_particles*.mjs send.

  python3 -I crates/x3d-oracle/tools/gen_page_js.py      (from the repository root)
"""
import os, subprocess, sys
here = os.path.dirname(os.path.abspath(__file__))
root = os.path.normpath(os.path.join(here, "..", "..", ".."))
src = os.path.join(here, "..", "src")
subprocess.run([sys.executable, "-I", os.path.join(here, "gen_anim_js.py"), os.path.join(root, "oracle", "x_ite-anim.mjs"), os.path.join(src, "page_js.rs"), os.path.join(root, "oracle", "x_ite-anim-msaa.mjs")], check=True)
subprocess.run([sys.executable, "-I", os.path.join(here, "gen_particles_js.py"), root, os.path.join(src, "particles_js.rs")], check=True)
