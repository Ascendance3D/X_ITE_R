//! MovieTexture.js (GIF and APNG) with Browser/Texturing/GifMedia.js, Browser/Texturing/PNGMedia.js and the
//! apng-js Player.
//!
//! MovieTexture is an X3DSoundSourceNode, so an X3DTimeDependentNode: the Sound module's set_start, set_pause,
//! set_resume, set_stop, set_loop and set_time hooks call the functions here for it. Its media element is a
//! [`MovieMedia`] instead of an AudioElement; the sound of a MovieTexture is not part of this port.
//!
//! The frames are decoded with the `image` crate. Its GIF frames are composited like libgif's (transparent
//! index keeps the pixel below, disposal 2 clears the rectangle to transparent black, disposal 3 restores the
//! picture before the frame), its APNG frames like apng-js on its canvas for frames that start from a cleared
//! canvas. Not ported: apng-js draws frame 0 again over the last picture when it loops or stops, so a frame 0
//! that is not opaque over the whole image shows the pictures before it there.
//!
//! GifMedia takes the frame from the node's `elapsedTime` in equal steps (not from the frame delays). The
//! apng-js Player runs on `requestAnimationFrame` and the wall clock; here its tick runs at the start of each
//! set_time with the browser time as the clock. In the pinned X_ITE oracle only the APNG frames that do not
//! depend on that clock can be compared (oracle/tools/gen_movie_anim.py).
//!
//! The load states of PNG data follow apng-js parseAPNG ([`DecodeError`], oracle/tools/gen_movie_load_errors.py).
//! All decoded frames of one MovieTexture together are at most [`MAX_DECODED_BYTES`]; larger media are rejected
//! before the frames are allocated and the node stays IN_PROGRESS.

use crate::*;

/// The next token of a page video element: unique across the runtimes of a page.
static NEXT_VIDEO_TOKEN: std::sync::atomic::AtomicU64 = std::sync::atomic::AtomicU64::new(1);

/// A candidate that the page's video element loads.
pub(crate) struct PendingVideo {
    pub(crate) node: NodeId,
    /// The `url` field when the load started; another list is a newer load.
    urls: Vec<String>,
    /// The candidate's index in `urls`.
    index: usize,
    url: String,
}

/// The frames of a GIF or an APNG, full-size RGBA rows from the top.
pub struct MovieMedia {
    pub width: u32,
    pub height: u32,
    pub frames: Vec<Vec<u8>>,
    /// Frame delays in milliseconds.
    delays: Vec<f64>,
    kind: Kind,
    playback_rate: f64,
    /// The frame the texture holds (`updateTextureData (media .currentFrame)`).
    pub shown: usize,
    /// The audio track of a decoded WebM ([`crate::webm::audio`]); `AudioTrack::None` for every other media.
    pub audio: crate::webm::AudioTrack,
    /// Does the media take a `currentTime` (a decoded WebM does; GifMedia ignores it, PNGMedia only stops)?
    seekable: bool,
    /// Seconds added to the clock of a decoded WebM by a seek.
    offset: f64,
    /// The state of [`MovieTiming::Iso`]; unused in the pinned mode.
    iso: IsoState,
}

/// What ISO timing remembers of a media: whether it has been active since it loaded (an inactive media that never
/// played shows frame 0 or the last frame, one that played keeps the frame it stopped on), the movie time it
/// shows, and its `<video>` seek requests.
#[derive(Default)]
struct IsoState {
    played: bool,
    time: f64,
    seeks: MovieSeeks,
}

use super::movie_time::{
    self as iso, IsoClock, MovieTiming, SeekLedger as MovieSeeks, frame_at, frame_starts,
};

enum Kind {
    /// GifMedia: `cycle` and `loop` of its closure.
    Gif { cycle: f64, looping: bool },
    /// PNGMedia: `apng .numPlays` and the apng-js Player.
    Apng { num_plays: u32, player: Player },
    /// A video element of the page (the browser build, [`VideoOp`]): the page decodes and plays it, and gives the
    /// frame pixels and `currentTime` ([`Runtime::video_frame`]).
    Video {
        token: u64,
        duration: f64,
        current_time: f64,
        looping: bool,
        /// A frame arrived that the texture does not hold yet.
        dirty: bool,
    },
}

/// What the engine asks of the page's `<video>` elements (the browser build; [`Runtime::take_video_ops`]). The
/// page makes one element for each live token ([`Runtime::video_tokens`]) and removes the elements of the tokens
/// that are not live any more.
#[derive(Clone, Debug, PartialEq)]
pub enum VideoOp {
    /// MovieTexture.loadVideo: `crossOrigin = "anonymous"`, `preload = "auto"`, `playsInline`, `src = url`. The page
    /// answers [`Runtime::video_loaded`] on `loadeddata`, or [`Runtime::video_failed`] on `error` or `abort`.
    Load { token: u64, url: String },
    /// X3DSoundSourceNode.startAudioElement: `play ()`.
    Play(u64),
    /// X3DSoundSourceNode.stopAudioElement: `pause ()`.
    Pause(u64),
    /// X3DSoundSourceNode.set_start: `currentTime = 0`.
    Rewind(u64),
    /// X3DSoundSourceNode.set_loop: `loop`.
    Loop(u64, bool),
    /// MovieTexture.set_speed__: `playbackRate` and `preservesPitch`.
    Rate(u64, f64, bool),
    /// [`MovieTiming::Iso`]: seek the paused element to `time` seconds (`currentTime = time`). `id` only grows; the page
    /// answers [`Runtime::video_seeked`] with the id of the request whose picture it sends, and the engine drops any
    /// answer that is not of the newest request.
    Seek { token: u64, id: u64, time: f64 },
}

/// The apng-js Player (lib/index.js), without the canvas: frame `n` of [`MovieMedia::frames`] is the picture
/// after `renderNextFrame` drew it.
#[derive(Default)]
struct Player {
    current_frame_number: usize,
    ended: bool,
    paused: bool,
    num_plays: u32,
    /// `_rafId`: a tick is requested.
    raf: bool,
    next_render_time: f64,
}

