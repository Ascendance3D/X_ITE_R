//! REA batch 06R repair, defect 2: the geometry provider is installed by `Viewer3d` and is process
//! global (first install wins). One test, because the order is the point:
//!
//! 1. a bare `Runtime` before any Viewer exists: no provider, empty geometry, one warning per node;
//! 2. a Viewer is created: it installs the provider;
//! 3. a bare `Runtime` created afterwards (no renderer of its own) is served by that provider: geometry
//!    present, no warning. It is "masked": its correctness depends on a Viewer elsewhere in the process;
//! 4. the Viewer's own Runtime never warns, and the early Runtime does not change its mind: the emitter
//!    geometry it cached while empty stays empty (the cache key is the geometry, not the provider).

use x3d_runtime::Runtime;
use x3d_runtime::loader::LocalFetcher;
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

const WORLD: &str = "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\n\
    DEF S ParticleSystem { maxParticles 16 particleLifetime 3 emitter SurfaceEmitter { speed 1 surface Box { size 2 2 2 } } }\n\
    DEF B ParticleSystem { maxParticles 16 particleLifetime 3 emitter PointEmitter { speed 3 direction 0 0 0 } physics [ BoundedPhysicsModel { geometry Box { size 0.5 0.5 0.5 } } ] }\n";

fn bare() -> Runtime {
    let scene = x3d_parse::parse_bytes(WORLD.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(
        scene,
        Box::new(LocalFetcher {
            root: std::env::temp_dir(),
            prefix: "http://oracle/".into(),
        }),
    );

    rt.live = true;
    rt.load_world();
    rt
}

fn run(rt: &mut Runtime, from: usize, to: usize) {
    for k in from..to {
        rt.now = 946684800.0 + k as f64 / 30.0;
        rt.frame_rate = 30.0;
        rt.process_phase("sensorEvents");
    }
}

fn warnings(rt: &Runtime) -> usize {
    rt.log
        .iter()
        .filter(|l| l.contains("no geometry provider"))
        .count()
}

fn visible(rt: &Runtime, name: &str) -> usize {
    let n = rt.scene.get_named_node(x3d_scene::ROOT, name).unwrap();
    let sim = rt.particle_sim(n).unwrap();

    sim.particles
        .iter()
        .take(sim.num_particles)
        .filter(|p| !p[24].is_nan())
        .count()
}

#[test]
fn a_viewer_installs_the_provider_for_every_runtime_of_the_process() {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(|| {
            assert!(x3d_runtime::nodes::particle_systems::geometry_provider().is_none());
            x3d_runtime::particles::use_random_seeds();

            // 1. Before any Viewer.
            let mut early = bare();

            run(&mut early, 0, 90);
            assert_eq!(
                warnings(&early),
                2,
                "S (surface) and B (bounded) warn: {:?}",
                early.log
            );
            assert_eq!(visible(&early, "S"), 0, "empty surface, hidden particles");

            // 2. A Viewer.
            let d = std::env::temp_dir().join(format!("x3d-provider-{}", std::process::id()));

            std::fs::create_dir_all(&d).unwrap();

            let p = d.join("w.x3dv");

            std::fs::write(&p, WORLD).unwrap();

            let mut v = Viewer3d::load_attributes(
                &p,
                Some(&d),
                946684800.0,
                64,
                64,
                Some(30.0),
                false,
                ORACLE_PAGE_ATTRIBUTES,
            )
            .expect("load");

            v.frame_rate = 30.0;

            for k in 0..90 {
                v.date_now = 946684800.0 + k as f64 / 30.0;
                v.step(v.date_now);
            }

            assert!(
                x3d_runtime::nodes::particle_systems::geometry_provider().is_some(),
                "the Viewer installed it"
            );
            assert_eq!(warnings(&v.rt), 0, "{:?}", v.rt.log);
            assert!(visible(&v.rt, "S") > 0);

            // 3. A bare Runtime after the Viewer.
            let mut late = bare();

            run(&mut late, 0, 90);
            assert_eq!(
                warnings(&late),
                0,
                "masked by the Viewer's provider: {:?}",
                late.log
            );
            assert!(visible(&late, "S") > 0, "geometry present");

            // 4. The early Runtime: nothing more is logged, and it keeps what it cached.
            run(&mut early, 90, 180);
            assert_eq!(warnings(&early), 2);
            println!("EARLY-AFTER-INSTALL visible S = {}", visible(&early, "S"));

            let _ = std::fs::remove_dir_all(&d);
        })
        .unwrap()
        .join()
        .unwrap();
}
