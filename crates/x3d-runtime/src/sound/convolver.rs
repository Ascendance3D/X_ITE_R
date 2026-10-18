//! ConvolverNode with Chromium's Reverb.
//!
//! Blink partitions the impulse response into a direct stage of 64 frames and FFT stages that
//! double up to 8192 (`platform/audio/{reverb,reverb_convolver,reverb_convolver_stage,
//! reverb_accumulation_buffer,fft_convolver}.cc`), normalizes with -58 dB and the power of the
//! response (`reverb.cc`), and chooses the output channels from the input and the response
//! (`modules/webaudio/convolver_handler.cc`). `web-audio-api` uses other partitions and the
//! calibration 0.00125 instead of 10^(-58/20), 0.71 % lower. This AudioWorklet processor
//! follows Blink, with the FFTFrame of fft_frame.rs.
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc.

use super::fft_frame::{DirectConvolver, FftConvolver, FftFrame};
use std::any::Any;
use web_audio_api::worklet::{AudioParamValues, AudioWorkletGlobalScope, AudioWorkletProcessor};

const MIN_FFT_SIZE: usize = 128;
const MAX_FFT_SIZE: usize = 8192;
const RENDER_SLICE_SIZE: usize = 128;
const GAIN_CALIBRATION: f32 = -58.0;
const GAIN_CALIBRATION_SAMPLE_RATE: f32 = 44100.0;
const MIN_POWER: f32 = 0.000125;

/// The AVX alignment, in floats, assumed for the impulse response data: Blink's vector
/// reductions split a buffer by the alignment of its address. 0 is 32-byte aligned.
pub const RESPONSE_AVX_ALIGNMENT_OFFSET: usize = 0;

/// vector_math::Vsvesq on x86 with AVX (vector_math_x86.h): scalar and SSE frames up to the
/// AVX alignment, AVX, SSE, scalar; each SIMD part sums its lanes, then the lanes in order.
fn vsvesq(source: &[f32], avx_alignment_offset: usize) -> f32 {
    let lanes = |part: &[f32], width: usize| -> f32 {
        let mut m = vec![0f32; width];

        for chunk in part.chunks_exact(width) {
            for (l, x) in m.iter_mut().zip(chunk) {
                *l += x * x;
            }
        }

        m.iter().fold(0f32, |s, v| s + v)
    };
    let scalar = |part: &[f32]| part.iter().fold(0f32, |s, x| s + x * x);

    let n = source.len();
    let total_for_alignment = (8 - avx_alignment_offset) & 7;
    let scalar_for_alignment = total_for_alignment & 3;
    let sse_for_alignment = total_for_alignment & !3;
    let use_avx = n >= scalar_for_alignment + sse_for_alignment + 8;
    let use_sse = use_avx || n >= scalar_for_alignment + 4;
    let (mut c_scalar_a, mut c_sse_a, mut c_avx, mut c_sse) = (0, 0, 0, 0);
    let mut rest = n;

    if use_sse {
        c_scalar_a = scalar_for_alignment;
        rest -= c_scalar_a;

        if use_avx {
            c_sse_a = sse_for_alignment;
            rest -= c_sse_a;
            c_avx = rest & !7;
            rest -= c_avx;
        }

        c_sse = rest & !3;
        rest -= c_sse;
    }

    let mut sum = 0f32;
    let mut o = 0;

    for (count, width) in [(c_scalar_a, 1), (c_sse_a, 4), (c_avx, 8), (c_sse, 4), (rest, 1)] {
        if count > 0 {
            let part = &source[o..o + count];
            sum += if width == 1 { scalar(part) } else { lanes(part, width) };
            o += count;
        }
    }

    sum
}

