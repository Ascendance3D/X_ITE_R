# Safe checkout update

Use this procedure before any Git command that can write a tracked path in a checkout that holds work or evidence.
The main checkout holds ignored copies of answer keys (`oracle/answer-keys.tsv`) and empty key paths that older commits track as symlinks, so this procedure always applies there.
The commands need GNU coreutils and findutils (`sha256sum -z`, `find -printf`, `cp --parents`, `stat -c`). On macOS, install the Homebrew `coreutils` and `findutils` packages and use their `g`-prefixed commands.

## Rules

1. Before a command that can write a tracked path, record the sha256 of each path. After the command, check each path again. For a tracked symlink, record its target (`readlink`), not the bytes it points to.
2. Never call a command byte-preserving unless a hash before and after shows it.
3. Name the source revision of every restore, and pass the path literally: `git --literal-pathspecs restore --worktree --source=<rev> -- <path>`. Without `--literal-pathspecs`, Git reads `*`, `?`, `[` and a leading `:` in a path as pathspec syntax and can write other paths. Do not use `git checkout -- <path>`, which writes the index version and does not name it.
4. Keep every holding directory. Delete nothing.
5. In the main checkout, do not use `git pull`, `git reset`, `git stash`, `git clean` or `git checkout .`.
6. In the main checkout, the only permitted command that moves HEAD is `merge --no-autostash --ff-only`, with the procedure below. The only permitted restore is step d of the one-file restore variant, with its own checks. The answer-key hazard section lists the operations that delete keys.

## Answer-key hazard (issue #65)

Older commits track the 8 render-key paths (`oracle/render-*-baseline`) and `upstream` as symlinks. Master does not track them, but a checkout, reset, restore or merge of an older commit writes them again. Git treats ignored files as expendable. When a real ignored directory is at one of these paths, a command that writes the tracked symlink deletes the directory, and it often exits 0 and prints nothing.

Until 2026-10-29 the main checkout kept the 8 render keys and `upstream/` as real directories at these paths. They are now in the external evidence store (next section), and the 9 paths in the main checkout are empty. Master ignores the 9 paths and tracks none of them, so `git status --short` shows nothing for them.

A failed merge also deletes keys. A non-fast-forward `git merge` or `git pull` of a branch that changes or deletes one of these symlinks stops with "Aborting" and exit 2. The merge then restores the symlink over the real directory.

### External evidence store

