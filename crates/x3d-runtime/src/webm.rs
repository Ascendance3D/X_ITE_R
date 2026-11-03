//! A minimal WebM (Matroska subset) demuxer and the VP9 or VP8 decode of its first video track, for MovieTexture.
//!
//! Only what a MovieTexture needs: the first video track whose CodecID is `V_VP9` or `V_VP8`, its blocks in file order with their
//! presentation times in milliseconds, and the frame size. Lacing is not supported (WebM video does not use it).
//! VP9 is decoded by the `vp9dec` crate (MIT, no dependencies), VP8 by [`crate::vp8`]; the picture is converted to RGBA the way a browser's video
//! texture upload does (see [`yuv_to_rgba`]).
//!
//! Limits. [`decode`] takes `max_bytes`, which caps the RGBA pictures that are *kept* (and is checked per picture,
//! before its conversion). The decoder's own working memory (the reference pictures of `vp9dec`, about 1.5 u16 per
//! luma sample each, up to eight held) is bounded by [`MAX_LUMA_SAMPLES`], which is checked from the frame headers
//! before the decoder runs; it is not bounded by `max_bytes`. A fixed 8192 x 4352 picture measured 860 MB peak.

use crate::vp8;
use vp9dec::{Decoder, PlaneData};

const ID_EBML: u64 = 0x1A45_DFA3;
const ID_SEGMENT: u64 = 0x1853_8067;
const ID_INFO: u64 = 0x1549_A966;
const ID_TIMECODE_SCALE: u64 = 0x2A_D7B1;
const ID_TRACKS: u64 = 0x1654_AE6B;
const ID_TRACK_ENTRY: u64 = 0xAE;
const ID_TRACK_NUMBER: u64 = 0xD7;
const ID_TRACK_TYPE: u64 = 0x83;
const ID_CODEC_ID: u64 = 0x86;
const ID_CLUSTER: u64 = 0x1F43_B675;
const ID_TIMECODE: u64 = 0xE7;
const ID_SIMPLE_BLOCK: u64 = 0xA3;
const ID_BLOCK_GROUP: u64 = 0xA0;
const ID_BLOCK: u64 = 0xA1;

/// The largest picture, in luma samples, that [`decode`] hands to the decoder: 35 651 584, the largest picture of
/// VP9's highest levels (8192 x 4352 fits; the level tables are not in the local standards library, so this is a
/// limit of this implementation, not a quotation). `vp9dec` allows 2^27 samples, about 400 MB per picture and up
/// to eight pictures held as references; a size from a file is checked here first.
pub const MAX_LUMA_SAMPLES: u64 = 35_651_584;

/// Why a file is not decoded.
#[derive(Debug)]
pub enum WebmError {
    NotWebm,
    /// No video track with a codec that is decoded here (VP9, VP8).
    NoVideoTrack,
    Decode(String),
    /// Either a picture codes more luma samples than [`MAX_LUMA_SAMPLES`] (refused before the decoder runs), or the
    /// RGBA pictures kept so far exceed the `max_bytes` of [`decode`] (refused before the next picture is kept). The
    /// second limit does not bound the working memory of the decoder; the first does.
    TooLarge,
}

/// One decoded picture: RGBA rows from the top, and its presentation time in milliseconds.
pub struct VideoFrame {
    pub rgba: Vec<u8>,
    pub time_ms: f64,
}

pub struct Video {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<VideoFrame>,
}

/// An element id (marker bits kept) and its size (`None`: unknown size), and the header length.
fn element(d: &[u8], at: usize) -> Option<(u64, Option<usize>, usize)> {
    let first = *d.get(at)?;
    let idlen = first.leading_zeros() as usize + 1;

    if idlen > 4 {
        return None;
    }

    let mut id = 0u64;

    for i in 0..idlen {
        id = (id << 8) | *d.get(at + i)? as u64;
    }

    let s0 = *d.get(at + idlen)?;
    let slen = s0.leading_zeros() as usize + 1;

    if slen > 8 {
        return None;
    }

    let mut size = (s0 as u64) & (0xFF >> slen);
    let mut all_ones = size == (0xFF >> slen) as u64;

    for i in 1..slen {
        let b = *d.get(at + idlen + i)?;
        all_ones &= b == 0xFF;
        size = (size << 8) | b as u64;
    }

    let size = (!all_ones).then_some(size as usize);

    Some((id, size, idlen + slen))
}

fn uint(b: &[u8]) -> u64 {
    b.iter().fold(0, |a, x| (a << 8) | *x as u64)
}

/// The children of the master element in `d[start..end]`: (id, body range).
fn children(d: &[u8], start: usize, end: usize) -> Vec<(u64, usize, usize)> {
    let mut out = vec![];
    let mut at = start;

    while at < end {
        let Some((id, size, head)) = element(d, at) else {
            break;
        };
        let body = at + head;

        // A header that ends beyond the parent is cut off: no body to read (and `body..stop` would be reversed).
        if body > end {
            break;
        }

        let stop = size.map_or(end, |s| body.saturating_add(s)).min(end);

        out.push((id, body, stop));
        at = stop;
    }

    out
}

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Codec {
    Vp9,
    Vp8,
}

struct Block {
    time_ms: f64,
    start: usize,
    end: usize,
}

struct Bits<'a> {
    d: &'a [u8],
    at: usize,
}

impl Bits<'_> {
    fn bits(&mut self, n: usize) -> Option<u32> {
        let mut v = 0u32;

        for _ in 0..n {
            // Past the end every bit is 0, as in the bit reader of `vp9dec`: a header cut off inside the picture size is
            // parsed by the decoder with the missing bits zero, and the check must see the size the decoder sees.
            let byte = self.d.get(self.at / 8).copied().unwrap_or(0);

            v = (v << 1) | u32::from(byte >> (7 - self.at % 8) & 1);
            self.at += 1;
        }

        Some(v)
    }
}

/// The frames of a chunk: a superframe (VP9 Annex B) is its frames, any other chunk is one frame.
fn frames_of(chunk: &[u8]) -> Vec<&[u8]> {
    // Cut where `vp9dec::superframe::split_superframe` cuts (an empty chunk is one empty frame there as well).
    let Some(&last) = chunk.last() else {
        return vec![chunk];
    };

    if last >> 5 == 0b110 {
        let count = (last & 7) as usize + 1;
        let size_bytes = ((last >> 3) & 3) as usize + 1;
        let index = 2 + size_bytes * count;

        if chunk.len() >= index && chunk[chunk.len() - index] == last {
            let mut out = vec![];
            let (mut at, mut pos) = (0usize, chunk.len() - index + 1);

            for _ in 0..count {
                let size = chunk[pos..pos + size_bytes]
                    .iter()
                    .rev()
                    .fold(0usize, |a, b| (a << 8) | *b as usize);

                pos += size_bytes;

                let Some(end) = at.checked_add(size).filter(|e| *e <= chunk.len() - index) else {
                    return vec![chunk];
                };

                out.push(&chunk[at..end]);
                at = end;
            }

            return out;
        }
    }

    vec![chunk]
}

