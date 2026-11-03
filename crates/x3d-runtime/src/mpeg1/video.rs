//! An MPEG-1 video (ISO/IEC 11172-2, public identity only; clause numbers pending) decoder for the picture types I, P
//! and B, a safe Rust port of the video part of pl_mpeg (MIT, Dominic Szablewski; pinned commit and notice in
//! docs/mpeg1-implementation.md and THIRD_PARTY_NOTICES.md).
//!
//! Differences from pl_mpeg, all about robustness or completeness:
//!
//! * the inverse DCT is the double-precision definition, rounded (pl_mpeg's integer IDCT with 8-bit constants differed from
//!   ffmpeg by several levels through the prediction chain); the dequantizer divides the magnitude (truncation toward
//!   zero, as the standard says; pl_mpeg's shift rounds negative levels the other way);
//! * the macroblocks at the very end of a stream that has no start code behind its last slice are decoded (pl_mpeg's
//!   test for the end of a slice needs 23 more bits and loses them);
//! * every index is checked; a reference outside the picture, a bad code, a run past the end of a block, a bit read past
//!   the end of the data or a size change returns an [`Mpeg1Error`] instead of reading or writing out of bounds;
//! * a repeated sequence header replaces the quantizer matrices (pl_mpeg reads the first one only);
//! * MPEG-2 video (a sequence extension after the sequence header) is refused;
//! * the last reference picture is output at the end of the stream whatever the last coded picture was (pl_mpeg drops
//!   it when the stream ends on a B picture);
//! * the whole picture is converted, also the last column and row of an odd size.
//!
//! Only 4:2:0 sampling exists in MPEG-1 video. Pictures come out in display order.

use super::Mpeg1Error;
use super::tables::*;

/// Bits from a byte slice, most significant first. Bits past the end read as zero and set `overrun`.
struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    overrun: bool,
}

impl<'a> Bits<'a> {
    fn new(data: &'a [u8]) -> Self {
        Bits {
            data,
            pos: 0,
            overrun: false,
        }
    }

    fn read(&mut self, n: usize) -> u32 {
        let mut v = 0u32;

        for _ in 0..n {
            let byte = self.data.get(self.pos >> 3).copied();

            if byte.is_none() {
                self.overrun = true;
            }

            v = (v << 1) | u32::from(byte.unwrap_or(0) >> (7 - (self.pos & 7)) & 1);
            self.pos += 1;
        }

        v
    }

    fn skip(&mut self, n: usize) {
        self.pos += n;

        if self.pos > self.data.len() * 8 {
            self.overrun = true;
        }
    }

    fn align(&mut self) {
        self.pos = (self.pos + 7) & !7;
    }

    /// Are `n` more bits there?
    fn has(&self, n: usize) -> bool {
        self.pos + n <= self.data.len() * 8
    }

    /// The code of the next start code (`00 00 01 xx`) at a byte boundary, after which the reader stands. `None` at the
    /// end of the data.
    fn next_start_code(&mut self) -> Option<u8> {
        self.align();

        let mut i = self.pos >> 3;

        while i + 3 < self.data.len() {
            if self.data[i] == 0 && self.data[i + 1] == 0 && self.data[i + 2] == 1 {
                self.pos = (i + 4) * 8;

                return Some(self.data[i + 3]);
            }

            i += 1;
        }

        self.pos = self.data.len() * 8;

        None
    }

    /// Is there a non-zero bit in the next `n` bits? Bits past the end of the data count as zero (a stream that ends
    /// inside the last slice, with no start code behind it, still has its last macroblocks decoded).
    fn peek_non_zero(&mut self, n: usize) -> bool {
        let at = self.pos;
        let overrun = self.overrun;
        let left = (self.data.len() * 8).saturating_sub(at).min(n);
        let v = self.read(left);

        self.pos = at;
        self.overrun = overrun;

        v != 0
    }

    fn vlc(&mut self, table: &[Vlc]) -> Result<i32, Mpeg1Error> {
        let mut index = 0usize;

        loop {
            let bit = self.read(1) as usize;
            let &(next, value) = table
                .get(index + bit)
                .ok_or(Mpeg1Error::Malformed("code table"))?;

            if self.overrun {
                return Err(Mpeg1Error::Truncated);
            }

            if next < 0 {
                return Err(Mpeg1Error::Malformed("bit pattern is no code"));
            }

            if next == 0 {
                return Ok(value);
            }

            index = next as usize;
        }
    }
}

