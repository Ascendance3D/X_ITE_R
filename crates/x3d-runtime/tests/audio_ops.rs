//! The ops the engine records for the page's AudioContext (issue #140) say the same as the graph it keeps.
//!
//! The page makes each recorded call on a real browser node (web/audio_mirror.mjs). A test here replays the log into the
//! reference interpreter ([`Model`]) and compares it, node by node and connection by connection, with `AudioEngine::graph`:
//! the log cannot lose a call without the graph showing it.

use x3d_runtime::sound::ops::{Arg, Model, to_json};
use x3d_runtime::sound::{AId, AudioEngine, AudioMode};

fn engine() -> AudioEngine {
    let mut e = AudioEngine::new(AudioMode::Silent);

    e.record_ops(true);
    e
}

fn assert_same(e: &AudioEngine, m: &Model) {
    let g = e.graph();
    let ids: Vec<usize> = g.nodes.iter().map(|n| n.id).collect();

    assert_eq!(
        ids,
        m.nodes.keys().copied().collect::<Vec<_>>(),
        "the nodes"
    );

    for n in &g.nodes {
        let model = &m.nodes[&n.id];

        assert_eq!(n.kind, model.kind, "kind of node {}", n.id);

        if let Some(c) = model.channel_count {
            assert_eq!(n.channel_count, c, "channelCount of node {}", n.id);
        }

        if let Some(mode) = model.channel_count_mode {
            assert_eq!(
                n.channel_count_mode, mode,
                "channelCountMode of node {}",
                n.id
            );
        }

        if let Some(i) = model.channel_interpretation {
            assert_eq!(
                n.channel_interpretation, i,
                "channelInterpretation of node {}",
                n.id
            );
        }

        if let Some(gain) = n.gain {
            assert_eq!(
                f64::from(gain),
                model.gain.unwrap(),
                "gain of node {}",
                n.id
            );
        }

        if let Some(s) = &n.source {
            assert_eq!(s.loop_, model.loop_, "loop of node {}", n.id);
            assert_eq!(s.started, model.started, "start of node {}", n.id);
            assert_eq!(s.stopped, model.stopped, "stop of node {}", n.id);
        }
    }

    let edges: std::collections::BTreeSet<_> = g.connections.iter().copied().collect();

    assert_eq!(edges, m.connections, "the connections");
}

#[test]
fn an_engine_that_does_not_record_logs_nothing() {
    let mut e = AudioEngine::new(AudioMode::Silent);
    let g = e.gain(0.5);

    e.connect(g, AId(0));
    assert!(e.take_ops().is_empty());
}

#[test]
fn every_node_kind_and_its_calls_are_recorded() {
    let mut e = engine();
    let destination = e.destination;
    let data: Vec<f32> = (0..2 * 480).map(|i| (i as f32 / 100.0).sin()).collect();
    let buffer = e.create_buffer(2, 480, 48000.0, Some(&data));
    let source = e.buffer_source(Some(&buffer));
    let gain = e.gain(0.25);
    let oscillator = e.oscillator();
    let biquad = e.biquad();
    let delay = e.delay(1.0, 0.1).unwrap();
    let shaper = e.wave_shaper();
    let convolver = e.convolver();
    let compressor = e.compressor();
    let analyser = e.analyser();
    let splitter = e.splitter(2);
    let merger = e.merger(2);
    let panner = e.panner();

    e.connect(source, gain);
    e.connect(gain, panner);
    e.connect(panner, destination);
    e.connect_io(splitter, merger, 1, 0);
    e.connect(oscillator, biquad);
    e.connect(biquad, delay);
    e.connect(delay, shaper);
    e.connect(shaper, convolver);
    e.connect(convolver, compressor);
    e.connect(compressor, analyser);
    e.connect(analyser, destination);
    e.set_channel_count(gain, 1);
    e.set_channel_count_mode(gain, "explicit");
    e.set_channel_interpretation(gain, "discrete");
    e.set_param(gain, "gain", 0.5);
    e.set_param(biquad, "frequency", 1000.0);
    e.buffer_source_set_loop(source, true);
    e.buffer_source_start(source, 0.0, 0.0);
    e.buffer_source_stop(source, 3.0);
    e.oscillator_start(oscillator);
    e.disconnect_to(splitter, 1, merger, 0);

    let ops = e.take_ops();
    let mut m = Model::new();

    m.apply_all(&ops);
    assert_same(&e, &m);
}

#[test]
fn a_released_slot_is_reused_after_its_release() {
    let mut e = engine();
    let destination = e.destination;
    let first = e.gain(1.0);

    e.connect(first, destination);
    e.release(first);

    let second = e.oscillator();

    assert_eq!(first, second, "the engine reuses the slot");
    e.connect(second, destination);

    let ops = e.take_ops();
    let release = ops.iter().position(|o| o.name == "release").unwrap();
    let oscillator = ops.iter().position(|o| o.name == "oscillator").unwrap();

    assert!(
        release < oscillator,
        "the release is logged before the node that takes the slot"
    );

    let mut m = Model::new();

    m.apply_all(&ops);
    assert_same(&e, &m);
}

#[test]
fn a_source_released_with_its_connections_kept_is_logged_so() {
    let mut e = engine();
    let destination = e.destination;
    let source = e.oscillator();

    e.connect(source, destination);
    e.oscillator_start(source);
    e.oscillator_stop(source, 5.0);
    e.release_keep_connections(source);

    let ops = e.take_ops();

    assert!(ops.iter().any(|o| o.name == "releaseKeep"));
    assert!(
        !ops.iter().any(|o| o.name == "disconnect"
            || o.name == "disconnectDest"
            || o.name == "disconnectTo"),
        "the page's node stays connected until its stop"
    );
}

#[test]
fn the_json_has_one_array_for_each_op_and_numbers_the_buffers() {
    let mut e = engine();
    let data = vec![0.5_f32; 8];
    let buffer = e.create_buffer(1, 8, 48000.0, Some(&data));
    let source = e.buffer_source(Some(&buffer));

    e.buffer_source_start(source, 0.0, 0.25);

    let ops = e.take_ops();
    let mut buffers = Vec::new();
    let json = to_json(&ops, &mut buffers);

    assert_eq!(
        json,
        r#"[["bufferSource",1,{"buffer":0}],["sourceStart",1,0.0,0.25]]"#
    );
    assert_eq!(buffers.len(), 1);
    assert_eq!(buffers[0].0.as_slice(), &[data]);
    assert_eq!(buffers[0].1, 48000.0);
    assert!(matches!(ops[0].args[1], Arg::Buffer(..)));
}
