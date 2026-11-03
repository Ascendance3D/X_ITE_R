//! MovieTexture video sound, seeking and bad media (issue #90, BR1.2, BR1.3, BR1.8). The media are the synthetic
//! files of `oracle/synthetic-movie-codecs` (`oracle/tools/gen_movie_codecs.py`) and `oracle/synthetic-movie-anim`.
//!
//! Chrome measurements behind the expectations (pinned oracle Chrome 156, a `video` element):
//! - `currentTime = -5` gives 0 and `currentTime = 100` gives the duration (clamped).
//! - `hasSound` is `webkitAudioDecodedByteCount > 0` at `loadeddata`: true for a file with an audio track, false for
//!   a silent one.

use std::path::PathBuf;
use x3d_fields::NodeId;
use x3d_runtime::{Fetcher, Runtime};

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

/// Serves a file by its name from the media directories, or the bytes registered under a name.
struct Files(Vec<(String, Vec<u8>)>);

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        let name = url.path().trim_start_matches('/');

        if let Some((_, b)) = self.0.iter().find(|(n, _)| n == name) {
            return Ok(b.clone());
        }

        for dir in ["synthetic-movie-codecs/media", "synthetic-movie-anim/media"] {
            if let Ok(b) = std::fs::read(oracle().join(dir).join(name)) {
                return Ok(b);
            }
        }

        Err("TypeError: Failed to fetch".into())
    }
}

fn load(body: &str, extra: Vec<(String, Vec<u8>)>) -> Runtime {
    let text = format!("#X3D V4.0 utf8\nPROFILE Full\n{body}");
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(Files(extra)));

    // These cases are the pinned X_ITE 16.5.0 behavior (docs/movie-texture-timing.md), not the ISO default.
    rt.set_movie_timing(x3d_runtime::nodes::movie_time::MovieTiming::Pinned);

    rt.load_world();

    for _ in 0..10 {
        rt.run_tasks();
    }

    // The browser goes live; the frames start the node (startTime is in the past).
    rt.live = true;
    rt.go_live();
    for _ in 0..2 {
        rt.frame();
    }

    rt
}

fn movie(rt: &Runtime) -> NodeId {
    rt.scene.get_named_node(x3d_scene::ROOT, "M").unwrap()
}

/// The AudioBufferSourceNodes of the engine that have a buffer: (channels, frames, rate) and whether `start` ran
/// without a `stop`.
fn playing_buffers(rt: &mut Runtime) -> Vec<((usize, usize, f32), bool)> {
    rt.audio()
        .graph()
        .nodes
        .iter()
        .filter_map(|n| {
            let s = n.source.as_ref()?;

            Some((s.buffer?, s.started.is_some() && s.stopped.is_none()))
        })
        .collect()
}

const MOVIE: &str = "DEF M MovieTexture { url \"%\" loop TRUE startTime 1 }";

fn world(file: &str) -> String {
    format!("Sound {{ source {} }}", MOVIE.replace('%', file))
}

#[test]
fn a_webm_with_a_vorbis_track_plays_its_sound_through_the_node() {
    for file in ["vp8-vorbis.webm", "vp9-vorbis.webm"] {
        let mut rt = load(&world(file), vec![]);
        let m = movie(&rt);

        assert!(rt.get_bool(m, "hasSound"), "{file}");
        assert_eq!(rt.get_f64(m, "width"), 64.0, "{file}");
        // 0.6 s of 16 kHz audio, resampled to the context's rate (48 kHz): 28800 frames, mono, started.
        let buffers = playing_buffers(&mut rt);

        assert!(
            buffers.contains(&((1, 28800, 48000.0), true)),
            "{file}: {buffers:?}"
        );
        assert!(!rt.log.iter().any(|l| l.contains("audio track")), "{file}");
    }
}

#[test]
fn a_silent_video_keeps_rendering_without_sound_and_without_a_diagnostic() {
    let mut rt = load(&world("vp8-testsrc.webm"), vec![]);
    let m = movie(&rt);

    // BR1.2: no audio track is not bad media.
    assert!(!rt.get_bool(m, "hasSound"));
    assert_eq!(rt.get_f64(m, "width"), 96.0);
    assert!(rt.movie_has_frames(m));
    assert!(playing_buffers(&mut rt).is_empty());
    assert!(
        rt.log.iter().all(|l| !l.contains("MovieTexture")),
        "{:?}",
        rt.log
    );
}

#[test]
fn an_opus_track_is_reported_and_the_video_keeps_rendering() {
    // No Opus decoder is admitted (BR1.7): hasSound is true as the page's would be, the video plays silently and the
    // log says why (BR1.8).
    let mut rt = load(&world("vp8-opus.webm"), vec![]);
    let m = movie(&rt);

    assert!(rt.get_bool(m, "hasSound"));
    assert!(rt.movie_has_frames(m));
    assert!(playing_buffers(&mut rt).is_empty());
    assert!(
        rt.log
            .iter()
            .any(|l| l.contains("A_OPUS") && l.contains("silently")),
        "{:?}",
        rt.log
    );
}