/// One picture of the stream, cropped to the coded size: `y` is `width * height`, `cb` and `cr` are
/// `width.div_ceil(2) * height.div_ceil(2)`.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Picture {
    pub y: Vec<u8>,
    pub cb: Vec<u8>,
    pub cr: Vec<u8>,
}

/// Planes at the size of whole macroblocks.
struct Frame {
    y: Vec<u8>,
    cb: Vec<u8>,
    cr: Vec<u8>,
}

#[derive(Clone, Copy, Default)]
struct Motion {
    full_px: bool,
    is_set: bool,
    r_size: i32,
    h: i32,
    v: i32,
}

const TYPE_I: u32 = 1;
const TYPE_P: u32 = 2;
const TYPE_B: u32 = 3;

const START_PICTURE: u8 = 0x00;
const START_SEQUENCE: u8 = 0xB3;
const START_EXTENSION: u8 = 0xB5;
const START_USER_DATA: u8 = 0xB2;

fn is_slice(code: u8) -> bool {
    (0x01..=0xAF).contains(&code)
}

fn clamp(n: i64) -> u8 {
    n.clamp(0, 255) as u8
}

pub struct Decoder<'a> {
    r: Bits<'a>,
    pub width: usize,
    pub height: usize,
    pub framerate: f64,
    mb_width: usize,
    mb_height: usize,
    luma_width: usize,
    chroma_width: usize,
    intra_quant: [u8; 64],
    non_intra_quant: [u8; 64],
    frames: [Frame; 3],
    cur: usize,
    fwd: usize,
    bwd: usize,
    picture_type: u32,
    forward: Motion,
    backward: Motion,
    quantizer_scale: i32,
    slice_begin: bool,
    mb_address: i64,
    mb_row: usize,
    mb_col: usize,
    mb_type: i32,
    mb_intra: bool,
    dc_predictor: [i32; 3],
    block: [i32; 64],
    has_reference: bool,
    /// A start code that ended the previous picture and is not handled yet.
    pending: Option<u8>,
    ended: bool,
}

impl<'a> Decoder<'a> {
    /// Reads the first sequence header of the video elementary stream `data`.
    pub fn new(data: &'a [u8]) -> Result<Self, Mpeg1Error> {
        let mut r = Bits::new(data);

        loop {
            match r.next_start_code() {
                Some(START_SEQUENCE) => break,
                Some(_) => {}
                None => return Err(Mpeg1Error::NotMpeg1),
            }
        }

        let (width, height, framerate, intra_quant, non_intra_quant) =
            Self::sequence_header(&mut r)?;

        // MPEG-2 video has a sequence extension (start code 0xB5, extension id 1) right after the header.
        let mut peek = Bits::new(data);

        peek.pos = r.pos;

        if peek.next_start_code() == Some(START_EXTENSION) && peek.read(4) == 1 {
            return Err(Mpeg1Error::UnsupportedMpeg2);
        }

        let mb_width = width.div_ceil(16);
        let mb_height = height.div_ceil(16);
        let (luma_width, chroma_width) = (mb_width * 16, mb_width * 8);
        let frame = || Frame {
            y: vec![0; luma_width * mb_height * 16],
            cb: vec![128; chroma_width * mb_height * 8],
            cr: vec![128; chroma_width * mb_height * 8],
        };

        Ok(Decoder {
            r,
            width,
            height,
            framerate,
            mb_width,
            mb_height,
            luma_width,
            chroma_width,
            intra_quant,
            non_intra_quant,
            frames: [frame(), frame(), frame()],
            cur: 0,
            fwd: 1,
            bwd: 2,
            picture_type: 0,
            forward: Motion::default(),
            backward: Motion::default(),
            quantizer_scale: 0,
            slice_begin: false,
            mb_address: 0,
            mb_row: 0,
            mb_col: 0,
            mb_type: 0,
            mb_intra: false,
            dc_predictor: [128; 3],
            block: [0; 64],
            has_reference: false,
            pending: None,
            ended: false,
        })
    }

