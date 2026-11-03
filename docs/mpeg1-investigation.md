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

Checked, no-known-encumbrance. The Wikipedia MPEG-1 article states that the essential patents declared to ISO for
MPEG-1 Video (BBC, IBM, Sony, Philips and others) "have since expired", that US 4,472,747 (listed for ISO 11172)
expired in 2003, that US 5,214,678 (possibly Layer II) is expired, that no unexpired MPEG-1 Video or Layer I/II patent
could be listed, and that every patent worldwide on MP3 (Layer III) expired on 30 December 2017
([MPEG-1](https://en.wikipedia.org/wiki/MPEG-1)). This is a secondary source; no pool notice (Via LA) was found to
contradict it. A web search for a primary MPEG-1 expiry statement found none.

## Decoder options

| Option | License | Verdict |
| --- | --- | --- |
| `oxideav-mpeg12video` 0.0.13 (pure Rust, MPEG-1 and MPEG-2 video) | MIT | License passes. Same publisher family as `oxideav-vp8`, whose own description says its history was rebuilt (see `docs/decoder-admission.md`); provenance not examined for this crate. Video only: no Systems demux, no audio. |
| `southstar-codecs` 1.0.5 (port of pl_mpeg for MPEG-1 video and MP2) | `(LicenseRef-NSL-1.0 OR GPL-3.0-or-later) AND MIT` | Rejected: copyleft or non-standard license. |
| pl_mpeg (C, single file: Systems demux, MPEG-1 video, MP2 audio) | MIT | License passes; would be a port into Rust (the `southstar-codecs` port above shows it is possible). Carries the tables that the local library does not. |
| libmpeg2, FFmpeg | GPL, LGPL | Rejected (BR1.7). |
| In-repo decoder from ISO/IEC 11172-1, -2 and -3 | MIT | Needs the three standards, which the local library lacks. |

## Decision

Not implemented in this step. The constraint is not a patent or a license: it is a conflict between two goals, and the
owner decides it.

OWNER DECISION: either
(a) keep parity with the pinned X_ITE, which cannot play MPEG-1 in Chrome (current behavior; the ledger would carry the
    difference from the clause as an approved DIFFERENT, because the clause says MPEG-1 "shall" be supported), or
(b) exceed the pinned X_ITE and implement the clause: an MPEG-1 Systems demuxer, an ISO/IEC 11172-2 video decoder and
    (for BR1.2) an ISO/IEC 11172-3 Layer I/II audio decoder, as a port of pl_mpeg (MIT) or from the standards once the
    owner supplies ISO/IEC 11172. Patents are expired and the licenses are permissive, so BR1.7 would not block it;
    there is no pinned reference to compare frames with, so BR1.1's comparison would use an independent decoder's
    output (ffmpeg) as for VP8.

Recommendation: (a) until a corpus case needs MPEG-1; (b) as a separate plan step if the owner wants the clause met.
