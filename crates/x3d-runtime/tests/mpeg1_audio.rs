//! The sound of an MPEG-1 Systems stream in a MovieTexture (issue #90, ISO/IEC 19775-1 18.4.2 "MPEG1-Systems (audio and
//! video)"): the audio elementary stream is collected by `mpeg1::demux` and decoded by `mpeg1::audio` (symphonia's MPEG
//! audio Layer II and III decoders). What these tests check is stated per test; none of them claims conformance to
//! ISO/IEC 11172 (the standard is not in the local library and was not consulted).
//!
//! # Fixtures
//!
//! `tests/data/mpeg1/sys-av-*.mpg` and their `*.audio.f32.gz` references: newly authored with ffmpeg 6.1.1 from `lavfi`
//! sources (`testsrc2`, `sine`, `anoisesrc`), no third-party or historical content. The commands are in
//! `tests/data/mpeg1/PROVENANCE.txt`. The reference of each stream is what ffmpeg's own float decoder (`mp2float`,
//! `mp3float`) decodes from it, written as interleaved little-endian f32 at the stream's own rate and channel count (no
//! resampling, no remix) and stored gzip-compressed.
//!
//! # The bound
//!
//! Two independent float decoders of the same stream. FIXED BEFORE THE FIRST COMPARISON (2026-11-03) and not changed
//! since: the decode has the reference's rate, channel count and length (in whole frames) exactly, and every sample is
//! within [`MAX_ABS`] of the reference. The value is the one the WebM Vorbis test uses for the same kind of comparison
//! (`the_vorbis_track_of_a_webm_equals_the_reference_decode`). The measured worst case is printed by each test
//! (`--nocapture`).

use x3d_fields::NodeId;
use x3d_runtime::mpeg1::audio::{self, AudioError};
use x3d_runtime::mpeg1::demux;
use x3d_runtime::nodes::movie_texture::MovieMedia;
use x3d_runtime::video_types::AudioTrack;
use x3d_runtime::{Fetcher, Runtime};

const MAX_ABS: f32 = 2e-4;
const BUDGET: u64 = 64 << 20;

/// (file, channels, rate, frames per channel)
const STREAMS: [(&str, usize, u32, usize); 4] = [
    ("sys-av-64x48.mpg", 1, 44100, 26496),
    ("sys-av-stereo-48k.mpg", 2, 48000, 23040),
    ("sys-av-noise-32k.mpg", 2, 32000, 16128),
    ("sys-av-mp3-44k.mpg", 1, 44100, 21888),
];

