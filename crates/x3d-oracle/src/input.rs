//! Playwright's page.mouse / page.keyboard for Chromium, message for message (playwright-core 1.62:
//! server/input.ts, server/usKeyboardLayout.ts, server/chromium/crInput.ts). The Node tools' input events
//! become exactly these `Input.*` commands, so the browser sees the same event stream.

use serde_json::{Value, json};
use std::collections::{BTreeSet, HashMap};

#[derive(Clone, Debug, PartialEq)]
pub struct KeyDescription {
    pub key: String,
    pub key_code: i64,
    pub key_code_without_location: i64,
    pub code: String,
    pub text: String,
    pub location: i64,
    pub shifted: Option<Box<KeyDescription>>,
}

// code, keyCode, keyCodeWithoutLocation (0 = keyCode), key, shiftKey, shiftKeyCode, text, location
type Def = (
    &'static str,
    i64,
    i64,
    &'static str,
    &'static str,
    i64,
    &'static str,
    i64,
);

#[rustfmt::skip]
const US_LAYOUT: &[Def] = &[
    ("Escape", 27, 0, "Escape", "", 0, "", 0),
    ("F1", 112, 0, "F1", "", 0, "", 0), ("F2", 113, 0, "F2", "", 0, "", 0), ("F3", 114, 0, "F3", "", 0, "", 0),
    ("F4", 115, 0, "F4", "", 0, "", 0), ("F5", 116, 0, "F5", "", 0, "", 0), ("F6", 117, 0, "F6", "", 0, "", 0),
    ("F7", 118, 0, "F7", "", 0, "", 0), ("F8", 119, 0, "F8", "", 0, "", 0), ("F9", 120, 0, "F9", "", 0, "", 0),
    ("F10", 121, 0, "F10", "", 0, "", 0), ("F11", 122, 0, "F11", "", 0, "", 0), ("F12", 123, 0, "F12", "", 0, "", 0),
    ("Backquote", 192, 0, "`", "~", 0, "", 0),
    ("Digit1", 49, 0, "1", "!", 0, "", 0), ("Digit2", 50, 0, "2", "@", 0, "", 0), ("Digit3", 51, 0, "3", "#", 0, "", 0),
    ("Digit4", 52, 0, "4", "$", 0, "", 0), ("Digit5", 53, 0, "5", "%", 0, "", 0), ("Digit6", 54, 0, "6", "^", 0, "", 0),
    ("Digit7", 55, 0, "7", "&", 0, "", 0), ("Digit8", 56, 0, "8", "*", 0, "", 0), ("Digit9", 57, 0, "9", "(", 0, "", 0),
    ("Digit0", 48, 0, "0", ")", 0, "", 0),
    ("Minus", 189, 0, "-", "_", 0, "", 0), ("Equal", 187, 0, "=", "+", 0, "", 0), ("Backslash", 220, 0, "\\", "|", 0, "", 0),
    ("Backspace", 8, 0, "Backspace", "", 0, "", 0),
    ("Tab", 9, 0, "Tab", "", 0, "", 0),
    ("KeyQ", 81, 0, "q", "Q", 0, "", 0), ("KeyW", 87, 0, "w", "W", 0, "", 0), ("KeyE", 69, 0, "e", "E", 0, "", 0),
    ("KeyR", 82, 0, "r", "R", 0, "", 0), ("KeyT", 84, 0, "t", "T", 0, "", 0), ("KeyY", 89, 0, "y", "Y", 0, "", 0),
    ("KeyU", 85, 0, "u", "U", 0, "", 0), ("KeyI", 73, 0, "i", "I", 0, "", 0), ("KeyO", 79, 0, "o", "O", 0, "", 0),
    ("KeyP", 80, 0, "p", "P", 0, "", 0), ("BracketLeft", 219, 0, "[", "{", 0, "", 0), ("BracketRight", 221, 0, "]", "}", 0, "", 0),
    ("CapsLock", 20, 0, "CapsLock", "", 0, "", 0),
    ("KeyA", 65, 0, "a", "A", 0, "", 0), ("KeyS", 83, 0, "s", "S", 0, "", 0), ("KeyD", 68, 0, "d", "D", 0, "", 0),
    ("KeyF", 70, 0, "f", "F", 0, "", 0), ("KeyG", 71, 0, "g", "G", 0, "", 0), ("KeyH", 72, 0, "h", "H", 0, "", 0),
    ("KeyJ", 74, 0, "j", "J", 0, "", 0), ("KeyK", 75, 0, "k", "K", 0, "", 0), ("KeyL", 76, 0, "l", "L", 0, "", 0),
    ("Semicolon", 186, 0, ";", ":", 0, "", 0), ("Quote", 222, 0, "'", "\"", 0, "", 0),
    ("Enter", 13, 0, "Enter", "", 0, "\r", 0),
    ("ShiftLeft", 160, 16, "Shift", "", 0, "", 1),
    ("KeyZ", 90, 0, "z", "Z", 0, "", 0), ("KeyX", 88, 0, "x", "X", 0, "", 0), ("KeyC", 67, 0, "c", "C", 0, "", 0),
    ("KeyV", 86, 0, "v", "V", 0, "", 0), ("KeyB", 66, 0, "b", "B", 0, "", 0), ("KeyN", 78, 0, "n", "N", 0, "", 0),
    ("KeyM", 77, 0, "m", "M", 0, "", 0), ("Comma", 188, 0, ",", "<", 0, "", 0), ("Period", 190, 0, ".", ">", 0, "", 0),
    ("Slash", 191, 0, "/", "?", 0, "", 0),
    ("ShiftRight", 161, 16, "Shift", "", 0, "", 2),
    ("ControlLeft", 162, 17, "Control", "", 0, "", 1),
    ("MetaLeft", 91, 0, "Meta", "", 0, "", 1),
    ("AltLeft", 164, 18, "Alt", "", 0, "", 1),
    ("Space", 32, 0, " ", "", 0, "", 0),
    ("AltRight", 165, 18, "Alt", "", 0, "", 2),
    ("AltGraph", 225, 0, "AltGraph", "", 0, "", 0),
    ("MetaRight", 92, 0, "Meta", "", 0, "", 2),
    ("ContextMenu", 93, 0, "ContextMenu", "", 0, "", 0),
    ("ControlRight", 163, 17, "Control", "", 0, "", 2),
    ("PrintScreen", 44, 0, "PrintScreen", "", 0, "", 0), ("ScrollLock", 145, 0, "ScrollLock", "", 0, "", 0),
    ("Pause", 19, 0, "Pause", "", 0, "", 0), ("PageUp", 33, 0, "PageUp", "", 0, "", 0), ("PageDown", 34, 0, "PageDown", "", 0, "", 0),
    ("Insert", 45, 0, "Insert", "", 0, "", 0), ("Delete", 46, 0, "Delete", "", 0, "", 0), ("Home", 36, 0, "Home", "", 0, "", 0),
    ("End", 35, 0, "End", "", 0, "", 0), ("ArrowLeft", 37, 0, "ArrowLeft", "", 0, "", 0), ("ArrowUp", 38, 0, "ArrowUp", "", 0, "", 0),
    ("ArrowRight", 39, 0, "ArrowRight", "", 0, "", 0), ("ArrowDown", 40, 0, "ArrowDown", "", 0, "", 0),
    ("AudioVolumeMute", 173, 0, "AudioVolumeMute", "", 0, "", 0), ("AudioVolumeDown", 174, 0, "AudioVolumeDown", "", 0, "", 0),
    ("AudioVolumeUp", 175, 0, "AudioVolumeUp", "", 0, "", 0), ("MediaTrackNext", 176, 0, "MediaTrackNext", "", 0, "", 0),
    ("MediaTrackPrevious", 177, 0, "MediaTrackPrevious", "", 0, "", 0), ("MediaPlayPause", 179, 0, "MediaPlayPause", "", 0, "", 0),
    ("NumLock", 144, 0, "NumLock", "", 0, "", 0),
    ("NumpadDivide", 111, 0, "/", "", 0, "", 3), ("NumpadMultiply", 106, 0, "*", "", 0, "", 3), ("NumpadSubtract", 109, 0, "-", "", 0, "", 3),
    ("Numpad7", 36, 0, "Home", "7", 103, "", 3), ("Numpad8", 38, 0, "ArrowUp", "8", 104, "", 3), ("Numpad9", 33, 0, "PageUp", "9", 105, "", 3),
    ("Numpad4", 37, 0, "ArrowLeft", "4", 100, "", 3), ("Numpad5", 12, 0, "Clear", "5", 101, "", 3), ("Numpad6", 39, 0, "ArrowRight", "6", 102, "", 3),
    ("NumpadAdd", 107, 0, "+", "", 0, "", 3),
    ("Numpad1", 35, 0, "End", "1", 97, "", 3), ("Numpad2", 40, 0, "ArrowDown", "2", 98, "", 3), ("Numpad3", 34, 0, "PageDown", "3", 99, "", 3),
    ("Numpad0", 45, 0, "Insert", "0", 96, "", 3), ("NumpadDecimal", 46, 0, "\0", ".", 110, "", 3),
    ("NumpadEnter", 13, 0, "Enter", "", 0, "\r", 3),
];

