//! The sensor events of XR input sources (issue #83), after X_ITE 16.5.0 X3DWebXRContext.xrUpdatePointers: each active
//! source picks along its ray, its button 0 presses once and releases once (buttonPressEvent, buttonReleaseEvent), the
//! source moves the pointer (motionNotifyEvent), and a source whose ray starts to hit a sensor is due one haptic pulse.
//! A TouchSensor reports isOver, isActive and touchTime to a Script that prints them. The world is synthetic.

use std::path::{Path, PathBuf};
use x3d_fields::math::{Matrix4, Vector3};
use x3d_viewer::viewer::{Viewer3d, XrSource};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 60.0;

const WORLD: &str = r#"#X3D V3.3 utf8
PROFILE Immersive
Viewpoint { position 0 0 10 }
Group { children [ DEF T TouchSensor { } Shape { geometry Box { size 2 2 2 } } ] }
DEF S Script {
  inputOnly SFBool over
  inputOnly SFBool active
  inputOnly SFTime touch
  url "javascript:
    function over (v) { print ('over ' + v); }
    function active (v) { print ('active ' + v); }
    function touch (v) { print ('touch'); }
  "
}
ROUTE T.isOver TO S.over
ROUTE T.isActive TO S.active
ROUTE T.touchTime TO S.touch
"#;

struct World {
    v: Viewer3d,
    t: f64,
    dir: PathBuf,
}

impl World {
    fn load() -> World {
        let dir = std::env::temp_dir().join(format!("x3d-xr-pointers-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("main.x3dv");
        std::fs::write(&p, WORLD).unwrap();
        let v = Viewer3d::load_opts(&p, Some(Path::new(&dir)), T0, 64, 48, Some(60.0), true)
            .expect("load");
        let mut w = World { v, t: T0, dir };
        w.frame(&[]);
        w.frame(&[]);
        w
    }

    /// One frame: the world steps, then the pointers of the sources run (displayFinishEvents). Returns the new console
    /// lines and the pulsed ids.
    fn frame(&mut self, sources: &[XrSource]) -> (Vec<String>, Vec<u32>) {
        let before = self.v.rt.console.len();
        self.t += DT;
        self.v.date_now = self.t;
        self.v.step(self.t);
        let pulses = self.v.xr_pointers(sources);
        self.t += DT;
        self.v.date_now = self.t;
        self.v.step(self.t);
        let lines = self.v.rt.console[before..]
            .iter()
            .map(|l| l.trim_start_matches("log: ").to_string())
            .collect();
        (lines, pulses)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

fn source(id: u32, x: f64, button0: bool) -> XrSource {
    let mut inverse = Matrix4::IDENTITY;
    inverse.translate(&Vector3::new(x, 0.0, 0.0));
    inverse.inverse();
    XrSource {
        id,
        active: true,
        button0,
        inverse,
    }
}

#[test]
fn a_controller_ray_and_button_drive_a_touch_sensor() {
    let mut w = World::load();

    // The ray points past the box: nothing.
    let (lines, pulses) = w.frame(&[source(7, 5.0, false)]);
    assert!(lines.is_empty(), "{lines:?}");
    assert!(pulses.is_empty());

    // The ray moves onto the box: isOver TRUE and one haptic pulse.
    let (lines, pulses) = w.frame(&[source(7, 0.0, false)]);
    assert_eq!(lines, ["over true"]);
    assert_eq!(pulses, [7]);

    // Still on the box: no second pulse.
    let (lines, pulses) = w.frame(&[source(7, 0.0, false)]);
    assert!(lines.is_empty(), "{lines:?}");
    assert!(pulses.is_empty());

    // Button 0 down: isActive TRUE once, also when it is held.
    let (lines, _) = w.frame(&[source(7, 0.0, true)]);
    assert_eq!(lines, ["active true"]);
    let (lines, _) = w.frame(&[source(7, 0.0, true)]);
    assert!(lines.is_empty(), "{lines:?}");

    // Button 0 up over the box: isActive FALSE and touchTime.
    let (lines, _) = w.frame(&[source(7, 0.0, false)]);
    assert_eq!(lines, ["active false", "touch"]);

    // The ray leaves the box: isOver FALSE. Back on it: a new pulse.
    let (lines, _) = w.frame(&[source(7, 5.0, false)]);
    assert_eq!(lines, ["over false"]);
    let (_, pulses) = w.frame(&[source(7, 0.0, false)]);
    assert_eq!(pulses, [7]);
}

#[test]
fn an_inactive_source_does_nothing() {
    let mut w = World::load();
    let mut s = source(1, 0.0, true);
    s.active = false;

    let (lines, pulses) = w.frame(&[s]);
    assert!(lines.is_empty(), "{lines:?}");
    assert!(pulses.is_empty());
}
