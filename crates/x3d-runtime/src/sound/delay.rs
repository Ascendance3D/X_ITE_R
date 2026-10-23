//! DelayNode with Chromium's read-out.
//!
//! Chromium computes the read position in single precision (`cpu/x86/delay_sse2.cc`), so a
//! delay of 0.0125 s at 48 kHz is exactly 600 frames there (0.0125f · 48000f rounds to 600f).
//! `web-audio-api` computes it in double precision, 600.0000089 frames, and its output then
//! differs by about 1e-5 on a plain sawtooth. This AudioWorklet processor follows Blink's
//! `platform/audio/delay.cc` and `delay_sse2.cc` (a-rate, the default automation rate of
//! `delayTime`) and `modules/webaudio/delay_handler.cc` (one kernel per input channel).
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc.

use std::any::Any;
use web_audio_api::AudioParamDescriptor;
use web_audio_api::AutomationRate;
use web_audio_api::worklet::{AudioParamValues, AudioWorkletGlobalScope, AudioWorkletProcessor};

const RENDER_QUANTUM_FRAMES: usize = 128;

/// audio_utilities::TimeToSampleFrame with kRoundUp.
fn time_to_frame_up(time: f64, sample_rate: f64) -> usize {
    let frame = (time * sample_rate * 1024.0).round() / 1024.0;
    frame.ceil().max(0.0) as usize
}

pub struct Processor {
    max_delay_time: f64,
    buffer_length: usize,
    buffers: Vec<Vec<f32>>,
    write_index: usize,
    delay_times: Vec<f32>,
    silent_frames: f64,
}

impl AudioWorkletProcessor for Processor {
    /// maxDelayTime in seconds.
    type ProcessorOptions = f64;

    fn constructor(max_delay_time: f64) -> Self {
        Processor {
            max_delay_time,
            buffer_length: 0,
            buffers: vec![],
            write_index: 0,
            delay_times: vec![0.0; RENDER_QUANTUM_FRAMES],
            silent_frames: 0.0,
        }
    }

    fn parameter_descriptors() -> Vec<AudioParamDescriptor> {
        vec![AudioParamDescriptor {
            name: "delayTime".into(),
            automation_rate: AutomationRate::A,
            default_value: 0.0,
            min_value: f32::MIN,
            max_value: f32::MAX,
        }]
    }

    fn onmessage(&mut self, _msg: &mut dyn Any) {}

    fn process<'a, 'b>(
        &mut self,
        inputs: &'b [&'a [&'a [f32]]],
        outputs: &'b mut [&'a mut [&'a mut [f32]]],
        params: AudioParamValues<'b>,
        scope: &'b AudioWorkletGlobalScope,
    ) -> bool {
        let sample_rate = scope.sample_rate;
        let source = inputs[0];
        let dest = &mut outputs[0];
        let channels = dest.len();

        // Delay::Delay, BufferLengthForDelay; new kernels when the channel count changes.
        if self.buffers.len() != channels {
            self.buffer_length =
                RENDER_QUANTUM_FRAMES + time_to_frame_up(self.max_delay_time, sample_rate as f64);
            self.buffers = vec![vec![0.0; self.buffer_length]; channels];
            self.write_index = 0;
        }

        // The AudioParam range [0, maxDelayTime].
        let max = self.max_delay_time as f32;
        let values = params.get("delayTime");
        let frames = dest.first().map_or(RENDER_QUANTUM_FRAMES, |d| d.len());

        for i in 0..frames {
            let v = if values.len() > 1 {
                values[i]
            } else {
                values[0]
            };
            // Delay::HandleNaN: a NaN delay time becomes the maximum.
            self.delay_times[i] = if v.is_nan() { max } else { v.clamp(0.0, max) };
        }

        let buffer_length = self.buffer_length;
        let length_f = buffer_length as f32;
        let mut silent = true;
        let start_write_index = self.write_index;

        for (c, d) in dest.iter_mut().enumerate() {
            let buffer = &mut self.buffers[c];
            let s = source.get(c).copied().filter(|s| s.len() == frames);

            // CopyToCircularBuffer.
            for i in 0..frames {
                let x = s.map_or(0.0, |s| s[i]);

                if x != 0.0 {
                    silent = false;
                }

                buffer[(start_write_index + i) % buffer_length] = x;
            }

            // ProcessARateVector (single precision), then ProcessARateScalar for the rest.
            let vector_frames = frames / 4 * 4;

            for k in 0..vector_frames {
                let w = (start_write_index + k) % buffer_length;
                let desired_delay_frames = self.delay_times[k].max(0.0) * sample_rate;
                let mut read_position = w as f32 + (length_f - desired_delay_frames);

                if read_position >= length_f {
                    read_position -= length_f;
                }

                let mut r1 = read_position as i32 as usize;

                if r1 >= buffer_length {
                    r1 -= buffer_length;
                }

                let r2 = if r1 + 1 >= buffer_length {
                    r1 + 1 - buffer_length
                } else {
                    r1 + 1
                };
                let f = read_position - r1 as f32;
                let (s1, s2) = (buffer[r1], buffer[r2]);
                d[k] = s1 + f * (s2 - s1);
            }

            for k in vector_frames..frames {
                let w = (start_write_index + k) % buffer_length;
                let desired_delay_frames =
                    (self.delay_times[k] as f64).max(0.0) * sample_rate as f64;
                let mut read_position = (w + buffer_length) as f64 - desired_delay_frames;

                if read_position >= buffer_length as f64 {
                    read_position -= buffer_length as f64;
                }

                let r1 = read_position as usize;
                let r2 = if r1 + 1 >= buffer_length {
                    r1 + 1 - buffer_length
                } else {
                    r1 + 1
                };
                let f = (read_position - r1 as f64) as f32;
                let (s1, s2) = (buffer[r1], buffer[r2]);
                d[k] = s1 + f * (s2 - s1);
            }
        }

        self.write_index = (start_write_index + frames) % buffer_length.max(1);

        // TailTime: maxDelayTime.
        self.silent_frames = if silent {
            self.silent_frames + frames as f64
        } else {
            0.0
        };
        self.silent_frames <= buffer_length as f64
    }
}
