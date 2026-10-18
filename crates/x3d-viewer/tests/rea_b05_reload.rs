//! REA batch 05: the lifetime of the Sound nodes through Inline unload / reload and world reloads.
//! reload/reload-inline.wrl loads reload/reload-inline-content.wrl (every batch 05 node: Gain,
//! BiquadFilter, Delay, WaveShaper, Convolver, DynamicsCompressor, Analyser, ChannelSplitter,
//! ChannelMerger, ChannelSelector, OscillatorSource with PeriodicWave, BufferAudioSource,
//! StreamAudioDestination, StreamAudioSource, MicrophoneSource, SpatialSound, ListenerPointSource
//! and two AudioDestinations) in an Inline that a KeySensor unloads ('s') and loads again ('r').
//! Each load makes new Web Audio nodes; the runtime's sound state, the audio nodes and their
//! connections must go with the X3D nodes, so their number stays bounded however often the Inline
//! is loaded, and the oscillator still starts after each reload. Stepping only, no GPU.

use std::path::{Path, PathBuf};
use x3d_viewer::embed::spawn_big_stack;
use x3d_viewer::viewer::{UiEvent, Viewer3d, ORACLE_PAGE_ATTRIBUTES};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;

struct Driver {
    v: Viewer3d,
    now: f64,
}

impl Driver {
    fn load(world: &Path, oracle: &Path) -> Driver {
        let mut v = Viewer3d::load_attributes(world, Some(oracle), T0, 64, 64, None, false, ORACLE_PAGE_ATTRIBUTES).unwrap();
        v.frame_rate = 1.0 / DT;

        for _ in 0..8 {
            v.step(T0);
        }

        Driver { v, now: T0 }
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.now += DT;
            self.v.date_now = self.now;
            self.v.step(self.now);
        }
    }

    fn key(&mut self, key: &str) {
        self.v.input(UiEvent::KeyDown { key: key.into() });
        self.v.input(UiEvent::KeyUp { key: key.into() });
        self.frames(3);
    }

    fn starts(&self) -> i32 {
        let rt = &self.v.rt;
        let r = rt.scene.get_named_node(0, "R").expect("R");
        match rt.get_field(r, "starts").map(|k| rt.value(k)) {
            Some(x3d_fields::FieldValue::SFInt32(n)) => *n,
            other => panic!("starts {other:?}"),
        }
    }

    /// Sound state entries, live Web Audio nodes, connections.
    fn state(&self) -> [usize; 3] {
        let rt = &self.v.rt;
        let (nodes, connections) = rt.audio.as_ref().map_or((0, 0), |e| (e.node_count(), e.connection_count()));
        [rt.sound.len(), nodes, connections]
    }
}

#[test]
fn rea_b05_sound_state_is_released_on_reload() {
    spawn_big_stack(|| {
        let oracle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle");
        let world = oracle.join("synthetic-rea-b05/reload/reload-inline.wrl");
        let mut d = Driver::load(&world, &oracle);
        d.frames(20);
        let first = d.state();
        assert_eq!(d.starts(), 1, "the oscillator did not start");

        d.key("s");
        d.frames(5);
        let empty = d.state();
        assert!(empty.iter().zip(first.iter()).all(|(a, b)| a < b), "the unload released nothing: {first:?} -> {empty:?}");
        d.key("r");
        d.frames(20);
        let mut states = vec![d.state()];

        for cycle in 0..30 {
            d.key("s");
            d.frames(5);
            assert_eq!(d.state(), empty, "cycle {cycle}: sound state or audio nodes kept after the unload");
            let before = d.starts();
            d.key("r");
            d.frames(20);
            assert_eq!(d.starts(), before + 1, "cycle {cycle}: the oscillator did not start after the reload");
            states.push(d.state());
        }

        for _ in 0..5 {
            d = Driver::load(&world, &oracle);
            d.frames(20);
            states.push(d.state());
        }

        eprintln!("batch 05 state [sound, audio nodes, connections]: loaded {first:?}, unloaded {empty:?}, after reloads {states:?}");

        for s in &states {
            assert_eq!(s, &first, "the sound state changes with reloads: first {first:?}, then {states:?}");
        }
    });
}