/// reverb.cc CalculateNormalizationScale.
pub fn normalization_scale(channels: &[Vec<f32>], sample_rate: f32) -> f32 {
    let length = channels[0].len();
    let mut power = 0f32;

    for c in channels {
        power += vsvesq(c, RESPONSE_AVX_ALIGNMENT_OFFSET);
    }

    power = (power / (channels.len() * length) as f32).sqrt();

    if !power.is_finite() || power < MIN_POWER {
        power = MIN_POWER;
    }

    let mut scale = 1.0 / power;
    // fdlibm::powf: the double pow, rounded.
    scale *= 10f64.powf((GAIN_CALIBRATION * 0.05) as f64) as f32;
    scale *= GAIN_CALIBRATION_SAMPLE_RATE / sample_rate;

    if channels.len() == 4 {
        scale *= 0.5;
    }

    scale
}

/// ReverbAccumulationBuffer.
struct Accumulation {
    buffer: Vec<f32>,
    read_index: usize,
}

impl Accumulation {
    fn read_and_clear(&mut self, dest: &mut [f32]) {
        let len = self.buffer.len();

        for (k, d) in dest.iter_mut().enumerate() {
            let i = (self.read_index + k) % len;
            *d = self.buffer[i];
            self.buffer[i] = 0.0;
        }

        self.read_index = (self.read_index + dest.len()) % len;
    }

    fn update_read_index(&self, read_index: &mut usize, frames: usize) {
        *read_index = (*read_index + frames) % self.buffer.len();
    }

    fn accumulate(&mut self, source: &[f32], read_index: &mut usize, delay_frames: usize) {
        let len = self.buffer.len();
        let write_index = (*read_index + delay_frames) % len;
        *read_index = (*read_index + source.len()) % len;

        for (k, x) in source.iter().enumerate() {
            let i = (write_index + k) % len;
            self.buffer[i] = x + self.buffer[i];
        }
    }
}

enum StageConvolver {
    Direct(DirectConvolver),
    Fft(Box<FftFrame>, FftConvolver),
}

/// ReverbConvolverStage.
struct Stage {
    convolver: StageConvolver,
    temporary: Vec<f32>,
    pre_delay: Vec<f32>,
    pre_delay_length: usize,
    post_delay_length: usize,
    pre_read_write_index: usize,
    frames_processed: usize,
    accumulation_read_index: usize,
}

impl Stage {
    #[allow(clippy::too_many_arguments)]
    fn new(response: &[f32], stage_offset: usize, stage_length: usize, fft_size: usize, render_phase: usize, scale: f32, direct: bool) -> Stage {
        let convolver = if direct {
            let mut kernel = vec![0f32; fft_size / 2];
            kernel[..stage_length].copy_from_slice(&response[..stage_length]);

            if scale != 1.0 {
                kernel[..stage_length].iter_mut().for_each(|x| *x *= scale);
            }

            StageConvolver::Direct(DirectConvolver::new(RENDER_SLICE_SIZE, kernel))
        } else {
            let mut kernel = FftFrame::new(fft_size);
            kernel.do_padded_fft(&response[stage_offset..stage_offset + stage_length]);

            if scale != 1.0 {
                kernel.scale_fft(scale);
            }

            StageConvolver::Fft(Box::new(kernel), FftConvolver::new(fft_size))
        };

        let mut total_delay = stage_offset;
        let half_size = fft_size / 2;

        if !direct && total_delay >= half_size {
            total_delay -= half_size;
        }

        let max_pre_delay_length = half_size.min(total_delay);
        let mut pre_delay_length = if total_delay > 0 { render_phase % max_pre_delay_length } else { 0 };
        pre_delay_length = (pre_delay_length / RENDER_SLICE_SIZE) * RENDER_SLICE_SIZE;

        if pre_delay_length > total_delay {
            pre_delay_length = 0;
        }

        let delay_buffer_size = if pre_delay_length < fft_size { fft_size } else { pre_delay_length }.max(RENDER_SLICE_SIZE);

        Stage {
            convolver,
            temporary: vec![0.0; RENDER_SLICE_SIZE],
            pre_delay: vec![0.0; delay_buffer_size],
            pre_delay_length,
            post_delay_length: total_delay - pre_delay_length,
            pre_read_write_index: 0,
            frames_processed: 0,
            accumulation_read_index: 0,
        }
    }

