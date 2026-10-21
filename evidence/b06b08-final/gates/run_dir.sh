#!/bin/bash
# usage: run_rate.sh DIR(rates/rN or other) baseline|check [names...]
WT=/home/ryan/Projects/cybertown/.worktrees/rust-port-private/integration-b06-b08
R=/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-06r-qa-closure/physics
D=$1; shift; M=$1; shift
export B06_ORACLE=/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-06r-qa-closure/physics B06_ANIM=/tmp/b06r-closure/physics/serve/a/b/oracle/x_ite-anim.mjs B06_WORLDS=/tmp/b06r-closure/physics/serve/a/b/oracle/$(basename $(dirname $D))/$(basename $D)/worlds B06_BASE=$D/baseline B06_TMP=/tmp/b06r-closure/physics/$(basename $(dirname $D))-$(basename $D)
mkdir -p $OUTD
cd $WT
if [ $M = baseline ]; then
  python3 -I $R/tools/rea_b06_closure.py baseline "$@" --jobs ${JOBS:-12} 2>&1 | tee $OUTD/baseline.log
else
  python3 -I $R/tools/rea_b06_closure.py check "$@" --target $WT/target/release --jobs ${JOBS:-12} --json $OUTD/check.json 2>&1 | tee $OUTD/check.log
fi