/// Why a GIF or an APNG gives no media element.
#[derive(Debug, PartialEq)]
pub enum DecodeError {
    /// loadData's `catch`: the warning, the next URL, then FAILED_STATE. The JavaScript error name and message:
    /// libgif's on_error type (`Error: parse`) or what apng-js parseAPNG throws.
    Fail(&'static str, String),
    /// apng-js parseAPNG returns (does not throw) its error for data that is not a PNG or not an animated PNG:
    /// `setAPNG` is not awaited, its rejection is unhandled and the node stays IN_PROGRESS without a message.
    NotAnimated,
    /// The decoded frames would be larger than [`MAX_DECODED_BYTES`]: an error message and the node stays
    /// IN_PROGRESS, as X_ITE does when the browser cannot create the frames ("Out of memory at ImageData
    /// creation").
    TooLarge(String),
}

/// The largest size of all decoded frames of one MovieTexture together (RGBA, 4 bytes a pixel).
pub const MAX_DECODED_BYTES: u64 = 512 << 20;

/// libgif `load_raw` on_error type for a GIF it cannot parse.
pub const GIF_PARSE_ERROR: &str = "parse";

/// The bytes of one full-size RGBA frame, if `frames` of them fit in [`MAX_DECODED_BYTES`].
fn frame_bytes(width: u32, height: u32, frames: usize) -> Result<usize, DecodeError> {
    let one = u64::from(width)
        .checked_mul(u64::from(height))
        .and_then(|n| n.checked_mul(4));
    let all = one.and_then(|n| n.checked_mul(frames as u64));

    match (one, all) {
        (Some(one), Some(all)) if all <= MAX_DECODED_BYTES => Ok(one as usize),
        _ => Err(DecodeError::TooLarge(format!(
            "MovieTexture: {frames} frames of {width} x {height} pixels are more than the limit of {} MiB.",
            MAX_DECODED_BYTES >> 20
        ))),
    }
}

/// The number of images of a GIF from its block structure, without decoding (`None` if the blocks are not
/// complete; the decoder then finds the error or the end).
fn gif_image_count(d: &[u8]) -> Option<usize> {
    fn skip_sub_blocks(d: &[u8], mut i: usize) -> Option<usize> {
        loop {
            let length = *d.get(i)? as usize;
            i = i.checked_add(1 + length)?;

            if length == 0 {
                return Some(i);
            }
        }
    }

    let color_table = |flags: u8| {
        if flags & 0x80 != 0 {
            3 << ((flags & 7) + 1)
        } else {
            0
        }
    };
    let mut i = 13 + color_table(*d.get(10)?);
    let mut count = 0;

    loop {
        match *d.get(i)? {
            0x3b => return Some(count),
            0x21 => i = skip_sub_blocks(d, i + 2)?,
            0x2c => {
                i += 10 + color_table(*d.get(i + 9)?);
                i = skip_sub_blocks(d, i + 1)?;
                count += 1;
            }
            _ => return None,
        }
    }
}

/// An eachChunk callback: chunk type, offset and length; `false` ends the walk.
type ChunkCallback<'a> = dyn FnMut(&[u8], u64, u64) -> Result<bool, DecodeError> + 'a;

/// The parts of an APNG that apng-js parseAPNG reads before the browser decodes the frame images.
struct ApngStructure {
    width: u32,
    height: u32,
    /// `frame .delay` of each fcTL in milliseconds.
    delays: Vec<f64>,
}

/// apng-js parseAPNG without the images: `Ok (None)` where it returns errNotPNG or errNotAPNG, `Err` with the
/// name and message of what it throws. Its `DataView` reads past the end of the data throw; the frames' IHDR
/// copies (`headerDataBytes .set`) throw without an IHDR or with one shorter than 8 bytes.
fn apng_structure(b: &[u8]) -> Result<Option<ApngStructure>, DecodeError> {
    const SIGNATURE: [u8; 8] = [0x89, 0x50, 0x4e, 0x47, 0x0d, 0x0a, 0x1a, 0x0a];
    let range = || {
        DecodeError::Fail(
            "RangeError",
            "Offset is outside the bounds of the DataView".into(),
        )
    };
    let read = |off: u64, n: usize| -> Result<u32, DecodeError> {
        let o = usize::try_from(off).map_err(|_| range())?;
        let bytes = b
            .get(o..o.checked_add(n).ok_or_else(range)?)
            .ok_or_else(range)?;
        Ok(bytes.iter().fold(0, |v, &x| v << 8 | u32::from(x)))
    };

    // eachChunk: chunk type of up to 4 bytes (readString of a subarray), the walk ends after `false`, IEND or the
    // end of the data.
    let each_chunk = |f: &mut ChunkCallback| {
        let mut off = 8u64;

        loop {
            let length = u64::from(read(off, 4)?);
            let start = (off + 4).min(b.len() as u64) as usize;
            let kind = &b[start..(off + 8).min(b.len() as u64) as usize];
            let more = f(kind, off, length)?;
            off += 12 + length;

            if !more || kind == b"IEND" || off >= b.len() as u64 {
                return Ok(());
            }
        }
    };

    if b.get(..8) != Some(&SIGNATURE[..]) {
        return Ok(None);
    }

    let mut animated = false;
    each_chunk(&mut |kind, _, _| {
        animated = kind == b"acTL";
        Ok(!animated)
    })?;

    if !animated {
        return Ok(None);
    }

    let (mut width, mut height, mut header, mut delays) = (0, 0, None, vec![]);
    each_chunk(&mut |kind, off, length| {
        match kind {
            b"IHDR" => {
                header = Some(length.min((b.len() as u64).saturating_sub(off + 8)));
                width = read(off + 8, 4)?;
                height = read(off + 12, 4)?;
            }
            b"acTL" => {
                read(off + 12, 4)?;
            }
            b"fcTL" => {
                read(off + 12, 4)?;
                read(off + 16, 4)?;
                read(off + 20, 4)?;
                read(off + 24, 4)?;
                let numerator = read(off + 28, 2)?;
                let denominator = match read(off + 30, 2)? {
                    0 => 100,
                    d => d,
                };
                read(off + 32, 1)?;
                read(off + 33, 1)?;
                // At least 100 ms if 10 ms or less.
                let delay = 1000.0 * f64::from(numerator) / f64::from(denominator);
                delays.push(if delay <= 10.0 { 100.0 } else { delay });
            }
            _ => {}
        }
        Ok(true)
    })?;

    if delays.is_empty() {
        return Ok(None);
    }

    match header {
        None => Err(DecodeError::Fail(
            "TypeError",
            "Cannot read properties of null (reading 'set')".into(),
        )),
        Some(length) if length < 8 => Err(DecodeError::Fail(
            "RangeError",
            "offset is out of bounds".into(),
        )),
        Some(_) => Ok(Some(ApngStructure {
            width,
            height,
            delays,
        })),
    }
}

impl MovieMedia {
    /// libgif `load_raw`: the frames and their delays (centiseconds × 10).
    pub fn decode_gif(data: &[u8]) -> Result<MovieMedia, DecodeError> {
        use image::{AnimationDecoder, ImageDecoder};

        let parse = || DecodeError::Fail("Error", GIF_PARSE_ERROR.into());
        let mut decoder =
            image::codecs::gif::GifDecoder::new(std::io::Cursor::new(data)).map_err(|_| parse())?;
        let (width, height) = decoder.dimensions();
        // The size of all images before the decoder allocates its canvas (at least one).
        let one = frame_bytes(width, height, gif_image_count(data).unwrap_or(1).max(1))?;
        decoder
            .set_limits(image::Limits::default())
            .map_err(|_| parse())?;

        let (mut frames, mut delays) = (vec![], vec![]);

        for frame in decoder.into_frames() {
            let frame = frame.map_err(|_| parse())?;
            // The block count can be lower than the frames of a GIF without a trailer.
            frame_bytes(width, height, frames.len() + 1)?;
            delays.push(ratio_ms(frame.delay()));
            frames.push(frame.into_buffer().into_raw());
        }

        if frames.is_empty() || frames.iter().any(|f| f.len() != one) {
            return Err(parse());
        }

        Ok(MovieMedia {
            width,
            height,
            frames,
            delays,
            kind: Kind::Gif {
                cycle: 0.0,
                looping: false,
            },
            playback_rate: 1.0,
            shown: 0,
            audio: crate::webm::AudioTrack::None,
            seekable: false,
            offset: 0.0,
            iso: IsoState::default(),
        })
    }

    /// A WebM with a VP9 or VP8 video track, decoded here ([`crate::webm`]) where the page has no `<video>` element. The
    /// frames play like a GIF's (GifMedia's mapping of `elapsedTime` to a frame, the frame times as delays): the Vorbis
    /// sound of the file plays through the node's audio source ([`Runtime::movie_set_frames`]); a WebM takes
    /// `currentTime` ([`Runtime::movie_seek`]). An error when the file is not a WebM with a VP9 or VP8
    /// track that decodes; it says why (the caller logs it and treats the video as one the engine cannot show).
    pub fn decode_webm(data: &[u8]) -> Result<MovieMedia, crate::webm::WebmError> {
        let video = crate::webm::decode(data, MAX_DECODED_BYTES)?;
        let n = video.frames.len();

        frame_bytes(video.width, video.height, n).map_err(|_| crate::webm::WebmError::TooLarge)?;

        // A frame lasts until the next one starts; the last repeats the step before it.
        let times: Vec<f64> = video.frames.iter().map(|f| f.time_ms).collect();
        let delays: Vec<f64> = (0..n)
            .map(|i| match times.get(i + 1) {
                Some(next) => next - times[i],
                None if i > 0 => times[i] - times[i - 1],
                None => 0.0,
            })
            .collect();

        Ok(MovieMedia {
            width: video.width,
            height: video.height,
            frames: video.frames.into_iter().map(|f| f.rgba).collect(),
            delays,
            kind: Kind::Gif {
                cycle: 0.0,
                looping: false,
            },
            playback_rate: 1.0,
            shown: 0,
            audio: crate::webm::audio(data),
            seekable: true,
            offset: 0.0,
            iso: IsoState::default(),
        })
    }

