//! The browser's audio context (Browser/Sound/X3DSoundContext.js `getAudioContext`) and the Web
//! Audio nodes the Sound component builds on it.
//!
//! X_ITE's Sound nodes are a thin layer over the browser's Web Audio API. Here the same graph is
//! built with `web-audio-api`, a Rust implementation of the W3C Web Audio API, so the node
//! classes in `nodes/sound.rs` port line by line. Web Audio nodes live in an arena ([`AId`]); the
//! engine keeps its own list of connections, because a Web Audio `disconnect (destination)` of a
//! connection that does not exist throws (here: is skipped).
//!
//! Three modes ([`AudioMode`]):
//! * `Silent` (default; the oracle dump runs and tests that never play): an
//!   OfflineAudioContext that is never rendered. The graph exists, nothing is heard.
//! * `Offline`: an OfflineAudioContext rendered in step with the frames (x3d-drive's `"audio"`
//!   scripts, see [`AudioEngine::arm`]), the same as oracle/x_ite-anim.mjs does with the pinned
//!   X_ITE.
//! * `Realtime`: an AudioContext on the default output device (cpal: ALSA on Linux, CoreAudio
//!   on macOS); the interactive viewer.

pub mod analyser;
pub mod biquad;
pub mod buffer_source;
pub mod compressor;
pub mod convolver;
pub mod decode;
pub mod delay;
pub mod fft_frame;
pub mod gain;
pub mod ops;
pub mod oscillator;
pub mod panner;
pub mod param_timeline;
pub mod wave_shaper;

use ops::{Arg, Op};
use std::pin::Pin;
use std::sync::Arc;
use std::task::{Context as TaskContext, Poll};
use web_audio_api::context::{
    AudioContext, AudioContextOptions, BaseAudioContext, ConcreteBaseAudioContext,
    OfflineAudioContext,
};
use web_audio_api::node::*;
use web_audio_api::{AudioBuffer, AudioBufferOptions, AudioListener, AudioParam};

/// How the browser's audio context renders.
#[derive(Clone, Debug, Default, PartialEq)]
pub enum AudioMode {
    #[default]
    Silent,
    Offline {
        channels: usize,
        sample_rate: f32,
        length: usize,
    },
    Realtime,
}

static DEFAULT_MODE: std::sync::Mutex<AudioMode> = std::sync::Mutex::new(AudioMode::Silent);

/// The mode new runtimes take (`Runtime::audio_mode`): x3d-view sets `Realtime`, x3d-drive
/// `Offline` for an audio script. Silent otherwise.
pub fn set_default_mode(mode: AudioMode) {
    *DEFAULT_MODE.lock().unwrap() = mode;
}

pub fn default_mode() -> AudioMode {
    DEFAULT_MODE.lock().unwrap().clone()
}

static NEXT_UID: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

static DEFAULT_RECORD_OPS: std::sync::atomic::AtomicBool =
    std::sync::atomic::AtomicBool::new(false);

/// Whether new engines record their calls as [`ops::Op`]s (`AudioEngine::record_ops`): the page of X_ITE_R sets it, so
/// that the engine's Web Audio calls reach the page's AudioContext.
pub fn set_default_record_ops(on: bool) {
    DEFAULT_RECORD_OPS.store(on, std::sync::atomic::Ordering::Relaxed);
}

/// A Web Audio node of the engine's arena.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub struct AId(pub usize);

pub enum ANode {
    /// The Chromium GainHandler worklet (gain.rs) and its intrinsic value.
    Gain(web_audio_api::worklet::AudioWorkletNode, Arc<gain::Shared>),
    /// The Chromium-style oscillator (oscillator.rs), an AudioWorkletNode.
    Oscillator(web_audio_api::worklet::AudioWorkletNode),
    /// The Chromium AudioBufferSourceHandler worklet (buffer_source.rs).
    BufferSource(web_audio_api::worklet::AudioWorkletNode),
    /// The Chromium-style biquad filter (biquad.rs).
    Biquad(web_audio_api::worklet::AudioWorkletNode),
    /// The Chromium-style delay (delay.rs).
    Delay(web_audio_api::worklet::AudioWorkletNode),
    /// The Chromium-style wave shaper (wave_shaper.rs).
    WaveShaper(web_audio_api::worklet::AudioWorkletNode),
    /// The Chromium-style convolver (convolver.rs); true when its output is stereo.
    Convolver(web_audio_api::worklet::AudioWorkletNode, bool),
    /// The Chromium-style compressor (compressor.rs) and its `reduction`.
    Compressor(
        web_audio_api::worklet::AudioWorkletNode,
        Arc<compressor::Reduction>,
    ),
    /// The Chromium RealtimeAnalyser worklet and its control-thread half.
    Analyser(
        web_audio_api::worklet::AudioWorkletNode,
        Box<analyser::Analyser>,
    ),
    Splitter(ChannelSplitterNode),
    Merger(ChannelMergerNode),
    /// The Chromium-style equal-power panner (panner.rs) and, for "HRTF", web-audio-api's.
    Panner(Box<PannerSet>),
    StreamDestination(MediaStreamAudioDestinationNode),
    StreamSource(MediaStreamAudioSourceNode),
    Destination(AudioDestinationNode),
}

/// A PannerNode: the panning model chooses which node takes the input; both feed the outputs.
pub struct PannerSet {
    pub equal_power: web_audio_api::worklet::AudioWorkletNode,
    pub hrtf: PannerNode,
    pub hrtf_active: bool,
    pub attributes: panner::Attributes,
}

impl PannerSet {
    fn active(&self) -> &dyn AudioNode {
        if self.hrtf_active {
            &self.hrtf
        } else {
            &self.equal_power
        }
    }
}

impl ANode {
    /// The nodes whose outputs are this node's outputs.
    fn outputs(&self) -> Vec<&dyn AudioNode> {
        match self {
            ANode::Panner(p) => vec![&p.equal_power, &p.hrtf],
            n => vec![n.node()],
        }
    }

    /// The nodes that take the channel settings.
    fn all(&self) -> Vec<&dyn AudioNode> {
        self.outputs()
    }

    pub fn node(&self) -> &dyn AudioNode {
        match self {
            ANode::Gain(n, _) => n,
            ANode::Oscillator(n) => n,
            ANode::BufferSource(n) => n,
            ANode::Biquad(n) => n,
            ANode::Delay(n) => n,
            ANode::WaveShaper(n) => n,
            ANode::Convolver(n, _) => n,
            ANode::Compressor(n, _) => n,
            ANode::Analyser(n, _) => n,
            ANode::Splitter(n) => n,
            ANode::Merger(n) => n,
            ANode::Panner(p) => p.active(),
            ANode::StreamDestination(n) => n,
            ANode::StreamSource(n) => n,
            ANode::Destination(n) => n,
        }
    }
}

enum Ctx {
    Offline(Arc<OfflineAudioContext>),
    Realtime(AudioContext),
}

/// Rendering in step with the frames: the suspend futures of the frames and the render thread.
struct Stepping {
    suspends: Vec<Pin<Box<dyn std::future::Future<Output = ()> + Send>>>,
    next: usize,
    render: Option<std::thread::JoinHandle<AudioBuffer>>,
    waiting: bool,
}

pub struct AudioEngine {
    ctx: Ctx,
    pub mode: AudioMode,
    nodes: Vec<Option<ANode>>,
    /// (from, output, to, input): the connections made and not undone.
    connections: Vec<(AId, usize, AId, usize)>,
    pub destination: AId,
    stepping: Option<Stepping>,
    pub rendered: Option<AudioBuffer>,
    waves: std::collections::HashMap<oscillator::Shape, Arc<oscillator::WaveTables>>,
    convolver_normalize: std::collections::HashMap<usize, bool>,
    listener_values: [f32; 9],
    /// The MediaStreamAudioDestinationNodes: the graph pulls them (the oracle page connects its
    /// stand-in to the destination).
    stream_destinations: std::collections::HashSet<usize>,
    /// The buffer sources and oscillators told that the graph pulls them.
    pulled: std::collections::HashSet<usize>,
    /// `currentTime` from an outside audio context ([`AudioEngine::set_clock`]).
    clock: Option<f64>,
    /// The creation number of the node in each arena slot ([`AudioEngine::graph`] orders by it).
    serials: Vec<u64>,
    next_serial: u64,
    /// What a page could read back from each AudioBufferSourceNode ([`AudioEngine::graph`]).
    sources: std::collections::HashMap<usize, SourceInfo>,
    /// Whether the calls on the nodes are recorded in `ops` ([`ops`]).
    recording: bool,
    ops: std::sync::Mutex<Vec<Op>>,
    /// The files `decode` read while recording, by the identity of the buffer they gave ([`buffer_key`]).
    encoded:
        std::sync::Mutex<std::collections::HashMap<(usize, usize, usize), (u64, Arc<Vec<u8>>)>>,
    /// A number no other engine of this process has: the page names its mirror of this engine's nodes by it.
    uid: u64,
}

