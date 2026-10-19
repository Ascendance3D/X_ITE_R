#!/bin/bash
# The eight volume style worlds (initial style, update, clear, assign again, remove the volume from its Group, add it again) compared
# with the pinned X_ITE at MSAA 0 and 4 (scene text and pictures after every step).
cd "$(dirname "$0")/../.." || exit 2
TARGET=${TARGET:-$PWD/target/release}
fail=0
for s in boundary cartoon composed edge opacitymap projection silhouette tonemapped; do
  for m in 0 4; do
    out=/tmp/rem-volume-styles/$s-$m
    rm -rf "$out"; mkdir -p /tmp/rem-volume-styles
    python3 -I oracle/tools/x10_compare.py oracle/synthetic-rea-b07-vol/styles/style-$s.wrl oracle/synthetic-rea-b07-vol/styles/styles.json "$out" --target "$TARGET" $([ $m = 4 ] && echo --msaa 4) > "$out.log" 2>&1
    rc=$?
    echo "style-$s MSAA $m exit $rc: $(grep -E 'within tolerance, ' "$out.log") $(grep -c '^FAIL' "$out.log") FAIL lines"
    [ $rc = 0 ] || fail=1
  done
done
exit $fail
