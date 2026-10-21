//! Scripted input shared by `x3d-drive` (headless) and `x3d-view --script` (window smoke test).
//!
//! The same JSON event list drives the X_ITE oracle (oracle/x_ite-anim.mjs), so both sides see
//! identical input. Event types: mousedown, mouseup, mousemove, wheel (deltaY), dblclick, key (a press:
//! keydown + keyup of a Playwright key name), keydown, keyup, mods (shift, ctrl, alt), viewer (name). Pointer coordinates are pixels with
//! the origin at the bottom left of the canvas.
//!
//! X_ITE reads the pointer from MouseEvent pageX / pageY (getPointerFromEvent), and Chrome gives a mouse event whole CSS
//! pixels, cut toward zero (`page .mouse .move (132.593, 181.836)` fires mousemove with pageX 132, pageY 181; only
//! PointerEvent keeps the fraction). A fractional event position therefore reaches X_ITE as (floor (x), H - floor (H - y))
//! with y measured from the bottom, and the viewer gets the same.

use crate::viewer::{UiEvent, Viewer3d};
use serde_json::Value;

pub fn apply(v: &mut Viewer3d, e: &Value) {
    let height = v.height as f64;
    let f = |k: &str| {
        let value = e.get(k).and_then(Value::as_f64).unwrap_or(0.0);

        // The whole page pixel of a Chrome mouse event (the canvas is at the page origin).
        match k {
            "x" => value.floor(),
            "y" => height - (height - value).floor(),
            _ => value,
        }
    };
    let b = |k: &str| e.get(k).and_then(Value::as_bool).unwrap_or(false);

    match e["type"].as_str().unwrap_or("") {
        // The oracle moves the mouse to (x, y) before every press, release and wheel turn
        // (`page .mouse .move`), and Chrome fires that "mousemove" even when the pointer is
        // already there.
        "mousedown" => {
            v.input(UiEvent::MouseMove { x: f("x"), y: f("y") });
            v.input(UiEvent::MouseDown { button: f("button") as u8, x: f("x"), y: f("y") })
        }
        "mouseup" => {
            v.input(UiEvent::MouseMove { x: f("x"), y: f("y") });
            v.input(UiEvent::MouseUp { button: f("button") as u8, x: f("x"), y: f("y") })
        }
        "mousemove" => v.input(UiEvent::MouseMove { x: f("x"), y: f("y") }),
        // REA batch 10: the canvas changes size (the oracle changes the canvas element's CSS size; reshape () follows).
        "resize" => v.resize(f("width") as u32, f("height") as u32),
        "wheel" => {
            v.input(UiEvent::MouseMove { x: f("x"), y: f("y") });
            v.input(UiEvent::Wheel { delta_y: f("deltaY"), x: f("x"), y: f("y") })
        }
        // Playwright's mouse.dblclick: move, down / up (clickCount 1), down / up (clickCount 2), then the
        // browser fires "dblclick".
        "dblclick" => {
            let (x, y) = (f("x"), f("y"));
            v.input(UiEvent::MouseMove { x, y });
            v.input(UiEvent::MouseDown { button: 0, x, y });
            v.input(UiEvent::MouseUp { button: 0, x, y });
            v.input(UiEvent::MouseDown { button: 0, x, y });
            v.input(UiEvent::MouseUp { button: 0, x, y });
            v.input(UiEvent::DblClick { x, y });
        }
        "mods" => v.input(UiEvent::Mods { shift: b("shift"), ctrl: b("ctrl"), alt: b("alt") }),
        // Playwright keyboard.press: keydown, then keyup.
        "key" => {
            let key = e["key"].as_str().unwrap_or("").to_string();
            v.input(UiEvent::KeyDown { key: key.clone() });
            v.input(UiEvent::KeyUp { key });
        }
        "keydown" => v.input(UiEvent::KeyDown { key: e["key"].as_str().unwrap_or("").to_string() }),
        "keyup" => v.input(UiEvent::KeyUp { key: e["key"].as_str().unwrap_or("").to_string() }),
        "viewer" => v.set_viewer(e["name"].as_str().unwrap_or("EXAMINE")),
        other => eprintln!("unknown event type {other}"),
    }
}