fn data(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/tests/data/mpeg1/{name}",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// The planar reference samples of a stream.
fn reference(stream: &str, channels: usize) -> Vec<Vec<f32>> {
    let gz = data(&format!(
        "{}.audio.f32.gz",
        stream.strip_suffix(".mpg").unwrap()
    ));
    let mut raw = vec![];

    std::io::Read::read_to_end(&mut flate2::read::GzDecoder::new(&gz[..]), &mut raw).unwrap();

    let all: Vec<f32> = raw
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect();

    (0..channels)
        .map(|c| all.iter().skip(c).step_by(channels).copied().collect())
        .collect()
}

fn worst(got: &[Vec<f32>], want: &[Vec<f32>]) -> f32 {
    got.iter()
        .zip(want)
        .flat_map(|(g, w)| g.iter().zip(w).map(|(a, b)| (a - b).abs()))
        .fold(0.0, f32::max)
}

fn decoded(track: AudioTrack) -> (Vec<Vec<f32>>, f32) {
    match track {
        AudioTrack::Decoded { channels, rate } => (channels, rate),
        AudioTrack::None => panic!("no audio track"),
        AudioTrack::Unsupported(c) => panic!("unsupported: {c}"),
        AudioTrack::Corrupt => panic!("corrupt"),
    }
}

#[test]
fn the_audio_of_each_fixture_equals_the_reference_decode() {
    for (name, channels, rate, frames) in STREAMS {
        let (got, got_rate) = decoded(audio::track(&data(name), BUDGET));
        let want = reference(name, channels);
        let w = worst(&got, &want);

        println!(
            "{name}: {channels} ch, {got_rate} Hz, {} frames, worst difference {w:e}",
            got[0].len()
        );

        assert_eq!(got_rate, rate as f32, "{name}: rate");
        assert_eq!(got.len(), channels, "{name}: channels");
        assert_eq!(
            want[0].len(),
            frames,
            "{name}: the reference is what the table says"
        );

        for c in 0..channels {
            assert_eq!(got[c].len(), want[c].len(), "{name}: channel {c} length");
        }

        assert!(w <= MAX_ABS, "{name}: worst sample difference {w}");
        // Not silence: a decoder that returns zeros would "equal" a silent reference only.
        assert!(
            got.iter().all(|c| c.iter().any(|s| s.abs() > 0.05)),
            "{name}: silent channel"
        );
    }
}

#[test]
fn the_two_channels_of_the_stereo_fixture_are_the_two_tones() {
    // The left channel is 440 Hz and the right 660 Hz: count zero crossings (0.5 s at 48 kHz, minus the encoder lead).
    let (got, _) = decoded(audio::track(&data("sys-av-stereo-48k.mpg"), BUDGET));
    let crossings = |c: &[f32]| c.windows(2).filter(|w| w[0] < 0.0 && w[1] >= 0.0).count() as f64;
    let seconds = got[0].len() as f64 / 48000.0;

    assert!(
        (crossings(&got[0]) / seconds - 440.0).abs() < 25.0,
        "{}",
        crossings(&got[0]) / seconds
    );
    assert!(
        (crossings(&got[1]) / seconds - 660.0).abs() < 25.0,
        "{}",
        crossings(&got[1]) / seconds
    );
}

#[test]
fn the_movie_media_carries_the_decoded_track_and_its_pictures() {
    for (name, channels, rate, frames) in STREAMS {
        let media = MovieMedia::decode_mpeg1(&data(name)).unwrap();
        let (got, got_rate) = decoded(media.audio);

        assert!(!media.frames.is_empty(), "{name}: pictures");
        assert_eq!(
            (got.len(), got[0].len(), got_rate),
            (channels, frames, rate as f32),
            "{name}"
        );
    }
}

// --- hand-built Systems streams ---------------------------------------------------------------------------------

/// An MPEG-1 packet: start code, length, the "no time stamps" byte `0x0F`, the payload.
fn packet(id: u8, payload: &[u8]) -> Vec<u8> {
    let mut p = vec![0, 0, 1, id];

    p.extend_from_slice(&(payload.len() as u16 + 1).to_be_bytes());
    p.push(0x0F);
    p.extend_from_slice(payload);
    p
}

const PACK: [u8; 12] = [
    0, 0, 1, 0xBA, 0x21, 0x00, 0x01, 0x00, 0x01, 0x80, 0x00, 0x01,
];

/// A Systems stream: the video elementary stream in packets of at most `chunk` bytes, and each `(stream id, bytes)` of
/// `audio` in packets of at most `chunk` bytes, interleaved packet by packet.
fn mux(video: &[u8], audio: &[(u8, &[u8])], chunk: usize) -> Vec<u8> {
    let mut out = vec![];
    let mut lists: Vec<(u8, Vec<&[u8]>)> = vec![(0xE0, video.chunks(chunk).collect())];

    for (id, bytes) in audio {
        lists.push((*id, bytes.chunks(chunk).collect()));
    }

    for i in 0..lists.iter().map(|(_, l)| l.len()).max().unwrap() {
        out.extend_from_slice(&PACK);

        for (id, list) in &lists {
            if let Some(c) = list.get(i) {
                out.extend(packet(*id, c));
            }
        }
    }

    out.extend_from_slice(&[0, 0, 1, 0xB9]);
    out
}

fn video_es() -> Vec<u8> {
    demux::demux(&data("sys-av-64x48.mpg")).unwrap().video
}

fn audio_es(name: &str) -> Vec<u8> {
    demux::demux(&data(name)).unwrap().audio
}

#[test]
fn a_frame_may_run_across_packets_and_the_packet_size_does_not_matter() {
    let video = video_es();
    let whole = decoded(audio::track(&data("sys-av-stereo-48k.mpg"), BUDGET));
    let es = audio_es("sys-av-stereo-48k.mpg");

    // 1 byte packets split every header; 100 and 777 cut the 384 byte frames in the middle; 60000 is one packet.
    for chunk in [1usize, 100, 777, 60000] {
        // The 1 byte case would be 100k packets: use the audio only at that size and a short video.
        let v = if chunk == 1 { &video[..64] } else { &video[..] };
        let stream = mux(v, &[(0xC0, &es)], chunk);
        let got = demux::demux(&stream).unwrap();

        assert_eq!(
            got.audio, es,
            "chunk {chunk}: the audio elementary stream is the same bytes"
        );

        // Decoded by the same decoder from the same bytes: identical, not just close.
        if chunk != 1 {
            assert_eq!(
                decoded(audio::track(&stream, BUDGET)),
                whole,
                "chunk {chunk}"
            );
        }
    }
}

#[test]
fn only_the_first_audio_stream_is_decoded() {
    let first = audio_es("sys-av-stereo-48k.mpg");
    let second = audio_es("sys-av-noise-32k.mpg");
    let stream = mux(&video_es(), &[(0xC1, &first), (0xC0, &second)], 2000);
    let (got, rate) = decoded(audio::track(&stream, BUDGET));

    // 0xC1 is the first audio packet in the file, so it is the stream.
    assert_eq!((got.len(), rate), (2, 48000.0));
    assert_eq!(got[0].len(), 23040);
}

#[test]
fn a_stream_without_audio_has_no_track() {
    let stream = mux(&video_es(), &[], 2000);

    assert!(matches!(audio::track(&stream, BUDGET), AudioTrack::None));
    assert!(matches!(
        audio::track(&data("intra-64x48.m1v"), BUDGET),
        AudioTrack::None
    ));
    assert!(matches!(
        MovieMedia::decode_mpeg1(&stream).unwrap().audio,
        AudioTrack::None
    ));
    // An audio stream whose packets are empty is an audio stream that does not decode, not a silent video.
    let mut empty = mux(&video_es(), &[], 2000);

    empty.splice(0..0, PACK.iter().copied().chain(packet(0xC0, &[])));

    assert!(matches!(audio::track(&empty, BUDGET), AudioTrack::Corrupt));
}

/// Layer I: the header of a Layer I frame (MPEG-1, no CRC, 384 kbit/s, 44.1 kHz) and then nothing that matters.
#[test]
fn layer_one_is_reported_unsupported_and_the_pictures_still_decode() {
    let mut layer1 = vec![0xFF, 0xFF, 0xA0, 0x00];

    layer1.resize(400, 0x55);

    assert_eq!(audio::first_frame(&layer1), Some((0, audio::Layer::I)));
    assert_eq!(audio::decode(&layer1, BUDGET), Err(AudioError::LayerI));

    let stream = mux(&video_es(), &[(0xC0, &layer1)], 2000);
    let media = MovieMedia::decode_mpeg1(&stream).unwrap();

    assert!(matches!(&media.audio, AudioTrack::Unsupported(c) if c.contains("layer I")));
    assert_eq!(media.frames.len(), 15);
}

#[test]
fn audio_that_does_not_decode_is_a_corrupt_track_and_the_pictures_still_decode() {
    let mut noise = vec![0u8; 3000];
    let mut x = 0x1234_5678u32;

    for b in &mut noise {
        x = x.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
        *b = (x >> 24) as u8 & 0x7F; // no 0xFF byte: no frame header
    }

    // No frame header at all.
    assert_eq!(audio::decode(&noise, BUDGET), Err(AudioError::NoFrame));
    // A frame header followed by garbage.
    let mut bad = audio_es("sys-av-64x48.mpg")[..4].to_vec();

    bad.extend_from_slice(&noise);

    for es in [&noise, &bad] {
        let media = MovieMedia::decode_mpeg1(&mux(&video_es(), &[(0xC0, es)], 2000)).unwrap();

        assert!(matches!(media.audio, AudioTrack::Corrupt));
        assert_eq!(media.frames.len(), 15);
    }
}

#[test]
fn a_bad_audio_packet_header_costs_the_sound_that_packet_and_not_the_pictures() {
    let es = audio_es("sys-av-64x48.mpg");
    let mut stream = vec![];

    stream.extend_from_slice(&PACK);
    // An MPEG-2 style PES header (`10` in the top bits) in an audio packet of an MPEG-1 stream.
    stream.extend_from_slice(&[0, 0, 1, 0xC0, 0, 4, 0x80, 0x80, 0x00, 0x00]);
    stream.extend(packet(0xE0, &video_es()[..2000]));
    stream.extend(packet(0xC0, &es[..1000]));
    stream.extend(packet(0xE0, &video_es()[2000..]));
    stream.extend_from_slice(&[0, 0, 1, 0xB9]);

    let media = MovieMedia::decode_mpeg1(&stream).unwrap();

    assert_eq!(media.frames.len(), 15, "the pictures are all there");
    // The good audio packet decoded: 1000 bytes of the 208 and 209 byte frames (64 kbit/s, 44.1 kHz) are four whole frames.
    let (got, _) = decoded(media.audio);

    assert_eq!(got[0].len(), 4 * 1152);
}

#[test]
fn the_byte_budget_refuses_more_sound_than_fits() {
    let es = audio_es("sys-av-stereo-48k.mpg");

    assert_eq!(audio::decode(&es, 1000), Err(AudioError::TooLarge));
    assert!(audio::decode(&es, 23040 * 2 * 4).is_ok());
    assert!(matches!(
        audio::track(&data("sys-av-stereo-48k.mpg"), 1000),
        AudioTrack::Corrupt
    ));
}

// --- malformed and truncated streams ---------------------------------------------------------------------------

/// Every cut of the Systems stream: no panic (the decoder is called without the `catch_unwind` of [`audio::track`], so a
/// panic fails here), and what decodes is the start of the reference, within the bound (the frames before a cut are
/// the same frames).
#[test]
fn every_truncation_of_every_fixture_returns_without_panic_and_keeps_a_prefix() {
    for (name, channels, _, _) in STREAMS {
        let full = data(name);
        let want = reference(name, channels);
        let mut decoded_some = 0;

        for len in (0..full.len()).step_by(7).chain(0..64) {
            let cut = &full[..len];
            let result = std::panic::catch_unwind(|| match demux::demux(cut) {
                Ok(d) => audio::decode(&d.audio, BUDGET).ok(),
                Err(_) => None,
            });

            let Ok(result) = result else {
                panic!("{name}: a cut at {len} panicked");
            };

            if let Some((got, _)) = result {
                decoded_some += 1;

                assert_eq!(got.len(), channels, "{name}@{len}");
                assert!(
                    got[0].len() <= want[0].len(),
                    "{name}@{len}: longer than the whole"
                );
                assert!(
                    got[0].len().is_multiple_of(1152),
                    "{name}@{len}: whole frames only"
                );

                let prefix: Vec<Vec<f32>> =
                    want.iter().map(|c| c[..got[0].len()].to_vec()).collect();

                assert!(
                    worst(&got, &prefix) <= MAX_ABS,
                    "{name}@{len}: not the start of the whole"
                );
            }
        }

        assert!(decoded_some > 0, "{name}: a long enough prefix decodes");
    }
}

/// Deterministic generator (SplitMix64).
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);

        let mut z = self.0;

        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

