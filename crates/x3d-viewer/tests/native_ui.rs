//! The native browser UI without a window: winit-style events through `Viewer3d::input` (as `window.rs` sends
//! them) into the context menu, the Notification, and the browser's own keys of X3DCoreContext. The expected
//! entries, texts, descriptions and closing rules follow ContextMenu.js, Notification.js and X3DCoreContext.js of
//! the pinned X_ITE 16.5.0; the Shift+F8 texts are the ones the pinned X_ITE put on the clipboard
//! (oracle/web-browser-ui-01-baseline, case `shiftF8`).
#![cfg(feature = "gpu")]

use std::path::PathBuf;
use winit::keyboard::{Key, KeyCode, ModifiersState, NamedKey, PhysicalKey};
use x3d_viewer::keys::{KeyAction, KeyTracker, Platform, Shortcut, shortcut_in};
use x3d_viewer::ui::UiRequest;
use x3d_viewer::viewer::{UiEvent, Viewer3d};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;
const SIZE: u32 = 512;

fn world(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("x3d-viewer-native-ui-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

fn big<F: FnOnce() + Send + 'static>(f: F) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

/// A viewer and a clock.
struct Ui {
    v: Viewer3d,
    now: f64,
}

impl Ui {
    fn open(name: &str, text: &str) -> Ui {
        let p = world(name, text);
        // A host page has no attributes: X_ITE's option defaults (ContextMenu, Notifications on).
        let mut v = Viewer3d::load_attributes(&p, p.parent(), T0, SIZE, SIZE, None, false, &[])
            .expect("load");
        v.frame_rate = 1.0 / DT;
        v.native_ui = true;
        let mut ui = Ui { v, now: T0 };

        ui.frames(8);
        ui
    }

    fn frames(&mut self, n: usize) {
        for _ in 0..n {
            self.now += DT;
            self.v.date_now = self.now;
            self.v.step(self.now);
        }
    }

    fn seconds(&mut self, s: f64) {
        self.frames((s / DT).round() as usize);
    }

    /// Mouse events take a bottom-left origin; the menu is laid out from the top.
    fn at(&self, x: f64, y_top: f64) -> (f64, f64) {
        (x, SIZE as f64 - y_top)
    }

    fn moved(&mut self, x: f64, y_top: f64) {
        let (x, y) = self.at(x, y_top);
        self.v.input(UiEvent::MouseMove { x, y });
    }

    fn down(&mut self, button: u8, x: f64, y_top: f64) {
        let (x, y) = self.at(x, y_top);
        self.v.input(UiEvent::MouseDown { button, x, y });
    }

    fn up(&mut self, button: u8, x: f64, y_top: f64) {
        let (x, y) = self.at(x, y_top);
        self.v.input(UiEvent::MouseUp { button, x, y });
    }

    fn click(&mut self, x: f64, y_top: f64) {
        self.moved(x, y_top);
        self.down(0, x, y_top);
        self.up(0, x, y_top);
    }

    /// The context menu event: the right button goes down (X11, Windows' Linux behavior of Chrome) and up.
    fn right_click(&mut self, x: f64, y_top: f64) {
        self.moved(x, y_top);
        self.down(2, x, y_top);
        self.up(2, x, y_top);
        self.frames(1);
    }

    fn texts(&self, owner: &[usize]) -> Vec<String> {
        self.v.context_menu().expect("menu").texts(owner)
    }

    fn menu_open(&self) -> bool {
        self.v.context_menu().is_some()
    }

    /// Hover each entry of the path (opening its submenu) and click the last one.
    fn click_path(&mut self, path: &[&str]) {
        let mut owner: Vec<usize> = vec![];

        for (k, text) in path.iter().enumerate() {
            let menu = self.v.context_menu().expect("menu");
            let p = menu
                .find(&owner, text)
                .unwrap_or_else(|| panic!("entry {text} in {:?}", menu.texts(&owner)));
            let (x, y) = menu.center_of(&p).expect("center");

            self.moved(x, y);

            if k + 1 == path.len() {
                self.down(0, x, y);
                self.up(0, x, y);
            } else {
                owner = p;
            }
        }

        self.frames(2);
    }

    fn option(&self, name: &str) -> x3d_runtime::browser_options::OptionValue {
        self.v.rt.browser_options.get_option(name).cloned().unwrap()
    }

    fn flag(&self, name: &str) -> bool {
        self.option(name).as_bool()
    }

    fn key(&mut self, down: bool, key: &str, code: u32) {
        self.v.input(UiEvent::NativeKey {
            down,
            key: key.into(),
            code,
        });
    }

    fn hist(&mut self) -> String {
        self.frames(1);
        self.v.rt.publish_clone_counts();
        let s = self.v.rt.scene.to_vrml_string();
        let i = s.find("hist \"").expect("hist") + 6;
        s[i..s[i..].find('"').unwrap() + i].trim().to_string()
    }
}

const SCRIPT: &str = r#"
DEF OUT Script {
  inputOnly SFBool act
  inputOnly SFBool kact
  inputOnly SFString kp
  inputOutput SFString hist ""
  url "ecmascript:
    function act (v) { hist = hist + ' act=' + v; }
    function kact (v) { hist = hist + ' key=' + v; }
    function kp (v) { hist = hist + ' kp=' + v; }
  "
}
"#;

fn main_world() -> String {
    format!(
        r#"#X3D V3.3 utf8
PROFILE Immersive
COMPONENT KeyDeviceSensor:2
META "creator" "A tester"
WorldInfo {{ title "Native UI" info ["First line" "Second line"] }}
NavigationInfo {{ type ["EXAMINE" "WALK"] }}
DEF V1 Viewpoint {{ description "First" position 0 0 10 }}
DEF V2 Viewpoint {{ description "Second" position 5 0 10 }}
Group {{ children [ DEF TS TouchSensor {{ description "Touch me" }} Shape {{ geometry Box {{ size 4 4 4 }} }} ] }}
DEF KS KeySensor {{ }}
{SCRIPT}
ROUTE TS.isActive TO OUT.act
ROUTE KS.isActive TO OUT.kact
ROUTE KS.keyPress TO OUT.kp
"#
    )
}

fn plain_world() -> &'static str {
    "#VRML V2.0 utf8\nShape { geometry Box { } }\n"
}

fn animation_world() -> &'static str {
    r#"#X3D V3.3 utf8
PROFILE Immersive
DEF Animations Group { children [
  Group { children [ DEF T1 TimeSensor { description "Spin" cycleInterval 2 } ] }
  Group { children [ DEF T2 TimeSensor { description "Wave" cycleInterval 2 } ] }
] }
Shape { geometry Box { } }
EXPORT Animations
"#
}

