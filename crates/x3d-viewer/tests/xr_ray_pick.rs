//! The pick along the target ray of an XR input source (issue #83): X_ITE 16.5.0 X3DWebXRContext.xrUpdatePointers calls
//! `touch (viewport [2] / 2, viewport [3] / 2, hit, inputSource)`, and X3DLayerNode.camera (POINTER) then looks along the
//! input source's ray with an orthographic projection and the input source's matrices times the viewpoint's. The ray
//! starts at the input source and goes along its -Z axis; the canvas pointer does not matter. The world is synthetic.

use std::path::{Path, PathBuf};
use x3d_fields::math::{Matrix4, Rotation4, Vector3};
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 60.0;

// A box of size 2 at the origin with a TouchSensor, a box of size 1 at x 4, the viewpoint at z 10.
const WORLD: &str = "#X3D V3.3 utf8
PROFILE Interactive
Viewpoint { position 0 0 10 }
Group { children [ TouchSensor { } Shape { geometry Box { size 2 2 2 } } ] }
Transform { translation 4 0 0 children Shape { geometry Box { size 1 1 1 } } }
";

struct World {
    v: Viewer3d,
    dir: PathBuf,
}

impl World {
    fn load() -> World {
        let dir = std::env::temp_dir().join(format!("x3d-xr-ray-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let p = dir.join("main.x3dv");
        std::fs::write(&p, WORLD).unwrap();
        let mut v = Viewer3d::load_attributes(
            &p,
            Some(Path::new(&dir)),
            T0,
            64,
            48,
            Some(60.0),
            false,
            ORACLE_PAGE_ATTRIBUTES,
        )
        .expect("load");
        let mut t = T0;
        for _ in 0..3 {
            t += DT;
            v.date_now = t;
            v.step(t);
        }
        World { v, dir }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        std::fs::remove_dir_all(&self.dir).ok();
    }
}

/// The target-ray pose's `transform.inverse.matrix` of an input source at `position` with `orientation`.
fn ray_inverse(position: Vector3, orientation: Rotation4) -> Matrix4 {
    let mut m = Matrix4::IDENTITY;
    m.translate(&position);
    m.rotate(&orientation);
    m.inverse();
    m
}

/// The size of the hit shape's box, or None for a miss.
fn hit_size(w: &mut World, position: Vector3, orientation: Rotation4) -> Option<f64> {
    let h = w.v.xr_touch(&ray_inverse(position, orientation));

    if !h.id {
        return None;
    }

    let (min, max) = h.bbox.expect("a hit has a bbox");
    Some(max.x - min.x)
}

#[test]
fn the_ray_of_an_input_source_picks_what_it_points_at() {
    let mut w = World::load();
    let ahead = Rotation4::identity();

    // At the viewer, along -Z: the big box with its TouchSensor.
    let h = w.v.xr_touch(&ray_inverse(Vector3::ZERO, ahead));
    assert!(h.id, "no hit straight ahead");
    assert_eq!(h.sensors.len(), 1, "the TouchSensor is not in the hit");
    assert!((h.bbox.unwrap().1.x - h.bbox.unwrap().0.x - 2.0).abs() < 1e-9);

    // Moved 4 to the right: the small box (no sensor).
    assert_eq!(
        hit_size(&mut w, Vector3::new(4.0, 0.0, 0.0), ahead),
        Some(1.0)
    );
    let h =
        w.v.xr_touch(&ray_inverse(Vector3::new(4.0, 0.0, 0.0), ahead));
    assert!(h.sensors.is_empty());

    // Between the boxes, and far to the right: nothing.
    assert_eq!(hit_size(&mut w, Vector3::new(2.25, 0.0, 0.0), ahead), None);
    assert_eq!(hit_size(&mut w, Vector3::new(10.0, 0.0, 0.0), ahead), None);

    // At x 4 but turned 90 degrees to the left (along -X): the ray passes 10 in front of the big box and misses both.
    let left = Rotation4::new(0.0, 1.0, 0.0, std::f64::consts::FRAC_PI_2);
    assert_eq!(hit_size(&mut w, Vector3::new(4.0, 0.0, 0.0), left), None);

    // Behind the big box at z -10 in the viewer's space, turned around (along +Z): the ray hits it from behind.
    let back = Rotation4::new(0.0, 1.0, 0.0, std::f64::consts::PI);
    assert_eq!(
        hit_size(&mut w, Vector3::new(0.0, 0.0, -20.0), back),
        Some(2.0)
    );
}