#[test]
fn a_corrupt_stream_logs_and_the_scene_keeps_running() {
    // BR1.8: not a WebM, a cut file and mutated files: an error in the log, no frame, no panic, FAILED.
    let good =
        std::fs::read(oracle().join("synthetic-movie-codecs/media/vp8-vorbis.webm")).unwrap();
    let mut cases = vec![
        b"this is not a video".to_vec(),
        good[..good.len() / 3].to_vec(),
    ];

    for i in 0..12usize {
        let mut d = good.clone();
        let at = (i * 7919) % d.len();

        d[at] ^= 0xFF;
        d[(at + 1000) % good.len()] = 0;
        cases.push(d);
    }

    for (i, bytes) in cases.into_iter().enumerate() {
        let name = format!("bad{i}.webm");
        let mut rt = load(&world(&name), vec![(name.clone(), bytes)]);
        let m = movie(&rt);

        // The scene goes on: more frames run without a panic.
        for _ in 0..3 {
            rt.frame();
        }

        // Either the file still decodes (a flipped sample), or the node has no frame and says so.
        if !rt.movie_has_frames(m) {
            assert!(
                rt.log.iter().any(|l| l.contains("Error loading movie")),
                "{name}: {:?}",
                rt.log
            );
        }
    }
}

#[test]
fn seeking_clamps_to_the_duration_like_the_browser_and_moves_the_sound() {
    let mut rt = load(&world("vp8-vorbis.webm"), vec![]);
    let m = movie(&rt);
    let duration = rt.get_f64(m, "duration_changed");

    assert!((duration - 0.6).abs() < 0.11, "{duration}");
    assert_eq!(rt.movie_seek(m, -5.0), Some(0.0));
    assert_eq!(rt.movie_seek(m, 100.0), Some(duration));
    assert_eq!(rt.movie_seek(m, 0.25), Some(0.25));
    assert!((rt.movie_current_time(m).unwrap() - 0.25).abs() < 1e-9);
    // The sound plays from the new position.
    let started = rt
        .audio()
        .graph()
        .nodes
        .iter()
        .filter_map(|n| n.source.as_ref()?.started)
        .map(|(_, offset)| offset)
        .collect::<Vec<_>>();

    assert!(
        started.iter().any(|o| (o - 0.25).abs() < 1e-6),
        "{started:?}"
    );
}

#[test]
fn a_media_that_cannot_seek_ignores_the_request_and_logs_it() {
    // BR1.3: GifMedia ignores currentTime; a NaN is no time at all.
    let mut rt = load(
        "DEF M MovieTexture { url \"anim4.gif\" loop TRUE startTime 1 }",
        vec![],
    );
    let m = movie(&rt);

    assert!(rt.movie_has_frames(m));
    assert_eq!(rt.movie_seek(m, 0.5), None);
    assert!(
        rt.log.iter().any(|l| l.contains("cannot seek")),
        "{:?}",
        rt.log
    );

    let mut rt = load(&world("vp8-vorbis.webm"), vec![]);
    let m = movie(&rt);

    assert_eq!(rt.movie_seek(m, f64::NAN), None);
}

/// Playback timing against the pinned browser (BR1.4, FR1.5). `oracle/movie-timing-baseline/vp8-ramp.json` is which
/// picture of `vp8-ramp.webm` (eight pictures at 10 per second, uniform gray of luma 16 + 28 k) headless Chrome 156
/// presents at which media time, three identical runs (`oracle/chrome-video-timing.mjs`). The tolerance is one frame
/// period of the stream, 100 ms, that is one picture index, and it was fixed here before the first run. For every
/// presented picture the native clock (`current_frame` at that elapsed time, speed 1) must show the picture the
/// browser shows or the one next to it.
#[test]
fn native_playback_timing_is_within_one_frame_period_of_the_browser() {
    use x3d_runtime::nodes::movie_texture::MovieMedia;

    const FRAME_PERIOD_PICTURES: i64 = 1;

    let reference: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(oracle().join("movie-timing-baseline/vp8-ramp.json")).unwrap(),
    )
    .unwrap();
    let pictures = reference["pictures"].as_array().unwrap();
    let webm = std::fs::read(oracle().join("synthetic-movie-codecs/media/vp8-ramp.webm")).unwrap();
    let media = MovieMedia::decode_webm(&webm).unwrap();

    assert!(
        pictures.len() >= 7,
        "the reference has {} pictures",
        pictures.len()
    );
    assert_eq!(media.frames.len(), 8);
    assert!(
        (media.duration() - 0.8).abs() < 1e-9,
        "{}",
        media.duration()
    );

    // Picture k is the gray the stream codes (luma 16 + 28 k, BT.601 full range 32.6 k, within 2).
    for (k, f) in media.frames.iter().enumerate() {
        let want = 32.6 * k as f64;

        assert!((f[0] as f64 - want).abs() < 2.5, "picture {k}: {}", f[0]);
    }

    let mut worst = 0;

    for p in pictures {
        let t = p["mediaTime"].as_f64().unwrap();
        let browser = p["index"].as_i64().unwrap();
        let native = media.current_frame(t).unwrap() as i64;

        worst = worst.max((native - browser).abs());
        assert!(
            (native - browser).abs() <= FRAME_PERIOD_PICTURES,
            "at {t} s the browser shows picture {browser}, the native clock {native}"
        );
    }

    eprintln!("worst difference: {worst} picture(s) of 0.1 s");
}