/// The state of an AudioBufferSourceNode as a page reads it back: its buffer, its loop attributes and
/// the calls of `start` and `stop` it got.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SourceInfo {
    /// `buffer`: (numberOfChannels, length, sampleRate), or none.
    pub buffer: Option<(usize, usize, f32)>,
    pub loop_: bool,
    pub loop_start: f64,
    pub loop_end: f64,
    /// The arguments of `start (when, offset)`.
    pub started: Option<(f64, f64)>,
    /// `stop (when)`.
    pub stopped: Option<f64>,
}

/// One node of [`AudioGraph`].
#[derive(Clone, Debug, PartialEq)]
pub struct GraphNode {
    /// The arena slot ([`AId`]).
    pub id: usize,
    /// The creation number: the order in which the nodes were made.
    pub serial: u64,
    /// The Web Audio class name, for example `GainNode`.
    pub kind: &'static str,
    pub number_of_inputs: usize,
    pub number_of_outputs: usize,
    pub channel_count: usize,
    pub channel_count_mode: &'static str,
    pub channel_interpretation: &'static str,
    /// `gain .value` of a GainNode.
    pub gain: Option<f32>,
    /// `playbackRate .value` and the [`SourceInfo`] of an AudioBufferSourceNode.
    pub playback_rate: Option<f32>,
    pub source: Option<SourceInfo>,
}

/// The audio graph of the engine as a page sees a Web Audio graph (a diagnostic snapshot): the live
/// nodes, the connections made and not undone, and the context's clock.
#[derive(Clone, Debug, PartialEq)]
pub struct AudioGraph {
    pub nodes: Vec<GraphNode>,
    /// (from, output, to, input), arena slots.
    pub connections: Vec<(usize, usize, usize, usize)>,
    pub destination: usize,
    pub current_time: f64,
    pub sample_rate: f32,
}

/// What identifies a decoded buffer: where its first channel is, its length and its channel count; and a checksum of the
/// samples, so that a buffer which later takes a freed address is not taken for the old one.
fn buffer_key(b: &AudioBuffer) -> ((usize, usize, usize), u64) {
    let first = b.get_channel_data(0);
    let sum = first.iter().fold(0xcbf2_9ce4_8422_2325_u64, |h, v| {
        (h ^ u64::from(v.to_bits())).wrapping_mul(0x0100_0000_01b3)
    });

    (
        (first.as_ptr() as usize, first.len(), b.number_of_channels()),
        sum,
    )
}

/// The name of an AudioParam as an op argument (the names `set_param` takes).
fn param_name(name: &str) -> &'static str {
    [
        "frequency",
        "detune",
        "playbackRate",
        "Q",
        "gain",
        "delayTime",
        "attack",
        "knee",
        "ratio",
        "release",
        "threshold",
        "positionX",
        "positionY",
        "positionZ",
        "orientationX",
        "orientationY",
        "orientationZ",
    ]
    .into_iter()
    .find(|n| *n == name)
    .unwrap_or("")
}

/// The render quantum size of Web Audio.
pub const RENDER_QUANTUM: usize = 128;

fn noop_waker() -> std::task::Waker {
    use std::task::{RawWaker, RawWakerVTable};

    fn clone(_: *const ()) -> RawWaker {
        RawWaker::new(std::ptr::null(), &VTABLE)
    }
    fn noop(_: *const ()) {}

    static VTABLE: RawWakerVTable = RawWakerVTable::new(clone, noop, noop, noop);

    unsafe { std::task::Waker::from_raw(RawWaker::new(std::ptr::null(), &VTABLE)) }
}

/// Wait for a future on this thread (the render thread wakes nobody; poll with short sleeps).
fn block_on<F: std::future::Future + ?Sized>(mut f: Pin<&mut F>) -> F::Output {
    let waker = noop_waker();
    let mut cx = TaskContext::from_waker(&waker);

    loop {
        if let Poll::Ready(v) = f.as_mut().poll(&mut cx) {
            return v;
        }

        std::thread::sleep(std::time::Duration::from_micros(50));
    }
}

impl AudioEngine {
    pub fn new(mode: AudioMode) -> AudioEngine {
        let ctx = match &mode {
            AudioMode::Silent => Ctx::Offline(Arc::new(OfflineAudioContext::new(
                2,
                RENDER_QUANTUM,
                48000.0,
            ))),
            AudioMode::Offline {
                channels,
                sample_rate,
                length,
            } => Ctx::Offline(Arc::new(OfflineAudioContext::new(
                *channels,
                *length,
                *sample_rate,
            ))),
            AudioMode::Realtime => match AudioContext::try_new(AudioContextOptions::default()) {
                Ok(c) => Ctx::Realtime(c),
                // No output device: the graph still works, nothing is heard.
                Err(_) => Ctx::Offline(Arc::new(OfflineAudioContext::new(
                    2,
                    RENDER_QUANTUM,
                    48000.0,
                ))),
            },
        };

        let mut engine = AudioEngine {
            ctx,
            mode,
            nodes: vec![],
            connections: vec![],
            destination: AId(0),
            stepping: None,
            rendered: None,
            waves: Default::default(),
            convolver_normalize: Default::default(),
            listener_values: [0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 1.0, 0.0],
            stream_destinations: Default::default(),
            pulled: Default::default(),
            clock: None,
            serials: vec![],
            next_serial: 0,
            sources: Default::default(),
            recording: DEFAULT_RECORD_OPS.load(std::sync::atomic::Ordering::Relaxed),
            ops: Default::default(),
            encoded: Default::default(),
            uid: NEXT_UID.fetch_add(1, std::sync::atomic::Ordering::Relaxed),
        };
        let destination = engine.base().destination();
        engine.destination = engine.add(ANode::Destination(destination));
        engine
    }

    /// The number that names this engine (see `uid`).
    pub fn uid(&self) -> u64 {
        self.uid
    }

    /// Record the calls on the nodes from now on ([`ops`]), or stop.
    pub fn record_ops(&mut self, on: bool) {
        self.recording = on;
    }

    /// The calls recorded since the last call, oldest first.
    pub fn take_ops(&self) -> Vec<Op> {
        std::mem::take(&mut *self.ops.lock().unwrap())
    }

    fn emit(&self, name: &'static str, args: Vec<Arg>) {
        if self.recording {
            self.ops.lock().unwrap().push(Op { name, args });
        }
    }

    pub fn base(&self) -> &ConcreteBaseAudioContext {
        match &self.ctx {
            Ctx::Offline(c) => c.base(),
            Ctx::Realtime(c) => c.base(),
        }
    }

    /// `audioContext .currentTime`: the context's own clock, or the clock [`AudioEngine::set_clock`] gives.
    pub fn current_time(&self) -> f64 {
        self.clock.unwrap_or_else(|| self.base().current_time())
    }

    /// Take `currentTime` from an outside audio context from now on. The browser build's engine is
    /// `Silent` and never renders, so its own clock stays at 0; the page gives it the clock of its
    /// AudioContext (X3DSoundContext.getAudioContext) before each frame, so the AudioElements and
    /// the time-dependent Sound nodes see the time pass as in X_ITE.
    pub fn set_clock(&mut self, time: f64) {
        self.clock = Some(time);
    }

    pub fn sample_rate(&self) -> f32 {
        self.base().sample_rate()
    }

    pub fn listener(&self) -> AudioListener {
        self.base().listener()
    }

    pub fn add(&mut self, node: ANode) -> AId {
        let serial = self.next_serial;

        self.next_serial += 1;

        if let Some(i) = self.nodes.iter().position(Option::is_none) {
            self.stream_destinations.remove(&i);
            self.pulled.remove(&i);
            self.sources.remove(&i);
            self.nodes[i] = Some(node);
            self.serials[i] = serial;
            return AId(i);
        }

        self.nodes.push(Some(node));
        self.serials.push(serial);
        AId(self.nodes.len() - 1)
    }