/// The picture size that the uncompressed header of one VP9 frame codes itself (a key frame, an intra-only frame, an
/// inter frame that does not copy the size of a reference), `None` for any other frame. A size
/// copied from a reference was coded by an earlier frame, so checking the coded ones checks them all.
fn coded_size(frame: &[u8]) -> Option<(u32, u32)> {
    let mut r = Bits { d: frame, at: 0 };

    if r.bits(2)? != 2 {
        return None;
    }

    let profile = r.bits(1)? | (r.bits(1)? << 1);

    if profile == 3 {
        r.bits(1)?;
    }

    if r.bits(1)? == 1 {
        return None; // show_existing_frame
    }

    let key = r.bits(1)? == 0;
    let show_frame = r.bits(1)?;
    let error_resilient = r.bits(1)?;

    let color_config = |r: &mut Bits| -> Option<()> {
        if profile >= 2 {
            r.bits(1)?;
        }

        if r.bits(3)? != 7 {
            r.bits(1)?;

            if profile == 1 || profile == 3 {
                r.bits(3)?;
            }
        } else if profile == 1 || profile == 3 {
            r.bits(1)?;
        }

        Some(())
    };
    let size = |r: &mut Bits| -> Option<(u32, u32)> { Some((r.bits(16)? + 1, r.bits(16)? + 1)) };

    if key {
        r.bits(24)?;
        color_config(&mut r)?;

        return size(&mut r);
    }

    let intra_only = if show_frame == 0 {
        r.bits(1)? == 1
    } else {
        false
    };

    if error_resilient == 0 {
        r.bits(2)?;
    }

    if intra_only {
        r.bits(24)?;

        if profile > 0 {
            color_config(&mut r)?;
        }

        r.bits(8)?;

        return size(&mut r);
    }

    r.bits(8 + 3 * 4)?;

    for _ in 0..3 {
        if r.bits(1)? == 1 {
            return None; // found_ref: the size of a reference
        }
    }

    size(&mut r)
}

/// The codec and the blocks of the first VP9 or VP8 video track of `data`, in file order.
fn demux(data: &[u8]) -> Result<(Codec, Vec<Block>), WebmError> {
    let top = children(data, 0, data.len());

    if top.first().map(|t| t.0) != Some(ID_EBML) {
        return Err(WebmError::NotWebm);
    }

    let Some(&(_, seg_start, seg_end)) = top.iter().find(|t| t.0 == ID_SEGMENT) else {
        return Err(WebmError::NotWebm);
    };
    let segment = children(data, seg_start, seg_end);
    let mut scale = 1_000_000u64;
    let mut track = None;

    for &(id, s, e) in &segment {
        if id == ID_INFO {
            for (cid, cs, ce) in children(data, s, e) {
                if cid == ID_TIMECODE_SCALE {
                    scale = uint(&data[cs..ce]).max(1);
                }
            }
        } else if id == ID_TRACKS {
            for (tid, ts, te) in children(data, s, e) {
                if tid != ID_TRACK_ENTRY || track.is_some() {
                    continue;
                }

                let (mut number, mut kind, mut codec) = (0, 0, &data[0..0]);

                for (cid, cs, ce) in children(data, ts, te) {
                    match cid {
                        ID_TRACK_NUMBER => number = uint(&data[cs..ce]),
                        ID_TRACK_TYPE => kind = uint(&data[cs..ce]),
                        ID_CODEC_ID => codec = &data[cs..ce],
                        _ => {}
                    }
                }

                if kind == 1 && codec == b"V_VP9" {
                    track = Some((number, Codec::Vp9));
                } else if kind == 1 && codec == b"V_VP8" {
                    track = Some((number, Codec::Vp8));
                }
            }
        }
    }

    let (track, codec) = track.ok_or(WebmError::NoVideoTrack)?;
    let mut blocks = vec![];

    for &(id, s, e) in &segment {
        if id != ID_CLUSTER {
            continue;
        }

        let mut cluster_time = 0u64;

        for (cid, cs, ce) in children(data, s, e) {
            let block = match cid {
                ID_TIMECODE => {
                    cluster_time = uint(&data[cs..ce]);
                    continue;
                }
                ID_SIMPLE_BLOCK => Some((cs, ce)),
                ID_BLOCK_GROUP => children(data, cs, ce)
                    .into_iter()
                    .find(|c| c.0 == ID_BLOCK)
                    .map(|c| (c.1, c.2)),
                _ => None,
            };

            let Some((bs, be)) = block else {
                continue;
            };
            let Some((number, _, nlen)) = element_number(data, bs) else {
                continue;
            };

            // track number, 16-bit relative timecode, flags (lacing bits must be 0).
            let header = nlen + 3;

            if number != track || be < bs + header || data[bs + nlen + 2] & 0x06 != 0 {
                continue;
            }

            let rel = i16::from_be_bytes([data[bs + nlen], data[bs + nlen + 1]]) as i64;
            let ticks = cluster_time as i64 + rel;

            blocks.push(Block {
                time_ms: ticks as f64 * scale as f64 / 1_000_000.0,
                start: bs + header,
                end: be,
            });
        }
    }

    Ok((codec, blocks))
}

/// Demux `data`: the width/height are known only after decoding. The pictures are RGBA in memory, so `max_bytes` is
/// the most that all of them may take together; a picture larger than [`MAX_LUMA_SAMPLES`] is not decoded.
pub fn decode(data: &[u8], max_bytes: u64) -> Result<Video, WebmError> {
    let (codec, blocks) = demux(data)?;

    match codec {
        Codec::Vp9 => decode_vp9(data, &blocks, max_bytes),
        Codec::Vp8 => decode_vp8(data, &blocks, max_bytes),
    }
}

/// The VP8 blocks of a file: one frame per block. A frame the decoder refuses ends the decode with an error (the
/// caller treats the file as a video the engine cannot show); a frame that is not shown only updates the references.
fn decode_vp8(data: &[u8], blocks: &[Block], max_bytes: u64) -> Result<Video, WebmError> {
    let mut decoder = vp8::Vp8Decoder::new(vp8::Limits {
        max_width: 16383,
        max_height: 16383,
        max_allocation_bytes: MAX_LUMA_SAMPLES * 6,
    });
    let mut frames = vec![];
    let (mut width, mut height) = (0, 0);
    let mut bytes = 0u64;

    for b in blocks {
        let chunk = &data[b.start..b.end];

        if let Some((w, h)) = vp8::key_frame_size(chunk)
            && u64::from(w) * u64::from(h) > MAX_LUMA_SAMPLES
        {
            return Err(WebmError::TooLarge);
        }

        // As for VP9: a panic is a refused file, not a crash; the limits and the checks are what bound a hostile file.
        let shown =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decoder.decode(chunk)))
                .map_err(|_| WebmError::Decode("the decoder panicked".into()))?
                .map_err(|e| WebmError::Decode(e.to_string()))?;

        let Some(f) = shown else {
            continue;
        };
        let (w, h) = (f.width, f.height);

        if f.planes[0].len() != w * h
            || f.planes[1].len() != w.div_ceil(2) * h.div_ceil(2)
            || f.planes[2].len() != w.div_ceil(2) * h.div_ceil(2)
        {
            return Err(WebmError::Decode("inconsistent picture".into()));
        }

        width = w as u32;
        height = h as u32;
        bytes = bytes.saturating_add(u64::from(width) * u64::from(height) * 4);

        if bytes > max_bytes {
            return Err(WebmError::TooLarge);
        }

        frames.push(VideoFrame {
            rgba: yuv_to_rgba(
                w,
                h,
                true,
                true,
                &f.planes[0],
                &f.planes[1],
                &f.planes[2],
                VP8_MATRIX,
            ),
            time_ms: b.time_ms,
        });
    }

    if frames.is_empty() {
        return Err(WebmError::Decode("no picture".into()));
    }

    Ok(Video {
        width,
        height,
        frames,
    })
}

