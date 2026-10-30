//! The Sound component (Components/Sound/*.js) and the browser's sound context
//! (Browser/Sound/X3DSoundContext.js, AudioElement.js), on the Web Audio graph of `sound.rs`.
//!
//! Every class keeps X_ITE's Web Audio nodes and their connections: X3DSoundSourceNode,
//! X3DSoundProcessingNode and X3DSoundChannelNode end in an `audioSource` GainNode that the
//! parent connects to its `audioDestination`; X3DSoundDestinationNode and SpatialSound end in a
//! `soundDestination` GainNode (the browser's SoundIntensity and Mute). AudioDestination's
//! MediaStreamAudioDestinationNode, whose stream an `<audio>` element plays in X_ITE, is a stereo
//! GainNode that feeds the context's output here (see `AudioEngine::stream_destination`).
//!
//! The VRML97 Sound and AudioClip nodes (Sound.js, AudioClip.js) run on the same graph: AudioClip is an
//! X3DSoundSourceNode with BufferAudioSource's url loading, Sound keeps X_ITE's stereo / four-gain pan network
//! and its min/max ellipsoid attenuation (`Runtime::sound_traverse`).

use super::State;
use crate::sound::biquad::Type as BiquadType;
use crate::sound::oscillator::Shape;
use crate::sound::panner::DistanceModel;
use crate::sound::wave_shaper::Oversample;
use crate::sound::{AId, ANode, AudioEngine};
use crate::*;
use web_audio_api::AudioBuffer;

/// Interest callbacks of the Sound nodes (`Cb::Sound`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum SoundCb {
    Gain,
    ChannelCount,
    ChannelCountMode,
    ChannelInterpretation,
    Children,
    Enabled,
    /// timeEvents: set_time.
    Time,
    Detune,
    Frequency,
    PeriodicWave,
    /// OscillatorSource: the periodic wave node's `type` changed.
    WaveType,
    WaveOptions,
    Buffer,
    PlaybackRate,
    LoopStart,
    LoopEnd,
    Type,
    QualityFactor,
    FftSize,
    Decibels,
    Smoothing,
    Normalize,
    Curve,
    Oversample,
    MaxDelayTime,
    DelayTime,
    Attack,
    Knee,
    Ratio,
    Release,
    Threshold,
    Outputs,
    ChannelSelection,
    MediaDeviceId,
    Spatialize,
    Intensity,
    ConeOuterGain,
    ConeInnerAngle,
    ConeOuterAngle,
    DistanceModel,
    MaxDistance,
    ReferenceDistance,
    EnableHrtf,
    TrackCurrentView,
    /// SpatialSound and Sound: the browser's sensorEvents (`update`).
    Update,
    /// AudioClip.set_pitch__.
    Pitch,
    /// MovieTexture.set_speed__ (speed and pitch).
    Speed,
}

/// AudioElement.create (Browser/Sound/AudioElement.js): a media element over an AudioBuffer.
pub struct AudioElement {
    source: AId,
    audio_source: AId,
    buffer: Option<AudioBuffer>,
    detune: f64,
    playback_rate: f64,
    loop_start: f64,
    loop_end: f64,
    loop_: bool,
    start_time: f64,
    current_time: f64,
    active: bool,
    pub duration: f64,
}

/// The private state of a Sound node (the members X_ITE keeps on the node object).
#[derive(Default)]
pub struct SoundState {
    pub audio_source: Option<AId>,
    pub audio_destination: Option<AId>,
    pub sound_destination: Option<AId>,
    pub sound_processor: Option<AId>,
    /// The class's own Web Audio node: oscillatorNode, biquadFilterNode, delayNode, analyserNode,
    /// convolverNode, dynamicsCompressorNode, waveShaperNode, channelSplitterNode,
    /// channelMergerNode, pannerNode, mediaStreamAudioDestinationNode, mediaStreamAudioSourceNode.
    pub node: Option<AId>,
    /// SpatialSound.gainNode.
    pub gain_node: Option<AId>,
    pub child_nodes: Vec<NodeId>,
    /// ChannelSplitter.outputNodes.
    pub output_nodes: Vec<NodeId>,
    /// OscillatorSource.periodicWaveNode (`None`: the browser's default PeriodicWave).
    pub periodic_wave: Option<NodeId>,
    pub media: Option<AudioElement>,
    /// MovieTexture: its media element, a decoded GIF or APNG.
    pub movie: Option<super::movie_texture::MovieMedia>,
    /// MicrophoneSource.restore.
    pub restore: bool,
    /// SpatialSound.currentTraversed (Sound too).
    pub current_traversed: bool,
    /// Sound: the Web Audio nodes beside `gain_node` and `sound_destination`.
    pub vrml: Option<VrmlSound>,
    /// PeriodicWave.real / imag (Float32Array, kept between createPeriodicWave calls).
    pub real: Vec<f32>,
    pub imag: Vec<f32>,
    /// Analyser byteFrequencyData length.
    pub byte_bins: usize,
}

/// Sound.initialize: gainStereoNode, splitterNode, the four pan gains and mergerNode.
pub struct VrmlSound {
    pub stereo: AId,
    pub splitter: AId,
    pub front_left: AId,
    pub front_right: AId,
    pub back_left: AId,
    pub back_right: AId,
    pub merger: AId,
    /// The closure variables of Sound.getEllipsoidParameter (`sphereMatrix`, `enterPoint`), which keep their
    /// values between calls. X_ITE shares them between all Sound nodes; here each node has its own.
    sphere_matrix: x3d_fields::math::Matrix4,
    enter_point: x3d_fields::math::Vector3,
}

impl SoundState {
    fn ids(&self) -> Vec<AId> {
        let mut v: Vec<AId> = [
            self.audio_source,
            self.audio_destination,
            self.sound_destination,
            self.node,
            self.gain_node,
        ]
        .into_iter()
        .flatten()
        .collect();

        if let Some(s) = &self.vrml {
            v.extend([
                s.stereo,
                s.splitter,
                s.front_left,
                s.front_right,
                s.back_left,
                s.back_right,
                s.merger,
            ]);
        }

        if let Some(m) = &self.media {
            v.push(m.source);
        }

        v
    }
}

/// The DOMException of a rejected decodeAudioData in Chrome.
const DECODE_ERROR: &str = "EncodingError: Unable to decode audio data";

const SOURCES: &[&str] = &[
    "AudioClip",
    "BufferAudioSource",
    "ListenerPointSource",
    "MicrophoneSource",
    "OscillatorSource",
    "StreamAudioSource",
];
const PROCESSING: &[&str] = &[
    "Analyser",
    "BiquadFilter",
    "Convolver",
    "Delay",
    "DynamicsCompressor",
    "Gain",
    "WaveShaper",
];
const CHANNELS: &[&str] = &["ChannelMerger", "ChannelSelector", "ChannelSplitter"];
const DESTINATIONS: &[&str] = &["AudioDestination", "StreamAudioDestination"];

/// The Sound component nodes this module runs.
pub fn is_sound_node(name: &str) -> bool {
    SOURCES.contains(&name)
        || PROCESSING.contains(&name)
        || CHANNELS.contains(&name)
        || DESTINATIONS.contains(&name)
        || matches!(name, "Sound" | "SpatialSound" | "PeriodicWave")
}

/// X3DSoundSourceNode or X3DSoundProcessingNode: an X3DTimeDependentNode.
pub fn is_sound_time_node(name: &str) -> bool {
    (SOURCES.contains(&name) && name != "ListenerPointSource") || PROCESSING.contains(&name)
}

/// The child types the X3DSound*Node `set_children__` methods keep: X3DSoundChannelNode,
/// X3DSoundProcessingNode or X3DSoundSourceNode (ListenerPointSource is an X3DSoundNode only).
fn is_audio_child(name: &str) -> bool {
    (SOURCES.contains(&name) && name != "ListenerPointSource")
        || PROCESSING.contains(&name)
        || CHANNELS.contains(&name)
}

fn clamp(v: f64, min: f64, max: f64) -> f64 {
    super::time::clamp(v, min, max)
}

/// Algorithm.nextPowerOfTwo.
fn next_power_of_two(n: f64) -> f64 {
    let mut n = n as i64 as i32;
    n -= 1;
    n |= n >> 1;
    n |= n >> 2;
    n |= n >> 4;
    n |= n >> 8;
    n |= n >> 16;
    n.wrapping_add(1) as f64
}

fn count_mode(v: &str) -> Option<&'static str> {
    match v {
        "MAX" => Some("max"),
        "CLAMPED-MAX" | "CLAMPED_MAX" => Some("clamped-max"),
        "EXPLICIT" => Some("explicit"),
        _ => None,
    }
}

fn interpretation(v: &str) -> Option<&'static str> {
    match v {
        "SPEAKERS" => Some("speakers"),
        "DISCRETE" => Some("discrete"),
        _ => None,
    }
}

impl Runtime {
    /// X3DSoundContext.getAudioContext: one context per browser, made by the first Sound node.
    pub fn audio(&mut self) -> &mut AudioEngine {
        if self.audio.is_none() {
            self.audio = Some(AudioEngine::new(self.audio_mode.clone()));
        }

        self.audio.as_mut().unwrap()
    }

    pub(super) fn snd(&mut self, node: NodeId) -> &mut SoundState {
        self.sound.entry(node).or_default()
    }

    fn snd_ref(&self, node: NodeId) -> Option<&SoundState> {
        self.sound.get(&node)
    }

    fn get_str(&self, node: NodeId, name: &str) -> String {
        match self.pre_value(node, name) {
            Some(FieldValue::SFString(s)) => s.clone(),
            _ => String::new(),
        }
    }

    fn get_mffloat(&self, node: NodeId, name: &str) -> Vec<f32> {
        match self.pre_value(node, name) {
            Some(FieldValue::MFFloat(v)) => v.clone(),
            _ => vec![],
        }
    }

    fn get_sfnode(&self, node: NodeId, name: &str) -> Option<NodeId> {
        match self.pre_value(node, name) {
            Some(FieldValue::SFNode(n)) => *n,
            _ => None,
        }
    }

    pub(super) fn sound_interest(&mut self, node: NodeId, field: &str, cb: SoundCb) {
        self.interest(node, field, Cb::Sound(cb));
    }

    /// `this ._name = value` of an outputOnly / private field (always an event).
    fn sound_emit(&mut self, node: NodeId, name: &str, value: FieldValue) {
        if let Some(k) = self
            .pre(node, name)
            .or_else(|| self.private_field(node, name))
        {
            self.set_value(k, value);
        }
    }

    /// X3DBaseNode.getLive () of a Sound node that is not time dependent.
    /// Does the node still have its audio source (not disposed)?
    pub(crate) fn sound_has_source(&self, node: NodeId) -> bool {
        self.snd_ref(node).is_some_and(|s| s.audio_source.is_some())
    }

    fn sound_live(&self, node: NodeId) -> bool {
        self.ctx_live(self.scene.node(node).ctx)
    }

    // --- construction -------------------------------------------------------------------------

