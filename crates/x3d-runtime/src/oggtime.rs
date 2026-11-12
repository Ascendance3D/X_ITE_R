//! The start times of the tracks of an Ogg file (RFC 3533), for the sound of a browser MovieTexture (Phase 9A).
//!
//! A browser's `decodeAudioData` gives the samples from the first one, without the time at which the audio track
//! starts. The element plays that track from its start (measured in Chromium 151: a file whose audio starts 0.5 s after
//! the video decodes to 0.5 s less sound, and the element plays the sound from 0.5 s). So the engine puts the start
//! time of the audio track, from the video track's start, before the decoded sound.
//!
//! - The audio start (Vorbis or Opus) is the start timestamp of symphonia's Ogg reader. It comes from the granule
//!   position of the first audio page less the duration of the packets on that page, and for Opus also less the
//!   pre-skip (RFC 7845 4.1 and 4.5). Symphonia only reads the headers here: no audio is decoded (no Opus decoder is
//!   in any build, `tools/check_no_opus.py`).
//! - The video start (Theora) is read here from the granule position of the first Theora page that ends a picture
//!   (the Theora I specification, 6.2 and A.2.3): the key frame number shifted by KFGSHIFT plus the frames after it,
//!   counted from 1 since bitstream version 3.2.1, less the pictures that end on the pages up to that one. This is
//!   FFmpeg's rule (libavformat/oggparsetheora.c, `theora_packet`: one frame per packet before the first granule
//!   position), and Chromium plays Ogg through FFmpeg. An encoder that drops repeated pictures (FFmpeg's Ogg muxer does)
//!   leaves gaps that the rule does not see, so the result is FFmpeg's start time, not always the encoder's.

/// The start times in seconds of the first audio track (Vorbis or Opus) and of the first Theora track of `data` (None
/// for a track that is not there or whose time cannot be read), or None when `data` is not an Ogg file. Damaged data
/// never panics.
pub fn first_times(data: &[u8]) -> Option<(Option<f64>, Option<f64>)> {
    if !data.starts_with(b"OggS") {
        return None;
    }

    let audio = std::panic::catch_unwind(|| audio_start(data))
        .ok()
        .flatten();

    Some((theora_start(data), audio))
}

fn audio_start(data: &[u8]) -> Option<f64> {
    use symphonia::core::formats::probe::Hint;
    use symphonia::core::formats::{FormatOptions, TrackType};
    use symphonia::core::meta::MetadataOptions;

    let source = symphonia::core::io::MediaSourceStream::new(
        Box::new(std::io::Cursor::new(data.to_vec())),
        Default::default(),
    );
    let format = symphonia::default::get_probe()
        .probe(
            Hint::new().with_extension("ogg"),
            source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .ok()?;
    let track = format.default_track(TrackType::Audio)?;
    let base = track.time_base?;
    let start = track.start_ts.get();
    let seconds = start as f64 * f64::from(base.numer.get()) / f64::from(base.denom.get());

    seconds.is_finite().then_some(seconds)
}

/// One Ogg page: its stream serial number, granule position, flags and lacing values.
struct Page<'a> {
    serial: u32,
    granule: i64,
    lacing: &'a [u8],
    body: &'a [u8],
}

/// The pages of `data` in file order, up to the first damaged one.
fn pages(data: &[u8]) -> impl Iterator<Item = Page<'_>> {
    let mut at = 0usize;

    std::iter::from_fn(move || {
        let head = data.get(at..at.checked_add(27)?)?;

        if &head[..4] != b"OggS" {
            return None;
        }

        let lacing = data.get(at + 27..at + 27 + head[26] as usize)?;
        let size: usize = lacing.iter().map(|&n| n as usize).sum();
        let body_start = at + 27 + lacing.len();
        let body = data.get(body_start..body_start.checked_add(size)?)?;

        at = body_start + size;

        Some(Page {
            serial: u32::from_le_bytes(head[14..18].try_into().ok()?),
            granule: i64::from_le_bytes(head[6..14].try_into().ok()?),
            lacing,
            body,
        })
    })
}

