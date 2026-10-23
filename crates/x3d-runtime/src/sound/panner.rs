//! PannerNode, "equalpower" panning model, with Chromium's arithmetic.
//!
//! Blink computes the source direction with `gfx::Vector3dF` (float vectors, double lengths and
//! cross products), the distance and cone gains in double, and the equal-power gains with
//! fdlibm (`modules/webaudio/panner_handler.cc`, `platform/audio/{equal_power_panner,
//! distance_effect,cone_effect}.cc`, `ui/gfx/geometry/vector3d_f.cc`). `web-audio-api` does the
//! same steps with other precisions and differs by a few float ulps. This AudioWorklet
//! processor follows Blink, with the AudioParam timelines of the positions, the orientations
//! and the listener (param_timeline.rs): a `.value` set lands on the frame Chromium computes
//! for its time, and the sample-accurate and cached paths are taken when Chromium takes them.
//! The engine posts the listener values together with `audioContext .listener`.
//!
//! The "HRTF" model is not here: Blink's HRTF uses the IRCAM composite impulse responses of
//! Chromium's `platform/audio/resources`, which carry no stated license. The engine uses
//! `web-audio-api`'s HRTF PannerNode for it (a measured difference, see the batch report).
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc.

use super::param_timeline::{Event, Events, Kind};
use std::any::Any;
use web_audio_api::worklet::{AudioParamValues, AudioWorkletGlobalScope, AudioWorkletProcessor};

const EPSILON: f64 = 1.0e-6;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum DistanceModel {
    Linear,
    Inverse,
    Exponential,
}

/// The PannerNode attributes (not AudioParams).
#[derive(Clone, Copy, Debug)]
pub struct Attributes {
    pub distance_model: DistanceModel,
    pub ref_distance: f64,
    pub max_distance: f64,
    pub rolloff_factor: f64,
    pub cone_inner_angle: f64,
    pub cone_outer_angle: f64,
    pub cone_outer_gain: f64,
}

impl Default for Attributes {
    fn default() -> Self {
        Attributes {
            distance_model: DistanceModel::Inverse,
            ref_distance: 1.0,
            max_distance: 10000.0,
            rolloff_factor: 1.0,
            cone_inner_angle: 360.0,
            cone_outer_angle: 360.0,
            cone_outer_gain: 0.0,
        }
    }
}

pub enum Message {
    Attributes(Attributes),
    /// `<param> .value = v` at `time`, for the param index (positions, orientations, listener).
    SetValue(usize, f32, f64),
}

/// The names of the panner's own AudioParams, in the order of the param indexes.
pub const PARAM_NAMES: [&str; 6] = [
    "positionX",
    "positionY",
    "positionZ",
    "orientationX",
    "orientationY",
    "orientationZ",
];

#[derive(Clone, Copy, PartialEq)]
struct V3(f32, f32, f32);

impl V3 {
    fn sub(self, o: V3) -> V3 {
        V3(self.0 + -o.0, self.1 + -o.1, self.2 + -o.2)
    }

    fn length_squared(self) -> f64 {
        self.0 as f64 * self.0 as f64
            + self.1 as f64 * self.1 as f64
            + self.2 as f64 * self.2 as f64
    }

    fn length(self) -> f32 {
        self.length_squared().sqrt() as f32
    }

    fn is_zero(self) -> bool {
        self.0 == 0.0 && self.1 == 0.0 && self.2 == 0.0
    }

    /// Vector3dF::GetNormalized: false (and unchanged) for a tiny vector.
    fn normalized(self) -> (bool, V3) {
        let length_squared = self.length_squared();

        if length_squared < EPSILON * EPSILON {
            return (false, self);
        }

        let s = length_squared.sqrt() as f32;
        (true, V3(self.0 / s, self.1 / s, self.2 / s))
    }

    fn cross(self, o: V3) -> V3 {
        let (dx, dy, dz) = (self.0 as f64, self.1 as f64, self.2 as f64);
        V3(
            (dy * o.2 as f64 - dz * o.1 as f64) as f32,
            (dz * o.0 as f64 - dx * o.2 as f64) as f32,
            (dx * o.1 as f64 - dy * o.0 as f64) as f32,
        )
    }

    fn dot(self, o: V3) -> f32 {
        self.0 * o.0 + self.1 * o.1 + self.2 * o.2
    }