const ALIASES: &[(&str, &[&str])] = &[
    ("ShiftLeft", &["Shift"]),
    ("ControlLeft", &["Control"]),
    ("AltLeft", &["Alt"]),
    ("MetaLeft", &["Meta"]),
    ("Enter", &["\n", "\r"]),
];

const KEYPAD_LOCATION: i64 = 3;
const MODIFIERS: [&str; 4] = ["Alt", "Control", "Meta", "Shift"];

/// `buildLayoutClosure (USKeyboardLayout)`.
pub fn us_layout() -> HashMap<String, KeyDescription> {
    let mut map = HashMap::new();
    for &(code, key_code, kcwl, key, shift_key, shift_key_code, text, location) in US_LAYOUT {
        let mut description = KeyDescription {
            key: key.into(),
            key_code,
            key_code_without_location: if kcwl != 0 { kcwl } else { key_code },
            code: code.into(),
            text: text.into(),
            location,
            shifted: None,
        };
        if key.chars().count() == 1 {
            description.text = key.into();
        }
        let shifted = (!shift_key.is_empty()).then(|| {
            let mut s = description.clone();
            s.key = shift_key.into();
            s.text = shift_key.into();
            if shift_key_code != 0 {
                s.key_code = shift_key_code;
            }
            s
        });
        map.insert(
            code.to_string(),
            KeyDescription {
                shifted: shifted.clone().map(Box::new),
                ..description.clone()
            },
        );
        if let Some((_, aliases)) = ALIASES.iter().find(|(c, _)| *c == code) {
            for a in *aliases {
                map.insert(a.to_string(), description.clone());
            }
        }
        if location != 0 {
            continue;
        }
        if description.key.chars().count() == 1 {
            map.insert(description.key.clone(), description.clone());
        }
        if let Some(s) = shifted {
            map.insert(s.key.clone(), s);
        }
    }
    map
}

