# MPEG-1 MovieTexture investigation (issue #90, plan Step 11, BR1.6)

Checked 2026-11-02.

## Applicable standard

ISO/IEC 19775-1 V4.0 (local copy in the workspace standards library, `iso-iec-19775-1-v4.0-x3d`, Part 1, Texturing
component):

- 18.4.2 MovieTexture: "The _url_ field that defines the movie data shall support MPEG1-Systems (audio and video) or
  MPEG1-Video (video-only) movie file formats as defined in ISO/IEC 11172-1."
- 18.2.2 Texture map image formats: "Texture nodes that support MPEG files (see ISO/IEC 11172-1) shall treat MPEG files
  as full RGB textures."
- Table 18.11, support level 4: MovieTexture, all fields fully supported.

The Systems part (ISO/IEC 11172-1) names the container; the pictures are ISO/IEC 11172-2 and the audio ISO/IEC 11172-3.
Those three parts are not in the local standards library (the library holds the X3D and VRML97 texts), so no decoder
table or algorithm is quoted here.

## Observable reference

Pinned X_ITE 16.5.0 does no decoding: `MovieTexture.js` hands the file to an HTML `video` element (REA Evidence IDs
`ev_5178960f6ea6a4471e1568dd919bcc2ddef2346d42118d4f4079ff9034d03660`, view of
`ev_da7a25a06d05d9dbd248b15038401e8291f8cf301524f33603789351a399d145`). Measured in the Chrome the oracle uses
(156.0.8078.47, headless): `canPlayType("video/mpeg")` is `""`, and a one-second MPEG-1 Systems stream made with
ffmpeg's `mpeg1video` encoder fails to load with `MEDIA_ERR_SRC_NOT_SUPPORTED` (code 4). The same Chrome reports
"probably" for VP8 and AV1 in WebM. So the pinned X_ITE in this browser does not play MPEG-1: the node warns "Error
loading movie" and enters FAILED_STATE. X_ITE_R matches that today (`.mpg` is not in the playable list of
`loader.rs`, so the url is treated as a video the engine cannot show and the next candidate is tried).

## Patent status