    /// A snapshot of the graph as a page would read it from Web Audio nodes (see [`AudioGraph`]).
    pub fn graph(&self) -> AudioGraph {
        let nodes = self
            .nodes
            .iter()
            .enumerate()
            .filter_map(|(i, n)| {
                let n = n.as_ref()?;
                let node = n.node();
                let kind = match n {
                    ANode::Gain(..) => "GainNode",
                    ANode::Oscillator(_) => "OscillatorNode",
                    ANode::BufferSource(_) => "AudioBufferSourceNode",
                    ANode::Biquad(_) => "BiquadFilterNode",
                    ANode::Delay(_) => "DelayNode",
                    ANode::WaveShaper(_) => "WaveShaperNode",
                    ANode::Convolver(..) => "ConvolverNode",
                    ANode::Compressor(..) => "DynamicsCompressorNode",
                    ANode::Analyser(..) => "AnalyserNode",
                    ANode::Splitter(_) => "ChannelSplitterNode",
                    ANode::Merger(_) => "ChannelMergerNode",
                    ANode::Panner(_) => "PannerNode",
                    ANode::StreamDestination(_) => "MediaStreamAudioDestinationNode",
                    ANode::StreamSource(_) => "MediaStreamAudioSourceNode",
                    ANode::Destination(_) => "AudioDestinationNode",
                };
                let source = matches!(n, ANode::BufferSource(_));

                Some(GraphNode {
                    id: i,
                    serial: self.serials[i],
                    kind,
                    number_of_inputs: node.number_of_inputs(),
                    number_of_outputs: node.number_of_outputs(),
                    channel_count: node.channel_count(),
                    channel_count_mode: match node.channel_count_mode() {
                        ChannelCountMode::Max => "max",
                        ChannelCountMode::ClampedMax => "clamped-max",
                        ChannelCountMode::Explicit => "explicit",
                    },
                    channel_interpretation: match node.channel_interpretation() {
                        ChannelInterpretation::Speakers => "speakers",
                        ChannelInterpretation::Discrete => "discrete",
                    },
                    gain: matches!(n, ANode::Gain(..)).then(|| self.param_value(AId(i), "gain")),
                    playback_rate: source.then(|| self.param_value(AId(i), "playbackRate")),
                    source: source.then(|| self.sources.get(&i).cloned().unwrap_or_default()),
                })
            })
            .collect();

        AudioGraph {
            nodes,
            connections: self
                .connections
                .iter()
                .map(|c| (c.0.0, c.1, c.2.0, c.3))
                .collect(),
            destination: self.destination.0,
            current_time: self.current_time(),
            sample_rate: self.sample_rate(),
        }
    }

    pub fn get(&self, id: AId) -> &ANode {
        self.nodes[id.0].as_ref().expect("live audio node")
    }

    pub fn get_mut(&mut self, id: AId) -> &mut ANode {
        self.nodes[id.0].as_mut().expect("live audio node")
    }

    /// Number of Web Audio nodes held (resource accounting).
    pub fn node_count(&self) -> usize {
        self.nodes.iter().filter(|n| n.is_some()).count()
    }

    pub fn connection_count(&self) -> usize {
        self.connections.len()
    }

    /// The connections made and not undone: (from, output, to, input).
    pub fn connections(&self) -> &[(AId, usize, AId, usize)] {
        &self.connections
    }

    /// Drop a node: its connections are undone and the handle released (the render graph frees
    /// it once nothing feeds it).
    pub fn release(&mut self, id: AId) {
        if id == self.destination || self.nodes.get(id.0).is_none_or(Option::is_none) {
            return;
        }

        self.disconnect(id);
        let incoming: Vec<_> = self
            .connections
            .iter()
            .filter(|c| c.2 == id)
            .copied()
            .collect();

        for (from, output, to, input) in incoming {
            self.disconnect_to(from, output, to, input);
        }

        self.emit("release", vec![Arg::Int(id.0)]);
        self.nodes[id.0] = None;
        self.convolver_normalize.remove(&id.0);
        self.update_pull();
    }

    /// Drop the handle of a node and forget its connections without undoing them: a source that
    /// was told to stop plays on until then, and the render graph frees it afterwards.
    pub fn release_keep_connections(&mut self, id: AId) {
        if id == self.destination || self.nodes.get(id.0).is_none_or(Option::is_none) {
            return;
        }

        self.connections.retain(|c| c.0 != id && c.2 != id);
        self.emit("releaseKeep", vec![Arg::Int(id.0)]);
        self.nodes[id.0] = None;
        self.convolver_normalize.remove(&id.0);
        self.update_pull();
    }

    /// Chromium processes a node only when a node it feeds is processed, from the destination
    /// and the automatic pull nodes (an AnalyserNode whose output is not connected); a
    /// MediaStreamAudioDestinationNode is pulled as well. `web-audio-api` renders every node, so
    /// each buffer source and oscillator is told whether the graph pulls it: an unpulled one keeps
    /// its read position or phase, as in Chromium. Called after every change of the connections.
    fn update_pull(&mut self) {
        let mut reached: std::collections::HashSet<usize> = (0..self.nodes.len())
            .filter(|&i| match &self.nodes[i] {
                Some(ANode::Destination(_)) => true,
                Some(ANode::Analyser(..)) => !self.connections.iter().any(|c| c.0.0 == i),
                Some(_) => self.stream_destinations.contains(&i),
                None => false,
            })
            .collect();
        let mut stack: Vec<usize> = reached.iter().copied().collect();

        while let Some(to) = stack.pop() {
            for c in self.connections.iter().filter(|c| c.2.0 == to) {
                if reached.insert(c.0.0) {
                    stack.push(c.0.0);
                }
            }
        }

        for i in 0..self.nodes.len() {
            let pulled = reached.contains(&i);

            if pulled != self.pulled.contains(&i) {
                let source = match &self.nodes[i] {
                    Some(ANode::BufferSource(n)) => {
                        n.port()
                            .post_message(Some(buffer_source::Message::Pulled(pulled)));
                        true
                    }
                    Some(ANode::Oscillator(n)) => {
                        n.port()
                            .post_message(Some(oscillator::Message::Pulled(pulled)));
                        true
                    }
                    _ => false,
                };

                if source {
                    if pulled {
                        self.pulled.insert(i);
                    } else {
                        self.pulled.remove(&i);
                    }
                }
            }
        }
    }

    // --- connections (AudioNode.connect / disconnect) -----------------------------------------

    pub fn connect(&mut self, from: AId, to: AId) {
        self.connect_io(from, to, 0, 0);
    }

    /// `from .connect (to, output, input)`. Connecting twice is one connection (Web Audio).
    pub fn connect_io(&mut self, from: AId, to: AId, output: usize, input: usize) {
        let (a, b) = (self.get(from).node(), self.get(to).node());

        if output >= a.number_of_outputs() || input >= b.number_of_inputs() {
            return;
        }

        for a in self.get(from).outputs() {
            a.connect_from_output_to_input(b, output, input);
        }

        if !self.connections.contains(&(from, output, to, input)) {
            self.connections.push((from, output, to, input));
            self.emit(
                "connect",
                vec![
                    Arg::Int(from.0),
                    Arg::Int(output),
                    Arg::Int(to.0),
                    Arg::Int(input),
                ],
            );
            self.update_pull();
        }
    }

    /// `from .disconnect ()`.
    pub fn disconnect(&mut self, from: AId) {
        if self.connections.iter().any(|c| c.0 == from) {
            for a in self.get(from).outputs() {
                a.disconnect();
            }
        }

        self.connections.retain(|c| c.0 != from);
        self.emit("disconnect", vec![Arg::Int(from.0)]);
        self.update_pull();
    }

    /// `from .disconnect (to)`: false where Web Audio throws (no such connection).
    pub fn disconnect_dest(&mut self, from: AId, to: AId) -> bool {
        if !self.connections.iter().any(|c| c.0 == from && c.2 == to) {
            return false;
        }

        for a in self.get(from).outputs() {
            a.disconnect_dest(self.get(to).node());
        }

        self.connections.retain(|c| !(c.0 == from && c.2 == to));
        self.emit("disconnectDest", vec![Arg::Int(from.0), Arg::Int(to.0)]);
        self.update_pull();
        true
    }