    /// apng-js parseAPNG, the frame images and PNGMedia (whose `player .stop ()` renders frame 0). The size, the
    /// frame count and the delays are those of parseAPNG. The browser decodes each frame image; the frames are
    /// those the `image` crate composites until it finds an error. A frame after that error keeps the picture
    /// before it: X_ITE draws nothing for frame data the browser cannot decode (an empty canvas for an invalid
    /// first frame); what it draws for the rows of a partly cut frame is not known.
    pub fn decode_apng(data: &[u8]) -> Result<MovieMedia, DecodeError> {
        use image::{AnimationDecoder, ImageDecoder};

        let ApngStructure {
            width,
            height,
            delays,
        } = apng_structure(data)?.ok_or(DecodeError::NotAnimated)?;
        let one = frame_bytes(width, height, delays.len())?;
        let mut frames: Vec<Vec<u8>> = Vec::with_capacity(delays.len());

        let decoder = image::codecs::png::PngDecoder::with_limits(
            std::io::Cursor::new(data),
            image::Limits::default(),
        )
        .ok()
        .filter(|d| d.dimensions() == (width, height) && d.is_apng().unwrap_or(false));

        if let Some(apng) = decoder.and_then(|d| d.apng().ok()) {
            for frame in apng.into_frames() {
                match frame.map(|f| f.into_buffer().into_raw()) {
                    Ok(f) if f.len() == one && frames.len() < delays.len() => frames.push(f),
                    _ => break,
                }
            }
        }

        while frames.len() < delays.len() {
            frames.push(frames.last().cloned().unwrap_or_else(|| vec![0; one]));
        }

        let mut media = MovieMedia {
            width,
            height,
            frames,
            delays,
            kind: Kind::Apng {
                num_plays: 0,
                player: Player::default(),
            },
            playback_rate: 1.0,
            shown: 0,
            audio: crate::webm::AudioTrack::None,
            seekable: false,
            offset: 0.0,
            iso: IsoState::default(),
        };

        media.player_stop();
        Ok(media)
    }

    /// `duration` in seconds: libgif get_duration_ms, apng .playTime.
    pub fn duration(&self) -> f64 {
        if let Kind::Video { duration, .. } = self.kind {
            return duration;
        }

        self.play_time() / 1000.0
    }

    /// The token of the page's video element, if this media is a video.
    pub fn video_token(&self) -> Option<u64> {
        match self.kind {
            Kind::Video { token, .. } => Some(token),
            _ => None,
        }
    }

    fn play_time(&self) -> f64 {
        self.delays.iter().sum()
    }

    /// `cycle`: Math.floor (elapsedTime / duration).
    fn cycle(&self, elapsed: f64) -> f64 {
        (elapsed / self.duration()).floor()
    }

    /// `currentTime`.
    pub fn current_time(&self, elapsed: f64) -> f64 {
        match &self.kind {
            Kind::Gif { cycle, looping } => {
                if self.offset != 0.0 {
                    // After a seek: the clock plus the seek, ending at the duration unless the media loops.
                    let t = elapsed * self.playback_rate + self.offset;

                    return if *looping {
                        t.rem_euclid(self.duration())
                    } else {
                        t.clamp(0.0, self.duration())
                    };
                }
                if !looping && *cycle < self.cycle(elapsed) {
                    return self.duration();
                }
                (elapsed * self.playback_rate) % self.duration()
            }
            Kind::Apng { player, .. } => {
                if player.ended {
                    self.duration()
                } else {
                    player.current_frame_number as f64 / self.frames.len() as f64 * self.duration()
                }
            }
            Kind::Video { current_time, .. } => *current_time,
        }
    }

    /// `currentTime = 0`: GifMedia ignores it, PNGMedia stops the player.
    pub fn set_current_time_zero(&mut self) {
        self.offset = 0.0;
        if matches!(self.kind, Kind::Apng { .. }) {
            self.player_stop();
        }
    }

    /// What a `currentTime = t` request does, measured in the pinned Chrome (a `video` element): a time before 0
    /// gives 0 and a time after the end gives the duration. `None`: no seek (the media takes none, or `t` is NaN,
    /// which Chrome rejects with a TypeError).
    pub fn seek_target(&self, t: f64) -> Option<f64> {
        (self.seekable && !t.is_nan()).then(|| t.clamp(0.0, self.duration()))
    }

    /// Moves the clock so that `current_time (elapsed)` is `target` (a value of [`Self::seek_target`]).
    pub fn seek_to(&mut self, target: f64, elapsed: f64) {
        if self.seekable {
            self.offset = target - elapsed * self.playback_rate;
        }
    }

    /// The index of `currentFrame`. GifMedia: Math.max (Math.ceil (currentTime / duration * length) - 1, 0);
    /// `None` where X_ITE reads `get_frames () [NaN]` (a GIF of no duration) and throws.
    pub fn current_frame(&self, elapsed: f64) -> Option<usize> {
        match &self.kind {
            Kind::Gif { .. } => {
                let length = self.frames.len() as f64;
                let frame =
                    ((self.current_time(elapsed) / self.duration() * length).ceil() - 1.0).max(0.0);

                (!frame.is_nan() && frame < length).then_some(frame as usize)
            }
            Kind::Apng { player, .. } => Some(player.current_frame_number),
            Kind::Video { .. } => Some(0),
        }
    }

    /// `loop`.
    pub fn looping(&self) -> bool {
        match &self.kind {
            Kind::Gif { looping, .. } => *looping,
            Kind::Apng { num_plays, .. } => *num_plays == 0,
            Kind::Video { looping, .. } => *looping,
        }
    }

    /// `loop = value`.
    pub fn set_loop(&mut self, value: bool, elapsed: f64) {
        let cycle_now = self.cycle(elapsed);

        match &mut self.kind {
            Kind::Gif { cycle, looping } => {
                *cycle = cycle_now;
                *looping = value;
            }
            Kind::Apng { num_plays, .. } => *num_plays = if value { 0 } else { 1 },
            Kind::Video { looping, .. } => *looping = value,
        }
    }

    /// `playbackRate = value` (PNGMedia: no negative values).
    pub fn set_playback_rate(&mut self, value: f64) {
        self.playback_rate = match self.kind {
            Kind::Gif { .. } => value,
            Kind::Apng { .. } => value.max(0.0),
            // Chrome throws for a negative playbackRate (MovieTexture.set_speed__ then plays at 1).
            Kind::Video { .. } => value,
        };
    }

    /// `play ()` at `now` (milliseconds): GifMedia keeps the cycle, PNGMedia plays the player.
    pub fn play(&mut self, elapsed: f64, now: f64) {
        let cycle_now = self.cycle(elapsed);

        match &mut self.kind {
            Kind::Gif { cycle, .. } => *cycle = cycle_now,
            Kind::Apng { .. } => self.player_play(now),
            Kind::Video { .. } => {}
        }
    }

    /// `pause ()`: libgif's player pause (no effect on currentFrame), the apng-js Player's.
    pub fn pause(&mut self) {
        if let Kind::Apng { player, .. } = &mut self.kind {
            if !player.paused {
                player.raf = false;
                player.paused = true;
            }
        }
    }

