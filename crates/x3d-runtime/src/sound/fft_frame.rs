//! Blink's FFTFrame and convolvers. FFTFrame (`platform/audio/fft_frame.cc`) on its Rust back end
//! (`platform/audio/rustfft_ffi.rs`): a real FFT of even size N computed as a complex `rustfft`
//! FFT of size N/2 in single precision, with DC and Nyquist packed into `real[0]` and `imag[0]`.
//! The same crate version and features as Chromium 151 give the same bits.
//!
//! Chromium is BSD-3-Clause, Copyright Google Inc.

use rustfft::num_complex::Complex;
use rustfft::{Fft, FftPlanner};
use std::sync::Arc;

pub struct FftFrame {
    size: usize,
    half_size: usize,
    limit: usize,
    forward: Arc<dyn Fft<f32>>,
    inverse: Arc<dyn Fft<f32>>,
    twiddles: Vec<Complex<f32>>,
    complex_data: Vec<Complex<f32>>,
    scratch: Vec<Complex<f32>>,
    scale: f32,
    pub real: Vec<f32>,
    pub imag: Vec<f32>,
}

impl FftFrame {
    /// rustfft_new for an even size.
    pub fn new(size: usize) -> FftFrame {
        assert!(size % 2 == 0);

        let half_size = size / 2;
        let mut planner = FftPlanner::new();
        let forward = planner.plan_fft_forward(half_size);
        let inverse = planner.plan_fft_inverse(half_size);
        let scratch_len = forward.get_inplace_scratch_len().max(inverse.get_inplace_scratch_len());
        let twiddles = (0..half_size)
            .map(|k| {
                let angle = -2.0 * std::f64::consts::PI * (k as f64) / (size as f64);
                let (sin, cos) = angle.sin_cos();
                Complex::new((0.5 * cos) as f32, (0.5 * sin) as f32)
            })
            .collect();

        FftFrame {
            size,
            half_size,
            limit: (half_size - 1) / 2,
            forward,
            inverse,
            twiddles,
            complex_data: vec![Complex::new(0.0, 0.0); half_size],
            scratch: vec![Complex::new(0.0, 0.0); scratch_len],
            scale: 1.0 / half_size as f32,
            real: vec![0.0; half_size],
            imag: vec![0.0; half_size],
        }
    }

    pub fn size(&self) -> usize {
        self.size
    }

    /// do_fft_even and reconstruct_real_fft_even.
    pub fn do_fft(&mut self, data: &[f32]) {
        let (h, z) = (self.half_size, &mut self.complex_data);

        for (k, c) in z.iter_mut().enumerate() {
            *c = Complex::new(data[2 * k], data[2 * k + 1]);
        }

        self.forward.process_with_scratch(z, &mut self.scratch);

        let (real, imag, twiddles) = (&mut self.real, &mut self.imag, &self.twiddles);
        real[0] = z[0].re + z[0].im;
        imag[0] = z[0].re - z[0].im;

        for k in 1..=self.limit {
            let z_k = z[k];
            let z_mk = z[h - k];
            let f_even = Complex::new(0.5 * (z_k.re + z_mk.re), 0.5 * (z_k.im - z_mk.im));
            let f_odd_unscaled = Complex::new(z_k.im + z_mk.im, -(z_k.re - z_mk.re));
            let w_f_odd = twiddles[k] * f_odd_unscaled;
            let x_k = f_even + w_f_odd;
            let x_mk_conj = f_even - w_f_odd;
            real[k] = x_k.re;
            imag[k] = x_k.im;
            real[h - k] = x_mk_conj.re;
            imag[h - k] = -x_mk_conj.im;
        }

        if h % 2 == 0 {
            let k = h / 2;
            real[k] = z[k].re;
            imag[k] = -z[k].im;
        }
    }

    /// DoPaddedFFT: the FFT of `data` padded with zeros to the frame size.
    pub fn do_padded_fft(&mut self, data: &[f32]) {
        let mut padded = vec![0f32; self.size];
        padded[..data.len()].copy_from_slice(data);
        self.do_fft(&padded);
    }

    /// prepare_inverse_fft_even and do_inverse_fft_even.
    pub fn do_inverse_fft(&mut self, data: &mut [f32]) {
        let (h, z) = (self.half_size, &mut self.complex_data);
        let (real, imag, twiddles) = (&self.real, &self.imag, &self.twiddles);
        z[0] = Complex::new(0.5 * (real[0] + imag[0]), 0.5 * (real[0] - imag[0]));

        for k in 1..=self.limit {
            let x_k = Complex::new(real[k], imag[k]);
            let x_mk_conj = Complex::new(real[h - k], -imag[h - k]);
            let f_even = (x_k + x_mk_conj) * 0.5;
            let x_diff = x_k - x_mk_conj;
            let f_odd = twiddles[k].conj() * x_diff;
            let i_f_odd = Complex::new(-f_odd.im, f_odd.re);
            z[k] = f_even + i_f_odd;
            z[h - k] = (f_even - i_f_odd).conj();
        }

        if h % 2 == 0 {
            let k = h / 2;
            z[k] = Complex::new(real[k], -imag[k]);
        }

        self.inverse.process_with_scratch(z, &mut self.scratch);

        for (k, c) in z.iter().enumerate() {
            data[2 * k] = c.re * self.scale;
            data[2 * k + 1] = c.im * self.scale;
        }
    }