    /// `from .disconnect (to, output, input)`.
    pub fn disconnect_to(&mut self, from: AId, output: usize, to: AId, input: usize) -> bool {
        if !self.connections.contains(&(from, output, to, input)) {
            return false;
        }

        for a in self.get(from).outputs() {
            a.disconnect_dest_from_output_to_input(self.get(to).node(), output, input);
        }

        self.connections.retain(|c| *c != (from, output, to, input));
        self.emit(
            "disconnectTo",
            vec![
                Arg::Int(from.0),
                Arg::Int(output),
                Arg::Int(to.0),
                Arg::Int(input),
            ],
        );
        self.update_pull();
        true
    }

    pub fn number_of_inputs(&self, id: AId) -> usize {
        self.get(id).node().number_of_inputs()
    }

    pub fn number_of_outputs(&self, id: AId) -> usize {
        self.get(id).node().number_of_outputs()
    }

    pub fn set_channel_count(&self, id: AId, count: usize) {
        let node = self.get(id).node();

        // Web Audio throws NotSupportedError for counts the node cannot take (the setter is
        // skipped like the rest of the callback would be).
        if matches!(
            self.get(id),
            ANode::Destination(_) | ANode::Splitter(_) | ANode::Merger(_)
        ) && node.channel_count() != count
        {
            return;
        }

        // DynamicsCompressorHandler, PannerHandler and ConvolverHandler::SetChannelCount: 1 or 2.
        if matches!(
            self.get(id),
            ANode::Compressor(..) | ANode::Panner(_) | ANode::Convolver(..)
        ) && !(1..=2).contains(&count)
        {
            return;
        }

        for n in self.get(id).all() {
            n.set_channel_count(count);
        }

        self.emit("channelCount", vec![Arg::Int(id.0), Arg::Int(count)]);
    }

    pub fn set_channel_count_mode(&self, id: AId, mode: &str) {
        let mode = match mode {
            "clamped-max" => ChannelCountMode::ClampedMax,
            "explicit" => ChannelCountMode::Explicit,
            _ => ChannelCountMode::Max,
        };
        let node = self.get(id).node();

        if matches!(self.get(id), ANode::Splitter(_) | ANode::Merger(_))
            && node.channel_count_mode() != mode
        {
            return;
        }

        // DynamicsCompressorHandler, PannerHandler and ConvolverHandler::SetChannelCountMode:
        // "max" is a NotSupportedError.
        if matches!(
            self.get(id),
            ANode::Compressor(..) | ANode::Panner(_) | ANode::Convolver(..)
        ) && mode == ChannelCountMode::Max
        {
            return;
        }

        for n in self.get(id).all() {
            n.set_channel_count_mode(mode);
        }

        self.emit(
            "channelCountMode",
            vec![
                Arg::Int(id.0),
                Arg::Str(match mode {
                    ChannelCountMode::ClampedMax => "clamped-max",
                    ChannelCountMode::Explicit => "explicit",
                    _ => "max",
                }),
            ],
        );
    }

    pub fn set_channel_interpretation(&self, id: AId, interpretation: &str) {
        let interpretation = if interpretation == "discrete" {
            ChannelInterpretation::Discrete
        } else {
            ChannelInterpretation::Speakers
        };
        let node = self.get(id).node();

        if matches!(self.get(id), ANode::Splitter(_))
            && node.channel_interpretation() != interpretation
        {
            return;
        }

        if let ANode::Analyser(_, a) = self.get(id) {
            a.shared().discrete.store(
                interpretation == ChannelInterpretation::Discrete,
                std::sync::atomic::Ordering::Relaxed,
            );
        }

        for n in self.get(id).all() {
            n.set_channel_interpretation(interpretation);
        }

        self.emit(
            "channelInterpretation",
            vec![
                Arg::Int(id.0),
                Arg::Str(if interpretation == ChannelInterpretation::Discrete {
                    "discrete"
                } else {
                    "speakers"
                }),
            ],
        );
    }

    // --- node constructors ------------------------------------------------------------------

    /// `new GainNode (audioContext, { gain })`: the intrinsic value, no timeline event.
    pub fn gain(&mut self, gain: f32) -> AId {
        use web_audio_api::worklet::{AudioWorkletNode, AudioWorkletNodeOptions};

        let shared = Arc::new(gain::Shared::new(gain));
        let options = AudioWorkletNodeOptions {
            number_of_inputs: 1,
            number_of_outputs: 1,
            output_channel_count: vec![],
            parameter_data: Default::default(),
            processor_options: Arc::clone(&shared),
            audio_node_options: Default::default(),
        };
        let node = AudioWorkletNode::new::<gain::Processor>(self.base(), options);
        let id = self.add(ANode::Gain(node, shared));

        self.emit("gain", vec![Arg::Int(id.0), Arg::Num(f64::from(gain))]);
        id
    }

    fn emit_gain_event(&self, id: AId, kind: &'static str, value: f32, time: f64) {
        self.emit(
            "gainEvent",
            vec![
                Arg::Int(id.0),
                Arg::Str(kind),
                Arg::Num(f64::from(value)),
                Arg::Num(time),
            ],
        );
    }

    fn post_gain(&self, id: AId, kind: param_timeline::Kind, value: f32, time: f64) {
        if let ANode::Gain(n, _) = self.get(id) {
            n.port()
                .post_message(Some(gain::Message::Event(param_timeline::Event {
                    kind,
                    value,
                    time,
                })));
        }
    }

    /// `gain .gain .setValueAtTime (value, time)`.
    pub fn gain_set_value_at_time(&self, id: AId, value: f32, time: f64) {
        self.emit_gain_event(id, "setValue", value, time);
        self.post_gain(id, param_timeline::Kind::SetValue, value, time);
    }

    /// `gain .gain .exponentialRampToValueAtTime (value, time)`.
    pub fn gain_exponential_ramp(&self, id: AId, value: f32, time: f64) {
        self.emit_gain_event(id, "exponentialRamp", value, time);
        self.post_gain(id, param_timeline::Kind::ExponentialRamp, value, time);
    }

    /// The AudioParam a JavaScript `node .<name> .value = v` addresses.
    pub fn param(&self, id: AId, name: &str) -> Option<&AudioParam> {
        Some(match (self.get(id), name) {
            (ANode::Oscillator(n), "frequency" | "detune") => n.parameters().get(name)?,
            (ANode::BufferSource(n), "playbackRate" | "detune") => n.parameters().get(name)?,
            (ANode::Biquad(n), "frequency" | "detune" | "Q" | "gain") => {
                n.parameters().get(name)?
            }
            (ANode::Delay(n), "delayTime") => n.parameters().get(name)?,
            (ANode::Compressor(n, _), "attack" | "knee" | "ratio" | "release" | "threshold") => {
                n.parameters().get(name)?
            }
            _ => return None,
        })
    }

    /// `node .<param> .value = value`. A non-finite value throws a TypeError in Web Audio;
    /// returns false then.
    pub fn set_param(&self, id: AId, name: &str, value: f64) -> bool {
        let v = value as f32;

        if !v.is_finite() {
            return false;
        }

        if let Some(p) = self.param(id, name) {
            p.set_value(v);
        }

        // AudioParam .value setter: the intrinsic value, then setValueAtTime (intrinsic, now).
        if let (ANode::Gain(_, shared), "gain") = (self.get(id), name) {
            shared.set(v);
            self.post_gain(
                id,
                param_timeline::Kind::SetValue,
                shared.get(),
                self.current_time(),
            );
        }

        // The page's node takes `.value = v` itself (its own setValueAtTime of the current time).
        self.emit(
            "param",
            vec![
                Arg::Int(id.0),
                Arg::Str(param_name(name)),
                Arg::Num(f64::from(v)),
            ],
        );

        // `.value = v` is setValueAtTime (v, currentTime): the kernels that switch to their
        // sample-accurate code keep the event times (param_timeline.rs).
        let now = self.current_time();

        match (self.get(id), name) {
            (ANode::Oscillator(n), "frequency" | "detune") => n.port().post_message(Some(
                oscillator::Message::ParamEvent(name == "frequency", now),
            )),
            (ANode::Biquad(n), "frequency" | "Q" | "gain" | "detune") => {
                let index = ["frequency", "Q", "gain", "detune"]
                    .iter()
                    .position(|p| *p == name)
                    .unwrap();
                n.port()
                    .post_message(Some(biquad::Message::ParamEvent(index, now)));
            }
            (ANode::Panner(p), _) => {
                if let Some(index) = panner::PARAM_NAMES.iter().position(|p| *p == name) {
                    p.equal_power
                        .port()
                        .post_message(Some(panner::Message::SetValue(index, v, now)));
                }
            }
            _ => {}
        }

        if let ANode::Panner(p) = self.get(id) {
            let h = &p.hrtf;
            let param = match name {
                "positionX" => h.position_x(),
                "positionY" => h.position_y(),
                "positionZ" => h.position_z(),
                "orientationX" => h.orientation_x(),
                "orientationY" => h.orientation_y(),
                _ => h.orientation_z(),
            };
            param.set_value(v);
        }

        true
    }

