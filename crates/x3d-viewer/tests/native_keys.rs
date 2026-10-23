//! The native window's keyboard path without a window: winit key events through
//! `keys::KeyTracker` into `UiEvent::NativeKey`, then KeySensor / StringSensor and the X3DCoreContext
//! viewpoint keys, as `window.rs` wires them. The expected events follow KeySensor.js,
//! StringSensor.js and X3DCoreContext.js of the pinned X_ITE 16.5.0.
#![cfg(feature = "gpu")]

use std::path::PathBuf;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use x3d_viewer::keys::{KeyAction, KeyTracker, Platform, Shortcut};
use x3d_viewer::viewer::{UiEvent, Viewer3d};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;

fn world(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("x3d-viewer-native-keys-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

/// Run on a big stack (QuickJS), like the binaries do.
fn big<F: FnOnce() + Send + 'static>(f: F) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

fn open(name: &str, text: &str) -> Viewer3d {
    let p = world(name, text);
    let mut v = Viewer3d::load(&p, p.parent(), T0, 512, 512).expect("load");
    v.frame_rate = 1.0 / DT;
    for _ in 0..8 {
        v.step(T0);
    }
    v
}

/// The window: a tracker, the modifier state, and the events X3D got.
struct Win {
    v: Viewer3d,
    t: KeyTracker,
    p: Platform,
    mods: ModifiersState,
    shortcuts: Vec<Shortcut>,
    dom: usize,
    now: f64,
}

impl Win {
    fn new(v: Viewer3d, p: Platform) -> Win {
        Win {
            v,
            t: KeyTracker::default(),
            p,
            mods: ModifiersState::empty(),
            shortcuts: Vec::new(),
            dom: 0,
            now: T0,
        }
    }

    fn apply(&mut self, a: KeyAction) {
        match a {
            KeyAction::Dom { down, key, code } => {
                self.dom += 1;
                self.v.input(UiEvent::NativeKey { down, key, code });
            }
            KeyAction::Shortcut(s) => self.shortcuts.push(s),
            KeyAction::None => {}
        }
    }

    fn raw(&mut self, down: bool, repeat: bool, k: Key, c: KeyCode) {
        let modifier = match &k {
            Key::Named(NamedKey::Shift) => ModifiersState::SHIFT,
            Key::Named(NamedKey::Control) => ModifiersState::CONTROL,
            Key::Named(NamedKey::Alt) => ModifiersState::ALT,
            Key::Named(NamedKey::Super) => ModifiersState::SUPER,
            _ => ModifiersState::empty(),
        };

        // winit reports ModifiersChanged with the key's press / release.
        if down {
            self.mods |= modifier;
        } else {
            self.mods -= modifier;
        }

        let a = self.t.event(
            self.p,
            down,
            repeat,
            false,
            &k,
            &k,
            &PhysicalKey::Code(c),
            self.mods,
        );
        self.apply(a);
    }

    fn press(&mut self, k: Key, c: KeyCode) {
        self.raw(true, false, k.clone(), c);
        self.raw(false, false, k, c);
    }

    fn focus_lost(&mut self) {
        self.mods = ModifiersState::empty();

        for a in self.t.release_all() {
            self.apply(a);
        }
    }

    fn frame(&mut self) {
        self.now += DT;
        self.v.date_now = self.now;
        self.v.step(self.now);
    }

    fn hist(&mut self) -> String {
        self.frame();
        self.v.rt.publish_clone_counts();
        let s = self.v.rt.scene.to_vrml_string();
        let i = s.find("hist \"").expect("hist") + 6;
        s[i..s[i..].find('"').unwrap() + i].trim().to_string()
    }
}

fn ch(s: &str) -> Key {
    Key::Character(s.into())
}

fn named(n: NamedKey) -> Key {
    Key::Named(n)
}

const LOG: &str = r#"
    function esc (s) { var r = ''; for (var i = 0; i < s .length; ++ i) { var c = s .charCodeAt (i); r += c < 32 || c == 127 ? '\\\\' + c : s .charAt (i); } return r; }
    function add (n, v) { hist = hist + ' ' + n + '=' + v; }
"#;

fn key_sensor_world() -> String {
    format!(
        r#"#X3D V3.3 utf8
PROFILE Immersive
COMPONENT KeyDeviceSensor:2
DEF KS KeySensor {{ }}
DEF OUT Script {{
  inputOnly SFString kp
  inputOnly SFString kr
  inputOnly SFInt32 ap
  inputOnly SFInt32 ar
  inputOnly SFBool sh
  inputOnly SFBool ct
  inputOnly SFBool al
  inputOnly SFBool ka
  inputOutput SFString hist ""
  url "ecmascript:
    {LOG}
    function kp (v) {{ add ('kp', esc (v)); }}
    function kr (v) {{ add ('kr', esc (v)); }}
    function ap (v) {{ add ('ap', v); }}
    function ar (v) {{ add ('ar', v); }}
    function sh (v) {{ add ('sh', v); }}
    function ct (v) {{ add ('ct', v); }}
    function al (v) {{ add ('al', v); }}
    function ka (v) {{ add ('ka', v); }}
  "
}}
ROUTE KS.keyPress TO OUT.kp
ROUTE KS.keyRelease TO OUT.kr
ROUTE KS.actionKeyPress TO OUT.ap
ROUTE KS.actionKeyRelease TO OUT.ar
ROUTE KS.shiftKey TO OUT.sh
ROUTE KS.controlKey TO OUT.ct
ROUTE KS.altKey TO OUT.al
ROUTE KS.isActive TO OUT.ka
"#
    )
}

fn string_sensor_world() -> String {
    format!(
        r#"#X3D V3.3 utf8
PROFILE Immersive
COMPONENT KeyDeviceSensor:2
DEF SS StringSensor {{ }}
DEF OUT Script {{
  inputOnly SFString et
  inputOnly SFString ft
  inputOnly SFBool sa
  inputOutput SFString hist ""
  url "ecmascript:
    {LOG}
    function et (v) {{ add ('et', esc (v)); }}
    function ft (v) {{ add ('ft', esc (v)); }}
    function sa (v) {{ add ('sa', v); }}
  "
}}
ROUTE SS.enteredText TO OUT.et
ROUTE SS.finalText TO OUT.ft
ROUTE SS.isActive TO OUT.sa
"#
    )
}

#[test]
fn key_sensor_gets_native_keys() {
    big(|| {
        let mut w = Win::new(open("ks.wrl", &key_sensor_world()), Platform::Linux);

        w.press(ch("q"), KeyCode::KeyQ);
        assert_eq!(w.hist(), "ka=true kp=q kr=q ka=false");

        // Shift + b: modifier order, then the shifted letter.
        w.raw(true, false, named(NamedKey::Shift), KeyCode::ShiftLeft);
        w.press(ch("B"), KeyCode::KeyB);
        w.raw(false, false, named(NamedKey::Shift), KeyCode::ShiftLeft);
        // Ctrl and Alt: controlKey / altKey; Ctrl + a is text "a" for X3D (no shortcut).
        w.raw(true, false, named(NamedKey::Control), KeyCode::ControlLeft);
        w.press(ch("a"), KeyCode::KeyA);
        w.raw(false, false, named(NamedKey::Control), KeyCode::ControlLeft);
        w.raw(true, false, named(NamedKey::Alt), KeyCode::AltLeft);
        w.raw(false, false, named(NamedKey::Alt), KeyCode::AltLeft);
        // Action keys: F1, F12, Home, End, PageUp, PageDown, arrows.
        for (k, c) in [
            (NamedKey::F1, KeyCode::F1),
            (NamedKey::F12, KeyCode::F12),
            (NamedKey::Home, KeyCode::Home),
            (NamedKey::End, KeyCode::End),
            (NamedKey::PageUp, KeyCode::PageUp),
            (NamedKey::PageDown, KeyCode::PageDown),
            (NamedKey::ArrowUp, KeyCode::ArrowUp),
            (NamedKey::ArrowDown, KeyCode::ArrowDown),
            (NamedKey::ArrowLeft, KeyCode::ArrowLeft),
            (NamedKey::ArrowRight, KeyCode::ArrowRight),
        ] {
            w.press(named(k), c);
        }
        // Text keys with control characters; Escape is an X3D key.
        for (k, c) in [
            (NamedKey::Enter, KeyCode::Enter),
            (NamedKey::Backspace, KeyCode::Backspace),
            (NamedKey::Tab, KeyCode::Tab),
            (NamedKey::Escape, KeyCode::Escape),
            (NamedKey::Delete, KeyCode::Delete),
            (NamedKey::Space, KeyCode::Space),
        ] {
            w.press(named(k), c);
        }
        // A letter of another layout (German ö on the US ; key) is text too.
        w.press(ch("ö"), KeyCode::Semicolon);

        let h = w.hist();
        let expect = [
            "ka=true kp=q kr=q ka=false",
            "ka=true sh=true kp=B kr=B ka=false sh=false",
            "ka=true ct=true kp=a kr=a ka=false ct=false",
            "ka=true al=true al=false ka=false",
            "ka=true ap=1 ar=1 ka=false",
            "ka=true ap=12 ar=12 ka=false",
            "ka=true ap=13 ar=13 ka=false",
            "ka=true ap=14 ar=14 ka=false",
            "ka=true ap=15 ar=15 ka=false",
            "ka=true ap=16 ar=16 ka=false",
            "ka=true ap=17 ar=17 ka=false",
            "ka=true ap=18 ar=18 ka=false",
            "ka=true ap=19 ar=19 ka=false",
            "ka=true ap=20 ar=20 ka=false",
            "ka=true kp=\\\\10 kr=\\\\10 ka=false",
            "ka=true kp=\\\\8 kr=\\\\8 ka=false",
            "ka=true kp=\\\\9 kr=\\\\9 ka=false",
            "ka=true kp=\\\\27 kr=\\\\27 ka=false",
            "ka=true kp=\\\\127 kr=\\\\127 ka=false",
            "ka=true kp=  kr=  ka=false",
            "ka=true kp=ö kr=ö ka=false",
        ]
        .join(" ");
        assert_eq!(h, expect);
        assert!(w.shortcuts.is_empty());
    });
}

#[test]
fn key_sensor_repeat_and_shortcut() {
    big(|| {
        let mut w = Win::new(open("ks-repeat.wrl", &key_sensor_world()), Platform::Linux);

        // A held key: keydown, two repeats (keydowns, no keyup between), one keyup.
        w.raw(true, false, ch("x"), KeyCode::KeyX);
        w.raw(true, true, ch("x"), KeyCode::KeyX);
        w.raw(true, true, ch("x"), KeyCode::KeyX);
        w.raw(false, false, ch("x"), KeyCode::KeyX);
        assert_eq!(w.hist(), "ka=true kp=x kp=x kp=x kr=x ka=false");
        assert_eq!(w.dom, 4);

        // Ctrl + R: the Ctrl key reaches the sensor, R is the reload shortcut only (once, with repeats).
        w.raw(true, false, named(NamedKey::Control), KeyCode::ControlLeft);
        w.raw(true, false, ch("r"), KeyCode::KeyR);
        w.raw(true, true, ch("r"), KeyCode::KeyR);
        w.raw(false, false, ch("r"), KeyCode::KeyR);
        w.raw(false, false, named(NamedKey::Control), KeyCode::ControlLeft);
        assert_eq!(w.shortcuts, vec![Shortcut::Reload]);
        assert!(
            w.hist()
                .ends_with("ka=false ka=true ct=true ct=false ka=false"),
            "{}",
            w.hist()
        );
    });
}

#[test]
fn key_sensor_released_on_focus_loss() {
    big(|| {
        let mut w = Win::new(open("ks-focus.wrl", &key_sensor_world()), Platform::Linux);

        w.raw(true, false, named(NamedKey::Shift), KeyCode::ShiftLeft);
        w.raw(true, false, named(NamedKey::Control), KeyCode::ControlLeft);
        w.raw(true, false, named(NamedKey::ArrowUp), KeyCode::ArrowUp);
        w.focus_lost();
        assert_eq!(
            w.hist(),
            "ka=true sh=true ct=true ap=17 sh=false ka=false ct=false ar=17"
        );
        // Nothing stays down for the next key.
        w.press(ch("k"), KeyCode::KeyK);
        assert!(
            w.hist().ends_with("ar=17 ka=true kp=k kr=k ka=false"),
            "{}",
            w.hist()
        );
    });
}

#[test]
fn string_sensor_gets_native_text() {
    big(|| {
        let mut w = Win::new(open("ss.wrl", &string_sensor_world()), Platform::Linux);

        w.press(ch("h"), KeyCode::KeyH);
        w.press(ch("i"), KeyCode::KeyI);
        w.press(named(NamedKey::Backspace), KeyCode::Backspace);
        w.raw(true, false, named(NamedKey::Shift), KeyCode::ShiftLeft);
        w.press(ch("I"), KeyCode::KeyI);
        w.press(ch("!"), KeyCode::Digit1);
        w.raw(false, false, named(NamedKey::Shift), KeyCode::ShiftLeft);
        w.press(named(NamedKey::Space), KeyCode::Space);
        // Repeats insert the character again.
        w.raw(true, false, ch("o"), KeyCode::KeyO);
        w.raw(true, true, ch("o"), KeyCode::KeyO);
        w.raw(false, false, ch("o"), KeyCode::KeyO);
        // Ctrl + letters (not shortcuts) and AltGr / Alt combinations are text for the sensor.
        w.raw(true, false, named(NamedKey::Control), KeyCode::ControlLeft);
        w.press(ch("a"), KeyCode::KeyA);
        w.raw(false, false, named(NamedKey::Control), KeyCode::ControlLeft);
        w.press(named(NamedKey::Tab), KeyCode::Tab);
        w.press(named(NamedKey::Enter), KeyCode::Enter);

        // isActive TRUE, then enteredText "" and "h" in one event cascade: X3DBaseNode.addEvent sends a
        // field once per cascade (a tainted field only takes the new value), so "" is never seen.
        let expect = "sa=true et=h et=hi et=h et=hI et=hI! et=hI!  et=hI! o et=hI! oo et=hI! ooa ft=hI! ooa sa=false";
        assert_eq!(w.hist(), expect);

        // Escape clears without finalText; the window stays open (no shortcut).
        w.press(ch("x"), KeyCode::KeyX);
        w.press(named(NamedKey::Escape), KeyCode::Escape);
        assert!(
            w.hist().ends_with("sa=false sa=true et=x sa=false"),
            "{}",
            w.hist()
        );
        assert!(w.shortcuts.is_empty());
    });
}

#[test]
fn viewpoint_keys_run_once() {
    big(|| {
        let text = r#"#X3D V3.3 utf8
PROFILE Immersive
DEF A Viewpoint { description "A" }
DEF B Viewpoint { description "B" position 0 0 20 }
DEF C Viewpoint { description "C" position 0 0 30 }
"#;
        // The binding takes effect in the next frame.
        let bound = |w: &mut Win| {
            w.frame();
            w.v.active_viewpoint_name()
        };
        let mut w = Win::new(open("vp.wrl", text), Platform::Linux);
        assert_eq!(bound(&mut w), "A");

        // PageDown once: one viewpoint further, not two.
        w.press(named(NamedKey::PageDown), KeyCode::PageDown);
        assert_eq!(bound(&mut w), "B");
        w.press(named(NamedKey::PageDown), KeyCode::PageDown);
        assert_eq!(bound(&mut w), "C");
        w.press(named(NamedKey::PageUp), KeyCode::PageUp);
        assert_eq!(bound(&mut w), "B");
        w.press(named(NamedKey::Home), KeyCode::Home);
        assert_eq!(bound(&mut w), "A");
        w.press(named(NamedKey::End), KeyCode::End);
        assert_eq!(bound(&mut w), "C");
        // With Ctrl (Linux) / Command (macOS) they are still viewpoint keys, not shortcuts.
        w.raw(true, false, named(NamedKey::Control), KeyCode::ControlLeft);
        w.press(named(NamedKey::Home), KeyCode::Home);
        w.raw(false, false, named(NamedKey::Control), KeyCode::ControlLeft);
        assert_eq!(bound(&mut w), "A");
        assert!(w.shortcuts.is_empty());
        assert_eq!(w.dom, 14);
    });
}

#[test]
fn macos_command_shortcuts_leave_ctrl_to_x3d() {
    big(|| {
        let mut w = Win::new(open("ks-mac.wrl", &key_sensor_world()), Platform::MacOs);

        // Ctrl + Q on macOS is X3D input; Command + Q is the quit shortcut.
        w.raw(true, false, named(NamedKey::Control), KeyCode::ControlLeft);
        w.press(ch("q"), KeyCode::KeyQ);
        w.raw(false, false, named(NamedKey::Control), KeyCode::ControlLeft);
        w.raw(true, false, named(NamedKey::Super), KeyCode::SuperLeft);
        w.press(ch("q"), KeyCode::KeyQ);
        w.press(ch("1"), KeyCode::Digit1);
        w.raw(false, false, named(NamedKey::Super), KeyCode::SuperLeft);
        assert_eq!(
            w.shortcuts,
            vec![Shortcut::Quit, Shortcut::Viewer("EXAMINE")]
        );
        // The Command key itself is a KeySensor key without a field (X_ITE: keyCode 91, no text).
        assert_eq!(
            w.hist(),
            "ka=true ct=true kp=q kr=q ka=false ct=false ka=true ka=false"
        );
    });
}