    /// The fields of a sequence header; the reader stands after the start code and ends after the header.
    #[allow(clippy::type_complexity)]
    fn sequence_header(
        r: &mut Bits,
    ) -> Result<(usize, usize, f64, [u8; 64], [u8; 64]), Mpeg1Error> {
        // 64 bits of header and two optional matrices of 64 bytes.
        if !r.has(64) {
            return Err(Mpeg1Error::Truncated);
        }

        let width = r.read(12) as usize;
        let height = r.read(12) as usize;

        if width == 0 || height == 0 {
            return Err(Mpeg1Error::Malformed("picture size zero"));
        }

        r.skip(4); // pel_aspect_ratio
        let rate_code = r.read(4) as usize;
        let framerate = PICTURE_RATE[rate_code];

        if framerate == 0.0 {
            return Err(Mpeg1Error::Malformed("picture rate code"));
        }

        r.skip(18 + 1 + 10 + 1); // bit_rate, marker, vbv_buffer_size, constrained_parameters_flag

        fn matrix(default: &[u8; 64], r: &mut Bits) -> Result<[u8; 64], Mpeg1Error> {
            if r.read(1) == 0 {
                return Ok(*default);
            }

            if !r.has(512) {
                return Err(Mpeg1Error::Truncated);
            }

            let mut m = [0u8; 64];

            for z in ZIG_ZAG {
                m[z as usize] = r.read(8) as u8;
            }

            Ok(m)
        }

        let intra = matrix(&INTRA_QUANT_MATRIX, r)?;
        let non_intra = matrix(&NON_INTRA_QUANT_MATRIX, r)?;

        if r.overrun {
            return Err(Mpeg1Error::Truncated);
        }

        Ok((width, height, framerate, intra, non_intra))
    }

    /// The next picture in display order, or `None` at the end of the stream. After an error the decoder is finished.
    pub fn next_picture(&mut self) -> Result<Option<Picture>, Mpeg1Error> {
        let r = self.next_inner();

        if r.is_err() {
            self.ended = true;
        }

        r
    }

    fn next_inner(&mut self) -> Result<Option<Picture>, Mpeg1Error> {
        if self.ended {
            return Ok(None);
        }

        loop {
            if !self.find_picture()? {
                // The end of the stream: a reference picture still held back is the last one.
                self.ended = true;

                if self.has_reference {
                    self.has_reference = false;

                    return Ok(Some(self.crop(self.bwd)));
                }

                return Ok(None);
            }

            if !self.decode_picture()? {
                continue; // a D picture or a reserved type: nothing to show
            }

            let shown = if self.picture_type == TYPE_B {
                Some(self.cur)
            } else if self.has_reference {
                Some(self.fwd)
            } else {
                self.has_reference = true;

                None
            };

            if let Some(i) = shown {
                return Ok(Some(self.crop(i)));
            }
        }
    }

    fn next_code(&mut self) -> Option<u8> {
        self.pending.take().or_else(|| self.r.next_start_code())
    }

    /// Goes on to the next picture start code, taking sequence headers on the way. False at the end of the data.
    fn find_picture(&mut self) -> Result<bool, Mpeg1Error> {
        while let Some(code) = self.next_code() {
            match code {
                START_PICTURE => return Ok(true),
                START_SEQUENCE => {
                    let (w, h, rate, intra, non_intra) = Self::sequence_header(&mut self.r)?;

                    if w != self.width || h != self.height {
                        return Err(Mpeg1Error::Unsupported("picture size changes"));
                    }

                    self.framerate = rate;
                    self.intra_quant = intra;
                    self.non_intra_quant = non_intra;
                }
                _ => {}
            }
        }

        Ok(false)
    }

    /// The part of the frame at index `i` that is the picture, as a [`Picture`].
    fn crop(&self, i: usize) -> Picture {
        let f = &self.frames[i];
        let (w, h) = (self.width, self.height);
        let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
        let plane = |src: &[u8], stride: usize, w: usize, h: usize| {
            let mut out = Vec::with_capacity(w * h);

            for row in 0..h {
                out.extend_from_slice(&src[row * stride..row * stride + w]);
            }

            out
        };

        Picture {
            y: plane(&f.y, self.luma_width, w, h),
            cb: plane(&f.cb, self.chroma_width, cw, ch),
            cr: plane(&f.cr, self.chroma_width, cw, ch),
        }
    }

