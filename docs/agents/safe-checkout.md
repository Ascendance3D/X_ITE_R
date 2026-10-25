# Safe checkout update

Use this procedure before any Git command that can write a tracked path in a checkout that holds work or evidence.
The main checkout holds the gitignored answer keys (`oracle/answer-keys.tsv`), so this procedure always applies there.

## Rules

1. Before a command that can write a tracked path, record the sha256 of each path. After the command, check each path again. For a tracked symlink, record its target (`readlink`), not the bytes it points to.
2. Never call a command byte-preserving unless a hash before and after shows it.
3. Name the source revision of every restore: `git restore --worktree --source=<rev> -- <path>`. Do not use `git checkout -- <path>`, which writes the index version and does not name it.
4. Keep every holding directory. Delete nothing.
5. In the main checkout, do not use `git pull`, `git reset`, `git stash`, `git clean` or `git checkout .`.

## Procedure

| step | action | stop when |
| --- | --- | --- |
| 1 | `git rev-parse HEAD` and `git status --porcelain`; record both. Resolve the target once: `T=$(git rev-parse --verify <target>^{commit})`, and use `$T` (not a branch name) in every later step. `git diff --cached --quiet` must pass | HEAD or status differs from what you expect; `$T` is not the expected SHA; the index has staged changes |
| 2 | list the tracked paths the command can write: `git diff -z --no-renames --name-only HEAD $T` for a merge (a rename lists the deleted old path and the new path); the path list for a restore. List the untracked and ignored paths: `git ls-files -z --others --exclude-standard` and `git ls-files -z --others --ignored --exclude-standard --directory` | a listed untracked or ignored path is also a path that `$T` adds |
| 3 | make a dated holding directory under `~/Projects/cybertown/.backups/X_ITE_R/`. Write two manifests of every listed path: `sha256sum` of each regular file, and `readlink` of each symlink. Record a listed path that does not exist as absent. Stop on any other file type | a listed path is not a regular file, a directory of regular files, or a symlink |
| 4 | copy each path with unique content (working-tree bytes that `$T` does not hold, including paths that `$T` deletes) into the holding directory with its relative path (`cp -P -p --parents`, which keeps a symlink as a symlink); check the copies against both manifests | a destination file already exists; a copy differs |
| 5 | run the command | it does something other than the expected effect (for a merge: HEAD is not `$T`) |
| 6 | check the tracked paths from step 2 against `$T`: `git diff --quiet $T -- <paths>` (bytes, mode and symlink targets). Check every other listed path against the step 3 manifests | a tracked path differs from `$T`; any other path changed |

### Restore one modified tracked file before a fast-forward

Use this when the working tree holds the target version of `<path>`, HEAD holds an older version, and the index is clean (`git diff --cached --quiet`). `$T` is the target SHA from step 1. `<path>` is a regular file.

| step | command | stop when |
| --- | --- | --- |
| a | `sha256sum <path>`; record the value as `W` | — |
| b | `git rev-parse $T:<path>` and `git hash-object <path>` | the two blob IDs differ (the working tree holds content that `$T` does not) |
| c | `cp -P -p --parents <path> <holding>/`, then `sha256sum` of the copy | `<holding>/<path>` already exists; the copy differs from `W` |
| d | `git restore --worktree --source=HEAD -- <path>` | — |
| e | `git hash-object <path>` | it differs from `git rev-parse HEAD:<path>` |
| f | `git merge --ff-only $T`, then `git rev-parse HEAD` | HEAD is not `$T` |
| g | `sha256sum <path>` | it differs from `W` |

Step d changes the working-tree bytes for a short time. Step g proves that the final bytes equal the bytes before the procedure.

## Correction record

Recovery Phase 2 (`LOCAL_MASTER_RECONCILIATION.md` §4 step 5, 2026-10-25) said: "run `git checkout -- tools/candidate.lock`; the working-tree bytes do not change". That statement was false.
`git checkout -- <path>` wrote the index version (sha256 `cdc4840f…`) over the working-tree version (sha256 `995cc994…`) until the fast-forward wrote `995cc994…` back.

No data was lost, because the step 5 guard proved that the working tree equalled origin/master. The cause was a description from memory, with no hash before and after.
Recovery Phase 5 measured the defect, and Recovery Phase 6 replaced step 5 with steps a–g above. The Phase 2 report stays unchanged as historical evidence.