/// 400 rounds of one to eight byte changes of the audio elementary stream of each fixture, and of the whole Systems
/// stream: neither panics (the audio decoder is called without the `catch_unwind` of [`audio::track`]) and a decode that
/// succeeds stays within the byte budget.
#[test]
fn mutated_audio_never_panics() {
    let mut rng = Rng(0x4D50_3241_5544_494F);

    for (name, _, _, _) in STREAMS {
        let whole = data(name);
        let es = audio_es(name);

        for round in 0..400 {
            let base = if round % 2 == 0 { &es } else { &whole };
            let mut d = base.clone();

            for _ in 0..1 + rng.next() % 8 {
                let at = (rng.next() as usize) % d.len();

                d[at] = match rng.next() % 3 {
                    0 => d[at] ^ (1 << (rng.next() % 8)),
                    1 => rng.next() as u8,
                    _ => 0,
                };
            }

            let r = std::panic::catch_unwind(|| {
                let es = if round % 2 == 0 {
                    d.clone()
                } else {
                    demux::demux(&d).map(|x| x.audio).unwrap_or_default()
                };

                if let Ok((c, rate)) = audio::decode(&es, BUDGET) {
                    assert!(rate > 0.0);
                    assert!(c.iter().map(|c| c.len() * 4).sum::<usize>() as u64 <= BUDGET);
                }
            });

            assert!(r.is_ok(), "{name}: mutation round {round} panicked");
        }
    }
}