fn modifiers_mask(m: &BTreeSet<String>) -> i64 {
    let mut mask = 0;
    if m.contains("Alt") {
        mask |= 1;
    }
    if m.contains("Control") {
        mask |= 2;
    }
    if m.contains("Meta") {
        mask |= 4;
    }
    if m.contains("Shift") {
        mask |= 8;
    }
    mask
}

fn buttons_mask(b: &BTreeSet<String>) -> i64 {
    let mut mask = 0;
    if b.contains("left") {
        mask |= 1;
    }
    if b.contains("right") {
        mask |= 2;
    }
    if b.contains("middle") {
        mask |= 4;
    }
    mask
}

/// One CDP command, or the drag interception toggle around it.
#[derive(Debug, Clone, PartialEq)]
pub struct Cmd {
    pub method: &'static str,
    pub params: Value,
}

fn cmd(method: &'static str, params: Value) -> Cmd {
    Cmd { method, params }
}

/// Mouse and keyboard state of one Playwright page.
pub struct Input {
    layout: HashMap<String, KeyDescription>,
    pressed_modifiers: BTreeSet<String>,
    pressed_keys: BTreeSet<String>,
    x: f64,
    y: f64,
    last_button: String,
    buttons: BTreeSet<String>,
}

impl Default for Input {
    fn default() -> Self {
        Input {
            layout: us_layout(),
            pressed_modifiers: BTreeSet::new(),
            pressed_keys: BTreeSet::new(),
            x: 0.0,
            y: 0.0,
            last_button: "none".into(),
            buttons: BTreeSet::new(),
        }
    }
}

