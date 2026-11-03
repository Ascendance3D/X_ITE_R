//! The pure ISO/IEC 19775-1 18.4.2 MovieTexture time functions (`x3d_runtime::nodes::movie_time`), against no clock at all,
//! and the seek-id ledger that drops obsolete `<video>` seek results. (An integration test: the crate's lib test target
//! does not build on this base, see docs/movie-texture-timing.md.)

use x3d_runtime::nodes::movie_time::*;

fn clock(elapsed: f64, rate: f64, looping: bool) -> IsoClock {
    IsoClock {
        elapsed,
        rate,
        duration: 4.0,
        looping,
    }
}

#[test]
fn positive_speed_wraps_when_looping_and_ends_without_loop() {
    assert_eq!(iso_time(clock(1.0, 1.0, true)), (1.0, false));
    assert_eq!(iso_time(clock(5.0, 1.0, true)), (1.0, false));
    assert_eq!(iso_time(clock(3.0, 2.0, true)), (2.0, false));
    assert_eq!(iso_time(clock(3.9, 1.0, false)), (3.9, false));
    assert_eq!(iso_time(clock(4.0, 1.0, false)), (4.0, true));
    assert_eq!(iso_time(clock(2.0, 2.0, false)), (4.0, true));
}

#[test]
fn negative_speed_runs_the_movie_backwards() {
    assert_eq!(iso_time(clock(0.0, -1.0, true)), (4.0, false));
    assert_eq!(iso_time(clock(1.0, -1.0, true)), (3.0, false));
    assert_eq!(iso_time(clock(5.0, -1.0, true)), (3.0, false));
    assert_eq!(iso_time(clock(1.0, -2.0, true)), (2.0, false));
    assert_eq!(iso_time(clock(3.0, -1.0, false)), (1.0, false));
    assert_eq!(iso_time(clock(4.0, -1.0, false)), (0.0, true));
}

#[test]
fn zero_speed_shows_frame_zero_and_never_ends() {
    assert_eq!(iso_time(clock(0.0, 0.0, false)), (0.0, false));
    assert_eq!(iso_time(clock(100.0, 0.0, false)), (0.0, false));
    assert_eq!(iso_time(clock(100.0, 0.0, true)), (0.0, false));
}

#[test]
fn an_inactive_node_shows_frame_zero_or_the_last_frame() {
    assert_eq!(inactive_time(1.0, 4.0), 0.0);
    assert_eq!(inactive_time(0.0, 4.0), 0.0);
    assert_eq!(inactive_time(-0.5, 4.0), 4.0);

    let starts = frame_starts(&[1000.0; 4]);

    assert_eq!(frame_at(&starts, inactive_time(1.0, 4.0)), 0);
    assert_eq!(frame_at(&starts, inactive_time(-1.0, 4.0)), 3);
}

#[test]
fn the_frame_follows_the_frame_times_not_equal_steps() {
    let starts = frame_starts(&[500.0, 1500.0, 2000.0]);

    assert_eq!(starts, vec![0.0, 0.5, 2.0]);
    assert_eq!(frame_at(&starts, 0.0), 0);
    assert_eq!(frame_at(&starts, 0.49), 0);
    assert_eq!(frame_at(&starts, 0.5), 1);
    assert_eq!(frame_at(&starts, 1.99), 1);
    assert_eq!(frame_at(&starts, 2.0), 2);
    assert_eq!(frame_at(&starts, 4.0), 2);
}

#[test]
fn a_degenerate_duration_or_rate_does_not_panic() {
    for d in [0.0, -1.0, f64::NAN, f64::INFINITY] {
        let c = IsoClock {
            elapsed: 1.0,
            rate: 1.0,
            duration: d,
            looping: true,
        };

        assert_eq!(iso_time(c), (0.0, false));
    }

    assert_eq!(iso_time(clock(1.0, f64::NAN, true)), (0.0, false));
    assert_eq!(frame_at(&[], 1.0), 0);
}

#[test]
fn a_seek_result_older_than_the_newest_request_is_dropped() {
    let mut l = SeekLedger::default();
    let a = l.request(1.0, false).unwrap();
    let b = l.request(2.0, true).unwrap();

    assert!(b > a);
    // Out of order: the newer completes first, then the obsolete one.
    assert!(l.accept(b));
    assert!(!l.accept(a));

    let c = l.request(3.0, false).unwrap();

    // The obsolete one arriving first is dropped too, and the newest still counts.
    assert!(!l.accept(a));
    assert!(!l.accept(b));
    assert!(l.accept(c));
}

#[test]
fn requests_wait_for_the_seek_in_flight_unless_forced() {
    let mut l = SeekLedger::default();
    let a = l.request(1.0, false).unwrap();

    assert_eq!(l.request(2.0, false), None);
    assert!(l.accept(a));
    // Done: the same time is not asked again, another time is.
    assert_eq!(l.request(1.0, false), None);
    assert!(l.request(2.0, false).is_some());
    assert!(l.request(2.0, true).is_some());
}
