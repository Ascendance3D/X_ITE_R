//! An MP4 movie as a [`MovieMedia`] (ADR 0015; a child module of `movie_texture` so it can build the private fields).
//! The container structure is read in Rust ([`crate::mp4probe`]); a platform decoder decodes the pictures and the
//! sound. A build without a platform decoder names the codecs it cannot decode.

#[cfg(feature = "native-video")]
use super::Kind;
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

        #[cfg(not(feature = "native-video"))]
        return Err(WebmError::Unsupported {
            container: "mp4",
            codec: info.codecs(),
        });

        #[cfg(feature = "native-video")]
        {
            let t = crate::sysvideo::decode(data, &info, super::MAX_DECODED_BYTES)?;

            Ok(Self::from_timeline(t))
        }
    }

    /// The media of a [`crate::sysvideo::Timeline`]: the pictures play like a WebM's (a picture lasts until the next one
    /// starts), the last picture lasts to the end of the longer track, and a file without pictures has no frame and one
    /// delay of the whole duration (its sound plays; the texture stays empty, as the element's `videoWidth` 0).
    #[cfg(feature = "native-video")]
    fn from_timeline(t: crate::sysvideo::Timeline) -> MovieMedia {
        let duration_ms = t.duration * 1000.0;
        let times: Vec<f64> = t.video.frames.iter().map(|f| f.time_ms).collect();
        let delays: Vec<f64> = if times.is_empty() {
            vec![duration_ms]
        } else {
            (0..times.len())
                .map(|i| times.get(i + 1).unwrap_or(&duration_ms) - times[i])
                .map(|d| d.max(0.0))
                .collect()
        };
        let (width, height) = if times.is_empty() {
            (0, 0)
        } else {
            (t.video.width, t.video.height)
        };

        MovieMedia {
            width,
            height,
            frames: t.video.frames.into_iter().map(|f| f.rgba).collect(),
            delays,
            kind: Kind::Gif {
                cycle: 0.0,
                looping: false,
            },
            playback_rate: 1.0,
            shown: 0,
            audio: t.audio,
            seekable: true,
            offset: 0.0,
            held: None,
            iso: Default::default(),
            sync_audio: true,
            audio_resyncs: 0,
        }
    }
}
