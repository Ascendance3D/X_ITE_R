# Safe checkout update

Use this procedure before any Git command that can write a tracked path in a checkout that holds work or evidence.
The main checkout holds the gitignored answer keys (`oracle/answer-keys.tsv`), so this procedure always applies there.
The commands need GNU coreutils and findutils (`sha256sum -z`, `find -printf`, `cp --parents`, `stat -c`). On macOS, install the Homebrew `coreutils` and `findutils` packages and use their `g`-prefixed commands.

## Rules

1. Before a command that can write a tracked path, record the sha256 of each path. After the command, check each path again. For a tracked symlink, record its target (`readlink`), not the bytes it points to.
2. Never call a command byte-preserving unless a hash before and after shows it.
3. Name the source revision of every restore, and pass the path literally: `git --literal-pathspecs restore --worktree --source=<rev> -- <path>`. Without `--literal-pathspecs`, Git reads `*`, `?`, `[` and a leading `:` in a path as pathspec syntax and can write other paths. Do not use `git checkout -- <path>`, which writes the index version and does not name it.
4. Keep every holding directory. Delete nothing.
5. In the main checkout, do not use `git pull`, `git reset`, `git stash`, `git clean` or `git checkout .`.
6. In the main checkout, the only permitted command that moves HEAD is `merge --no-autostash --ff-only`, with the procedure below. The only permitted restore is step d of the one-file restore variant, with its own checks. The answer-key hazard section lists the operations that delete keys.

## Answer-key hazard (issue #65)

The main checkout keeps 8 render keys and `upstream/` as real ignored directories at 9 paths that Git tracks as symlinks. `git status --short` shows each of them with `D` in the worktree column and an unchanged index.
Git treats ignored files as expendable. A command that writes the tracked symlink at one of these paths deletes the real directory under it, and it often exits 0 and prints nothing.

A failed merge also deletes keys. A non-fast-forward `git merge` or `git pull` of a branch that changes or deletes one of these symlinks stops with "Aborting" and exit 2. The merge then restores the symlink over the real directory.

### Operations that delete keys in the main checkout

Each operation below deleted every file of a real ignored key directory in a synthetic repository with Git 2.43.0 (Recovery Phase 8A). Do not run any of them in the main checkout.

| operation | result |
| --- | --- |
| `git merge` that is not a fast-forward, when the other branch changes or deletes a key symlink | exit 2, directory replaced by the symlink |
| `git pull` in any form. A pull without `--ff-only` merges, as above. `git pull --ff-only` also deletes the keys when `merge.autoStash` or `rebase.autoStash` is set | exit 2 for a failed merge; exit 0 for an autostash; directory replaced |
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
| 0 | `env -u X3D_ORACLE_KEYS python3 -I -B tools/answer_keys.py verify` (all 12 keys; with the variable set, the resolver checks only the external location and not the keys in this checkout), and on the owner's workstation the protected-evidence check (`protect_check.sh` of Recovery Phase 6). Check that the 8 render keys and `upstream/` are real directories: `test -d <path> && test ! -L <path>` | a key fails, a check fails, or a key path is a symlink |
| 1 | `git rev-parse HEAD` and `git status --porcelain`; record both. Resolve the target once: `T=$(git rev-parse --verify <target>^{commit})`, and use `$T` (not a branch name) in every later step. `git diff --cached --quiet` must pass | HEAD or status differs from what you expect; `$T` is not the expected SHA; the index has staged changes |
| 2 | list the tracked paths the command can write: `git diff -z --no-renames --name-only HEAD $T` for a merge (a rename lists the deleted old path and the new path); the path list for a restore. List the untracked and ignored paths: `git ls-files -z --others --exclude-standard` and `git ls-files -z --others --ignored --exclude-standard --directory`. Write each list to its own file in the holding directory of step 3 (or in `/tmp`) and keep it NUL-separated. Never expand a list into a shell word list: pass it with `xargs -0 -a <list> <command>`, or read it with `while IFS= read -r -d '' p`, and run each check per path | a listed untracked or ignored path is equal to a tracked path from this step, is a directory above one, or is below one. Compare whole path components; `--directory` lists an ignored directory once, with a trailing `/`. (Example: `$T` replaces the directory `gen/` with a file `gen`; a fast-forward then deletes the ignored `gen/out.bin` and exits 0) |
| 3 | make a new, empty, dated holding directory under `<backups root>/X_ITE_R/` (the maintainer sets the backups root, outside the repository) (`mkdir` without `-p`; it must not exist before). Write three manifests of every listed path: `sha256sum -z` of each regular file, the target of each symlink (`find <path> -type l -printf '%P\0%l\0'`), and the type and mode of every entry (`find <path> -printf '%y %m %P\0'`), with relative paths, so the source and copy manifests compare directly. End every record with NUL, not a newline, because a file name can hold a newline. Record a listed path that does not exist as absent. Ownership, timestamps, ACLs and xattrs are not compared. Stop on any other file type | a listed path is not a regular file, a directory of regular files, or a symlink |
| 4 | copy each existing path with unique content (working-tree bytes that `$T` does not hold, including paths that `$T` deletes) into the holding directory with its relative path. First check `test ! -e <holding>/<path> && test ! -L <holding>/<path>` (`-e` alone misses a dangling symlink), then copy with `cp -R -P -p --parents -- <path> <holding>/` (`-R` copies a directory, `-P` keeps a symlink as a symlink). Do not use `cp -n`: it skips an existing file and exits 0. A path recorded as absent in step 3 has nothing to copy. Check the copies against all three manifests | `<holding>/<path>` already exists; `cp` exits non-zero; a copy differs |
| 5 | run the command with hooks and autostash off: `git -c core.hooksPath=/dev/null merge --no-autostash --ff-only $T`; record its exit status before any other command | `git merge` exits non-zero, or it does something other than the expected effect (for a merge: HEAD is not `$T`). After a non-zero exit, run no Git command that writes. Run step 7, then record `git rev-parse HEAD` and `git status --porcelain` (compare them with step 1) and check every listed path against the step 3 manifests. Record each difference, keep the holding directory and report; do not try a repair with Git |
| 6 | check the index against `$T`: `git diff --cached --quiet $T`. Check the tracked paths from step 2 against `$T`: `xargs -0 -a <tracked list> git --literal-pathspecs diff --quiet $T --` (bytes, mode and symlink targets; any non-zero exit is a difference). Check every other listed path against the step 3 manifests | the index differs from `$T`; a tracked path differs from `$T`; a manifest entry differs |
| 7 | repeat step 0, with `X3D_ORACLE_KEYS` unset | a key fails, a check fails, or a key path is a symlink. Stop and report; do not try a repair with Git |

