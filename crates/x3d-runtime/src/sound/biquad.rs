//! BiquadFilterNode with Chromium's arithmetic.
//!
//! Blink evaluates the difference equation in double precision but stores each output sample,
//! and with it the feedback state y[n-1], y[n-2], as a float (`platform/audio/biquad.cc`,
//! Linux and Windows; macOS uses vDSP in double). `web-audio-api` keeps the state in double, and
//! a lowpass on a sawtooth then drifts by a few 1e-6. This AudioWorklet processor follows
//! `biquad.cc` (coefficients, Process) and `modules/webaudio/biquad_filter_handler.cc`
//! (coefficient updates, one filter per input channel).
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc.

use super::param_timeline::Timeline;
use std::any::Any;
use std::f64::consts::PI;
use web_audio_api::AudioParamDescriptor;
use web_audio_api::AutomationRate;
use web_audio_api::worklet::{AudioParamValues, AudioWorkletGlobalScope, AudioWorkletProcessor};

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Type {
    Lowpass,
    Highpass,
    Bandpass,
    Lowshelf,
    Highshelf,
    Peaking,
    Notch,
    Allpass,
}

pub enum Message {
    Type(Type),
    /// `<param> .value = …` for frequency, Q, gain, detune (0 … 3): setValueAtTime (v, time).
    ParamEvent(usize, f64),
}

#[derive(Clone, Copy, Default)]
struct Coefficients {
    b0: f64,
    b1: f64,
    b2: f64,
    a1: f64,
    a2: f64,
}

fn normalized(b0: f64, b1: f64, b2: f64, a0: f64, a1: f64, a2: f64) -> Coefficients {
    let a0_inverse = 1.0 / a0;
    Coefficients { b0: b0 * a0_inverse, b1: b1 * a0_inverse, b2: b2 * a0_inverse, a1: a1 * a0_inverse, a2: a2 * a0_inverse }
}

/// biquad.cc pow10: `fdlibm::expf (x * ln10)`, the double exp of a float, rounded to float.
fn pow10(x: f64) -> f64 {
    (((x * std::f64::consts::LN_10) as f32) as f64).exp() as f32 as f64
}