    fn scale(self, s: f32) -> V3 {
        V3(self.0 * s, self.1 * s, self.2 * s)
    }
}

/// base::RadToDeg<float>; `acos` of a float is `acosf` in Blink.
fn rad_to_deg(r: f32) -> f32 {
    r * (180.0 / std::f32::consts::PI)
}

/// gfx::AngleBetweenVectorsInDegrees.
fn angle_between_vectors_in_degrees(base: V3, other: V3) -> f32 {
    let c = (base.dot(other) / base.length() / other.length())
        .min(1.0)
        .max(-1.0);
    rad_to_deg(c.acos())
}

fn ensure_finite(x: f64) -> f64 {
    if x.is_finite() { x } else { 0.0 }
}

/// PannerHandler::CalculateAzimuthElevation (the elevation is not used by equal-power panning).
fn azimuth(position: V3, listener_position: V3, listener_forward: V3, listener_up: V3) -> f64 {
    let (ok, source_listener) = position.sub(listener_position).normalized();

    if !ok {
        return 0.0;
    }

    let listener_right = listener_forward.cross(listener_up).normalized().1;
    let listener_forward_norm = listener_forward.normalized().1;
    let up = listener_right.cross(listener_forward_norm);
    let up_projection = source_listener.dot(up);
    let projected_source = source_listener.sub(up.scale(up_projection)).normalized().1;
    let mut azimuth =
        rad_to_deg(projected_source.dot(listener_right).clamp(-1.0, 1.0).acos()) as f64;
    azimuth = ensure_finite(azimuth);

    let front_back = projected_source.dot(listener_forward_norm) as f64;

    if front_back < 0.0 {
        azimuth = 360.0 - azimuth;
    }

    if (0.0..=270.0).contains(&azimuth) {
        90.0 - azimuth
    } else {
        450.0 - azimuth
    }
}

/// DistanceEffect::Gain.
fn distance_gain(a: &Attributes, distance: f64) -> f64 {
    match a.distance_model {
        DistanceModel::Linear => {
            let dref = a.ref_distance.min(a.max_distance);
            let dmax = a.ref_distance.max(a.max_distance);
            let distance = distance.clamp(dref, dmax);

            if dref == dmax {
                return 1.0 - a.rolloff_factor;
            }

            1.0 - a.rolloff_factor.clamp(0.0, 1.0) * (distance - dref) / (dmax - dref)
        }
        DistanceModel::Inverse => {
            if a.ref_distance == 0.0 {
                return 0.0;
            }

            let distance = distance.max(a.ref_distance);
            a.ref_distance
                / (a.ref_distance + a.rolloff_factor.max(0.0) * (distance - a.ref_distance))
        }
        DistanceModel::Exponential => {
            if a.ref_distance == 0.0 {
                return 0.0;
            }

            let distance = distance.max(a.ref_distance);
            (distance / a.ref_distance).powf(-a.rolloff_factor.max(0.0))
        }
    }
}

/// ConeEffect::Gain.
fn cone_gain(
    a: &Attributes,
    source_position: V3,
    source_orientation: V3,
    listener_position: V3,
) -> f64 {
    if source_orientation.is_zero() || (a.cone_inner_angle == 360.0 && a.cone_outer_angle == 360.0)
    {
        return 1.0;
    }

    let source_to_listener = listener_position.sub(source_position);
    let angle = angle_between_vectors_in_degrees(source_to_listener, source_orientation) as f64;
    let abs_angle = angle.abs();
    let abs_inner_angle = a.cone_inner_angle.abs() / 2.0;
    let abs_outer_angle = a.cone_outer_angle.abs() / 2.0;

    if abs_angle <= abs_inner_angle {
        1.0
    } else if abs_angle >= abs_outer_angle {
        a.cone_outer_gain
    } else {
        let x = (abs_angle - abs_inner_angle) / (abs_outer_angle - abs_inner_angle);
        (1.0 - x) + a.cone_outer_gain * x
    }
}

/// PannerHandler::CalculateDistanceConeGain.
fn distance_cone_gain(a: &Attributes, position: V3, orientation: V3, listener_position: V3) -> f32 {
    let listener_distance = position.sub(listener_position).length() as f64;
    (distance_gain(a, listener_distance) * cone_gain(a, position, orientation, listener_position))
        as f32
}