    /// The constructors: private fields and the Web Audio nodes made at construction.
    pub(crate) fn sound_construct(&mut self, node: NodeId, name: &str) {
        if is_sound_time_node(name) {
            // X3DTimeDependentNode constructor.
            self.add_private_field(
                node,
                "initialized",
                access::OUTPUT_ONLY,
                FieldValue::SFTime(0.0),
            );
            self.add_private_field(
                node,
                "evenLive",
                access::INPUT_OUTPUT,
                FieldValue::SFBool(false),
            );
            self.nrt_mut(node).state = State::Time(super::time::TimeState::default());

            // `addChildObjects (inputOutput, "loop", new SFBool ())`: the classes without a loop field.
            if self.pre(node, "loop").is_none() {
                self.add_private_field(
                    node,
                    "loop",
                    access::INPUT_OUTPUT,
                    FieldValue::SFBool(false),
                );
            }
        }

        if SOURCES.contains(&name) && name != "ListenerPointSource" {
            // X3DSoundSourceNode: audioSource = new GainNode (audioContext, { gain: 0 }).
            let a = self.audio().gain(0.0);
            self.snd(node).audio_source = Some(a);
        }

        if name == "BufferAudioSource" || name == "AudioClip" {
            // X3DUrlObject constructor.
            self.add_private_field(
                node,
                "loadState",
                access::OUTPUT_ONLY,
                FieldValue::SFInt32(0),
            );
            self.add_private_field(
                node,
                "loadData",
                access::OUTPUT_ONLY,
                FieldValue::SFTime(0.0),
            );
            // The node's State is its X3DUrlObject state; the X3DTimeDependentNode state is kept
            // in `time_states`.
            let now_ms = self.now * 1000.0;
            self.time_states.insert(node, Default::default());
            self.nrt_mut(node).state = State::UrlObject(super::UrlState {
                start_ms: now_ms,
                complete_ms: now_ms,
                ..Default::default()
            });
        }

        if PROCESSING.contains(&name) || CHANNELS.contains(&name) {
            let e = self.audio();
            let destination = e.gain(0.0);
            let source = e.gain(1.0);
            let s = self.snd(node);
            s.audio_destination = Some(destination);
            s.audio_source = Some(source);
            s.sound_processor = Some(source);
        }

        if DESTINATIONS.contains(&name) {
            let e = self.audio();
            let destination = e.gain(0.0);
            let sound_destination = e.gain(0.0);
            let s = self.snd(node);
            s.audio_destination = Some(destination);
            s.sound_destination = Some(sound_destination);
        }

        // The class constructors.
        match name {
            "Analyser" | "BiquadFilter" | "Convolver" | "DynamicsCompressor" | "WaveShaper" => {
                let e = self.audio();
                let n = match name {
                    "Analyser" => e.analyser(),
                    "BiquadFilter" => e.biquad(),
                    "Convolver" => e.convolver(),
                    "DynamicsCompressor" => e.compressor(),
                    _ => e.wave_shaper(),
                };
                let source = self.snd(node).audio_source.unwrap();
                self.audio().connect(n, source);
                self.snd(node).node = Some(n);

                if name == "Analyser" {
                    // byteFrequencyData = new Uint8Array (analyserNode .frequencyBinCount)
                    self.snd(node).byte_bins = 1024;
                }
            }
            "AudioDestination" | "StreamAudioDestination" => {
                // new MediaStreamAudioDestinationNode (audioContext); soundDestination .connect (it).
                let to_output = name == "AudioDestination";
                let stream = self.audio().stream_destination(to_output);
                let sound_destination = self.snd(node).sound_destination.unwrap();
                self.audio().connect(sound_destination, stream);
                self.snd(node).node = Some(stream);
            }
            "SpatialSound" | "Sound" => {
                self.add_private_field(
                    node,
                    "traversed",
                    access::OUTPUT_ONLY,
                    FieldValue::SFBool(false),
                );
                self.snd(node).current_traversed = true;
            }
            "PeriodicWave" => {
                let s = self.snd(node);
                s.real = vec![0.0; 2];
                s.imag = vec![0.0; 2];
            }
            _ => {}
        }
    }

    // --- initialize -----------------------------------------------------------------------------

    pub(crate) fn sound_initialize(&mut self, node: NodeId, name: &str) {
        if is_sound_time_node(name) {
            // X3DTimeDependentNode.initialize.
            let enabled_cb = if PROCESSING.contains(&name) {
                Cb::Sound(SoundCb::Enabled)
            } else {
                Cb::TimeSetEnabled
            };
            self.time_dependent_initialize(node, enabled_cb);
        }

        if SOURCES.contains(&name) && name != "ListenerPointSource" {
            // X3DSoundSourceNode.initialize.
            self.sound_interest(node, "gain", SoundCb::Gain);
            self.source_set_gain(node);
        }

        if PROCESSING.contains(&name) {
            self.processing_initialize(node);
        }

        if CHANNELS.contains(&name) {
            self.channel_initialize(node);
        }

        if DESTINATIONS.contains(&name) {
            self.destination_initialize(node);
        }

        match name {
            "OscillatorSource" => {
                self.sound_interest(node, "detune", SoundCb::Detune);
                self.sound_interest(node, "frequency", SoundCb::Frequency);
                self.sound_interest(node, "periodicWave", SoundCb::PeriodicWave);
                self.osc_set_periodic_wave(node);
            }
            "BufferAudioSource" => {
                // X3DUrlObject.initialize (its set_live__ is BufferAudioSource.set_live__, already
                // registered by X3DTimeDependentNode.initialize).
                self.interest(node, "load", Cb::UrlSetLoad);
                self.interest(node, "url", Cb::UrlSetUrl);
                if let Some(k) = self.private_field(node, "loadData") {
                    self.add_interest(k, node, Cb::UrlLoadData);
                }
                self.interest(node, "autoRefresh", Cb::UrlSetAutoRefresh);
                self.interest(node, "autoRefreshTimeLimit", Cb::UrlSetAutoRefresh);

                for f in ["numberOfChannels", "sampleRate", "bufferLength", "buffer"] {
                    self.sound_interest(node, f, SoundCb::Buffer);
                }
                self.sound_interest(node, "detune", SoundCb::Detune);
                self.sound_interest(node, "playbackRate", SoundCb::PlaybackRate);
                self.sound_interest(node, "loopStart", SoundCb::LoopStart);
                self.sound_interest(node, "loopEnd", SoundCb::LoopEnd);
                self.buffer_set_buffer(node);
                self.request_immediate_load(node);
            }
            "AudioClip" => {
                // X3DUrlObject.initialize (set_live__ is registered by X3DTimeDependentNode.initialize, as for
                // BufferAudioSource).
                self.interest(node, "load", Cb::UrlSetLoad);
                self.interest(node, "url", Cb::UrlSetUrl);
                if let Some(k) = self.private_field(node, "loadData") {
                    self.add_interest(k, node, Cb::UrlLoadData);
                }
                self.interest(node, "autoRefresh", Cb::UrlSetAutoRefresh);
                self.interest(node, "autoRefreshTimeLimit", Cb::UrlSetAutoRefresh);

                self.sound_interest(node, "pitch", SoundCb::Pitch);
                self.request_immediate_load(node);
            }
            "MicrophoneSource" => {
                self.sound_interest(node, "mediaDeviceID", SoundCb::MediaDeviceId)
            }
            "Analyser" => {
                self.sound_interest(node, "fftSize", SoundCb::FftSize);
                self.sound_interest(node, "minDecibels", SoundCb::Decibels);
                self.sound_interest(node, "maxDecibels", SoundCb::Decibels);
                self.sound_interest(node, "smoothingTimeConstant", SoundCb::Smoothing);
                let n = self.snd(node).node;
                self.set_sound_processor(node, n);
                self.analyser_set_fft_size(node);
                self.analyser_set_decibels(node);
                self.analyser_set_smoothing(node);
            }
            "BiquadFilter" => {
                self.sound_interest(node, "detune", SoundCb::Detune);
                self.sound_interest(node, "type", SoundCb::Type);
                self.sound_interest(node, "frequency", SoundCb::Frequency);
                self.sound_interest(node, "qualityFactor", SoundCb::QualityFactor);
                let n = self.snd(node).node;
                self.set_sound_processor(node, n);
                self.sound_call(node, SoundCb::Detune);
                self.sound_call(node, SoundCb::Type);
                self.sound_call(node, SoundCb::Frequency);
                self.sound_call(node, SoundCb::QualityFactor);
            }
            "Convolver" => {
                self.sound_interest(node, "numberOfChannels", SoundCb::Buffer);
                self.sound_interest(node, "buffer", SoundCb::Buffer);
                self.sound_interest(node, "normalize", SoundCb::Normalize);
                let n = self.snd(node).node;
                self.set_sound_processor(node, n);
                self.convolver_set_buffer(node);
                self.convolver_set_normalize(node);
            }
            "Delay" => {
                self.sound_interest(node, "maxDelayTime", SoundCb::MaxDelayTime);
                self.sound_interest(node, "delayTime", SoundCb::DelayTime);
                self.delay_set_max_delay_time(node);
            }
            "DynamicsCompressor" => {
                for (f, cb) in [
                    ("attack", SoundCb::Attack),
                    ("knee", SoundCb::Knee),
                    ("ratio", SoundCb::Ratio),
                    ("release", SoundCb::Release),
                    ("threshold", SoundCb::Threshold),
                ] {
                    self.sound_interest(node, f, cb);
                }
                let n = self.snd(node).node;
                self.set_sound_processor(node, n);
                for cb in [
                    SoundCb::Attack,
                    SoundCb::Knee,
                    SoundCb::Ratio,
                    SoundCb::Release,
                    SoundCb::Threshold,
                ] {
                    self.sound_call(node, cb);
                }
            }
            "WaveShaper" => {
                self.sound_interest(node, "curve", SoundCb::Curve);
                self.sound_interest(node, "oversample", SoundCb::Oversample);
                let n = self.snd(node).node;
                self.set_sound_processor(node, n);
                self.wave_shaper_set_curve(node);
                self.wave_shaper_set_oversample(node);
            }
            "ChannelMerger" => self.sound_interest(node, "enabled", SoundCb::Children),
            "ChannelSelector" => {
                self.sound_interest(node, "channelSelection", SoundCb::ChannelSelection);
                self.selector_set_channel_selection(node);
            }
            "ChannelSplitter" => {
                self.sound_interest(node, "enabled", SoundCb::Outputs);
                self.sound_interest(node, "outputs", SoundCb::Outputs);
                self.splitter_set_outputs(node);
            }
            "AudioDestination" | "StreamAudioDestination" => {
                // `this ._maxChannelCount = audioContext .destination .maxChannelCount` (AudioDestination).
                if name == "AudioDestination" {
                    let max = {
                        let e = self.audio();
                        match e.get(e.destination) {
                            ANode::Destination(d) => d.max_channel_count() as i32,
                            _ => 2,
                        }
                    };
                    self.sound_emit(node, "maxChannelCount", FieldValue::SFInt32(max));
                }

                self.sound_interest(node, "mediaDeviceID", SoundCb::MediaDeviceId);
                self.destination_set_media_device_id(node);
            }
            "SpatialSound" => self.spatial_initialize(node),
            "Sound" => self.vrml_sound_initialize(node),
            "ListenerPointSource" => {
                self.sound_interest(node, "enabled", SoundCb::TrackCurrentView);
                self.sound_interest(node, "trackCurrentView", SoundCb::TrackCurrentView);
                self.listener_set_track_current_view(node);
            }
            _ => {}
        }
    }

    /// Run an interest callback directly (`this .set_xxx__ ()`).
    fn sound_call(&mut self, node: NodeId, cb: SoundCb) {
        self.sound_interest_call(node, cb);
    }

