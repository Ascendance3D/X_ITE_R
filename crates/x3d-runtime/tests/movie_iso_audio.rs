//! ISO/IEC 19775-1 18.4.2 MovieTexture: the sound of a decoded video follows the movie time (issue #90). The media is
//! `oracle/synthetic-movie-codecs/media/vp8-vorbis.webm` (0.6 s of mono audio, decoded natively), the clock is fake
//! (`Runtime::now`), and the sound is read from the audio graph: the playing AudioBufferSourceNode, its
//! `playbackRate` and the offset it was started at.
//!
//! A negative `speed` plays the sound backwards: the node's AudioBufferSourceNode gets the negative `playbackRate`
//! and starts at the end of the buffer, so the buffer is read backwards with no reversed copy.

use std::path::PathBuf;
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::nodes::movie_time::MovieTiming;
use x3d_runtime::{Fetcher, Runtime};

struct Files;

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        let name = url.path().trim_start_matches('/');
        let dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("../../oracle/synthetic-movie-codecs/media");

        std::fs::read(dir.join(name)).map_err(|_| "TypeError: Failed to fetch".to_string())
    }
}

fn movie(rt: &Runtime) -> NodeId {
    rt.scene.get_named_node(x3d_scene::ROOT, "M").unwrap()
}

/// The world is live at 100 s; the time fields and `loop` are set afterwards (see `movie_iso_time.rs`).
fn load(fields: &str) -> Runtime {
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
        "#X3D V4.0 utf8\nPROFILE Full\nSound {{ source DEF M MovieTexture {{ url \"vp8-vorbis.webm\" {fields} }} }}\n"
    );
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(Files));

    rt.set_movie_timing(MovieTiming::Iso);
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

fn set(rt: &mut Runtime, field: &str, value: FieldValue) {
    let k = rt.pre(movie(rt), field).unwrap();

    rt.set_value(k, value);
    rt.process_events();
    rt.frame();
}

/// What the playing sound is: (playbackRate, start offset, looping), or none when no buffer source plays.
fn playing(rt: &mut Runtime) -> Option<(f32, f64, bool)> {
    let nodes = rt.audio().graph().nodes;

    nodes.iter().rev().find_map(|n| {
        let s = n.source.as_ref()?;

        s.buffer?;

        if s.started.is_some() && s.stopped.is_none() {
            Some((n.playback_rate?, s.started?.1, s.loop_))
        } else {
            None
        }
    })
}

#[test]
fn a_positive_speed_plays_the_sound_forward_at_that_rate_from_the_start() {
    let mut rt = load("startTime 102 speed 2 loop TRUE");

    at(&mut rt, 102.1);

    let (rate, offset, looping) = playing(&mut rt).expect("a sound is playing");

    assert_eq!(rate, 2.0);
    assert_eq!(offset, 0.0);
    assert!(looping);
}

#[test]
fn a_negative_speed_plays_the_sound_backwards_from_the_end() {
    let mut rt = load("startTime 102 speed -1 loop TRUE");
    let duration = rt.get_f64(movie(&rt), "duration_changed");

    at(&mut rt, 102.1);

    let (rate, offset, _) = playing(&mut rt).expect("a sound is playing");

    assert_eq!(rate, -1.0);
    assert!((offset - duration).abs() < 1e-9, "{offset} vs {duration}");

    // speed -2 and a pitch of 0.5: the rate is speed times pitch, backwards.
    let mut rt = load("startTime 102 speed -2 pitch 0.5 loop TRUE");

    at(&mut rt, 102.1);
    assert_eq!(playing(&mut rt).unwrap().0, -1.0);
}

/// After a pause the sound resumes at the movie time of the pause, not at its own clock's reading (which counts
/// seconds, not movie time, once the rate is not 1).
#[test]
fn a_resume_continues_the_sound_at_the_movie_time_of_the_pause() {
    let mut rt = load("startTime 102 speed 0.5 loop TRUE");

    at(&mut rt, 102.4);
    set(&mut rt, "pauseTime", FieldValue::SFTime(102.5));
    at(&mut rt, 102.6);
    assert!(playing(&mut rt).is_none(), "paused: no sound plays");

    // The movie time of the last tick before the pause: 0.4 s of the clock at speed 0.5 (a pause leaves the frame
    // of the last tick, see docs/movie-texture-timing.md). The sound's own clock would say 0.4 or 0.5 here.
    let paused_at = rt.movie_current_time(movie(&rt)).unwrap();

    assert!((paused_at - 0.2).abs() < 0.02, "{paused_at}");
    set(&mut rt, "resumeTime", FieldValue::SFTime(103.0));
    at(&mut rt, 103.1);

    let (rate, offset, _) = playing(&mut rt).expect("the sound resumed");

    assert_eq!(rate, 0.5);
    assert!((offset - paused_at).abs() < 1e-9, "{offset} vs {paused_at}");
}

