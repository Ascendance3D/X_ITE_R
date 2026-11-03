//! The MPEG-1 Systems layer (ISO/IEC 11172-1, public identity only; clause numbers pending): a stream of packs, each with
//! packets of one elementary stream. Only what a MovieTexture needs: the payload of the first video stream
//! (stream ids 0xE0 to 0xEF) in file order and the payload of the first audio stream (0xC0 to 0xDF), which is an MPEG
//! audio elementary stream (a sequence of audio frames; a frame may run across packets).
//!
//! The parser is a scan over start codes (`00 00 01 xx`). A packet whose length runs past the end of the data is
//! taken as far as it goes (a cut-off file plays up to the cut). An MPEG-2 program stream (a pack header or packet
//! header with the MPEG-2 marker bits) is refused.

use super::Mpeg1Error;

const START_PACK: u8 = 0xBA;
const START_END: u8 = 0xB9;
const START_SYSTEM_HEADER: u8 = 0xBB;

/// What [`demux`] found.
#[derive(Debug, PartialEq)]
pub struct Demuxed {
    /// The video elementary stream: the payloads of the packets of the first video stream, concatenated.
    pub video: Vec<u8>,
    /// Presentation time of the first video packet that has one, in seconds.
    pub first_video_pts: Option<f64>,
    /// Does the stream carry an audio stream?
    pub has_audio: bool,
    /// The audio elementary stream: the payloads of the packets of the first audio stream, concatenated (decoded by
    /// [`super::audio`]). Empty when `has_audio` is false or the audio packets hold no data.
    pub audio: Vec<u8>,
}

/// Is `data` the start of an MPEG-1 Systems stream (a pack start code)?
pub fn is_system_stream(data: &[u8]) -> bool {
    data.starts_with(&[0, 0, 1, START_PACK])
}

/// The 33-bit time stamp coded in five bytes (marker bits `0010`/`0011` first), in seconds of the 90 kHz clock.
fn time_stamp(b: &[u8]) -> Option<f64> {
    if b.len() < 5 {
        return None;
    }

    let t = (u64::from(b[0] >> 1) & 0x07) << 30
        | u64::from(b[1]) << 22
        | (u64::from(b[2] >> 1)) << 15
        | u64::from(b[3]) << 7
        | u64::from(b[4] >> 1);

    Some(t as f64 / 90_000.0)
}

/// Splits an MPEG-1 Systems stream (data starting with a pack start code).
pub fn demux(data: &[u8]) -> Result<Demuxed, Mpeg1Error> {
    if !is_system_stream(data) {
        return Err(Mpeg1Error::NotMpeg1);
    }

    let mut out = Demuxed {
        video: vec![],
        first_video_pts: None,
        has_audio: false,
        audio: vec![],
    };
    let mut video_id = None;
    let mut audio_id = None;
    let mut at = 0usize;

    while at + 4 <= data.len() {
        if data[at..at + 3] != [0, 0, 1] {
            at += 1;

            continue;
        }

        let id = data[at + 3];

        match id {
            START_PACK => {
                // MPEG-1: marker `0010`; MPEG-2 program streams have `01`.
                let Some(&marker) = data.get(at + 4) else {
                    break;
                };

                if marker & 0xC0 == 0x40 {
                    return Err(Mpeg1Error::UnsupportedMpeg2);
                }

                if marker & 0xF0 != 0x20 {
                    return Err(Mpeg1Error::Malformed("pack header marker"));
                }

                at += 12;
            }
            START_END => break,
            START_SYSTEM_HEADER | 0xBC..=0xFF => {
                let Some(len) = data.get(at + 4..at + 6) else {
                    break;
                };
                let len = usize::from(u16::from_be_bytes([len[0], len[1]]));
                let start = at + 6;
                let end = (start + len).min(data.len());

                if id >= 0xC0 {
                    let is_video = (0xE0..=0xEF).contains(&id);
                    let is_audio = (0xC0..=0xDF).contains(&id);

                    if is_audio {
                        out.has_audio = true;
                    }

                    if is_video && video_id.is_none_or(|v| v == id) {
                        video_id = Some(id);

                        let (payload, pts) = packet(&data[start..end])?;

                        if out.first_video_pts.is_none() {
                            out.first_video_pts = pts;
                        }

                        out.video.extend_from_slice(payload);
                    } else if is_audio && audio_id.is_none_or(|a| a == id) {
                        audio_id = Some(id);

                        // A bad audio packet header costs the sound its packet, not the pictures theirs.
                        if let Ok((payload, _)) = packet(&data[start..end]) {
                            out.audio.extend_from_slice(payload);
                        }
                    }
                }

                at = end;
            }
            _ => at += 4,
        }
    }

    if out.video.is_empty() {
        return Err(Mpeg1Error::NoPicture);
    }

    Ok(out)
}

/// The payload of one packet and its presentation time stamp, if it has one: the header fields before the payload are
/// stuffing bytes, an optional STD buffer field and an optional time stamp (and decoding time stamp).
fn packet(body: &[u8]) -> Result<(&[u8], Option<f64>), Mpeg1Error> {
    let mut i = 0;

    while body.get(i) == Some(&0xFF) {
        i += 1;
    }

    let Some(&b) = body.get(i) else {
        return Ok((&[], None)); // an empty packet
    };

    if b & 0xC0 == 0x80 {
        return Err(Mpeg1Error::UnsupportedMpeg2); // an MPEG-2 PES header
    }

    if b & 0xC0 == 0x40 {
        i += 2;
    }

    let Some(&b) = body.get(i) else {
        return Ok((&[], None));
    };
    let mut pts = None;

    match b & 0xF0 {
        0x20 => {
            pts = body.get(i..i + 5).and_then(time_stamp);
            i += 5;
        }
        0x30 => {
            pts = body.get(i..i + 5).and_then(time_stamp);
            i += 10;
        }
        _ if b == 0x0F => i += 1,
        _ => return Err(Mpeg1Error::Malformed("packet header")),
    }

    Ok((body.get(i..).unwrap_or(&[]), pts))
}
