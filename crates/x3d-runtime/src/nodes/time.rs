//! X3DTimeDependentNode.js and TimeSensor.js.
//!
//! The oracle scene is never live (dump.html sets `update="false"`, so the browser calls
//! endUpdate → setLive (false), and the loaded scene's live state derives from it). So
//! `do_start` always takes the "not live" branch: `isActive` and the set_start () events are
//! sent, then the sensor is paused (real_pause) and never ticks. setTimeout-based start/stop
//! times in the future never fire before the dump.

use super::State;
use crate::*;

#[derive(Debug, Default, Clone)]
pub struct TimeState {
    pub start_time_value: f64,
    pub pause_time_value: f64,
    pub resume_time_value: f64,
    pub stop_time_value: f64,
    pub start: f64,
    pub pause: f64,
    pub pause_interval: f64,
    pub disabled: bool,
    // TimeSensor
    pub cycle: f64,
    pub interval: f64,
    pub fraction: f64,
    pub first: f64,
    pub last: f64,
    pub scale: f64,
    pub offset: f64,
}

/// Algorithm.fract: `value % 1` (sign of the dividend).
pub fn fract(value: f64) -> f64 {
    value % 1.0
}

/// Algorithm.clamp: Math.min (Math.max (value, min), max).
pub fn clamp(value: f64, min: f64, max: f64) -> f64 {
    x3d_fields::jsmath::js_min(x3d_fields::jsmath::js_max(value, min), max)
}

impl Runtime {
    fn ts(&mut self, node: NodeId) -> &mut TimeState {
        match &mut self.nrt_mut(node).state {
            State::Time(t) => t,
            _ => panic!("not a time dependent node"),
        }
    }

    fn range(&self, node: NodeId) -> [f64; 3] {
        let k = self.private_field(node, "range").unwrap();

        match self.value(k) {
            FieldValue::MFFloat(v) => [0, 1, 2].map(|i| v.get(i).copied().unwrap_or(0.0) as f64),
            _ => [0.0, 0.0, 1.0],
        }
    }

    fn set_pre(&mut self, node: NodeId, name: &str, value: FieldValue) {
        if let Some(k) = self.pre(node, name) {
            self.set_value(k, value);
        }
    }

    pub(crate) fn time_sensor_initialize(&mut self, node: NodeId) {
        // X3DSensorNode.initialize: nothing observable. X3DTimeDependentNode.initialize:
        let initialized = self.private_field(node, "initialized").unwrap();

        self.add_interest(initialized, node, Cb::TimeSetLoop);
        self.interest(node, "enabled", Cb::TimeSetEnabled);
        self.interest(node, "loop", Cb::TimeSetLoop);
        self.interest(node, "startTime", Cb::TimeSetStartTime);
        self.interest(node, "pauseTime", Cb::TimeSetPauseTime);
        self.interest(node, "resumeTime", Cb::TimeSetResumeTime);
        self.interest(node, "stopTime", Cb::TimeSetStopTime);

        let (start, pause, resume, stop) = (
            self.get_f64(node, "startTime"),
            self.get_f64(node, "pauseTime"),
            self.get_f64(node, "resumeTime"),
            self.get_f64(node, "stopTime"),
        );

        let s = self.ts(node);
        s.start_time_value = start;
        s.pause_time_value = pause;
        s.resume_time_value = resume;
        s.stop_time_value = stop;

        let now = self.now;
        self.set_value(initialized, FieldValue::SFTime(now));

        // TimeSensor.initialize
        self.interest(node, "cycleInterval", Cb::TimeSensorSetCycleInterval);
        let range = self.private_field(node, "range").unwrap();
        self.add_interest(range, node, Cb::TimeSensorSetRange);
    }

    fn is_active(&self, node: NodeId) -> bool {
        self.get_bool(node, "isActive")
    }

    fn is_paused(&self, node: NodeId) -> bool {
        self.get_bool(node, "isPaused")
    }

    pub(crate) fn time_set_enabled(&mut self, node: NodeId) {
        if self.get_bool(node, "enabled") {
            self.time_set_loop(node);
        } else {
            self.time_stop(node);
        }
    }

    pub(crate) fn time_set_loop(&mut self, node: NodeId) {
        if self.get_bool(node, "enabled") && self.get_bool(node, "loop") {
            let s = self.ts(node).clone();

            if s.stop_time_value <= s.start_time_value && s.start_time_value <= self.now {
                self.time_do_start(node);
            }
        }

        // set_loop (): empty for TimeSensor.
    }

    pub(crate) fn time_set_start_time(&mut self, node: NodeId) {
        let v = self.get_f64(node, "startTime");
        self.ts(node).start_time_value = v;

        if self.get_bool(node, "enabled") && v <= self.now {
            self.time_do_start(node);
        }
        // else addTimeout ("start", …): a future start never fires before the dump.
    }

    pub(crate) fn time_set_pause_time(&mut self, node: NodeId) {
        let v = self.get_f64(node, "pauseTime");
        self.ts(node).pause_time_value = v;

        if self.get_bool(node, "enabled") {
            let s = self.ts(node).clone();

            if s.pause_time_value <= s.resume_time_value {
                return;
            }

            if s.pause_time_value <= self.now {
                self.time_do_pause(node);
            }
        }
    }

