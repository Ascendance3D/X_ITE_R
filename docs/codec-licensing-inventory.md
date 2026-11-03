# Codec licensing inventory (release gate)

Owner ruling 2026-11-02 (issue #90): before any public release of binaries that contain a video or audio codec, this inventory is complete and the owner gives a separate release authorization after legal review. Nothing here says that any patent right is cleared, and nothing here is legal advice. No license is bought and no agreement is signed without the owner's explicit permission.

**Release gate status: NOT COMPLETE (blocked).** Every row marked "Not assessed" is an unresolved release blocker, and no row has had the qualified review the gate requires. Adding this file does not satisfy the gate.

State of this file: **first draft, 2026-11-02, from `Cargo.lock` at master `7e50fa2b` plus open PRs.** Columns the owner listed: codec and implementation, exact crate and source, copyright license, applicable patent grants, patent-license conditions, third-party claims, distribution and notice requirements, recommended disposition. "Not assessed" means no patent research was done for that row; it is not a statement that none exists.

No release exists yet (`gh release list` is empty; README: "There is no stable release"). The repository publishes source only.

## Video

| Codec | Implementation (crate, source, checksum) | Copyright license | Patent grants that apply | Grant conditions | Third-party claims, unresolved | Notice and distribution | Recommended disposition |
| --- | --- | --- | --- | --- | --- | --- | --- |
| VP9 | `vp9dec` 0.1.1, crates.io, `https://github.com/ChanyaVRC/vp9dec`, sha256 `85dc9eb9e26fa02ce81898998c5e1ccc718f14bfc485c7a4518f464bba57d338`. In the tree before this inventory; used by `MovieTexture` WebM. | MIT | **Not assessed for `vp9dec` itself.** `vp9dec` is an independent decoder (upstream `ChanyaVRC/vp9dec`), not one of Google's distributed implementations, so the Google WebM Additional IP Rights Grant (implementation-specific) is not treated as covering it. The WebM Bitstream License is written for implementations of the WebM Specifications and covers Google's claims only; whether it reaches this decoder is an inference, not a finding. Neither document is about the Matroska/WebM container code in this repository. See `docs/decoder-admission.md`. | Ends for a licensee that sues over an implementation. | Sisvel operates a VP9 patent pool (secondary reports only, not verified). | Keep MIT notice. | Keep and maintain (owner ruling). Not releasable on a claim of clearance; legal review first. |
| VP8 | Vendored decoder adapted from `zvidlib-vp8` 0.5.0 (crates.io), in PR #353 (not merged). Exact checksum and upstream revision to be recorded by that PR. | MIT; tables transcribed from RFC 6386 under BSD-3-Clause (notice shipped upstream in `THIRD_PARTY_NOTICES.md`) | As for VP9: the two WebM documents. The VP8 Patent Cross-License Agreement is a third, voluntary instrument (grant-back; see `docs/decoder-admission.md`); not signed. | Cross-license: licensee grants back its VP8-essential patents; ends on suit. Terms read from a summary page and secondary sources, not the agreement text. | MPEG LA 2011 pool effort, ended by the March 2013 Google settlement (press releases only). Continuity after the 2023 Via LA change not established. | Keep MIT and BSD-3-Clause notices with the vendored code. | Development and internal testing. Legal review to decide whether to join the Cross-License before release. |
| AV1 (video) | `rav1d` (BSD-2-Clause), planned in plan Steps 5 and 6, not yet in the tree. Exact version, source and checksum to be recorded by the PR that adds it. | BSD-2-Clause (`COPYING`: VideoLAN / dav1d authors, Internet Security Research Group; no patent notice) | AOMedia Patent License 1.0: reciprocal, royalty-free, irrevocable except on litigation, runs to any Licensee, not limited to AOMedia members as licensees. | Licensee must make its Necessary Claims available under the same license; defensive termination (1.3). | Sisvel AV1 patent pool (since 2019); non-member holders reported to have sued platforms (secondary reports). The AOMedia license does not bind holders who are neither members nor implementers. | Keep BSD-2-Clause text and copyright lines. | Development and internal testing. Legal review before release. |
| AV1 (still image, AVIF) | `avif-rust` 0.1.2 (+ `avif-image-buffer` 0.1.0, `bin-rs` 0.0.10), crates.io, `https://github.com/mith-mmk/avif-rust`, sha256 `fe0b5ee8b4698179b3bbb8b0fd86930846c9eb932451035f02ec2e723b1a4086`. Enabled by the default features of `x3d-render` and `x3d-web` (feature `avif`); any future binary built with default features therefore includes AV1 decoding unless this dependency is removed or made non-default. | MIT (Cargo metadata) | Same AOMedia grant applies to AV1 decoding in any form. | As above. | As above. | Keep MIT notice. | **Owner to be aware: AV1 decoding is in every default-feature build through AVIF textures.** Disposition together with AV1 video. |
| AV1 (encoders for tests) | `rav1e` 0.8.1 (BSD-2-Clause), `ravif` 0.13.0 (BSD-3-Clause): dev-dependencies of `x3d-render` only; not in a release build. | | | | | | Not shipped; keep out of release features. |
| WebP lossy (VP8 key frames) | `image-webp` 0.2.4, sha256 `525e9ff3e1a4be2fbea1fdf0e98686a6d98b4d8f937e1bf7402245af1909e8c3`; decodes images | MIT OR Apache-2.0 | WebP is VP8 intra coding; the VP8 documents above apply. Not assessed separately. | | | Keep notices. | Review with VP8. |
| MPEG-1 | Not implemented. Chromium (the oracle) cannot play it; DIFFERENT approved. See `docs/mpeg1-investigation.md` (PR #356). | | | | | | None. |

## Audio

Which version a build uses (`cargo tree -i`, checked 2026-11-02): 0.5.5 comes in through `creek-decode-symphonia` 0.3.3, `creek` 1.2.3, `web-audio-api` 1.7.0; 0.6.1 comes in through `web-audio-api` 1.7.0 directly. Both are linked into every build that has `web-audio-api`, so each codec row is compiled twice, once per version.

Decoders reach `AudioClip` and `decodeAudioData` through `web-audio-api` 1.7.0 (MIT; patched copy in PR #357) and `symphonia`. No patent research has been done for these rows beyond what is stated.

| Codec | Implementation | Copyright license | Patent position | Disposition |
| --- | --- | --- | --- | --- |
| AAC (AAC-LC) | **Excluded** by owner decision (PR #357): `symphonia-codec-aac` 0.5.5 and 0.6.1 are absent from `Cargo.lock`. Background: `docs/aac-dependency.md`. | MPL-2.0 (when it was present) | Via LA patent pool; terms for source or free binaries unverified. | Excluded from every build. Chromium parity recorded as unsupported. |
| MP3, MP1, MP2 | package `symphonia-bundle-mp3` (a real package in `Cargo.lock`, not only a feature name), 0.5.5 and 0.6.1 both resolved | MPL-2.0 | Not assessed (MP3 patents are reported expired in secondary sources; not verified here). | Not assessed. |
| FLAC | package `symphonia-bundle-flac`, 0.5.5 and 0.6.1 both resolved | MPL-2.0 | Not assessed. | Not assessed. |
| Vorbis (in Ogg) | packages `symphonia-codec-vorbis`, `symphonia-format-ogg`, 0.5.5 and 0.6.1 both resolved | MPL-2.0 | Not assessed. | Not assessed. |
| PCM, ADPCM, A-law, mu-law, AIFF/WAV/CAF | `symphonia-codec-pcm`, `symphonia-codec-adpcm`, `symphonia-format-riff`, `-caf` (0.6.1); `hound` 3.5.1 (Apache-2.0, WAV) | MPL-2.0 / Apache-2.0 | Not assessed. | Not assessed. |
| ALAC | `symphonia-codec-alac` | MPL-2.0 | Not assessed; in the build through the MP4 demuxer (`symphonia-format-isomp4`, kept). | Decide with the audio review. |
| Opus | Not decodable natively (rejected). | | | None. |

Source-license note for the Symphonia crates (MPL-2.0), a reading to be confirmed by a qualified reviewer, not a procedure: MPL-2.0 is a file-level copyleft. We do not modify the Symphonia files, so the obligations that matter for a distribution are keeping each crate's license text and copyright notices with the distribution, and, if a binary form is distributed, making the source of those MPL-covered files available as the license's section 3.2 describes (an unmodified file's source is the crate on crates.io). Changes to MPL-covered files would have to stay under MPL-2.0 with their source available. The exact wording of the release notices is for the legal review.

## Images (non-video) in the default build, for completeness

`png` 0.18.1, `gif` 0.14.2 (MIT OR Apache-2.0), `zune-jpeg` 0.5.15 (MIT OR Apache-2.0 OR Zlib), `image` 0.25.10 (MIT OR Apache-2.0), BMP via `image`. Not assessed for patents.

## What a release needs from this file

1. Every row has a patent position that someone qualified has reviewed, and a disposition the owner has approved in writing.
2. The notices listed are in the release package.
3. `python3 -I tools/check_no_aac.py --binary <artifact>` passes on each native and wasm artifact (PR #357), and an equivalent check exists for any other excluded codec.
4. The owner's separate release authorization.
