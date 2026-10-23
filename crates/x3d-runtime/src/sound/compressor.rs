//! DynamicsCompressorNode with Chromium's algorithm.
//!
//! The Web Audio specification describes the compressor only loosely, and implementations
//! differ: `web-audio-api`'s compressor is a different design and its output differs from
//! Chromium's by more than the signal amplitude on a plain sawtooth. This AudioWorklet
//! processor follows Blink's `platform/audio/dynamics_compressor.cc` (pre-delay, detector,
//! envelope polynomial, knee curve) and `modules/webaudio/dynamics_compressor_handler.cc`
//! (k-rate parameters, two output channels, `reduction` after each render quantum).
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc. and Apple Inc.

use std::any::Any;
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use web_audio_api::AudioParamDescriptor;
use web_audio_api::AutomationRate;
use web_audio_api::worklet::{AudioParamValues, AudioWorkletGlobalScope, AudioWorkletProcessor};

const METERING_RELEASE_TIME_CONSTANT: f32 = 0.325;
const UNINITIALIZED: f32 = -1.0;
const PRE_DELAY: f32 = 0.006;
const RELEASE_ZONE_1: f32 = 0.09;
const RELEASE_ZONE_2: f32 = 0.16;
const RELEASE_ZONE_3: f32 = 0.42;
const RELEASE_ZONE_4: f32 = 0.98;
const A_BASE: f32 = 0.9999999999999998 * RELEASE_ZONE_1
    + 1.843_221_968_432_392_3e-16 * RELEASE_ZONE_2
    - 1.937_339_435_167_642_3e-16 * RELEASE_ZONE_3
    + 8.824_516_011_816_245e-18 * RELEASE_ZONE_4;
const B_BASE: f32 = -1.578_832_035_284_588_8 * RELEASE_ZONE_1
    + 2.330_583_703_207_428_6 * RELEASE_ZONE_2
    - 0.914_119_420_484_042_9 * RELEASE_ZONE_3
    + 0.162_367_752_561_203_2 * RELEASE_ZONE_4;
const C_BASE: f32 = 0.533_414_286_910_642_4 * RELEASE_ZONE_1
    - 1.272_736_789_213_631 * RELEASE_ZONE_2
    + 0.925_885_604_220_751_2 * RELEASE_ZONE_3
    - 0.186_563_101_917_762_26 * RELEASE_ZONE_4;
const D_BASE: f32 = 0.087_834_631_382_072_34 * RELEASE_ZONE_1
    - 0.169_416_296_792_562_2 * RELEASE_ZONE_2
    + 0.085_880_579_515_952_72 * RELEASE_ZONE_3
    - 0.004_298_914_105_462_83 * RELEASE_ZONE_4;
const E_BASE: f32 = -0.042_416_883_008_123_074 * RELEASE_ZONE_1
    + 0.111_569_382_798_760_2 * RELEASE_ZONE_2
    - 0.097_646_763_252_658_72 * RELEASE_ZONE_3
    + 0.028_494_263_462_021_576 * RELEASE_ZONE_4;
const SAT_RELEASE_TIME: f32 = 0.0025;
const MAX_PRE_DELAY_FRAMES: usize = 1024;
const MAX_PRE_DELAY_FRAMES_MASK: usize = MAX_PRE_DELAY_FRAMES - 1;
const DEFAULT_PRE_DELAY_FRAMES: usize = 256;
const PI_OVER_TWO: f32 = std::f32::consts::FRAC_PI_2;
const DIVISION_FRAMES: usize = 32;

fn ensure_finite(x: f32, default_value: f32) -> f32 {
    if x.is_finite() { x } else { default_value }
}

fn decibels_to_linear(decibels: f32) -> f32 {
    10f32.powf(0.05 * decibels)
}

fn linear_to_decibels(linear: f32) -> f32 {
    20.0 * linear.log10()
}

/// Chromium's `fdlibm::powf` and `fdlibm::asinf` (third_party/fdlibm/ieee754.cc): the double
/// function, rounded to float.
fn fdlibm_powf(x: f32, y: f32) -> f32 {
    (x as f64).powf(y as f64) as f32
}