    /// Dispatch of `Cb::Sound`.
    pub(crate) fn sound_interest_call(&mut self, node: NodeId, cb: SoundCb) {
        let Some(name) = self.builtin_name(node) else {
            return;
        };

        match cb {
            SoundCb::Gain => {
                if PROCESSING.contains(&name) {
                    self.processing_set_gain(node)
                } else if CHANNELS.contains(&name) {
                    self.channel_set_gain(node)
                } else if DESTINATIONS.contains(&name) {
                    self.destination_set_gain(node)
                } else {
                    self.source_set_gain(node)
                }
            }
            SoundCb::ChannelCount => self.bus_set_channel_count(node),
            SoundCb::ChannelCountMode => {
                if DESTINATIONS.contains(&name) || self.get_bool(node, "enabled") {
                    self.bus_set_channel_count_mode(node)
                }
            }
            SoundCb::ChannelInterpretation => {
                if DESTINATIONS.contains(&name) || self.get_bool(node, "enabled") {
                    self.bus_set_channel_interpretation(node)
                }
            }
            SoundCb::Children => {
                if CHANNELS.contains(&name) {
                    self.channel_set_children(node)
                } else if name == "SpatialSound" {
                    self.spatial_set_children(node)
                } else if name == "Sound" {
                    self.vrml_sound_set_children(node)
                } else {
                    self.bus_set_children(node)
                }
            }
            SoundCb::Enabled => {
                if PROCESSING.contains(&name) {
                    self.processing_set_enabled(node)
                } else if CHANNELS.contains(&name) {
                    self.channel_set_enabled(node)
                } else if DESTINATIONS.contains(&name) {
                    self.destination_set_enabled(node)
                } else if name == "SpatialSound" || name == "Sound" {
                    self.spatial_set_enabled(node)
                }
            }
            SoundCb::Time => self.sound_set_time(node),
            SoundCb::Speed => self.movie_set_speed(node),
            SoundCb::Detune => match name {
                "OscillatorSource" => {
                    if let Some(o) = self.snd(node).node {
                        let v = self.get_f64(node, "detune");
                        self.audio().set_param(o, "detune", v);
                    }
                }
                "BufferAudioSource" => {
                    let v = self.get_f64(node, "detune");
                    self.media_set(node, |e, m| m.set_detune(e, v));
                }
                _ => {
                    let v = self.get_f64(node, "detune");
                    let n = self.snd(node).node.unwrap();
                    self.audio().set_param(n, "detune", v);
                }
            },
            SoundCb::Frequency => {
                let v = self.get_f64(node, "frequency").max(0.0);

                if let Some(n) = self.snd(node).node {
                    self.audio().set_param(n, "frequency", v);
                }
            }
            SoundCb::PeriodicWave => self.osc_set_periodic_wave(node),
            SoundCb::WaveType => self.osc_set_type(node),
            SoundCb::WaveOptions => self.osc_set_periodic_wave_options(node),
            SoundCb::Buffer => {
                if name == "Convolver" {
                    self.convolver_set_buffer(node)
                } else {
                    self.buffer_set_buffer(node)
                }
            }
            // AudioClip.set_pitch__: `media .playbackRate = Math .max (pitch, 0)`.
            SoundCb::Pitch => {
                let v = self.get_f64(node, "pitch").max(0.0);
                self.media_set(node, |e, m| m.set_playback_rate(e, v));
            }
            SoundCb::PlaybackRate => {
                let v = self.get_f64(node, "playbackRate");
                self.media_set(node, |e, m| m.set_playback_rate(e, v));
            }
            SoundCb::LoopStart => {
                let v = self.get_f64(node, "loopStart").max(0.0);
                self.media_set(node, |e, m| m.set_loop_start(e, v));
            }
            SoundCb::LoopEnd => {
                let v = self.get_f64(node, "loopEnd").max(0.0);
                self.media_set(node, |e, m| m.set_loop_end(e, v));
            }
            SoundCb::Type => {
                let t = match self.get_str(node, "type").as_str() {
                    "HIGHPASS" => BiquadType::Highpass,
                    "BANDPASS" => BiquadType::Bandpass,
                    "LOWSHELF" => BiquadType::Lowshelf,
                    "HIGHSHELF" => BiquadType::Highshelf,
                    "PEAKING" => BiquadType::Peaking,
                    "NOTCH" => BiquadType::Notch,
                    "ALLPASS" => BiquadType::Allpass,
                    _ => BiquadType::Lowpass,
                };
                let n = self.snd(node).node.unwrap();
                self.audio().biquad_set_type(n, t);
            }
            SoundCb::QualityFactor => {
                let v = clamp(self.get_f64(node, "qualityFactor"), 0.0001, 1000.0);
                let n = self.snd(node).node.unwrap();
                self.audio().set_param(n, "Q", v);
            }
            SoundCb::FftSize => self.analyser_set_fft_size(node),
            SoundCb::Decibels => self.analyser_set_decibels(node),
            SoundCb::Smoothing => self.analyser_set_smoothing(node),
            SoundCb::Normalize => self.convolver_set_normalize(node),
            SoundCb::Curve => self.wave_shaper_set_curve(node),
            SoundCb::Oversample => self.wave_shaper_set_oversample(node),
            SoundCb::MaxDelayTime => self.delay_set_max_delay_time(node),
            SoundCb::DelayTime => {
                let max = self.get_f64(node, "maxDelayTime").max(0.0);
                let v = clamp(self.get_f64(node, "delayTime"), 0.0, max);
                if let Some(n) = self.snd(node).node {
                    self.audio().set_param(n, "delayTime", v);
                }
            }
            SoundCb::Attack
            | SoundCb::Knee
            | SoundCb::Ratio
            | SoundCb::Release
            | SoundCb::Threshold => {
                let (field, param, min, max) = match cb {
                    SoundCb::Attack => ("attack", "attack", 0.0, 1.0),
                    SoundCb::Knee => ("knee", "knee", 0.0, 40.0),
                    SoundCb::Ratio => ("ratio", "ratio", 1.0, 20.0),
                    SoundCb::Release => ("release", "release", 0.0, 1.0),
                    _ => ("threshold", "threshold", -100.0, 0.0),
                };
                let v = clamp(self.get_f64(node, field), min, max);
                let n = self.snd(node).node.unwrap();
                self.audio().set_param(n, param, v);
            }
            SoundCb::Outputs => self.splitter_set_outputs(node),
            SoundCb::ChannelSelection => self.selector_set_channel_selection(node),
            SoundCb::MediaDeviceId => {
                if name == "MicrophoneSource" {
                    self.mic_set_media_device_id(node)
                } else {
                    self.destination_set_media_device_id(node)
                }
            }
            SoundCb::Spatialize if name == "Sound" => self.vrml_sound_set_spatialize(node),
            SoundCb::Intensity if name == "Sound" => self.vrml_sound_set_intensity(node),
            SoundCb::Spatialize => self.spatial_set_spatialize(node),
            SoundCb::Intensity => self.spatial_set_intensity(node),
            SoundCb::ConeOuterGain
            | SoundCb::ConeInnerAngle
            | SoundCb::ConeOuterAngle
            | SoundCb::DistanceModel
            | SoundCb::MaxDistance
            | SoundCb::ReferenceDistance
            | SoundCb::EnableHrtf => self.spatial_set_panner(node, cb),
            SoundCb::TrackCurrentView => self.listener_set_track_current_view(node),
            SoundCb::Update => self.spatial_set_traversed(node, false),
        }
    }

    // --- X3DSoundSourceNode -------------------------------------------------------------------

    /// X3DSoundSourceNode.set_gain__.
    fn source_set_gain(&mut self, node: NodeId) {
        let v = self.get_f64(node, "gain");
        let a = self.snd(node).audio_source.unwrap();
        self.audio().set_param(a, "gain", v);
        // `mediaElement .muted = gain === 0`: no effect on an AudioElement.
    }

    /// X3DSoundSourceNode.setMediaElement.
    fn set_media_element(&mut self, node: NodeId, value: Option<AudioElement>) {
        // getBrowser () .stopAudioElement (this .mediaElement)
        self.media_set(node, |e, m| m.pause(e));

        if let Some(old) = self.snd(node).media.take() {
            self.audio().release(old.source);
        }

        let Some(value) = value else { return };

        self.snd(node).media = Some(value);

        // Init mediaElement.
        self.source_set_gain(node);
        self.source_set_loop(node);

        // Handle events.
        self.time_reset_elapsed(node);

        if self.get_bool(node, "isActive") {
            if self.get_bool(node, "isPaused") {
                self.sound_set_pause(node);
            } else if self.time_node_live(node) {
                self.sound_set_start(node);
            } else {
                self.sound_set_pause(node);
            }
        } else {
            self.sound_set_stop(node);
        }
    }

    fn media_set(&mut self, node: NodeId, f: impl FnOnce(&mut AudioEngine, &mut AudioElement)) {
        let Some(mut m) = self.sound.get_mut(&node).and_then(|s| s.media.take()) else {
            return;
        };
        f(self.audio(), &mut m);
        self.snd(node).media = Some(m);
    }

    fn media_get<T>(
        &mut self,
        node: NodeId,
        f: impl FnOnce(&AudioEngine, &AudioElement) -> T,
    ) -> Option<T> {
        let m = self.sound.get_mut(&node)?.media.take()?;
        let r = f(self.audio(), &m);
        self.snd(node).media = Some(m);
        Some(r)
    }

    /// X3DSoundSourceNode.set_loop.
    fn source_set_loop(&mut self, node: NodeId) {
        let v = self.time_loop(node);
        self.media_set(node, |e, m| m.set_loop(e, v));
    }

    /// set_loop () of an X3DTimeDependentNode (after set_loop__).
    pub(crate) fn sound_set_loop(&mut self, node: NodeId) {
        match self.builtin_name(node) {
            Some("MovieTexture") => self.movie_set_loop(node),
            Some(n) if SOURCES.contains(&n) => self.source_set_loop(node),
            _ => {}
        }
    }

    /// set_start () (X3DTimeDependentNode.do_start).
    pub(crate) fn sound_set_start(&mut self, node: NodeId) {
        match self.builtin_name(node) {
            Some("OscillatorSource") => self.osc_set_start(node),
            Some("MicrophoneSource") => self.mic_set_start(node),
            Some(n) if PROCESSING.contains(&n) => {
                self.sound_emit(node, "elapsedTime", FieldValue::SFTime(0.0))
            }
            Some("MovieTexture") => self.movie_set_start(node),
            _ => {
                // X3DSoundSourceNode.set_start.
                if self.snd_ref(node).is_none_or(|s| s.media.is_none()) {
                    return;
                }
                self.sound_emit(node, "elapsedTime", FieldValue::SFTime(0.0));
                self.media_set(node, |e, m| m.set_current_time(e, 0.0));
                self.media_set(node, |e, m| m.play(e));
            }
        }
    }

    /// set_pause () (X3DTimeDependentNode.real_pause).
    pub(crate) fn sound_set_pause(&mut self, node: NodeId) {
        match self.builtin_name(node) {
            Some("OscillatorSource") => self.osc_set_stop(node),
            Some("MicrophoneSource") => self.mic_set_pause(node),
            Some(n) if PROCESSING.contains(&n) => {}
            Some("MovieTexture") => self.movie_pause(node),
            _ => self.media_set(node, |e, m| m.pause(e)),
        }
    }

    /// set_resume () (X3DTimeDependentNode.real_resume).
    pub(crate) fn sound_set_resume(&mut self, node: NodeId) {
        match self.builtin_name(node) {
            Some("OscillatorSource") => self.osc_set_start(node),
            Some("MicrophoneSource") => self.mic_set_resume(node),
            Some(n) if PROCESSING.contains(&n) => {}
            Some("MovieTexture") => self.movie_resume(node),
            _ => self.media_set(node, |e, m| m.play(e)),
        }
    }

    /// set_stop () (X3DTimeDependentNode.stop).
    pub(crate) fn sound_set_stop(&mut self, node: NodeId) {
        match self.builtin_name(node) {
            Some("OscillatorSource") => self.osc_set_stop(node),
            Some("MicrophoneSource") => self.mic_set_stop(node, false),
            Some(n) if PROCESSING.contains(&n) => {}
            Some("MovieTexture") => self.movie_pause(node),
            _ => self.media_set(node, |e, m| m.pause(e)),
        }
    }

    /// set_time () (timeEvents).
    fn sound_set_time(&mut self, node: NodeId) {
        match self.builtin_name(node) {
            Some("OscillatorSource") => {
                let t = self.time_elapsed_time(node);
                self.sound_emit(node, "elapsedTime", FieldValue::SFTime(t));
            }
            Some("MicrophoneSource") => {
                if self.snd(node).node.is_none() {
                    return;
                }
                let t = self.time_elapsed_time(node);
                self.sound_emit(node, "elapsedTime", FieldValue::SFTime(t));
            }
            Some("Analyser") => {
                let t = self.time_elapsed_time(node);
                self.sound_emit(node, "elapsedTime", FieldValue::SFTime(t));
                self.analyser_set_time(node);
            }
            Some("DynamicsCompressor") => {
                let n = self.snd(node).node.unwrap();
                let reduction = self.audio().compressor_reduction(n) as f64;
                self.sound_emit(node, "reduction", FieldValue::SFFloat(reduction));
                let t = self.time_elapsed_time(node);
                self.sound_emit(node, "elapsedTime", FieldValue::SFTime(t));
            }
            Some(n) if PROCESSING.contains(&n) => {
                let t = self.time_elapsed_time(node);
                self.sound_emit(node, "elapsedTime", FieldValue::SFTime(t));
            }
            Some("MovieTexture") => self.movie_set_time(node),
            _ => {
                // X3DSoundSourceNode.set_time.
                if self.snd_ref(node).is_none_or(|s| s.media.is_none()) {
                    return;
                }
                let t = self.time_elapsed_time(node);
                self.sound_emit(node, "elapsedTime", FieldValue::SFTime(t));
                let ended = self
                    .media_get(node, |e, m| !(m.current_time(e) < m.duration))
                    .unwrap_or(false);
                if !ended {
                    return;
                }
                // set_end
                if self.time_loop(node) {
                    return;
                }
                self.time_stop_node(node);
            }
        }
    }

