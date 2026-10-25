//! One device for the frame, the picks and the collision passes (lane XITE_R_B25_0_SINGLE_GPU_DEVICE): the embedded
//! viewer's pointing buffer and collision depth buffer are on its renderer's device, a reload and an unload hand them
//! back, viewers on one device keep their own, buffers of two devices are not put together, and a lost device makes
//! the viewer pick and collide on the CPU instead of failing.
//!
//! Newly authored world in a temp directory; time is virtual. Without an adapter the tests are skipped.

mod common;

use common::gpu::device;
use std::cell::RefCell;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use x3d_fields::math::Vector3;
use x3d_render::depth_gpu::DepthGpu;
use x3d_render::pointing_gpu::PointingGpu;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;
const W: u32 = 96;
const H: u32 = 72;

/// A WALK avatar on a floor, 4 in front of a wall with a TouchSensor; a QUAD particle at the left.
fn world(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("x3d-gpu-device-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    let p = d.join("w.x3dv");
    std::fs::write(
        &p,
        "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\nNavigationInfo { type \"WALK\" }\n\
         Viewpoint { position 0 1.6 4 }\n\
         DEF FLOOR Shape { geometry Box { size 8 0.1 8 } }\n\
         Group { children [ DEF TS TouchSensor { } Transform { translation 0 1.6 0 children DEF WALL Shape { \
         geometry Box { size 1 1 0.1 } } } ] }\n\
         Transform { translation -1.2 1.6 0 children DEF PS ParticleSystem { geometryType \"QUAD\" maxParticles 1 \
         particleLifetime 0.25 lifetimeVariation 0 particleSize 0.4 0.4 emitter PointEmitter { direction 0 1 0 \
         speed 0 variation 0 } } }\n",
    )
    .unwrap();
    p
}

fn load(v: &mut EmbeddedViewer, p: &Path) {
    v.load(
        p,
        LoadOptions {
            root: None,
            start_time: Some(T0),
        },
    )
    .unwrap();
}

fn steps(v: &mut EmbeddedViewer, from: u32, to: u32) {
    for k in from..to {
        v.step(T0 + k as f64 * DT);
    }
}

/// The DEF name of the shape node under (x, y) (pointer coordinates, y up) and the hit point, or "" for no hit.
fn hit(v: &mut EmbeddedViewer, x: f64, y: f64) -> (String, Vector3) {
    let core = v.core().unwrap();
    let h = core.probe_pick(x, y);
    let name = match (&h.shape, h.id) {
        (Some(s), true) => x3d_viewer::walk::rt_at(&core.rt, &s.path)
            .map(|rt| rt.scene.nodes[s.node as usize].name.clone())
            .unwrap_or_default(),
        _ => String::new(),
    };

    (name, h.point)
}

/// The wall, the particle and the floor under the pointer, and the sky.
const PROBES: [(f64, f64); 4] = [(48.5, 36.5), (22.5, 36.5), (48.5, 2.5), (48.5, 70.5)];

fn names(v: &mut EmbeddedViewer) -> Vec<String> {
    PROBES.iter().map(|&(x, y)| hit(v, x, y).0).collect()
}

fn pointing(v: &mut EmbeddedViewer) -> Option<&PointingGpu> {
    v.core().unwrap().pointing_gpu()
}

fn depth(v: &mut EmbeddedViewer) -> Option<Rc<RefCell<DepthGpu>>> {
    v.core().unwrap().depth_gpu().cloned()
}

#[test]
fn picks_and_collision_use_the_renderer_device() {
    spawn_big_stack(|| {
        let Some((d, q)) = device() else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        let p = world("device");
        let mut v = EmbeddedViewer::new(d.clone(), q.clone(), (W, H));
        assert!(v.gpu_picking());
        load(&mut v, &p);
        steps(&mut v, 1, 20);

        assert!(pointing(&mut v).unwrap().device() == &d);
        let gpu_depth = depth(&mut v).unwrap();
        assert!(gpu_depth.borrow().device() == &d);
        assert!(v.renderer().device() == &d);

        // The WALK avatar's collision and gravity drew the depth passes; the picks drew the pointing pass.
        assert!(gpu_depth.borrow().passes > 0);
        let sampled = pointing(&mut v).unwrap().samples;
        let on_gpu = names(&mut v);
        assert_eq!(on_gpu, ["WALL", "PS", "FLOOR", ""]);
        assert_eq!(pointing(&mut v).unwrap().samples, sampled + PROBES.len());
        let closest = v
            .core()
            .unwrap()
            .probe_closest(Vector3::new(0.0, -1.0, 0.0));
        assert!(closest.hit);

        // The CPU ray cast and depth stand-in pick the same shapes.
        v.set_gpu_picking(false);
        v.reload().unwrap();
        steps(&mut v, 1, 20);
        assert!(pointing(&mut v).is_none() && depth(&mut v).is_none());
        assert_eq!(names(&mut v), on_gpu);
        let cpu = v
            .core()
            .unwrap()
            .probe_closest(Vector3::new(0.0, -1.0, 0.0));
        assert_eq!(cpu.hit, closest.hit);
        assert_eq!(cpu.shape, closest.shape);
        assert!(
            (cpu.distance - closest.distance).abs() < 1e-3,
            "{} {}",
            cpu.distance,
            closest.distance
        );
    });
}

#[test]
fn reload_and_unload_hand_the_buffers_back() {
    spawn_big_stack(|| {
        let Some((d, q)) = device() else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        let p = world("reload");
        let mut v = EmbeddedViewer::new(d, q, (W, H));
        load(&mut v, &p);
        steps(&mut v, 1, 20);
        let first = depth(&mut v).unwrap();
        names(&mut v);
        let samples = pointing(&mut v).unwrap().samples;
        assert!(first.borrow().resources() > 0);

        // The same buffers again, without the old world's vertex buffers.
        v.reload().unwrap();
        let again = depth(&mut v).unwrap();
        assert!(Rc::ptr_eq(&first, &again));
        assert_eq!(again.borrow().resources(), 0);
        assert_eq!(pointing(&mut v).unwrap().samples, samples);
        steps(&mut v, 1, 20);
        assert_eq!(names(&mut v), ["WALL", "PS", "FLOOR", ""]);
        drop(again);

        // Unloaded, the viewer keeps them for the next world, and no world holds the depth buffer.
        v.unload();
        assert_eq!(Rc::strong_count(&first), 2);
        load(&mut v, &p);
        assert!(Rc::ptr_eq(&first, &depth(&mut v).unwrap()));
        steps(&mut v, 1, 20);
        assert_eq!(names(&mut v), ["WALL", "PS", "FLOOR", ""]);

        // Dropped, the viewer frees them.
        drop(v);
        assert_eq!(Rc::strong_count(&first), 1);
    });
}

#[test]
fn viewers_on_one_device_keep_their_own_buffers() {
    spawn_big_stack(|| {
        let Some((d, q)) = device() else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        let p = world("viewers");
        let (mut a, mut b) = (
            EmbeddedViewer::new(d.clone(), q.clone(), (W, H)),
            EmbeddedViewer::new(d, q, (W, H)),
        );
        load(&mut a, &p);
        load(&mut b, &p);
        steps(&mut a, 1, 20);
        steps(&mut b, 1, 20);
        let (da, db) = (depth(&mut a).unwrap(), depth(&mut b).unwrap());
        assert!(!Rc::ptr_eq(&da, &db));

        // A's picks and collision passes do not touch B's buffers, and A's unload leaves B's vertex buffers.
        let (samples, passes, resources) = (
            pointing(&mut b).unwrap().samples,
            db.borrow().passes,
            db.borrow().resources(),
        );
        assert_eq!(names(&mut a), ["WALL", "PS", "FLOOR", ""]);
        steps(&mut a, 20, 30);
        a.unload();
        assert_eq!(pointing(&mut b).unwrap().samples, samples);
        assert_eq!(
            (db.borrow().passes, db.borrow().resources()),
            (passes, resources)
        );
        assert_eq!(names(&mut b), ["WALL", "PS", "FLOOR", ""]);
    });
}

#[test]
fn buffers_of_two_devices_are_refused() {
    spawn_big_stack(|| {
        let (Some((d1, q1)), Some((d2, q2))) = (device(), device()) else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        let p = world("two-devices");
        let mut v = EmbeddedViewer::new(d1.clone(), q1.clone(), (W, H));
        load(&mut v, &p);
        let core = v.core().unwrap();

        // One buffer on the frame's device, one on another.
        let e = core
            .set_gpu(
                &d1,
                Some(PointingGpu::new(d1.clone(), q1.clone())),
                Some(Rc::new(RefCell::new(DepthGpu::new(d2.clone(), q2.clone())))),
            )
            .unwrap_err();
        assert!(e.contains("collision depth buffer"), "{e}");

        // Both on one device, but not the one that draws the frame: agreeing with each other is not enough.
        let e = core
            .set_gpu(
                &d1,
                Some(PointingGpu::new(d2.clone(), q2.clone())),
                Some(Rc::new(RefCell::new(DepthGpu::new(d2.clone(), q2.clone())))),
            )
            .unwrap_err();
        assert!(e.contains("pointing buffer"), "{e}");

        // The depth buffer alone, on the other device.
        let e = core
            .set_gpu(
                &d1,
                None,
                Some(Rc::new(RefCell::new(DepthGpu::new(d2, q2)))),
            )
            .unwrap_err();
        assert!(e.contains("collision depth buffer"), "{e}");

        // The viewer's own buffers stay, and they are the renderer's.
        assert!(core.pointing_gpu().is_some() && core.depth_gpu().is_some());
        assert!(core.pointing_gpu().unwrap().device() == &d1);
        assert!(core.depth_gpu().unwrap().borrow().device() == &d1);
    });
}

#[test]
fn a_lost_device_falls_back_to_the_cpu() {
    spawn_big_stack(|| {
        let Some((d, q)) = device() else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        let p = world("lost");
        let mut v = EmbeddedViewer::new(d.clone(), q, (W, H));
        load(&mut v, &p);
        steps(&mut v, 1, 20);
        assert_eq!(names(&mut v), ["WALL", "PS", "FLOOR", ""]);

        // The ray cast's and the depth stand-in's answers in this state: without the GPU buffers for a moment.
        let core = v.core().unwrap();
        let (lent, depth_buffer) = core.take_gpu();
        let want: Vec<(String, Vector3)> = PROBES.iter().map(|&(x, y)| hit(&mut v, x, y)).collect();
        let down = Vector3::new(0.0, -1.0, 0.0);
        let want_closest = v.core().unwrap().probe_closest(down);
        v.core().unwrap().set_gpu(&d, lent, depth_buffer).unwrap();

        // The errors of the destroyed device go to the host (like the window's GpuErrors), not to wgpu's panic.
        d.on_uncaptured_error(std::sync::Arc::new(|_| {}));
        d.destroy();

        // A destroyed device still polls clean, so the first failed readback reads as the mapping's own: that pick
        // answers with the ray cast, and the buffer is kept rather than written off on one failure.
        let first = hit(&mut v, PROBES[0].0, PROBES[0].1);
        assert_eq!(first, want[0]);
        let p = pointing(&mut v).expect("one failed mapping does not give the device up");
        assert_eq!((p.lost(), p.readback_failures()), (false, 1));

        // Every pick keeps answering with the ray cast, and a run of failed mappings does give it up.
        let got: Vec<(String, Vector3)> = PROBES.iter().map(|&(x, y)| hit(&mut v, x, y)).collect();
        assert_eq!(got, want);
        assert!(pointing(&mut v).is_none());

        // The next collision pass finds it lost too: the depth stand-in answers, and the next frame drops the buffer.
        let closest = v.core().unwrap().probe_closest(down);
        assert_eq!(
            (closest.hit, closest.shape, closest.distance),
            (want_closest.hit, want_closest.shape, want_closest.distance)
        );
        let before = v.core().unwrap().user_pose().0;
        steps(&mut v, 20, 30);
        assert!(depth(&mut v).is_none());
        assert_eq!(
            v.core().unwrap().user_pose().0,
            before,
            "the avatar stands on the floor"
        );

        // A reload on the lost device stays on the CPU, and the viewer shuts down.
        v.reload().unwrap();
        steps(&mut v, 1, 20);
        assert!(pointing(&mut v).is_none() && depth(&mut v).is_none());
        assert_eq!(names(&mut v), ["WALL", "PS", "FLOOR", ""]);
        drop(v);
    });
}

/// Turning GPU picking off and on again is not a lost device (MiniMax P1 on #41): enable, load, disable, reload,
/// enable, reload — and the buffers are back on the renderer's device.
#[test]
fn gpu_picking_comes_back_after_a_disabled_reload() {
    spawn_big_stack(|| {
        let Some((d, q)) = device() else {
            eprintln!("skipped: no GPU adapter");
            return;
        };
        let p = world("off-on");
        let mut v = EmbeddedViewer::new(d.clone(), q, (W, H));
        assert!(v.gpu_picking(), "on by default");
        load(&mut v, &p);
        steps(&mut v, 1, 20);
        assert!(pointing(&mut v).is_some() && depth(&mut v).is_some());
        assert_eq!(names(&mut v), ["WALL", "PS", "FLOOR", ""]);

        // Off, then a reload while off: the world gives the buffers back and the viewer drops them.
        v.set_gpu_picking(false);
        v.reload().unwrap();
        steps(&mut v, 1, 20);
        assert!(pointing(&mut v).is_none() && depth(&mut v).is_none());
        assert_eq!(
            names(&mut v),
            ["WALL", "PS", "FLOOR", ""],
            "the ray cast answers while the GPU is off"
        );

        // On again, and a reload: nothing was lost, so the viewer makes them again on its renderer's device.
        v.set_gpu_picking(true);
        v.reload().unwrap();
        steps(&mut v, 1, 20);
        let back = {
            let g = pointing(&mut v).expect("GPU picking comes back after a disabled reload");
            (g.device() == &d, g.lost())
        };
        assert_eq!(back, (true, false));
        assert!(depth(&mut v).is_some());
        assert_eq!(names(&mut v), ["WALL", "PS", "FLOOR", ""]);
    });
}
