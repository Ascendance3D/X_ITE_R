//! GainNode with Chromium's gain AudioParam.
//!
//! X_ITE fades an OscillatorSource out with `setValueAtTime` and `exponentialRampToValueAtTime`
//! on a GainNode. `web-audio-api` computes the ramp sample by sample with its own formula;
//! Chromium (`modules/webaudio/audio_param_handler.cc`, ProcessExponentialRamp) starts each
//! render quantum with `pow` and multiplies in double from there. This AudioWorklet processor
//! follows Blink's `gain_handler.cc`, with the AudioParam timeline of param_timeline.rs.
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc.

use super::param_timeline::{Event, Events};
use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use web_audio_api::worklet::{AudioParamValues, AudioWorkletGlobalScope, AudioWorkletProcessor};

/// The intrinsic value (`gain .value`), written by the render thread.
pub struct Shared(AtomicU32);

impl Shared {
    pub fn new(value: f32) -> Shared {
        Shared(AtomicU32::new(value.to_bits()))
    }

    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    /// AudioParamHandler::SetValue (the range of gain is the whole float range).
    pub fn set(&self, value: f32) {
        self.0
            .store(value.clamp(f32::MIN, f32::MAX).to_bits(), Ordering::Relaxed)
    }
}

pub enum Message {
    Event(Event),
}

pub struct Processor {
    shared: Arc<Shared>,
    timeline: Events,
    pending: Vec<Event>,
    values: Vec<f32>,
    /// AudioHandler::last_non_silent_time_, in seconds.
    last_non_silent_time: f64,
}

impl Processor {
    /// CalculateTimelineValues: sample-accurate values, the last one becomes the value.
    fn timeline_values(&mut self, frame: u64, frames: usize, sample_rate: f64) {
        let intrinsic = self.shared.get();
        let mut values = std::mem::take(&mut self.values);
        values.resize(frames, 0.0);
        let last = self
            .timeline
            .timeline_values(intrinsic, frame, &mut values, sample_rate);
        self.values = values;
        self.shared.set(last);
    }
}

impl AudioWorkletProcessor for Processor {
    type ProcessorOptions = Arc<Shared>;

    fn constructor(shared: Arc<Shared>) -> Self {
        Processor {
            shared,
            timeline: Events::default(),
            pending: vec![],
            values: vec![0.0; 128],
            last_non_silent_time: 0.0,
        }
    }

    fn onmessage(&mut self, msg: &mut dyn Any) {
        if let Some(Some(Message::Event(e))) =
            msg.downcast_mut::<Option<Message>>().map(Option::take)
        {
            self.pending.push(e);
        }
    }

    fn process<'a, 'b>(
        &mut self,
        inputs: &'b [&'a [&'a [f32]]],
        outputs: &'b mut [&'a mut [&'a mut [f32]]],
        _params: AudioParamValues<'b>,
        scope: &'b AudioWorkletGlobalScope,
    ) -> bool {
        let sample_rate = scope.sample_rate as f64;
        let frame = scope.current_frame;

        for e in std::mem::take(&mut self.pending) {
            self.timeline.insert(e, frame, sample_rate);
        }

        let source = inputs[0];
        let dest = &mut outputs[0];
        let frames = dest.first().map_or(128, |d| d.len());
        let silent = source.iter().all(|c| c.iter().all(|x| *x == 0.0));
        let propagates_silence = silent && self.last_non_silent_time < frame as f64 / sample_rate;

        if propagates_silence {
            // ProcessOnlyAudioParams.
            self.timeline_values(frame, frames, sample_rate);
            dest.iter_mut().for_each(|d| d.fill(0.0));
        } else if source.is_empty() {
            dest.iter_mut().for_each(|d| d.fill(0.0));
        } else if self
            .timeline
            .has_sample_accurate_values(frame, sample_rate, frames)
        {
            self.timeline_values(frame, frames, sample_rate);

            for (c, d) in dest.iter_mut().enumerate() {
                match source.get(c).filter(|s| s.len() == d.len()) {
                    Some(s) => (0..frames).for_each(|i| d[i] = s[i] * self.values[i]),
                    None => d.fill(0.0),
                }
            }
        } else {
            // Value (): the timeline value for the quantum's first frame, if there is one.
            let gain = self
                .timeline
                .final_value(self.shared.get(), frame, sample_rate);
            self.shared.set(gain);

            for (c, d) in dest.iter_mut().enumerate() {
                match source.get(c).filter(|s| s.len() == d.len()) {
                    Some(s) if gain == 1.0 => d.copy_from_slice(s),
                    Some(s) if gain != 0.0 => (0..frames).for_each(|i| d[i] = s[i] * gain),
                    _ => d.fill(0.0),
                }
            }
        }

        if !silent {
            self.last_non_silent_time = (frame + frames as u64) as f64 / sample_rate;
        }

        !propagates_silence
    }
}