/// EqualPowerPanner::UpdateDesiredGain: (clamped azimuth, gain left, gain right).
fn desired_gain(azimuth: f64, number_of_input_channels: usize) -> (f64, f64, f64) {
    let mut azimuth = azimuth.clamp(-180.0, 180.0);

    if azimuth < -90.0 {
        azimuth = -180.0 - azimuth;
    } else if azimuth > 90.0 {
        azimuth = 180.0 - azimuth;
    }

    let desired_pan_position = if number_of_input_channels == 1 {
        (azimuth + 90.0) / 180.0
    } else if azimuth <= 0.0 {
        (azimuth + 90.0) / 90.0
    } else {
        azimuth / 90.0
    };

    let a = std::f64::consts::FRAC_PI_2 * desired_pan_position;
    (azimuth, a.cos(), a.sin())
}

/// Positions and orientations (0 … 5), then the listener (6 … 14).
const PARAMS: usize = 15;

pub struct Processor {
    attributes: Attributes,
    /// The intrinsic values and the event timelines of the 15 params.
    intrinsic: [f32; PARAMS],
    events: [Events; PARAMS],
    pending: Vec<(usize, f32, f64)>,
    values: Vec<Vec<f32>>,
    /// PannerHandler::last_position_, last_orientation_, the dirty flags and the cached values.
    last_position: V3,
    last_orientation: V3,
    azimuth_dirty: bool,
    gain_dirty: bool,
    cached_azimuth: f64,
    cached_gain: f32,
    /// AudioListenerHandler::last_position_, last_forward_, last_up_ (UpdateState).
    last_listener: [f32; 9],
    last_non_silent_time: f64,
}

impl Processor {
    /// FinalValue (): the timeline value at `frame`, which becomes the intrinsic value.
    fn final_value(&mut self, i: usize, frame: u64, sample_rate: f64) -> f32 {
        let v = self.events[i].final_value(self.intrinsic[i], frame, sample_rate);
        self.intrinsic[i] = v;
        v
    }

    fn final_v3(&mut self, i: usize, frame: u64, sample_rate: f64) -> V3 {
        V3(
            self.final_value(i, frame, sample_rate),
            self.final_value(i + 1, frame, sample_rate),
            self.final_value(i + 2, frame, sample_rate),
        )
    }

    /// CalculateSampleAccurateValues of param `i` into `self.values[i]`.
    fn sample_accurate_values(&mut self, i: usize, frame: u64, frames: usize, sample_rate: f64) {
        let mut values = std::mem::take(&mut self.values[i]);
        values.resize(frames, 0.0);
        self.intrinsic[i] =
            self.events[i].timeline_values(self.intrinsic[i], frame, &mut values, sample_rate);
        self.values[i] = values;
    }
}

impl AudioWorkletProcessor for Processor {
    /// The listener values when the panner is made.
    type ProcessorOptions = [f32; 9];

    fn constructor(listener: [f32; 9]) -> Self {
        let mut intrinsic = [
            0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0,
        ];
        intrinsic[6..].copy_from_slice(&listener);

        Processor {
            attributes: Attributes::default(),
            intrinsic,
            events: Default::default(),
            pending: vec![],
            values: vec![vec![0.0; 128]; PARAMS],
            last_position: V3(0.0, 0.0, 0.0),
            last_orientation: V3(1.0, 0.0, 0.0),
            azimuth_dirty: true,
            gain_dirty: true,
            cached_azimuth: 0.0,
            cached_gain: 1.0,
            last_listener: listener,
            last_non_silent_time: 0.0,
        }
    }