#[test]
fn empty_tiny_and_non_audio_inputs_are_not_a_track() {
    assert_eq!(audio::decode(&[], BUDGET), Err(AudioError::NoFrame));
    assert_eq!(audio::decode(&[0xFF], BUDGET), Err(AudioError::NoFrame));
    assert_eq!(
        audio::decode(&[0xFF, 0xFD, 0x90], BUDGET),
        Err(AudioError::NoFrame)
    );
    // A sync word with the reserved layer, the reserved version, the bit rate index 15 and the sample rate index 3.
    for header in [
        [0xFF, 0xF9, 0x90, 0],
        [0xFF, 0xE9, 0x90, 0],
        [0xFF, 0xFD, 0xF0, 0],
        [0xFF, 0xFD, 0x9C, 0],
    ] {
        assert_eq!(audio::first_frame(&header), None, "{header:?}");
    }

    for d in [
        &b""[..],
        &[0, 0, 1, 0xBA][..],
        &[0, 0, 1, 0xBA, 0x21][..],
        b"RIFF....WAVE",
        &[0u8; 100][..],
    ] {
        assert!(matches!(
            audio::track(d, BUDGET),
            AudioTrack::None | AudioTrack::Corrupt
        ));
    }

    // The leading bytes before the first frame header (a packet that starts in the middle of a frame) are skipped.
    let mut es = vec![0x12, 0x34, 0x56];

    es.extend(audio_es("sys-av-64x48.mpg"));

    let (got, _) = audio::decode(&es, BUDGET).unwrap();

    assert_eq!(got[0].len(), 26496);
}