The owner chose one external store for the keys (issue #65). It is a private, dated directory `<backups root>/X_ITE_R/<date>-evidence-store/`, outside every worktree. The maintainer sets the backups root, outside the repository. The current store is dated 2026-10-29 and holds the main checkout's keys at `a5a6b6f5`.

The store holds every row of `oracle/answer-keys.tsv` (24 entries) as real files and directories, with no symlinks:

- the 12 X_ITE answer keys: `items-baseline.json`, `items2-baseline.json`, `x1-baseline.json`, `x_ite-text.json` and the 8 `render-*-baseline` directories;
- the 12 private paths of issue #168.

The store also holds `README.md` (the origin of each entry) and `SHA256SUMS` (every file, relative paths). Do not edit an entry in place, and never re-record a key in the store. To use it, set `X3D_ORACLE_KEYS` to its absolute path. Then the resolver searches only the store.

The main checkout layout is:

| path | state |
| --- | --- |
| the 8 `oracle/render-*-baseline` key paths | empty: no file, directory or symlink. The keys are only in the store |
| `upstream` | empty. `a5a6b6f5` does not track it; older commits track it as a symlink |
| the 4 JSON keys in `oracle/` | real ignored files, equal to the store. Their paths are not tracked |
| the 12 #168 paths in `oracle/` | real ignored files and directories, equal to the store. Their paths are not tracked at `a5a6b6f5` |

The render keys are not in the main checkout, so the strict tests there need `X3D_ORACLE_KEYS`. Keep the store and the checkout copies. Delete nothing.

A command that reads a key by its `oracle/` path (for example `x3d-shot compare oracle/render-items2-baseline …`) needs a link there: `X3D_ORACLE_KEYS=<store> python3 -I -B tools/answer_keys.py link . <name>...`. The link is a symlink into the store, and the ignore rules cover it. A Git operation that writes the path replaces the link and does not change the store. A link is for reading only: a command that writes through it writes the store. The link is not a security boundary. It protects the store from a Git path replacement, not from a process that follows the link. Only `check-output` (in the 4 key recorders) refuses such a write, and other writers (for example `x3d-shot batch` or `cp`) do not run it. Never give a linked path, a key path or a store path to a command as its output. Write a regenerated key to a staging directory (`oracle/README.md`, "Regenerate a key"). Remove the links before the procedure below, because step 0 stops on a symlink at a render-key path: `test -L oracle/<name> && rm -- oracle/<name>` for each linked name, with no trailing slash, so `rm` removes only the link. Never copy a key to a key path.

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

`git ls-files --others --ignored --exclude-standard --directory` lists nothing for a real directory at a tracked symlink path. So step 2 of the procedure does not list data at the 8 render-key paths or at `upstream`.
Verify the keys and the empty key paths directly before and after the command: steps 0 and 7.

### Limits of other protections

- `--no-overwrite-ignore` on `checkout`, `switch` and `merge` is an extra layer only. It stopped a checkout, but it did not stop the failed-merge deletion. `reset`, `restore`, `stash`, `rebase`, `revert` and `pull` have no such option.
- Git hooks do not protect the keys. In the Phase 7 tests a `reference-transaction` hook stopped the ref update only after the files were gone, and `post-checkout` runs after the change.
- `git update-index --skip-worktree` is not a protection. The bit is in one local index only, and a new clone or worktree does not have it.
- Git 2.43 has no "precious" ignore patterns.

Only a move of the keys out of every tracked path removes the hazard. In the main checkout, that move is done: the render keys are in the external store. Do not move a key back to a tracked symlink path. Master does not track the 8 render-key symlinks. Older commits still track them, so run `tools/answer_keys.py guard . HEAD <rev>` before any operation to an older `<rev>`. The guard refuses when real data is at a path that `<rev>` tracks.

## Procedure

This procedure is validated for the main checkout only. In a linked worktree, the resolver can use the main worktree's copy of a key. The printed-path checks of step 0, step 7 and exception condition 1 stop that case. No other linked-worktree case is validated, so do not use the procedure in a linked worktree without its own test.

| step | action | stop when |
| --- | --- | --- |
| 0 | `X3D_ORACLE_KEYS=<store> python3 -I -B tools/answer_keys.py verify` (every row of `oracle/answer-keys.tsv`, in the store only), and `sha256sum -c --quiet SHA256SUMS` in the store. The store layout check, in bash, in the store: `find . ! -type f ! -type d` prints nothing (no symlink and no other file type), no line of `SHA256SUMS` starts with `\` (an escaped name), and `diff <(find . -type f ! -path ./SHA256SUMS ! -path ./README.md \| LC_ALL=C sort) <(cut -c 67- SHA256SUMS \| LC_ALL=C sort)` prints nothing (no extra file and no missing file). These three are the store checks. `env -u X3D_ORACLE_KEYS python3 -I -B tools/answer_keys.py verify <name>` for each of the 16 keys that the main checkout holds in `oracle/` (the 4 JSON keys and the 12 #168 paths). Each must print the absolute path of `oracle/<name>` in this checkout. Another path means that the resolver found a copy in another worktree, so stop. That check covers the checkout copies only; it does not verify the store. Under the exception below, check here only the checkout copies at unaffected key paths, before any copy moves. Exception condition 1 verifies each affected copy before it moves the copy. `python3 -I -B tools/answer_keys.py guard . HEAD`, with and without `X3D_ORACLE_KEYS`. On the owner's workstation, the protected-evidence check (`protect_check.sh` of Recovery Phase 6). Check that the 8 render-key paths and `upstream` are empty: `test ! -e <path> && test ! -L <path>` | a key fails, a check fails, `verify` prints another path, the guard refuses, or a render-key path or `upstream` is not empty |
| 1 | `git rev-parse HEAD` and `git status --porcelain`; record both. Resolve the target once: `T=$(git rev-parse --verify <target>^{commit})`, and use `$T` (not a branch name) in every later step. `git diff --cached --quiet` must pass | HEAD or status differs from what you expect; `$T` is not the expected SHA; the index has staged changes |
| 2 | list the tracked paths the command can write: `git diff -z --no-renames --name-only HEAD $T` for a merge (a rename lists the deleted old path and the new path); the path list for a restore. List the untracked and ignored paths: `git ls-files -z --others --exclude-standard` and `git ls-files -z --others --ignored --exclude-standard --directory`. Write each list to its own file in the holding directory of step 3 (or in `/tmp`) and keep it NUL-separated. Never expand a list into a shell word list: pass it with `xargs -0 -a <list> <command>`, or read it with `while IFS= read -r -d '' p`, and run each check per path | a listed untracked or ignored path is equal to a tracked path from this step, is a directory above one, or is below one. Compare whole path components; `--directory` lists an ignored directory once, with a trailing `/`. (Example: `$T` replaces the directory `gen/` with a file `gen`; a fast-forward then deletes the ignored `gen/out.bin` and exits 0). The tracked list contains `oracle/answer-keys.tsv` (the manifest rule below the table) |
| 3 | make a new, empty, dated holding directory under `<backups root>/X_ITE_R/` (the maintainer sets the backups root, outside the repository) (`mkdir` without `-p`; it must not exist before). Write three manifests of every listed path: `sha256sum -z` of each regular file, the target of each symlink (`find <path> -type l -printf '%P\0%l\0'`), and the type and mode of every entry (`find <path> -printf '%y %m %P\0'`), with relative paths, so the source and copy manifests compare directly. End every record with NUL, not a newline, because a file name can hold a newline. Record a listed path that does not exist as absent. Ownership, timestamps, ACLs and xattrs are not compared. Stop on any other file type | a listed path is not a regular file, a directory of regular files, or a symlink |
| 4 | copy each existing path with unique content (working-tree bytes that `$T` does not hold, including paths that `$T` deletes) into the holding directory with its relative path. First check `test ! -e <holding>/<path> && test ! -L <holding>/<path>` (`-e` alone misses a dangling symlink), then copy with `cp -R -P -p --parents -- <path> <holding>/` (`-R` copies a directory, `-P` keeps a symlink as a symlink). Do not use `cp -n`: it skips an existing file and exits 0. A path recorded as absent in step 3 has nothing to copy. Check the copies against all three manifests | `<holding>/<path>` already exists; `cp` exits non-zero; a copy differs |
| 5 | run the command with hooks and autostash off: `git -c core.hooksPath=/dev/null merge --no-autostash --ff-only $T`; record its exit status before any other command | `git merge` exits non-zero, or it does something other than the expected effect (for a merge: HEAD is not `$T`). After a non-zero exit, run no Git command that writes. Run step 7, then record `git rev-parse HEAD` and `git status --porcelain` (compare them with step 1) and check every listed path against the step 3 manifests. Record each difference, keep the holding directory and report; do not try a repair with Git |
| 6 | check the index against `$T`: `git diff --cached --quiet $T`. Check the tracked paths from step 2 against `$T`: `xargs -0 -a <tracked list> git --literal-pathspecs diff --quiet $T --` (bytes, mode and symlink targets; any non-zero exit is a difference). Check every other listed path against the step 3 manifests | the index differs from `$T`; a tracked path differs from `$T`; a manifest entry differs |
| 7 | repeat step 0. After the exception below, use its step 7 list instead | the stop conditions of step 0. Stop and report; do not try a repair with Git |

Before step 5, the step 2 list of tracked paths (`git diff -z --no-renames --name-only HEAD $T`) must not contain a key path. The key paths are exactly 25: `upstream` and `oracle/<name>` for each row of `oracle/answer-keys.tsv`, of kind `file` or `dir`. They are the paths that `tools/answer_keys.py guard` protects (`GUARDED` and every manifest row). The 12 X_ITE answer keys are the files `items-baseline.json`, `items2-baseline.json`, `x1-baseline.json` and `x_ite-text.json`, and the directories `render-items-baseline`, `render-items2-baseline`, `render-msaa4-items-baseline`, `render-msaa4-items2-baseline`, `render-msaa4-x8-baseline`, `render-msaa8-items-baseline`, `render-msaa8-x8-baseline` and `render-synthetic-x8-baseline`. The 12 private paths of issue #168 are the directories `corpus-items`, `corpus-items2`, `corpus-items-media`, `corpus-items2-media`, `render-items`, `render-items2`, `corpus-x0`, `corpus-x1`, `render-x0-baseline` and `render-msaa4-x0-baseline`, and the files `x0-baseline.json` and `x0-xml-baseline.json`. Compare each listed path for exact equality with a key path, and also stop on a listed path below one or above one. Other `oracle/render-*-baseline` directories are tracked baselines, not keys. Use exactly that `--no-renames` list. Do not use a diff with rename detection for this check, because it can omit the old path of a renamed key symlink. A fast-forward whose step 2 tracked list contains a key path, or a path below or above one, is out of scope for this procedure, except as the next section permits.

Before step 5, the step 2 tracked list must also not contain `oracle/answer-keys.tsv`. The key paths above come from the manifest in the worktree, which is the manifest of `HEAD`. If `$T` adds, removes or changes a row, it can add a key path that this check, step 0 and `guard . HEAD` do not see. Then stop at step 2, before any Git command that writes. A manifest change (a key migration) is out of scope for this procedure and for the exception below. It needs its own reviewed procedure.

### Exception: a fast-forward that deletes a key symlink or tracked files at a key path

Use this only for `git -c core.hooksPath=/dev/null merge --no-autostash --ff-only $T` in a checkout that uses the external store. The affected key paths are every key path that is equal to a path in the step 2 tracked list, or that is above or below one. The exception covers deletions only. Every step 2 tracked path at, below or above an affected key path must be in `git diff -z --no-renames --name-only --diff-filter=D HEAD $T`. A fast-forward that adds or changes such a path (for example, a new target for a key symlink) is out of scope.

The affected key paths come from the step 2 tracked list, and condition 1 moves copies inside step 0. So, under the exception, run the steps in this order:

1. Step 1: record HEAD and the status, and resolve `$T`. These commands only read.
2. The tracked list of step 2 only (`git diff -z --no-renames --name-only HEAD $T`), with its stop conditions: the manifest rule, the key-path rule and the deletions-only rule above. Find the affected key paths.
3. Make the step 3 holding directory (`mkdir` without `-p`). It holds the copies that condition 1 moves and the copies of steps 3 and 4.
4. Step 0, with the moves of condition 1.
5. The rest of step 2 (the untracked and ignored lists), then steps 3 and 4 into the same holding directory, then steps 5–7.

All of these must be true before step 5:

1. Every affected key path is empty: `test ! -e <path> && test ! -L <path>`. This includes tracked files below the path: they must be absent from the worktree too. A Git operation that deletes a tracked symlink or tracked file must never run while real data is at that path. A key path that holds a checkout copy (a #168 path or a JSON key) is not empty. Move each such copy out inside step 0, before the checkout checks of step 0, in this order:
   1. Run the store checks of step 0 (24 of 24 rows, `sha256sum -c` of `SHA256SUMS`, and the layout check). Then verify the checkout copies at unaffected key paths, as in step 0 (with the printed-path check). Both run before any copy moves.
   2. If the copy is a symlink (`test -L <path>`), stop. Do not move, verify or delete it.
   3. Verify the checkout copy itself, without the store: `env -u X3D_ORACLE_KEYS python3 -I -B tools/answer_keys.py verify <name>`. It must exit 0, and the path that it prints must be the absolute path of `oracle/<name>` in this checkout. A different path means that the resolver found another copy (for example in the main worktree), so stop. A valid copy in the store does not replace this check.
   4. Move the copy into the holding directory, at its relative path: `mkdir -p -- <holding>/oracle`, then `mv -T -- oracle/<name> <holding>/oracle/<name>` (same filesystem). Record its three manifests (step 3) in the holding directory.

   If step 3 fails or cannot run, stop. Do not move, delete or replace the copy. Report the name and the output of `verify`. An affected key path that is already empty holds no copy, so steps 2–4 do not apply to it.
2. A verified holding copy of every key exists outside every worktree: the step 0 store checks pass (24 of 24 rows, `sha256sum -c` of `SHA256SUMS`, and the layout check).
3. `python3 -I -B tools/answer_keys.py guard . HEAD $T` passes, with and without `X3D_ORACLE_KEYS`.
4. Steps 3 and 4 copied every listed path, and each copy equals all three source manifests (file hashes, symlink targets, and type and mode).

Stop on any unexpected state: a check that fails or cannot run, a path that is not empty, a count that differs from the record, or a file, symlink or directory that you did not expect. Do not try a repair with Git, and do not move data to make a failed check pass. The planned move of condition 1 happens inside step 0, not after a check fails. Record what you found, keep the holding directory, and report.

After step 5, run step 6. Then run this step 7 list in place of a plain repeat of step 0. A moved copy is not in the checkout at this point, so step 7 does not verify it there:

1. The store checks of step 0.
2. The checkout copies at unaffected key paths, as in step 0 (with the printed-path check).
3. Each moved copy: its key path is empty (`test ! -e <path> && test ! -L <path>`), and the holding copy equals the three manifests that condition 1 recorded.
4. `guard . HEAD`, with and without `X3D_ORACLE_KEYS`. HEAD is now `$T`.
5. The 8 render-key paths and `upstream` are empty.

Stop on any failure, as in step 7. Then a moved checkout copy may go back to its key path, but only when `$T` does not track the path (`git ls-tree $T -- <path>` prints nothing) and `git check-ignore -q -- <path>` passes. For a directory key, give the path with a trailing slash (`oracle/<name>/`): for a path that does not exist, Git does not match a `dir/` rule without it. Copy it from the store (`cp -R -p`). Check it with `env -u X3D_ORACLE_KEYS python3 -I -B tools/answer_keys.py verify <name>`: it must exit 0 and print the absolute path of `oracle/<name>` in this checkout. Then run `guard . HEAD` again, with and without `X3D_ORACLE_KEYS`. Keep the holding directory. Record the exception, its checks and their results on issue #65.

The 2026-10-29 update of the main checkout from `cdfbffc7` to `a5a6b6f5` used this exception. It deleted the tracked `upstream` symlink and 3,605 tracked files below 11 #168 paths. Before the update, the 8 render keys and the 11 #168 paths were moved out of the checkout, and `upstream/` was moved to a holding directory, so every affected path was empty. The update passed steps 6 and 7. The 12 #168 paths were then restored from the store as ignored files.

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
