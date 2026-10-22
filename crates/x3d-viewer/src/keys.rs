//! Native keys as the DOM KeyboardEvent X_ITE reads (`event .key`, `event .keyCode`), and the
//! viewer's application shortcuts.
//!
//! X_ITE gets keydown / keyup of its element from the browser: X3DCoreContext (modifier state,
//! Home / End / PageUp / PageDown viewpoints) and X3DKeyDeviceSensorContext (KeySensor,
//! StringSensor) both see every event, repeats included. The browser keeps its own shortcuts. The
//! native window is the browser here: the application shortcuts need the platform shortcut
//! modifier (Ctrl on Linux, Command on macOS), and every other key goes to X3D unchanged.

use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};

/// `event .key` of a winit logical key. Unknown keys are "Unidentified" (no text, keyCode 0 unless
/// the physical key has one); a dead key is "Dead", as in the browser.
pub fn dom_key(key: &Key) -> String {
    match key {
        Key::Character(c) => c.to_string(),
        Key::Named(n) => named_key(*n).unwrap_or("Unidentified").to_string(),
        Key::Dead(_) => "Dead".into(),
        Key::Unidentified(_) => "Unidentified".into(),
    }
}

fn named_key(n: NamedKey) -> Option<&'static str> {
    Some(match n {
        NamedKey::Space => " ",
        NamedKey::Enter => "Enter",
        NamedKey::Tab => "Tab",
        NamedKey::Backspace => "Backspace",
        NamedKey::Delete => "Delete",
        NamedKey::Insert => "Insert",
        NamedKey::Escape => "Escape",
        NamedKey::ArrowLeft => "ArrowLeft",
        NamedKey::ArrowRight => "ArrowRight",
        NamedKey::ArrowUp => "ArrowUp",
        NamedKey::ArrowDown => "ArrowDown",
        NamedKey::Home => "Home",
        NamedKey::End => "End",
        NamedKey::PageUp => "PageUp",
        NamedKey::PageDown => "PageDown",
        NamedKey::Shift => "Shift",
        NamedKey::Control => "Control",
        NamedKey::Alt => "Alt",
        NamedKey::AltGraph => "AltGraph",
        // winit's Super is the DOM's Meta (Windows / Command key).
        NamedKey::Super => "Meta",
        NamedKey::CapsLock => "CapsLock",
        NamedKey::NumLock => "NumLock",
        NamedKey::ScrollLock => "ScrollLock",
        NamedKey::Pause => "Pause",
        NamedKey::PrintScreen => "PrintScreen",
        NamedKey::ContextMenu => "ContextMenu",
        NamedKey::F1 => "F1",
        NamedKey::F2 => "F2",
        NamedKey::F3 => "F3",
        NamedKey::F4 => "F4",
        NamedKey::F5 => "F5",
        NamedKey::F6 => "F6",
        NamedKey::F7 => "F7",
        NamedKey::F8 => "F8",
        NamedKey::F9 => "F9",
        NamedKey::F10 => "F10",
        NamedKey::F11 => "F11",
        NamedKey::F12 => "F12",
        _ => return None,
    })
}