    fn onmessage(&mut self, msg: &mut dyn Any) {
        match msg.downcast_mut::<Option<Message>>().and_then(Option::take) {
            Some(Message::Attributes(a)) => {
                self.attributes = a;
                self.azimuth_dirty = true;
                self.gain_dirty = true;
            }
            // `.value = v`: the intrinsic value now, then setValueAtTime (v, time).
            Some(Message::SetValue(i, v, time)) => {
                self.intrinsic[i] = v;
                self.pending.push((i, v, time));
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
        let sample_rate = scope.sample_rate as f64;
        let frame = scope.current_frame;

        for (i, value, time) in std::mem::take(&mut self.pending) {
            self.events[i].insert(
                Event {
                    kind: Kind::SetValue,
                    value,
                    time,
                },
                frame,
                sample_rate,
            );
        }

        // AudioListenerHandler::UpdateState, before the quantum is rendered.
        let listener: [f32; 9] =
            std::array::from_fn(|k| k + 6).map(|i| self.final_value(i, frame, sample_rate));
        let listener_dirty = listener != self.last_listener;
        self.last_listener = listener;

        let source = inputs[0];
        let dest = &mut outputs[0];
        let frames = dest[0].len();
        let silent = source.iter().all(|c| c.iter().all(|x| *x == 0.0));
        let propagates_silence = silent && self.last_non_silent_time < frame as f64 / sample_rate;

        if !silent {
            self.last_non_silent_time = (frame + frames as u64) as f64 / sample_rate;
        }

        if propagates_silence {
            // ProcessOnlyAudioParams.
            for i in 0..6 {
                self.sample_accurate_values(i, frame, frames, sample_rate);
            }

            dest.iter_mut().for_each(|d| d.fill(0.0));
            return false;
        }

        let number_of_input_channels = source.len().clamp(1, 2);
        let source_l = source.first().copied().filter(|s| s.len() == frames);
        let source_r = if number_of_input_channels > 1 {
            source.get(1).copied().filter(|s| s.len() == frames)
        } else {
            source_l
        };
        let a = self.attributes;
        let (l, r) = dest.split_at_mut(1);
        let (l, r) = (&mut l[0], &mut r[0]);

        let pan = |k: usize, az: f64, gain: f32, l: &mut [f32], r: &mut [f32]| {
            let (azimuth, gain_l, gain_r) = desired_gain(az, number_of_input_channels);
            let input_l = source_l.map_or(0.0, |s| s[k]);
            let input_r = source_r.map_or(0.0, |s| s[k]);

            let (out_l, out_r) = if number_of_input_channels == 1 {
                (
                    (input_l as f64 * gain_l) as f32,
                    (input_l as f64 * gain_r) as f32,
                )
            } else if azimuth <= 0.0 {
                (
                    (input_l as f64 + input_r as f64 * gain_l) as f32,
                    (input_r as f64 * gain_r) as f32,
                )
            } else {
                (
                    (input_l as f64 * gain_l) as f32,
                    (input_r as f64 + input_l as f64 * gain_r) as f32,
                )
            };

            l[k] = out_l * gain;
            r[k] = out_r * gain;
        };

        if self
            .events
            .iter()
            .any(|e| e.has_sample_accurate_values(frame, sample_rate, frames))
        {
            // ProcessSampleAccurateValues, CopyWithSampleAccurateGainValuesFrom.
            for i in 0..PARAMS {
                self.sample_accurate_values(i, frame, frames, sample_rate);
            }

            let v = &self.values;
            let at = |i: usize, k: usize| V3(v[i][k], v[i + 1][k], v[i + 2][k]);
            let (mut az, mut gain) = (0.0, 0.0);

            for k in 0..frames {
                let (position, orientation, listener_position) = (at(0, k), at(3, k), at(6, k));
                az = azimuth(position, listener_position, at(9, k), at(12, k));
                gain = distance_cone_gain(&a, position, orientation, listener_position);
                pan(k, az, gain, l, r);
            }

            self.cached_azimuth = az;
            self.cached_gain = gain;
        } else {
            // UpdateDirtyState, AzimuthElevation, Pan, DistanceConeGain, CopyWithGainFrom.
            let position = self.final_v3(0, frame, sample_rate);
            let orientation = self.final_v3(3, frame, sample_rate);

            if position != self.last_position || orientation != self.last_orientation {
                self.last_position = position;
                self.last_orientation = orientation;
                self.azimuth_dirty = true;
                self.gain_dirty = true;
            }

            let lv = |i: usize| V3(listener[i], listener[i + 1], listener[i + 2]);

            if self.azimuth_dirty || listener_dirty {
                self.cached_azimuth = azimuth(position, lv(0), lv(3), lv(6));
                self.azimuth_dirty = false;
            }

            if self.gain_dirty || listener_dirty {
                self.cached_gain = distance_cone_gain(&a, position, orientation, lv(0));
                self.gain_dirty = false;
            }

            for k in 0..frames {
                pan(k, self.cached_azimuth, self.cached_gain, l, r);
            }
        }

        true
    }
}
