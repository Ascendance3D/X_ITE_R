//! REA batch 06R + R0.3-E: the lifetime of ParticleSystem state. An Inline holding four ParticleSystems
//! (Surface, Volume, Explosion and Cone emitters, a ForcePhysicsModel, a BoundedPhysicsModel; geometry
//! tables and the BVH built from real geometry) is unloaded and loaded again 100 times through the real
//! runtime. The particle buffers, the `sensorEvents` interests of `animateParticles`, the live watchers
//! and the runtime's tables must go with the nodes: they stay flat after the warm-up, no interest names a
//! node without particle state, and an unloaded scene animates nothing. `MEM` lines at cycles 10, 50 and 100.

mod particles_b06_common;

use particles_b06_common::big;
use std::path::{Path, PathBuf};
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::nodes::State;
use x3d_runtime::{Cb, Runtime};
use x3d_scene::{CtxId, ROOT};
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 60.0;

const HEADER: &str = "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\n";

fn scene(variant: u32) -> String {
    let cube = "coordIndex [ 0 3 2 1 -1 4 5 6 7 -1 0 1 5 4 -1 2 3 7 6 -1 1 2 6 5 -1 0 4 7 3 -1 ] coord Coordinate { point [ -1 -1 -1, 1 -1 -1, 1 1 -1, -1 1 -1, -1 -1 1, 1 -1 1, 1 1 1, -1 1 1 ] }";
    let first = if variant == 0 {
        "SurfaceEmitter { speed 1 surface Box { size 2 2 2 } }".to_string()
    } else {
        "SurfaceEmitter { speed 1 surface IndexedFaceSet { solid FALSE coord Coordinate { point [ 0 0 0, 2 0 0, 2 1 0, 0 1 0 ] } coordIndex [ 0 1 2 3 -1 ] } }".to_string()
    };

    format!(
        "{HEADER}\nGroup {{ children [
  DEF P1 ParticleSystem {{ maxParticles 60 particleLifetime 1 emitter {first}
    physics [ ForcePhysicsModel {{ force 0 -9.8 0 }} BoundedPhysicsModel {{ damping 0.5 geometry Box {{ size 4 4 4 }} }} ] }}
  DEF P2 ParticleSystem {{ maxParticles 30 particleLifetime 2 emitter VolumeEmitter {{ direction 0 1 0 speed 1 {cube} }} }}
  DEF P3 ParticleSystem {{ maxParticles 20 particleLifetime 1 emitter ExplosionEmitter {{ speed 2 }} }}
  DEF P4 ParticleSystem {{ maxParticles 20 particleLifetime 1 emitter ConeEmitter {{ direction 0 1 0 angle 0.3 speed 2 mass 1 }}
    physics [ ForcePhysicsModel {{ force 0 -9.8 0 }} ] }}
] }}\n"
    )
}

fn dir(tag: &str) -> PathBuf {
    let d = std::env::temp_dir().join(format!("x3d-particles-{}-{tag}", std::process::id()));
    std::fs::create_dir_all(&d).unwrap();
    d
}

fn write(d: &Path, name: &str, text: &str) -> PathBuf {
    let p = d.join(name);
    std::fs::write(&p, text).unwrap();
    p
}

fn rss_kib() -> usize {
    std::fs::read_to_string("/proc/self/status")
        .ok()
        .and_then(|s| {
            s.lines().find_map(|l| {
                l.strip_prefix("VmRSS:")
                    .and_then(|r| r.split_whitespace().next())
                    .and_then(|n| n.parse().ok())
            })
        })
        .unwrap_or(0)
}

/// Everything the particles keep in the runtime.
#[derive(Debug, PartialEq, Eq, Clone)]
struct Resources {
    /// Nodes holding ParticleSystem state, and the floats in their particle buffers.
    systems: usize,
    buffer_floats: usize,
    /// Interests of the browser's sensorEvents that animate a ParticleSystem.
    animate_interests: usize,
    live_watchers: usize,
    /// Runtime tables.
    live_nodes: usize,
    arena: usize,
    arena_cap: usize,
    nrt: usize,
    frt: usize,
    contexts: usize,
}