    // --- OscillatorSource -----------------------------------------------------------------------

    fn osc_set_periodic_wave(&mut self, node: NodeId) {
        if let Some(pw) = self.snd(node).periodic_wave {
            self.remove_interest_named(pw, "type", node, Cb::Sound(SoundCb::WaveType));
            self.remove_interest_named(pw, "optionsReal", node, Cb::Sound(SoundCb::WaveOptions));
            self.remove_interest_named(pw, "optionsImag", node, Cb::Sound(SoundCb::WaveOptions));
        }

        let pw = self
            .get_sfnode(node, "periodicWave")
            .and_then(|n| self.inner_node(n))
            .filter(|&n| self.builtin_name(n) == Some("PeriodicWave"));
        self.snd(node).periodic_wave = pw;

        // The default PeriodicWave of the browser never changes; its interest does nothing.
        if let Some(pw) = pw {
            self.interest_named(pw, "type", node, Cb::Sound(SoundCb::WaveType));
        }

        self.osc_set_type(node);
    }

    fn interest_named(&mut self, target: NodeId, field: &str, node: NodeId, cb: Cb) {
        if let Some(k) = self.pre(target, field) {
            self.add_interest(k, node, cb);
        }
    }

    fn remove_interest_named(&mut self, target: NodeId, field: &str, node: NodeId, cb: Cb) {
        if let Some(k) = self.pre(target, field) {
            self.remove_interest(k, node, cb);
        }
    }

    fn osc_set_type(&mut self, node: NodeId) {
        let Some(osc) = self.snd(node).node else {
            return;
        };
        let pw = self.snd(node).periodic_wave;

        if let Some(pw) = pw {
            self.remove_interest_named(pw, "optionsReal", node, Cb::Sound(SoundCb::WaveOptions));
            self.remove_interest_named(pw, "optionsImag", node, Cb::Sound(SoundCb::WaveOptions));
        }

        let type_name = pw.map_or("SQUARE".to_string(), |pw| self.get_str(pw, "type"));
        let t = match type_name.as_str() {
            "SINE" => Some(Shape::Sine),
            "SQUARE" => Some(Shape::Square),
            "SAWTOOTH" => Some(Shape::Sawtooth),
            "TRIANGLE" => Some(Shape::Triangle),
            "CUSTOM" => None,
            _ => Some(Shape::Square),
        };

        match t {
            None => {
                let pw = pw.unwrap();
                self.interest_named(pw, "optionsReal", node, Cb::Sound(SoundCb::WaveOptions));
                self.interest_named(pw, "optionsImag", node, Cb::Sound(SoundCb::WaveOptions));
                self.osc_set_periodic_wave_options(node);
            }
            Some(t) => self.audio().oscillator_set_type(osc, t),
        }
    }

    /// set_periodicWaveOptions__: oscillatorNode .setPeriodicWave (periodicWaveNode .createPeriodicWave ()).
    fn osc_set_periodic_wave_options(&mut self, node: NodeId) {
        let Some(osc) = self.snd(node).node else {
            return;
        };
        let Some(pw) = self.snd(node).periodic_wave else {
            return;
        };
        let (real, imag) = self.periodic_wave_create(pw);
        self.audio().oscillator_set_periodic_wave(osc, &real, &imag);
    }

    /// PeriodicWave.createPeriodicWave: the Float32Arrays are made anew only when the length changes.
    fn periodic_wave_create(&mut self, pw: NodeId) -> (Vec<f32>, Vec<f32>) {
        let (r, i) = (
            self.get_mffloat(pw, "optionsReal"),
            self.get_mffloat(pw, "optionsImag"),
        );
        let length = r.len().max(i.len()).max(2);
        let s = self.snd(pw);

        if s.real.len() != length {
            s.real = vec![0.0; length];
            s.imag = vec![0.0; length];
        }

        s.real[..r.len()].copy_from_slice(&r);
        s.imag[..i.len()].copy_from_slice(&i);
        (s.real.clone(), s.imag.clone())
    }

    fn osc_set_start(&mut self, node: NodeId) {
        let osc = self.audio().oscillator();

        // The oscillator of an earlier start is dropped; it plays on until its scheduled stop.
        if let Some(old) = self.snd(node).node.replace(osc) {
            self.audio().release_keep_connections(old);
        }

        self.source_set_gain(node);
        self.sound_call(node, SoundCb::Detune);
        self.sound_call(node, SoundCb::Frequency);
        self.osc_set_type(node);

        let source = self.snd(node).audio_source.unwrap();
        let e = self.audio();
        e.connect(osc, source);
        e.oscillator_start(osc);

        self.sound_emit(node, "elapsedTime", FieldValue::SFTime(0.0));
    }

    fn osc_set_stop(&mut self, node: NodeId) {
        let source = self.snd(node).audio_source.unwrap();
        let osc = self.snd(node).node;
        let e = self.audio();
        let now = e.current_time();
        let fade = 0.03;

        let v = e.param_value(source, "gain");
        e.gain_set_value_at_time(source, v, now);
        e.gain_exponential_ramp(source, 1e-3, now + fade);

        // `this .oscillatorNode .stop (...)` throws a TypeError before the first start.
        if let Some(osc) = osc {
            e.oscillator_stop(osc, now + fade);
        }
    }

    // --- BufferAudioSource ----------------------------------------------------------------------

    /// BufferAudioSource.set_buffer__.
    pub(crate) fn buffer_set_buffer(&mut self, node: NodeId) {
        if self.get_bool(node, "load") {
            return;
        }

        let channels = clamp(self.get_f64(node, "numberOfChannels"), 1.0, 32.0) as usize;
        let sample_rate = clamp(self.get_f64(node, "sampleRate"), 3000.0, 768000.0);
        let length = self.get_f64(node, "bufferLength").max(0.0) as usize;

        if length != 0 {
            let data = self.get_mffloat(node, "buffer");
            let fill = data.len() >= length * channels;
            let buffer = self.audio().create_buffer(
                channels,
                length,
                sample_rate as f32,
                fill.then_some(&data[..]),
            );

            self.sound_emit(
                node,
                "bufferDuration",
                FieldValue::SFTime(length as f64 / sample_rate),
            );

            let audio_source = self.snd(node).audio_source.unwrap();

            let element = AudioElement::create(self.audio(), audio_source, Some(buffer));
            self.set_media_element(node, Some(element));
            for cb in [
                SoundCb::Detune,
                SoundCb::PlaybackRate,
                SoundCb::LoopStart,
                SoundCb::LoopEnd,
            ] {
                self.sound_call(node, cb);
            }
        } else {
            self.sound_emit(node, "bufferDuration", FieldValue::SFTime(-1.0));
            self.set_media_element(node, None);
        }
    }

    /// BufferAudioSource.unloadData.
    pub(crate) fn buffer_unload_data(&mut self, node: NodeId) {
        self.buffer_set_buffer(node);
    }

    /// BufferAudioSource.loadData: `this .setMediaElement (null)` before the fetch.
    pub(crate) fn buffer_load_start(&mut self, node: NodeId) {
        self.set_media_element(node, None);
    }

    /// BufferAudioSource.setArrayBuffer: decodeAudioData, the buffer outputs, an AudioElement over the buffer.
    /// A decode error rejects the FileLoader callback (the next URL is tried, see `buffer_load`).
    pub(crate) fn buffer_set_array_buffer(
        &mut self,
        node: NodeId,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let buffer = match self.audio().decode(bytes) {
            Ok(b) => b,
            // The rejection of decodeAudioData, reported by FileLoader with the candidate (Chrome's message).
            Err(_) => return Err(DECODE_ERROR.to_string()),
        };

        let channels = buffer.number_of_channels();
        let length = buffer.length();
        let mut data = Vec::with_capacity(channels * length);

        for c in 0..channels {
            data.extend_from_slice(buffer.get_channel_data(c));
        }

        self.sound_emit(
            node,
            "numberOfChannels",
            FieldValue::SFInt32(channels as i32),
        );
        self.sound_emit(
            node,
            "sampleRate",
            FieldValue::SFFloat(buffer.sample_rate() as f64),
        );
        self.sound_emit(node, "bufferLength", FieldValue::SFInt32(length as i32));
        self.sound_emit(node, "buffer", FieldValue::MFFloat(data));
        self.sound_emit(
            node,
            "bufferDuration",
            FieldValue::SFTime(buffer.duration()),
        );

        let audio_source = self.snd(node).audio_source.unwrap();

        let element = AudioElement::create(self.audio(), audio_source, Some(buffer));
        self.set_media_element(node, Some(element));
        for cb in [
            SoundCb::Detune,
            SoundCb::PlaybackRate,
            SoundCb::LoopStart,
            SoundCb::LoopEnd,
        ] {
            self.sound_call(node, cb);
        }
        Ok(())
    }

    /// BufferAudioSource.set_live__, the X3DUrlObject part: the auto refresh timer.
    pub(crate) fn buffer_url_set_live(&mut self, node: NodeId, live: bool) {
        if live {
            self.url_set_auto_refresh(node);
        } else if let Some(super::State::UrlObject(s)) = self.nrt(node).map(|n| &n.state) {
            let _ = s;
            if let super::State::UrlObject(s) = &mut self.nrt_mut(node).state {
                s.timer = None;
            }
        }
    }

    // --- AudioClip ------------------------------------------------------------------------------

    /// AudioClip.unloadData: `setMediaElement (null)`, `duration_changed = -1`.
    pub(crate) fn audio_clip_unload_data(&mut self, node: NodeId) {
        self.set_media_element(node, None);
        self.sound_emit(node, "duration_changed", FieldValue::SFTime(-1.0));
    }

    /// AudioClip.setArrayBuffer: decodeAudioData, an AudioElement over the buffer, `duration_changed`,
    /// setMediaElement, set_pitch__. A decode error rejects the FileLoader callback (the next URL is tried).
    pub(crate) fn audio_clip_set_array_buffer(
        &mut self,
        node: NodeId,
        bytes: Vec<u8>,
    ) -> Result<(), String> {
        let buffer = match self.audio().decode(bytes) {
            Ok(b) => b,
            // The rejection of decodeAudioData, reported by FileLoader with the candidate (Chrome's message).
            Err(_) => return Err(DECODE_ERROR.to_string()),
        };

        let audio_source = self.snd(node).audio_source.unwrap();
        let element = AudioElement::create(self.audio(), audio_source, Some(buffer));

        self.sound_emit(
            node,
            "duration_changed",
            FieldValue::SFTime(element.duration),
        );
        self.set_media_element(node, Some(element));
        self.sound_call(node, SoundCb::Pitch);
        Ok(())
    }

    // --- MicrophoneSource -----------------------------------------------------------------------

    fn mic_set_media_device_id(&mut self, node: NodeId) {
        self.mic_set_stop(node, false);

        if self.get_bool(node, "isActive") {
            self.mic_set_start(node);
        }
    }

    /// MicrophoneSource.set_start: getUserMedia, then (a promise continuation) the node.
    fn mic_set_start(&mut self, node: NodeId) {
        self.snd(node).restore = false;
        let device = self.get_str(node, "mediaDeviceID");

        match self.audio().microphone(&device) {
            Ok(source) => {
                if let Some(old) = self.snd(node).node.replace(source) {
                    self.audio().release(old);
                }
                self.sound_emit(node, "elapsedTime", FieldValue::SFTime(0.0));

                if self.get_bool(node, "isActive") {
                    if self.get_bool(node, "isPaused") || !self.time_node_live(node) {
                        self.mic_set_pause(node);
                    } else {
                        self.mic_set_resume(node);
                    }
                } else {
                    self.mic_set_stop(node, false);
                }
            }
            Err(message) => self.log_error(message),
        }
    }

    fn mic_set_pause(&mut self, node: NodeId) {
        let Some(source) = self.snd(node).node else {
            return;
        };

        if self.time_node_live(node) {
            // disconnect; tracks disabled.
            self.audio().disconnect(source);
        } else {
            self.mic_set_stop(node, true);
        }
    }

