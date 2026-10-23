//! Blink's AudioParamHandler event timeline (`modules/webaudio/audio_param_handler.cc`).
//!
//! `Timeline` is the part that decides when a node takes its sample-accurate path.
//! `audioParam .value = v` is `setValueAtTime (v, currentTime)` in Chromium. While such an event
//! is not in the past (and while an older event is still kept), HasSampleAccurateValues is true
//! and the oscillator, biquad and delay kernels use their a-rate code, whose rounding differs
//! from the k-rate code. `web-audio-api` hands a constant value as a k-rate value, so the
//! processors keep this timeline of setValue times next to it.
//!
//!
//! `Events` also computes the values, for setValue and exponential ramp events: the gain of
//! X_ITE's fade out, and the panner and listener positions.
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc.

#[derive(Default, Clone, Debug)]
pub struct Timeline {
    /// The times of the kSetValue events, in order.
    times: Vec<f64>,
}

impl Timeline {
    /// IsEventCurrent for kSetValue events.
    fn is_current(&self, i: usize, frame: f64, sample_rate: f64) -> bool {
        let Some(next) = self.times.get(i + 1) else {
            return true;
        };

        if next * sample_rate >= frame {
            return true;
        }

        let event_frame = self.times[i] * sample_rate;
        event_frame <= frame && frame < event_frame + 1.0
    }

    /// The leading events that are not current are removed; at least one stays (RemoveOldEvents).
    fn remove_old_events(&mut self, frame: f64, sample_rate: f64) {
        let skipped = (0..self.times.len())
            .take_while(|i| !self.is_current(*i, frame, sample_rate))
            .count();
        let n = self.times.len();

        if n > 1 {
            self.times.drain(..skipped.min(n - 1));
        }
    }

    /// InsertEvent (setValueAtTime), at the render frame `frame`.
    pub fn insert(&mut self, time: f64, frame: u64, sample_rate: f64) {
        self.remove_old_events(frame as f64, sample_rate);
        let index = self.times.partition_point(|t| *t <= time);
        self.times.insert(index, time);
    }

    /// HasSampleAccurateValues (no connections) for the quantum at `frame`.
    pub fn has_sample_accurate_values(&self, frame: u64, sample_rate: f64, quantum: usize) -> bool {
        let Some(first) = self.times.first() else {
            return false;
        };
        let frame = frame as f64;

        if *first > (frame + quantum as f64) / sample_rate {
            return false;
        }

        if self.times.len() >= 2 {
            return true;
        }

        !(*first < frame / sample_rate)
    }

    /// The pruning of ValuesForFrameRange when the quantum's value is computed.
    pub fn after_values(&mut self, frame: u64, sample_rate: f64, quantum: usize) {
        let Some(last) = self.times.last() else {
            return;
        };
        let frame_f = frame as f64;
        let current_time = frame_f / sample_rate;

        // ValueForContextTime: no timeline value before the first event.
        if current_time < self.times[0] {
            return;
        }

        // HandleAllEventsInThePast.
        if last + 1.5 * quantum as f64 / sample_rate < current_time {
            let n = self.times.len();
            self.times.drain(..n - 1);
            return;
        }

        self.remove_old_events(frame_f, sample_rate);
    }
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Kind {
    SetValue,
    ExponentialRamp,
}

#[derive(Clone, Copy, Debug)]
pub struct Event {
    pub kind: Kind,
    pub value: f32,
    pub time: f64,
}

/// The setValue and exponential ramp events of one AudioParam (ValuesForFrameRangeImpl), for
/// the params whose values the processors compute themselves (gain.rs, panner.rs).
#[derive(Default, Clone, Debug)]
pub struct Events {
    events: Vec<Event>,
}

impl Events {
    fn is_current(&self, i: usize, frame: f64, sample_rate: f64) -> bool {
        let Some(next) = self.events.get(i + 1) else {
            return true;
        };

        if next.time * sample_rate >= frame {
            return true;
        }

        let e = self.events[i];
        let event_frame = e.time * sample_rate;
        e.kind == Kind::SetValue && event_frame <= frame && frame < event_frame + 1.0
    }

    fn remove_old_events(&mut self, count: usize) {
        let n = self.events.len();

        if n > 1 {
            self.events.drain(..count.min(n - 1));
        }
    }

