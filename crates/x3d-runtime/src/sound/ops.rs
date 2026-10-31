//! The operations the engine makes on its Web Audio nodes, as plain data.
//!
//! X_ITE's Sound nodes call the browser's Web Audio API. Here the same calls go to the engine's own graph (`web-audio-api` with
//! the Chromium-style kernels), which renders on a native machine. A page of X_ITE_R must reach the page's AudioContext, so the
//! engine can also record every call it makes as an [`Op`] (`AudioEngine::record_ops`). The page drains the log (`take_ops`) and
//! makes the same call on a real browser node of its AudioContext (`web/audio_mirror.mjs`). The log holds no browser types, so
//! the native tests can replay it into a model and compare the model with `AudioEngine::graph`.
//!
//! Nodes are named by their arena slot ([`super::AId`]). A slot is reused after a `release`; the log keeps the order, so the
//! release comes before the node that takes the slot.

use std::sync::Arc;

/// An argument of an [`Op`].
#[derive(Clone, Debug, PartialEq)]
pub enum Arg {
    Int(usize),
    Num(f64),
    Bool(bool),
    Str(&'static str),
    /// A list of numbers (a wave table, a curve).
    List(Vec<f32>),
    /// Planar sample data and its sample rate (an AudioBuffer made by `createBuffer`).
    Buffer(Arc<Vec<Vec<f32>>>, f32),
    /// The encoded file an AudioBuffer was decoded from (`decodeAudioData`): the page decodes it itself, so that it is
    /// resampled once, to the page context's rate, as X_ITE does.
    Encoded(Arc<Vec<u8>>),
}

/// One call on a Web Audio node: its name (`"connect"`, `"param"`, ...) and arguments.
#[derive(Clone, Debug, PartialEq)]
pub struct Op {
    pub name: &'static str,
    pub args: Vec<Arg>,
}

fn number(v: f64) -> String {
    if v.is_finite() {
        format!("{v:?}")
    } else if v.is_nan() {
        "null".into()
    } else if v > 0.0 {
        "1e999".into()
    } else {
        "-1e999".into()
    }
}

/// Planar samples and the sample rate of an AudioBuffer argument.
pub type BufferData = (Arc<Vec<Vec<f32>>>, f32);

/// The big arguments of the ops of one `to_json` call, by index.
#[derive(Default)]
pub struct Payloads {
    pub buffers: Vec<BufferData>,
    pub encoded: Vec<Arc<Vec<u8>>>,
}

/// The ops as JSON for the page: `[["connect", 1, 0, 0, 0], ...]`. A buffer argument is `{"buffer": n}` and an encoded
/// file `{"encoded": n}`, `n` indexing `payloads`, to which each one of the ops is appended; the page reads them one by one.
pub fn to_json(ops: &[Op], payloads: &mut Payloads) -> String {
    let mut out = String::from("[");

    for (i, op) in ops.iter().enumerate() {
        if i > 0 {
            out.push(',');
        }

        out.push_str(&format!("[\"{}\"", op.name));

        for arg in &op.args {
            out.push(',');

            match arg {
                Arg::Int(v) => out.push_str(&v.to_string()),
                Arg::Num(v) => out.push_str(&number(*v)),
                Arg::Bool(v) => out.push_str(if *v { "true" } else { "false" }),
                Arg::Str(v) => out.push_str(&format!("\"{v}\"")),
                Arg::List(v) => {
                    out.push('[');
                    out.push_str(
                        &v.iter()
                            .map(|&x| number(f64::from(x)))
                            .collect::<Vec<_>>()
                            .join(","),
                    );
                    out.push(']');
                }
                Arg::Buffer(data, rate) => {
                    out.push_str(&format!("{{\"buffer\":{}}}", payloads.buffers.len()));
                    payloads.buffers.push((Arc::clone(data), *rate));
                }
                Arg::Encoded(bytes) => {
                    out.push_str(&format!("{{\"encoded\":{}}}", payloads.encoded.len()));
                    payloads.encoded.push(Arc::clone(bytes));
                }
            }
        }

        out.push(']');
    }

    out.push(']');
    out
}

/// What a page that makes the ops would hold: the nodes (class name, channel attributes, a GainNode's `gain`, an
/// AudioBufferSourceNode's loop and start and stop calls) and the connections. It is the reference interpreter of the ops: a
/// test replays the log into it and compares it with `AudioEngine::graph`, so the ops cannot drift from the graph the engine
/// keeps. (The real mirror, web/audio_mirror.mjs, runs in the page; its tests are the browser tests of sound04.)
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Model {
    /// Slot to node; slot 0 is the destination.
    pub nodes: std::collections::BTreeMap<usize, ModelNode>,
    pub connections: std::collections::BTreeSet<(usize, usize, usize, usize)>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ModelNode {
    pub kind: &'static str,
    pub channel_count: Option<usize>,
    pub channel_count_mode: Option<&'static str>,
    pub channel_interpretation: Option<&'static str>,
    pub gain: Option<f64>,
    pub loop_: bool,
    pub started: Option<(f64, f64)>,
    pub stopped: Option<f64>,
}

impl ModelNode {
    fn new(kind: &'static str) -> ModelNode {
        ModelNode {
            kind,
            channel_count: None,
            channel_count_mode: None,
            channel_interpretation: None,
            gain: (kind == "GainNode").then_some(1.0),
            loop_: false,
            started: None,
            stopped: None,
        }
    }
}

fn int(a: &Arg) -> usize {
    match a {
        Arg::Int(v) => *v,
        other => panic!("not an integer: {other:?}"),
    }
}

fn num(a: &Arg) -> f64 {
    match a {
        Arg::Num(v) => *v,
        other => panic!("not a number: {other:?}"),
    }
}

fn text(a: &Arg) -> &'static str {
    match a {
        Arg::Str(v) => v,
        other => panic!("not a string: {other:?}"),
    }
}

impl Model {
    /// A model with the destination, as an engine starts.
    pub fn new() -> Model {
        let mut m = Model::default();

        m.nodes.insert(0, ModelNode::new("AudioDestinationNode"));
        m
    }