    fn process(&mut self, source: &[f32], accumulation: &mut Accumulation) {
        let frames = source.len();

        if self.pre_delay_length > 0 && self.pre_read_write_index + frames > self.pre_delay.len() {
            return;
        }

        if self.frames_processed < self.pre_delay_length {
            accumulation.update_read_index(&mut self.accumulation_read_index, frames);
        } else {
            // With a pre-delay the input is the delayed source and the result goes to the
            // temporary buffer; without, the source goes in and the pre-delay buffer is the
            // temporary buffer.
            let (input, temporary): (Vec<f32>, &mut [f32]) = if self.pre_delay_length > 0 {
                let i = self.pre_read_write_index;
                (self.pre_delay[i..i + frames].to_vec(), &mut self.temporary[..frames])
            } else {
                (source.to_vec(), &mut self.pre_delay[..frames])
            };

            match &mut self.convolver {
                StageConvolver::Direct(c) => c.process(&input, temporary),
                StageConvolver::Fft(kernel, c) => c.process(kernel, &input, temporary),
            }

            accumulation.accumulate(temporary, &mut self.accumulation_read_index, self.post_delay_length);
        }

        if self.pre_delay_length > 0 {
            let i = self.pre_read_write_index;
            self.pre_delay[i..i + frames].copy_from_slice(source);
            self.pre_read_write_index += frames;

            if self.pre_read_write_index >= self.pre_delay_length {
                self.pre_read_write_index = 0;
            }
        }

        self.frames_processed += frames;
    }
}

/// ReverbConvolver: one channel of the response.
struct ReverbConvolver {
    stages: Vec<Stage>,
    accumulation: Accumulation,
}

impl ReverbConvolver {
    fn new(response: &[f32], convolver_render_phase: usize, scale: f32) -> ReverbConvolver {
        let total_response_length = response.len();
        let mut stages = vec![];
        let mut stage_offset = 0;
        let mut i = 0;
        let mut fft_size = MIN_FFT_SIZE;

        while stage_offset < total_response_length {
            let mut stage_size = fft_size / 2;

            if stage_size + stage_offset > total_response_length {
                stage_size = total_response_length - stage_offset;
            }

            let render_phase = convolver_render_phase + i * RENDER_SLICE_SIZE;
            let direct = stage_offset == 0;
            stages.push(Stage::new(response, stage_offset, stage_size, fft_size, render_phase, scale, direct));
            stage_offset += stage_size;
            i += 1;

            if !direct {
                fft_size *= 2;
            }

            fft_size = fft_size.min(MAX_FFT_SIZE);
        }

        ReverbConvolver { stages, accumulation: Accumulation { buffer: vec![0.0; total_response_length + RENDER_SLICE_SIZE], read_index: 0 } }
    }

    fn process(&mut self, source: &[f32], dest: &mut [f32]) {
        for stage in self.stages.iter_mut() {
            stage.process(source, &mut self.accumulation);
        }

        self.accumulation.read_and_clear(dest);
    }
}

/// Reverb: the convolvers of all response channels.
pub struct Reverb {
    convolvers: Vec<ReverbConvolver>,
    response_channels: usize,
    response_length: usize,
}

impl Reverb {
    /// Reverb::TryCreate: `channels` are the 1, 2 or 4 channels of the response.
    pub fn new(channels: &[Vec<f32>], sample_rate: f32, normalize: bool) -> Reverb {
        let scale = if normalize { normalization_scale(channels, sample_rate) } else { 1.0 };
        let n = channels.len();
        let convolvers = (0..n.max(2)).map(|i| ReverbConvolver::new(&channels[i.min(n - 1)], i * RENDER_SLICE_SIZE, scale)).collect();

        Reverb { convolvers, response_channels: n, response_length: channels[0].len() }
    }

    pub fn response_channels(&self) -> usize {
        self.response_channels
    }