// --- through the node ------------------------------------------------------------------------------------------

struct Files;

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        std::fs::read(format!(
            "{}/tests/data/mpeg1/{}",
            env!("CARGO_MANIFEST_DIR"),
            url.path().trim_start_matches('/')
        ))
        .map_err(|e| e.to_string())
    }
}

fn load(file: &str) -> Runtime {
    let text = format!(
        "#X3D V4.0 utf8\nPROFILE Full\nSound {{ source DEF M MovieTexture {{ url \"{file}\" loop TRUE startTime 1 }} }}"
    );
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(Files));

    rt.load_world();

    for _ in 0..10 {
        rt.run_tasks();
    }

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

#[test]
fn an_mpeg1_movie_with_sound_plays_its_sound_through_the_node() {
    for (file, channels, rate, frames) in [
        ("sys-av-64x48.mpg", 1, 44100u32, 26496usize),
        ("sys-av-stereo-48k.mpg", 2, 48000, 23040),
    ] {
        let mut rt = load(file);
        let m = movie(&rt);

        assert!(rt.get_bool(m, "hasSound"), "{file}");
        assert!(rt.movie_has_frames(m), "{file}");

        // The context resamples the track to its own rate (48 kHz).
        let want = (frames as f64 * 48000.0 / rate as f64).round() as usize;
        let buffers = playing_buffers(&mut rt);

        assert!(
            buffers.iter().any(|((c, f, r), started)| *c == channels
                && f.abs_diff(want) <= 1
                && *r == 48000.0
                && *started),
            "{file}: {buffers:?}, wanted {channels} ch {want} frames started"
        );
        assert!(
            !rt.log.iter().any(|l| l.contains("audio track")),
            "{file}: {:?}",
            rt.log
        );
    }
}

#[test]
fn an_mpeg1_movie_without_sound_has_no_sound_and_no_diagnostic() {
    let mut rt = load("intra-64x48.m1v");
    let m = movie(&rt);

    assert!(!rt.get_bool(m, "hasSound"));
    assert!(rt.movie_has_frames(m));
    assert!(playing_buffers(&mut rt).is_empty());
    assert!(rt.log.iter().all(|l| !l.contains("audio")), "{:?}", rt.log);
}

// --- the start of the audio and of the pictures ------------------------------------------------------------------

