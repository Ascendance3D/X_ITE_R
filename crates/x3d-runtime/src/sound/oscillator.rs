//! OscillatorNode with the waveforms and the read-out of Chromium's Web Audio implementation.
//!
//! The Web Audio specification defines the oscillator types by Fourier series and leaves the
//! band limiting to the implementation. The pinned X_ITE runs in Chromium, so its oscillators
//! sound like Chromium's: a PeriodicWave of 36 band-limited tables (three per octave), read with
//! linear interpolation between neighboring samples and between the two tables around the pitch.
//! `web-audio-api`'s OscillatorNode generates square and sawtooth waves with polyBLEP instead,
//! which differs from that by up to the full amplitude at every edge. This node is an
//! AudioWorklet processor that follows Blink's algorithm:
//!
//! * `periodic_wave_handler.cc`: table size, ranges, partial culling, normalization (with the
//!   FFTFrame of fft_frame.rs);
//! * `oscillator_handler.cc`: k-rate and a-rate read-out, 2/3/5-point interpolation for small
//!   increments, start offset;
//! * `cpu/x86/oscillator_kernel_sse2.cc`: the four-sample vector kernels (float read index for
//!   k-rate, double for a-rate), whose single-precision arithmetic is reproduced in scalar f32;
//! * `audio_scheduled_source_handler.cc` and `audio_utilities.cc`: start and stop frames.
//!
//! Chromium (third_party/blink/renderer/modules/webaudio) is BSD-3-Clause, Copyright Google Inc.
//! and Apple Inc.

use super::fft_frame::FftFrame;
use super::param_timeline::Timeline;
use std::any::Any;
use std::sync::Arc;
use web_audio_api::AutomationRate;
use web_audio_api::AudioParamDescriptor;
use web_audio_api::worklet::{AudioParamValues, AudioWorkletGlobalScope, AudioWorkletProcessor};

const NUMBER_OF_OCTAVE_BANDS: f32 = 3.0;
const CENTS_PER_OCTAVE: f32 = 1200.0;
const CENTS_PER_RANGE: f32 = CENTS_PER_OCTAVE / NUMBER_OF_OCTAVE_BANDS;
const INTERPOLATE_2_POINT: f32 = 0.3;
const INTERPOLATE_3_POINT: f32 = 0.16;

/// OscillatorHandler SINE, SQUARE, SAWTOOTH, TRIANGLE.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
pub enum Shape {
    Sine,
    Square,
    Sawtooth,
    Triangle,
}

/// PeriodicWaveHandler: the band-limited tables of one waveform at one sample rate.
pub struct WaveTables {
    size: usize,
    ranges: usize,
    lowest_fundamental_frequency: f32,
    rate_scale: f32,
    tables: Vec<Vec<f32>>,
}

fn periodic_wave_size(sample_rate: f32) -> usize {
    if sample_rate <= 24000.0 {
        2048
    } else if sample_rate <= 88200.0 {
        4096
    } else {
        16384
    }
}

impl WaveTables {
    fn empty(sample_rate: f32) -> WaveTables {
        let size = periodic_wave_size(sample_rate);
        let ranges = (NUMBER_OF_OCTAVE_BANDS * (size as f32).log2()).round() as usize;

        WaveTables { size, ranges, lowest_fundamental_frequency: sample_rate / size as f32, rate_scale: size as f32 / sample_rate, tables: vec![] }
    }

    /// PeriodicWaveHandler::Create (createPeriodicWave (real, imag)).
    pub fn new(sample_rate: f32, real: &[f32], imag: &[f32], disable_normalization: bool) -> WaveTables {
        let mut w = WaveTables::empty(sample_rate);
        w.create_band_limited_tables(real, imag, disable_normalization);
        w
    }