fn resources(rt: &Runtime) -> Resources {
    let mut systems = 0;
    let mut floats = 0;

    for n in &rt.nrt {
        if let State::ParticleSystem(_) = n.state {
            systems += 1;
        }
    }

    for node in 0..rt.nrt.len() as NodeId {
        if let Some(sim) = rt.particle_sim(node) {
            floats += sim.particles.len() * x3d_runtime::particles::STRIDE;
        }
    }

    let sensor = rt.private_field(rt.browser_node, "sensorEvents").unwrap();
    let animate_interests = rt.frt.get(&sensor).map_or(0, |f| {
        f.interests
            .iter()
            .filter(|i| matches!(i.cb, Cb::Particle(_)))
            .count()
    });
    let s = &rt.scene;

    Resources {
        systems,
        buffer_floats: floats,
        animate_interests,
        live_watchers: rt.live_watchers.len(),
        live_nodes: s.nodes.len() - s.free_nodes.len(),
        arena: s.nodes.len(),
        arena_cap: s.nodes.capacity(),
        nrt: rt.nrt.len(),
        frt: rt.frt.len(),
        contexts: s.contexts.len(),
    }
}

/// No interest of a ParticleSystem names a node that is not a ParticleSystem with state.
fn assert_no_stale_ids(rt: &Runtime, when: &str) {
    for (k, f) in &rt.frt {
        for i in f
            .interests
            .iter()
            .filter(|i| matches!(i.cb, Cb::Particle(_)))
        {
            assert!(
                rt.particle_sim(i.node).is_some(),
                "{when}: an interest of field {k:?} names node {} which holds no ParticleSystem state",
                i.node
            );
            assert_eq!(
                rt.builtin_name(i.node),
                Some("ParticleSystem"),
                "{when}: the interest names a node of another type"
            );
        }
    }
}

struct World {
    v: Viewer3d,
    t: f64,
}

impl World {
    fn steps(&mut self, n: usize) {
        for _ in 0..n {
            self.t += DT;
            self.v.date_now = self.t;
            self.v.step(self.t);
        }
    }

    fn named(&self, ctx: CtxId, name: &str) -> NodeId {
        self.v.rt.scene.contexts[ctx]
            .named_nodes
            .get(name)
            .unwrap_or_else(|| panic!("no {name}"))
    }

    fn inline_ctx(&self) -> CtxId {
        self.v.rt.inline_scenes[&self.named(ROOT, "Inl")]
    }

    fn field(&mut self, node: NodeId, name: &str, value: FieldValue) {
        let k = self.v.rt.get_field(node, name).unwrap();
        self.v.rt.set_value(k, value);
    }
}

fn start(d: &Path) -> World {
    write(d, "a.x3dv", &scene(0));
    write(d, "b.x3dv", &scene(1));

    let p = write(
        d,
        "main.x3dv",
        &format!("{HEADER}\nDEF Inl Inline {{ url \"a.x3dv\" }}\n"),
    );
    let mut w = World {
        v: Viewer3d::load_attributes(
            &p,
            Some(d),
            T0,
            64,
            64,
            Some(60.0),
            false,
            ORACLE_PAGE_ATTRIBUTES,
        )
        .expect("load"),
        t: T0,
    };

    w.v.frame_rate = 60.0;
    w.steps(6);
    w
}