    /// Decodes the picture whose start code was just read. False for a picture type that is not shown.
    fn decode_picture(&mut self) -> Result<bool, Mpeg1Error> {
        self.r.skip(10); // temporal_reference
        self.picture_type = self.r.read(3);
        self.r.skip(16); // vbv_delay

        if self.r.overrun {
            return Err(Mpeg1Error::Truncated);
        }

        if !(TYPE_I..=TYPE_B).contains(&self.picture_type) {
            return Ok(false);
        }

        if self.picture_type == TYPE_P || self.picture_type == TYPE_B {
            self.forward.full_px = self.r.read(1) == 1;

            let f_code = self.r.read(3) as i32;

            if f_code == 0 {
                return Err(Mpeg1Error::Malformed("forward f_code zero"));
            }

            self.forward.r_size = f_code - 1;
        }

        if self.picture_type == TYPE_B {
            self.backward.full_px = self.r.read(1) == 1;

            let f_code = self.r.read(3) as i32;

            if f_code == 0 {
                return Err(Mpeg1Error::Malformed("backward f_code zero"));
            }

            self.backward.r_size = f_code - 1;
        }

        let reference = self.picture_type != TYPE_B;
        let spare = self.fwd;

        if reference {
            self.fwd = self.bwd;
        }

        let mut code = loop {
            match self.r.next_start_code() {
                Some(START_EXTENSION | START_USER_DATA) => {}
                other => break other,
            }
        };

        while let Some(c) = code.filter(|c| is_slice(*c)) {
            self.decode_slice(c as usize)?;

            if self.mb_address >= (self.mb_width * self.mb_height) as i64 - 1 {
                code = None;

                break;
            }

            code = self.r.next_start_code();
        }

        // The code that ended the slices belongs to what follows.
        self.pending = code;

        if reference {
            self.bwd = self.cur;
            self.cur = spare;
        }

        Ok(true)
    }

    fn decode_slice(&mut self, slice: usize) -> Result<(), Mpeg1Error> {
        if slice > self.mb_height {
            return Err(Mpeg1Error::Malformed("slice below the picture"));
        }

        self.slice_begin = true;
        self.mb_address = (slice as i64 - 1) * self.mb_width as i64 - 1;
        self.forward.h = 0;
        self.forward.v = 0;
        self.backward.h = 0;
        self.backward.v = 0;
        self.dc_predictor = [128; 3];
        self.quantizer_scale = self.r.read(5) as i32;

        while self.r.read(1) == 1 {
            self.r.skip(8); // extra_information_slice

            if self.r.overrun {
                return Err(Mpeg1Error::Truncated);
            }
        }

        let size = (self.mb_width * self.mb_height) as i64;

        loop {
            self.decode_macroblock()?;

            if !(self.mb_address < size - 1 && self.r.peek_non_zero(23)) {
                return Ok(());
            }
        }
    }

    fn decode_macroblock(&mut self) -> Result<(), Mpeg1Error> {
        let size = (self.mb_width * self.mb_height) as i64;
        let mut increment = 0i64;
        let mut t = self.r.vlc(&MACROBLOCK_ADDRESS_INCREMENT)?;

        while t == 34 {
            t = self.r.vlc(&MACROBLOCK_ADDRESS_INCREMENT)?; // macroblock_stuffing
        }

        while t == 35 {
            increment += 33; // macroblock_escape
            t = self.r.vlc(&MACROBLOCK_ADDRESS_INCREMENT)?;
        }

        increment += i64::from(t);

        if self.slice_begin {
            // The first increment of a slice counts from the start of the row above.
            self.slice_begin = false;
            self.mb_address += increment;
        } else {
            if self.mb_address + increment >= size {
                return Err(Mpeg1Error::Malformed("macroblock address"));
            }

            if increment > 1 {
                // Skipped macroblocks reset the DC predictors, and in a P picture the motion vector.
                self.dc_predictor = [128; 3];

                if self.picture_type == TYPE_P {
                    self.forward.h = 0;
                    self.forward.v = 0;
                }
            }

            while increment > 1 {
                self.mb_address += 1;
                self.set_position()?;
                self.predict_macroblock();
                increment -= 1;
            }

            self.mb_address += 1;
        }

        self.set_position()?;

        let table: &[Vlc] = match self.picture_type {
            TYPE_I => &MACROBLOCK_TYPE_INTRA,
            TYPE_P => &MACROBLOCK_TYPE_PREDICTIVE,
            _ => &MACROBLOCK_TYPE_B,
        };

        self.mb_type = self.r.vlc(table)?;
        self.mb_intra = self.mb_type & 0x01 != 0;
        self.forward.is_set = self.mb_type & 0x08 != 0;
        self.backward.is_set = self.mb_type & 0x04 != 0;

        if self.mb_type & 0x10 != 0 {
            self.quantizer_scale = self.r.read(5) as i32;
        }

        if self.mb_intra {
            self.forward.h = 0;
            self.forward.v = 0;
            self.backward.h = 0;
            self.backward.v = 0;
        } else {
            self.dc_predictor = [128; 3];
            self.decode_motion_vectors()?;
            self.predict_macroblock();
        }

        let cbp = if self.mb_type & 0x02 != 0 {
            self.r.vlc(&CODE_BLOCK_PATTERN)?
        } else if self.mb_intra {
            0x3f
        } else {
            0
        };

        for block in 0..6 {
            if cbp & (0x20 >> block) != 0 {
                self.decode_block(block)?;
            }
        }

        if self.r.overrun {
            return Err(Mpeg1Error::Truncated);
        }

        Ok(())
    }