    pub fn param_value(&self, id: AId, name: &str) -> f32 {
        if let (ANode::Gain(_, shared), "gain") = (self.get(id), name) {
            return shared.get();
        }

        self.param(id, name).map_or(0.0, |p| p.value())
    }

    /// `new OscillatorNode (audioContext)`: type "sine".
    pub fn oscillator(&mut self) -> AId {
        let id = self.oscillator_node();

        self.emit("oscillator", vec![Arg::Int(id.0)]);
        id
    }

    fn oscillator_node(&mut self) -> AId {
        use web_audio_api::worklet::{AudioWorkletNode, AudioWorkletNodeOptions};

        let wave = self.basic_wave(oscillator::Shape::Sine);
        let options = AudioWorkletNodeOptions {
            number_of_inputs: 0,
            number_of_outputs: 1,
            output_channel_count: vec![1],
            parameter_data: Default::default(),
            processor_options: wave,
            audio_node_options: AudioNodeOptions::default(),
        };
        let node = AudioWorkletNode::new::<oscillator::Processor>(self.base(), options);
        self.add(ANode::Oscillator(node))
    }

    /// The context's PeriodicWave of a basic type (BaseAudioContext.GetPeriodicWave), made once.
    fn basic_wave(&mut self, shape: oscillator::Shape) -> Arc<oscillator::WaveTables> {
        let rate = self.sample_rate();
        Arc::clone(
            self.waves
                .entry(shape)
                .or_insert_with(|| Arc::new(oscillator::WaveTables::basic(rate, shape))),
        )
    }

    fn post(&self, id: AId, message: oscillator::Message) {
        if let ANode::Oscillator(n) = self.get(id) {
            n.port().post_message(Some(message));
        }
    }

    /// `oscillatorNode .type = …` (not "custom").
    pub fn oscillator_set_type(&mut self, id: AId, shape: oscillator::Shape) {
        let wave = self.basic_wave(shape);
        self.emit(
            "oscillatorType",
            vec![
                Arg::Int(id.0),
                Arg::Str(match shape {
                    oscillator::Shape::Sine => "sine",
                    oscillator::Shape::Square => "square",
                    oscillator::Shape::Sawtooth => "sawtooth",
                    oscillator::Shape::Triangle => "triangle",
                }),
            ],
        );
        self.post(id, oscillator::Message::Wave(wave));
    }

    /// `oscillatorNode .setPeriodicWave (audioContext .createPeriodicWave (real, imag))`.
    pub fn oscillator_set_periodic_wave(&mut self, id: AId, real: &[f32], imag: &[f32]) {
        let wave = Arc::new(oscillator::WaveTables::new(
            self.sample_rate(),
            real,
            imag,
            false,
        ));
        self.emit(
            "oscillatorWave",
            vec![
                Arg::Int(id.0),
                Arg::List(real.to_vec()),
                Arg::List(imag.to_vec()),
            ],
        );
        self.post(id, oscillator::Message::Wave(wave));
    }

    /// `oscillatorNode .start ()`.
    pub fn oscillator_start(&mut self, id: AId) {
        self.emit("oscillatorStart", vec![Arg::Int(id.0)]);
        self.post(id, oscillator::Message::Start(0.0));
    }

    /// `oscillatorNode .stop (when)`.
    pub fn oscillator_stop(&mut self, id: AId, when: f64) {
        self.emit(
            "oscillatorStop",
            vec![Arg::Int(id.0), Arg::Num(when.max(0.0))],
        );
        self.post(id, oscillator::Message::Stop(when.max(0.0)));
    }

    /// `new AudioBufferSourceNode (audioContext)` and `.buffer = buffer`: the output has one
    /// channel per buffer channel (one without a buffer).
    pub fn buffer_source(&mut self, buffer: Option<&AudioBuffer>) -> AId {
        use web_audio_api::worklet::{AudioWorkletNode, AudioWorkletNodeOptions};

        let shared = buffer.map(|b| {
            Arc::new(buffer_source::Buffer {
                channels: (0..b.number_of_channels())
                    .map(|c| b.get_channel_data(c).to_vec())
                    .collect(),
                sample_rate: b.sample_rate(),
            })
        });
        let channels = buffer.map_or(1, |b| b.number_of_channels());
        let options = AudioWorkletNodeOptions {
            number_of_inputs: 0,
            number_of_outputs: 1,
            output_channel_count: vec![channels],
            parameter_data: Default::default(),
            processor_options: shared,
            audio_node_options: Default::default(),
        };
        let node = AudioWorkletNode::new::<buffer_source::Processor>(self.base(), options);
        let info = SourceInfo {
            buffer: buffer.map(|b| (b.number_of_channels(), b.length(), b.sample_rate())),
            ..Default::default()
        };
        let id = self.add(ANode::BufferSource(node));

        self.sources.insert(id.0, info);
        self.emit(
            "bufferSource",
            vec![
                Arg::Int(id.0),
                buffer.map_or(Arg::Bool(false), |b| self.buffer_arg(b)),
            ],
        );
        id
    }

    fn source_info(&mut self, id: AId) -> Option<&mut SourceInfo> {
        self.sources.get_mut(&id.0)
    }

    fn post_buffer_source(&self, id: AId, message: buffer_source::Message) {
        if let ANode::BufferSource(n) = self.get(id) {
            n.port().post_message(Some(message));
        }
    }

    /// `audioBufferSource .start (when, offset)`: `when` becomes max (when, currentTime);
    /// ClampGrainParameters reads playbackRate and detune now.
    pub fn buffer_source_start(&mut self, id: AId, when: f64, offset: f64) {
        let value = |name| self.param(id, name).map_or(0.0, |p| p.value());
        let unit_rate = value("playbackRate") == 1.0 && value("detune") == 0.0;
        if let Some(info) = self.source_info(id) {
            info.started = Some((when, offset));
        }
        self.emit(
            "sourceStart",
            vec![Arg::Int(id.0), Arg::Num(when), Arg::Num(offset)],
        );
        let when = when.max(self.current_time());
        self.post_buffer_source(
            id,
            buffer_source::Message::Start {
                when,
                offset,
                unit_rate,
            },
        );
    }

    /// `audioBufferSource .stop (when)`.
    pub fn buffer_source_stop(&mut self, id: AId, when: f64) {
        if let Some(info) = self.source_info(id) {
            info.stopped = Some(when);
        }
        self.emit("sourceStop", vec![Arg::Int(id.0), Arg::Num(when)]);
        self.post_buffer_source(id, buffer_source::Message::Stop(when));
    }

    pub fn buffer_source_set_loop(&mut self, id: AId, v: bool) {
        if let Some(info) = self.source_info(id) {
            info.loop_ = v;
        }
        self.emit("sourceLoop", vec![Arg::Int(id.0), Arg::Bool(v)]);
        self.post_buffer_source(id, buffer_source::Message::Loop(v));
    }

    pub fn buffer_source_set_loop_start(&mut self, id: AId, v: f64) {
        if let Some(info) = self.source_info(id) {
            info.loop_start = v;
        }
        self.emit("sourceLoopStart", vec![Arg::Int(id.0), Arg::Num(v)]);
        self.post_buffer_source(id, buffer_source::Message::LoopStart(v));
    }

    pub fn buffer_source_set_loop_end(&mut self, id: AId, v: f64) {
        if let Some(info) = self.source_info(id) {
            info.loop_end = v;
        }
        self.emit("sourceLoopEnd", vec![Arg::Int(id.0), Arg::Num(v)]);
        self.post_buffer_source(id, buffer_source::Message::LoopEnd(v));
    }