fn decode_vp9(data: &[u8], blocks: &[Block], max_bytes: u64) -> Result<Video, WebmError> {
    let mut decoder = Decoder::new();
    let mut frames = vec![];
    let (mut width, mut height) = (0, 0);
    let mut bytes = 0u64;

    for b in blocks {
        let chunk = &data[b.start..b.end];

        for f in frames_of(chunk) {
            if let Some((w, h)) = coded_size(f)
                && u64::from(w) * u64::from(h) > MAX_LUMA_SAMPLES
            {
                return Err(WebmError::TooLarge);
            }
        }

        // The decoder is a third-party crate: a panic in it is a refused file, not a crash of the viewer. This is a
        // fallback, not a security boundary: it does nothing under `panic = "abort"` (the workspace sets none, in any
        // profile), the default hook still prints the message, a panic in a loop-filter worker leaves the other workers
        // spinning (`vp9dec` wavefront), and it cannot undo memory a decoder wrote through `unsafe`. The limits above and
        // the header checks are what bound a hostile file. Only the decoder call is inside it: the code after it (the
        // plane match and `yuv_to_rgba`) is this crate's own, indexes only within the picture the decoder reports, and
        // is covered by the mutation tests below.
        let shown =
            std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| decoder.decode_frame(chunk)))
                .map_err(|_| WebmError::Decode("the decoder panicked".into()))?
                .map_err(|e| WebmError::Decode(format!("{e:?}")))?;

        for f in shown.into_iter().filter_map(|d| d.frame) {
            let (PlaneData::U8(y), PlaneData::U8(u), PlaneData::U8(v)) = (&f.y, &f.u, &f.v) else {
                return Err(WebmError::Decode("not an 8-bit stream".into()));
            };

            width = f.width as u32;
            height = f.height as u32;
            bytes = bytes.saturating_add(
                u64::from(width)
                    .saturating_mul(u64::from(height))
                    .saturating_mul(4),
            );

            if bytes > max_bytes {
                return Err(WebmError::TooLarge);
            }

            frames.push(VideoFrame {
                rgba: yuv_to_rgba(
                    f.width as usize,
                    f.height as usize,
                    f.subsampling_x != 0,
                    f.subsampling_y != 0,
                    y,
                    u,
                    v,
                    BT709,
                ),
                time_ms: b.time_ms,
            });
        }
    }

    if frames.is_empty() {
        return Err(WebmError::Decode("no picture".into()));
    }

    Ok(Video {
        width,
        height,
        frames,
    })
}

/// A track number: an EBML variable-length integer with the marker removed.
fn element_number(d: &[u8], at: usize) -> Option<(u64, (), usize)> {
    let first = *d.get(at)?;
    let len = first.leading_zeros() as usize + 1;

    if len > 8 {
        return None;
    }

    let mut v = (first as u64) & (0xFF >> len);

    for i in 1..len {
        v = (v << 8) | *d.get(at + i)? as u64;
    }

    Some((v, (), len))
}

/// BT.709 limited range to full-range RGB. A browser's video upload converts an untagged stream with the BT.709
/// matrix (measured against the pinned page: BT.601 is off by 10 levels on saturated colors) and interpolates the
/// subsampled chroma linearly between the sample centers, which is done here too.
/// The YUV to RGB coefficients: Y scale, then the red V, green U, green V and blue U terms.
type Matrix = [f32; 5];

/// BT.709, which the page applies to an untagged VP9 stream.
const BT709: Matrix = [1.164_383, 1.792_741, 0.213_249, 0.532_909, 2.112_402];
/// What the page applies to a VP8 stream (set from the measurement in the VP8 tests).
const VP8_MATRIX: Matrix = BT709;

