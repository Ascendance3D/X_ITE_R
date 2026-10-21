//! AudioBufferSourceNode with Chromium's playback.
//!
//! `web-audio-api` reads the buffer with its own interpolation, loop and grain rules; at a
//! playback rate other than 1 the output differs from Chromium's by up to the full amplitude.
//! This AudioWorklet processor follows Blink's `modules/webaudio/audio_buffer_source_handler.cc`
//! and the scheduling of `audio_scheduled_source_handler.cc`.
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc.

use std::any::Any;
use std::sync::Arc;
use web_audio_api::worklet::{AudioParamValues, AudioWorkletGlobalScope, AudioWorkletProcessor};

const DEFAULT_GRAIN_DURATION: f64 = 0.020;
const MAX_RATE: f64 = 1024.0;

/// The buffer's channels (SharedAudioBuffer).
pub struct Buffer {
    pub channels: Vec<Vec<f32>>,
    pub sample_rate: f32,
}

impl Buffer {
    fn length(&self) -> usize {
        self.channels.first().map_or(0, |c| c.len())
    }

    fn duration(&self) -> f64 {
        self.length() as f64 / self.sample_rate as f64
    }
}

pub enum Message {
    /// `start (when, offset)` with `when` already max (when, currentTime), and whether
    /// playbackRate is 1 and detune 0 (ClampGrainParameters reads their values).
    Start { when: f64, offset: f64, unit_rate: bool },
    Stop(f64),
    Loop(bool),
    LoopStart(f64),
    LoopEnd(f64),
    /// Whether the render graph pulls the node (AudioEngine::update_pull).
    Pulled(bool),
}

#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    Unscheduled,
    Scheduled,
    Playing,
    Finished,
}

/// audio_utilities::TimeToSampleFrame.
fn time_to_sample_frame(time: f64, sample_rate: f64, round_up: bool) -> f64 {
    let frame = (time * sample_rate * 1024.0).round() / 1024.0;
    if round_up { frame.ceil() } else { frame.round() }
}

fn time_to_floating_sample_frame(time: f64, sample_rate: f64) -> f64 {
    let frame = time * sample_rate;
    let round_frame = frame.round();
    let tolerance = round_frame.max(1.0) * (10.0 * f64::EPSILON);

    if (frame - round_frame).abs() <= tolerance { round_frame } else { frame }
}

pub struct Processor {
    buffer: Option<Arc<Buffer>>,
    is_looping: bool,
    did_set_looping: bool,
    loop_start: f64,
    loop_end: f64,
    virtual_loop_start_frame: f64,
    virtual_loop_end_frame: f64,
    virtual_read_index: f64,
    is_grain: bool,
    grain_offset: f64,
    grain_duration: f64,
    is_duration_given: bool,
    min_playback_rate: f64,
    buffer_played_frames: f64,
    state: State,
    start_time: f64,
    end_time: Option<f64>,
    pulled: bool,
}

impl Processor {
    fn finish(&mut self) {
        self.state = State::Finished;
    }

    fn clamp_grain_parameters(&mut self, unit_rate: bool) {
        let Some(b) = &self.buffer else { return };
        let duration = b.duration();
        self.grain_offset = self.grain_offset.clamp(0.0, duration);
        self.grain_duration = if self.is_duration_given { self.grain_duration.max(0.0) } else { f64::INFINITY };
        self.virtual_read_index = if unit_rate { time_to_sample_frame(self.grain_offset, b.sample_rate as f64, false) } else { self.grain_offset * b.sample_rate as f64 };
    }

    fn update_effective_loop_points(&mut self) {
        let Some(b) = &self.buffer else {
            self.virtual_loop_start_frame = 0.0;
            self.virtual_loop_end_frame = 0.0;
            return;
        };

        let duration = b.duration();
        let rate = b.sample_rate as f64;
        let length = b.length() as f64;
        self.virtual_loop_start_frame = 0.0;
        self.virtual_loop_end_frame = length;

        if !self.is_looping {
            return;
        }

        let start = self.loop_start.clamp(0.0, duration);
        let end = if self.loop_end == 0.0 { duration } else { self.loop_end.clamp(0.0, duration) };

        if start < end {
            if start > 0.0 {
                self.virtual_loop_start_frame = time_to_floating_sample_frame(start, rate);
            }
            if end < duration {
                self.virtual_loop_end_frame = time_to_floating_sample_frame(end, rate);
            }
        }

        if self.virtual_loop_start_frame >= self.virtual_loop_end_frame {
            self.virtual_loop_start_frame = 0.0;
            self.virtual_loop_end_frame = length;
        }
    }