    fn mic_set_resume(&mut self, node: NodeId) {
        if self.snd(node).restore {
            return self.mic_set_start(node);
        }

        let Some(source) = self.snd(node).node else {
            return;
        };
        let target = self.snd(node).audio_source.unwrap();
        self.audio().connect(source, target);
    }

    fn mic_set_stop(&mut self, node: NodeId, restore: bool) {
        let Some(source) = self.snd(node).node.take() else {
            return;
        };
        self.audio().release(source);
        self.snd(node).restore = restore;
    }

    // --- X3DSoundProcessingNode -----------------------------------------------------------------

    fn processing_initialize(&mut self, node: NodeId) {
        self.sound_interest(node, "gain", SoundCb::Gain);
        self.sound_interest(node, "channelCount", SoundCb::ChannelCount);
        self.sound_interest(node, "channelCountMode", SoundCb::ChannelCountMode);
        self.sound_interest(
            node,
            "channelInterpretation",
            SoundCb::ChannelInterpretation,
        );
        self.sound_interest(node, "children", SoundCb::Children);

        self.processing_set_enabled(node);
        self.processing_set_gain(node);
        self.bus_set_channel_count(node);
        if self.get_bool(node, "enabled") {
            self.bus_set_channel_count_mode(node);
            self.bus_set_channel_interpretation(node);
        }
        self.bus_set_children(node);
    }

    /// X3DSoundProcessingNode.setSoundProcessor.
    fn set_sound_processor(&mut self, node: NodeId, value: Option<AId>) {
        let s = self.snd(node);
        s.sound_processor = value.or(s.audio_source);

        match self.builtin_name(node) {
            Some(n) if CHANNELS.contains(&n) => self.channel_set_enabled(node),
            _ => self.processing_set_enabled(node),
        }
    }

    /// X3DSoundProcessingNode.set_enabled__.
    fn processing_set_enabled(&mut self, node: NodeId) {
        // X3DTimeDependentNode.set_enabled__.
        self.time_set_enabled(node);
        self.bus_set_enabled(node);
    }

    /// The Web Audio part of X3DSoundProcessingNode.set_enabled__ / X3DSoundChannelNode.set_enabled__.
    fn bus_set_enabled(&mut self, node: NodeId) {
        let s = self.snd(node);
        let (destination, processor, source) = (
            s.audio_destination.unwrap(),
            s.sound_processor.unwrap(),
            s.audio_source.unwrap(),
        );

        self.audio().disconnect(destination);

        if self.get_bool(node, "enabled") {
            self.bus_set_gain(node);
            self.bus_set_channel_count_mode(node);
            self.bus_set_channel_interpretation(node);
            self.audio().connect(destination, processor);
        } else {
            // If enabled field is FALSE, the audio signal passes through unmodified and is not blocked.
            let e = self.audio();
            e.set_param(destination, "gain", 1.0);
            e.set_channel_count_mode(destination, "max");
            e.set_channel_interpretation(destination, "speakers");
            e.connect(destination, source);
        }
    }

    fn processing_set_gain(&mut self, node: NodeId) {
        if !self.get_bool(node, "enabled") {
            return;
        }

        self.bus_set_gain(node);
    }

    fn bus_set_gain(&mut self, node: NodeId) {
        let v = self.get_f64(node, "gain");
        let d = self.snd(node).audio_destination.unwrap();
        self.audio().set_param(d, "gain", v);
    }

    fn bus_set_channel_count(&mut self, node: NodeId) {
        let v = clamp(self.get_f64(node, "channelCount"), 1.0, 32.0) as usize;
        let d = self.snd(node).audio_destination.unwrap();
        self.audio().set_channel_count(d, v);
    }

    fn bus_set_channel_count_mode(&mut self, node: NodeId) {
        let v = count_mode(&self.get_str(node, "channelCountMode")).unwrap_or("max");
        let d = self.snd(node).audio_destination.unwrap();
        self.audio().set_channel_count_mode(d, v);
    }

    fn bus_set_channel_interpretation(&mut self, node: NodeId) {
        let v = interpretation(&self.get_str(node, "channelInterpretation")).unwrap_or("speakers");
        let d = self.snd(node).audio_destination.unwrap();
        self.audio().set_channel_interpretation(d, v);
    }

    /// The sound children of `children` (X3DCast and the type test of set_children__).
    fn audio_children(
        &self,
        node: NodeId,
        field: &str,
        keep: impl Fn(&str) -> bool,
    ) -> Vec<NodeId> {
        let v = match self.pre_value(node, field) {
            Some(FieldValue::MFNode(v)) => v.clone(),
            _ => vec![],
        };

        v.into_iter()
            .flatten()
            .filter_map(|n| self.inner_node(n))
            .filter(|&n| self.builtin_name(n).is_some_and(&keep))
            .collect()
    }

    /// X3DSoundProcessingNode.set_children__ / X3DSoundDestinationNode.set_children__.
    fn bus_set_children(&mut self, node: NodeId) {
        let destination = self.snd(node).audio_destination.unwrap();

        for child in std::mem::take(&mut self.snd(node).child_nodes) {
            if let Some(a) = self.snd_ref(child).and_then(|s| s.audio_source) {
                self.audio().disconnect_dest(a, destination);
            }
        }

        let children = self.audio_children(node, "children", is_audio_child);

        for &child in &children {
            if let Some(a) = self.snd_ref(child).and_then(|s| s.audio_source) {
                self.audio().connect(a, destination);
            }
        }

        self.snd(node).child_nodes = children;
    }

    // --- Analyser, Convolver, Delay, WaveShaper -------------------------------------------------

    fn analyser_set_fft_size(&mut self, node: NodeId) {
        let size = clamp(
            next_power_of_two(self.get_f64(node, "fftSize")),
            32.0,
            32768.0,
        ) as usize;
        let n = self.snd(node).node.unwrap();
        let bins = match self.audio().get_mut(n) {
            ANode::Analyser(_, a) => {
                a.set_fft_size(size);
                a.frequency_bin_count()
            }
            _ => 0,
        };
        self.sound_emit(node, "frequencyBinCount", FieldValue::SFInt32(bins as i32));
    }

    fn analyser_set_decibels(&mut self, node: NodeId) {
        let min = self.get_f64(node, "minDecibels").min(0.0);
        let max = self.get_f64(node, "maxDecibels").min(0.0);
        let n = self.snd(node).node.unwrap();

        if let ANode::Analyser(_, a) = self.audio().get_mut(n) {
            let (lo, hi) = (min.min(max), min.max(max));
            // `minDecibels = …; maxDecibels = …` one after the other: a setter that would leave
            // min >= max throws IndexSizeError and the rest is skipped.
            if a.set_min_decibels(lo) {
                a.set_max_decibels(hi);
            }
        }
    }

    fn analyser_set_smoothing(&mut self, node: NodeId) {
        let v = clamp(self.get_f64(node, "smoothingTimeConstant"), 0.0, 1.0);
        let n = self.snd(node).node.unwrap();

        if let ANode::Analyser(_, a) = self.audio().get_mut(n) {
            a.set_smoothing_time_constant(v);
        }
    }

    /// Analyser.set_time: the four data fields, every frame.
    fn analyser_set_time(&mut self, node: NodeId) {
        let n = self.snd(node).node.unwrap();
        let now = self.audio().current_time();
        let (bytes_f, bytes_t, floats_f, floats_t) = match self.audio().get_mut(n) {
            ANode::Analyser(_, a) => {
                let bins = a.frequency_bin_count();
                let mut bf = vec![0u8; bins];
                let mut bt = vec![0u8; bins];
                let mut ff = vec![0f32; bins];
                let mut ft = vec![0f32; bins];
                // getByteFrequencyData, getByteTimeDomainData, getFloatFrequencyData and
                // getFloatTimeDomainData, with context .currentTime.
                a.get_byte_frequency_data(&mut bf, now);
                a.get_byte_time_domain_data(&mut bt);
                a.get_float_frequency_data(&mut ff, now);
                a.get_float_time_domain_data(&mut ft);
                (bf, bt, ff, ft)
            }
            _ => return,
        };

        self.snd(node).byte_bins = bytes_f.len();
        self.sound_emit(
            node,
            "byteFrequencyData",
            FieldValue::MFInt32(bytes_f.into_iter().map(i32::from).collect()),
        );
        self.sound_emit(
            node,
            "byteTimeDomainData",
            FieldValue::MFInt32(bytes_t.into_iter().map(i32::from).collect()),
        );
        self.sound_emit(node, "floatFrequencyData", FieldValue::MFFloat(floats_f));
        self.sound_emit(node, "floatTimeDomainData", FieldValue::MFFloat(floats_t));
    }

    fn convolver_set_buffer(&mut self, node: NodeId) {
        let channels = clamp(self.get_f64(node, "numberOfChannels"), 1.0, 32.0) as usize;
        let data = self.get_mffloat(node, "buffer");
        let rate = self.audio().sample_rate();
        let length = (data.len() / channels).max(1);
        let fill = data.len() >= length * channels;
        let buffer = self
            .audio()
            .create_buffer(channels, length, rate, fill.then_some(&data[..]));
        let n = self.snd(node).node.unwrap();

        // ConvolverNode .buffer: NotSupportedError for more than 4 channels or 3 channels.
        if channels > 4 || channels == 3 {
            return;
        }

        self.audio().convolver_set_buffer(n, buffer);
    }

    fn convolver_set_normalize(&mut self, node: NodeId) {
        let v = self.get_bool(node, "normalize");
        let n = self.snd(node).node.unwrap();

        self.audio().convolver_set_normalize(n, v);
    }

    fn delay_set_max_delay_time(&mut self, node: NodeId) {
        if let Some(old) = self.snd(node).node.take() {
            self.audio().disconnect(old);
            self.audio().release(old);
        }

        let max = self.get_f64(node, "maxDelayTime").max(0.0);
        let delay = clamp(self.get_f64(node, "delayTime"), 0.0, max);
        let Some(d) = self.audio().delay(max, delay) else {
            // new DelayNode throws NotSupportedError: the rest of set_maxDelayTime__ is skipped.
            return;
        };
        let source = self.snd(node).audio_source.unwrap();
        self.audio().connect(d, source);
        self.snd(node).node = Some(d);
        self.set_sound_processor(node, Some(d));
    }

    fn wave_shaper_set_curve(&mut self, node: NodeId) {
        let curve = self.get_mffloat(node, "curve");
        let curve = if curve.len() < 2 { vec![0.0; 2] } else { curve };
        let n = self.snd(node).node.unwrap();

        self.audio().wave_shaper_set_curve(n, curve);
    }

    fn wave_shaper_set_oversample(&mut self, node: NodeId) {
        let v = match self.get_str(node, "oversample").as_str() {
            "2X" => Oversample::X2,
            "4X" => Oversample::X4,
            _ => Oversample::None,
        };
        let n = self.snd(node).node.unwrap();
        self.audio().wave_shaper_set_oversample(n, v);
    }

    // --- X3DSoundChannelNode: ChannelMerger, ChannelSelector, ChannelSplitter -------------------

    fn channel_initialize(&mut self, node: NodeId) {
        self.sound_interest(node, "enabled", SoundCb::Enabled);
        self.sound_interest(node, "gain", SoundCb::Gain);
        self.sound_interest(node, "channelCount", SoundCb::ChannelCount);
        self.sound_interest(node, "channelCountMode", SoundCb::ChannelCountMode);
        self.sound_interest(
            node,
            "channelInterpretation",
            SoundCb::ChannelInterpretation,
        );
        self.sound_interest(node, "children", SoundCb::Children);

        self.channel_set_enabled(node);
        self.channel_set_gain(node);
        self.bus_set_channel_count(node);
        if self.get_bool(node, "enabled") {
            self.bus_set_channel_count_mode(node);
            self.bus_set_channel_interpretation(node);
        }
        self.channel_set_children(node);
    }

    fn channel_set_enabled(&mut self, node: NodeId) {
        self.bus_set_enabled(node);
    }

    fn channel_set_gain(&mut self, node: NodeId) {
        if !self.get_bool(node, "enabled") {
            return;
        }

        self.bus_set_gain(node);
    }

