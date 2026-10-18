//! AnalyserNode with Chromium's RealtimeAnalyser.
//!
//! `web-audio-api` keeps its own ring buffer, window and FFT; the four data arrays differ from
//! Chromium's in timing and in value. This AudioWorklet processor follows Blink's
//! `modules/webaudio/analyser_handler.cc` (pass-through, silence rule) and writes the down-mixed
//! input into a ring buffer that the control thread reads like `realtime_analyser.cc`.
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc.

use super::fft_frame::FftFrame;
use std::any::Any;
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use web_audio_api::worklet::{AudioParamValues, AudioWorkletGlobalScope, AudioWorkletProcessor};

pub const MIN_FFT_SIZE: usize = 32;
pub const MAX_FFT_SIZE: usize = 32768;
const DEFAULT_FFT_SIZE: usize = 2048;
const INPUT_BUFFER_SIZE: usize = MAX_FFT_SIZE * 2;

/// The input ring buffer, shared by the render and the control thread.
pub struct Shared {
    ring: Mutex<Ring>,
    /// channelInterpretation "discrete", for the down-mix.
    pub discrete: AtomicBool,
}

struct Ring {
    buffer: Vec<f32>,
    write_index: usize,
}

impl Default for Shared {
    fn default() -> Shared {
        Shared { ring: Mutex::new(Ring { buffer: vec![0.0; INPUT_BUFFER_SIZE], write_index: 0 }), discrete: AtomicBool::new(false) }
    }
}

impl Shared {
    fn ring(&self) -> std::sync::MutexGuard<'_, Ring> {
        self.ring.lock().unwrap_or_else(|e| e.into_inner())
    }
}

/// AudioBus::SumFrom into a mono bus (`audio_bus.cc` SumFromByDownMixing and DiscreteSumFrom).
fn down_mix(source: &[&[f32]], discrete: bool, frames: usize, dest: &mut [f32]) {
    dest[..frames].fill(0.0);

    let vsma = |dest: &mut [f32], s: &[f32], scale: f32| {
        for i in 0..frames {
            dest[i] += scale * s[i];
        }
    };

    let channels: Vec<&[f32]> = source.iter().copied().filter(|c| c.len() >= frames).collect();

    match (channels.len(), discrete) {
        (0, _) => {}
        (1, _) => vsma(dest, channels[0], 1.0),
        (2, false) => {
            vsma(dest, channels[0], 0.5);
            vsma(dest, channels[1], 0.5);
        }
        (4, false) => {
            for c in &channels {
                vsma(dest, c, 0.25);
            }
        }
        (6, false) => {
            let sqrt_half = 0.5f32.sqrt();
            vsma(dest, channels[0], sqrt_half);
            vsma(dest, channels[1], sqrt_half);

            for i in 0..frames {
                dest[i] += channels[2][i];
            }

            vsma(dest, channels[4], 0.5);
            vsma(dest, channels[5], 0.5);
        }
        // All other cases: the discrete sum of the first channel.
        _ => vsma(dest, channels[0], 1.0),
    }
}

pub struct Processor {
    shared: Arc<Shared>,
    down_mix: Vec<f32>,
    /// AudioHandler::last_non_silent_time_, in seconds.
    last_non_silent_time: f64,
}

impl AudioWorkletProcessor for Processor {
    type ProcessorOptions = Arc<Shared>;

    fn constructor(shared: Arc<Shared>) -> Self {
        Processor { shared, down_mix: vec![0.0; 128], last_non_silent_time: 0.0 }
    }

    fn onmessage(&mut self, _msg: &mut dyn Any) {}