const MENU_MAIN: &[&str] = &[
    "X_ITE_R v0.1.0",
    "",
    "Viewpoints",
    "Available Viewers",
    "Straighten Horizon",
    "",
    "Primitive Quality",
    "Texture Quality",
    "",
    "",
    "Browser Timings",
    "Fullscreen",
    "",
    "Show World Info",
    "About X_ITE_R",
];

// --- opening and closing ---------------------------------------------------------------------------

#[test]
fn context_menu_entries_follow_build() {
    big(|| {
        let mut ui = Ui::open("menu.x3dv", &main_world());

        // The surface sees the right press; the menu opens at the pointer.
        ui.right_click(100.0, 100.0);
        assert!(ui.menu_open());
        assert_eq!(ui.texts(&[]), MENU_MAIN);

        let menu = ui.v.context_menu().unwrap();
        let sub = |name: &str| menu.texts(&menu.find(&[], name).unwrap());

        assert_eq!(sub("Viewpoints"), ["First", "Second"]);
        assert_eq!(sub("Available Viewers"), ["Examine Viewer", "Walk Viewer"]);
        assert_eq!(sub("Primitive Quality"), ["High", "Medium", "Low"]);
        assert_eq!(sub("Texture Quality"), ["High", "Medium", "Low"]);
    });
}

#[test]
fn context_menu_entries_follow_the_conditions() {
    big(|| {
        // WALK: the rubberband entry replaces the straighten entry; FLY has both; Shading needs Debug.
        let mut ui = Ui::open("menu2.x3dv", &main_world());

        ui.v.set_viewer("WALK");
        ui.right_click(100.0, 100.0);
        assert_eq!(ui.texts(&[])[4], "Display Rubberband");
        assert!(!ui.texts(&[]).contains(&"Straighten Horizon".to_string()));
        ui.v.hide_context_menu();

        ui.v.set_viewer("FLY");
        ui.right_click(100.0, 100.0);
        assert_eq!(
            &ui.texts(&[])[4..6],
            ["Straighten Horizon", "Display Rubberband"]
        );
        ui.v.hide_context_menu();

        ui.v.set_viewer("NONE");
        ui.right_click(100.0, 100.0);
        assert!(
            !ui.texts(&[])
                .iter()
                .any(|t| t.contains("Straighten") || t.contains("Rubberband"))
        );
        ui.v.hide_context_menu();

        ui.v.set_browser_option(
            "Debug",
            x3d_runtime::browser_options::OptionValue::Bool(true),
        );
        ui.right_click(100.0, 100.0);
        let menu = ui.v.context_menu().unwrap();
        let shading = menu.find(&[], "Shading").expect("Shading with Debug");

        assert_eq!(
            menu.texts(&shading),
            ["Points", "Wireframe", "Flat", "Gouraud", "Phong"]
        );
        ui.v.hide_context_menu();

        // A world with nothing to say: no viewpoints, one viewer, no world info.
        let mut plain = Ui::open("plain.wrl", plain_world());

        plain.right_click(100.0, 100.0);
        let t = plain.texts(&[]);

        assert!(!t.contains(&"Viewpoints".to_string()));
        assert!(
            t.contains(&"Available Viewers".to_string()),
            "the default NavigationInfo type is ANY"
        );
        assert!(!t.contains(&"Show World Info".to_string()));
        assert!(t.contains(&"Primitive Quality".to_string()));
    });
}

#[test]
fn context_menu_opens_and_closes_like_the_layer() {
    big(|| {
        let mut ui = Ui::open("close.x3dv", &main_world());

        // The press that opens the menu also reached the touch sensor (the box is under the pointer).
        ui.moved(256.0, 256.0);
        ui.down(2, 256.0, 256.0);
        assert!(ui.menu_open());
        ui.up(2, 256.0, 256.0);
        assert!(
            !ui.hist().contains("act=true"),
            "the right button does not press a TouchSensor"
        );

        // The layer takes the events: a left press elsewhere only closes the menu.
        ui.down(0, 450.0, 450.0);
        assert!(!ui.menu_open());
        ui.up(0, 450.0, 450.0);
        assert_eq!(ui.hist(), "", "nothing reached the world");

        // Over the sensor: the menu in the way, the press is the layer's.
        ui.right_click(256.0, 256.0);
        ui.down(0, 256.0, 256.0);
        ui.up(0, 256.0, 256.0);
        assert!(!ui.hist().contains("act=true"));

        // The layer's press closes the menu with the left and middle button. A right press is a `contextmenu` event too:
        // the layer hides the menu and the element's listener opens a new one at the new point (the pinned X_ITE:
        // case menuOpenClose, `reopen`); the world gets nothing of it.
        for button in [0u8, 1] {
            ui.right_click(100.0, 100.0);
            assert!(ui.menu_open(), "button {button}: opens");
            ui.down(button, 480.0, 20.0);
            assert!(!ui.menu_open(), "button {button}");
            ui.up(button, 480.0, 20.0);
        }

        ui.right_click(100.0, 100.0);
        let first = ui.v.context_menu().unwrap().center_of(&[0]).unwrap();

        ui.down(2, 300.0, 200.0);
        ui.up(2, 300.0, 200.0);
        assert!(ui.menu_open(), "reopened");
        assert_ne!(
            ui.v.context_menu().unwrap().center_of(&[0]).unwrap(),
            first,
            "at the new point"
        );
        assert!(!ui.hist().contains("act="), "the world got no press");

        // A right press on an entry is a contextmenu event of the menu: hidden, then opened again at that point.
        let menu = ui.v.context_menu().unwrap();
        let (x, y) = menu
            .center_of(&menu.find(&[], "Fullscreen").unwrap())
            .unwrap();

        ui.down(2, x, y);
        ui.up(2, x, y);
        assert!(ui.menu_open());
        ui.v.hide_context_menu();

        // Wheel and double click do not reach the viewer through the layer.
        ui.right_click(100.0, 100.0);
        let before = ui.v.user_pose();

        ui.v.input(UiEvent::Wheel {
            delta_y: 300.0,
            x: 256.0,
            y: 256.0,
        });
        ui.frames(3);
        assert_eq!(ui.v.user_pose(), before);
        assert!(ui.menu_open());

        // Escape and the arrow keys are for X3D: the menu has no keys.
        ui.key(true, "Escape", 27);
        ui.key(false, "Escape", 27);
        assert!(ui.menu_open());
    });
}

