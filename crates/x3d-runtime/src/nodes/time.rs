//! X3DTimeDependentNode.js and TimeSensor.js.
//!
//! With `Runtime::live` (the viewer) TimeSensors tick: set_start adds timePrepareEvents →
//! set_prepare, do_start adds timeEvents → set_time, pause/resume/stop remove or re-add them.
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
        // BufferAudioSource keeps its X3DUrlObject state in State; its time state is here.
        if !matches!(self.nrt_mut(node).state, State::Time(_)) {
            return self.time_states.entry(node).or_default();
        }

        match &mut self.nrt_mut(node).state {
            State::Time(t) => t,
            _ => unreachable!(),
        }
    }

    /// TimeSensor, or a Sound node (X3DSoundSourceNode, X3DSoundProcessingNode).
    fn is_time_sensor(&self, node: NodeId) -> bool {
        self.builtin_name(node) == Some("TimeSensor")
    }

    /// The set_time interest on timeEvents.
    fn time_events_cb(&self, node: NodeId) -> Cb {
        if self.is_time_sensor(node) { Cb::TimeSensorSetTime } else { Cb::Sound(super::sound::SoundCb::Time) }
    }

    /// `this ._loop`: the loop field, or the private `loop` child object of the Sound nodes
    /// without one.
    pub(crate) fn time_loop(&self, node: NodeId) -> bool {
        match self.pre(node, "loop").or_else(|| self.private_field(node, "loop")) {
            Some(k) => matches!(self.value(k), FieldValue::SFBool(true)),
            None => false,
        }
    }

    /// X3DTimeDependentNode.initialize. `enabled_cb`: the set_enabled__ the class has
    /// (X3DSoundProcessingNode overrides it).
    pub(crate) fn time_dependent_initialize(&mut self, node: NodeId, enabled_cb: Cb) {
        self.watch_live(node);

        let initialized = self.private_field(node, "initialized").unwrap();

        self.add_interest(initialized, node, Cb::TimeSetLoop);
        self.interest(node, "enabled", enabled_cb);
        if let Some(k) = self.pre(node, "loop").or_else(|| self.private_field(node, "loop")) {
            self.add_interest(k, node, Cb::TimeSetLoop);
        }
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
    }

    /// X3DTimeDependentNode.resetElapsedTime.
    pub(crate) fn time_reset_elapsed(&mut self, node: NodeId) {
        let now = self.now;
        let s = self.ts(node);
        s.start = now;
        s.pause = now;
        s.pause_interval = 0.0;
    }

    /// X3DTimeDependentNode.getElapsedTime.
    pub(crate) fn time_elapsed_time(&mut self, node: NodeId) -> f64 {
        self.time_elapsed(node)
    }

    /// X3DTimeDependentNode.stop.
    pub(crate) fn time_stop_node(&mut self, node: NodeId) {
        self.time_stop(node);
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

        // this .getLive () .addInterest ("set_live__", this)
        self.watch_live(node);

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
        if self.get_bool(node, "enabled") && self.time_loop(node) {
            let s = self.ts(node).clone();

            if s.stop_time_value <= s.start_time_value && s.start_time_value <= self.now {
                self.time_do_start(node);
            }
        }

        // set_loop (): empty for TimeSensor.
        if !self.is_time_sensor(node) {
            self.sound_set_loop(node);
        }
    }

    pub(crate) fn time_set_start_time(&mut self, node: NodeId) {
        let v = self.get_f64(node, "startTime");
        self.ts(node).start_time_value = v;

        if self.get_bool(node, "enabled") {
            self.remove_timeout(node, "start");

            if v <= self.now {
                self.time_do_start(node);
            } else {
                self.add_timeout(node, "start", v);
            }
        }
    }

    pub(crate) fn time_set_pause_time(&mut self, node: NodeId) {
        let v = self.get_f64(node, "pauseTime");
        self.ts(node).pause_time_value = v;

        if self.get_bool(node, "enabled") {
            let s = self.ts(node).clone();

            self.remove_timeout(node, "pause");

            if s.pause_time_value <= s.resume_time_value {
                return;
            }

            if s.pause_time_value <= self.now {
                self.time_do_pause(node);
            } else {
                let t = s.pause_time_value;
                self.add_timeout(node, "pause", t);
            }
        }
    }

    pub(crate) fn time_set_resume_time(&mut self, node: NodeId) {
        let v = self.get_f64(node, "resumeTime");
        self.ts(node).resume_time_value = v;

        if self.get_bool(node, "enabled") {
            let s = self.ts(node).clone();

            self.remove_timeout(node, "resume");

            if s.resume_time_value <= s.pause_time_value {
                return;
            }

            if s.resume_time_value <= self.now {
                self.time_do_resume(node);
            } else {
                let t = s.resume_time_value;
                self.add_timeout(node, "resume", t);
            }
        }
    }

    pub(crate) fn time_set_stop_time(&mut self, node: NodeId) {
        let v = self.get_f64(node, "stopTime");
        self.ts(node).stop_time_value = v;

        if self.get_bool(node, "enabled") {
            let s = self.ts(node).clone();

            self.remove_timeout(node, "stop");

            if s.stop_time_value <= s.start_time_value {
                return;
            }

            if s.stop_time_value <= self.now {
                self.time_stop(node);
            } else {
                let t = s.stop_time_value;
                self.add_timeout(node, "stop", t);
            }
        }
    }

    fn time_do_start(&mut self, node: NodeId) {
        if self.is_active(node) {
            return;
        }

        if !self.is_time_sensor(node) {
            self.time_reset_elapsed(node);

            // The event order below is very important.
            self.set_pre(node, "isActive", FieldValue::SFBool(true));
            self.sound_set_start(node);

            if self.time_node_live(node) {
                let k = self.private_field(self.browser_node, "timeEvents").unwrap();
                let cb = self.time_events_cb(node);
                self.add_interest(k, node, cb);
            } else {
                self.ts(node).disabled = true;
                self.time_real_pause(node);
            }

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

        if self.time_node_live(node) {
            // getBrowser () .timeEvents () .addInterest ("set_time", this);
            let k = self.private_field(self.browser_node, "timeEvents").unwrap();
            self.add_interest(k, node, Cb::TimeSensorSetTime);
        } else {
            // Not live: disabled = true; real_pause () → set_pause (): no observable events.
            self.ts(node).disabled = true;
            self.time_sensor_remove_prepare(node);
        }
    }

    fn time_do_pause(&mut self, node: NodeId) {
        if self.is_active(node) && !self.is_paused(node) {
            self.set_pre(node, "isPaused", FieldValue::SFBool(true));

            if self.time_node_live(node) {
                self.time_real_pause(node);
            }
        }
    }

    fn time_do_resume(&mut self, node: NodeId) {
        if self.is_active(node) && self.is_paused(node) {
            self.set_pre(node, "isPaused", FieldValue::SFBool(false));

            if self.time_node_live(node) {
                self.time_real_resume(node);
            }
        }
    }

    /// X3DTimeDependentNode.set_live__.
    pub(crate) fn time_set_live(&mut self, node: NodeId, live: bool) {
        let active = self.is_active(node);
        let paused = self.is_paused(node);

        if live {
            if self.ts(node).disabled {
                self.ts(node).disabled = false;

                if active && !paused {
                    self.time_real_resume(node);
                }
            }
        } else if !self.ts(node).disabled && active && !paused {
            self.ts(node).disabled = true;
            self.time_real_pause(node);
        }
    }

    /// X3DTimeDependentNode.real_pause.
    fn time_real_pause(&mut self, node: NodeId) {
        let now = self.now;
        self.ts(node).pause = now;

        // set_pause (): timePrepareEvents .removeInterest ("set_prepare", this).
        if self.is_time_sensor(node) {
            self.time_sensor_remove_prepare(node);
        } else {
            self.sound_set_pause(node);
        }

        let k = self.private_field(self.browser_node, "timeEvents").unwrap();
        let cb = self.time_events_cb(node);
        self.remove_interest(k, node, cb);
    }

    /// X3DTimeDependentNode.real_resume.
    fn time_real_resume(&mut self, node: NodeId) {
        let interval = self.now - self.ts(node).pause;
        self.ts(node).pause_interval += interval;

        if !self.is_time_sensor(node) {
            self.sound_set_resume(node);
            let k = self.private_field(self.browser_node, "timeEvents").unwrap();
            let cb = self.time_events_cb(node);
            self.add_interest(k, node, cb);
            return;
        }

        // TimeSensor.set_resume
        self.time_sensor_add_prepare(node);
        let [_, r1, r2] = self.range(node);
        let fraction = self.ts(node).fraction;
        self.time_sensor_set_range_values(node, fraction, r1, r2, false);

        let k = self.private_field(self.browser_node, "timeEvents").unwrap();
        self.add_interest(k, node, Cb::TimeSensorSetTime);
    }

    fn time_sensor_add_prepare(&mut self, node: NodeId) {
        let k = self.private_field(self.browser_node, "timePrepareEvents").unwrap();
        self.add_interest(k, node, Cb::TimeSensorSetPrepare);
    }

    fn time_sensor_remove_prepare(&mut self, node: NodeId) {
        let k = self.private_field(self.browser_node, "timePrepareEvents").unwrap();
        self.remove_interest(k, node, Cb::TimeSensorSetPrepare);
    }

    // --- timeouts (X3DTimeDependentNode.addTimeout / removeTimeout / processTimeout) ----------

    pub(crate) fn add_timeout(&mut self, node: NodeId, name: &'static str, time: f64) {
        self.remove_timeout(node, name);

        // `setTimeout (fn, (time - currentTime) * 1000)`: a NaN delay is 0, the timer fires at
        // the next timer run (a NaN time is never <= now).
        let time = if time.is_nan() { self.now } else { time };

        // Only a live scene has a frame clock; the oracle's frozen scene never fires them.
        if self.live {
            self.timeouts.push(Timeout { time, node, name });
        }
    }

    pub(crate) fn remove_timeout(&mut self, node: NodeId, name: &str) {
        self.timeouts.retain(|t| !(t.node == node && t.name == name));
    }

    /// Fire the timeouts due at `now` in time order (setTimeout order). processTimeout:
    /// `if (!enabled) return; advanceOnlyTime (); this [callback] ()`.
    pub fn fire_timeouts(&mut self, now: f64) {
        self.font_style_timers();
        loop {
            let node_timeout = self.timeouts.iter().enumerate().filter(|(_, t)| t.time <= now).min_by(|a, b| a.1.time.partial_cmp(&b.1.time).unwrap()).map(|(i, _)| i);

            // The auto refresh timers of EXTERNPROTO declarations and Inlines (setTimeout) run in due order with these.
            let extern_timer = self.extern_next_timer(now).map(|(due, e)| (due, Some(e), 0));
            let inline_timer = self.inline_next_timer(now).map(|(due, n)| (due, None, n));
            let url_timer = [extern_timer, inline_timer].into_iter().flatten().min_by(|a, b| a.0.partial_cmp(&b.0).unwrap());

            // The Scripts' setTimeout / setInterval timers run in due order with the others; the callback sees the browser time of the
            // previous frame (or the due time, if later) and the date of this frame (or the due time).
            if let Some(mut host) = self.script_host.take() {
                let due = host.next_timer(self, now * 1000.0);

                self.script_host = Some(host);

                if let Some(due) = due {
                    let earlier_than_nodes = node_timeout.is_none_or(|i| due < self.timeouts[i].time * 1000.0);
                    let earlier_than_urls = url_timer.is_none_or(|(u, _, _)| due < u);

                    if earlier_than_nodes && earlier_than_urls {
                        self.now = self.timer_prev_time.max(due / 1000.0);
                        self.date_override = Some(self.timer_date.max(due / 1000.0));

                        if let Some(mut host) = self.script_host.take() {
                            host.fire_timer(self);
                            self.script_host = Some(host);
                        }

                        self.date_override = None;
                        self.now = now;
                        continue;
                    }
                }
            }

            if let Some((due, extern_id, inline_node)) = url_timer {
                if node_timeout.is_none_or(|i| due < self.timeouts[i].time * 1000.0) {
                    self.now = now;

                    match extern_id {
                        Some(e) => self.extern_perform_auto_refresh(e),
                        None => self.inline_perform_auto_refresh(inline_node),
                    }

                    continue;
                }
            }

            let Some(i) = node_timeout else {
                break;
            };

            let t = self.timeouts.remove(i);

            if !self.get_bool(t.node, "enabled") {
                continue;
            }

            self.now = now;

            match t.name {
                "start" => self.time_do_start(t.node),
                "pause" => self.time_do_pause(t.node),
                "resume" => self.time_do_resume(t.node),
                "abort" => self.load_sensor_abort(t.node),
                _ => self.time_stop(t.node),
            }

            self.process_events();
        }
    }

    /// X3DTimeDependentNode.stop.
    fn time_stop(&mut self, node: NodeId) {
        if !self.is_active(node) {
            return;
        }

        // set_stop (): removes the prepare interest only.
        if self.is_time_sensor(node) {
            self.time_sensor_remove_prepare(node);
        } else {
            self.sound_set_stop(node);
        }

        if self.is_paused(node) {
            self.set_pre(node, "isPaused", FieldValue::SFBool(false));
        }

        self.set_pre(node, "isActive", FieldValue::SFBool(false));

        let k = self.private_field(self.browser_node, "timeEvents").unwrap();
        let cb = self.time_events_cb(node);
        self.remove_interest(k, node, cb);
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
        self.time_sensor_add_prepare(node);

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

impl Runtime {
    /// X3DTimeDependentNode.getElapsedTime.
    fn time_elapsed(&mut self, node: NodeId) -> f64 {
        let now = self.now;
        let s = self.ts(node);
        now - s.start - s.pause_interval
    }

    /// TimeSensor.set_prepare (timePrepareEvents).
    pub(crate) fn time_sensor_set_prepare(&mut self, node: NodeId) {
        let time = self.now;
        let elapsed = self.time_elapsed(node);

        self.set_pre(node, "time", FieldValue::SFTime(time));
        self.set_pre(node, "elapsedTime", FieldValue::SFTime(elapsed));

        let s = self.ts(node).clone();

        if time - s.cycle < s.interval {
            return;
        }

        self.set_pre(node, "fraction_changed", FieldValue::SFFloat(s.last));

        let count = match self.pre_value(node, "cycleCount") {
            Some(FieldValue::SFInt32(c)) => *c,
            _ => 0,
        };

        self.set_pre(node, "cycleCount", FieldValue::SFInt32(count.wrapping_add(1)));
        self.set_pre(node, "cycleComplete", FieldValue::SFBool(true));
        self.set_pre(node, "cycleCompleteTime", FieldValue::SFTime(time));
    }

    /// TimeSensor.set_time (timeEvents).
    pub(crate) fn time_sensor_set_time(&mut self, node: NodeId) {
        let time = self.now;
        let s = self.ts(node).clone();

        if time - s.cycle >= s.interval {
            if self.get_bool(node, "loop") {
                if s.interval != 0.0 {
                    let cycle = s.cycle + s.interval * ((time - s.cycle) / s.interval).floor();
                    self.ts(node).cycle = cycle;

                    self.set_pre(node, "cycleTime", FieldValue::SFTime(time));

                    let f = self.time_sensor_get_fraction(node, time);
                    self.ts(node).fraction = f;
                    self.set_pre(node, "fraction_changed", FieldValue::SFFloat(f));
                }
            } else {
                self.time_stop(node);
            }
        } else {
            let f = self.time_sensor_get_fraction(node, time);
            self.ts(node).fraction = f;
            self.set_pre(node, "fraction_changed", FieldValue::SFFloat(f));
        }
    }
}
