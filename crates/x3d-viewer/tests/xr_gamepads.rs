//! XR gamepad navigation (issue #83): the rules of X_ITE 16.5.0 `Browser/WebXR/xrExamineViewer.js` and
//! `xrX3DFlyViewer.js`, which X3DWebXRContext.xrFrame calls with the gamepads of the XR input sources before each
//! frame's events. EXAMINE rotates with the thumbstick, zooms with button 0 and pans with button 1, and it disconnects
//! once when the thumbstick goes back to the center. FLY and WALK fly with the thumbstick (button 0 doubles the speed)
//! and pan with button 1. NONE, PLANE and LOOKAT keep X3DViewer's empty `gamepads`. The world is synthetic.

use std::path::{Path, PathBuf};
use x3d_fields::math::{Rotation4, Vector3};
use x3d_viewer::nav::Gamepad;
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 60.0;

fn world(kind: &str) -> String {
    format!(
        "#X3D V3.3 utf8\nPROFILE Interchange\nNavigationInfo {{ type \"{kind}\" }}\nViewpoint {{ position 0 0 10 }}\nShape {{ geometry Box {{ }} }}\n"
    )
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("x3d-xr-gamepads-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

struct World {
    v: Viewer3d,
    t: f64,
    dir: PathBuf,
}

impl World {
    fn load(kind: &str) -> World {
        let d = dir(kind);
        let p = d.join("main.x3dv");
        std::fs::write(&p, world(kind)).unwrap();
        let mut v = Viewer3d::load_attributes(
            &p,
            Some(Path::new(&d)),
            T0,
            64,
            64,
            Some(60.0),
            false,
            ORACLE_PAGE_ATTRIBUTES,
        )
        .expect("load");
        v.frame_rate = 1.0 / DT;
        v.xr_session_started();
        let mut w = World { v, t: T0, dir: d };
        w.frames(&[], 2);
        w
    }

    /// `n` XR frames: the gamepads go to the viewer, then the frame's events run.
    fn frames(&mut self, gamepads: &[Gamepad], n: usize) {
        for _ in 0..n {
            self.t += DT;
            self.v.date_now = self.t;
            self.v.xr_gamepads(gamepads);
            self.v.step(self.t);
        }
    }

    fn pose(&mut self) -> (Vector3, Rotation4) {
        let (p, o, _) = self.v.user_pose();
        (p, o)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

fn stick(x: f64, y: f64, button0: bool, button1: bool) -> Gamepad {
    Gamepad {
        axes: [x, y],
        button0,
        button1,
    }
}

fn same(a: &(Vector3, Rotation4), b: &(Vector3, Rotation4)) -> bool {
    let (pa, oa) = a;
    let (pb, ob) = b;
    (pa.x - pb.x).abs() < 1e-9
        && (pa.y - pb.y).abs() < 1e-9
        && (pa.z - pb.z).abs() < 1e-9
        && (angle(oa) - angle(ob)).abs() < 1e-9
}

/// Rotation4::angle caches its value, so it takes `&mut self`.
fn angle(r: &Rotation4) -> f64 {
    r.clone().angle()
}

fn norm(v: &Vector3) -> f64 {
    (v.x * v.x + v.y * v.y + v.z * v.z).sqrt()
}

#[test]
fn examine_thumbstick_rotates_about_the_center_and_stops_when_released() {
    let mut w = World::load("EXAMINE");
    let start = w.pose();

    w.frames(&[stick(0.5, 0.0, false, false)], 20);
    let turned = w.pose();

    assert!(angle(&turned.1) > 0.01, "no rotation: {:?}", turned.1);
    assert!(
        (norm(&turned.0) - 10.0).abs() < 1e-6,
        "the distance to the center changed: {:?}",
        turned.0
    );
    assert!(!same(&start, &turned));

    // The thumbstick back at the center: one disconnect (the chasers stop), then the pose stays.
    w.frames(&[stick(0.0, 0.0, false, false)], 1);
    let released = w.pose();
    w.frames(&[stick(0.0, 0.0, false, false)], 30);
    assert!(
        same(&released, &w.pose()),
        "the viewer moved after the release"
    );
}

#[test]
fn examine_button0_zooms_in_with_the_thumbstick_forward() {
    let mut w = World::load("EXAMINE");

    w.frames(&[stick(0.0, -0.8, true, false)], 20);
    let (position, _) = w.pose();

    assert!(position.z < 10.0 - 1e-3, "no zoom in: {position:?}");
    assert!(
        position.x.abs() < 1e-9 && position.y.abs() < 1e-9,
        "{position:?}"
    );
}

#[test]
fn examine_gamepads_without_a_moved_thumbstick_change_nothing() {
    let mut w = World::load("EXAMINE");
    let start = w.pose();

    // Axes 2 and 3 at 0 (or NaN, which is falsy) select no gamepad, also with a pressed button.
    w.frames(
        &[
            stick(0.0, 0.0, true, true),
            stick(f64::NAN, 0.0, false, false),
        ],
        20,
    );

    assert!(same(&start, &w.pose()));
}

#[test]
fn fly_thumbstick_forward_flies_forward_and_button0_is_faster() {
    let mut slow = World::load("FLY");
    let mut fast = World::load("FLY");

    slow.frames(&[stick(0.0, -1.0, false, false)], 30);
    fast.frames(&[stick(0.0, -1.0, true, false)], 30);

    let (p_slow, _) = slow.pose();
    let (p_fast, _) = fast.pose();

    assert!(p_slow.z < 10.0 - 1e-3, "no forward flight: {p_slow:?}");
    assert!(
        10.0 - p_fast.z > 10.0 - p_slow.z,
        "button 0 is not faster: {p_fast:?} vs {p_slow:?}"
    );
}

#[test]
fn fly_button1_pans_sideways() {
    let mut w = World::load("FLY");

    w.frames(&[stick(1.0, 0.0, false, true)], 30);
    let (position, _) = w.pose();

    assert!(position.x > 1e-3, "no pan to the right: {position:?}");
    assert!((position.z - 10.0).abs() < 1e-6, "{position:?}");
}

#[test]
fn none_plane_and_lookat_ignore_the_gamepads() {
    for kind in ["NONE", "PLANE", "LOOKAT"] {
        let mut w = World::load(kind);
        let start = w.pose();

        w.frames(&[stick(1.0, -1.0, true, false)], 20);

        assert!(same(&start, &w.pose()), "{kind} moved");
    }
}
