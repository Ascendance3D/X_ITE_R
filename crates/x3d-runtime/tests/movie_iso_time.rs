//! ISO/IEC 19775-1 18.4.2 MovieTexture time (docs/movie-texture-timing.md) against a fake clock: every test sets
//! `Runtime::now` itself, no wall time is read. The frame list is `oracle/synthetic-movie-anim/media/anim4.gif`: four
//! frames of 0.1, 0.2, 0.3 and 0.4 s, so the frames start at 0, 0.1, 0.3 and 0.6 s and the duration is 1 s.
//!
//! The `<video>` path is emulated: the test plays the page's part (it answers `loadeddata` and the seeks by calling
//! `Runtime::video_loaded` and `Runtime::video_seeked`), there is no browser.

use std::path::PathBuf;
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::nodes::movie_texture::VideoOp;
use x3d_runtime::nodes::movie_time::MovieTiming;
use x3d_runtime::{Fetcher, Runtime};

struct Files;

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        let name = url.path().trim_start_matches('/');
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../oracle/synthetic-movie-anim/media");

        std::fs::read(dir.join(name)).map_err(|_| "TypeError: Failed to fetch".to_string())
    }

    fn plays_video(&self) -> bool {
        true
    }
}

/// The world with a MovieTexture `M` of the given fields; the clock starts at 100 s.
fn load(url: &str, fields: &str, timing: MovieTiming) -> Runtime {
    // A time field set before the browser is live gets no timer (Runtime::add_timeout), and a looping node whose
    // startTime and stopTime are both 0 starts at once. So the time fields and `loop` are taken out of the text and
    // set once the scene is live, as a script would, the start time first.
    let mut words: Vec<&str> = fields.split_whitespace().collect();
    let mut later = vec![];

    for key in ["startTime", "pauseTime", "resumeTime", "stopTime"] {
        if let Some(i) = words.iter().position(|w| *w == key) {
            later.push((key, FieldValue::SFTime(words[i + 1].parse().unwrap())));
            words.drain(i..i + 2);
        }
    }

    if let Some(i) = words.iter().position(|w| *w == "loop") {
        later.push(("loop", FieldValue::SFBool(words[i + 1] == "TRUE")));
        words.drain(i..i + 2);
    }

    let fields = words.join(" ");
    let text = format!(
        "#X3D V4.0 utf8\nPROFILE Full\nSound {{ source DEF M MovieTexture {{ url \"{url}\" {fields} }} }}\n"
    );
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(Files));

    rt.set_movie_timing(timing);
    rt.now = 100.0;
    rt.load_world();

    for _ in 0..10 {
        rt.run_tasks();
    }

    rt.live = true;
    rt.go_live();
    rt.frame();

    for (key, value) in later {
        let k = rt.pre(movie(&rt), key).unwrap();

        rt.set_value(k, value);
        rt.process_events();
    }

    rt.frame();
    rt
}

fn movie(rt: &Runtime) -> NodeId {
    rt.scene.get_named_node(x3d_scene::ROOT, "M").unwrap()
}

/// The fake clock: the browser time becomes `t` seconds. The timers of the node's four time fields (the setTimeouts
/// of X3DTimeDependentNode) fire at their due times on the way, each with a frame, as a host's timer loop does, then
/// one frame runs at `t`.
fn at(rt: &mut Runtime, t: f64) {
    let m = movie(rt);
    let mut due: Vec<f64> = ["startTime", "pauseTime", "resumeTime", "stopTime"]
        .iter()
        .map(|f| rt.get_f64(m, f))
        .filter(|d| *d > rt.now && *d < t)
        .collect();

    due.sort_by(|a, b| a.partial_cmp(b).unwrap());

    for d in due {
        rt.now = d;
        rt.fire_timeouts(d);
        rt.frame();
    }

    rt.now = t;
    rt.fire_timeouts(t);
    rt.frame();
}

fn shown(rt: &Runtime) -> usize {
    rt.movie_texture_frame(movie(rt)).unwrap().3
}

fn set(rt: &mut Runtime, field: &str, value: FieldValue) {
    let k = rt.pre(movie(rt), field).unwrap();

    rt.set_value(k, value);
    rt.process_events();
    rt.frame();
}

