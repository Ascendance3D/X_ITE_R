//! MP4 video and audio through the platform decoder (ADR 0015): GStreamer on Linux (feature `system-video-gst`),
//! AVFoundation on macOS (feature `system-video-avf`).
//!
//! The files are the synthetic ones of `tests/data/mp4` (`oracle/tools/gen_movie_mp4.py`): segment k (0.25 s each, 8 per
//! play) shows the grey level `GREY[k]` and sounds a sine of `TONE[k]` Hz. The tests check the decode against that known
//! content (not against another decoder), and the synchronization on the engine's own audio output: the world is a
//! Sound node whose source is the MovieTexture, the audio context renders offline in step with the frames (as x3d-drive
//! does), and the tone in the rendered output at each moment must be the segment of the picture shown at that moment.
//!
//! Without a platform decoder for H.264 and AAC the tests call `x3d_evidence::skip ("no-platform-decoder")`: that
//! fails under the strict policy and is a named skip only where X3D_EVIDENCE=ci allows it. It never passes silently.

#![cfg(any(
    feature = "system-video-gst",
    all(feature = "system-video-avf", target_os = "macos")
))]

use std::path::PathBuf;
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::mp4probe::probe;
use x3d_runtime::nodes::LoadState;
use x3d_runtime::nodes::movie_time::MovieTiming;
use x3d_runtime::sound::{AudioMode, set_default_mode};
use x3d_runtime::sysvideo;
use x3d_runtime::video_types::{AudioTrack, WebmError};
use x3d_runtime::{Fetcher, Runtime};

const SEGMENT: f64 = 0.25;
const SEGMENTS: usize = 8;
const GREY: [u8; 8] = [16, 46, 76, 106, 136, 166, 196, 226];
const TONE: [f64; 8] = [300.0, 400.0, 500.0, 600.0, 700.0, 800.0, 900.0, 1000.0];
const RATE: f64 = 48000.0;
/// One picture period of the files (20 pictures a second): the synchronization tolerance (ADR 0015, Acceptance).
const PICTURE: f64 = 0.05;

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/mp4")
            .join(name),
    )
    .unwrap()
}

/// The decode of `name`, or a recorded skip when the platform has no decoder for it.
fn decode(name: &str) -> Option<sysvideo::Decoded> {
    let d = data(name);
    let info = probe(&d).unwrap();

    match sysvideo::decode_tracks(&d, &info, 512 << 20) {
        Ok(r) => Some(r),
        Err(e @ WebmError::Unsupported { .. }) => {
            x3d_evidence::skip("no-platform-decoder", None, format!("{name}: {e:?}"));
            None
        }
        Err(e) => panic!("{name}: {e:?}"),
    }
}

/// The segment of a grey level (RGB), or `None` when it is not within 3 levels of one.
fn grey_segment(v: u8) -> Option<usize> {
    GREY.iter()
        .position(|g| (i16::from(*g) - i16::from(v)).abs() <= 3)
}

/// The power of a tone in `s` (Goertzel).
fn power(s: &[f32], f: f64) -> f64 {
    let w = 2.0 * std::f64::consts::PI * f / RATE;
    let (mut a, mut b) = (0.0, 0.0);

    for &x in s {
        let c = f64::from(x) + 2.0 * w.cos() * a - b;
        b = a;
        a = c;
    }

    a * a + b * b - 2.0 * w.cos() * a * b
}

/// The segment whose tone `s` sounds, or `None` for silence.
fn tone_segment(s: &[f32]) -> Option<usize> {
    tone_segment_at(s, 1.0)
}

/// [`tone_segment`] for sound played at `rate`: its pitch is `rate` times the tone's (the port does not reproduce
/// `preservesPitch`, docs: MovieTexture.set_speed__).
fn tone_segment_at(s: &[f32], rate: f64) -> Option<usize> {
    let rms = (s.iter().map(|x| f64::from(*x).powi(2)).sum::<f64>() / s.len() as f64).sqrt();

    if rms < 0.02 {
        return None;
    }

    (0..SEGMENTS).max_by(|a, b| power(s, TONE[*a] * rate).total_cmp(&power(s, TONE[*b] * rate)))
}

