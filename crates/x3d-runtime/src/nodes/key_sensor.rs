//! Components/KeyDeviceSensor/{X3DKeyDeviceSensorNode,KeySensor,StringSensor}.js and the browser's
//! X3DKeyDeviceSensorContext: the enabled, live key device sensors get every keydown / keyup of the
//! browser element, in the order they were enabled, and the events are processed right away.

use crate::*;

/// The part of a DOM KeyboardEvent the sensors read.
#[derive(Clone, Debug)]
pub struct KeyEvent {
    /// `event .key`.
    pub key: String,
    /// `event .which` / `event .keyCode` (0 for a key the browser has no code for).
    pub code: u32,
}

/// KeySensor action keys: F1 … F12, HOME, END, PGUP, PGDN, UP, DOWN, LEFT, RIGHT.
fn action_key(code: u32) -> Option<i32> {
    Some(match code {
        112..=123 => (code - 111) as i32,
        36 => 13,
        35 => 14,
        33 => 15,
        34 => 16,
        38 => 17,
        40 => 18,
        37 => 19,
        39 => 20,
        _ => return None,
    })
}

/// KeySensor keyPress / keyRelease text of a key without an action code.
fn key_text(e: &KeyEvent) -> Option<String> {
    // `if (event .charCode || event .keyCode)`: keydown / keyup have no charCode.
    if e.code == 0 {
        return None;
    }

    match e.key.as_str() {
        "AltGraph" | "CapsLock" | "Insert" => None,
        "Backspace" => Some("\u{8}".into()),
        "Delete" => Some("\u{7f}".into()),
        "Enter" => Some("\n".into()),
        "Escape" => Some("\u{1b}".into()),
        "Tab" => Some("\t".into()),
        k if k.chars().count() == 1 => Some(k.to_string()),
        _ => None,
    }
}

impl Runtime {
    fn ks_set(&mut self, node: NodeId, name: &str, value: FieldValue) {
        if let Some(k) = self.pre(node, name) {
            self.set_value(k, value);
        }
    }

    /// X3DKeyDeviceSensorNode.initialize: `getLive () .addInterest ("set_live__")`, `set_live__ ()`.
    pub(crate) fn key_sensor_initialize(&mut self, node: NodeId) {
        self.watch_live(node);

        if self.time_node_live(node) {
            self.key_set_live(node, true);
        }
    }

    /// set_live__.
    pub(crate) fn key_set_live(&mut self, node: NodeId, live: bool) {
        if live {
            self.interest(node, "enabled", Cb::KeySensorSetEnabled);

            if self.get_bool(node, "enabled") {
                self.key_enable(node);
            }
        } else {
            if let Some(k) = self.pre(node, "enabled") {
                self.remove_interest(k, node, Cb::KeySensorSetEnabled);
            }

            self.key_disable(node);
        }
    }

    /// set_enabled__.
    pub(crate) fn key_set_enabled(&mut self, node: NodeId) {
        if self.get_bool(node, "enabled") {
            self.key_enable(node);
        } else {
            self.key_disable(node);
        }
    }

    /// enable (): only one key device sensor may be active at a time.
    fn key_enable(&mut self, node: NodeId) {
        if !self.key_sensors.contains(&node) {
            self.key_sensors.push(node);
        }

        for other in self.key_sensors.clone() {
            if other == node || !self.get_bool(other, "enabled") {
                continue;
            }

            self.ks_set(other, "enabled", FieldValue::SFBool(false));
        }
    }

    /// disable ().
    fn key_disable(&mut self, node: NodeId) {
        self.key_sensors.retain(|&n| n != node);

        // release (): KeySensor only.
        if self.builtin_name(node) == Some("KeySensor") {
            for name in ["shiftKey", "controlKey", "altKey", "isActive"] {
                if self.get_bool(node, name) {
                    self.ks_set(node, name, FieldValue::SFBool(false));
                }
            }
        }
    }

    /// X3DKeyDeviceSensorContext [_keydown] / [_keyup]: every registered sensor, then processEvents.
    pub fn key_event(&mut self, down: bool, e: &KeyEvent) {
        for node in self.key_sensors.clone() {
            match (self.builtin_name(node), down) {
                (Some("KeySensor"), true) => self.key_sensor_keydown(node, e),
                (Some("KeySensor"), false) => self.key_sensor_keyup(node, e),
                (Some("StringSensor"), true) => self.string_sensor_keydown(node, e),
                _ => {}
            }
        }

        self.process_events();
    }

    fn key_sensor_keydown(&mut self, node: NodeId, e: &KeyEvent) {
        if !self.get_bool(node, "isActive") {
            self.ks_set(node, "isActive", FieldValue::SFBool(true));
        }

        match e.code {
            16 => self.ks_set(node, "shiftKey", FieldValue::SFBool(true)),
            17 => self.ks_set(node, "controlKey", FieldValue::SFBool(true)),
            18 => self.ks_set(node, "altKey", FieldValue::SFBool(true)),
            c => match action_key(c) {
                Some(a) => self.ks_set(node, "actionKeyPress", FieldValue::SFInt32(a)),
                None => {
                    if let Some(t) = key_text(e) {
                        self.ks_set(node, "keyPress", FieldValue::SFString(t));
                    }
                }
            },
        }
    }