fn fdlibm_asinf(x: f32) -> f32 {
    (x as f64).asin() as f32
}

fn flush_denormal(x: f32) -> f32 {
    if x.abs() < f32::MIN_POSITIVE { 0.0 } else { x }
}

/// The `reduction` attribute, written by the render thread after each quantum.
#[derive(Default)]
pub struct Reduction(AtomicU32);

impl Reduction {
    pub fn get(&self) -> f32 {
        f32::from_bits(self.0.load(Ordering::Relaxed))
    }

    fn set(&self, value: f32) {
        self.0.store(value.to_bits(), Ordering::Relaxed)
    }
}

pub struct Processor {
    reduction: Arc<Reduction>,
    pre_delay_buffers: [Vec<f32>; 2],
    pre_delay_read_index: usize,
    pre_delay_write_index: usize,
    last_pre_delay_frames: usize,
    detector_average: f32,
    compressor_gain: f32,
    metering_gain: f32,
    metering_release_k: f32,
    db_max_attack_compression_diff: f32,
    ratio: f32,
    slope: f32,
    linear_threshold: f32,
    db_threshold: f32,
    db_knee: f32,
    knee_threshold: f32,
    db_knee_threshold: f32,
    db_yknee_threshold: f32,
    knee: f32,
    silent_frames: f64,
}

impl Processor {
    fn knee_curve(&self, x: f32, k: f32) -> f32 {
        if x < self.linear_threshold {
            return x;
        }

        self.linear_threshold + (1.0 - (-(k * (x - self.linear_threshold)) as f64).exp() as f32) / k
    }

    fn saturate(&self, x: f32, k: f32) -> f32 {
        if x < self.knee_threshold {
            return self.knee_curve(x, k);
        }

        let db_x = linear_to_decibels(x);
        let db_y = self.db_yknee_threshold + self.slope * (db_x - self.db_knee_threshold);
        decibels_to_linear(db_y)
    }

    fn k_at_slope(&self, desired_slope: f32) -> f32 {
        let db_x = self.db_threshold + self.db_knee;
        let x = decibels_to_linear(db_x);
        let mut x2 = 1.0;
        let mut db_x2 = 0.0;

        if !(x < self.linear_threshold) {
            x2 = (x as f64 * 1.001) as f32;
            db_x2 = linear_to_decibels(x2);
        }

        let mut min_k = 0.1f32;
        let mut max_k = 10000f32;
        let mut k = 5f32;
        let mut slope = 1f32;

        for _ in 0..15 {
            if !(x < self.linear_threshold) {
                let db_y = linear_to_decibels(self.knee_curve(x, k));
                let db_y2 = linear_to_decibels(self.knee_curve(x2, k));
                slope = (db_y2 - db_y) / (db_x2 - db_x);
            }

            if slope < desired_slope {
                max_k = k;
            } else {
                min_k = k;
            }

            k = (min_k * max_k).sqrt();
        }

        k
    }

    fn update_static_curve_parameters(
        &mut self,
        db_threshold: f32,
        db_knee: f32,
        ratio: f32,
    ) -> f32 {
        if db_threshold != self.db_threshold || db_knee != self.db_knee || ratio != self.ratio {
            self.db_threshold = db_threshold;
            self.linear_threshold = decibels_to_linear(db_threshold);
            self.db_knee = db_knee;
            self.ratio = ratio;
            self.slope = 1.0 / ratio;
            let k = self.k_at_slope(1.0 / ratio);
            self.db_knee_threshold = db_threshold + db_knee;
            self.knee_threshold = decibels_to_linear(self.db_knee_threshold);
            self.db_yknee_threshold = linear_to_decibels(self.knee_curve(self.knee_threshold, k));
            self.knee = k;
        }

        self.knee
    }

    fn set_pre_delay_time(&mut self, pre_delay_time: f32, sample_rate: f32) {
        let pre_delay_frames =
            ((pre_delay_time * sample_rate) as usize).min(MAX_PRE_DELAY_FRAMES - 1);

        if self.last_pre_delay_frames != pre_delay_frames {
            self.last_pre_delay_frames = pre_delay_frames;

            for b in self.pre_delay_buffers.iter_mut() {
                b.fill(0.0);
            }

            self.pre_delay_read_index = 0;
            self.pre_delay_write_index = pre_delay_frames;
        }
    }
}