    /// The apng-js tick of a requestAnimationFrame callback at `now` (milliseconds).
    pub fn tick(&mut self, now: f64) {
        let rate = self.playback_rate;
        let play_time = self.play_time();
        let Kind::Apng { player, .. } = &mut self.kind else {
            return;
        };

        if !player.raf {
            return;
        }

        if player.ended || player.paused {
            player.raf = false;
            return;
        }

        if now >= player.next_render_time {
            // apng-js adds the missed plays one at a time. At a high rate a play is less than the precision
            // of `now` (milliseconds since 1970), and that loop does not end: one division gives the same
            // count. `min (now)` keeps the rounding of the product from going past `now`.
            let step = play_time / rate;
            let behind = now - player.next_render_time;

            if step > 0.0 && behind >= step {
                let plays = (behind / step).floor();

                player.next_render_time = (player.next_render_time + plays * step).min(now);
                player.num_plays = player
                    .num_plays
                    .saturating_add(plays.min(u32::MAX as f64) as u32);
            }

            loop {
                self.render_next_frame();

                let Kind::Apng { player, .. } = &mut self.kind else {
                    unreachable!()
                };
                let last = player.next_render_time;
                player.next_render_time += self.delays[player.current_frame_number] / rate;

                // A delay that does not move the time (less than its precision) renders 1 frame each tick.
                if player.ended
                    || player.paused
                    || now <= player.next_render_time
                    || player.next_render_time <= last
                    || player.next_render_time.is_nan()
                {
                    break;
                }
            }
        }
    }

    /// Player.play.
    fn player_play(&mut self, now: f64) {
        if matches!(&self.kind, Kind::Apng { player, .. } if player.ended) {
            self.player_stop();
        }

        let rate = self.playback_rate;
        let Kind::Apng { player, .. } = &mut self.kind else {
            return;
        };

        player.paused = false;
        player.next_render_time = now + self.delays[player.current_frame_number] / rate;
        player.raf = true;
    }

    /// Player.stop: rewinds and renders the first frame.
    fn player_stop(&mut self) {
        let Kind::Apng { player, .. } = &mut self.kind else {
            return;
        };

        player.raf = false;
        player.num_plays = 0;
        player.ended = false;
        player.paused = true;
        player.current_frame_number = usize::MAX;
        self.render_next_frame();
    }

    /// Player.renderNextFrame without the drawing: the frame number and the end of the plays.
    fn render_next_frame(&mut self) {
        let length = self.frames.len();
        let Kind::Apng { num_plays, player } = &mut self.kind else {
            return;
        };

        player.current_frame_number = player.current_frame_number.wrapping_add(1) % length;

        if player.current_frame_number == length - 1 {
            player.num_plays = player.num_plays.saturating_add(1);

            if *num_plays != 0 && player.num_plays >= *num_plays {
                player.ended = true;
                player.paused = true;
            }
        }
    }
}

fn ratio_ms(delay: image::Delay) -> f64 {
    let (n, d) = delay.numer_denom_ms();
    n as f64 / d as f64
}

impl Runtime {
    /// The MovieTexture constructor: the X3DTimeDependentNode private fields (the node's State is its
    /// X3DUrlObject state; its time state is kept in `time_states`, as for AudioClip).
    pub(crate) fn movie_construct(&mut self, node: NodeId) {
        self.add_private_field(
            node,
            "initialized",
            access::OUTPUT_ONLY,
            FieldValue::SFTime(0.0),
        );
        self.add_private_field(
            node,
            "evenLive",
            access::INPUT_OUTPUT,
            FieldValue::SFBool(false),
        );
        self.time_states.insert(node, Default::default());

        // X3DSoundSourceNode: audioSource = new GainNode (audioContext, { gain: 0 }).
        let a = self.audio().gain(0.0);
        self.snd(node).audio_source = Some(a);
    }

    /// MovieTexture.initialize between X3DTexture2DNode's and X3DUrlObject's: X3DSoundSourceNode.initialize
    /// (X3DTimeDependentNode.initialize; set_gain__ only mutes the element) and the speed and pitch interests.
    pub(crate) fn movie_initialize(&mut self, node: NodeId) {
        self.time_dependent_initialize(node, Cb::TimeSetEnabled);
        self.sound_interest(node, "speed", super::sound::SoundCb::Speed);
        self.sound_interest(node, "pitch", super::sound::SoundCb::Speed);
        // X3DSoundSourceNode.initialize: the gain interest and set_gain__.
        self.sound_interest(node, "gain", super::sound::SoundCb::Gain);
        self.source_set_gain(node);
    }

    fn movie(&mut self, node: NodeId) -> Option<&mut MovieMedia> {
        self.sound.get_mut(&node)?.movie.as_mut()
    }

    /// The decoded frame the MovieTexture's texture holds: size, RGBA rows from the top and the frame index.
    pub fn movie_texture_frame(&self, node: NodeId) -> Option<(u32, u32, &[u8], usize)> {
        let m = self.sound.get(&node)?.movie.as_ref()?;
        Some((m.width, m.height, &m.frames[m.shown], m.shown))
    }

    /// Has the MovieTexture a decoded media element (GIF or APNG)?
    pub fn movie_has_frames(&self, node: NodeId) -> bool {
        self.sound.get(&node).is_some_and(|s| s.movie.is_some())
    }

    fn elapsed_value(&self, node: NodeId) -> f64 {
        self.get_f64(node, "elapsedTime")
    }

    /// MovieTexture.setGif / setAPNG: the media element, the first frame as texture, the outputs, COMPLETE_STATE
    /// and set_speed__.
    pub(crate) fn movie_set_frames(&mut self, node: NodeId, mut media: MovieMedia) {
        let (width, height, duration) = (media.width, media.height, media.duration());
        let audio = std::mem::replace(&mut media.audio, crate::webm::AudioTrack::None);
        let mut has_sound = false;

        // The previous media element's sound goes with it.
        self.movie_release_audio(node);

        match audio {
            crate::webm::AudioTrack::None => {}
            crate::webm::AudioTrack::Decoded { channels, rate } => {
                has_sound = true;

                match self.audio().buffer_from_planar(channels, rate) {
                    Ok(buffer) => {
                        let source = self.snd(node).audio_source.unwrap();
                        let element = super::sound::AudioElement::create(self.audio(), source, Some(buffer));

                        self.snd(node).media = Some(element);
                    }
                    Err(_) => self.log_warn(
                        "MovieTexture: the audio track of the video has no samples; the video plays silently."
                            .to_string(),
                    ),
                }
            }
            // The track is there and cannot be played: hasSound is TRUE as the page's would be, the video keeps
            // rendering and plays silently, and the diagnostic says why (BR1.8).
            crate::webm::AudioTrack::Unsupported(codec) => {
                has_sound = true;
                self.log_warn(format!(
                    "MovieTexture: the audio track of the video ({codec}) cannot be decoded by this build; the video plays silently."
                ));
            }
            crate::webm::AudioTrack::Corrupt => self.log_warn(
                "MovieTexture: the audio track of the video does not decode; the video plays silently."
                    .to_string(),
            ),
        }

        self.movie_set_media_element(node, Some(media));
        self.set_texture_2d_size(node, width, height);
        self.movie_update_outputs(node, width as i32, height as i32, 4, has_sound, duration);
        self.set_load_state(node, nodes::LoadState::Complete);
        self.movie_set_speed(node);
    }

    /// The audio element of the node, paused and released.
    fn movie_release_audio(&mut self, node: NodeId) {
        self.media_set(node, |e, m| m.pause(e));

        if let Some(old) = self.sound.get_mut(&node).and_then(|s| s.media.take()) {
            self.audio().release(old.source());
        }
    }

