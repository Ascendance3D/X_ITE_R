#!/bin/bash
# restore.sh <name> <dest>: rebuild the worktree from a backup into <dest>, then compare to the worktree.
set -euo pipefail
WT=/home/ryan/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup
B=/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-08/backup/$1
D=$2
( cd "$B" && sha256sum -c SHA256SUMS )
rm -rf "$D"; mkdir -p "$D"
git -C "$WT" archive "$(cat "$B/HEAD.txt")" | tar -C "$D" -x
( cd "$D" && git init -q && git apply --binary "$B/tracked.patch" )
tar -C "$D" -xzf "$B/untracked.tgz"
rm -rf "$D/.git"
# compare: tracked + untracked (non-ignored) files of the worktree vs restore
( cd "$WT" && { git ls-files -z --cached; git ls-files -z --others --exclude-standard; } ) > /tmp/b08-restore-files.$$
N=0; DIFF=0
while IFS= read -r -d '' f; do
  [ -e "$WT/$f" ] || [ -L "$WT/$f" ] || continue   # deleted in worktree
  N=$((N+1))
  if [ -L "$WT/$f" ]; then [ "$(readlink "$WT/$f")" = "$(readlink "$D/$f" 2>/dev/null)" ] || { echo "DIFF $f"; DIFF=$((DIFF+1)); }
  else cmp -s "$WT/$f" "$D/$f" || { echo "DIFF $f"; DIFF=$((DIFF+1)); }; fi
done < /tmp/b08-restore-files.$$
rm -f /tmp/b08-restore-files.$$
echo "compared=$N differences=$DIFF"
