# eXciteR

### X_ITE_R — A Native Rust Engine for VRML97 and X3D

**Bringing X_ITE into native Rust. Preserving interactive 3D worlds. Building for the future.**

[![Rust Tests](https://github.com/DJAscendance/X_ITE_R/actions/workflows/rust-tests.yml/badge.svg?branch=master)](https://github.com/DJAscendance/X_ITE_R/actions/workflows/rust-tests.yml)
[![Rust](https://img.shields.io/badge/Rust-Native_Engine-CE422B?logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![License: MIT](https://img.shields.io/badge/License-MIT-blue.svg)](https://opensource.org/licenses/MIT)
[![Linux](https://img.shields.io/badge/Linux-x86__64-tested-2ea44f?logo=linux&logoColor=white)](https://github.com/DJAscendance/X_ITE_R)
[![macOS](https://img.shields.io/badge/macOS_15%2B-Planned-lightgrey?logo=apple)](https://github.com/DJAscendance/X_ITE_R)
[![VRML97](https://img.shields.io/badge/VRML97-Supported-007ACC)](https://www.web3d.org/)
[![X3D](https://img.shields.io/badge/X3D-In_Development-7952B3)](https://www.web3d.org/)

[![Audio Oracle](https://img.shields.io/badge/Audio_Oracle-X__ITE_Comparison-326CE5)](https://github.com/DJAscendance/X_ITE_R/actions/workflows/rust-tests.yml)
[![Code Verification](https://img.shields.io/badge/Verification-Reference_Based-6f42c1)](https://github.com/DJAscendance/X_ITE_R/tree/master/oracle)
[![Nightly Builds](https://img.shields.io/badge/Nightly_Builds-Planned-lightgrey)](https://github.com/DJAscendance/X_ITE_R/actions)
[![Releases](https://img.shields.io/badge/Releases-Not_Yet_Public-lightgrey)](https://github.com/DJAscendance/X_ITE_R/releases)

---

## Overview

**eXciteR**, developed in the `X_ITE_R` repository, is a native Rust port of the [X_ITE X3D Browser](https://create3000.github.io/x_ite/).

The project brings X_ITE's scene processing, rendering, interaction, and runtime behavior into a native Rust engine.

X_ITE is an established JavaScript implementation of the X3D standard. It supports interactive 3D content through a browser-based runtime.

eXciteR follows a different execution model. It uses native Rust code, a native application window, and GPU rendering without requiring the original JavaScript X_ITE engine at runtime.

The project has two goals.

**The first goal is compatibility.** Existing VRML97 and X3D content should produce behavior that matches the reference X_ITE implementation.

**The second goal is a reusable native engine.** The Rust code should support standalone viewers and future applications that need interactive 3D scene processing.

The initial desktop viewer runs on Linux. Full engine compatibility and macOS delivery remain development goals.

> **Development status:** Active development. A working Linux desktop viewer exists, but the Rust port does not yet implement every X_ITE feature.

## Project Origins

eXciteR began as a direct port of X_ITE 16.5.0 from JavaScript to Rust.

The project uses a fixed upstream version to make behavior comparisons repeatable.

| Property | Source |
|---|---|
| Original engine | [X_ITE X3D Browser](https://create3000.github.io/x_ite/) |
| Upstream source | [create3000/x_ite](https://github.com/create3000/x_ite) |
| Reference version | X_ITE 16.5.0 |
| Reference commit | `34cdf9f97` |
| Port language | Rust |
| Repository | [DJAscendance/X_ITE_R](https://github.com/DJAscendance/X_ITE_R) |
| Native viewer executable | `x3d-view` |
| Project license declaration | MIT |

X_ITE provides the reference for the port.

eXciteR does not run the original JavaScript renderer inside a native wrapper. It implements scene behavior in Rust and compares the results with the original engine.

The fixed upstream reference does not prevent future upgrades. It gives developers a stable point for measuring compatibility before they adopt changes from newer X_ITE versions.

## What Works Today

The current Linux viewer has passed independent desktop acceptance testing.

| Capability | Current state |
|---|---|
| Native Linux application | Tested |
| VRML97 world loading | Tested |
| X3D world loading | Tested |
| 3D geometry and materials | Tested |
| Textures and lighting | Tested |
| Camera navigation | Tested |
| Viewpoint switching | Tested |
| TimeSensors and animation | Tested |
| Script execution and ROUTEs | Tested |
| TouchSensor interaction | Tested |
| Window resizing | Tested |
| Sound and AudioClip | Tested |
| Audio URL fallback | Tested |
| PROTO-wrapped audio sources | Tested |
| Clean application shutdown | Tested |

These results establish a working native viewer.

They do not establish complete X_ITE equivalence for every node, component, extension, or platform.

### Native rendering

The viewer uses the Rust `wgpu` graphics API.

The engine contains work for geometry processing, scene traversal, materials, lighting, textures, shaders, picking, and related rendering behavior.

The development program has also covered advanced rendering components. These include volume rendering, particle systems, and other X3D capabilities.

Some of these components still require additional compatibility work.

### Scene behavior

The runtime contains implementations for interactive scene behavior.

Development areas include event routing, sensors, scripting, field updates, scene state, navigation, and time-based animation.

The project tests these behaviors against the fixed X_ITE reference.

### Native audio

The audio system supports the tested VRML97 Sound and AudioClip behavior.

Completed repairs include audio URL fallback, PROTO-wrapped sound sources, and source playback when audio graph connections change.

The test process also covers sound placement, distance attenuation, and stereo output.

Some differences remain. These include the recorded HRTF limitation and timing-sensitive cases.

## Desktop Viewer

The current standalone application is named `x3d-view`.

It provides a native window for loading and interacting with VRML97 and X3D scenes.

### Supported files

The tested viewer accepts these file formats:

| Extension | Format |
|---|---|
| `.wrl` | VRML97 |
| `.wrz` | Compressed VRML |
| `.x3d` | X3D XML |
| `.x3dv` | X3D Classic VRML encoding |

Support for an extension does not mean that every component in every file is complete.

### Requirements

The tested Linux baseline is:

- Ubuntu 24.04.
- Linux x86-64.
- A Vulkan-capable graphics driver.
- Required Linux window and audio libraries.

The tested standalone package does not require the Rust compiler.

The current package needs glibc 2.39. Its runtime libraries include Vulkan, ALSA, and the applicable X11 or Wayland client libraries.

The development tests used an NVIDIA RTX 3060 Ti.

Other GPU configurations and desktop environments require further testing.

### Running a packaged build

Extract the Linux archive.

```bash
tar xzf x3d-view-linux-x86_64.tar.gz
```

Start the included sample world.

```bash
./x3d-view-linux-x86_64/x3d-view-launch.sh
```

Open a VRML97 world.

```bash
./x3d-view-linux-x86_64/x3d-view-launch.sh /path/to/world.wrl
```

Open an X3D world.

```bash
./x3d-view-linux-x86_64/x3d-view-launch.sh /path/to/world.x3d
```

A packaged build must be obtained from an approved build source. The project has not published a stable public release.

### Viewer controls

The viewer shortcuts use Ctrl on Linux and Command on macOS. All other keys go to the world
(KeySensor, StringSensor).

| Key | Action |
|---|---|
| Ctrl/Command + `1` | EXAMINE navigation |
| Ctrl/Command + `2` | WALK navigation |
| Ctrl/Command + `3` | FLY navigation |
| Ctrl/Command + `4` | NONE navigation |
| `Page Up` / `Page Down` | Change viewpoint |
| `Home` / `End` | Viewpoint navigation |
| Ctrl/Command + `l` | List viewpoints |
| Ctrl/Command + `m` | Cycle multisampling |
| Ctrl/Command + `z` | Cycle depth buffer |
| Ctrl/Command + `r` | Reload |
| Ctrl/Command + `s` | Save screenshot |
| Ctrl/Command + `q` | Exit viewer |

`Esc` does not exit the viewer. It goes to the world. Close the window to exit.

Mouse controls depend on the selected navigation mode.

## Build from Source

The project uses a Rust workspace.

The current repository includes the native engine crates, viewer code, automated tests, and reference comparison tools.

### Prerequisites

Install a compatible Rust toolchain and the Linux development libraries.

The current GitHub CI uses Rust 1.95.0 on Ubuntu 24.04.

For Ubuntu 24.04, install the libraries used by CI:

```bash
sudo apt-get update

sudo apt-get install -y \
  pkg-config \
  libasound2-dev \
  libwayland-dev \
  libxkbcommon-dev \
  libudev-dev \
  clang
```

Install and configure a Vulkan driver for GPU rendering.

### Clone

```bash
git clone https://github.com/DJAscendance/X_ITE_R.git
cd X_ITE_R
```

Repository access is currently restricted.

### Build the viewer

```bash
cargo build --release -p x3d-viewer --bin x3d-view
```

Run the viewer with a world file.

```bash
./target/release/x3d-view /path/to/world.wrl
```

### Use the repository launcher

The repository also contains a launcher script.

```bash
./tools/x3d-view-launch.sh /path/to/world.wrl
```

This launcher looks for the built executable and passes supported arguments to it.

## Architecture

eXciteR separates scene data, runtime behavior, and rendering.

This structure helps the project compare individual systems with X_ITE. It also supports future use of the native engine by other applications.

### Main Rust components

| Component | Purpose |
|---|---|
| `x3d-fields` | X3D field types and values |
| `x3d-parse` | VRML97 and X3D parsing |
| `x3d-scene` | Scene and node representation |
| `x3d-runtime` | Runtime state, events, and behavior |
| `x3d-script` | Script execution support |
| `x3d-render` | Rendering support |
| `x3d-viewer` | Native viewer, input, navigation, and executable tools |

The workspace contains additional components and test tools.

The Rust architecture does not depend on running X_ITE JavaScript as its production renderer.

## Compatibility and Code Verification

**Reference behavior is the measure of compatibility.**

A Rust unit test can show that a function works as written. It cannot, by itself, prove that the result matches X_ITE.

eXciteR uses direct reference comparisons to measure that difference.

### Verification process

The project uses the following process:

1. Identify a behavior in the fixed X_ITE reference.
2. Record the reference output or event state.
3. Implement or repair the behavior in Rust.
4. Run the same input through the Rust implementation.
5. Compare results against the reference.
6. Keep known differences and blocked cases visible.
7. Record the evidence before changing compatibility status.

The reference tools and test worlds are stored under `oracle/`.

This process covers more than rendered images. It also covers events, source loading, script behavior, audio samples, sensor results, and other runtime outputs.

### REA coverage tracking

The project also maintains a feature coverage ledger.

REA refers to **Reverse Engineer Anything**, a development plugin used where its tools apply.

The ledger is a separate project record. It tracks how far the Rust implementation matches the required behavior.

The last confirmed ledger baseline contains 346 entries.

| State | Entries |
|---|---:|
| COMPLETE | 250 |
| PARTIAL | 69 |
| DIFFERENT | 25 |
| MISSING | 2 |
| NOT_TESTED | 0 |
| **Total** | **346** |

These counts describe the recorded development baseline. They are not a percentage of full standards compliance.

`COMPLETE` means that the entry has the required evidence for its approved scope.

`PARTIAL` means that some required behavior or evidence remains incomplete.

`DIFFERENT` records an intentional or approved difference.

`MISSING` means that the required implementation is absent.

An individual test result does not change the ledger automatically.

The ledger remains in development. Future changes must follow the project's evidence rules.

## Continuous Integration

eXciteR uses GitHub Actions for automated checks.

[View Rust test workflow](https://github.com/DJAscendance/X_ITE_R/actions/workflows/rust-tests.yml)

The current workflow runs on a GitHub-hosted Ubuntu 24.04 runner.

### Current CI checks

| Check | Description |
|---|---|
| Rust regression suite | Runs the X_ITE-only Rust workspace tests |
| Audio reference comparison | Compares Rust audio behavior with recorded X_ITE baselines |
| Test summary | Records passed, failed, ignored, and filtered test counts |
| Build validation | Builds the native Rust test tools used by the workflow |

The CI workflow excludes the separate CodeMirror crates.

The standard GitHub runner has no dedicated GPU. Nine GPU-dependent test entries remain outside the hosted CI test run.

These tests remain part of the local GPU verification process.

The project does not count skipped GPU tests as passed.

### Running the Rust tests

```bash
cargo test --workspace --release \
  --exclude cm-core \
  --exclude cm-modes \
  --exclude cm-view \
  --no-fail-fast
```

Run this command in an environment with the required resources and graphics support.

For a standard runner without a GPU, use the exclusions documented in `.github/workflows/rust-tests.yml`.

### Audio reference checks

Build the scene test driver.

```bash
cargo build --release -p x3d-viewer --bin x3d-drive
```

Run the audio comparisons.

```bash
python3 -I -B oracle/tools/rea_b05.py check \
  --target target/release \
  --jobs 2
```

The tool reports exact matches, known differences, blocked comparisons, and failures.

A known difference is not an exact match. A blocked comparison is not a passing comparison.

## Development and Platform Status

| Platform | Status |
|---|---|
| Linux x86-64 | Working native viewer; tested on Ubuntu 24.04 |
| macOS 15+ | Approved development target; native package not yet verified |
| Windows | Outside current development scope |
| Mobile | Outside current development scope |

Linux is the current development and verification platform.

The next platform goal is a native macOS 15+ viewer.

Windows and mobile development are not part of the approved roadmap.

## Known Limitations

eXciteR is an active port. Some behavior still differs from the original engine.

Current limitations and open work include:

- Some X3D components remain PARTIAL.
- HRTF spatial audio behavior does not yet match the reference.
- Some audio URL changes produce timing-dependent reference results.
- CFF OpenType fonts use fallback behavior in the tested desktop viewer.
- MovieTexture audio is not yet implemented.
- MIDI playback remains limited.
- WebXR and the browser keyboard UI are recorded as MISSING.
- GPU picking precision still needs additional compatibility work.
- Some diagnostics can contain internal reference paths.
- macOS packaging and testing are not yet complete.

The full compatibility ledger and regression tests provide more detailed records.

Do not use this software as a complete replacement for every X_ITE function until the relevant compatibility checks pass.

## Roadmap

Development will continue in phases.

### Phase 1 — Native Engine Foundation

**Working baseline established.**

Deliverables include the Rust workspace, parsing, scene handling, rendering, native viewer, and the initial compatibility test system.

### Phase 2 — X_ITE Behavior Parity

**In progress.**

Complete the remaining PARTIAL ledger entries.

Improve component behavior, shader and rendering support, sensor accuracy, scripting, audio, fonts, and scene handling.

Preserve the approved differences until the project authorizes a change.

### Phase 3 — Desktop Platform Support

**Linux working. macOS planned.**

Improve Linux packaging and test coverage.

Build and verify the native application on macOS 15+.

Document the required system libraries and graphics support for each platform.

### Phase 4 — Engine Integration

**Planned.**

Provide stable ways for other native applications to use the Rust engine.

Improve the embedded viewer interfaces.

Continue separating reusable scene and runtime logic from application-specific window controls.

Support future Rust-based tools that need VRML97 and X3D processing.

Integration with WRL Forge is a possible future use. It is not part of the current X_ITE_R implementation scope.

### Phase 5 — Automated Nightly Builds

**Planned. Not yet active.**

The long-term build system should create development packages automatically.

Planned functions include:

- Scheduled nightly builds from an approved development branch.
- Validation of changes from completed pull requests.
- Linux desktop packages.
- macOS desktop packages after platform validation.
- Build manifest and commit SHA.
- SHA-256 checksums.
- Automated test summaries.
- A defined artifact retention period.
- Separate development and stable release channels.
- Clear labels for unverified builds.

Nightly builds will be for testing.

A passing nightly build will not be a stable release.

The release system must preserve an owner approval step before publishing a stable release.

No nightly download link is available until the workflow exists and produces verified packages.

## Release Channels

The planned release model separates development work from stable delivery.

| Channel | Purpose | Current state |
|---|---|---|
| Development | Builds and tests for active work | Active |
| Nightly | Automated test packages | Planned |
| Preview | Milestone builds for wider testing | Planned |
| Stable | Owner-approved production releases | Not yet available |

Release packages should identify their source commit.

Published packages should include checksums and platform requirements.

The build process should not publish a release merely because a pull request passes CI.

## Security and Supply Chain

The project will use automated checks to help verify the Rust code and its dependencies.

The current CI workflow runs the Rust regression suite and the audio reference tests.

Future work may add:

- Rust formatting checks with `cargo fmt`.
- Static analysis with `cargo clippy`.
- Dependency vulnerability checks with `cargo audit`.
- Dependency license checks.
- GitHub CodeQL where supported.
- Release artifact checksums.
- Software bill of materials (SBOM) output.
- Release signing and provenance records.

These checks are planned unless a workflow already implements them.

A CI badge shows the result of the linked workflow. It does not prove that every dependency or code path is secure.

Security reports should be handled privately when the repository becomes public and a reporting process is available.

## Project Scope

eXciteR is a native VRML97/X3D engine project.

It is not a CodeMirror port.

It is not the WRL Forge editor.

It is not a browser extension.

It is not a JavaScript wrapper around the original X_ITE renderer.

The project may supply engine technology to other applications in the future. Such integration requires its own development and verification work.

## Upstream Attribution

eXciteR is derived from the open-source X_ITE project.

**Original project:** [X_ITE X3D Browser](https://create3000.github.io/x_ite/)

**Source repository:** [create3000/x_ite](https://github.com/create3000/x_ite)

**Reference version:** 16.5.0

**Reference commit:** `34cdf9f97`

**Original development:** Holger Seelig and X_ITE contributors.

The project recognizes the work that made X_ITE possible.

The Rust port uses that work as its reference and preserves applicable third-party rights and notices.

eXciteR is an independent native port. It is not presented as an official replacement or release from the upstream X_ITE maintainers.

## License

The Rust workspace declares the **MIT License**.

Upstream X_ITE and other third-party components retain their applicable copyright and license notices.

Before public release, the repository must provide a root `LICENSE` file and verify the notices for all reused code and distributed assets.

License declarations do not remove obligations attached to third-party code.

## Contributing

The repository is currently private.

Public contribution instructions will be added before the project opens to outside contributors.

Future contributions should include clear test evidence and preserve the compatibility requirements.

Changes that affect X_ITE behavior should include reference comparisons when practical.

Contributors should not remove known differences or weaken tests simply to produce a passing result.

## Project Links

| Resource | Link |
|---|---|
| Repository | [DJAscendance/X_ITE_R](https://github.com/DJAscendance/X_ITE_R) |
| GitHub Actions | [Build and test status](https://github.com/DJAscendance/X_ITE_R/actions) |
| Issues | [Issue tracker](https://github.com/DJAscendance/X_ITE_R/issues) |
| Pull requests | [Development changes](https://github.com/DJAscendance/X_ITE_R/pulls) |
| Future releases | [Releases](https://github.com/DJAscendance/X_ITE_R/releases) |
| Original X_ITE | [X_ITE website](https://create3000.github.io/x_ite/) |
| Original source | [X_ITE on GitHub](https://github.com/create3000/x_ite) |
| X3D standards | [Web3D Consortium](https://www.web3d.org/) |
| Rust | [rust-lang.org](https://www.rust-lang.org/) |

---

**eXciteR — X_ITE, rebuilt in Rust.**

*Preserving the worlds of yesterday. Building the native 3D engine of tomorrow.*
