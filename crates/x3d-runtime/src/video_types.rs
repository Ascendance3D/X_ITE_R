//! The small types of the native video path that exist in every build, so code outside the `native-video` feature (the node,
//! the loader) names them without pulling in the decoders ([`crate::webm`]).

/// Why a file is not decoded.
#[derive(Debug)]
pub enum WebmError {
    NotWebm,
    /// No video track with a codec that is decoded here (VP9, VP8, and AV1 when the `av1` feature is on).
    NoVideoTrack,
    Decode(String),
    /// Either a picture codes more luma samples than [`MAX_LUMA_SAMPLES`] (refused before the decoder runs), or the
    /// RGBA pictures kept so far exceed the `max_bytes` of [`decode`] (refused before the next picture is kept). The
    /// second limit does not bound the working memory of the decoder; the first does.
    TooLarge,
    /// This build has no video decoder (the `native-video` feature is off; the browser build plays the video in the page).
    NoDecoder,
    /// The container, or a codec in it, has no decoder in this build (ADR 0015): `container` is `"mp4"` or `"webm"`,
    /// `codec` the sample-entry or codec code (`"hvc1"`, `"avc1+mp4a"`), or why no codec was found.
    Unsupported {
        container: &'static str,
        codec: String,
    },
}

/// One decoded picture: RGBA rows from the top, and its presentation time in milliseconds.
pub struct VideoFrame {
    pub rgba: Vec<u8>,
    pub time_ms: f64,
}

/// The decoded pictures of a video track.
pub struct Video {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<VideoFrame>,
}

/// The audio track of a WebM, as far as this build decodes it.
pub enum AudioTrack {
    /// The file has no audio track (a silent video: not an error).
    None,
    /// The track's codec has no decoder in this build (CodecID, e.g. `A_OPUS`).
    Unsupported(String),
    /// The track is there and does not decode.
    Corrupt,
    /// The planar samples at the track's own rate.
    Decoded { channels: Vec<Vec<f32>>, rate: f32 },
}
