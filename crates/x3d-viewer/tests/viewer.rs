//! Viewer core tests: live TimeSensor/interpolator, viewpoint binding, navigation, pointer
//! sensors. Worlds are written to a temp directory; time is virtual (`Viewer3d::step`).

use std::path::PathBuf;
use x3d_viewer::chaser::Chaser;
use x3d_viewer::viewer::{Key, UiEvent, Viewer3d};
use x3d_fields::math::Vector3;

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;

fn world(name: &str, text: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("x3d-viewer-tests-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();
    let p = dir.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

/// Run on a big stack (QuickJS), like the binaries do.
fn big<F: FnOnce() + Send + 'static>(f: F) {
    std::thread::Builder::new().stack_size(256 << 20).spawn(f).unwrap().join().unwrap();
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

fn number_after(s: &str, key: &str) -> f64 {
    let i = s.find(key).unwrap_or_else(|| panic!("{key:?} not in\n{s}")) + key.len();
    s[i..].split(|c: char| c.is_whitespace()).next().unwrap().parse().unwrap()
}

fn run(v: &mut Viewer3d, from: u64, to: u64) {
    for k in from..=to {
        v.date_now = T0 + k as f64 * DT;
        v.step(T0 + k as f64 * DT);
    }
}

fn scene(v: &mut Viewer3d) -> String {
    v.rt.publish_clone_counts();
    v.rt.scene.to_vrml_string()
}

#[test]
fn chaser_converges_with_cosine_step_response() {
    let mut c: Chaser<Vector3> = Chaser::new(1.0, 0.0);
    c.set_value(Vector3::new(0.0, 0.0, 0.0), 0.0);
    c.set_destination(Vector3::new(10.0, 0.0, 0.0), 0.0);
    let mut last = 0.0;

    for k in 1..=200 {
        let v = c.prepare(k as f64 / 30.0);
        assert!(v.x >= last - 1e-9, "monotone");
        last = v.x;
    }

    assert!((last - 10.0).abs() < 1e-6, "converged, got {last}");
}

#[test]
fn live_time_sensor_drives_interpolator() {
    big(|| {
        let mut v = open(
            "anim.wrl",
            "#VRML V2.0 utf8\nDEF T Transform { children Shape { geometry Box {} } }\nDEF C TimeSensor { cycleInterval 2 loop TRUE startTime 1 }\nDEF I OrientationInterpolator { key [0 1] keyValue [0 1 0 0, 0 1 0 3.0] }\nROUTE C.fraction_changed TO I.set_fraction\nROUTE I.value_changed TO T.set_rotation\n",
        );
        // Half a cycle after start (frame at t0 + 2 s = start 1 + 1 s): fraction 0.5.
        run(&mut v, 1, 60);
        let s = scene(&mut v);
        assert!(s.contains("rotation 0 1 0 1.5"), "rotation at half cycle:\n{s}");
    });
}

#[test]
fn script_events_processed_each_frame() {
    big(|| {
        let mut v = open(
            "script.wrl",
            "#VRML V2.0 utf8\nDEF T Transform { children Shape { geometry Box {} } }\nDEF C TimeSensor { cycleInterval 1000 loop TRUE startTime 1 }\nDEF S Script { eventIn SFTime t eventOut SFVec3f p field SFInt32 n 0\n url \"javascript: function t (v) { n = n + 1; p = new SFVec3f (n, 0, 0); }\" }\nROUTE C.time TO S.t\nROUTE S.p TO T.set_translation\n",
        );
        run(&mut v, 1, 60);
        let s = scene(&mut v);
        // startTime 1 is in the past, so the loop TimeSensor runs from the first frame and the
        // Script's eventsProcessed-style counter advances exactly once per frame; the routed
        // translation agrees with the script field.
        let n = number_after(&s, "translation ");
        assert_eq!(n, number_after(&s, "SFInt32 n "), "{s}");
        run(&mut v, 61, 70);
        let s = scene(&mut v);
        assert_eq!(number_after(&s, "translation "), n + 10.0, "one tick per frame");
    });
}

const VPS: &str = "#VRML V2.0 utf8\nNavigationInfo { type [\"EXAMINE\" \"WALK\" \"FLY\" \"ANY\"] }\nDEF A Viewpoint { position 0 0 10 description \"a\" }\nDEF B Viewpoint { position 5 0 0 orientation 0 1 0 1.5707963 description \"b\" }\nShape { geometry Box { size 1 1 1 } }\n";

#[test]
fn viewpoint_stack_next_previous_first_last() {
    big(|| {
        let mut v = open("vps.wrl", VPS);
        assert_eq!(v.user_viewpoints().len(), 2);
        let (p, _, _) = v.user_pose();
        assert!((p.z - 10.0).abs() < 1e-9, "first viewpoint is bound at load");

        v.input(UiEvent::Key(Key::PageDown));
        run(&mut v, 1, 90);
        let (p, _, _) = v.user_pose();
        assert!((p.x - 5.0).abs() < 1e-9 && p.z.abs() < 1e-9, "next viewpoint: {p:?}");

        v.input(UiEvent::Key(Key::Home));
        run(&mut v, 91, 180);
        let (p, _, _) = v.user_pose();
        assert!((p.z - 10.0).abs() < 1e-9, "first viewpoint: {p:?}");

        v.input(UiEvent::Key(Key::End));
        run(&mut v, 181, 270);
        let (p, _, _) = v.user_pose();
        assert!((p.x - 5.0).abs() < 1e-9, "last viewpoint: {p:?}");
    });
}

#[test]
fn examine_drag_rotates_and_wheel_zooms() {
    big(|| {
        let mut v = open("examine.wrl", VPS);
        let (p0, _, _) = v.user_pose();
        v.input(UiEvent::MouseDown { button: 0, x: 256.0, y: 256.0 });
        for i in 1..=10 {
            v.input(UiEvent::MouseMove { x: 256.0 + 10.0 * i as f64, y: 256.0 });
            run(&mut v, i, i);
        }
        v.input(UiEvent::MouseUp { button: 0, x: 356.0, y: 256.0 });
        run(&mut v, 11, 120);
        let (p1, _, _) = v.user_pose();
        assert!(p1.x.abs() > 1.0, "dragging right orbits the camera sideways: {p1:?}");
        assert!((p1.norm() - p0.norm()).abs() < 1e-6, "orbit keeps the distance");

        v.input(UiEvent::Wheel { delta_y: -100.0, x: 256.0, y: 256.0 });
        run(&mut v, 121, 180);
        let (p2, _, _) = v.user_pose();
        assert!(p2.norm() < p1.norm() - 0.1, "wheel zooms in: {} -> {}", p1.norm(), p2.norm());
    });
}

#[test]
fn walk_forward_moves_and_gravity_holds_on_ground() {
    big(|| {
        let mut v = open(
            "walk.wrl",
            "#VRML V2.0 utf8\nNavigationInfo { type \"WALK\" avatarSize [0.25 1.6 0.75] }\nViewpoint { position 0 1.6 10 }\nShape { geometry Box { size 100 0.1 100 } }\n",
        );
        run(&mut v, 1, 30);
        let (p0, _, _) = v.user_pose();
        assert!((p0.y - 1.6).abs() < 0.2, "standing on the ground: {p0:?}");
        v.input(UiEvent::MouseDown { button: 0, x: 256.0, y: 256.0 });
        v.input(UiEvent::MouseMove { x: 256.0, y: 312.0 });
        run(&mut v, 31, 90);
        v.input(UiEvent::MouseUp { button: 0, x: 256.0, y: 312.0 });
        let (p1, _, _) = v.user_pose();
        assert!(p1.z < p0.z - 0.5, "dragging up walks forward (-z): {p0:?} -> {p1:?}");
        assert!((p1.y - p0.y).abs() < 0.05, "stays at eye height: {p1:?}");
    });
}

#[test]
fn touch_and_plane_sensors_fire() {
    big(|| {
        let mut v = open(
            "sensors.wrl",
            "#VRML V2.0 utf8\nNavigationInfo { type \"NONE\" }\nViewpoint { position 0 0 10 }\nTransform { translation -2 0 0 children [ DEF TS TouchSensor {} Shape { geometry Box {} } ] }\nDEF TC TimeSensor {}\nROUTE TS.touchTime TO TC.set_startTime\nDEF P Transform { translation 2 0 0 children [ DEF PS PlaneSensor { autoOffset TRUE } Shape { geometry Box {} } ] }\nROUTE PS.translation_changed TO P.set_translation\n",
        );
        run(&mut v, 1, 3);
        // 512 px, fov pi/4 => about 61 px per unit at z = 0.
        let (cx, cy) = (256.0 - 2.0 * 61.0, 256.0);
        v.input(UiEvent::MouseMove { x: cx, y: cy });
        v.input(UiEvent::MouseDown { button: 0, x: cx, y: cy });
        v.input(UiEvent::MouseUp { button: 0, x: cx, y: cy });
        run(&mut v, 4, 6);
        let s = scene(&mut v);
        assert!(!s.contains("DEF TC TimeSensor {\n}") && s.contains("startTime"), "touchTime reached the TimeSensor:\n{s}");

        let (px, py) = (256.0 + 2.0 * 61.0, 256.0);
        v.input(UiEvent::MouseMove { x: px, y: py });
        v.input(UiEvent::MouseDown { button: 0, x: px, y: py });
        run(&mut v, 7, 7);
        v.input(UiEvent::MouseMove { x: px + 61.0, y: py });
        run(&mut v, 8, 8);
        v.input(UiEvent::MouseUp { button: 0, x: px + 61.0, y: py });
        run(&mut v, 9, 10);
        let s = scene(&mut v);
        let x = number_after(&s, "DEF P Transform {\n  translation ");
        assert!((0.7..=1.2).contains(&x), "plane sensor moved the box by about 1 unit (translation = offset + delta): {x}\n{s}");
    });
}

#[test]
fn anchor_activation_is_reported_not_opened() {
    big(|| {
        let mut v = open(
            "anchor.wrl",
            "#VRML V2.0 utf8\nNavigationInfo { type \"NONE\" }\nViewpoint { position 0 0 10 }\nAnchor { url \"http://example.invalid/x.wrl\" description \"go\" children Shape { geometry Box { size 4 4 4 } } }\n",
        );
        run(&mut v, 1, 3);
        v.input(UiEvent::MouseMove { x: 256.0, y: 256.0 });
        v.input(UiEvent::MouseDown { button: 0, x: 256.0, y: 256.0 });
        v.input(UiEvent::MouseUp { button: 0, x: 256.0, y: 256.0 });
        run(&mut v, 4, 6);
        assert!(v.status.contains("Anchor") && v.status.contains("example.invalid"), "status: {:?}", v.status);
    });
}