    /// InsertEvent: old events go first, then the event after those at the same time or earlier.
    pub fn insert(&mut self, event: Event, frame: u64, sample_rate: f64) {
        let skipped = (0..self.events.len())
            .take_while(|i| !self.is_current(*i, frame as f64, sample_rate))
            .count();

        if skipped > 0 {
            self.remove_old_events(skipped);
        }

        let index = self.events.partition_point(|e| e.time <= event.time);
        self.events.insert(index, event);
    }

    /// HasSampleAccurateValues (no connections) for the quantum at `frame`.
    pub fn has_sample_accurate_values(&self, frame: u64, sample_rate: f64, quantum: usize) -> bool {
        let Some(first) = self.events.first() else {
            return false;
        };
        let frame = frame as f64;

        if first.time > (frame + quantum as f64) / sample_rate && first.kind == Kind::SetValue {
            return false;
        }

        if self.events.len() >= 2 {
            return true;
        }

        !(first.time < frame / sample_rate)
    }

    #[allow(clippy::too_many_arguments)]
    fn process_exponential_ramp(
        fill_to_frame: usize,
        time1: f64,
        time2: f64,
        value1: f32,
        value2: f32,
        sample_rate: f64,
        values: &mut [f32],
        mut current_frame: u64,
        write_index: usize,
    ) -> (u64, f32, usize) {
        if value1 * value2 <= 0.0 || time1 >= time2 {
            values[write_index..fill_to_frame].fill(value1);
            return (current_frame, value1, fill_to_frame);
        }

        let delta_time = time2 - time1;
        let num_sample_frames = delta_time * sample_rate;
        let multiplier = ((value2 / value1) as f64).powf(1.0 / num_sample_frames);
        let mut value = (value1 as f64
            * (value2 as f64 / value1 as f64)
                .powf((current_frame as f64 / sample_rate - time1) / delta_time))
            as f32;
        let mut accumulator = value as f64;

        for v in values[write_index..fill_to_frame].iter_mut() {
            value = accumulator as f32;
            accumulator *= multiplier;
            current_frame += 1;
            *v = value;
        }

        if current_frame as f64 > time2 * sample_rate - 0.5 {
            value = value2;
        }

        (current_frame, value, fill_to_frame)
    }

    /// ValuesForFrameRangeImpl for setValue and exponential ramp events.
    fn values_for_frame_range(
        &mut self,
        start_frame: u64,
        end_frame: u64,
        default_value: f32,
        values: &mut [f32],
        sample_rate: f64,
        quantum: usize,
    ) -> f32 {
        if self.events.is_empty() || end_frame as f64 / sample_rate <= self.events[0].time {
            values.fill(default_value);
            return default_value;
        }

        let n = self.events.len();

        // HandleAllEventsInThePast.
        let last = self.events[n - 1];
        if last.time + 1.5 * quantum as f64 / sample_rate < start_frame as f64 / sample_rate {
            values.fill(default_value);
            self.remove_old_events(n);
            return default_value;
        }

        // HandleFirstEvent.
        let mut current_frame = start_frame;
        let mut write_index = 0;
        let first_event_time = self.events[0].time;

        if first_event_time > start_frame as f64 / sample_rate {
            let first_event_frame = (first_event_time * sample_rate).ceil();
            let fill_to_end_frame = if (end_frame as f64) > first_event_frame {
                first_event_frame as u64
            } else {
                end_frame
            };
            let fill_to_frame = ((fill_to_end_frame - start_frame) as usize).min(values.len());
            values[..fill_to_frame].fill(default_value);
            write_index = fill_to_frame;
            current_frame += fill_to_frame as u64;
        }

        let mut value = default_value;
        let mut skipped = 0;

        for i in 0..n {
            if write_index >= values.len() {
                break;
            }

            if !self.is_current(i, current_frame as f64, sample_rate) {
                skipped += 1;
                continue;
            }

            let event = self.events[i];
            let next = self.events.get(i + 1).copied();
            let (value1, time1) = (event.value, event.time);
            let (value2, time2) = next
                .map_or((value1, end_frame as f64 / sample_rate + 1.0), |e| {
                    (e.value, e.time)
                });
            let fill_to_end_frame = if end_frame as f64 > time2 * sample_rate {
                (time2 * sample_rate).ceil() as u64
            } else {
                end_frame
            };
            let fill_to_frame =
                (fill_to_end_frame.saturating_sub(start_frame) as usize).min(values.len());

            if next.is_some_and(|e| e.kind == Kind::ExponentialRamp) {
                (current_frame, value, write_index) = Self::process_exponential_ramp(
                    fill_to_frame,
                    time1,
                    time2,
                    value1,
                    value2,
                    sample_rate,
                    values,
                    current_frame,
                    write_index,
                );
            } else {
                current_frame = fill_to_end_frame;
                value = event.value;
                values[write_index..fill_to_frame.max(write_index)].fill(value);
                write_index = fill_to_frame.max(write_index);
            }
        }

        if skipped > 0 {
            self.remove_old_events(skipped);
        }

        values[write_index..].fill(value);
        values[values.len() - 1]
    }

