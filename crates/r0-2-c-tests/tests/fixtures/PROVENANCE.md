# R0.2-C1 test fixtures and expected values

These files are test expectations for `crates/r0-2-c-tests`. They are not regression
baselines. No file under `oracle/` was created or changed.

## `xite/*.wrl` — synthetic inputs

Written for R0.2-C1. They contain no historical or third-party content.
`minimal-gzip.wrl` is `gzip -n -9` of `minimal.wrl`. `truncated-gzip.wrl` is the 4 bytes
`1f 8b 00 00` used by R0.2-C.

## `xite/ORACLE.json` — pinned X_ITE output

Produced by the repository's own oracle harness, unchanged:

```
ln -s /home/ryan/Projects/cybertown/X_ITE/x_ite/node_modules oracle/x_ite-pin/node_modules
mkdir oracle/c1-stage && cp crates/r0-2-c-tests/tests/fixtures/xite/*.wrl oracle/c1-stage/
node oracle/x_ite-dump.mjs oracle/c1-stage /tmp/r02c1/xite-oracle.json
rm -r oracle/c1-stage
```

`pinned` is `34cdf9f97` (X_ITE 16.5.0, `oracle/x_ite-pin`). Headless Chromium from
Playwright; non-local requests aborted. 19 of 22 files load; the 3 failures are the
expected-error fixtures.

## `js-oracle.json` — Math, number format, TextDecoder

`node tools/probe-js.mjs`. The same function runs in node v24.21.0 and in headless
Chromium 151.0.7922.34. It applies X3DGenerator.js `FloatFormat` / `DoubleFormat`
(pinned 34cdf9f97) with the same `Intl.NumberFormat` options. The two engines agree on
every value.

## `matrix-oracle.json` — X_ITE Matrix4

`node tools/probe-matrix.mjs`. Imports pinned `standard/Math/Numbers/Matrix4.js`,
`Vector3.js`, `Rotation4.js` in node v24.21.0 and in headless Chromium 151. All values are
identical except `factor` rotation `y`: node `0x3ff000000000000d`, Chromium
`0x3ff000000000000c`.

## Recorded vectors reused from the port

`crates/x3d-fields/tests/chrome_format.rs` (Chrome 155 `Intl.NumberFormat`) and
`crates/x3d-fields/tests/chrome_math.rs` (Chrome 155 `Math.*`). See `oracle/README.md`,
"Followers".

## Node data counts

`crates/vrml97/data/x_ite-nodes.json`: 263 `nodes`, 39 `components`, 8 `profiles`
(counted with Python `json`).