    fn set_position(&mut self) -> Result<(), Mpeg1Error> {
        if self.mb_address < 0 {
            return Err(Mpeg1Error::Malformed("macroblock address"));
        }

        self.mb_row = self.mb_address as usize / self.mb_width;
        self.mb_col = self.mb_address as usize % self.mb_width;

        if self.mb_row >= self.mb_height {
            return Err(Mpeg1Error::Malformed("macroblock below the picture"));
        }

        Ok(())
    }

    fn decode_motion_vectors(&mut self) -> Result<(), Mpeg1Error> {
        if self.forward.is_set {
            let r_size = self.forward.r_size;

            self.forward.h = self.motion_vector(r_size, self.forward.h)?;
            self.forward.v = self.motion_vector(r_size, self.forward.v)?;
        } else if self.picture_type == TYPE_P {
            self.forward.h = 0;
            self.forward.v = 0;
        }

        if self.backward.is_set {
            let r_size = self.backward.r_size;

            self.backward.h = self.motion_vector(r_size, self.backward.h)?;
            self.backward.v = self.motion_vector(r_size, self.backward.v)?;
        }

        Ok(())
    }

    fn motion_vector(&mut self, r_size: i32, motion: i32) -> Result<i32, Mpeg1Error> {
        let fscale = 1i32 << r_size;
        let m_code = self.r.vlc(&MOTION)?;
        let d = if m_code != 0 && fscale != 1 {
            let r = self.r.read(r_size as usize) as i32;
            let d = ((m_code.abs() - 1) << r_size) + r + 1;

            if m_code < 0 { -d } else { d }
        } else {
            m_code
        };
        let mut motion = motion + d;

        if motion > (fscale << 4) - 1 {
            motion -= fscale << 5;
        } else if motion < -(fscale << 4) {
            motion += fscale << 5;
        }

        Ok(motion)
    }

    fn predict_macroblock(&mut self) {
        let double = |m: &Motion| {
            if m.full_px {
                (m.h << 1, m.v << 1)
            } else {
                (m.h, m.v)
            }
        };
        let (fw_h, fw_v) = double(&self.forward);

        if self.picture_type == TYPE_B {
            let (bw_h, bw_v) = double(&self.backward);

            if self.forward.is_set {
                self.copy_macroblock(self.fwd, fw_h, fw_v, false);

                if self.backward.is_set {
                    self.copy_macroblock(self.bwd, bw_h, bw_v, true);
                }
            } else {
                self.copy_macroblock(self.bwd, bw_h, bw_v, false);
            }
        } else {
            self.copy_macroblock(self.fwd, fw_h, fw_v, false);
        }
    }

    /// Predicts the macroblock at the current position from frame `from`, the half-sample motion vector `(mh, mv)`;
    /// `interpolate` averages with what is there. A vector that points outside the picture predicts nothing.
    fn copy_macroblock(&mut self, from: usize, mh: i32, mv: i32, interpolate: bool) {
        let (row, col) = (self.mb_row, self.mb_col);
        let rows = self.mb_height;
        let (luma_width, chroma_width) = (self.luma_width, self.chroma_width);
        let (src, dst) = frame_pair(&mut self.frames, from, self.cur);

        process_macroblock(
            &src.y,
            &mut dst.y,
            (luma_width, rows * 16),
            (row, col),
            16,
            (mh, mv),
            interpolate,
        );
        process_macroblock(
            &src.cb,
            &mut dst.cb,
            (chroma_width, rows * 8),
            (row, col),
            8,
            (mh / 2, mv / 2),
            interpolate,
        );
        process_macroblock(
            &src.cr,
            &mut dst.cr,
            (chroma_width, rows * 8),
            (row, col),
            8,
            (mh / 2, mv / 2),
            interpolate,
        );
    }

