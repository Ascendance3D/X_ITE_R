//! The WebM (Matroska subset) container structure: EBML elements, the Segment, the tracks and the blocks with their
//! times. No codec, so it is in every build (the browser build too, where the page's `<video>` element decodes the
//! pictures): [`crate::webm`] demuxes the pictures with it, and the browser movie sound (Phase 9A) reads the start of
//! the audio track with [`first_block_times`].

use crate::video_types::WebmError;

pub(crate) const ID_EBML: u64 = 0x1A45_DFA3;
pub(crate) const ID_SEGMENT: u64 = 0x1853_8067;
pub(crate) const ID_INFO: u64 = 0x1549_A966;
pub(crate) const ID_TIMECODE_SCALE: u64 = 0x2A_D7B1;
pub(crate) const ID_TRACKS: u64 = 0x1654_AE6B;
pub(crate) const ID_TRACK_ENTRY: u64 = 0xAE;
pub(crate) const ID_TRACK_NUMBER: u64 = 0xD7;
pub(crate) const ID_TRACK_TYPE: u64 = 0x83;
pub(crate) const ID_CLUSTER: u64 = 0x1F43_B675;
pub(crate) const ID_TIMECODE: u64 = 0xE7;
pub(crate) const ID_SIMPLE_BLOCK: u64 = 0xA3;
pub(crate) const ID_BLOCK_GROUP: u64 = 0xA0;
pub(crate) const ID_BLOCK: u64 = 0xA1;

