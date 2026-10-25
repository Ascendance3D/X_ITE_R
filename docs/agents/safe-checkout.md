# Safe checkout update

Use this procedure before any Git command that can write a tracked path in a checkout that holds work or evidence.
The main checkout holds the gitignored answer keys (`oracle/answer-keys.tsv`), so this procedure always applies there.

## Rules

1. Before a command that can write a tracked path, record the sha256 of each path. After the command, check each path again. For a tracked symlink, record its target (`readlink`), not the bytes it points to.
2. Never call a command byte-preserving unless a hash before and after shows it.
3. Name the source revision of every restore: `git restore --worktree --source=<rev> -- <path>`. Do not use `git checkout -- <path>`, which writes the index version and does not name it.
4. Keep every holding directory. Delete nothing.
5. In the main checkout, do not use `git pull`, `git reset`, `git stash`, `git clean` or `git checkout .`.
6. In the main checkout, the only permitted history operation is `merge --no-autostash --ff-only` with the procedure below. The answer-key hazard section lists the operations that delete keys.

## Answer-key hazard (issue #65)

The main checkout keeps 8 render keys and `upstream/` as real ignored directories at 9 paths that Git tracks as symlinks. `git status --short` shows each of them with `D` in the worktree column and an unchanged index.
Git treats ignored files as expendable. A command that writes the tracked symlink at one of these paths deletes the real directory under it, and it often exits 0 and prints nothing.

A failed merge also deletes keys. A non-fast-forward `git merge` or `git pull` of a branch that changes or deletes one of these symlinks stops with "Aborting" and exit 2. The merge then restores the symlink over the real directory.

### Operations that delete keys in the main checkout

Each operation below deleted every file of a real ignored key directory in a synthetic repository with Git 2.43.0 (Recovery Phase 8A). Do not run any of them in the main checkout.

| operation | result |
| --- | --- |
| `git merge` that is not a fast-forward, when the other branch changes or deletes a key symlink | exit 2, directory replaced by the symlink |
| `git pull` that merges (any `git pull` without `--ff-only`) | same as the merge |
| `git checkout -f`, `git switch --discard-changes` | exit 0, directory replaced |
| `git reset --hard`, also `git reset --hard HEAD` | exit 0, directory replaced |
| `git restore --source=<rev>` or `git checkout -- <pathspec>` where the pathspec is `.`, `oracle`, `upstream` or a key path | exit 0, directory replaced |
| `git stash`, `git stash -u` | exit 0, directory replaced |
| `git merge --ff-only` when `merge.autoStash=true` is set (Git creates an autostash) | exit 0, directory replaced. Always pass `--no-autostash` |

`git clean -X` and `git clean -x` delete the 4 untracked JSON keys (`items-baseline.json`, `items2-baseline.json`, `x1-baseline.json`, `x_ite-text.json`). Rule 5 forbids `git clean`.

These operations kept the keys in the same tests: `merge --no-autostash --ff-only`, and `checkout` or `switch` of a commit whose key symlinks equal the index. Use only `merge --no-autostash --ff-only`, with the procedure below.

### The inventory does not see the keys

`git ls-files --others --ignored --exclude-standard --directory` lists nothing for a real directory at a tracked symlink path. So step 2 of the procedure does not list the 8 render keys or `upstream/`.
Verify the keys directly before and after the command: steps 0 and 7.

### Limits of other protections

- `--no-overwrite-ignore` on `checkout`, `switch` and `merge` is an extra layer only. It stopped a checkout, but it did not stop the failed-merge deletion. `reset`, `restore`, `stash`, `rebase`, `revert` and `pull` have no such option.
- Git hooks do not protect the keys. In the Phase 7 tests a `reference-transaction` hook stopped the ref update only after the files were gone, and `post-checkout` runs after the change.
- `git update-index --skip-worktree` is not a protection. The bit is in one local index only, and a new clone or worktree does not have it.
- Git 2.43 has no "precious" ignore patterns.

Only a move of the keys out of every tracked path removes the hazard. That migration is open in issue #65; do not untrack the symlinks before it.

## Procedure