    /// `new BiquadFilterNode (audioContext)`: "lowpass".
    pub fn biquad(&mut self) -> AId {
        use web_audio_api::worklet::{AudioWorkletNode, AudioWorkletNodeOptions};

        let node = AudioWorkletNode::new::<biquad::Processor>(
            self.base(),
            AudioWorkletNodeOptions::<()>::default(),
        );
        let id = self.add(ANode::Biquad(node));

        self.emit("biquad", vec![Arg::Int(id.0)]);
        id
    }

    /// `biquadFilterNode .type = value`.
    pub fn biquad_set_type(&self, id: AId, value: biquad::Type) {
        self.emit(
            "biquadType",
            vec![
                Arg::Int(id.0),
                Arg::Str(match value {
                    biquad::Type::Lowpass => "lowpass",
                    biquad::Type::Highpass => "highpass",
                    biquad::Type::Bandpass => "bandpass",
                    biquad::Type::Lowshelf => "lowshelf",
                    biquad::Type::Highshelf => "highshelf",
                    biquad::Type::Peaking => "peaking",
                    biquad::Type::Notch => "notch",
                    biquad::Type::Allpass => "allpass",
                }),
            ],
        );
        if let ANode::Biquad(n) = self.get(id) {
            n.port().post_message(Some(biquad::Message::Type(value)));
        }
    }

    /// `new DelayNode (audioContext, { maxDelayTime, delayTime })`. Web Audio throws for a
    /// maxDelayTime outside (0, 180): `None` then.
    pub fn delay(&mut self, max_delay_time: f64, delay_time: f64) -> Option<AId> {
        if !(max_delay_time > 0.0 && max_delay_time < 180.0) {
            return None;
        }

        use web_audio_api::worklet::{AudioWorkletNode, AudioWorkletNodeOptions};

        let mut options = AudioWorkletNodeOptions::<f64>::default();
        options.processor_options = max_delay_time;
        options
            .parameter_data
            .insert("delayTime".into(), delay_time);
        let node = AudioWorkletNode::new::<delay::Processor>(self.base(), options);
        let id = self.add(ANode::Delay(node));

        self.emit(
            "delay",
            vec![
                Arg::Int(id.0),
                Arg::Num(max_delay_time),
                Arg::Num(delay_time),
            ],
        );
        Some(id)
    }

    /// `new WaveShaperNode (audioContext)`.
    pub fn wave_shaper(&mut self) -> AId {
        use web_audio_api::worklet::{AudioWorkletNode, AudioWorkletNodeOptions};

        let options = AudioWorkletNodeOptions::<()>::default();
        let node = AudioWorkletNode::new::<wave_shaper::Processor>(self.base(), options);
        let id = self.add(ANode::WaveShaper(node));

        self.emit("waveShaper", vec![Arg::Int(id.0)]);
        id
    }

    /// `waveShaperNode .curve = curve`.
    pub fn wave_shaper_set_curve(&self, id: AId, curve: Vec<f32>) {
        self.emit(
            "waveShaperCurve",
            vec![Arg::Int(id.0), Arg::List(curve.clone())],
        );
        if let ANode::WaveShaper(n) = self.get(id) {
            n.port()
                .post_message(Some(wave_shaper::Message::Curve(Arc::new(curve))));
        }
    }

    /// `waveShaperNode .oversample = value`.
    pub fn wave_shaper_set_oversample(&self, id: AId, value: wave_shaper::Oversample) {
        self.emit(
            "waveShaperOversample",
            vec![
                Arg::Int(id.0),
                Arg::Str(match value {
                    wave_shaper::Oversample::None => "none",
                    wave_shaper::Oversample::X2 => "2x",
                    wave_shaper::Oversample::X4 => "4x",
                }),
            ],
        );
        if let ANode::WaveShaper(n) = self.get(id) {
            n.port()
                .post_message(Some(wave_shaper::Message::Oversample(value)));
        }
    }

    /// `new ConvolverNode (audioContext)`: no buffer, normalize, channelCount 2, "clamped-max".
    pub fn convolver(&mut self) -> AId {
        let node = self.convolver_node(false);
        let id = self.add(ANode::Convolver(node, false));
        self.convolver_normalize.insert(id.0, true);
        self.emit("convolver", vec![Arg::Int(id.0)]);
        id
    }

    /// The Chromium-style convolver (convolver.rs). Its output has as many channels as its input
    /// (a response of one channel or none), or two (ComputeNumberOfOutputChannels).
    fn convolver_node(&self, stereo: bool) -> web_audio_api::worklet::AudioWorkletNode {
        use web_audio_api::worklet::{AudioWorkletNode, AudioWorkletNodeOptions};

        let audio_node_options = AudioNodeOptions {
            channel_count: 2,
            channel_count_mode: ChannelCountMode::ClampedMax,
            channel_interpretation: ChannelInterpretation::Speakers,
        };
        let output_channel_count = if stereo { vec![2] } else { vec![] };
        let options = AudioWorkletNodeOptions::<()> {
            number_of_inputs: 1,
            number_of_outputs: 1,
            output_channel_count,
            audio_node_options,
            ..Default::default()
        };
        AudioWorkletNode::new::<convolver::Processor>(self.base(), options)
    }

    /// `convolverNode .normalize = value`: used by the next `.buffer = …`.
    pub fn convolver_set_normalize(&mut self, id: AId, value: bool) {
        self.convolver_normalize.insert(id.0, value);
        self.emit("convolverNormalize", vec![Arg::Int(id.0), Arg::Bool(value)]);
    }

    /// `convolverNode .buffer = buffer` (ConvolverHandler::SetBuffer): 1, 2 or 4 channels.
    pub fn convolver_set_buffer(&mut self, id: AId, buffer: AudioBuffer) {
        let channels: Vec<Vec<f32>> = (0..buffer.number_of_channels())
            .map(|c| buffer.get_channel_data(c).to_vec())
            .collect();

        if !matches!(channels.len(), 1 | 2 | 4) {
            return;
        }

        self.emit(
            "convolverBuffer",
            vec![Arg::Int(id.0), self.buffer_arg(&buffer)],
        );
        let normalize = self.convolver_normalize.get(&id.0).copied().unwrap_or(true);
        let reverb = Box::new(convolver::Reverb::new(
            &channels,
            buffer.sample_rate(),
            normalize,
        ));
        let stereo = reverb.response_channels() > 1;

        if matches!(self.get(id), ANode::Convolver(_, s) if *s != stereo) {
            let node = self.convolver_node(stereo);
            let (count, mode, interpretation) = {
                let n = self.get(id).node();
                (
                    n.channel_count(),
                    n.channel_count_mode(),
                    n.channel_interpretation(),
                )
            };
            node.set_channel_count(count);
            node.set_channel_count_mode(mode);
            node.set_channel_interpretation(interpretation);
            self.replace(id, ANode::Convolver(node, stereo));
        }

        if let ANode::Convolver(n, _) = self.get(id) {
            n.port()
                .post_message(Some(convolver::Message::Reverb(Some(reverb))));
        }
    }

    /// Put another Web Audio node in the place of `id`, with the same connections.
    fn replace(&mut self, id: AId, node: ANode) {
        let incoming: Vec<_> = self
            .connections
            .iter()
            .filter(|c| c.2 == id)
            .copied()
            .collect();
        let outgoing: Vec<_> = self
            .connections
            .iter()
            .filter(|c| c.0 == id)
            .copied()
            .collect();

        for &(from, ..) in incoming.iter() {
            for a in self.get(from).outputs() {
                a.disconnect_dest(self.get(id).node());
            }
        }

        if !outgoing.is_empty() {
            for a in self.get(id).outputs() {
                a.disconnect();
            }
        }

        self.nodes[id.0] = Some(node);

        for (from, output, _, input) in incoming {
            for a in self.get(from).outputs() {
                a.connect_from_output_to_input(self.get(id).node(), output, input);
            }
        }

        for (_, output, to, input) in outgoing {
            for a in self.get(id).outputs() {
                a.connect_from_output_to_input(self.get(to).node(), output, input);
            }
        }
    }

