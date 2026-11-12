//! The MP4 container probe and the MovieTexture failure path for MP4 (ADR 0015, decisions 1 and 7). The files are
//! the synthetic ones of `tests/data/mp4` (`oracle/tools/gen_movie_mp4.py`); the damaged and unknown-codec files are
//! made here.

use std::path::PathBuf;
use x3d_fields::NodeId;
use x3d_runtime::mp4probe::{Mp4Error, TrackKind, probe};
use x3d_runtime::nodes::LoadState;
use x3d_runtime::{Fetcher, Runtime};

fn data(name: &str) -> Vec<u8> {
    std::fs::read(
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("tests/data/mp4")
            .join(name),
    )
    .unwrap()
}

fn mp4_box(kind: &[u8; 4], body: &[u8]) -> Vec<u8> {
    let mut b = ((body.len() + 8) as u32).to_be_bytes().to_vec();

    b.extend_from_slice(kind);
    b.extend_from_slice(body);
    b
}

/// A minimal MP4: one video track whose sample entry has the code `codec` and the size 32 x 16.
fn crafted(codec: &[u8; 4]) -> Vec<u8> {
    let mut entry = vec![0; 24];

    entry.extend_from_slice(&32u16.to_be_bytes());
    entry.extend_from_slice(&16u16.to_be_bytes());
    entry.extend_from_slice(&[0; 50]);

    let mut stsd = vec![0, 0, 0, 0, 0, 0, 0, 1];
    stsd.extend(mp4_box(codec, &entry));

    let stbl = mp4_box(b"stbl", &mp4_box(b"stsd", &stsd));
    let minf = mp4_box(b"minf", &stbl);
    let mut mdhd = vec![0; 12];
    mdhd.extend_from_slice(&1000u32.to_be_bytes());
    mdhd.extend_from_slice(&1500u32.to_be_bytes());
    mdhd.extend_from_slice(&[0; 4]);
    let mut hdlr = vec![0; 8];
    hdlr.extend_from_slice(b"vide");
    hdlr.extend_from_slice(&[0; 13]);

    let mdia = mp4_box(
        b"mdia",
        &[mp4_box(b"mdhd", &mdhd), mp4_box(b"hdlr", &hdlr), minf].concat(),
    );
    let moov = mp4_box(b"moov", &mp4_box(b"trak", &mdia));

    [mp4_box(b"ftyp", b"isom\0\0\0\0isomiso2"), moov].concat()
}

#[test]
fn the_probe_names_the_codec_size_and_rate_of_each_track() {
    // (file, video (code, w, h), audio (code, channels, rate))
    let cases: [(
        &str,
        Option<(&[u8; 4], u16, u16)>,
        Option<(&[u8; 4], u16, u32)>,
    ); 6] = [
        (
            "h264-aac.mp4",
            Some((b"avc1", 64, 48)),
            Some((b"mp4a", 2, 48000)),
        ),
        ("h264-only.mp4", Some((b"avc1", 64, 48)), None),
        ("aac-only.m4a", None, Some((b"mp4a", 2, 48000))),
        (
            "h264-aac-audio-late.mp4",
            Some((b"avc1", 64, 48)),
            Some((b"mp4a", 2, 48000)),
        ),
        (
            "hevc-aac.mp4",
            Some((b"hvc1", 64, 48)),
            Some((b"mp4a", 2, 48000)),
        ),
        (
            "av1-aac.mp4",
            Some((b"av01", 64, 48)),
            Some((b"mp4a", 2, 48000)),
        ),
    ];

    for (name, video, audio) in cases {
        let info = probe(&data(name)).unwrap();

        assert!(!info.fragmented, "{name}");
        assert!(
            (info.duration.unwrap() - 2.0).abs() < 0.05,
            "{name}: {:?}",
            info.duration
        );

        match video {
            Some((code, w, h)) => {
                let v = info.video().unwrap();

                assert_eq!(
                    (v.codec.as_ref(), v.width, v.height),
                    (Some(code), w, h),
                    "{name}"
                );
                assert!(v.timescale > 0, "{name}");
            }
            None => assert!(info.video().is_none(), "{name}"),
        }

        match audio {
            Some((code, channels, rate)) => {
                let a = info.audio().unwrap();

                assert_eq!(
                    (a.codec.as_ref(), a.channels, a.sample_rate),
                    (Some(code), channels, rate),
                    "{name}"
                );
            }
            None => assert!(info.audio().is_none(), "{name}"),
        }
    }

    // The late audio track starts after the video: ffmpeg writes an edit list for it.
    let late = probe(&data("h264-aac-audio-late.mp4")).unwrap();

    assert!(late.audio().unwrap().edit_list);
    // Its empty edit: the first sample shows at 0.228 s (ffprobe start_time 0.228). The others start at 0.
    assert!((late.audio().unwrap().start - 0.228).abs() < 1e-9);
    assert_eq!(late.video().unwrap().start, 0.0);
    assert_eq!(
        probe(&data("h264-aac.mp4")).unwrap().audio().unwrap().start,
        0.0
    );
    assert_eq!(probe(&data("h264-aac.mp4")).unwrap().codecs(), "avc1+mp4a");
    assert_eq!(probe(&data("aac-only.m4a")).unwrap().codecs(), "mp4a");
}

#[test]
fn a_crafted_track_is_read_with_its_unknown_code() {
    let info = probe(&crafted(b"xyz1")).unwrap();
    let v = info.video().unwrap();

    assert_eq!(v.kind, TrackKind::Video);
    assert_eq!(
        (v.codec_name().as_str(), v.width, v.height),
        ("xyz1", 32, 16)
    );
    assert_eq!(v.duration, Some(1.5));
    assert_eq!(info.brands[0], *b"isom");
}

