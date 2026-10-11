#!/usr/bin/env bash
# Nightly build + full oracle check of the committed rust-port HEAD. Local only.
#
# Runs in its own git worktree so it never touches the working tree agents and Ryan use:
#   $BASE/scratch/rust-port   (worktree, detached at main)
#   $BASE/x_ite-tests         (symlink; oracle scripts load playwright via ../../../x_ite-tests)
# Gitignored inputs (big baselines, upstream/, built CM5) are symlinked from the main tree.
# Output: $MAIN/nightly/<UTC date>/{report.txt,logs/,bin/}; keeps the newest 14.
# DRAFT — NOT INSTALLED. Ryan (2026-10-11): nightlies must NOT run on this workstation.
# Once X_ITE and CodeMirror are ported 1:1, this moves to a nightly VM on Ryan's Proxmox VE,
# together with the weekly upstream X_ITE catch-up. Paths below are workstation paths and
# must be adapted for the VM. Do not install a timer or cron job for it here.
set -uo pipefail

MAIN=/home/ryan/Projects/cybertown/X_ITE/scratch/rust-port
LANE=/home/ryan/Projects/cybertown/X_ITE
BASE=/home/ryan/Projects/cybertown/.worktrees/rust-port/nightly
W=$BASE/scratch/rust-port
export CARGO_TARGET_DIR=$MAIN/target-nightly
STAMP=$(date -u +%Y-%m-%d)
OUT=$MAIN/nightly/$STAMP
LOGS=$OUT/logs
JOBS=16

exec 9>"$MAIN/nightly/.lock" 2>/dev/null || { mkdir -p "$MAIN/nightly"; exec 9>"$MAIN/nightly/.lock"; }
flock -n 9 || { echo "another nightly is running"; exit 0; }

mkdir -p "$LOGS" "$OUT/bin"
REPORT=$OUT/report.txt
: > "$REPORT"
FAILED=0

note () { echo "$*" | tee -a "$REPORT"; }

# step <name> <timeout seconds> <command...>  — PASS/FAIL on exit status, log kept per step.
step ()
{
   local name=$1 limit=$2; shift 2
   local start=$SECONDS
   if timeout "$limit" "$@" > "$LOGS/$name.log" 2>&1; then
      note "PASS  $name  ($((SECONDS - start))s)"
   else
      note "FAIL  $name  ($((SECONDS - start))s, exit $?)  -> logs/$name.log"
      FAILED=1
   fi
}

# Like step, but also requires the last lines of the log to contain an expected summary.
expect ()
{
   local name=$1 want=$2
   if ! tail -n 3 "$LOGS/$name.log" | grep -qF -- "$want"; then
      note "FAIL  $name  expected: $want"
      FAILED=1
   fi
}

# Worktree at the newest commit on main.
if [ ! -e "$W/.git" ]; then
   mkdir -p "$BASE/scratch"
   git -C "$MAIN" worktree add --detach "$W" main > "$LOGS/worktree.log" 2>&1
fi
git -C "$W" checkout --detach --force main >> "$LOGS/worktree.log" 2>&1
git -C "$W" clean -fdq >> "$LOGS/worktree.log" 2>&1
ln -sfn "$LANE/x_ite-tests" "$BASE/x_ite-tests"
while read -r rel; do
   rel=${rel%/}
   [ -e "$MAIN/$rel" ] && ln -sfn "$MAIN/$rel" "$W/$rel"
done <<'EOF'
upstream
oracle/items-baseline.json
oracle/x1-baseline.json
oracle/render-items-baseline
oracle/render-synthetic-x8-baseline
oracle/x_ite-text.json
oracle/x_ite-pin/node_modules
oracle/cm5-build/node_modules
oracle/cm5-build/lib/codemirror.js
oracle/cm5-build/addon/runmode/runmode.node.js
oracle/cm5-build/addon/runmode/runmode-standalone.js
oracle/cm5-build/keymap/vim.js
EOF

