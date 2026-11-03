//! AV1 decoding for MovieTexture, through `rav1d` (BSD-2-Clause, a Rust port of dav1d; owner decision 2026-11-02).
//!
//! `rav1d` only exposes dav1d's C interface (`dav1d_open`, `dav1d_send_data`, `dav1d_get_picture`), so this module is the
//! one place with `unsafe`: a small safe wrapper that owns the decoder context, copies each picture out into plain
//! planes and releases every dav1d object before it returns. One thread (`n_threads = 1`), so the decode is
//! deterministic and needs no thread support (WebAssembly). Output pictures are 8 bit planes: a deeper picture is
//! reduced by its low bits, as a browser does when it uploads the picture to an 8 bit texture.
//!
//! AV1 decoding is normative: every conforming decoder gives the same samples, so the tests compare the planes with
//! those of ffmpeg's decoder (`oracle/tools/gen_movie_codecs.py`).

use rav1d::include::dav1d::common::Dav1dDataProps;
use rav1d::include::dav1d::data::Dav1dData;
use rav1d::include::dav1d::dav1d::{Dav1dContext, Dav1dSettings};
use rav1d::include::dav1d::headers::Dav1dSequenceHeader;
use rav1d::include::dav1d::picture::Dav1dPicture;
use rav1d::src::lib::{
    dav1d_close, dav1d_data_create, dav1d_data_unref, dav1d_default_settings, dav1d_flush,
    dav1d_get_picture, dav1d_open, dav1d_parse_sequence_header, dav1d_picture_unref,
    dav1d_send_data,
};
use std::mem::MaybeUninit;
use std::ptr::NonNull;

/// The error numbers of `Dav1dResult` that are not failures: `EAGAIN` (-11) asks for more data or for a drain.
const EAGAIN: i32 = -11;

/// The most pictures a decoder may hold back: dav1d's own frame delay with one thread is 1.
const MAX_FRAME_DELAY: i32 = 1;

/// A decoded picture: 8 bit planes (chroma planes at the chroma size of `layout`), and what the sequence header
/// says about the color.
pub struct Picture {
    pub width: usize,
    pub height: usize,
    /// 1 = 4:2:0, 2 = 4:2:2, 3 = 4:4:4, 0 = monochrome (the chroma planes are then 128).
    pub layout: u8,
    pub planes: [Vec<u8>; 3],
    /// The matrix coefficients of the sequence header (ISO/IEC 23091-4: 1 = BT.709, 5 or 6 = BT.601, 2 = unspecified).
    pub matrix: u32,
    /// Whether the samples span 0..=255 (the sequence header's `color_range`) instead of 16..=235 / 16..=240.
    pub full_range: bool,
}

/// What a decoder may allocate for one picture.
#[derive(Clone, Copy)]
pub struct Limits {
    pub max_width: u32,
    pub max_height: u32,
}

#[derive(Debug)]
pub struct Av1Error(pub String);

impl std::fmt::Display for Av1Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct Av1Decoder {
    context: Option<Dav1dContext>,
    /// Whether a sequence header that the parser accepts has been seen.
    sequence_header: bool,
}

// SAFETY: the context is only used through `&mut self`, and with one thread the decoder spawns none.
unsafe impl Send for Av1Decoder {}

impl Av1Decoder {
    pub fn new(limits: Limits) -> Result<Self, Av1Error> {
        let mut settings = MaybeUninit::<Dav1dSettings>::uninit();

        // SAFETY: `settings` is valid to write to; dav1d_default_settings initializes all of it.
        let mut settings = unsafe {
            dav1d_default_settings(NonNull::new_unchecked(settings.as_mut_ptr()));
            settings.assume_init()
        };

        settings.n_threads = 1;
        settings.max_frame_delay = MAX_FRAME_DELAY;
        // The film grain is part of the picture the decoder outputs (the spec makes it a normative post-process).
        settings.apply_grain = 1;
        settings.frame_size_limit = limits.max_width.saturating_mul(limits.max_height);

        let mut context = None;
        // SAFETY: both pointers are valid for the call.
        let result = unsafe {
            dav1d_open(
                Some(NonNull::from(&mut context)),
                Some(NonNull::from(&mut settings)),
            )
        };

        if result.0 != 0 || context.is_none() {
            return Err(Av1Error(format!("dav1d_open failed ({})", result.0)));
        }

        Ok(Self {
            context,
            sequence_header: false,
        })
    }