/// (file, video start, audio start) in seconds: `ffprobe -show_entries stream=codec_type,start_time` (ffmpeg 6.1.1), which
/// prints six decimals. The reference for the time line, because the pinned X_ITE cannot play MPEG-1.
const STARTS: [(&str, f64, f64); 4] = [
    ("sys-av-64x48.mpg", 0.540000, 0.529089),
    ("sys-av-stereo-48k.mpg", 0.540000, 0.529978),
    ("sys-av-noise-32k.mpg", 0.540000, 0.524967),
    ("sys-av-mp3-44k.mpg", 0.540000, 0.514944),
];

#[test]
fn the_first_time_stamps_equal_ffprobe_and_the_later_stream_is_delayed() {
    for (name, video, audio_start) in STARTS {
        let d = demux::demux(&data(name)).unwrap();
        let (v, a) = (d.first_video_pts.unwrap(), d.first_audio_pts.unwrap());

        // Six printed decimals: within half a microsecond, plus the 90 kHz tick.
        assert!((v - video).abs() < 6e-7, "{name}: video {v}");
        assert!((a - audio_start).abs() < 6e-7, "{name}: audio {a}");

        // In every fixture the audio starts first: the pictures wait, the sound starts at 0.
        let (video_lead, audio_lead) = demux::start_leads(&d);

        assert_eq!(audio_lead, 0.0, "{name}");
        assert!((video_lead - (v - a)).abs() < 1e-12, "{name}");

        // The first picture is held by the lead, so the later pictures keep their time stamps: the movie is longer by
        // the lead. The sound is unchanged (the_movie_media_carries_the_decoded_track_and_its_pictures).
        let media = MovieMedia::decode_mpeg1(&data(name)).unwrap();
        let video = x3d_runtime::mpeg1::decode(&data(name), BUDGET).unwrap();
        let pictures = video.frames.len() as f64 / video.framerate;

        assert!(
            (media.duration() - (pictures + video_lead)).abs() < 1e-9,
            "{name}: {}",
            media.duration()
        );
    }
}

/// [`packet`] with a presentation time stamp (`0010` marker, 90 kHz ticks).
fn packet_pts(id: u8, payload: &[u8], ticks: u64) -> Vec<u8> {
    let mut p = vec![0, 0, 1, id];

    p.extend_from_slice(&(payload.len() as u16 + 5).to_be_bytes());
    p.extend_from_slice(&[
        0x21 | (((ticks >> 30) & 0x07) as u8) << 1,
        (ticks >> 22) as u8,
        ((((ticks >> 15) & 0x7F) as u8) << 1) | 1,
        (ticks >> 7) as u8,
        (((ticks & 0x7F) as u8) << 1) | 1,
    ]);
    p.extend_from_slice(payload);
    p
}

/// [`mux`] with one time stamp on the first video packet and one on the first audio packet.
fn mux_pts(
    video: &[u8],
    audio: &[u8],
    chunk: usize,
    video_ticks: u64,
    audio_ticks: u64,
) -> Vec<u8> {
    let mut out = vec![];
    let (v, a): (Vec<&[u8]>, Vec<&[u8]>) =
        (video.chunks(chunk).collect(), audio.chunks(chunk).collect());

    for i in 0..v.len().max(a.len()) {
        out.extend_from_slice(&PACK);

        if let Some(c) = v.get(i) {
            out.extend(if i == 0 {
                packet_pts(0xE0, c, video_ticks)
            } else {
                packet(0xE0, c)
            });
        }

        if let Some(c) = a.get(i) {
            out.extend(if i == 0 {
                packet_pts(0xC0, c, audio_ticks)
            } else {
                packet(0xC0, c)
            });
        }
    }

    out.extend_from_slice(&[0, 0, 1, 0xB9]);
    out
}

