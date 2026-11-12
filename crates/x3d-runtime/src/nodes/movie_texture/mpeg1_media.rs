//! The MPEG-1 movie as a [`MovieMedia`] (a child module of `movie_texture` so it can build the private fields).

use super::{Kind, MAX_DECODED_BYTES, MovieMedia};
use crate::mpeg1::{self, Mpeg1Error};

impl MovieMedia {
    /// An MPEG-1 Systems stream or MPEG-1 video elementary stream ([`crate::mpeg1`]) decoded where the page has no
    /// `<video>` element. ISO/IEC 19775-1 18.4.2 requires this format; a browser's `<video>` cannot play it (Chrome's
    /// `canPlayType("video/mpeg")` is empty), so the pinned X_ITE shows nothing for it: the loader keeps that as the
    /// compatibility option [`crate::Runtime::mpeg1_unplayable`]. The pictures play like a GIF's, every picture lasting
    /// one picture period of the stream. The sound is the Systems stream's MPEG audio track ([`mpeg1::audio`]); reverse
    /// playback is not part of it.
    pub fn decode_mpeg1(data: &[u8]) -> Result<MovieMedia, Mpeg1Error> {
        let video = mpeg1::decode(data, MAX_DECODED_BYTES)?;
        let delay = 1000.0 / video.framerate;

        Ok(MovieMedia {
            width: video.width,
            height: video.height,
            delays: vec![delay; video.frames.len()],
            frames: video.frames,
            kind: Kind::Gif {
                cycle: 0.0,
                looping: false,
            },
            playback_rate: 1.0,
            shown: 0,
            audio: mpeg1::audio::track(data, MAX_DECODED_BYTES),
            seekable: false,
            offset: 0.0,
            held: None,
            iso: Default::default(),
            sync_audio: false,
            audio_resyncs: 0,
        })
    }
}