/// The US-layout keyCode of a physical key, for a character the keyCode table of the key does not
/// know (a letter of another layout). 0 for a key without one.
fn physical_key_code(key: &PhysicalKey) -> u32 {
    let PhysicalKey::Code(c) = key else { return 0 };

    match c {
        KeyCode::KeyA => 65,
        KeyCode::KeyB => 66,
        KeyCode::KeyC => 67,
        KeyCode::KeyD => 68,
        KeyCode::KeyE => 69,
        KeyCode::KeyF => 70,
        KeyCode::KeyG => 71,
        KeyCode::KeyH => 72,
        KeyCode::KeyI => 73,
        KeyCode::KeyJ => 74,
        KeyCode::KeyK => 75,
        KeyCode::KeyL => 76,
        KeyCode::KeyM => 77,
        KeyCode::KeyN => 78,
        KeyCode::KeyO => 79,
        KeyCode::KeyP => 80,
        KeyCode::KeyQ => 81,
        KeyCode::KeyR => 82,
        KeyCode::KeyS => 83,
        KeyCode::KeyT => 84,
        KeyCode::KeyU => 85,
        KeyCode::KeyV => 86,
        KeyCode::KeyW => 87,
        KeyCode::KeyX => 88,
        KeyCode::KeyY => 89,
        KeyCode::KeyZ => 90,
        KeyCode::Digit0 => 48,
        KeyCode::Digit1 => 49,
        KeyCode::Digit2 => 50,
        KeyCode::Digit3 => 51,
        KeyCode::Digit4 => 52,
        KeyCode::Digit5 => 53,
        KeyCode::Digit6 => 54,
        KeyCode::Digit7 => 55,
        KeyCode::Digit8 => 56,
        KeyCode::Digit9 => 57,
        KeyCode::Space => 32,
        KeyCode::Semicolon => 186,
        KeyCode::Equal => 187,
        KeyCode::Comma => 188,
        KeyCode::Minus => 189,
        KeyCode::Period => 190,
        KeyCode::Slash => 191,
        KeyCode::Backquote => 192,
        KeyCode::BracketLeft => 219,
        KeyCode::Backslash => 220,
        KeyCode::BracketRight => 221,
        KeyCode::Quote => 222,
        KeyCode::IntlBackslash => 226,
        KeyCode::Numpad0 => 96,
        KeyCode::Numpad1 => 97,
        KeyCode::Numpad2 => 98,
        KeyCode::Numpad3 => 99,
        KeyCode::Numpad4 => 100,
        KeyCode::Numpad5 => 101,
        KeyCode::Numpad6 => 102,
        KeyCode::Numpad7 => 103,
        KeyCode::Numpad8 => 104,
        KeyCode::Numpad9 => 105,
        KeyCode::NumpadMultiply => 106,
        KeyCode::NumpadAdd => 107,
        KeyCode::NumpadSubtract => 109,
        KeyCode::NumpadDecimal => 110,
        KeyCode::NumpadDivide => 111,
        _ => 0,
    }
}

/// `event .key` and `event .keyCode` of a native key: the keyCode of the key name (the table the
/// oracle harness uses), else the US-layout keyCode of the physical key.
pub fn dom_event(logical: &Key, physical: &PhysicalKey) -> (String, u32) {
    let key = dom_key(logical);
    let code = match x3d_runtime::nodes::key_sensor::key_code(&key) {
        0 => physical_key_code(physical),
        c => c,
    };

    (key, code)
}

/// Whose shortcut modifier the viewer uses.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Platform {
    /// Ctrl.
    Linux,
    /// Command (winit's Super).
    MacOs,
}

impl Platform {
    pub const fn current() -> Platform {
        if cfg!(target_os = "macos") { Platform::MacOs } else { Platform::Linux }
    }

    /// Only the shortcut modifier is down: Shift, Alt and the other platform's modifier make the
    /// key a key for X3D.
    fn shortcut_modifier(self, m: ModifiersState) -> bool {
        let (primary, other) = match self {
            Platform::Linux => (m.control_key(), m.super_key()),
            Platform::MacOs => (m.super_key(), m.control_key()),
        };

        primary && !other && !m.shift_key() && !m.alt_key()
    }
}

/// An application shortcut of the native viewer.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Shortcut {
    /// Ctrl/Command + 1 … 4: EXAMINE, WALK, FLY, NONE.
    Viewer(&'static str),
    /// Ctrl/Command + Q.
    Quit,
    /// Ctrl/Command + M: cycle multisampling.
    Multisampling,
    /// Ctrl/Command + Z: cycle the depth buffer mode.
    DepthMode,
    /// Ctrl/Command + R.
    Reload,
    /// Ctrl/Command + L: list the viewpoints on stderr.
    ListViewpoints,
    /// Ctrl/Command + S: screenshot to /tmp.
    Screenshot,
}

/// The shortcut of a key press, if any. `key` is the key without modifiers
/// (`key_without_modifiers`). Escape, Home, End, PageUp and PageDown are never shortcuts: they are
/// X3D keys (X_ITE handles the last four itself, for every key device sensor too).
pub fn shortcut(platform: Platform, key: &Key, mods: ModifiersState) -> Option<Shortcut> {
    if !platform.shortcut_modifier(mods) {
        return None;
    }

    let Key::Character(c) = key else { return None };

    Some(match c.as_str() {
        "1" => Shortcut::Viewer("EXAMINE"),
        "2" => Shortcut::Viewer("WALK"),
        "3" => Shortcut::Viewer("FLY"),
        "4" => Shortcut::Viewer("NONE"),
        "q" | "Q" => Shortcut::Quit,
        "m" | "M" => Shortcut::Multisampling,
        "z" | "Z" => Shortcut::DepthMode,
        "r" | "R" => Shortcut::Reload,
        "l" | "L" => Shortcut::ListViewpoints,
        "s" | "S" => Shortcut::Screenshot,
        _ => return None,
    })
}

