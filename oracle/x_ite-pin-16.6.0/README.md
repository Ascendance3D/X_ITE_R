# X_ITE 16.6.0 reference pin (inert)

An **immutable identity record** for X_ITE 16.6.0. It is *not* the active oracle
reference. Nothing in this repository reads this directory; `tools/candidate.lock`
still points at `oracle/x_ite-pin` (16.5.0 @ `34cdf9f97`) and is unchanged.

| | |
| --- | --- |
| upstream | `create3000/x_ite` |
| source SHA | `5f282709c124ffa4e2bbd57d9b199893bf94bd32` |
| tag | `16.6.0` (the tag points at exactly this commit) |
| author / date | Holger Seelig, 2026-10-21 16:03:21 +0200 |
| `package.json` version | `16.6.0` |
| `BROWSER_VERSION.js` | `16.6.0` |
| `src/` files | 846 |
| manifest | `MANIFEST.sha256` — per-file SHA-256 of every `src/` blob |
| manifest SHA-256 | `ef02ce267103193df166cff0e6f1875c9a50c0ff5d595aa629028a56e0185542` |
| `src/` tree digest | `846 1972df1f225b620acf979cde1fac1da29f11cda05b7894a10cc4832fc5960891` (`tools/reference-digest.py`) |

## `src/` is materialized and verified

The tree is here, 846 files, extracted with `git archive` from
`5f282709c124ffa4e2bbd57d9b199893bf94bd32` and **not modified**.

Verified 2026-10-24:

| check | result |
| --- | --- |
| file count under `src/` | 846, matching the manifest's 846 entries |
| `sha256sum -c` from the extracted tree | 846 OK, 0 failed |
| `sha256sum -c` from this committed tree | 846 OK, 0 failed |
| files in upstream `src/` missing here | none |

Re-check it at any time:

```sh
cd oracle/x_ite-pin-16.6.0/src && sha256sum -c ../MANIFEST.sha256
python3 tools/reference-digest.py oracle/x_ite-pin-16.6.0/src
```

The 11 harness pages the active pin carries on top of upstream `src/`
(`anim.html`, `anim-msaa.html`, `anim-sized.html`, `dump.html`, `extern.html`,
`inline.html`, `nodes.html`, `shot.html`, `shot-log.html`, `shot-msaa.html`,
`zfight.html`) are **ours**, not upstream's, and are not in `MANIFEST.sha256`.
A runnable 16.6.0 reference needs them copied in and re-checked against 16.6.0's
`src/x_ite.*` entry points. That is the **one remaining step** before this tree can
record oracle captures, and it belongs to the Phase 4 oracle lane, not here — the
pages must be verified against 16.6.0's entry points rather than assumed.
