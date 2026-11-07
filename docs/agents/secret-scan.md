# Secret scan

`tools/secret_scan.py` scans this repository with Gitleaks and TruffleHog. It runs offline and its output holds no
candidate value.

```
python3 -I tools/secret_scan.py             # tracked files at HEAD, then all history
python3 -I tools/secret_scan.py --tree      # tracked files at HEAD only (git archive export)
python3 -I tools/secret_scan.py --history   # every commit reachable from any ref
python3 -I tools/test_secret_scan.py        # self-test: synthetic positive and negative cases
```

Pinned tools: Gitleaks 8.30.1, TruffleHog 3.99.8. Another version stops the scan (exit 2). Both scans together take
about 2 minutes on a 24-thread machine; each scanner call has a 900 s limit.

## Offline

Each scanner runs in a new network namespace (`unshare -rn`) that has no interface except loopback. No
verification request, update check or other connection can leave the machine. TruffleHog also gets
`--no-verification` and `--no-update`, but those flags alone are not the guarantee. When the namespace cannot be made
(for example, a host that blocks unprivileged user namespaces), the scan stops with exit 2. There is no option to run
with network access. The self-test checks that a connection from the namespace fails.

## Output and exit codes

Each finding line has the tool, the rule or detector, the file, the commit (12 digits), the line, the value length and
the first 12 hex digits of the SHA-256 of the value. Gitleaks runs with `--redact=100`. The script reads TruffleHog
values in memory only, to hash them.

| exit | meaning |
| ---: | --- |
| 0 | every finding is reviewed and every file was read |
| 1 | at least one NEW finding |
| 2 | tool error: missing tool, wrong version, no network namespace, a scanner failed or did not report its end |
| 3 | files that TruffleHog could not read (`NOT SCANNED`); a coverage gap, not a pass. Exit 1 has precedence over 3 |

A detector result is not a classification. Exit 0 means that the two detectors found nothing new; it does not prove
that the history holds no secret. TruffleHog ran without verification, so a finding is never proof that a credential
is inactive.

## Exceptions

There are no path exclusions. Each exception names an exact case:

- **Gitleaks**: `.gitleaks.toml` adds one `[[allowlists]]` entry to the default rules. It needs all three of: the rule
  `generic-api-key`, one of the five removed CodeMirror 5 baseline files (`oracle/cm5-*-baseline.json`, history only),
  and a match that is exactly `tokenHash":"<64 lowercase hex digits>"`. These values are SHA-256 digests that
  `oracle/cm5-modes-tokens.mjs` made (1,692 findings in commit `16b763ad`).
- **TruffleHog**: `tools/secret-scan-reviewed.tsv`. An entry covers one detector, one exact path and one exact value
  (the SHA-256 of the raw value). The same value in another file, or another value in the same file, is NEW. An
  `unreadable` entry covers one Git blob ID of a file that TruffleHog cannot read; a changed blob is NOT SCANNED again. Digests and blob IDs are written in 8-digit groups joined by `:`, so the list itself has no long hex run for a
  detector to match.

To review a new TruffleHog finding, run with `--template FILE`. The file gets one line per new finding with the full
value hash and `TODO classify`; it holds no value. Find the value in its blob and identify the structure around it
(for example, a digest field, a data URI or a base64 sample array). Add the line only when that check shows that the
value is not a credential. Never add a path-only or detector-only line.

The self-test (`tools/test_secret_scan.py`) makes a disposable repository with values made at run time. It checks
that the Gitleaks exception holds only for its exact file and shape, that a credential-format token is found by both
scanners, and that a TruffleHog review entry does not cover the same value in another file.

## Reviewed findings (2026-11-07)

| scan | tool | count | classification |
| --- | --- | ---: | --- |
| history | Gitleaks | 1,692 | `tokenHash` SHA-256 digests, removed CodeMirror 5 baselines (allowlist above) |
| tree, history | TruffleHog | 19, 21 | parts of SHA-256 file digests, PNG data URIs and base64 float32 audio sample arrays (13 distinct values) |
| tree, history | TruffleHog | 9 files, 12 blobs | not readable by TruffleHog: raw float32 sample files and 2-to-4-byte truncated gzip fixtures |

Keep these scans separate from issue #168 (historical asset provenance). A scanner result says nothing about
redistribution permission.
