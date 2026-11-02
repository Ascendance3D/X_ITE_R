//! What opens the native context menu, compared with the pinned X_ITE 16.5.0 (oracle/menu-triggers-01-baseline, recorded by
//! oracle/x_ite-menu-triggers-01.mjs, five identical runs): the right button, the `contextmenu` event that macOS gives a
//! Ctrl+click, and PointingDevice's long tap (1200 ms, 7 px, not on a sensor). Each case of the recording is replayed on the
//! native viewer through `Viewer3d::input`, with the events that `window.rs` sends (`Platform::press_events`, `TouchTracker`),
//! and the menu's open / closed state is compared.
//!
//! Not exercised here (no such host): a real macOS Ctrl+click (the Linux host gives `Platform::MacOs` only to the pure
//! mapping), and a real touch screen (the winit `Touch` events are mapped by `TouchTracker`, tested on synthetic phases).
#![cfg(feature = "gpu")]

use serde_json::Value;
use std::path::PathBuf;
use x3d_viewer::keys::Platform;
use x3d_viewer::touch::{Phase, TouchTracker};
use x3d_viewer::viewer::{UiEvent, Viewer3d};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;
/// The recording's canvas is 320 x 320.
const SIZE: u32 = 320;

const BASELINE: &str =
    include_str!("../../../oracle/menu-triggers-01-baseline/menu-triggers-01-baseline.json");

const PLAIN: &str =
    "#VRML V2.0 utf8\nShape { appearance Appearance { material Material { } } geometry Box { } }\n";
const SENSOR: &str = "#VRML V2.0 utf8
Group { children [ DEF TS TouchSensor { } Shape { appearance Appearance { material Material { } } geometry Box { } } ] }
DEF OUT Script {
  inputOnly SFBool act
  inputOutput SFString hist \"\"
  url \"ecmascript:
    function act (v) { hist = hist + ' act=' + v; }
  \"
}
ROUTE TS.isActive TO OUT.act
";

fn big<F: FnOnce() + Send + 'static>(f: F) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

struct Ui {
    v: Viewer3d,
    now: f64,
    touches: TouchTracker,
}

