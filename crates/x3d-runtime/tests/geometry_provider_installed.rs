//! REA batch 06R repair, defect 2: a bare `Runtime` in a process in which the geometry provider is
//! installed (here by the test, in `x3d-viewer` by every `Viewer3d`). The provider is a process-global
//! `OnceLock`: the first install wins, and every Runtime of the process uses it. Nothing is logged and
//! the geometry is present. (The case without a provider is `geometry_provider_bare.rs`, its own binary.)

use x3d_runtime::nodes::particle_systems::{
    GeometryProvider, GeometryRequest, GeometrySoup, geometry_provider, set_geometry_provider,
};
use x3d_runtime::{Fetcher, Runtime};

struct NoFetch;

impl Fetcher for NoFetch {
    fn fetch(&self, _: &url::Url) -> Result<Vec<u8>, String> {
        Err("no fetch".into())
    }
}

/// A 2x2 quad at y = 0 (two triangles, normal +y) for every geometry node.
fn first(_: &Runtime, _: &GeometryRequest) -> Option<GeometrySoup> {
    let v = [
        [-1.0, 0.0, -1.0],
        [1.0, 0.0, -1.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, -1.0],
        [1.0, 0.0, 1.0],
        [-1.0, 0.0, 1.0],
    ];

    Some(GeometrySoup {
        vertices: v.iter().flat_map(|p| [p[0], p[1], p[2], 1.0]).collect(),
        normals: v.iter().flat_map(|_| [0.0, 1.0, 0.0]).collect(),
        solid: false,
    })
}

fn second(_: &Runtime, _: &GeometryRequest) -> Option<GeometrySoup> {
    None
}

const WORLD: &str = "#X3D V3.3 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\n\
    DEF S ParticleSystem { maxParticles 16 particleLifetime 3 emitter SurfaceEmitter { speed 1 surface Box { size 2 2 2 } } }\n\
    DEF B ParticleSystem { maxParticles 16 particleLifetime 3 emitter PointEmitter { speed 3 direction 0 0 0 } physics [ BoundedPhysicsModel { geometry Box { size 0.5 0.5 0.5 } } ] }\n";

fn runtime() -> Runtime {
    let scene = x3d_parse::parse_bytes(WORLD.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(NoFetch));

    rt.live = true;
    rt.load_world();
    rt
}

fn run(rt: &mut Runtime) {
    for k in 0..90 {
        rt.now = 946684800.0 + k as f64 / 30.0;
        rt.frame_rate = 30.0;
        rt.process_phase("sensorEvents");
    }
}

fn big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(f)
        .unwrap()
        .join()
        .unwrap();
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
fn runtimes_with_an_installed_provider_do_not_warn_and_share_it() {
    big(|| {
        set_geometry_provider(first as GeometryProvider);

        // A second install (another Viewer, another embedder) is ignored: the first one wins.
        set_geometry_provider(second as GeometryProvider);
        assert_eq!(
            geometry_provider().unwrap() as usize,
            first as GeometryProvider as usize,
            "first install wins"
        );

        x3d_runtime::particles::use_random_seeds();

        let mut a = runtime();
        let mut b = runtime();

        run(&mut a);
        run(&mut b);

        for rt in [&a, &b] {
            assert!(
                rt.log.iter().all(|l| !l.contains("no geometry provider")),
                "{:?}",
                rt.log
            );
            assert!(visible(rt, "S") > 0, "the surface emits visible particles");
        }

        assert_eq!(
            visible(&a, "S"),
            visible(&b, "S"),
            "two Runtimes, one provider, the same particles"
        );
    });
}
