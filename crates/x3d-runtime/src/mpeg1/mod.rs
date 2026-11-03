//! MPEG-1 movies for MovieTexture: ISO/IEC 19775-1 18.4.2 requires "MPEG1-Systems (audio and video) or MPEG1-Video
//! (video-only) movie file formats as defined in ISO/IEC 11172-1". This module decodes the pictures of both: an MPEG-1
//! Systems stream ([`demux`]) or a bare video elementary stream, with the video decoder in [`video`].
//!
//! The sound of a Systems stream is not decoded here (see docs/mpeg1-implementation.md for the reason and the plan).
//! Reverse playback and the ISO time model are a different lane; this module only produces pictures with times.
//!
//! Safety: no `unsafe`. Every malformed, truncated or unsupported input ends in an [`Mpeg1Error`]; pictures decoded before
//! a mid-stream error are kept ([`Video::stopped`]).

pub mod demux;
mod tables;
pub mod video;

use video::Decoder;

/// Why MPEG-1 data is not decoded.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Mpeg1Error {
    /// Neither an MPEG-1 Systems stream (pack start code) nor a video elementary stream (sequence header).
    NotMpeg1,
    /// An MPEG-2 program stream or MPEG-2 video: not supported.
    UnsupportedMpeg2,
    /// A variant that MPEG-1 allows but this decoder does not handle.
    Unsupported(&'static str),
    /// The data breaks a rule of the format.
    Malformed(&'static str),
    /// The data ends inside a header or a picture.
    Truncated,
    /// The stream holds no picture.
    NoPicture,
    /// The decoded pictures would be larger than the byte budget.
    TooLarge,
}

impl std::fmt::Display for Mpeg1Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Mpeg1Error::NotMpeg1 => write!(f, "not an MPEG-1 stream"),
            Mpeg1Error::UnsupportedMpeg2 => write!(f, "MPEG-2 streams are not supported"),
            Mpeg1Error::Unsupported(what) => write!(f, "unsupported MPEG-1 variant: {what}"),
            Mpeg1Error::Malformed(what) => write!(f, "malformed MPEG-1 data: {what}"),
            Mpeg1Error::Truncated => write!(f, "truncated MPEG-1 data"),
            Mpeg1Error::NoPicture => write!(f, "no picture in the MPEG-1 stream"),
            Mpeg1Error::TooLarge => write!(f, "the decoded pictures are too large"),
        }
    }
}

/// Does `data` start like an MPEG-1 stream the decoder can try: a pack start code (`00 00 01 BA`) or a video sequence
/// header (`00 00 01 B3`)?
pub fn looks_like_mpeg1(data: &[u8]) -> bool {
    data.starts_with(&[0, 0, 1, 0xBA]) || data.starts_with(&[0, 0, 1, 0xB3])
}

/// File names a MovieTexture url hands to this decoder: `.mpg`, `.mpeg`, `.mpe` (MPEG-1 Systems) and `.m1v` (video only).
pub fn has_mpeg1_extension(path: &str) -> bool {
    let path = path.to_ascii_lowercase();

    [".mpg", ".mpeg", ".mpe", ".m1v"]
        .iter()
        .any(|e| path.ends_with(e))
}

/// The decoded pictures in display order, as planar YUV (see [`video::Picture`]).
pub struct YuvVideo {
    pub width: usize,
    pub height: usize,
    pub framerate: f64,
    pub pictures: Vec<video::Picture>,
    pub has_audio: bool,
    /// The error that ended the decode before the end of the data, if any.
    pub stopped: Option<Mpeg1Error>,
}

/// Decodes the pictures of an MPEG-1 Systems stream or video elementary stream. At most `max_pictures` are kept.
pub fn decode_yuv(data: &[u8], max_pictures: usize) -> Result<YuvVideo, Mpeg1Error> {
    let (owned, has_audio);
    let es: &[u8] = if demux::is_system_stream(data) {
        let d = demux::demux(data)?;

        owned = d.video;
        has_audio = d.has_audio;

        &owned
    } else if data.starts_with(&[0, 0, 1, 0xB3]) {
        has_audio = false;

        data
    } else {
        return Err(Mpeg1Error::NotMpeg1);
    };
    let mut decoder = Decoder::new(es)?;
    let mut pictures = vec![];
    let mut stopped = None;

    while pictures.len() < max_pictures {
        match decoder.next_picture() {
            Ok(Some(p)) => pictures.push(p),
            Ok(None) => break,
            Err(e) => {
                stopped = Some(e);

                break;
            }
        }
    }

    if pictures.is_empty() {
        return Err(stopped.unwrap_or(Mpeg1Error::NoPicture));
    }

    Ok(YuvVideo {
        width: decoder.width,
        height: decoder.height,
        framerate: decoder.framerate,
        pictures,
        has_audio,
        stopped,
    })
}

