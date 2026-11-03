# AAC-LC in the dependency tree: investigation (issue #90)

Status: investigation record, 2026-11-02. Owner decision 2026-11-02: the existing Symphonia AAC-LC dependency stays while this is investigated; native AAC decoding is not a supported feature; nothing here buys a license or adds a patent obligation. AAC is not classified as permanently unsupported.

This is an engineering record, not legal advice. Items marked UNVERIFIED need a primary source before anyone relies on them.

## 1. What is compiled in, and who pulls it

| Crate | Version | License (Cargo.toml) | Pulled by |
| --- | --- | --- | --- |
| `symphonia-codec-aac` | 0.6.1 | MPL-2.0 | `symphonia` feature `aac`, enabled by `web-audio-api` 1.7.0 asking for symphonia feature `all` |
| `symphonia-codec-aac` | 0.5.5 | MPL-2.0 | `creek-decode-symphonia` 0.3.3, enabled by `creek` feature `decode-all`, which `web-audio-api` requests |
| `symphonia-format-isomp4` | 0.5.5, 0.6.1 | MPL-2.0 | same two paths (MP4/M4A container) |

Neither is requested by this repository. `crates/x3d-runtime/Cargo.toml` asks symphonia 0.6.1 for `mp3, flac, ogg, vorbis, wav, pcm, all-meta` only (PR #347). `web-audio-api` (MIT) declares symphonia and creek as non-optional dependencies with the full codec set, so Cargo feature unification turns AAC on for the whole graph.

Checked with `cargo tree --locked -i symphonia-codec-aac@<version> --target all`:

* Native: in the graph under `x3d-runtime` (so `x3d-render`, `x3d-script`, `x3d-viewer`, `x3d-web`).
* wasm32: in the graph for `x3d-web`, and the built `x3d_web.wasm` (release profile) contains `symphonia-codec-aac` and `symphonia-format-isomp4` code.
* The published crates `vrml`, `vrml97` and `x3d` do not depend on it.

## 2. Is it exposed at run time

Yes, partly, and it is not documented as supported. `AudioEngine::decode_pcm` (`crates/x3d-runtime/src/sound/mod.rs`) decodes WAV itself and sends everything else to `web-audio-api`'s `decode_audio_data_sync`, which probes with `symphonia::default::get_probe()` and `get_codecs()`, the registries that hold every enabled codec. Measured by `known_aac_differs_from_chromium` (`rea_b24_audio_decode.rs`):

* AAC-LC in an M4A with the `moov` atom first (faststart) decodes, to the wrong length (not Chromium's 14400 frames).
* AAC in an M4A with `moov` last fails (the reader is not seekable).
* No other path reaches the decoder: there is no AAC reference in `x3d-runtime/src`, and `MovieTexture` decodes VP9 WebM only.

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

Probe result (a copy of `web-audio-api` 1.7.0 with those two edits, applied with `--config patch.crates-io.web-audio-api.path=...` in a scratch worktree, nothing committed):

* `cargo tree -i symphonia-codec-aac`: no package matches, for either version.
* `cargo nextest run -p x3d-runtime` on `audio_ops`, `audio_pull`, `rea_b24_audio_decode`: 21 passed, 1 failed. The failure is `known_aac_differs_from_chromium`, which pins the AAC decode and now sees `unsupported audio codec: 0x1007`. Nothing else changed: MP3, FLAC, Ogg Vorbis, WAV, A-law and mu-law results are the same.
* The isomp4 demuxer and ALAC stay, because they are separate from AAC and this keeps the change to AAC alone.

Ways to apply it, none done yet:

1. Upstream change to `web-audio-api` that makes its codec set selectable (a feature per codec, default unchanged). Cleanest; depends on a third party and its release.
2. `[patch.crates-io]` to a fork or a vendored copy of `web-audio-api` 1.7.0 with the two edits. Immediate; adds maintenance of a patched third-party crate (MIT, so redistribution is allowed), and a fork repository is an outward-facing action that needs the owner's say-so.
3. Decode everything ourselves with `symphonia` and drop `web-audio-api`'s decode path. Does not remove the compiled code unless combined with 1 or 2, because the dependency still enables the features.

## 5. Source-code license versus codec patents

Two separate things.

Copyright license. `symphonia*` is MPL-2.0 (file-level copyleft: changes to MPL files must stay MPL when distributed; linking from an MIT project is allowed). The project is MIT. `web-audio-api` is MIT, `creek-decode-symphonia` is MIT OR Apache-2.0. These are compatible with an MIT open-source project, and #347 already relies on that. The AAC decoder in Symphonia was ported from NihAV and relicensed by its author (the crate README says so). This answers "may we use and redistribute the source", and nothing about patents.

Patents. AAC is an ISO/IEC 14496-3 codec. A copyright license, including MPL-2.0, does not grant rights under third-party patents. Findings, all from secondary sources and UNVERIFIED against Via LA's current terms:

* Via LA administers the AAC patent license program. Its FAQ, as quoted in a 2017 mailing-list thread, says fees are due on the sale of encoders and decoders, and not on distributing bitstreams. [Icecast thread quoting the FAQ](https://lists.xiph.org/pipermail/icecast/2017-May/013947.html)
* Fraunhofer's FDK AAC license states that its software license does not license patent claims, and that products must get patent licenses from Via or the patent owners. [FDK license text](https://android.googlesource.com/platform/external/aac/+/baa3203/libAACdec/src/aacdecoder.h)
* Some extensions (HE-AAC v2 parametric stereo, xHE-AAC) have separate or later-expiring patents; Symphonia's decoder covers AAC-LC (and HE-AAC SBR is a separate question). Expiry claims in forums conflict (2017 versus 2028 for the baseline), and expiry differs by country. Do not rely on any of them.
* Whether publishing source that contains a decoder, with no binary, triggers any license duty is not answered by these sources. The FAQ reading above suggests product sales are the trigger, which is a reading, not a ruling.

## 6. What our distribution model requires (as far as verified)

* Source-only on GitHub (today): no sale of a product, no binary. On the FAQ reading above, no AAC license duty is evident. Not confirmed by Via LA or counsel.
* Binary or wasm release packages (planned): the decoder is in them unless section 4 is applied. A free open-source binary that decodes AAC is the case where the question "does a free product count as a product sold" arises. This is the one decision to settle before publishing release packages.
* Hosted use in a browser: the page's own `AudioContext` decodes media with the browser's codecs; the engine's AAC code in the wasm module is not what the page uses. This should be re-checked when the wasm audio path is next changed.

## 7. Recommendation

1. Keep AAC unsupported and unclaimed, as decided. Keep `known_aac_differs_from_chromium` as the record of the compiled-in behavior.
2. Before any release package is built, apply section 4 option 1 or 2 so the packages do not contain the decoder, and change that test to expect an `unsupported audio codec` error. This needs no patent license and loses no supported feature.
3. Raise the upstream change in section 4 option 1 when the owner wants to contact that project (outward-facing; not done).
4. Settle the section 5 question for the case the owner actually wants (a free binary that decodes AAC) only if AAC decoding is ever to become a supported feature.
