//! The Linux backend of ADR 0015: the system GStreamer decodes an MP4's pictures and sound.
//!
//! `filesrc ! decodebin3`; each decoded video pad `! videoconvert ! video/x-raw,format=I420 ! appsink`, each audio pad
//! `! audioconvert ! audio/x-raw,format=F32LE,layout=interleaved ! appsink`. Both sinks take every sample as fast as the
//! decoder gives it (`sync=false`, `async=false`): the engine, not GStreamer, plays the media. The decoders are the
//! user's installed plugins. The file goes to a private temporary file for `qtdemux` (which needs random access for a
//! file whose `moov` box is at the end); the file, the pipeline and its threads are gone when [`decode`] returns.

use super::{Decoded, Picture, TempFile, YuvMatrix};
use crate::mp4probe::Mp4Info;
use crate::video_types::WebmError;
use crate::webm::MAX_LUMA_SAMPLES;
use gstreamer as gst;
use gstreamer::prelude::*;
use gstreamer_app as gst_app;
use gstreamer_audio as gst_audio;
use gstreamer_video as gst_video;
use gstreamer_video::prelude::*;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

/// The longest time a decode may take.
const TIMEOUT: Duration = Duration::from_secs(60);

/// What the sinks collect.
#[derive(Default)]
struct Collected {
    width: u32,
    height: u32,
    matrix: Option<YuvMatrix>,
    full_range: bool,
    pictures: Vec<Picture>,
    video_end: Option<f64>,
    video_pad: bool,
    audio: Vec<Vec<f32>>,
    audio_rate: u32,
    audio_start: Option<f64>,
    audio_pad: bool,
    /// The decoder elements GStreamer chose (factory names), for the reports.
    decoders: Vec<String>,
    bytes: u64,
    too_large: bool,
    error: Option<String>,
}

fn seconds(t: gst::ClockTime) -> f64 {
    t.nseconds() as f64 / 1e9
}

/// The stream time of a buffer time in its sample's segment (edit lists applied by `qtdemux`).
fn stream_time(sample: &gst::Sample, t: Option<gst::ClockTime>) -> Option<f64> {
    let t = t?;
    let segment = sample.segment()?.downcast_ref::<gst::ClockTime>()?;

    segment.to_stream_time(t).map(seconds)
}

fn video_sample(c: &mut Collected, sample: &gst::Sample, max_bytes: u64) -> Result<(), String> {
    let caps = sample.caps().ok_or("a picture without caps")?;
    let info = gst_video::VideoInfo::from_caps(caps).map_err(|e| e.to_string())?;
    let buffer = sample.buffer().ok_or("a picture without data")?;
    let (w, h) = (info.width(), info.height());

    if u64::from(w) * u64::from(h) > MAX_LUMA_SAMPLES {
        c.too_large = true;
        return Err("picture too large".into());
    }

    if c.pictures.is_empty() {
        let colorimetry = info.colorimetry();

        c.width = w;
        c.height = h;
        c.full_range = colorimetry.range() == gst_video::VideoColorRange::Range0_255;
        c.matrix = Some(match colorimetry.matrix() {
            gst_video::VideoColorMatrix::Bt709 => YuvMatrix::Bt709,
            gst_video::VideoColorMatrix::Bt2020 => YuvMatrix::Bt2020,
            gst_video::VideoColorMatrix::Smpte240m => YuvMatrix::Smpte240,
            _ => YuvMatrix::Bt601,
        });
    } else if (w, h) != (c.width, c.height) {
        return Err(format!(
            "the picture size changes ({} x {} to {w} x {h})",
            c.width, c.height
        ));
    }

    c.bytes += u64::from(w) * u64::from(h) * 4;

    if c.bytes > max_bytes {
        c.too_large = true;
        return Err("the decoded media is larger than the limit".into());
    }

    let frame = gst_video::VideoFrameRef::from_buffer_ref_readable(buffer, &info)
        .map_err(|_| "a picture that cannot be read")?;
    let mut planes = [vec![], vec![], vec![]];

    for (i, plane) in planes.iter_mut().enumerate() {
        let (pw, ph) = (
            info.comp_width(i as u8) as usize,
            info.comp_height(i as u8) as usize,
        );
        let stride = frame.plane_stride()[i] as usize;
        let data = frame.plane_data(i as u32).map_err(|e| e.to_string())?;

        plane.reserve(pw * ph);

        for row in 0..ph {
            plane.extend_from_slice(
                data.get(row * stride..row * stride + pw)
                    .ok_or("a cut picture plane")?,
            );
        }
    }

    let pts = stream_time(sample, buffer.pts()).ok_or("a picture without a time")?;
    let [y, u, v] = planes;

    c.video_end = Some(pts + buffer.duration().map_or(0.0, seconds));
    c.pictures.push(Picture { pts, y, u, v });
    Ok(())
}