#[test]
fn context_menu_option_off_leaves_the_press_to_the_world() {
    big(|| {
        let mut ui = Ui::open("off.x3dv", &main_world());

        ui.v.set_browser_option(
            "ContextMenu",
            x3d_runtime::browser_options::OptionValue::Bool(false),
        );
        ui.right_click(256.0, 256.0);
        assert!(!ui.menu_open());

        // Back on: the next right press opens it.
        ui.v.set_browser_option(
            "ContextMenu",
            x3d_runtime::browser_options::OptionValue::Bool(true),
        );
        ui.right_click(256.0, 256.0);
        assert!(ui.menu_open());
    });
}

// --- the entries' actions --------------------------------------------------------------------------

#[test]
fn radio_and_checkbox_entries_keep_the_menu_and_set_options_and_descriptions() {
    big(|| {
        let mut ui = Ui::open("act.x3dv", &main_world());

        ui.right_click(40.0, 40.0);

        ui.click_path(&["Primitive Quality", "Low"]);
        assert_eq!(ui.option("PrimitiveQuality").as_str(), "LOW");
        assert_eq!(ui.v.rt.description, "Primitive Quality: low");
        assert!(ui.menu_open(), "a radio entry does not hide the menu");

        ui.click_path(&["Primitive Quality", "High"]);
        assert_eq!(ui.option("PrimitiveQuality").as_str(), "HIGH");
        assert_eq!(ui.v.status, "Primitive Quality: high");

        ui.click_path(&["Texture Quality", "Low"]);
        assert_eq!(ui.option("TextureQuality").as_str(), "LOW");
        assert_eq!(ui.v.rt.description, "Texture Quality: low");

        // The radio group: exactly one is selected after the clicks.
        let menu = ui.v.context_menu().unwrap();
        let sel = |name: &str| {
            let q = menu.find(&[], name).unwrap();
            menu.texts(&q)
                .iter()
                .filter(|t| {
                    let p = menu.find(&q, t).unwrap();
                    matches!(
                        menu.item(&p).unwrap().kind,
                        x3d_viewer::ui::Kind::Radio { selected: true, .. }
                    )
                })
                .cloned()
                .collect::<Vec<_>>()
        };

        assert_eq!(sel("Primitive Quality"), ["High"]);
        assert_eq!(sel("Texture Quality"), ["Low"]);

        ui.click_path(&["Straighten Horizon"]);
        assert!(!ui.flag("StraightenHorizon"));
        assert_eq!(ui.v.rt.description, "Straighten Horizon: off");
        assert!(ui.menu_open());
        ui.click_path(&["Straighten Horizon"]);
        assert!(ui.flag("StraightenHorizon"));
        assert_eq!(ui.v.rt.description, "Straighten Horizon: on");

        // Browser Timings: the option, no description.
        ui.v.rt.description.clear();
        ui.click_path(&["Browser Timings"]);
        assert!(ui.flag("Timings"));
        assert_eq!(ui.v.rt.description, "");
        assert!(ui.menu_open());
        ui.click_path(&["Browser Timings"]);
        assert!(!ui.flag("Timings"));
    });
}

#[test]
fn viewer_and_viewpoint_entries_act_on_the_browser() {
    big(|| {
        let mut ui = Ui::open("vp.x3dv", &main_world());

        ui.right_click(40.0, 40.0);
        assert_eq!(ui.v.viewer_field(), "EXAMINE");

        ui.click_path(&["Available Viewers", "Walk Viewer"]);
        assert_eq!(ui.v.viewer_field(), "WALK");
        assert_eq!(ui.v.rt.description, "Walk Viewer");
        assert!(ui.menu_open());

        // The menu was built for EXAMINE; a new one shows the rubberband entry.
        ui.v.hide_context_menu();
        ui.right_click(40.0, 40.0);
        assert!(ui.texts(&[]).contains(&"Display Rubberband".to_string()));

        ui.click_path(&["Display Rubberband"]);
        assert!(!ui.flag("Rubberband"));
        assert_eq!(ui.v.rt.description, "Rubberband: off");

        let first = ui.v.user_viewpoints()[0];
        let second = ui.v.user_viewpoints()[1];

        assert_eq!(ui.v.viewpoint_description(first), "First");
        ui.click_path(&["Viewpoints", "Second"]);
        ui.seconds(2.0);
        assert_eq!(ui.v.active_viewpoint().unwrap(), second);
        assert_eq!(
            ui.v.rt.description, "Second",
            "the bound viewpoint's description"
        );
        assert!(ui.menu_open());

        ui.click_path(&["Viewpoints", "First"]);
        ui.seconds(2.0);
        assert_eq!(ui.v.active_viewpoint().unwrap(), first);
    });
}

#[test]
fn plain_entries_hide_the_menu_and_ask_the_window() {
    big(|| {
        let mut ui = Ui::open("plainentries.x3dv", &main_world());

        ui.right_click(40.0, 40.0);
        ui.click_path(&["Fullscreen"]);
        assert!(
            !ui.menu_open(),
            "an entry that is no checkbox or radio hides the menu"
        );
        assert_eq!(ui.v.take_ui_requests(), [UiRequest::ToggleFullscreen]);

        // The text follows the window.
        ui.v.set_fullscreen(true);
        ui.right_click(40.0, 40.0);
        assert!(ui.texts(&[]).contains(&"Leave Fullscreen".to_string()));
        ui.v.set_fullscreen(false);
        ui.v.hide_context_menu();

        ui.right_click(40.0, 40.0);
        ui.click_path(&["About X_ITE_R"]);
        assert!(!ui.menu_open());
        assert!(
            matches!(&ui.v.take_ui_requests()[..], [UiRequest::OpenUrl(u)] if u.starts_with("https://"))
        );
    });
}

