//! ParticleSystem avatar collision (lane XITE_R_PARTICLE_AVATAR_COLLISION_02) beyond the X_ITE probes of
//! rea_b15_particle_collision: independent viewers, an Inline that loads and unloads the particles, and the vertex
//! buffers of the collision depth buffer. Newly authored worlds in a temp directory; time is virtual.
//!
//! The collision depth buffer needs a GPU: without an adapter the tests are skipped, except the CPU stand-in's.

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
    depth: Option<Rc<RefCell<DepthGpu>>>,
    frame: u64,
}

impl Run {
    fn new(world: &Path, depth: Rc<RefCell<DepthGpu>>) -> Run {
        Run::with(world, Some(depth))
    }

    /// Without a depth buffer the CPU stand-in answers.
    fn with(world: &Path, depth: Option<Rc<RefCell<DepthGpu>>>) -> Run {
        let mut v = Viewer3d::load(world, None, T0, 256, 256).unwrap();
        v.frame_rate = 1.0 / DT;
        if let Some(d) = depth.clone() {
            let device = d.borrow().device().clone();
            v.set_gpu(&device, None, Some(d)).unwrap();
        }
        Run { v, depth, frame: 0 }
    }

    fn gpu(&self) -> &Rc<RefCell<DepthGpu>> {
        self.depth.as_ref().unwrap()
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

/// POINT and LINE particles crossing in front of a FLY avatar at the origin, 0.3 away (as in pc-point and pc-line).
fn crossing(geometry: &str) -> String {
    format!(
        "{HEAD}{FLY}DEF PS ParticleSystem {{ geometryType \"{geometry}\" maxParticles 20 particleLifetime 1.6 \
         lifetimeVariation 0 particleSize 0.1 0.15 emitter PointEmitter {{ position -0.4 0.011 -0.3 direction 1 0 0 \
         speed 0.5 variation 0 }} appearance Appearance {{ material Material {{ }} }} }}\n"
    )
}

/// The closest object in four directions: (hit, distance) each.
fn around(r: &mut Run) -> Vec<(bool, f64)> {
    [
        (0.0, 0.0, -1.0),
        (1.0, 0.0, 0.0),
        (-0.6, 0.0, -0.8),
        (0.6, 0.0, -0.8),
    ]
    .into_iter()
    .map(|(x, y, z)| {
        let c = r.v.probe_closest(Vector3::new(x, y, z));
        (c.hit, c.distance)
    })
    .collect()
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

/// X3DRenderObject.collide's six closest objects and gravitate's depth read the same shapes from the same viewpoint:
/// one batch, one submission and one readback a frame. The avatar still falls to the floor and stands there.
#[test]
fn collide_and_gravitate_are_one_batch() {
    big(|| {
        let Some(depth) = depth_gpu() else {
            return eprintln!("no adapter: the collision depth buffer needs a GPU, skipped");
        };
        let d = dir("batch");
        let floor = write(
            &d,
            "floor.x3dv",
            &format!("{HEAD}{FLOOR}DEF RUN Collision {{ }}\n"),
        );
        let mut r = Run::new(&floor, depth);

        // While the avatar is high above the floor, the floor particle is outside the COLLISION traversal's volume:
        // gravitate alone, with nothing to draw, needs no submission. Once it is in, the frame is one batch.
        let mut batches = 0;

        for _ in 0..40 {
            r.gpu().borrow_mut().take_stats();
            r.step();
            let stats = r.gpu().borrow_mut().take_stats();
            // gravitate alone, or collide's six axes with it.
            assert!(
                matches!(stats.queries, 1 | 7),
                "frame {}: {stats:?}",
                r.frame
            );
            let batch = stats.queries == 7;
            assert!(
                batch || batches == 0,
                "frame {}: the floor left the collision shapes",
                r.frame
            );
            batches += batch as usize;
            let want = if batch { (1, 1, 1) } else { (0, 0, 0) };

            assert_eq!(
                (stats.submissions, stats.polls, stats.readbacks),
                want,
                "frame {}",
                r.frame
            );
        }

        assert!(batches >= 30, "{batches} frames with the floor");
        let height = r.height();
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
        let passes = r.gpu().borrow().passes;
        let held = r.gpu().borrow().resources();
        assert_eq!(held, 1, "one vertex buffer: the particle quad");

        // Every frame draws the six axes of X3DRenderObject.collide; the buffer stays the same one.
        r.steps(50);
        assert!(r.gpu().borrow().passes >= passes + 300);
        assert_eq!(r.gpu().borrow().resources(), held);

        set_load(&mut r, false);
        r.steps(5);
        assert!(!r.ahead().0, "the unloaded wall is not hit");
        assert_eq!(r.v.probe_collision_shapes().0.len(), 0);
        r.steps(50);
        assert_eq!(r.gpu().borrow().resources(), held);

        set_load(&mut r, true);
        r.steps(20);
        let (hit, distance) = r.ahead();
        assert!(
            hit && (distance - 0.3).abs() < 1e-6,
            "reloaded wall {distance}"
        );
        assert!(r.gpu().borrow().resources() <= 2);

        // Six queries a frame: after 110 frames the unloaded system's buffer is gone, the reloaded one's stays.
        r.steps(110);
        assert_eq!(r.gpu().borrow().resources(), 1);

        r.gpu().borrow_mut().release();
        assert_eq!(r.gpu().borrow().resources(), 0);
        r.step();
        assert_eq!(r.gpu().borrow().resources(), 1);

        // The viewer goes: its collision state with it, the depth buffer keeps nothing beyond its KEEP window.
        drop(r);
    });
}

/// The CPU stand-in draws POINT and LINE particles: two viewers on a POINT and a LINE world, stepped in turn, see what
/// each sees alone while the particles move across the view, and (with an adapter) what the GPU depth buffer sees.
#[test]
fn cpu_viewers_points_and_lines() {
    big(|| {
        let d = dir("cpu-viewers");
        let worlds = [
            write(&d, "point.x3dv", &crossing("POINT")),
            write(&d, "line.x3dv", &crossing("LINE")),
        ];
        let frames = 50;
        let alone = |world: &Path, depth: Option<Rc<RefCell<DepthGpu>>>| {
            let mut r = Run::with(world, depth);
            (0..frames)
                .map(|_| {
                    r.step();
                    around(&mut r)
                })
                .collect::<Vec<_>>()
        };
        let cpu: Vec<_> = worlds.iter().map(|w| alone(w, None)).collect();

        for (k, frames) in cpu.iter().enumerate() {
            let hits = frames.iter().flatten().filter(|(hit, _)| *hit).count();
            assert!(hits >= 20, "world {k}: {hits} hits");

            for (frame, probes) in frames.iter().enumerate() {
                for (hit, distance) in probes {
                    assert!(
                        !hit || *distance < 0.6,
                        "world {k} frame {frame}: {distance}"
                    );
                }
            }
        }

        let mut a = Run::with(&worlds[0], None);
        let mut b = Run::with(&worlds[1], None);
        let (mut points, mut lines) = (vec![], vec![]);

        for _ in 0..frames {
            a.step();
            b.step();
            points.push(around(&mut a));
            lines.push(around(&mut b));
        }

        assert_eq!(cpu[0], points);
        assert_eq!(cpu[1], lines);

        let Some(depth) = depth_gpu() else {
            return eprintln!("no adapter: CPU and GPU not compared");
        };

        for (k, world) in worlds.iter().enumerate() {
            for (frame, (c, g)) in cpu[k]
                .iter()
                .zip(alone(world, Some(depth.clone())))
                .enumerate()
            {
                for ((ch, cd), (gh, gd)) in c.iter().zip(&g) {
                    assert!(
                        ch == gh && (cd - gd).abs() < 1e-6,
                        "world {k} frame {frame}: CPU {c:?} / GPU {g:?}"
                    );
                }
            }
        }
    });
}
