# Video decoder admission records (MovieTexture)

Rule BR1.7: a decoder dependency (or vendored decoder source) may be added without a further owner decision only if
its license is MIT, Apache-2.0 or BSD AND its patent status was actually checked and recorded below as
no-known-encumbrance. Unknown or encumbered status, or any other license, goes to the owner first.

Owner rulings of 2026-11-02 (below, "Owner ruling") authorize development, internal testing and guarded development merges of VP8, VP9
and AV1 decoders. They do not authorize a public release of binaries that contain them, and they do not say that any patent right
is cleared.

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
| `zvidlib-vp8` 0.5.0 | VP8 | MIT; tables transcribed from RFC 6386 (BSD-3-Clause, notice shipped in `THIRD_PARTY_NOTICES.md`) | Not usable as a dependency: the crate is a slice of the `zvidlib` framework (its `lib.rs` imports `crate::{...}` framework types, has a build script for a Swift runtime path, depends on `zvidlib-core` and `zvidlib-color`). Its decoder modules (`bool_decoder`, `decoder`, `predict`, `loop_filter`, `tables`) are self-contained and may be vendored with their notices. Used (vendored) for VP8 under the owner ruling; patent position: see the VP8 sections below. |
| `oxideav-vp8` 0.2.7 | VP8 | MIT | Not admitted. Its own description reads "orphan-rebuild scaffold pending clean-room re-implementation", which leaves the provenance of the current code unclear. 51k lines including an encoder, far more than a texture decoder needs. |
| `image-webp` 0.2.4 | VP8 key frames only (WebP lossy) | MIT OR Apache-2.0 | Not sufficient: no inter frames, so it cannot decode a video. Already in the tree. |
| in-repo VP8 decoder from RFC 6386 | VP8 | MIT (this repository); RFC tables BSD-3-Clause | The standards-compliant alternative. RFC 6386 is not in the local standards library and specs are not fetched from the web, so the tables come from the RFC-derived vendored source and are verified by bit-exact comparison with an independent decoder. |
| `rav1d` 1.1.0, `re_rav1d` 0.1.3 | AV1 | BSD-2-Clause (`COPYING`) | License passes. Selected for AV1 under the owner ruling; patent position and the Sisvel assertion: see AV1 below. |
| `rav1d-safe` 0.6.0 | AV1 | AGPL-3.0-only OR commercial | License fails BR1.7. Not used. |
| in-repo AV1 decoder | AV1 | MIT | Does not change the patent question, which attaches to the technique, not the implementation. Not pursued. |

## Owner ruling, 2026-11-02

