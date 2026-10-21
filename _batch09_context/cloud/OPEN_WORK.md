# Batch 09 cloud: open work

Branch `feature/xite-rust-rea-b09-cloud`. X_ITE 16.5.0 @ `34cdf9f97` → native Rust only.

## Hardware-dependent tests: locally validated (owner decision 2026-10-20)

The cloud container has no Vulkan adapter, lacks two gitignored answer keys and blocks a UDP probe, so these 8 tests cannot run
there. The owner ran them with `_batch09_context/cloud/local_validate.sh` and accepted the results as PASS:

- Environment: Linux desktop, NVIDIA GeForce RTX 3060 Ti, pinned X_ITE 16.5.0 @ `34cdf9f97`.
- Validation commit: `0f3de084d8c0630018f8f15e65dd15c9ad0aa749`.
- Result: 8 passed, 0 failed, 0 blocked.
- Answer keys: the historical `oracle/x1-baseline.json` (sha256 `fbe54fa2a0e89f3a…`) and `oracle/items-baseline.json`
  (`8f4bb11593e98f85…`) were used unchanged; their hashes match the recorded ones. They are not re-recorded.

| test | binary | why the cloud cannot run it |
| --- | --- | --- |
| `items_runtime_matches_x_ite` | `x3d-script --test corpus` | `oracle/items-baseline.json` not in git (228 MB) |
| `x1_runtime_matches_x_ite` | `x3d-script --test corpus` | `oracle/x1-baseline.json` not in git (86 MB) |
| `rea_batch09_nav_shard0` … `shard3` (4) | `x3d-viewer --test rea_batch09_nav` | pointing and collision passes need a Vulkan adapter |
| `rea_batch09_inputs_cases_match_x_ite` | `x3d-viewer --test rea_batch09_inputs` | collision-walk needs the GPU path |
| `rea_batch08_net_cases_match_x_ite` | `x3d-viewer --test rea_batch08_net` | the container's loopback does not answer every UDP send to 41234/41235 with port unreachable |

### The three time-dependent x1 worlds

`244-tardis666.wrl`, `265-tardis666.wrl` and `87-weaver_anime_final_fixed2.wrl` read `new Date ()` in a Script: the first two
write the wall clock's hours:minutes:seconds into a Text node, the third seeds a clock whose hand rotations follow from it. The
historical baseline holds the wall clock of the moment X_ITE recorded it ("8:05:17", "8:05:47"), so no new X_ITE capture, and no
Rust run, can reproduce it byte for byte. This is a property of the worlds, not a Rust defect.

- They stay listed in `crates/x3d-script/tests/x1-known.tsv` (X6 nondeterministic) against the historical baseline.
- `Date.now` is not frozen (owner decision; `oracle/README.md`: freezing it stalls X_ITE's loading). The oracle freezes only the
  browser's `getCurrentTime`.
- The three-identical-recordings rule stays for every deterministic case. For these three the established historical baseline
  is the reference and the reason above is why a new capture cannot equal it.

## Remaining work, hardest first

1. Load timing that follows Chrome's I/O, not X_ITE's code (documented open, root cause found): after a fetch, FileLoader reads the
   body through `getBlob` (`reader .read ()` loop) and `$.gunzip` (`blob .slice () .arrayBuffer ()`, `blob .arrayBuffer ()`); those
   reads complete on Chrome's I/O thread, after the harness's `settle` (which waits for requests in flight and three message
   turns). So a fetched Script candidate initializes frames later (scr-url-order-open: frame 4, a base64 data URL after the files)
   and a scene whose EXTERNPROTO / Inline / Script file is fetched resolves 1-2 frames later (parse-def-proto 37, 38, 40;
   parse-loads). Matching it would mean modelling Chrome's I/O thread; the oracle is not changed to hide it. The microtask part
   matches (scr-url-order-micro).
2. Text during the oracle's warm-up (node-text-outputs-open): anim.html loads the world on wall-clock animation frames, so the frame
   the default font arrives in is not X_ITE's logic. After the warm-up the rebuild cascade, the FontStyle load timing (timer,
   uncached font file) and every value match (text-events, text-fontstyle, text-family). ScreenFontStyle's ScreenText outputs are
   not modelled.
3. toString / valueOf calls a node's own math makes on a raw component (fldx-print-count-open: 52 vs 57 = Transform 3,
   TextureTransform 2 when the value arrives). Prints and the oracle's dumps convert raw components as X_ITE does (fldx-count
   exact); the runtime keeps NaN for an object component and converts it only where it prints it.
4. Engine internals of the objects an execution context hands out (scx-layers-open): the engine's own members (connect,
   refreshInstance, newInstance, loadNow, the X3DBaseNode field machinery); X3DScene.dispose. Printing them matches since this
   checkpoint (scx-print).
5. Node class chains: a node X_ITE hands out is an instance of its node class; the port's is an X3DNodeRef (owner boundary
   2026-10-20); getType () matches (brx-call-open).
6. Browser boundary kept open (owner decisions 2026-10-20): brx-api-open, brx-call-open, scr-classes-open (no DOM), X3DConstants of
   external-component node types.
7. sub:fields.math: Triangle3.triangulatePolygon (libtess) not compared directly; everything else bit-exact (3567 cases).
8. The 84 PARTIAL entries; the 33 nodes need the desktop GPU run for visual evidence before promotion proposals.

Kept documented (not defects of the port's model): the text of TypeErrors the engine words (cfg-scene-odd: QuickJS "cannot read
property 'name' of undefined", V8 "Cannot read properties of undefined (reading 'name')"; the thrown class and the state are
compared in cfg-comp).