    /// MovieTexture.unloadData and the end of a loadData that found nothing: clearTexture, setMediaElement (null),
    /// updateOutputs (0, 0, 0, false, -1).
    pub(crate) fn movie_clear(&mut self, node: NodeId) {
        self.movie_release_audio(node);
        self.set_texture_2d_size(node, 1, 1);
        self.movie_set_media_element(node, None);
        self.movie_update_outputs(node, 0, 0, 0, false, -1.0);
    }

    /// MovieTexture.updateOutputs (hasSound is FALSE for a GIF or an APNG).
    fn movie_update_outputs(
        &mut self,
        node: NodeId,
        width: i32,
        height: i32,
        color_depth: i32,
        has_sound: bool,
        duration: f64,
    ) {
        for (name, value) in [
            ("width", FieldValue::SFInt32(width)),
            ("height", FieldValue::SFInt32(height)),
            ("colorDepth", FieldValue::SFInt32(color_depth)),
            ("hasSound", FieldValue::SFBool(has_sound)),
            ("duration_changed", FieldValue::SFTime(duration)),
        ] {
            if let Some(k) = self.pre(node, name) {
                self.set_value(k, value);
            }
        }
    }

    /// X3DSoundSourceNode.setMediaElement.
    fn movie_set_media_element(&mut self, node: NodeId, value: Option<MovieMedia>) {
        // getBrowser () .stopAudioElement (this .mediaElement)
        self.movie_pause(node);

        let Some(value) = value else {
            if let Some(s) = self.sound.get_mut(&node) {
                s.movie = None;
            }
            return;
        };

        self.snd(node).movie = Some(value);

        // Init mediaElement (set_gain__ only mutes it).
        self.movie_set_loop(node);

        // Handle events.
        self.time_reset_elapsed(node);

        if self.get_bool(node, "isActive") {
            if self.get_bool(node, "isPaused") {
                self.movie_pause(node);
            } else if self.time_node_live(node) {
                self.movie_set_start(node);
            } else {
                self.movie_pause(node);
            }
        } else {
            self.movie_pause(node);
        }
    }

    /// X3DSoundSourceNode.set_loop.
    pub(crate) fn movie_set_loop(&mut self, node: NodeId) {
        let (value, elapsed) = (self.time_loop(node), self.elapsed_value(node));

        self.media_set(node, |e, m| m.set_loop(e, value));

        if let Some(m) = self.movie(node) {
            m.set_loop(value, elapsed);

            if let Some(token) = m.video_token() {
                self.video_ops.push(VideoOp::Loop(token, value));
            }
        }
    }

    /// MovieTexture.set_speed__: playbackRate = speed × max (pitch, 0).
    pub(crate) fn movie_set_speed(&mut self, node: NodeId) {
        if self.movie_timing == MovieTiming::Iso {
            return self.movie_iso_set_speed(node, false);
        }

        let rate = self.get_f64(node, "speed") * self.get_f64(node, "pitch").max(0.0);

        let pitch = self.get_f64(node, "pitch");

        // The sound plays at the same rate (Chrome throws for a negative one; set_speed__ then plays at 1). The
        // pitch follows the rate: `preservesPitch` is not reproduced.
        self.media_set(node, |e, m| {
            m.set_playback_rate(e, if rate < 0.0 { 1.0 } else { rate })
        });

        if let Some(m) = self.movie(node) {
            m.set_playback_rate(rate);

            if let Some(token) = m.video_token() {
                // Chrome throws for a negative playbackRate; set_speed__ then logs it and plays at 1.
                self.video_ops.push(VideoOp::Rate(
                    token,
                    if rate < 0.0 { 1.0 } else { rate },
                    pitch == 1.0,
                ));
            }
        }
    }

    /// The ISO 18.4.2 `set_speed`: ignored while the node is active (`force`: the start takes the field's value), applied
    /// while it is inactive. Negative rates are legal; the sound still plays at 1 (it cannot run backwards).
    fn movie_iso_set_speed(&mut self, node: NodeId, force: bool) {
        let active = self.get_bool(node, "isActive");

        if active && !force {
            return;
        }

        let rate = self.get_f64(node, "speed") * self.get_f64(node, "pitch").max(0.0);

        self.media_set(node, |e, m| {
            m.set_playback_rate(e, if rate < 0.0 { 1.0 } else { rate })
        });

        if let Some(m) = self.movie(node) {
            m.set_playback_rate(rate);
        }

        if !active {
            self.movie_iso_show_inactive(node);
        }
    }

    /// An inactive media that never played shows frame 0 (`rate >= 0`) or the last frame (`rate < 0`), 8.2.4.
    fn movie_iso_show_inactive(&mut self, node: NodeId) {
        let Some(m) = self.movie(node) else {
            return;
        };

        if m.iso.played {
            return;
        }

        let t = iso::inactive_time(m.playback_rate, m.duration());

        self.movie_iso_show(node, t, true);
    }

    /// Shows the movie time `t`: a frame list picks its frame, a `<video>` is asked to seek (`force`: the request
    /// supersedes a seek in flight).
    fn movie_iso_show(&mut self, node: NodeId, t: f64, force: bool) {
        let Some(m) = self.movie(node) else {
            return;
        };

        m.iso.time = t;

        if let Some(token) = m.video_token() {
            if let Some(id) = m.iso.seeks.request(t, force) {
                self.video_ops.push(VideoOp::Seek { token, id, time: t });
            }
            return;
        }

        let frame = frame_at(&frame_starts(&m.delays), t);

        if frame != m.shown {
            m.shown = frame;
            self.touch(node);
        }
    }

    /// X3DSoundSourceNode.set_start.
    pub(crate) fn movie_set_start(&mut self, node: NodeId) {
        if !self.movie_has_frames(node) {
            return;
        }

        if let Some(k) = self.pre(node, "elapsedTime") {
            self.set_value(k, FieldValue::SFTime(0.0));
        }

        if self.movie_timing == MovieTiming::Iso {
            // The clause's clock starts here: the rate is the field's now, the first picture is time 0 (or the
            // end of the movie for a negative rate).
            self.movie_iso_set_speed(node, true);

            if let Some(m) = self.movie(node) {
                m.iso.played = true;

                let t = iso::iso_time(IsoClock {
                    elapsed: 0.0,
                    rate: m.playback_rate,
                    duration: m.duration(),
                    looping: true,
                })
                .0;

                self.movie_iso_show(node, t, true);
            }

            self.audio_element_started = true;
            self.media_set(node, |e, m| {
                m.set_current_time(e, 0.0);
                m.play(e)
            });
            return;
        }

        let (elapsed, now) = (self.elapsed_value(node), self.now * 1000.0);

        if let Some(m) = self.movie(node) {
            m.set_current_time_zero();
            m.play(elapsed, now);

            if let Some(token) = m.video_token() {
                self.video_ops.push(VideoOp::Rewind(token));
                self.video_ops.push(VideoOp::Play(token));
            }

            // getBrowser () .startAudioElement (this .mediaElement)
            self.audio_element_started = true;
        }

        self.media_set(node, |e, m| {
            m.set_current_time(e, 0.0);
            m.play(e)
        });
    }

    /// `mediaElement .currentTime = t` of a MovieTexture: a decoded WebM takes it, clamped to [0, duration] as Chrome
    /// does (measured: -5 gives 0, 100 gives the duration), and its sound follows. GifMedia ignores `currentTime`
    /// and PNGMedia only stops, so those, and a NaN, are not seeks: nothing moves and a diagnostic is logged.
    /// Returns the position that was set.
    pub fn movie_seek(&mut self, node: NodeId, t: f64) -> Option<f64> {
        let elapsed = self.elapsed_value(node);
        let target = self.movie(node).and_then(|m| m.seek_target(t));

        let Some(target) = target else {
            self.log_warn(format!(
                "MovieTexture: the media cannot seek to {t}; the request is ignored."
            ));
            return None;
        };

        if let Some(m) = self.movie(node) {
            m.seek_to(target, elapsed);
        }

        self.media_set(node, |e, m| m.set_current_time(e, target));
        Some(target)
    }