#[test]
fn h264_and_aac_decode_to_the_known_pictures_and_tones() {
    let Some(r) = decode("h264-aac.mp4") else {
        return;
    };

    eprintln!(
        "h264-aac.mp4: decoders {:?}, {} x {}, {} pictures, matrix {:?}, full range {}",
        r.decoders,
        r.width,
        r.height,
        r.pictures.len(),
        r.matrix,
        r.full_range
    );
    assert_eq!((r.width, r.height, r.pictures.len()), (64, 48, 40));

    let (channels, rate, start) = r.audio.as_ref().unwrap();

    eprintln!(
        "  sound: {} channels, {rate} Hz, {} samples, first sample at {start} s",
        channels.len(),
        channels[0].len()
    );
    assert_eq!((channels.len(), *rate), (2, 48000.0));
    assert!(start.abs() < 1e-6);
    // 2 s of sound (the decoder delay of AAC is not presented: GstAudioClippingMeta).
    assert!((channels[0].len() as f64 / RATE - 2.0).abs() < 0.001);

    let t = sysvideo::timeline(r).unwrap();

    assert!((t.duration - 2.0).abs() < 0.001, "{}", t.duration);

    for (i, f) in t.video.frames.iter().enumerate() {
        // 5 pictures a segment, 50 ms apart.
        assert!(
            (f.time_ms - i as f64 * 50.0).abs() < 0.5,
            "{i}: {}",
            f.time_ms
        );
        assert!(f.rgba.chunks_exact(4).all(|p| {
            p[3] == 255 && (0..3).all(|c| (i16::from(p[c]) - i16::from(GREY[i / 5])).abs() <= 3)
        }));
    }

    let AudioTrack::Decoded { channels, .. } = t.audio else {
        panic!()
    };

    // Every 20 ms window that is not at a segment boundary sounds the tone of its segment.
    for w in 0..100 {
        let at = w as f64 * 0.02;
        let k = (at / SEGMENT) as usize;

        if (at + 0.02 - (k + 1) as f64 * SEGMENT).abs() < 0.02 || at - k as f64 * SEGMENT < 0.005 {
            continue;
        }

        let s = &channels[0][(at * RATE) as usize..((at + 0.02) * RATE) as usize];

        assert_eq!(tone_segment(s), Some(k), "window at {at}");
    }
}

#[test]
fn hevc_decodes_too_and_av1_names_its_codec_when_it_does_not() {
    if let Some(r) = decode("hevc-aac.mp4") {
        eprintln!("hevc-aac.mp4: decoders {:?}", r.decoders);
        assert_eq!(r.pictures.len(), 40);
        assert!(r.audio.is_some());
    }

    let d = data("av1-aac.mp4");

    match sysvideo::decode_tracks(&d, &probe(&d).unwrap(), 512 << 20) {
        Ok(r) => {
            eprintln!("av1-aac.mp4: decoders {:?}", r.decoders);
            assert_eq!(r.pictures.len(), 40);
        }
        Err(WebmError::Unsupported { container, codec }) => {
            eprintln!("av1-aac.mp4: unsupported here: {codec}");
            assert_eq!(container, "mp4");
            assert!(codec.starts_with("av01+mp4a"), "{codec}");
        }
        Err(e) => panic!("{e:?}"),
    }
}

#[test]
fn audio_starting_late_is_silent_first_and_on_the_same_timeline() {
    let Some(r) = decode("h264-aac-audio-late.mp4") else {
        return;
    };
    let start = r.audio.as_ref().unwrap().2;

    eprintln!("h264-aac-audio-late.mp4: first sample at {start} s (ffprobe: 0.228)");
    assert!((start - 0.228).abs() < 0.002, "{start}");

    let t = sysvideo::timeline(r).unwrap();
    let AudioTrack::Decoded { channels, .. } = t.audio else {
        panic!()
    };

    assert!((t.duration - 2.0).abs() < 0.001);
    // 2 s of video; the sound is 96001 samples (2.00002 s), so the timeline is that long.
    assert!(
        (channels[0].len() as i64 - 96000).abs() <= 1,
        "{}",
        channels[0].len()
    );
    assert!(
        channels[0][..(0.2 * RATE) as usize]
            .iter()
            .all(|s| *s == 0.0)
    );
    // From 0.3 s on, the tones are those of the full file: the same movie time sounds the same segment.
    assert_eq!(
        tone_segment(&channels[0][(0.3 * RATE) as usize..(0.32 * RATE) as usize]),
        Some(1)
    );
    assert_eq!(
        tone_segment(&channels[0][(1.6 * RATE) as usize..(1.62 * RATE) as usize]),
        Some(6)
    );
}