    fn compute_playback_rate(&mut self, playback_rate: f64, detune: f64, context_rate: f64) -> f64 {
        let sample_rate_factor = self.buffer.as_ref().map_or(1.0, |b| b.sample_rate as f64 / context_rate);
        let mut rate = sample_rate_factor * playback_rate;
        rate *= 2f64.powf(detune / 1200.0);
        let rate = rate.clamp(-MAX_RATE, MAX_RATE);

        if rate < self.min_playback_rate {
            self.min_playback_rate = rate;
        }

        rate
    }

    /// UpdateSchedulingInfo: (quantum frame offset, frames to process, start frame offset).
    fn update_scheduling_info(&mut self, frames: usize, current_frame: u64, sample_rate: f64, dest: &mut [&mut [f32]]) -> (usize, usize, f64) {
        let quantum_start_frame = current_frame as f64;
        let quantum_end_frame = quantum_start_frame + frames as f64;
        let start_frame = time_to_sample_frame(self.start_time, sample_rate, true);
        let end_frame = self.end_time.map(|t| time_to_sample_frame(t, sample_rate, true));

        if end_frame.is_some_and(|e| e <= quantum_start_frame) {
            self.finish();
        }

        if matches!(self.state, State::Unscheduled | State::Finished) || start_frame >= quantum_end_frame {
            return (0, 0, 0.0);
        }

        let start_frame_offset = if self.state == State::Scheduled {
            self.state = State::Playing;
            self.start_time * sample_rate - start_frame
        } else {
            0.0
        };

        let quantum_frame_offset = if start_frame > quantum_start_frame { ((start_frame - quantum_start_frame) as usize).min(frames) } else { 0 };
        let mut non_silent = frames - quantum_frame_offset;

        if non_silent == 0 {
            return (quantum_frame_offset, 0, start_frame_offset);
        }

        if let Some(end) = end_frame.filter(|e| *e >= quantum_start_frame && *e <= quantum_end_frame) {
            if end < quantum_end_frame {
                let zero_start = (end - quantum_start_frame) as usize;
                let frames_to_zero = frames - zero_start;
                non_silent = non_silent.saturating_sub(frames_to_zero);

                for d in dest.iter_mut() {
                    d[zero_start..].fill(0.0);
                }
            }

            self.finish();
        }

        (quantum_frame_offset, non_silent, start_frame_offset)
    }

    fn render_silence_and_finish(&mut self, dest: &mut [&mut [f32]], index: usize, frames: usize) {
        for d in dest.iter_mut() {
            d[index..index + frames].fill(0.0);
        }

        self.finish();
    }

    #[allow(clippy::too_many_arguments)]
    fn process_fast_path(&mut self, b: &Buffer, dest: &mut [&mut [f32]], virtual_delta_frames: f64, virtual_end_frame: f64, mut frames_to_process: i64, mut write_index: usize, virtual_read_index: f64) -> (usize, f64) {
        let buffer_length = b.length() as u64;
        let mut read_index = virtual_read_index as u64;
        let delta_frames = virtual_delta_frames as u64;
        let end_frame = virtual_end_frame as u64;

        while frames_to_process > 0 {
            let frames_to_end = end_frame as i64 - read_index as i64;
            let frames_this_time = frames_to_process.min(frames_to_end).max(0) as usize;

            for (c, d) in dest.iter_mut().enumerate() {
                let s = &b.channels[c];
                d[write_index..write_index + frames_this_time].copy_from_slice(&s[read_index as usize..read_index as usize + frames_this_time]);
            }

            write_index += frames_this_time;
            read_index += frames_this_time as u64;
            frames_to_process -= frames_this_time as i64;
            let mut temp_read_index = read_index as f64;

            if delta_frames == 0 {
                self.finish();
                break;
            }

            if temp_read_index >= end_frame as f64 {
                if !self.is_looping {
                    self.render_silence_and_finish(dest, write_index, frames_to_process as usize);
                    read_index = read_index.min(buffer_length);
                    break;
                }

                let overflow = temp_read_index - end_frame as f64;
                temp_read_index = end_frame as f64 - delta_frames as f64 + overflow % delta_frames as f64;
            }

            read_index = temp_read_index as u64;
        }

        (write_index, read_index as f64)
    }