    fn decode_block(&mut self, block: usize) -> Result<(), Mpeg1Error> {
        let mut n = 0usize;
        let intra = self.mb_intra;

        if intra {
            let plane = block.saturating_sub(3);
            let predictor = self.dc_predictor[plane];
            let table: &[Vlc] = if plane == 0 {
                &DCT_SIZE_LUMINANCE
            } else {
                &DCT_SIZE_CHROMINANCE
            };
            let dct_size = self.r.vlc(table)? as usize;
            let dc = if dct_size > 0 {
                let differential = self.r.read(dct_size) as i32;

                if differential & (1 << (dct_size - 1)) != 0 {
                    predictor.wrapping_add(differential)
                } else {
                    predictor.wrapping_add(-(1 << dct_size) | (differential + 1))
                }
            } else {
                predictor
            };

            self.dc_predictor[plane] = dc;
            // The intra DC coefficient is the differential sum times 8 (no quantizer scale).
            self.block[0] = dc.wrapping_mul(8);
            n = 1;
        }

        let quant = if intra {
            self.intra_quant
        } else {
            self.non_intra_quant
        };

        loop {
            let coeff = self.r.vlc(&DCT_COEFF)?;

            if coeff == 0x0001 && n > 0 && self.r.read(1) == 0 {
                break; // end_of_block
            }

            let run;
            let mut level;

            if coeff == 0xffff {
                run = self.r.read(6) as usize;
                level = self.r.read(8) as i32;

                if level == 0 {
                    level = self.r.read(8) as i32;
                } else if level == 128 {
                    level = self.r.read(8) as i32 - 256;
                } else if level > 128 {
                    level -= 256;
                }
            } else {
                run = (coeff >> 8) as usize;
                level = coeff & 0xff;

                if self.r.read(1) == 1 {
                    level = -level;
                }
            }

            n += run;

            if n >= 64 {
                return Err(Mpeg1Error::Malformed("coefficient run past the block"));
            }

            let position = ZIG_ZAG[n] as usize;

            n += 1;

            // Dequantize (the division truncates toward zero, so on the magnitude), make the magnitude odd, clip.
            let negative = level < 0;
            let mut magnitude = level.abs() * 2 + i32::from(!intra);

            magnitude = (magnitude * self.quantizer_scale * i32::from(quant[position])) >> 4;

            if magnitude & 1 == 0 {
                magnitude -= 1;
            }

            magnitude = magnitude.clamp(0, if negative { 2048 } else { 2047 });
            level = if negative { -magnitude } else { magnitude };
            self.block[position] = level;

            if self.r.overrun {
                return Err(Mpeg1Error::Truncated);
            }
        }

        let (row, col) = (self.mb_row, self.mb_col);
        let (stride, start) = if block < 4 {
            let mut at = (row * self.luma_width + col) * 16;

            if block & 1 != 0 {
                at += 8;
            }

            if block & 2 != 0 {
                at += self.luma_width * 8;
            }

            (self.luma_width, at)
        } else {
            (self.chroma_width, row * 8 * self.chroma_width + col * 8)
        };
        let frame = &mut self.frames[self.cur];
        let plane = match block {
            0..=3 => &mut frame.y,
            4 => &mut frame.cb,
            _ => &mut frame.cr,
        };

        // Eight rows of eight samples must fit: the macroblock position was checked against the picture.
        let last = start + 7 * stride + 7;

        if last >= plane.len() {
            return Err(Mpeg1Error::Malformed("block outside the picture"));
        }

        let residual = idct(&self.block);

        for y in 0..8 {
            for x in 0..8 {
                let at = start + y * stride + x;
                let r = residual[y * 8 + x];

                plane[at] = if intra {
                    clamp(r)
                } else {
                    clamp(i64::from(plane[at]) + r)
                };
            }
        }

        self.block = [0; 64];

        Ok(())
    }
}

/// Mutable access to frame `dst` and shared access to frame `src` (two different indexes).
fn frame_pair(frames: &mut [Frame; 3], src: usize, dst: usize) -> (&Frame, &mut Frame) {
    assert_ne!(src, dst, "a picture is not predicted from itself");

    let (a, b) = if src < dst {
        let (lo, hi) = frames.split_at_mut(dst);

        (&lo[src], &mut hi[0])
    } else {
        let (lo, hi) = frames.split_at_mut(src);

        (&hi[0], &mut lo[dst])
    };

    (a, b)
}

