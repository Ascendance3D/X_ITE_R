# Batch 08 suites, sourced by run.sh ($G, $R set there; cwd = worktree). Written 2026-10-18 (cloud recovery session).
t() { local name=$1; shift; cargo test --release -j 12 "$@" -- --nocapture > $G/$name.log 2>&1; echo "$name exit $? : $(grep -E 'shots checked|cases checked|^test result' $G/$name.log | tr '\n' ' ')" >> $G/summary.txt; }
# physics
t b08-physics-engine -p x3d-runtime --test bullet_box_box --test bullet_dyn_scenes --test bullet_b08_engine --test physics_b08_lifecycle
# shaders (rea_batch08_shaders needs oracle/rea-batch08-shaders-baseline, made by oracle/tools/rea_b08_shaders.py case)
t b08-shaders-glsl -p x3d-render --test glsl_compat_b08
t b08-shaders-render -p x3d-render --test render_rea_b08_shaders
t b08-shaders-viewer -p x3d-viewer --test rea_batch08_shaders
t b08-shaders-reload -p x3d-viewer --test rea_b08_shaders_reload
# gltf
t b08-gltf-parse -p x3d-parse --test gltf_rea
t b08-gltf-render -p x3d-render --test render_rea_b08_gltf
t b08-gltf-inline -p x3d-script --test gltf_inline_b08
t b08-gltf-reload -p x3d-viewer --test rea_b08_gltf_reload --test rea_b08_gltf_viewer_buffers
# render2 (GaussianSplats, VolumeScatter)
t b08-gs-render -p x3d-render --test render_rea_b08_gs
t b08-vs-render -p x3d-render --test render_rea_b08_vs
t b08-gs-vs-viewer -p x3d-viewer --test rea_batch08_gs --test rea_batch08_vs --test rea_b08_gs_reload --test rea_b08_vs_reload
# net
t b08-net-render -p x3d-render --test render_rea_b08_net
t b08-net-viewer -p x3d-viewer --test rea_batch08_net --test rea_b08_net_reload
t b08-net-unload -p x3d-script --test rea_b08_net_unload
# svg / html
t b08-svg-parse -p x3d-parse --test svg_rea --test html_support_rea --test css_colors_b08
t b08-svg-render -p x3d-render --test render_rea_b08_svg
cargo test --release -j 12 -p x3d-viewer --test svg_b08_lifecycle -- --nocapture --test-threads=1 > $G/b08-svg-lifecycle.log 2>&1; echo "b08-svg-lifecycle exit $? : $(grep '^test result' $G/b08-svg-lifecycle.log | tr '\n' ' ')" >> $G/summary.txt
# multiview (x_ite_two_canvases_rigid_bodies_msaa4 is expected to fail at 0.579% > 0.5%: owner decision, entry PARTIAL)
t b08-multiview -p x3d-viewer --test multi_viewer_b08
# integration (recovery session)
t b08-release-tables -p x3d-viewer --test rea_b08_release_tables
# fresh 20000-case pick sweep (physics residue: expected 3 differences, cases 4697 5480 11878)
node oracle/tools/ammo_pick_vectors.mjs $G/pick20000.json 20000 20261018 > $G/b08-pick-gen.log 2>&1 && PICK_VECTORS=$G/pick20000.json cargo test --release -j 12 -p x3d-viewer --test rea_b03_pick_vectors -- --nocapture > $G/b08-pick20000.log 2>&1; echo "b08-pick20000 exit $? : $(grep -E 'cases, |^test result' $G/b08-pick20000.log | tr '\n' ' ')" >> $G/summary.txt
