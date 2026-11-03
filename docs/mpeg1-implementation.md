# MPEG-1 decoding for MovieTexture (issue #90)

Owner decision record a0acb58b (2026-11-02): the ISO-required MPEG-1 functionality of `MovieTexture` is implemented. ISO/IEC
19775-1 V4.0 18.4.2 says the url "shall support MPEG1-Systems (audio and video) or MPEG1-Video (video-only) movie file formats
as defined in ISO/IEC 11172-1". The pinned X_ITE 16.5.0 plays a movie through a `<video>` element, and Chrome cannot play MPEG-1
(`canPlayType("video/mpeg")` is empty), so the pinned page shows nothing for such a file. That is **not** an approved
DIFFERENT. The ISO behavior (MPEG-1 plays) is the default; the pinned behavior is a documented compatibility option.

The standards ISO/IEC 11172-1 (Systems), 11172-2 (Video) and 11172-3 (Audio) are not in the local library and are not fetched.
They are cited by identity only; **every clause number is pending the owner**. Nothing here claims conformance to them.

Codec support is separate from reverse playback and from the ISO time model (another lane). This change only produces
pictures with a picture period.

## What is implemented (this change)

| Part | File | Notes |
| --- | --- | --- |
| Systems demux | `crates/x3d-runtime/src/mpeg1/demux.rs` | Start-code scan; pack, system header, packets of the first video stream (0xE0-0xEF); detects an audio stream (0xC0-0xDF) but does not decode it. MPEG-2 pack and PES headers are refused. A packet cut off by the end of the file is taken as far as it goes. |
| Video decoder | `crates/x3d-runtime/src/mpeg1/video.rs`, `tables.rs` | I, P and B pictures, 4:2:0, custom quantizer matrices, any size up to 4095 x 4095 (not a multiple of 16, odd sizes), repeated sequence headers, display-order output. D pictures are skipped. MPEG-2 video is refused. |
| API | `crates/x3d-runtime/src/mpeg1/mod.rs` | `decode_yuv`, `decode` (RGBA with a byte budget), `looks_like_mpeg1`, `has_mpeg1_extension`, `Mpeg1Error`. |
| Node wiring | `nodes/movie_texture/mpeg1_media.rs` (child module, one `mod` line in `movie_texture.rs`), `loader.rs` (one branch in `movie_load`), `lib.rs` (`mpeg1_unplayable`) | `MovieMedia::decode_mpeg1`; frames play like a GIF's, each lasting one picture period (`1000 / framerate` ms). |

Detection: a url whose path ends in `.mpg`, `.mpeg`, `.mpe` or `.m1v` is fetched and decoded; the decoder then checks the first
four bytes (a pack start code `00 00 01 BA`, or a sequence header `00 00 01 B3`). A url with another extension is not fetched to
sniff it (that would add requests the existing flow does not make); extension-less MPEG-1 urls are a follow-up if the owner
wants them.

A file that does not decode is an unplayable video: a warning `MPEG-1 movie '<url>': <reason>` and then the same `Couldn't load video`
diagnostic as any video the engine cannot show, the next url of the list is tried, and the scene keeps running.

### Where this runs

`Fetcher::plays_video()` is false for the native build (`x3d-view`, tests): `movie_load` then decodes video itself, and MPEG-1 joins
VP8 and VP9 there. In the browser build (`plays_video()` true) the page's `<video>` element loads the file and Chrome cannot play
MPEG-1, so the ISO behavior is **not** reachable there yet: decoding in the wasm build needs the fetched bytes handed to
`MovieMedia::decode_mpeg1` and a way to show the frames, which belongs to the browser video path of PR #353/#361. Recorded as a
follow-up, not done here.

### The compatibility option

`Runtime::mpeg1_unplayable` (public field, default `false`). When `true` the loader does not try to decode an MPEG-1 url, so the
file falls through to the existing "not a playable video" flow, exactly as before this change and as the pinned page behaves (test
`the_compatibility_option_treats_mpeg1_as_unplayable_like_the_pinned_page`). It is named a compatibility option, never a
DIFFERENT.

## Provenance of the video decoder

The decoder is a safe-Rust port of the video part of **pl_mpeg** (C, MIT, Dominic Szablewski).