#[test]
fn video_only_and_audio_only_files_have_one_track() {
    if let Some(r) = decode("h264-only.mp4") {
        assert!(r.audio.is_none());
        assert_eq!(r.pictures.len(), 40);
    }

    if let Some(r) = decode("aac-only.m4a") {
        assert!(r.pictures.is_empty());

        let t = sysvideo::timeline(r).unwrap();

        assert!((t.duration - 2.0).abs() < 0.001);
        assert!(matches!(t.audio, AudioTrack::Decoded { .. }));
    }
}

#[test]
fn the_limit_counts_pictures_and_sound_and_nothing_is_left_behind() {
    let d = data("h264-aac.mp4");
    let info = probe(&d).unwrap();
    let pictures = 40 * 64 * 48 * 4;

    // The pictures alone fit; with the sound they do not.
    assert!(matches!(
        sysvideo::decode_tracks(&d, &info, pictures + 1000),
        Err(WebmError::TooLarge)
    ));

    let a = data("aac-only.m4a");

    assert!(matches!(
        sysvideo::decode_tracks(&a, &probe(&a).unwrap(), 100_000),
        Err(WebmError::TooLarge)
    ));

    // No temporary file of this process stays.
    let mine = format!("x3d-mp4-{}-", std::process::id());
    let left: Vec<_> = std::fs::read_dir(std::env::temp_dir())
        .unwrap()
        .flatten()
        .filter(|e| e.file_name().to_string_lossy().starts_with(&mine))
        .collect();

    assert!(left.is_empty(), "{left:?}");
}

fn crafted_unknown() -> Vec<u8> {
    fn b(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
        let mut v = ((body.len() + 8) as u32).to_be_bytes().to_vec();
        v.extend_from_slice(kind);
        v.extend_from_slice(body);
        v
    }
    let mut entry = vec![0; 24];
    entry.extend_from_slice(&32u16.to_be_bytes());
    entry.extend_from_slice(&16u16.to_be_bytes());
    entry.extend_from_slice(&[0; 50]);
    let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
    stsd.extend(b(b"xyz1", &entry));
    let mut mdhd = vec![0; 12];
    mdhd.extend_from_slice(&1000u32.to_be_bytes());
    mdhd.extend_from_slice(&1500u32.to_be_bytes());
    mdhd.extend_from_slice(&[0; 4]);
    let mut hdlr = vec![0; 8];
    hdlr.extend_from_slice(b"vide");
    hdlr.extend_from_slice(&[0; 13]);
    let minf = b(b"minf", &b(b"stbl", &b(b"stsd", &stsd)));
    let mdia = b(
        b"mdia",
        &[b(b"mdhd", &mdhd), b(b"hdlr", &hdlr), minf].concat(),
    );

    [
        b(b"ftyp", b"isom\0\0\0\0isom"),
        b(b"moov", &b(b"trak", &mdia)),
    ]
    .concat()
}

// --- the runtime: one media session, audible through a Sound node --------------------------------------------------

/// Serves the fixtures by name.
struct Files(Vec<(String, Vec<u8>)>);

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        let name = url.path().trim_start_matches('/');

        if let Some((_, b)) = self.0.iter().find(|(n, _)| n == name) {
            return Ok(b.clone());
        }

        std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("tests/data/mp4")
                .join(name),
        )
        .map_err(|_| "TypeError: Failed to fetch".into())
    }
}

/// 640 samples (5 render quanta) a frame: the audio context and the frames step together exactly.
const QUANTA: usize = 5;
const DT: f64 = (QUANTA * 128) as f64 / RATE;
const T0: f64 = 100.0;