impl Ui {
    fn open(name: &str, text: &str) -> Ui {
        let dir =
            std::env::temp_dir().join(format!("x3d-viewer-menu-triggers-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p: PathBuf = dir.join(name);
        std::fs::write(&p, text).unwrap();
        let mut v = Viewer3d::load_attributes(&p, p.parent(), T0, SIZE, SIZE, None, false, &[])
            .expect("load");
        v.frame_rate = 1.0 / DT;
        v.native_ui = true;
        let mut ui = Ui {
            v,
            now: T0,
            touches: TouchTracker::default(),
        };

        ui.seconds(8.0 * DT);
        ui
    }

    fn seconds(&mut self, s: f64) {
        for _ in 0..(s / DT).round() as usize {
            self.now += DT;
            self.v.date_now = self.now;
            self.v.step(self.now);
        }
    }

    /// The recording's top-left coordinates to the viewer's bottom-left ones.
    fn at(x: f64, y_top: f64) -> (f64, f64) {
        (x, SIZE as f64 - y_top)
    }

    fn open_menu(&self) -> bool {
        self.v.context_menu().is_some()
    }

    fn close(&mut self) {
        self.v.hide_context_menu();
    }

    /// Mouse events as `window.rs` sends them, on `platform`.
    fn press(&mut self, platform: Platform, button: u8, ctrl: bool, x: f64, y_top: f64) {
        let (x, y) = Ui::at(x, y_top);

        self.v.input(UiEvent::MouseMove { x, y });

        for e in platform.press_events(button, ctrl, x, y) {
            self.v.input(e);
        }
    }

    fn release(&mut self, button: u8, x: f64, y_top: f64) {
        let (x, y) = Ui::at(x, y_top);

        self.v.input(UiEvent::MouseUp { button, x, y });
    }

    fn touch(&mut self, phase: Phase, x: f64, y_top: f64) {
        let pos = Ui::at(x, y_top);

        if let Some(e) = self.touches.event(phase, 1, pos, SIZE as f64) {
            self.v.input(e);
        }
    }

    /// The recorder's `touch`: a start, an optional move 50 ms later, the hold, the end.
    fn hold(&mut self, x: f64, y: f64, hold: f64, mv: f64) {
        self.touch(Phase::Started, x, y);

        if mv != 0.0 {
            self.seconds(0.05);
            self.touch(Phase::Moved, x + mv, y);
        }

        self.seconds(hold);
        self.touch(Phase::Ended, x + mv, y);
        self.seconds(DT * 2.0);
    }

    fn hist(&mut self) -> String {
        self.seconds(DT);
        self.v.rt.publish_clone_counts();
        let s = self.v.rt.scene.to_vrml_string();
        let i = s.find("hist \"").expect("hist") + 6;
        s[i..s[i..].find('"').unwrap() + i].trim().to_string()
    }
}

fn baseline(name: &str) -> bool {
    let all: Value = serde_json::from_str(BASELINE).unwrap();

    all["cases"][name]["menu"]["open"]
        .as_bool()
        .unwrap_or_else(|| panic!("case {name} in the baseline"))
}

#[test]
fn mouse_triggers_match_the_recording() {
    big(|| {
        let mut ui = Ui::open("plain.wrl", PLAIN);

        // The right button: the press brings the `contextmenu` event on Linux and Windows.
        ui.press(Platform::Linux, 2, false, 100.0, 100.0);
        ui.v.input(UiEvent::ContextMenu { x: 100.0, y: 220.0 });
        ui.release(2, 100.0, 100.0);
        assert_eq!(ui.open_menu(), baseline("rightClick"));
        assert!(ui.open_menu());
        ui.close();

        // A left click, and a Ctrl+left click on Linux (and Windows): no menu.
        ui.press(Platform::Linux, 0, false, 100.0, 100.0);
        ui.release(0, 100.0, 100.0);
        assert_eq!(ui.open_menu(), baseline("leftClick"));
        ui.press(Platform::Linux, 0, true, 100.0, 100.0);
        ui.release(0, 100.0, 100.0);
        assert_eq!(ui.open_menu(), baseline("ctrlClickReal"));
        assert!(!ui.open_menu());

        // macOS: the press, the `contextmenu` event (button 0, Ctrl), the release.
        ui.press(Platform::MacOs, 0, true, 100.0, 100.0);
        ui.release(0, 100.0, 100.0);
        assert_eq!(ui.open_menu(), baseline("ctrlClickMacSequence"));
        assert!(ui.open_menu());
        ui.close();

        // Without Ctrl, macOS gives a plain click; Ctrl with another button is not Ctrl+click.
        ui.press(Platform::MacOs, 0, false, 100.0, 100.0);
        ui.release(0, 100.0, 100.0);
        assert!(!ui.open_menu());
        ui.press(Platform::MacOs, 1, true, 100.0, 100.0);
        ui.release(1, 100.0, 100.0);
        assert!(!ui.open_menu());

        // The `contextmenu` event of any button, with or without Ctrl (the keyboard's menu key, a touch device).
        ui.v.input(UiEvent::ContextMenu { x: 140.0, y: 200.0 });
        assert_eq!(ui.open_menu(), baseline("contextmenuButton0"));
        ui.close();
        ui.v.input(UiEvent::ContextMenu { x: 140.0, y: 200.0 });
        assert_eq!(ui.open_menu(), baseline("contextmenuCtrl"));

        // A second `contextmenu` event opens the menu again at the new point (the layer hides it first).
        ui.v.input(UiEvent::ContextMenu { x: 20.0, y: 20.0 });
        assert!(ui.open_menu());
    });
}

#[test]
fn macos_ctrl_click_reaches_the_world_like_the_recording() {
    big(|| {
        // The recording: mousedown (button 0), contextmenu, mouseup. The world gets the press and, after the menu opened,
        // the release of the button that went down before it.
        let mut ui = Ui::open("sensor.wrl", SENSOR);

        ui.press(Platform::MacOs, 0, true, 160.0, 160.0);
        assert!(ui.open_menu());
        ui.release(0, 160.0, 160.0);
        assert_eq!(ui.hist(), "act=true act=false");
    });
}

#[test]
fn touch_long_press_matches_the_recording() {
    big(|| {
        let cases: [(&str, &str, f64, f64, f64, f64); 12] = [
            ("touchTap", PLAIN, 100.0, 0.05, 0.0, 100.0),
            ("touchHold400", PLAIN, 100.0, 0.4, 0.0, 100.0),
            ("touchHold700", PLAIN, 100.0, 0.7, 0.0, 100.0),
            ("touchHold1000", PLAIN, 100.0, 1.0, 0.0, 100.0),
            ("touchHold1100", PLAIN, 100.0, 1.1, 0.0, 100.0),
            ("touchHold1300", PLAIN, 100.0, 1.3, 0.0, 100.0),
            ("touchHold1500", PLAIN, 100.0, 1.5, 0.0, 100.0),
            ("touchHold1500Move6", PLAIN, 100.0, 1.5, 6.0, 100.0),
            ("touchHold1500Move7", PLAIN, 100.0, 1.5, 7.0, 100.0),
            ("touchHold1500Move8", PLAIN, 100.0, 1.5, 8.0, 100.0),
            ("touchHold1500Move40", PLAIN, 100.0, 1.5, 40.0, 100.0),
            ("touchHold1500OnShape", PLAIN, 160.0, 1.5, 0.0, 160.0),
        ];

        for (name, world, x, hold, mv, y) in cases {
            let mut ui = Ui::open("touch.wrl", world);

            ui.hold(x, y, hold, mv);
            assert_eq!(ui.open_menu(), baseline(name), "{name}");
        }

        // A touch that a TouchSensor takes does not start the timer.
        let mut ui = Ui::open("touch-sensor.wrl", SENSOR);

        ui.hold(160.0, 160.0, 1.5, 0.0);
        assert_eq!(ui.open_menu(), baseline("touchHold1500OnSensor"));
        assert!(!ui.open_menu());
        assert_eq!(ui.hist(), "act=true act=false", "the sensor got the touch");
    });
}

#[test]
fn touch_long_press_rules_not_in_the_recording() {
    big(|| {
        // The timer fires while the finger is still down; the menu is there at that moment.
        let mut ui = Ui::open("plain.wrl", PLAIN);

        ui.touch(Phase::Started, 100.0, 100.0);
        ui.seconds(1.1);
        assert!(!ui.open_menu());
        ui.seconds(0.2);
        assert!(ui.open_menu(), "open at 1.3 s with the finger down");
        ui.touch(Phase::Ended, 100.0, 100.0);
        assert!(ui.open_menu(), "the lift leaves the menu");
        ui.close();

        // The ContextMenu option off: nothing opens.
        ui.v.set_browser_option(
            "ContextMenu",
            x3d_runtime::browser_options::OptionValue::Bool(false),
        );
        ui.hold(100.0, 100.0, 1.5, 0.0);
        assert!(!ui.open_menu());
        ui.v.set_browser_option(
            "ContextMenu",
            x3d_runtime::browser_options::OptionValue::Bool(true),
        );

        // A second finger cancels it (PointingDevice.touchstart with two touches calls touchend).
        ui.touch(Phase::Started, 100.0, 100.0);
        ui.seconds(0.5);
        let two = ui
            .touches
            .event(Phase::Started, 2, Ui::at(200.0, 200.0), SIZE as f64);
        ui.v.input(two.unwrap());
        ui.seconds(1.5);
        assert!(!ui.open_menu());
        ui.touch(Phase::Ended, 100.0, 100.0);
        let _ = ui.touches.event(Phase::Ended, 2, (0.0, 0.0), SIZE as f64);

        // The browser build and the drivers (no native UI) leave the long tap to the page.
        ui.v.native_ui = false;
        ui.hold(100.0, 100.0, 1.5, 0.0);
        assert!(!ui.open_menu());
        ui.v.native_ui = true;

        // A tap on an entry of the open menu is a click on it (the touch is the layer's press).
        ui.v.input(UiEvent::ContextMenu { x: 100.0, y: 220.0 });

        let menu = ui.v.context_menu().expect("menu");
        let p = menu.find(&[], "Browser Timings").expect("entry");
        let (cx, cy) = menu.center_of(&p).expect("center");

        ui.v.input(UiEvent::MouseMove {
            x: cx,
            y: SIZE as f64 - cy,
        });
        ui.touch(Phase::Started, cx, cy);
        ui.touch(Phase::Ended, cx, cy);
        // A checkbox entry leaves the menu open (ContextMenu.js hides it only for other entries).
        assert!(ui.open_menu());
        assert!(
            ui.v.rt
                .browser_options
                .get_option("Timings")
                .is_some_and(|v| v.as_bool())
        );
    });
}
