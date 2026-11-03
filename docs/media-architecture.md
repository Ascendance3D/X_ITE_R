# Media backend architecture review (issue #90)

Owner request 2026-11-02: before expanding independent Rust codec implementations, compare reuse of mature browser and operating-system decoders, and recommend an architecture that minimizes duplicated codecs while keeping verified X_ITE_R behavior. Targets: browser/WASM, Linux, macOS 15+. Windows and mobile are out of scope. Existing passing decoder work, tests and evidence stay.

Status: review and recommendation, 2026-11-02. No code changes. Facts are marked **measured** (done on this machine, with the evidence stated), **read** (from the pinned source, the ISO text or the repository), or **unverified** (from general knowledge; needs the stated experiment before anyone relies on it). Nothing here is legal advice; patent and license items point to `docs/codec-licensing-inventory.md` and `docs/decoder-admission.md`.

## 1. What has to be matched

**Pinned X_ITE 16.5.0 (read, `MovieTexture.js`).** It has no decoder. `loadVideo` makes a `<video>` element (`crossOrigin = "anonymous"`, `preload = "auto"`, `playsInline`), waits for `loadeddata`, routes the element's sound into the page's `AudioContext` with `createMediaElementSource` (so a `Sound` node spatializes it), and uploads the element as the texture on every `set_time` (`updateTextureData(media.currentFrame)`). Speed and pitch set `media.playbackRate = speed * max(pitch, 0)`; Chrome throws on a negative rate, which X_ITE catches, logs, and replaces with 1. `preservesPitch = (pitch === 1)`. Start, stop, pause, loop and elapsed time come from the `X3DSoundSourceNode` time machinery (`set_time` calls its base `set_time` first); the exact split between that and the element's own state was not traced for this review. Which codecs play is whatever the browser decodes. Measured by Agent A in the oracle Chrome (156.0.8078.47): VP8 and AV1 "probably" playable, MPEG-1 not playable.

**ISO/IEC 19775-1 V4.0, 18.4.2 MovieTexture (read, local standards library).**
* `url` "shall support MPEG1-Systems (audio and video) or MPEG1-Video (video-only)"; 18.2.2 treats MPEG as full RGB textures.
* Display time is defined by formula: for positive speed an active node shows movie time `t = (now - startTime) modulo (duration / speed)`. `speed = 0` shows frame 0; a negative `speed` plays backwards; `set_speed` is ignored while active; `duration_changed` is not affected by `speed`; frame 0 (or the last frame for a negative speed) shows while inactive; `pitch` scales rate and pitch together.

So the pinned reference and the ISO text differ in two places that matter for a backend choice: the pinned page cannot play backwards (negative speed becomes 1 with a logged error) and it follows the browser's clock, not the formula. X_ITE_R's rule is parity with the pinned page (the verified behavior), with the ISO behavior recorded where it differs. A backend that lets the engine choose the displayed frame by time can do either; a backend that only plays can do the pinned behavior only.

## 2. What X_ITE_R already does (read, and measured on the built wasm)

* `loader.rs`: when the host says it plays video (`fetcher.plays_video()`, the browser page), `MovieTexture` is handed to the page: `crates/x3d-web/web/video_mirror.mjs` creates the `<video>` element as X_ITE does, plays it, and sends each new picture to the engine (`drawImage` into an `OffscreenCanvas`, then `getImageData`, a CPU copy per frame). The engine decodes no video there. This is approach 1 below and it is the pinned behavior by construction.
* Otherwise (native viewer, tests), the file is decoded in Rust: `webm.rs` demuxes WebM and decodes VP9 (`vp9dec`) and, in PR #353, VP8. AV1 is planned (plan Steps 5 and 6). MP4 and Ogg are "assumed playable" (no frames).
* Measured: the shipped wasm module still contains `vp9dec` (86 matching strings in `x3d_web.wasm`) although the browser path never calls it. It is dead code in the browser build. AVIF stills (`avif-rust`, AV1) are also decoded in the engine in every default build (844 strings), as `docs/codec-licensing-inventory.md` records.
* Audio: `AudioClip`/`Sound` use the Rust `web-audio-api` engine natively and the page's own `AudioContext` through `audio_mirror.mjs` in the browser.

## 3. Comparison

Legend: ok = meets the need; partial = with limits stated; no = cannot; ? = unverified.