fn theora_start(data: &[u8]) -> Option<f64> {
    let mut theora = None;
    // (packets that ended on the pages so far)
    let mut packets = 0u64;

    for page in pages(data) {
        let Some((serial, rate, shift, from_one)) = theora else {
            // The identification header is the first packet of the stream, alone on its page (A.2.3).
            let id = page.body;

            if id.len() >= 42 && id.starts_with(b"\x80theora") {
                let version = (id[7], id[8], id[9]);
                let frn = u32::from_be_bytes(id[22..26].try_into().ok()?);
                let frd = u32::from_be_bytes(id[26..30].try_into().ok()?);
                let shift = (u32::from(id[40] & 0x03) << 3) | u32::from(id[41] >> 5);

                if frn == 0 || frd == 0 {
                    return None;
                }

                theora = Some((
                    page.serial,
                    f64::from(frd) / f64::from(frn),
                    shift,
                    version >= (3, 2, 1),
                ));
                packets = page.lacing.iter().filter(|&&n| n < 255).count() as u64;
            }
            continue;
        };

        if page.serial != serial {
            continue;
        }

        packets += page.lacing.iter().filter(|&&n| n < 255).count() as u64;

        // -1: no packet ends on this page. The three header packets come first.
        if page.granule < 0 || packets <= 3 {
            continue;
        }

        let granule = page.granule as u64;
        let frames = (granule >> shift.min(63)) + (granule & ((1u64 << shift.min(63)) - 1));
        // The number of the last picture on this page, from 0.
        let last = if from_one {
            frames.checked_sub(1)?
        } else {
            frames
        };
        let first = last.checked_sub(packets - 3 - 1)?;

        return Some(first as f64 * rate);
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    fn file(name: &str) -> Vec<u8> {
        std::fs::read(format!(
            "{}/tests/data/websound/{name}",
            env!("CARGO_MANIFEST_DIR")
        ))
        .unwrap()
    }

    /// The start times of the synthetic files (oracle/tools/gen_movie_websound.py), against ffprobe: the video starts at
    /// 0; the audio of the late files at 0.5 s (ffprobe: Opus 0.500000, Vorbis 0.497333). A Vorbis stream from 0 starts
    /// 128 frames early: its first packet is the lapped half of a short block, which FFmpeg (Chromium's decoder) does
    /// not output and ffprobe shows as 0; the lead is then 0.
    #[test]
    fn the_start_times_of_the_synthetic_files() {
        for (name, audio) in [
            ("theora-vorbis.ogv", -128.0 / 48000.0),
            ("theora-opus.ogv", 0.0),
            ("theora-vorbis-late.ogv", 23872.0 / 48000.0),
            ("theora-opus-late.ogv", 0.5),
        ] {
            let (v, a) = first_times(&file(name)).unwrap();

            assert_eq!(v, Some(0.0), "{name}: video start");
            assert!(
                (a.unwrap() - audio).abs() < 1e-6,
                "{name}: audio start {a:?}, want {audio}"
            );
        }
    }

    /// Both tracks start at 0.5 s (ffprobe: Theora 0.500000, Vorbis 0.497333): the first Theora granule position is not
    /// 0, and the muxer dropped the repeated pictures (8 packets for 40 frames).
    #[test]
    fn an_offset_file_starts_where_ffprobe_says() {
        let (v, a) = first_times(&file("theora-vorbis-offset.ogv")).unwrap();

        assert!((v.unwrap() - 0.5).abs() < 1e-9, "{v:?}");
        assert!((a.unwrap() - (0.5 - 128.0 / 48000.0)).abs() < 1e-6, "{a:?}");
    }

    #[test]
    fn a_file_that_is_not_ogg_has_no_times() {
        assert_eq!(first_times(&file("vp8-opus.webm")), None);
        assert_eq!(first_times(b""), None);
    }

    /// Every cut of a file and damaged bytes in its headers give a time or None, never a panic.
    #[test]
    fn cut_or_damaged_ogg_never_panics() {
        for name in ["theora-vorbis-late.ogv", "theora-opus-late.ogv"] {
            let data = file(name);

            for end in (0..data.len()).step_by(97).chain([4, 27, 28, 60]) {
                let _ = first_times(&data[..end.min(data.len())]);
            }

            for at in (0..600.min(data.len())).step_by(7) {
                let mut bad = data.clone();

                bad[at] ^= 0xA5;
                let _ = first_times(&bad);
            }
        }
    }
}