/// What happens at a frame.
enum Do {
    Set(&'static str, FieldValue),
    /// `mediaElement .currentTime = t` (pinned) or the ISO seek.
    Seek(f64),
}

struct Run {
    rt: Runtime,
    /// Per frame: the context time the frame's sound starts at, and the segment of the picture shown.
    pictures: Vec<(f64, Option<usize>)>,
    sound: Vec<f32>,
}

fn movie(rt: &Runtime) -> NodeId {
    rt.scene.get_named_node(x3d_scene::ROOT, "M").unwrap()
}

/// Runs `file` for `frames` frames, with `events` at their frames. The MovieTexture is on a Shape and is the source of a
/// Sound node that is heard everywhere at full gain.
fn run(
    file: &str,
    timing: MovieTiming,
    fields: &str,
    frames: usize,
    events: Vec<(usize, Do)>,
) -> Run {
    set_default_mode(AudioMode::Offline {
        channels: 2,
        sample_rate: RATE as f32,
        length: (frames + 4) * QUANTA * 128,
    });

    let text = format!(
        "#X3D V4.0 utf8\nPROFILE Full\n\
         Shape {{ appearance Appearance {{ texture DEF M MovieTexture {{ url \"{file}\" {fields} }} }} geometry Box {{ }} }}\n\
         DEF S Sound {{ spatialize FALSE minFront 1000 minBack 1000 maxFront 1000 maxBack 1000 source USE M }}\n"
    );
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(Files(vec![])));

    rt.set_movie_timing(timing);
    rt.now = T0;
    rt.load_world();

    for _ in 0..10 {
        rt.run_tasks();
    }

    let quanta: Vec<usize> = (1..=frames).map(|k| k * QUANTA).collect();

    rt.audio().arm(&quanta);
    rt.live = true;
    rt.go_live();

    let mut pictures = vec![];

    for k in 1..=frames {
        rt.audio().wait();
        rt.now = T0 + k as f64 * DT;
        // A future startTime, pauseTime … is a timeout (X3DTimeDependentNode, setTimeout).
        rt.fire_timeouts(rt.now);

        for (_, e) in events.iter().filter(|(f, _)| *f == k) {
            let m = movie(&rt);

            match e {
                Do::Set(name, value) => {
                    let key = rt.pre(m, name).unwrap();

                    rt.set_value(key, value.clone());
                    rt.process_events();
                }
                Do::Seek(t) => {
                    rt.movie_seek(m, *t);
                }
            }
        }

        rt.frame();
        // The viewer's DISPLAY traversal of the Sound node (SpatialSound.traverse): the listener at the sound.
        let sound = rt.scene.get_named_node(x3d_scene::ROOT, "S").unwrap();

        rt.sound_traverse(sound, &x3d_fields::math::Matrix4::IDENTITY);

        let segment = rt
            .movie_texture_frame(movie(&rt))
            .and_then(|f| grey_segment(f.2[0]));

        pictures.push((k as f64 * DT, segment));
        rt.audio().resume();
    }

    let sound = rt
        .audio()
        .finish()
        .expect("rendered")
        .get_channel_data(0)
        .to_vec();

    Run {
        rt,
        pictures,
        sound,
    }
}

/// The synchronization of a run: every 20 ms window of the output, 5 ms apart, whose time is more than one picture
/// period away from a change of the shown segment must sound the shown segment (or be silent while the picture does not
/// move). Returns (windows compared, the largest lag in seconds between a picture change and the matching tone change).
fn check_sync(r: &Run, label: &str) -> (usize, f64) {
    check_sync_at(r, label, 1.0)
}