impl AudioWorkletProcessor for Processor {
    type ProcessorOptions = Arc<Reduction>;

    fn constructor(reduction: Arc<Reduction>) -> Self {
        Processor {
            reduction,
            pre_delay_buffers: [
                vec![0.0; MAX_PRE_DELAY_FRAMES],
                vec![0.0; MAX_PRE_DELAY_FRAMES],
            ],
            pre_delay_read_index: 0,
            pre_delay_write_index: DEFAULT_PRE_DELAY_FRAMES,
            last_pre_delay_frames: DEFAULT_PRE_DELAY_FRAMES,
            detector_average: 0.0,
            compressor_gain: 1.0,
            metering_gain: 1.0,
            metering_release_k: 0.0,
            db_max_attack_compression_diff: -1.0,
            ratio: UNINITIALIZED,
            slope: UNINITIALIZED,
            linear_threshold: UNINITIALIZED,
            db_threshold: UNINITIALIZED,
            db_knee: UNINITIALIZED,
            knee_threshold: UNINITIALIZED,
            db_knee_threshold: UNINITIALIZED,
            db_yknee_threshold: UNINITIALIZED,
            knee: UNINITIALIZED,
            silent_frames: 0.0,
        }
    }

    fn parameter_descriptors() -> Vec<AudioParamDescriptor> {
        let p = |name: &str, default_value, min_value, max_value| AudioParamDescriptor {
            name: name.into(),
            automation_rate: AutomationRate::K,
            default_value,
            min_value,
            max_value,
        };

        vec![
            p("threshold", -24.0, -100.0, 0.0),
            p("knee", 30.0, 0.0, 40.0),
            p("ratio", 12.0, 1.0, 20.0),
            p("attack", 0.003, 0.0, 1.0),
            p("release", 0.25, 0.0, 1.0),
        ]
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

        if self.metering_release_k == 0.0 {
            self.metering_release_k = (1.0
                - (-1.0 / (sample_rate as f64 * METERING_RELEASE_TIME_CONSTANT as f64)).exp())
                as f32;
        }

        let db_threshold = params.get("threshold")[0];
        let db_knee = params.get("knee")[0];
        let ratio = params.get("ratio")[0];
        let attack_time = params.get("attack")[0];
        let release_time = params.get("release")[0];
        let source = inputs[0];
        let output = &mut outputs[0];
        let frames_to_process = output[0].len();

        let k = self.update_static_curve_parameters(db_threshold, db_knee, ratio);
        let linear_post_gain = fdlibm_powf(1.0 / self.saturate(1.0, k), 0.6);
        let attack_frames = attack_time.max(0.001) * sample_rate;
        let release_frames = sample_rate * release_time;
        let sat_release_frames = SAT_RELEASE_TIME * sample_rate;
        let a = release_frames * A_BASE;
        let b = release_frames * B_BASE;
        let c = release_frames * C_BASE;
        let d = release_frames * D_BASE;
        let e = release_frames * E_BASE;

        self.set_pre_delay_time(PRE_DELAY, sample_rate);

        let mut frame_index = 0;
        let mut silent = true;

        while frame_index < frames_to_process {
            self.detector_average = ensure_finite(self.detector_average, 1.0);
            let desired_gain = self.detector_average;
            let scaled_desired_gain = fdlibm_asinf(desired_gain) / PI_OVER_TWO;
            let envelope_rate;
            let is_releasing = scaled_desired_gain > self.compressor_gain;
            let mut db_compression_diff = if scaled_desired_gain == 0.0 {
                if is_releasing { -1.0 } else { 1.0 }
            } else {
                linear_to_decibels(self.compressor_gain / scaled_desired_gain)
            };

            if is_releasing {
                self.db_max_attack_compression_diff = -1.0;
                db_compression_diff = ensure_finite(db_compression_diff, -1.0);
                let mut x = db_compression_diff.clamp(-12.0, 0.0);
                x = 0.25 * (x + 12.0);
                let x2 = x * x;
                let x3 = x2 * x;
                let x4 = x2 * x2;
                let calc_release_frames = a + b * x + c * x2 + d * x3 + e * x4;
                let db_per_frame = 5.0 / calc_release_frames;
                envelope_rate = decibels_to_linear(db_per_frame);
            } else {
                db_compression_diff = ensure_finite(db_compression_diff, 1.0);

                if self.db_max_attack_compression_diff == -1.0
                    || self.db_max_attack_compression_diff < db_compression_diff
                {
                    self.db_max_attack_compression_diff = db_compression_diff;
                }

                let db_eff_atten_diff = self.db_max_attack_compression_diff.max(0.5);
                let x = 0.25 / db_eff_atten_diff;
                envelope_rate = 1.0 - fdlibm_powf(x, 1.0 / attack_frames);
            }

            let mut pre_delay_read_index = self.pre_delay_read_index;
            let mut pre_delay_write_index = self.pre_delay_write_index;
            let mut detector_average = self.detector_average;
            let mut compressor_gain = self.compressor_gain;
            let loop_frames = DIVISION_FRAMES.min(frames_to_process - frame_index);

            for _ in 0..loop_frames {
                let mut compressor_input = 0f32;

                for j in 0..2 {
                    let channel = if source.len() > j {
                        source[j]
                    } else {
                        source[0]
                    };
                    let undelayed_source = channel[frame_index];
                    self.pre_delay_buffers[j][pre_delay_write_index] = undelayed_source;
                    let abs_undelayed_source = if undelayed_source > 0.0 {
                        undelayed_source
                    } else {
                        -undelayed_source
                    };

                    if compressor_input < abs_undelayed_source {
                        compressor_input = abs_undelayed_source;
                    }
                }

                if compressor_input != 0.0 {
                    silent = false;
                }

                let abs_input = compressor_input;
                let shaped_input = self.saturate(abs_input, k);
                let attenuation = if abs_input <= 0.0001 {
                    1.0
                } else {
                    shaped_input / abs_input
                };
                let db_attenuation = 2f32.max(-linear_to_decibels(attenuation));
                let db_per_frame = db_attenuation / sat_release_frames;
                let sat_release_rate = decibels_to_linear(db_per_frame) - 1.0;
                let rate = if attenuation > detector_average {
                    sat_release_rate
                } else {
                    1.0
                };
                detector_average += (attenuation - detector_average) * rate;
                detector_average = detector_average.min(1.0);
                detector_average = ensure_finite(detector_average, 1.0);

                if envelope_rate < 1.0 {
                    compressor_gain += (scaled_desired_gain - compressor_gain) * envelope_rate;
                } else {
                    compressor_gain *= envelope_rate;
                    compressor_gain = compressor_gain.min(1.0);
                }

                let post_warp_compressor_gain =
                    ((PI_OVER_TWO * compressor_gain) as f64).sin() as f32;
                let total_gain = linear_post_gain * post_warp_compressor_gain;
                let db_real_gain = linear_to_decibels(post_warp_compressor_gain);

                if db_real_gain < self.metering_gain {
                    self.metering_gain = db_real_gain;
                } else {
                    self.metering_gain +=
                        (db_real_gain - self.metering_gain) * self.metering_release_k;
                }

                for j in 0..2 {
                    output[j][frame_index] =
                        self.pre_delay_buffers[j][pre_delay_read_index] * total_gain;
                }

                frame_index += 1;
                pre_delay_read_index = (pre_delay_read_index + 1) & MAX_PRE_DELAY_FRAMES_MASK;
                pre_delay_write_index = (pre_delay_write_index + 1) & MAX_PRE_DELAY_FRAMES_MASK;
            }

            self.pre_delay_read_index = pre_delay_read_index;
            self.pre_delay_write_index = pre_delay_write_index;
            self.detector_average = flush_denormal(detector_average);
            self.compressor_gain = flush_denormal(compressor_gain);
        }

        self.reduction.set(self.metering_gain);

        // TailTime: 5 · 0.325 s after the input goes silent.
        self.silent_frames = if silent {
            self.silent_frames + frames_to_process as f64
        } else {
            0.0
        };
        self.silent_frames < 5.0 * METERING_RELEASE_TIME_CONSTANT as f64 * sample_rate as f64
    }
}
