//! WaveShaperNode with Chromium's curve read-out and oversampling filters.
//!
//! The Web Audio specification leaves the up- and down-sampling filters of "2x" and "4x" to the
//! implementation. `web-audio-api` resamples with `rubato`; Chromium uses fixed windowed-sinc
//! half-band filters, and the two differ by up to the full amplitude on a hard curve. This
//! AudioWorklet processor follows Blink's `modules/webaudio/wave_shaper_handler.cc` and
//! `platform/audio/{up_sampler,down_sampler,direct_convolver,simple_fft_convolver}.cc`.
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc.

use super::fft_frame::{DirectConvolver, SimpleFftConvolver};
use std::any::Any;
use std::sync::Arc;
use web_audio_api::worklet::{AudioParamValues, AudioWorkletGlobalScope, AudioWorkletProcessor};

const SUB_BLOCK_FRAMES: usize = 128;
const UP_KERNEL_SIZE: usize = 128;
const DOWN_KERNEL_SIZE: usize = 256;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Oversample {
    None,
    X2,
    X4,
}

pub enum Message {
    Curve(Arc<Vec<f32>>),
    Oversample(Oversample),
}

/// DirectConvolver, or SimpleFFTConvolver for blocks larger than the kernel.
enum Convolver {
    Direct(DirectConvolver),
    Fft(SimpleFftConvolver),
}

impl Convolver {
    fn new(block: usize, kernel: Vec<f32>, direct: bool) -> Convolver {
        if direct {
            Convolver::Direct(DirectConvolver::new(block, kernel))
        } else {
            Convolver::Fft(SimpleFftConvolver::new(block, &kernel))
        }
    }

    fn process(&mut self, source: &[f32], dest: &mut [f32]) {
        match self {
            Convolver::Direct(c) => c.process(source, dest),
            Convolver::Fft(c) => c.process(source, dest),
        }
    }

    fn reset(&mut self) {
        match self {
            Convolver::Direct(c) => c.reset(),
            Convolver::Fft(c) => c.reset(),
        }
    }
}

/// up_sampler.cc MakeKernel.
fn up_kernel() -> Vec<f32> {
    let (alpha, n) = (0.16f64, UP_KERNEL_SIZE as i32);
    let (a0, a1, a2) = (0.5 * (1.0 - alpha), 0.5, 0.5 * alpha);
    let half_size = n / 2;
    let subsample_offset = -0.5;

    (0..n)
        .map(|i| {
            let s = std::f64::consts::PI * (i as f64 - half_size as f64 - subsample_offset);
            let sinc = if s == 0.0 { 1.0 } else { s.sin() / s };
            let x = (i as f64 - subsample_offset) / n as f64;
            let window = a0 - a1 * (std::f64::consts::TAU * x).cos()
                + a2 * (std::f64::consts::TAU * 2.0 * x).cos();
            (sinc * window) as f32
        })
        .collect()
}

/// down_sampler.cc MakeReducedKernel.
fn down_kernel() -> Vec<f32> {
    let (alpha, n) = (0.16f64, DOWN_KERNEL_SIZE as i32);
    let (a0, a1, a2) = (0.5 * (1.0 - alpha), 0.5, 0.5 * alpha);
    let half_size = n / 2;
    let sinc_scale_factor = 0.5;
    let mut kernel = vec![0f32; n as usize / 2];

    for i in (1..n).step_by(2) {
        let s = sinc_scale_factor * std::f64::consts::PI * (i - half_size) as f64;
        let mut sinc = if s == 0.0 { 1.0 } else { s.sin() / s };
        sinc *= sinc_scale_factor;
        let x = i as f64 / n as f64;
        let window = a0 - a1 * (std::f64::consts::TAU * x).cos()
            + a2 * (std::f64::consts::TAU * 2.0 * x).cos();
        kernel[(i as usize - 1) / 2] = (sinc * window) as f32;
    }

    kernel
}

struct UpSampler {
    convolver: Convolver,
    input: Vec<f32>,
    odd: Vec<f32>,
}

impl UpSampler {
    fn new(block: usize) -> UpSampler {
        UpSampler {
            convolver: Convolver::new(block, up_kernel(), block <= UP_KERNEL_SIZE),
            input: vec![0.0; UP_KERNEL_SIZE / 2 + block],
            odd: vec![0.0; block],
        }
    }

    fn process(&mut self, source: &[f32], dest: &mut [f32]) {
        let n = source.len();
        let half = UP_KERNEL_SIZE / 2;
        self.input[half..half + n].copy_from_slice(source);

        for i in 0..n {
            dest[i * 2] = self.input[i];
        }

        self.convolver.process(source, &mut self.odd);

        for i in 0..n {
            dest[i * 2 + 1] = self.odd[i];
        }

        self.input.copy_within(n..n + half, 0);
    }

    fn reset(&mut self) {
        self.convolver.reset();
        self.input.fill(0.0);
    }
}

struct DownSampler {
    convolver: Convolver,
    input: Vec<f32>,
    odd: Vec<f32>,
}

impl DownSampler {
    fn new(block: usize) -> DownSampler {
        DownSampler {
            convolver: Convolver::new(block / 2, down_kernel(), block <= DOWN_KERNEL_SIZE),
            input: vec![0.0; DOWN_KERNEL_SIZE / 2 + block],
            odd: vec![0.0; block / 2],
        }
    }

