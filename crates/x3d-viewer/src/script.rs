//! Scripted input shared by `x3d-drive` (headless) and `x3d-view --script` (window smoke test).
//!
//! The same JSON event list drives the X_ITE oracle (oracle/x_ite-anim.mjs), so both sides see
//! identical input. Event types: mousedown, mouseup, mousemove, wheel (deltaY), key (Home, End,
//! PageUp, PageDown), mods (shift, ctrl, alt), viewer (name). Pointer coordinates are pixels with
//! the origin at the bottom left of the canvas.

use crate::viewer::{Key, UiEvent, Viewer3d};
use serde_json::Value;

pub fn apply(v: &mut Viewer3d, e: &Value) {
    let f = |k: &str| e.get(k).and_then(Value::as_f64).unwrap_or(0.0);
    let b = |k: &str| e.get(k).and_then(Value::as_bool).unwrap_or(false);

    match e["type"].as_str().unwrap_or("") {
        "mousedown" => v.input(UiEvent::MouseDown { button: f("button") as u8, x: f("x"), y: f("y") }),
        "mouseup" => v.input(UiEvent::MouseUp { button: f("button") as u8, x: f("x"), y: f("y") }),
        "mousemove" => v.input(UiEvent::MouseMove { x: f("x"), y: f("y") }),
        "wheel" => v.input(UiEvent::Wheel { delta_y: f("deltaY"), x: f("x"), y: f("y") }),
        "mods" => v.input(UiEvent::Mods { shift: b("shift"), ctrl: b("ctrl"), alt: b("alt") }),
        "key" => {
            let k = match e["key"].as_str().unwrap_or("") {
                "Home" => Key::Home,
                "End" => Key::End,
                "PageUp" => Key::PageUp,
                _ => Key::PageDown,
            };
            v.input(UiEvent::Key(k));
        }
        "viewer" => v.set_viewer(e["name"].as_str().unwrap_or("EXAMINE")),
        other => eprintln!("unknown event type {other}"),
    }
}
