//! decodeAudioData of MP3, Ogg Vorbis, FLAC, Opus and WAV files against Chromium (issue #98).
//!
//! `oracle/rea-b24-audio-decode-baseline/audio-decode-baseline.json` is what the AudioContext of the pinned
//! X_ITE (16.5.0, 34cdf9f97) in headless Chromium decodes the synthetic files of
//! `oracle/synthetic-rea-b24-audio/` to (`oracle/x_ite-audio-decode.mjs`, five identical recordings): length,
//! sample rate, channels and every sample. The files are clean-room: ffmpeg sine plus seeded noise, 0.3 s.
//!
//! The context is 48 kHz (Chromium's, as recorded), so a file at another rate is resampled by the decoder.
//!
//! Results (max abs difference over the common prefix; Chromium peak is about 0.4 to 0.5):
//! - same rate as the context (MP3 48 kHz, FLAC 48 kHz 24 bit): bit-identical.
//! - PCM, A-law and mu-law WAV, and MP3, FLAC and Ogg Vorbis, at any other rate (decode.rs: the file's own rate and
//!   frames, Chromium's SincResampler): same length, within 5e-5 (measured 1.2e-5 to 3.9e-5). Ogg Vorbis returns
//!   none of the frames of its last packet in Chromium, which decode.rs reproduces.
//! - Ogg Vorbis 48 kHz: samples within 9e-8.
//! - Opus: Chromium decodes it, `web-audio-api` rejects it (KNOWN).
//! - AAC in M4A: Chromium decodes it; the port excludes the AAC decoder by decision, so parity is UNSUPPORTED
//!   (`oracle/rea-b24-audio-decode-2-baseline`, `aac_is_excluded_and_chromium_parity_is_unsupported`).

use serde_json::Value;
use sha2::{Digest, Sha256};
use x3d_runtime::sound::{AudioEngine, AudioMode};

