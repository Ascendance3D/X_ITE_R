//! The structure of an MP4 file (ISO/IEC 14496-12 boxes) without any codec: the tracks, their sample-entry codes,
//! sizes, rates and time scales (ADR 0015). The engine names the codec of a file with it before a platform decoder
//! runs, so an MP4 is never taken to be H.264, and a build without a decoder can say which codec it cannot decode.
//!
//! Every box size is checked against the data before it is used. The walk follows only the fixed path
//! moov/trak/mdia/minf/stbl/stsd and stops at [`MAX_BOXES`] boxes, so damaged data gives an error, never a panic or a
//! long loop.

/// The most boxes the walk reads.
pub const MAX_BOXES: usize = 100_000;

/// Why data is not read as an MP4.
#[derive(Debug, PartialEq)]
pub enum Mp4Error {
    /// The first box is not a box that starts an MP4 or QuickTime file.
    NotMp4,
    /// A box is cut, has an impossible size, or the file has no `moov` box.
    Malformed(&'static str),
}

impl std::fmt::Display for Mp4Error {
    fn fmt(&self, f: &mut std::fmt::Formatter) -> std::fmt::Result {
        match self {
            Mp4Error::NotMp4 => write!(f, "not an MP4 file"),
            Mp4Error::Malformed(why) => write!(f, "damaged MP4 file ({why})"),
        }
    }
}

/// The kind of a track (`hdlr` handler type).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum TrackKind {
    Video,
    Audio,
    /// Any other handler (`text`, `subt`, `hint`, `meta` …).
    Other([u8; 4]),
}

/// One track of an MP4.
#[derive(Clone, Debug, PartialEq)]
pub struct Track {
    pub kind: TrackKind,
    /// The code of the first sample entry (`avc1`, `hvc1`, `av01`, `mp4a` …), `None` without a sample description.
    pub codec: Option<[u8; 4]>,
    /// Video: the sample entry's width and height.
    pub width: u16,
    pub height: u16,
    /// Audio: the sample entry's channel count and sample rate (the integer part of the 16.16 value).
    pub channels: u16,
    pub sample_rate: u32,
    /// `mdhd` time scale (units a second); 0 when absent.
    pub timescale: u32,
    /// `mdhd` duration in seconds, when the time scale is not 0.
    pub duration: Option<f64>,
    /// The track has an edit list (`edts/elst`): its presentation does not start at its first sample's time.
    pub edit_list: bool,
    /// The empty edits at the start of the edit list, in seconds: the presentation time of the track's first sample
    /// (0 without an edit list). A decoder that gives only the samples (`decodeAudioData`) loses it.
    pub start: f64,
}

impl Track {
    /// The sample-entry code as text (`"avc1"`), or `"none"`.
    pub fn codec_name(&self) -> String {
        match self.codec {
            Some(c) => String::from_utf8_lossy(&c).into_owned(),
            None => "none".into(),
        }
    }
}

/// What [`probe`] found.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Mp4Info {
    /// `ftyp` major brand and compatible brands, in file order.
    pub brands: Vec<[u8; 4]>,
    pub tracks: Vec<Track>,
    /// The file has movie fragments (`mvex` or `moof`): sample tables can be in the fragments.
    pub fragmented: bool,
    /// `mvhd` duration in seconds, when its time scale is not 0.
    pub duration: Option<f64>,
}

impl Mp4Info {
    /// The first video track.
    pub fn video(&self) -> Option<&Track> {
        self.tracks.iter().find(|t| t.kind == TrackKind::Video)
    }

    /// The first audio track.
    pub fn audio(&self) -> Option<&Track> {
        self.tracks.iter().find(|t| t.kind == TrackKind::Audio)
    }

    /// The codecs of the video and audio tracks for a message: `"avc1+mp4a"`, `"hvc1"`, `"mp4a"`, `"none"`.
    pub fn codecs(&self) -> String {
        let names: Vec<String> = [self.video(), self.audio()]
            .into_iter()
            .flatten()
            .map(Track::codec_name)
            .collect();

        if names.is_empty() {
            "none".into()
        } else {
            names.join("+")
        }
    }
}

/// The boxes that can start an MP4 or QuickTime file.
const FIRST_BOXES: [&[u8; 4]; 6] = [b"ftyp", b"moov", b"free", b"skip", b"wide", b"mdat"];

/// One box: its type and the range of its body.
struct BoxRef {
    kind: [u8; 4],
    body: std::ops::Range<usize>,
}