/// One keyboard event of the window, after the shortcut decision.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum KeyAction {
    /// Run this shortcut (the key goes nowhere else).
    Shortcut(Shortcut),
    /// A DOM keydown / keyup for X3D.
    Dom { down: bool, key: String, code: u32 },
    /// Nothing: the release (or a repeat) of a shortcut key, or a key the window never saw pressed.
    None,
}

/// The keys held down, so that every keydown X3D got gets its keyup, a shortcut key's release
/// stays with the application, and focus loss releases what is still held.
#[derive(Default)]
pub struct KeyTracker {
    /// Keys forwarded to X3D, in press order, with their keydown `event .key` / keyCode.
    pressed: Vec<(PhysicalKey, String, u32)>,
    /// Keys whose press was a shortcut.
    consumed: Vec<PhysicalKey>,
}

impl KeyTracker {
    /// A winit key event. `synthetic`: winit's focus-change events (X11, Windows), which a browser
    /// does not send; their releases count only for keys X3D saw pressed.
    #[allow(clippy::too_many_arguments)]
    pub fn event(&mut self, platform: Platform, pressed: bool, repeat: bool, synthetic: bool, logical: &Key, without_mods: &Key, physical: &PhysicalKey, mods: ModifiersState) -> KeyAction {
        if pressed {
            if synthetic || self.consumed.contains(physical) {
                // A synthetic press on focus gain, or a repeat of a shortcut: the shortcut ran once.
                return KeyAction::None;
            }

            let held = self.pressed.iter().any(|(p, ..)| p == physical);

            if !held && !repeat {
                if let Some(s) = shortcut(platform, without_mods, mods) {
                    self.consumed.push(*physical);
                    return KeyAction::Shortcut(s);
                }
            }

            let (key, code) = dom_event(logical, physical);

            if !held {
                self.pressed.push((*physical, key.clone(), code));
            }

            // Repeats are keydowns, as in the browser.
            return KeyAction::Dom { down: true, key, code };
        }

        if let Some(i) = self.consumed.iter().position(|p| p == physical) {
            self.consumed.remove(i);
            return KeyAction::None;
        }

        match self.pressed.iter().position(|(p, ..)| p == physical) {
            Some(i) => {
                self.pressed.remove(i);
                let (key, code) = dom_event(logical, physical);
                KeyAction::Dom { down: false, key, code }
            }
            // Pressed before the window had focus (or a synthetic release of such a key).
            None if synthetic => KeyAction::None,
            None => {
                let (key, code) = dom_event(logical, physical);
                KeyAction::Dom { down: false, key, code }
            }
        }
    }

