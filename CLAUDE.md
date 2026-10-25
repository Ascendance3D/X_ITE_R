# X_ITE Cybertown Legacy Compatibility — Agent Guide

This directory (`~/Projects/cybertown/X_ITE`) is the lane root for a preservation /
upstream-contribution project: making historically valid Cybertown / blaxxun VRML97
content run correctly in modern [X_ITE](https://github.com/create3000/x_ite), by
contributing *generally useful* compatibility improvements upstream.

`AGENTS.md` beside this file is a **symlink to this file**, matching the convention
every repo in the `~/Projects/cybertown` workspace uses — both names serve identical
bytes, so they cannot drift. This guide is the single source of truth for the
read-only archive roots, the preservation principle, the clean-room rule, the ban on
hard-coding Cybertown/blaxxun specifics into X_ITE, fork/remote/branch facts, phase
discipline, and the X_ITE subsystem map. The workspace-root guide at
`~/Projects/cybertown/CLAUDE.md` still applies and takes precedence on anything about
workspace layout, backups, worktrees, and production access.

---

## Layout

```
X_ITE/
├── AGENTS.md          -> pointer to this file
├── CLAUDE.md          <- you are here
├── README.md          human-facing overview
├── .gitignore         guards the *lane root*, not the fork
├── x_ite/             Git clone of DJAscendance/x_ite (fork of create3000/x_ite)
├── x_ite-tests/       clone of create3000/x_ite-tests — NOT optional, see below
├── scratch/           throwaway QA harness + derived experiments — gitignored
│   └── qa/                headless test harness, see "Running X_ITE locally"
└── recon/             PRIVATE archaeology output — never pushed anywhere
    ├── REPORT.md          the Phase 0 findings report
    ├── scan.py            gzip-aware read-only archive scanner
    ├── snapshot.py        archive metadata snapshotter (integrity proof)
    ├── analyze.py         index -> dependency graph + summaries
    ├── manifests/         before/after archive metadata snapshots
    ├── index/             per-archive JSONL scan records
    ├── dependency-graphs/ machine-readable dependency edges + summaries
    ├── fixtures/          curated copies of historical evidence + provenance
    └── logs/              scanner logs
```

**Archived 2026-10-21 (Ryan's request).** `recon/` and every non-Rust item of
`scratch/` (including `scratch/qa/` and `scratch/phase1a-reviewed.patch`) now live in
`~/Projects/cybertown/.backups/X_ITE/2026-10-21-lane-cleanup/{recon,scratch}`, a
verified `rsync -aHAX` tree. Paths elsewhere in this guide still use the old
lane-root-relative names. The QA harness serves the lane root and depends on
relative symlinks, so to use it or `recon/*.py`, `rsync -aHAX` the item back into
this directory first. Do not run it from inside `.backups`. `X_ITE_R/` is the
X_ITE → Rust port repo (`Ascendance3D/X_ITE_R`). `scratch/rust-port` is now only a
symlink to `../X_ITE_R`, kept because X_ITE_R branches commit links through that path.
Do not delete it. The old Rust repos are in the same archive under `rust-old/`.

`recon/` is **outside** the Git clone on purpose. It is not a submodule, not a
worktree, and must never become one. The same goes for `scratch/`.

### A different X_ITE lives next door

`~/Projects/cybertown/new-items` contains **another fork of X_ITE**, used for the
Cybertown Mall checker/viewer. It is a separate lane with separate goals. Do not
patch it as a side effect of work here, do not copy changes between the two without
being asked, and do not assume a file path means the same thing in both. When a task
says "the fork", it means `X_ITE/x_ite` unless it names `new-items`.

---

## Hard rules

### 1. The archives are strictly read-only

Two source trees are authorized for **reading only**:

| key | path |
| --- | --- |
| `skatefm` | `/home/ryan/Downloads/Skate.FM Submissions - 202608141549/Cybertown` |
| `wbct` | `/home/ryan/Projects/cybertown/wb-ct-scrape` |

Under either root: no writes, renames, moves, deletes, chmod/chown, in-place
decompression or recompression, editor saves, line-ending or encoding normalization,
timestamp "repair", generated sidecar files, or temporary files. Do not run any
command that could mutate them as a side effect.

Open files with mode `rb`. Decompress **in memory**. Copying *out* is allowed.

Everything this lane generates lives under `~/Projects/cybertown/X_ITE` or `/tmp`.

Prove non-mutation at the end of any session that touched the archives:
`python3 recon/snapshot.py before` … work … `python3 recon/snapshot.py after`, then
diff the TSVs in `recon/manifests/`.

Note: `wb-ct-scrape` contains a private key (`localhost+2-key.pem`) and the
`Cybertown Backups/IVN*` trees carry real 2001 member PII and plaintext credentials.
Never read, copy, quote, or report any of it. Scan for VRML structure only.

### 2. Preservation over tidiness

Historical material is evidence. Preserve comments, creator names, dates, original
filenames, directory relationships, URLs, URNs, relative paths, old syntax,
malformed-but-historically-tolerated syntax, compression state, and exact bytes.

If something looks broken, **document what it actually says**. Do not silently correct
it. Do not "clean up" archival source material. Derive experiments into a separate
scratch location; never edit a fixture in place.

### 3. Clean-room rule for the public fork

Do **not** copy historical Cybertown or blaxxun assets into `x_ite/`. Their licensing
is unclear; they are evidence, not automatically upstream test fixtures.

Upstream contributions must use minimal synthetic reproductions or newly authored
clean fixtures, unless licensing/permission for an original is explicitly established.
`recon/fixtures/` may hold historical copies for research — with provenance and
hashes — and is never pushed.

### 4. Never hard-code Cybertown or blaxxun into X_ITE

Anti-patterns, permanently:

* `if (hostname === "blaxxun.com")`
* `if (world === "cybertown")`
* shipping a private archive inside X_ITE
* silently rewriting historical URLs without application control

Prefer instead: standards-compatible legacy parsing improvements, *generic* URL/URN
resolver hooks the embedding application drives, correct EXTERNPROTO behavior,
broadly useful compatibility diagnostics, accurate implementations of standardized
node behavior, and minimal regression tests.

Cybertown supplies the corpus. X_ITE receives general engineering.

---

## The fork

| | |
| --- | --- |
| account | `DJAscendance` |
| fork | https://github.com/DJAscendance/x_ite |
| clone | `~/Projects/cybertown/X_ITE/x_ite` |
| `origin` | `https://github.com/DJAscendance/x_ite.git` |
| `upstream` | `https://github.com/create3000/x_ite.git` |
| contribution base | **`upstream/development`** — see below |
| working branch | `ctr-legacy-compat-recon` (tracks `upstream/development`) |

**Branch target — `development`, not `main`.** Upstream's `.github/CONTRIBUTING.md`
says only "fork the repository and clone your fork"; it names no target branch. The
evidence does:

* `build/bin/merge-development.sh` (`npm run merge:development`) is
  `git checkout main; git merge development; git push origin; git checkout development`
  — the maintainer works on `development` and fast-forwards `main` at release.
* Both merged *code* PRs from outside contributors (#154, #156) targeted `development`.
  Every PR that targeted `main` was either a bot (ImgBot, Dependabot) or was closed
  unmerged (#171, #182).
* `development` is 2 commits ahead of `main` and 0 behind — it is the live branch.

`main` is only the GitHub *default* branch, which is what makes the mis-target easy.
Re-check this before opening any PR rather than trusting the table.

### Build and test facts

* Node 24 / npm 11 work. There is **no committed `package-lock.json`** — it is
  `.gitignore`d — so `npm ci` fails by design. Use `npm install`, as CONTRIBUTING says.
* `npm test` is `cd ../x_ite-tests && npm test` — the tests live in a **separate
  repository**, `create3000/x_ite-tests`, expected as a *sibling* of the clone. That is
  why `x_ite-tests/` exists at this lane root; it is not a stray directory, and the
  path is not ours to choose. The clone's own `tests/` directory is only a stub README
  plus module-import smoke files.
* **`x_ite-tests` is the suite the maintainer runs for X_ITE development.** Treat it as
  the upstream test authority, not an optional extra. Remotes: `origin` =
  `create3000/x_ite-tests` (upstream), `fork` = `DJAscendance/x_ite-tests`. Keep the
  local `main` fast-forwarded to `origin/main` before relying on it (`git fetch --all`,
  then `git merge --ff-only origin/main`). Our PR #1 (`fix/firefox-vrml-xml-parser`,
  `997e43d`) is merged upstream. On 2026-10-14 local `main` was synced to `d09ffba`;
  `fork/main` was still 10 commits behind (updating it needs a push).
* The **node** project runs: 4 pass, 1 fails on a missing Perl module (`Array::Utils`)
  that `tests/node/docs/documentation.pl` needs.
* The **browser** project (vitest + Playwright chromium) imports X_ITE from a fixed
  URL. Since upstream `ab10729` ("Update test imports to use localhost")
  `x_ite-tests/tests/browser/X3D.js` begins
  `import X3D from "https://localhost/x_ite/dist/x_ite.mjs"`; before that it was
  `https://weiputer/...`, a host on the maintainer's LAN. Running it needs the checkout
  served over HTTPS at `https://localhost/x_ite/` (CONTRIBUTING's "Setup a localhost
  server" step). Do not set that up without asking. Prefer authoring new fixtures
  against the `node` project.
* `npm run lint` (`eslint src`) reports **8243 pre-existing errors** on upstream `main`
  at `aad1c12`, dominated by `no-useless-assignment` (6665) and `no-constant-condition`
  (1007). This is the upstream baseline. **Do not fix unrelated lint failures.**

### Running X_ITE locally without the maintainer's host

The browser test project is unrunnable here, but X_ITE itself runs fine locally, and
QA should not depend on the upstream suite. `src/` has **zero non-JS imports**, so the
unbundled production source loads directly in a browser as plain ES modules — no
`npm run dist`, no bundler, and you are testing the real files you just edited.

`scratch/qa/run.mjs` does this end to end: it serves the lane root, loads each case in
**headless** chromium (borrowed from `x_ite-tests/node_modules`), and prints the
console output plus each url object's final load state.

```
node scratch/qa/run.mjs                      # all synthetic cases
node scratch/qa/run.mjs case-b-inline-exhausted
node scratch/qa/run.mjs /scratch/qa/hist/...  # a staged historical scene
```

Four things this cost a day to learn — do not rediscover them:

* **Headless. Always.** Do not drive QA by opening visible browser windows; a
  case-by-case sweep turns into a screenful of them.
* **Serve with `Cache-Control: no-store`.** Chromium will otherwise happily reuse a
  previously fetched `FileLoader.js` and you will spend an hour testing the old code
  while the file on disk is patched. Verify with `page.evaluate` that the function
  source you expect is actually loaded before trusting a result.
* **The server must be threaded.** ESM fan-out opens many parallel connections;
  single-threaded `python3 -m http.server` drops some and imports fail at random.
* **`src/` expects `node_modules` as its *sibling*** (`../../../node_modules/...`), and
  X_ITE resolves its own `assets/` relative to the *page* URL. `scratch/qa/` therefore
  mirrors `src/`'s layout with symlinks rather than holding copies.

DNS failures for a dead host are cached per browser context, so give each synthetic
case its **own** invalid hostname or a later case will silently log nothing.

---

## Phase discipline

| lane | state |
| --- | --- |
| Phase 0 — reconnaissance | complete; see `recon/REPORT.md`. Changed no X_ITE source. |
| Phase 1A — resource-failure diagnostics | **submitted upstream, awaiting review** — see below |

### Phase 1A submission record (2026-08-17)

| | |
| --- | --- |
| scope | `src/x_ite/InputOutput/FileLoader.js` only, +58/−7 across two commits |
| commits | `f2d764eff` (initial), `eaddb8422` (Copilot review corrections) |
| branch | `fix/resource-fallback-diagnostics` (public name; the recon branch `ctr-legacy-compat-recon` was *not* used) |
| base | `upstream/development` at `f1a36451a` |
| fork push | `DJAscendance/x_ite` |
| PR | [create3000/x_ite#232](https://github.com/create3000/x_ite/pull/232) → `create3000/x_ite:development`, maintainer-can-modify enabled |
| QA | independent QA pass, plus 15 focused behavioural checks re-run headless on the current upstream base |
| build | source-only. No `dist/` rebuild, no lockfile, no `x_ite-tests` change |

The QA harness for this lane is `scratch/qa/run.mjs` with 19 synthetic cases; the
dynamic ones (url-field mutation) run from `scratch/qa/harness-dyn.html`. The reviewed
patch is preserved outside the clone at `scratch/phase1a-reviewed.patch`.

**Copilot review round (2026-08-17), commit `eaddb8422`.** Both inline findings
reproduced and were real:

* `printError` read `this .fileURL`, which `loadDocumentAsync` assigns *after* the
  `javascript:`/`vrmlscript:` branch returns. A failing script candidate therefore
  inherited the previous candidate's resolved URL and protocol — or, when it was the
  first candidate, the constructor's base URL, so the diagnostic named the containing
  world file. Fixed with a `this .resolvedURL` that is cleared per candidate and set
  only where resolution actually happens.
* The exhausted-fallback `console .error` passed only a string, discarding every
  `Error` and stack. Now `console .error (summary, ... errors)`.

`Script` is the node that exercises script URLs (`Script.createContext` evaluates
source with no try/catch, so a syntax error lands in `loadDocumentError`). Cases
`case-o`…`case-s` cover it. The harness records console *argument kinds*, not just
rendered text — that is the only way to tell a real `Error` from its stringified
message, and it is what proved finding #2.

Upstream moved from `7f4900a2` to `f1a36451a` during this lane, but `FileLoader.js`
itself was untouched upstream, so the reviewed patch reapplied byte-identically
(SHA-256 `229fe228…`). Re-verify rather than trusting that if you rebase again.

Both Copilot threads were resolved on 2026-08-17 after independent QA passed the
corrections. PR #232 is now waiting on the maintainer.

#### Deferred: falsy thrown values are not recorded as attempts

**Not part of PR #232, and deliberately not raised upstream yet.**
`FileLoader .printError ()` opens with a pre-existing guard:

```js
if (!error)
   return;
```

So a candidate that fails by throwing a falsy value — `throw null`, `throw 0`,
`throw false`, `throw ""` — is never pushed onto `attempts` and never appears in
either the per-candidate warning or the exhausted-fallback summary. Fallback itself
still proceeds correctly and the load state is right; only the diagnostic is silent.

This predates PR #232 — the guard is upstream's, and Phase 1A neither introduced nor
worsened it. Independent QA agreed it does not block the PR.

A future narrow fix would look at replacing the broad falsy test with an `undefined`
sentinel check, since the one caller that legitimately passes nothing is
`loadDocument`'s empty-`MFString` path, which must stay silent. Do not implement this
or open an upstream issue for it without a lane brief that says so.

Every implementation lane must satisfy all three of:

* the clone's working tree starts clean and free of lane commits
* the change is justified by evidence in `recon/`, not by memory or assumption
* the change is defensible to upstream on general X3D/VRML97 grounds

**`Inline`, `PROTO` and `EXTERNPROTO` are legitimate subjects of this work, not
forbidden ground.** They are the mechanisms the historical corpus is built out of;
half the useful contributions here will touch them. What is gated is not the *topic*
but the *authorization*: implement a given change only when the active lane brief
calls for it.

Currently out of scope until a lane brief says otherwise — the generic URL resolver
hook, blaxxun URN support, recovered-host mapping, `NurbsSurface` / `TrimmedSurface`
compatibility, ROUTE-inside-MFNode parsing, VRML 1.0 support, cp1252 decoding, and
any change to parser leniency, URL resolution semantics, or Script behavior.

**Nothing leaves this machine without Ryan's say-so.** No commit, push, issue, PR,
or message to the maintainer unless the active lane authorizes it — and local QA
comes first regardless.

---

## Where things live in X_ITE (as of upstream `aad1c12`, v16.1.2)

| concern | path |
| --- | --- |
| VRML97 / classic encoding parser | `src/x_ite/Parser/VRMLParser.js` |
| format dispatch (has public `GoldenGate.addParsers`) | `src/x_ite/Parser/GoldenGate.js` |
| shared parser base, encoding conversion | `src/x_ite/Parser/X3DParser.js` |
| URL fetch, MFString fallback chain, gzip | `src/x_ite/InputOutput/FileLoader.js` |
| gzip magic sniffing + `DecompressionStream` | `src/lib/helper.js` (`$.gunzip`) |
| text decoding (`new TextDecoder()`, UTF-8 only) | `src/lib/helper.js` (`$.decodeText`) |
| `Inline`, `Anchor`, `X3DUrlObject` | `src/x_ite/Components/Networking/` |
| `PROTO` / `EXTERNPROTO` declarations | `src/x_ite/Prototype/` |
| NURBS component (16 nodes) | `src/x_ite/Components/NURBS/` |
| scene/execution context | `src/x_ite/Execution/` |
| X_ITE-internal asset URLs | `src/x_ite/Browser/Networking/URLs.js` |

---

## Search and navigation

Follow the global rules in `~/.claude/CLAUDE.md`: LSP first, `ast-grep` for structural
queries, `rg` / `fd` for text and filenames. Scope searches to the smallest relevant
directory.

**`rg -r` is a replacement flag, not "recursive."** Ripgrep recurses by default. Using
`rg -rn PATTERN src` rewrites matched lines in the output and produces hundreds of KB
of garbage. This has already bitten this lane once.

For the archives, prefer `recon/scan.py` over `rg`: a plain text sweep silently misses
every gzip-compressed `.wrl`, and **60.8% of the surviving corpus (8,888 of 14,607
files) is gzipped while still carrying a `.wrl` extension.** A text-only sweep sees
well under half the evidence.

---

## Agent skills

### Issue tracker

Issues live in GitHub Issues for `Ascendance3D/X_ITE_R`, via the `gh` CLI. See `docs/agents/issue-tracker.md`.

### Triage labels

Default five-role vocabulary (`needs-triage`, `needs-info`, `ready-for-agent`, `ready-for-human`, `wontfix`). See `docs/agents/triage-labels.md`.

### Domain docs

Single-context: one root `GLOSSARY.md` + `docs/adr/`, shared by all workspace crates. See `docs/agents/domain.md`.
