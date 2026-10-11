//! PositionChaser / OrientationChaser: Components/Followers/X3DChaserNode.js, X3DFollowerNode.js,
//! PositionChaser.js, OrientationChaser.js. The viewers use these as private helper nodes; here
//! they are plain structs driven by the frame clock (`prepare`, the browser's prepareEvents).

use x3d_fields::math::{Rotation4, Vector3};

/// The value type of a chaser (getVector / equals / interpolate / step).
pub trait Chase: Copy {
    fn zero() -> Self;
    fn equals(a: &Self, b: &Self, tolerance: f64) -> bool;
    fn interpolate(source: &Self, destination: &Self, weight: f64) -> Self;
    /// `step (value1, value2, t)` applied to `output`.
    fn step(output: &mut Self, value1: &Self, value2: &Self, t: f64);
}

impl Chase for Vector3 {
    fn zero() -> Self {
        Vector3::ZERO
    }

    fn equals(a: &Self, b: &Self, tolerance: f64) -> bool {
        let mut d = *a;
        d.subtract(b);
        d.norm() < tolerance
    }

    fn interpolate(source: &Self, destination: &Self, weight: f64) -> Self {
        let mut v = *source;
        v.lerp(destination, weight);
        v
    }

    fn step(output: &mut Self, value1: &Self, value2: &Self, t: f64) {
        let mut delta = *value1;
        delta.subtract(value2).multiply(t);
        output.add(&delta);
    }
}

impl Chase for Rotation4 {
    fn zero() -> Self {
        Rotation4::identity()
    }

    fn equals(lhs: &Self, rhs: &Self, tolerance: f64) -> bool {
        let mut a = *lhs;
        a.inverse().mult_right(rhs);
        a.angle().abs() < tolerance
    }

    fn interpolate(source: &Self, destination: &Self, weight: f64) -> Self {
        let mut r = *source;
        r.slerp(destination, weight);
        r
    }

    fn step(output: &mut Self, value1: &Self, value2: &Self, t: f64) {
        let mut delta = *value2;
        delta.inverse().mult_right(value1).mult_left(output);
        output.slerp(&delta, t);
    }
}

const NUM_BUFFERS: usize = 60;
const TOLERANCE: f64 = 1e-8;

pub struct Chaser<T: Chase> {
    pub duration: f64,
    step_time: f64,
    buffer: Vec<T>,
    previous: T,
    buffer_end_time: f64,
    destination: T,
    /// set_destination as last received (`_set_destination .getValue ()`).
    pub set_destination_value: T,
    pub is_active: bool,
    /// The viewer attached its `value_changed` callback (`hasInterest`).
    pub connected: bool,
    pub value: T,
}

impl<T: Chase> Chaser<T> {
    /// initialize (): initialValue = initialDestination = zero.
    pub fn new(duration: f64, now: f64) -> Self {
        Chaser {
            duration,
            step_time: duration / NUM_BUFFERS as f64,
            buffer: vec![T::zero(); NUM_BUFFERS],
            previous: T::zero(),
            buffer_end_time: now,
            destination: T::zero(),
            set_destination_value: T::zero(),
            is_active: false,
            connected: false,
            value: T::zero(),
        }
    }

    /// set_value__: returns the new value_changed.
    pub fn set_value(&mut self, v: T, now: f64) -> T {
        if !self.is_active {
            self.buffer_end_time = now;
        }

        for b in self.buffer.iter_mut() {
            *b = v;
        }

        self.previous = v;
        self.value = v;
        self.is_active = true;
        v
    }

    /// set_destination__.
    pub fn set_destination(&mut self, v: T, now: f64) {
        self.set_destination_value = v;
        self.destination = v;

        if !self.is_active {
            self.buffer_end_time = now;
        }

        self.is_active = true;
    }

    fn step_response(&self, t: f64) -> f64 {
        if t <= 0.0 {
            return 0.0;
        }

        if t >= self.duration {
            return 1.0;
        }

        0.5 - 0.5 * ((t / self.duration) * std::f64::consts::PI).cos()
    }

    fn update_buffer(&mut self, now: f64) -> f64 {
        let n = NUM_BUFFERS;
        let mut fraction = (now - self.buffer_end_time) / self.step_time;

        if fraction >= 1.0 {
            let seconds = fraction.floor();
            fraction -= seconds;
            let s = seconds as usize;

            if s < n {
                self.previous = self.buffer[n - s];

                for i in (s..n).rev() {
                    self.buffer[i] = self.buffer[i - s];
                }

                for i in 0..s {
                    let alpha = i as f64 / seconds;
                    self.buffer[i] = T::interpolate(&self.destination, &self.buffer[s], alpha);
                }
            } else {
                self.previous = if s == n { self.buffer[0] } else { self.destination };

                for b in self.buffer.iter_mut() {
                    *b = self.destination;
                }
            }

            self.buffer_end_time += seconds * self.step_time;
        }

        fraction
    }

    /// prepareEvents: returns value_changed.
    pub fn prepare(&mut self, now: f64) -> T {
        let n = NUM_BUFFERS;
        let fraction = self.update_buffer(now);

        let mut output = T::interpolate(&self.previous, &self.buffer[n - 1], self.step_response((n as f64 - 1.0 + fraction) * self.step_time));

        for i in (0..=n - 2).rev() {
            let r = self.step_response((i as f64 + fraction) * self.step_time);
            T::step(&mut output, &self.buffer[i], &self.buffer[i + 1], r);
        }

        self.value = output;

        if T::equals(&output, &self.destination, TOLERANCE) {
            self.is_active = false;
        }

        output
    }
}
