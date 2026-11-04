# eXciteR (X_ITE_R)

[![Rust tests](https://img.shields.io/github/actions/workflow/status/Ascendance3D/X_ITE_R/rust-tests.yml?branch=master&label=Rust%20tests&style=flat-square&labelColor=0d1117&logo=githubactions&logoColor=white)](https://github.com/Ascendance3D/X_ITE_R/actions/workflows/rust-tests.yml)
[![Web tests](https://img.shields.io/github/actions/workflow/status/Ascendance3D/X_ITE_R/web-tests.yml?branch=master&label=Web%20tests&style=flat-square&labelColor=0d1117&logo=githubactions&logoColor=white)](https://github.com/Ascendance3D/X_ITE_R/actions/workflows/web-tests.yml)
[![license MIT](https://img.shields.io/badge/license-MIT-161b22?style=flat-square&labelColor=0d1117)](https://github.com/Ascendance3D/X_ITE_R/blob/master/LICENSE)
[![X_ITE 16.5.0 pin](https://img.shields.io/badge/X__ITE-16.5.0_pin-161b22?style=flat-square&labelColor=0d1117)](https://github.com/Ascendance3D/X_ITE_R/tree/master/oracle/x_ite-pin)
[![Rust 1.95](https://img.shields.io/badge/Rust-1.95-161b22?style=flat-square&labelColor=0d1117&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![WebAssembly WebGPU](https://img.shields.io/badge/WebAssembly-WebGPU-161b22?style=flat-square&labelColor=0d1117&logo=webassembly&logoColor=white)](https://github.com/Ascendance3D/X_ITE_R/tree/master/crates/x3d-web)
[![VRML97 X3D](https://img.shields.io/badge/VRML97-X3D-161b22?style=flat-square&labelColor=0d1117)](https://www.web3d.org/)

eXciteR is a Rust port of the [X_ITE](https://create3000.github.io/x_ite/) X3D browser. It loads VRML97 and X3D worlds, runs their events, sensors and scripts, and renders them with [wgpu](https://wgpu.rs/).

The port follows one pinned upstream version, X_ITE 16.5.0. Tests compare the Rust results with recordings of that version.

## Goals

- **Browser first.** The engine compiles to WebAssembly and draws with WebGPU. The `x3d-web` crate provides an `<x3d-canvas>` element with X_ITE's JavaScript API names. No X_ITE JavaScript runs in the page.
- **Native viewer.** `x3d-view` is a desktop application on the same engine. It is the main tool for testing rendering, input and audio against X_ITE outside a browser.
- **A reusable engine.** The crates can be used by other Rust applications that need X3D scene processing.

## Status

The project is under active development. There is no stable release. It does not have full X_ITE compatibility.

This README keeps two things apart. **Implemented** means the code exists and runs. **Verified** means the result is
compared with a recording of the pinned X_ITE 16.5.0, or with a reference decoder, by a test that fails when the result
differs. A merged pull request does not make a feature complete. The coverage ledger (`ledger/ledger.json`, see the REA
section) records the status of each upstream feature and what is still missing. Check it before you rely on a feature.

### Implemented

- Parsing of VRML97, X3D classic encoding (`.x3dv`), X3D XML (`.x3d`) and gzip-compressed worlds (`.wrz`), and glTF 2 and
  GLB loading (Draco, meshopt and some texture extensions are not decoded: ledger `sub:parse.gltf2`).
- Scene runtime: fields, ROUTEs, PROTO and EXTERNPROTO, Inline, TimeSensor animation, interpolators, sensors (touch, plane, key, string and others), and Script nodes (JavaScript through QuickJS).
- Rendering with wgpu: geometry, materials, textures, lighting, shadows, particles, volume rendering, text, picking and Gaussian splats.
- Navigation: EXAMINE, WALK, FLY, PLANE, LOOKAT and NONE, and viewpoint binding.
- Audio: Sound and AudioClip, URL fallback and spatial attenuation, on a Rust port of the Web Audio graph (native viewer). In the browser the page's own `AudioContext` plays the sound.
- Browser build: the `<x3d-canvas>` element (WebAssembly and WebGPU) with mouse, touch, wheel and keyboard input, a browser user interface (splash screen, timings, context menu) and a large subset of X_ITE's JavaScript API: scene creation, browser functions and options, the field classes and the X3D entry points. [crates/x3d-web/web/API.md](crates/x3d-web/web/API.md) marks each member as compared with a pinned X_ITE recording or only tested, and lists the differences. DOM integration is partial. [crates/x3d-web/web/API.md](crates/x3d-web/web/API.md) lists the supported members, the known differences and the members that are not implemented. It is the reference for the API, not this README.

### Verified compatibility and open work

The measured state is in the ledger and in the open issues. In short:

- **Compared with the pinned X_ITE:** most rendering, navigation, sensor, scripting and browser-API cases that the ledger marks COMPLETE (280 of 346 entries, see the REA section). The remaining entries are PARTIAL (44), DIFFERENT (21, recorded differences) or MISSING (1).
- **GPU backends:** the pictures were recorded and compared on one NVIDIA GPU (Vulkan), and the GPU tests run on that machine class. A Metal run on a physical Apple M1 passed 4 of the 16 GPU tests and failed 12; the classification is in [docs/gpu-failure-classification.md](docs/gpu-failure-classification.md). Metal parity is not established ([#89](https://github.com/Ascendance3D/X_ITE_R/issues/89), [#69](https://github.com/Ascendance3D/X_ITE_R/issues/69)).
- **GaussianSplats:** castShadow, picking, collision and the depth modes are compared with the pinned X_ITE. The sort is synchronous by an engineering decision, while X_ITE sorts in a worker ([#69](https://github.com/Ascendance3D/X_ITE_R/issues/69)).
- **Keyboard and sound on macOS and Linux Wayland:** not verified by a person on that hardware ([#88](https://github.com/Ascendance3D/X_ITE_R/issues/88)). HRTF spatial audio is a recorded difference (ledger `node:SpatialSound`).
- **Mobile browsers:** tested in desktop Chrome emulation only ([#143](https://github.com/Ascendance3D/X_ITE_R/issues/143)). WebGPU compatibility mode is not used ([#142](https://github.com/Ascendance3D/X_ITE_R/issues/142)).
- **Release packages** for all platforms do not exist.

### MovieTexture and codecs

| format | native viewer and tests | browser build |
| --- | --- | --- |
| GIF, APNG | decoded in Rust. APNG frames between the first and the last are not compared with the pinned page (it runs APNG on the wall clock) | decoded in Rust |
| WebM with VP8 or VP9 | decoded in Rust; pictures at fixed movie times are compared with the pinned page; Vorbis sound plays. Opus is not decoded | the page's `<video>` element plays it |
| WebM with AV1 | decoded through the vendored rav1d, **only when built with the non-default `av1` feature** (`--features x3d-viewer/av1`). The pictures equal the libaom reference decoder plane by plane on six synthetic streams. A damaged file must not crash the process: tests run such files in child processes | the page's `<video>` element plays it |
| MPEG-1 (video, and Systems audio Layer II and III) | decoded in Rust. The pinned page cannot play MPEG-1, so there is no recording to compare; the audio equals ffmpeg within 2e-4 | not implemented: the `<video>` element cannot play MPEG-1 |
| MP4, Ogg | no frames: the file is assumed playable | the page's `<video>` element plays it |

The ISO timing formulas of ISO/IEC 19775-1 18.4.2 (negative speed, seeking, pause and stop) are implemented for the native build and tested; the pinned X_ITE follows the browser's media clock instead. Details and the open items: [docs/movie-texture-timing.md](docs/movie-texture-timing.md), [docs/mpeg1-implementation.md](docs/mpeg1-implementation.md), [docs/decoder-admission.md](docs/decoder-admission.md). A decoder license or patent review is not a clearance: see the admission records. In the browser, the sound of a video does not go through the engine's audio graph (API.md).

### WebXR

Not complete. The renderer can draw a world once per view of an XR pose, and a test compares that with the pinned `X3DWebXRContext` on a mock session (`crates/x3d-render/src/xr.rs`). There is no XR session API in the `<x3d-canvas>` element and no native OpenXR runtime. The ledger entry `sub:browser.webxr` is MISSING. The choice of the runtime is open ([#83](https://github.com/Ascendance3D/X_ITE_R/issues/83), [docs/webxr-runtime-options.md](docs/webxr-runtime-options.md)).

## Targets

| target | state |
| --- | --- |
| Browser: `wasm32-unknown-unknown` with WebGPU | A browser with WebGPU (the docs name Chrome or Edge 113 or later). CI tests it in headless Chromium on software WebGPU (SwiftShader). Not tested on a real phone or tablet. |
| Linux x86_64 | Main development platform. CI builds and tests it without a GPU on GitHub-hosted runners. GPU CI runs on the project's GPU runner on pushes to `master`, nightly and by hand; it is not a pull-request check. |
| Windows x86_64 | Cross-built from Linux. An earlier run on one PC used DX12. Not in CI, no package. |
| macOS (arm64) | CI builds the workspace and runs the non-GPU tests on the hosted `macos-15` runner (a virtual machine, not a Metal result). A physical M1 run exists with failures (see above). Not verified on macOS 15 hardware ([#87](https://github.com/Ascendance3D/X_ITE_R/issues/87), [#88](https://github.com/Ascendance3D/X_ITE_R/issues/88)). |

## Requirements

- Rust 1.95.0, the version that CI uses. The workspace uses the 2024 edition.
- On Ubuntu 24.04: `pkg-config`, `libasound2-dev`, `libwayland-dev`, `libxkbcommon-dev`, `libudev-dev`, `clang` and `lld`, and a Vulkan driver for rendering (for example `mesa-vulkan-drivers`).
- For the browser build: the `wasm32-unknown-unknown` target and `wasm-bindgen-cli` 0.2.129 (the version in `Cargo.lock`).
- For the browser tests: Node.js 24 and Playwright 1.62.1 with Chromium.
- Python 3, for the reference checks in `tools/` and `oracle/tools/`.

```sh
sudo apt-get install -y pkg-config libasound2-dev libwayland-dev libxkbcommon-dev libudev-dev clang lld mesa-vulkan-drivers
```

## Build

Native viewer:

```sh
cargo build --release -p x3d-viewer --bin x3d-view
```

Browser module:

```sh
rustup target add wasm32-unknown-unknown
cargo install wasm-bindgen-cli --version 0.2.129 --locked
tools/build-web.sh
```

`tools/build-web.sh` writes the module to `crates/x3d-web/web/pkg/` (gitignored).

## Usage

Open a world in the native viewer:

```sh
./target/release/x3d-view crates/x3d-viewer/samples/desktop-demo/demo.wrl
```

`tools/x3d-view-launch.sh [world]` does the same and opens the demo world when you give no world.

The viewer shortcuts use Ctrl on Linux and Windows, and Command on macOS. All other keys go to the world (KeySensor, StringSensor).

| key | action |
| --- | --- |
| Ctrl + `1` / `2` / `3` / `4` | EXAMINE / WALK / FLY / NONE navigation |
| `Page Up` / `Page Down` | previous / next viewpoint |
| Ctrl + `l` | list the viewpoints |
| Ctrl + `m` | cycle multisampling |
| Ctrl + `r` | reload |
| Ctrl + `s` | save a screenshot |
| Ctrl + `q` | exit |

`Esc` goes to the world and does not exit the viewer.

Show a world in a web page (after the browser build):

```sh
python3 -m http.server -d crates/x3d-web/web 8000
```

Open <http://localhost:8000/example/> in Chrome or Edge. The page uses the element as an X_ITE page does:

```html
<script type="module" src="../x_ite_r.mjs"></script>
<x3d-canvas src="box.wrl"></x3d-canvas>
```

[crates/x3d-web/README.md](crates/x3d-web/README.md) has more examples.

## Test

CI runs the workspace tests with cargo-nextest, then the doctests, in its evidence mode. Some reference data (the answer keys in `oracle/answer-keys.tsv`) is not in the repository, and a runner without a GPU cannot run the GPU tests. In CI mode a test may skip only for a reason in `X3D_EVIDENCE_ALLOW`, and each skip is logged. **A skip is not a pass.** A test that renders on a GPU returns at once on a runner without an adapter, and nextest shows it as passed; the evidence-skip log and the GPU workflow tell the difference. The workflow `.github/workflows/rust-tests.yml` is the source of truth for the commands, the environment, the feature flags and the list of GPU tests (`GPU_TESTS`). The commands below are a local version of them without the `GPU_TESTS` filter, which a runner without a GPU adds (next paragraph); they need `cargo-nextest` (`cargo install cargo-nextest`, or the prebuilt binary). An allowed GPU skip gives a successful CI result, but a skipped test proves nothing about GPU compatibility: only a run on a GPU adapter does:

```sh
export X3D_EVIDENCE=ci
export X3D_EVIDENCE_ALLOW=no-gpu-adapter,backend-absent,x_ite-text.json,items-baseline.json,x0-baseline.json,x0-xml-baseline.json
export X3D_EVIDENCE_LOG=$PWD/evidence-skips.tsv
cargo nextest run --workspace --release --features x3d-runtime/av1,x3d-render/av1,x3d-viewer/av1 --profile ci --no-fail-fast
cargo test --workspace --release --doc --no-fail-fast
python3 -I -B tools/evidence_summary.py evidence-skips.tsv
```

Without a GPU, also exclude the `GPU_TESTS` of `rust-tests.yml` with `-E 'not (...)'`. With a GPU, `X3D_ORACLE_KEYS=<evidence store> tools/strict-suite.sh` runs everything strict (a missing key or adapter fails). `docs/agents/ci.md` describes the runners, the browser shards and the GPU workflow.

Reference and format checks (CI's "Fast checks" job; it also runs actionlint, `tools/ci_changes.py`, `tools/check_gpu_test_lists.py`, `tools/check_no_aac.py`, `tools/check_vendored_rav1d.py` and others, listed in `rust-tests.yml`):

```sh
cargo fmt --all -- --check
tools/verify-candidate.sh
python3 -I -B tools/verify-reference-pin.py
python3 -I -B tools/test_answer_keys.py
python3 -I -B ledger/tools/verify_ledger.py
```

The last command is not run by CI. Run it when you change `ledger/`.

Browser tests:

```sh
tools/build-web.sh
cd crates/x3d-web && npm ci && npx playwright install chromium && npm test
```

Without a GPU, run them on Mesa's software Vulkan driver: `VK_ICD_FILENAMES=/usr/share/vulkan/icd.d/lvp_icd.json npm test`.

## Mandatory Reverse Engineer Anything (REA) Evidence

The port measures compatibility against the pinned X_ITE source and its recorded runtime behavior. REA (Reverse Engineer Anything) is the analysis tool for that work. These rules apply to every compatibility change:

- **Source identity.** X_ITE 16.5.0, commit `34cdf9f97145b3cd2b8e250f8c88dfdd083c0517`, vendored unchanged in `oracle/x_ite-pin/`. `tools/candidate.lock` pins its digest, and `tools/verify-candidate.sh` checks it. `oracle/x_ite-pin-16.6.0/` holds 16.6.0 for a later upgrade audit. No test reads it.
- **Tool version.** New compatibility work uses `rea-agents@6.5.0`. The existing evidence does not record the REA version that made it.
- **Operations.** Find the real REA operations before you use them, and record the operations you ran. Read the pinned JavaScript source directly, and compare the Rust code separately.
- **Runtime evidence.** Observable behavior is compared with recordings of the pinned X_ITE in `oracle/` (baselines, recorders in `oracle/*.mjs` and `oracle/tools/`). Run logs are in `evidence/`.
- **Scope and limitations.** A REA no-match does not prove that a feature is absent. Static analysis does not prove Rust runtime behavior: only a runtime test against a recording does.
- **Ledger.** The coverage ledger has one entry per upstream feature. The in-repository copy is `ledger/ledger.json`. On 2026-11-03 (master `2146bbd7`) it held 346 entries: 280 COMPLETE, 44 PARTIAL, 21 DIFFERENT, 1 MISSING and 0 NOT_TESTED. The counts change with each ledger step; run the checker for the current ones: `python3 -I -B ledger/tools/verify_ledger.py` (it reports the promotion history and the evidence references; on 2026-11-03: consistent, 0 missing). Each entry names its `tests` and its `oracle` recordings, and a PARTIAL entry says in `missing` what is open; read those fields instead of the status word alone. A merged pull request does not set a status. A test result does not change a ledger status by itself: a ledger step is a separate pull request made with a snapshot and a migration tool in `ledger/tools/`.
- **Remaining differences.** The DIFFERENT entries are recorded differences. The PARTIAL and MISSING entries are open work, tracked in the [issues](https://github.com/Ascendance3D/X_ITE_R/issues).

A pull request that changes compatibility behavior records the source identity, tool version, operations, evidence IDs, scope, limitations, runtime tests and remaining differences in its description.

## Repository layout

| path | contents |
| --- | --- |
| `crates/x3d-fields`, `x3d-parse`, `x3d-scene` | field types, the VRML97/X3D parser and the scene model |
| `crates/x3d-runtime`, `x3d-script` | event engine, PROTO and loading; Script nodes on QuickJS |
| `crates/x3d-render`, `x3d-text` | wgpu renderer; fonts and glyph tessellation |
| `crates/x3d-viewer` | native viewer `x3d-view` and test drivers |
| `crates/x3d-web` | browser build: WebAssembly, WebGPU and `<x3d-canvas>` |
| `crates/vrml97`, `x3d`, `vrml` | public facade crates and the `vrml` command-line tool |
| `crates/x3d-oracle`, `x3d-evidence`, `r0-2-c-tests` | reference drivers, evidence policy and compatibility tests |
| `oracle/` | pinned X_ITE sources, recordings, baselines and recording scripts |
| `evidence/` | REA and test run logs |
| `ledger/` | the coverage ledger (`ledger.json`), its snapshots and migration tools, and the REA batch records in `ledger/history/` |
| `tools/` | build, packaging and reference-check scripts |
| `docs/` | design notes, findings, agent guides and reports |

## Upstream and attribution

X_ITE is developed by Holger Seelig and contributors at [create3000/x_ite](https://github.com/create3000/x_ite). The pinned 16.5.0 source in `oracle/x_ite-pin/` is unchanged, and its `package.json` declares the MIT License. The bundled X_ITE fonts keep their own licenses in their directories.

eXciteR is an independent project. It is not affiliated with or endorsed by the X_ITE authors.

## License

MIT. See [LICENSE](LICENSE). The published crates `vrml`, `vrml97` and `x3d` include their own MIT license files.

## Contributing

Bug reports and pull requests are welcome. For a large change, please open an [issue](https://github.com/Ascendance3D/X_ITE_R/issues) first.

- Run `cargo fmt --all` and the tests above before you open a pull request.
- Link one issue in the pull request. Use `Refs #N` for partial work, and a closing keyword only when the issue is complete.
- Do not change a pin, a baseline, a recording or an answer key to make a test pass.
- Follow the REA rules above for compatibility changes.
- A maintainer approves CI runs for pull requests from outside contributors. The required checks are `Rust tests result` and `Web tests result`. The CPU and web jobs run on GitHub-hosted runners; GPU CI runs on a GPU runner on pushes to `master`, nightly and by hand, never for a pull request.

CI: [GitHub Actions](https://github.com/Ascendance3D/X_ITE_R/actions).