Before step 5, the step 2 list of tracked paths (`git diff -z --no-renames --name-only HEAD $T`) must not contain a key path. The key paths are exactly 9: `upstream` and `oracle/<name>` for each `dir` row of `oracle/answer-keys.tsv` (`render-items-baseline`, `render-items2-baseline`, `render-msaa4-items-baseline`, `render-msaa4-items2-baseline`, `render-msaa4-x8-baseline`, `render-msaa8-items-baseline`, `render-msaa8-x8-baseline`, `render-synthetic-x8-baseline`). Compare each listed path for exact equality with a key path, and also stop on a listed path below one. Other `oracle/render-*-baseline` directories are tracked baselines, not keys. Use exactly that `--no-renames` list. Do not use a diff with rename detection for this check, because it can omit the old path of a renamed key symlink. A fast-forward that changes, renames or deletes a key symlink is out of scope for this procedure.

### Restore one modified tracked file before a fast-forward

Use this when the working tree holds the target version of `<path>`, HEAD holds an older version, and the index is clean (`git diff --cached --quiet`). `$T` is the target SHA from step 1. `<path>` is a regular file, never a directory, `.` or a key path. This variant replaces only step 5 of the procedure. Run steps 0–4 first: they resolve `$T`, list the paths that the fast-forward can write, refuse an overlap with an untracked or ignored path, and make the holding copies. The step 2 list must not contain a key path (the key-path check before step 5). Then run steps a–g in place of step 5, and run steps 6 and 7 after step g.

| step | command | stop when |
| --- | --- | --- |
| a | `H=$(git rev-parse HEAD)`; `sha256sum <path>`; record the value as `W` | — |
| b | `git rev-parse $T:<path>` and `git hash-object -- <path>`. `git --literal-pathspecs ls-tree $T -- <path>` gives the target mode: compare it with the working file: `100755` when the owner execute bit is set (the fourth character of `stat -c %A <path>` is `x`), else `100644`. Git records only that bit; do not use `test -x`, which checks access rights | the two blob IDs differ (the working tree holds content that `$T` does not); the target mode is not `100644` or `100755`, or it differs from the working file |
| c | `test ! -e <holding>/<path> && test ! -L <holding>/<path>`, then `cp -P -p --parents -- <path> <holding>/`, then `sha256sum` and `stat -c %a` of the copy and of `<path>`; record the mode of `<path>` as `M` | `<holding>/<path>` already exists; the copy differs from `W` or its mode differs from `M` |
| d | `git --literal-pathspecs restore --worktree --source=$H -- <path>` | — |
| e | `git hash-object -- <path>` | it differs from `git rev-parse $H:<path>` |
| f | `git -c core.hooksPath=/dev/null merge --no-autostash --ff-only $T`; record its exit status; if it exits zero, run `git rev-parse HEAD` | `git merge` exits non-zero: handle it as in step 5, then see the recovery below; HEAD is not `$T` |
| g | `sha256sum <path>` and `stat -c %a <path>` | the hash differs from `W`, or the mode differs from `M` |

Step d changes the working-tree bytes for a short time. Step g proves that the final bytes and mode equal those before the procedure.

Recovery after a failed step f: step d left the `$H` version in `<path>`, and the holding copy keeps `W`. Do the recovery only when all of these read-only checks pass: `git rev-parse HEAD` is `$H`; `git diff --cached --quiet $H` (the index is unchanged and has no conflict stages); `git status --porcelain` equals the step 1 record without the line for `<path>`; `git hash-object -- <path>` equals `git rev-parse $H:<path>`; and every step 3 manifest entry other than `<path>` is unchanged. If a check fails or cannot run, do not recover. Then copy the bytes back without Git: `cp -p -- <holding>/<path> <path>`. Check that `sha256sum <path>` equals `W` and `stat -c %a <path>` equals `M`, and report. In every other case, keep the holding copy and report.

## Correction record

Recovery Phase 2 (`LOCAL_MASTER_RECONCILIATION.md` §4 step 5, 2026-10-25) said: "run `git checkout -- tools/candidate.lock`; the working-tree bytes do not change". That statement was false.
`git checkout -- <path>` wrote the index version (sha256 `cdc4840f…`) over the working-tree version (sha256 `995cc994…`) until the fast-forward wrote `995cc994…` back.

No data was lost, because the step 5 guard proved that the working tree equalled origin/master. The cause was a description from memory, with no hash before and after.
Recovery Phase 5 measured the defect, and Recovery Phase 6 replaced step 5 with steps a–g above. The Phase 2 report stays unchanged as historical evidence.