/// Biquad::Set*Params.
fn coefficients(t: Type, frequency: f64, q: f64, db_gain: f64) -> Coefficients {
    match t {
        Type::Lowpass | Type::Highpass => {
            let cutoff = frequency.clamp(0.0, 1.0);
            let low = t == Type::Lowpass;

            if cutoff == 1.0 {
                if low { normalized(1.0, 0.0, 0.0, 1.0, 0.0, 0.0) } else { normalized(0.0, 0.0, 0.0, 1.0, 0.0, 0.0) }
            } else if cutoff > 0.0 {
                let resonance = pow10(q / 20.0);
                let theta = PI * cutoff;
                let alpha = theta.sin() / (2.0 * resonance);
                let cosw = theta.cos();
                let beta = if low { (1.0 - cosw) / 2.0 } else { (1.0 + cosw) / 2.0 };
                let b1 = if low { 2.0 * beta } else { -2.0 * beta };
                normalized(beta, b1, beta, 1.0 + alpha, -2.0 * cosw, 1.0 - alpha)
            } else if low {
                normalized(0.0, 0.0, 0.0, 1.0, 0.0, 0.0)
            } else {
                normalized(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
            }
        }
        Type::Lowshelf | Type::Highshelf => {
            let frequency = frequency.clamp(0.0, 1.0);
            let a = pow10(db_gain / 40.0);
            let low = t == Type::Lowshelf;

            if frequency == 1.0 {
                if low { normalized(a * a, 0.0, 0.0, 1.0, 0.0, 0.0) } else { normalized(1.0, 0.0, 0.0, 1.0, 0.0, 0.0) }
            } else if frequency > 0.0 {
                let w0 = PI * frequency;
                let s = 1.0;
                let alpha = 0.5 * w0.sin() * ((a + 1.0 / a) * (1.0 / s - 1.0) + 2.0).sqrt();
                let k = w0.cos();
                let k2 = 2.0 * a.sqrt() * alpha;
                let a_plus_one = a + 1.0;
                let a_minus_one = a - 1.0;

                if low {
                    normalized(
                        a * (a_plus_one - a_minus_one * k + k2),
                        2.0 * a * (a_minus_one - a_plus_one * k),
                        a * (a_plus_one - a_minus_one * k - k2),
                        a_plus_one + a_minus_one * k + k2,
                        -2.0 * (a_minus_one + a_plus_one * k),
                        a_plus_one + a_minus_one * k - k2,
                    )
                } else {
                    normalized(
                        a * (a_plus_one + a_minus_one * k + k2),
                        -2.0 * a * (a_minus_one + a_plus_one * k),
                        a * (a_plus_one + a_minus_one * k - k2),
                        a_plus_one - a_minus_one * k + k2,
                        2.0 * (a_minus_one - a_plus_one * k),
                        a_plus_one - a_minus_one * k - k2,
                    )
                }
            } else if low {
                normalized(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
            } else {
                normalized(a * a, 0.0, 0.0, 1.0, 0.0, 0.0)
            }
        }
        Type::Peaking => {
            let frequency = frequency.clamp(0.0, 1.0);
            let q = q.max(0.0);
            let a = pow10(db_gain / 40.0);

            if frequency > 0.0 && frequency < 1.0 {
                if q > 0.0 {
                    let w0 = PI * frequency;
                    let alpha = w0.sin() / (2.0 * q);
                    let k = w0.cos();
                    normalized(1.0 + alpha * a, -2.0 * k, 1.0 - alpha * a, 1.0 + alpha / a, -2.0 * k, 1.0 - alpha / a)
                } else {
                    normalized(a * a, 0.0, 0.0, 1.0, 0.0, 0.0)
                }
            } else {
                normalized(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
            }
        }
        Type::Allpass | Type::Notch => {
            let frequency = frequency.clamp(0.0, 1.0);
            let q = q.max(0.0);

            if frequency > 0.0 && frequency < 1.0 {
                if q > 0.0 {
                    let w0 = PI * frequency;
                    let alpha = w0.sin() / (2.0 * q);
                    let k = w0.cos();

                    if t == Type::Allpass {
                        normalized(1.0 - alpha, -2.0 * k, 1.0 + alpha, 1.0 + alpha, -2.0 * k, 1.0 - alpha)
                    } else {
                        normalized(1.0, -2.0 * k, 1.0, 1.0 + alpha, -2.0 * k, 1.0 - alpha)
                    }
                } else if t == Type::Allpass {
                    normalized(-1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
                } else {
                    normalized(0.0, 0.0, 0.0, 1.0, 0.0, 0.0)
                }
            } else {
                normalized(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
            }
        }
        Type::Bandpass => {
            let frequency = frequency.max(0.0);
            let q = q.max(0.0);

            if frequency > 0.0 && frequency < 1.0 {
                let w0 = PI * frequency;

                if q > 0.0 {
                    let alpha = w0.sin() / (2.0 * q);
                    let k = w0.cos();
                    normalized(alpha, 0.0, -alpha, 1.0 + alpha, -2.0 * k, 1.0 - alpha)
                } else {
                    normalized(1.0, 0.0, 0.0, 1.0, 0.0, 0.0)
                }
            } else {
                normalized(0.0, 0.0, 0.0, 1.0, 0.0, 0.0)
            }
        }
    }
}

/// NormalizeFrequency (biquad_filter_handler.cc).
fn normalize_frequency(frequency: f32, nyquist: f64, detune: f32) -> f64 {
    let normalized_frequency = frequency as f64 / nyquist;

    // `exp2 (detune / 1200)` with a float detune is the float exp2f.
    if detune != 0.0 { normalized_frequency * (detune / 1200.0).exp2() as f64 } else { normalized_frequency }
}

fn flush_denormal(x: f64) -> f64 {
    if (x as f32).abs() < f32::MIN_POSITIVE { 0.0 } else { x }
}

#[derive(Clone, Copy, Default)]
struct State {
    x1: f64,
    x2: f64,
    y1: f64,
    y2: f64,
}

pub struct Processor {
    filter_type: Type,
    states: Vec<State>,
    coefficients: Vec<Coefficients>,
    sample_accurate: bool,
    /// The setValue timelines of frequency, Q, gain, detune.
    timelines: [Timeline; 4],
    param_events: Vec<(usize, f64)>,
    has_just_reset: bool,
    previous: [f32; 4],
    silent_frames: f64,
}

impl AudioWorkletProcessor for Processor {
    type ProcessorOptions = ();

    fn constructor(_: ()) -> Self {
        Processor { filter_type: Type::Lowpass, states: vec![], coefficients: vec![Coefficients::default(); 128], sample_accurate: false, has_just_reset: true, previous: [f32::NAN; 4], silent_frames: 0.0, timelines: Default::default(), param_events: vec![] }
    }

    fn parameter_descriptors() -> Vec<AudioParamDescriptor> {
        let p = |name: &str, default_value| AudioParamDescriptor { name: name.into(), automation_rate: AutomationRate::A, default_value, min_value: f32::MIN, max_value: f32::MAX };

        vec![p("frequency", 350.0), p("detune", 0.0), p("Q", 1.0), p("gain", 0.0)]
    }

    fn onmessage(&mut self, msg: &mut dyn Any) {
        match msg.downcast_mut::<Option<Message>>().and_then(Option::take) {
            // BiquadFilterHandler::SetType: reset the filters, new coefficients at the next quantum.
            Some(Message::Type(t)) => {
                if t != self.filter_type {
                    self.filter_type = t;
                    self.states.iter_mut().for_each(|s| *s = State::default());
                    self.has_just_reset = true;
                }
            }
            Some(Message::ParamEvent(index, time)) => self.param_events.push((index, time)),
            None => {}
        }
    }

    fn process<'a, 'b>(&mut self, inputs: &'b [&'a [&'a [f32]]], outputs: &'b mut [&'a mut [&'a mut [f32]]], params: AudioParamValues<'b>, scope: &'b AudioWorkletGlobalScope) -> bool {
        let nyquist = 0.5 * scope.sample_rate as f64;

        for (index, time) in std::mem::take(&mut self.param_events) {
            self.timelines[index].insert(time, scope.current_frame, scope.sample_rate as f64);
        }

        let source = inputs[0];
        let dest = &mut outputs[0];
        let channels = dest.len();
        let frames = dest.first().map_or(128, |d| d.len());

        // Initialize: one Biquad per channel, again when the channel count changes.
        if self.states.len() != channels {
            self.states = vec![State::default(); channels];
            self.has_just_reset = true;
        }

        // The AudioParam ranges.
        let ny = nyquist as f32;
        let detune_max = 1200.0 * f32::MAX.log2();
        let gain_max = 40.0 * f32::MAX.log10();
        let frequency: Vec<f32> = params.get("frequency").iter().map(|v| v.clamp(0.0, ny)).collect();
        let detune: Vec<f32> = params.get("detune").iter().map(|v| v.clamp(-detune_max, detune_max)).collect();
        let q: Vec<f32> = params.get("Q").to_vec();
        let gain: Vec<f32> = params.get("gain").iter().map(|v| v.min(gain_max)).collect();
        // HasSampleAccurateValues of the four params: an a-rate value or a recent setValue event.
        let (frame, rate) = (scope.current_frame, scope.sample_rate as f64);
        let sample_accurate = [frequency.len(), q.len(), gain.len(), detune.len()].iter().any(|n| *n > 1) || self.timelines.iter().any(|t| t.has_sample_accurate_values(frame, rate, frames));

        for t in self.timelines.iter_mut() {
            t.after_values(frame, rate, frames);
        }

        let value = |v: &[f32], k: usize| if v.len() > 1 { v[k] } else { v[0] };

        if sample_accurate {
            let constant = [&frequency, &detune, &q, &gain].iter().all(|v| v.iter().all(|x| *x == v[0]));
            let needed = if constant { 1 } else { frames };

            for k in 0..needed {
                let f = normalize_frequency(value(&frequency, k), nyquist, value(&detune, k));
                self.coefficients[k] = coefficients(self.filter_type, f, value(&q, k) as f64, value(&gain, k) as f64);
            }

            self.sample_accurate = needed > 1;
        } else {
            let finals = [frequency[0], q[0], gain[0], detune[0]];
            let dirty = if self.has_just_reset {
                self.has_just_reset = false;
                self.previous = [f32::NAN; 4];
                true
            } else if finals.iter().zip(self.previous.iter()).any(|(a, b)| a != b) {
                self.previous = finals;
                true
            } else {
                false
            };

            if dirty {
                let f = normalize_frequency(frequency[0], nyquist, detune[0]);
                self.coefficients[0] = coefficients(self.filter_type, f, q[0] as f64, gain[0] as f64);
                self.sample_accurate = false;
            }
        }

        let mut silent = true;

        for (c, d) in dest.iter_mut().enumerate() {
            let s = source.get(c).copied().filter(|s| s.len() == frames);
            let State { mut x1, mut x2, mut y1, mut y2 } = self.states[c];

            for k in 0..frames {
                let x = s.map_or(0.0, |s| s[k]);
                let co = self.coefficients[if self.sample_accurate { k } else { 0 }];
                let y = (co.b0 * x as f64 + co.b1 * x1 + co.b2 * x2 - co.a1 * y1 - co.a2 * y2) as f32;
                d[k] = y;
                x2 = x1;
                x1 = x as f64;
                y2 = y1;
                y1 = y as f64;

                if x != 0.0 || y != 0.0 {
                    silent = false;
                }
            }

            self.states[c] = State { x1: flush_denormal(x1), x2: flush_denormal(x2), y1: flush_denormal(y1), y2: flush_denormal(y2) };
        }

        // TailTime: at most kMaxTailTime (30 s); the filter is kept while it has signal.
        self.silent_frames = if silent { self.silent_frames + frames as f64 } else { 0.0 };
        self.silent_frames < 128.0 || scope.current_frame == 0
    }
}