    /// PeriodicWaveHandler::CreateBasicWaveform.
    pub fn basic(sample_rate: f32, shape: Shape) -> WaveTables {
        let mut w = WaveTables::empty(sample_rate);
        let half = w.size / 2;
        let mut real = vec![0f32; half];
        let mut imag = vec![0f32; half];

        for n in 1..half {
            let pi_factor = 2.0 / (n as f32 * std::f32::consts::PI);
            let b = match shape {
                Shape::Sine => {
                    if n == 1 {
                        1.0
                    } else {
                        0.0
                    }
                }
                Shape::Square => {
                    if n & 1 == 1 {
                        2.0 * pi_factor
                    } else {
                        0.0
                    }
                }
                Shape::Sawtooth => pi_factor * if n & 1 == 1 { 1.0 } else { -1.0 },
                Shape::Triangle => {
                    if n & 1 == 1 {
                        2.0 * (pi_factor * pi_factor) * if ((n - 1) >> 1) & 1 == 1 { -1.0 } else { 1.0 }
                    } else {
                        0.0
                    }
                }
            };
            real[n] = 0.0;
            imag[n] = b;
        }

        w.create_band_limited_tables(&real, &imag, false);
        w
    }

    /// PeriodicWaveHandler::CreateBandLimitedTables.
    fn create_band_limited_tables(&mut self, real_data: &[f32], imag_data: &[f32], disable_normalization: bool) {
        let fft_size = self.size;
        let half_size = fft_size / 2;
        let number_of_components = real_data.len().min(imag_data.len()).min(half_size);
        let mut normalization_scale = 0.5f32;
        let mut frame = FftFrame::new(fft_size);

        for range_index in 0..self.ranges {
            let scale = fft_size as f32;

            for i in 0..number_of_components {
                frame.real[i] = real_data[i] * scale;
                frame.imag[i] = imag_data[i] * -scale;
            }

            let cents_to_cull = range_index as f32 * CENTS_PER_RANGE;
            let culling_scale = 2f64.powf((-cents_to_cull / CENTS_PER_OCTAVE) as f64) as f32;
            let number_of_partials = (culling_scale * half_size as f32) as usize;

            for i in number_of_components.min(number_of_partials + 1)..half_size {
                frame.real[i] = 0.0;
                frame.imag[i] = 0.0;
            }

            frame.real[0] = 0.0;
            frame.imag[0] = 0.0;

            let mut table = vec![0f32; fft_size];
            frame.do_inverse_fft(&mut table);

            if !disable_normalization && range_index == 0 {
                let max_value = table.iter().fold(0f32, |m, x| m.max(x.abs()));

                if max_value != 0.0 {
                    normalization_scale = 1.0 / max_value;
                }
            }

            for x in table.iter_mut() {
                *x *= normalization_scale;
            }

            self.tables.push(table);
        }
    }

    /// PeriodicWaveHandler::WaveDataForFundamentalFrequency: (lower, higher, table factor).
    fn wave_data(&self, fundamental_frequency: f32) -> (usize, usize, f32) {
        let f = fundamental_frequency.abs();
        let ratio = if f > 0.0 { f / self.lowest_fundamental_frequency } else { 0.5 };
        let cents = ratio.log2() * CENTS_PER_OCTAVE;
        let pitch_range = (1.0 + cents / CENTS_PER_RANGE).max(0.0).min((self.ranges - 1) as f32);
        let i1 = pitch_range as usize;
        let i2 = if i1 < self.ranges - 1 { i1 + 1 } else { i1 };

        (i2, i1, pitch_range - i1 as f32)
    }
}

