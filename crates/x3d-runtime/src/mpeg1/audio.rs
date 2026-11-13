//! The sound of an MPEG-1 Systems stream: the audio elementary stream that [`super::demux`] collects (ISO/IEC 11172-3 audio
//! frames, public identity only; clause numbers pending) decoded with `symphonia` (pure Rust, already a dependency for
//! AudioClip). Layer II is what MPEG-1 movies carry; Layer III decodes through the same reader. Layer I has no decoder
//! in this build and is reported as such.
//!
//! The decode is the audio of the stream from its first valid frame header to its last whole frame. A frame that does not
//! decode is skipped (the decoders the stream is compared with do the same), a stream cut inside a frame keeps the frames
//! before the cut, and nothing here panics on bad input: a panic in the decoder is caught and is a corrupt track.

use symphonia::core::codecs::audio::AudioDecoderOptions;
use symphonia::core::errors::Error;
use symphonia::core::formats::probe::Hint;
use symphonia::core::formats::{FormatOptions, TrackType};
use symphonia::core::meta::MetadataOptions;

use crate::video_types::AudioTrack;

/// The layer of an MPEG audio frame header (`11` for layer I, `10` for II, `01` for III in the header).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Layer {
    I,
    II,
    III,
}

/// The first position in `es` that starts a plausible MPEG audio frame header (the 11 bit sync word, a version other
/// than the reserved one, a layer, a bit rate index other than 15 and a sample rate index other than 3), and the layer
/// it names.
pub fn first_frame(es: &[u8]) -> Option<(usize, Layer)> {
    es.windows(4).enumerate().find_map(|(at, w)| {
        let layer = match (w[1] >> 1) & 3 {
            3 => Layer::I,
            2 => Layer::II,
            1 => Layer::III,
            _ => return None,
        };
        let plausible = w[0] == 0xFF
            && w[1] & 0xE0 == 0xE0
            && (w[1] >> 3) & 3 == 3 // MPEG-1 audio only (not the MPEG-2 or 2.5 low sample rates)
            && w[2] >> 4 != 0x0F
            && (w[2] >> 2) & 3 != 3;

        plausible.then_some((at, layer))
    })
}

/// Why the sound is not decoded.
#[derive(Debug, PartialEq, Eq)]
pub enum AudioError {
    /// No frame header in the data.
    NoFrame,
    /// Layer I: this build has no decoder for it.
    LayerI,
    /// The data does not decode (or no frame of it does).
    Corrupt,
    /// The decoded samples would be larger than the byte budget.
    TooLarge,
}

/// Decodes the audio elementary stream `es` to planar samples at the stream's own rate. At most `max_bytes` of f32
/// samples are produced.
pub fn decode(es: &[u8], max_bytes: u64) -> Result<(Vec<Vec<f32>>, f32), AudioError> {
    let (start, layer) = first_frame(es).ok_or(AudioError::NoFrame)?;

    if layer == Layer::I {
        return Err(AudioError::LayerI);
    }

    let source = symphonia::core::io::MediaSourceStream::new(
        Box::new(std::io::Cursor::new(es[start..].to_vec())),
        Default::default(),
    );
    let mut format = symphonia::default::get_probe()
        .probe(
            &Hint::new(),
            source,
            FormatOptions::default(),
            MetadataOptions::default(),
        )
        .map_err(|_| AudioError::Corrupt)?;
    let track = format
        .default_track(TrackType::Audio)
        .ok_or(AudioError::Corrupt)?;
    let id = track.id;
    let params = track
        .codec_params
        .as_ref()
        .and_then(|p| p.audio())
        .ok_or(AudioError::Corrupt)?
        .clone();
    let mut decoder = symphonia::default::get_codecs()
        .make_audio_decoder(&params, &AudioDecoderOptions::default().gapless(false))
        .map_err(|_| AudioError::Corrupt)?;
    let mut channels: Vec<Vec<f32>> = vec![];
    let mut rate = 0u32;
    let mut part = Vec::new();
    let mut bytes = 0u64;

    loop {
        let packet = match format.next_packet() {
            Ok(Some(p)) => p,
            Ok(None) => break,
            Err(Error::IoError(e)) if e.kind() == std::io::ErrorKind::UnexpectedEof => break,
            // A stream that goes bad keeps the frames before the bad place.
            Err(_) => break,
        };

        if packet.track_id != id {
            continue;
        }

        match decoder.decode(&packet) {
            Ok(buffer) => {
                // A change of rate in the middle of a movie is not a sound the node can play: the sound ends there.
                if rate != 0 && buffer.spec().rate() != rate {
                    break;
                }

                rate = buffer.spec().rate();
                buffer.copy_to_vecs_planar::<f32>(&mut part);

                if part.is_empty() {
                    continue;
                }

                if channels.is_empty() {
                    channels = vec![vec![]; part.len()];
                }

                if part.len() != channels.len() {
                    break;
                }

                bytes += part.iter().map(|c| c.len() as u64 * 4).sum::<u64>();

                if bytes > max_bytes {
                    return Err(AudioError::TooLarge);
                }

                for (c, p) in channels.iter_mut().zip(&part) {
                    c.extend_from_slice(p);
                }
            }
            Err(Error::DecodeError(_) | Error::IoError(_)) => {}
            Err(_) => break,
        }
    }

    if channels.first().is_some_and(|c| !c.is_empty()) && rate > 0 {
        Ok((channels, rate as f32))
    } else {
        Err(AudioError::Corrupt)
    }
}

/// The sound of `data` (an MPEG-1 Systems stream) as the node takes it: `None` for a stream without an audio stream,
/// `Unsupported` for a layer this build does not decode, `Corrupt` for a track that does not decode.
pub fn track(data: &[u8], max_bytes: u64) -> AudioTrack {
    if !super::demux::is_system_stream(data) {
        return AudioTrack::None;
    }

    let Ok(demuxed) = super::demux::demux(data) else {
        return AudioTrack::None;
    };

    if !demuxed.has_audio {
        return AudioTrack::None;
    }

    match std::panic::catch_unwind(|| decode(&demuxed.audio, max_bytes)) {
        Ok(Ok((mut channels, rate))) => {
            // The audio starts at its own time stamp on the movie's time line (demux::start_leads): leading silence when
            // the video starts first. The silence counts against the same byte budget; a lead that does not fit is left out.
            let lead = (super::demux::start_leads(&demuxed).1 * f64::from(rate)).round() as usize;
            let bytes = channels
                .iter()
                .map(|c| (c.len() + lead) as u64 * 4)
                .sum::<u64>();

            if lead > 0 && bytes <= max_bytes {
                for c in &mut channels {
                    c.splice(0..0, std::iter::repeat_n(0.0, lead));
                }
            }

            AudioTrack::Decoded { channels, rate }
        }
        Ok(Err(AudioError::LayerI)) => AudioTrack::Unsupported("MPEG-1 audio layer I".to_string()),
        _ => AudioTrack::Corrupt,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_mpeg_1_frame_headers_are_accepted() {
        // FF FD 90 00: MPEG-1 (version bits 11), layer II (10), 44.1 kHz class bits valid.
        assert_eq!(first_frame(&[0xFF, 0xFD, 0x90, 0x00]), Some((0, Layer::II)));
        // FF F5 90 00: version bits 10 (MPEG-2), layer II; FF E5 90 00: version bits 00 (MPEG-2.5).
        assert_eq!(first_frame(&[0xFF, 0xF5, 0x90, 0x00]), None);
        assert_eq!(first_frame(&[0xFF, 0xE5, 0x90, 0x00]), None);
    }
}