fn active(rt: &Runtime) -> bool {
    rt.get_bool(movie(rt), "isActive")
}

#[test]
fn an_inactive_node_shows_frame_zero_or_for_a_negative_speed_the_last_frame() {
    let rt = load("anim4.gif", "", MovieTiming::Iso);
    assert_eq!(shown(&rt), 0);

    let rt = load("anim4.gif", "speed -1", MovieTiming::Iso);
    assert_eq!(shown(&rt), 3);

    let rt = load("anim4.gif", "speed 0", MovieTiming::Iso);
    assert_eq!(shown(&rt), 0);
}

#[test]
fn positive_speed_selects_frames_by_their_times_and_loops() {
    let mut rt = load("anim4.gif", "startTime 102 loop TRUE", MovieTiming::Iso);

    at(&mut rt, 102.05);
    assert!(active(&rt));
    assert_eq!(shown(&rt), 0);
    // 0.35 s into the movie: the third frame starts at 0.3 s (GifMedia's equal steps would give frame 1).
    at(&mut rt, 102.35);
    assert_eq!(shown(&rt), 2);
    at(&mut rt, 102.95);
    assert_eq!(shown(&rt), 3);
    // The second cycle: t = 1.15 mod 1.
    at(&mut rt, 103.15);
    assert_eq!(shown(&rt), 1);
    assert!(active(&rt));
}

#[test]
fn without_loop_the_play_ends_on_the_last_frame() {
    let mut rt = load("anim4.gif", "startTime 102", MovieTiming::Iso);

    at(&mut rt, 102.9);
    assert_eq!(shown(&rt), 3);
    assert!(active(&rt));
    at(&mut rt, 103.5);
    assert!(!active(&rt));
    // "The frame corresponding to the time at which the MovieTexture became inactive will remain."
    assert_eq!(shown(&rt), 3);
    at(&mut rt, 110.0);
    assert_eq!(shown(&rt), 3);
}

