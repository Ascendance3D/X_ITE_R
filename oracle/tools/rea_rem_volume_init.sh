#!/bin/bash
# Initialization-order comparisons of IsoSurfaceVolumeData / SegmentedVolumeData `voxels` against the pinned
# X_ITE: a write by a ROUTE from a Script placed before / after the node, and a direct Script write, all
# before the first draw. Each world is compared at MSAA 0 and 4 (scene text at frames 10, 12, 20, 24 and the
# pictures at 12 and 20).   TARGET=<release dir> bash oracle/tools/rea_rem_volume_init.sh [log]
cd "$(dirname "$0")/../.." || exit 2
TARGET=${TARGET:-$PWD/target/release}
fail=0
for k in iso seg; do
  for v in route-before route-after direct-after; do
    for m in 0 4; do
      n=init-$k-$v
      out=/tmp/rem-volume-init/$n-$m
      rm -rf "$out"
      python3 -I oracle/tools/x10_compare.py oracle/synthetic-rea-b07-vol/init/$n.wrl oracle/synthetic-rea-b07-vol/init/$n.json "$out" --target "$TARGET" $([ $m = 4 ] && echo --msaa 4) > "$out.log" 2>&1
      rc=$?
      echo "$n MSAA $m exit $rc: $(grep -E 'within tolerance, ' "$out.log")"
      [ $rc = 0 ] || fail=1
    done
  done
done
exit $fail
