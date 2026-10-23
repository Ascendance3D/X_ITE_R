//! ParticleSystem avatar collision (lane XITE_R_PARTICLE_AVATAR_COLLISION_02) beyond the X_ITE probes of
//! rea_b15_particle_collision: independent viewers, an Inline that loads and unloads the particles, and the vertex
//! buffers of the collision depth buffer. Newly authored worlds in a temp directory; time is virtual.
//!
//! The collision depth buffer needs a GPU: without an adapter the tests are skipped.

use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use x3d_fields::FieldValue;
use x3d_fields::math::Vector3;
use x3d_render::depth_gpu::DepthGpu;
use x3d_scene::ROOT;
use x3d_viewer::viewer::Viewer3d;

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;

fn big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!(
        "x3d-particle-collision-{}-{tag}",
        std::process::id()
    ));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(d: &Path, name: &str, text: &str) -> PathBuf {
    let p = d.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

const HEAD: &str = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\n";

/// A FLY avatar at the origin; RUN keeps the COLLISION traversal running (FlyViewer.isCollidable is FALSE at rest).
const FLY: &str =
    "NavigationInfo { type \"FLY\" }\nViewpoint { position 0 0 0 }\nDEF RUN Collision { }\n";

/// A QUAD particle 2 x 2 standing still 0.3 in front of the avatar.
const WALL: &str = "DEF PS ParticleSystem { geometryType \"QUAD\" maxParticles 1 particleLifetime 0.25 \
                    lifetimeVariation 0 particleSize 2 2 emitter PointEmitter { position 0 0 -0.3 direction 0 1 0 \
                    speed 0 variation 0 } appearance Appearance { material Material { } } }\n";

/// A WALK avatar 2.5 above a horizontal QUAD particle floor.
const FLOOR: &str = "NavigationInfo { type \"WALK\" }\nViewpoint { position 0 2.5 0 }\n\
                     Transform { rotation 1 0 0 -1.5707963 children DEF PS ParticleSystem { geometryType \"QUAD\" \
                     maxParticles 1 particleLifetime 0.25 lifetimeVariation 0 particleSize 4 4 emitter PointEmitter { \
                     direction 0 1 0 speed 0 variation 0 } appearance Appearance { material Material { } } } }\n";

struct Run {
    v: Viewer3d,
    depth: Rc<RefCell<DepthGpu>>,
    frame: u64,
}

impl Run {
    fn new(world: &Path, depth: Rc<RefCell<DepthGpu>>) -> Run {
        let mut v = Viewer3d::load(world, None, T0, 256, 256).unwrap();
        v.frame_rate = 1.0 / DT;
        v.depth_gpu = Some(depth.clone());
        Run { v, depth, frame: 0 }
    }

    fn step(&mut self) {
        self.frame += 1;
        let now = T0 + self.frame as f64 * DT;
        self.v.date_now = now;
        self.v.step(now);
    }

    fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.step();
        }
    }

    /// The closest object straight ahead: (hit, distance).
    fn ahead(&mut self) -> (bool, f64) {
        let c = self.v.probe_closest(Vector3::new(0.0, 0.0, -1.0));
        (c.hit, c.distance)
    }

    fn height(&mut self) -> f64 {
        self.v.user_pose().0.y
    }
}

fn depth_gpu() -> Option<Rc<RefCell<DepthGpu>>> {
    let g = x3d_render::pointing_gpu::PointingGpu::new_headless().ok()?;

    Some(Rc::new(RefCell::new(DepthGpu::new(
        g.device().clone(),
        g.queue().clone(),
    ))))
}