    /// Focus lost: a keyup for every key X3D still holds, in press order, with its keydown key.
    pub fn release_all(&mut self) -> Vec<KeyAction> {
        self.consumed.clear();
        self.pressed.drain(..).map(|(_, key, code)| KeyAction::Dom { down: false, key, code }).collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn ch(s: &str) -> Key {
        Key::Character(s.into())
    }

    fn named(n: NamedKey) -> Key {
        Key::Named(n)
    }

    fn code(c: KeyCode) -> PhysicalKey {
        PhysicalKey::Code(c)
    }

    const CTRL: ModifiersState = ModifiersState::CONTROL;
    const CMD: ModifiersState = ModifiersState::SUPER;
    const NONE: ModifiersState = ModifiersState::empty();

    #[test]
    fn letters_digits_punctuation() {
        assert_eq!(dom_event(&ch("a"), &code(KeyCode::KeyA)), ("a".into(), 65));
        assert_eq!(dom_event(&ch("A"), &code(KeyCode::KeyA)), ("A".into(), 65));
        assert_eq!(dom_event(&ch("z"), &code(KeyCode::KeyZ)), ("z".into(), 90));
        assert_eq!(dom_event(&ch("0"), &code(KeyCode::Digit0)), ("0".into(), 48));
        assert_eq!(dom_event(&ch("9"), &code(KeyCode::Digit9)), ("9".into(), 57));
        // US Shift + digit / punctuation: the key is the shifted character, the keyCode the key's.
        assert_eq!(dom_event(&ch("!"), &code(KeyCode::Digit1)), ("!".into(), 49));
        assert_eq!(dom_event(&ch("@"), &code(KeyCode::Digit2)), ("@".into(), 50));
        assert_eq!(dom_event(&ch("?"), &code(KeyCode::Slash)), ("?".into(), 191));
        assert_eq!(dom_event(&ch(":"), &code(KeyCode::Semicolon)), (":".into(), 186));
        assert_eq!(dom_event(&ch("\""), &code(KeyCode::Quote)), ("\"".into(), 222));
        assert_eq!(dom_event(&ch("{"), &code(KeyCode::BracketLeft)), ("{".into(), 219));
        assert_eq!(dom_event(&named(NamedKey::Space), &code(KeyCode::Space)), (" ".into(), 32));
    }

    #[test]
    fn other_layouts() {
        // AZERTY: the A key sits where US has Q; its keyCode follows the letter, not the position.
        assert_eq!(dom_event(&ch("a"), &code(KeyCode::KeyQ)), ("a".into(), 65));
        // A character the keyCode table does not know takes the physical key's US keyCode,
        // so the sensors get the text (German ö on the US ; key, French é on the US 2 key).
        assert_eq!(dom_event(&ch("ö"), &code(KeyCode::Semicolon)), ("ö".into(), 186));
        assert_eq!(dom_event(&ch("é"), &code(KeyCode::Digit2)), ("é".into(), 50));
        // No physical code either: keyCode 0, so no text is made up for it.
        assert_eq!(dom_event(&ch("ö"), &PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified)), ("ö".into(), 0));
    }

    #[test]
    fn named_keys() {
        let cases = [
            (NamedKey::Enter, "Enter", 13),
            (NamedKey::Backspace, "Backspace", 8),
            (NamedKey::Escape, "Escape", 27),
            (NamedKey::Tab, "Tab", 9),
            (NamedKey::ArrowLeft, "ArrowLeft", 37),
            (NamedKey::ArrowUp, "ArrowUp", 38),
            (NamedKey::ArrowRight, "ArrowRight", 39),
            (NamedKey::ArrowDown, "ArrowDown", 40),
            (NamedKey::Shift, "Shift", 16),
            (NamedKey::Control, "Control", 17),
            (NamedKey::Alt, "Alt", 18),
            (NamedKey::Super, "Meta", 91),
            (NamedKey::AltGraph, "AltGraph", 225),
            (NamedKey::Home, "Home", 36),
            (NamedKey::End, "End", 35),
            (NamedKey::PageUp, "PageUp", 33),
            (NamedKey::PageDown, "PageDown", 34),
            (NamedKey::Delete, "Delete", 46),
            (NamedKey::Insert, "Insert", 45),
            (NamedKey::CapsLock, "CapsLock", 20),
        ];

        for (n, key, c) in cases {
            assert_eq!(dom_event(&named(n), &PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified)), (key.to_string(), c), "{n:?}");
        }

        let f = [NamedKey::F1, NamedKey::F2, NamedKey::F3, NamedKey::F4, NamedKey::F5, NamedKey::F6, NamedKey::F7, NamedKey::F8, NamedKey::F9, NamedKey::F10, NamedKey::F11, NamedKey::F12];