#[test]
fn damaged_data_gives_an_error_and_never_a_panic() {
    assert_eq!(probe(b"this is not a video"), Err(Mp4Error::NotMp4));
    assert_eq!(probe(b""), Err(Mp4Error::NotMp4));
    assert_eq!(
        probe(&data("../vp9-1024x576-4-tile-columns.webm")),
        Err(Mp4Error::NotMp4)
    );

    let good = data("h264-aac.mp4");

    // A file without a moov box.
    assert!(matches!(
        probe(&mp4_box(b"ftyp", b"isom\0\0\0\0")),
        Err(Mp4Error::Malformed(_))
    ));
    // A box larger than the file, a box smaller than its header, a 64-bit size beyond the file.
    let mut big = good.clone();
    big[..4].copy_from_slice(&u32::MAX.to_be_bytes());
    assert!(matches!(probe(&big), Err(Mp4Error::Malformed(_))));
    let mut small = good.clone();
    small[..4].copy_from_slice(&3u32.to_be_bytes());
    assert!(matches!(probe(&small), Err(Mp4Error::Malformed(_))));
    let mut large = vec![0, 0, 0, 1];
    large.extend_from_slice(b"ftyp");
    large.extend_from_slice(&u64::MAX.to_be_bytes());
    assert!(matches!(probe(&large), Err(Mp4Error::Malformed(_))));

    // Every cut and many mutations of a real file: an answer, never a panic.
    for n in (0..good.len()).step_by(97) {
        let _ = probe(&good[..n]);
    }

    for i in 0..400usize {
        let mut d = good.clone();
        let at = (i * 7919) % d.len();

        d[at] ^= 0xFF;
        d[(at + 3) % good.len()] = (i % 251) as u8;
        let _ = probe(&d);
    }
}

/// Serves the files registered under a name, and the GIFs of the movie oracle.
struct Files(Vec<(String, Vec<u8>)>);

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        let name = url.path().trim_start_matches('/');

        if let Some((_, b)) = self.0.iter().find(|(n, _)| n == name) {
            return Ok(b.clone());
        }

        std::fs::read(
            PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                .join("../../oracle/synthetic-movie-anim/media")
                .join(name),
        )
        .map_err(|_| "TypeError: Failed to fetch".into())
    }
}

fn load(urls: &str, files: Vec<(String, Vec<u8>)>) -> Runtime {
    let text = format!(
        "#X3D V4.0 utf8\nPROFILE Full\nShape {{ appearance Appearance {{ texture DEF M MovieTexture {{ url [ {urls} ] }} }} geometry Box {{ }} }}\n"
    );
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(Files(files)));

    rt.load_world();

    for _ in 0..10 {
        rt.run_tasks();
    }

    rt
}

fn movie(rt: &Runtime) -> NodeId {
    rt.scene.get_named_node(x3d_scene::ROOT, "M").unwrap()
}

/// X_ITE's failure path (MovieTexture.js:125-137): the reason, the two messages, then FAILED with the cleared outputs.
fn failed(rt: &Runtime, file: &str, reason: &str) {
    let m = movie(rt);

    assert_eq!(rt.url_load_state(m), LoadState::Failed, "{:?}", rt.log);
    assert!(!rt.movie_has_frames(m));
    assert!(
        rt.log.iter().any(|l| l.contains(&format!(
            "the video 'http://oracle/{file}' is not decoded by this build"
        )) && l.contains(reason)),
        "{reason}: {:?}",
        rt.log
    );
    assert!(
        rt.log.iter().any(|l| l.contains(&format!(
            "Couldn't load video 'http://oracle/{file}': error."
        ))),
        "{:?}",
        rt.log
    );
    assert_eq!(rt.get_f64(m, "duration_changed"), -1.0);
    assert!(!rt.get_bool(m, "hasSound"));
}

#[test]
fn an_mp4_with_a_codec_that_nothing_decodes_fails_like_the_element_and_names_the_codec() {
    let rt = load("\"a.mp4\"", vec![("a.mp4".into(), crafted(b"xyz1"))]);

    failed(
        &rt,
        "a.mp4",
        "Unsupported { container: \"mp4\", codec: \"xyz1",
    );
}

#[test]
fn damaged_mp4_data_fails_like_the_element() {
    let rt = load(
        "\"a.mp4\"",
        vec![("a.mp4".into(), b"not an mp4 at all".to_vec())],
    );

    failed(&rt, "a.mp4", "not an MP4 file");

    let good = data("h264-aac.mp4");
    let rt = load(
        "\"cut.m4v\"",
        vec![("cut.m4v".into(), good[..good.len() / 2].to_vec())],
    );

    assert_eq!(
        rt.url_load_state(movie(&rt)),
        LoadState::Failed,
        "{:?}",
        rt.log
    );
}

#[test]
fn after_an_mp4_that_fails_the_next_candidate_loads() {
    let rt = load(
        "\"a.mp4\" \"anim4.gif\"",
        vec![("a.mp4".into(), crafted(b"xyz1"))],
    );
    let m = movie(&rt);

    assert_eq!(rt.url_load_state(m), LoadState::Complete, "{:?}", rt.log);
    assert!(rt.movie_has_frames(m));
    assert!(
        rt.log
            .iter()
            .any(|l| l.contains("Couldn't load video 'http://oracle/a.mp4': error."))
    );
}