    fn process<'a, 'b>(&mut self, inputs: &'b [&'a [&'a [f32]]], outputs: &'b mut [&'a mut [&'a mut [f32]]], _params: AudioParamValues<'b>, scope: &'b AudioWorkletGlobalScope) -> bool {
        let source = inputs[0];
        let dest = &mut outputs[0];
        let frames = dest.first().map_or(128, |d| d.len());
        let silent = source.iter().all(|c| c.iter().all(|x| *x == 0.0));
        let sample_rate = scope.sample_rate as f64;
        let current_time = scope.current_frame as f64 / sample_rate;

        // AnalyserHandler::TailTime.
        let tail_time = MAX_FFT_SIZE as f64 / sample_rate;

        // AudioHandler::ProcessIfNecessary: with silent inputs the node is not processed once
        // PropagatesSilence, and the ring buffer keeps its last contents.
        let propagates_silence = silent && self.last_non_silent_time + tail_time < current_time;

        if propagates_silence {
            dest.iter_mut().for_each(|d| d.fill(0.0));
        } else {
            // RealtimeAnalyser::WriteInput.
            if self.down_mix.len() < frames {
                self.down_mix.resize(frames, 0.0);
            }

            down_mix(source, self.shared.discrete.load(Ordering::Relaxed), frames, &mut self.down_mix);

            let mut ring = self.shared.ring();
            let write_index = ring.write_index;

            for i in 0..frames {
                ring.buffer[(write_index + i) % INPUT_BUFFER_SIZE] = self.down_mix[i];
            }

            ring.write_index = (write_index + frames) % INPUT_BUFFER_SIZE;
            drop(ring);

            // AnalyserHandler::Process: the output is the input.
            for (i, d) in dest.iter_mut().enumerate() {
                match source.get(i).filter(|s| s.len() == d.len()) {
                    Some(s) => d.copy_from_slice(s),
                    None => d.fill(0.0),
                }
            }
        }

        if !silent {
            self.last_non_silent_time = (scope.current_frame as f64 + frames as f64) / sample_rate;
        }

        !propagates_silence
    }
}

/// RealtimeAnalyser on the control thread.
pub struct Analyser {
    shared: Arc<Shared>,
    fft_size: usize,
    frame: FftFrame,
    magnitude: Vec<f32>,
    min_decibels: f64,
    max_decibels: f64,
    smoothing_time_constant: f64,
    last_analysis_time: f64,
}

/// ApplyWindow: the Blackman window.
fn apply_window(p: &mut [f32]) {
    let alpha = 0.16f64;
    let a0 = 0.5 * (1.0 - alpha);
    let a1 = 0.5;
    let a2 = 0.5 * alpha;
    let n = p.len();
    let two_pi = 2.0 * std::f64::consts::PI;

    for (i, x) in p.iter_mut().enumerate() {
        let t = i as f64 / n as f64;
        let window = a0 - a1 * (two_pi * t).cos() + a2 * (two_pi * 2.0 * t).cos();
        *x *= window as f32;
    }
}

/// audio_utilities::LinearToDecibels.
fn linear_to_decibels(linear: f32) -> f32 {
    20.0 * linear.log10()
}

/// ClampTo (value, 0, UCHAR_MAX), then static_cast<unsigned char>.
fn to_byte(value: f64) -> u8 {
    if value.is_nan() {
        return 0;
    }

    value.clamp(0.0, 255.0) as u8
}

impl Analyser {
    pub fn new(shared: Arc<Shared>) -> Analyser {
        Analyser {
            shared,
            fft_size: DEFAULT_FFT_SIZE,
            frame: FftFrame::new(DEFAULT_FFT_SIZE),
            magnitude: vec![0.0; DEFAULT_FFT_SIZE / 2],
            min_decibels: -100.0,
            max_decibels: -30.0,
            smoothing_time_constant: 0.8,
            last_analysis_time: -1.0,
        }
    }

    pub fn shared(&self) -> &Arc<Shared> {
        &self.shared
    }

    pub fn fft_size(&self) -> usize {
        self.fft_size
    }

    pub fn frequency_bin_count(&self) -> usize {
        self.fft_size / 2
    }

    pub fn min_decibels(&self) -> f64 {
        self.min_decibels
    }

    pub fn max_decibels(&self) -> f64 {
        self.max_decibels
    }

    /// AnalyserHandler::SetFftSize: false for a size the setter rejects.
    pub fn set_fft_size(&mut self, size: usize) -> bool {
        if !(MIN_FFT_SIZE..=MAX_FFT_SIZE).contains(&size) || !size.is_power_of_two() {
            return false;
        }

        if self.fft_size != size {
            self.frame = FftFrame::new(size);
            self.magnitude = vec![0.0; size / 2];
            self.fft_size = size;
        }

        true
    }

