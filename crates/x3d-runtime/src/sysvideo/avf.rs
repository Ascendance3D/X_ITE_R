//! The macOS backend of ADR 0015: AVFoundation decodes an MP4's pictures and sound.
//!
//! An `AVAssetReader` on an `AVURLAsset` of a private temporary file, with one `AVAssetReaderTrackOutput` for the first
//! video track (`420v`: 8-bit 4:2:0, two planes, video range) and one for the first audio track (32-bit float linear
//! PCM, interleaved, the track's own rate and channels). The outputs are read in turn, as fast as the decoders give
//! samples: the engine, not AVFoundation, plays the media. The decoders are the operating system's (VideoToolbox,
//! AudioToolbox). The reader is cancelled and the file is removed when [`decode`] returns.

use super::{Decoded, Picture, TempFile, YuvMatrix};
use crate::mp4probe::Mp4Info;
use crate::video_types::WebmError;
use crate::webm::MAX_LUMA_SAMPLES;
use objc2::rc::{Retained, autoreleasepool};
use objc2::runtime::AnyObject;
use objc2_av_foundation::{
    AVAssetReader, AVAssetReaderStatus, AVAssetReaderTrackOutput, AVAssetTrack, AVMediaTypeAudio,
    AVMediaTypeVideo, AVURLAsset,
};
use objc2_core_audio_types::kAudioFormatLinearPCM;
use objc2_core_foundation::{CFString, CFType};
use objc2_core_media::{
    CMAudioFormatDescriptionGetStreamBasicDescription, CMSampleBuffer, CMTime, CMTimeFlags,
};
use objc2_core_video::{
    CVPixelBufferGetBaseAddressOfPlane, CVPixelBufferGetBytesPerRowOfPlane, CVPixelBufferGetHeight,
    CVPixelBufferGetHeightOfPlane, CVPixelBufferGetPixelFormatType, CVPixelBufferGetWidth,
    CVPixelBufferGetWidthOfPlane, CVPixelBufferLockBaseAddress, CVPixelBufferLockFlags,
    CVPixelBufferUnlockBaseAddress, kCVImageBufferYCbCrMatrix_ITU_R_709_2,
    kCVImageBufferYCbCrMatrix_ITU_R_2020, kCVImageBufferYCbCrMatrix_SMPTE_240M_1995,
    kCVImageBufferYCbCrMatrixKey, kCVPixelBufferPixelFormatTypeKey,
    kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange,
};
use objc2_foundation::{NSArray, NSDictionary, NSNumber, NSString, NSURL};
use std::ptr::NonNull;

/// What the outputs collect.
#[derive(Default)]
struct Collected {
    width: u32,
    height: u32,
    matrix: Option<YuvMatrix>,
    pictures: Vec<Picture>,
    video_end: Option<f64>,
    audio: Vec<Vec<f32>>,
    audio_rate: f64,
    audio_start: Option<f64>,
    bytes: u64,
}

fn seconds(t: CMTime) -> Option<f64> {
    (t.flags.contains(CMTimeFlags::Valid) && t.timescale > 0)
        .then(|| t.value as f64 / f64::from(t.timescale))
}

/// A Core Foundation string as an `NSString` (toll-free bridged).
fn ns(s: &CFString) -> &NSString {
    // SAFETY: CFString and NSString are toll-free bridged: the same object.
    unsafe { &*(s as *const CFString).cast::<NSString>() }
}

fn too_large(c: &mut Collected, bytes: u64, max_bytes: u64) -> Result<(), WebmError> {
    c.bytes += bytes;

    if c.bytes > max_bytes {
        Err(WebmError::TooLarge)
    } else {
        Ok(())
    }
}

/// The matrix of a picture from its `CVImageBufferYCbCrMatrix` attachment; BT.601 when there is none.
fn matrix(pixels: &objc2_core_video::CVPixelBuffer) -> YuvMatrix {
    // SAFETY: the key is a CFString constant, the mode pointer may be null.
    let value = unsafe { pixels.attachment(kCVImageBufferYCbCrMatrixKey, std::ptr::null_mut()) };
    let Some(value) = value else {
        return YuvMatrix::Bt601;
    };
    // SAFETY: the value of this attachment is a CFString (CVImageBuffer.h).
    let value: &NSString = unsafe { &*(&*value as *const CFType).cast::<NSString>() };

    // SAFETY: the constants are CFStrings.
    let is = |k: &CFString| value.isEqualToString(ns(k));

    unsafe {
        if is(kCVImageBufferYCbCrMatrix_ITU_R_709_2) {
            YuvMatrix::Bt709
        } else if is(kCVImageBufferYCbCrMatrix_ITU_R_2020) {
            YuvMatrix::Bt2020
        } else if is(kCVImageBufferYCbCrMatrix_SMPTE_240M_1995) {
            YuvMatrix::Smpte240
        } else {
            YuvMatrix::Bt601
        }
    }
}