#[test]
fn particle_systems_come_and_go_with_their_inline_100_cycles() {
    big(|| {
        let d = dir("cycles");
        let mut w = start(&d);
        let mut loaded = None;
        let mut unloaded = None;

        // The Inline is loaded: four systems animate.
        let r = resources(&w.v.rt);

        assert_eq!((r.systems, r.animate_interests), (4, 4), "{r:?}");
        assert_no_stale_ids(&w.v.rt, "loaded");

        for c in 1..=100 {
            let inl = w.named(ROOT, "Inl");

            // Unload.
            w.field(inl, "load", FieldValue::SFBool(false));
            w.steps(3);

            let r = resources(&w.v.rt);

            assert_eq!(
                (r.systems, r.buffer_floats, r.animate_interests),
                (0, 0, 0),
                "cycle {c}: an unloaded scene keeps particle state: {r:?}"
            );
            assert_no_stale_ids(&w.v.rt, "unloaded");

            if c == 10 {
                unloaded = Some(r.clone());
            }

            if let Some(b) = &unloaded {
                assert_eq!(
                    &r, b,
                    "cycle {c}: a table grew after the warm-up (unloaded)"
                );
            }

            // Load the other scene.
            w.field(
                inl,
                "url",
                FieldValue::MFString(vec![if c % 2 == 0 { "a.x3dv" } else { "b.x3dv" }.into()]),
            );
            w.field(inl, "load", FieldValue::SFBool(true));
            w.steps(8);

            let ctx = w.inline_ctx();
            let systems: Vec<NodeId> = ["P1", "P2", "P3", "P4"]
                .iter()
                .map(|n| w.named(ctx, n))
                .collect();
            let r = resources(&w.v.rt);

            assert_eq!((r.systems, r.animate_interests), (4, 4), "cycle {c}: {r:?}");
            assert_no_stale_ids(&w.v.rt, "reloaded");

            // They run: every system has created particles, the Explosion burst all of its.
            for (i, &p) in systems.iter().enumerate() {
                let sim = w.v.rt.particle_sim(p).unwrap();

                assert!(
                    sim.num_particles > 0,
                    "cycle {c}: system {i} has no particles after 8 frames"
                );
                assert!(
                    w.v.rt.get_bool(p, "isActive"),
                    "cycle {c}: system {i} is not active"
                );
            }

            assert_eq!(
                w.v.rt.particle_sim(systems[2]).unwrap().num_particles,
                20,
                "cycle {c}: the ExplosionEmitter burst"
            );
            assert_eq!(w.v.rt.particle_sim(systems[0]).unwrap().max_particles, 60);

            if [10, 50, 100].contains(&c) {
                println!(
                    "MEM\tparticles-inline-cycle\t{c}\tsystems={}\tbuffer_floats={}\tanimate_interests={}\tlive_watchers={}\tlive_nodes={}\tarena={}\tnrt={}\tfrt={}\trss_kib={}",
                    r.systems,
                    r.buffer_floats,
                    r.animate_interests,
                    r.live_watchers,
                    r.live_nodes,
                    r.arena,
                    r.nrt,
                    r.frt,
                    rss_kib()
                );
            }

            // a.x3dv and b.x3dv have different geometry, so compare like with like.
            if c == 10 || c == 11 {
                loaded = Some((c % 2, r.clone()));
            }

            if let Some((parity, b)) = &loaded {
                if c % 2 == *parity {
                    assert_eq!(&r, b, "cycle {c}: a table grew after the warm-up (loaded)");
                }
            }
        }

        let _ = std::fs::remove_dir_all(&d);
    });
}

/// A system of the scene that went away: no state, no interest, and the frames after it neither panic
/// nor animate anything that is gone.
#[test]
fn a_released_system_has_no_state_and_no_interest() {
    big(|| {
        let d = dir("released");
        let mut w = start(&d);
        let ctx = w.inline_ctx();
        let old: Vec<NodeId> = ["P1", "P2", "P3", "P4"]
            .iter()
            .map(|n| w.named(ctx, n))
            .collect();
        let inl = w.named(ROOT, "Inl");

        assert!(
            old.iter()
                .all(|&p| w.v.rt.particle_sim(p).is_some_and(|s| s.num_particles > 0))
        );

        w.field(inl, "load", FieldValue::SFBool(false));
        w.steps(3);

        for &p in &old {
            assert!(
                w.v.rt.particle_sim(p).is_none(),
                "node {p} still holds particle state after its scene was unloaded"
            );
        }

        let sensor =
            w.v.rt
                .private_field(w.v.rt.browser_node, "sensorEvents")
                .unwrap();
        let named: Vec<NodeId> =
            w.v.rt
                .frt
                .get(&sensor)
                .map(|f| {
                    f.interests
                        .iter()
                        .filter(|i| matches!(i.cb, Cb::Particle(_)))
                        .map(|i| i.node)
                        .collect()
                })
                .unwrap_or_default();

        assert!(
            named.is_empty(),
            "animate interests of released nodes: {named:?}"
        );

        // Many frames without the scene, then the scene again: new systems start from nothing.
        w.steps(30);
        w.field(inl, "load", FieldValue::SFBool(true));
        w.steps(4);

        let ctx = w.inline_ctx();
        let p3 = w.named(ctx, "P3");

        assert_eq!(w.v.rt.particle_sim(p3).unwrap().num_particles, 20);
        assert!(
            w.v.rt
                .particle_sim(p3)
                .unwrap()
                .particles
                .iter()
                .all(|p| p[0] >= 1.0 || p[0] == 0.0),
            "life counts births"
        );
        assert_no_stale_ids(&w.v.rt, "reloaded");
        let _ = std::fs::remove_dir_all(&d);
    });
}