    #[allow(clippy::too_many_arguments)]
    fn process_interpolated_path(&mut self, b: &Buffer, dest: &mut [&mut [f32]], virtual_start_frame: f64, virtual_delta_frames: f64, virtual_end_frame: f64, computed_playback_rate: f64, frames_to_process: i64, mut write_index: usize, mut virtual_read_index: f64) -> (usize, f64) {
        let buffer_length = b.length() as u64;
        let max_index = buffer_length.saturating_sub(1);

        for i in 0..frames_to_process {
            let frames_remaining = (frames_to_process - i - 1) as usize;
            let mut read_index = virtual_read_index as u64;
            let interpolation_factor = virtual_read_index - read_index as f64;
            let mut read_index2 = read_index + 1;

            if self.is_looping && (read_index as f64) < virtual_end_frame && read_index2 as f64 >= virtual_end_frame {
                let wrapped_index = if virtual_delta_frames >= 1.0 {
                    virtual_read_index + 1.0 - virtual_delta_frames
                } else if virtual_delta_frames > 0.0 {
                    virtual_start_frame + (virtual_read_index + 1.0 - virtual_start_frame) % virtual_delta_frames
                } else {
                    virtual_start_frame
                };
                read_index2 = if wrapped_index >= 0.0 { wrapped_index as u64 } else { 0 };
            } else if read_index2 >= buffer_length {
                read_index2 = read_index;
            }

            read_index = read_index.min(max_index);
            read_index2 = read_index2.min(max_index);

            for (c, d) in dest.iter_mut().enumerate() {
                let s = &b.channels[c];
                let sample = if read_index == read_index2 && read_index >= 1 {
                    let sample1 = s[read_index as usize - 1] as f64;
                    let sample2 = s[read_index as usize] as f64;
                    sample2 + (sample2 - sample1) * interpolation_factor
                } else {
                    let sample1 = s[read_index as usize] as f64;
                    let sample2 = s[read_index2 as usize] as f64;
                    (1.0 - interpolation_factor) * sample1 + interpolation_factor * sample2
                };
                // ClampTo<float>.
                d[write_index] = sample.clamp(f32::MIN as f64, f32::MAX as f64) as f32;
            }

            write_index += 1;
            virtual_read_index += computed_playback_rate;

            if virtual_delta_frames <= 0.0 {
                self.finish();
                break;
            }

            if computed_playback_rate >= 0.0 && virtual_read_index >= virtual_end_frame {
                if !self.is_looping {
                    self.render_silence_and_finish(dest, write_index, frames_remaining);
                    virtual_read_index = virtual_read_index.min(buffer_length as f64);
                    break;
                }

                let overflow = virtual_read_index - virtual_end_frame;
                virtual_read_index = virtual_end_frame - virtual_delta_frames + overflow % virtual_delta_frames;
            } else if computed_playback_rate < 0.0 && virtual_read_index < virtual_start_frame {
                if !self.is_looping {
                    self.render_silence_and_finish(dest, write_index, frames_remaining);
                    virtual_read_index = virtual_read_index.max(0.0);
                    break;
                }

                let overflow = virtual_start_frame - virtual_read_index;
                virtual_read_index = virtual_start_frame + virtual_delta_frames - overflow % virtual_delta_frames;
            }
        }

        (write_index, virtual_read_index)
    }