| | 1. Browser: HTMLMediaElement, Web Audio, WebCodecs | 2. Linux GStreamer | 3. macOS AVFoundation | 4. Rust decoders (vp9dec, VP8 in tree, rav1d, symphonia) | 5. FFmpeg |
| --- | --- | --- | --- | --- | --- |
| Available where | Browser/WASM only. Not callable from native Rust; a native viewer has no browser. | Linux native. Runtime 1.24.2 is installed here (measured); the development package is not (`pkg-config gstreamer-1.0` fails), so a Rust binding cannot build here yet. macOS has no system GStreamer. | macOS native only. | Everywhere, including wasm. | Everywhere if linked; a system copy exists here (6.1.1, measured). |
| Pinned X_ITE parity | Identical by construction for the element (it is the reference). WebCodecs differs from the element in timing and needs our demux. | Not the reference; pictures must be compared against the pinned page. | Not the reference; same. | Compared against the pinned page per codec: VP8 first picture 0 pixels over 8 and max delta 1 to 2 on six streams (PR #355, tolerance fixed before the run); VP9 earlier. | Not the reference. |
| ISO X3D (formula time, reverse, speed 0) | Element: forward only, playbackRate clamps (Chrome rejects negative), time follows the media clock. WebCodecs: yes, we choose frames by time and can seek backwards (we manage the GOP); `VideoDecoder` needs each frame's data. | Seek with segment events and negative rates exist; decoder support for reverse varies (?). Frame by time via appsink and seeks (?). | `AVAssetReader` gives sequential frames; random access means seeking to a sync sample and decoding forward (?). `AVPlayer` rate negative needs support in the asset (?). | Yes: we index keyframes and decode forward; reverse needs a GOP cache we write. Full control. | Yes via libav seeking; we write the logic. |
| Codecs | Whatever the browser ships: VP8, VP9, AV1, H.264/AAC (browser licensed) and more; MPEG-1 no (measured). | Installed plugins: vp8dec, vp9dec, av1dec, avdec_vp9, avdec_h264, nvh264dec, vulkanh264dec, matroskademux, qtdemux (measured); `avdec_mpeg1video` absent here (measured). Depends on the user's packages. | H.264, HEVC, ProRes, AAC, MP3 in MP4/MOV; VP9 via VideoToolbox on macOS 11+ and AV1 hardware only on newer chips; WebM container support (?). Needs a Mac to verify. | WebM VP8/VP9 (and AV1 once admitted); Vorbis, MP3, FLAC, PCM audio. No MP4 demux, no H.264/AAC (AAC excluded by decision), no Opus. | Everything, MPEG-1 included. |
| A/V sync | Element plays audio and video on one clock (the pinned behavior). WebCodecs: we sync ourselves. | Pipeline clock syncs a pipeline; we would pull audio out of it (appsink) into our audio engine, which breaks that sync unless we slave video to our audio clock. | `AVPlayer` syncs its own audio; routing it through our engine is the same problem. | We sync: audio clock from the engine, video picked by media time. Possible, not yet written. | Same as Rust. |
| Seek, loop, speed | Element: seek yes (accurate in Chrome by decoding forward), loop per the X3D fields, rate 0.0625 to 16 in Chrome (?). | Yes (flush seek, rate changes). | Yes for `AVPlayer`; accurate seek `toleranceBefore/After = zero`. | Seek/loop/speed written by us; Agent A's Steps 7 and 8. | Same as Rust. |
| Frame timing and texture update | Element: CPU readback today (`getImageData`); WebGPU/WebGL can import `VideoFrame`/`HTMLVideoElement` directly with `copyExternalImageToTexture` or `importExternalTexture`, no CPU copy (?). | appsink gives system-memory or GL/Vulkan/DMA-BUF frames (?). | `AVPlayerItemVideoOutput` gives `CVPixelBuffer`/IOSurface that can back a Metal texture without a copy (?). | CPU YUV to RGBA today, then upload. GPU upload of the YUV planes with a shader conversion is possible (our rule: per-frame graphics work on the GPU). | CPU frames; hardware via VAAPI/VideoToolbox/NVDEC if built that way. |
| Pixel/sample accuracy | Browser picture = the reference. | YUV equal to the reference on VP8 (measured below); RGB depends on conversion matrix and range. | Decoders are conformant, but colour management and conversion are platform choices (?). | YUV exact for conformant decoders; RGB conversion is ours and was matched to Chrome (VP8 BT.601, VP9 BT.709). | YUV exact; conversion ours or libswscale. |
| GPU / hardware decode | Yes, transparent. | Yes with the right plugins (VA-API, NVDEC, Vulkan; present here). | Yes (VideoToolbox), transparent. | No (CPU); rav1d has asm, vp9dec is scalar. | Optional. |
| Size and maintenance | Zero code in the page; bigger dependence on the browser version. | Large system dependency (GStreamer plus plugins), per distribution; Rust binding `gstreamer-rs` (MIT/Apache, needs the dev packages). | System framework; Rust binding via `objc2` crates (MIT/Apache) (?). macOS-only code path to test on hardware we only have virtualized. | We maintain the decoders; small, no system dependency; needs ongoing safety work. | Very large; build complexity. |
| License / distribution | The browser's licenses, not ours. | LGPL core and plugins; "ugly"/"bad"/libav plugins carry their own licenses and patents; dynamic use of the user's installed plugins means we ship none. | Apple's. We ship no codec. | MIT/BSD-2 sources (vp9dec MIT, rav1d BSD-2, VP8 MIT+BSD-3); patent questions in `docs/decoder-admission.md`. | LGPL or GPL by configuration; includes AAC/H.264/HEVC, which conflicts with the AAC exclusion unless disabled; static linking brings LGPL duties. |

### Measured: system decoders and "bit-identical"

The YUV planes of a conformant VP8/VP9/AV1 decoder are exact by the specification; what differs between platforms is the YUV-to-RGB step (matrix, range, chroma upsampling) and any scaling or colour management. I checked the first half of that on this machine: GStreamer 1.24.2 `matroskademux ! vp8dec` (libvpx) on Agent A's synthetic fixtures, compared plane by plane with the SHA-256 reference hashes made with ffmpeg:

| Stream | Frames | Planes equal to the reference |
| --- | --- | --- |
| `vp8-testsrc` (96x64) | 20 | 20 of 20 |
| `vp8-parts` (128x80) | 15 | 15 of 15 |
| `vp8-crop` (50x34) | 10 | not compared: GStreamer padded the rows (2720 bytes per frame against 2550), so the hashes of the unpadded planes were not checked |

So a second, independent decoder agrees with the in-tree one on the normative output. For audio the situation is the opposite: AAC, MP3 and Vorbis decoders are not bit-exact with each other (floating point; #347 needed a Chromium-matched resampler to land within 5e-5), so sample accuracy across platforms is a tolerance, never equality.

## 4. Findings

1. For the **browser build the question is already answered**: use the page's media element (the reference) and put no codec in the wasm module. The `vp9dec` in `x3d_web.wasm` is dead weight and, for release, a patent-inventory item that does nothing. (AVIF stills are a separate case: the engine decodes them in every default build.)
2. For **native**, no system decoder is a drop-in for the reference. GStreamer and AVFoundation decode well, hardware accelerate, and do not require us to ship a codec, but they are unavailable on the other platform, differ in colour conversion, and cannot be driven by the engine's clock without effort.
3. The **Rust decoders remain valuable for a different reason than coverage**: they are the only backend that is the same on every native platform and in CI, so the pinned comparisons stay reproducible. That is why existing VP9/VP8 work and tests stay.
4. The **hard-to-replace codecs are the licensed ones**: H.264/HEVC/MP4 and AAC. Writing or bundling those is a patent and size problem we should not take on; delegating them to the OS (AVFoundation on macOS, GStreamer on Linux, the browser in WASM) puts the codec license with the OS vendor or the user's distribution, and keeps our binaries free of them. This agrees with the AAC exclusion (PR #357).
5. **WebCodecs** is the one browser API that can beat the element for X3D semantics (frame chosen by time, reverse play, no CPU readback), but it needs a demuxer in front of it (we have a WebM one, no MP4), and it would change timing away from the pinned page. It should be an opt-in experiment, not a replacement, until a pinned comparison says otherwise.
6. **FFmpeg**: use it where it already is, as the fixture generator and independent oracle in tests. Do not ship or link it: LGPL/GPL, size, and it bundles exactly the codecs the project excluded.

## 5. Recommended architecture

One interface, several backends, and exactly one implementation per codec per platform.

```
MediaSource (engine side: load, play/pause/seek/rate/loop, picture(t), audio)
 ├─ HostElementBackend   browser/WASM: HTMLMediaElement + Web Audio (exists: video_mirror.mjs). The reference.
 │    └─ optional later: WebCodecs/GPU-import path behind a flag, compared with the element before use.
 ├─ RustBackend          native + CI: webm.rs demux, vp9dec, VP8 (in tree), rav1d (planned), symphonia audio.
 │                       Deterministic, license-clean, the backend the pinned comparisons verify.
 └─ SystemBackend        native, cargo feature off by default, opt-in at run time:
      ├─ macOS: AVFoundation (H.264/HEVC/AAC/MP4 and anything else the OS plays)
      └─ Linux: GStreamer via the user's installed plugins
```

Rules that follow:
* **No new Rust codec is written for something an OS or the browser decodes and we may not ship.** That means no H.264, HEVC, AAC or MP4 decoder in Rust. MPEG-1 stays an approved DIFFERENT (the oracle Chrome cannot play it).
* **Rust backend scope is the WebM family** (VP8, VP9, AV1) and the royalty-free audio already present (Vorbis, FLAC, PCM family). It is also the fallback when a SystemBackend is absent or fails.
* **The wasm module does not carry the native video decoders.** Gate `webm.rs`, `vp9dec`, `vp8` and later `rav1d` out of the wasm build with a cargo feature (`native-video`, default on for native, off for wasm). Expected effect: smaller module, fewer inventory rows in the browser release, no change to the browser behavior because that path never calls them. The AVIF decision is separate.
* **SystemBackend results are labeled and tested differently.** Never claimed bit-identical to the Rust backend or across platforms; the test is tolerance-based against the pinned page (YUV exact where the codec is normative, RGB within the fixed per-test tolerance), and macOS results from the hosted runner are labeled virtualized (not a physical Mac).
* **One clock.** In native backends the engine's audio clock is the master and the video backend is asked for the picture at media time `t`. That is what makes the ISO formula, `speed`, `loop` and sync testable with the existing timing tolerance (one frame period). The element backend keeps the pinned behavior (the browser clock).
* **Color conversion lives in one place** (the engine's YUV-to-RGBA or a GPU shader), with the matrix chosen from the stream's colour tags as the pinned Chrome does (VP8 measured BT.601, VP9 BT.709), so backends that hand us YUV agree.

## 6. Work that is independent of this review, and what changes

Continues as planned: Agent A's VP8 PRs (#353, #355), seek/audio/AudioClip steps, AV1 (rav1d); the AAC exclusion (#357); the licensing inventory. Nothing completed is discarded.

New items this review proposes (each small, none started):
1. Gate the native video decoders out of the wasm build (`native-video` feature) and add the artifact check that `vp9dec`/`vp8`/`rav1d` strings are absent from `x3d_web.wasm`, like `tools/check_no_aac.py --binary`. Must keep every browser test green.
2. A `MediaSource` trait that the existing browser and Rust paths implement, with no behavior change, so a SystemBackend can plug in. Prerequisite for anything below.
3. Experiment: GStreamer backend spike on Linux (needs `libgstreamer1.0-dev` and `libgstreamer-plugins-base1.0-dev`; sudo is available here, not installed yet): MP4/H.264 first picture against the pinned page and VP8/VP9/AV1 WebM against the Rust backend. Output: tolerance table and the A/V-sync cost.
4. Experiment: AVFoundation spike on macOS 15 (the hosted runner, labeled virtualized, and the owner's Mac for the physical result): which of WebM VP9, AV1, MP4 H.264 open, seek accuracy, `CVPixelBuffer` to Metal without a copy.
5. Experiment: direct GPU import of the `<video>` frame in the browser build (WebGPU `importExternalTexture`) against the current `getImageData` path: picture equality and cost. It would remove the per-frame CPU readback; compare before adopting.
6. Reverse play and the ISO time formula: implement in the Rust backend only as a recorded DIFFERENT-from-pinned feature, behind an explicit option, because the pinned page does not do it.

## 7. Open questions and what would answer them

* AVFoundation: WebM/VP9/AV1 support on macOS 15, seek accuracy, negative rates. Answer: item 4.
* GStreamer: negative-rate support and frame-accurate seek per decoder, and how much of the A/V clock work it saves. Answer: item 3.
* Whether a native viewer should open H.264/MP4 at all in a first release. Product decision, not technical.
* Patent position of delegating to OS decoders: expected to sit with the OS vendor or distribution, but that is a legal question for the release review, not settled here.