/// DoInterpolation (oscillator_handler.cc).
fn do_interpolation(virtual_read_index: f64, incr: f32, mask: usize, factor: f32, lower: &[f32], higher: &[f32]) -> f32 {
    let mut sample_lower = 0f64;
    let mut sample_higher = 0f64;
    let mut r0 = virtual_read_index as u32 as usize;

    if incr >= INTERPOLATE_2_POINT {
        let f = (virtual_read_index as f32 - r0 as f32) as f64;
        let r2 = (r0 + 1) & mask;
        r0 &= mask;
        sample_higher = (1.0 - f) * higher[r0] as f64 + f * higher[r2] as f64;
        sample_lower = (1.0 - f) * lower[r0] as f64 + f * lower[r2] as f64;
    } else if incr >= INTERPOLATE_3_POINT {
        let t = virtual_read_index - r0 as f64;
        let a = [0.5 * t * (t - 1.0), 1.0 - t * t, 0.5 * t * (t + 1.0)];

        for k in 0..3 {
            let i = (r0.wrapping_add(k).wrapping_sub(1)) & mask;
            sample_lower += a[k] * lower[i] as f64;
            sample_higher += a[k] * higher[i] as f64;
        }
    } else {
        let t = virtual_read_index - r0 as f64;
        let t2 = t * t;
        let a = [
            t * (t2 - 1.0) * (t - 2.0) / 24.0,
            -t * (t - 1.0) * (t2 - 4.0) / 6.0,
            (t2 - 1.0) * (t2 - 4.0) / 4.0,
            -t * (t + 1.0) * (t2 - 4.0) / 6.0,
            t * (t2 - 1.0) * (t + 2.0) / 24.0,
        ];

        for k in 0..5 {
            let i = (r0.wrapping_add(k).wrapping_sub(2)) & mask;
            sample_lower += a[k] * lower[i] as f64;
            sample_higher += a[k] * higher[i] as f64;
        }
    }

    ((1.0 - factor) as f64 * sample_higher + factor as f64 * sample_lower) as f32
}

/// WrapVirtualIndexVector (SSE, single precision).
fn wrap_f32(x: f32, size: f32, inv: f32) -> f32 {
    let r = x * inv;
    let mut f = r as i32;

    if r < f as f32 {
        f -= 1;
    }

    x - f as f32 * size
}

/// WrapVirtualIndexVectorPd (SSE2, double precision).
fn wrap_f64(x: f64, size: f64, inv: f64) -> f64 {
    let r = x * inv;
    let mut f = r as i32;

    if r < f as f64 {
        f -= 1;
    }

    x - f as f64 * size
}

fn wrap(x: f64, size: usize) -> f64 {
    x - (x * (1.0 / size as f64)).floor() * size as f64
}

/// Messages from the node to its processor.
pub enum Message {
    Wave(Arc<WaveTables>),
    Start(f64),
    Stop(f64),
    /// `frequency .value = …` (true) or `detune .value = …` (false): setValueAtTime (v, time).
    ParamEvent(bool, f64),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Unscheduled,
    Scheduled,
    Playing,
    Finished,
}

pub struct Processor {
    wave: Option<Arc<WaveTables>>,
    virtual_read_index: f64,
    state: State,
    start_time: f64,
    end_time: Option<f64>,
    phase_increments: Vec<f32>,
    detune_values: Vec<f32>,
    frequency_timeline: Timeline,
    detune_timeline: Timeline,
    /// setValue events posted since the last quantum, inserted at its start.
    param_events: Vec<(bool, f64)>,
}

/// audio_utilities::TimeToSampleFrame with kRoundUp.
fn time_to_frame_up(time: f64, sample_rate: f64) -> u64 {
    let frame = (time * sample_rate * 1024.0).round() / 1024.0;
    frame.ceil().max(0.0) as u64
}

impl AudioWorkletProcessor for Processor {
    type ProcessorOptions = Arc<WaveTables>;

    fn constructor(wave: Arc<WaveTables>) -> Self {
        Processor { wave: Some(wave), virtual_read_index: 0.0, state: State::Unscheduled, start_time: 0.0, end_time: None, phase_increments: vec![0.0; 128], detune_values: vec![0.0; 128], frequency_timeline: Timeline::default(), detune_timeline: Timeline::default(), param_events: vec![] }
    }

    fn parameter_descriptors() -> Vec<AudioParamDescriptor> {
        vec![
            AudioParamDescriptor { name: "frequency".into(), automation_rate: AutomationRate::A, default_value: 440.0, min_value: f32::MIN, max_value: f32::MAX },
            AudioParamDescriptor { name: "detune".into(), automation_rate: AutomationRate::A, default_value: 0.0, min_value: f32::MIN, max_value: f32::MAX },
        ]
    }