    /// AnalyserHandler::SetMinDecibels: false when the setter throws.
    pub fn set_min_decibels(&mut self, k: f64) -> bool {
        if k < self.max_decibels {
            self.min_decibels = k;
            true
        } else {
            false
        }
    }

    /// AnalyserHandler::SetMaxDecibels: false when the setter throws.
    pub fn set_max_decibels(&mut self, k: f64) -> bool {
        if k > self.min_decibels {
            self.max_decibels = k;
            true
        } else {
            false
        }
    }

    /// AnalyserHandler::SetSmoothingTimeConstant: false when the setter throws.
    pub fn set_smoothing_time_constant(&mut self, k: f64) -> bool {
        if (0.0..=1.0).contains(&k) {
            self.smoothing_time_constant = k;
            true
        } else {
            false
        }
    }

    fn analyse_if_new(&mut self, current_time: f64) {
        if current_time > self.last_analysis_time {
            self.last_analysis_time = current_time;
            self.do_fft_analysis();
        }
    }

    /// DoFFTAnalysis.
    fn do_fft_analysis(&mut self) {
        let fft_size = self.fft_size;
        let mut temp = vec![0f32; fft_size];

        {
            let ring = self.shared.ring();
            let write_index = ring.write_index;

            for (i, t) in temp.iter_mut().enumerate() {
                *t = ring.buffer[(i + write_index + INPUT_BUFFER_SIZE - fft_size) % INPUT_BUFFER_SIZE];
            }
        }

        apply_window(&mut temp);
        self.frame.do_fft(&temp);
        self.frame.imag[0] = 0.0;

        let magnitude_scale = 1.0 / fft_size as f64;
        let k = self.smoothing_time_constant.clamp(0.0, 1.0);

        for (i, d) in self.magnitude.iter_mut().enumerate() {
            let scalar_magnitude = (self.frame.real[i] as f64).hypot(self.frame.imag[i] as f64) * magnitude_scale;
            let v = (k * *d as f64 + (1.0 - k) * scalar_magnitude) as f32;
            *d = if v.is_finite() { v } else { 0.0 };
        }
    }

    pub fn get_float_frequency_data(&mut self, destination: &mut [f32], current_time: f64) {
        self.analyse_if_new(current_time);

        for (d, m) in destination.iter_mut().zip(&self.magnitude) {
            *d = linear_to_decibels(*m);
        }
    }

    pub fn get_byte_frequency_data(&mut self, destination: &mut [u8], current_time: f64) {
        self.analyse_if_new(current_time);

        let range_scale_factor = if self.max_decibels == self.min_decibels { 1.0 } else { 1.0 / (self.max_decibels - self.min_decibels) };

        for (d, m) in destination.iter_mut().zip(&self.magnitude) {
            let db_mag = linear_to_decibels(*m) as f64;
            *d = to_byte(255.0 * (db_mag - self.min_decibels) * range_scale_factor);
        }
    }

    pub fn get_float_time_domain_data(&self, destination: &mut [f32]) {
        let fft_size = self.fft_size;
        let ring = self.shared.ring();
        let write_index = ring.write_index;

        for (i, d) in destination.iter_mut().take(fft_size).enumerate() {
            *d = ring.buffer[(i + write_index + INPUT_BUFFER_SIZE - fft_size) % INPUT_BUFFER_SIZE];
        }
    }

    pub fn get_byte_time_domain_data(&self, destination: &mut [u8]) {
        let fft_size = self.fft_size;
        let ring = self.shared.ring();
        let write_index = ring.write_index;

        for (i, d) in destination.iter_mut().take(fft_size).enumerate() {
            let value = ring.buffer[(i + write_index + INPUT_BUFFER_SIZE - fft_size) % INPUT_BUFFER_SIZE];
            // `128 * (value + 1)` is float arithmetic.
            *d = to_byte((128.0 * (value + 1.0)) as f64);
        }
    }
}