    /// RenderFromBuffer: false zeroes the output.
    fn render_from_buffer(&mut self, b: &Buffer, dest: &mut [&mut [f32]], destination_frame_offset: usize, number_of_frames: usize, start_time_offset: f64, computed_playback_rate: f64) -> bool {
        for d in dest.iter_mut() {
            d[..destination_frame_offset].fill(0.0);
        }

        let mut write_index = destination_frame_offset;
        let buffer_length = b.length() as f64;
        let virtual_start_frame = self.virtual_loop_start_frame;
        let virtual_end_frame = self.virtual_loop_end_frame;
        let virtual_delta_frames = virtual_end_frame - virtual_start_frame;

        if computed_playback_rate.abs() > virtual_delta_frames {
            return false;
        }

        let mut virtual_read_index = self.virtual_read_index;
        let mut frames_to_process = number_of_frames as i64;

        if computed_playback_rate >= 0.0 {
            if self.is_looping && virtual_read_index >= virtual_end_frame {
                virtual_read_index = virtual_start_frame.min(buffer_length - 1.0);
                self.virtual_read_index = virtual_read_index;
            }

            if start_time_offset < 0.0 && computed_playback_rate != 0.0 {
                let skipped_frames = (start_time_offset * computed_playback_rate).abs();
                virtual_read_index += skipped_frames;
                self.buffer_played_frames += skipped_frames;
            }
        } else {
            if self.is_looping && virtual_read_index < virtual_start_frame {
                virtual_read_index = virtual_start_frame;
                self.virtual_read_index = virtual_read_index;
            }

            if start_time_offset < 0.0 {
                let skipped_frames = (start_time_offset * computed_playback_rate).abs();
                virtual_read_index -= skipped_frames;
                self.buffer_played_frames += skipped_frames;
            }

            while frames_to_process > 0 && virtual_read_index >= buffer_length {
                for d in dest.iter_mut() {
                    d[write_index] = 0.0;
                }

                write_index += 1;
                virtual_read_index += computed_playback_rate;
                frames_to_process -= 1;
            }
        }

        let mut is_stopping_this_quantum = false;

        if self.is_duration_given {
            let max_source_frames = self.grain_duration * b.sample_rate as f64;
            let source_frames_left = max_source_frames - self.buffer_played_frames;

            if source_frames_left <= 0.0 {
                frames_to_process = 0;
                is_stopping_this_quantum = true;
            } else if computed_playback_rate != 0.0 {
                let frames_until_limit = (source_frames_left / computed_playback_rate.abs()).ceil();

                if frames_until_limit < frames_to_process as f64 {
                    frames_to_process = frames_until_limit as i64;
                    is_stopping_this_quantum = true;
                }
            }
        }

        if !self.is_looping && ((computed_playback_rate >= 0.0 && virtual_read_index >= buffer_length) || (computed_playback_rate < 0.0 && virtual_read_index < 0.0)) {
            virtual_read_index = virtual_read_index.clamp(0.0, buffer_length);
            frames_to_process = 0;
            is_stopping_this_quantum = true;
        }

        let (w, r) = if computed_playback_rate == 1.0 && virtual_read_index == virtual_read_index.floor() && virtual_delta_frames == virtual_delta_frames.floor() && virtual_end_frame == virtual_end_frame.floor() {
            self.process_fast_path(b, dest, virtual_delta_frames, virtual_end_frame, frames_to_process, write_index, virtual_read_index)
        } else {
            self.process_interpolated_path(b, dest, virtual_start_frame, virtual_delta_frames, virtual_end_frame, computed_playback_rate, frames_to_process, write_index, virtual_read_index)
        };
        write_index = w;
        virtual_read_index = r;

        if computed_playback_rate != 0.0 {
            let frames_processed = (write_index - destination_frame_offset) as f64;
            self.buffer_played_frames += frames_processed * computed_playback_rate.abs();
        }

        if is_stopping_this_quantum {
            for d in dest.iter_mut() {
                d[write_index..destination_frame_offset + number_of_frames].fill(0.0);
            }

            self.finish();
        }

        self.virtual_read_index = virtual_read_index;
        true
    }

    /// HandleStoppableSourceNode: a finite source finishes once it has surely played out.
    fn handle_stoppable_source_node(&mut self, current_time: f64, sample_rate: f64) {
        if self.end_time.is_some_and(|e| current_time > e) {
            self.finish();
            return;
        }

        let is_finite = !self.did_set_looping || self.is_duration_given;
        let playing_or_scheduled = matches!(self.state, State::Scheduled | State::Playing);

        if let (true, Some(b), true, true) = (is_finite, &self.buffer, playing_or_scheduled, self.min_playback_rate > 0.0) {
            let source_duration = if self.is_duration_given { self.grain_duration } else { b.duration() };
            let stop_time = self.start_time + source_duration / self.min_playback_rate + 256.0 / sample_rate;

            if current_time > stop_time {
                self.finish();
            }
        }
    }
}

