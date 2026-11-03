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
#[derive(Debug)]
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
    /// The most luma samples (width times height) a frame header may declare. A frame header that declares more is
    /// refused before any picture buffer is allocated, so a damaged or hostile size cannot make the decoder allocate
    /// without bound (the decoder holds up to eight such pictures as references).
    pub max_luma_samples: u64,
}

#[derive(Debug)]
pub struct Av1Error(pub String);

/// The message of the error for a sequence header that declares more luma samples than [`Limits::max_luma_samples`].
const OVERSIZE: &str = "the sequence header declares a picture larger than the limit";

impl Av1Error {
    /// Whether the stream was refused for the size it declares (not because it is damaged).
    pub fn is_oversize(&self) -> bool {
        self.0 == OVERSIZE
    }
}

impl std::fmt::Display for Av1Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

pub struct Av1Decoder {
    max_luma_samples: u64,
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
        // 0 means "no limit" to dav1d, so a limit of 0 is raised to 1 and a limit beyond 32 bits is clamped.
        settings.frame_size_limit =
            u32::try_from(limits.max_luma_samples.max(1)).unwrap_or(u32::MAX);

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
            max_luma_samples: limits.max_luma_samples,
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

            // SAFETY: a zero result initialized the header.
            let parsed = unsafe { parsed.assume_init() };

            // The decoder allocates for the largest frame the sequence can code (a frame header cannot exceed it),
            // so the size is checked here, before anything is allocated. dav1d's own `frame_size_limit` is the
            // second guard, at the frame header.
            if (parsed.max_width.max(0) as u64) * (parsed.max_height.max(0) as u64)
                > self.max_luma_samples
            {
                return Err(Av1Error(OVERSIZE.into()));
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

#[cfg(test)]
pub(crate) mod tests {
    use super::*;

    /// Writes bits most significant first, as the AV1 bitstream does.
    struct Bits(Vec<u8>, u32);

    impl Bits {
        fn put(&mut self, value: u32, count: u32) {
            for k in (0..count).rev() {
                if self.1 % 8 == 0 {
                    self.0.push(0);
                }

                let last = self.0.len() - 1;

                self.0[last] |= (((value >> k) & 1) as u8) << (7 - self.1 % 8);
                self.1 += 1;
            }
        }
    }

    /// A sequence header OBU (a still picture, reduced header, 8 bit 4:2:0) that declares `width` by `height` (at
    /// most 32768 each). It is a valid header, so only its size can be a reason to refuse it.
    pub(crate) fn sequence_header_obu(width: u32, height: u32) -> Vec<u8> {
        let mut b = Bits(vec![], 0);

        b.put(0, 3); // seq_profile
        b.put(1, 1); // still_picture
        b.put(1, 1); // reduced_still_picture_header
        b.put(0, 5); // seq_level_idx
        b.put(14, 4); // frame_width_bits_minus_1
        b.put(14, 4); // frame_height_bits_minus_1
        b.put(width - 1, 15);
        b.put(height - 1, 15);
        b.put(0, 3); // use_128x128_superblock, enable_filter_intra, enable_intra_edge_filter
        b.put(0, 3); // enable_superres, enable_cdef, enable_restoration
        b.put(0, 1); // high_bitdepth
        b.put(0, 1); // mono_chrome
        b.put(0, 1); // color_description_present_flag
        b.put(0, 1); // color_range
        b.put(0, 2); // chroma_sample_position
        b.put(0, 1); // separate_uv_delta_q
        b.put(0, 1); // film_grain_params_present
        b.put(1, 1); // trailing_one_bit
        let payload = b.0;
        let mut obu = vec![(1 << 3) | 2, payload.len() as u8];

        obu.extend_from_slice(&payload);
        obu
    }

    fn decoder(max_luma_samples: u64) -> Av1Decoder {
        Av1Decoder::new(Limits { max_luma_samples }).unwrap()
    }

    #[test]
    fn a_sequence_header_within_the_limit_is_accepted() {
        let mut d = decoder(64 * 48);

        assert!(d.decode(&sequence_header_obu(64, 48)).unwrap().is_empty());
    }

    /// Sizes that need gigabytes (a header can declare up to 65536 by 65536) are refused before any allocation.
    #[test]
    fn a_sequence_header_that_declares_more_than_the_limit_is_refused() {
        for (w, h) in [(65, 48), (64, 49), (32768, 32768), (16384, 16384)] {
            let mut d = decoder(64 * 48);
            let e = d.decode(&sequence_header_obu(w, h)).unwrap_err();

            assert!(e.is_oversize(), "{w}x{h}: {e}");
        }
    }

    /// The limit also reaches dav1d's own `frame_size_limit` (a limit of 0 would mean none).
    #[test]
    fn a_limit_of_zero_still_limits() {
        let mut d = decoder(0);

        assert!(
            d.decode(&sequence_header_obu(2, 2))
                .unwrap_err()
                .is_oversize()
        );
    }

    /// Input that is not a valid unit is an error, never a panic or an abort.
    #[test]
    fn damaged_units_are_errors() {
        let good = sequence_header_obu(64, 48);
        let cases: Vec<(&str, Vec<u8>)> = vec![
            (
                "a frame before any sequence header",
                vec![(6 << 3) | 2, 1, 0x80],
            ),
            ("a reserved bit in the OBU header", vec![0x81, 0]),
            ("a cut OBU size", vec![(1 << 3) | 2, 0x80]),
            ("a cut OBU", vec![(1 << 3) | 2, 9, 1, 2]),
            (
                "an OBU size that is too long",
                vec![(1 << 3) | 2, 0xFF, 0xFF, 0xFF, 0xFF, 0xFF, 0x7F],
            ),
            (
                "a sequence header that does not parse",
                vec![(1 << 3) | 2, 1, 0xFF],
            ),
            (
                "a truncated sequence header",
                good[..good.len() - 3].to_vec(),
            ),
        ];

        for (name, chunk) in cases {
            let mut d = decoder(64 * 48);

            assert!(d.decode(&chunk).is_err(), "{name}");
        }
    }

    #[test]
    fn empty_and_random_input_never_panics() {
        let mut d = decoder(64 * 48);

        assert!(d.decode(&[]).unwrap().is_empty());
        assert!(d.finish().unwrap().is_empty());

        let mut state = 0x1234_5678_u32;

        for _ in 0..2000 {
            let n = state as usize % 40;
            let chunk: Vec<u8> = (0..n)
                .map(|_| {
                    state = state.wrapping_mul(1_664_525).wrapping_add(1_013_904_223);
                    (state >> 24) as u8
                })
                .collect();

            // Only that it returns: random bytes that pass the framing are valid OBU sequences, which rav1d handles.
            let _ = decoder(64 * 48).decode(&chunk);
        }
    }
}
