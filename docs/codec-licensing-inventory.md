# Codec licensing inventory (release gate)

Owner ruling 2026-11-02 (issue #90): before any public release of binaries that contain a video or audio codec, this inventory is complete and the owner gives a separate release authorization after legal review. Nothing here says that any patent right is cleared, and nothing here is legal advice. No license is bought and no agreement is signed without the owner's explicit permission.

**Release gate status: NOT COMPLETE (blocked).** Every row marked "Not assessed" is an unresolved release blocker, and no row has had the qualified review the gate requires. Adding this file does not satisfy the gate.

State of this file: **first draft 2026-11-02 (`Cargo.lock` at master `7e50fa2b`); technical facts corrected 2026-11-07 from `Cargo.lock`, `vendor/` and the crate manifests at master `e0e053ec`.** The 2026-11-07 correction changes no patent position, no disposition and no owner ruling. Columns the owner listed: codec and implementation, exact crate and source, copyright license, applicable patent grants, patent-license conditions, third-party claims, distribution and notice requirements, recommended disposition. "Not assessed" means no patent research was done for that row; it is not a statement that none exists.

Releases (corrected 2026-11-09): when this file was first written, no release existed. Since 2026-11-05 the GitHub prerelease `browser-preview-0.1.0` ("X_ITE_R Browser Preview 0.1.0 (alpha)", commit `11e148db`) publishes a binary browser package, and https://xiter-alpha.ascendance3d.org serves the same package. That publication did not pass this gate and is not evidence of clearance; see "Evidence inventory for Canary 0.2.0" below. There is no stable release.

## Video

| Codec | Implementation (crate, source, checksum) | Copyright license | Patent grants that apply | Grant conditions | Third-party claims, unresolved | Notice and distribution | Recommended disposition |
| --- | --- | --- | --- | --- | --- | --- | --- |
| VP9 | `vp9dec` 0.1.1, crates.io, `https://github.com/ChanyaVRC/vp9dec`, sha256 `85dc9eb9e26fa02ce81898998c5e1ccc718f14bfc485c7a4518f464bba57d338`. In the tree before this inventory; used by `MovieTexture` WebM. | MIT | **Not assessed for `vp9dec` itself.** `vp9dec` is an independent decoder (upstream `ChanyaVRC/vp9dec`), not one of Google's distributed implementations, so the Google WebM Additional IP Rights Grant (implementation-specific) is not treated as covering it. The WebM Bitstream License is written for implementations of the WebM Specifications and covers Google's claims only; whether it reaches this decoder is an inference, not a finding. Neither document is about the Matroska/WebM container code in this repository. See `docs/decoder-admission.md`. | Ends for a licensee that sues over an implementation. | Sisvel operates a VP9 patent pool (secondary reports only, not verified). | Keep MIT notice. | Keep and maintain (owner ruling). Not releasable on a claim of clearance; legal review first. |
| VP8 | In-tree decoder `crates/x3d-runtime/src/vp8/` (PR #353, merged), adapted from `zvidlib-vp8` 0.5.0, upstream crate sha256 (not of the adapted copy) `ed40a5ba17bf2b7db9b9ae08a9c5669753332be34f3b346439a1c1188aa3834a`, `https://github.com/lsegal/zvidlib` commit `bcb92a6ab080d58fc34bafbf05455b2a8cfc1010` (`docs/decoder-admission.md`). Compiled only with feature `native-video` of `x3d-runtime`. | MIT; tables transcribed from RFC 6386 under BSD-3-Clause (both notices in this repository's `THIRD_PARTY_NOTICES.md`) | As for VP9: the two WebM documents. The VP8 Patent Cross-License Agreement is a third, voluntary instrument (grant-back; see `docs/decoder-admission.md`); not signed. | Cross-license: licensee grants back its VP8-essential patents; ends on suit. Terms read from a summary page and secondary sources, not the agreement text. | MPEG LA 2011 pool effort, ended by the March 2013 Google settlement (press releases only). Continuity after the 2023 Via LA change not established. | Keep MIT and BSD-3-Clause notices with the vendored code. | Development and internal testing. Legal review to decide whether to join the Cross-License before release. |
| AV1 (video) | `rav1d` 1.1.0, a patched copy in `vendor/rav1d` (one statement changed, see `vendor/rav1d/VENDOR.md`), upstream `https://github.com/memorysafety/rav1d`, upstream `.crate` sha256 `1932f060d5e7bd49dc9f8b272c1dc5e9ce0ffe141c28be900265d3989b36c9ed`. Compiled only with feature `x3d-runtime/av1`, forwarded by `x3d-render/av1` and `x3d-viewer/av1`; `x3d-web` has no `av1` feature. Off by default in all three; CI turns it on for tests (`cargo tree -i rav1d` with default features: absent from `x3d-viewer` and `x3d-web`). `tools/check_vendored_rav1d.py`: `vendor/rav1d is rav1d-1.1.0 plus the recorded patch`. | BSD-2-Clause (`COPYING`: VideoLAN / dav1d authors, Internet Security Research Group; no patent notice) | AOMedia Patent License 1.0: reciprocal, royalty-free, irrevocable except on litigation, runs to any Licensee, not limited to AOMedia members as licensees. | Licensee must make its Necessary Claims available under the same license; defensive termination (1.3). | Sisvel AV1 patent pool (since 2019); non-member holders reported to have sued platforms (secondary reports). The AOMedia license does not bind holders who are neither members nor implementers. | Keep BSD-2-Clause text and copyright lines. | Development and internal testing. Legal review before release. |
| AV1 (still image, AVIF) | `avif-rust` 0.1.2 (+ `avif-image-buffer` 0.1.0, `bin-rs` 0.0.10), crates.io, `https://github.com/mith-mmk/avif-rust`, sha256 `fe0b5ee8b4698179b3bbb8b0fd86930846c9eb932451035f02ec2e723b1a4086`. Enabled by the default features of `x3d-render` and `x3d-web` (feature `avif`), native and wasm32. `x3d-viewer` takes `x3d-render` without default features, so its default build has no `avif-rust` (`cargo tree -i avif-rust`, 2026-11-07). A default `x3d-web` build therefore includes AV1 intra decoding unless the feature is turned off. | MIT (Cargo metadata) | Same AOMedia grant applies to AV1 decoding in any form. | As above. | As above. | Keep MIT notice. | **Owner to be aware: AV1 intra decoding is in every default `x3d-web` build (browser) through AVIF textures.** Disposition together with AV1 video. |
| AV1 (encoders for tests) | `rav1e` 0.8.1 (BSD-2-Clause), `ravif` 0.13.0 (BSD-3-Clause): dev-dependencies of `x3d-render` only; not in a release build. | | | | | | Not shipped; keep out of release features. |
| WebP lossy (VP8 key frames) | `image-webp` 0.2.4, sha256 `525e9ff3e1a4be2fbea1fdf0e98686a6d98b4d8f937e1bf7402245af1909e8c3`; decodes images | MIT OR Apache-2.0 | WebP is VP8 intra coding; the VP8 documents above apply. Not assessed separately. | | | Keep notices. | Review with VP8. |
| MPEG-1 Video and Systems | In-tree decoder `crates/x3d-runtime/src/mpeg1/` (PR #367, merged), a Rust port of pl_mpeg at commit `c871f2be022ece7ef4f64230b4fb8e1fb9eb6023`. Since 2026-11-08 behind the feature `native-video` of `x3d-runtime`, like VP8 and VP9: in the default native `x3d-viewer` build, absent from the browser (`x3d-web` wasm32) build, where the page's `<video>` element loads a MovieTexture and the decoder was never called (`docs/mpeg1-implementation.md`). Chromium (the oracle) cannot play MPEG-1; the pinned behavior is a compatibility option. | MIT (notice in `THIRD_PARTY_NOTICES.md`, section pl_mpeg) | No grant identified. Preliminary patent screening only: `docs/mpeg1-investigation.md` (PR #356), `docs/mpeg1-implementation.md` section Licensing. Not a freedom-to-operate opinion. | | Not assessed by a qualified reviewer. | Keep the pl_mpeg MIT notice. | **Not assessed.** Release blocker until reviewed. |

## Audio

Which version a build uses (`cargo tree -i`, checked 2026-11-02): 0.5.5 comes in through `creek-decode-symphonia` 0.3.3, `creek` 1.2.3, `web-audio-api` 1.7.0; 0.6.1 comes in through `web-audio-api` 1.7.0 directly. Both are linked into every build that has `web-audio-api`, so each codec row is compiled twice, once per version.

Decoders reach `AudioClip` and `decodeAudioData` through `web-audio-api` 1.7.0 (MIT; the patched copy `vendor/web-audio-api`, PR #357 merged, only `Cargo.toml` differs from upstream `.crate` sha256 `013903031fb735e7fd9aee7c159f3440a19c6969dc81365aec8c64caddcf39bc`) and `symphonia`. `web-audio-api` is in the native and the wasm32 graph of both `x3d-viewer` and `x3d-web`. No patent research has been done for these rows beyond what is stated.

| Codec | Implementation | Copyright license | Patent position | Disposition |
| --- | --- | --- | --- | --- |
| AAC (AAC-LC) | **Excluded** by owner decision (PR #357): `symphonia-codec-aac` 0.5.5 and 0.6.1 are absent from `Cargo.lock`. Background: `docs/aac-dependency.md`. | MPL-2.0 (when it was present) | Via LA patent pool; terms for source or free binaries unverified. | Excluded from every build. Chromium parity recorded as unsupported. |
| MP3, MP2, MP1 | package `symphonia-bundle-mp3` (a real package in `Cargo.lock`, not only a feature name), 0.5.5 and 0.6.1 both resolved. Features compiled (`cargo tree -e features -i symphonia-bundle-mp3@<v>`, 2026-11-07): 0.5.5 `mp3`; 0.6.1 `mp1`, `mp2`, `mp3`. `x3d-runtime` turns on `mp2` and `mp3`; `vendor/web-audio-api` turns on `mp1` too, so the symphonia 0.6.1 dependency path that `vendor/web-audio-api` enables includes the Layer I decoder of `symphonia-bundle-mp3` in every build, and `AudioClip`/`decodeAudioData` can reach it. The MPEG-1 Systems audio track (PR #381) decodes Layer II and III through symphonia and refuses Layer I on purpose (`crates/x3d-runtime/src/mpeg1/audio.rs`, `Unsupported("MPEG-1 audio layer I")`); the in-tree pl_mpeg port has no audio decoder. | MPL-2.0 | Not assessed (MP3 patents are reported expired in secondary sources; not verified here). Layer I and II: see the MPEG-1 row. | Not assessed. |
| FLAC | package `symphonia-bundle-flac`, 0.5.5 and 0.6.1 both resolved | MPL-2.0 | Not assessed. | Not assessed. |
| Vorbis (in Ogg) | packages `symphonia-codec-vorbis`, `symphonia-format-ogg`, 0.5.5 and 0.6.1 both resolved | MPL-2.0 | Not assessed. | Not assessed. |
| PCM, ADPCM, A-law, mu-law, AIFF/WAV/CAF | `symphonia-codec-pcm`, `symphonia-codec-adpcm`, `symphonia-format-riff`, `-caf` (0.6.1); `hound` 3.5.1 (Apache-2.0, WAV) | MPL-2.0 / Apache-2.0 | Not assessed. | Not assessed. |
| ALAC | `symphonia-codec-alac` 0.5.5 and 0.6.1 | MPL-2.0 | Not assessed; in the build through `vendor/web-audio-api` (the MP4 demuxer `symphonia-format-isomp4` is kept). | Decide with the audio review. |
| Matroska/WebM audio demux | `symphonia-format-mkv` 0.5.5 and 0.6.1 (`x3d-runtime` feature `mkv` of symphonia 0.6.1), with `symphonia-utils-xiph` 0.5.5 | MPL-2.0 | Container code; no codec of its own. Not assessed. | Not assessed. |
| Opus | Not decodable natively (rejected). `Cargo.lock` has no Opus decoder package; `tools/check_no_opus.py` checks this (item 3 below). | | | None. |

Source-license note for the Symphonia crates (MPL-2.0), a reading to be confirmed by a qualified reviewer, not a procedure: MPL-2.0 is a file-level copyleft. We do not modify the Symphonia files, so the obligations that matter for a distribution are keeping each crate's license text and copyright notices with the distribution, and, if a binary form is distributed, making the source of those MPL-covered files available as the license's section 3.2 describes (an unmodified file's source is the crate on crates.io). Changes to MPL-covered files would have to stay under MPL-2.0 with their source available. The exact wording of the release notices is for the legal review.

## Images (non-video) in the default build, for completeness

`png` 0.18.1, `gif` 0.14.2 (MIT OR Apache-2.0), `zune-jpeg` 0.5.15 (MIT OR Apache-2.0 OR Zlib), `image` 0.25.10 (MIT OR Apache-2.0), BMP via `image`. Not assessed for patents.

## Exact in-tree sources (Git tree IDs at master `e0e053ec`)

The upstream checksums above identify the upstream input. The Git tree ID identifies the exact source in this repository, patches and adaptations included (`git rev-parse <commit>:<path>`).

| source | Git tree ID | how to check it against upstream |
| --- | --- | --- |
| `vendor/rav1d` | `5a7cdfc2b307779ac1c5efe5df69f59df14cf982` | `python3 -I tools/check_vendored_rav1d.py` (upstream `.crate` plus the recorded patch) |
| `vendor/web-audio-api` | `d73c3f7000d87ae8433c8c50c47461c92808e6b3` | `vendor/web-audio-api/VENDOR.md`: only `Cargo.toml` differs |
| `crates/x3d-runtime/src/vp8` | `6a9570bc2f3748f32ff1397d5dc9766374fbd188` | adapted, not byte-identical; changes listed in `vp8/mod.rs` |
| `crates/x3d-runtime/src/mpeg1` | `aaece93705407dfedf231fe4c16ad9e82a801e2d` | a port, not byte-identical; changes listed in `video.rs` and `docs/mpeg1-implementation.md` |

Not in this table: crates used unchanged from crates.io (`vp9dec`, `avif-rust`, `image-webp`, `symphonia`, `creek`, `hound`). Their `Cargo.lock` checksum identifies the exact source.

## Audio support crates without a codec of their own

| crate | version | `Cargo.lock` checksum | source | license (Cargo metadata) | notice action |
| --- | --- | --- | --- | --- | --- |
| `creek` | 1.2.3 | `fb3106d374fa70e8a7a8cf3abf6afbb7f3016e923a1e8941a3eee2b3a21583ee` | https://codeberg.org/Meadowlark/creek | MIT OR Apache-2.0 | add to `THIRD_PARTY_NOTICES.md` |
| `creek-core` | 0.2.3 | `2b8d299b2001167b9a51ac46458cbdcfafc42d6a59125a265d8c1de2e16b28a3` | same | MIT OR Apache-2.0 | same |
| `creek-decode-symphonia` | 0.3.3 | `416063cb8f815ee27036075e544a778770b577e82348f017d145a704df90f246` | same | MIT OR Apache-2.0 | same |
| `hound` | 3.5.1 | `62adaabb884c94955b19907d60019f4e145d091c75345379e70d1ee696f7854f` | https://github.com/ruuda/hound | Apache-2.0 | same |

## Which build contains which decoder (2026-11-07, `cargo tree --offline -i`, master `e0e053ec`)

Commands: `cargo tree --offline -q -p <crate> [--target wasm32-unknown-unknown] -i <package>`, default features, run for `x3d-viewer` and `x3d-web` on the host target and on `wasm32-unknown-unknown`. This is a dependency-graph check, not an artifact check.

| decoder | `x3d-viewer` default, native | `x3d-viewer` default, wasm32 | `x3d-web` default, wasm32 | how to leave it out |
| --- | --- | --- | --- | --- |
| VP9 (`vp9dec`), VP8, WebM demux | yes | yes | no (`tools/check_no_native_video.py --cargo`: OK) | feature `native-video` off |
| AV1 video (`rav1d`) | no | no | no | feature `av1` stays off |
| AV1 intra, AVIF (`avif-rust`) | no | no | yes | `x3d-web` feature `avif` off |
| WebP lossy (`image-webp`) | yes | yes | yes | not separable today |
| MPEG-1 Video, Systems | yes | yes | no (since 2026-11-08; `tools/check_no_native_video.py`) | feature `native-video` off |
| Audio: MP2/MP3, FLAC, Vorbis, PCM/ADPCM, ALAC, WAV (`symphonia`, `web-audio-api`, `hound`) | yes | yes | yes | not separable today |
| AAC | no (not in `Cargo.lock`; not detected) | no | no | excluded (`tools/check_no_aac.py`: `OK: no AAC decoder among 524 packages`) |

## Notice gaps found 2026-11-07 (technical, not legal findings)

`THIRD_PARTY_NOTICES.md` holds the notices of pl_mpeg, zvidlib-vp8, RFC 6386, rav1d, avif-rust and image-webp. It holds no section for `vp9dec` (MIT), the `symphonia` crates (MPL-2.0), `web-audio-api` (MIT), `creek` or `hound`. A release package must carry their license texts (item 2 below); the release notice set is not built yet.

Update 2026-11-08 (technical, not a legal finding): the browser package now carries `THIRD_PARTY_CRATES.md` (PR #509, `tools/third_party_notices.py`; `tools/package-web-preview.sh` refuses a stale file). Its crate set is the feature-resolved `cargo tree` of the wasm32 `x3d-web` build: 231 packages at master `d36e0759`. It holds the license text of every `symphonia` crate in that build (with the MPL-2.0 text), `web-audio-api`, `creek`, `creek-core`, `creek-decode-symphonia`, `hound`, `image-webp` and `avif-rust`. `vp9dec` is not in the browser build (`tools/check_no_native_video.py --cargo`), so the browser package does not list it. A native release (`x3d-viewer`) has no notice set yet.

Update 2026-11-08 (technical, not a legal finding): the MPEG-1 decoder moved behind `native-video`. Before, the browser module compiled it, but the browser loader hands every MovieTexture to the page's `<video>` element first (`crates/x3d-runtime/src/loader.rs`, `plays_video`), so the wasm never ran it. Browser behavior does not change: Chromium cannot play MPEG-1, and pinned X_ITE shows nothing for it. `tools/check_no_native_video.py --binary` now also refuses the `mpeg1/` source path in an artifact.

Update 2026-11-08 (technical, not a legal finding): the browser package carries the Source Code Form of its 29 MPL-2.0 crates (`symphonia` 0.5.5 and 0.6.1, unmodified) in `third-party-source/`. These are the published crate archives, each checked against its `Cargo.lock` checksum, with a README and the MPL-2.0 text (`tools/mpl_sources.py`, run by `tools/package-web-preview.sh`; about 1.1 MB). The owner still decides the delivery method (archive or link) after legal review.

## Evidence inventory for Canary 0.2.0 (2026-11-09; technical, not a legal finding)

Artifact under review: `x_ite_r-canary-0.2.0.tar.gz`, SHA-256 `a49423ae119b22099ead8f545729499ac4036e5f657ea76310e41949fb63e9d7`, commit `a6947a6b` (merge of PR #521), `pkg/x3d_web_bg.wasm` SHA-256 `0cc0cf58c067c390750d2c2b2c36e1125abd00342a27d09b88b14f310e8c328d`. The canary is not published. This section lists the evidence a reviewer has and the evidence that is missing. It assesses no patent and gives no advice.

### Decoders in the canary browser module

The second column has two kinds of evidence. "No" rows: the decoder is not in the build graph (`Cargo.lock` or `cargo tree` check), and the binary string scan does not detect it. "Yes" rows: the crate is in the build graph, and the binary string scan detects its crate paths in the WASM. A string scan is a supplemental check, not proof of presence or absence (item 3 below). It does not show whether a decoder is called.

| decoder | in the canary WASM | evidence in this repository | evidence missing for the review |
| --- | --- | --- | --- |
| AAC | no (not in `Cargo.lock`; not detected) | `tools/check_no_aac.py --binary`: OK; `Cargo.lock` form in CI | none for the exclusion |
| Opus | no (not in `Cargo.lock`; not detected) | `tools/check_no_opus.py --binary`: OK | none for the exclusion |
| VP8, VP9, WebM video, MPEG-1 Video and Systems | no (not in the wasm32 `cargo tree`; not detected) | `tools/check_no_native_video.py --cargo --binary`: OK | none for the browser package; still open for a native release |
| AV1 intra (AVIF, `avif-rust`) | yes (default `avif` feature; detected) | AOMedia Patent License 1.0 text read; disposition "with AV1 video" | qualified review of the AOMedia grant for this decoder; the owner's written disposition, or a build with `avif` off |
| WebP lossy (`image-webp`, VP8 intra) | yes (not separable today; detected) | VP8 documents listed in the Video table | qualified review with VP8; whether the Cross-License is needed |
| MP3, MP2, MP1 (`symphonia-bundle-mp3` 0.5.5, 0.6.1) | yes (detected) | crate, version, features, `Cargo.lock` checksums | patent status of MPEG-1/2 Layer I to III verified by a qualified reviewer (only secondary sources here) |
| FLAC, Vorbis/Ogg, PCM/ADPCM/A-law/mu-law, ALAC, WAV/AIFF/CAF, Matroska audio demux | yes (in the build graph; ALAC detected) | crate rows above; `hound` (WAV) | a patent position for each row ("Not assessed" today) |

### Copyright notices and source (MPL-2.0) in the canary package

One commit gives all of this: `a6947a6b` (its `Cargo.lock`, `tools/third_party_notices.py`, `tools/mpl_sources.py` and `tools/package-web-preview.sh`). The package's `build-info.json` records that commit. The build command was `tools/package-web-preview.sh --release canary-0.2.0 <out>`. The script runs `tools/third_party_notices.py --check` (it stops on a stale `THIRD_PARTY_CRATES.md`), then builds the WASM from the same tree, then runs `tools/mpl_sources.py --write` and `--verify` on the package (`tools/package-web-preview.sh:44,96-97`).

- `THIRD_PARTY_CRATES.md`: 231 packages of the feature-resolved wasm32 `x3d-web` build, with every license text, MPL-2.0 included (`tools/third_party_notices.py`).
- `third-party-source/`: the 29 unmodified MPL-2.0 crate archives (symphonia 0.5.5 and 0.6.1), each equal to its `Cargo.lock` checksum, with `README.md` and `MPL-2.0.txt` (`tools/mpl_sources.py`).
- The site build of `Ascendance3D/xiter-www` keeps `third-party-source/` in the deployed site (PR 1 there), so the source is served beside the WASM.
- Open for the review: whether this delivery meets MPL-2.0 section 3.2, and the exact notice wording.

### Facts of the published Browser Preview 0.1.0 (checked 2026-11-09 on the release asset)

`x_ite_r-browser-preview-0.1.0.tar.gz` SHA-256 `d48b092b93270764e0bf566f22e8aea3661d397d22757099bd4b9797ddd1ded4` (the release asset; the same package is deployed at the alpha site):

- AAC and Opus: absent (`check_no_aac.py --binary`, `check_no_opus.py --binary`: OK).
- MPEG-1: **present**. `check_no_native_video.py --binary` reports `x3d-runtime/src/mpeg1/` in `pkg/x3d_web_bg.wasm`. The MPEG-1 row is "Not assessed. Release blocker until reviewed." The decoder was compiled in but the browser loader did not call it (see the 2026-11-08 update above).
- AV1 intra (`avif-rust`), WebP (`image-webp`) and the symphonia audio decoders: present.
- Notices: `THIRD_PARTY_NOTICES.md` has no symphonia section and no MPL-2.0 text; the package has no `THIRD_PARTY_CRATES.md` and no `third-party-source/`.

These are facts for the reviewer and the owner. This file does not decide what they mean or what to do about 0.1.0.

### Open review requirements (from "What a release needs" below)

1. A qualified patent position, and the owner's written disposition, for every row marked "Not assessed": MPEG-1 (native only now), MP1/MP2/MP3, FLAC, Vorbis, PCM/ADPCM, ALAC, Matroska demux, and the separate VP8 Cross-License question.
2. A qualified review of the AOMedia grant for AV1 intra (`avif-rust`) in the default browser build.
3. A legal reading of the MPL-2.0 notice and source delivery above.
4. The owner's separate release authorization.

## What a release needs from this file

1. Every row has a patent position that someone qualified has reviewed, and a disposition the owner has approved in writing.
2. The notices listed are in the release package.
3. `python3 -I tools/check_no_aac.py --binary <artifact>` passes on each native and wasm artifact (PR #357), and an equivalent check exists for any other excluded codec. On 2026-11-07 only the `Cargo.lock` form ran (OK); no release artifact existed to check. On 2026-11-08, on the technical candidate `d36e0759`: the browser package's `pkg/x3d_web_bg.wasm` passes `check_no_aac.py --binary`, `check_no_native_video.py --cargo --binary` and `check_no_opus.py --binary`, and a release build of `x3d-drive` passes the AAC and Opus binary checks. `tools/check_no_opus.py` is the Opus check (lockfile, `--cargo`, `--binary`, `--self-test`); CI and the package script run it. The binary forms are supplemental smoke checks, not proof of absence.
4. The owner's separate release authorization.