#[test]
fn negative_speed_plays_backwards_and_ends_on_frame_zero() {
    let mut rt = load(
        "anim4.gif",
        "startTime 102 speed -1 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.0);
    assert_eq!(shown(&rt), 3);
    // t = 1 - 0.5
    at(&mut rt, 102.5);
    assert_eq!(shown(&rt), 2);
    at(&mut rt, 102.95);
    assert_eq!(shown(&rt), 0);
    // Looping: back at the end of the movie.
    at(&mut rt, 103.1);
    assert_eq!(shown(&rt), 3);

    let mut rt = load("anim4.gif", "startTime 102 speed -1", MovieTiming::Iso);

    at(&mut rt, 102.5);
    assert_eq!(shown(&rt), 2);
    at(&mut rt, 103.2);
    assert!(!active(&rt));
    assert_eq!(shown(&rt), 0);
}

#[test]
fn speed_scales_the_movie_time_and_zero_speed_holds_frame_zero() {
    // Speed 2: 0.2 s of the clock is 0.4 s of the movie.
    let mut rt = load(
        "anim4.gif",
        "startTime 102 speed 2 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.2);
    assert_eq!(shown(&rt), 2);
    // 0.85 s of the clock mod 0.5 = 0.35 -> 0.7 s of the movie.
    at(&mut rt, 102.85);
    assert_eq!(shown(&rt), 3);

    let mut rt = load(
        "anim4.gif",
        "startTime 102 speed 0 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 105.0);
    assert!(active(&rt));
    assert_eq!(shown(&rt), 0);
}

#[test]
fn set_speed_is_ignored_while_active_and_applied_while_inactive() {
    // Inactive, never played: the new speed picks the inactive frame.
    let mut rt = load("anim4.gif", "startTime 102 loop TRUE", MovieTiming::Iso);

    set(&mut rt, "speed", FieldValue::SFFloat(-1.0));
    assert_eq!(shown(&rt), 3);
    set(&mut rt, "speed", FieldValue::SFFloat(1.0));
    assert_eq!(shown(&rt), 0);

    // Active: set_speed -1 at 102.5 does nothing, the movie goes on forward.
    at(&mut rt, 102.5);
    assert_eq!(shown(&rt), 2);
    set(&mut rt, "speed", FieldValue::SFFloat(-1.0));
    at(&mut rt, 102.7);
    assert!(active(&rt));
    assert_eq!(shown(&rt), 3);
}

#[test]
fn stop_pause_and_resume_follow_the_time_dependent_node_rules() {
    // stopTime ends the play; the frame at that moment remains.
    let mut rt = load(
        "anim4.gif",
        "startTime 102 stopTime 102.4 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.35);
    assert_eq!(shown(&rt), 2);
    at(&mut rt, 102.9);
    assert!(!active(&rt));
    assert_eq!(shown(&rt), 2);

    // pauseTime 102.2, resumeTime 102.8: the paused 0.6 s do not count.
    let mut rt = load(
        "anim4.gif",
        "startTime 102 pauseTime 102.2 resumeTime 102.8 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.15);
    assert_eq!(shown(&rt), 1);
    at(&mut rt, 102.5);
    assert!(rt.get_bool(movie(&rt), "isPaused"));
    assert_eq!(shown(&rt), 1);
    at(&mut rt, 102.9);
    assert!(!rt.get_bool(movie(&rt), "isPaused"));
    // 0.2 s before the pause + 0.1 s after the resume = 0.3 s into the movie.
    assert_eq!(shown(&rt), 2);
}

#[test]
fn pinned_timing_keeps_the_equal_steps_and_the_negative_rate_frame_zero() {
    let mut rt = load(
        "anim4.gif",
        "startTime 102 speed -1 loop TRUE",
        MovieTiming::Pinned,
    );

    // Inactive: frame 0 whatever the sign (GifMedia).
    assert_eq!(shown(&rt), 0);
    // A negative rate gives a negative time: frame 0 (the ISO clause shows frame 2 here).
    at(&mut rt, 102.5);
    assert_eq!(shown(&rt), 0);

    // Equal steps of a quarter of the duration, not the frame delays.
    let mut rt = load("anim4.gif", "startTime 102 loop TRUE", MovieTiming::Pinned);

    at(&mut rt, 102.2);
    assert_eq!(shown(&rt), 0);
    at(&mut rt, 102.3);
    assert_eq!(shown(&rt), 1);
}

/// The page's part for a `<video>`: answers `loadeddata` with a 2 x 2 picture whose texels are `value`.
fn video_world(timing: MovieTiming, fields: &str) -> (Runtime, u64) {
    let mut rt = load("clip.mp4", fields, timing);
    let ops = rt.take_video_ops();
    let Some(VideoOp::Load { token, .. }) = ops.first().cloned() else {
        panic!("no load: {ops:?}");
    };

    assert!(rt.video_loaded(token, 2, 2, 4.0, false, vec![0; 16]));
    (rt, token)
}

fn seeks(ops: &[VideoOp]) -> Vec<(u64, f64)> {
    ops.iter()
        .filter_map(|o| match o {
            VideoOp::Seek { id, time, .. } => Some((*id, *time)),
            _ => None,
        })
        .collect()
}

/// The page answers the newest seek request among `ops` with a picture of `value`.
fn answer(rt: &mut Runtime, token: u64, ops: &[VideoOp], value: u8) -> (u64, f64) {
    let (id, time) = *seeks(ops).last().expect("a seek request");

    assert!(rt.video_seeked(token, id, time, vec![value; 16]));
    (id, time)
}

#[test]
fn a_video_is_driven_by_seeks_and_never_played_in_iso_mode() {
    let (mut rt, token) = video_world(MovieTiming::Iso, "startTime 102 loop TRUE");
    let ops = rt.take_video_ops();

    // Inactive: the element is asked for frame 0 (time 0).
    assert_eq!(answer(&mut rt, token, &ops, 1).1, 0.0);

    // The start asks for time 0 again (forced), then the clock runs.
    at(&mut rt, 102.0);
    let ops = rt.take_video_ops();

    assert_eq!(answer(&mut rt, token, &ops, 2).1, 0.0);
    at(&mut rt, 103.5);

    let ops = rt.take_video_ops();

    assert!(
        !ops.iter()
            .any(|o| matches!(o, VideoOp::Play(_) | VideoOp::Rewind(_))),
        "{ops:?}"
    );

    // 1.5 s into a 4 s movie at speed 1.
    let (_, time) = answer(&mut rt, token, &ops, 3);

    assert!((time - 1.5).abs() < 1e-9, "{time}");
    assert_eq!(
        rt.movie_current_time(movie(&rt))
            .map(|t| (t - 1.5).abs() < 1e-9),
        Some(true)
    );
    assert_eq!(rt.movie_texture_frame(movie(&rt)).unwrap().2[0], 3);
}

#[test]
fn a_negative_speed_video_starts_at_the_end_and_asks_for_the_last_frame_when_inactive() {
    let (mut rt, token) = video_world(MovieTiming::Iso, "startTime 102 speed -1 loop TRUE");
    let ops = rt.take_video_ops();

    assert_eq!(answer(&mut rt, token, &ops, 1).1, 4.0);
    at(&mut rt, 102.0);
    let ops = rt.take_video_ops();

    assert_eq!(answer(&mut rt, token, &ops, 2).1, 4.0);
    at(&mut rt, 103.0);

    let ops = rt.take_video_ops();

    assert_eq!(answer(&mut rt, token, &ops, 3).1, 3.0);
}

#[test]
fn an_obsolete_seek_result_never_overwrites_a_newer_picture() {
    let (mut rt, token) = video_world(MovieTiming::Iso, "startTime 102 stopTime 102.6 loop TRUE");
    let ops = rt.take_video_ops();
    let first = answer(&mut rt, token, &ops, 1);

    // The start (102) asks for time 0 and the page is slow: that request stays unanswered.
    at(&mut rt, 102.0);
    let second = *seeks(&rt.take_video_ops()).last().unwrap();

    // The play stops at 102.6 and starts again at 103: a forced request supersedes the one in flight.
    at(&mut rt, 102.7);
    assert!(!active(&rt));
    set(&mut rt, "startTime", FieldValue::SFTime(103.0));
    set(&mut rt, "stopTime", FieldValue::SFTime(0.0));
    at(&mut rt, 103.0);
    let third = *seeks(&rt.take_video_ops()).last().unwrap();

    assert!(
        first.0 < second.0 && second.0 < third.0,
        "{first:?} {second:?} {third:?}"
    );

    // Out of order: the answer of the newest request arrives first, the old ones after it.
    assert!(rt.video_seeked(token, third.0, 0.0, vec![4; 16]));
    assert!(rt.video_seeked(token, second.0, 0.5, vec![3; 16]));
    assert!(rt.video_seeked(token, first.0, 0.0, vec![9; 16]));

    assert_eq!(
        rt.movie_texture_frame(movie(&rt)).unwrap().2[0],
        4,
        "an obsolete picture overwrote the newer one"
    );
    // An answer of a token that is not this runtime's says so.
    assert!(!rt.video_seeked(token + 1000, 1, 0.0, vec![]));
}

#[test]
fn a_seek_answered_without_a_picture_is_asked_again_at_the_same_time() {
    let (mut rt, token) = video_world(MovieTiming::Iso, "startTime 102 loop TRUE");
    let ops = rt.take_video_ops();

    answer(&mut rt, token, &ops, 5);

    // The start asks for time 0; the page could not read the picture and answers empty.
    at(&mut rt, 102.0);
    let (id, time) = *seeks(&rt.take_video_ops()).last().expect("a seek request");

    assert!(rt.video_seeked(token, id, time, vec![]));
    assert_eq!(rt.movie_texture_frame(movie(&rt)).unwrap().2[0], 5);

    // The next tick at the same time asks again (without the retry the identical request is suppressed).
    at(&mut rt, 102.0);
    let again = seeks(&rt.take_video_ops());

    assert_eq!(again.last().map(|s| s.1), Some(time), "{again:?}");
}

#[test]
fn pinned_video_keeps_playing_and_a_negative_speed_plays_at_one() {
    let (mut rt, token) = video_world(MovieTiming::Pinned, "startTime 102 speed -1 loop TRUE");
    let ops = rt.take_video_ops();

    assert!(seeks(&ops).is_empty(), "{ops:?}");
    // set_speed__: Chrome rejects a negative playbackRate, the element plays at 1.
    assert!(
        ops.iter().any(|o| *o == VideoOp::Rate(token, 1.0, true)),
        "{ops:?}"
    );

    at(&mut rt, 102.0);
    assert!(
        rt.take_video_ops()
            .iter()
            .any(|o| *o == VideoOp::Play(token))
    );
    // The page's frames still reach the texture.
    assert!(rt.video_frame(token, 0.5, vec![7; 16]));
    at(&mut rt, 102.1);
    assert_eq!(rt.movie_texture_frame(movie(&rt)).unwrap().2[0], 7);
}

/// "The frame corresponding to the time at which the MovieTexture became inactive will remain": the stop is a timer
/// event between two frames, and the picture is the one of that instant. The frames start at 0, 0.1, 0.3 and 0.6 s.
#[test]
fn a_stop_between_two_frames_keeps_the_picture_of_the_instant_of_the_stop() {
    let mut rt = load(
        "anim4.gif",
        "startTime 102 stopTime 102.65 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.55);
    assert!(active(&rt));
    assert_eq!(shown(&rt), 2);
    // The last tick before the stop was at 102.55 (frame 2); the stop is at 102.65 (t = 0.65, frame 3).
    at(&mut rt, 102.7);
    assert!(!active(&rt));
    assert_eq!(shown(&rt), 3);
}

#[test]
fn a_pause_between_two_frames_keeps_the_picture_of_the_instant_of_the_pause() {
    let mut rt = load(
        "anim4.gif",
        "startTime 102 pauseTime 102.65 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.55);
    assert_eq!(shown(&rt), 2);
    at(&mut rt, 102.9);
    assert!(rt.get_bool(movie(&rt), "isPaused"));
    assert_eq!(shown(&rt), 3);
    at(&mut rt, 103.5);
    assert_eq!(shown(&rt), 3);
}

// ---- Runtime::movie_seek in ISO timing: the seek moves the origin of the clause's formula ----

fn seek(rt: &mut Runtime, t: f64) -> Option<f64> {
    let m = movie(rt);

    rt.movie_seek(m, t)
}

/// An active, looping node: the movie goes on from the seeked time at the same rate and wraps.
#[test]
fn a_seek_of_an_active_node_continues_from_that_time_and_wraps() {
    let mut rt = load("anim4.gif", "startTime 102 loop TRUE", MovieTiming::Iso);

    at(&mut rt, 102.2);
    assert_eq!(shown(&rt), 1);
    assert_eq!(seek(&mut rt, 0.65), Some(0.65));
    assert_eq!(shown(&rt), 3);
    // 0.1 s later: t = 0.75, 0.3 s later t = 0.95 (both frame 3; unseeked they would be frames 1 and 2).
    at(&mut rt, 102.3);
    assert_eq!(shown(&rt), 3);
    at(&mut rt, 102.5);
    assert_eq!(shown(&rt), 3);
    // 0.4 s later: 1.05 wraps to 0.05.
    at(&mut rt, 102.6);
    assert_eq!(shown(&rt), 0);
    assert!(active(&rt));
}

/// Without `loop` the rest of the movie is what remains of the play: the node ends on the last frame after
/// `(duration - t) / speed`, and the end is the node's `isActive` FALSE.
#[test]
fn a_seek_shortens_or_extends_the_play_of_a_node_without_loop() {
    let mut rt = load("anim4.gif", "startTime 102", MovieTiming::Iso);

    at(&mut rt, 102.2);
    seek(&mut rt, 0.9);
    at(&mut rt, 102.25);
    assert!(active(&rt));
    assert_eq!(shown(&rt), 3);
    at(&mut rt, 102.35);
    assert!(!active(&rt));
    assert_eq!(shown(&rt), 3);

    // Backwards in time: a seek to the start restarts the whole movie.
    let mut rt = load("anim4.gif", "startTime 102", MovieTiming::Iso);

    at(&mut rt, 102.8);
    seek(&mut rt, 0.0);
    assert_eq!(shown(&rt), 0);
    at(&mut rt, 103.5);
    assert!(active(&rt), "0.7 s into the second pass");
    assert_eq!(shown(&rt), 3);
    at(&mut rt, 103.9);
    assert!(!active(&rt));
}

#[test]
fn a_seek_of_a_node_playing_backwards_continues_backwards() {
    let mut rt = load(
        "anim4.gif",
        "startTime 102 speed -1 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.1);
    assert_eq!(shown(&rt), 3);
    seek(&mut rt, 0.25);
    assert_eq!(shown(&rt), 1);
    at(&mut rt, 102.2);
    assert_eq!(shown(&rt), 1);
    // t = 0.25 - 0.2 = 0.05
    at(&mut rt, 102.3);
    assert_eq!(shown(&rt), 0);
    // Wrapped to the end: 0.25 - 0.4 = -0.15, that is 0.85.
    at(&mut rt, 102.5);
    assert_eq!(shown(&rt), 3);
}

/// `time` is a movie time: at speed 2 the movie goes on at twice the clock's rate from the seeked time.
#[test]
fn a_seek_goes_on_at_the_rate_of_the_node() {
    let mut rt = load(
        "anim4.gif",
        "startTime 102 speed 2 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.1);
    seek(&mut rt, 0.32);
    assert_eq!(shown(&rt), 2);
    // 0.1 s of the clock is 0.2 s of the movie: t = 0.52
    at(&mut rt, 102.2);
    assert_eq!(shown(&rt), 2);
    // t = 0.72
    at(&mut rt, 102.3);
    assert_eq!(shown(&rt), 3);
}

/// A pause keeps the seeked picture and the node resumes from it, with the paused time not counted.
#[test]
fn a_seek_during_a_pause_resumes_from_the_seeked_time() {
    let mut rt = load(
        "anim4.gif",
        "startTime 102 pauseTime 102.2 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.3);
    assert!(rt.get_bool(movie(&rt), "isPaused"));
    assert_eq!(shown(&rt), 1);
    seek(&mut rt, 0.7);
    assert_eq!(shown(&rt), 3);
    at(&mut rt, 102.4);
    assert_eq!(shown(&rt), 3, "still paused");
    set(&mut rt, "resumeTime", FieldValue::SFTime(102.5));
    at(&mut rt, 102.6);
    // 0.1 s after the resume: t = 0.8.
    assert_eq!(shown(&rt), 3);
    // 0.5 s after the resume: t = 1.2 wraps to 0.2 (without the seek it would be 0.7, frame 3).
    at(&mut rt, 103.0);
    assert_eq!(shown(&rt), 1);
}

/// An inactive node shows the seeked picture and keeps it; a start begins at the start of the movie again.
#[test]
fn a_seek_of_an_inactive_node_shows_the_picture_and_a_start_begins_again() {
    let mut rt = load("anim4.gif", "startTime 102", MovieTiming::Iso);

    assert_eq!(shown(&rt), 0);
    assert_eq!(seek(&mut rt, 0.35), Some(0.35));
    assert_eq!(shown(&rt), 2);
    // A frame does not undo it (the inactive picture rule does not apply to a node that was seeked).
    rt.now = 101.5;
    rt.frame();
    assert_eq!(shown(&rt), 2);
    at(&mut rt, 102.05);
    assert!(active(&rt));
    assert_eq!(shown(&rt), 0);

    // After the play ended the seeked picture remains.
    at(&mut rt, 103.5);
    assert!(!active(&rt));
    assert_eq!(seek(&mut rt, 0.15), Some(0.15));
    assert_eq!(shown(&rt), 1);
    at(&mut rt, 105.0);
    assert_eq!(shown(&rt), 1);
}

#[test]
fn the_stop_time_still_stops_a_seeked_node() {
    let mut rt = load(
        "anim4.gif",
        "startTime 102 stopTime 102.4 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.2);
    seek(&mut rt, 0.5);
    at(&mut rt, 102.5);
    assert!(!active(&rt));
    // At the stop (0.2 s after the seek) the movie was at t = 0.7: frame 3 (starts 0.6) stays, where the unseeked
    // node would have stopped at t = 0.4, frame 2.
    assert_eq!(shown(&rt), 3);
}

/// Speed 0 does not move the movie; a seek is the held picture.
#[test]
fn a_seek_at_speed_zero_holds_the_picture() {
    let mut rt = load(
        "anim4.gif",
        "startTime 102 speed 0 loop TRUE",
        MovieTiming::Iso,
    );

    at(&mut rt, 102.2);
    assert_eq!(shown(&rt), 0);
    seek(&mut rt, 0.65);
    assert_eq!(shown(&rt), 3);
    at(&mut rt, 102.9);
    assert_eq!(shown(&rt), 3);
}

#[test]
fn a_seek_is_clamped_to_the_movie_and_a_nan_is_refused() {
    let mut rt = load("anim4.gif", "", MovieTiming::Iso);

    assert_eq!(seek(&mut rt, -5.0), Some(0.0));
    assert_eq!(seek(&mut rt, 100.0), Some(1.0));
    assert_eq!(shown(&rt), 3);
    assert_eq!(seek(&mut rt, f64::NAN), None);
    assert!(rt.log.iter().any(|l| l.contains("cannot seek")));
    assert_eq!(shown(&rt), 3);
}

/// A `<video>`: the seek is a request to the page for the time asked, and the answer is the picture.
#[test]
fn a_seek_of_a_video_asks_the_page_for_that_time() {
    let (mut rt, token) = video_world(MovieTiming::Iso, "startTime 102 loop TRUE");
    let _ = rt.take_video_ops();

    at(&mut rt, 102.0);
    let _ = rt.take_video_ops();
    at(&mut rt, 102.5);
    let _ = rt.take_video_ops();
    assert_eq!(seek(&mut rt, 1.5), Some(1.5));

    let ops = rt.take_video_ops();

    assert_eq!(seeks(&ops).last().map(|s| s.1), Some(1.5), "{ops:?}");
    answer(&mut rt, token, &ops, 9);
    assert!((rt.movie_current_time(movie(&rt)).unwrap() - 1.5).abs() < 1e-9);
    // 0.25 s later the formula continues from 1.5 (not from the 0.75 it would have been at).
    at(&mut rt, 102.75);

    let ops = rt.take_video_ops();

    assert!(
        seeks(&ops).iter().any(|s| (s.1 - 1.75).abs() < 0.03),
        "{ops:?}"
    );
}

/// Pinned timing keeps the earlier seek of #361: a GIF cannot seek.
#[test]
fn pinned_timing_keeps_its_seek_rules() {
    let mut rt = load("anim4.gif", "startTime 102 loop TRUE", MovieTiming::Pinned);

    at(&mut rt, 102.2);
    assert_eq!(seek(&mut rt, 0.65), None);
}

#[test]
fn an_mp4_video_asks_the_page_for_its_sound_and_plays_it_on_the_timeline() {
    // ADR 0015: the load of an MP4 is followed by a Sound call; the page answers with what decodeAudioData gave.
    let mut rt = load("clip.mp4", "startTime 102 loop TRUE", MovieTiming::Iso);
    let ops = rt.take_video_ops();
    let token = match ops.as_slice() {
        [VideoOp::Load { token, .. }, VideoOp::Sound(t)] if t == token => *token,
        _ => panic!("{ops:?}"),
    };

    assert!(rt.video_loaded(token, 2, 2, 2.0, true, vec![0; 16]));

    // The late-audio file: an empty edit of 0.228 s; decodeAudioData gives the 1.772 s of samples from 0.
    let late = std::fs::read(
        std::path::PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/mp4/h264-aac-audio-late.mp4"),
    )
    .unwrap();
    let samples = vec![vec![0.5f32; 85024]; 2];

    assert!(rt.video_sound(token, &late, samples, 48000.0));
    assert!(!rt.video_sound(token + 1000, &late, vec![vec![0.0; 4]], 48000.0));

    let m = movie(&rt);
    let element = rt
        .sound
        .get(&m)
        .and_then(|s| s.media.as_ref())
        .expect("the sound");

    // Padded to the element's duration (2 s), so a loop of the sound and of the pictures is the same length.
    assert!(
        (element.duration - 2.0).abs() < 1e-6,
        "{}",
        element.duration
    );
    assert_eq!(rt.movie_audio_resyncs(m), Some(0));
}
