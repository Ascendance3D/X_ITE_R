#!/usr/bin/env bash
# Batch 08: the steps only the owner's machine can do (GPU, pinned X_ITE in Chrome, local baselines, backups).
# Written 2026-10-18 by the cloud recovery session. Run each stage on its own and read its output first:
#
#   FINAL=<recovery commit sha> bash local_finish.sh verify     # live worktree == snapshot a91f0a4 (read only)
#   FINAL=<sha>                 bash local_finish.sh apply      # pre-recovery backup, then apply the recovery diff
#                               bash local_finish.sh shaders    # stepped shader baselines from pinned X_ITE
#                               bash local_finish.sh gates      # run.sh + b08-extra.sh on the final source
#                               bash local_finish.sh ledger     # check the ledger after set_status.py
#                               bash local_finish.sh backup     # final backup + real restore comparison
#
# Nothing here commits, pushes, stashes, resets or cleans. The worktree is changed only by `apply`.
set -uo pipefail

WT=${WT:-/home/ryan/Projects/cybertown/.worktrees/rust-port-private/r0-3-e-retention-cleanup}
RPT=${RPT:-/home/ryan/Projects/cybertown/.reports/xite-rust-rea-batch-08}
STAGE=${STAGE:-$HOME/x_ite_r_stage}
REMOTE=${REMOTE:-x_ite_r}
SNAP=a91f0a48a1fd45a03cb6b5a662824e7b5b5b2520
LEDGER=${LEDGER:-/home/ryan/Projects/cybertown/.reports/xite-rust-full-port-rea-completion/ledger.json}
# Files of the live worktree that were kept out of the snapshot commit (over 50 MB; sha256 in _batch08_context/large_backups.sha256).
KEPT_OUT='evidence/final-backup/untracked.tgz|evidence/start-backup/untracked.tgz'

worktree_files() { ( cd "$WT" && { git ls-files -z --cached; git ls-files -z --others --exclude-standard; } | tr '\0' '\n' | sort -u | while IFS= read -r f; do { [ -e "$f" ] || [ -L "$f" ]; } && echo "$f"; done ); }

case "${1:-}" in
verify)
    : "${FINAL:?set FINAL to the recovery commit sha}"
    [ "$(git -C "$WT" rev-parse HEAD)" = 03f7a28ae613d7a10fce8c13644f71f2906b231d ] || { echo "STOP: worktree HEAD changed"; exit 1; }
    [ "$(git -C "$WT" branch --show-current)" = feature/r0-3-e-retention-cleanup ] || { echo "STOP: branch changed"; exit 1; }
    git -C "$STAGE" fetch -q "$REMOTE" batch-08-snapshot || { echo "STOP: fetch failed"; exit 1; }
    git -C "$STAGE" cat-file -e "$FINAL^{commit}" || { echo "STOP: $FINAL not fetched"; exit 1; }
    [ "$(git -C "$STAGE" rev-parse "$FINAL~1")" = "$SNAP" ] || echo "NOTE: $FINAL is not a direct child of the snapshot; check git log"
    rm -rf /tmp/b08-snap && mkdir -p /tmp/b08-snap && git -C "$STAGE" archive "$SNAP" | tar -C /tmp/b08-snap -x
    worktree_files | grep -Ev "^($KEPT_OUT)$" > /tmp/b08-wt.list
    ( cd /tmp/b08-snap && find . -type f -o -type l | sed 's|^\./||' | grep -v '^_batch08_context/' | sort ) > /tmp/b08-snap.list
    echo "worktree files $(wc -l < /tmp/b08-wt.list), snapshot files $(wc -l < /tmp/b08-snap.list)"
    diff /tmp/b08-wt.list /tmp/b08-snap.list > /tmp/b08-list.diff && echo "file lists equal" || { echo "STOP: file lists differ (/tmp/b08-list.diff)"; head -20 /tmp/b08-list.diff; exit 1; }
    n=0; d=0
    while IFS= read -r f; do
        n=$((n+1))
        if [ -L "$WT/$f" ]; then [ "$(readlink "$WT/$f")" = "$(readlink "/tmp/b08-snap/$f" 2>/dev/null)" ] || { echo "DIFF $f"; d=$((d+1)); }
        else cmp -s "$WT/$f" "/tmp/b08-snap/$f" || { echo "DIFF $f"; d=$((d+1)); }; fi
    done < /tmp/b08-wt.list
    echo "compared=$n differences=$d"
    [ "$d" = 0 ] && echo "VERIFY OK: the live worktree is the snapshot; the recovery diff applies to it" || { echo "STOP: the worktree changed after the snapshot"; exit 1; }
    ;;