impl AudioWorkletProcessor for Processor {
    /// SetBuffer before start, as AudioElement does it.
    type ProcessorOptions = Option<Arc<Buffer>>;

    fn constructor(buffer: Option<Arc<Buffer>>) -> Self {
        let mut p = Processor {
            buffer,
            is_looping: false,
            did_set_looping: false,
            loop_start: 0.0,
            loop_end: 0.0,
            virtual_loop_start_frame: 0.0,
            virtual_loop_end_frame: 0.0,
            virtual_read_index: 0.0,
            is_grain: false,
            grain_offset: 0.0,
            grain_duration: DEFAULT_GRAIN_DURATION,
            is_duration_given: false,
            min_playback_rate: 1.0,
            buffer_played_frames: 0.0,
            state: State::Unscheduled,
            start_time: 0.0,
            end_time: None,
            pulled: false,
        };

        p.update_effective_loop_points();
        p
    }

    fn onmessage(&mut self, msg: &mut dyn Any) {
        let Some(m) = msg.downcast_mut::<Option<Message>>() else { return };

        match m.take() {
            // AudioBufferSourceHandler::StartSource (start (when, offset): no duration given).
            Some(Message::Start { when, offset, unit_rate }) => {
                if self.state != State::Unscheduled {
                    return;
                }

                self.is_duration_given = false;
                self.is_grain = true;
                self.grain_offset = offset;
                self.grain_duration = self.buffer.as_ref().map_or(0.0, |b| b.duration());
                self.start_time = when;
                self.clamp_grain_parameters(unit_rate);
                self.state = State::Scheduled;
            }
            Some(Message::Stop(when)) => {
                if self.state != State::Unscheduled {
                    self.end_time = Some(when.max(0.0));
                }
            }
            Some(Message::Loop(v)) => {
                self.is_looping = v;
                self.did_set_looping |= v;
                self.update_effective_loop_points();
            }
            Some(Message::LoopStart(v)) => {
                self.loop_start = v;
                self.update_effective_loop_points();
            }
            Some(Message::LoopEnd(v)) => {
                self.loop_end = v;
                self.update_effective_loop_points();
            }
            Some(Message::Pulled(v)) => self.pulled = v,
            None => {}
        }
    }

    fn parameter_descriptors() -> Vec<web_audio_api::AudioParamDescriptor> {
        use web_audio_api::{AudioParamDescriptor, AutomationRate};

        vec![
            AudioParamDescriptor { name: "playbackRate".into(), min_value: f32::MIN, max_value: f32::MAX, default_value: 1.0, automation_rate: AutomationRate::K },
            AudioParamDescriptor { name: "detune".into(), min_value: f32::MIN, max_value: f32::MAX, default_value: 0.0, automation_rate: AutomationRate::K },
        ]
    }

    fn process<'a, 'b>(&mut self, _inputs: &'b [&'a [&'a [f32]]], outputs: &'b mut [&'a mut [&'a mut [f32]]], params: AudioParamValues<'b>, scope: &'b AudioWorkletGlobalScope) -> bool {
        let dest = &mut outputs[0];
        let frames = dest.first().map_or(128, |d| d.len());
        let sample_rate = scope.sample_rate as f64;

        // Process. Chromium processes a node only when the graph pulls it: an unpulled source
        // keeps its read position and its state (HandleStoppableSourceNode still runs).
        let rendered = match self.buffer.clone() {
            _ if !self.pulled => false,
            None => {
                if self.state != State::Unscheduled {
                    self.finish();
                }
                false
            }
            Some(b) if b.channels.len() != dest.len() => false,
            Some(b) => {
                let (offset, count, start_time_offset) = self.update_scheduling_info(frames, scope.current_frame, sample_rate, dest);

                if count == 0 {
                    false
                } else {
                    let rate = self.compute_playback_rate(params.get("playbackRate")[0] as f64, params.get("detune")[0] as f64, sample_rate);
                    self.render_from_buffer(&b, dest, offset, count, start_time_offset, rate)
                }
            }
        };

        if !rendered {
            dest.iter_mut().for_each(|d| d.fill(0.0));
        }

        self.handle_stoppable_source_node((scope.current_frame + frames as u64) as f64 / sample_rate, sample_rate);

        // PropagatesSilence once finished; the node can then be freed.
        self.state != State::Finished
    }
}