impl Input {
    fn describe(&self, key: &str) -> Result<KeyDescription, String> {
        let d = self
            .layout
            .get(key)
            .ok_or_else(|| format!("Unknown key: \"{key}\""))?;
        let shift = self.pressed_modifiers.contains("Shift");
        let d = match (&d.shifted, shift) {
            (Some(s), true) => (**s).clone(),
            _ => d.clone(),
        };
        if self.pressed_modifiers.len() > 1
            || (!self.pressed_modifiers.contains("Shift") && self.pressed_modifiers.len() == 1)
        {
            return Ok(KeyDescription {
                text: String::new(),
                ..d
            });
        }
        Ok(d)
    }

    /// `keyboard.down (key)`.
    pub fn key_down(&mut self, key: &str) -> Result<Vec<Cmd>, String> {
        let d = self.describe(key)?;
        let auto_repeat = self.pressed_keys.contains(&d.code);
        self.pressed_keys.insert(d.code.clone());
        if MODIFIERS.contains(&d.key.as_str()) {
            self.pressed_modifiers.insert(d.key.clone());
        }
        Ok(vec![cmd(
            "Input.dispatchKeyEvent",
            json!({
                "type": if d.text.is_empty() { "rawKeyDown" } else { "keyDown" },
                "modifiers": modifiers_mask(&self.pressed_modifiers),
                "windowsVirtualKeyCode": d.key_code_without_location,
                "code": d.code,
                "commands": [],
                "key": d.key,
                "text": d.text,
                "unmodifiedText": d.text,
                "autoRepeat": auto_repeat,
                "location": d.location,
                "isKeypad": d.location == KEYPAD_LOCATION,
            }),
        )])
    }

    /// `keyboard.up (key)`.
    pub fn key_up(&mut self, key: &str) -> Result<Vec<Cmd>, String> {
        let d = self.describe(key)?;
        if MODIFIERS.contains(&d.key.as_str()) {
            self.pressed_modifiers.remove(&d.key);
        }
        self.pressed_keys.remove(&d.code);
        Ok(vec![cmd(
            "Input.dispatchKeyEvent",
            json!({
                "type": "keyUp",
                "modifiers": modifiers_mask(&self.pressed_modifiers),
                "key": d.key,
                "windowsVirtualKeyCode": d.key_code_without_location,
                "code": d.code,
                "location": d.location,
            }),
        )])
    }

    /// `keyboard.press (key)`: "Shift+a" style chords are split on '+' like Playwright.
    pub fn key_press(&mut self, key: &str) -> Result<Vec<Cmd>, String> {
        let mut tokens = Vec::new();
        let mut building = String::new();
        for c in key.chars() {
            if c == '+' && !building.is_empty() {
                tokens.push(std::mem::take(&mut building));
            } else {
                building.push(c);
            }
        }
        tokens.push(building);
        let last = tokens.pop().unwrap();
        let mut out = Vec::new();
        for t in &tokens {
            out.extend(self.key_down(t)?);
        }
        out.extend(self.key_down(&last)?);
        out.extend(self.key_up(&last)?);
        for t in tokens.iter().rev() {
            out.extend(self.key_up(t)?);
        }
        Ok(out)
    }