    fn onmessage(&mut self, msg: &mut dyn Any) {
        if let Some(m) = msg.downcast_mut::<Option<Message>>() {
            match m.take() {
                Some(Message::Wave(w)) => self.wave = Some(w),
                Some(Message::Start(t)) => {
                    if self.state == State::Unscheduled {
                        self.start_time = t;
                        self.state = State::Scheduled;
                    }
                }
                Some(Message::Stop(t)) => self.end_time = Some(t),
                Some(Message::ParamEvent(frequency, time)) => self.param_events.push((frequency, time)),
                None => {}
            }
        }
    }

    fn process<'a, 'b>(&mut self, _inputs: &'b [&'a [&'a [f32]]], outputs: &'b mut [&'a mut [&'a mut [f32]]], params: AudioParamValues<'b>, scope: &'b AudioWorkletGlobalScope) -> bool {
        let out = &mut outputs[0][0];
        let frames = out.len();
        out.fill(0.0);

        for (frequency, time) in std::mem::take(&mut self.param_events) {
            let tl = if frequency { &mut self.frequency_timeline } else { &mut self.detune_timeline };
            tl.insert(time, scope.current_frame, scope.sample_rate as f64);
        }

        let Some(wave) = self.wave.clone() else { return self.state != State::Finished };

        let sample_rate = scope.sample_rate as f64;
        let nyquist = scope.sample_rate / 2.0;
        // The param ranges of OscillatorNode: frequency ±nyquist, detune ±1200 log2 (FLT_MAX).
        let detune_max = 1200.0 * f32::MAX.log2();
        let frequency_values: Vec<f32> = params.get("frequency").iter().map(|f| f.clamp(-nyquist, nyquist)).collect();
        let detune_values: Vec<f32> = params.get("detune").iter().map(|d| d.clamp(-detune_max, detune_max)).collect();

        // UpdateSchedulingInfo.
        let quantum_start = scope.current_frame;
        let quantum_end = quantum_start + frames as u64;
        let start_frame = time_to_frame_up(self.start_time, sample_rate);
        let end_frame = self.end_time.map(|t| time_to_frame_up(t, sample_rate));

        if end_frame.is_some_and(|e| e <= quantum_start) && self.state != State::Unscheduled {
            self.state = State::Finished;
        }

        if matches!(self.state, State::Unscheduled | State::Finished) || start_frame >= quantum_end {
            return self.state != State::Finished;
        }

        let start_frame_offset = if self.state == State::Scheduled {
            self.state = State::Playing;
            self.start_time * sample_rate - start_frame as f64
        } else {
            0.0
        };

        let quantum_frame_offset = (start_frame.saturating_sub(quantum_start) as usize).min(frames);
        let mut non_silent = frames - quantum_frame_offset;

        if non_silent == 0 {
            return true;
        }

        let mut zero_from = frames;

        if let Some(e) = end_frame {
            if e >= quantum_start && e <= quantum_end {
                if e < quantum_end {
                    let zero_start = (e - quantum_start) as usize;
                    let to_zero = frames - zero_start;
                    non_silent = non_silent.saturating_sub(to_zero);
                    zero_from = zero_start;
                }
                self.state = State::Finished;
            }
        }

        if non_silent == 0 {
            return self.state != State::Finished;
        }

        // CalculateSampleAccuratePhaseIncrements.
        let rate_scale = wave.rate_scale;
        // HasSampleAccurateValues: an a-rate value, or a recent setValue event.
        let frequency_sa = frequency_values.len() > 1 || self.frequency_timeline.has_sample_accurate_values(quantum_start, sample_rate, frames);
        let detune_sa = detune_values.len() > 1 || self.detune_timeline.has_sample_accurate_values(quantum_start, sample_rate, frames);
        let frequency_values = if frequency_sa && frequency_values.len() == 1 { vec![frequency_values[0]; frames] } else { frequency_values };
        let detune_values = if detune_sa && detune_values.len() == 1 { vec![detune_values[0]; frames] } else { detune_values };
        self.frequency_timeline.after_values(quantum_start, sample_rate, frames);
        self.detune_timeline.after_values(quantum_start, sample_rate, frames);
        let has_sa = frequency_sa || detune_sa;
        let mut final_scale = rate_scale;

        if frequency_sa {
            self.phase_increments[..frames].copy_from_slice(&frequency_values[..frames]);
        } else {
            final_scale *= frequency_values[0];
        }

        if detune_sa {
            let k = (1.0f64 / 1200.0) as f32;
            let target: &mut Vec<f32> = if frequency_sa { &mut self.detune_values } else { &mut self.phase_increments };

            for i in 0..frames {
                target[i] = (detune_values[i] * k).exp2();
            }

            if frequency_sa {
                for i in 0..frames {
                    self.phase_increments[i] *= self.detune_values[i];
                }
            }
        } else {
            final_scale *= (detune_values[0] / 1200.0).exp2();
        }

        if has_sa {
            for p in self.phase_increments[..frames].iter_mut() {
                *p = if p.is_nan() { nyquist } else { p.clamp(-nyquist, nyquist) };
                *p *= final_scale;
            }
        }

        // Process.
        let mut virtual_read_index = self.virtual_read_index;
        let mut frequency = 0f32;

        if !has_sa {
            frequency = frequency_values[0] * (detune_values[0] / 1200.0).exp2();
            frequency = if frequency.is_nan() { nyquist } else { frequency.clamp(-nyquist, nyquist) };
        }

        let mut dest = quantum_frame_offset;
        let mut n = non_silent;

        if start_frame_offset > 0.0 {
            dest += 1;
            n -= 1;
            virtual_read_index += (1.0 - start_frame_offset) * frequency as f64 * rate_scale as f64;
        } else if start_frame_offset < 0.0 {
            virtual_read_index = -start_frame_offset * frequency as f64 * rate_scale as f64;
        }

        let range = dest..dest + n;

        virtual_read_index = if has_sa {
            let increments = self.phase_increments[..n].to_vec();
            process_a_rate(&wave, &mut out[range], virtual_read_index, &increments)
        } else {
            process_k_rate(&wave, &mut out[range], virtual_read_index, frequency)
        };

        self.virtual_read_index = virtual_read_index;

        for x in out[zero_from..].iter_mut() {
            *x = 0.0;
        }

        self.state != State::Finished
    }
}

