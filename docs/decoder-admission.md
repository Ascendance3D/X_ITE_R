# Video decoder admission records (MovieTexture)

Rule BR1.7: a decoder dependency (or vendored decoder source) may be added without a further owner decision only if
its license is MIT, Apache-2.0 or BSD AND its patent status was actually checked and recorded below as
no-known-encumbrance. Unknown or encumbered status, or any other license, goes to the owner first.

Reference behavior: the pinned X_ITE 16.5.0 `MovieTexture.js` does no decoding of its own. It creates an HTML
`video` element (`loadVideo`), waits for `loadeddata`, and uploads the element as the texture (`setVideo`,
`set_time`). Which codecs play is whatever the browser decodes. REA Evidence IDs for that source:
`ev_5178960f6ea6a4471e1568dd919bcc2ddef2346d42118d4f4079ff9034d03660` (view) of
`ev_da7a25a06d05d9dbd248b15038401e8291f8cf301524f33603789351a399d145` (analysis of
`src/x_ite/Components/Texturing/MovieTexture.js` in `oracle/x_ite-pin`). The native comparison reference is therefore
headless Chromium decoding the same stream.

Checked on 2026-11-02. Patent checks were web searches of public sources (cited below); they are a good-faith
search, not legal advice.

## Current state

`vp9dec` 0.1.1 (MIT, no dependencies) decodes VP9. Nothing decodes VP8 or AV1 natively (ledger `node:MovieTexture`,
PARTIAL).

## Candidates

| Candidate | Codec | License (read from the crate) | Verdict |
| --- | --- | --- | --- |
| `zvidlib-vp8` 0.5.0 | VP8 | MIT; tables transcribed from RFC 6386 (BSD-3-Clause, notice shipped in `THIRD_PARTY_NOTICES.md`) | Not usable as a dependency: the crate is a slice of the `zvidlib` framework (its `lib.rs` imports `crate::{...}` framework types, has a build script for a Swift runtime path, depends on `zvidlib-core` and `zvidlib-color`). Its decoder modules (`bool_decoder`, `decoder`, `predict`, `loop_filter`, `tables`) are self-contained and may be vendored with their notices. Admission of the VP8 codec: see VP8 below. |
| `oxideav-vp8` 0.2.7 | VP8 | MIT | Not admitted. Its own description reads "orphan-rebuild scaffold pending clean-room re-implementation", which leaves the provenance of the current code unclear. 51k lines including an encoder, far more than a texture decoder needs. |
| `image-webp` 0.2.4 | VP8 key frames only (WebP lossy) | MIT OR Apache-2.0 | Not sufficient: no inter frames, so it cannot decode a video. Already in the tree. |
| in-repo VP8 decoder from RFC 6386 | VP8 | MIT (this repository); RFC tables BSD-3-Clause | The standards-compliant alternative. RFC 6386 is not in the local standards library and specs are not fetched from the web, so the tables come from the RFC-derived vendored source and are verified by bit-exact comparison with an independent decoder. |
| `rav1d` 1.1.0, `re_rav1d` 0.1.3 | AV1 | BSD-2-Clause (`COPYING`) | License passes. Patent status does not (see AV1). Not added. |
| `rav1d-safe` 0.6.0 | AV1 | AGPL-3.0-only OR commercial | License fails BR1.7. Not added. |
| in-repo AV1 decoder | AV1 | MIT | Does not change the patent question, which attaches to the technique, not the implementation. Not started. |

## VP8: status no-known-encumbrance

- Technique: Google holds and licenses the VP8 patents under the WebM Project's irrevocable, royalty-free patent
  license for the reference implementation and its derivatives.
- Third-party claims: MPEG LA announced a VP8 pool effort in 2011. In March 2013 Google and MPEG LA settled: Google
  took a license to the techniques held by eleven holders that might be essential to VP8 and earlier VPx codecs, may
  sublicense it to any VP8 user, and MPEG LA ended the pool effort
  ([TechCrunch](https://techcrunch.com/2013/03/07/google-and-mpeg-la-sign-licensing-agreement-covering-googles-vp8-video-codec-clearing-the-way-for-wider-adoption),
  [Engadget](https://www.engadget.com/2013-03-07-google-mpeg-la-settle-vp8-webm.html),
  [Business Wire](https://www.businesswire.com/news/home/20130307006192/en/Google-MPEG-LA-Announce-Agreement-Covering-VP8%29)).
- No VP8 pool or assertion against implementers turned up in the search after that date.
- Not established: whether the 2013 license moved unchanged when MPEG LA became Via Licensing Alliance in 2023. The
  sources found do not say.
- Result: no-known-encumbrance, with that one open point. License of the vendored code: MIT plus the BSD-3-Clause
  notice for the RFC tables, both permitted. VP8 meets BR1.7 and proceeds to Step 3.

## AV1: status encumbered or disputed. OWNER DECISION required

- Technique: royalty-free under the AOMedia Patent License 1.0, which covers patents of AOMedia members only.
- Third-party claims: Sisvel launched an AV1 patent pool in 2019 and expanded it in 2020. A 2026 summary reports 21
  holders and fees per device
  ([Streaming Media](https://www.streamingmedia.com/Articles/News/Online-Video-News/Sisvel-Announces-AV1-Patent-Pool-139636.aspx),
  [Lumenci](https://lumenci.com/blogs/hevc-vs-av1-vs-vvc-royalties-licensing-risk/)). Non-member holders are reported as
  suing platforms; those reports are secondary and unverified here.
- Result: this is not no-known-encumbrance. Under BR1.7 neither `rav1d` (license passes) nor an in-repo decoder is
  added until the owner decides. Plan Steps 5 and 6 are blocked on that decision only.
- Related, for the owner's awareness: Sisvel also runs a VP9 pool, and `vp9dec` is already a dependency. No admission
  record for it existed before this file. The owner may want to decide the VP9 position in the same ruling.
- Options for the owner: (a) accept the AOMedia grant and admit `rav1d` (BSD-2-Clause, Rust port of dav1d; the
  `asm` default feature needs `nasm` and should be disabled for the wasm build, which still has to be checked);
  (b) admit AV1 only for the native viewer; (c) leave AV1 to the browser build, as now.