/// An element id (marker bits kept) and its size (`None`: unknown size), and the header length.
pub(crate) fn element(d: &[u8], at: usize) -> Option<(u64, Option<usize>, usize)> {
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

pub(crate) fn uint(b: &[u8]) -> u64 {
    b.iter().fold(0, |a, x| (a << 8) | *x as u64)
}

/// The children of the master element in `d[start..end]`: (id, body range).
pub(crate) fn children(d: &[u8], start: usize, end: usize) -> Vec<(u64, usize, usize)> {
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

/// Child elements: (id, body start, body end).
pub(crate) type Children = Vec<(u64, usize, usize)>;

/// A block of a track: its time, and its frame data `start..end` (read only by the native picture demux).
#[cfg_attr(not(feature = "native-video"), allow(dead_code))]
pub(crate) struct Block {
    pub(crate) time_ms: f64,
    pub(crate) start: usize,
    pub(crate) end: usize,
}

/// The children of the Segment of `data`, and its TimecodeScale (ns per tick, 1 ms by default).
pub(crate) fn segment_of(data: &[u8]) -> Result<(Children, u64), WebmError> {
    let top = children(data, 0, data.len());

    if top.first().map(|t| t.0) != Some(ID_EBML) {
        return Err(WebmError::NotWebm);
    }

    let Some(&(_, seg_start, seg_end)) = top.iter().find(|t| t.0 == ID_SEGMENT) else {
        return Err(WebmError::NotWebm);
    };
    let segment = children(data, seg_start, seg_end);
    let mut scale = 1_000_000u64;

    for &(id, s, e) in &segment {
        if id == ID_INFO {
            for (cid, cs, ce) in children(data, s, e) {
                if cid == ID_TIMECODE_SCALE {
                    scale = uint(&data[cs..ce]).max(1);
                }
            }
        }
    }

    Ok((segment, scale))
}

/// Every SimpleBlock and Block of the Clusters of `segment`, in file order: `f(track number, flags, block)`, where the
/// block's time is the cluster time plus the block's relative time, in ms. A block too short for its header is left
/// out. `f` returns false to stop.
pub(crate) fn each_block(
    data: &[u8],
    segment: &[(u64, usize, usize)],
    scale: u64,
    mut f: impl FnMut(u64, u8, Block) -> bool,
) {
    for &(id, s, e) in segment {
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

            // track number, 16-bit relative timecode, flags.
            let header = nlen + 3;

            if be < bs + header || data.len() < bs + header {
                continue;
            }

            let rel = i16::from_be_bytes([data[bs + nlen], data[bs + nlen + 1]]) as i64;
            // A damaged cluster time cannot overflow.
            let ticks = (cluster_time as i64).saturating_add(rel);
            let block = Block {
                time_ms: ticks as f64 * scale as f64 / 1_000_000.0,
                start: bs + header,
                end: be,
            };

            if !f(number, data[bs + nlen + 2], block) {
                return;
            }
        }
    }
}

/// The times in seconds of the first block of the first video track and of the first audio track of a WebM (None for
/// a track that is not there or has no block), or None when `data` is not a WebM. A browser's `decodeAudioData` gives
/// the samples from the first one, so the first audio block time is the leading silence of the sound on the movie
/// time (measured in Chromium: a file whose audio starts 0.5 s after the video decodes to 0.5 s less sound, and the
/// element plays that sound from 0.5 s).
pub fn first_block_times(data: &[u8]) -> Option<(Option<f64>, Option<f64>)> {
    let (segment, scale) = segment_of(data).ok()?;
    let (mut video, mut audio) = (None, None);

    for &(id, s, e) in &segment {
        if id != ID_TRACKS {
            continue;
        }

        for (tid, ts, te) in children(data, s, e) {
            if tid != ID_TRACK_ENTRY {
                continue;
            }

            let (mut number, mut kind) = (0, 0);

            for (cid, cs, ce) in children(data, ts, te) {
                match cid {
                    ID_TRACK_NUMBER => number = uint(&data[cs..ce]),
                    ID_TRACK_TYPE => kind = uint(&data[cs..ce]),
                    _ => {}
                }
            }

            match kind {
                1 if video.is_none() => video = Some(number),
                2 if audio.is_none() => audio = Some(number),
                _ => {}
            }
        }
    }

    let (mut video_time, mut audio_time) = (None::<f64>, None::<f64>);

    each_block(data, &segment, scale, |number, _, block| {
        let t = block.time_ms / 1000.0;

        // The first block of a track in file order; a later block with an earlier time (B-frames) is taken too.
        if Some(number) == video {
            video_time = Some(video_time.map_or(t, |v| v.min(t)));
        }
        if Some(number) == audio {
            audio_time = Some(audio_time.map_or(t, |a| a.min(t)));
        }

        true
    });

    Some((video_time, audio_time))
}

/// A track number: an EBML variable-length integer with the marker removed.
pub(crate) fn element_number(d: &[u8], at: usize) -> Option<(u64, (), usize)> {
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

#[cfg(test)]
mod tests {
    use super::*;

    fn websound(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/data/websound/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    /// The first block times of the synthetic files of oracle/tools/gen_movie_websound.py: the audio of the late files
    /// starts 0.5 s after the video, within one audio block (ffprobe gives Opus 0.494, the block time less the 6.5 ms
    /// CodecDelay, and Vorbis 0.497; the Chromium element plays the sound from 0.499 and 0.501). A file without audio has
    /// no audio time. The demux of the pictures (webm.rs) uses the same
    /// block walk.
    #[test]
    fn the_first_block_times_of_video_and_audio() {
        for (name, video, audio) in [
            ("vp8-opus.webm", 0.0, Some(0.0)),
            ("vp8-vorbis.webm", 0.003, Some(0.0)),
            ("vp8-opus-late.webm", 0.0, Some(0.501)),
            ("vp8-vorbis-late.webm", 0.0, Some(0.497)),
            ("vp8-only.webm", 0.0, None),
        ] {
            let (v, a) = first_block_times(&websound(name)).unwrap();

            assert_eq!(v, Some(video), "{name}: video");
            assert_eq!(a, audio, "{name}: audio");
        }
    }

    /// Not a WebM: None. Every cut of a file, and damaged bytes, give times or None, never a panic.
    #[test]
    fn first_block_times_of_cut_or_damaged_data_never_panic() {
        assert_eq!(first_block_times(b""), None);
        assert_eq!(first_block_times(b"OggS\0\0\0\0"), None);

        let data = websound("vp8-vorbis-late.webm");

        for end in (0..data.len()).step_by(31) {
            let _ = first_block_times(&data[..end]);
        }

        for at in (0..data.len()).step_by(13) {
            let mut bad = data.clone();

            bad[at] ^= 0xFF;
            let _ = first_block_times(&bad);
        }

        // A block cut inside its header (track number, then 1 of the 3 header bytes) is left out.
        let mut short = data.clone();
        let (segment, scale) = segment_of(&data).unwrap();
        let mut first = None;

        each_block(&data, &segment, scale, |_, _, b| {
            first = Some(b);
            false
        });

        let first = first.unwrap();

        short.truncate(first.start - 2);
        assert!(matches!(first_block_times(&short), Some((None, None))));
    }
}
