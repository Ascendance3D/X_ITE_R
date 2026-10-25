# Safe checkout update

Use this procedure before any Git command that can write a tracked path in a checkout that holds work or evidence.
The main checkout holds the gitignored answer keys (`oracle/answer-keys.tsv`), so this procedure always applies there.

## Rules

1. Before a command that can write a tracked path, record the sha256 of each path. After the command, check each path again.
2. Never call a command byte-preserving unless a hash before and after shows it.
3. Name the source revision of every restore: `git restore --worktree --source=<rev> -- <path>`. Do not use `git checkout -- <path>`, which writes the index version and does not name it.
4. Keep every holding directory. Delete nothing.
5. In the main checkout, do not use `git pull`, `git reset`, `git stash`, `git clean` or `git checkout .`.

## Procedure

| step | action | stop when |
| --- | --- | --- |
| 1 | `git rev-parse HEAD` and `git status --porcelain`; record both. Resolve the target once: `T=$(git rev-parse --verify <target>^{commit})`, and use `$T` (not a branch name) in every later step | HEAD or status differs from what you expect; `$T` is not the expected SHA |
| 2 | list every path the command can write: `git diff --name-only HEAD $T` for a merge; the path list for a restore | — |
| 3 | make a dated holding directory under `~/Projects/cybertown/.backups/X_ITE_R/`; write a `sha256sum` manifest of the listed paths and of every untracked or ignored path near them | — |
| 4 | copy each path with unique content (working-tree bytes that `$T` does not hold) into the holding directory with its relative path (`cp -p --parents`); run `sha256sum -c` on the copies | a destination file already exists; a copy differs |
| 5 | run the command | it does something other than the expected effect (for a merge: not a fast-forward to the expected SHA) |
| 6 | run `sha256sum -c` of the manifest again | a change occurs that step 2 did not predict in writing |

### Restore one modified tracked file before a fast-forward

Use this when the working tree holds the target version of `<path>`, and HEAD holds an older version. `$T` is the target SHA from step 1.

| step | command | stop when |
| --- | --- | --- |
| a | `sha256sum <path>`; record the value as `W` | — |
| b | `git rev-parse $T:<path>` and `git hash-object <path>` | the two blob IDs differ (the working tree holds content that `$T` does not) |
| c | `cp -p --parents <path> <holding>/`, then `sha256sum` of the copy | `<holding>/<path>` already exists; the copy differs from `W` |
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