    /// Feeds one temporal unit (a Matroska block, or the configOBUs of the CodecPrivate) and collects the pictures
    /// the decoder has ready, in output order.
    pub fn decode(&mut self, chunk: &[u8]) -> Result<Vec<Picture>, Av1Error> {
        self.check(chunk)?;

        let mut out = vec![];
        let mut data = Dav1dData {
            data: None,
            sz: 0,
            r#ref: None,
            m: Dav1dDataProps::default(),
        };
        // dav1d allocates the buffer (and frees it with the data); the chunk is copied in.
        // SAFETY: `data` is valid to write to.
        let buffer = unsafe { dav1d_data_create(Some(NonNull::from(&mut data)), chunk.len()) };

        if buffer.is_null() {
            return Err(Av1Error("dav1d_data_create failed".into()));
        }

        // SAFETY: `buffer` has room for `chunk.len()` bytes.
        unsafe { std::ptr::copy_nonoverlapping(chunk.as_ptr(), buffer, chunk.len()) };

        let mut failure = None;

        while data.sz > 0 {
            // SAFETY: the context is open; `data` is valid to read and write.
            let r =
                unsafe { dav1d_send_data(self.context.clone(), Some(NonNull::from(&mut data))) };

            if r.0 != 0 && r.0 != EAGAIN {
                failure = Some(Av1Error(format!("dav1d_send_data failed ({})", r.0)));
                break;
            }

            match self.collect(&mut out) {
                Ok(()) => {}
                Err(e) => {
                    failure = Some(e);
                    break;
                }
            }
        }

        // SAFETY: `data` is valid; unref frees what dav1d did not take.
        unsafe { dav1d_data_unref(Some(NonNull::from(&mut data))) };

        match failure {
            Some(e) => Err(e),
            None => {
                self.collect(&mut out)?;
                Ok(out)
            }
        }
    }

    /// Refuses a unit that rav1d is not meant to see: bad OBU framing, a frame before any accepted sequence header,
    /// a sequence header that does not parse. rav1d's C interface cannot unwind, so a panic inside it aborts the
    /// process; the one such path found (a corrupt tile) is fixed in vendor/rav1d, and tests/av1_safety.rs proves in
    /// child processes that damaged files do not crash.
    fn check(&mut self, chunk: &[u8]) -> Result<(), Av1Error> {
        let (mut has_sequence, mut has_frame) = (false, false);
        let mut at = 0;

        while at < chunk.len() {
            let header = chunk[at];
            let kind = (header >> 3) & 15;
            let extension = header & 4 != 0;
            let sized = header & 2 != 0;

            if header & 0x81 != 0 {
                return Err(Av1Error("a damaged OBU header".into()));
            }

            at += 1 + usize::from(extension);

            let size = if sized {
                let (mut value, mut shift) = (0usize, 0);

                loop {
                    let byte = *chunk
                        .get(at)
                        .ok_or_else(|| Av1Error("a cut OBU size".into()))?;

                    at += 1;
                    value |= usize::from(byte & 0x7F) << shift;
                    shift += 7;

                    if byte & 0x80 == 0 {
                        break;
                    }

                    if shift > 28 {
                        return Err(Av1Error("an OBU size that is too long".into()));
                    }
                }

                value
            } else {
                chunk.len().saturating_sub(at)
            };

            if at + size > chunk.len() {
                return Err(Av1Error("a cut OBU".into()));
            }

            match kind {
                1 => has_sequence = true,
                3 | 4 | 6 | 7 => has_frame = true,
                _ => {}
            }

            at += size;
        }

        if has_sequence {
            let mut parsed = MaybeUninit::<Dav1dSequenceHeader>::zeroed();
            // SAFETY: both pointers are valid for the call.
            let r = unsafe {
                dav1d_parse_sequence_header(
                    Some(NonNull::new_unchecked(parsed.as_mut_ptr())),
                    NonNull::new(chunk.as_ptr() as *mut u8),
                    chunk.len(),
                )
            };

            if r.0 != 0 {
                return Err(Av1Error("a sequence header that does not parse".into()));
            }

            self.sequence_header = true;
        }

        if has_frame && !self.sequence_header {
            return Err(Av1Error("a frame before any sequence header".into()));
        }

        Ok(())
    }