Preliminary screening result, not a freedom-to-operate opinion: no unexpired MPEG-1 Video or Layer I/II patent was found, based primarily on a secondary source; jurisdiction- and component-specific review is outside this investigation. MPEG-1 Video and Layer I/II are treated apart from MP3 (Layer III), whose expiry date and portfolio differ. The Wikipedia MPEG-1 article states that the essential patents declared to ISO for
MPEG-1 Video (BBC, IBM, Sony, Philips and others) "have since expired", that US 4,472,747 (listed for ISO 11172)
expired in 2003, that US 5,214,678 (possibly Layer II) is expired, that no unexpired MPEG-1 Video or Layer I/II patent
could be listed, and that every patent worldwide on MP3 (Layer III) expired on 30 December 2017
([MPEG-1](https://en.wikipedia.org/wiki/MPEG-1)). This is a secondary source; no pool notice (Via LA) was found to
contradict it. A web search for a primary MPEG-1 expiry statement found none (checked 2026-11-02, the date of this investigation; the sources were read the same day).

## Decoder options

| Option | License | Verdict |
| --- | --- | --- |
| `oxideav-mpeg12video` 0.0.13 (pure Rust, MPEG-1 and MPEG-2 video) | MIT | License passes. Same publisher family as `oxideav-vp8`, whose own description says its history was rebuilt (see `docs/decoder-admission.md`); provenance not examined for this crate. Video only: no Systems demux, no audio. |
| `southstar-codecs` 1.0.5 (port of pl_mpeg for MPEG-1 video and MP2) | `(LicenseRef-NSL-1.0 OR GPL-3.0-or-later) AND MIT` | Rejected. In SPDX, `AND` binds both sides: the MIT part applies together with one choice of `(LicenseRef-NSL-1.0 OR GPL-3.0-or-later)`, so MIT alone is not an available option for the whole crate; the choices are a non-standard license (not read) or GPL-3.0-or-later (copyleft, not compatible with this MIT project's distribution as a library). The crate's license files were not read, so this verdict rests on the metadata expression. |
| pl_mpeg (C, single file: Systems demux, MPEG-1 video, MP2 audio) | MIT | License passes; would be a port into Rust (the `southstar-codecs` port above shows it is possible). Carries the tables that the local library does not. |
| libmpeg2, FFmpeg | GPL, LGPL | Rejected (BR1.7). |
| In-repo decoder from ISO/IEC 11172-1, -2 and -3 | MIT | Needs the three standards, which the local library lacks. |

## Decision

Not implemented in this step. The constraint is not a patent or a license: it is a conflict between two goals, and the
owner decides it.

OWNER DECISION: either
(a) keep parity with the pinned X_ITE, which cannot play MPEG-1 in Chrome (current behavior). This is a known
    non-conformance with the clause ("shall support" MPEG-1). A ledger DIFFERENT would only record it; it does not make X_ITE_R
    conformant and takes effect only if the owner authorizes it in writing. Matching a browser's limitation is not evidence that
    the clause is optional, and a future browser or X_ITE may change the reference behavior. Or
(b) exceed the pinned X_ITE and implement the clause: an MPEG-1 Systems demuxer, an ISO/IEC 11172-2 video decoder and
    (for BR1.2) an ISO/IEC 11172-3 Layer I/II audio decoder, as a port of pl_mpeg (MIT) or from the standards once the
    owner supplies ISO/IEC 11172. The preliminary screening above found no patent obstacle and pl_mpeg is MIT, so BR1.7 does not appear to block it (subject to the legal review; this is not a legal conclusion);
    there is no pinned reference to compare frames with, so BR1.1's comparison would use an independent decoder's
    output (ffmpeg) as for VP8.

Recommendation: (a) until a corpus case needs MPEG-1; (b) as a separate plan step if the owner wants the clause met.

## Reproducing the browser observation and pinning the sources

* Pinned X_ITE: version 16.5.0, source at `oracle/x_ite-pin/` in this repository (reference digest pinned by `tools/candidate.lock`; short commit `34cdf9f97`), file `src/x_ite/Components/Texturing/MovieTexture.js`, functions `loadVideo` (creates the `video` element, waits for `loadeddata`, rejects on `error`/`abort`) and `setVideo`. X_ITE_R: `crates/x3d-runtime/src/loader.rs`, the branch that builds the `playable` extension list (`.webm .ogv .ogg .mp4 .m4v`; `.mpg` is not in it) at the repository head of this PR.
* Recipe (any machine with the Chrome version above and ffmpeg): `ffmpeg -f lavfi -i testsrc=duration=1:size=64x48:rate=10 -c:v mpeg1video -f mpeg -y t.mpg` (video-only MPEG-1 program stream, `-f mpeg`; the ffmpeg version used is the one on the workstation, 6.1.1 from the distribution package), serve it as `video/mpeg` over HTTP, create `document.createElement("video")`, set `src`, and read `canPlayType("video/mpeg")`, the `error` event and `video.error.code`. Observed: `""`, an `error` event, code 4 (`MEDIA_ERR_SRC_NOT_SUPPORTED`). No browser flags were changed. The fixture itself was not retained; regenerate it with the command above (ffmpeg's `testsrc` is deterministic). A file extension or MIME type does not by itself identify a valid MPEG-1 Systems stream; the stream above is one by construction.
* Evidence IDs (`ev_...`) are identifiers in the REA evidence store of the maintainer's workstation; they are not retrievable from this repository. They are recorded so the maintainer can reopen the same capture; every claim above also stands on the file and function names given here.
* Standard references: ISO/IEC 19775-1 V4.0, clause 18.4.2 (MovieTexture; the sentence "shall support MPEG1-Systems ... or MPEG1-Video ..."), clause 18.2.2 (texture map image formats), Table 18.11 (support levels; MovieTexture at level 4 with "All fields fully supported"); profile listings in Part 1 `immersive.md` and `fullProfile.md` (local standards library copy).