    /// The playback position of the node's media in seconds (`currentTime`).
    pub fn movie_current_time(&mut self, node: NodeId) -> Option<f64> {
        let elapsed = self.elapsed_value(node);
        let timing = self.movie_timing;

        self.movie(node).map(|m| match timing {
            MovieTiming::Iso => m.iso.time,
            MovieTiming::Pinned => m.current_time(elapsed),
        })
    }

    /// Chooses the ISO 19775-1 mapping (the default) or the pinned X_ITE 16.5.0 behavior for MovieTextures made or
    /// changed from now on. Set it before the world loads.
    pub fn set_movie_timing(&mut self, timing: MovieTiming) {
        self.movie_timing = timing;
    }

    /// set_pause, set_stop: stopAudioElement.
    pub(crate) fn movie_pause(&mut self, node: NodeId) {
        self.media_set(node, |e, m| m.pause(e));

        if let Some(m) = self.movie(node) {
            m.pause();

            if let Some(token) = m.video_token() {
                self.video_ops.push(VideoOp::Pause(token));
            }
        }
    }

    /// set_resume: startAudioElement.
    pub(crate) fn movie_resume(&mut self, node: NodeId) {
        let (elapsed, now) = (self.elapsed_value(node), self.now * 1000.0);

        self.media_set(node, |e, m| m.play(e));

        if let Some(m) = self.movie(node) {
            m.play(elapsed, now);

            // ISO timing: the engine drives the paused element by seeks; it is never played.
            if let (Some(token), MovieTiming::Pinned) = (m.video_token(), self.movie_timing) {
                self.video_ops.push(VideoOp::Play(token));
            }

            self.audio_element_started = true;
        }
    }

    /// MovieTexture.set_time: X3DSoundSourceNode.set_time, then `updateTextureData (media .currentFrame)`. The
    /// apng-js tick of the frame runs first.
    pub(crate) fn movie_set_time(&mut self, node: NodeId) {
        if self.movie_timing == MovieTiming::Iso {
            return self.movie_iso_set_time(node);
        }

        let now = self.now * 1000.0;

        let Some(m) = self.movie(node) else {
            return;
        };

        m.tick(now);

        let t = self.time_elapsed_time(node);

        if let Some(k) = self.pre(node, "elapsedTime") {
            self.set_value(k, FieldValue::SFTime(t));
        }

        let elapsed = self.elapsed_value(node);
        let ended = self
            .movie(node)
            .is_some_and(|m| !(m.current_time(elapsed) < m.duration()));

        // set_end
        if ended && !self.time_loop(node) {
            self.time_stop_node(node);
        }

        let elapsed = self.elapsed_value(node);
        let Some(m) = self.movie(node) else {
            return;
        };

        if let Some(frame) = m.current_frame(elapsed) {
            let fresh = match &mut m.kind {
                Kind::Video { dirty, .. } => std::mem::take(dirty),
                _ => false,
            };

            if frame != m.shown || fresh {
                m.shown = frame;
                self.touch(node);
            }

            self.add_node_event(node);
        }
    }

    /// The ISO 18.4.2 set_time: the movie time of the active node from its elapsed time, the frame of that time, and
    /// the end of a play that does not loop.
    fn movie_iso_set_time(&mut self, node: NodeId) {
        if !self.movie_has_frames(node) {
            return;
        }

        let t = self.time_elapsed_time(node);

        if let Some(k) = self.pre(node, "elapsedTime") {
            self.set_value(k, FieldValue::SFTime(t));
        }

        let looping = self.time_loop(node);
        let Some(m) = self.movie(node) else {
            return;
        };

        m.iso.played = true;

        let (time, finished) = iso::iso_time(IsoClock {
            elapsed: t,
            rate: m.playback_rate,
            duration: m.duration(),
            looping,
        });

        self.movie_iso_show(node, time, false);

        // set_end: the last picture of the play stays as the texture.
        if finished && !looping {
            self.time_stop_node(node);
        }

        self.add_node_event(node);
    }

    /// The page's video element `token` finished seek request `id`: `current_time` and the picture `rgba` (empty when
    /// the page could not read it). An answer that is not of the newest request is obsolete and dropped, so an old
    /// frame never overwrites a newer one. Returns whether the token is this runtime's.
    pub fn video_seeked(&mut self, token: u64, id: u64, current_time: f64, rgba: Vec<u8>) -> bool {
        let mut touched = None;
        let mut found = false;

        for (node, s) in self.sound.iter_mut() {
            let Some(m) = s.movie.as_mut() else {
                continue;
            };

            if m.video_token() != Some(token) {
                continue;
            }

            found = true;

            if !m.iso.seeks.accept(id) {
                break;
            }

            if let Kind::Video {
                current_time: c, ..
            } = &mut m.kind
            {
                *c = current_time;
            }

            if rgba.len() == m.frames[0].len() {
                m.frames[0] = rgba;
                touched = Some(*node);
            }
            break;
        }

        if let Some(node) = touched {
            self.touch(node);
        }

        found
    }

    /// A MovieTexture candidate that is no GIF or PNG, in a browser build: the page's `<video>` element loads it
    /// (MovieTexture.loadVideo). The node stays IN_PROGRESS until the page answers [`Runtime::video_loaded`] or
    /// [`Runtime::video_failed`]; the candidates after it wait for that.
    pub(crate) fn movie_load_video(
        &mut self,
        node: NodeId,
        urls: Vec<String>,
        index: usize,
        file_url: url::Url,
    ) {
        let token = NEXT_VIDEO_TOKEN.fetch_add(1, std::sync::atomic::Ordering::Relaxed);

        self.pending_videos.insert(
            token,
            PendingVideo {
                node,
                urls,
                index,
                url: file_url.to_string(),
            },
        );
        self.video_ops.push(VideoOp::Load {
            token,
            url: file_url.to_string(),
        });
    }

    /// The calls for the page's `<video>` elements since the last call.
    pub fn take_video_ops(&mut self) -> Vec<VideoOp> {
        std::mem::take(&mut self.video_ops)
    }

    /// The tokens of the video elements this runtime wants: those that load and those a MovieTexture plays.
    pub fn video_tokens(&self) -> Vec<u64> {
        let mut tokens: Vec<u64> = self.pending_videos.keys().copied().collect();

        tokens.extend(
            self.sound
                .values()
                .filter_map(|s| s.movie.as_ref()?.video_token()),
        );
        tokens
    }

    /// The page's video element `token` has its first frame (`loadeddata`): `rgba` are the pixels of that frame
    /// (`width` x `height`, rows from the top). MovieTexture.setVideo: the media element, the texture, the outputs
    /// (colorDepth 3), COMPLETE_STATE and set_speed__. Returns whether the token is this runtime's.
    pub fn video_loaded(
        &mut self,
        token: u64,
        width: u32,
        height: u32,
        duration: f64,
        has_sound: bool,
        rgba: Vec<u8>,
    ) -> bool {
        let Some(p) = self.pending_videos.remove(&token) else {
            return false;
        };

        // A newer load, or a node that is gone, took over.
        if !self.scene.is_live(p.node) || self.url_strings(p.node) != p.urls {
            return true;
        }

        if rgba.len() as u64 != u64::from(width) * u64::from(height) * 4 {
            return true;
        }

        let media = MovieMedia {
            width,
            height,
            frames: vec![rgba],
            delays: vec![],
            kind: Kind::Video {
                token,
                duration,
                current_time: 0.0,
                looping: false,
                dirty: false,
            },
            playback_rate: 1.0,
            shown: 0,
            audio: crate::webm::AudioTrack::None,
            seekable: false,
            offset: 0.0,
            iso: IsoState::default(),
        };

        self.movie_set_media_element(p.node, Some(media));
        self.set_texture_2d_size(p.node, width, height);
        self.movie_update_outputs(p.node, width as i32, height as i32, 3, has_sound, duration);
        self.set_load_state(p.node, nodes::LoadState::Complete);
        self.movie_set_speed(p.node);
        self.touch(p.node);
        true
    }

