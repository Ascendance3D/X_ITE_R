#!/usr/bin/env bash
# REA batch 08 gates (batch 05 runner + batch 06 physics/particle suites), one heavy job at a time. Output: (gates dir)/*.log, summary.txt
cd /home/ryan/Projects/cybertown/.worktrees/rust-port-private/integration-b06-b08 || exit 1
G=/home/ryan/Projects/cybertown/.worktrees/rust-port-private/integration-b06-b08/evidence/b06b08-rem/gates
R=/home/ryan/Projects/cybertown/.worktrees/rust-port-private/integration-b06-b08/target/release
: > $G/summary.txt
cargo build --release --workspace --locked -j 12 > $G/build.log 2>&1; echo "release build exit $?" >> $G/summary.txt
cargo test --release --workspace --no-fail-fast --locked -j 12 > $G/workspace.log 2>&1; echo "workspace exit $? : $(grep -c '^test result: ok' $G/workspace.log) ok groups, $(grep -c 'FAILED' $G/workspace.log) FAILED lines" >> $G/summary.txt
bash crates/r0-2-c-tests/future-gate.sh > $G/future-gate.log 2>&1; echo "future-gate exit $? : $(tail -n 1 $G/future-gate.log)" >> $G/summary.txt
cargo test -p x3d-script --test extern_reload --test inline_reload -j 12 > $G/reload.log 2>&1; echo "reload exit $? : $(grep '^test result' $G/reload.log | tr '\n' ' ')" >> $G/summary.txt
c() { $R/x3d-run compare oracle/$1-baseline.json oracle/$2 --logs --known crates/x3d-script/tests/$1-known.tsv > $G/$3.log 2>&1; echo "$3 exit $? : $(tail -n 2 $G/$3.log | tr '\n' ' ')" >> $G/summary.txt; }
c items2 corpus-items2 items2
c items corpus-items items
c x0 corpus-x0 x0
c x1 corpus-x1 x1
c synthetic-inline-scripts synthetic-inline-scripts sis
c synthetic-x6 synthetic-x6 x6
c synthetic-externproto synthetic-externproto ep
python3 -I oracle/tools/run_followers_suite.py --target $R > $G/followers.log 2>&1; echo "followers exit $? : $(tail -n 2 $G/followers.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-parse --test formats_rea --test optimizer_rea -- --nocapture > $G/rea-parse.log 2>&1; echo "rea parse exit $? : $(grep '^test result' $G/rea-parse.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-render --test render_rea2 -- --nocapture > $G/rea-render.log 2>&1; echo "rea render exit $? : $(grep -E 'shots checked|^test result' $G/rea-render.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test rea_batch01 -- --nocapture > $G/rea-viewer.log 2>&1; echo "rea viewer exit $? : $(grep -E 'cases checked|^test result' $G/rea-viewer.log | tr '\n' ' ')" >> $G/summary.txt
rm -rf $G/gpu0 $G/gpu4; mkdir -p $G/gpu0 $G/gpu4
$R/x3d-shot batch oracle/render-items2 $G/gpu0 > $G/gpu0-batch.log 2>&1; $R/x3d-shot compare oracle/render-items2-baseline $G/gpu0 > $G/gpu0-compare.txt 2>&1; echo "gpu msaa0 : $(tail -n 1 $G/gpu0-compare.txt)" >> $G/summary.txt
$R/x3d-shot batch oracle/render-items2 $G/gpu4 --msaa 4 --gl-samples > $G/gpu4-batch.log 2>&1; $R/x3d-shot compare oracle/render-msaa4-items2-baseline $G/gpu4 > $G/gpu4-compare.txt 2>&1; echo "gpu msaa4 : $(tail -n 1 $G/gpu4-compare.txt)" >> $G/summary.txt
cargo test --release -j 12 -p x3d-script --test retention_tables -- --nocapture > $G/memory-runtime.log 2>&1; echo "memory runtime exit $? : $(grep '^test result' $G/memory-runtime.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test retention_viewer -- --nocapture > $G/memory-viewer.log 2>&1; echo "memory viewer exit $? : $(grep '^test result' $G/memory-viewer.log | tr '\n' ' ')" >> $G/summary.txt

cargo test --release -j 12 -p x3d-render --test render_rea_b02 -- --nocapture > $G/b02-render.log 2>&1; echo "b02 render exit $? : $(grep -E 'shots checked|^test result' $G/b02-render.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test rea_batch02 -- --nocapture > $G/b02-viewer.log 2>&1; echo "b02 viewer exit $? : $(grep -E 'cases checked|^test result' $G/b02-viewer.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test rea_b02_reload --test rea_b02_layers --test pointer_slot_reuse -- --nocapture > $G/b02-reload.log 2>&1; echo "b02 reload/layers/slots exit $? : $(grep '^test result' $G/b02-reload.log | tr '\n' ' ')" >> $G/summary.txt

cargo test --release -j 12 -p x3d-render --test render_rea_b03 -- --nocapture > $G/b03-render.log 2>&1; echo "b03 render exit $? : $(grep -E 'shots checked|^test result' $G/b03-render.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test rea_batch03 -- --nocapture > $G/b03-viewer.log 2>&1; echo "b03 viewer exit $? : $(grep -E 'cases checked|^test result' $G/b03-viewer.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test rea_b03_pick_vectors -- --nocapture > $G/b03-pick.log 2>&1; echo "b03 pick vectors exit $? : $(grep -E 'cases, |^test result' $G/b03-pick.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test rea_b03_reload --test rea_b03_render_reload -- --nocapture > $G/b03-reload.log 2>&1; echo "b03 reload exit $? : $(grep '^test result' $G/b03-reload.log | tr '\n' ' ')" >> $G/summary.txt

cargo test --release -j 12 -p x3d-render --test rea_b04_geometry --test rea_b04_nurbs_iso -- --nocapture > $G/b04-geometry.log 2>&1; echo "b04 geometry/iso exit $? : $(grep '^test result' $G/b04-geometry.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-render --test render_rea_b04 -- --nocapture > $G/b04-render.log 2>&1; echo "b04 render exit $? : $(grep -E 'shots checked|^test result' $G/b04-render.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test rea_batch04 -- --nocapture > $G/b04-viewer.log 2>&1; echo "b04 viewer exit $? : $(grep -E 'cases checked|^test result' $G/b04-viewer.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test rea_b04_reload --test rea_b04_render_reload -- --nocapture > $G/b04-reload.log 2>&1; echo "b04 reload exit $? : $(grep '^test result' $G/b04-reload.log | tr '\n' ' ')" >> $G/summary.txt

python3 -I oracle/tools/rea_b05.py check --jobs 8 --json $G/b05-sound.json > $G/b05-sound.log 2>&1; echo "b05 sound exit $? : $(tail -n 1 $G/b05-sound.log)" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test rea_b05_reload -- --nocapture > $G/b05-reload.log 2>&1; echo "b05 reload exit $? : $(grep '^test result' $G/b05-reload.log | tr '\n' ' ')" >> $G/summary.txt

python3 -I oracle/tools/rea_b06.py check --target $R --jobs 12 --json $G/b06-physics.json > $G/b06-physics.log 2>&1; echo "b06 physics oracle exit $? : $(tail -n 1 $G/b06-physics.log)" >> $G/summary.txt
python3 -I oracle/tools/rea_b06_particles.py check --driver $R/x3d-particles-drive --jobs 12 --json $G/b06-particles.json > $G/b06-particles.log 2>&1; echo "b06 particles oracle exit $? : $(tail -n 1 $G/b06-particles.log)" >> $G/summary.txt
cargo test --release -j 12 -p x3d-runtime --test bullet_dyn_scenes -- --nocapture > $G/b06-engine.log 2>&1; echo "b06 engine vs Ammo exit $? : $(grep '^test result' $G/b06-engine.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test physics_b06_baseline --test physics_b06_lifecycle -- --nocapture > $G/b06-physics-tests.log 2>&1; echo "b06 physics scene+lifecycle exit $? : $(grep '^test result' $G/b06-physics-tests.log | tr '\n' ' ')" >> $G/summary.txt
cargo test --release -j 12 -p x3d-viewer --test particles_b06_exact --test particles_b06_stat --test particles_b06_events --test particles_b06_retention -- --nocapture > $G/b06-particles-tests.log 2>&1; echo "b06 particles scene+retention exit $? : $(grep '^test result' $G/b06-particles-tests.log | tr '\n' ' ')" >> $G/summary.txt
P=/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-06r-qa-closure/physics
for r in r24 r30 r50 r60 r75 r120 r144; do
  JOBS=12 OUTD=$G/logs-stacks-$r bash $G/run_dir.sh $P/stacks/$r check > $G/stacks-$r.log 2>&1; echo "stacks $r exit $? : $(tail -n 1 $G/stacks-$r.log)" >> $G/summary.txt
done
for r in r24 r30 r50 r75 r120 r144; do
  JOBS=12 OUTD=$G/logs-rates-$r bash $G/run_dir.sh $P/rates/$r check > $G/rates-$r.log 2>&1; echo "rates $r exit $? : $(tail -n 1 $G/rates-$r.log)" >> $G/summary.txt
done
# Batch 07 suites (B07_EXTRA is filled from the agents' reports before the run)
source $G/b07-extra.sh
[ -f $G/b08-extra.sh ] && source $G/b08-extra.sh
echo DONE >> $G/summary.txt