#[allow(clippy::too_many_arguments)]
fn yuv_to_rgba(
    w: usize,
    h: usize,
    sx: bool,
    sy: bool,
    y: &[u8],
    u: &[u8],
    v: &[u8],
    m: Matrix,
) -> Vec<u8> {
    let cw = if sx { w.div_ceil(2) } else { w };
    let ch = if sy { h.div_ceil(2) } else { h };
    let mut out = Vec::with_capacity(w * h * 4);
    let clamp = |x: f32| x.round().clamp(0.0, 255.0) as u8;

    // The chroma plane `p` at luma position (i, j), bilinear between the chroma sample centers.
    let chroma = |p: &[u8], i: usize, j: usize| -> f32 {
        let fx = if sx {
            (i as f32 + 0.5) / 2.0 - 0.5
        } else {
            i as f32
        };
        let fy = if sy {
            (j as f32 + 0.5) / 2.0 - 0.5
        } else {
            j as f32
        };
        let (x0, y0) = (fx.floor(), fy.floor());
        let (tx, ty) = (fx - x0, fy - y0);
        let at = |x: f32, y: f32| {
            let xi = (x.max(0.0) as usize).min(cw - 1);
            let yi = (y.max(0.0) as usize).min(ch - 1);

            p[yi * cw + xi] as f32
        };
        let top = at(x0, y0) * (1.0 - tx) + at(x0 + 1.0, y0) * tx;
        let bottom = at(x0, y0 + 1.0) * (1.0 - tx) + at(x0 + 1.0, y0 + 1.0) * tx;

        top * (1.0 - ty) + bottom * ty
    };

    for j in 0..h {
        for i in 0..w {
            let yy = y[j * w + i] as f32;
            let (uu, vv) = (chroma(u, i, j) - 128.0, chroma(v, i, j) - 128.0);
            let c = m[0] * (yy - 16.0);

            out.extend_from_slice(&[
                clamp(c + m[1] * vv),
                clamp(c - m[2] * uu - m[3] * vv),
                clamp(c + m[4] * uu),
                255,
            ]);
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The byte budget of MovieTexture.
    const BUDGET: u64 = 512 << 20;

    fn synthetic() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../oracle/synthetic-rea-b09-visual/media/movie-static.webm"
        ))
        .unwrap()
    }

    /// Deterministic generator for the mutation tests (SplitMix64).
    struct Rng(u64);

    impl Rng {
        fn next(&mut self) -> u64 {
            self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
            let mut z = self.0;
            z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
            z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
            z ^ (z >> 31)
        }

        fn below(&mut self, n: usize) -> usize {
            (self.next() % n.max(1) as u64) as usize
        }
    }

    /// One mutated copy of `base`: bit flips, random bytes, a run of random bytes, a truncation or a removed span.
    fn mutate(base: &[u8], rng: &mut Rng) -> Vec<u8> {
        let mut d = base.to_vec();

        for _ in 0..1 + rng.below(4) {
            match rng.below(6) {
                0 | 1 => {
                    let at = rng.below(d.len());
                    d[at] ^= 1 << rng.below(8);
                }
                2 => {
                    let at = rng.below(d.len());
                    d[at] = rng.next() as u8;
                }
                3 => {
                    let at = rng.below(d.len());
                    let n = rng.below(16).min(d.len() - at);
                    for b in &mut d[at..at + n] {
                        *b = rng.next() as u8;
                    }
                }
                4 => {
                    let keep = rng.below(d.len());
                    d.truncate(keep.max(1));
                }
                _ => {
                    let at = rng.below(d.len());
                    let n = rng.below(32).min(d.len() - at);
                    d.drain(at..at + n);
                    if d.is_empty() {
                        d.push(0);
                    }
                }
            }
        }

        d
    }

    /// One copy of `base` with the bytes of one block (a VP9 frame, mostly its first 24 bytes) mutated.
    fn mutate_block(base: &[u8], rng: &mut Rng) -> Vec<u8> {
        let blocks = demux(base).unwrap().1;
        let b = &blocks[rng.below(blocks.len())];
        let mut d = base.to_vec();
        let len = b.end - b.start;

        for _ in 0..1 + rng.below(3) {
            let span = if rng.below(4) == 0 { len } else { len.min(24) };
            let at = b.start + rng.below(span);

            if rng.below(2) == 0 {
                d[at] ^= 1 << rng.below(8);
            } else {
                d[at] = rng.next() as u8;
            }
        }

        d
    }

    /// Decodes `n` mutations of the synthetic file; returns the inputs that panicked, with the panic message.
    fn panics_in(base: &[u8], seed: u64, n: usize) -> Vec<(String, Vec<u8>)> {
        let mut rng = Rng(seed);
        let mut bad = vec![];

        for i in 0..n {
            if i % 250 == 249 && std::env::var_os("WEBM_FUZZ_DUMP").is_some() {
                eprintln!("{} done, {} panics", i + 1, bad.len());
            }

            let d = if i % 2 == 0 {
                mutate(base, &mut rng)
            } else {
                mutate_block(base, &mut rng)
            };
            let r = std::panic::catch_unwind(|| decode(&d, BUDGET).map(|v| v.frames.len()));

            if let Err(e) = r {
                let msg = e
                    .downcast_ref::<String>()
                    .cloned()
                    .or_else(|| e.downcast_ref::<&str>().map(|s| s.to_string()))
                    .unwrap_or_default();

                bad.push((msg, d));
            }
        }

        bad
    }

    fn big() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/vp9-1024x576-4-tile-columns.webm"
        ))
        .unwrap()
    }

    /// An EBML element with an 8-byte size.
    fn el(id: &[u8], body: &[u8]) -> Vec<u8> {
        let mut v = id.to_vec();

        v.push(1);
        v.extend_from_slice(&(body.len() as u64).to_be_bytes()[1..]);
        v.extend_from_slice(body);
        v
    }

    /// A WebM with one VP9 track (number 1) whose blocks are `frames`, 100 ms apart.
    fn webm_of(frames: &[&[u8]]) -> Vec<u8> {
        let track = [el(&[0xD7], &[1]), el(&[0x83], &[1]), el(&[0x86], b"V_VP9")].concat();
        let mut cluster = el(&[0xE7], &[0]);

        for (i, f) in frames.iter().enumerate() {
            let t = (i as i16 * 100).to_be_bytes();
            let body = [&[0x81, t[0], t[1], 0x80][..], f].concat();

            cluster.extend(el(&[0xA3], &body));
        }

        let segment = [
            el(&[0x16, 0x54, 0xAE, 0x6B], &el(&[0xAE], &track)),
            el(&[0x1F, 0x43, 0xB6, 0x75], &cluster),
        ]
        .concat();

        [
            el(&[0x1A, 0x45, 0xDF, 0xA3], &[]),
            el(&[0x18, 0x53, 0x80, 0x67], &segment),
        ]
        .concat()
    }

    /// The uncompressed header start of a profile 0 key frame that codes `w` x `h`.
    fn key_frame_header(w: u32, h: u32) -> Vec<u8> {
        key_frame_header_of(0, 2, w, h)
    }

    /// The uncompressed header start of a key frame of `profile` (0 to 3) with `color_space`, laid out as in
    /// VP9 section 6.2: `color_space` is coded for every profile, `bit_depth` only for profiles 2 and 3, and the
    /// subsampling bits only for profiles 1 and 3 (RGB codes one reserved bit instead of `color_range`).
    fn key_frame_header_of(profile: u32, color_space: u32, w: u32, h: u32) -> Vec<u8> {
        let mut bits: Vec<bool> = vec![];
        let mut put = |v: u32, n: usize| bits.extend((0..n).rev().map(|i| v >> i & 1 == 1));

        put(2, 2); // frame_marker
        put(profile & 1, 1); // profile_low_bit
        put(profile >> 1, 1); // profile_high_bit

        if profile == 3 {
            put(0, 1); // reserved_zero
        }

        put(0, 1); // show_existing_frame
        put(0, 1); // key frame
        put(1, 1); // show_frame
        put(0, 1); // error_resilient_mode
        put(0x49_8342, 24); // sync code

        if profile >= 2 {
            put(1, 1); // ten_or_twelve_bit
        }

        put(color_space, 3);

        if color_space != 7 {
            put(0, 1); // color_range

            if profile == 1 || profile == 3 {
                put(0, 3); // subsampling_x, subsampling_y, reserved_zero
            }
        } else if profile == 1 || profile == 3 {
            put(0, 1); // reserved_zero
        }

        put(w - 1, 16);
        put(h - 1, 16);

        bits.chunks(8)
            .map(|c| {
                c.iter()
                    .enumerate()
                    .fold(0u8, |a, (i, b)| a | (u8::from(*b) << (7 - i)))
            })
            .collect()
    }

    #[test]
    fn the_size_is_read_from_key_frames_of_every_profile_and_color_space() {
        for profile in 0..4 {
            for color_space in [0, 2, 7] {
                let h = key_frame_header_of(profile, color_space, 640, 360);

                assert_eq!(
                    coded_size(&h),
                    Some((640, 360)),
                    "profile {profile} color_space {color_space}"
                );
                assert_eq!(
                    coded_size(&key_frame_header_of(profile, color_space, 8192, 4353)),
                    Some((8192, 4353))
                );
            }
        }
    }

    #[test]
    fn the_size_is_read_from_real_key_frames_of_profiles_1_2_and_3() {
        // libvpx output: 4:4:4 (profile 1), 10-bit 4:2:0 (profile 2) and 10-bit 4:4:4 (profile 3).
        for p in 1..4 {
            let d = std::fs::read(format!(
                "{}/tests/data/vp9-profile{p}-96x80.webm",
                env!("CARGO_MANIFEST_DIR")
            ))
            .unwrap();
            let blocks = demux(&d).unwrap().1;

            assert!(!blocks.is_empty(), "profile {p}");
            assert_eq!(
                coded_size(&d[blocks[0].start..blocks[0].end]),
                Some((96, 80)),
                "profile {p}"
            );
        }
    }

    #[test]
    fn an_element_header_that_ends_beyond_its_parent_is_skipped_not_sliced() {
        // A cluster of one byte holding a Timecode id; the size byte of that element lies after the cluster. Before
        // this was guarded, `data[body..stop]` was reversed and decoding panicked.
        let mut d = webm_of(&[]);
        let at = d
            .windows(4)
            .position(|w| w == [0x1F, 0x43, 0xB6, 0x75])
            .unwrap();

        d.truncate(at);
        d.extend(el(&[0x1F, 0x43, 0xB6, 0x75], &[]));

        let n = d.len();

        d[n - 8..].copy_from_slice(&[0x01, 0, 0, 0, 0, 0, 0, 1]); // cluster size 1
        d.extend_from_slice(&[0xE7, 0x81, 0x05]);

        // The tracks sit before the cluster: patch the segment size to cover the added bytes.
        let seg = d
            .windows(4)
            .position(|w| w == [0x18, 0x53, 0x80, 0x67])
            .unwrap();
        let len = (d.len() - seg - 12) as u64;

        d[seg + 5..seg + 12].copy_from_slice(&len.to_be_bytes()[1..]);
        assert!(matches!(decode(&d, BUDGET), Err(WebmError::Decode(_))));
    }

    #[test]
    fn the_coded_size_of_every_block_of_the_fixtures_is_read() {
        for (d, size) in [(synthetic(), (128, 128)), (big(), (1024, 576))] {
            let blocks = demux(&d).unwrap().1;

            assert_eq!(coded_size(&d[blocks[0].start..blocks[0].end]), Some(size));
            // The other blocks are inter frames that copy the size of a reference, or key frames again.
            for b in &blocks {
                for f in frames_of(&d[b.start..b.end]) {
                    assert!(matches!(coded_size(f), None | Some(_)));

                    if let Some(s) = coded_size(f) {
                        assert_eq!(s, size);
                    }
                }
            }
        }
    }

    #[test]
    fn a_picture_larger_than_the_limit_is_refused_before_it_is_decoded() {
        let at_limit = key_frame_header(8192, 4352);
        let too_big = key_frame_header(8192, 4353);
        let huge = key_frame_header(65536, 65536);

        assert_eq!(coded_size(&at_limit), Some((8192, 4352)));
        const { assert!(8192u64 * 4352 <= MAX_LUMA_SAMPLES && 8192u64 * 4353 > MAX_LUMA_SAMPLES) };

        for f in [&too_big, &huge] {
            assert!(
                matches!(decode(&webm_of(&[f]), BUDGET), Err(WebmError::TooLarge)),
                "{:?}",
                coded_size(f)
            );
        }

        // The header alone is not a picture: within the limit the decoder is reached and refuses the cut-off frame.
        assert!(matches!(
            decode(&webm_of(&[&at_limit]), BUDGET),
            Err(WebmError::Decode(_))
        ));
        // A superframe is checked frame by frame: the index is [marker, size, marker] after two frames.
        let mut sf = [&at_limit[..], &too_big[..]].concat();
        let marker = 0b1100_0000 | 1; // two frames, one size byte
        let sizes = [at_limit.len() as u8, too_big.len() as u8];

        sf.extend([marker, sizes[0], sizes[1], marker]);
        assert_eq!(frames_of(&sf), vec![&at_limit[..], &too_big[..]]);
        assert!(matches!(
            decode(&webm_of(&[&sf]), BUDGET),
            Err(WebmError::TooLarge)
        ));
    }

    /// A header bit string as bytes (zero padded).
    fn bytes_of(fields: &[(u32, usize)]) -> Vec<u8> {
        let bits: Vec<bool> = fields
            .iter()
            .flat_map(|&(v, n)| (0..n).rev().map(move |i| v >> i & 1 == 1))
            .collect();

        bits.chunks(8)
            .map(|c| {
                c.iter()
                    .enumerate()
                    .fold(0u8, |a, (i, b)| a | (u8::from(*b) << (7 - i)))
            })
            .collect()
    }

    /// An inter frame (profile 0, shown, not error resilient) whose three `found_ref` bits are `found`: when none is
    /// set, `frame_size_with_refs` codes the size itself.
    fn inter_frame_header(found: [u32; 3], w: u32, h: u32) -> Vec<u8> {
        inter_frame_header_with(found, w, h, false)
    }

    /// As [`inter_frame_header`]; with `always_size`, the width and height bits follow even when a `found_ref` bit
    /// is set (the decoder must not read them then).
    fn inter_frame_header_with(found: [u32; 3], w: u32, h: u32, always_size: bool) -> Vec<u8> {
        let mut f = vec![
            (2, 2),      // frame_marker
            (0, 2),      // profile 0
            (0, 1),      // show_existing_frame
            (1, 1),      // non-key frame
            (1, 1),      // show_frame (so no intra_only bit)
            (0, 1),      // error_resilient_mode
            (0, 2),      // reset_frame_context
            (0, 8),      // refresh_frame_flags
            (0 << 1, 4), // ref_frame_idx[0] = 0, sign_bias 0
            (1 << 1, 4), // ref_frame_idx[1] = 1
            (2 << 1, 4), // ref_frame_idx[2] = 2
        ];

        f.extend(found.iter().map(|&b| (b, 1)));

        if found == [0, 0, 0] || always_size {
            f.extend([(w - 1, 16), (h - 1, 16)]);
        }

        f.push((0, 32)); // padding, so that the decoder's header parser does not run out of bits

        bytes_of(&f)
    }

    /// An intra-only frame (profile 0: no color config, hidden so `intra_only` is coded).
    fn intra_only_header(w: u32, h: u32) -> Vec<u8> {
        bytes_of(&[
            (2, 2),
            (0, 2),
            (0, 1),
            (1, 1), // non-key
            (0, 1), // show_frame 0
            (0, 1), // error_resilient_mode
            (1, 1), // intra_only
            (0, 2), // reset_frame_context
            (0x49_8342, 24),
            (0, 8), // refresh_frame_flags
            (w - 1, 16),
            (h - 1, 16),
        ])
    }

    /// `frame_size_with_refs`: found_ref = 0 carries an explicit size, which the cap must see; found_ref = 1 copies the
    /// size of a reference, which an earlier frame coded (and the cap saw).
    #[test]
    fn an_inter_or_intra_only_frame_with_an_explicit_oversized_size_is_refused() {
        let big = inter_frame_header([0, 0, 0], 8192, 4353);

        assert_eq!(coded_size(&big), Some((8192, 4353)));
        assert_eq!(
            coded_size(&inter_frame_header([0, 0, 0], 640, 480)),
            Some((640, 480))
        );

        // The first found_ref bit that is set copies that reference's size and ends the loop: nothing is coded.
        for found in [[1, 0, 0], [0, 1, 0], [0, 0, 1]] {
            assert_eq!(coded_size(&inter_frame_header(found, 1, 1)), None);
        }

        assert!(matches!(
            decode(&webm_of(&[&big]), BUDGET),
            Err(WebmError::TooLarge)
        ));

        let intra = intra_only_header(65536, 65536);

        assert_eq!(coded_size(&intra), Some((65536, 65536)));
        assert!(matches!(
            decode(&webm_of(&[&intra]), BUDGET),
            Err(WebmError::TooLarge)
        ));

        // An oversized inter frame after a valid key frame: the file is refused, not decoded as far as that frame.
        let d = synthetic();
        let blocks = demux(&d).unwrap().1;
        let mut frames: Vec<&[u8]> = blocks.iter().map(|b| &d[b.start..b.end]).collect();

        frames.push(&big);
        assert!(matches!(
            decode(&webm_of(&frames), BUDGET),
            Err(WebmError::TooLarge)
        ));
    }

    /// `frame_size_with_refs` (VP9 section 6.2.5) stops at the first `found_ref` bit that is set and copies the size
    /// of that reference; only when all three are clear is a size coded. This compares `coded_size` with the header
    /// parser of `vp9dec` for every pattern, with an explicit size placed after the bits to catch a parser that reads
    /// it when it should not (`[0, 1, 0]` and `[0, 0, 1]` copy a size; a size after them is not a size).
    #[test]
    fn coded_size_agrees_with_the_decoder_on_every_found_ref_pattern() {
        let mut persist = vp9dec::header::PersistentState::new();

        persist.ref_frame_sizes = [
            (111, 11),
            (222, 22),
            (333, 33),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
            (0, 0),
        ];

        for n in 0..8u32 {
            let found = [n >> 2 & 1, n >> 1 & 1, n & 1];
            let frame = inter_frame_header_with(found, 8192, 4353, true);
            let (parsed, _) = vp9dec::header::parse_uncompressed_header(&frame, &persist).unwrap();
            let vp9dec::header::FrameHeader::New(h) = parsed else {
                panic!("not a new frame");
            };

            match coded_size(&frame) {
                Some(s) => {
                    assert_eq!(found, [0, 0, 0], "{found:?}");
                    assert_eq!(s, (h.width, h.height), "{found:?}");
                    assert_eq!(s, (8192, 4353));
                }
                None => {
                    assert_ne!(found, [0, 0, 0], "{found:?}");

                    // The decoder copied the size of the first reference whose bit is set, not the coded one.
                    let first = found.iter().position(|&b| b == 1).unwrap();

                    assert_eq!((h.width, h.height), persist.ref_frame_sizes[first]);
                }
            }
        }
    }

    /// The invariant behind the cap: whatever the bytes of a frame header, if the decoder's own header parser accepts
    /// the frame and finds a picture larger than `MAX_LUMA_SAMPLES`, then `coded_size` reports that very size (so
    /// `decode` refuses the frame before the decoder runs). Random single-frame headers of every kind (key frames of
    /// the four profiles and the color spaces, inter frames of every `found_ref` pattern, intra-only frames), with
    /// random bit flips and cut-offs. The reference slots all hold sizes within the cap, as they do in `decode`, where
    /// a frame larger than the cap is never decoded and so never becomes a reference.
    #[test]
    fn no_header_the_decoder_parses_as_oversized_escapes_the_cap() {
        let mut rng = Rng(0xCA9);
        let mut persist = vp9dec::header::PersistentState::new();

        persist.ref_frame_sizes = [(1920, 1080); 8];

        let (mut oversized, mut accepted) = (0, 0);

        for i in 0..300_000 {
            let (w, h) = if rng.below(2) == 0 {
                (1 + rng.below(65536) as u32, 1 + rng.below(65536) as u32)
            } else {
                (8192, 4350 + rng.below(8) as u32)
            };
            let mut f = match i % 4 {
                0 => key_frame_header_of(rng.below(4) as u32, [0, 2, 7][rng.below(3)], w, h),
                1 => inter_frame_header_with(
                    [[0, 0, 0], [1, 0, 0], [0, 1, 0], [0, 0, 1]][rng.below(4)],
                    w,
                    h,
                    rng.below(2) == 0,
                ),
                2 => intra_only_header(w, h),
                _ => key_frame_header(w, h),
            };

            f.extend([0u8; 8]);

            for _ in 0..rng.below(4) {
                let at = rng.below(f.len());

                f[at] ^= 1 << rng.below(8);
            }

            if rng.below(8) == 0 {
                f.truncate(rng.below(f.len()));
            }

            let Ok((vp9dec::header::FrameHeader::New(parsed), _)) =
                vp9dec::header::parse_uncompressed_header(&f, &persist)
            else {
                continue;
            };

            accepted += 1;

            if u64::from(parsed.width) * u64::from(parsed.height) > MAX_LUMA_SAMPLES {
                oversized += 1;

                assert_eq!(
                    coded_size(&f),
                    Some((parsed.width, parsed.height)),
                    "{f:02X?}"
                );
                assert!(matches!(
                    decode(&webm_of(&[&f]), BUDGET),
                    Err(WebmError::TooLarge)
                ));
            }
        }

        // Not vacuous: many frames parse, and many of those are oversized.
        assert!(
            accepted > 50_000 && oversized > 5_000,
            "{accepted} {oversized}"
        );
    }

    /// Every frame of real libvpx files, inter frames included, goes through the header parser of `vp9dec` with the
    /// reference sizes it keeps (updated by `refresh_frame_flags` after each frame). `coded_size` must agree: where it
    /// reports a size, it is the one the decoder parsed; where it reports none (a size copied from a reference), the
    /// decoder parsed a non-intra frame. A misread of `reset_frame_context` (two bits, VP9 section 6.2) or of any
    /// other field before the `found_ref` bits would show here, because real inter frames set those bits.
    #[test]
    fn coded_size_agrees_with_the_decoder_on_the_frames_of_real_files() {
        let sf = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/vp9-superframe-2-frames.webm"
        ))
        .unwrap();
        let (mut keys, mut inters) = (0, 0);

        for d in [synthetic(), big(), sf] {
            let mut persist = vp9dec::header::PersistentState::new();

            for b in demux(&d).unwrap().1 {
                for f in frames_of(&d[b.start..b.end]) {
                    let (parsed, _) =
                        vp9dec::header::parse_uncompressed_header(f, &persist).unwrap();
                    let vp9dec::header::FrameHeader::New(h) = parsed else {
                        panic!("show_existing_frame in a fixture");
                    };

                    match coded_size(f) {
                        Some(s) => {
                            assert_eq!(s, (h.width, h.height));
                            keys += 1;
                        }
                        None => {
                            assert!(!h.frame_is_intra);
                            inters += 1;
                        }
                    }

                    for (slot, size) in persist.ref_frame_sizes.iter_mut().enumerate() {
                        if h.refresh_frame_flags >> slot & 1 == 1 {
                            *size = (h.width, h.height);
                        }
                    }
                }
            }
        }

        assert!(
            keys >= 3 && inters >= 10,
            "{keys} key frames, {inters} inter frames"
        );
    }

    /// A superframe that an independent splitter cut first. The file is two libvpx frames (a key frame of 2845 bytes
    /// and an inter frame of 429) joined by an index in the layout of VP9 Annex B and libvpx (marker, the sizes in
    /// frame order, little-endian, marker); ffmpeg's `vp9_superframe_split` bitstream filter cuts it into packets of
    /// 2845 and 429 bytes. Run `ffmpeg -i vp9-superframe-2-frames.webm -c copy -bsf:v vp9_superframe_split -f ivf -`
    /// to reproduce that.
    #[test]
    fn a_superframe_is_cut_where_ffmpeg_cuts_it() {
        let d = std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/tests/data/vp9-superframe-2-frames.webm"
        ))
        .unwrap();
        let blocks = demux(&d).unwrap().1;

        assert_eq!(blocks.len(), 1);

        let chunk = &d[blocks[0].start..blocks[0].end];
        let lens: Vec<usize> = frames_of(chunk).iter().map(|f| f.len()).collect();

        assert_eq!(lens, [2845, 429]);
        assert_eq!(coded_size(frames_of(chunk)[0]), Some((128, 96)));

        let v = decode(&d, BUDGET).unwrap();

        assert_eq!((v.width, v.height, v.frames.len()), (128, 96, 1));
    }

    /// The size check looks at the frames `frames_of` cuts; the decoder decodes the frames it cuts itself. They must
    /// be the same for every chunk, well formed or not: random chunks that end in each of the 32 marker bytes, with
    /// the marker repeated where the index would start, sizes that do not add up, and counts and widths of every kind.
    #[test]
    fn frames_of_cuts_every_chunk_where_the_decoder_cuts_it() {
        let mut rng = Rng(0x5EED);
        let (mut split, mut whole) = (0, 0);

        for i in 0..400_000 {
            let n = rng.below(48);
            let mut c: Vec<u8> = (0..n).map(|_| [0, 1, 2, 3, 7, 255][rng.below(6)]).collect();

            if i % 3 != 0 && !c.is_empty() {
                let marker = 0xC0 | rng.below(32) as u8;
                let last = c.len() - 1;
                let index = 2 + ((marker >> 3 & 3) as usize + 1) * ((marker & 7) as usize + 1);

                c[last] = marker;

                if i % 3 == 1 && c.len() >= index {
                    let first = c.len() - index;

                    c[first] = marker;
                }
            }

            let ours = frames_of(&c);

            assert_eq!(ours, vp9dec::superframe::split_superframe(&c), "{c:?}");

            if ours.len() > 1 {
                split += 1;
            } else {
                whole += 1;
            }
        }

        // Both outcomes are well represented, so the comparison is not vacuous.
        assert!(split > 1000 && whole > 1000, "{split} {whole}");
    }

    /// Superframe sizes are little-endian (VP9 Annex B): `[1, 0]` is 1 and `[0, 1]` is 256.
    #[test]
    fn superframe_sizes_are_little_endian() {
        let mut second = key_frame_header(8192, 4353);

        second.resize(256, 0);

        let first = [0u8];
        let marker = 0b1100_0000 | (1 << 3) | 1; // two frames, two size bytes
        let mut sf = [&first[..], &second[..]].concat();

        sf.extend([marker, 1, 0, 0, 1, marker]);

        let frames = frames_of(&sf);

        assert_eq!(frames.len(), 2);
        assert_eq!((frames[0].len(), frames[1].len()), (1, 256));
        assert_eq!(frames[1], &second[..]);
        // Big-endian reading would give 256 then 1: the first frame would be the zero byte plus the header start.
        assert_eq!(coded_size(frames[1]), Some((8192, 4353)));
        assert!(matches!(
            decode(&webm_of(&[&sf]), BUDGET),
            Err(WebmError::TooLarge)
        ));

        // Three size bytes and a size that does not fit: the chunk stays whole.
        let bad = [
            &[0u8; 4][..],
            &[
                0b1100_0000 | (2 << 3) | 1,
                9,
                0,
                0,
                1,
                0,
                0,
                0b1100_0000 | (2 << 3) | 1,
            ][..],
        ]
        .concat();

        assert_eq!(frames_of(&bad), vec![&bad[..]]);
    }

    #[test]
    fn the_pictures_may_not_take_more_than_the_budget() {
        let d = synthetic();
        let one = 128 * 128 * 4;

        assert!(matches!(decode(&d, 3 * one), Err(WebmError::TooLarge)));
        assert!(matches!(decode(&d, 10 * one - 1), Err(WebmError::TooLarge)));
        assert_eq!(decode(&d, 10 * one).unwrap().frames.len(), 10);
    }

    /// Every cut of the file, container and frames alike, is an error or fewer pictures, never a panic.
    #[test]
    fn a_truncated_file_never_panics() {
        for d in [synthetic(), big()] {
            let step = (d.len() / 40).max(1);
            let mut ok = 0;

            for cut in (0..d.len()).step_by(step).chain([d.len() - 1]) {
                match std::panic::catch_unwind(|| decode(&d[..cut], BUDGET)) {
                    Ok(Ok(_)) => ok += 1,
                    Ok(Err(_)) => {}
                    Err(_) => panic!("panicked on a file cut at {cut} of {}", d.len()),
                }
            }

            assert!(ok > 0, "no cut decoded a picture");
        }
    }

    #[test]
    fn mutated_files_never_panic() {
        let bad = panics_in(&synthetic(), 7, 120);

        assert!(
            bad.is_empty(),
            "{:?}",
            bad.iter().map(|b| &b.0).collect::<Vec<_>>()
        );
    }

    /// 1024 x 576, four tile columns: the tile-parallel decode and the loop-filter wavefront run.
    #[test]
    fn mutated_multi_tile_files_never_panic() {
        let bad = panics_in(&big(), 9, 25);

        assert!(
            bad.is_empty(),
            "{:?}",
            bad.iter().map(|b| &b.0).collect::<Vec<_>>()
        );
    }

    #[test]
    #[ignore = "measurement: WEBM_FILE=<path>, run in release with /usr/bin/time -v"]
    fn decode_a_file() {
        let d = std::fs::read(std::env::var_os("WEBM_FILE").unwrap()).unwrap();
        let t = std::time::Instant::now();
        let r = decode(&d, BUDGET);

        eprintln!(
            "{:?} in {:?}",
            r.map(|v| (v.width, v.height, v.frames.len())),
            t.elapsed()
        );
    }

    #[test]
    #[ignore = "exploration: WEBM_FUZZ_ITERS mutations from WEBM_FUZZ_SEED"]
    fn explore_mutations() {
        let n = std::env::var("WEBM_FUZZ_ITERS")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(2000);
        let seed = std::env::var("WEBM_FUZZ_SEED")
            .ok()
            .and_then(|v| v.parse().ok())
            .unwrap_or(1);
        let base = match std::env::var_os("WEBM_FUZZ_BASE") {
            Some(p) => std::fs::read(p).unwrap(),
            None => synthetic(),
        };
        let bad = panics_in(&base, seed, n);
        let mut kinds = std::collections::BTreeMap::new();

        for (m, _) in &bad {
            *kinds.entry(m.clone()).or_insert(0) += 1;
        }

        eprintln!("{} of {n} panicked", bad.len());
        for (m, c) in &kinds {
            eprintln!("{c:5}  {m}");
        }
        if let Some(dir) = std::env::var_os("WEBM_FUZZ_DUMP") {
            for (i, (_, d)) in bad.iter().enumerate().take(20) {
                std::fs::write(std::path::Path::new(&dir).join(format!("p{i}.webm")), d).unwrap();
            }
        }
    }

    #[test]
    fn a_valid_superframe_decodes_like_its_frames_decoded_one_after_the_other() {
        // `vp9dec` splits a superframe itself (`Decoder::decode_frame` calls `split_superframe`); this proves that
        // `frames_of` cuts it in the same places and that the whole chunk, not a piece, is what reaches the decoder.
        let d = synthetic();
        let blocks = demux(&d).unwrap().1;
        let (a, b) = (
            &d[blocks[0].start..blocks[0].end],
            &d[blocks[1].start..blocks[1].end],
        );

        assert_eq!(frames_of(a).len(), 1);
        assert_eq!(frames_of(b).len(), 1);

        // Two sizes of 4 bytes each: marker 0b110_11_001.
        let marker = 0b1101_1001u8;
        let mut sf = [a, b].concat();

        sf.push(marker);
        sf.extend((a.len() as u32).to_le_bytes());
        sf.extend((b.len() as u32).to_le_bytes());
        sf.push(marker);

        assert_eq!(frames_of(&sf), vec![a, b]);
        assert_eq!(
            vp9dec::superframe::split_superframe(&sf),
            vec![a, b],
            "the splitter of the decoder cuts the same frames"
        );

        let apart = decode(&webm_of(&[a, b]), BUDGET).unwrap();
        let together = decode(&webm_of(&[&sf]), BUDGET).unwrap();

        assert_eq!(apart.frames.len(), 2);
        // The decoder surfaces the last shown picture of a chunk, which is the second frame.
        assert_eq!(together.frames.len(), 1);
        assert_eq!((together.width, together.height), (128, 128));
        assert_eq!(together.frames[0].rgba, apart.frames[1].rgba);
    }

    #[test]
    fn the_synthetic_webm_decodes_to_ten_frames_of_128_by_128() {
        let v = decode(&synthetic(), BUDGET).unwrap();

        assert_eq!((v.width, v.height, v.frames.len()), (128, 128, 10));
        assert_eq!(
            v.frames.iter().map(|f| f.time_ms).collect::<Vec<_>>(),
            (0..10).map(|i| i as f64 * 100.0).collect::<Vec<_>>()
        );
        assert!(v.frames.iter().all(|f| f.rgba.len() == 128 * 128 * 4));

        // The first checker square, away from its edges: the blue the page shows for this file (BT.709 upload).
        let px = &v.frames[0].rgba[(16 * 128 + 16) * 4..][..4];

        assert_eq!(px[3], 255);
        assert!(px[2] > px[0], "blue square, got {px:?}");
    }

    #[test]
    fn data_that_is_not_a_vp9_webm_is_refused() {
        assert!(matches!(
            decode(b"not a webm", BUDGET),
            Err(WebmError::NotWebm)
        ));
        // The EBML header and a segment with no tracks.
        let mut d = synthetic();

        d.truncate(40);
        assert!(decode(&d, BUDGET).is_err());
        assert!(super::super::nodes::movie_texture::MovieMedia::decode_webm(b"x").is_none());
    }

    // ---- VP8 ----

    fn codec_fixture(name: &str) -> (Vec<u8>, Vec<Vec<String>>) {
        let dir = concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../oracle/synthetic-movie-codecs/media/"
        );
        let webm = std::fs::read(format!("{dir}{name}.webm")).unwrap();
        let hashes = std::fs::read_to_string(format!("{dir}{name}.planes.tsv")).unwrap();

        (
            webm,
            hashes
                .lines()
                .map(|l| l.split('\t').skip(1).map(String::from).collect())
                .collect(),
        )
    }

    fn sha(bytes: &[u8]) -> String {
        use sha2::{Digest, Sha256};

        Sha256::digest(bytes)
            .iter()
            .map(|b| format!("{b:02x}"))
            .collect()
    }

    const VP8_FIXTURES: [(&str, usize, usize); 6] = [
        ("vp8-testsrc", 96, 64),
        ("vp8-crop", 50, 34),
        ("vp8-parts", 128, 80),
        ("vp8-profile1", 64, 48),
        ("vp8-profile2", 64, 48),
        ("vp8-profile3", 64, 48),
    ];

    fn vp8_pictures(webm: &[u8]) -> Vec<(f64, vp8::Picture)> {
        let (codec, blocks) = demux(webm).unwrap();
        let mut decoder = vp8::Vp8Decoder::new(vp8::Limits {
            max_width: 16383,
            max_height: 16383,
            max_allocation_bytes: 1 << 30,
        });

        assert_eq!(codec, Codec::Vp8);

        blocks
            .iter()
            .filter_map(|b| {
                decoder
                    .decode(&webm[b.start..b.end])
                    .unwrap()
                    .map(|p| (b.time_ms, p))
            })
            .collect()
    }

    /// BR1.1: every picture of every stream equals what ffmpeg's decoder (bit-exact with libvpx) outputs, plane by
    /// plane. The streams cover key and inter frames, hidden frames, a size that is not a multiple of 16, four
    /// token partitions and bitstream versions 1 to 3 (bilinear prediction, simple loop filter, full-pixel chroma).
    #[test]
    fn vp8_pictures_equal_the_reference_decoder_plane_by_plane() {
        for (name, w, h) in VP8_FIXTURES {
            let (webm, expected) = codec_fixture(name);
            let pictures = vp8_pictures(&webm);

            assert_eq!(pictures.len(), expected.len(), "{name}: picture count");

            for (i, ((_, p), want)) in pictures.iter().zip(&expected).enumerate() {
                assert_eq!((p.width, p.height), (w, h), "{name} {i}");

                for (k, plane) in p.planes.iter().enumerate() {
                    assert_eq!(&sha(plane), &want[k], "{name}: picture {i}, plane {k}");
                }
            }
        }
    }

    #[test]
    fn a_vp8_webm_decodes_to_rgba_pictures_with_their_times() {
        let (webm, expected) = codec_fixture("vp8-testsrc");
        let v = decode(&webm, BUDGET).unwrap();

        assert_eq!(
            (v.width, v.height, v.frames.len()),
            (96, 64, expected.len())
        );
        assert_eq!(v.frames[0].time_ms, 0.0);
        assert!(v.frames.windows(2).all(|w| w[0].time_ms < w[1].time_ms));
        assert!(v.frames.iter().all(|f| f.rgba.len() == 96 * 64 * 4));
        assert!(
            v.frames
                .iter()
                .all(|f| f.rgba.chunks(4).all(|p| p[3] == 255))
        );
        // The pictures are not all one color: testsrc2 draws bars and a moving counter.
        assert_ne!(v.frames[0].rgba, v.frames[v.frames.len() - 1].rgba);
    }
}