    /// Takes the pictures that are ready.
    fn collect(&mut self, out: &mut Vec<Picture>) -> Result<(), Av1Error> {
        loop {
            let mut picture = MaybeUninit::<Dav1dPicture>::zeroed();
            // SAFETY: the context is open; `picture` is valid to write to.
            let r = unsafe {
                dav1d_get_picture(
                    self.context.clone(),
                    Some(NonNull::new_unchecked(picture.as_mut_ptr())),
                )
            };

            if r.0 == EAGAIN {
                return Ok(());
            }

            if r.0 != 0 {
                return Err(Av1Error(format!("dav1d_get_picture failed ({})", r.0)));
            }

            // SAFETY: a zero result initialized the picture.
            let mut picture = unsafe { picture.assume_init() };
            let copied = copy_picture(&picture);

            // SAFETY: the picture came from dav1d_get_picture and is released once.
            unsafe { dav1d_picture_unref(Some(NonNull::from(&mut picture))) };
            out.push(copied?);
        }
    }

    /// Ends the stream: the pictures the decoder still holds.
    pub fn finish(&mut self) -> Result<Vec<Picture>, Av1Error> {
        let mut out = vec![];

        self.collect(&mut out)?;
        Ok(out)
    }

    pub fn flush(&mut self) {
        // SAFETY: the context is open.
        unsafe { dav1d_flush(self.context.clone().expect("open")) };
    }
}

impl Drop for Av1Decoder {
    fn drop(&mut self) {
        // SAFETY: the context came from dav1d_open and is closed once.
        unsafe { dav1d_close(Some(NonNull::from(&mut self.context))) };
    }
}

fn copy_picture(p: &Dav1dPicture) -> Result<Picture, Av1Error> {
    let (w, h) = (p.p.w.max(0) as usize, p.p.h.max(0) as usize);
    let bpc = p.p.bpc;
    // dav1d_pixel_layout: 0 = I400, 1 = I420, 2 = I422, 3 = I444.
    let layout = p.p.layout as u8;
    let (sx, sy) = match layout {
        1 => (true, true),
        2 => (true, false),
        _ => (false, false),
    };
    let (cw, ch) = (
        if sx { w.div_ceil(2) } else { w },
        if sy { h.div_ceil(2) } else { h },
    );

    if w == 0 || h == 0 || !(bpc == 8 || bpc == 10 || bpc == 12) || layout > 3 {
        return Err(Av1Error("an unsupported picture".into()));
    }

    let shift = bpc - 8;
    let read = |index: usize, width: usize, height: usize| -> Result<Vec<u8>, Av1Error> {
        let base = p.data[index].ok_or_else(|| Av1Error("a plane is missing".into()))?;
        let stride = p.stride[usize::from(index > 0)];
        let mut plane = Vec::with_capacity(width * height);

        for row in 0..height {
            // SAFETY: dav1d guarantees `height` rows of at least `width` samples at `stride` bytes from `base`.
            let line = unsafe {
                let start = (base.as_ptr() as *const u8).offset(row as isize * stride);

                if bpc == 8 {
                    std::slice::from_raw_parts(start, width).to_vec()
                } else {
                    std::slice::from_raw_parts(start as *const u16, width)
                        .iter()
                        .map(|&v| (v >> shift).min(255) as u8)
                        .collect()
                }
            };

            plane.extend_from_slice(&line);
        }

        Ok(plane)
    };
    let y = read(0, w, h)?;
    let (u, v) = if layout == 0 {
        (vec![128; cw * ch], vec![128; cw * ch])
    } else {
        (read(1, cw, ch)?, read(2, cw, ch)?)
    };
    let (matrix, full_range) = match p.seq_hdr {
        // SAFETY: the sequence header lives as long as the picture.
        Some(s) => unsafe { (s.as_ref().mtrx as u32, s.as_ref().color_range != 0) },
        None => (2, false),
    };

    Ok(Picture {
        width: w,
        height: h,
        layout,
        planes: [y, u, v],
        matrix,
        full_range,
    })
}

/// The `configOBUs` of an `av1C` CodecPrivate (AV1 in Matroska: a 4 byte header, then OBUs), which hold the sequence
/// header. Empty when the CodecPrivate is shorter than that.
pub fn config_obus(codec_private: &[u8]) -> &[u8] {
    codec_private.get(4..).unwrap_or(&[])
}