    /// X3DSoundChannelNode.set_children__ with the setChildNodes / connectChildNode /
    /// disconnectChildNode of ChannelMerger.
    fn channel_set_children(&mut self, node: NodeId) {
        let merger = self.builtin_name(node) == Some("ChannelMerger");
        let destination = self.snd(node).audio_destination.unwrap();

        for (i, child) in std::mem::take(&mut self.snd(node).child_nodes)
            .into_iter()
            .enumerate()
        {
            let Some(a) = self.snd_ref(child).and_then(|s| s.audio_source) else {
                continue;
            };

            match self.snd(node).node.filter(|_| merger) {
                Some(m) => {
                    if i < 32 {
                        self.audio().disconnect_to(a, 0, m, i);
                    }
                }
                None => {
                    self.audio().disconnect_dest(a, destination);
                }
            }
        }

        let children = self.audio_children(node, "children", is_audio_child);

        if merger {
            // ChannelMerger.setChildNodes.
            if let Some(m) = self.snd(node).node {
                self.audio().disconnect(m);
            }

            if self.get_bool(node, "enabled") {
                let inputs = clamp(children.len() as f64, 1.0, 32.0) as usize;

                let current = self.snd(node).node;

                if current
                    .is_none_or(|m| self.audio.as_ref().unwrap().number_of_inputs(m) != inputs)
                {
                    let m = self.audio().merger(inputs);
                    if let Some(old) = self.snd(node).node.replace(m) {
                        self.audio().release(old);
                    }
                }

                let m = self.snd(node).node.unwrap();
                self.audio().connect(m, destination);
            } else if let Some(old) = self.snd(node).node.take() {
                self.audio().release(old);
            }
        }

        for (i, &child) in children.iter().enumerate() {
            let Some(a) = self.snd_ref(child).and_then(|s| s.audio_source) else {
                continue;
            };

            match self.snd(node).node.filter(|_| merger) {
                Some(m) => {
                    if i < 32 {
                        self.audio().connect_io(a, m, 0, i);
                    }
                }
                None => self.audio().connect(a, destination),
            }
        }

        self.snd(node).child_nodes = children;
    }

    fn selector_set_channel_selection(&mut self, node: NodeId) {
        let selection = clamp(self.get_f64(node, "channelSelection"), 0.0, 31.0) as usize;
        let outputs = selection + 1;

        if let Some(s) = self.snd(node).node {
            self.audio().disconnect(s);
        }

        let current = self.snd(node).node;

        if current.is_none_or(|s| self.audio.as_ref().unwrap().number_of_outputs(s) != outputs) {
            let s = self.audio().splitter(outputs);
            if let Some(old) = self.snd(node).node.replace(s) {
                self.audio().release(old);
            }
        }

        let s = self.snd(node).node.unwrap();
        let source = self.snd(node).audio_source.unwrap();
        self.audio().connect_io(s, source, selection, 0);
        self.set_sound_processor(node, Some(s));
    }

    fn splitter_set_outputs(&mut self, node: NodeId) {
        let source = self.snd(node).audio_source.unwrap();

        match self.snd(node).node {
            Some(s) => self.audio().disconnect(s),
            None => {
                for out in self.snd(node).output_nodes.clone() {
                    if let Some(d) = self.snd_ref(out).and_then(|s| s.audio_destination) {
                        self.audio().disconnect_dest(source, d);
                    }
                }
            }
        }

        let outputs = self.audio_children(node, "outputs", |t| {
            CHANNELS.contains(&t) || PROCESSING.contains(&t)
        });
        self.snd(node).output_nodes = outputs.clone();

        if let Some(s) = self.snd(node).node {
            self.audio().disconnect_dest(source, s);
        }

        if self.get_bool(node, "enabled") {
            let n = clamp(outputs.len() as f64, 1.0, 32.0) as usize;

            let current = self.snd(node).node;

            if current.is_none_or(|s| self.audio.as_ref().unwrap().number_of_outputs(s) != n) {
                let s = self.audio().splitter(n);
                if let Some(old) = self.snd(node).node.replace(s) {
                    self.audio().release(old);
                }
            }

            let s = self.snd(node).node.unwrap();
            self.audio().connect(source, s);
        } else if let Some(old) = self.snd(node).node.take() {
            self.audio().release(old);
        }

        match self.snd(node).node {
            Some(s) => {
                for (i, out) in outputs.iter().take(32).enumerate() {
                    if let Some(d) = self.snd_ref(*out).and_then(|s| s.audio_destination) {
                        self.audio().connect_io(s, d, i, 0);
                    }
                }
            }
            None => {
                for out in outputs {
                    if let Some(d) = self.snd_ref(out).and_then(|s| s.audio_destination) {
                        self.audio().connect(source, d);
                    }
                }
            }
        }
    }

    // --- X3DSoundDestinationNode: AudioDestination, StreamAudioDestination ---------------------

    fn destination_initialize(&mut self, node: NodeId) {
        self.watch_live(node);
        self.sound_interest(node, "enabled", SoundCb::Enabled);
        self.sound_interest(node, "gain", SoundCb::Gain);
        self.sound_interest(node, "channelCount", SoundCb::ChannelCount);
        self.sound_interest(node, "channelCountMode", SoundCb::ChannelCountMode);
        self.sound_interest(
            node,
            "channelInterpretation",
            SoundCb::ChannelInterpretation,
        );
        self.sound_interest(node, "children", SoundCb::Children);

        self.destination_set_enabled(node);
        self.destination_set_gain(node);
        self.bus_set_channel_count(node);
        self.bus_set_channel_count_mode(node);
        self.bus_set_channel_interpretation(node);
        self.bus_set_children(node);
    }

    /// X3DSoundDestinationNode.set_enabled__ (StreamAudioDestination overrides it with nothing).
    pub(crate) fn destination_set_enabled(&mut self, node: NodeId) {
        if self.builtin_name(node) == Some("StreamAudioDestination") {
            return;
        }

        let active = self.sound_live(node) && self.get_bool(node, "enabled");
        let s = self.snd(node);
        let (destination, sound_destination) =
            (s.audio_destination.unwrap(), s.sound_destination.unwrap());

        if active {
            self.add_sound_destination(node);
            self.audio().connect(destination, sound_destination);
        } else {
            self.remove_sound_destination(node);
            self.audio().disconnect(destination);
        }

        // `this ._isActive = this ._enabled`.
        let enabled = self.get_bool(node, "enabled");
        self.sound_emit(node, "isActive", FieldValue::SFBool(enabled));

        // AudioDestination: start or stop the <audio> element (it plays the stream; here the
        // stream feeds the output).
    }

    fn destination_set_gain(&mut self, node: NodeId) {
        self.bus_set_gain(node);
    }

    /// AudioDestination.set_mediaDeviceID__ (`setSinkId`). The output device of the native
    /// context is the system default; another device is not selected.
    fn destination_set_media_device_id(&mut self, _node: NodeId) {}

    /// X3DSoundContext.addSoundDestination.
    fn add_sound_destination(&mut self, node: NodeId) {
        let mute = matches!(self.browser_options.get_option("Mute"), Some(v) if v.as_bool());
        let intensity = self
            .browser_options
            .get_option("SoundIntensity")
            .map_or(1.0, |v| v.as_f64());
        let gain = if mute {
            0.0
        } else {
            clamp(intensity, 0.0, 1.0)
        };
        let sd = self.snd(node).sound_destination.unwrap();
        self.audio().set_param(sd, "gain", gain);

        if !self.sound_destinations.contains(&node) {
            self.sound_destinations.push(node);
        }

        let n = self.sound_destinations.len() as i32;
        self.browser_options
            .set_rendering_value("SoundDestinations", browser_options::OptionValue::Int32(n));
    }

    fn remove_sound_destination(&mut self, node: NodeId) {
        self.sound_destinations.retain(|&n| n != node);
        let n = self.sound_destinations.len() as i32;
        self.browser_options
            .set_rendering_value("SoundDestinations", browser_options::OptionValue::Int32(n));
    }

    /// BrowserOptions.set_SoundIntensity__ (Mute or SoundIntensity changed).
    pub(crate) fn sound_set_intensity(&mut self) {
        let mute = matches!(self.browser_options.get_option("Mute"), Some(v) if v.as_bool());
        let intensity = self
            .browser_options
            .get_option("SoundIntensity")
            .map_or(1.0, |v| v.as_f64());
        let gain = if mute {
            0.0
        } else {
            clamp(intensity, 0.0, 1.0)
        };

        for node in self.sound_destinations.clone() {
            if let Some(sd) = self.snd_ref(node).and_then(|s| s.sound_destination) {
                self.audio().set_param(sd, "gain", gain);
            }
        }
    }

    // --- SpatialSound ---------------------------------------------------------------------------

    fn spatial_initialize(&mut self, node: NodeId) {
        let e = self.audio();
        let gain = e.gain(0.0);
        let panner = e.panner();
        let sound_destination = e.gain(0.0);
        e.set_channel_count(gain, 2);
        e.set_channel_count_mode(gain, "explicit");
        e.set_channel_interpretation(gain, "speakers");
        e.connect(gain, panner);
        e.connect(panner, sound_destination);
        let out = e.destination;
        e.connect(sound_destination, out);

        let s = self.snd(node);
        s.gain_node = Some(gain);
        s.node = Some(panner);
        s.sound_destination = Some(sound_destination);

        self.watch_live(node);
        if let Some(k) = self.private_field(node, "traversed") {
            self.add_interest(k, node, Cb::Sound(SoundCb::Enabled));
        }
        self.sound_interest(node, "enabled", SoundCb::Enabled);
        self.sound_interest(node, "spatialize", SoundCb::Spatialize);
        self.sound_interest(node, "intensity", SoundCb::Intensity);
        self.sound_interest(node, "gain", SoundCb::Intensity);
        self.sound_interest(node, "enableHRTF", SoundCb::EnableHrtf);
        self.sound_interest(node, "coneOuterGain", SoundCb::ConeOuterGain);
        self.sound_interest(node, "coneInnerAngle", SoundCb::ConeInnerAngle);
        self.sound_interest(node, "coneOuterAngle", SoundCb::ConeOuterAngle);
        self.sound_interest(node, "distanceModel", SoundCb::DistanceModel);
        self.sound_interest(node, "maxDistance", SoundCb::MaxDistance);
        self.sound_interest(node, "referenceDistance", SoundCb::ReferenceDistance);
        self.sound_interest(node, "rolloffFactor", SoundCb::DistanceModel);
        self.sound_interest(node, "children", SoundCb::Children);

        self.spatial_set_enabled(node);
        self.spatial_set_spatialize(node);
        self.spatial_set_intensity(node);
        for cb in [
            SoundCb::ConeOuterGain,
            SoundCb::ConeInnerAngle,
            SoundCb::ConeOuterAngle,
            SoundCb::DistanceModel,
            SoundCb::MaxDistance,
            SoundCb::ReferenceDistance,
            SoundCb::EnableHrtf,
        ] {
            self.spatial_set_panner(node, cb);
        }
        self.spatial_set_children(node);
    }

    /// SpatialSound.setTraversed.
    fn spatial_set_traversed(&mut self, node: NodeId, value: bool) {
        let traversed = self
            .private_field(node, "traversed")
            .is_some_and(|k| matches!(self.value(k), FieldValue::SFBool(true)));
        let current = self.snd(node).current_traversed;

        if value {
            if !traversed {
                self.sound_emit(node, "traversed", FieldValue::SFBool(true));
            }
        } else if current != traversed {
            self.sound_emit(node, "traversed", FieldValue::SFBool(current));
        }

        self.snd(node).current_traversed = value;
    }

    pub(crate) fn spatial_set_enabled(&mut self, node: NodeId) {
        let traversed = self
            .private_field(node, "traversed")
            .is_some_and(|k| matches!(self.value(k), FieldValue::SFBool(true)));
        let active = self.sound_live(node) && traversed && self.get_bool(node, "enabled");
        let sd = self.snd(node).sound_destination.unwrap();
        let k = self.private_field(self.browser_node, "sensorEvents");

        if active {
            let out = self.audio().destination;
            self.audio().connect(sd, out);
            self.add_sound_destination(node);
            if let Some(k) = k {
                self.add_interest(k, node, Cb::Sound(SoundCb::Update));
            }
        } else {
            self.audio().disconnect(sd);
            self.remove_sound_destination(node);
            if let Some(k) = k {
                self.remove_interest(k, node, Cb::Sound(SoundCb::Update));
            }
        }
    }