fn audio_sample(c: &mut Collected, sample: &gst::Sample, max_bytes: u64) -> Result<(), String> {
    let caps = sample.caps().ok_or("a sound buffer without caps")?;
    let info = gst_audio::AudioInfo::from_caps(caps).map_err(|e| e.to_string())?;
    let buffer = sample.buffer().ok_or("a sound buffer without data")?;
    let channels = info.channels() as usize;

    if c.audio.is_empty() {
        c.audio = vec![vec![]; channels];
        c.audio_rate = info.rate();
    } else if channels != c.audio.len() || info.rate() != c.audio_rate {
        return Err("the sound format changes".into());
    }

    let map = buffer.map_readable().map_err(|e| e.to_string())?;
    let samples: Vec<f32> = map
        .chunks_exact(4)
        .map(|b| f32::from_le_bytes(b.try_into().unwrap()))
        .collect();
    let frames = samples.len() / channels.max(1);

    // Samples that the container marks as not presented (the decoder delay of AAC): GstAudioClippingMeta.
    let (mut skip_start, mut skip_end) = (0usize, 0usize);

    if let Some(meta) = buffer.meta::<gst_audio::AudioClippingMeta>()
        && let (gst::GenericFormattedValue::Default(s), gst::GenericFormattedValue::Default(e)) =
            (meta.start(), meta.end())
    {
        skip_start = s.map_or(0, |v| *v as usize).min(frames);
        skip_end = e.map_or(0, |v| *v as usize).min(frames - skip_start);
    }

    let kept = frames - skip_start - skip_end;

    c.bytes += (kept * channels) as u64 * 4;

    if c.bytes > max_bytes {
        c.too_large = true;
        return Err("the decoded media is larger than the limit".into());
    }

    if c.audio_start.is_none() && kept > 0 {
        let pts = stream_time(sample, buffer.pts()).ok_or("a sound buffer without a time")?;

        c.audio_start = Some(pts + skip_start as f64 / f64::from(c.audio_rate));
    }

    for frame in samples.chunks_exact(channels).skip(skip_start).take(kept) {
        for (ch, s) in c.audio.iter_mut().zip(frame) {
            ch.push(*s);
        }
    }

    Ok(())
}

fn element(factory: &str) -> Result<gst::Element, String> {
    gst::ElementFactory::make(factory)
        .build()
        .map_err(|_| format!("GStreamer has no {factory}"))
}

/// One branch after a decoded pad of `decodebin3`: a converter, the caps the engine takes, and an app sink.
fn branch(
    pipeline: &gst::Pipeline,
    pad: &gst::Pad,
    state: &Arc<Mutex<Collected>>,
    video: bool,
    max_bytes: u64,
) -> Result<(), String> {
    let convert = element(if video {
        "videoconvert"
    } else {
        "audioconvert"
    })?;
    let caps = if video {
        gst::Caps::builder("video/x-raw")
            .field("format", "I420")
            .build()
    } else {
        gst::Caps::builder("audio/x-raw")
            .field("format", "F32LE")
            .field("layout", "interleaved")
            .build()
    };
    let sink = gst_app::AppSink::builder().caps(&caps).sync(false).build();

    sink.set_property("async", false);

    let collected = Arc::clone(state);

    sink.set_callbacks(
        gst_app::AppSinkCallbacks::builder()
            .new_sample(move |s| {
                let sample = s.pull_sample().map_err(|_| gst::FlowError::Eos)?;
                let mut c = collected.lock().unwrap();
                let result = if video {
                    video_sample(&mut c, &sample, max_bytes)
                } else {
                    audio_sample(&mut c, &sample, max_bytes)
                };

                result.map(|_| gst::FlowSuccess::Ok).map_err(|e| {
                    c.error.get_or_insert(e);
                    gst::FlowError::Error
                })
            })
            .build(),
    );

    let sink: gst::Element = sink.upcast();

    pipeline
        .add_many([&convert, &sink])
        .map_err(|e| e.to_string())?;
    gst::Element::link_many([&convert, &sink]).map_err(|e| e.to_string())?;

    for e in [&convert, &sink] {
        e.sync_state_with_parent().map_err(|e| e.to_string())?;
    }

    pad.link(&convert.static_pad("sink").unwrap())
        .map_err(|e| format!("{e:?}"))?;

    let mut c = state.lock().unwrap();

    if video {
        c.video_pad = true;
    } else {
        c.audio_pad = true;
    }

    Ok(())
}