    /// The page's video element `token` failed (`error` or `abort`, `event`): MovieTexture.loadData's catch (the
    /// warning and the error), then the next candidate, or FAILED_STATE when none is left. Returns whether the token
    /// is this runtime's.
    pub fn video_failed(&mut self, token: u64, event: &str) -> bool {
        let Some(p) = self.pending_videos.remove(&token) else {
            return false;
        };

        if !self.scene.is_live(p.node) || self.url_strings(p.node) != p.urls {
            return true;
        }

        let decoded = crate::loader::decode_uri(&p.url).unwrap_or_else(|| p.url.clone());
        let error = format!("Couldn't load video '{}': {event}.", p.url);

        self.log_warn(format!("Error loading movie '{decoded}': {error}"));
        self.log_error(format!("Error: {error}"));

        let next = p.index + 1;

        if next >= p.urls.len() {
            self.movie_clear(p.node);
            self.set_load_state(p.node, nodes::LoadState::Failed);
            return true;
        }

        // The chain goes on with the candidate after this one: a task that waits for its response, as after a
        // candidate that did not decode.
        let key = loader::FetchKey::Url(p.node, 0);

        self.fetch_resume.insert(
            key.clone(),
            loader::FetchResume {
                index: next,
                attempts: next,
                urls: p.urls,
            },
        );
        self.parked_fetches.extend(key.task());
        self.resume_fetches();
        true
    }

    /// A new frame of the page's video element `token`: `currentTime`, and the pixels when the picture changed
    /// (`rgba` is empty when it did not). The texture holds them from the next frame on (set_time). Returns whether
    /// the token is this runtime's.
    pub fn video_frame(&mut self, token: u64, current_time: f64, rgba: Vec<u8>) -> bool {
        // ISO timing: the pictures come only from the engine's seeks ([`Runtime::video_seeked`]).
        if self.movie_timing == MovieTiming::Iso {
            return self.video_tokens().contains(&token);
        }

        for s in self.sound.values_mut() {
            let Some(m) = s.movie.as_mut() else {
                continue;
            };
            let Kind::Video {
                token: t,
                current_time: c,
                dirty,
                ..
            } = &mut m.kind
            else {
                continue;
            };

            if *t != token {
                continue;
            }

            *c = current_time;

            if rgba.len() == m.frames[0].len() {
                m.frames[0] = rgba;
                *dirty = true;
            }

            return true;
        }

        false
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn media(name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../oracle/synthetic-movie-anim/media")
                .join(name),
        )
        .unwrap()
    }

    /// The color of texel (x, y) (row 0 at the top).
    fn texel(m: &MovieMedia, frame: usize, x: u32, y: u32) -> [u8; 4] {
        let i = ((y * m.width + x) * 4) as usize;
        m.frames[frame][i..i + 4].try_into().unwrap()
    }

    const RED: [u8; 4] = [255, 0, 0, 255];
    const GREEN: [u8; 4] = [0, 255, 0, 255];
    const BLUE: [u8; 4] = [0, 0, 255, 255];
    const YELLOW: [u8; 4] = [255, 255, 0, 255];
    const WHITE: [u8; 4] = [255, 255, 255, 255];

    #[test]
    fn gif_frames_follow_elapsed_time_in_equal_steps() {
        let mut m = MovieMedia::decode_gif(&media("anim4.gif")).unwrap();

        assert_eq!((m.width, m.height, m.frames.len()), (8, 8, 4));
        assert_eq!(m.duration(), 1.0);
        assert_eq!(texel(&m, 0, 0, 0), WHITE);
        assert_eq!(texel(&m, 2, 7, 7), BLUE);

        m.set_loop(true, 0.0);
        // 0.47 s: frame 1 in equal steps; the delays (0.1, 0.2, 0.3, 0.4 s) would give frame 2.
        let frames: Vec<_> = [0.0, 0.125, 0.47, 0.625, 0.875, 1.125]
            .iter()
            .map(|&e| m.current_frame(e).unwrap())
            .collect();
        assert_eq!(frames, [0, 0, 1, 2, 3, 0]);

        // Not looping: from the next cycle on the time is the duration, the last frame.
        m.set_loop(false, 0.0);
        assert_eq!(m.current_time(1.2), 1.0);
        assert_eq!(m.current_frame(1.2), Some(3));

        // A negative rate gives a negative time: frame 0.
        m.set_playback_rate(-1.0);
        assert_eq!(m.current_frame(0.6), Some(0));
    }

    #[test]
    fn gif_disposal_restores_the_previous_picture_and_keeps_transparent_texels() {
        let m = MovieMedia::decode_gif(&media("dispose.gif")).unwrap();

        assert_eq!(texel(&m, 1, 0, 3), RED);
        // Disposal 3 of frame 1: frame 2 is drawn over frame 0, not over the red block.
        assert_eq!(texel(&m, 2, 0, 0), WHITE);
        assert_eq!(texel(&m, 2, 3, 3), BLUE);
        assert_eq!(texel(&m, 2, 5, 5), GREEN);
        // The transparent index of frame 3 shows the blue below.
        assert_eq!(texel(&m, 3, 4, 0), YELLOW);
        assert_eq!(texel(&m, 3, 5, 0), BLUE);
    }

    #[test]
    fn apng_frames_composite_dispose_previous_and_blend_over() {
        let m = MovieMedia::decode_apng(&media("once.png")).unwrap();

        assert_eq!(m.frames.len(), 4);
        // 10 ms frames last 100 ms in apng-js.
        assert_eq!(m.duration(), 0.4);
        assert_eq!(texel(&m, 1, 0, 3), RED);
        assert_eq!(texel(&m, 2, 3, 3), BLUE);
        assert_eq!(texel(&m, 2, 5, 5), GREEN);
        assert_eq!(texel(&m, 3, 5, 1), YELLOW);
        assert_eq!(texel(&m, 3, 0, 0), WHITE);

        // Not animated: apng-js returns its error.
        let mut png = vec![];
        image::RgbaImage::new(2, 2)
            .write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png)
            .unwrap();
        assert_eq!(
            MovieMedia::decode_apng(&png).err(),
            Some(DecodeError::NotAnimated)
        );
    }

    #[test]
    fn apng_player_plays_once_and_ends_on_the_last_frame() {
        let mut m = MovieMedia::decode_apng(&media("once.png")).unwrap();

        m.set_loop(false, 0.0);
        m.play(0.0, 1000.0);
        assert_eq!(m.current_frame(0.0), Some(0));

        m.tick(1050.0);
        assert_eq!(m.current_frame(0.0), Some(0));

        m.tick(1150.0);
        assert_eq!(m.current_frame(0.0), Some(1));
        m.tick(1250.0);
        assert_eq!(m.current_frame(0.0), Some(2));
        assert!(m.current_time(0.0) < m.duration());

        // The last frame ends the play.
        m.tick(1350.0);
        assert_eq!(m.current_frame(0.0), Some(3));
        assert_eq!(m.current_time(0.0), m.duration());
        m.tick(5000.0);
        assert_eq!(m.current_frame(0.0), Some(3));

        // play () after the end starts over from frame 0.
        m.play(0.0, 3000.0);
        assert_eq!(m.current_frame(0.0), Some(0));
    }

