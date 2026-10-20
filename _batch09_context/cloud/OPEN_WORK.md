# Batch 09 cloud: open work

Branch `feature/xite-rust-rea-b09-cloud`. X_ITE 16.5.0 @ `34cdf9f97` → native Rust only.

## ENVIRONMENT-BLOCKED tests (not PASS; owner decision 3, 2026-10-20)

The cloud container has no Vulkan adapter, lacks two gitignored answer keys and blocks a UDP probe. These 8 tests fail there
identically before and after the batch 09 cloud commits. They are not removed, not ignored and their limits are unchanged. They
must be run on the RTX 3060 Ti desktop with `_batch09_context/cloud/local_validate.sh` (exact commands inside).

| test | binary | blocked by | needs |
| --- | --- | --- | --- |
| `items_runtime_matches_x_ite` | `x3d-script --test corpus` | `oracle/items-baseline.json` missing (gitignored, 228 MB) | the earlier local file (sha256 prefix `8f4bb11593e98f85`), or `node oracle/x_ite-dump.mjs oracle/corpus-items oracle/items-baseline.json` from the pin, 3 identical recordings. Source data in git: `oracle/corpus-items/` (606 `.wrl`, 21 `.js`, 231 `.json`) |
| `x1_runtime_matches_x_ite` | `x3d-script --test corpus` | `oracle/x1-baseline.json` missing (gitignored, 86 MB) | the earlier local file (sha256 prefix `fbe54fa2a0e89f3a`), or `node oracle/x_ite-dump.mjs oracle/corpus-x1 oracle/x1-baseline.json`, 3 identical recordings. Source data in git: `oracle/corpus-x1/` (500 files) |
| `rea_batch09_nav_shard0` … `shard3` (4) | `x3d-viewer --test rea_batch09_nav` | "the pointing and collision passes need a Vulkan adapter" | GPU with Vulkan (`vulkaninfo --summary`) |
| `rea_batch09_inputs_cases_match_x_ite` | `x3d-viewer --test rea_batch09_inputs` | collision-walk: 38 frames differ on the CPU fallback (same on `f008c67`) | GPU with Vulkan |
| `rea_batch08_net_cases_match_x_ite` | `x3d-viewer --test rea_batch08_net` | dis-runtime: "udp: port 41234 sent 2 refused 1" (the container's loopback does not answer every send with port unreachable) | 127.0.0.1 UDP 41234 and 41235 free with no listener (sender targets), 41236 free (the test's listener); no firewall rule dropping ICMP on `lo` |

## Remaining work, hardest first

1. scr-browser-open: 38 Browser object probes (properties and methods) differ.
2. Execution context / SAI timing differences A-D (ctxev-inline, ctxev-startup-proto, ie-err-string-frames, cfg-scene-odd).
3. scr-classes-open: 19 globals Script.js defines that the port does not (ComponentInfoArray, X3DProtoDeclaration, …).
4. Field representation differences (sub:fields.types): an SF component that is neither number, undefined nor null is stored as
   NaN by the runtime; IterableWeakSet strong members (QuickJS); external-component X3DConstants order (X_ITE nondeterministic).
5. Step 3 promotion candidates: sub:exec.import-export, sub:exec.bindable-stacks, sub:config.components-profiles,
   sub:exec.context-api.
6. scr-url-order-open (url candidates that need a fetch finish frames later in X_ITE); meta-parse-open (PROTO field value printed
   with `DEF _1`, sub:proto.proto); the Rust XML MFString warning lacks the field name (parser).
7. sub:fields.math: compare the Rust Geometry math (Box3, Line3, Plane3, Triangle, ViewVolume, Spline) with X_ITE's
   standard/Math/Geometry directly.
8. The rest of the REA queue.