    /// `new DynamicsCompressorNode (audioContext)`: two channels, "clamped-max", two output channels.
    pub fn compressor(&mut self) -> AId {
        use web_audio_api::worklet::{AudioWorkletNode, AudioWorkletNodeOptions};

        let reduction = Arc::new(compressor::Reduction::default());
        let audio_node_options = AudioNodeOptions {
            channel_count: 2,
            channel_count_mode: ChannelCountMode::ClampedMax,
            channel_interpretation: ChannelInterpretation::Speakers,
        };
        let options = AudioWorkletNodeOptions {
            number_of_inputs: 1,
            number_of_outputs: 1,
            output_channel_count: vec![2],
            parameter_data: Default::default(),
            processor_options: Arc::clone(&reduction),
            audio_node_options,
        };
        let node = AudioWorkletNode::new::<compressor::Processor>(self.base(), options);
        let id = self.add(ANode::Compressor(node, reduction));

        self.emit("compressor", vec![Arg::Int(id.0)]);
        id
    }

    /// `dynamicsCompressorNode .reduction`.
    pub fn compressor_reduction(&self, id: AId) -> f32 {
        match self.get(id) {
            ANode::Compressor(_, r) => r.get(),
            _ => 0.0,
        }
    }

    pub fn analyser(&mut self) -> AId {
        use web_audio_api::worklet::{AudioWorkletNode, AudioWorkletNodeOptions};

        let shared = Arc::new(analyser::Shared::default());
        // AnalyserNode: channelCount 2, "max", "speakers"; the output follows the input.
        let audio_node_options = AudioNodeOptions {
            channel_count: 2,
            channel_count_mode: ChannelCountMode::Max,
            channel_interpretation: ChannelInterpretation::Speakers,
        };
        let options = AudioWorkletNodeOptions {
            number_of_inputs: 1,
            number_of_outputs: 1,
            output_channel_count: vec![],
            parameter_data: Default::default(),
            processor_options: Arc::clone(&shared),
            audio_node_options,
        };
        let node = AudioWorkletNode::new::<analyser::Processor>(self.base(), options);
        let id = self.add(ANode::Analyser(
            node,
            Box::new(analyser::Analyser::new(shared)),
        ));

        self.emit("analyser", vec![Arg::Int(id.0)]);
        id
    }

    /// An AnalyserNode setting that the control half (`analyser.rs`) took: the page's node takes it too.
    pub fn analyser_setting(&self, id: AId, name: &'static str, values: &[f64]) {
        let mut args = vec![Arg::Int(id.0)];

        args.extend(values.iter().map(|&v| Arg::Num(v)));
        self.emit(name, args);
    }

    pub fn splitter(&mut self, number_of_outputs: usize) -> AId {
        let node = ChannelSplitterNode::new(
            self.base(),
            ChannelSplitterOptions {
                number_of_outputs,
                ..Default::default()
            },
        );
        let id = self.add(ANode::Splitter(node));

        self.emit(
            "splitter",
            vec![Arg::Int(id.0), Arg::Int(number_of_outputs)],
        );
        id
    }

    pub fn merger(&mut self, number_of_inputs: usize) -> AId {
        let node = ChannelMergerNode::new(
            self.base(),
            ChannelMergerOptions {
                number_of_inputs,
                ..Default::default()
            },
        );
        let id = self.add(ANode::Merger(node));

        self.emit("merger", vec![Arg::Int(id.0), Arg::Int(number_of_inputs)]);
        id
    }

    /// `new PannerNode (audioContext)`: "equalpower", "inverse", channelCount 2, "clamped-max".
    pub fn panner(&mut self) -> AId {
        use web_audio_api::worklet::{AudioWorkletNode, AudioWorkletNodeOptions};

        let audio_node_options = AudioNodeOptions {
            channel_count: 2,
            channel_count_mode: ChannelCountMode::ClampedMax,
            channel_interpretation: ChannelInterpretation::Speakers,
        };
        let options = AudioWorkletNodeOptions {
            number_of_inputs: 1,
            number_of_outputs: 1,
            output_channel_count: vec![2],
            parameter_data: Default::default(),
            processor_options: self.listener_values,
            audio_node_options,
        };
        let equal_power = AudioWorkletNode::new::<panner::Processor>(self.base(), options);
        let hrtf = PannerNode::new(self.base(), PannerOptions::default());
        let id = self.add(ANode::Panner(Box::new(PannerSet {
            equal_power,
            hrtf,
            hrtf_active: false,
            attributes: Default::default(),
        })));

        self.emit("panner", vec![Arg::Int(id.0)]);
        id
    }

    /// The PannerNode attribute setters.
    pub fn panner_set(&mut self, id: AId, f: impl FnOnce(&mut panner::Attributes)) {
        let ANode::Panner(p) = self.get_mut(id) else {
            return;
        };
        f(&mut p.attributes);
        let a = p.attributes;
        p.equal_power
            .port()
            .post_message(Some(panner::Message::Attributes(a)));
        let h = &mut p.hrtf;
        h.set_distance_model(match a.distance_model {
            panner::DistanceModel::Linear => DistanceModelType::Linear,
            panner::DistanceModel::Inverse => DistanceModelType::Inverse,
            panner::DistanceModel::Exponential => DistanceModelType::Exponential,
        });
        h.set_ref_distance(a.ref_distance);
        h.set_max_distance(a.max_distance);
        h.set_rolloff_factor(a.rolloff_factor);
        h.set_cone_inner_angle(a.cone_inner_angle);
        h.set_cone_outer_angle(a.cone_outer_angle);
        h.set_cone_outer_gain(a.cone_outer_gain);
        self.emit(
            "pannerAttributes",
            vec![
                Arg::Int(id.0),
                Arg::Str(match a.distance_model {
                    panner::DistanceModel::Linear => "linear",
                    panner::DistanceModel::Inverse => "inverse",
                    panner::DistanceModel::Exponential => "exponential",
                }),
                Arg::Num(a.ref_distance),
                Arg::Num(a.max_distance),
                Arg::Num(a.rolloff_factor),
                Arg::Num(a.cone_inner_angle),
                Arg::Num(a.cone_outer_angle),
                Arg::Num(a.cone_outer_gain),
            ],
        );
    }

    /// `pannerNode .panningModel = hrtf ? "HRTF" : "equalpower"`: the inputs move to the other
    /// node.
    pub fn panner_set_hrtf(&mut self, id: AId, hrtf: bool) {
        let ANode::Panner(p) = self.get(id) else {
            return;
        };

        if p.hrtf_active == hrtf {
            return;
        }

        self.emit("pannerModel", vec![Arg::Int(id.0), Arg::Bool(hrtf)]);

        let incoming: Vec<_> = self
            .connections
            .iter()
            .filter(|c| c.2 == id)
            .copied()
            .collect();
        let sources: Vec<AId> = incoming.iter().map(|c| c.0).collect();

        for &from in sources.iter() {
            for a in self.get(from).outputs() {
                a.disconnect_dest(self.get(id).node());
            }
        }

        if let ANode::Panner(p) = self.get_mut(id) {
            p.hrtf_active = hrtf;

            if hrtf {
                p.hrtf.set_panning_model(PanningModelType::HRTF);
            }
        }

        for (from, output, _, input) in incoming {
            for a in self.get(from).outputs() {
                a.connect_from_output_to_input(self.get(id).node(), output, input);
            }
        }
    }

    pub fn listener_values(&self) -> [f32; 9] {
        self.listener_values
    }

    /// `audioContext .listener .{position,forward,up}{X,Y,Z} .value = …`.
    pub fn set_listener(&mut self, values: [f32; 9]) {
        let l = self.listener();

        for (param, v) in [
            l.position_x(),
            l.position_y(),
            l.position_z(),
            l.forward_x(),
            l.forward_y(),
            l.forward_z(),
            l.up_x(),
            l.up_y(),
            l.up_z(),
        ]
        .into_iter()
        .zip(values)
        {
            param.set_value(v);
        }

        // The listener's setValueAtTime (v, currentTime) events, for each panner's copy.
        let now = self.current_time();

        for n in self.nodes.iter().flatten() {
            if let ANode::Panner(p) = n {
                for (k, v) in values.into_iter().enumerate() {
                    p.equal_power
                        .port()
                        .post_message(Some(panner::Message::SetValue(6 + k, v, now)));
                }
            }
        }

        self.emit(
            "listener",
            values.iter().map(|&v| Arg::Num(f64::from(v))).collect(),
        );
        self.listener_values = values;
    }