/// One block of a prediction (`bs` is 16 for luma, 8 for chroma): the samples of `src` at the half-sample vector `mv`
/// into `dst`, averaged with `dst` for `interpolate`. `plane` is the stride and the height.
fn process_macroblock(
    src: &[u8],
    dst: &mut [u8],
    plane: (usize, usize),
    at: (usize, usize),
    bs: usize,
    mv: (i32, i32),
    interpolate: bool,
) {
    let (dw, dh) = plane;
    let (hp, vp) = ((mv.0 >> 1) as i64, (mv.1 >> 1) as i64);
    let (odd_h, odd_v) = (mv.0 & 1 == 1, mv.1 & 1 == 1);
    let x0 = (at.1 * bs) as i64 + hp;
    let y0 = (at.0 * bs) as i64 + vp;
    let (w, h) = (bs + usize::from(odd_h), bs + usize::from(odd_v));

    // The block and the samples it averages must lie in the picture; a vector that does not is a corrupt stream, which
    // pl_mpeg (and a decoder of the standard) leaves unpredicted.
    if x0 < 0 || y0 < 0 || x0 as usize + w > dw || y0 as usize + h > dh {
        return;
    }

    let (x0, y0) = (x0 as usize, y0 as usize);
    let (dx, dy) = (at.1 * bs, at.0 * bs);

    for y in 0..bs {
        for x in 0..bs {
            let s = (y0 + y) * dw + x0 + x;
            let p = |i: usize| u32::from(src[i]);
            let pred = match (odd_h, odd_v) {
                (false, false) => p(s),
                (false, true) => (p(s) + p(s + dw) + 1) >> 1,
                (true, false) => (p(s) + p(s + 1) + 1) >> 1,
                (true, true) => (p(s) + p(s + 1) + p(s + dw) + p(s + dw + 1) + 2) >> 2,
            };
            let d = &mut dst[(dy + y) * dw + dx + x];

            *d = if interpolate {
                ((u32::from(*d) + pred + 1) >> 1) as u8
            } else {
                pred as u8
            };
        }
    }
}

/// The 8 by 8 inverse DCT of the coefficients `input` (row-major, vertical frequency first), computed in double
/// precision from the definition, `f(x, y) = 1/4 sum c(u) c(v) F(v, u) cos((2x+1) u pi / 16) cos((2y+1) v pi / 16)` with
/// `c(0) = 1/sqrt(2)`, and rounded to the nearest integer. This is the reference the accuracy requirement of the standard
/// (IEEE 1180) is measured against, so it is as accurate as an inverse DCT can be; pl_mpeg's integer IDCT with 8-bit
/// constants is not (it differed from ffmpeg by up to 8 levels through the prediction chain, see
/// docs/mpeg1-implementation.md). The result is clipped to -256..=255 as the standard requires before it is added to a
/// prediction.
fn idct(input: &[i32; 64]) -> [i64; 64] {
    static BASIS: std::sync::OnceLock<[[f64; 8]; 8]> = std::sync::OnceLock::new();

    // basis[u][x] = c(u) / 2 * cos((2x + 1) u pi / 16)
    let basis = BASIS.get_or_init(|| {
        std::array::from_fn(|u| {
            std::array::from_fn(|x| {
                let c = if u == 0 {
                    std::f64::consts::FRAC_1_SQRT_2
                } else {
                    1.0
                };

                c / 2.0 * (((2 * x + 1) * u) as f64 * std::f64::consts::PI / 16.0).cos()
            })
        })
    });
    let mut rows = [[0f64; 8]; 8];

    // Along each row (horizontal frequencies u to positions x).
    for (i, row) in rows.iter_mut().enumerate() {
        for (x, out) in row.iter_mut().enumerate() {
            *out = (0..8)
                .map(|u| f64::from(input[i * 8 + u]) * basis[u][x])
                .sum();
        }
    }

    let mut out = [0i64; 64];

    // Along each column (vertical frequencies v to positions y).
    for y in 0..8 {
        for x in 0..8 {
            let v: f64 = (0..8).map(|v| rows[v][x] * basis[v][y]).sum();

            out[y * 8 + x] = (v.round() as i64).clamp(-256, 255);
        }
    }

    out
}
