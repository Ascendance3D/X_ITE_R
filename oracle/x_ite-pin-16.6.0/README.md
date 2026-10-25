# X_ITE 16.6.0 reference pin (inert)

An **immutable identity record** for X_ITE 16.6.0. It is *not* the active oracle
reference. No crate and no oracle tool reads this directory at run time;
`tools/candidate.lock` still points `X_ITE_REF_DIR` at `oracle/x_ite-pin`
(16.5.0 @ `34cdf9f97`), and that target is unchanged. (The lock's own stale
*measurements* for the 16.5.0 tree were re-blessed in the same change that added this
directory — see the audit's §2.3. The pinned commit and version were not touched.)

"Immutable" is enforced, not just asserted: `tools/verify-reference-pin.py` checks every
file here against the manifest and the identity below, and CI runs it on every pull
request.

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

Re-check it at any time. This is what CI runs, and it needs only Python — no GNU
`sha256sum`, which macOS does not ship:

```sh
python3 tools/verify-reference-pin.py            # manifest + tree digest + identity
python3 tools/verify-reference-pin.py --self-test  # prove the checker still refuses a tamper
```

It covers more than the manifest does on its own: an **extra** file under `src/` that the
manifest never listed, the manifest's own SHA-256, the recorded tree digest, and the four
identity records (`SOURCE_SHA`, `PINNED_COMMIT`, `package.json`, both `BROWSER_VERSION.js`
copies). The expected values live in the checker *and* in the files here, so editing one
side alone makes them disagree and the check fails.

The GNU equivalents, for a human cross-check:

```sh
(cd oracle/x_ite-pin-16.6.0/src && sha256sum -c ../MANIFEST.sha256)
python3 tools/reference-digest.py oracle/x_ite-pin-16.6.0/src   # -> 846 1972df1f…
```

The 11 harness pages the active pin carries on top of upstream `src/`
(`anim.html`, `anim-msaa.html`, `anim-sized.html`, `dump.html`, `extern.html`,
`inline.html`, `nodes.html`, `shot.html`, `shot-log.html`, `shot-msaa.html`,
`zfight.html`) are **ours**, not upstream's, and are not in `MANIFEST.sha256`.
A runnable 16.6.0 reference needs them copied in and re-checked against 16.6.0's
`src/x_ite.*` entry points. That is the **one remaining step** before this tree can
record oracle captures, and it belongs to the Phase 4 oracle lane, not here — the
pages must be verified against 16.6.0's entry points rather than assumed.