/// The boxes in `d[range]`, in order. `count` is the number of boxes read so far over the whole walk.
fn boxes(
    d: &[u8],
    range: std::ops::Range<usize>,
    count: &mut usize,
) -> Result<Vec<BoxRef>, Mp4Error> {
    let mut out = vec![];
    let mut at = range.start;

    while at < range.end {
        *count += 1;

        if *count > MAX_BOXES {
            return Err(Mp4Error::Malformed("too many boxes"));
        }

        let header = d
            .get(at..at + 8)
            .filter(|_| at + 8 <= range.end)
            .ok_or(Mp4Error::Malformed("cut box header"))?;
        let size32 = u32::from_be_bytes(header[..4].try_into().unwrap());
        let kind: [u8; 4] = header[4..8].try_into().unwrap();

        let (size, header_len) = match size32 {
            0 => (range.end - at, 8),
            1 => {
                let large = d
                    .get(at + 8..at + 16)
                    .filter(|_| at + 16 <= range.end)
                    .ok_or(Mp4Error::Malformed("cut box header"))?;
                let large = u64::from_be_bytes(large.try_into().unwrap());

                (
                    usize::try_from(large).map_err(|_| Mp4Error::Malformed("box size"))?,
                    16,
                )
            }
            n => (n as usize, 8),
        };

        let end = at
            .checked_add(size)
            .ok_or(Mp4Error::Malformed("box size"))?;

        if size < header_len || end > range.end {
            return Err(Mp4Error::Malformed("box size"));
        }

        out.push(BoxRef {
            kind,
            body: at + header_len..end,
        });
        at = end;
    }

    Ok(out)
}

fn u16_at(d: &[u8], at: usize) -> Option<u16> {
    Some(u16::from_be_bytes(d.get(at..at + 2)?.try_into().ok()?))
}

fn u32_at(d: &[u8], at: usize) -> Option<u32> {
    Some(u32::from_be_bytes(d.get(at..at + 4)?.try_into().ok()?))
}

fn u64_at(d: &[u8], at: usize) -> Option<u64> {
    Some(u64::from_be_bytes(d.get(at..at + 8)?.try_into().ok()?))
}

/// The time scale and the duration in seconds of an `mvhd` or `mdhd` body (version 0 or 1).
fn header_times(d: &[u8], body: &std::ops::Range<usize>) -> Option<(u32, Option<f64>)> {
    let b = d.get(body.clone())?;
    let (timescale, duration) = match b.first()? {
        1 => (u32_at(b, 20)?, u64_at(b, 24)?),
        _ => (u32_at(b, 12)?, u64::from(u32_at(b, 16)?)),
    };
    // All bits set: the duration is not known.
    let known = duration != u64::MAX && duration != u64::from(u32::MAX);

    Some((
        timescale,
        (timescale != 0 && known).then(|| duration as f64 / f64::from(timescale)),
    ))
}

fn find<'a>(list: &'a [BoxRef], kind: &[u8; 4]) -> Option<&'a BoxRef> {
    list.iter().find(|b| &b.kind == kind)
}

/// The seconds of the empty edits (media time -1) at the start of an `elst` body, in the movie time scale.
fn empty_edits(d: &[u8], body: &std::ops::Range<usize>, movie_timescale: u32) -> f64 {
    let Some(b) = d.get(body.clone()) else {
        return 0.0;
    };
    let (Some(&version), Some(n)) = (b.first(), u32_at(b, 4)) else {
        return 0.0;
    };
    let size = if version == 1 { 20 } else { 12 };
    let mut total = 0u64;

    for i in 0..(n as usize).min(b.len() / size) {
        let at = 8 + i * size;
        let (duration, media_time) = if version == 1 {
            match (u64_at(b, at), u64_at(b, at + 8)) {
                (Some(dur), Some(t)) => (dur, t as i64),
                _ => break,
            }
        } else {
            match (u32_at(b, at), u32_at(b, at + 4)) {
                (Some(dur), Some(t)) => (u64::from(dur), i64::from(t as i32)),
                _ => break,
            }
        };

        if media_time != -1 {
            break;
        }

        total = total.saturating_add(duration);
    }

    if movie_timescale == 0 {
        0.0
    } else {
        total as f64 / f64::from(movie_timescale)
    }
}