    fn key_sensor_keyup(&mut self, node: NodeId, e: &KeyEvent) {
        match e.code {
            16 => self.ks_set(node, "shiftKey", FieldValue::SFBool(false)),
            17 => self.ks_set(node, "controlKey", FieldValue::SFBool(false)),
            18 => self.ks_set(node, "altKey", FieldValue::SFBool(false)),
            c => match action_key(c) {
                Some(a) => self.ks_set(node, "actionKeyRelease", FieldValue::SFInt32(a)),
                None => {
                    if let Some(t) = key_text(e) {
                        self.ks_set(node, "keyRelease", FieldValue::SFString(t));
                    }
                }
            },
        }

        if self.get_bool(node, "isActive") {
            self.ks_set(node, "isActive", FieldValue::SFBool(false));
        }
    }

    fn string_sensor_keydown(&mut self, node: NodeId, e: &KeyEvent) {
        let entered = match self.pre_value(node, "enteredText") {
            Some(FieldValue::SFString(s)) => s.clone(),
            _ => String::new(),
        };
        let active = self.get_bool(node, "isActive");

        match e.key.as_str() {
            "Backspace" => {
                // `substring (0, length - 1)` on UTF-16 code units.
                if active && self.get_bool(node, "deletionAllowed") && !entered.is_empty() {
                    let mut units: Vec<u16> = entered.encode_utf16().collect();
                    units.pop();
                    self.ks_set(node, "enteredText", FieldValue::SFString(String::from_utf16_lossy(&units)));
                }
            }
            "Enter" => {
                self.ks_set(node, "finalText", FieldValue::SFString(entered));

                // `this ._enteredText .set ("")`: no event.
                if let Some(k) = self.pre(node, "enteredText") {
                    self.set_raw(k, FieldValue::SFString(String::new()));
                }

                if active {
                    self.ks_set(node, "isActive", FieldValue::SFBool(false));
                }
            }
            "Escape" => {
                if let Some(k) = self.pre(node, "enteredText") {
                    self.set_raw(k, FieldValue::SFString(String::new()));
                }

                if active {
                    self.ks_set(node, "isActive", FieldValue::SFBool(false));
                }
            }
            "Tab" => {}
            k => {
                if e.code != 0 && k.chars().count() == 1 {
                    let mut text = entered;

                    if !active {
                        self.ks_set(node, "isActive", FieldValue::SFBool(true));
                        self.ks_set(node, "enteredText", FieldValue::SFString(String::new()));
                        text = String::new();
                    }

                    text.push_str(k);
                    self.ks_set(node, "enteredText", FieldValue::SFString(text));
                }
            }
        }
    }
}

/// `event .keyCode` of a key name as Chromium reports it for Playwright's US keyboard layout.
pub fn key_code(key: &str) -> u32 {
    let mut chars = key.chars();

    if let (Some(c), None) = (chars.next(), chars.next()) {
        return match c {
            'a'..='z' => c.to_ascii_uppercase() as u32,
            'A'..='Z' | '0'..='9' => c as u32,
            ' ' => 32,
            ';' | ':' => 186,
            '=' | '+' => 187,
            ',' | '<' => 188,
            '-' | '_' => 189,
            '.' | '>' => 190,
            '/' | '?' => 191,
            '`' | '~' => 192,
            '[' | '{' => 219,
            '\\' | '|' => 220,
            ']' | '}' => 221,
            '\'' | '"' => 222,
            ')' => 48,
            '!' => 49,
            '@' => 50,
            '#' => 51,
            '$' => 52,
            '%' => 53,
            '^' => 54,
            '&' => 55,
            '*' => 56,
            '(' => 57,
            _ => 0,
        };
    }

    match key {
        "Backspace" => 8,
        "Tab" => 9,
        "Enter" => 13,
        "Shift" => 16,
        "Control" => 17,
        "Alt" => 18,
        "Pause" => 19,
        "CapsLock" => 20,
        "Escape" => 27,
        "PageUp" => 33,
        "PageDown" => 34,
        "End" => 35,
        "Home" => 36,
        "ArrowLeft" => 37,
        "ArrowUp" => 38,
        "ArrowRight" => 39,
        "ArrowDown" => 40,
        "Insert" => 45,
        "Delete" => 46,
        "Meta" => 91,
        "AltGraph" => 225,
        f if f.len() >= 2 && f.starts_with('F') => match f[1..].parse::<u32>() {
            Ok(n @ 1..=24) => 111 + n,
            _ => 0,
        },
        _ => 0,
    }
}