/// Two viewers on two worlds, stepped in turn, see what each sees alone: the wall 0.3 ahead blocks a move, the floor
/// stops the fall at the avatar height (1.6), and neither moves the other's avatar.
#[test]
fn independent_viewers() {
    big(|| {
        let Some(depth) = depth_gpu() else {
            return eprintln!("no adapter: the collision depth buffer needs a GPU, skipped");
        };
        let d = dir("viewers");
        let wall = write(&d, "wall.x3dv", &format!("{HEAD}{FLY}{WALL}"));
        let floor = write(&d, "floor.x3dv", &format!("{HEAD}{FLOOR}"));

        let alone = |world: &Path| {
            let mut r = Run::new(world, depth.clone());
            let mut out = vec![];

            for _ in 0..30 {
                r.step();
                out.push((r.ahead(), r.height()));
            }

            out
        };
        let wall_alone = alone(&wall);
        let floor_alone = alone(&floor);

        // Each viewer with its own depth buffer, stepped in turn.
        let mut a = Run::new(&wall, depth_gpu().unwrap());
        let mut b = Run::new(&floor, depth_gpu().unwrap());
        let (mut wall_both, mut floor_both) = (vec![], vec![]);

        for _ in 0..30 {
            a.step();
            b.step();
            wall_both.push((a.ahead(), a.height()));
            floor_both.push((b.ahead(), b.height()));
        }

        assert_eq!(wall_alone, wall_both);
        assert_eq!(floor_alone, floor_both);

        let ((hit, distance), height) = *wall_both.last().unwrap();
        assert!(hit && (distance - 0.3).abs() < 1e-6, "wall {distance}");
        assert_eq!(height, 0.0);

        // The wall stops a move of 0.1 at 0.05 (0.3 - collisionRadius 0.25).
        let blocked = a.v.probe_constrain(Vector3::new(0.0, 0.0, -0.1));
        assert!((blocked.z + 0.05).abs() < 1e-6, "blocked {blocked:?}");

        let height = floor_both.last().unwrap().1;
        assert!((height - 1.6).abs() < 1e-6, "floor {height}");
    });
}

/// An Inline loads the wall, unloads it and loads it again: the wall blocks only while it is loaded. The depth buffer
/// keeps one vertex buffer for the particle quad while it runs (no new buffer per collision test or frame); unloaded,
/// nothing is drawn and nothing is added; the reloaded system's quad is a new geometry, and the old one's buffer goes
/// once KEEP (600) queries have passed without it, back to one.
#[test]
fn inline_reload_and_resources() {
    big(|| {
        let Some(depth) = depth_gpu() else {
            return eprintln!("no adapter: the collision depth buffer needs a GPU, skipped");
        };
        let d = dir("inline");
        write(&d, "wall.x3dv", &format!("{HEAD}{WALL}"));
        let world = write(
            &d,
            "world.x3dv",
            &format!("{HEAD}{FLY}DEF IN Inline {{ url \"wall.x3dv\" }}\n"),
        );
        let mut r = Run::new(&world, depth);
        let inline = r.v.rt.scene.contexts[ROOT].named_nodes.get("IN").unwrap();
        let set_load = |r: &mut Run, load: bool| {
            let k = r.v.rt.get_field(inline, "load").unwrap();
            r.v.rt.set_value(k, FieldValue::SFBool(load));
        };

        r.steps(20);
        assert!(r.ahead().0, "the loaded wall is hit");
        let passes = r.depth.borrow().passes;
        let held = r.depth.borrow().resources();
        assert_eq!(held, 1, "one vertex buffer: the particle quad");

        // Every frame draws the six axes of X3DRenderObject.collide; the buffer stays the same one.
        r.steps(50);
        assert!(r.depth.borrow().passes >= passes + 300);
        assert_eq!(r.depth.borrow().resources(), held);

        set_load(&mut r, false);
        r.steps(5);
        assert!(!r.ahead().0, "the unloaded wall is not hit");
        assert_eq!(r.v.probe_collision_shapes().0.len(), 0);
        r.steps(50);
        assert_eq!(r.depth.borrow().resources(), held);

        set_load(&mut r, true);
        r.steps(20);
        let (hit, distance) = r.ahead();
        assert!(
            hit && (distance - 0.3).abs() < 1e-6,
            "reloaded wall {distance}"
        );
        assert!(r.depth.borrow().resources() <= 2);

        // Six queries a frame: after 110 frames the unloaded system's buffer is gone, the reloaded one's stays.
        r.steps(110);
        assert_eq!(r.depth.borrow().resources(), 1);

        r.depth.borrow_mut().release();
        assert_eq!(r.depth.borrow().resources(), 0);
        r.step();
        assert_eq!(r.depth.borrow().resources(), 1);

        // The viewer goes: its collision state with it, the depth buffer keeps nothing beyond its KEEP window.
        drop(r);
    });
}