* VP8 (PRs #353, #355), VP9 (`vp9dec`, kept and maintained) and AV1 (`rav1d`): approved for implementation, safety and parity
  testing, and merge of verified development PRs under the exact-head gates.
* No public release of binaries containing any of the three on the assertion that patent rights are cleared. A codec licensing
  inventory (`docs/codec-licensing-inventory.md`) and a separate release authorization after legal review come first.
* No patent agreement is signed, and no license bought, without the owner's explicit permission.
* Third-party assertions are recorded here as unresolved release risks, separate from the grants.

## The licenses involved (primary sources, read 2026-11-02)

These are the WebM Project and AOMedia texts as published. They are quoted from the pages; reading them is not legal advice.

### Google's WebM grants: two different documents

1. **Additional IP Rights Grant** (<https://www.webmproject.org/license/additional/>): a perpetual, worldwide, royalty-free,
   irrevocable (except on patent litigation) license to patent claims Google owns that are "necessarily infringed by these
   implementations of WebM", where "these implementations" are the copyrightable works Google distributes as part of the WebM
   Project. Claims "infringed only as a consequence of further modification" are excluded. This one is implementation-specific:
   it follows Google's distributed code and unmodified use, and does not by its text reach a decoder written or adapted
   elsewhere. The in-repo VP8 decoder is adapted from `zvidlib-vp8` (not libvpx), and `vp9dec` is not libvpx either. Neither is
   represented here as covered by this grant.
2. **Bitstream License** (<https://www.webmproject.org/license/bitstream/>): a perpetual, worldwide, royalty-free, irrevocable
   patent license to "make, have made, use, offer to sell, sell, import, and otherwise transfer implementations" of the "WebM
   Specifications", limited to patents "necessarily infringed by implementation of the WebM Specifications" that Google owns or
   later acquires. The Specifications are "the specifications to the WebM codecs as embodied in the source code to the WebM
   codecs", with any written description, as distributed by Google. The page does not mention "independent" implementations.
   Its text appears to cover an implementation of the specification wherever Google's claims are necessarily infringed; that is
   an inference from the text and not a statement on the page. Both documents end the licensee's rights if the licensee starts
   patent litigation claiming that an implementation infringes.

Google's grants cover Google's patents only. They say nothing about other holders.

### VP8 Patent Cross-License Agreement (<https://www.webmproject.org/cross-license/>)

The page is a summary. It says Google, Matroska and Xiph.Org make WebM components available royalty-free, refers to "a number of
IP owners" on a separate list, and "encourages" signing up for the VP8 Patent Cross-License Agreement. Participants "likewise
agree to license patents they may have that are essential to VP8 to other participants in the cross-license." The page gives no
licensor, field of use, grant-back scope, sublicensing, termination or date; the agreement text itself was not available in
this read. From secondary sources only (LWN, OSI commentary on the 2013 draft; not verified against the final text): the
agreement is a patent cross-license separate from the software license, covers VP8 patents of MPEG LA's licensors, is limited to
encoding, decoding, transcoding or playing VP8, requires the licensee to grant back its own VP8-related patents on a
royalty-free basis, ends for a licensee that sues, and was joined by signing or accepting a web form.

**Is action needed for a release?** Joining is voluntary on the page's wording, and joining means a grant-back of the project's
and its contributors' own VP8-essential patents. This project is not known to hold any. Whether a release can rely on the
2013 Google and MPEG LA settlement without joining is not answered by the sources read. Not signed; the owner decides, after
legal review, whether to join. Until then VP8 stays a development and internal-test feature.

### VP9

`vp9dec` 0.1.1 (crates.io, MIT, no dependencies; checksum in `Cargo.lock`) was in the tree before this file. The same two
distinctions apply: the implementation-specific grant covers Google's libvpx and its unmodified use, and the bitstream license
is the broader one. Recorded separately, as a third-party assertion and not a grant: Sisvel operates a VP9 patent pool
(secondary reports only). The VP8 cross-license does not cover VP9. VP9 is kept and maintained by owner ruling; the assertion
stays an unresolved release risk.

### Exact sources and checksums (read 2026-11-02 from the local cargo cache of the crates.io downloads)

| Crate | sha256 of the `.crate` file | Upstream (from the crate's metadata) |
| --- | --- | --- |
| `zvidlib-vp8` 0.5.0 | `ed40a5ba17bf2b7db9b9ae08a9c5669753332be34f3b346439a1c1188aa3834a` | `https://github.com/lsegal/zvidlib`, path `crates/zvidlib-vp8`, commit `bcb92a6ab080d58fc34bafbf05455b2a8cfc1010` (`.cargo_vcs_info.json`); MIT, copyright 2026 Loren Segal |
| `vp9dec` 0.1.1 | `85dc9eb9e26fa02ce81898998c5e1ccc718f14bfc485c7a4518f464bba57d338` | `https://github.com/ChanyaVRC/vp9dec`; MIT |
| `rav1d` 1.1.0 | `1932f060d5e7bd49dc9f8b272c1dc5e9ce0ffe141c28be900265d3989b36c9ed` | BSD-2-Clause (`COPYING`); not yet in the tree |
| `southstar-codecs` 1.0.5 | `a4db934e3f06ab7add39a68ff975656daa3ea681575ec3681a71393264233a31` | rejected (see the MPEG-1 record) |
| `oxideav-vp8` 0.2.7 | `506f2495a728488c2fee1ee54bc7f819ca934dc3246369b4100c83848f057852` | rejected, provenance unclear |

### Provenance of the VP8 tables

The vendored VP8 decoder in PR #353 is adapted from `zvidlib-vp8` 0.5.0 (above). That crate's own `THIRD_PARTY_NOTICES.md`
states that its coefficient, motion-vector, key-frame sub-block mode and quantizer tables are transcribed from the reference
decoder published in RFC 6386 section 20 (`vp8_prob_data.h`, `modemv_data.h`, `dequant_data.h`), and carries the RFC's
BSD-3-Clause notice (Copyright 2010, 2011 Google Inc.). The crate's test fixtures come from the libvpx project. This is the
upstream author's statement; this repository did not compare the tables with the RFC text (RFC 6386 is not in the local
standards library and specs are not fetched from the web). What this repository did verify is behavior: the decoder is
bit-exact with an independent decoder (ffmpeg) on six synthetic streams, and the decoded planes match the reference hashes in
`oracle/synthetic-movie-codecs/media/*.planes.tsv`. **Open item:** PR #353 must carry the BSD-3-Clause notice itself as a file
in the repository (its `tables.rs` header points to a `THIRD_PARTY_NOTICES.md`); that file is not in the branch yet and is a
precondition of merging #353.

### VP8 summary

* Google grants: as above, two documents, one implementation-specific and one for implementations of the specification.
* Third-party: MPEG LA's 2011 pool effort ended with the March 2013 settlement, reported by press releases only; no VP8 pool or
  assertion against implementers turned up afterwards (good-faith web search, not an exhaustive patent search). Whether the 2013
  arrangement moved unchanged when MPEG LA became Via LA in 2023 is not established.
* Source licenses of the vendored decoder: MIT, plus the BSD-3-Clause notice for tables transcribed from RFC 6386.
* Disposition: development and internal testing; the Cross-License and release questions go to legal review.

### AV1: AOMedia Patent License 1.0 (<https://aomedia.org/license/patent-license/>)

* Grant: from each Licensor, a no-charge, royalty-free, irrevocable (except as stated) license under "Necessary Claims" (essential
  under the W3C policy, or infringed by the Reference Implementation) to "make, use, sell, offer for sale, import or distribute any
  Implementation", an Encoder and/or Decoder including the Reference Implementation. It is not limited to Google's code.
* Who grants: Licensors include any Licensee that makes, sells, offers, imports or distributes an Implementation, and parties bound
  through the working group that developed the Specification (members). A holder that is neither is not bound by this license.
* Availability to us: the grant runs to any Licensee, "any person or entity who exercises patent rights granted under this
  License"; no membership is required.
* Reciprocity: to benefit, the Licensee "must make its Necessary Claims available under this License".
* Defensive termination (1.3): if a Licensee starts or joins patent litigation claiming an Implementation infringes Necessary
  Claims, the licenses granted to it directly end immediately, with exceptions for suits that answer a prior suit or enforce the
  license.
* Unresolved release risk, recorded separately: Sisvel runs an AV1 patent pool (since 2019; a 2026 summary reports 21 holders and
  per-device fees) and non-member holders are reported to have sued platforms. These are secondary reports
  ([Streaming Media](https://www.streamingmedia.com/Articles/News/Online-Video-News/Sisvel-Announces-AV1-Patent-Pool-139636.aspx),
  [Lumenci](https://lumenci.com/blogs/hevc-vs-av1-vs-vvc-royalties-licensing-risk/)). The AOMedia license does not bind those
  holders, so an open-source license plus this grant does not give complete patent clearance.
* Implementation: `rav1d` (BSD-2-Clause; its `COPYING` is the two-clause BSD text with the copyright lines of VideoLAN and the
  dav1d authors and of the Internet Security Research Group, and no patent notice). Preserve its license text and copyright
  lines wherever the code is vendored or distributed. `re_rav1d` is the same lineage. `rav1d-safe` is AGPL-3.0-only or
  commercial and is not used. The `asm` default feature needs `nasm` and is off for the wasm build.
* Disposition: development and internal testing.