/// [`check_sync`] for a movie played at `rate`.
fn check_sync_at(r: &Run, label: &str, rate: f64) -> (usize, f64) {
    let win = (0.02 * RATE) as usize;
    let shown = |t: f64| {
        r.pictures
            .iter()
            .take_while(|(c, _)| *c <= t)
            .last()
            .and_then(|p| p.1)
    };
    let changes: Vec<f64> = r
        .pictures
        .windows(2)
        .filter(|w| w[0].1 != w[1].1)
        .map(|w| w[1].0)
        .collect();
    let mut compared = 0;
    let mut tones = vec![];

    for start in (0..r.sound.len().saturating_sub(win)).step_by(240) {
        let mid = (start + win / 2) as f64 / RATE;
        let tone = tone_segment_at(&r.sound[start..start + win], rate);

        tones.push((mid, tone));

        if changes.iter().any(|c| (c - mid).abs() <= PICTURE) || mid < r.pictures[0].0 + PICTURE {
            continue;
        }

        let Some(picture) = shown(mid) else {
            continue;
        };

        if tone.is_none() {
            // Silence is allowed only while the picture stands (a pause).
            continue;
        }

        compared += 1;
        assert_eq!(
            tone,
            Some(picture),
            "{label}: at {mid:.3} s the picture is segment {picture}, the sound {tone:?}"
        );
    }

    // The lag of each picture change: the nearest tone change to the same segment.
    let mut lag: f64 = 0.0;

    for w in r.pictures.windows(2).filter(|w| w[0].1 != w[1].1) {
        let (t, to) = (w[1].0, w[1].1);
        let near = tones
            .windows(2)
            .filter(|p| p[0].1 != p[1].1 && p[1].1 == to && to.is_some())
            .map(|p| p[1].0 - t)
            .min_by(|a, b| a.abs().total_cmp(&b.abs()));

        if let Some(d) = near
            && d.abs() < 0.2
        {
            lag = lag.max(d.abs());
        }
    }

    eprintln!(
        "{label}: {compared} windows compared, largest lag {lag:.4} s, resyncs {:?}",
        r.rt.movie_audio_resyncs(movie(&r.rt))
    );
    assert!(compared > 50, "{label}: {compared}");
    assert!(lag <= PICTURE, "{label}: lag {lag}");
    (compared, lag)
}

fn have_decoder() -> bool {
    decode("h264-aac.mp4").is_some()
}

#[test]
fn the_movie_loads_with_its_sound_and_plays_both_in_step_and_loops() {
    if !have_decoder() {
        return;
    }

    for timing in [MovieTiming::Iso, MovieTiming::Pinned] {
        let r = run("h264-aac.mp4", timing, "loop TRUE", 330, vec![]);
        let m = movie(&r.rt);

        assert_eq!(
            r.rt.url_load_state(m),
            LoadState::Complete,
            "{:?}",
            r.rt.log
        );
        assert!(r.rt.get_bool(m, "hasSound"));
        assert!((r.rt.get_f64(m, "duration_changed") - 2.0).abs() < 0.001);
        assert!(matches!(
            r.rt.pre_value(m, "width"),
            Some(FieldValue::SFInt32(64))
        ));
        // 330 frames = 4.4 s: two loop wraps.
        check_sync(&r, &format!("{timing:?} loop"));
    }
}

#[test]
fn pause_and_resume_keep_both_tracks_together() {
    if !have_decoder() {
        return;
    }

    for timing in [MovieTiming::Iso, MovieTiming::Pinned] {
        let pause = T0 + 45.0 * DT;
        let resume = T0 + 90.0 * DT;
        let r = run(
            "h264-aac.mp4",
            timing,
            "loop TRUE",
            200,
            vec![
                (45, Do::Set("pauseTime", FieldValue::SFTime(pause))),
                (90, Do::Set("resumeTime", FieldValue::SFTime(resume))),
            ],
        );

        check_sync(&r, &format!("{timing:?} pause"));

        // While paused (frames 50 to 85) the picture stands and the sound is silent.
        let still: Vec<_> = r.pictures[50..85].iter().map(|p| p.1).collect();

        assert!(still.windows(2).all(|w| w[0] == w[1]), "{still:?}");

        let a = (r.pictures[52].0 * RATE) as usize;
        let b = (r.pictures[84].0 * RATE) as usize;

        assert!(
            tone_segment(&r.sound[a..b]).is_none() || r.sound[a..b].iter().all(|s| s.abs() < 0.02)
        );
    }
}

#[test]
fn seeks_forward_and_back_move_both_tracks() {
    if !have_decoder() {
        return;
    }

    for timing in [MovieTiming::Iso, MovieTiming::Pinned] {
        let r = run(
            "h264-aac.mp4",
            timing,
            "loop TRUE",
            200,
            vec![(30, Do::Seek(1.5)), (100, Do::Seek(0.3))],
        );

        check_sync(&r, &format!("{timing:?} seek"));
        // After the forward seek the picture is in segment 6, after the backward one in segment 1.
        assert_eq!(r.pictures[35].1, Some(6));
        assert_eq!(r.pictures[103].1, Some(1));
    }
}