    fn spatial_set_spatialize(&mut self, node: NodeId) {
        let s = self.snd(node);
        let (gain, panner) = (s.gain_node.unwrap(), s.node.unwrap());
        let spatialize = self.get_bool(node, "spatialize");
        let e = self.audio();
        e.disconnect(gain);

        if spatialize {
            e.connect(gain, panner);
        } else {
            let out = e.destination;
            e.connect(gain, out);
        }
    }

    fn spatial_set_intensity(&mut self, node: NodeId) {
        let v = clamp(self.get_f64(node, "intensity"), 0.0, 1.0) * self.get_f64(node, "gain");
        let gain = self.snd(node).gain_node.unwrap();
        self.audio().set_param(gain, "gain", v);
    }

    fn spatial_set_panner(&mut self, node: NodeId, cb: SoundCb) {
        let degrees = |r: f64| r * 180.0 / std::f64::consts::PI;
        let cone_outer_gain = clamp(self.get_f64(node, "coneOuterGain"), 0.0, 1.0);
        let cone_inner = clamp(degrees(self.get_f64(node, "coneInnerAngle")), 0.0, 360.0);
        let cone_outer = clamp(degrees(self.get_f64(node, "coneOuterAngle")), 0.0, 360.0);
        let model = match self.get_str(node, "distanceModel").as_str() {
            "LINEAR" => DistanceModel::Linear,
            "EXPONENTIAL" => DistanceModel::Exponential,
            _ => DistanceModel::Inverse,
        };
        let rolloff = self.get_f64(node, "rolloffFactor").max(0.0);
        let max_distance = self.get_f64(node, "maxDistance").max(0.0);
        let ref_distance = self.get_f64(node, "referenceDistance").max(0.0);
        let hrtf = self.get_bool(node, "enableHRTF");
        let panner = self.snd(node).node.unwrap();

        let e = self.audio();

        // The PannerNode setters throw for values outside their ranges (RangeError); those
        // assignments do nothing.
        match cb {
            SoundCb::ConeOuterGain => e.panner_set(panner, |a| a.cone_outer_gain = cone_outer_gain),
            SoundCb::ConeInnerAngle => e.panner_set(panner, |a| a.cone_inner_angle = cone_inner),
            SoundCb::ConeOuterAngle => e.panner_set(panner, |a| a.cone_outer_angle = cone_outer),
            SoundCb::DistanceModel => e.panner_set(panner, |a| {
                a.distance_model = model;
                a.rolloff_factor = if model == DistanceModel::Linear {
                    rolloff.min(1.0)
                } else {
                    rolloff
                };
            }),
            SoundCb::MaxDistance => {
                if max_distance > 0.0 {
                    e.panner_set(panner, |a| a.max_distance = max_distance);
                }
            }
            SoundCb::ReferenceDistance => e.panner_set(panner, |a| a.ref_distance = ref_distance),
            SoundCb::EnableHrtf => e.panner_set_hrtf(panner, hrtf),
            _ => {}
        }
    }

    fn spatial_set_children(&mut self, node: NodeId) {
        let gain = self.snd(node).gain_node.unwrap();

        for child in std::mem::take(&mut self.snd(node).child_nodes) {
            if let Some(a) = self.snd_ref(child).and_then(|s| s.audio_source) {
                self.audio().disconnect_dest(a, gain);
            }
        }

        let children = self.audio_children(node, "children", is_audio_child);

        for &child in &children {
            if let Some(a) = self.snd_ref(child).and_then(|s| s.audio_source) {
                self.audio().connect(a, gain);
            }
        }

        // setVisibleObject (this .childNodes .length)
        self.snd(node).child_nodes = children;
    }

    // --- Sound ----------------------------------------------------------------------------------

    /// Sound.initialize.
    fn vrml_sound_initialize(&mut self, node: NodeId) {
        let e = self.audio();
        let gain = e.gain(0.0);
        let stereo = e.gain(0.0);
        let splitter = e.splitter(2);
        let front_left = e.gain(0.0);
        let front_right = e.gain(0.0);
        let back_left = e.gain(0.0);
        let back_right = e.gain(0.0);
        let merger = e.merger(2);
        let sound_destination = e.gain(0.0);

        e.set_channel_count(gain, 2);
        e.set_channel_count_mode(gain, "explicit");
        e.set_channel_interpretation(gain, "speakers");

        e.connect(stereo, sound_destination);
        e.connect_io(splitter, front_left, 0, 0);
        e.connect_io(splitter, front_right, 1, 0);
        e.connect_io(splitter, back_right, 0, 0);
        e.connect_io(splitter, back_left, 1, 0);
        e.connect_io(front_left, merger, 0, 0);
        e.connect_io(front_right, merger, 0, 1);
        e.connect_io(back_left, merger, 0, 0);
        e.connect_io(back_right, merger, 0, 1);
        e.connect(merger, sound_destination);
        let out = e.destination;
        e.connect(sound_destination, out);

        let s = self.snd(node);
        s.gain_node = Some(gain);
        s.sound_destination = Some(sound_destination);
        s.vrml = Some(VrmlSound {
            stereo,
            splitter,
            front_left,
            front_right,
            back_left,
            back_right,
            merger,
            sphere_matrix: x3d_fields::math::Matrix4::IDENTITY,
            enter_point: x3d_fields::math::Vector3::ZERO,
        });

        self.watch_live(node);
        if let Some(k) = self.private_field(node, "traversed") {
            self.add_interest(k, node, Cb::Sound(SoundCb::Enabled));
        }
        self.sound_interest(node, "enabled", SoundCb::Enabled);
        self.sound_interest(node, "spatialize", SoundCb::Spatialize);
        self.sound_interest(node, "intensity", SoundCb::Intensity);
        self.sound_interest(node, "source", SoundCb::Children);
        self.sound_interest(node, "children", SoundCb::Children);

        self.spatial_set_enabled(node);
        self.vrml_sound_set_spatialize(node);
        self.vrml_sound_set_intensity(node);
        self.vrml_sound_set_children(node);
    }

    /// Sound.set_spatialize__.
    fn vrml_sound_set_spatialize(&mut self, node: NodeId) {
        let s = self.snd(node);
        let (gain, splitter, stereo) = (
            s.gain_node.unwrap(),
            s.vrml.as_ref().unwrap().splitter,
            s.vrml.as_ref().unwrap().stereo,
        );
        let spatialize = self.get_bool(node, "spatialize");
        let e = self.audio();
        e.disconnect(gain);
        e.connect(gain, if spatialize { splitter } else { stereo });
    }

    /// Sound.set_intensity__.
    fn vrml_sound_set_intensity(&mut self, node: NodeId) {
        let v = clamp(self.get_f64(node, "intensity"), 0.0, 1.0);
        let gain = self.snd(node).gain_node.unwrap();
        self.audio().set_param(gain, "gain", v);
    }

    /// Sound.set_children__: the `source` node if it is an X3DSoundSourceNode, then the sound children.
    fn vrml_sound_set_children(&mut self, node: NodeId) {
        let gain = self.snd(node).gain_node.unwrap();

        for child in std::mem::take(&mut self.snd(node).child_nodes) {
            if let Some(a) = self.snd_ref(child).and_then(|s| s.audio_source) {
                self.audio().disconnect_dest(a, gain);
            }
        }

        // MovieTexture is an X3DSoundSourceNode too; its sound is not part of this port, so it only counts.
        let is_source =
            |n: &str| (SOURCES.contains(&n) && n != "ListenerPointSource") || n == "MovieTexture";
        // X3DCast sees through a PROTO instance to its inner node, as for `children`.
        let mut children: Vec<NodeId> = self
            .get_sfnode(node, "source")
            .and_then(|n| self.inner_node(n))
            .filter(|&n| self.builtin_name(n).is_some_and(is_source))
            .into_iter()
            .collect();
        children.extend(self.audio_children(node, "children", is_audio_child));

        for &child in &children {
            if let Some(a) = self.snd_ref(child).and_then(|s| s.audio_source) {
                self.audio().connect(a, gain);
            }
        }

        // setVisibleObject (this .childNodes .length)
        self.snd(node).child_nodes = children;
    }

    /// Sound.setGain (gain, pan = 0.5, rotation = 0).
    fn vrml_sound_set_gain(&mut self, node: NodeId, gain: f64, pan: f64, rotation: f64) {
        let pan_left = 1.0 - pan.powi(2);
        let pan_right = 1.0 - (1.0 - pan).powi(2);
        let rotation_front = 1.0 - rotation;
        let rotation_back = rotation;
        let Some(v) = self.snd_ref(node).and_then(|s| s.vrml.as_ref()).map(|v| {
            (
                v.stereo,
                v.front_left,
                v.front_right,
                v.back_left,
                v.back_right,
            )
        }) else {
            return;
        };
        let e = self.audio();

        for (id, value) in [
            (v.0, gain),
            (v.1, gain * rotation_front * pan_left),
            (v.2, gain * rotation_front * pan_right),
            (v.3, gain * rotation_back * pan_left),
            (v.4, gain * rotation_back * pan_right),
        ] {
            e.set_param(id, "gain", value);
        }
    }

    /// Sound.getEllipsoidParameter: the viewer's distance in the space where the ellipsoid is the unit sphere,
    /// and the point where the line from the viewer to the centre enters it, in view space.
    fn vrml_sound_ellipsoid(
        &mut self,
        node: NodeId,
        model_view: &x3d_fields::math::Matrix4,
        back: f64,
        front: f64,
    ) -> (f64, x3d_fields::math::Vector3) {
        use x3d_fields::math::{Matrix4, Rotation4, Vector3};

        let location = match self.pre_value(node, "location") {
            Some(FieldValue::SFVec3f(v)) => *v,
            _ => Vector3::ZERO,
        };
        let direction = match self.pre_value(node, "direction") {
            Some(FieldValue::SFVec3f(v)) => *v,
            _ => Vector3::new(0.0, 0.0, 1.0),
        };
        let Some(state) = self.sound.get_mut(&node).and_then(|s| s.vrml.as_mut()) else {
            return (1.0, Vector3::ZERO);
        };

        if back == 0.0 || front == 0.0 {
            let mut intersection = location;
            state.sphere_matrix.mult_vec_matrix3(&mut intersection);
            return (1.0, intersection);
        }

        let a = (back + front) / 2.0;
        let e = a - back;
        let b = (a * a - e * e).sqrt();

        let mut centre = Vector3::new(0.0, 0.0, e);
        let scale = Vector3::new(b, b, a);
        let mut rotation = Rotation4::default();
        rotation.set_vectors(&Vector3::new(0.0, 0.0, 1.0), &direction);

        let mut sphere_matrix = *model_view;
        sphere_matrix
            .translate(&location)
            .rotate(&rotation)
            .translate(&centre)
            .scale(&scale);
        state.sphere_matrix = sphere_matrix;

        let mut inv: Matrix4 = sphere_matrix;
        inv.inverse();

        let viewer = Vector3::new(inv.0[12], inv.0[13], inv.0[14]);
        centre.negate().div_vec(&scale);

        let mut normal = centre;
        normal.subtract(&viewer).normalize();

        // Sphere3 (1, Vector3 .ZERO) .intersectsLine (new Line3 (viewer, normal), enterPoint, exitPoint): no
        // intersection leaves enterPoint as it was.
        let bq = 2.0 * viewer.dot(&normal);
        let c = viewer.x * viewer.x + viewer.y * viewer.y + viewer.z * viewer.z - 1.0;
        let core = bq * bq - 4.0 * c;

        if core >= 0.0 {
            let t1 = (-bq + core.sqrt()) / 2.0;
            let t2 = (-bq - core.sqrt()) / 2.0;
            let t = t1.min(t2);
            let mut enter = normal;
            enter.multiply(t).add(&viewer);
            state.enter_point = enter;
        }

        let mut intersection = state.enter_point;
        state.sphere_matrix.mult_vec_matrix3(&mut intersection);

        (viewer.norm(), intersection)
    }

