//! ParticleSystem pointing in the embedded viewer (lane XITE_R_PARTICLE_PICKING_01): the cases X_ITE's single-canvas
//! reference (rea_b14_particle_pick.rs) cannot show. Two viewers keep their particle hits apart, a reload and an unload
//! leave no stale particle candidate, a TouchSensor around a ParticleSystem sees the pointer over a particle, and picking
//! creates no renderer resource.
//!
//! The embedded viewer picks with the pointing buffer on its renderer's device, so these are the interactive viewer's hits.
//! The worlds' particles stand still (PointEmitter speed 0, no variation): the emitters' random draws, which are shared
//! on the thread, do not move them.

mod common;

use common::gpu::device;
use std::path::{Path, PathBuf};
use x3d_fields::FieldValue;
use x3d_scene::ROOT;
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 30.0;
const W: u32 = 96;
const H: u32 = 72;

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("x3d-particle-pick-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

/// One QUAD particle system named PS at `x`, inside a group with a TouchSensor TS; with `boxed` a Shape BOX at the
/// center behind it.
fn world(d: &Path, x: f64, boxed: bool) -> PathBuf {
    let p = d.join("w.x3dv");
    let shape = if boxed {
        "Transform { translation 0 0 -1 children DEF BOX Shape { geometry Box { size 1 1 0.1 } } }\n"
    } else {
        ""
    };

    std::fs::write(
        &p,
        format!(
            "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\nViewpoint {{ position 0 0 3 }}\n{shape}\
             Transform {{ translation {x} 0 0 children Group {{ children [\n  DEF TS TouchSensor {{ }}\n  DEF PS ParticleSystem {{\n\
             geometryType \"QUAD\" maxParticles 1 particleLifetime 0.25 lifetimeVariation 0 particleSize 0.8 0.8\n\
             emitter PointEmitter {{ direction 0 1 0 speed 0 variation 0 }}\n  }}\n] }} }}\n"
        ),
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

/// The DEF name of the shape node under (x, y) (pointer coordinates, y up), or "" for no hit.
fn hit_name(v: &mut EmbeddedViewer, x: f64, y: f64) -> String {
    let core = v.core().unwrap();
    let h = core.probe_pick(x, y);

    match (&h.shape, h.id) {
        (Some(s), true) => x3d_viewer::walk::rt_at(&core.rt, &s.path)
            .map(|rt| rt.scene.nodes[s.node as usize].name.clone())
            .unwrap_or_default(),
        _ => String::new(),
    }
}

/// The hits of a row of probes across the canvas, with their points, for exact comparisons between runs.
fn row(v: &mut EmbeddedViewer) -> Vec<(String, [u64; 3])> {
    (0..W)
        .map(|x| {
            let core = v.core().unwrap();
            let h = core.probe_pick(x as f64 + 0.5, H as f64 / 2.0 + 0.5);
            let name = h
                .shape
                .as_ref()
                .filter(|_| h.id)
                .and_then(|s| {
                    x3d_viewer::walk::rt_at(&core.rt, &s.path)
                        .map(|rt| rt.scene.nodes[s.node as usize].name.clone())
                })
                .unwrap_or_default();

            (
                name,
                [
                    h.point.x.to_bits(),
                    h.point.y.to_bits(),
                    h.point.z.to_bits(),
                ],
            )
        })
        .collect()
}

fn set_bool(v: &mut EmbeddedViewer, node: &str, field: &str, value: bool) {
    let rt = &mut v.core().unwrap().rt;
    let n = rt.scene.contexts[ROOT].named_nodes.get(node).unwrap();
    let k = rt.get_field(n, field).unwrap();

    rt.set_value(k, FieldValue::SFBool(value));
}

const CENTER: (f64, f64) = (W as f64 / 2.0 + 0.5, H as f64 / 2.0 + 0.5);

/// Two viewers, interleaved: each one's probes equal those of the same world alone, bit for bit, and a probe over the
/// other viewer's particles hits nothing of them.
#[test]
fn particle_hits_stay_per_viewer() {
    let Some((d, q)) = device() else {
        return eprintln!("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let (a, b) = (world(&dir("a"), 0.0, false), world(&dir("b"), 1.0, true));

        let mut alone = EmbeddedViewer::new(d.clone(), q.clone(), (W, H));
        load(&mut alone, &a);
        steps(&mut alone, 1, 20);
        let a_alone = row(&mut alone);
        drop(alone);

        let mut alone = EmbeddedViewer::new(d.clone(), q.clone(), (W, H));
        load(&mut alone, &b);
        steps(&mut alone, 1, 20);
        let b_alone = row(&mut alone);
        drop(alone);

        let (mut va, mut vb) = (
            EmbeddedViewer::new(d.clone(), q.clone(), (W, H)),
            EmbeddedViewer::new(d, q, (W, H)),
        );
        load(&mut va, &a);
        load(&mut vb, &b);

        for k in 1..20 {
            va.step(T0 + k as f64 * DT);
            vb.step(T0 + k as f64 * DT);
        }

        assert_eq!(
            hit_name(&mut va, CENTER.0, CENTER.1),
            "PS",
            "A's particle is at its center"
        );
        assert_eq!(
            hit_name(&mut vb, CENTER.0, CENTER.1),
            "BOX",
            "B has no particle at its center, its Box is behind it"
        );
        assert!(
            a_alone.iter().any(|h| h.0 == "PS") && b_alone.iter().any(|h| h.0 == "PS"),
            "both rows cross a particle"
        );
        assert!(row(&mut va) == a_alone, "A's hits equal A alone");
        assert!(row(&mut vb) == b_alone, "B's hits equal B alone");
    });
}

/// enabled FALSE and visible FALSE take the particles out of the pointer's walk, a reload brings them back, and after
/// an unload and a load of another world no hit names the old world's ParticleSystem.
#[test]
fn particle_hits_follow_state_reload_and_unload() {
    let Some((d, q)) = device() else {
        return eprintln!("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let (a, b) = (
            world(&dir("r-a"), 0.0, false),
            world(&dir("r-b"), 1.0, true),
        );
        let mut v = EmbeddedViewer::new(d, q, (W, H));

        load(&mut v, &a);
        steps(&mut v, 1, 20);
        assert_eq!(hit_name(&mut v, CENTER.0, CENTER.1), "PS");

        set_bool(&mut v, "PS", "visible", false);
        steps(&mut v, 20, 22);
        assert_eq!(
            hit_name(&mut v, CENTER.0, CENTER.1),
            "",
            "an invisible ParticleSystem is no pointing object"
        );

        set_bool(&mut v, "PS", "visible", true);
        set_bool(&mut v, "PS", "pointerEvents", false);
        steps(&mut v, 22, 24);
        assert_eq!(
            hit_name(&mut v, CENTER.0, CENTER.1),
            "",
            "pointerEvents FALSE"
        );

        set_bool(&mut v, "PS", "pointerEvents", true);
        set_bool(&mut v, "PS", "enabled", false);
        steps(&mut v, 24, 26);
        assert_eq!(
            hit_name(&mut v, CENTER.0, CENTER.1),
            "",
            "a disabled ParticleSystem has no particles"
        );

        for _ in 0..3 {
            v.reload().unwrap();
            steps(&mut v, 1, 20);
            assert_eq!(
                hit_name(&mut v, CENTER.0, CENTER.1),
                "PS",
                "the reloaded world's particle"
            );
        }

        v.unload();
        assert!(v.core().is_none());

        load(&mut v, &b);
        steps(&mut v, 1, 20);
        assert_eq!(
            hit_name(&mut v, CENTER.0, CENTER.1),
            "BOX",
            "the old world's particle is gone"
        );
    });
}

/// The pointer over a particle: the TouchSensor next to the ParticleSystem gets isOver TRUE and a hit point on the
/// particle's plane; a click sets touchTime. Picking adds no renderer resource.
#[test]
fn touch_sensor_over_particles() {
    let Some((d, q)) = device() else {
        return eprintln!("no adapter, skipped");
    };
    spawn_big_stack(move || {
        let a = world(&dir("t"), 0.0, false);
        let mut v = EmbeddedViewer::new(d, q, (W, H));

        load(&mut v, &a);
        steps(&mut v, 1, 20);

        let before = v.renderer().resource_counts();

        for _ in 0..200 {
            hit_name(&mut v, CENTER.0, CENTER.1);
        }

        assert_eq!(
            v.renderer().resource_counts(),
            before,
            "picking creates no renderer resource"
        );

        // UiEvent coordinates are window pixels, y down; the center is the same both ways.
        v.input(x3d_viewer::viewer::UiEvent::MouseMove {
            x: W as f64 / 2.0 + 0.5,
            y: H as f64 / 2.0 - 0.5,
        });
        steps(&mut v, 20, 22);

        let rt = &v.core().unwrap().rt;
        let ts = rt.scene.contexts[ROOT].named_nodes.get("TS").unwrap();
        assert!(
            rt.get_bool(ts, "isOver"),
            "the pointer is over the particle"
        );

        match rt.pre_value(ts, "hitPoint_changed") {
            Some(FieldValue::SFVec3f(p)) => assert!(
                p.z.abs() < 1e-6 && p.x.abs() < 0.4 && p.y.abs() < 0.4,
                "hit point on the particle: {p:?}"
            ),
            other => panic!("hitPoint_changed: {other:?}"),
        }

        v.input(x3d_viewer::viewer::UiEvent::MouseDown {
            button: 0,
            x: W as f64 / 2.0 + 0.5,
            y: H as f64 / 2.0 - 0.5,
        });
        steps(&mut v, 22, 23);
        v.input(x3d_viewer::viewer::UiEvent::MouseUp {
            button: 0,
            x: W as f64 / 2.0 + 0.5,
            y: H as f64 / 2.0 - 0.5,
        });
        steps(&mut v, 23, 24);

        let rt = &v.core().unwrap().rt;
        let ts = rt.scene.contexts[ROOT].named_nodes.get("TS").unwrap();
        assert!(
            matches!(rt.pre_value(ts, "touchTime"), Some(FieldValue::SFTime(t)) if *t > T0),
            "a click on the particle sets touchTime"
        );

        // Away from the particle: not over.
        v.input(x3d_viewer::viewer::UiEvent::MouseMove { x: 2.5, y: 2.5 });
        steps(&mut v, 24, 26);

        let rt = &v.core().unwrap().rt;
        let ts = rt.scene.contexts[ROOT].named_nodes.get("TS").unwrap();
        assert!(!rt.get_bool(ts, "isOver"), "the pointer left the particle");
    });
}
