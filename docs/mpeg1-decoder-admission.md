# MPEG-1 decoder admission (issue #90, BR1.7)

Checked 2026-11-02. This document is separate from `docs/decoder-admission.md` (VP8, VP9, AV1) on purpose. It follows
owner record a0acb58b: the ISO-required MPEG-1 functionality of `MovieTexture` is to be implemented; it is not an
approved difference because pinned X_ITE or Chromium lacks it; pinned behavior stays available under a documented
compatibility option; codec support is separate from reverse playback (ISO/IEC 19775-1 18.4.2 time, plan step (a)).

## What is required

ISO/IEC 19775-1 V4.0 18.4.2 (quoted in `docs/mpeg1-investigation.md`): `MovieTexture` "shall support MPEG1-Systems (audio
and video) or MPEG1-Video (video-only) movie file formats as defined in ISO/IEC 11172-1". So three things:

| Part | Public identity | What it is for | Clause numbers |
| --- | --- | --- | --- |
| ISO/IEC 11172-1 | Information technology - Coding of moving pictures and associated audio for digital storage media at up to about 1,5 Mbit/s - Part 1: Systems | the container: pack and system headers, packets, PTS/DTS | pending: not in the local standards library, specs are not fetched from the web |
| ISO/IEC 11172-2 | Part 2: Video | pictures (I, P, B, D) | pending, same reason |
| ISO/IEC 11172-3 | Part 3: Audio | Layer I and II (and III) audio | pending, same reason |

The owner is asked to supply the three parts; until then no table, constant or algorithm is quoted from them. Nothing in
this repository may be described as conforming to them yet.

## Candidates

All were looked at on 2026-11-02 (crates in the local cargo cache or `cargo info`; no source was copied).

### Video