    /// FinalValue (k-rate) and Value (): the timeline value at `frame`, else the intrinsic value.
    /// The caller stores the result as the new intrinsic value.
    pub fn final_value(&mut self, intrinsic: f32, frame: u64, sample_rate: f64) -> f32 {
        if self
            .events
            .first()
            .is_none_or(|e| (frame as f64 / sample_rate) < e.time)
        {
            return intrinsic;
        }

        let mut one = [0f32];
        self.values_for_frame_range(frame, frame + 1, intrinsic, &mut one, sample_rate, 128)
    }

    /// CalculateTimelineValues (a-rate): returns the last value, the new intrinsic value.
    pub fn timeline_values(
        &mut self,
        intrinsic: f32,
        frame: u64,
        values: &mut [f32],
        sample_rate: f64,
    ) -> f32 {
        self.values_for_frame_range(
            frame,
            frame + values.len() as u64,
            intrinsic,
            values,
            sample_rate,
            128,
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SR: f64 = 48000.0;

    #[test]
    fn set_value_lands_on_the_frame_chromium_computes() {
        // 0.272 * 48000 is a little more than 13056, so the new value starts at the next frame.
        let mut e = Events::default();
        e.insert(
            Event {
                kind: Kind::SetValue,
                value: 1.0,
                time: 0.0,
            },
            0,
            SR,
        );
        e.insert(
            Event {
                kind: Kind::SetValue,
                value: 2.0,
                time: 13056.0 / SR,
            },
            13056,
            SR,
        );
        let mut v = [0f32; 128];
        assert!(e.has_sample_accurate_values(13056, SR, 128));
        let last = e.timeline_values(1.0, 13056, &mut v, SR);
        assert_eq!((v[0], v[1], v[127], last), (1.0, 2.0, 2.0, 2.0));
    }

    #[test]
    fn exponential_ramp_is_chromium_pow_then_double_products() {
        let mut e = Events::default();
        e.insert(
            Event {
                kind: Kind::SetValue,
                value: 0.5,
                time: 0.0,
            },
            0,
            SR,
        );
        e.insert(
            Event {
                kind: Kind::ExponentialRamp,
                value: 1e-3,
                time: 0.03,
            },
            0,
            SR,
        );
        let mut v = [0f32; 128];
        e.timeline_values(0.5, 128, &mut v, SR);

        let multiplier = ((1e-3f32 / 0.5f32) as f64).powf(1.0 / (0.03 * SR));
        let mut accumulator = (0.5f32 as f64
            * (1e-3f32 as f64 / 0.5f32 as f64).powf((128.0 / SR) / 0.03))
            as f32 as f64;

        for x in v {
            assert_eq!(x, accumulator as f32);
            accumulator *= multiplier;
        }
    }

    #[test]
    fn one_old_event_ends_the_sample_accurate_path() {
        let mut t = Timeline::default();
        t.insert(0.1, 4800, SR);
        assert!(t.has_sample_accurate_values(4864, SR, 128) == !(0.1 < 4864.0 / SR));
        assert!(!t.has_sample_accurate_values(9600, SR, 128));
        t.after_values(9600, SR, 128);
        assert!(!t.has_sample_accurate_values(9728, SR, 128));
    }
}
