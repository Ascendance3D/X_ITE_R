//! Platform decoders for MovieTexture media that the engine does not decode itself (ADR 0015): MP4 video and audio.
//!
//! A backend decodes the compressed media and gives the pictures as YUV planes and the sound as `f32` samples, each with
//! its presentation time. This module puts both tracks on one timeline ([`timeline`]): the origin is the earliest
//! track start, the first picture shows from 0, the sound gets silence before its first sample, and both tracks end at
//! the latest track end. The engine then plays them as one media ([`crate::nodes::movie_texture::MovieMedia`]).

#[cfg(all(feature = "system-video-avf", target_os = "macos"))]
pub mod avf;
#[cfg(feature = "system-video-gst")]
pub mod gst;

use crate::video_types::{AudioTrack, Video, VideoFrame, WebmError};
use crate::webm::{BT601, BT709, Matrix, full_to_limited, matrix_of, yuv_to_rgba};

/// A file in the temporary directory that is removed when it is dropped (the backends read a file, not memory).
#[cfg(any(
    feature = "system-video-gst",
    all(feature = "system-video-avf", target_os = "macos")
))]
pub(crate) struct TempFile(pub(crate) std::path::PathBuf);

#[cfg(any(
    feature = "system-video-gst",
    all(feature = "system-video-avf", target_os = "macos")
))]
impl TempFile {
    pub(crate) fn write(data: &[u8]) -> std::io::Result<TempFile> {
        use std::io::Write;
        static NEXT: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(0);

        let n = NEXT.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("x3d-mp4-{}-{n}.mp4", std::process::id()));
        let mut f = std::fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&path)?;
        let file = TempFile(path);

        f.write_all(data)?;
        Ok(file)
    }
}

#[cfg(any(
    feature = "system-video-gst",
    all(feature = "system-video-avf", target_os = "macos")
))]
impl Drop for TempFile {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.0);
    }
}

/// The YUV to RGB matrix of a picture, as its stream tags give it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum YuvMatrix {
    Bt601,
    Bt709,
    Bt2020,
    Smpte240,
}

/// One decoded 4:2:0 picture (planes without row padding; chroma `ceil (w / 2) x ceil (h / 2)`).
pub struct Picture {
    /// Presentation time in seconds.
    pub pts: f64,
    pub y: Vec<u8>,
    pub u: Vec<u8>,
    pub v: Vec<u8>,
}

/// What a backend decoded.
pub struct Decoded {
    pub width: u32,
    pub height: u32,
    pub matrix: YuvMatrix,
    pub full_range: bool,
    pub pictures: Vec<Picture>,
    /// The sound: planar samples, the sample rate, and the presentation time in seconds of the first sample.
    pub audio: Option<(Vec<Vec<f32>>, f32, f64)>,
    /// The end of the video track in seconds (the last picture's time plus its duration), if there is one.
    pub video_end: Option<f64>,
    /// The decoder elements the platform chose (for the reports).
    pub decoders: Vec<String>,
}

impl Decoded {
    /// The bytes the engine keeps for this media: RGBA pictures and `f32` samples.
    pub fn kept_bytes(width: u32, height: u32, pictures: usize, samples: usize) -> u64 {
        u64::from(width) * u64::from(height) * 4 * pictures as u64 + samples as u64 * 4
    }
}

/// The media on one timeline: the pictures (RGBA, times in milliseconds from the origin), the sound (silence first, then
/// padded to the duration so that a loop of the sound and a loop of the pictures are the same length) and the duration
/// in seconds.
pub struct Timeline {
    pub video: Video,
    pub audio: AudioTrack,
    pub duration: f64,
}

fn matrix(m: YuvMatrix) -> Matrix {
    match m {
        YuvMatrix::Bt601 => BT601,
        YuvMatrix::Bt709 => BT709,
        YuvMatrix::Bt2020 => matrix_of(0.2627, 0.0593),
        YuvMatrix::Smpte240 => matrix_of(0.212, 0.087),
    }
}

/// [`Decoded`] on one timeline (ADR 0015, 3). The origin is the earliest first presentation time of the two tracks; a
/// picture's time is its presentation time minus the origin; the first picture shows from 0; the sound starts with
/// silence up to its first sample and is padded with silence to the duration; the duration is the latest track end.
pub fn timeline(d: Decoded) -> Result<Timeline, WebmError> {
    let first_picture = d.pictures.first().map(|p| p.pts);
    let first_sample = d.audio.as_ref().map(|a| a.2);
    let origin = match (first_picture, first_sample) {
        (Some(a), Some(b)) => a.min(b),
        (Some(a), None) | (None, Some(a)) => a,
        (None, None) => {
            return Err(WebmError::Decode(
                "the file gave no picture and no sound".into(),
            ));
        }
    };

    let audio_end = d.audio.as_ref().map(|(channels, rate, start)| {
        start + channels.first().map_or(0, Vec::len) as f64 / f64::from(*rate)
    });
    let video_end = d.video_end.or(d.pictures.last().map(|p| p.pts));
    let duration = [audio_end, video_end]
        .into_iter()
        .flatten()
        .fold(0.0f64, f64::max)
        - origin;

    let (w, h) = (d.width as usize, d.height as usize);
    let m = matrix(d.matrix);
    let frames = d
        .pictures
        .into_iter()
        .enumerate()
        .map(|(i, mut p)| {
            if d.full_range {
                full_to_limited(&mut p.y, &mut p.u, &mut p.v);
            }

            VideoFrame {
                rgba: yuv_to_rgba(w, h, true, true, &p.y, &p.u, &p.v, m),
                // The first picture shows from the origin.
                time_ms: if i == 0 {
                    0.0
                } else {
                    (p.pts - origin) * 1000.0
                },
            }
        })
        .collect();

    let audio = match d.audio {
        None => AudioTrack::None,
        Some((channels, rate, start)) => {
            let lead = ((start - origin) * f64::from(rate)).round().max(0.0) as usize;
            let total = ((duration * f64::from(rate)).round() as usize).max(1);
            let channels = channels
                .into_iter()
                .map(|c| {
                    let mut out = vec![0.0; lead];

                    out.extend(c);
                    out.resize(total.max(out.len()), 0.0);
                    out.truncate(total);
                    out
                })
                .collect();

            AudioTrack::Decoded { channels, rate }
        }
    };

    Ok(Timeline {
        video: Video {
            width: d.width,
            height: d.height,
            frames,
        },
        audio,
        duration,
    })
}

