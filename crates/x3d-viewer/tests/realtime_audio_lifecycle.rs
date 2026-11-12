//! Realtime audio lifecycle on the host's output device (Phase 7A, issue #88): an OscillatorSource plays, pauses
//! (pauseTime), resumes (resumeTime), and the viewer is dropped (AudioEngine's Drop closes the context; see below). The viewer core is stepped
//! on the wall clock, as the window does. Ignored by default: it needs an output device and plays a tone for 8 s.
//! It checks the order and the wall-clock spacing of the X3D events and that the context clock runs until the drop. It cannot
//! hear the device: record the default sink's monitor during the run (for example `parec -d <sink>.monitor`) to see that
//! the tone is on for 2 s, off for 2 s, on again, and off at the drop.

use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

fn now() -> f64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_secs_f64()
}

#[test]
#[ignore = "needs an audio output device and plays a tone"]
fn oscillator_plays_pauses_resumes_and_closes_on_the_device() {
    x3d_runtime::sound::set_default_mode(x3d_runtime::sound::AudioMode::Realtime);

    let world = concat!(
        env!("CARGO_MANIFEST_DIR"),
        "/tests/data/realtime-audio/lifecycle.wrl"
    );
    let t = now();
    let mut v = Viewer3d::load_attributes(
        world.as_ref(),
        None,
        t,
        64,
        64,
        None,
        false,
        ORACLE_PAGE_ATTRIBUTES,
    )
    .unwrap();
    v.rt.record_console = true;
    let realtime = v.rt.audio.as_ref().is_some_and(|e| e.is_realtime());
    eprintln!("{t:.3} loaded; realtime engine {realtime}");
    assert!(realtime, "no realtime output device");

    let (start, mut seen, mut log) = (Instant::now(), 0, Vec::<(f64, String)>::new());
    while start.elapsed() < Duration::from_millis(8000) {
        let t = now();
        v.date_now = t;
        v.step(t);
        for line in &v.rt.console[seen..] {
            eprintln!("{t:.3} console: {line}");
            // "log: play 1794475025.0145" -> (1794475025.0145, "play"): the X3D event time (cycleTime or the event's
            // timestamp) and the label.
            let words: Vec<&str> = line
                .trim_start_matches("log: ")
                .split_whitespace()
                .collect();
            let (label, time) = words.split_at(words.len() - 1);
            let time: f64 = time[0]
                .parse()
                .unwrap_or_else(|_| panic!("no event time in {line:?}"));

            log.push((time, label.join(" ")));
        }
        seen = v.rt.console.len();
        std::thread::sleep(Duration::from_millis(20));
    }

    // Until the drop the engine is the realtime one and its clock runs. The drop closes the context (AudioEngine's Drop:
    // close_sync); the test cannot see the device after that, so the release is shown outside it: the sink monitor goes
    // silent and the PipeWire stream node goes away.
    assert!(v.rt.audio.as_ref().is_some_and(|e| e.is_realtime()));
    let clock = v.rt.audio.as_ref().map(|e| e.current_time()).unwrap_or(0.0);
    eprintln!("{:.3} context clock {clock:.3} s; drop (shutdown)", now());
    drop(v);
    eprintln!("{:.3} dropped", now());

    // The order of the events and the X3D time of each step (one TimeSensor cycle is 0.5 s; the steps are 4 cycles apart).
    let at = |want: &str| -> f64 {
        log.iter()
            .find(|(_, l)| l == want)
            .unwrap_or_else(|| panic!("no {want:?} in {log:?}"))
            .0
    };
    let order: Vec<&str> = log.iter().map(|(_, l)| l.as_str()).collect();

    assert_eq!(
        order,
        [
            "play",
            "O isActive true",
            "pause",
            "O isPaused true",
            "resume",
            "O isPaused false"
        ],
        "{log:?}"
    );

    let (play, pause, resume) = (at("play"), at("pause"), at("resume"));

    assert!(
        (1.8..=2.2).contains(&(pause - play)),
        "play to pause {:.3} s",
        pause - play
    );
    assert!(
        (1.8..=2.2).contains(&(resume - pause)),
        "pause to resume {:.3} s",
        resume - pause
    );
    assert!(clock > 5.0, "the context clock ran only {clock:.3} s");
}
