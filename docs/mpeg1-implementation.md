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

## Status: partial. ISO 18.4.2 acceptance criteria that stay OPEN

This change is **not** full MPEG-1 MovieTexture compliance and must not be described as such. Owner review (record 2673a574):

- **MPEG-1 Systems audio decoding is done in the native build** (this change; see "Audio" below): the first audio stream of a
  Systems stream is decoded (Layer II, and Layer III) and plays through the node (`hasSound` true). Layer I is reported
  unsupported. Not done: the start offset between the audio and video streams is not applied.
- **Browser build integration is NOT done.** In the browser build the page's `<video>` element loads the file and cannot play MPEG-1.
  The ISO criterion is OPEN for the browser build.
- Done: the picture (video) half of both formats in the native build, with the compatibility option. Clause numbers, reverse playback
  and the ISO time model are also open (other lanes or the owner).

## What is implemented (this change)

| Part | File | Notes |
| --- | --- | --- |
| Systems demux | `crates/x3d-runtime/src/mpeg1/demux.rs` | Start-code scan; pack, system header, packets of the first video stream (0xE0-0xEF) and of the first audio stream (0xC0-0xDF), each concatenated into its elementary stream. MPEG-2 pack and PES headers are refused. A packet cut off by the end of the file is taken as far as it goes. |
| Video decoder | `crates/x3d-runtime/src/mpeg1/video.rs`, `tables.rs` | I, P and B pictures, 4:2:0, custom quantizer matrices, any size up to 4095 x 4095 (not a multiple of 16, odd sizes), repeated sequence headers, display-order output. D pictures are skipped. MPEG-2 video is refused. |
| API | `crates/x3d-runtime/src/mpeg1/mod.rs` | `decode_yuv`, `decode` (RGBA with a byte budget), `looks_like_mpeg1`, `has_mpeg1_extension`, `Mpeg1Error`. |
| Node wiring | `nodes/movie_texture/mpeg1_media.rs` (child module, one `mod` line in `movie_texture.rs`), `loader.rs` (one branch in `movie_load`), `lib.rs` (`mpeg1_unplayable`) | `MovieMedia::decode_mpeg1`; frames play like a GIF's, each lasting one picture period (`1000 / framerate` ms). |

Detection (pinned by tests): the **extension decides whether the decoder is tried, the bytes decide whether the file is MPEG-1**. An MPEG-1
stream under a non-MPEG extension (`.wrl`, `.bin`, no extension, `.mpgx`, `.mpg.txt`) is **not sniffed and not even requested**: it is
an unplayable video, as before this change. A `.mpg` holding garbage, zero bytes, MPEG-4, WebM, MPEG-2 or text is refused by its bytes.
The extension is matched on the url path, so a query or fragment does not hide it. A url whose path ends in `.mpg`, `.mpeg`, `.mpe` or `.m1v` is fetched and decoded; the decoder then checks the first
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

## Audio

`crates/x3d-runtime/src/mpeg1/audio.rs`. The demuxer concatenates the payloads of the first audio stream (stream id 0xC0 to 0xDF) of the
Systems stream into an MPEG audio elementary stream (an audio frame may run across packets); `symphonia` 0.6.1 (already a dependency;
its `mp2` feature is the only change, and the Layer II decoder lives in `symphonia-bundle-mp3`, which was already compiled for MP3 and
for `web-audio-api`) decodes it, from the first valid frame header to the last whole frame, at the stream's own rate and channel count.
`MovieMedia::decode_mpeg1` stores the result in `MovieMedia::audio` as the WebM Vorbis track is, so the existing path gives the movie
`hasSound` and plays the samples through the Sound node. The decode needs no new crate and no `unsafe`; `tools/check_no_aac.py` is
unaffected (`symphonia-codec-aac` is not enabled) and `tools/check_no_native_video.py --cargo` still passes (`mpeg1` was never part of
the `native-video` feature).

| Case | Result |
| --- | --- |
| no audio stream | `AudioTrack::None`: `hasSound` false, no diagnostic |
| Layer II or III | `Decoded`: `hasSound` true, the sound plays |
| Layer I | `Unsupported("MPEG-1 audio layer I")`: `hasSound` true as the page's would be, the pictures play, the log says why |
| audio stream that does not decode (no frame header, garbage, empty packets, over the byte budget) | `Corrupt`: the pictures play, the log says the track does not decode |
| a bad audio packet header | the packet is skipped (the sound loses it, the pictures do not) |
| a frame that does not decode | skipped, as the reference decoder does |
| a stream cut inside a frame | the frames before the cut play |
| a sample rate change inside the audio | the sound ends there |
| more than `MAX_DECODED_BYTES` of f32 samples | `Corrupt` |
| second audio stream | not decoded (the first audio stream in the file is the sound) |

Not done: the audio and video streams are not aligned by their time stamps (the picture schedule starts at 0 and so does the sound; in
the fixtures the streams start 25 ms or less apart); Layer I (no decoder enabled: symphonia's `mp1` feature is a one-line change if it is
wanted, but no fixture exists to test it, since ffmpeg has no Layer I encoder); MPEG-2 audio extensions.