/// The pinned behavior is unchanged: a negative rate plays the sound at 1, forward.
#[test]
fn pinned_timing_still_plays_the_sound_of_a_negative_speed_at_one() {
    let text = "#X3D V4.0 utf8\nPROFILE Full\nSound { source DEF M MovieTexture { url \"vp8-vorbis.webm\" speed -1 loop TRUE startTime 1 } }\n";
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(Files));

    rt.set_movie_timing(MovieTiming::Pinned);
    rt.load_world();

    for _ in 0..10 {
        rt.run_tasks();
    }

    rt.live = true;
    rt.go_live();
    rt.frame();
    rt.frame();

    let (rate, offset, _) = playing(&mut rt).expect("a sound is playing");

    assert_eq!((rate, offset), (1.0, 0.0));
}

/// The claim behind the tests above, in the engine: an `AudioElement` with a negative `playbackRate`, started at the
/// end of its buffer, outputs the buffer backwards (a ramp comes out as a falling ramp), at `|rate|` frames per frame.
mod engine {
    use x3d_runtime::nodes::sound::AudioElement;
    use x3d_runtime::sound::{AudioEngine, AudioMode};

    const SR: usize = 48000;
    const SCALE: f32 = 1048576.0;

    /// Renders 0.25 s of a one-second ramp (sample i = (i + 1) / 2^20) played by an AudioElement at `rate` from `from`.
    fn render(rate: f64, from: f64, looping: bool) -> Vec<f32> {
        let mut e = AudioEngine::new(AudioMode::Offline {
            channels: 1,
            sample_rate: SR as f32,
            length: SR / 4,
        });
        let data: Vec<f32> = (0..SR).map(|i| (i + 1) as f32 / SCALE).collect();
        let buffer = e.create_buffer(1, SR, SR as f32, Some(&data));
        let sink = e.gain(1.0);
        let destination = e.destination;
        let mut element = AudioElement::create(&mut e, sink, Some(buffer));

        e.connect(sink, destination);
        element.set_loop(&mut e, looping);
        element.set_playback_rate(&mut e, rate);
        element.set_current_time(&mut e, from);
        element.play(&mut e);
        e.arm(&[]);
        e.finish().expect("rendered").get_channel_data(0).to_vec()
    }

    fn index(sample: f32) -> i64 {
        (sample * SCALE).round() as i64 - 1
    }

    #[test]
    fn a_negative_rate_reads_the_buffer_backwards_from_the_offset() {
        let out = render(-1.0, 1.0, false);
        let first = out.iter().position(|v| *v != 0.0).expect("sound");

        assert!(first < 2048, "starts at once: {first}");
        // Falling by one frame per frame, from the end of the buffer.
        assert!(index(out[first]) >= SR as i64 - 3, "{}", index(out[first]));

        for k in first..first + 2000 {
            assert_eq!(index(out[k]) - index(out[k + 1]), 1, "frame {k}");
        }

        let out = render(-2.0, 0.5, false);
        let first = out.iter().position(|v| *v != 0.0).expect("sound");

        assert!((index(out[first]) - SR as i64 / 2).abs() <= 4);

        for k in first..first + 2000 {
            assert_eq!(index(out[k]) - index(out[k + 1]), 2, "frame {k}");
        }
    }

    /// A looping buffer read backwards wraps from the start to the end.
    #[test]
    fn a_looping_negative_rate_wraps_to_the_end() {
        let out = render(-1.0, 0.1, true);
        let first = out.iter().position(|v| *v != 0.0).expect("sound");
        let at = first + (0.1 * SR as f64) as usize + 8;

        // 0.1 s (4800 frames) from the start the read position reaches the start and wraps to the end.
        let wrapped =
            (first..at.min(out.len() - 1)).find(|&k| index(out[k + 1]) > index(out[k]) + 100);

        assert!(wrapped.is_some(), "no wrap within the first {at} frames");
        assert!(index(out[wrapped.unwrap() + 1]) > SR as i64 - 16);
    }
}