    fn raw_move(&self, x: f64, y: f64, for_click: bool) -> Vec<Cmd> {
        let mv = cmd(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseMoved",
                "button": self.last_button,
                "buttons": buttons_mask(&self.buttons),
                "x": x,
                "y": y,
                "modifiers": modifiers_mask(&self.pressed_modifiers),
                "force": if self.buttons.is_empty() { 0.0 } else { 0.5 },
            }),
        );
        // DragManager.interceptDragCausedByMove: a move with the left button down is wrapped in
        // Input.setInterceptDrags (no drag starts on the canvas, so interception is switched off again).
        if !for_click && self.last_button == "left" {
            return vec![
                cmd("Input.setInterceptDrags", json!({ "enabled": true })),
                mv,
                cmd("Input.setInterceptDrags", json!({ "enabled": false })),
            ];
        }
        vec![mv]
    }

    /// `mouse.move (x, y)` (steps 1).
    pub fn mouse_move(&mut self, x: f64, y: f64) -> Vec<Cmd> {
        self.x = x;
        self.y = y;
        self.raw_move(x, y, false)
    }

    /// `mouse.down ({ button, clickCount })`.
    pub fn mouse_down(&mut self, button: &str, click_count: i64) -> Vec<Cmd> {
        self.last_button = button.into();
        self.buttons.insert(button.into());
        vec![cmd(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mousePressed",
                "button": self.last_button,
                "buttons": buttons_mask(&self.buttons),
                "x": self.x,
                "y": self.y,
                "modifiers": modifiers_mask(&self.pressed_modifiers),
                "clickCount": click_count,
                "force": if self.buttons.is_empty() { 0.0 } else { 0.5 },
            }),
        )]
    }

    /// `mouse.up ({ button, clickCount })`.
    pub fn mouse_up(&mut self, button: &str, click_count: i64) -> Vec<Cmd> {
        self.last_button = "none".into();
        self.buttons.remove(button);
        vec![cmd(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseReleased",
                "button": button,
                "buttons": buttons_mask(&self.buttons),
                "x": self.x,
                "y": self.y,
                "modifiers": modifiers_mask(&self.pressed_modifiers),
                "clickCount": click_count,
            }),
        )]
    }

    /// `mouse.wheel (deltaX, deltaY)`.
    pub fn mouse_wheel(&self, dx: f64, dy: f64) -> Vec<Cmd> {
        vec![cmd(
            "Input.dispatchMouseEvent",
            json!({
                "type": "mouseWheel",
                "x": self.x,
                "y": self.y,
                "modifiers": modifiers_mask(&self.pressed_modifiers),
                "deltaX": dx,
                "deltaY": dy,
            }),
        )]
    }

    /// `mouse.dblclick (x, y)` = click with clickCount 2: move (forClick), then down/up with clickCount 1 and 2.
    /// Playwright sends these without waiting in between (Promise.all); the caller pipelines them too.
    pub fn mouse_dblclick(&mut self, x: f64, y: f64) -> Vec<Cmd> {
        self.x = x;
        self.y = y;
        let mut out = self.raw_move(x, y, true);
        for cc in 1..=2 {
            out.extend(self.mouse_down("left", cc));
            out.extend(self.mouse_up("left", cc));
        }
        out
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_z_matches_playwright_messages() {
        let mut i = Input::default();
        let c = i.key_press("z").unwrap();
        assert_eq!(
            serde_json::to_string(&c[0].params).unwrap(),
            r#"{"autoRepeat":false,"code":"KeyZ","commands":[],"isKeypad":false,"key":"z","location":0,"modifiers":0,"text":"z","type":"keyDown","unmodifiedText":"z","windowsVirtualKeyCode":90}"#
        );
        assert_eq!(c[1].params["type"], "keyUp");
    }

    #[test]
    fn modifiers_and_shift() {
        let mut i = Input::default();
        i.key_down("Shift").unwrap();
        let c = i.key_press("KeyA").unwrap();
        assert_eq!(c[0].params["key"], "A");
        assert_eq!(c[0].params["modifiers"], 8);
        // The key "a" (not the code) has no shifted description in Playwright's layout.
        assert_eq!(i.key_press("a").unwrap()[0].params["text"], "a");
        i.key_down("Control").unwrap();
        let c = i.key_press("b").unwrap();
        assert_eq!(c[0].params["type"], "rawKeyDown");
        assert_eq!(c[0].params["text"], "");
        assert_eq!(c[0].params["modifiers"], 10);
        let c = i.key_press("PageDown").unwrap();
        assert_eq!(c[0].params["windowsVirtualKeyCode"], 34);
        assert_eq!(
            i.key_down("Shift").unwrap()[0].params["windowsVirtualKeyCode"],
            16
        );
        assert!(i.key_press("Nope").is_err());
        assert_eq!(i.key_press("Enter").unwrap()[0].params["code"], "Enter");
    }

    #[test]
    fn drag_interception_wraps_left_button_moves() {
        let mut i = Input::default();
        assert_eq!(i.mouse_move(1.0, 2.0).len(), 1);
        i.mouse_down("left", 1);
        let m = i.mouse_move(3.0, 4.0);
        assert_eq!(m.len(), 3);
        assert_eq!(m[1].params["buttons"], 1);
        assert_eq!(m[1].params["force"], 0.5);
        let u = i.mouse_up("left", 1);
        assert_eq!(u[0].params["buttons"], 0);
        i.mouse_down("right", 1);
        assert_eq!(i.mouse_move(5.0, 6.0).len(), 1);
        assert_eq!(i.mouse_dblclick(7.0, 8.0).len(), 5);
    }
}