#[test]
fn user_menu_entries_and_animations() {
    big(|| {
        let mut ui = Ui::open("user.x3dv", &main_world());

        // `createUserMenu`: the host's entries sit between the second and third separator.
        ui.v.set_user_menu(vec![
            ("a".into(), "Do A".into()),
            ("b".into(), "Do B".into()),
        ]);
        ui.right_click(40.0, 40.0);
        let t = ui.texts(&[]);
        let i = t.iter().position(|t| t == "Do A").unwrap();

        assert_eq!(&t[i - 1..i + 3], ["", "Do A", "Do B", ""]);
        ui.click_path(&["Do B"]);
        assert!(!ui.menu_open());
        assert_eq!(ui.v.take_user_menu_clicks(), ["user-b"]);

        // Animations: the exported `Animations` node's TimeSensors.
        let mut an = Ui::open("anim.x3dv", animation_world());

        an.right_click(40.0, 40.0);
        let menu = an.v.context_menu().unwrap();
        let animations = menu.find(&[], "Animations").expect("Animations");

        assert_eq!(menu.texts(&animations), ["Spin", "Wave", "", "All", "None"]);

        let active = |an: &Ui, name: &str| {
            an.v.rt.get_bool(
                an.v.rt
                    .scene
                    .get_named_node(an.v.rt.scene.world, name)
                    .unwrap(),
                "isActive",
            )
        };

        an.click_path(&["Animations", "Spin"]);
        an.frames(2);
        assert!(active(&an, "T1") && !active(&an, "T2"));
        assert!(an.menu_open(), "a checkbox entry does not hide the menu");

        an.click_path(&["Animations", "All"]);
        assert!(!an.menu_open());
        an.frames(2);
        assert!(active(&an, "T1") && active(&an, "T2"));

        an.right_click(40.0, 40.0);
        an.click_path(&["Animations", "None"]);
        an.frames(2);
        assert!(!active(&an, "T1") && !active(&an, "T2"));
    });
}

#[test]
fn world_info_dialog() {
    big(|| {
        let mut ui = Ui::open("info.x3dv", &main_world());

        ui.right_click(40.0, 40.0);
        ui.click_path(&["Show World Info"]);
        assert!(!ui.menu_open());

        let panel = ui.v.ui.world_info.as_ref().expect("dialog");

        assert_eq!(
            (panel.title.as_str(), panel.info.len(), panel.meta.len()),
            ("Native UI", 2, 1)
        );
        assert_eq!(panel.tab, 0);
        assert_eq!(panel.tabs().len(), 2);

        // A tab button switches; a click anywhere else closes (the overlay's click).
        let (x, y) = (
            SIZE as f64 * 0.15 + 12.0 + 112.0 + 40.0,
            SIZE as f64 * 0.15 + 22.0,
        );

        ui.click(x, y);
        assert_eq!(ui.v.ui.world_info.as_ref().unwrap().tab, 1);
        ui.click(3.0, 3.0);
        assert!(ui.v.ui.world_info.is_none());
        assert!(ui.hist().is_empty(), "the overlay took the clicks");
    });
}

// --- notification ------------------------------------------------------------------------------------

#[test]
fn notification_shows_the_description_for_five_seconds() {
    big(|| {
        let mut ui = Ui::open("note.x3dv", &main_world());

        // The first viewpoint's description is the first notification; five seconds later it is gone.
        assert_eq!(ui.v.ui.notification.text, "First");
        assert!(ui.v.overlay().is_some_and(|o| !o.canvas.is_empty()));
        ui.seconds(6.0);
        assert!(ui.v.overlay().is_some_and(|o| o.canvas.is_empty()));
        assert!(ui.v.overlay().is_none());
        ui.v.set_description("Hello notification");

        // The width grows over 300 ms, from the right edge, 20 px below the top.
        assert!(ui.v.overlay().is_none(), "no width yet");
        ui.frames(3);

        let first = ui.v.overlay().expect("overlay");

        assert!(!first.canvas.is_empty());
        assert_eq!(first.y, 20);
        ui.seconds(0.5);

        let full = ui.v.overlay().expect("overlay");

        assert!(full.canvas.width > first.canvas.width);
        assert_eq!(full.x + full.canvas.width, SIZE, "right: 0");
        assert!(ui.v.overlay().is_none(), "unchanged");

        // The text pixels are dark on the light pill.
        let dark = (0..full.canvas.height)
            .flat_map(|y| (0..full.canvas.width).map(move |x| (x, y)))
            .filter(|(x, y)| {
                let p = full.canvas.pixel(*x, *y);
                p[3] > 200 && p[0] < 120
            })
            .count();

        assert!(dark > 20, "text drawn: {dark}");

        // 5000 ms after it was set the pill goes away, and the overlay is cleared once.
        ui.seconds(5.0);

        let gone = ui.v.overlay().expect("cleared");

        assert!(gone.canvas.is_empty());
        assert!(ui.v.overlay().is_none());

        // A second description restarts it; an empty one changes nothing.
        ui.v.set_description("Again");
        ui.seconds(0.5);
        assert!(ui.v.overlay().is_some_and(|o| !o.canvas.is_empty()));
        ui.v.set_description("");
        ui.seconds(1.0);
        assert!(
            ui.v.overlay().is_none_or(|o| !o.canvas.is_empty()),
            "still showing"
        );
    });
}

#[test]
fn notifications_option_off_shows_nothing() {
    big(|| {
        let mut ui = Ui::open("noteoff.x3dv", &main_world());

        ui.seconds(6.0);
        let _ = ui.v.overlay();
        ui.v.set_browser_option(
            "Notifications",
            x3d_runtime::browser_options::OptionValue::Bool(false),
        );
        ui.v.set_description("Hidden");
        ui.seconds(0.5);
        assert_eq!(ui.v.status, "Hidden", "the description itself is still set");
        assert!(ui.v.overlay().is_none_or(|o| o.canvas.is_empty()));

        // Sensor descriptions and viewpoint descriptions come through the same path.
        ui.v.set_browser_option(
            "Notifications",
            x3d_runtime::browser_options::OptionValue::Bool(true),
        );
        ui.moved(256.0, 256.0);
        ui.frames(3);
        assert_eq!(ui.v.rt.description, "Touch me");
        assert!(ui.v.overlay().is_some_and(|o| !o.canvas.is_empty()));
    });
}