fn root() -> std::path::PathBuf {
    std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

fn unbase64(s: &str) -> Vec<u8> {
    let mut out = Vec::new();
    let (mut acc, mut bits) = (0u32, 0);

    for c in s.bytes() {
        let v = match c {
            b'A'..=b'Z' => c - b'A',
            b'a'..=b'z' => c - b'a' + 26,
            b'0'..=b'9' => c - b'0' + 52,
            b'+' => 62,
            b'/' => 63,
            _ => continue,
        } as u32;

        acc = acc << 6 | v;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }

    out
}

fn floats(bytes: &[u8]) -> Vec<f32> {
    bytes
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
        .collect()
}

struct Measure {
    rust_length: usize,
    chrome_length: usize,
    /// Over the common prefix of every channel.
    max_abs: f32,
    /// The largest absolute sample that the decoder produced: silence would pass a difference bound by luck.
    peak: f32,
    exact: bool,
}

fn measure(file: &str) -> Measure {
    measure_in(
        "rea-b24-audio-decode-baseline",
        "synthetic-rea-b24-audio",
        file,
    )
}

fn measure2(file: &str) -> Measure {
    measure_in(
        "rea-b24-audio-decode-2-baseline",
        "synthetic-rea-b24-audio-2",
        file,
    )
}

fn measure_in(baseline_dir: &str, corpus: &str, file: &str) -> Measure {
    let baseline: Value = serde_json::from_slice(
        &std::fs::read(root().join(baseline_dir).join("audio-decode-baseline.json")).unwrap(),
    )
    .unwrap();
    let entry = baseline["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|r| r["file"] == file)
        .unwrap_or_else(|| panic!("{file} not in the baseline"));
    assert_eq!(entry["ok"], true, "{file}");

    let engine = AudioEngine::new(AudioMode::Offline {
        channels: 2,
        sample_rate: entry["contextSampleRate"].as_f64().unwrap() as f32,
        length: 128,
    });
    let bytes = std::fs::read(root().join(corpus).join(file)).unwrap();
    let buffer = engine
        .decode(bytes)
        .unwrap_or_else(|e| panic!("{file}: {e}"));
    let chrome_length = entry["length"].as_u64().unwrap() as usize;
    let channels = entry["numberOfChannels"].as_u64().unwrap() as usize;

    assert_eq!(
        buffer.sample_rate(),
        entry["sampleRate"].as_f64().unwrap() as f32,
        "{file} rate"
    );
    assert_eq!(buffer.number_of_channels(), channels, "{file} channels");

    let mut max_abs = 0.0f32;
    let mut peak = 0.0f32;
    let mut exact = buffer.length() == chrome_length;

    for c in 0..channels {
        let ch = &entry["channels"][c];
        let want = floats(&unbase64(ch["data"].as_str().unwrap()));
        assert_eq!(want.len(), chrome_length);
        assert_eq!(
            format!(
                "{:x}",
                Sha256::digest(
                    want.iter()
                        .flat_map(|v| v.to_le_bytes())
                        .collect::<Vec<u8>>()
                )
            ),
            ch["sha256"].as_str().unwrap(),
            "{file}: the stored samples are the hashed ones"
        );

        for (g, w) in buffer.get_channel_data(c).iter().zip(&want) {
            exact &= g.to_bits() == w.to_bits();
            max_abs = max_abs.max((g - w).abs());
        }

        peak = buffer
            .get_channel_data(c)
            .iter()
            .fold(peak, |m, v| m.max(v.abs()));
    }

    assert!(peak > 0.01, "{file}: the decoded audio is silent ({peak})");

    println!(
        "{file}: chrome {chrome_length} frames, rust {}, max abs difference {max_abs:e}",
        buffer.length()
    );
    Measure {
        rust_length: buffer.length(),
        chrome_length,
        max_abs,
        peak,
        exact,
    }
}

/// The decoder's rate is the context's: nothing is resampled, so every sample is Chromium's.
#[test]
fn same_rate_mp3_and_flac_are_bit_identical() {
    for file in ["mp3-48000-stereo.mp3", "flac-48000-stereo-s24.flac"] {
        let m = measure(file);
        assert!(m.exact, "{file}: max abs difference {:e}", m.max_abs);
    }
}

/// decode.rs: Chromium's sample conversion and SincResampler, 44.1 kHz to 48 kHz. Not bit-identical.
#[test]
fn resampled_wav_is_within_5e_5_of_chromium() {
    let m = measure("wav-s16-44100-st.wav");
    assert_eq!(m.rust_length, m.chrome_length);
    assert!(m.max_abs <= 5e-5, "{:e}", m.max_abs);
    assert!(m.max_abs > 0.0, "now exact: tighten the bound");
}

/// The bound of the SincResampler port, set by the PCM WAV case (3.3e-5) before the compressed formats used it.
const RESAMPLED_BOUND: f32 = 5e-5;

/// Ogg Vorbis at the context rate: the same samples (the float conversion differs in the last bit), and Chromium's
/// length, 64 frames short of the file's: the end rule of decode.rs (the granule position less the first packet's
/// lapped frames, within the last packet; `ogg_end_follows_the_granule_position`).
#[test]
fn ogg_48000_has_chromiums_length_and_samples() {
    let m = measure("ogg-48000-mono.ogg");
    assert_eq!((m.rust_length, m.chrome_length), (14336, 14336));
    assert!(m.max_abs <= 1e-7, "{:e}", m.max_abs);
}

/// MP3, FLAC and Ogg Vorbis at 22.05, 32, 44.1 kHz and others: the length of Chromium and its samples within the
/// bound of the WAV case. Before decode.rs decoded them, `web-audio-api` resampled with rubato: max abs difference
/// 0.33 to 0.46, one frame longer, and Ogg up to 470 frames longer.
#[test]
fn resampled_compressed_files_match_chromium() {
    for file in [
        "mp3-22050-mono.mp3",
        "mp3-44100-stereo.mp3",
        "flac-22050-mono.flac",
        "flac-44100-stereo.flac",
        "ogg-22050-mono.ogg",
        "ogg-44100-stereo.ogg",
    ] {
        let m = measure(file);
        assert_eq!(m.rust_length, m.chrome_length, "{file}");
        assert!(m.max_abs <= RESAMPLED_BOUND, "{file}: {:e}", m.max_abs);
        assert!(m.max_abs > 0.0, "{file} now exact: tighten the bound");
    }
}

/// The second corpus (`synthetic-rea-b24-audio-2`, five identical recordings): other rates, A-law and mu-law.
#[test]
fn more_rates_and_g711_match_chromium() {
    for file in [
        "mp3-11025-mono.mp3",
        "mp3-32000-stereo.mp3",
        "flac-8000-mono.flac",
        "ogg-32000-stereo.ogg",
        "wav-alaw-44100-st.wav",
        "wav-ulaw-44100-st.wav",
        "wav-alaw-8000-mono.wav",
        "wav-ulaw-8000-mono.wav",
    ] {
        let m = measure2(file);
        assert_eq!(m.rust_length, m.chrome_length, "{file}");
        assert!(m.max_abs <= RESAMPLED_BOUND, "{file}: {:e}", m.max_abs);
    }

    // At the context rate nothing is resampled.
    let m = measure2("wav-ulaw-48000-mono.wav");
    assert!(m.exact, "{:e}", m.max_abs);
}

/// AAC in M4A is excluded by decision (owner, 2026-11-02, issue #90), not by a code path: `Cargo.lock` holds no AAC decoder
/// (see `vendor/web-audio-api/VENDOR.md`), so the four files fail to decode with `unsupported audio codec`, whatever their
/// atom layout. Chromium (FFmpeg) decodes all four (`oracle/rea-b24-audio-decode-2-baseline`: 14400, 14400, 14400 and 14399
/// frames), so parity for AAC is recorded as UNSUPPORTED, not complete. An earlier build decoded the faststart files to
/// 15604 frames (the 1024 priming frames are not trimmed: symphonia gives no edit list) and failed on the moov-last file.
#[test]
fn aac_is_excluded_and_chromium_parity_is_unsupported() {
    let baseline: Value = serde_json::from_slice(
        &std::fs::read(root().join("rea-b24-audio-decode-2-baseline/audio-decode-baseline.json"))
            .unwrap(),
    )
    .unwrap();
    let engine = AudioEngine::new(AudioMode::Offline {
        channels: 2,
        sample_rate: 48000.0,
        length: 128,
    });

    for file in [
        "aac-44100-stereo.m4a",
        "aac-44100-stereo-faststart.m4a",
        "aac-22050-mono.m4a",
        "aac-48000-stereo.m4a",
    ] {
        let chrome = baseline["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["file"] == file)
            .unwrap_or_else(|| panic!("{file} not in the baseline"));
        assert_eq!(chrome["ok"], true, "{file}: Chromium decodes it");

        let bytes = std::fs::read(root().join("synthetic-rea-b24-audio-2").join(file)).unwrap();
        let err = match engine.decode(bytes) {
            Ok(b) => panic!(
                "{file}: decodes ({} frames): the AAC decoder is back",
                b.length()
            ),
            Err(e) => e,
        };
        if file != "aac-44100-stereo.m4a" {
            // The moov-last file fails earlier, while the reader looks for the moov atom.
            assert!(err.contains("unsupported"), "{file}: {err}");
        }
    }
}

/// No AAC decoder is in the resolved dependency graph (native and wasm32 alike: Cargo.lock lists both), not merely refused
/// at run time. The same check runs as `python3 -I tools/check_no_aac.py`.
#[test]
fn cargo_lock_has_no_aac_decoder() {
    let lock = std::fs::read_to_string(
        std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("../../Cargo.lock"),
    )
    .unwrap();
    let packages: Vec<&str> = lock
        .lines()
        .filter_map(|l| l.strip_prefix("name = \""))
        .filter_map(|l| l.strip_suffix('"'))
        .collect();
    assert!(
        packages.len() > 100,
        "{} packages: lock not read",
        packages.len()
    );
    assert!(
        packages.contains(&"symphonia-core"),
        "symphonia is gone: update this test"
    );
    let aac: Vec<&&str> = packages
        .iter()
        .filter(|n| n.split(['-', '_']).any(|w| w == "aac"))
        .collect();
    assert!(aac.is_empty(), "AAC decoder in Cargo.lock: {aac:?}");
}

/// Chromium (FFmpeg) decodes Opus; web-audio-api has no Opus decoder.
#[test]
fn known_opus_is_rejected_by_the_port() {
    let engine = AudioEngine::new(AudioMode::Offline {
        channels: 2,
        sample_rate: 48000.0,
        length: 128,
    });
    let bytes =
        std::fs::read(root().join("synthetic-rea-b24-audio/opus-48000-stereo.opus")).unwrap();
    assert!(
        engine.decode(bytes).is_err(),
        "now decodes: compare it with the baseline"
    );
}