Tests (`crates/x3d-runtime/tests/mpeg1_audio.rs`, fixtures and commands in `tests/data/mpeg1/PROVENANCE.txt`): four Systems streams
(Layer II mono 44.1 kHz, stereo 48 kHz, stereo 32 kHz noise, and Layer III mono 44.1 kHz) decode to the rate, channel count and length
(whole frames) of ffmpeg 6.1.1's float decoder (`mp2float`, `mp3float`) and within a per-sample bound of 2e-4, **fixed before the first
comparison** (the bound of the WebM Vorbis test). Measured worst differences: 4.8e-7, 5.3e-7, 4.1e-7 and 3.1e-7. This is a comparison of
two decoders of the same streams, not a conformance test: ISO/IEC 11172-3 is not in the local library.

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
for f in *.m1v *.mpg; do ffmpeg -v error -y -i $f -an -f rawvideo -pix_fmt yuv420p ${f%.*}.yuv; gzip -9n ${f%.*}.yuv; done   # the references
```

The references are stored gzip-compressed (`gzip -9n`, deterministic; binary, so review tooling does not diff them as text) and the
tests read them with `flate2` (already in the dependency tree through `x3d-parse`, MIT OR Apache-2.0; a dev-dependency of
`x3d-runtime` only). SHA-256 of each `.yuv.gz`:

```
331382d468b7aca56fcec85ea82b9ede6d45541b7fed5f05cce4784cc50dcd90  custom-matrix-64x48.yuv.gz
a0b4b6c0058e162e734541a23d1c17fde6a5d68106d01f4970af5661119e9f89  intra-64x48.yuv.gz
c3f3531a50a221e9cfdd058f4951b5d37564b71df65ae453091318c6fd443d05  ip-64x48.yuv.gz
a2be9f0e33019a81c3ebc1e5418b77e0313a07402332e47a6236a38b6bf88f3d  ipb-64x48.yuv.gz
473b7f325db3754660654bd3bde3e6376a623a0becca7d7d3f5093c70fca8fbb  odd-33x25.yuv.gz
3085bdb5c6152805619951eb976c1114db781dfaaf51041ec206ce450618f077  odd-50x38.yuv.gz
151b670fa30215e94c42a74fb54cd1a60a7ca0c18ffb6879b37ee16b7fc72459  red-32x32.yuv.gz
a2be9f0e33019a81c3ebc1e5418b77e0313a07402332e47a6236a38b6bf88f3d  sys-av-64x48.yuv.gz
```

(`ipb` and `sys-av` are the same pictures, hence the same hash.) The gzip bytes depend on the gzip version; the raw YUV is what
ffmpeg 6.1.1 writes and is what the tests compare.

(`testsrc2` rounds odd sizes to even, hence the `scale` filter for 33x25. The custom matrix in the fixture is the default intra matrix
written explicitly, so the path that reads a matrix from the header is exercised with known values.)

## Tests

`crates/x3d-runtime/tests/mpeg1_decode.rs` (decoder and demux, 21 tests; includes MPEG-4, WebM, Ogg and VRML text refused as not MPEG-1): seven stream comparisons with ffmpeg, red-picture colour,
format and extension detection, demux equals the Systems decode, video-only is not a Systems stream, MPEG-2 pack / PES header / video
refused, a Systems stream with no video packet, empty / tiny / garbage / zero-size / header-only inputs, truncation at many lengths
of four fixtures, 1600 mutated streams (no panic), the byte budget, a stream cut inside a picture keeps the pictures before it.

`crates/x3d-runtime/tests/mpeg1_movie_texture.rs` (node wiring, 15 tests; the fetcher records its requests): content identification (zero-size,
MPEG-4, MPEG-2 pack named `.mpg`; MPEG-1 under `.wrl .bin` / none / `.mpgx` / `.mpg.txt` not sniffed and not requested; query after the
extension), MFString fallback (first decodable url wins and later urls are not requested; a mix of garbage, missing, MPEG-2 and good
urls is tried in order and stops at the first good one; all failing gives Failed with each tried), and: the option is off by default; a Systems movie and each of
`.m1v .MPEG .mpe .mpg` load as frames of the right size; the compatibility option gives the pinned result (Failed, the usual
diagnostic, no frames); garbage, MPEG-2 and truncated data fail with the usual diagnostic and the reason; a bad file falls through
to the next url; a missing file fails and the scene keeps running.

Run: `CARGO_BUILD_JOBS=6 cargo nextest run -j 6 -p x3d-runtime mpeg1` and `cargo test --doc -p x3d-runtime`.

## Open items

- Clause numbers of ISO/IEC 11172-1/-2/-3: pending the owner supplying the standards.
- License file for pl_mpeg: accept the SPDX tag, or ask the author.
- Audio (above), the browser build path (above), extension-less urls, and a comparison with a second independent decoder (an in-repo
  decoder written from 11172-2 once the owner supplies Part 2, as `docs/mpeg1-decoder-admission.md` allows).