#[test]
fn menu_overlay_is_drawn_and_cleaned_up() {
    big(|| {
        let mut ui = Ui::open("draw.x3dv", &main_world());

        // The first viewpoint's notification is over.
        ui.seconds(6.0);
        let _ = ui.v.overlay();
        let _ = ui.v.overlay();
        ui.right_click(100.0, 100.0);

        let o = ui.v.overlay().expect("menu overlay");

        assert_eq!(
            (o.x, o.y, o.canvas.width, o.canvas.height),
            (0, 0, SIZE, SIZE)
        );
        assert!(!o.canvas.is_empty());
        // The menu covers its rectangle; the text is dark on it.
        assert!(o.canvas.pixel(110, 110)[3] > 200);
        assert_eq!(o.canvas.pixel(5, 5)[3], 0, "the rest is transparent");

        // The hovered entry is highlighted (a redraw).
        let (x, y) = {
            let m = ui.v.context_menu().unwrap();
            m.center_of(&m.find(&[], "Primitive Quality").unwrap())
                .unwrap()
        };

        ui.moved(x, y);
        let hovered = ui.v.overlay().expect("hover redraw");

        assert_ne!(
            hovered.canvas.pixel(x as u32 - 40, y as u32),
            o.canvas.pixel(x as u32 - 40, y as u32)
        );
        assert!(ui.v.overlay().is_none());

        // Closing clears it once; a world reload closes an open menu.
        ui.down(0, 480.0, 480.0);
        assert!(ui.v.overlay().is_some_and(|o| o.canvas.is_empty()));
        assert!(ui.v.overlay().is_none());

        ui.right_click(100.0, 100.0);
        assert!(ui.menu_open());
        ui.v.input(UiEvent::MouseUp {
            button: 0,
            x: 1.0,
            y: 1.0,
        });
        let _ = ui.v.overlay();
        ui.v.hide_context_menu();
        assert!(!ui.menu_open());
        assert!(ui.v.take_ui_requests().is_empty());
        assert!(ui.v.take_user_menu_clicks().is_empty());
        assert!(ui.v.ui.world_buttons.iter().all(|b| !b));
    });
}

// --- the browser's own keys ---------------------------------------------------------------------------

#[test]
fn ctrl_plus_toggles_timings() {
    big(|| {
        let mut ui = Ui::open("plus.x3dv", &main_world());

        assert!(!ui.flag("Timings"));
        // 187: Plus // Opera; Ctrl is down.
        ui.key(true, "Control", 17);
        ui.key(true, "=", 187);
        ui.key(false, "=", 187);
        assert!(ui.flag("Timings"));
        ui.key(true, "+", 171);
        ui.key(false, "+", 171);
        assert!(!ui.flag("Timings"));
        ui.key(false, "Control", 17);

        // Without Ctrl the key is only a key.
        ui.key(true, "=", 187);
        assert!(!ui.flag("Timings"));
        ui.key(false, "=", 187);

        // The numeric keypad's plus (107) is not in the list.
        ui.key(true, "Control", 17);
        ui.key(true, "+", 107);
        ui.key(false, "+", 107);
        assert!(!ui.flag("Timings"));
    });
}

#[test]
fn debug_shading_keys() {
    big(|| {
        let mut ui = Ui::open("debug.x3dv", &main_world());
        let shading = |ui: &Ui| ui.option("Shading").as_str().to_string();
        let before = shading(&ui);

        // Debug off: the keys are keys.
        ui.key(true, "Control", 17);
        ui.key(true, "1", 49);
        ui.key(false, "1", 49);
        assert_eq!(shading(&ui), before);
        assert_ne!(ui.v.rt.description, "Shading: Pointset");

        ui.v.set_browser_option(
            "Debug",
            x3d_runtime::browser_options::OptionValue::Bool(true),
        );

        for (code, key, value, text) in [
            (49, "1", "POINT", "Shading: Pointset"),
            (50, "2", "WIREFRAME", "Shading: Wireframe"),
            (51, "3", "FLAT", "Shading: Flat"),
            (52, "4", "GOURAUD", "Shading: Gouraud"),
            (53, "5", "PHONG", "Shading: Phong"),
        ] {
            ui.key(true, key, code);
            ui.key(false, key, code);
            assert_eq!(shading(&ui), value, "Ctrl+{key}");
            assert_eq!(ui.v.rt.description, text);
            assert_eq!(ui.v.status, text);
        }

        // Ctrl+S: end update, begin update. The clock stops.
        let t = ui.v.frame_count;

        ui.key(true, "s", 83);
        ui.key(false, "s", 83);
        assert_eq!(
            ui.v.rt.description,
            "Begin Update".replace("Begin", "End"),
            "isLive () is read after the change"
        );
        ui.frames(5);
        assert_eq!(ui.v.frame_count, t, "frozen");
        ui.key(true, "s", 83);
        ui.key(false, "s", 83);
        assert_eq!(ui.v.rt.description, "Begin Update");
        ui.frames(5);
        assert!(ui.v.frame_count > t);

        // Without Ctrl the number keys do nothing.
        ui.key(false, "Control", 17);
        ui.v.set_browser_option(
            "Shading",
            x3d_runtime::browser_options::OptionValue::String("PHONG".into()),
        );
        ui.key(true, "1", 49);
        ui.key(false, "1", 49);
        assert_eq!(shading(&ui), "PHONG");
    });
}

#[test]
fn debug_option_hands_ctrl_number_keys_to_x3d() {
    // The viewer shortcuts keep Ctrl+1..4 and Ctrl+S unless Debug is on.
    let ctrl = ModifiersState::CONTROL;
    let one = Key::Character("1".into());
    let s = Key::Character("s".into());

    assert_eq!(
        shortcut_in(Platform::Linux, &one, ctrl, false),
        Some(Shortcut::Viewer("EXAMINE"))
    );
    assert_eq!(
        shortcut_in(Platform::Linux, &s, ctrl, false),
        Some(Shortcut::Screenshot)
    );
    assert_eq!(shortcut_in(Platform::Linux, &one, ctrl, true), None);
    assert_eq!(shortcut_in(Platform::Linux, &s, ctrl, true), None);
    assert_eq!(
        shortcut_in(Platform::Linux, &Key::Character("m".into()), ctrl, true),
        Some(Shortcut::Multisampling)
    );

    // Through the tracker: with Debug the key reaches X3D as a keydown / keyup pair.
    let mut t = KeyTracker::default();

    t.debug = true;
    let a = t.event(
        Platform::Linux,
        true,
        false,
        false,
        &one,
        &one,
        &PhysicalKey::Code(KeyCode::Digit1),
        ctrl,
    );

    assert_eq!(
        a,
        KeyAction::Dom {
            down: true,
            key: "1".into(),
            code: 49
        }
    );
}

