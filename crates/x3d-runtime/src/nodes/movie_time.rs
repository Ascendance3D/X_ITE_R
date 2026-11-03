//! ISO/IEC 19775-1 clause 18.4.2 MovieTexture time, as pure functions (docs/movie-texture-timing.md).
//!
//! Nothing here reads a clock: every function takes the elapsed time of the active node (`now - startTime`
//! without the paused intervals, X3D 8.2.4) and returns the movie time or the frame, so a fake clock tests it.
//!
//! [`MovieTiming::Pinned`] is the behavior of the pinned X_ITE 16.5.0 and is not implemented here (it is the
//! engine's earlier code path, kept unchanged); this module is the [`MovieTiming::Iso`] mapping.

/// Which time-to-frame mapping a MovieTexture uses.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum MovieTiming {
    /// The ISO 19775-1 18.4.2 formulas, driven by the scene clock. The default.
    #[default]
    Iso,
    /// Exactly the pinned X_ITE 16.5.0 behavior: GifMedia/PNGMedia/`<video>` timing, a negative `speed` plays at 1.
    Pinned,
}

static PROCESS_DEFAULT: std::sync::atomic::AtomicU8 = std::sync::atomic::AtomicU8::new(0);

impl MovieTiming {
    /// The timing a [`crate::Runtime`] starts with: [`MovieTiming::Iso`] unless [`MovieTiming::set_process_default`]
    /// chose another (the viewer's `X3D_MOVIE_TIMING` and the web page's `movieTiming` do).
    pub fn process_default() -> MovieTiming {
        match PROCESS_DEFAULT.load(std::sync::atomic::Ordering::Relaxed) {
            1 => MovieTiming::Pinned,
            _ => MovieTiming::Iso,
        }
    }

    /// Chooses the timing of the runtimes made from now on.
    pub fn set_process_default(timing: MovieTiming) {
        PROCESS_DEFAULT.store(
            u8::from(timing == MovieTiming::Pinned),
            std::sync::atomic::Ordering::Relaxed,
        );
    }

    /// `"iso"` or `"pinned"` (any case); `None` for another word.
    pub fn parse(name: &str) -> Option<MovieTiming> {
        match name.to_ascii_lowercase().as_str() {
            "iso" => Some(MovieTiming::Iso),
            "pinned" => Some(MovieTiming::Pinned),
            _ => None,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            MovieTiming::Iso => "iso",
            MovieTiming::Pinned => "pinned",
        }
    }
}

/// The movie time of an inactive node that has not played: 0 for a non-negative `rate`, the duration for a negative
/// one ("frame 0 ... or the last frame ... if speed is negative").
pub fn inactive_time(rate: f64, duration: f64) -> f64 {
    if rate < 0.0 { duration } else { 0.0 }
}

/// The inputs of the active-node mapping. `rate` is `speed` times `max (pitch, 0)`, frozen at the start
/// (`set_speed` is ignored while the node is active).
#[derive(Clone, Copy, Debug)]
pub struct IsoClock {
    /// `now - startTime` in seconds, the paused time not counted.
    pub elapsed: f64,
    pub rate: f64,
    /// The movie duration in seconds at rate 1.
    pub duration: f64,
    pub looping: bool,
}

/// The movie time of an active node and whether the play is over (a node that does not loop ends after one cycle,
/// `duration / |rate|` seconds; the time is then the end of the movie in the direction of play).
///
/// 18.4.2: for a positive `speed`, `t = (now - startTime) modulo (duration / speed)`; for a negative one,
/// `t = duration - ((now - startTime) modulo |duration / speed|)`. The `t` of the clause is a time within one cycle;
/// the movie time it selects is `t` scaled by `|speed|` (identical at speed 1, and the movie then plays `speed` times
/// as fast). `rate == 0` shows frame 0 and never ends.
pub fn iso_time(c: IsoClock) -> (f64, bool) {
    let IsoClock {
        elapsed,
        rate,
        duration,
        looping,
    } = c;

    if rate == 0.0 || rate.is_nan() || !(duration > 0.0) || !duration.is_finite() {
        return (0.0, false);
    }

    let elapsed = elapsed.max(0.0);
    let cycle = duration / rate.abs();
    let end = if rate < 0.0 { 0.0 } else { duration };

    if !looping && elapsed >= cycle {
        return (end, true);
    }

    let phase = elapsed.rem_euclid(cycle) * rate.abs();
    // rem_euclid can return `cycle` itself for a tiny negative remainder; keep the time inside the movie.
    let phase = phase.min(duration);

    (if rate < 0.0 { duration - phase } else { phase }, false)
}

/// The index of the frame shown at movie time `t`: the last frame that starts at or before `t` (`starts` are the
/// frame start times in seconds, ascending, the first 0). A `t` at or past the end gives the last frame.
pub fn frame_at(starts: &[f64], t: f64) -> usize {
    if starts.is_empty() || t.is_nan() {
        return 0;
    }

    starts.partition_point(|s| *s <= t).saturating_sub(1)
}

/// The start times in seconds of frames with these delays in milliseconds.
pub fn frame_starts(delays_ms: &[f64]) -> Vec<f64> {
    let mut at = 0.0;

    delays_ms
        .iter()
        .map(|d| {
            let start = at;
            at += d / 1000.0;
            start
        })
        .collect()
}

/// The seek requests of the page's `<video>` element (ISO mode). The engine decides the media time and asks the page to
/// seek the paused element; a seek finishes later, so every request has an id that only grows, and a result that is not
/// of the newest request is obsolete and dropped: an old frame can never overwrite a newer one.
#[derive(Clone, Debug, Default)]
pub struct SeekLedger {
    newest: u64,
    in_flight: bool,
    /// The newest time asked for, so an identical request is not repeated.
    asked: Option<f64>,
}

impl SeekLedger {
    /// A request for media time `t`. `None`: the same time is already asked for, or a seek is in flight and
    /// `force` is not set (the next tick asks again). `force` supersedes the one in flight (a start, stop or jump).
    pub fn request(&mut self, t: f64, force: bool) -> Option<u64> {
        if self.asked == Some(t) && !force {
            return None;
        }

        if self.in_flight && !force {
            return None;
        }

        self.newest += 1;
        self.in_flight = true;
        self.asked = Some(t);
        Some(self.newest)
    }

    /// A result of request `id`: `true` when it is the newest request's and the picture is to be used.
    pub fn accept(&mut self, id: u64) -> bool {
        if id != self.newest {
            return false;
        }

        self.in_flight = false;
        true
    }

    /// The newest request produced no usable picture (the page could not read it): forget the time asked for, so the
    /// next tick asks for it again instead of leaving the texture stale.
    pub fn retry(&mut self) {
        self.asked = None;
    }

    /// The id of the newest request (0: none yet).
    pub fn newest(&self) -> u64 {
        self.newest
    }
}