        for (i, n) in f.into_iter().enumerate() {
            assert_eq!(dom_event(&named(n), &code(KeyCode::Escape)), (format!("F{}", i + 1), 112 + i as u32));
        }
    }

    #[test]
    fn unknown_keys() {
        let none = PhysicalKey::Unidentified(winit::keyboard::NativeKeyCode::Unidentified);
        assert_eq!(dom_event(&named(NamedKey::MediaPlayPause), &none), ("Unidentified".into(), 0));
        assert_eq!(dom_event(&Key::Unidentified(winit::keyboard::NativeKey::Unidentified), &none), ("Unidentified".into(), 0));
        assert_eq!(dom_event(&Key::Dead(Some('´')), &code(KeyCode::Equal)), ("Dead".into(), 187));
    }

    #[test]
    fn shortcut_modifier_per_platform() {
        assert_eq!(shortcut(Platform::Linux, &ch("q"), CTRL), Some(Shortcut::Quit));
        assert_eq!(shortcut(Platform::Linux, &ch("q"), CMD), None);
        assert_eq!(shortcut(Platform::MacOs, &ch("q"), CMD), Some(Shortcut::Quit));
        assert_eq!(shortcut(Platform::MacOs, &ch("q"), CTRL), None);

        for p in [Platform::Linux, Platform::MacOs] {
            let m = if p == Platform::Linux { CTRL } else { CMD };
            assert_eq!(shortcut(p, &ch("1"), m), Some(Shortcut::Viewer("EXAMINE")));
            assert_eq!(shortcut(p, &ch("2"), m), Some(Shortcut::Viewer("WALK")));
            assert_eq!(shortcut(p, &ch("3"), m), Some(Shortcut::Viewer("FLY")));
            assert_eq!(shortcut(p, &ch("4"), m), Some(Shortcut::Viewer("NONE")));
            assert_eq!(shortcut(p, &ch("m"), m), Some(Shortcut::Multisampling));
            assert_eq!(shortcut(p, &ch("z"), m), Some(Shortcut::DepthMode));
            assert_eq!(shortcut(p, &ch("r"), m), Some(Shortcut::Reload));
            assert_eq!(shortcut(p, &ch("l"), m), Some(Shortcut::ListViewpoints));
            assert_eq!(shortcut(p, &ch("s"), m), Some(Shortcut::Screenshot));
            // Other letters, extra modifiers and the X3D keys are not shortcuts.
            assert_eq!(shortcut(p, &ch("a"), m), None);
            assert_eq!(shortcut(p, &ch("5"), m), None);
            assert_eq!(shortcut(p, &ch("q"), m | ModifiersState::SHIFT), None);
            assert_eq!(shortcut(p, &ch("q"), m | ModifiersState::ALT), None);
            assert_eq!(shortcut(p, &ch("q"), CTRL | CMD), None);

            for n in [NamedKey::Escape, NamedKey::Home, NamedKey::End, NamedKey::PageUp, NamedKey::PageDown] {
                assert_eq!(shortcut(p, &named(n), m), None, "{n:?}");
                assert_eq!(shortcut(p, &named(n), NONE), None, "{n:?}");
            }
        }

        // Plain letters and digits are text for X3D on every platform.
        for k in ["1", "2", "3", "4", "q", "m", "z", "r", "l", "s", "Q", "S"] {
            assert_eq!(shortcut(Platform::Linux, &ch(k), NONE), None);
            assert_eq!(shortcut(Platform::MacOs, &ch(k), NONE), None);
            assert_eq!(shortcut(Platform::Linux, &ch(k), ModifiersState::SHIFT), None);
        }
    }

    fn ev(t: &mut KeyTracker, p: Platform, down: bool, repeat: bool, k: Key, c: KeyCode, mods: ModifiersState) -> KeyAction {
        t.event(p, down, repeat, false, &k, &k, &code(c), mods)
    }

    fn dom(down: bool, key: &str, code: u32) -> KeyAction {
        KeyAction::Dom { down, key: key.into(), code }
    }

    #[test]
    fn tracker_forwards_keys_and_repeats() {
        let mut t = KeyTracker::default();
        let p = Platform::Linux;
        assert_eq!(ev(&mut t, p, true, false, ch("q"), KeyCode::KeyQ, NONE), dom(true, "q", 81));
        assert_eq!(ev(&mut t, p, true, true, ch("q"), KeyCode::KeyQ, NONE), dom(true, "q", 81));
        assert_eq!(ev(&mut t, p, true, true, ch("q"), KeyCode::KeyQ, NONE), dom(true, "q", 81));
        assert_eq!(ev(&mut t, p, false, false, ch("q"), KeyCode::KeyQ, NONE), dom(false, "q", 81));
        assert!(t.pressed.is_empty());
        // Escape is an X3D key.
        assert_eq!(ev(&mut t, p, true, false, named(NamedKey::Escape), KeyCode::Escape, NONE), dom(true, "Escape", 27));
        assert_eq!(ev(&mut t, p, false, false, named(NamedKey::Escape), KeyCode::Escape, NONE), dom(false, "Escape", 27));
        // Home / End / PageUp / PageDown: one keydown each, even with the shortcut modifier.
        assert_eq!(ev(&mut t, p, true, false, named(NamedKey::Home), KeyCode::Home, CTRL), dom(true, "Home", 36));
        assert_eq!(ev(&mut t, p, false, false, named(NamedKey::Home), KeyCode::Home, CTRL), dom(false, "Home", 36));
    }

    #[test]
    fn tracker_shortcuts_consume_press_repeat_and_release() {
        for (p, m) in [(Platform::Linux, CTRL), (Platform::MacOs, CMD)] {
            let mut t = KeyTracker::default();
            let modifier = if p == Platform::Linux { NamedKey::Control } else { NamedKey::Super };
            let (mk, mc) = if p == Platform::Linux { (KeyCode::ControlLeft, 17) } else { (KeyCode::SuperLeft, 91) };
            let mname = if p == Platform::Linux { "Control" } else { "Meta" };
            // The modifier itself reaches X3D (KeySensor controlKey on Linux).
            assert_eq!(ev(&mut t, p, true, false, named(modifier), mk, m), dom(true, mname, mc));
            assert_eq!(ev(&mut t, p, true, false, ch("r"), KeyCode::KeyR, m), KeyAction::Shortcut(Shortcut::Reload));
            assert_eq!(ev(&mut t, p, true, true, ch("r"), KeyCode::KeyR, m), KeyAction::None);
            assert_eq!(ev(&mut t, p, false, false, ch("r"), KeyCode::KeyR, m), KeyAction::None);
            assert_eq!(ev(&mut t, p, false, false, named(modifier), mk, NONE), dom(false, mname, mc));
            assert!(t.pressed.is_empty() && t.consumed.is_empty());
        }
    }

    #[test]
    fn tracker_modifier_order_and_keyup_key() {
        let mut t = KeyTracker::default();
        let p = Platform::Linux;
        let s = ModifiersState::SHIFT;
        assert_eq!(ev(&mut t, p, true, false, named(NamedKey::Shift), KeyCode::ShiftLeft, s), dom(true, "Shift", 16));
        assert_eq!(ev(&mut t, p, true, false, ch("B"), KeyCode::KeyB, s), dom(true, "B", 66));
        // Shift released first: the keyup carries the key as it is now, as in the browser.
        assert_eq!(ev(&mut t, p, false, false, named(NamedKey::Shift), KeyCode::ShiftLeft, NONE), dom(false, "Shift", 16));
        assert_eq!(ev(&mut t, p, false, false, ch("b"), KeyCode::KeyB, NONE), dom(false, "b", 66));
    }

    #[test]
    fn tracker_focus_loss_releases_held_keys_once() {
        let mut t = KeyTracker::default();
        let p = Platform::Linux;
        ev(&mut t, p, true, false, named(NamedKey::Shift), KeyCode::ShiftLeft, ModifiersState::SHIFT);
        ev(&mut t, p, true, false, ch("A"), KeyCode::KeyA, ModifiersState::SHIFT);
        ev(&mut t, p, true, false, named(NamedKey::Control), KeyCode::ControlLeft, ModifiersState::SHIFT | CTRL);
        ev(&mut t, p, true, false, ch("s"), KeyCode::KeyS, CTRL);
        assert_eq!(t.release_all(), vec![dom(false, "Shift", 16), dom(false, "A", 65), dom(false, "Control", 17)]);
        assert!(t.release_all().is_empty());
        // X11's synthetic releases after that, and synthetic presses on focus gain: nothing.
        let k = ch("a");
        assert_eq!(t.event(p, false, false, true, &k, &k, &code(KeyCode::KeyA), NONE), KeyAction::None);
        assert_eq!(t.event(p, true, false, true, &k, &k, &code(KeyCode::KeyA), NONE), KeyAction::None);
        // The shortcut key's real release after focus came back is a plain keyup (X3D never saw a press;
        // KeySensor and StringSensor ignore it unless they are active, as for a key pressed elsewhere).
        assert_eq!(ev(&mut t, p, false, false, ch("s"), KeyCode::KeyS, NONE), dom(false, "s", 83));
    }

    #[test]
    fn tracker_synthetic_release_of_held_key() {
        let mut t = KeyTracker::default();
        let p = Platform::Linux;
        ev(&mut t, p, true, false, ch("x"), KeyCode::KeyX, NONE);
        let k = ch("x");
        assert_eq!(t.event(p, false, false, true, &k, &k, &code(KeyCode::KeyX), NONE), dom(false, "x", 88));
        assert!(t.release_all().is_empty());
    }
}
