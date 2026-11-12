# 0015. MP4 in MovieTexture: video and audio through platform decoders, one timeline in the engine

Status: **Accepted (decision of the repository owner, 2026-11-12).** Attribution: the owner's written instruction "NATIVE MP4 SUPPORT" of 2026-11-12, its "PHASE 8 PLAN CORRECTIONS" of the same day, and two answers of the owner of the same day: the Linux backend uses the `gstreamer-rs` binding, and the platform decoder also decodes the audio of an MP4 (AAC included). A change of this record needs a new dated decision of the owner.

Reference: X_ITE 16.5.0 at `34cdf9f97` (`oracle/x_ite-pin`). Standard: ISO/IEC 19775-1 V4.0, Part 1, 18.4.2 (MovieTexture) and 16.4 (Sound, the `source` field), read in the local capture of the standard. Related: `docs/media-architecture.md` (issue #90 review), `docs/aac-dependency.md`, `docs/codec-licensing-inventory.md`, `docs/decoder-admission.md`.

## Context

The native viewer decodes WebM (VP8, VP9, AV1) and MPEG-1 in Rust. An MP4 file is "assumed playable": the loader returns with no frame, no sound, no message, and the node stays IN_PROGRESS (`crates/x3d-runtime/src/loader.rs`, the branch after `video_source_kind`). The owner requires MP4 video and audio in the native viewer when a supported platform decoder is present.

Rules of the owner that this record keeps:

* Do not write a Rust H.264, HEVC or AAC decoder. Do not bundle such a codec. Do not use JavaScript X_ITE as a fallback.
* The platform decoder decodes the compressed media. The engine keeps MovieTexture timing, state, events, synchronization, seek, loop and resource release.
* Do not assume that an MP4 is H.264. Report a clear unsupported-container or unsupported-codec result.
* Keep the WebM, VP8, VP9, AV1 and MPEG-1 behavior.
* OS-delegated AAC is approved. The Symphonia AAC path (`docs/aac-dependency.md`) stays unsupported.

## Evidence

REA: `analyze_javascript_application` on `oracle/x_ite-pin/src/x_ite/Components` (MovieTexture.js, X3DSoundSourceNode.js, Sound.js, SpatialSound.js), Evidence `ev_42f876ad17637cd33d79eaa71ad6169f1f449b0e931a4354b066869fcd6acd3a` (summary `ev_b154aa5abc4f03e9cf7143c7851045d048ee89a47fd6add002d9c0294a7705af`, directory digest `61f7772d0ec17b633b4b82daa757072a4a2379a86db1e86cda7463f923ebdf04`). The static graph has no method-level relation for these calls (trace `ev_9e824fbfac3f839d7b76d3cdac9731150ec3b266ab784f558c8b6802aabb1c4e`: no match), so the line citations below come from the pinned source that the Evidence identifies.

1. **Audio routing (pinned).** `X3DSoundSourceNode.js:15` makes the node's audio source, a `GainNode` with gain 0; `:78` sets the gain from the `gain` field. `MovieTexture.js:26-28` connects that source to a `MediaStreamDestination` that nothing plays. `MovieTexture.js:118-119` connects the video element to the audio source (`createMediaElementSource`). `Sound.js:180-209` connects the audio source of the node in its `source` field (and its children) to the Sound node's graph. So the sound of a movie reaches the output only through a Sound node. There is no automatic output.
2. **Standard.** 18.4.2: "A MovieTexture node can also be used as the source of sound data for a Sound node", and it "can be referenced ... by a Sound node's source field (as an audio source only)". 16.4 Sound: "If the source field is not specified, the Sound node will not emit audio."
3. **Load failure (pinned).** `MovieTexture.js:152-167` (`loadVideo`) rejects on the element's `error` or `abort` with `Couldn't load video '<url>': <type>.`; `:125-131` logs `Error loading movie '<url>':` with that message and tries the next url; `:134-137` after the last url: `setMediaElement (null)`, `clearTexture ()`, `updateOutputs (0, 0, 0, false, -1)`, FAILED_STATE.
4. **One element clock (pinned).** `X3DSoundSourceNode.js:85-121`: `set_loop` sets `element.loop`; `set_start` sets `currentTime = 0` and starts the element; `set_pause` and `set_stop` stop it; `set_resume` starts it; `set_time` (:122-132) ends the node when `currentTime >= duration` and `loop` is FALSE. The picture is the element's current frame. Audio and picture have one clock: the element's.
5. **The port (read).** A MovieTexture is a sound source with two members: `MovieMedia` (pictures, `crates/x3d-runtime/src/nodes/movie_texture.rs`) and an `AudioElement` (a web-audio-api buffer source on the node's audio source, `crates/x3d-runtime/src/nodes/sound.rs`). `movie_set_frames` makes the `AudioElement` from `AudioTrack::Decoded`. Each transport function (start, pause, resume, stop, seek, loop, rate, unload) sends the same operation to both. WebM Vorbis and MPEG-1 audio already play this way.

## Decisions

### 1. Use the existing interface; add no media framework

The MP4 path ends in the existing `movie_set_frames`, like `decode_webm` and `decode_mpeg1`:

```
loader.rs MovieTexture, native build
  mp4 / m4v / mov / video/mp4
    -> mp4probe (Rust, container boxes only, no codec)
    -> MovieMedia::decode_platform (movie_texture/platform_media.rs)
    -> sysvideo::decode (data, &Mp4Info, MAX_DECODED_BYTES) -> (Video, AudioTrack) or WebmError
    -> movie_set_frames (existing: frames, AudioElement, outputs, COMPLETE_STATE)
```

`Video` and `VideoFrame` move from `webm.rs` to `video_types.rs` (`webm` re-exports them). `WebmError` gets the variant `Unsupported { container, codec }`. The streaming `MediaSource` contract of `docs/media-architecture.md` 8.3 is not built: the pre-decoded interface is sufficient for the limits of decision 6.

### 2. One backend for each platform, behind optional features

| Platform | Backend | Cargo feature | Status |
| --- | --- | --- | --- |
| Linux | GStreamer 1.x through `gstreamer-rs` (`gstreamer`, `gstreamer-app`, `gstreamer-video`, `gstreamer-audio`) | `system-video-gst` | First implementation |
| macOS | AVFoundation (`AVAssetReader`) through the `objc2` crates | `system-video-avf` | Second implementation |
| Windows | Media Foundation (`IMFSourceReader`) | none | Not implemented: no approved test environment |
| Browser (wasm) | The page's `<video>` element (exists: `video_mirror.mjs`) | none | Tests only |

`x3d-render` and `x3d-viewer` forward the features. They are off by default until each backend passes its acceptance tests on its platform. The owner decides the default afterwards.

The Linux pipeline: `appsrc` (the file bytes) `! decodebin3`; video pad `! videoconvert ! video/x-raw,format=I420 ! appsink`; audio pad `! audioconvert ! audio/x-raw,format=F32LE,layout=non-interleaved ! appsink`; both sinks with `sync=false`. The backend pulls every sample to the end of the stream. The engine converts YUV to RGBA with the converter of `webm.rs`, with the matrix and range from the stream caps, so colour conversion stays in one place.

### 3. One timeline from the container timestamps

* The backends give presentation times after edit lists (GStreamer: the stream time of `qtdemux` buffers; AVFoundation: `CMSampleBufferGetPresentationTimeStamp`).
* Origin `t0` = the earliest first presentation time of the video and audio tracks (the element's `currentTime` 0). Tracks do not have to start at zero.
* A picture's time is `pts - t0`. Before the first picture, the first picture shows.
* Audio: silence of `first_audio_pts - t0` goes before the first sample, so buffer offset 0 is `t0`. A gap inside the audio track is filled with silence.
* `duration` = the latest track end minus `t0` (the element's `duration`).
* Video only: `AudioTrack::None`, `hasSound` FALSE. Audio only: a `MovieMedia` without pictures, width and height 0, `hasSound` TRUE (the element plays an audio-only file and gives `videoWidth` 0; measured before the change is made).

### 4. Clock authority and synchronization

* The engine's movie time is the master: the pinned or the ISO 18.4.2 formula from `elapsedTime` (`movie_time.rs`, `MovieTiming`). The picture is the latest picture whose time is not after the movie time.
* The audio follows. The `AudioElement` starts at the movie time on each start, resume, seek and loop (existing calls). Rate changes go to both (existing).
* New: on each movie tick, the engine compares the `AudioElement` position with the movie time. If the difference is more than the larger of two picture periods and 40 ms, the engine seeks the audio to the movie time. A counter records each correction for the tests. There is no log line.
* This differs from the pinned page in one point: there the element's clock is the master. The engine clock is necessary for the ISO formula (reverse play, `speed` 0) and for deterministic tests.

### 5. Audio output

Planar `f32` samples at the track's own rate go to `AudioEngine::buffer_from_planar` and the `AudioElement` on the node's audio source (existing). The sound is audible only through a Sound node whose `source` is the MovieTexture (evidence 1 and 2). This record adds no automatic output.

### 6. Limits

All pictures (RGBA) and all audio samples (`f32`) of one MovieTexture together are at most `MAX_DECODED_BYTES` (512 MiB). A larger file gives `TooLarge`, with the existing message. Practical durations at 30 pictures a second with 48 kHz stereo audio:

| Picture size | Bytes a second | Longest file |
| --- | --- | --- |
| 320 x 240 | about 9.6 MB | about 53 s |
| 640 x 480 | about 37 MB | about 14 s |
| 1280 x 720 | about 111 MB | about 4.6 s |

A streaming decode is the smallest change for longer files. It is not part of this record.

### 7. Failure

* The probe finds no MP4 structure, the build has no backend, the backend has no decoder for the codec (GStreamer `missing-plugin`, no pad), or the decode fails: the result is `WebmError::Unsupported { container, codec }` or `Decode`.
* The engine logs `MovieTexture: the video '<url>' is not decoded by this build (<reason>).`, then X_ITE's two messages, and tries the next url. After the last url: no media, cleared texture, outputs `0, 0, 0, FALSE, -1`, FAILED_STATE (evidence 3). This replaces today's silent IN_PROGRESS for MP4.
* No pipeline, reader or temporary file lives after `sysvideo::decode` returns, on success or failure.
* Not changed by this record: a WebM that this build does not decode still leaves the node IN_PROGRESS. That is reported to the owner as a separate finding.

### 8. Licensing and dependencies

* `gstreamer-rs`: MIT or Apache-2.0. It links the system `libgstreamer` (LGPL-2.1) dynamically. The decoders are the user's installed plugins (for example `gst-libav`, `openh264`). The project ships no codec.
* AVFoundation and Media Foundation are operating-system components.
* Linux build: `libgstreamer1.0-dev` and `libgstreamer-plugins-base1.0-dev`. Linux run: `gstreamer1.0-plugins-good` (`qtdemux`) and a decoder plugin; without them the result is `Unsupported`. macOS: the Xcode SDK.
* Test fixtures are synthetic files made locally with ffmpeg (a development tool only, `docs/media-architecture.md` 8.4).
* `docs/codec-licensing-inventory.md` gets a row for each backend. The release patent review stays open, as for AV1.

## Acceptance

For each backend, on its platform:

* Open, codec detection, rate, channels and duration recorded.
* Pictures: count, times, RGBA within a tolerance fixed before the run against ffmpeg's decode of the same file. Never called bit-identical.
* Audio: samples within a tolerance of ffmpeg's decode (AAC decoders are not bit-exact).
* Synchronization: with a Sound node, the tone of the audio at each movie time matches the picture shown at that time, through start, pause and resume, seek forward and back, loop, and rates 0.5 and 2, within one picture period.
* Late audio start, video only, audio only.
* Unsupported codec and damaged files: the failure path of decision 7, no crash.
* Unload and scene replacement release the audio and the pictures.
* Audible playback is claimed only from a measured output signal, never from a successful decode.