fn video_sample(c: &mut Collected, s: &CMSampleBuffer, max_bytes: u64) -> Result<(), WebmError> {
    // SAFETY: AVFoundation gives a valid sample buffer; the image buffer is a CVPixelBuffer of the format we asked for.
    let Some(pixels) = (unsafe { s.image_buffer() }) else {
        return Ok(());
    };
    let pts = seconds(unsafe { s.presentation_time_stamp() })
        .ok_or_else(|| WebmError::Decode("a picture without a time".into()))?;

    if CVPixelBufferGetPixelFormatType(&pixels) != kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange {
        return Err(WebmError::Decode(
            "AVFoundation gave another pixel format".into(),
        ));
    }

    let (w, h) = (
        CVPixelBufferGetWidth(&pixels),
        CVPixelBufferGetHeight(&pixels),
    );

    if (w * h) as u64 > MAX_LUMA_SAMPLES {
        return Err(WebmError::TooLarge);
    }

    if c.pictures.is_empty() {
        (c.width, c.height) = (w as u32, h as u32);
        c.matrix = Some(matrix(&pixels));
    } else if (w as u32, h as u32) != (c.width, c.height) {
        return Err(WebmError::Decode("the picture size changes".into()));
    }

    too_large(c, (w * h * 4) as u64, max_bytes)?;

    let (cw, ch) = (w.div_ceil(2), h.div_ceil(2));
    let mut picture = Picture {
        pts,
        y: Vec::with_capacity(w * h),
        u: Vec::with_capacity(cw * ch),
        v: Vec::with_capacity(cw * ch),
    };

    // SAFETY: the base addresses are valid between lock and unlock; rows are read inside the plane's size.
    unsafe {
        if CVPixelBufferLockBaseAddress(&pixels, CVPixelBufferLockFlags::ReadOnly) != 0 {
            return Err(WebmError::Decode("a picture that cannot be read".into()));
        }

        let luma = CVPixelBufferGetBaseAddressOfPlane(&pixels, 0) as *const u8;
        let chroma = CVPixelBufferGetBaseAddressOfPlane(&pixels, 1) as *const u8;
        let ok = !luma.is_null()
            && !chroma.is_null()
            && CVPixelBufferGetWidthOfPlane(&pixels, 0) >= w
            && CVPixelBufferGetHeightOfPlane(&pixels, 0) >= h
            && CVPixelBufferGetWidthOfPlane(&pixels, 1) >= cw
            && CVPixelBufferGetHeightOfPlane(&pixels, 1) >= ch;

        if ok {
            let (ys, cs) = (
                CVPixelBufferGetBytesPerRowOfPlane(&pixels, 0),
                CVPixelBufferGetBytesPerRowOfPlane(&pixels, 1),
            );

            for row in 0..h {
                picture
                    .y
                    .extend_from_slice(std::slice::from_raw_parts(luma.add(row * ys), w));
            }

            for row in 0..ch {
                // Cb and Cr alternate in the second plane.
                let line = std::slice::from_raw_parts(chroma.add(row * cs), cw * 2);

                for pair in line.chunks_exact(2) {
                    picture.u.push(pair[0]);
                    picture.v.push(pair[1]);
                }
            }
        }

        CVPixelBufferUnlockBaseAddress(&pixels, CVPixelBufferLockFlags::ReadOnly);

        if !ok {
            return Err(WebmError::Decode(
                "a picture with an unexpected layout".into(),
            ));
        }
    }

    let end = seconds(unsafe { s.duration() }).map_or(pts, |d| pts + d);

    c.video_end = Some(c.video_end.map_or(end, |e: f64| e.max(end)));
    c.pictures.push(picture);
    Ok(())
}

