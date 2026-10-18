#!/bin/bash
# backup.sh <name>: HEAD, status, tracked patch (incl. binary), untracked tar, SHA256SUMS.
set -euo pipefail
WT=/home/ryan/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup
OUT=/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-08/backup/$1
mkdir -p "$OUT"
git -C "$WT" rev-parse HEAD > "$OUT/HEAD.txt"
git -C "$WT" status --short > "$OUT/status.txt"
git -C "$WT" diff --binary HEAD > "$OUT/tracked.patch"
git -C "$WT" ls-files --others --exclude-standard -z > "$OUT/untracked.list0"
tar -C "$WT" --null -T "$OUT/untracked.list0" -czf "$OUT/untracked.tgz"
( cd "$OUT" && sha256sum HEAD.txt status.txt tracked.patch untracked.list0 untracked.tgz > SHA256SUMS && sha256sum -c SHA256SUMS )
wc -l < "$OUT/status.txt"