    /// FFTFrame::ScaleFFT.
    pub fn scale_fft(&mut self, factor: f32) {
        for x in self.real.iter_mut().chain(self.imag.iter_mut()) {
            *x *= factor;
        }
    }

    /// FFTFrame::Multiply (vector_math::Zvmul, then the packed DC and Nyquist terms).
    pub fn multiply(&mut self, other: &FftFrame) {
        let real0 = self.real[0];
        let imag0 = self.imag[0];

        for k in 0..self.half_size {
            let (r1, i1, r2, i2) = (self.real[k], self.imag[k], other.real[k], other.imag[k]);
            self.real[k] = r1 * r2 - i1 * i2;
            self.imag[k] = r1 * i2 + i1 * r2;
        }

        self.real[0] = real0 * other.real[0];
        self.imag[0] = imag0 * other.imag[0];
    }
}

/// A FIR filter with history (DirectConvolver, vector_math::Conv).
pub struct DirectConvolver {
    kernel: Vec<f32>,
    buffer: Vec<f32>,
}

impl DirectConvolver {
    pub fn new(block: usize, kernel: Vec<f32>) -> DirectConvolver {
        let buffer = vec![0.0; kernel.len() - 1 + block];
        DirectConvolver { kernel, buffer }
    }

    pub fn process(&mut self, source: &[f32], dest: &mut [f32]) {
        let n = dest.len();
        let size = self.kernel.len();
        let history = size - 1;
        self.buffer[history..history + n].copy_from_slice(&source[..n]);

        for (i, d) in dest.iter_mut().enumerate() {
            let mut sum = 0f32;

            for j in 0..size {
                sum += self.buffer[i + j] * self.kernel[size - 1 - j];
            }

            *d = sum;
        }

        self.buffer.copy_within(n..n + history, 0);
    }

    pub fn reset(&mut self) {
        self.buffer.fill(0.0);
    }
}

/// SimpleFFTConvolver (`platform/audio/simple_fft_convolver.cc`): overlap-add with an FFT of
/// twice the block size.
pub struct SimpleFftConvolver {
    kernel: FftFrame,
    frame: FftFrame,
    input: Vec<f32>,
    output: Vec<f32>,
    last_overlap: Vec<f32>,
}

impl SimpleFftConvolver {
    pub fn new(input_block_size: usize, convolution_kernel: &[f32]) -> SimpleFftConvolver {
        let mut kernel = FftFrame::new(2 * input_block_size);
        assert!(convolution_kernel.len() <= input_block_size);
        kernel.do_padded_fft(convolution_kernel);

        SimpleFftConvolver { kernel, frame: FftFrame::new(2 * input_block_size), input: vec![0.0; 2 * input_block_size], output: vec![0.0; 2 * input_block_size], last_overlap: vec![0.0; input_block_size] }
    }

    pub fn process(&mut self, source: &[f32], dest: &mut [f32]) {
        let half = self.frame.size() / 2;
        self.input[..half].copy_from_slice(&source[..half]);
        self.frame.do_fft(&self.input);
        self.frame.multiply(&self.kernel);
        self.frame.do_inverse_fft(&mut self.output);

        for i in 0..half {
            dest[i] = self.output[i] + self.last_overlap[i];
        }

        self.last_overlap.copy_from_slice(&self.output[half..]);
    }

    pub fn reset(&mut self) {
        self.last_overlap.fill(0.0);
    }
}

/// FFTConvolver (`platform/audio/fft_convolver.cc`): overlap-add of any number of frames with a
/// kernel given as an FFTFrame of twice the block size.
pub struct FftConvolver {
    frame: FftFrame,
    input: Vec<f32>,
    output: Vec<f32>,
    last_overlap: Vec<f32>,
    read_write_index: usize,
}

impl FftConvolver {
    pub fn new(fft_size: usize) -> FftConvolver {
        FftConvolver { frame: FftFrame::new(fft_size), input: vec![0.0; fft_size], output: vec![0.0; fft_size], last_overlap: vec![0.0; fft_size / 2], read_write_index: 0 }
    }

    pub fn process(&mut self, kernel: &FftFrame, mut source: &[f32], mut dest: &mut [f32]) {
        let half_size = self.frame.size() / 2;

        while !source.is_empty() {
            let n = source.len().min(half_size - self.read_write_index);
            let i = self.read_write_index;
            self.input[i..i + n].copy_from_slice(&source[..n]);
            dest[..n].copy_from_slice(&self.output[i..i + n]);
            source = &source[n..];
            dest = &mut dest[n..];
            self.read_write_index += n;

            if self.read_write_index == half_size {
                self.frame.do_fft(&self.input);
                self.frame.multiply(kernel);
                self.frame.do_inverse_fft(&mut self.output);

                for k in 0..half_size {
                    self.output[k] += self.last_overlap[k];
                }

                self.last_overlap.copy_from_slice(&self.output[half_size..]);
                self.read_write_index = 0;
            }
        }
    }

    pub fn reset(&mut self) {
        self.last_overlap.fill(0.0);
        self.read_write_index = 0;
    }
}