    #[test]
    fn apng_player_holds_at_rate_zero_and_loops() {
        let mut m = MovieMedia::decode_apng(&media("once.png")).unwrap();

        m.set_loop(true, 0.0);
        m.set_playback_rate(0.0);
        m.play(0.0, 0.0);
        m.tick(1e6);
        assert_eq!(m.current_frame(0.0), Some(0));

        m.set_playback_rate(-2.0);
        assert_eq!(m.playback_rate, 0.0);

        m.set_playback_rate(1.0);
        m.play(0.0, 0.0);
        // One late tick renders every frame that is due: round once to frame 0 at 0.45 s.
        m.tick(450.0);
        assert_eq!(m.current_frame(0.0), Some(0));
        m.tick(520.0);
        assert_eq!(m.current_frame(0.0), Some(1));
        assert!(m.looping());

        // A tick a whole play (0.4 s) or more late skips the plays it missed, as apng-js does.
        m.tick(1520.0);
        assert_eq!(m.current_frame(0.0), Some(3));
    }

    #[test]
    fn apng_player_ticks_end_at_extreme_rates() {
        // The runtime clock is milliseconds since 1970: at a rate of 1e8 or more, a play (0.4 s) is less than
        // the precision of `now`, and a tick that adds one play or one delay at a time does not end.
        let now = 946_684_800_125.0;

        for rate in [1e8, 1e9, f64::MAX, f64::INFINITY, f64::NAN] {
            let mut m = MovieMedia::decode_apng(&media("once.png")).unwrap();

            m.set_loop(true, 0.0);
            m.set_playback_rate(rate);
            m.play(0.0, now);
            // 5 ticks pass the last frame after a skip of more plays than u32::MAX.
            for t in [1.0, 2.0, 3.0, 4.0, 1000.0] {
                m.tick(now + t);
            }
            assert!(m.current_frame(0.0).is_some_and(|f| f < 4), "rate {rate}");
            assert!(m.looping());
        }

        // A high rate that the clock can still resolve keeps the frame order: each tick is 1 frame later.
        let mut m = MovieMedia::decode_apng(&media("once.png")).unwrap();

        m.set_loop(true, 0.0);
        m.set_playback_rate(1e3);
        m.play(0.0, 0.0);
        for (now, frame) in [(0.15, 1), (0.25, 2), (0.35, 3), (0.45, 0), (1.05, 2)] {
            m.tick(now);
            assert_eq!(m.current_frame(0.0), Some(frame), "now {now}");
        }
    }

    fn png_chunk(kind: &[u8], body: &[u8]) -> Vec<u8> {
        let crc = !kind.iter().chain(body).fold(!0u32, |c, &b| {
            (0..8).fold(c ^ u32::from(b), |c, _| {
                if c & 1 != 0 {
                    0xedb8_8320 ^ (c >> 1)
                } else {
                    c >> 1
                }
            })
        });
        [
            &(body.len() as u32).to_be_bytes()[..],
            kind,
            body,
            &crc.to_be_bytes(),
        ]
        .concat()
    }

    /// A GIF of `width` x `height` with `images` images of 1 x 1 pixel.
    fn gif_canvas(width: u16, height: u16, images: usize) -> Vec<u8> {
        let mut d = b"GIF89a".to_vec();
        d.extend(width.to_le_bytes());
        d.extend(height.to_le_bytes());
        d.extend([0xf0, 0, 0, 0, 0, 0, 255, 255, 255]);

        for _ in 0..images {
            d.extend([0x21, 0xf9, 4, 4, 10, 0, 0, 0]);
            d.extend([0x2c, 0, 0, 0, 0, 1, 0, 1, 0, 0, 2, 2, 0x44, 0x01, 0]);
        }

        d.push(0x3b);
        d
    }

    #[test]
    fn oversized_media_are_rejected_before_the_frames_are_allocated() {
        let too_large =
            |r: Result<MovieMedia, DecodeError>| matches!(r, Err(DecodeError::TooLarge(_)));

        // 43 bytes, a 65535 x 65535 canvas (16 GiB a frame): this stopped the process (SIGABRT) before the limit.
        let gif = gif_canvas(65535, 65535, 1);
        assert_eq!(gif.len(), 43);
        assert!(too_large(MovieMedia::decode_gif(&gif)));

        // 9 frames of 4096 x 4096 (64 MiB each) are more than 512 MiB together; 8 are not.
        assert!(too_large(MovieMedia::decode_gif(&gif_canvas(
            4096, 4096, 9
        ))));
        assert_eq!(gif_image_count(&gif_canvas(4096, 4096, 8)), Some(8));
        assert!(frame_bytes(4096, 4096, 8).is_ok());

        // 127 bytes, an APNG with a 65535 x 65535 canvas.
        let mut apng = b"\x89PNG\r\n\x1a\n".to_vec();
        apng.extend(png_chunk(
            b"IHDR",
            &[0, 0, 255, 255, 0, 0, 255, 255, 8, 6, 0, 0, 0],
        ));
        apng.extend(png_chunk(b"acTL", &[0, 0, 0, 1, 0, 0, 0, 0]));
        apng.extend(png_chunk(
            b"fcTL",
            &[
                0, 0, 0, 0, 0, 0, 255, 255, 0, 0, 255, 255, 0, 0, 0, 0, 0, 0, 0, 0, 0, 1, 0, 1, 0,
                0,
            ],
        ));
        // zlib data of 64 zero bytes, as in the probe file.
        apng.extend(png_chunk(
            b"IDAT",
            &[120, 156, 99, 96, 160, 12, 0, 0, 0, 64, 0, 1],
        ));
        apng.extend(png_chunk(b"IEND", &[]));
        assert_eq!(apng.len(), 127);
        assert!(too_large(MovieMedia::decode_apng(&apng)));

        // The products are checked: no overflow.
        assert!(frame_bytes(u32::MAX, u32::MAX, usize::MAX).is_err());
    }

    /// The media of oracle/synthetic-movie-load-errors (oracle/movie-load-errors-baseline).
    fn load_error_media(name: &str) -> Vec<u8> {
        std::fs::read(
            std::path::Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../oracle/synthetic-movie-load-errors/media")
                .join(name),
        )
        .unwrap()
    }

    #[test]
    fn png_load_errors_follow_apng_js() {
        for name in ["static.png", "notpng.png", "noframes.png"] {
            assert_eq!(
                MovieMedia::decode_apng(&load_error_media(name)).err(),
                Some(DecodeError::NotAnimated),
                "{name}"
            );
        }

        assert_eq!(
            MovieMedia::decode_apng(&load_error_media("trunc-head.png")).err(),
            Some(DecodeError::Fail(
                "RangeError",
                "Offset is outside the bounds of the DataView".into()
            ))
        );

        // The structure is valid: the size and the delays are apng-js's, frame 0 is decoded.
        let m = MovieMedia::decode_apng(&load_error_media("trunc-anim.png")).unwrap();
        assert_eq!(
            (m.width, m.height, m.frames.len(), m.duration()),
            (8, 8, 2, 2.0)
        );
        assert_eq!(texel(&m, 0, 3, 3), RED);

        // Frame data the decoder cannot read leaves the canvas empty.
        let m = MovieMedia::decode_apng(&load_error_media("bad-data.png")).unwrap();
        assert_eq!(
            (m.width, m.height, m.frames.len(), m.duration()),
            (8, 8, 1, 1.0)
        );
        assert!(m.frames[0].iter().all(|&b| b == 0));

        let m = MovieMedia::decode_apng(&load_error_media("good.png")).unwrap();
        assert_eq!(texel(&m, 1, 3, 3), GREEN);
    }
}
