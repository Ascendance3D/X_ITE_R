# MovieTexture time: ISO 19775-1 (default) and pinned X_ITE 16.5.0

Issue #90. Owner decisions 485a144a and f980cc8a (2026-11-02): ISO/IEC 19775-1 clause 18.4.2 reverse and forward
playback is the default; the behavior of the pinned X_ITE 16.5.0 is kept, unchanged, as a separate documented option.
The pinned behavior is not an approved DIFFERENT entry of the ledger.

## The option

`MovieTiming::{Iso, Pinned}` (`x3d_runtime::nodes::movie_time`).

| Where | How |
| --- | --- |
| Rust | `Runtime::movie_timing`, `Runtime::set_movie_timing (MovieTiming::Pinned)` (set it before the world loads). New runtimes start with `MovieTiming::process_default ()`, which is `Iso` unless `MovieTiming::set_process_default` chose otherwise. |
| `x3d-view`, `x3d-drive` | environment variable `X3D_MOVIE_TIMING=pinned` (or `iso`). |
| Web page | attribute `movietiming="pinned"` on `<x3d-canvas>` (ignored by X_ITE), or `WebBrowser.setMovieTiming ("pinned")` before the first world. |

## The clause

ISO/IEC 19775-1 v4.0, 18.4.2 MovieTexture (read from the local copy `reference/iso-iec-19775-1-v4.0-x3d`, `Part01/components/texturing.md`)
and 8.2.4 Time-dependent nodes:

- `speed` says how fast the movie plays; `duration_changed` is not affected by `speed`; "`set_speed` events are ignored while
  the movie is playing"; a negative `speed` plays the movie backwards.
- An inactive node that has just loaded shows frame 0 for a non-negative `speed` and the last frame for a negative one.
  `speed` 0 shows frame 0.
- Active, positive speed: `t = (now - startTime) modulo (duration / speed)`.
- Active, negative speed: `t = duration - ((now - startTime) modulo |duration / speed|)`.
- "When a MovieTexture node becomes inactive, the frame corresponding to the time at which the MovieTexture became inactive
  will remain as the texture."
- `pitch` scales the rate at which the movie plays (the engine keeps the pinned `rate = speed * max (pitch, 0)`).

## The formulas as implemented (`movie_time.rs`, pure functions)

`now - startTime` is the node's `elapsedTime` (paused intervals not counted, 8.2.4). With `rate = speed * max (pitch, 0)`
frozen at the start and `cycle = duration / |rate|`:

```
phase = (elapsed mod cycle) * |rate|              movie time covered in this cycle
t     = phase                      rate > 0
t     = duration - phase           rate < 0
```

- The clause's `t` is a time within one cycle; the movie time it selects is scaled by `|rate|` so that a movie at `speed 2`
  plays twice as fast. At `speed` 1 the formula is the clause's literally. This scaling is our reading of "movie time `t` ...
  with frame 0 at time 0 with `speed` = 1".
- `loop` FALSE: after `elapsed >= cycle` the play ends; `t` is the end of the movie in the direction of play (`duration`, or 0 for
  a negative rate), the node stops, and that frame remains.
- `rate == 0`: `t = 0` (frame 0), the node never ends by itself. A degenerate duration (0, negative, NaN, infinite) gives `t = 0`.
- Inactive and never played: `t = 0`, or `duration` for a negative rate. A node that played and stopped keeps its last frame.
- Frame of a frame list: the last frame whose start time is `<= t`, the start times being the running sum of the frame delays
  (`frame_at`, `frame_starts`). The pinned GifMedia uses equal steps of `duration / frames` instead.
- `set_speed`/`set_pitch` while the node is active is ignored (the rate was frozen at the start, `set_start`); while inactive it
  is applied (and re-picks the inactive frame while the media has never played).
- Pause and resume are the X3DTimeDependentNode rules of the engine (`elapsedTime` stops while paused, the frame remains).

## Native frame lists (GIF, APNG, decoded WebM)