/// The MP4 file decoded by this build's platform backend, on one timeline. `Unsupported` when the build has no backend.
pub fn decode(
    data: &[u8],
    info: &crate::mp4probe::Mp4Info,
    max_bytes: u64,
) -> Result<Timeline, WebmError> {
    timeline(decode_tracks(data, info, max_bytes)?)
}

/// The tracks of the MP4 file as this build's platform backend decodes them (macOS: AVFoundation, else GStreamer), before
/// [`timeline`]. `Unsupported` when the build has no backend.
#[allow(unused_variables)]
pub fn decode_tracks(
    data: &[u8],
    info: &crate::mp4probe::Mp4Info,
    max_bytes: u64,
) -> Result<Decoded, WebmError> {
    #[cfg(all(feature = "system-video-avf", target_os = "macos"))]
    return avf::decode(data, info, max_bytes);

    #[cfg(all(
        feature = "system-video-gst",
        not(all(feature = "system-video-avf", target_os = "macos"))
    ))]
    return gst::decode(data, info, max_bytes);

    #[allow(unreachable_code)]
    Err(WebmError::Unsupported {
        container: "mp4",
        codec: info.codecs(),
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn picture(pts: f64, y: u8) -> Picture {
        Picture {
            pts,
            y: vec![y; 4],
            u: vec![128],
            v: vec![128],
        }
    }

    fn decoded(pictures: Vec<Picture>, audio: Option<(Vec<Vec<f32>>, f32, f64)>) -> Decoded {
        Decoded {
            width: 2,
            height: 2,
            matrix: YuvMatrix::Bt601,
            full_range: false,
            video_end: pictures.last().map(|p| p.pts + 0.5),
            decoders: vec![],
            pictures,
            audio,
        }
    }

    #[test]
    fn the_origin_is_the_earliest_track_start_and_the_sound_gets_leading_silence() {
        // The pictures start at 1.0 s, the sound at 1.25 s (rate 4: one sample a quarter second).
        let t = timeline(decoded(
            vec![picture(1.0, 16), picture(1.5, 235)],
            Some((vec![vec![0.5; 4]], 4.0, 1.25)),
        ))
        .unwrap();

        // The sound ends at 2.25 s, the pictures at 2.0 s: 1.25 s from the origin.
        assert_eq!(t.duration, 1.25);
        assert_eq!(
            t.video.frames.iter().map(|f| f.time_ms).collect::<Vec<_>>(),
            [0.0, 500.0]
        );
        assert_eq!(t.video.frames[0].rgba[..4], [0, 0, 0, 255]);
        assert_eq!(t.video.frames[1].rgba[..4], [255, 255, 255, 255]);

        let AudioTrack::Decoded { channels, rate } = t.audio else {
            panic!()
        };

        assert_eq!(rate, 4.0);
        // One sample of silence, then the four samples: 5 samples, the duration.
        assert_eq!(channels[0], [0.0, 0.5, 0.5, 0.5, 0.5]);
    }

    #[test]
    fn a_sound_that_starts_first_moves_the_origin_and_the_first_picture_shows_from_zero() {
        let t = timeline(decoded(
            vec![picture(0.5, 16), picture(1.0, 16)],
            Some((vec![vec![0.25; 8]], 4.0, 0.0)),
        ))
        .unwrap();

        // Sound 0 to 2 s, pictures to 1.5 s.
        assert_eq!(t.duration, 2.0);
        assert_eq!(t.video.frames[0].time_ms, 0.0);
        assert_eq!(t.video.frames[1].time_ms, 1000.0);
    }

    #[test]
    fn a_short_sound_is_padded_to_the_duration() {
        let t = timeline(decoded(
            vec![picture(0.0, 16), picture(1.5, 16)],
            Some((vec![vec![1.0; 2], vec![1.0; 2]], 4.0, 0.0)),
        ))
        .unwrap();
        let AudioTrack::Decoded { channels, .. } = t.audio else {
            panic!()
        };

        assert_eq!(t.duration, 2.0);
        assert_eq!(channels.len(), 2);
        assert_eq!(channels[1], [1.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0]);
    }

    #[test]
    fn audio_only_and_video_only_have_one_track() {
        let t = timeline(decoded(vec![], Some((vec![vec![0.0; 8]], 4.0, 0.1)))).unwrap();

        assert!(t.video.frames.is_empty());
        assert_eq!(t.duration, 2.0);

        let t = timeline(decoded(vec![picture(0.0, 16)], None)).unwrap();

        assert!(matches!(t.audio, AudioTrack::None));
        assert_eq!(t.duration, 0.5);
        assert!(timeline(decoded(vec![], None)).is_err());
    }
}
