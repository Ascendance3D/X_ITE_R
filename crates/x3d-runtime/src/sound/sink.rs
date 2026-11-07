//! The output device of an AudioDestination (`mediaDeviceID`).
//!
//! X_ITE's AudioDestination.set_mediaDeviceID_impl__ is
//!
//! ```js
//! if (this .audioElement .sinkId === sinkId) return Promise .resolve ();
//! return this .audioElement .setSinkId ?.(sinkId) ?? Promise .resolve ();
//! ```
//!
//! and X3DSoundContext.startAudioElements ends the promise with `.catch (Function .prototype)`. So an id
//! that is the current sink is left alone, `""` is the default device, a device that exists is selected,
//! and an id that no output device has makes `setSinkId` reject with a NotFoundError that nothing logs:
//! the output stays where it was (recorded with the pinned X_ITE in headless Chromium,
//! `oracle/web-audio-sink-baseline/`). [`select_sink`] does the same on a [`SinkBackend`], which the
//! native AudioContext implements and a test replaces with a list of devices.
//!
//! One native context serves every AudioDestination of the browser, where X_ITE gives each node its own
//! `<audio>` element, so the last `mediaDeviceID` that was set decides the device of the context.

use web_audio_api::context::AudioContext;
use web_audio_api::media_devices::{self, MediaDeviceInfoKind};

/// What [`select_sink`] did.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SinkOutcome {
    /// The id is the current sink: nothing to do (`audioElement .sinkId === sinkId`).
    Unchanged,
    /// The output moved to the device (or to the default device for `""`).
    Selected,
    /// No output device has this id (`setSinkId` rejects, silently): the output stays.
    Unknown,
    /// The device exists, but the backend could not open it: the output stays.
    Failed,
}

/// The part of an audio output that the selection needs.
pub trait SinkBackend {
    /// The id of the current sink (`""`: the default device).
    fn sink_id(&self) -> String;
    /// The ids of the output devices that can be opened.
    fn output_ids(&self) -> Vec<String>;
    /// Move the output to a device id (`""`: the default device).
    fn set_sink_id(&mut self, id: &str) -> Result<(), String>;
}

/// AudioDestination.set_mediaDeviceID_impl__ against `backend`.
pub fn select_sink(backend: &mut dyn SinkBackend, requested: &str) -> SinkOutcome {
    if backend.sink_id() == requested {
        return SinkOutcome::Unchanged;
    }

    if !requested.is_empty() && !backend.output_ids().iter().any(|d| d == requested) {
        return SinkOutcome::Unknown;
    }

    match backend.set_sink_id(requested) {
        Ok(()) => SinkOutcome::Selected,
        Err(_) => SinkOutcome::Failed,
    }
}

/// The output devices of the native host (cpal: ALSA on Linux, CoreAudio on macOS).
pub fn native_output_ids() -> Vec<String> {
    media_devices::enumerate_devices_sync()
        .into_iter()
        .filter(|d| d.kind() == MediaDeviceInfoKind::AudioOutput)
        .map(|d| d.device_id().to_string())
        .collect()
}

/// The realtime context of the engine as a [`SinkBackend`].
pub struct ContextSink<'a>(pub &'a AudioContext);