    pub fn apply(&mut self, op: &Op) {
        let a = &op.args;
        let kind = match op.name {
            "gain" => Some("GainNode"),
            "oscillator" => Some("OscillatorNode"),
            "bufferSource" => Some("AudioBufferSourceNode"),
            "biquad" => Some("BiquadFilterNode"),
            "delay" => Some("DelayNode"),
            "waveShaper" => Some("WaveShaperNode"),
            "convolver" => Some("ConvolverNode"),
            "compressor" => Some("DynamicsCompressorNode"),
            "analyser" => Some("AnalyserNode"),
            "splitter" => Some("ChannelSplitterNode"),
            "merger" => Some("ChannelMergerNode"),
            "panner" => Some("PannerNode"),
            _ => None,
        };

        if let Some(kind) = kind {
            let mut node = ModelNode::new(kind);

            if let (Some(Arg::Num(g)), "gain") = (a.get(1), op.name) {
                node.gain = Some(*g);
            }

            self.nodes.insert(int(&a[0]), node);
            return;
        }

        match op.name {
            "release" | "releaseKeep" => {
                let id = int(&a[0]);

                self.connections.retain(|c| c.0 != id && c.2 != id);
                self.nodes.remove(&id);
            }
            "connect" => {
                self.connections
                    .insert((int(&a[0]), int(&a[1]), int(&a[2]), int(&a[3])));
            }
            "disconnect" => self.connections.retain(|c| c.0 != int(&a[0])),
            "disconnectDest" => self
                .connections
                .retain(|c| !(c.0 == int(&a[0]) && c.2 == int(&a[1]))),
            "disconnectTo" => {
                self.connections
                    .remove(&(int(&a[0]), int(&a[1]), int(&a[2]), int(&a[3])));
            }
            "channelCount" => {
                self.nodes.get_mut(&int(&a[0])).unwrap().channel_count = Some(int(&a[1]))
            }
            "channelCountMode" => {
                self.nodes.get_mut(&int(&a[0])).unwrap().channel_count_mode = Some(text(&a[1]))
            }
            "channelInterpretation" => {
                self.nodes
                    .get_mut(&int(&a[0]))
                    .unwrap()
                    .channel_interpretation = Some(text(&a[1]));
            }
            "param" if text(&a[1]) == "gain" => {
                let node = self.nodes.get_mut(&int(&a[0])).unwrap();

                if node.kind == "GainNode" {
                    node.gain = Some(num(&a[2]));
                }
            }
            "sourceStart" => {
                self.nodes.get_mut(&int(&a[0])).unwrap().started = Some((num(&a[1]), num(&a[2])))
            }
            "sourceStop" => self.nodes.get_mut(&int(&a[0])).unwrap().stopped = Some(num(&a[1])),
            "sourceLoop" => {
                self.nodes.get_mut(&int(&a[0])).unwrap().loop_ = matches!(a[1], Arg::Bool(true))
            }
            // The other calls change what a page hears, not what it reads from the graph here.
            _ => {}
        }
    }

    pub fn apply_all(&mut self, ops: &[Op]) {
        for op in ops {
            self.apply(op);
        }
    }
}