| step | action | stop when |
| --- | --- | --- |
| 0 | `python3 -I -B tools/answer_keys.py verify` (all 12 keys), and on the owner's workstation the protected-evidence check (`protect_check.sh` of Recovery Phase 6). Check that the 8 render keys and `upstream/` are real directories: `test -d <path> -a ! -L <path>` | a key fails, a check fails, or a key path is a symlink |
| 1 | `git rev-parse HEAD` and `git status --porcelain`; record both. Resolve the target once: `T=$(git rev-parse --verify <target>^{commit})`, and use `$T` (not a branch name) in every later step. `git diff --cached --quiet` must pass | HEAD or status differs from what you expect; `$T` is not the expected SHA; the index has staged changes |
| 2 | list the tracked paths the command can write: `git diff -z --no-renames --name-only HEAD $T` for a merge (a rename lists the deleted old path and the new path); the path list for a restore. List the untracked and ignored paths: `git ls-files -z --others --exclude-standard` and `git ls-files -z --others --ignored --exclude-standard --directory` | a listed untracked or ignored path is also a path that `$T` adds, or holds a path that `$T` adds (`--directory` lists an ignored directory once) |
| 3 | make a new, empty, dated holding directory under `~/Projects/cybertown/.backups/X_ITE_R/` (`mkdir` without `-p`; it must not exist before). Write three manifests of every listed path: `sha256sum` of each regular file, `readlink` of each symlink, and the type and mode of every entry (`find <path> -printf '%y %m %P\n'`, relative paths, so the source and copy manifests compare directly). Record a listed path that does not exist as absent. Ownership, timestamps, ACLs and xattrs are not compared. Stop on any other file type | a listed path is not a regular file, a directory of regular files, or a symlink |
| 4 | copy each existing path with unique content (working-tree bytes that `$T` does not hold, including paths that `$T` deletes) into the holding directory with its relative path. First check `test ! -e <holding>/<path> && test ! -L <holding>/<path>` (`-e` alone misses a dangling symlink), then copy with `cp -R -P -p --parents -- <path> <holding>/` (`-R` copies a directory, `-P` keeps a symlink as a symlink). Do not use `cp -n`: it skips an existing file and exits 0. A path recorded as absent in step 3 has nothing to copy. Check the copies against all three manifests | `<holding>/<path>` already exists; `cp` exits non-zero; a copy differs |
| 5 | run the command with hooks and autostash off: `git -c core.hooksPath=/dev/null merge --no-autostash --ff-only $T`; record its exit status before any other command | `git merge` exits non-zero, or it does something other than the expected effect (for a merge: HEAD is not `$T`). After a non-zero exit, run only step 7 to record the key state, keep the holding directory and report; do not try a repair with Git |
| 6 | check the tracked paths from step 2 against `$T`: `git --literal-pathspecs diff --quiet $T -- <paths>` (bytes, mode and symlink targets). Check every other listed path against the step 3 manifests | a tracked path differs from `$T`; a manifest entry differs |
| 7 | repeat step 0 | a key fails, a check fails, or a key path is a symlink. Stop and report; do not try a repair with Git |

Before step 5, the step 2 list of tracked paths (`git diff -z --no-renames --name-only HEAD $T`) must not contain a key path (`oracle/render-*-baseline`, `upstream`). Use exactly that `--no-renames` list. Do not use a diff with rename detection for this check, because it can omit the old path of a renamed key symlink. A fast-forward that changes, renames or deletes a key symlink is out of scope for this procedure.

### Restore one modified tracked file before a fast-forward

Use this when the working tree holds the target version of `<path>`, HEAD holds an older version, and the index is clean (`git diff --cached --quiet`). `$T` is the target SHA from step 1. `<path>` is a regular file, never a directory, `.` or a key path. This variant replaces only step 5 of the procedure. Run steps 0–4 first: they resolve `$T`, list the paths that the fast-forward can write, refuse an overlap with an untracked or ignored path, and make the holding copies. The step 2 list must not contain a key path (the key-path check before step 5). Then run steps a–g in place of step 5, and run steps 6 and 7 after step g.

| step | command | stop when |
| --- | --- | --- |
| a | `H=$(git rev-parse HEAD)`; `sha256sum <path>`; record the value as `W` | — |
| b | `git rev-parse $T:<path>` and `git hash-object <path>` | the two blob IDs differ (the working tree holds content that `$T` does not) |
| c | `test ! -e <holding>/<path> && test ! -L <holding>/<path>`, then `cp -P -p --parents -- <path> <holding>/`, then `sha256sum` and `stat -c %a` of the copy and of `<path>` | `<holding>/<path>` already exists; the copy differs from `W` or its mode differs |
| d | `git restore --worktree --source=$H -- <path>` | — |
| e | `git hash-object <path>` | it differs from `git rev-parse $H:<path>` |
| f | `git -c core.hooksPath=/dev/null merge --no-autostash --ff-only $T`; record its exit status; then `git rev-parse HEAD` | `git merge` exits non-zero (handle it as in step 5); HEAD is not `$T` |
| g | `sha256sum <path>` | it differs from `W` |

Step d changes the working-tree bytes for a short time. Step g proves that the final bytes equal the bytes before the procedure.

## Correction record

Recovery Phase 2 (`LOCAL_MASTER_RECONCILIATION.md` §4 step 5, 2026-10-25) said: "run `git checkout -- tools/candidate.lock`; the working-tree bytes do not change". That statement was false.
`git checkout -- <path>` wrote the index version (sha256 `cdc4840f…`) over the working-tree version (sha256 `995cc994…`) until the fast-forward wrote `995cc994…` back.

No data was lost, because the step 5 guard proved that the working tree equalled origin/master. The cause was a description from memory, with no hash before and after.
Recovery Phase 5 measured the defect, and Recovery Phase 6 replaced step 5 with steps a–g above. The Phase 2 report stays unchanged as historical evidence.
