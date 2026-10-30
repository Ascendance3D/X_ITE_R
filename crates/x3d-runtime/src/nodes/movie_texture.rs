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

use crate::*;

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
}

enum Kind {
    /// GifMedia: `cycle` and `loop` of its closure.
    Gif { cycle: f64, looping: bool },
    /// PNGMedia: `apng .numPlays` and the apng-js Player.
    Apng { num_plays: u32, player: Player },
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

/// libgif `load_raw` on_error type for a GIF it cannot parse.
pub const GIF_PARSE_ERROR: &str = "parse";

impl MovieMedia {
    /// libgif `load_raw`: the frames and their delays (centiseconds × 10).
    pub fn decode_gif(data: &[u8]) -> Option<MovieMedia> {
        use image::{AnimationDecoder, ImageDecoder};

        let decoder = image::codecs::gif::GifDecoder::new(std::io::Cursor::new(data)).ok()?;
        let (width, height) = decoder.dimensions();
        let frames = decoder.into_frames().collect_frames().ok()?;

        if frames.is_empty() {
            return None;
        }

        let delays = frames.iter().map(|f| ratio_ms(f.delay())).collect();

        Some(MovieMedia {
            width,
            height,
            frames: frames
                .into_iter()
                .map(|f| f.into_buffer().into_raw())
                .collect(),
            delays,
            kind: Kind::Gif {
                cycle: 0.0,
                looping: false,
            },
            playback_rate: 1.0,
            shown: 0,
        })
    }

    /// apng-js parseAPNG and PNGMedia (whose `player .stop ()` renders frame 0). `None` for data apng-js does not
    /// take as an animated PNG (parseAPNG returns, not throws, its error).
    pub fn decode_apng(data: &[u8]) -> Option<MovieMedia> {
        use image::{AnimationDecoder, ImageDecoder};

        let decoder = image::codecs::png::PngDecoder::new(std::io::Cursor::new(data)).ok()?;

        if !decoder.is_apng().ok()? {
            return None;
        }

        let (width, height) = decoder.dimensions();
        let frames = decoder.apng().ok()?.into_frames().collect_frames().ok()?;

        if frames.is_empty() {
            return None;
        }

        // `frame .delay = 1000 * delayN / delayD` (delayD 0: 100), and at least 100 ms if 10 ms or less.
        let delays = frames
            .iter()
            .map(|f| match ratio_ms(f.delay()) {
                d if d <= 10.0 => 100.0,
                d => d,
            })
            .collect();

        let mut media = MovieMedia {
            width,
            height,
            frames: frames
                .into_iter()
                .map(|f| f.into_buffer().into_raw())
                .collect(),
            delays,
            kind: Kind::Apng {
                num_plays: 0,
                player: Player::default(),
            },
            playback_rate: 1.0,
            shown: 0,
        };

        media.player_stop();
        Some(media)
    }

    /// `duration` in seconds: libgif get_duration_ms, apng .playTime.
    pub fn duration(&self) -> f64 {
        self.play_time() / 1000.0
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
        }
    }

    /// `currentTime = 0`: GifMedia ignores it, PNGMedia stops the player.
    pub fn set_current_time_zero(&mut self) {
        if matches!(self.kind, Kind::Apng { .. }) {
            self.player_stop();
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
        }
    }

    /// `loop`.
    pub fn looping(&self) -> bool {
        match &self.kind {
            Kind::Gif { looping, .. } => *looping,
            Kind::Apng { num_plays, .. } => *num_plays == 0,
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
        }
    }

    /// `playbackRate = value` (PNGMedia: no negative values).
    pub fn set_playback_rate(&mut self, value: f64) {
        self.playback_rate = match self.kind {
            Kind::Gif { .. } => value,
            Kind::Apng { .. } => value.max(0.0),
        };
    }