    pub(crate) fn time_set_resume_time(&mut self, node: NodeId) {
        let v = self.get_f64(node, "resumeTime");
        self.ts(node).resume_time_value = v;

        if self.get_bool(node, "enabled") {
            let s = self.ts(node).clone();

            if s.resume_time_value <= s.pause_time_value {
                return;
            }

            if s.resume_time_value <= self.now {
                self.time_do_resume(node);
            }
        }
    }

    pub(crate) fn time_set_stop_time(&mut self, node: NodeId) {
        let v = self.get_f64(node, "stopTime");
        self.ts(node).stop_time_value = v;

        if self.get_bool(node, "enabled") {
            let s = self.ts(node).clone();

            if s.stop_time_value <= s.start_time_value {
                return;
            }

            if s.stop_time_value <= self.now {
                self.time_stop(node);
            }
        }
    }

    fn time_do_start(&mut self, node: NodeId) {
        if self.is_active(node) {
            return;
        }

        // resetElapsedTime
        let now = self.now;
        let s = self.ts(node);
        s.start = now;
        s.pause = now;
        s.pause_interval = 0.0;

        // The event order below is very important.
        self.set_pre(node, "isActive", FieldValue::SFBool(true));
        self.time_sensor_set_start(node);

        // Not live: disabled = true; real_pause () → set_pause (): no observable events.
        self.ts(node).disabled = true;
    }

    fn time_do_pause(&mut self, node: NodeId) {
        if self.is_active(node) && !self.is_paused(node) {
            self.set_pre(node, "isPaused", FieldValue::SFBool(true));
            // Not live: no real_pause.
        }
    }

    fn time_do_resume(&mut self, node: NodeId) {
        if self.is_active(node) && self.is_paused(node) {
            self.set_pre(node, "isPaused", FieldValue::SFBool(false));
        }
    }

    /// X3DTimeDependentNode.stop.
    fn time_stop(&mut self, node: NodeId) {
        if !self.is_active(node) {
            return;
        }

        // set_stop (): removes the prepare interest only.
        if self.is_paused(node) {
            self.set_pre(node, "isPaused", FieldValue::SFBool(false));
        }

        self.set_pre(node, "isActive", FieldValue::SFBool(false));
    }

    /// TimeSensor.setRange.
    fn time_sensor_set_range_values(&mut self, node: NodeId, fraction: f64, first: f64, last: f64, offset: bool) {
        let now = self.now;
        let start_time = self.get_f64(node, "startTime");
        let cycle_interval = self.get_f64(node, "cycleInterval");
        let s = self.ts(node);

        s.first = clamp(first, 0.0, 1.0);
        s.last = clamp(last, 0.0, 1.0);
        s.scale = s.last - s.first;
        s.interval = cycle_interval * s.scale;
        s.offset = if offset && s.interval != 0.0 && !s.interval.is_nan() { (now - start_time) / s.interval } else { 0.0 };
        s.fraction = fract(fraction + s.offset);
        s.cycle = now - (s.fraction - s.first) * cycle_interval;
    }

    /// TimeSensor.getFraction.
    fn time_sensor_get_fraction(&mut self, node: NodeId, time: f64) -> f64 {
        let s = self.ts(node);
        let f = if s.interval != 0.0 && !s.interval.is_nan() { fract((time - s.cycle) / s.interval) } else { 0.0 };
        s.first + f * s.scale
    }

    /// TimeSensor.set_start.
    fn time_sensor_set_start(&mut self, node: NodeId) {
        let [r0, r1, r2] = self.range(node);
        self.time_sensor_set_range_values(node, r0, r1, r2, true);

        let time = self.now;
        let fraction = self.ts(node).fraction;

        self.set_pre(node, "time", FieldValue::SFTime(time));
        self.set_pre(node, "elapsedTime", FieldValue::SFTime(0.0));
        self.set_pre(node, "cycleTime", FieldValue::SFTime(time));
        self.set_pre(node, "fraction_changed", FieldValue::SFFloat(fraction));
        self.set_pre(node, "cycleCount", FieldValue::SFInt32(0));
    }

    pub(crate) fn time_sensor_set_cycle_interval(&mut self, node: NodeId) {
        if !self.is_active(node) {
            return;
        }

        let [_, r1, r2] = self.range(node);
        let fraction = self.ts(node).fraction;
        self.time_sensor_set_range_values(node, fraction, r1, r2, false);
    }

    pub(crate) fn time_sensor_set_range(&mut self, node: NodeId) {
        if !self.is_active(node) {
            return;
        }

        let [r0, r1, r2] = self.range(node);
        self.time_sensor_set_range_values(node, r0, r1, r2, false);

        if self.is_paused(node) {
            return;
        }

        let now = self.now;
        let f = self.time_sensor_get_fraction(node, now);
        self.ts(node).fraction = f;
        self.set_pre(node, "fraction_changed", FieldValue::SFFloat(f));
    }
}