fn audio_sample(c: &mut Collected, s: &CMSampleBuffer, max_bytes: u64) -> Result<(), WebmError> {
    // SAFETY: AVFoundation gives a valid sample buffer of linear PCM as we asked for.
    let (desc, data) = unsafe { (s.format_description(), s.data_buffer()) };
    let (Some(desc), Some(data)) = (desc, data) else {
        return Ok(());
    };
    let asbd = unsafe { CMAudioFormatDescriptionGetStreamBasicDescription(&desc) };

    if asbd.is_null() {
        return Err(WebmError::Decode("a sound buffer without a format".into()));
    }

    // SAFETY: not null, owned by the format description.
    let asbd = unsafe { &*asbd };
    let channels = asbd.mChannelsPerFrame as usize;

    if asbd.mFormatID != kAudioFormatLinearPCM || asbd.mBitsPerChannel != 32 || channels == 0 {
        return Err(WebmError::Decode(
            "AVFoundation gave another sound format".into(),
        ));
    }

    if c.audio.is_empty() {
        c.audio = vec![vec![]; channels];
        c.audio_rate = asbd.mSampleRate;
    } else if channels != c.audio.len() || asbd.mSampleRate != c.audio_rate {
        return Err(WebmError::Decode("the sound format changes".into()));
    }

    let len = unsafe { data.data_length() };
    let mut bytes = vec![0u8; len];

    if len > 0 {
        // SAFETY: the destination holds `len` bytes.
        let status = unsafe {
            data.copy_data_bytes(0, len, NonNull::new(bytes.as_mut_ptr().cast()).unwrap())
        };

        if status != 0 {
            return Err(WebmError::Decode(format!(
                "the sound data cannot be read ({status})"
            )));
        }
    }

    let frames = len / 4 / channels;

    too_large(c, (frames * channels * 4) as u64, max_bytes)?;

    if c.audio_start.is_none() && frames > 0 {
        c.audio_start = Some(
            seconds(unsafe { s.presentation_time_stamp() })
                .ok_or_else(|| WebmError::Decode("a sound buffer without a time".into()))?,
        );
    }

    for frame in bytes.chunks_exact(4 * channels) {
        for (ch, b) in c.audio.iter_mut().zip(frame.chunks_exact(4)) {
            ch.push(f32::from_ne_bytes(b.try_into().unwrap()));
        }
    }

    Ok(())
}

fn number(n: u32) -> Retained<NSNumber> {
    NSNumber::new_u32(n)
}

/// The output settings of a track: the video or the sound format the engine reads.
fn settings(video: bool) -> Retained<NSDictionary<NSString, AnyObject>> {
    let (yes, no) = (NSNumber::new_bool(true), NSNumber::new_bool(false));

    if video {
        let format = number(kCVPixelFormatType_420YpCbCr8BiPlanarVideoRange);

        // SAFETY: a CFString constant.
        NSDictionary::from_slices(
            &[ns(unsafe { kCVPixelBufferPixelFormatTypeKey })],
            &[&*format as &AnyObject],
        )
    } else {
        use objc2_avf_audio::{
            AVFormatIDKey, AVLinearPCMBitDepthKey, AVLinearPCMIsBigEndianKey,
            AVLinearPCMIsFloatKey, AVLinearPCMIsNonInterleaved,
        };

        let pcm = number(kAudioFormatLinearPCM);
        let bits = number(32);
        // SAFETY: framework constants; present on every macOS that has AVFoundation.
        let key = |k: Option<&'static NSString>| k.expect("an AVFoundation constant");

        unsafe {
            NSDictionary::from_slices(
                &[
                    key(AVFormatIDKey),
                    key(AVLinearPCMIsFloatKey),
                    key(AVLinearPCMBitDepthKey),
                    key(AVLinearPCMIsNonInterleaved),
                    key(AVLinearPCMIsBigEndianKey),
                ],
                &[
                    &*pcm as &AnyObject,
                    &*yes as &AnyObject,
                    &*bits as &AnyObject,
                    &*no as &AnyObject,
                    &*no as &AnyObject,
                ],
            )
        }
    }
}

/// The `localizedDescription` of the reader's error.
fn reader_error(reader: &AVAssetReader) -> String {
    match unsafe { reader.error() } {
        Some(e) => format!("{} ({})", e.localizedDescription(), e.code()),
        None => "AVAssetReader failed".into(),
    }
}

/// Cancels the reader when it is dropped, so no decode survives a return.
struct Reading(Retained<AVAssetReader>);

impl Drop for Reading {
    fn drop(&mut self) {
        unsafe {
            if self.0.status() == AVAssetReaderStatus::Reading {
                self.0.cancelReading();
            }
        }
    }
}