    /// `new MediaStreamAudioDestinationNode (audioContext)`: channelCount 2, "explicit",
    /// "speakers". With an OfflineAudioContext (which a browser does not allow for this node) it
    /// is the stereo GainNode the oracle page substitutes, connected to the destination.
    pub fn stream_destination(&mut self, to_output: bool) -> AId {
        if to_output || matches!(self.ctx, Ctx::Offline(_)) {
            let id = self.gain(1.0);
            self.set_channel_count(id, 2);
            self.set_channel_count_mode(id, "explicit");
            self.set_channel_interpretation(id, "speakers");

            if to_output {
                let destination = self.destination;
                self.connect(id, destination);
            }

            self.stream_destinations.insert(id.0);
            self.update_pull();
            return id;
        }

        let node = MediaStreamAudioDestinationNode::new(
            self.base(),
            AudioNodeOptions {
                channel_count: 2,
                channel_count_mode: ChannelCountMode::Explicit,
                channel_interpretation: ChannelInterpretation::Speakers,
            },
        );
        let id = self.add(ANode::StreamDestination(node));
        self.stream_destinations.insert(id.0);
        id
    }

    /// The microphone: `new MediaStreamAudioSourceNode (audioContext, { mediaStream })` for the
    /// stream of getUserMedia. Only a realtime context can open one.
    pub fn microphone(&mut self, device_id: &str) -> Result<AId, String> {
        let Ctx::Realtime(ctx) = &self.ctx else {
            return Err("Requested device not found".to_string());
        };

        use web_audio_api::media_devices::{
            self, MediaDeviceInfoKind, MediaStreamConstraints, MediaTrackConstraints,
        };

        let inputs: Vec<_> = media_devices::enumerate_devices_sync()
            .into_iter()
            .filter(|d| d.kind() == MediaDeviceInfoKind::AudioInput)
            .collect();

        if inputs.is_empty() {
            return Err("Requested device not found".to_string());
        }

        let constraints = if device_id.is_empty() {
            MediaStreamConstraints::Audio
        } else if inputs.iter().any(|d| d.device_id() == device_id) {
            // getUserMedia ({ audio: { deviceId } }): a bare deviceId is an ideal constraint, an
            // unknown one falls back to the default device.
            let mut c = MediaTrackConstraints::default();
            c.device_id = Some(device_id.to_string());
            MediaStreamConstraints::AudioWithConstraints(c)
        } else {
            MediaStreamConstraints::Audio
        };

        let stream = std::panic::catch_unwind(|| media_devices::get_user_media_sync(constraints))
            .map_err(|_| "Could not start audio source".to_string())?;
        let node = ctx.create_media_stream_source(&stream);
        Ok(self.add(ANode::StreamSource(node)))
    }

    /// audioContext .createBuffer (channels, length, sampleRate) filled with `data` (planar).
    pub fn create_buffer(
        &self,
        channels: usize,
        length: usize,
        sample_rate: f32,
        data: Option<&[f32]>,
    ) -> AudioBuffer {
        let mut buffer = AudioBuffer::new(AudioBufferOptions {
            number_of_channels: channels,
            length,
            sample_rate,
        });

        if let Some(data) = data {
            for c in 0..channels {
                buffer.copy_to_channel(&data[c * length..(c + 1) * length], c);
            }
        }

        buffer
    }

    /// audioContext .decodeAudioData (arrayBuffer): resampled to the context's sample rate.
    /// PCM WAV as Chromium does it (decode.rs); other formats with `web-audio-api`.
    pub fn decode(&self, bytes: Vec<u8>) -> Result<AudioBuffer, String> {
        let buffer = self.decode_pcm(&bytes)?;

        if self.recording {
            let (key, sum) = buffer_key(&buffer);

            self.encoded
                .lock()
                .unwrap()
                .insert(key, (sum, Arc::new(bytes)));
        }

        Ok(buffer)
    }

    /// The argument that gives a buffer to the page: the file it was decoded from, else its samples.
    fn buffer_arg(&self, b: &AudioBuffer) -> Arg {
        let (key, sum) = buffer_key(b);

        if let Some((s, bytes)) = self.encoded.lock().unwrap().get(&key) {
            if *s == sum {
                return Arg::Encoded(Arc::clone(bytes));
            }
        }

        Arg::Buffer(
            Arc::new(
                (0..b.number_of_channels())
                    .map(|c| b.get_channel_data(c).to_vec())
                    .collect(),
            ),
            b.sample_rate(),
        )
    }

    fn decode_pcm(&self, bytes: &[u8]) -> Result<AudioBuffer, String> {
        let bytes = bytes.to_vec();
        if let Some((channels, rate)) = decode::decode_wav(&bytes) {
            let context_rate = self.sample_rate();
            let channels: Vec<Vec<f32>> = if rate == context_rate {
                channels
            } else {
                channels
                    .iter()
                    .map(|c| decode::resample(c, rate as f64, context_rate as f64))
                    .collect()
            };
            let length = channels[0].len();

            // A file without frames: decodeAudioData rejects (EncodingError).
            if length == 0 {
                return Err("no audio frames".into());
            }

            let data: Vec<f32> = channels.concat();
            return Ok(self.create_buffer(channels.len(), length, context_rate, Some(&data)));
        }

        self.base()
            .decode_audio_data_sync(std::io::Cursor::new(bytes))
            .map_err(|e| e.to_string())
    }

    // --- offline rendering in step with the frames -------------------------------------------

    /// Before the first frame: suspend points at the render quanta `quanta` (one per frame),
    /// then start rendering on a worker thread. The worker stops at each quantum until
    /// [`AudioEngine::resume`].
    pub fn arm(&mut self, quanta: &[usize]) {
        let Ctx::Offline(ctx) = &self.ctx else { return };
        let rate = ctx.sample_rate() as f64;
        let waker = noop_waker();
        let mut cx = TaskContext::from_waker(&waker);
        let mut suspends: Vec<Pin<Box<dyn std::future::Future<Output = ()> + Send>>> = vec![];

        for &q in quanta {
            let c = Arc::clone(ctx);
            // The engine rounds a suspend time up to a quantum: half a frame before its start.
            let t = (q * RENDER_QUANTUM) as f64 / rate - 0.5 / rate;
            let mut f: Pin<Box<dyn std::future::Future<Output = ()> + Send>> =
                Box::pin(async move { c.suspend(t).await });
            // The first poll registers the suspension (before rendering starts).
            let _ = f.as_mut().poll(&mut cx);
            suspends.push(f);
        }

        let c = Arc::clone(ctx);
        let render = std::thread::spawn(move || {
            let mut f = Box::pin(c.start_rendering());
            block_on(f.as_mut())
        });

        self.stepping = Some(Stepping {
            suspends,
            next: 0,
            render: Some(render),
            waiting: false,
        });
    }

    /// Wait until rendering is suspended before the next frame. Returns the suspended frame.
    pub fn wait(&mut self) -> Option<usize> {
        let s = self.stepping.as_mut()?;
        let f = s.suspends.get_mut(s.next)?;
        block_on(f.as_mut());
        s.next += 1;
        s.waiting = true;
        Some((self.current_time() * self.sample_rate() as f64).round() as usize)
    }

    /// Resume rendering after the frame's events.
    pub fn resume(&mut self) {
        let Ctx::Offline(ctx) = &self.ctx else { return };
        let Some(s) = self.stepping.as_mut() else {
            return;
        };

        if s.waiting {
            s.waiting = false;
            let mut f = Box::pin(ctx.resume());
            block_on(f.as_mut());
        }
    }

    /// After the last frame: the rendered buffer.
    pub fn finish(&mut self) -> Option<AudioBuffer> {
        self.resume();
        let render = self.stepping.as_mut()?.render.take()?;
        let buffer = render.join().ok()?;
        self.rendered = Some(buffer.clone());
        Some(buffer)
    }
}

impl Drop for AudioEngine {
    fn drop(&mut self) {
        // A suspended render thread would wait forever: run it to the end.
        if self.stepping.as_ref().is_some_and(|s| s.render.is_some()) {
            while self
                .stepping
                .as_ref()
                .is_some_and(|s| s.next < s.suspends.len())
            {
                self.resume();
                let _ = self.wait();
            }
            let _ = self.finish();
        }

        if let Ctx::Realtime(c) = &self.ctx {
            c.close_sync();
        }
    }
}
