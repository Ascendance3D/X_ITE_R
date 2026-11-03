//! A minimal WebM (Matroska subset) demuxer and the VP9 decode of its first video track, for MovieTexture.
//!
//! Only what a MovieTexture needs: the first video track whose CodecID is `V_VP9`, its blocks in file order with their
//! presentation times in milliseconds, and the frame size. Lacing is not supported (WebM video does not use it).
//! Decoding is the `vp9dec` crate (MIT, no dependencies); the picture is converted to RGBA the way a browser's video
//! texture upload does (see [`yuv_to_rgba`]).
//!
//! Limits. [`decode`] takes `max_bytes`, which caps the RGBA pictures that are *kept* (and is checked per picture,
//! before its conversion). The decoder's own working memory (the reference pictures of `vp9dec`, about 1.5 u16 per
//! luma sample each, up to eight held) is bounded by [`MAX_LUMA_SAMPLES`], which is checked from the frame headers
//! before the decoder runs; it is not bounded by `max_bytes`. A fixed 8192 x 4352 picture measured 860 MB peak.

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
    NoVp9Track,
    Decode(String),
    /// A picture or the decoded frames exceed the limits ([`MAX_LUMA_SAMPLES`], the byte budget of [`decode`]).
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
            let byte = *self.d.get(self.at / 8)?;

            v = (v << 1) | u32::from(byte >> (7 - self.at % 8) & 1);
            self.at += 1;
        }

        Some(v)
    }
}

/// The frames of a chunk: a superframe (VP9 Annex B) is its frames, any other chunk is one frame.
fn frames_of(chunk: &[u8]) -> Vec<&[u8]> {
    let Some(&last) = chunk.last() else {
        return vec![];
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
/// inter frame that does not copy the size of a reference), `None` for any other frame or one that is cut off. A size
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

/// The blocks of the first VP9 video track of `data`, in file order.
fn demux(data: &[u8]) -> Result<Vec<Block>, WebmError> {
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
                    track = Some(number);
                }
            }
        }
    }

    let track = track.ok_or(WebmError::NoVp9Track)?;
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

    Ok(blocks)
}

/// Demux `data`: the width/height are known only after decoding. The pictures are RGBA in memory, so `max_bytes` is
/// the most that all of them may take together; a picture larger than [`MAX_LUMA_SAMPLES`] is not decoded.
pub fn decode(data: &[u8], max_bytes: u64) -> Result<Video, WebmError> {
    let blocks = demux(data)?;
    let mut decoder = Decoder::new();
    let mut frames = vec![];
    let (mut width, mut height) = (0, 0);
    let mut bytes = 0u64;

    for b in &blocks {
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
        // the header checks are what bound a hostile file.
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
            bytes = bytes.saturating_add(u64::from(width) * u64::from(height) * 4);

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
fn yuv_to_rgba(w: usize, h: usize, sx: bool, sy: bool, y: &[u8], u: &[u8], v: &[u8]) -> Vec<u8> {
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
            let c = 1.164_383 * (yy - 16.0);

            out.extend_from_slice(&[
                clamp(c + 1.792_741 * vv),
                clamp(c - 0.213_249 * uu - 0.532_909 * vv),
                clamp(c + 2.112_402 * uu),
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
        let blocks = demux(base).unwrap();
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
        let mut bits: Vec<bool> = vec![];
        let mut put = |v: u32, n: usize| bits.extend((0..n).rev().map(|i| v >> i & 1 == 1));

        put(2, 2); // frame_marker
        put(0, 2); // profile 0
        put(0, 1); // show_existing_frame
        put(0, 1); // key frame
        put(1, 1); // show_frame
        put(0, 1); // error_resilient_mode
        put(0x49_8342, 24); // sync code
        put(2, 3); // color_space
        put(0, 1); // color_range
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
            let blocks = demux(&d).unwrap();

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
        let mut f = vec![
            (2, 2), // frame_marker
            (0, 2), // profile 0
            (0, 1), // show_existing_frame
            (1, 1), // non-key frame
            (1, 1), // show_frame (so no intra_only bit)
            (0, 1), // error_resilient_mode
            (0, 2), // reset_frame_context
            (0, 8), // refresh_frame_flags
            (0, 4), // ref_frame_idx[0], sign_bias
            (1, 4), // ref_frame_idx[1], sign_bias
            (2, 4), // ref_frame_idx[2], sign_bias
        ];

        f.extend(found.iter().map(|&b| (b, 1)));

        if found == [0, 0, 0] {
            f.extend([(w - 1, 16), (h - 1, 16)]);
        }

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

        // Each of the three found_ref positions copies a reference size: nothing is coded.
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
        let blocks = demux(&d).unwrap();
        let mut frames: Vec<&[u8]> = blocks.iter().map(|b| &d[b.start..b.end]).collect();

        frames.push(&big);
        assert!(matches!(
            decode(&webm_of(&frames), BUDGET),
            Err(WebmError::TooLarge)
        ));
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
}
