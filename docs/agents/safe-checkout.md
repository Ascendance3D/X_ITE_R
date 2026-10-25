# Safe checkout update

Use this procedure before any Git command that can write a tracked path in a checkout that holds work or evidence.
The main checkout holds the gitignored answer keys (`oracle/answer-keys.tsv`), so this procedure always applies there.

## Rules

1. Before a command that can write a tracked path, record the sha256 of each path. After the command, check each path again. For a tracked symlink, record its target (`readlink`), not the bytes it points to.
2. Never call a command byte-preserving unless a hash before and after shows it.
3. Name the source revision of every restore: `git restore --worktree --source=<rev> -- <path>`. Do not use `git checkout -- <path>`, which writes the index version and does not name it.
4. Keep every holding directory. Delete nothing.
5. In the main checkout, do not use `git pull`, `git reset`, `git stash`, `git clean` or `git checkout .`.
6. In the main checkout, the only permitted history operation is `merge --ff-only` with the procedure below. The answer-key hazard section lists the operations that delete keys.

## Answer-key hazard (issue #65)

The main checkout keeps 8 render keys and `upstream/` as real ignored directories at 9 paths that Git tracks as symlinks. `git status` shows them as ` D`.
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

`git clean -X` and `git clean -x` delete the 4 untracked JSON keys (`items-baseline.json`, `items2-baseline.json`, `x1-baseline.json`, `x_ite-text.json`). Rule 5 forbids `git clean`.

These operations kept the keys in the same tests: `merge --ff-only`, and `checkout` or `switch` of a commit whose key symlinks equal the index. Use only `merge --ff-only`, with the procedure below.

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
| 3 | make a dated holding directory under `~/Projects/cybertown/.backups/X_ITE_R/`. Write two manifests of every listed path: `sha256sum` of each regular file, and `readlink` of each symlink. Record a listed path that does not exist as absent. Stop on any other file type | a listed path is not a regular file, a directory of regular files, or a symlink |
| 4 | copy each path with unique content (working-tree bytes that `$T` does not hold, including paths that `$T` deletes) into the holding directory with its relative path (`cp -P -p -n --parents`, which keeps a symlink as a symlink); check the copies against both manifests | a destination file already exists; a copy differs |
| 5 | run the command with hooks off: `git -c core.hooksPath=/dev/null merge --ff-only $T` | it does something other than the expected effect (for a merge: HEAD is not `$T`) |
| 6 | check the tracked paths from step 2 against `$T`: `git --literal-pathspecs diff --quiet $T -- <paths>` (bytes, mode and symlink targets). Check every other listed path against the step 3 manifests | a tracked path differs from `$T`; a manifest entry differs |
| 7 | repeat step 0 | a key fails, a check fails, or a key path is a symlink. Stop and report; do not try a repair with Git |

Before step 5, `git diff --name-only HEAD $T` must not list a key path (`oracle/render-*-baseline`, `upstream`). A fast-forward that changes a key symlink is out of scope for this procedure.

### Restore one modified tracked file before a fast-forward

Use this when the working tree holds the target version of `<path>`, HEAD holds an older version, and the index is clean (`git diff --cached --quiet`). `$T` is the target SHA from step 1. `<path>` is a regular file, never a directory, `.` or a key path. Run steps 0 and 7 of the procedure around these steps.

| step | command | stop when |
| --- | --- | --- |
| a | `H=$(git rev-parse HEAD)`; `sha256sum <path>`; record the value as `W` | — |
| b | `git rev-parse $T:<path>` and `git hash-object <path>` | the two blob IDs differ (the working tree holds content that `$T` does not) |
| c | `cp -P -p -n --parents <path> <holding>/`, then `sha256sum` of the copy | `<holding>/<path>` already exists; the copy differs from `W` |
| d | `git restore --worktree --source=$H -- <path>` | — |
| e | `git hash-object <path>` | it differs from `git rev-parse $H:<path>` |
| f | `git -c core.hooksPath=/dev/null merge --ff-only $T`, then `git rev-parse HEAD` | HEAD is not `$T` |
| g | `sha256sum <path>` | it differs from `W` |

Step d changes the working-tree bytes for a short time. Step g proves that the final bytes equal the bytes before the procedure.

## Correction record

Recovery Phase 2 (`LOCAL_MASTER_RECONCILIATION.md` §4 step 5, 2026-10-25) said: "run `git checkout -- tools/candidate.lock`; the working-tree bytes do not change". That statement was false.
`git checkout -- <path>` wrote the index version (sha256 `cdc4840f…`) over the working-tree version (sha256 `995cc994…`) until the fast-forward wrote `995cc994…` back.

No data was lost, because the step 5 guard proved that the working tree equalled origin/master. The cause was a description from memory, with no hash before and after.
Recovery Phase 5 measured the defect, and Recovery Phase 6 replaced step 5 with steps a–g above. The Phase 2 report stays unchanged as historical evidence.