COMMIT=$(git -C "$W" rev-parse --short HEAD)
note "rust-port nightly $STAMP  commit $COMMIT  $(date -u +%H:%M)Z"
note ""
cd "$W" || exit 1
R=$CARGO_TARGET_DIR/release

step build 3600 cargo build --release --workspace -j $JOBS
step tests 3600 cargo test --release --workspace -j $JOBS

step scene-items 600 "$R/x3d-run" compare oracle/items-baseline.json oracle/corpus-items --known crates/x3d-script/tests/items-known.tsv --logs
step scene-x0    600 "$R/x3d-run" compare oracle/x0-baseline.json oracle/corpus-x0 --known crates/x3d-script/tests/x0-known.tsv --logs
step scene-x1    600 "$R/x3d-run" compare oracle/x1-baseline.json oracle/corpus-x1 --known crates/x3d-script/tests/x1-known.tsv --logs
step scene-x6    600 "$R/x3d-run" compare oracle/synthetic-x6-baseline.json oracle/synthetic-x6 --known crates/x3d-script/tests/synthetic-x6-known.tsv --logs

SHOTS=$OUT/shots; rm -rf "$SHOTS"
step render-items 900 bash -c "'$R/x3d-shot' batch oracle/render-items '$SHOTS/items' && '$R/x3d-shot' compare oracle/render-items-baseline '$SHOTS/items'"
expect render-items "604 within tolerance"
step render-x0 900 bash -c "'$R/x3d-shot' batch oracle/corpus-x0 '$SHOTS/x0' && '$R/x3d-shot' compare oracle/render-x0-baseline '$SHOTS/x0'"
expect render-x0 "46 within tolerance"
step render-x8 900 bash -c "'$R/x3d-shot' batch oracle/synthetic-x8 '$SHOTS/x8' --size 512x512 && '$R/x3d-shot' compare oracle/render-synthetic-x8-baseline '$SHOTS/x8'"
expect render-x8 "125 within tolerance"
rm -rf "$SHOTS"

V=$OUT/viewer
step viewer-click 600 python3 -I oracle/tools/x10_compare.py oracle/render-items/antiques/chronos-orrery/chronos-orrery.wrl oracle/x10-scripts/click-center.json "$V/click" --target "$R"
step viewer-walk  600 python3 -I oracle/tools/x10_compare.py oracle/corpus-x0/43-villasantzip.wrl oracle/x10-scripts/walk-forward.json "$V/walk" --target "$R"
step viewer-examine 600 python3 -I oracle/tools/x10_compare.py oracle/corpus-x0/07-bookcase.wrl oracle/x10-scripts/examine-drag.json "$V/examine" --target "$R"
step viewer-sensors 600 python3 -I oracle/tools/x10_compare.py oracle/x10-scripts/sensors.wrl oracle/x10-scripts/sensors.json "$V/sensors" --target "$R" --text-tol 1e-3
rm -rf "$V"

step cm5-suite 900 node oracle/cm5-run.mjs
expect cm5-suite "All passed"

# Keep the binaries of this night.
for b in x3d-view x3d-shot x3d-run x3d-drive x3d-parse cm-edit cm-render; do
   [ -x "$R/$b" ] && cp "$R/$b" "$OUT/bin/"
done

note ""
if [ $FAILED -eq 0 ]; then note "RESULT: ALL PASS"; else note "RESULT: FAILURES"; fi
ln -sfn "$OUT" "$MAIN/nightly/latest"

# Keep the newest 14 nights.
ls -1d "$MAIN"/nightly/20??-??-?? 2>/dev/null | sort | head -n -14 | xargs -r rm -rf

if [ $FAILED -ne 0 ] && command -v notify-send > /dev/null; then
   notify-send -u critical "rust-port nightly FAILED" "$STAMP $COMMIT — see $REPORT" || true
fi
exit $FAILED