/// ProcessKRate (with the x86 ProcessKRateVector).
fn process_k_rate(wave: &WaveTables, dest: &mut [f32], mut virtual_read_index: f64, frequency: f32) -> f64 {
    let n = dest.len();
    let size = wave.size;
    let mask = size - 1;
    let (lo, hi, factor) = wave.wave_data(frequency);
    let (lower, higher) = (&wave.tables[lo][..], &wave.tables[hi][..]);
    let rate_scale = wave.rate_scale;
    let incr = frequency * rate_scale;

    if incr >= INTERPOLATE_2_POINT {
        // ProcessKRateVector: four lanes in single precision.
        let wave_size = size as f32;
        let inv = 1.0f32 / size as f32;
        let mut lanes = [0f32; 4];

        for (m, lane) in lanes.iter_mut().enumerate() {
            let x = (virtual_read_index + (m as f32 * incr) as f64) as f32;
            *lane = wrap_f32(x, wave_size, inv);
        }

        let v_incr = 4.0 * incr;
        let mut k = 0;

        for _ in 0..n / 4 {
            for m in 0..4 {
                let v = lanes[m];
                let r0 = ((v as i32) as u32 as usize) & mask;
                let r1 = (r0 + 1) & mask;
                let f = v - r0 as i32 as f32;
                let sh = higher[r0] + f * (higher[r1] - higher[r0]);
                let sl = lower[r0] + f * (lower[r1] - lower[r0]);
                dest[k + m] = sh + factor * (sl - sh);
                lanes[m] = wrap_f32(v + v_incr, wave_size, inv);
            }

            k += 4;
        }

        let mut v_index = virtual_read_index + k as f64 * incr as f64;
        v_index = wrap(v_index, size);

        // ProcessKRateScalar for the rest.
        for d in dest[k..].iter_mut() {
            let r = v_index as u32 as usize;
            let f = v_index as f32 - r as f32;
            let r0 = r & mask;
            let r1 = (r0 + 1) & mask;
            let sh = higher[r0] + f * (higher[r1] - higher[r0]);
            let sl = lower[r0] + f * (lower[r1] - lower[r0]);
            *d = sh + factor * (sl - sh);
            v_index += incr as f64;
            v_index = wrap(v_index, size);
        }

        virtual_read_index += n as f64 * incr as f64;
        wrap(virtual_read_index, size)
    } else {
        for d in dest.iter_mut() {
            *d = do_interpolation(virtual_read_index, incr.abs(), mask, factor, lower, higher);
            virtual_read_index += incr as f64;
            virtual_read_index = wrap(virtual_read_index, size);
        }

        virtual_read_index
    }
}