/// The pictures of a movie as RGBA, with the picture rate. Times are `index / framerate`.
pub struct Video {
    pub width: u32,
    pub height: u32,
    pub framerate: f64,
    /// RGBA rows from the top, one `Vec` per picture.
    pub frames: Vec<Vec<u8>>,
    pub has_audio: bool,
    /// The error that ended the decode before the end of the data: the pictures before it are kept.
    pub stopped: Option<Mpeg1Error>,
}

/// Decodes `data` (MPEG-1 Systems or video elementary stream) to RGBA pictures. The RGBA pictures together may not be
/// larger than `max_bytes`; a movie that would be is refused ([`Mpeg1Error::TooLarge`]) before the next picture is kept.
pub fn decode(data: &[u8], max_bytes: u64) -> Result<Video, Mpeg1Error> {
    // The size is known from the header, so the budget bounds the number of pictures before they are decoded.
    let probe = decode_yuv(data, 1)?;
    let one = (probe.width as u64) * (probe.height as u64) * 4;
    let max_pictures = usize::try_from(max_bytes / one.max(1)).unwrap_or(usize::MAX);

    if max_pictures == 0 {
        return Err(Mpeg1Error::TooLarge);
    }

    let yuv = decode_yuv(data, max_pictures.saturating_add(1))?;

    if yuv.pictures.len() > max_pictures {
        return Err(Mpeg1Error::TooLarge);
    }

    let (w, h) = (yuv.width, yuv.height);

    Ok(Video {
        width: w as u32,
        height: h as u32,
        framerate: yuv.framerate,
        frames: yuv
            .pictures
            .iter()
            .map(|p| yuv_to_rgba(w, h, &p.y, &p.cb, &p.cr))
            .collect(),
        has_audio: yuv.has_audio,
        stopped: yuv.stopped,
    })
}

/// BT.601 limited range to full-range RGB, the conversion for MPEG-1 video (which has no colour description; its
/// pictures are SIF/CIF sized and conventionally ITU-R BT.601). The 4:2:0 chroma is interpolated linearly between the
/// sample centers, the siting MPEG-1 defines (between the luma samples in both directions).
fn yuv_to_rgba(w: usize, h: usize, y: &[u8], cb: &[u8], cr: &[u8]) -> Vec<u8> {
    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let mut out = Vec::with_capacity(w * h * 4);
    let clamp = |x: f32| x.round().clamp(0.0, 255.0) as u8;
    let chroma = |p: &[u8], i: usize, j: usize| -> f32 {
        let fx = (i as f32 + 0.5) / 2.0 - 0.5;
        let fy = (j as f32 + 0.5) / 2.0 - 0.5;
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let at = |x: f32, y: f32| {
            let xi = (x.max(0.0) as usize).min(cw - 1);
            let yi = (y.max(0.0) as usize).min(ch - 1);

            f32::from(p[yi * cw + xi])
        };
        let top = at(x0, y0) * (1.0 - tx) + at(x0 + 1.0, y0) * tx;
        let bottom = at(x0, y0 + 1.0) * (1.0 - tx) + at(x0 + 1.0, y0 + 1.0) * tx;

        top * (1.0 - ty) + bottom * ty
    };

    for j in 0..h {
        for i in 0..w {
            let c = 1.164_383 * (f32::from(y[j * w + i]) - 16.0);
            let (u, v) = (chroma(cb, i, j) - 128.0, chroma(cr, i, j) - 128.0);

            out.extend_from_slice(&[
                clamp(c + 1.596_027 * v),
                clamp(c - 0.391_762 * u - 0.812_968 * v),
                clamp(c + 2.017_232 * u),
                255,
            ]);
        }
    }

    out
}
