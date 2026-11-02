//! Audio output smoke run (issue #88): the realtime AudioContext of the native host on the default output device.
//!
//! The test needs a real output device (cpal: ALSA or PipeWire on Linux, CoreAudio on macOS), so it is `#[ignore]`d in the
//! suites, where nextest lists it by name as skipped. Run it with `tools/audio-smoke.sh`. Without an output device the
//! test calls `x3d_evidence::skip ("no-audio-device")`: it fails under the strict policy (the default) and records a named
//! skip only when X3D_EVIDENCE=ci lists `no-audio-device` in X3D_EVIDENCE_ALLOW. It never passes silently.
//!
//! What it shows: the engine opens the device (`AudioEngine::is_realtime`), the context clock runs, and a 440 Hz tone
//! through a gain and an AnalyserNode reaches the analyser, so the device callback pulls the graph. The gain is 0.05
//! (about -26 dB), so a run is quiet. What it cannot show: that sound leaves the speakers, or what it sounds like.

use std::time::{Duration, Instant};
use x3d_runtime::sound::sink::native_output_ids;
use x3d_runtime::sound::{ANode, AudioEngine, AudioMode};

const RATE: f32 = 48000.0;

#[test]
#[ignore = "needs an audio output device: run tools/audio-smoke.sh"]
fn realtime_output_pulls_a_tone_through_the_graph() {
    let devices = native_output_ids();
    eprintln!("audio output devices (cpal): {devices:?}");

    let mut e = AudioEngine::new(AudioMode::Realtime);

    if !e.is_realtime() {
        x3d_evidence::skip(
            "no-audio-device",
            None,
            format!(
                "AudioContext::try_new failed (cpal lists {} output devices); the engine fell back to a silent context",
                devices.len()
            ),
        );
        return;
    }

    eprintln!("realtime context: sample rate {} Hz", e.sample_rate());

    // 0.5 s of 440 Hz at full scale, played looped through gain 0.05 and an analyser into the destination.
    let n = (RATE / 2.0) as usize;
    let tone: Vec<f32> = (0..n)
        .map(|i| (2.0 * std::f32::consts::PI * 440.0 * i as f32 / RATE).sin())
        .collect();
    let buffer = e.create_buffer(1, n, RATE, Some(&tone));
    let source = e.buffer_source(Some(&buffer));
    let gain = e.gain(0.05);
    let analyser = e.analyser();
    let destination = e.destination;

    e.buffer_source_set_loop(source, true);
    e.connect(source, gain);
    e.connect(gain, analyser);
    e.connect(analyser, destination);
    e.buffer_source_start(source, 0.0, 0.0);

    let t0 = e.current_time();
    let started = Instant::now();
    let mut peak = 0.0f32;

    while started.elapsed() < Duration::from_millis(1500) {
        std::thread::sleep(Duration::from_millis(50));

        if let ANode::Analyser(_, a) = e.get_mut(analyser) {
            let mut data = vec![0f32; a.fft_size()];
            a.get_float_time_domain_data(&mut data);
            peak = data.iter().fold(peak, |m, v| m.max(v.abs()));
        }
    }

    let advanced = e.current_time() - t0;
    eprintln!(
        "context clock advanced {advanced:.3} s in 1.5 s; analyser peak {peak:.4} (tone peak 0.05)"
    );

    assert!(
        advanced > 1.0,
        "the device clock moved only {advanced:.3} s in 1.5 s"
    );
    assert!(
        (0.04..=0.06).contains(&peak),
        "the analyser peak is {peak}: expected about 0.05, the gain times the tone"
    );
}
