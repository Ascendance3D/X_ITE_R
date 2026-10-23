//! REA batch 06R repair, defect 2: a bare `Runtime` in a process in which nobody installs the geometry
//! provider (`nodes::particle_systems::set_geometry_provider`, installed by every `Viewer3d`). This file
//! never installs one: the provider is a process-global `OnceLock`, so each scenario that depends on its
//! presence has its own test binary (`geometry_provider_installed.rs` here, and
//! `x3d-viewer/tests/geometry_provider_viewer.rs`).
//!
//! Observed behavior: Surface, Volume and Bounded geometry is empty, a concave CollidableShape has no
//! collision shape, and the runtime logs one `warn:` per affected node, never silently.

use x3d_runtime::{Fetcher, Runtime};

struct NoFetch;

impl Fetcher for NoFetch {
    fn fetch(&self, _: &url::Url) -> Result<Vec<u8>, String> {
        Err("no fetch".into())
    }
}

const CUBE: &str = "coordIndex [ 0 3 2 1 -1 4 5 6 7 -1 0 1 5 4 -1 2 3 7 6 -1 1 2 6 5 -1 0 4 7 3 -1 ] coord Coordinate { point [ -1 -1 -1, 1 -1 -1, 1 1 -1, -1 1 -1, -1 -1 1, 1 -1 1, 1 1 1, -1 1 1 ] }";

fn particles() -> String {
    format!(
        "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\n\
         DEF S ParticleSystem {{ maxParticles 16 particleLifetime 3 emitter SurfaceEmitter {{ speed 1 surface Box {{ size 2 2 2 }} }} }}\n\
         DEF V ParticleSystem {{ maxParticles 16 particleLifetime 3 emitter VolumeEmitter {{ direction 0 1 0 speed 1 {CUBE} }} }}\n\
         DEF B ParticleSystem {{ maxParticles 16 particleLifetime 3 emitter PointEmitter {{ speed 3 direction 0 0 0 }} physics [ BoundedPhysicsModel {{ geometry Box {{ size 0.5 0.5 0.5 }} }} ] }}\n\
         DEF C ParticleSystem {{ maxParticles 16 particleLifetime 3 emitter PointEmitter {{ speed 1 }} }}\n"
    )
}

fn runtime(text: &str) -> Runtime {
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(NoFetch));

    rt.live = true;
    rt.load_world();
    rt
}

fn run(rt: &mut Runtime, frames: usize) {
    for k in 0..frames {
        rt.now = 946684800.0 + k as f64 / 30.0;
        rt.frame_rate = 30.0;
        rt.process_phase("sensorEvents");
    }
}

fn provider_warnings(rt: &Runtime) -> Vec<&String> {
    rt.log
        .iter()
        .filter(|l| l.contains("no geometry provider"))
        .collect()
}

fn big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
}

#[test]
fn bare_runtime_warns_once_per_node_and_its_geometry_is_empty() {
    big(|| {
        assert!(
            x3d_runtime::nodes::particle_systems::geometry_provider().is_none(),
            "this binary never installs a provider"
        );

        let mut rt = runtime(&particles());

        x3d_runtime::particles::use_random_seeds();
        run(&mut rt, 90);

        // One warning for each of the three systems that need geometry; the PointEmitter system is quiet.
        let w = provider_warnings(&rt);

        assert_eq!(w.len(), 3, "{:?}", rt.log);
        assert!(
            w.iter()
                .all(|l| l.starts_with("warn: ParticleSystem: no geometry provider is installed")),
            "{w:?}"
        );
        assert!(
            w.iter().any(|l| l.contains("SurfaceEmitter"))
                && w.iter().any(|l| l.contains("VolumeEmitter"))
                && w.iter().any(|l| l.contains("BoundedPhysicsModel")),
            "{w:?}"
        );

        // More frames: no repeat.
        run(&mut rt, 90);
        assert_eq!(
            provider_warnings(&rt).len(),
            3,
            "once per node, not per frame"
        );

        let sim = |rt: &Runtime, name: &str| {
            let n = rt.scene.get_named_node(x3d_scene::ROOT, name).unwrap();
            let sim = rt.particle_sim(n).expect("state");
            sim.particles
                .iter()
                .take(sim.num_particles)
                .copied()
                .collect::<Vec<[f32; 28]>>()
        };

        // Surface: every particle is hidden (NaN position); Bounded: nothing stops the particles.
        let s = sim(&rt, "S");

        assert!(
            !s.is_empty() && s.iter().all(|p| p[24].is_nan()),
            "an empty surface emits hidden particles"
        );

        let b = sim(&rt, "B");
        let far = b
            .iter()
            .filter(|p| !p[24].is_nan())
            .map(|p| (p[24] * p[24] + p[25] * p[25] + p[26] * p[26]).sqrt())
            .fold(0.0f32, f32::max);

        assert!(far > 1.0, "no walls: {far}");
    });
}

#[test]
fn two_bare_runtimes_in_one_process_each_warn() {
    big(|| {
        let mut a = runtime(&particles());
        let mut b = runtime(&particles());

        run(&mut a, 10);
        run(&mut b, 10);
        assert_eq!(provider_warnings(&a).len(), 3);
        assert_eq!(
            provider_warnings(&b).len(),
            3,
            "the second Runtime warns too: the log is per Runtime"
        );
    });
}

#[test]
fn a_concave_collidable_shape_without_provider_warns_and_has_no_collision_shape() {
    big(|| {
        let text = std::fs::read_to_string(concat!(
            env!("CARGO_MANIFEST_DIR"),
            "/../../oracle/synthetic-rea-b06/shape-ifs-floor.x3dv"
        ))
        .unwrap();
        let mut rt = runtime(&text);

        for k in 0..120 {
            rt.now = 946684800.0 + k as f64 / 60.0;
            rt.frame_rate = 60.0;
            rt.frame();
        }

        let w = provider_warnings(&rt);

        assert_eq!(
            w.len(),
            1,
            "one warning for the one concave shape: {:?}",
            rt.log
        );
        assert!(w[0].contains("concave CollidableShape"), "{w:?}");

        // The floor has no collision shape, so the sphere falls through it.
        let ball = rt.scene.get_named_node(x3d_scene::ROOT, "B").unwrap();
        let y = match rt.pre_value(ball, "position") {
            Some(x3d_fields::FieldValue::SFVec3f(v)) => v.y,
            o => panic!("{o:?}"),
        };

        assert!(y < -3.0, "the body fell through the floor: y = {y}");
    });
}
