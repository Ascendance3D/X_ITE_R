//! Native keys 01: every keyboard shortcut of X3DCoreContext replayed on the native viewer and compared, step by step, with what
//! the pinned X_ITE 16.5.0 did for the same keys (oracle/native-keys-01-baseline, recorded by oracle/x_ite-native-keys-01.mjs,
//! three identical runs): Timings (Ctrl+Plus), the Debug keys (Ctrl+1..5, Ctrl+S), Home / End / Page Up / Page Down,
//! the modifier flags (Shift, Ctrl, Alt, Command, Alt Gr) and Shift+F8. Compared after each key: the Timings, Shading and Debug
//! options, `isLive`, the description, the bound viewpoint, the five key flags and the clipboard.
#![cfg(feature = "gpu")]

use serde_json::Value;
use std::path::PathBuf;
use x3d_runtime::browser_options::OptionValue;
use x3d_viewer::ui::UiRequest;
use x3d_viewer::viewer::{UiEvent, Viewer3d};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;
const SIZE: u32 = 256;

const BASELINE: &str =
    include_str!("../../../oracle/native-keys-01-baseline/native-keys-01-baseline.json");

const WORLD: &str = "#VRML V2.0 utf8
DEF V1 Viewpoint { description \"A\" position 1 2 10 orientation 0 1 0 0.5 centerOfRotation 0 1 0 fieldOfView 0.6 }
DEF V2 Viewpoint { description \"B\" position 0 0 12 }
DEF V3 Viewpoint { description \"C\" position 0 0 14 }
Shape { appearance Appearance { material Material { } } geometry Box { } }
";

fn big<F: FnOnce() + Send + 'static>(f: F) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

struct Replay {
    v: Viewer3d,
    now: f64,
    clipboard: String,
}

impl Replay {
    fn open() -> Replay {
        let dir =
            std::env::temp_dir().join(format!("x3d-viewer-native-keys-01-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p: PathBuf = dir.join("keys.wrl");
        std::fs::write(&p, WORLD).unwrap();
        // The host page's option defaults (the browser build's attributes), as the recorder's page has.
        let mut v = Viewer3d::load_attributes(&p, p.parent(), T0, SIZE, SIZE, None, false, &[])
            .expect("load");
        v.frame_rate = 1.0 / DT;
        v.native_ui = true;
        let mut r = Replay {
            v,
            now: T0,
            clipboard: "before".into(),
        };
        r.frames(8);
        r
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.now += DT;
            self.v.date_now = self.now;
            self.v.step(self.now);
        }
    }

    fn option(&self, name: &str) -> OptionValue {
        self.v.rt.browser_options.get_option(name).cloned().unwrap()
    }

    fn requests(&mut self) {
        for request in self.v.take_ui_requests() {
            if let UiRequest::Clipboard(text) = request {
                self.clipboard = text;
                // The window reports a finished copy (navigator.clipboard.writeText resolves).
                self.v.clipboard_done();
            }
        }
    }

    fn state(&self) -> Value {
        let f = self.v.key_flags();
        serde_json::json!({
            "timings": self.option("Timings").as_bool(),
            "shading": self.option("Shading").as_str(),
            "debug": self.option("Debug").as_bool(),
            "live": self.v.is_live(),
            "description": self.v.rt.description,
            "bound": self.v.active_viewpoint_name(),
            "keys": f,
            "clipboard": self.clipboard,
        })
    }
}

fn replay(case: &str) {
    let baseline: Value = serde_json::from_str(BASELINE).unwrap();
    let steps = baseline["cases"][case].as_array().expect("case").clone();
    let case = case.to_string();

    big(move || {
        let mut r = Replay::open();
        let mut last = String::new();

        for step in &steps {
            let action = step["action"].as_str().unwrap();
            let words: Vec<&str> = action.split(' ').collect();

            match words[0] {
                "start" => {}
                "wait" => {
                    for _ in 0..(words[1].parse::<f64>().unwrap() / 1000.0 / DT).round() as usize {
                        r.frames(1);
                    }
                }
                "option" => {
                    let value = match words[2] {
                        "true" => OptionValue::Bool(true),
                        other => panic!("option value {other}"),
                    };
                    r.v.set_browser_option(words[1], value);
                }
                down_up => {
                    // "down Control 17", "up = 187"
                    let key = words[1];
                    let code: u32 = words[2].parse().unwrap();
                    r.v.input(UiEvent::NativeKey {
                        down: down_up == "down",
                        key: key.into(),
                        code,
                    });
                }
            }

            r.frames(3);
            r.requests();
            r.frames(1);

            // The pinned X_ITE's description and the native one are both what `browser .description` reads.
            assert_eq!(
                r.state(),
                step["state"],
                "{case}: after `{action}` (previous `{last}`)"
            );
            last = action.to_string();
        }
    });
}

#[test]
fn timings_ctrl_plus() {
    replay("timings");
}

#[test]
fn debug_keys_do_nothing_without_debug() {
    replay("debugOff");
}

#[test]
fn debug_shading_keys() {
    replay("debugShading");
}

#[test]
fn debug_live_key() {
    replay("debugLive");
}

#[test]
fn home_end_page_keys() {
    replay("viewpoints");
}

#[test]
fn modifier_flags() {
    replay("modifierFlags");
}

#[test]
fn shift_f8() {
    replay("shiftF8");
}