- Source: https://github.com/phoboslab/pl_mpeg at commit `c871f2be022ece7ef4f64230b4fb8e1fb9eb6023` ("Fix corrupt slice check;
  close #64", 2025-12-30), the head of the default branch on 2026-11-02.
- `pl_mpeg.h` SHA-256: `3a8cb30c83c2a1147719c30fe0c8b93da2987aa43140077c575b39aaa75fc2c9` (4439 lines).
- License: MIT, stated on line 3 of the header (`SPDX-License-Identifier: MIT`) and in the README ("Single-file MIT licensed
  library"). **There is no license file at that commit** (the tree is `README.md`, `makefile`, `pl_mpeg.h` and three example
  programs; the GitHub license API answers 404). The notice with the standard MIT text is in `THIRD_PARTY_NOTICES.md`,
  section `## pl_mpeg`. Owner decision: accept the SPDX tag as the license statement, or ask the author for a license file.
- Used: the structure of the video decoder (slice, macroblock, motion vector, block decoding), and the variable length code
  trees, zig-zag order and default quantizer matrices of `tables.rs`. Every tree entry and matrix was compared with the C
  source by a script (9 trees, 3 matrices; all equal) after transcription. Not used: its audio decoder (kjmp2 based), its demuxer
  (the demux here is written from the format's start-code structure), its color conversion.
- The video decoder of pl_mpeg says it is inspired by the Java MPEG-1 Video Decoder by Zoltan Korandi (Source Forge); noted in the
  notice.

### Where this port differs from pl_mpeg

Found by comparing with ffmpeg (see "Tolerance"), each with a reason:

1. **Inverse DCT.** pl_mpeg's integer IDCT (8-bit constants) is replaced by the double-precision definition, rounded. With pl_mpeg's
   IDCT, intra-only streams differed from ffmpeg by up to 3 levels; the exact IDCT alone brought that to 2 (and the mean from
   0.28 to 0.17), the dequantizer fix below to 1. The definition is the reference IEEE 1180 measures
   accuracy against, which MPEG-1 decoders must meet (clause pending).
2. **Dequantization of negative levels.** pl_mpeg shifts the signed product (`>> 4`), which rounds negative values away from
   zero; the standard divides, truncating toward zero. The magnitude is computed and the sign applied after (this also fixes the
   "make odd" step for negative levels).
3. **End of the last slice.** pl_mpeg ends a slice when fewer than 23 bits remain, so a stream that ends inside its last slice (no
   start code behind it) loses the last macroblocks (here: one macroblock row of a B picture, up to 7 levels in chroma). Bits past
   the end count as zero.
4. **Flush at the end.** pl_mpeg only outputs the held reference picture when the last coded picture was I or P; a stream
   ending with B pictures (the usual order `I P B B`) would lose its last P. Here it is always output.
5. **Bounds.** Every motion vector reference is checked against the picture (a vector that points outside the picture predicts
   nothing, as pl_mpeg's check intends but with a test that lets a block wrap across a row), every slice, block and run index is
   checked, bits past the end are an error (`Truncated`), a picture size change is refused, a code that is no code is an error.
   pl_mpeg ignores several of these.
6. A repeated sequence header replaces the quantizer matrices (pl_mpeg reads the first only); MPEG-2 video (a sequence extension
   behind the header) is refused; the last odd column and row of an odd size are converted.

## Colour and timing

- RGBA conversion is ITU-R BT.601 limited range with chroma interpolated linearly between the sample centers (the siting MPEG-1
  defines). MPEG-1 video carries no colour description and its pictures are conventionally BT.601; VP9 in the same engine uses
  BT.709 because that is what was measured for untagged VP9 in the pinned page. There is no pinned measurement for MPEG-1 (the pinned
  page cannot play it), so this is a decision, not a comparison. Test `a_red_picture_converts_to_red_rgba`.
- Pictures last `1000 / framerate` ms (the picture rate code of the sequence header, 23.976 to 60 Hz). Presentation time stamps of
  the Systems layer are read for the first video packet but not used for the schedule.

## Audio (not in this change)

The sound track of a Systems stream (MPEG-1 Layer I/II; Layer III is already decoded for AudioClip) is detected and **not
decoded**; `MovieTexture.hasSound` stays false for these movies, as it does for the VP8/VP9 frames decoded natively on master.
Reason, precisely: on master a natively decoded video has no sound path at all. The path that gives a decoded video its sound
(`Sound` reading a MovieTexture, the audio track of the WebM decoded through `symphonia`) is the work of PR #361, which is not merged.
Wiring MPEG-1 audio into a path that does not exist on master would invent its interface. The audio decode itself needs no new
crate: `symphonia` 0.6.1 is already a dependency of `x3d-runtime` (MPL-2.0, pure Rust, features `mp3 flac ogg vorbis wav pcm
all-meta`); its `mp1` and `mp2` features (the Layer I and II decoders live in `symphonia-bundle-mp3`, already compiled) are the only
change needed, so `python3 -I tools/check_no_aac.py` (PR #357) is unaffected. The step is: demux the audio stream (the demuxer already
sees it), decode with symphonia, hand the samples to the video audio path of #361. Compare against ffmpeg's Layer II decode
with a fixed sample bound.

## Licensing, patents, security

- Licenses: pl_mpeg MIT (notice above); no other code is added. No new crate.
- Patents (preliminary screening only, **not legal advice, not a freedom-to-operate opinion**): `docs/mpeg1-investigation.md` (PR #356)
  and `docs/mpeg1-decoder-admission.md` (PR #364) record that a screening from a secondary source found no unexpired MPEG-1 Video
  or Layer I/II patent. MPEG-1 Video and Layer I/II are treated apart from MP3 (Layer III), whose patents expired by 30 December
  2017. Nothing in this change claims clearance.
- Security: no `unsafe` and no FFI in `mpeg1/`. All input is untrusted: sizes come from 12-bit header fields (at most 4095 x 4095;
  the three working frames are about 75 MB at the maximum), the number of RGBA pictures is bounded by the existing
  `MAX_DECODED_BYTES` (512 MiB) of MovieTexture before they are kept, every read past the end is an error, arithmetic that
  untrusted values reach is widened or wrapping. The decoder cannot read or write outside its own buffers; a defect would show as
  a Rust panic, which the in-process tests below look for (the child-process isolation the plan asks for is for `unsafe`/FFI code,
  and there is none here). Truncation of every fixture at many lengths and 1600 deterministic byte mutations are in the tests.
- Determinism: the IDCT uses `f64` `cos`; the table is computed once. Results are exact to the rounding of IEEE double arithmetic;
  a platform whose `cos` differs in the last bit could in principle flip a rounding at an exact .5, changing a sample by 1.
  The tolerance below covers that.

## Tolerance and measured results

MPEG-1 allows decoders to differ in the inverse DCT within IEEE 1180 accuracy, and the difference accumulates through P and B
prediction. The comparison with ffmpeg (6.1.1, `mpeg1video` decoder, `-f rawvideo -pix_fmt yuv420p`) is a per-sample bound, **fixed in
the test before the first comparison**: every Y, Cb and Cr sample of every picture within `MAX_ABS = 3` of ffmpeg's, and the mean
absolute difference of a stream at most `MEAN_ABS = 0.5`. The bound was not changed afterwards.

Reference spread, for scale: ffmpeg's own `-idct int` against its default `-idct simple` on `ipb-64x48.m1v` differ by up to 2
(mean 0.022).

First comparison (pl_mpeg's algorithms as ported; this failed the bound and is why the differences above exist):

| Stream | worst | mean |
| --- | --- | --- |
| intra-64x48.m1v | 3 | 0.277 |
| ip-64x48.m1v | 3 | 0.313 |
| ipb-64x48.m1v | **8** (fails) | 0.309 |

Final (all seven streams, from `cargo nextest run -p x3d-runtime --test mpeg1_decode --no-capture`):

| Stream | Pictures | Size | worst | mean |
| --- | --- | --- | --- | --- |
| ipb-64x48.m1v (I, P, B) | 15 | 64x48 | 1 | 0.0103 |
| intra-64x48.m1v | 12 | 64x48 | 1 | 0.0087 |
| ip-64x48.m1v | 12 | 64x48 | 1 | 0.0113 |
| odd-50x38.m1v | 10 | 50x38 | 1 | 0.0083 |
| odd-33x25.m1v | 6 | 33x25 | 1 | 0.0155 |
| custom-matrix-64x48.m1v | 9 | 64x48 | 1 | 0.0103 |
| sys-av-64x48.mpg (Systems, mp2 audio) | 15 | 64x48 | 1 | 0.0103 |

Worst case over all streams: 1 level.

## Fixtures and how to regenerate them

`crates/x3d-runtime/tests/data/mpeg1/` (about 400 KB, newly authored, synthetic: ffmpeg `lavfi` sources only; no third-party or
historical content). ffmpeg 6.1.1, run from that directory; `F` is `ffmpeg -v error -y -f lavfi -i testsrc2=size=64x48:rate=25`:

```
$F -frames:v 15 -c:v mpeg1video -q:v 4 -g 6 -bf 2 -an -f mpeg1video ipb-64x48.m1v
$F -frames:v 12 -c:v mpeg1video -q:v 4 -g 1 -bf 0 -an -f mpeg1video intra-64x48.m1v
$F -frames:v 12 -c:v mpeg1video -q:v 4 -g 12 -bf 0 -an -f mpeg1video ip-64x48.m1v
ffmpeg -v error -y -f lavfi -i testsrc2=size=64x48:rate=25 -f lavfi -i sine=frequency=440:sample_rate=44100 \
  -frames:v 15 -t 0.6 -c:v mpeg1video -q:v 4 -g 6 -bf 2 -c:a mp2 -b:a 64k -f mpeg sys-av-64x48.mpg
ffmpeg -v error -y -f lavfi -i testsrc2=size=50x38:rate=25 -frames:v 10 -c:v mpeg1video -q:v 4 -g 5 -bf 2 -an -f mpeg1video odd-50x38.m1v
$F -frames:v 6 -vf "scale=33:25,format=yuv420p" -c:v mpeg1video -q:v 4 -g 3 -bf 1 -an -f mpeg1video odd-33x25.m1v
$F -frames:v 9 -c:v mpeg1video -q:v 4 -g 6 -bf 2 -an -intra_matrix 8,16,19,22,26,27,29,34,16,16,22,24,27,29,34,37,19,22,26,27,29,34,34,38,22,22,26,27,29,34,37,40,22,26,27,29,32,35,40,48,26,27,29,32,35,40,48,58,26,27,29,34,38,46,56,69,27,29,35,38,46,56,69,83 -f mpeg1video custom-matrix-64x48.m1v
ffmpeg -v error -y -f lavfi -i color=c=red:size=32x32:rate=25 -frames:v 4 -c:v mpeg1video -q:v 2 -g 4 -bf 0 -an -f mpeg1video red-32x32.m1v
for f in *.m1v *.mpg; do ffmpeg -v error -y -i $f -an -f rawvideo -pix_fmt yuv420p ${f%.*}.yuv; done   # the references
```

(`testsrc2` rounds odd sizes to even, hence the `scale` filter for 33x25. The custom matrix in the fixture is the default intra matrix
written explicitly, so the path that reads a matrix from the header is exercised with known values.)

## Tests

`crates/x3d-runtime/tests/mpeg1_decode.rs` (decoder and demux, 20 tests): seven stream comparisons with ffmpeg, red-picture colour,
format and extension detection, demux equals the Systems decode, video-only is not a Systems stream, MPEG-2 pack / PES header / video
refused, a Systems stream with no video packet, empty / tiny / garbage / zero-size / header-only inputs, truncation at many lengths
of four fixtures, 1600 mutated streams (no panic), the byte budget, a stream cut inside a picture keeps the pictures before it.

`crates/x3d-runtime/tests/mpeg1_movie_texture.rs` (node wiring, 7 tests): the option is off by default; a Systems movie and each of
`.m1v .MPEG .mpe .mpg` load as frames of the right size; the compatibility option gives the pinned result (Failed, the usual
diagnostic, no frames); garbage, MPEG-2 and truncated data fail with the usual diagnostic and the reason; a bad file falls through
to the next url; a missing file fails and the scene keeps running.

Run: `CARGO_BUILD_JOBS=6 cargo nextest run -j 6 -p x3d-runtime mpeg1` and `cargo test --doc -p x3d-runtime`.

## Open items

- Clause numbers of ISO/IEC 11172-1/-2/-3: pending the owner supplying the standards.
- License file for pl_mpeg: accept the SPDX tag, or ask the author.
- Audio (above), the browser build path (above), extension-less urls, and a comparison with a second independent decoder (an in-repo
  decoder written from 11172-2 once the owner supplies Part 2, as `docs/mpeg1-decoder-admission.md` allows).