impl SinkBackend for ContextSink<'_> {
    fn sink_id(&self) -> String {
        self.0.sink_id()
    }

    fn output_ids(&self) -> Vec<String> {
        native_output_ids()
    }

    fn set_sink_id(&mut self, id: &str) -> Result<(), String> {
        // The context call blocks while the new stream starts, and it may panic in a backend.
        std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
            self.0.set_sink_id_sync(id.to_string())
        }))
        .map_err(|_| "the audio backend panicked".to_string())?
        .map_err(|e| e.to_string())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// A device list instead of a sound card: records the ids it was asked to open.
    struct Mock {
        current: String,
        devices: Vec<String>,
        opened: Vec<String>,
        refuse: bool,
    }

    impl Mock {
        fn new(devices: &[&str]) -> Mock {
            Mock {
                current: String::new(),
                devices: devices.iter().map(|s| s.to_string()).collect(),
                opened: vec![],
                refuse: false,
            }
        }
    }

    impl SinkBackend for Mock {
        fn sink_id(&self) -> String {
            self.current.clone()
        }
        fn output_ids(&self) -> Vec<String> {
            self.devices.clone()
        }
        fn set_sink_id(&mut self, id: &str) -> Result<(), String> {
            self.opened.push(id.to_string());
            if self.refuse {
                return Err("busy".into());
            }
            self.current = id.to_string();
            Ok(())
        }
    }

    #[test]
    fn known_device_is_selected() {
        let mut m = Mock::new(&["alsa-a", "alsa-b"]);
        assert_eq!(select_sink(&mut m, "alsa-b"), SinkOutcome::Selected);
        assert_eq!(m.current, "alsa-b");
        assert_eq!(m.opened, ["alsa-b"]);
    }

    #[test]
    fn unknown_id_leaves_the_output_alone() {
        let mut m = Mock::new(&["alsa-a", "alsa-b"]);
        assert_eq!(select_sink(&mut m, "alsa-b"), SinkOutcome::Selected);
        assert_eq!(select_sink(&mut m, "no-such-device"), SinkOutcome::Unknown);
        assert_eq!(m.current, "alsa-b");
        assert_eq!(m.opened, ["alsa-b"]);
        // The backend's own pseudo ids are no devices either.
        assert_eq!(select_sink(&mut m, "none"), SinkOutcome::Unknown);
        assert_eq!(m.current, "alsa-b");
    }

    #[test]
    fn empty_id_is_the_default_device() {
        let mut m = Mock::new(&["alsa-a"]);
        // Already the default: no-op, even with no device listed.
        assert_eq!(select_sink(&mut m, ""), SinkOutcome::Unchanged);
        assert!(m.opened.is_empty());
        assert_eq!(select_sink(&mut m, "alsa-a"), SinkOutcome::Selected);
        // Back to the default, which is in no device list.
        assert_eq!(select_sink(&mut m, ""), SinkOutcome::Selected);
        assert_eq!(m.current, "");
        let mut none = Mock::new(&[]);
        none.current = "x".into();
        assert_eq!(select_sink(&mut none, ""), SinkOutcome::Selected);
    }

    #[test]
    fn current_sink_is_not_opened_again() {
        let mut m = Mock::new(&["alsa-a"]);
        assert_eq!(select_sink(&mut m, "alsa-a"), SinkOutcome::Selected);
        assert_eq!(select_sink(&mut m, "alsa-a"), SinkOutcome::Unchanged);
        assert_eq!(m.opened, ["alsa-a"]);
    }

    #[test]
    fn id_match_is_exact() {
        let mut m = Mock::new(&["alsa-a"]);
        for id in ["ALSA-A", "alsa-a ", " alsa-a", "alsa", "alsa-a\0"] {
            assert_eq!(select_sink(&mut m, id), SinkOutcome::Unknown, "{id:?}");
        }
        assert!(m.opened.is_empty());
    }

    #[test]
    fn device_that_will_not_open_keeps_the_output() {
        let mut m = Mock::new(&["alsa-a"]);
        m.refuse = true;
        assert_eq!(select_sink(&mut m, "alsa-a"), SinkOutcome::Failed);
        assert_eq!(m.current, "");
    }

    /// Replays the steps of the recorded X_ITE case (`oracle/web-audio-sink-baseline/sink-baseline.json`,
    /// headless Chromium, whose only output is the default device) and compares the sink after each step.
    #[test]
    fn follows_the_recorded_baseline() {
        let text = include_str!("../../../../oracle/web-audio-sink-baseline/sink-baseline.json");
        let rec: serde_json::Value = serde_json::from_str(text).unwrap();
        let mut m = Mock::new(&["default"]);
        let steps = rec["steps"].as_array().unwrap();

        assert!(steps.len() >= 7);
        assert_eq!(rec["rejections"].as_array().unwrap().len(), 0);
        assert_eq!(
            rec["log"].as_array().unwrap().len(),
            0,
            "X_ITE logs nothing"
        );

        for step in steps {
            let id = step["mediaDeviceID"].as_str().unwrap();
            select_sink(&mut m, id);
            assert_eq!(
                m.current,
                step["sinkId"].as_str().unwrap(),
                "step {}",
                step["name"]
            );
        }
    }

    /// The native host: select a listed device on a real context, then an unknown id, then the default.
    /// Opens a silent stream on the machine's sound card, so it only runs on request:
    /// `cargo nextest run -p x3d-runtime --run-ignored only native_context_selects_a_listed_device`.
    #[test]
    #[ignore = "opens the host's audio output (no sound)"]
    fn native_context_selects_a_listed_device() {
        use web_audio_api::context::AudioContextOptions;

        let Ok(ctx) = AudioContext::try_new(AudioContextOptions::default()) else {
            eprintln!("no audio output device: nothing to select");
            return;
        };
        let ids = native_output_ids();
        eprintln!("output ids: {ids:?}");
        let mut sink = ContextSink(&ctx);

        // A host lists a device that is busy or gone in one call and not in the next (ALSA does), so
        // only a device that was selected has to be the sink.
        let mut selected = 0;

        for id in &ids {
            if select_sink(&mut sink, id) == SinkOutcome::Selected {
                assert_eq!(&sink.sink_id(), id);
                selected += 1;
            }
        }

        assert!(
            ids.is_empty() || selected > 0,
            "no listed device was selected"
        );

        let before = sink.sink_id();
        assert_eq!(
            select_sink(&mut sink, "no-such-device"),
            SinkOutcome::Unknown
        );
        assert_eq!(sink.sink_id(), before);
        select_sink(&mut sink, "");
        assert_eq!(sink.sink_id(), "");
    }
}