    fn process(&mut self, source: &[&[f32]], dest: &mut [&mut [f32]]) {
        let (inputs, outputs, r) = (source.len(), dest.len(), self.response_channels);
        let frames = dest[0].len();
        let zero = vec![0f32; frames];
        let channel = |c: usize| source.get(c).copied().filter(|s| s.len() == frames).unwrap_or(&zero);
        let c = &mut self.convolvers;

        if inputs == 2 && (r == 1 || r == 2) && outputs == 2 {
            c[0].process(channel(0), dest[0]);
            c[1].process(channel(1), dest[1]);
        } else if inputs == 1 && outputs == 2 && r == 2 {
            c[0].process(channel(0), dest[0]);
            c[1].process(channel(0), dest[1]);
        } else if inputs == 1 && r == 1 && outputs == 1 {
            c[0].process(channel(0), dest[0]);
        } else if r == 4 && outputs == 2 && (inputs == 1 || inputs == 2) {
            let right = if inputs == 2 { 1 } else { 0 };
            let mut temp_l = vec![0f32; frames];
            let mut temp_r = vec![0f32; frames];
            c[0].process(channel(0), dest[0]);
            c[1].process(channel(0), dest[1]);
            c[2].process(channel(right), &mut temp_l);
            c[3].process(channel(right), &mut temp_r);

            // AudioBus::SumFrom.
            for k in 0..frames {
                dest[0][k] += temp_l[k];
                dest[1][k] += temp_r[k];
            }
        } else {
            dest.iter_mut().for_each(|d| d.fill(0.0));
        }
    }
}

pub enum Message {
    Reverb(Option<Box<Reverb>>),
}

pub struct Processor {
    reverb: Option<Box<Reverb>>,
    silent_frames: f64,
    /// AudioHandler::last_non_silent_time_, in seconds.
    last_non_silent_time: f64,
}

impl AudioWorkletProcessor for Processor {
    type ProcessorOptions = ();

    fn constructor(_: ()) -> Self {
        Processor { reverb: None, silent_frames: 0.0, last_non_silent_time: 0.0 }
    }

    fn onmessage(&mut self, msg: &mut dyn Any) {
        if let Some(Some(Message::Reverb(r))) = msg.downcast_mut::<Option<Message>>().map(Option::take) {
            // The old reverb is dropped here, in the render thread; it holds only buffers.
            self.reverb = r;
        }
    }

    fn process<'a, 'b>(&mut self, inputs: &'b [&'a [&'a [f32]]], outputs: &'b mut [&'a mut [&'a mut [f32]]], _params: AudioParamValues<'b>, scope: &'b AudioWorkletGlobalScope) -> bool {
        let dest = &mut outputs[0];
        let frames = dest.first().map_or(128, |d| d.len());
        let silent = inputs[0].iter().all(|c| c.iter().all(|x| *x == 0.0));
        let sample_rate = scope.sample_rate as f64;
        let current_time = scope.current_frame as f64 / sample_rate;

        // TailTime: the response length; LatencyTime is 0.
        let tail_time = self.reverb.as_ref().map_or(0, |r| r.response_length) as f64 / sample_rate;

        // AudioHandler::ProcessIfNecessary: a node with silent inputs is not processed once
        // PropagatesSilence, so the stage phases only advance while Chromium processes.
        if silent && self.last_non_silent_time + tail_time < current_time {
            dest.iter_mut().for_each(|d| d.fill(0.0));
        } else {
            match self.reverb.as_mut() {
                Some(r) => r.process(inputs[0], dest),
                None => dest.iter_mut().for_each(|d| d.fill(0.0)),
            }
        }

        if !silent {
            self.last_non_silent_time = (scope.current_frame as f64 + frames as f64) / sample_rate;
        }

        let tail = self.reverb.as_ref().map_or(0, |r| r.response_length) as f64;
        self.silent_frames = if silent { self.silent_frames + 128.0 } else { 0.0 };
        self.silent_frames <= tail + 128.0
    }
}