/// Shift+F8 of a GeoViewpoint and the encoding of the world (X3DCoreContext `[_keydown]`), compared with pinned X_ITE
/// (`oracle/web-browser-ui-02-baseline`, cases `shiftF8Geo` and `shiftF8Encodings`).
#[test]
fn shift_f8_copies_a_geo_viewpoint_and_follows_the_encoding() {
    big(|| {
        let text = |name: &str, world: &str| {
            let mut ui = Ui::open(name, world);

            ui.seconds(1.0);
            ui.v.viewpoint_text().expect("a viewpoint")
        };

        // GC: the position as it is, no centerOfRotation (the default 0 0 0 is not written), geoSystem written.
        assert_eq!(
            text(
                "gc.wrl",
                "#X3D V4.0 utf8\nPROFILE Full\nGeoViewpoint { description \"Geo\" geoSystem [ \"GC\" ] position 5000000 3000000 4000000 }\nShape { geometry Box { } }\n"
            ),
            "GeoViewpoint {\n  geoSystem \"GC\"\n  position 5000000 3000000 4000000\n  orientation 0.47552961264567 0.839875826769667 0.261687185595756 2.84740341944062\n}\n"
        );

        // GD/WE: the default geoSystem is not written; the position and the center are geographic coordinates again (X_ITE's
        // digits differ after the 6th, from matrix noise).
        let geo = text(
            "geo.wrl",
            "#X3D V4.0 utf8\nPROFILE Full\nGeoViewpoint { description \"Geo\" geoSystem [ \"GD\" \"WE\" ] position 10 20 1000000 orientation 0 1 0 0.3 centerOfRotation 10 20 0 fieldOfView 0.7 }\nShape { geometry Box { } }\n",
        );

        let lines: Vec<&str> = geo.lines().collect();
        let numbers = |line: &str| -> Vec<f64> {
            line.split_whitespace()
                .skip(1)
                .map(|n| n.parse().unwrap())
                .collect()
        };

        assert_eq!(lines[0], "GeoViewpoint {");
        assert!(lines[1].starts_with("  position "), "{geo}");

        for (got, want) in numbers(lines[1]).iter().zip([10.0, 20.0, 1e6]) {
            assert!((got - want).abs() < 1e-6, "{geo}");
        }

        for (got, want) in numbers(lines[2])
            .iter()
            .zip([0.0799367, 0.981942, 0.171463, 3.11368])
        {
            assert!((got - want).abs() < 1e-5, "{geo}");
        }

        assert!(geo.ends_with("  centerOfRotation 10 20 0\n  fieldOfView 0.7\n}\n"));

        // A GeoOrigin is copied with its geoSystem (default, not written), geoCoords and rotateYUp.
        let origin = text(
            "origin.wrl",
            "#X3D V4.0 utf8\nPROFILE Full\nGeoViewpoint { description \"Geo\" geoOrigin GeoOrigin { geoSystem [ \"GD\" \"WE\" ] geoCoords 10 20 0 rotateYUp TRUE } geoSystem [ \"GD\" \"WE\" ] position 10.5 20.5 5000 orientation 0 1 0 0.3 centerOfRotation 10 20 0 }\nShape { geometry Box { } }\n",
        );

        assert!(origin.starts_with("GeoViewpoint {\n  geoOrigin GeoOrigin {\n    geoCoords 10 20 0\n    rotateYUp TRUE\n  }\n  position 10.5 20.5 4999.99999999907\n  orientation -0.0244308583484375 0.999170532681367 -0.0325788241285392 0.301799429181076\n  centerOfRotation 10 20 0\n}\n"), "{origin}");

        // The text is in the encoding the scene was parsed from.
        assert_eq!(
            text(
                "enc.x3d",
                "<?xml version=\"1.0\" encoding=\"UTF-8\"?>\n<X3D profile=\"Full\" version=\"4.0\"><Scene><Viewpoint DEF=\"G\" description=\"G\" position=\"1 2 10\" orientation=\"0 1 0 0.5\" centerOfRotation=\"0 1 0\" fieldOfView=\"0.6\"/><Shape><Box/></Shape></Scene></X3D>"
            ),
            "<Viewpoint\n    position='1 2 10'\n    orientation='0 0.999999999999999 0 0.5'\n    centerOfRotation='0 1 0'\n    fieldOfView='0.6'/>\n"
        );
    });
}

#[test]
fn shift_f8_copies_the_viewpoint_as_text() {
    big(|| {
        let mut ui = Ui::open(
            "f8.wrl",
            "#VRML V2.0 utf8\nDEF V1 Viewpoint { description \"A\" position 1 2 10 orientation 0 1 0 0.5 centerOfRotation 0 1 0 fieldOfView 0.6 }\nDEF V2 Viewpoint { description \"B\" position 0 0 12 }\nShape { geometry Box { } }\n",
        );

        // F8 alone does nothing.
        ui.key(true, "F8", 119);
        ui.key(false, "F8", 119);
        assert!(ui.v.take_ui_requests().is_empty());

        ui.key(true, "Shift", 16);
        ui.key(true, "F8", 119);
        ui.key(false, "F8", 119);

        // The text pinned X_ITE put on the clipboard (oracle/web-browser-ui-01-baseline, case shiftF8).
        let expected = "Viewpoint {\n  position 1 2 10\n  orientation 0 0.999999999999999 0 0.5\n  centerOfRotation 0 1 0\n  fieldOfView 0.6\n}\n";

        assert_eq!(
            ui.v.take_ui_requests(),
            [UiRequest::Clipboard(expected.to_string())]
        );

        // The window reports the copy.
        ui.v.clipboard_done();
        assert_eq!(ui.v.rt.description, "Viewpoint copied to clipboard.");

        ui.v.next_viewpoint(1);
        ui.seconds(2.0);
        ui.key(true, "F8", 119);
        ui.key(false, "F8", 119);
        assert_eq!(
            ui.v.take_ui_requests(),
            [UiRequest::Clipboard(
                "Viewpoint {\n  position 0 0 12\n}\n".to_string()
            )]
        );
    });
}

#[test]
fn command_key_is_a_browser_flag_not_a_key_sensor_field() {
    big(|| {
        let mut ui = Ui::open("cmd.x3dv", &main_world());

        // KeySensor.js has no case for keyCode 91 or 225: the key activates the sensor and gives no keyPress. (The ledger
        // names commandKey / altGrKey as KeySensor fields; X_ITE has them on the browser, X3DCoreContext.)
        ui.key(true, "Meta", 91);
        ui.key(false, "Meta", 91);
        ui.key(true, "AltGraph", 225);
        ui.key(false, "AltGraph", 225);
        assert_eq!(ui.hist(), "key=true key=false key=true key=false");

        // PointingDevice.mousedown returns when `getShiftKey () && (getControlKey () || getCommandKey ())`:
        // Shift+Command (or Shift+Ctrl) press does not press the TouchSensor.
        ui.key(true, "Shift", 16);
        ui.key(true, "Meta", 91);
        ui.moved(256.0, 256.0);
        ui.down(0, 256.0, 256.0);
        ui.up(0, 256.0, 256.0);
        assert!(!ui.hist().contains("act="), "{}", ui.hist());
        ui.key(false, "Meta", 91);

        ui.key(true, "Control", 17);
        ui.down(0, 256.0, 256.0);
        ui.up(0, 256.0, 256.0);
        assert!(!ui.hist().contains("act="));
        ui.key(false, "Control", 17);

        // Shift alone presses it.
        ui.down(0, 256.0, 256.0);
        assert!(ui.hist().contains("act=true"));
        ui.up(0, 256.0, 256.0);
        ui.key(false, "Shift", 16);
    });
}