    /// `play ()` at `now` (milliseconds): GifMedia keeps the cycle, PNGMedia plays the player.
    pub fn play(&mut self, elapsed: f64, now: f64) {
        let cycle_now = self.cycle(elapsed);

        match &mut self.kind {
            Kind::Gif { cycle, .. } => *cycle = cycle_now,
            Kind::Apng { .. } => self.player_play(now),
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
            while now - player.next_render_time >= play_time / rate {
                player.next_render_time += play_time / rate;
                player.num_plays += 1;
            }

            loop {
                self.render_next_frame();

                let Kind::Apng { player, .. } = &mut self.kind else {
                    unreachable!()
                };
                player.next_render_time += self.delays[player.current_frame_number] / rate;

                if player.ended || player.paused || now <= player.next_render_time {
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
            player.num_plays += 1;

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
    }

    /// MovieTexture.initialize between X3DTexture2DNode's and X3DUrlObject's: X3DSoundSourceNode.initialize
    /// (X3DTimeDependentNode.initialize; set_gain__ only mutes the element) and the speed and pitch interests.
    pub(crate) fn movie_initialize(&mut self, node: NodeId) {
        self.time_dependent_initialize(node, Cb::TimeSetEnabled);
        self.sound_interest(node, "speed", super::sound::SoundCb::Speed);
        self.sound_interest(node, "pitch", super::sound::SoundCb::Speed);
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
    pub(crate) fn movie_set_frames(&mut self, node: NodeId, media: MovieMedia) {
        let (width, height, duration) = (media.width, media.height, media.duration());

        self.movie_set_media_element(node, Some(media));
        self.set_texture_2d_size(node, width, height);
        self.movie_update_outputs(node, width as i32, height as i32, 4, duration);
        self.set_load_state(node, nodes::LoadState::Complete);
        self.movie_set_speed(node);
    }

    /// MovieTexture.unloadData and the end of a loadData that found nothing: clearTexture, setMediaElement (null),
    /// updateOutputs (0, 0, 0, false, -1).
    pub(crate) fn movie_clear(&mut self, node: NodeId) {
        self.set_texture_2d_size(node, 1, 1);
        self.movie_set_media_element(node, None);
        self.movie_update_outputs(node, 0, 0, 0, -1.0);
    }

    /// MovieTexture.updateOutputs (hasSound is FALSE for a GIF or an APNG).
    fn movie_update_outputs(
        &mut self,
        node: NodeId,
        width: i32,
        height: i32,
        color_depth: i32,
        duration: f64,
    ) {
        for (name, value) in [
            ("width", FieldValue::SFInt32(width)),
            ("height", FieldValue::SFInt32(height)),
            ("colorDepth", FieldValue::SFInt32(color_depth)),
            ("hasSound", FieldValue::SFBool(false)),
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
        if let Some(m) = self.movie(node) {
            m.pause();
        }

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

        if let Some(m) = self.movie(node) {
            m.set_loop(value, elapsed);
        }
    }

    /// MovieTexture.set_speed__: playbackRate = speed × max (pitch, 0).
    pub(crate) fn movie_set_speed(&mut self, node: NodeId) {
        let rate = self.get_f64(node, "speed") * self.get_f64(node, "pitch").max(0.0);

        if let Some(m) = self.movie(node) {
            m.set_playback_rate(rate);
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

        let (elapsed, now) = (self.elapsed_value(node), self.now * 1000.0);

        if let Some(m) = self.movie(node) {
            m.set_current_time_zero();
            m.play(elapsed, now);
        }
    }

    /// set_pause, set_stop: stopAudioElement.
    pub(crate) fn movie_pause(&mut self, node: NodeId) {
        if let Some(m) = self.movie(node) {
            m.pause();
        }
    }

    /// set_resume: startAudioElement.
    pub(crate) fn movie_resume(&mut self, node: NodeId) {
        let (elapsed, now) = (self.elapsed_value(node), self.now * 1000.0);

        if let Some(m) = self.movie(node) {
            m.play(elapsed, now);
        }
    }

    /// MovieTexture.set_time: X3DSoundSourceNode.set_time, then `updateTextureData (media .currentFrame)`. The
    /// apng-js tick of the frame runs first.
    pub(crate) fn movie_set_time(&mut self, node: NodeId) {
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
            if frame != m.shown {
                m.shown = frame;
                self.touch(node);
            }

            self.add_node_event(node);
        }
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
        assert!(MovieMedia::decode_apng(&png).is_none());
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
}