#[test]
fn rates_half_and_two_play_both_tracks_at_that_rate() {
    if !have_decoder() {
        return;
    }

    for (timing, speed, frames) in [
        (MovieTiming::Iso, 2.0, 200),
        (MovieTiming::Iso, 0.5, 330),
        (MovieTiming::Pinned, 2.0, 200),
        (MovieTiming::Pinned, 0.5, 330),
    ] {
        let r = run(
            "h264-aac.mp4",
            timing,
            &format!("loop TRUE speed {speed}"),
            frames,
            vec![],
        );
        check_sync_at(&r, &format!("{timing:?} speed {speed}"), speed);
        // At speed 2 a segment lasts 0.125 s; at 0.5 it lasts 0.5 s.
        let first_change =
            r.pictures.windows(2).find(|w| w[0].1 != w[1].1).unwrap()[1].0 - r.pictures[0].0;

        assert!(
            (first_change - SEGMENT / speed).abs() <= 2.0 * DT,
            "{timing:?} {speed}: {first_change}"
        );
    }
}

#[test]
fn late_audio_video_only_and_audio_only_play_on_the_timeline() {
    if !have_decoder() {
        return;
    }

    let r = run(
        "h264-aac-audio-late.mp4",
        MovieTiming::Iso,
        "loop TRUE",
        200,
        vec![],
    );

    check_sync(&r, "late audio");

    let r = run("h264-only.mp4", MovieTiming::Iso, "loop TRUE", 60, vec![]);
    let m = movie(&r.rt);

    assert_eq!(r.rt.url_load_state(m), LoadState::Complete);
    assert!(!r.rt.get_bool(m, "hasSound"));
    assert!(r.sound.iter().all(|s| *s == 0.0));

    let r = run("aac-only.m4a", MovieTiming::Iso, "loop TRUE", 200, vec![]);
    let m = movie(&r.rt);

    assert_eq!(
        r.rt.url_load_state(m),
        LoadState::Complete,
        "{:?}",
        r.rt.log
    );
    assert!(r.rt.get_bool(m, "hasSound"));
    assert!(matches!(
        r.rt.pre_value(m, "width"),
        Some(FieldValue::SFInt32(0))
    ));
    assert!(r.rt.movie_texture_frame(m).is_none());
    // The tones follow the movie time: segment k from k * 0.25 s after the start.
    let start = (r.pictures[0].0 * RATE) as usize;
    let at = |t: f64| start + (t * RATE) as usize;

    assert_eq!(tone_segment(&r.sound[at(0.1)..at(0.12)]), Some(0));
    assert_eq!(tone_segment(&r.sound[at(1.1)..at(1.12)]), Some(4));
}

#[test]
fn an_unknown_codec_fails_with_the_pinned_messages_and_unload_releases_the_media() {
    if !have_decoder() {
        return;
    }

    let mut rt = {
        let text = "#X3D V4.0 utf8\nPROFILE Full\nShape { appearance Appearance { texture DEF M MovieTexture { url \"x.mp4\" } } geometry Box { } }\n";
        let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
        let mut rt = Runtime::new(
            scene,
            Box::new(Files(vec![("x.mp4".into(), crafted_unknown())])),
        );

        rt.load_world();
        for _ in 0..10 {
            rt.run_tasks();
        }
        rt
    };
    let m = movie(&rt);

    assert_eq!(rt.url_load_state(m), LoadState::Failed, "{:?}", rt.log);
    assert!(
        rt.log.iter().any(|l| l.contains("codec: \"xyz1")),
        "{:?}",
        rt.log
    );
    assert!(
        rt.log
            .iter()
            .any(|l| l.contains("Couldn't load video 'http://oracle/x.mp4': error."))
    );

    // A good file, then an empty url: the media and its sound are released.
    let key = rt.pre(m, "url").unwrap();

    rt.set_value(key, FieldValue::MFString(vec!["h264-aac.mp4".into()]));
    rt.process_events();
    for _ in 0..10 {
        rt.run_tasks();
    }
    assert_eq!(rt.url_load_state(m), LoadState::Complete, "{:?}", rt.log);
    assert!(rt.movie_has_frames(m));

    rt.set_value(key, FieldValue::MFString(vec![]));
    rt.process_events();
    for _ in 0..10 {
        rt.run_tasks();
    }
    assert!(!rt.movie_has_frames(m));
    assert!(!rt.get_bool(m, "hasSound"));
}

