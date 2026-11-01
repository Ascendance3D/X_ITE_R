//! decodeAudioData of MP3, Ogg Vorbis, FLAC, Opus and WAV files against Chromium (issue #98).
//!
//! `oracle/rea-batch24-audio-decode-baseline/audio-decode-baseline.json` is what the AudioContext of the pinned
//! X_ITE (16.5.0, 34cdf9f97) in headless Chromium decodes the synthetic files of
//! `oracle/synthetic-rea-b24-audio/` to (`oracle/x_ite-audio-decode.mjs`, five identical recordings): length,
//! sample rate, channels and every sample. The files are clean-room: ffmpeg sine plus seeded noise, 0.3 s.
//!
//! The context is 48 kHz (Chromium's, as recorded), so a file at another rate is resampled by the decoder.
//!
//! Results (max abs difference over the common prefix; Chromium peak is about 0.4 to 0.5):
//! - same rate as the context (MP3 48 kHz, FLAC 48 kHz 24 bit): bit-identical.
//! - PCM WAV 44.1 kHz resampled (decode.rs, SincResampler): 3.3e-5, same length.
//! - MP3, FLAC, Ogg at 22.05 or 44.1 kHz (web-audio-api's rubato resampler): KNOWN difference, 0.33 to 0.46
//!   (mean 0.04 to 0.07), one frame longer than Chromium; Ogg is also up to 470 frames longer than Chromium.
//! - Ogg Vorbis 48 kHz: samples within 9e-8, but Chromium returns 64 frames fewer.
//! - Opus: Chromium decodes it, `web-audio-api` rejects it (KNOWN).

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
    exact: bool,
}

fn measure(file: &str) -> Measure {
    let baseline: Value = serde_json::from_slice(
        &std::fs::read(root().join("rea-batch24-audio-decode-baseline/audio-decode-baseline.json"))
            .unwrap(),
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
    let bytes = std::fs::read(root().join("synthetic-rea-b24-audio").join(file)).unwrap();
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
    }

    println!(
        "{file}: chrome {chrome_length} frames, rust {}, max abs difference {max_abs:e}",
        buffer.length()
    );
    Measure {
        rust_length: buffer.length(),
        chrome_length,
        max_abs,
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

/// Ogg Vorbis at the context rate: the same samples (the float conversion differs in the last bit), but
/// Chromium returns 64 frames fewer than the file holds.
#[test]
fn known_ogg_48000_chromium_drops_the_last_64_frames() {
    let m = measure("ogg-48000-mono.ogg");
    assert_eq!((m.rust_length, m.chrome_length), (14400, 14336));
    assert!(m.max_abs <= 1e-7, "{:e}", m.max_abs);
}

/// MP3, FLAC and Ogg that web-audio-api resamples: KNOWN differences (max abs difference, Rust length,
/// Chromium length). The resampler is rubato, Chromium's is SincResampler with the file's own frames (the WAV
/// path does it that way); the fix needs the file's native rate and frames from the decoder, which
/// web-audio-api does not expose. A file that now matches fails: remove it from the list.
const KNOWN_RESAMPLED: &[(&str, usize, usize)] = &[
    ("mp3-22050-mono.mp3", 14401, 14400),
    ("mp3-44100-stereo.mp3", 14401, 14400),
    ("flac-22050-mono.flac", 14401, 14400),
    ("flac-44100-stereo.flac", 14401, 14400),
    ("ogg-22050-mono.ogg", 14401, 13931),
    ("ogg-44100-stereo.ogg", 14401, 14349),
];

#[test]
fn known_resampled_non_wav_differs_from_chromium() {
    for &(file, rust_length, chrome_length) in KNOWN_RESAMPLED {
        let m = measure(file);
        assert_eq!(
            (m.rust_length, m.chrome_length),
            (rust_length, chrome_length),
            "{file}"
        );
        assert!(
            !m.exact,
            "{file} now matches Chromium: remove it from KNOWN_RESAMPLED"
        );
        // Measured 0.33 to 0.46; a floor, so the entry is not satisfied by a near miss.
        assert!(m.max_abs > 0.25, "{file}: {:e}", m.max_abs);
    }
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