#[test]
fn raw_events_open_the_menu_like_the_window() {
    big(|| {
        // The window path: a tracker for keys, mouse events as UiEvent. Ctrl+Plus is not a shortcut.
        let mut ui = Ui::open("raw.x3dv", &main_world());
        let mut t = KeyTracker::default();
        let plus = Key::Character("+".into());
        let ctrl = Key::Named(NamedKey::Control);
        let mut apply = |ui: &mut Ui, a: KeyAction| {
            if let KeyAction::Dom { down, key, code } = a {
                ui.v.input(UiEvent::NativeKey { down, key, code });
            }
        };

        let a = t.event(
            Platform::Linux,
            true,
            false,
            false,
            &ctrl,
            &ctrl,
            &PhysicalKey::Code(KeyCode::ControlLeft),
            ModifiersState::CONTROL,
        );
        apply(&mut ui, a);
        let a = t.event(
            Platform::Linux,
            true,
            false,
            false,
            &plus,
            &Key::Character("=".into()),
            &PhysicalKey::Code(KeyCode::Equal),
            ModifiersState::CONTROL,
        );
        apply(&mut ui, a);
        assert!(ui.flag("Timings"));
    });
}

#[test]
fn the_browser_build_and_the_drivers_have_no_native_ui() {
    big(|| {
        // A Viewer3d that no native window hosts (the browser build, x3d-drive): the right button is the world's, the
        // browser keys are the host page's, and nothing is notified.
        let mut ui = Ui::open("host.x3dv", &main_world());

        ui.seconds(6.0);
        let _ = ui.v.overlay();
        let _ = ui.v.overlay();
        ui.v.native_ui = false;
        ui.right_click(256.0, 256.0);
        assert!(!ui.menu_open());
        ui.key(true, "Control", 17);
        ui.key(true, "=", 187);
        ui.key(false, "=", 187);
        assert!(!ui.flag("Timings"));
        ui.key(false, "Control", 17);
        ui.key(true, "Shift", 16);
        ui.key(true, "F8", 119);
        assert!(ui.v.take_ui_requests().is_empty());
        ui.v.set_description("Not shown");
        assert!(ui.v.overlay().is_none_or(|o| o.canvas.is_empty()));
    });
}

// --- the Timings panel (BrowserTimings.js) -----------------------------------------------------------------------

const UI01: &str = include_str!("../../../oracle/web-browser-ui-01-baseline/ui01-baseline.json");

/// The pinned panel's state in the form that oracle/web-browser-ui-01 recorded it (`timingsState`), as far as it is
/// not a property of the DOM.
fn pinned(name: &str) -> serde_json::Value {
    let all: serde_json::Value = serde_json::from_str(UI01).unwrap();

    all["values"]["browserUi.timings"][name].clone()
}

/// The native panel in that form.
fn native_state(ui: &Ui) -> serde_json::Value {
    use x3d_viewer::ui_timings::ROWS;

    let t = &ui.v.ui.timings;
    let numeric = |v: &str| {
        let ends = |u: &str| v.ends_with(u);
        let digits = |s: &str| {
            !s.is_empty()
                && s.chars()
                    .all(|c| c.is_ascii_digit() || c == ',' || c == '.')
        };

        v.is_empty()
            || (ends(" fps") || ends(" ms") || ends(" m/s") || ends(" km/h"))
                && v.split(' ')
                    .next()
                    .is_some_and(|n| n.len() >= 4 && n.as_bytes()[n.len() - 3] == b'.' && digits(n))
            || v.split("; ").count() == 3 && v.split("; ").all(digits)
            || v.split(" + ").count() == 2 && v.split(" + ").all(digits)
            || digits(v)
    };
    let shown = t.visible_rows();

    serde_json::json!({
        "hidden": !t.visible,
        "head": "Browser Timings",
        "button": t.button_text(),
        "table": [if t.more { "more" } else { "less" }],
        "option": ui.flag("Timings"),
        "numeric": t.values.iter().map(|v| numeric(v)).collect::<Vec<_>>(),
        "rowDisplay": (0..ROWS.len()).map(|i| if i < shown { "table-row" } else { "none" }).collect::<Vec<_>>(),
        "position": ["absolute", "0px", "0px"],
    })
}

/// The properties of a pinned state that the native panel reproduces.
fn same_as_pinned(ui: &Ui, name: &str) {
    let p = pinned(name);
    let n = native_state(ui);

    for key in [
        "hidden",
        "head",
        "button",
        "table",
        "option",
        "numeric",
        "rowDisplay",
        "position",
    ] {
        assert_eq!(n[key], p[key], "{name}: {key}");
    }
}

fn cover_world() -> String {
    main_world().replace("size 4 4 4", "size 40 40 40")
}

impl Ui {
    fn ctrl_plus(&mut self) {
        self.key(true, "Control", 17);
        self.key(true, "=", 187);
        self.key(false, "=", 187);
        self.key(false, "Control", 17);
        self.frames(2);
    }

    /// The panel's parts in top-left canvas pixels.
    fn panel(&self) -> x3d_viewer::ui_timings::Layout {
        self.v.ui.timings.layout(&self.v.ui.text, SIZE as f64)
    }

    fn panel_button(&self) -> (f64, f64) {
        let b = self.panel().button;

        (b[0] + b[2] / 2.0, b[1] + b[3] / 2.0)
    }
}

