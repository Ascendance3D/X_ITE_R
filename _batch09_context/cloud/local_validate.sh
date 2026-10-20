#!/usr/bin/env bash
# REA batch 09 cloud: the 8 tests the cloud container cannot run (owner decision 3, 2026-10-20). Run on the RTX 3060 Ti desktop
# (Linux, Vulkan driver installed: `vulkaninfo --summary` must list the GPU). The cloud cannot run them. Owner accepted the run
# on 0f3de08 (RTX 3060 Ti, 8 passed, 0 failed) as PASS on 2026-10-20; the historical answer keys are kept, not re-recorded.
#
#   bash _batch09_context/cloud/local_validate.sh [--make-baselines]
#
# Memory: every cargo command runs in a systemd scope capped at 8 GB (no swap), one build at a time, 4 jobs.
set -euo pipefail
cd "$(git rev-parse --show-toplevel)"

CAP=(systemd-run --user --scope --quiet -p MemoryMax=8G -p MemorySwapMax=0)
export B09_TARGET="$PWD/target/release"

git fetch origin feature/xite-rust-rea-b09-cloud
git checkout feature/xite-rust-rea-b09-cloud
git merge --ff-only origin/feature/xite-rust-rea-b09-cloud
echo "HEAD $(git rev-parse HEAD)"

# 1. The two gitignored X_ITE answer keys (oracle/README.md "X3/X4"). Source data is in git: oracle/corpus-x1 (500 files) and
#    oracle/corpus-items (606 .wrl + 21 .js + 231 .json). Copy them from the earlier local tree, or, with --make-baselines,
#    record them from the pinned X_ITE (oracle/x_ite-pin @ 34cdf9f97) three times and keep them only if the three are identical.
#    The x1 set cannot record identically: 244-tardis666, 265-tardis666 and 87-weaver_anime_final_fixed2 bake new Date () into
#    the dump. Owner decision 2026-10-20: keep the historical x1 baseline; do not use --make-baselines for it.
#    Expected sha256 prefixes: x1-baseline.json fbe54fa2a0e89f3a, items-baseline.json 8f4bb11593e98f85.
if [[ "${1:-}" == "--make-baselines" ]]; then
  for set in x1 items; do
    for k in 1 2 3; do node oracle/x_ite-dump.mjs "oracle/corpus-$set" "/tmp/$set-baseline.$k.json"; done
    cmp "/tmp/$set-baseline.1.json" "/tmp/$set-baseline.2.json"
    cmp "/tmp/$set-baseline.1.json" "/tmp/$set-baseline.3.json"
    cp "/tmp/$set-baseline.1.json" "oracle/$set-baseline.json"
  done
fi
sha256sum oracle/x1-baseline.json oracle/items-baseline.json

# 2. UDP: rea_batch08_net binds 127.0.0.1 UDP 41234 and 41235 (sender targets, must have NO listener so loopback answers
#    ICMP port unreachable) and 41236 (its own listener). Nothing else may use them, and no firewall rule may drop ICMP on lo.
if ss -lunH | grep -E ':(41234|41235|41236)\b'; then echo "UDP port 41234/41235/41236 in use: free it first"; exit 1; fi

# 3. Build once, then the blocked tests one binary at a time.
"${CAP[@]}" cargo build --release -j 4 -p x3d-script -p x3d-viewer --bins --tests

"${CAP[@]}" cargo test --release -j 4 -p x3d-script --test corpus -- items_runtime_matches_x_ite x1_runtime_matches_x_ite
"${CAP[@]}" cargo test --release -j 4 -p x3d-viewer --test rea_batch09_nav
"${CAP[@]}" cargo test --release -j 4 -p x3d-viewer --test rea_batch09_inputs
"${CAP[@]}" cargo test --release -j 4 -p x3d-viewer --test rea_batch08_net

echo "all 8 environment-blocked tests passed on $(git rev-parse HEAD)"
