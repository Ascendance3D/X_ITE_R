# eXciteR (X_ITE_R)

eXciteR is a Rust port of the [X_ITE](https://create3000.github.io/x_ite/) X3D browser. It loads VRML97 and X3D worlds, runs their events, sensors and scripts, and renders them with [wgpu](https://wgpu.rs/).

The port follows one pinned upstream version, X_ITE 16.5.0. Tests compare the Rust results with recordings of that version.

## Goals

- **Browser first.** The engine compiles to WebAssembly and draws with WebGPU. The `x3d-web` crate provides an `<x3d-canvas>` element with X_ITE's JavaScript API names. No X_ITE JavaScript runs in the page.
- **Native viewer.** `x3d-view` is a desktop application on the same engine. It is the main tool for testing rendering, input and audio against X_ITE outside a browser.
- **A reusable engine.** The crates can be used by other Rust applications that need X3D scene processing.

## Status

The project is under active development. There is no stable release. It does not have full X_ITE compatibility.

### Working

- Parsing of VRML97, X3D classic encoding (`.x3dv`), X3D XML (`.x3d`) and gzip-compressed worlds (`.wrz`).
- Scene runtime: fields, ROUTEs, PROTO and EXTERNPROTO, Inline, TimeSensor animation, interpolators, sensors (touch, plane, key, string and others), and Script nodes (JavaScript through QuickJS).
- Rendering with wgpu: geometry, materials, textures, lighting, shadows, particles, volume rendering, text and picking.
- Navigation: EXAMINE, WALK, FLY and NONE, and viewpoint binding.
- Audio: Sound and AudioClip, URL fallback and spatial attenuation, on the native viewer.
- Browser build: `<x3d-canvas>` loads and draws a world, takes mouse, touch, wheel and keyboard input, and supports a subset of the X_ITE JavaScript API. [crates/x3d-web/web/API.md](crates/x3d-web/web/API.md) lists the supported and unsupported members.

### Missing or incomplete

- Much of the browser JavaScript API: scene creation ([#134](https://github.com/Ascendance3D/X_ITE_R/issues/134)), browser context functions ([#135](https://github.com/Ascendance3D/X_ITE_R/issues/135)), browser options and properties ([#136](https://github.com/Ascendance3D/X_ITE_R/issues/136)), parts of the field API ([#137](https://github.com/Ascendance3D/X_ITE_R/issues/137)) and the X3D entry points ([#138](https://github.com/Ascendance3D/X_ITE_R/issues/138)).
- MovieTexture frame decoding in the browser ([#139](https://github.com/Ascendance3D/X_ITE_R/issues/139)) and sound in the browser ([#140](https://github.com/Ascendance3D/X_ITE_R/issues/140)). MovieTexture audio is not implemented on any target.
- HRTF spatial audio, full MIDI playback and CFF OpenType fonts.
- WebXR ([#83](https://github.com/Ascendance3D/X_ITE_R/issues/83)) and the browser keyboard UI.
- Some GPU picking precision and pointing-sensor cases ([#67](https://github.com/Ascendance3D/X_ITE_R/issues/67), [#79](https://github.com/Ascendance3D/X_ITE_R/issues/79)).
- Release packages for all platforms.

The REA section below gives the measured coverage.

## Targets

| target | state |
| --- | --- |
| Browser: `wasm32-unknown-unknown` with WebGPU | Chrome or Edge 113 or later. CI tests it in headless Chromium. |
| Linux x86_64 | Main development platform. CI builds and tests it on a GitHub-hosted runner without a GPU. |
| Windows x86_64 | Cross-built from Linux. Tested with DX12 on one PC. No package. |
| macOS | Not verified ([#87](https://github.com/Ascendance3D/X_ITE_R/issues/87)). |

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

CI runs the workspace tests in its evidence mode. Some reference data (the answer keys in `oracle/answer-keys.tsv`) is not in the repository, and a runner without a GPU cannot run the GPU tests. In CI mode these tests skip by name, and each skip is logged:

```sh
export X3D_EVIDENCE=ci
export X3D_EVIDENCE_ALLOW=no-gpu-adapter,backend-absent,x_ite-text.json,items-baseline.json
export X3D_EVIDENCE_LOG=$PWD/evidence-skips.tsv
cargo test --workspace --release --exclude cm-core --exclude cm-modes --exclude cm-view --no-fail-fast
python3 -I -B tools/evidence_summary.py evidence-skips.tsv
```

Without a GPU, also add the skip list of the `GPU_TESTS` variable in `.github/workflows/rust-tests.yml` after `--`.

Reference and format checks (CI's "Fast checks" job):

```sh
cargo fmt --all -- --check
tools/verify-candidate.sh
python3 -I -B tools/verify-reference-pin.py
python3 -I -B tools/test_answer_keys.py
```

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
- **Ledger.** The coverage ledger has one entry per upstream feature. The in-repository copy is `_batch09_context/ledger/ledger.json`. On 2026-10-29 it held 346 entries: 249 COMPLETE, 71 PARTIAL, 25 DIFFERENT, 1 MISSING and 0 NOT_TESTED. Check it with `python3 -I -B _batch09_context/tools/verify_ledger.py`. That run reported its promotion history as consistent and 0 missing evidence references. A test result does not change a ledger status by itself.
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
| `crates/cm-core`, `cm-modes`, `cm-view` | an earlier copy of the CodeMirror 5 port. Development continues in [CodeMirror_R](https://github.com/Ascendance3D/CodeMirror_R). No X3D crate depends on it, and CI does not test it. |
| `oracle/` | pinned X_ITE sources, recordings, baselines and recording scripts |
| `evidence/` | REA and test run logs |
| `_batch08_context/`, `_batch09_context/` | REA batch records, including the coverage ledger |
| `tools/` | build, packaging and reference-check scripts |
| `docs/` | design notes, findings, agent guides and reports |

## Upstream and attribution

X_ITE is developed by Holger Seelig and contributors at [create3000/x_ite](https://github.com/create3000/x_ite). The pinned 16.5.0 source in `oracle/x_ite-pin/` is unchanged, and its `package.json` declares the MIT License. The bundled X_ITE fonts keep their own licenses in their directories. The CodeMirror 5 tree in `oracle/cm5-build/` is copyright Marijn Haverbeke and others, under the MIT License (`oracle/cm5-build/LICENSE`).

eXciteR is an independent project. It is not affiliated with or endorsed by the X_ITE authors.

## License

MIT. See [LICENSE](LICENSE). The published crates `vrml`, `vrml97` and `x3d` include their own MIT license files.

## Contributing

Bug reports and pull requests are welcome. For a large change, please open an [issue](https://github.com/Ascendance3D/X_ITE_R/issues) first.

- Run `cargo fmt --all` and the tests above before you open a pull request.
- Link one issue in the pull request. Use `Refs #N` for partial work, and a closing keyword only when the issue is complete.
- Do not change a pin, a baseline, a recording or an answer key to make a test pass.
- Follow the REA rules above for compatibility changes.
- A maintainer approves CI runs for pull requests from outside contributors. The required checks are `Rust tests result` and `Web tests result`.

CI: [GitHub Actions](https://github.com/Ascendance3D/X_ITE_R/actions).
