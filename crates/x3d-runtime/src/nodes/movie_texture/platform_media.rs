//! An MP4 movie as a [`MovieMedia`] (ADR 0015; a child module of `movie_texture` so it can build the private fields).
//! The container structure is read in Rust ([`crate::mp4probe`]); a platform decoder decodes the pictures and the
//! sound. A build without a platform decoder names the codecs it cannot decode.

use super::MovieMedia;
use crate::mp4probe;
use crate::video_types::WebmError;

impl MovieMedia {
    /// An MP4 (or QuickTime) file decoded by the platform decoder of this build. The error says why the file is not
    /// shown: the data is not an MP4 or is damaged ([`WebmError::Decode`]), or this build has no decoder for its codecs
    /// ([`WebmError::Unsupported`]). The loader treats every error as the video element's `error` (ADR 0015, 7).
    pub fn decode_platform(data: &[u8]) -> Result<MovieMedia, WebmError> {
        let info = mp4probe::probe(data).map_err(|e| WebmError::Decode(e.to_string()))?;

        if info.video().is_none() && info.audio().is_none() {
            return Err(WebmError::Unsupported {
                container: "mp4",
                codec: "no video or audio track".into(),
            });
        }

        Err(WebmError::Unsupported {
            container: "mp4",
            codec: info.codecs(),
        })
    }
}