    /// Sound.getPan: pan 0 (left) … 1 (right) and rotation 0 (front) … 1 (back) of the source's x axis.
    fn vrml_sound_pan(&self, node: NodeId, model_view: &x3d_fields::math::Matrix4) -> (f64, f64) {
        use x3d_fields::math::{Rotation4, Vector3};

        if !self.get_bool(node, "spatialize") {
            return (0.5, 0.0);
        }

        let mut location = match self.pre_value(node, "location") {
            Some(FieldValue::SFVec3f(v)) => *v,
            _ => Vector3::ZERO,
        };
        let direction = match self.pre_value(node, "direction") {
            Some(FieldValue::SFVec3f(v)) => *v,
            _ => Vector3::new(0.0, 0.0, 1.0),
        };
        let mut rotation = Rotation4::default();
        rotation.set_vectors(&Vector3::new(0.0, 0.0, 1.0), &direction);
        straighten(&mut rotation);
        let mut x_axis = Vector3::new(1.0, 0.0, 0.0);
        rotation.mult_vec_rot(&mut x_axis);

        model_view.mult_vec_matrix3(&mut location);
        location.normalize();
        model_view.mult_dir_matrix(&mut x_axis);
        x_axis.normalize();

        let pan = 1.0 - clamp(location.x, -1.0, 1.0).acos() / std::f64::consts::PI;
        let rotation = clamp(x_axis.x, -1.0, 1.0).acos() / std::f64::consts::PI;

        (pan, rotation)
    }

    /// Sound.traverse.
    fn vrml_sound_traverse(&mut self, node: NodeId, model_view: &x3d_fields::math::Matrix4) {
        self.spatial_set_traversed(node, true);

        let max_back = self.get_f64(node, "maxBack").max(0.0);
        let max_front = self.get_f64(node, "maxFront").max(0.0);
        let (max_distance, max_intersection) =
            self.vrml_sound_ellipsoid(node, model_view, max_back, max_front);

        // Radius of normalized sphere is 1.
        if max_distance < 1.0 {
            let min_back = self.get_f64(node, "minBack").max(0.0);
            let min_front = self.get_f64(node, "minFront").max(0.0);
            let (min_distance, min_intersection) =
                self.vrml_sound_ellipsoid(node, model_view, min_back, min_front);
            let (pan, rotation) = self.vrml_sound_pan(node, model_view);

            if min_distance < 1.0 {
                self.vrml_sound_set_gain(node, 1.0, pan, rotation);
            } else {
                // Viewer is here at (0, 0, 0).
                let d1 = max_intersection.norm();
                let d2 = max_intersection.distance(&min_intersection);
                let d = (d1 / d2).min(1.0);

                self.vrml_sound_set_gain(node, d, pan, rotation);
            }
        } else {
            self.vrml_sound_set_gain(node, 0.0, 0.5, 0.0);
        }
    }

    /// SpatialSound is a visible object while it has sound children.
    pub fn spatial_sound_visible(&self, node: NodeId) -> bool {
        self.snd_ref(node)
            .is_some_and(|s| !s.child_nodes.is_empty())
    }

    /// SpatialSound.traverse and ListenerPointSource.traverse (DISPLAY), with the model-view
    /// matrix of the node.
    pub fn sound_traverse(&mut self, node: NodeId, model_view: &x3d_fields::math::Matrix4) {
        use x3d_fields::math::{Rotation4, Vector3};

        match self.builtin_name(node) {
            Some("Sound") => {
                if self.spatial_sound_visible(node) {
                    self.vrml_sound_traverse(node, model_view);
                }
            }
            Some("SpatialSound") => {
                if !self.spatial_sound_visible(node) {
                    return;
                }

                self.spatial_set_traversed(node, true);

                let location = match self.pre_value(node, "location") {
                    Some(FieldValue::SFVec3f(v)) => *v,
                    _ => Vector3::ZERO,
                };
                let direction = match self.pre_value(node, "direction") {
                    Some(FieldValue::SFVec3f(v)) => *v,
                    _ => Vector3::new(0.0, 0.0, 1.0),
                };
                let mut l = location;
                model_view.mult_vec_matrix3(&mut l);
                let mut d = direction;
                model_view.mult_dir_matrix(&mut d);
                d.normalize();
                let panner = self.snd(node).node.unwrap();
                let e = self.audio();
                // A TypeError (non-finite value) ends the assignments.
                for (name, v) in [
                    ("positionX", l.x),
                    ("positionY", l.y),
                    ("positionZ", l.z),
                    ("orientationX", d.x),
                    ("orientationY", d.y),
                    ("orientationZ", d.z),
                ] {
                    if !e.set_param(panner, name, v) {
                        break;
                    }
                }
            }
            Some("ListenerPointSource") => {
                if !self.get_bool(node, "enabled") || self.get_bool(node, "trackCurrentView") {
                    return;
                }

                let position = match self.pre_value(node, "position") {
                    Some(FieldValue::SFVec3f(v)) => *v,
                    _ => Vector3::ZERO,
                };
                let orientation = match self.pre_value(node, "orientation") {
                    Some(FieldValue::SFRotation(r)) => *r,
                    _ => Rotation4::default(),
                };
                let mut m = *model_view;
                m.translate(&position);
                m.rotate(&orientation);
                let (mut p, mut o) = (Vector3::ZERO, Rotation4::default());
                m.get_transform(Some(&mut p), Some(&mut o), None, None, None);
                let mut forward = Vector3::new(0.0, 0.0, -1.0);
                o.mult_vec_rot(&mut forward);
                forward.normalize();
                let mut up = Vector3::new(0.0, 1.0, 0.0);
                o.mult_vec_rot(&mut up);
                up.normalize();
                self.set_listener(
                    [p.x, p.y, p.z],
                    [forward.x, forward.y, forward.z],
                    [up.x, up.y, up.z],
                );
            }
            _ => {}
        }
    }

    fn set_listener(&mut self, p: [f64; 3], f: [f64; 3], u: [f64; 3]) {
        let e = self.audio();
        let mut values = e.listener_values();

        // `.value = v` throws a TypeError for a non-finite value: the assignments after it in
        // the same function are not made.
        for (i, v) in [p[0], p[1], p[2], f[0], f[1], f[2], u[0], u[1], u[2]]
            .into_iter()
            .enumerate()
        {
            if !(v as f32).is_finite() {
                break;
            }

            values[i] = v as f32;
        }

        e.set_listener(values);
    }

    fn listener_set_track_current_view(&mut self, node: NodeId) {
        if !self.get_bool(node, "enabled") || !self.get_bool(node, "trackCurrentView") {
            return;
        }

        self.set_listener([0.0; 3], [0.0, 0.0, -1.0], [0.0, 1.0, 0.0]);
    }

    // --- live state and release -----------------------------------------------------------------

    /// `getLive () .addInterest ("set_enabled__")` of a destination node or SpatialSound.
    pub(crate) fn sound_set_live(&mut self, node: NodeId) {
        match self.builtin_name(node) {
            Some("SpatialSound" | "Sound") => self.spatial_set_enabled(node),
            Some(n) if DESTINATIONS.contains(&n) => self.destination_set_enabled(node),
            _ => {}
        }
    }

    /// Released nodes: their Web Audio nodes are dropped.
    pub(crate) fn sound_release(&mut self, dead: &std::collections::HashSet<NodeId>) {
        let gone: Vec<NodeId> = self
            .sound
            .keys()
            .filter(|n| dead.contains(n))
            .copied()
            .collect();

        for n in gone {
            if let Some(s) = self.sound.remove(&n) {
                if let Some(e) = self.audio.as_mut() {
                    for id in s.ids() {
                        e.release(id);
                    }
                }
            }
        }

        self.sound_destinations.retain(|n| !dead.contains(n));

        for s in self.sound.values_mut() {
            s.child_nodes.retain(|n| !dead.contains(n));
            s.output_nodes.retain(|n| !dead.contains(n));
            if s.periodic_wave.is_some_and(|n| dead.contains(&n)) {
                s.periodic_wave = None;
            }
        }
    }
}

/// Rotation4.straighten (upVector = Vector3 .Y_AXIS).
fn straighten(r: &mut x3d_fields::math::Rotation4) {
    use x3d_fields::math::{Rotation4, Vector3};

    let mut up = Vector3::new(0.0, 1.0, 0.0);
    up.normalize();
    let mut local_x = Vector3::new(-1.0, 0.0, 0.0);
    r.mult_vec_rot(&mut local_x);
    let mut local_z = Vector3::new(0.0, 0.0, 1.0);
    r.mult_vec_rot(&mut local_z);

    // If viewer looks along up vector.
    if local_z.dot(&up).abs() >= 1.0 {
        return;
    }

    let mut new_x = local_z;
    new_x.cross(&up).normalize();

    if new_x.dot(&local_x) <= -1.0 {
        r.mult_left(&Rotation4::new(0.0, 0.0, 1.0, std::f64::consts::PI));
    } else {
        let mut rotation = Rotation4::default();
        rotation.set_vectors(&local_x, &new_x);
        r.mult_right(&rotation);
    }
}

impl AudioElement {
    /// AudioElement.create (audioContext, audioSource, audioBuffer).
    pub fn create(
        e: &mut AudioEngine,
        audio_source: AId,
        buffer: Option<AudioBuffer>,
    ) -> AudioElement {
        let source = e.buffer_source(None);
        let duration = buffer.as_ref().map_or(0.0, |b| b.duration());

        AudioElement {
            source,
            audio_source,
            buffer,
            detune: 0.0,
            playback_rate: 1.0,
            loop_start: 0.0,
            loop_end: 0.0,
            loop_: false,
            start_time: 0.0,
            current_time: 0.0,
            active: false,
            duration,
        }
    }

    pub fn set_detune(&mut self, e: &mut AudioEngine, v: f64) {
        self.detune = v;
        e.set_param(self.source, "detune", v);
    }

    pub fn set_playback_rate(&mut self, e: &mut AudioEngine, v: f64) {
        self.playback_rate = v;
        e.set_param(self.source, "playbackRate", v);
    }

    pub fn set_loop_start(&mut self, e: &mut AudioEngine, v: f64) {
        self.loop_start = v;
        e.buffer_source_set_loop_start(self.source, v);
    }

    pub fn set_loop_end(&mut self, e: &mut AudioEngine, v: f64) {
        self.loop_end = v;
        e.buffer_source_set_loop_end(self.source, v);
    }

    pub fn set_loop(&mut self, e: &mut AudioEngine, v: bool) {
        if !v {
            self.start_time = e.current_time() - self.current_time(e);
        }

        self.loop_ = v;
        e.buffer_source_set_loop(self.source, v);
    }

    pub fn current_time(&self, e: &AudioEngine) -> f64 {
        if self.active {
            if self.duration != 0.0 {
                if self.loop_ {
                    return (e.current_time() - self.start_time) % self.duration;
                }

                return e.current_time() - self.start_time;
            }

            return 0.0;
        }

        self.current_time
    }

    pub fn set_current_time(&mut self, e: &mut AudioEngine, v: f64) {
        self.current_time = v;
        self.start_time = e.current_time() - v;

        if !self.active {
            return;
        }

        self.pause(e);
        self.play(e);
    }

    pub fn play(&mut self, e: &mut AudioEngine) {
        if self.active {
            return;
        }

        // A new AudioBufferSourceNode with the buffer (its detune is the default 0).
        let new = e.buffer_source(self.buffer.as_ref());
        let old = std::mem::replace(&mut self.source, new);
        e.release(old);

        let (current, start_at) = (self.current_time, e.current_time());
        e.buffer_source_set_loop_start(self.source, self.loop_start);
        e.buffer_source_set_loop_end(self.source, self.loop_end);
        e.buffer_source_set_loop(self.source, self.loop_);
        e.set_param(self.source, "playbackRate", self.playback_rate);
        e.connect(self.source, self.audio_source);
        e.buffer_source_start(self.source, 0.0, current);

        self.start_time = start_at - current;
        self.active = true;
    }

    pub fn pause(&mut self, e: &mut AudioEngine) {
        if !self.active {
            return;
        }

        e.buffer_source_stop(self.source, 0.0);
        e.disconnect(self.source);

        self.current_time = self.current_time(e);
        self.active = false;

        // Create 1s silence to clear channels.
        let silence = e.buffer_source(None);
        e.connect(silence, self.audio_source);
        let now = e.current_time();
        e.buffer_source_start(silence, 0.0, 0.0);
        e.buffer_source_stop(silence, now + 1.0);
        // The handle is dropped; the render graph frees the node when it has ended.
        e.release_keep_connections(silence);
    }
}