#[allow(deprecated)]
fn first_track(asset: &AVURLAsset, video: bool) -> Option<Retained<AVAssetTrack>> {
    let kind = if video {
        unsafe { AVMediaTypeVideo }
    } else {
        unsafe { AVMediaTypeAudio }
    }?;
    // The synchronous accessor loads the local file's tracks at once (macOS 15 deprecates it for loadTracks).
    let tracks: Retained<NSArray<AVAssetTrack>> = unsafe { asset.tracksWithMediaType(kind) };

    tracks.firstObject()
}

pub fn decode(data: &[u8], info: &Mp4Info, max_bytes: u64) -> Result<Decoded, WebmError> {
    let unsupported = |why: String| WebmError::Unsupported {
        container: "mp4",
        codec: format!("{} ({why})", info.codecs()),
    };

    if let Some(v) = info.video()
        && u64::from(v.width) * u64::from(v.height) > MAX_LUMA_SAMPLES
    {
        return Err(WebmError::TooLarge);
    }

    let file = TempFile::write(data).map_err(|e| WebmError::Decode(e.to_string()))?;

    autoreleasepool(|_| {
        let path = NSString::from_str(&file.0.to_string_lossy());
        let url = NSURL::fileURLWithPath(&path);
        let asset = unsafe { AVURLAsset::URLAssetWithURL_options(&url, None) };
        let reader = unsafe { AVAssetReader::assetReaderWithAsset_error(&asset) }
            .map_err(|e| unsupported(e.localizedDescription().to_string()))?;
        let reader = Reading(reader);
        let mut outputs = vec![];

        for video in [true, false] {
            let Some(track) = first_track(&asset, video) else {
                continue;
            };
            let output = unsafe {
                AVAssetReaderTrackOutput::assetReaderTrackOutputWithTrack_outputSettings(
                    &track,
                    Some(&settings(video)),
                )
            };

            unsafe {
                output.setAlwaysCopiesSampleData(false);

                if !reader.0.canAddOutput(&output) {
                    return Err(unsupported(format!(
                        "AVAssetReader cannot read the {} track",
                        if video { "video" } else { "audio" }
                    )));
                }

                reader.0.addOutput(&output);
            }
            outputs.push((video, output, false));
        }

        if outputs.is_empty() {
            return Err(unsupported(
                "AVFoundation finds no video or audio track".into(),
            ));
        }

        if !unsafe { reader.0.startReading() } {
            return Err(unsupported(reader_error(&reader.0)));
        }

        let mut c = Collected::default();

        // The outputs in turn, one sample each, until every output is done.
        while outputs.iter().any(|o| !o.2) {
            for (video, output, done) in outputs.iter_mut().filter(|o| !o.2) {
                let sample = autoreleasepool(|_| unsafe { output.copyNextSampleBuffer() });

                match sample {
                    Some(s) if *video => video_sample(&mut c, &s, max_bytes)?,
                    Some(s) => audio_sample(&mut c, &s, max_bytes)?,
                    None => *done = true,
                }
            }
        }

        if unsafe { reader.0.status() } == AVAssetReaderStatus::Failed {
            let why = reader_error(&reader.0);

            return Err(if c.pictures.is_empty() && c.audio_start.is_none() {
                unsupported(why)
            } else {
                WebmError::Decode(why)
            });
        }

        // A track that the probe found and AVFoundation gave nothing for: its codec is not decoded here.
        if info.video().is_some() && c.pictures.is_empty() {
            return Err(unsupported("no picture decoded".into()));
        }

        if info.audio().is_some() && c.audio_start.is_none() {
            return Err(unsupported("no sound decoded".into()));
        }

        let mut decoders = vec![];

        if !c.pictures.is_empty() {
            decoders.push("AVFoundation video (VideoToolbox)".to_string());
        }

        if c.audio_start.is_some() {
            decoders.push("AVFoundation audio (AudioToolbox)".to_string());
        }

        c.pictures.sort_by(|a, b| a.pts.total_cmp(&b.pts));

        Ok(Decoded {
            width: c.width,
            height: c.height,
            matrix: c.matrix.unwrap_or(YuvMatrix::Bt601),
            full_range: false,
            audio: c
                .audio_start
                .map(|start| (c.audio, c.audio_rate as f32, start)),
            video_end: c.video_end,
            decoders,
            pictures: c.pictures,
        })
    })
}
