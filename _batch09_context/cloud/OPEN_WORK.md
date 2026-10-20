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

1. Event-loop timing that follows the oracle page's network, not X_ITE's code (documented open): a fetched Script candidate (a file,
   a base64 data URL) completes 3 frames later in the oracle (the port: the same frame), and Chrome completes a base64 data URL's
   fetch after the files' (scr-url-order-open); a scene a Script asks for whose EXTERNPROTO, Inline or Script file is fetched
   resolves 1-2 frames later (parse-def-proto snippets 37, 38, 40; parse-loads). The files are served locally by the oracle, so
   network access does not change this. The microtask part matches since this checkpoint (scr-url-order-micro).
2. Text's layout outputs (node-text-outputs-open): X_ITE sends origin, textBounds and lineBounds when it rebuilds the text, after
   its default font has loaded, twice for one change; the port computes the same values (x3d-text oracle test, 1e-6) but its
   runtime sends no such events. Needs X_ITE's rebuild triggers and the font load modelled.
3. Engine internals of the objects an execution context hands out (scx-layers-open, documented, not PASS): X_ITE's X3DRoute,
   X3DImportedNode, X3DExportedNode, X3DProtoDeclaration and X3DExternProtoDeclaration prototypes also carry the engine's own
   members; the port's carry the public ones (scx-members). Not passed to Scripts: a loaded EXTERNPROTO's appinfo and
   documentation, toVRMLString / toXMLString / toJSONString of a route, an imported or an exported node, X3DScene.dispose.
4. Node class chains: a node X_ITE hands out is an instance of its node class; the port's is an X3DNodeRef (owner boundary
   2026-10-20); getType () matches (brx-call-open).
5. Browser boundary kept open (owner decisions 2026-10-20, documented, not PASS): brx-api-open, brx-call-open (page objects are
   null), scr-classes-open (Decision 1: no DOM, no stand-in objects), X3DConstants of external-component node types.
6. sub:fields.types (PARTIAL): the number of toString calls on a raw component (fldx-print-count-open: X_ITE calls it once per
   print of it; the port takes the texts of a host's raw values when its Scripts have run and before a Script prints); engine
   Intl / Error.stack wording; external-component X3DConstants order. IterableWeakSet: the bundled QuickJS holds X_ITE's own
   class's members strongly too (weak_set_engine.rs) and crashes on the WeakRef pattern, so the strong reference stays.
7. sub:fields.math (PARTIAL): the X_ITE geometry the port has no counterpart of is not compared (Box2, Triangle2, Spheroid3,
   Camera.frustum / perspective, the ViewVolume instance, Box3 beyond extents, Triangle3.triangulatePolygon, Algorithms). Everything
   the port has is bit-exact (geom_math.rs, 2434 cases).
8. NOT_TESTED nodes: field and event evidence (node-* worlds) is in; visual evidence needs the desktop GPU run.
9. sub:exec.context-api stays PARTIAL (owner decision 2026-10-20) until item 3 is done.
10. The rest of the REA queue; promotion review by exact REA ID.

Kept documented (not defects of the port's model): the text of TypeErrors the engine words (cfg-scene-odd: QuickJS "cannot read
property 'name' of undefined", V8 "Cannot read properties of undefined (reading 'name')"; the thrown class and the state are
compared in cfg-comp).