#[test]
fn timings_panel_rows_texts_and_types_are_the_pinned_ones() {
    use x3d_viewer::ui_timings::ROWS;

    big(|| {
        let mut ui = Ui::open("timings.x3dv", &cover_world());

        // The Notification of the load has gone.
        ui.seconds(6.0);
        let _ = ui.v.overlay();
        let p = pinned("initial");

        // The rows of BrowserTimings.initialize: label, value cell class, title, row class.
        for (i, row) in p["rows"].as_array().unwrap().iter().enumerate() {
            assert_eq!(row[0], format!("{}:", ROWS[i].0), "label {i}");
            assert_eq!(row[1], ROWS[i].1, "class {i}");
            assert_eq!(row[2], ROWS[i].2, "title {i}");
            assert_eq!(row[3] == "x_ite-private-more", i == 1, "more {i}");
        }

        assert_eq!(p["rows"].as_array().unwrap().len(), ROWS.len());

        // Hidden until the option is set; the panel starts as LESS.
        same_as_pinned(&ui, "initial");
        assert!(ui.v.overlay().is_none_or(|o| o.canvas.is_empty()));

        // Ctrl+Plus (the winit key path of ctrl_plus_toggles_timings) shows it.
        ui.ctrl_plus();
        same_as_pinned(&ui, "on");
        assert!(ui.v.overlay().is_some_and(|o| !o.canvas.is_empty()));

        // The button: a real left click on it, down and up, changes the type; again, back.
        let (bx, by) = ui.panel_button();

        ui.click(bx, by);
        ui.frames(2);
        same_as_pinned(&ui, "more");
        ui.v.overlay();

        let (bx, by) = ui.panel_button();

        ui.click(bx, by);
        ui.frames(2);
        same_as_pinned(&ui, "less");

        // Ctrl+Plus again hides it, and again shows it.
        ui.ctrl_plus();
        same_as_pinned(&ui, "keyOff");
        ui.ctrl_plus();
        same_as_pinned(&ui, "keyOn");

        // The option off (the menu entry, setBrowserOption): hidden, and the overlay is cleared once.
        let _ = ui.v.overlay();
        ui.v.set_browser_option(
            "Timings",
            x3d_runtime::browser_options::OptionValue::Bool(false),
        );
        same_as_pinned(&ui, "off");
        assert!(ui.v.overlay().is_some_and(|o| o.canvas.is_empty()));
        assert!(ui.v.overlay().is_none());
    });
}

#[test]
fn timings_panel_pointer_rules_and_cleanup() {
    big(|| {
        let mut ui = Ui::open("timings-pointer.x3dv", &cover_world());

        ui.seconds(6.0);
        let _ = ui.v.overlay();

        ui.ctrl_plus();

        let l = ui.panel();
        let (bx, by) = ui.panel_button();

        // `pointer-events: none` on the panel: a press on its label cells goes to the world (the TouchSensor).
        let (lx, ly) = (l.table_x + 4.0, l.top + 40.0);

        assert!(lx < l.rect[0] + l.rect[2] && ly < l.rect[1] + l.rect[3]);
        ui.moved(lx, ly);
        ui.down(0, lx, ly);
        ui.up(0, lx, ly);
        let base = ui.hist();
        assert!(
            base.ends_with("act=true act=false"),
            "the world got the press: {base}"
        );

        // `pointer-events: all` on the button: the world gets nothing, and the panel changes its type once.
        let before = ui.v.ui.timings.more;

        ui.moved(bx, by);
        ui.down(0, bx, by);
        ui.up(0, bx, by);
        assert_eq!(ui.hist(), base, "no press reached the world");
        assert_ne!(ui.v.ui.timings.more, before);

        // A press on the button that is released elsewhere is no click.
        let (bx, by) = ui.panel_button();
        let before = ui.v.ui.timings.more;

        ui.moved(bx, by);
        ui.down(0, bx, by);
        ui.up(0, 300.0, 100.0);
        assert_eq!(ui.v.ui.timings.more, before);
        assert_eq!(ui.hist(), base, "the release did not reach the world");
        assert!(!ui.v.ui.timings.pressed);

        // The two titled cells take the pointer too (MORE shows them).
        assert!(ui.v.ui.timings.more);

        let cell = ui.panel().cells[0];
        let (cx, cy) = (cell[0] + cell[2] / 2.0, cell[1] + cell[3] / 2.0);

        ui.click(cx, cy);
        assert_eq!(ui.hist(), base, "a titled cell took the click");

        // The context menu event bubbles from the button to the surface.
        let (bx, by) = ui.panel_button();

        ui.right_click(bx, by);
        assert!(ui.menu_open());
        ui.v.hide_context_menu();

        // Cleanup: option off -> hidden and the overlay cleared; the panel takes no pointer any more.
        ui.seconds(6.0);
        let _ = ui.v.overlay();
        ui.ctrl_plus();

        let after_keys = ui.hist();

        assert!(!ui.v.ui.timings.visible);
        assert!(ui.v.overlay().is_some_and(|o| o.canvas.is_empty()));
        assert!(ui.v.overlay().is_none());
        ui.moved(bx, by);
        ui.down(0, bx, by);
        ui.up(0, bx, by);
        assert_eq!(ui.hist(), format!("{after_keys} act=true act=false"));
    });
}

#[test]
fn timings_panel_values() {
    big(|| {
        let mut ui = Ui::open(
            "timings-values.x3dv",
            "#VRML V2.0 utf8\nShape { geometry Box { } }\nShape { appearance Appearance { material Material { transparency 0.5 } } geometry Box { } }\n",
        );

        ui.ctrl_plus();
        assert_eq!(ui.v.ui.timings.values[0], "0.00 fps", "reset: no frame yet");
        assert_eq!(ui.v.ui.timings.values[1], "0.00 m/s");

        // The button is the first click: MORE.
        let (bx, by) = ui.panel_button();

        ui.click(bx, by);

        // A second of frames at 30 per second: the StopWatch's average is the 33.3 ms between the updates.
        for _ in 0..40 {
            ui.frames(1);
            let f = ui.v.build_frame();

            drop(f);
            ui.v.record_frame_times(5.0, 1.0, 2.0);
        }

        let v = ui.v.ui.timings.values.clone();

        assert_eq!(v[0], "30.00 fps");
        assert_eq!(v[1], "0.00 m/s");
        assert_eq!(v[2], "5.00 ms", "Browser: the tick");
        assert_eq!(v[3], "3.00 ms", "X3D total: step + display");
        assert_eq!(v[4], "1.00 ms", "Event Processing: the step");
        assert_eq!(v[9], "2.00 ms", "Rendering: build + draw");
        assert_eq!(
            v[10], "0; 0; 24",
            "12 triangles of the Box and the Sphere's"
        );
        assert_eq!(v[11], "1 + 1", "one opaque and one transparent shape");
        assert_eq!(v[12], "0");
    });
}