/// ProcessARate: ProcessARateVector (x86 kernel in double precision) and ProcessARateScalar.
fn process_a_rate(wave: &WaveTables, dest: &mut [f32], mut virtual_read_index: f64, increments: &[f32]) -> f64 {
    let n = dest.len();
    let size = wave.size;
    let mask = size - 1;
    let inv_rate_scale = 1.0 / wave.rate_scale;
    let mut k = 0;

    for _ in 0..n / 4 {
        let mut big = true;
        let mut data = [(0usize, 0usize, 0f32); 4];

        for m in 0..4 {
            let p = increments[k + m];
            big = big && p.abs() >= INTERPOLATE_2_POINT;
            data[m] = wave.wave_data(inv_rate_scale * p);
        }

        if big {
            // ProcessARateVectorKernel (SSE2).
            let mut sum = [0f64; 4];
            sum[0] = increments[k] as f64;
            for m in 1..4 {
                sum[m] = sum[m - 1] + increments[k + m] as f64;
            }

            let idx = [virtual_read_index, virtual_read_index + sum[0], virtual_read_index + sum[1], virtual_read_index + sum[2]];

            for m in 0..4 {
                let v = wrap_f64(idx[m], size as f64, 1.0 / size as f64);
                let r = v as i32;
                let f = v as f32 - r as f32;
                let r0 = (r as u32 as usize) & mask;
                let r1 = ((r + 1) as u32 as usize) & mask;
                let (lower, higher, factor) = (&wave.tables[data[m].0], &wave.tables[data[m].1], data[m].2);
                let sh = higher[r0] + f * (higher[r1] - higher[r0]);
                let sl = lower[r0] + f * (lower[r1] - lower[r0]);
                dest[k + m] = sh + factor * (sl - sh);
            }

            virtual_read_index += sum[3];
            virtual_read_index = wrap(virtual_read_index, size);
        } else {
            for m in 0..4 {
                let (lower, higher, factor) = (&wave.tables[data[m].0], &wave.tables[data[m].1], data[m].2);
                dest[k + m] = do_interpolation(virtual_read_index, increments[k + m].abs(), mask, factor, lower, higher);
                virtual_read_index += increments[k + m] as f64;
                virtual_read_index = wrap(virtual_read_index, size);
            }
        }

        k += 4;
    }

    for m in k..n {
        let incr = increments[m];
        let (lo, hi, factor) = wave.wave_data(inv_rate_scale * incr);
        dest[m] = do_interpolation(virtual_read_index, incr.abs(), mask, factor, &wave.tables[lo], &wave.tables[hi]);
        virtual_read_index += incr as f64;
        virtual_read_index = wrap(virtual_read_index, size);
    }

    virtual_read_index
}
