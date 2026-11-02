//! A minimal WebM (Matroska subset) demuxer and the VP9 decode of its first video track, for MovieTexture.
//!
//! Only what a MovieTexture needs: the first video track whose CodecID is `V_VP9`, its blocks in file order with their
//! presentation times in milliseconds, and the frame size. Lacing is not supported (WebM video does not use it).
//! Decoding is the `vp9dec` crate (MIT, no dependencies); the picture is converted to RGBA the way a browser's video
//! texture upload does (see [`yuv_to_rgba`]).

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

/// Why a file is not decoded.
#[derive(Debug)]
pub enum WebmError {
    NotWebm,
    NoVp9Track,
    Decode(String),
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

/// Demux `data`: the width/height are known only after decoding.
pub fn decode(data: &[u8]) -> Result<Video, WebmError> {
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

    let mut decoder = Decoder::new();
    let mut frames = vec![];
    let (mut width, mut height) = (0, 0);

    for b in &blocks {
        let shown = decoder
            .decode_frame(&data[b.start..b.end])
            .map_err(|e| WebmError::Decode(format!("{e:?}")))?;

        for f in shown.into_iter().filter_map(|d| d.frame) {
            let (PlaneData::U8(y), PlaneData::U8(u), PlaneData::U8(v)) = (&f.y, &f.u, &f.v) else {
                return Err(WebmError::Decode("not an 8-bit stream".into()));
            };

            width = f.width as u32;
            height = f.height as u32;
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

    fn synthetic() -> Vec<u8> {
        std::fs::read(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../oracle/synthetic-rea-b09-visual/media/movie-static.webm"
        ))
        .unwrap()
    }

    #[test]
    fn the_synthetic_webm_decodes_to_ten_frames_of_128_by_128() {
        let v = decode(&synthetic()).unwrap();

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
        assert!(matches!(decode(b"not a webm"), Err(WebmError::NotWebm)));
        // The EBML header and a segment with no tracks.
        let mut d = synthetic();

        d.truncate(40);
        assert!(decode(&d).is_err());
        assert!(super::super::nodes::movie_texture::MovieMedia::decode_webm(b"x").is_none());
    }
}
