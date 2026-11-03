# AAC-LC in the dependency tree: investigation (issue #90)

Status: investigation record, 2026-11-02. Owner decision 2026-11-02: the existing Symphonia AAC-LC dependency stays while this is investigated; native AAC decoding is not a supported feature; the decision does not authorize a license purchase or release of AAC decoding. This document does not decide any patent question. AAC is not classified as permanently unsupported.

This is an engineering record, not legal advice. Items marked UNVERIFIED need a primary source before anyone relies on them.

## 1. What is compiled in, and who pulls it

| Crate | Version | License (Cargo.toml) | Pulled by |
| --- | --- | --- | --- |
| `symphonia-codec-aac` | 0.6.1 | MPL-2.0 | `symphonia` feature `aac`, enabled by `web-audio-api` 1.7.0 asking for symphonia feature `all` |
| `symphonia-codec-aac` | 0.5.5 | MPL-2.0 | `creek-decode-symphonia` 0.3.3, enabled by `creek` feature `decode-all`, which `web-audio-api` requests |
| `symphonia-format-isomp4` | 0.5.5, 0.6.1 | MPL-2.0 | same two paths (MP4/M4A container) |

Neither is directly requested by `x3d-runtime`; both enter through `web-audio-api`. `crates/x3d-runtime/Cargo.toml` asks symphonia 0.6.1 for `mp3, flac, ogg, vorbis, wav, pcm, all-meta` only (PR #347). `web-audio-api` (MIT) declares symphonia and creek as non-optional dependencies with the full codec set, so Cargo feature unification turns AAC on for the whole graph.

Checked with `cargo tree --locked -i symphonia-codec-aac@<version> --target all` (`--target all` is a `cargo tree` option that merges every target's graph; it is not a target triple). Terms used below: **compiled in** = present in the artifact; **reachable** = selected by a runtime path in that deployment; **supported** = implemented on purpose and covered by the compatibility contract. AAC is compiled in and, on the host target (the desktop, non-wasm build), reachable; it is not supported.

* Native: in the graph under `x3d-runtime` (so `x3d-render`, `x3d-script`, `x3d-viewer`, `x3d-web`).
* wasm32: in the graph for `x3d-web` (`cargo tree --locked --offline -i symphonia-codec-aac@0.6.1 --target wasm32-unknown-unknown -p x3d-web`). The release-profile `x3d_web.wasm` in the shared build cache carries the crate source paths of `symphonia-codec-aac` and `symphonia-format-isomp4` (`strings -a x3d_web.wasm | rg 'symphonia.codec.aac|isomp4'`: 63 matching output lines for the AAC crate, not 63 distinct code references). That is evidence by embedded path strings (panic metadata), not by symbol inspection or a controlled decode in the wasm build; it shows the crate was compiled in, and the dependency graph shows why. Whether the wasm module's `decodeAudioData` path reaches it in a browser deployment was NOT tested: the page may decode with the browser's own `AudioContext` (the audio mirror) instead. Treat the wasm decoder as compiled in, reachability unknown.
* The published crates `vrml`, `vrml97` and `x3d` do not depend on it.

## 2. Is it exposed at run time

Yes, partly, and it is not documented as supported. `AudioEngine::decode_pcm` (`crates/x3d-runtime/src/sound/mod.rs`) decodes WAV itself and sends everything else to `web-audio-api`'s `decode_audio_data_sync`, which probes with `symphonia::default::get_probe()` and `get_codecs()`, the registries that hold every enabled codec. Measured by `known_aac_differs_from_chromium` (`rea_b24_audio_decode.rs`):

* AAC-LC in an M4A with the `moov` atom first (faststart) decodes, to the wrong length (not Chromium's 14400 frames).
* AAC in an M4A with `moov` last fails (the reader is not seekable).
* No other AAC-specific path was identified in `x3d-runtime/src` (a text search for `aac`, `m4a`, `mp4a` and `isomp4` found none, and `MovieTexture` decodes VP9 WebM only). That is a text search, not a call-graph review: any path that passes non-WAV data to `decode_audio_data_sync` can reach the enabled AAC decoder.

So "native AAC decoding disabled" is true of the feature set we claim and test against Chromium, and false of the compiled code. The AAC decoder can be reached by an AudioClip or `decodeAudioData` input.

## 3. Does a build distribute it

* No release has been published (`gh release list` is empty; the README says there is no stable release and lists release packages as future work). CI uploads test logs and evidence, not binaries.
* The repository is source-only today. A user who runs `cargo build` or `tools/build-web.sh` makes the binary that contains the decoder.
* If release packages (native or the wasm module) are published later, they would contain the AAC decoder unless it is removed first.

## 4. Can feature selection remove it

Not from this workspace: the features are fixed inside `web-audio-api`'s manifest. A probe shows two edits to that manifest remove AAC from the whole graph, both versions:

```toml
[dependencies.creek]      # replace "decode-all" by:
features = ["decode", "decode-alac", "decode-flac", "decode-mp3", "decode-pcm",
            "decode-isomp4", "decode-ogg", "decode-vorbis", "decode-wav", "encode-wav"]

[dependencies.symphonia]  # replace "all" by:
features = ["adpcm", "alac", "flac", "mp1", "mp2", "mp3", "pcm", "vorbis", "caf", "isomp4",
            "mkv", "ogg", "aiff", "wav", "all-meta", "opt-simd"]
```

Probe method (a copy of `web-audio-api` 1.7.0 with those two edits, applied with `--config 'patch.crates-io.web-audio-api.path="<copy>"'` in a scratch worktree; the probe rewrote `Cargo.lock` there, which was reverted and not committed):

```
cargo tree --config '<patch>' -i symphonia-codec-aac --target all
cargo nextest run -j 6 --no-fail-fast --config '<patch>' -p x3d-runtime -E 'binary(audio_ops) | binary(audio_pull) | binary(rea_b24_audio_decode)'
```

The unversioned `-i symphonia-codec-aac` matches every version in the graph, so "did not match any packages" means neither 0.5.5 nor 0.6.1 is present (before the edits the same name resolves to both). The implementation PR repeats this with `--locked` and with both versions named.

Probe result:

* `cargo tree -i symphonia-codec-aac`: no package matches, for either version.
* `cargo nextest run -p x3d-runtime` on `audio_ops`, `audio_pull`, `rea_b24_audio_decode`: 21 passed, 1 failed. The failure is `known_aac_differs_from_chromium`, which pins the AAC decode and now sees `unsupported audio codec: 0x1007`. Nothing else changed: MP3, FLAC, Ogg Vorbis, WAV, A-law and mu-law results are the same.
* The isomp4 demuxer and ALAC stay, because they are separate from AAC and this keeps the change to AAC alone.

Ways to apply it, none done yet:

1. Upstream change to `web-audio-api` that makes its codec set selectable. Cargo features are additive, so a feature that turns AAC off cannot exist; the shape that works is the codecs as opt-in features (`aac`, `alac`, ... each enabling the matching Symphonia and Creek feature) under a `default` that a consumer can drop: this project would depend on `web-audio-api` with `default-features = false` and list the codecs it accepts, while `default` keeps today's behavior for other consumers. The `symphonia` and `creek` dependencies already set `default-features = false` in the manifest. Cleanest; depends on a third party and its release.
2. `[patch.crates-io]` to a fork or a vendored copy of `web-audio-api` 1.7.0 with the two edits. Immediate; adds maintenance of a patched third-party crate (MIT, so redistribution is allowed), and a fork repository is an outward-facing action that needs the owner's say-so.
3. Decode everything ourselves with `symphonia` and drop `web-audio-api`'s decode path. Does not remove the compiled code unless combined with 1 or 2, because the dependency still enables the features.

## 5. Source-code license versus codec patents

Two separate things.

Copyright license. `symphonia*` is MPL-2.0 (modified MPL-covered files stay under MPL-2.0 source and notice obligations when distributed; MPL-covered files may be combined with larger works under other terms if those requirements are met; see <https://www.mozilla.org/en-US/MPL/2.0/>). The project is MIT. `web-audio-api` is MIT, `creek-decode-symphonia` is MIT OR Apache-2.0. These are compatible with an MIT open-source project, and #347 already relies on that. The AAC decoder in Symphonia was ported from NihAV and relicensed by its author (the crate README says so). This answers "may we use and redistribute the source", and nothing about patents.

Patents. AAC is an ISO/IEC 14496-3 codec. A copyright license, including MPL-2.0, does not grant rights under third-party patents. Findings, all from secondary sources and UNVERIFIED against Via LA's current terms. This investigation does not itself establish whether any patent license or royalty obligation applies to any distribution model; before distributing a binary that contains an AAC decoder, obtain a current primary-source assessment and, if needed, advice from qualified counsel.

* **UNVERIFIED, secondary source.** Via LA administers the AAC patent license program. Its FAQ, as quoted in a 2017 mailing-list thread, says fees are due on the sale of encoders and decoders, and not on distributing bitstreams. [Icecast thread quoting the FAQ](https://lists.xiph.org/pipermail/icecast/2017-May/013947.html)
* **UNVERIFIED, secondary source.** Fraunhofer's FDK AAC license states that its software license does not license patent claims, and that products must get patent licenses from Via or the patent owners. [FDK license text](https://android.googlesource.com/platform/external/aac/+/baa3203/libAACdec/src/aacdecoder.h)
* **UNVERIFIED, forum claims.** Some extensions (HE-AAC v2 parametric stereo, xHE-AAC) have separate or later-expiring patents; Symphonia's decoder covers AAC-LC (and HE-AAC SBR is a separate question). Expiry claims in forums conflict (2017 versus 2028 for the baseline), and expiry differs by country. Do not rely on any of them. Even a current Via LA primary source would not by itself decide whether a particular free binary or a source distribution needs a license.
* Whether publishing source that contains a decoder, with no binary, triggers any license duty is not answered by these sources and has not been confirmed.

## 6. What our distribution model requires (as far as verified)

* Source-only on GitHub (today): the repository publishes source and no release binaries. Whether source distribution creates any patent obligation is outside the scope of this engineering investigation and has not been confirmed.
* Binary or wasm release packages (planned): the decoder is in them unless section 4 is applied. A free open-source binary that decodes AAC is the case where the question "does a free product count as a product sold" arises. This is the one decision to settle before publishing release packages.
* Hosted use in a browser: the page's own `AudioContext` decodes media with the browser's codecs; the engine's AAC code in the wasm module is not what the page uses. This should be re-checked when the wasm audio path is next changed.

## 7. Recommendation

1. Keep AAC unsupported and unclaimed, as decided. Keep `known_aac_differs_from_chromium` as the record of the compiled-in behavior.
2. Before any release package is built, apply section 4 option 1 or 2 so the packages do not contain the decoder, and change that test to expect an `unsupported audio codec` error. This removes the AAC code from the packages and loses no supported feature. It does not by itself settle patent compliance: the resulting dependency graph and binaries must be checked, and the questions in section 5 stay open.
3. Raise the upstream change in section 4 option 1 when the owner wants to contact that project (outward-facing; not done).
4. If AAC stays out of every release artifact, the immediate distribution question narrows to source distribution. If AAC is ever left compiled into a distributable artifact, or becomes a supported feature, obtain the patent assessment first, whether or not decoding is advertised.