`MovieMedia` frame lists use `iso_time` and `frame_at` from the scene clock (`Runtime::now`, via the node's `elapsedTime`) in
`Runtime::movie_iso_set_time`. The apng-js player is not ticked in ISO mode (its wall clock is not the scene clock).

## Browser `<video>` (`video_mirror.mjs`)

In ISO mode the element is never played. Each frame the engine decides the media time `t` and sends
`VideoOp::Seek { token, id, time }`; the page seeks the paused element and answers `videoSeeked (token, id, currentTime, pixels)`.

- `id` only grows. `SeekLedger` (engine): a new request is made when none is in flight, or when it is forced (start, restart,
  inactive frame). `accept (id)` is true only for the newest request's id; any other answer is dropped, so an obsolete picture
  never overwrites a newer one.
- Page: one seek runs at a time; a request that arrives meanwhile waits and replaces an older waiting one; the picture of a
  seek that was superseded is not read or sent.
- `Runtime::video_frame` (the pinned `videoFrame` push) is ignored in ISO mode; the pinned mode still plays the element,
  sends `Rate` (a negative rate goes to the element as 1, Chrome rejecting it) and `videoFrame`.

## Pinned versus ISO

| Case | Pinned (X_ITE 16.5.0) | ISO (default) |
| --- | --- | --- |
| negative `speed`, GIF/WebM | rate -1 gives negative media time, frame 0 | plays backwards from the last frame |
| negative `speed`, `<video>` | `playbackRate` rejected by Chrome, plays at 1 (`Rate (token, 1.0, ...)`) | seeks backwards |
| `speed` 0 | rate 0, frame 0 | frame 0 |
| inactive, negative `speed` | frame 0 | last frame |
| frame choice | GifMedia: equal steps; apng-js: its own player on the wall clock | frame start times from the delays, scene clock |
| `set_speed` while active | applied at once | ignored until the next start |
| `<video>` | plays, page pushes pictures | paused, engine seeks |

## Tests

- `crates/x3d-runtime/tests/movie_time_pure.rs` (fake-clock-free, 8): positive/negative/zero rate, loop and no loop,
  inactive frame 0 versus last, frame times versus equal steps, degenerate input, the seek ledger (obsolete result dropped,
  in flight/forced).
- `crates/x3d-runtime/tests/movie_iso_time.rs` (12, fake clock: the test sets `Runtime::now`, fires the timers at their due
  times, no wall time): inactive frames, positive speed with loop, no loop ending on the last frame, negative speed, speed 2
  and speed 0, `set_speed` while active (ignored) and inactive (applied), stop/pause/resume, `<video>` seeks and no play, negative
  `<video>` start, out-of-order seek answers (EMULATED page), Pinned reproducing the old frames and the old `<video>` ops.
- `crates/x3d-web/tests/browser/videomirror01.test.mjs` (6, node, EMULATED element and engine): the page side of the seek protocol.
- Re-pointed at Pinned (set in their setup, expectations unchanged): `crates/x3d-runtime/tests/movie_video_audio.rs`
  (`Runtime::set_movie_timing`), `crates/x3d-viewer/tests/movie_anim.rs` (`X3D_MOVIE_TIMING=pinned` for x3d-drive;
  `oracle/movie-anim-baseline`, `oracle/movie-load-errors-baseline`), the web runtime 02 page `tests/browser/runtime02.html`
  (`movietiming="pinned"`; `movie01.test.mjs` against `oracle/web-browser-movie-01-baseline`).
- Unchanged and still passing without a re-point: `crates/x3d-render/tests/render_movie_codecs.rs` and the MovieTexture worlds
  of `render_rea_b09_visual.rs` (a static frame 0 at speed >= 0 is the same in both modes), `crates/x3d-web/tests/async_loading.rs`.

## Known gaps

- Sound of a decoded WebM (native): it follows the ISO movie time. A negative rate is the AudioBufferSourceNode's negative
  `playbackRate` started at the end of the buffer (tests/movie_iso_audio.rs, engine tests render it and read a falling ramp),
  and a resume restarts the sound at the movie time of the pause. A `<video>` element that is only seeked is still silent
  (its sound is not driven in ISO mode, browser only, not verifiable here). Seeking while looping is not sample-accurate.
- Seek latency in the browser: the shown picture follows the clause's `t` by the time a seek takes (one request in flight at
  a time); a frame of lag at most per seek.
- The clause's `t` is a cycle time (see the formulas); a reading with `t` taken literally as movie time differs only for
  `|speed| != 1`.
- `Runtime::movie_seek` (the earlier seek API of PR #361) keeps its pinned behavior and is not wired into the ISO formulas.
- A stop by `stopTime` or a pause by `pauseTime` keeps the picture of the movie time of the instant of the event (the timer fires between frames; `movie_iso_freeze`), not of the last frame before it.
- MPEG-1 is the format the clause names; it is not decoded here (step 11 of the plan).
- The browser tests (`movie01`) could not be run in this lane's environment; the CI Web job runs them.
- The crate's lib test target does not build on the base `agentA/step7-video-audio` (`webm.rs` test items `codec_fixture`,
  `sha`, `VP8_FIXTURES`, `vp8_pictures` are defined twice, E0428), so the pure-function tests are integration tests.