    fn process(&mut self, source: &[f32], dest: &mut [f32]) {
        let n = source.len();
        let half = DOWN_KERNEL_SIZE / 2;
        self.input[half..half + n].copy_from_slice(source);

        for i in 0..n / 2 {
            self.odd[i] = self.input[half - 1 + i * 2];
        }

        self.convolver.process(&self.odd, dest);

        for i in 0..n / 2 {
            dest[i] += 0.5 * self.input[i * 2];
        }

        self.input.copy_within(n..n + half, 0);
    }

    fn reset(&mut self) {
        self.convolver.reset();
        self.input.fill(0.0);
    }
}

/// WaveShaperKernel: one per channel.
struct Kernel {
    temp: Vec<f32>,
    temp2: Vec<f32>,
    up: UpSampler,
    down: DownSampler,
    up2: UpSampler,
    down2: DownSampler,
}

impl Kernel {
    fn new() -> Kernel {
        let b = SUB_BLOCK_FRAMES;
        Kernel {
            temp: vec![0.0; b * 2],
            temp2: vec![0.0; b * 4],
            up: UpSampler::new(b),
            down: DownSampler::new(b * 2),
            up2: UpSampler::new(b * 2),
            down2: DownSampler::new(b * 4),
        }
    }
}

/// WaveShaperHandler::WaveShaperCurveValues (in place).
fn curve_values(data: &mut [f32], curve: &[f32]) {
    let len = curve.len();
    let max_index = len as i64 - 1;
    let scale = (0.5 * (len - 1) as f64) as f32;

    for x in data.iter_mut() {
        let virtual_index = ((*x + 1.0) * scale).clamp(0.0, (len - 1) as f32);
        let index1 = (virtual_index as i32 as i64).clamp(0, max_index) as usize;
        let index2 = (index1 as i64 + 1).clamp(0, max_index) as usize;
        let f = virtual_index - index1 as f32;
        let v1 = curve[index1];
        let v2 = curve[index2];
        *x = f * (v2 - v1) + v1;
    }
}

pub struct Processor {
    curve: Option<Arc<Vec<f32>>>,
    oversample: Oversample,
    kernels: Vec<Kernel>,
    silent_frames: f64,
}

impl AudioWorkletProcessor for Processor {
    type ProcessorOptions = ();

    fn constructor(_: ()) -> Self {
        Processor {
            curve: None,
            oversample: Oversample::None,
            kernels: vec![],
            silent_frames: 0.0,
        }
    }

    fn onmessage(&mut self, msg: &mut dyn Any) {
        let Some(m) = msg.downcast_mut::<Option<Message>>() else {
            return;
        };

        match m.take() {
            Some(Message::Curve(c)) => self.curve = Some(c),
            Some(Message::Oversample(o)) => {
                // WaveShaperHandler::SetOversample.
                match o {
                    Oversample::None => {
                        for k in self.kernels.iter_mut() {
                            k.up.reset();
                            k.down.reset();
                            k.up2.reset();
                            k.down2.reset();
                        }
                    }
                    Oversample::X2 => {
                        for k in self.kernels.iter_mut() {
                            k.up2.reset();
                            k.down2.reset();
                        }
                    }
                    Oversample::X4 => {}
                }

                self.oversample = o;
            }
            None => {}
        }
    }

    fn process<'a, 'b>(
        &mut self,
        inputs: &'b [&'a [&'a [f32]]],
        outputs: &'b mut [&'a mut [&'a mut [f32]]],
        _params: AudioParamValues<'b>,
        scope: &'b AudioWorkletGlobalScope,
    ) -> bool {
        let source = inputs[0];
        let dest = &mut outputs[0];
        let channels = dest.len();

        // CheckNumberOfChannelsForInput: new kernels when the channel count changes.
        if self.kernels.len() != channels {
            self.kernels = (0..channels).map(|_| Kernel::new()).collect();
        }

        let mut silent = true;

        for (i, d) in dest.iter_mut().enumerate() {
            let s = source.get(i).copied().unwrap_or(&[]);
            let frames = d.len();

            if s.len() == frames {
                d.copy_from_slice(s);
            } else {
                d.fill(0.0);
            }

            if d.iter().any(|x| *x != 0.0) {
                silent = false;
            }

            let Some(curve) = &self.curve else { continue };
            let kernel = &mut self.kernels[i];

            for offset in (0..frames).step_by(SUB_BLOCK_FRAMES) {
                let n = SUB_BLOCK_FRAMES.min(frames - offset);
                let chunk = &mut d[offset..offset + n];

                match self.oversample {
                    Oversample::None => curve_values(chunk, curve),
                    Oversample::X2 => {
                        let temp = &mut kernel.temp[..n * 2];
                        kernel.up.process(chunk, temp);
                        curve_values(temp, curve);
                        kernel.down.process(temp, chunk);
                    }
                    Oversample::X4 => {
                        let temp = &mut kernel.temp[..n * 2];
                        let temp2 = &mut kernel.temp2[..n * 4];
                        kernel.up.process(chunk, temp);
                        kernel.up2.process(temp, temp2);
                        curve_values(temp2, curve);
                        kernel.down2.process(temp2, temp);
                        kernel.down.process(temp, chunk);
                    }
                }
            }
        }

        // TailTime: infinite when curve (0) is not 0, else the filter latency.
        let tail_infinite = self.curve.as_ref().is_some_and(|c| {
            let mut z = [0f32];
            curve_values(&mut z, c);
            z[0] != 0.0
        });

        self.silent_frames = if silent {
            self.silent_frames + 128.0
        } else {
            0.0
        };
        tail_infinite
            || self.silent_frames < (UP_KERNEL_SIZE + DOWN_KERNEL_SIZE) as f64
            || scope.current_frame == 0
    }
}