apply)
    : "${FINAL:?set FINAL to the recovery commit sha}"
    bash "$RPT/tools/backup.sh" pre-recovery || { echo "STOP: backup failed"; exit 1; }
    git -C "$STAGE" diff --binary "$SNAP" "$FINAL" -- . ':(exclude)_batch08_context' > /tmp/b08-recovery.patch
    echo "code files: $(git -C "$STAGE" diff --name-only "$SNAP" "$FINAL" -- . ':(exclude)_batch08_context' | tr '\n' ' ')"
    git -C "$WT" apply --check --binary /tmp/b08-recovery.patch || { echo "STOP: patch does not apply"; exit 1; }
    git -C "$WT" apply --binary /tmp/b08-recovery.patch && echo "code applied to the worktree (not staged)"
    # reports: copy every file the recovery commit added or changed under _batch08_context/reports into $RPT
    git -C "$STAGE" diff --name-only --diff-filter=AM "$SNAP" "$FINAL" -- _batch08_context/reports | while IFS= read -r f; do
        rel=${f#_batch08_context/reports/}
        mkdir -p "$RPT/$(dirname "$rel")"
        git -C "$STAGE" show "$FINAL:$f" > "$RPT/$rel" && echo "report $rel"
    done
    ;;
shaders)
    cd "$WT" || exit 1
    cargo build --release --workspace --locked -j 12 > /tmp/b08-shaders-build.log 2>&1 || { echo "STOP: build failed"; exit 1; }
    for w in created-runtime environment-runtime events-runtime fields-runtime parts-runtime textures-runtime; do
        for m in 0 4; do
            echo "== $w MSAA $m"
            B08_SHADERS_TARGET=$WT/target/release python3 -I oracle/tools/rea_b08_shaders.py case "$w" "oracle/synthetic-rea-b08-shaders/$w.wrl" "oracle/synthetic-rea-b08-shaders/$w.json" $m
        done
    done
    for w in xml-basic xml-fields xml-invalid; do
        for m in 0 4; do
            echo "== $w MSAA $m"
            B08_SHADERS_TARGET=$WT/target/release python3 -I oracle/tools/rea_b08_shaders.py case "$w" "oracle/synthetic-rea-b08-shaders/$w.x3d" "oracle/synthetic-rea-b08-shaders/$w.json" $m
        done
    done
    echo "A case that printed FAIL kept no baseline: record it as a Rust difference in the final report (do not edit baselines)."
    ;;
gates)
    bash "$RPT/gates/run.sh"; echo "summary: $RPT/gates/summary.txt"; cat "$RPT/gates/summary.txt"
    ;;
ledger)
    python3 -I "$RPT/tools/ledger_verify.py" "$LEDGER" "${@:2}"
    ;;
backup)
    before=$( (cd "$WT" && git status --porcelain=v1 -uall && git diff --binary HEAD) | sha256sum)
    bash "$RPT/tools/backup.sh" final || { echo "STOP: backup failed"; exit 1; }
    bash "$RPT/tools/restore.sh" final /tmp/b08-restore-final 2>&1 | tee "$RPT/backup/restore-final.log"
    after=$( (cd "$WT" && git status --porcelain=v1 -uall && git diff --binary HEAD) | sha256sum)
    [ "$before" = "$after" ] && echo "worktree unchanged by the restore test" | tee -a "$RPT/backup/restore-final.log" || echo "WORKTREE CHANGED DURING BACKUP/RESTORE" | tee -a "$RPT/backup/restore-final.log"
    ( cd "$RPT/backup/start" && sha256sum -c SHA256SUMS ) | tee -a "$RPT/backup/restore-final.log"   # start backup untouched
    ;;
*)
    sed -n 2,13p "$0"; exit 2
    ;;
esac