#[test]
fn a_drift_of_the_sound_is_moved_back_to_the_movie_time() {
    if !have_decoder() {
        return;
    }

    // Pinned timing resumes X_ITE's AudioElement at its own time, the context time since the start without the rate. At
    // speed 2 that is half the movie time, so after the resume the sound is behind the picture until the engine moves it
    // back (ADR 0015, 4).
    let r = run(
        "h264-aac.mp4",
        MovieTiming::Pinned,
        "loop TRUE speed 2",
        200,
        vec![
            (40, Do::Set("pauseTime", FieldValue::SFTime(T0 + 40.0 * DT))),
            (
                60,
                Do::Set("resumeTime", FieldValue::SFTime(T0 + 60.0 * DT)),
            ),
        ],
    );
    let resyncs = r.rt.movie_audio_resyncs(movie(&r.rt)).unwrap();

    check_sync_at(&r, "Pinned speed 2 pause", 2.0);
    assert!(resyncs >= 1, "{resyncs}");
}

/// The movie sound on the real output device (`tools/mp4-audible.sh` records it from the PipeWire monitor and finds the
/// tones). The test plays `h264-aac.mp4` once through a Sound node on the realtime AudioContext, with the frames on the
/// wall clock, and checks that the device clock ran and that the pictures went through all 8 segments. What it cannot
/// show alone: that the sound left the engine. The recording shows that.
#[test]
#[ignore = "needs an audio output device: run tools/mp4-audible.sh"]
fn realtime_output_plays_the_movie_sound() {
    if !have_decoder() {
        return;
    }

    set_default_mode(AudioMode::Realtime);

    let text = "#X3D V4.0 utf8\nPROFILE Full\n\
         Shape { appearance Appearance { texture DEF M MovieTexture { url \"h264-aac.mp4\" } } geometry Box { } }\n\
         DEF S Sound { spatialize FALSE minFront 1000 minBack 1000 maxFront 1000 maxBack 1000 source USE M }\n";
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(Files(vec![])));

    if !rt.audio().is_realtime() {
        x3d_evidence::skip(
            "no-audio-device",
            None,
            "the engine fell back to a silent context".to_string(),
        );
        return;
    }

    rt.now = T0;
    rt.load_world();

    for _ in 0..10 {
        rt.run_tasks();
    }

    let m = movie(&rt);
    let sound = rt.scene.get_named_node(x3d_scene::ROOT, "S").unwrap();

    assert_eq!(rt.url_load_state(m), LoadState::Complete, "{:?}", rt.log);

    let start = std::time::Instant::now();
    let clock = rt.audio().current_time();
    let mut seen = std::collections::BTreeSet::new();

    rt.live = true;
    rt.go_live();
    // Without loop the movie plays once from a startTime event (X3DTimeDependentNode.set_startTime).
    let key = rt.pre(m, "startTime").unwrap();

    rt.set_value(key, FieldValue::SFTime(T0));
    rt.process_events();
    eprintln!("MP4-AUDIBLE start");

    while start.elapsed().as_secs_f64() < 3.0 {
        rt.now = T0 + start.elapsed().as_secs_f64();
        rt.fire_timeouts(rt.now);
        rt.frame();
        rt.sound_traverse(sound, &x3d_fields::math::Matrix4::IDENTITY);

        if let Some(s) = rt.movie_texture_frame(m).and_then(|f| grey_segment(f.2[0])) {
            seen.insert(s);
        }

        std::thread::sleep(std::time::Duration::from_millis(10));
    }

    let ran = rt.audio().current_time() - clock;

    eprintln!("MP4-AUDIBLE end: context clock ran {ran:.3} s, segments shown {seen:?}");
    assert!(ran > 2.5, "{ran}");
    assert_eq!(seen.len(), SEGMENTS);
}