| Candidate | License | Findings | Verdict |
| --- | --- | --- | --- |
| `oxideav-mpeg12video` 0.0.13 (sha256 `3f9004fa786827c64dec526454e5f9de6f9a6426b9e239124fed494ab4816776`) | MIT, "Copyright (c) 2026 Karpeles Lab Inc." | 64 314 lines in 70 files, with encoders, rate control and MPEG-2 (far more than a texture decoder needs); no `unsafe`; depends on `oxideav-core`. Its `CHANGELOG.md` says "orphan rebuild: clean-room scaffold post 2026-05-18 audit" and "Prior master history was force-erased on 2026-05-18 under Hat-3 cold enforcement of the workspace clean-room policy". | Not admitted. Same provenance pattern as `oxideav-vp8`, rejected in `docs/decoder-admission.md`: the history of the code that is there cannot be established. |
| `na_mpeg2_decoder` 0.1.0 (sha256 `ffc7475176edabad8b4c3f6380bedca7327f7f8b4b7737645cf8eaec319d50f0`) | MPL-2.0 | 898 lines of glue (`av.rs`, `pipeline.rs`) that build on the `oxideav-*` crates; part of a game-engine repository. | Not admitted: it inherits the `oxideav` provenance, and adds a license that is neither MIT nor already in the dependency graph for video. |
| `southstar-codecs` 1.0.5 (sha256 `a4db934e3f06ab7add39a68ff975656daa3ea681575ec3681a71393264233a31`) | `(LicenseRef-NSL-1.0 OR GPL-3.0-or-later) AND (MIT)` | A Rust port of pl_mpeg (MPEG-1 video, MP2), of minimp3, and FFmpeg libav declarations. | Not admitted: in SPDX `AND` binds both sides, so MIT alone is not an option for the whole crate; the other side is a non-standard license or GPL-3.0-or-later. |
| pl_mpeg (C, one file, <https://github.com/phoboslab/pl_mpeg>) | MIT, Dominic Szablewski | MPEG-1 Systems (program stream) demux, MPEG-1 video, MP2 audio in about 4 000 lines. Well known, widely ported, no external dependencies. Its tables are the standard's. | License passes. It is C: linking it needs a build script and a C compiler on every target and does not build for `wasm32-unknown-unknown` without a toolchain, and C code is the class of risk the subprocess tests in BR1.8 exist for. As a source for a Rust port it is the shortest route: the structure is small and readable. Recommended as the REFERENCE for a port, with the MIT notice added to `THIRD_PARTY_NOTICES.md`. |
| libmpeg2 (GPL-2.0-or-later), FFmpeg (LGPL-2.1+/GPL) | copyleft | | Rejected (BR1.7). FFmpeg is also not to be added as a dependency (owner). |
| An in-repo decoder written from ISO/IEC 11172-2 | MIT (ours) | The cleanest provenance, and the only route that can quote clauses. Needs the standard. | Possible once the owner supplies Part 2; not a blocker for the pl_mpeg route, which can be checked against the standard later. |

### Audio (Layer I and II)

`symphonia` 0.6.1, which `x3d-runtime` already links (MPL-2.0, pure Rust), decodes MPEG-1 Layer I and II through its `mp1`
and `mp2` features (they are part of `symphonia-bundle-mp3`, already compiled for `mp3`). The only change is two feature
names in `crates/x3d-runtime/Cargo.toml`; no new crate enters the graph. `web-audio-api` already enables them through its
`all` feature. Layer III (MP3) is already decoded.

### Systems demux

MPEG-1 Systems is a flat stream of start-code delimited packs and packets: about 300 lines of Rust that read pack headers,
system headers and PES packets, and collect the video (stream ids 0xE0-0xEF) and audio (0xC0-0xDF) payloads with their
PTS values. Written in this repository, bounds-checked, no new dependency. `mpeg2ps` 0.1.1 (MIT) exists; it was not
admitted because the demux is smaller than the review of an extra dependency.

## Recommended route (needs the owner)

1. Demux: in-repo, from 11172-1.
2. Audio: `symphonia` `mp1` and `mp2`.
3. Video: a Rust port of pl_mpeg's video decoder, in-repo (`crates/x3d-runtime/src/mpeg1/`), with pl_mpeg's MIT notice in
   `THIRD_PARTY_NOTICES.md`, behind the `native-video` feature of plan step (b). Alternative: an in-repo decoder from 11172-2
   when the owner supplies it.

## Patent and distribution

Unchanged from `docs/mpeg1-investigation.md`: a preliminary screening from a secondary source found no unexpired MPEG-1 Video
or Layer I/II patent; Layer III (MP3) patents expired by 30 December 2017. This is not a freedom-to-operate opinion, and
nothing here claims clearance. MIT/MPL notices travel with the binaries through `THIRD_PARTY_NOTICES.md`.

## Tests required before admission

* Reference: fixtures made with ffmpeg's `mpeg1video` and `mp2` encoders from synthetic `lavfi` sources, for example

  ```
  ffmpeg -f lavfi -i testsrc2=size=96x64:rate=25 -t 1 -c:v mpeg1video -g 6 -bf 2 -an -f mpeg video.mpg
  ffmpeg -f lavfi -i testsrc2=size=96x64:rate=25 -f lavfi -i sine=frequency=440:sample_rate=44100 -t 1 \
         -c:v mpeg1video -c:a mp2 -f mpeg av.mpg
  ```

  and the planes of ffmpeg's own decoder (`-f rawvideo -pix_fmt yuv420p`) as the reference. MPEG-1 allows decoders to
  differ in the inverse DCT within the accuracy limits of IEEE 1180, so the comparison is a per-sample bound (measure it
  with ffmpeg's two decoders against each other first and fix it before the comparison, BR1.4), not a hash.
* Negative tests: unsupported variants (MPEG-2 video, a non-4:2:0 sequence, free-format audio), truncated and mutated
  streams; every one must end in an error or fewer pictures, never a crash. The mutation tests run in CHILD processes
  (`crates/x3d-runtime/tests/av1_safety.rs` is the pattern) whatever the implementation, and are mandatory for any
  `unsafe` or C code.
* Pinned behavior: pinned X_ITE in Chrome cannot play MPEG-1 (`canPlayType("video/mpeg")` is empty). That stays reachable
  as a documented compatibility option next to the ISO default; it is never labeled an approved difference.
* Chrome cannot serve as the pictures' oracle for MPEG-1; ffmpeg's decoder is.