/// The pictures and the sound of an MP4, decoded by the system GStreamer.
pub fn decode(data: &[u8], info: &Mp4Info, max_bytes: u64) -> Result<Decoded, WebmError> {
    let unsupported = |why: Option<String>| WebmError::Unsupported {
        container: "mp4",
        codec: match why {
            Some(why) => format!("{} ({why})", info.codecs()),
            None => info.codecs(),
        },
    };

    if let Some(v) = info.video()
        && u64::from(v.width) * u64::from(v.height) > MAX_LUMA_SAMPLES
    {
        return Err(WebmError::TooLarge);
    }

    gst::init().map_err(|e| unsupported(Some(format!("GStreamer: {e}"))))?;

    let file = TempFile::write(data).map_err(|e| WebmError::Decode(e.to_string()))?;
    let pipeline = gst::Pipeline::new();
    let state = Arc::new(Mutex::new(Collected::default()));
    let (src, decodebin) = match (element("filesrc"), element("decodebin3")) {
        (Ok(s), Ok(d)) => (s, d),
        (Err(e), _) | (_, Err(e)) => return Err(unsupported(Some(e))),
    };

    src.set_property("location", file.0.to_string_lossy().as_ref());
    pipeline
        .add_many([&src, &decodebin])
        .map_err(|e| WebmError::Decode(e.to_string()))?;
    src.link(&decodebin)
        .map_err(|e| WebmError::Decode(e.to_string()))?;

    {
        let state = Arc::clone(&state);

        pipeline.connect_deep_element_added(move |_, _, e| {
            if let Some(f) = e.factory()
                && f.klass().contains("Decoder")
            {
                state.lock().unwrap().decoders.push(f.name().to_string());
            }
        });
    }

    {
        let state = Arc::clone(&state);
        let weak = pipeline.downgrade();

        decodebin.connect_pad_added(move |_, pad| {
            let Some(pipeline) = weak.upgrade() else {
                return;
            };
            let caps = pad.current_caps().unwrap_or_else(|| pad.query_caps(None));
            let Some(name) = caps.structure(0).map(|s| s.name().to_string()) else {
                return;
            };
            let video = name.starts_with("video/");

            if (video || name.starts_with("audio/"))
                && let Err(e) = branch(&pipeline, pad, &state, video, max_bytes)
            {
                state.lock().unwrap().error.get_or_insert(e);
            }
        });
    }

    let mut missing = vec![];
    let mut error = None;
    let started = pipeline.set_state(gst::State::Playing);
    let bus = pipeline.bus().unwrap();
    let deadline = Instant::now() + TIMEOUT;

    if started.is_err() {
        error = Some("the pipeline does not start".to_string());
    }

    while error.is_none() {
        if Instant::now() > deadline {
            error = Some("the decode takes more than 60 s".into());
            break;
        }

        let Some(msg) = bus.timed_pop(gst::ClockTime::from_mseconds(100)) else {
            continue;
        };

        match msg.view() {
            gst::MessageView::Eos(_) => break,
            gst::MessageView::Error(e) => error = Some(e.error().to_string()),
            gst::MessageView::Element(e) => {
                if let Some(s) = e.structure()
                    && s.name() == "missing-plugin"
                {
                    missing.push(
                        s.get::<String>("detail")
                            .or_else(|_| s.get::<String>("name"))
                            .unwrap_or_default(),
                    );
                }
            }
            _ => {}
        }
    }

    let _ = pipeline.set_state(gst::State::Null);
    drop(pipeline);
    drop(file);

    let c = std::mem::take(&mut *state.lock().unwrap());

    if c.too_large {
        return Err(WebmError::TooLarge);
    }

    // A track of the file that gave nothing: GStreamer has no decoder for it.
    let video_missing = info.video().is_some() && c.pictures.is_empty();
    let audio_missing = info.audio().is_some() && c.audio_start.is_none();

    if !missing.is_empty() {
        return Err(unsupported(Some(format!(
            "no GStreamer decoder: {}",
            missing.join(", ")
        ))));
    }

    if let Some(e) = error.or(c.error) {
        if video_missing && !c.video_pad || audio_missing && !c.audio_pad {
            return Err(unsupported(Some(e)));
        }

        return Err(WebmError::Decode(e));
    }

    if video_missing || audio_missing {
        return Err(unsupported(Some("a track gave no data".into())));
    }

    Ok(Decoded {
        width: c.width,
        height: c.height,
        matrix: c.matrix.unwrap_or(YuvMatrix::Bt601),
        full_range: c.full_range,
        pictures: c.pictures,
        audio: c
            .audio_start
            .map(|start| (c.audio, c.audio_rate as f32, start)),
        video_end: c.video_end,
        decoders: c.decoders,
    })
}