/// One `trak` box.
fn track(
    d: &[u8],
    trak: &BoxRef,
    count: &mut usize,
    movie_timescale: u32,
) -> Result<Option<Track>, Mp4Error> {
    let children = boxes(d, trak.body.clone(), count)?;
    let elst = match find(&children, b"edts") {
        Some(edts) => find(&boxes(d, edts.body.clone(), count)?, b"elst").map(|e| e.body.clone()),
        None => None,
    };
    let edit_list = elst.is_some();
    let start = elst.map_or(0.0, |b| empty_edits(d, &b, movie_timescale));
    let Some(mdia) = find(&children, b"mdia") else {
        return Ok(None);
    };
    let mdia = boxes(d, mdia.body.clone(), count)?;

    let kind = match find(&mdia, b"hdlr").and_then(|h| d.get(h.body.start + 8..h.body.start + 12)) {
        Some(b"vide") => TrackKind::Video,
        Some(b"soun") => TrackKind::Audio,
        Some(other) => TrackKind::Other(other.try_into().unwrap()),
        None => return Ok(None),
    };
    let (timescale, duration) = find(&mdia, b"mdhd")
        .and_then(|m| header_times(d, &m.body))
        .unwrap_or((0, None));

    let mut t = Track {
        kind,
        codec: None,
        width: 0,
        height: 0,
        channels: 0,
        sample_rate: 0,
        timescale,
        duration,
        edit_list,
        start,
    };

    let stbl = match find(&mdia, b"minf") {
        Some(minf) => find(&boxes(d, minf.body.clone(), count)?, b"stbl")
            .map(|s| boxes(d, s.body.clone(), count))
            .transpose()?,
        None => None,
    };

    if let Some(stsd) = stbl.as_deref().and_then(|s| find(s, b"stsd")) {
        // Full box header (4) and entry count (4), then the sample entries as boxes.
        let start = stsd.body.start + 8;

        if start <= stsd.body.end
            && let Some(entry) = boxes(d, start..stsd.body.end, count)?.first()
        {
            t.codec = Some(entry.kind);
            let b = entry.body.start;

            match kind {
                // VisualSampleEntry: 6 reserved, data reference index, 16 bytes predefined, width, height.
                TrackKind::Video => {
                    t.width = u16_at(d, b + 24)
                        .filter(|_| b + 26 <= entry.body.end)
                        .unwrap_or(0);
                    t.height = u16_at(d, b + 26)
                        .filter(|_| b + 28 <= entry.body.end)
                        .unwrap_or(0);
                }
                // AudioSampleEntry: 6 reserved, data reference index, 8 reserved, channel count, sample size,
                // 4 predefined and reserved, sample rate 16.16.
                TrackKind::Audio => {
                    t.channels = u16_at(d, b + 16)
                        .filter(|_| b + 18 <= entry.body.end)
                        .unwrap_or(0);
                    t.sample_rate = u32_at(d, b + 24)
                        .filter(|_| b + 28 <= entry.body.end)
                        .map_or(0, |r| r >> 16);
                }
                TrackKind::Other(_) => {}
            }
        }
    }

    Ok(Some(t))
}

/// The structure of an MP4 or QuickTime file.
pub fn probe(d: &[u8]) -> Result<Mp4Info, Mp4Error> {
    match d.get(4..8) {
        Some(kind) if FIRST_BOXES.iter().any(|k| &k[..] == kind) => {}
        _ => return Err(Mp4Error::NotMp4),
    }

    let mut count = 0;
    let top = boxes(d, 0..d.len(), &mut count)?;
    let mut info = Mp4Info::default();

    if let Some(ftyp) = find(&top, b"ftyp") {
        let b = &d[ftyp.body.clone()];

        if b.len() >= 8 {
            info.brands.push(b[..4].try_into().unwrap());

            for c in b[8..].chunks_exact(4) {
                info.brands.push(c.try_into().unwrap());
            }
        }
    }

    info.fragmented = find(&top, b"moof").is_some();
    let moov = find(&top, b"moov").ok_or(Mp4Error::Malformed("no moov box"))?;
    let moov = boxes(d, moov.body.clone(), &mut count)?;

    info.fragmented |= find(&moov, b"mvex").is_some();
    let movie = find(&moov, b"mvhd").and_then(|m| header_times(d, &m.body));

    info.duration = movie.and_then(|(_, s)| s);

    for trak in moov.iter().filter(|b| &b.kind == b"trak") {
        if let Some(t) = track(d, trak, &mut count, movie.map_or(0, |(ts, _)| ts))? {
            info.tracks.push(t);
        }
    }

    Ok(info)
}