#[test]
fn audio_that_starts_after_the_pictures_begins_with_silence() {
    let es = audio_es("sys-av-stereo-48k.mpg");
    let plain = decoded(audio::track(
        &mux(&video_es(), &[(0xC0, &es)], 2000),
        BUDGET,
    ));

    // The audio starts 0.25 s (22500 ticks) after the first picture: 12000 silent frames at 48 kHz, then the same sound.
    let stream = mux_pts(&video_es(), &es, 2000, 45000, 67500);
    let d = demux::demux(&stream).unwrap();

    assert_eq!(demux::start_leads(&d), (0.0, 0.25));

    let (got, rate) = decoded(audio::track(&stream, BUDGET));

    assert_eq!(rate, 48000.0);

    for (c, p) in got.iter().zip(&plain.0) {
        assert_eq!(c.len(), p.len() + 12000);
        assert!(c[..12000].iter().all(|&s| s == 0.0));
        assert_eq!(&c[12000..], &p[..]);
    }

    // The pictures start at 0: the movie is as long as its pictures.
    let media = MovieMedia::decode_mpeg1(&stream).unwrap();
    let video = x3d_runtime::mpeg1::decode(&stream, BUDGET).unwrap();

    assert!((media.duration() - video.frames.len() as f64 / video.framerate).abs() < 1e-9);
}

#[test]
fn without_two_time_stamps_both_streams_start_at_zero() {
    // The hand-built streams of the tests above carry no time stamps.
    let es = audio_es("sys-av-stereo-48k.mpg");
    let d = demux::demux(&mux(&video_es(), &[(0xC0, &es)], 2000)).unwrap();

    assert_eq!((d.first_video_pts, d.first_audio_pts), (None, None));
    assert_eq!(demux::start_leads(&d), (0.0, 0.0));

    // Only the video packets carry a time stamp: the audio packets have the "no time stamps" byte.
    let video = video_es();
    let (v, a): (Vec<&[u8]>, Vec<&[u8]>) =
        (video.chunks(2000).collect(), es.chunks(2000).collect());
    let mut stream = vec![];

    for i in 0..v.len().max(a.len()) {
        stream.extend_from_slice(&PACK);

        if let Some(c) = v.get(i) {
            stream.extend(packet_pts(0xE0, c, 45000 + 3003 * i as u64));
        }

        if let Some(c) = a.get(i) {
            stream.extend(packet(0xC0, c));
        }
    }

    stream.extend_from_slice(&[0, 0, 1, 0xB9]);

    let d = demux::demux(&stream).unwrap();

    assert_eq!((d.first_video_pts, d.first_audio_pts), (Some(0.5), None));
    assert_eq!(demux::start_leads(&d), (0.0, 0.0));
}

#[test]
fn the_earliest_time_stamp_of_a_stream_is_its_start() {
    // Video packets in file order with time stamps 0.6 s, 0.5 s, 0.7 s (reordered pictures): the stream starts at 0.5 s.
    let es = audio_es("sys-av-stereo-48k.mpg");
    let video = video_es();
    let (v, a): (Vec<&[u8]>, Vec<&[u8]>) =
        (video.chunks(2000).collect(), es.chunks(2000).collect());
    let ticks = [54000, 45000, 63000];
    let mut stream = vec![];

    for i in 0..v.len().max(a.len()) {
        stream.extend_from_slice(&PACK);

        if let Some(c) = v.get(i) {
            stream.extend(match ticks.get(i) {
                Some(&t) => packet_pts(0xE0, c, t),
                None => packet(0xE0, c),
            });
        }

        if let Some(c) = a.get(i) {
            stream.extend(if i == 0 {
                packet_pts(0xC0, c, 40500)
            } else {
                packet(0xC0, c)
            });
        }
    }

    stream.extend_from_slice(&[0, 0, 1, 0xB9]);

    let d = demux::demux(&stream).unwrap();

    assert_eq!(d.first_video_pts, Some(0.5));
    assert_eq!(d.first_audio_pts, Some(0.45));
    assert!((demux::start_leads(&d).0 - 0.05).abs() < 1e-12);
}

#[test]
fn leading_silence_that_does_not_fit_the_budget_makes_the_track_corrupt() {
    // The audio starts 100 s after the pictures: 4.8 million silent frames per channel at 48 kHz, 38.4 MB for two
    // channels, over a 16 MiB budget. The sound is not shifted to 0; the track is Corrupt.
    let es = audio_es("sys-av-stereo-48k.mpg");
    let stream = mux_pts(&video_es(), &es, 2000, 45000, 45000 + 9_000_000);

    assert!(matches!(
        audio::track(&stream, 16 << 20),
        AudioTrack::Corrupt
    ));
    // A budget that holds the silence plays it.
    let (got, _) = decoded(audio::track(&stream, BUDGET));

    assert_eq!(got[0].len(), 23040 + 4_800_000);
}
