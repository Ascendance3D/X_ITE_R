//! Finding C: an AudioBufferSourceNode advances only while the render graph pulls it.
//!
//! Chromium processes a node only when a node it feeds is processed, starting from the
//! destination and the automatic pull nodes (an AnalyserNode whose output is not connected). A
//! playing source whose output reaches neither does not move its read position; a finite one
//! still ends on the context clock (HandleStoppableSourceNode). X_ITE's AudioElement keeps its
//! AudioBufferSourceNode connected to the clip's `audioSource` GainNode, and Sound connects that
//! GainNode to its own graph only while the clip is its source: a clip that is connected late, or
//! again, plays on from where it was last pulled.
//!
//! The buffer is a ramp, sample i = (i + 1) / 2^20, so one output sample gives the read position.
//! Every expected (first non-zero output frame, buffer index) comes from the same graph in
//! Chrome 156 (OfflineAudioContext, 48 kHz): oracle/tools/webaudio_pull_probe.mjs, result in
//! oracle/webaudio-pull/chrome.json.

use x3d_runtime::sound::{AudioEngine, AudioMode};

const SR: usize = 48000;
const LENGTH: usize = 2 * SR;
const SCALE: f32 = 1048576.0;

enum Op {
    Connect,
    Disconnect,
    /// The analyser's output is connected (it stops being an automatic pull node).
    AnalyserOut,
}

/// buffer source -> gain (X3DSoundSourceNode .audioSource) -> sink (Sound .gainNode) ->
/// destination; with `analyser` the sink is an AnalyserNode whose output goes nowhere until
/// `Op::AnalyserOut` connects it to `tap` -> destination.
fn render(loop_: bool, analyser: bool, connected: bool, events: &[(usize, Op)]) -> Vec<f32> {
    let mut e = AudioEngine::new(AudioMode::Offline { channels: 1, sample_rate: SR as f32, length: LENGTH });
    let data: Vec<f32> = (0..SR).map(|i| (i + 1) as f32 / SCALE).collect();
    let buffer = e.create_buffer(1, SR, SR as f32, Some(&data));
    let source = e.buffer_source(Some(&buffer));
    let gain = e.gain(1.0);
    let sink = if analyser { e.analyser() } else { e.gain(1.0) };
    let tap = e.gain(1.0);
    let destination = e.destination;

    e.buffer_source_set_loop(source, loop_);
    e.connect(source, gain);
    e.connect(tap, destination);

    if !analyser {
        e.connect(sink, destination);
    }

    if connected {
        e.connect(gain, sink);
    }

    e.buffer_source_start(source, 0.0, 0.0);

    let quanta: Vec<usize> = events.iter().map(|(q, _)| *q).collect();
    e.arm(&quanta);

    for (_, op) in events {
        e.wait();

        match op {
            Op::Connect => e.connect(gain, sink),
            Op::Disconnect => {
                e.disconnect_dest(gain, sink);
            }
            Op::AnalyserOut => e.connect(sink, tap),
        }

        e.resume();
    }

    e.finish().expect("rendered").get_channel_data(0).to_vec()
}

/// (first non-zero frame at or after `from`, buffer index there).
fn probe(out: &[f32], from: usize) -> (usize, Option<usize>) {
    let k = (from..out.len()).find(|&k| out[k] != 0.0).unwrap_or(out.len());
    (k, out.get(k).map(|v| (v * SCALE).round() as usize - 1))
}

#[test]
fn connected_source_plays_from_zero() {
    let out = render(false, false, true, &[]);
    assert_eq!(probe(&out, 0), (0, Some(0)));
}

#[test]
fn late_connected_source_starts_at_its_offset() {
    // Connected at render quantum 94 (Chrome: suspend (0.25)).
    let out = render(false, false, false, &[(94, Op::Connect)]);
    assert_eq!(probe(&out, 0), (12032, Some(0)));
}

#[test]
fn reconnected_source_goes_on_from_where_it_was_pulled() {
    let out = render(false, false, true, &[(94, Op::Disconnect), (188, Op::Connect)]);
    assert_eq!(probe(&out, 0), (0, Some(0)));
    assert_eq!(probe(&out, 12000), (12000, Some(12000)));
    assert_eq!(probe(&out, 24000), (24064, Some(12032)));
}

#[test]
fn unpulled_finite_source_ends_on_the_context_clock() {
    let out = render(false, false, false, &[(563, Op::Connect)]);
    assert_eq!(probe(&out, 0), (LENGTH, None));
}

#[test]
fn unpulled_looping_source_does_not_advance() {
    let out = render(true, false, false, &[(563, Op::Connect)]);
    assert_eq!(probe(&out, 0), (72064, Some(0)));
}

#[test]
fn unconnected_analyser_pulls_its_input() {
    let out = render(false, true, true, &[(188, Op::AnalyserOut)]);
    assert_eq!(probe(&out, 0), (24064, Some(24064)));
}
