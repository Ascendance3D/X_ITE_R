//! REA batch 06R: the 29 exact ParticleSystem cases of oracle/synthetic-rea-b06 through the real
//! runtime (Viewer3d, virtual time) against the per-particle state X_ITE 16.5.0 leaves in its
//! transform feedback buffer (oracle/rea-batch06-baseline/particles-*/js.json + js.f32, read back
//! from the GPU). The shader's `randomSeed` of every frame is X_ITE's recorded one; everything else
//! is the port: LCG, draw order, emitters, BVH, bounce, the 15-frame deltaTime average, the burst
//! and birth schedule. Tolerances are `rea_b06_particles.py` TOL (life, lifetime, elapsed, color
//! exact; velocity 3e-6 + 1e-6 |v|; position 5e-6 + 2e-6 |p|; measured: 4.2e-6 position, 2.4e-6
//! velocity against the GPU, identical to the numpy float32 emulation of the GLSL).

mod particles_b06_common;

use particles_b06_common::*;

fn run_case(name: &str) {
    let name = name.to_string();

    big(move || {
        let o = oracle();
        let script = json(&o.join("synthetic-rea-b06/scripts").join(format!("{name}.json")));
        let base = Baseline::load(&name);
        let run = replay(&o.join("synthetic-rea-b06").join(format!("{name}.x3dv")), &script, base.seeds());

        compare_steps(&name, &base, &run);

        let max = compare_dumps(&name, &base.dumps(), &run.dumps);

        println!("MAXERR\t{name}\t{}", max.iter().map(|(g, m)| format!("{g}={m:.2e}")).collect::<Vec<_>>().join("\t"));
        assert!(!run.dumps.is_empty());
    });
}

macro_rules! cases {
    ($($fn_name:ident => $case:literal),* $(,)?) => { $( #[test] fn $fn_name() { run_case($case); } )* };
}

cases! {
    point_dir => "particles-point-dir",
    point_dirraw => "particles-point-dirraw",
    point_fixed => "particles-point-fixed",
    point_spherical => "particles-point-spherical",
    point_nospeed => "particles-point-nospeed",
    point_off => "particles-point-off",
    point_create_off => "particles-point-create-off",
    cone_basic => "particles-cone-basic",
    cone_tilt => "particles-cone-tilt",
    cone_zeroangle => "particles-cone-zeroangle",
    cone_wide => "particles-cone-wide",
    cone_spherical => "particles-cone-spherical",
    cone_down => "particles-cone-down",
    explosion_basic => "particles-explosion-basic",
    explosion_fixed => "particles-explosion-fixed",
    surface_ifs_solid => "particles-surface-ifs-solid",
    surface_ifs_twosided => "particles-surface-ifs-twosided",
    surface_box => "particles-surface-box",
    surface_nosurface => "particles-surface-nosurface",
    volume_cube => "particles-volume-cube",
    volume_cube_spherical => "particles-volume-cube-spherical",
    volume_twocubes => "particles-volume-twocubes",
    volume_empty => "particles-volume-empty",
    bounded_floor => "particles-bounded-floor",
    bounded_floor_damp1 => "particles-bounded-floor-damp1",
    bounded_box => "particles-bounded-box",
    bounded_two => "particles-bounded-two",
    bounded_disabled => "particles-bounded-disabled",
    bounded_floor_gravity => "particles-bounded-floor-gravity",
}

/// Numbers read off the GPU by oracle/tools/x_ite_particles.mjs, written out: (case, frame, particle,
/// numParticles, deltaTime, [life, lifetime, elapsed], velocity, position).
#[test]
fn spot_values_from_the_gpu() {
    #[allow(clippy::type_complexity)]
    let spots: [(&str, u64, usize, usize, f64, [f32; 3], [f32; 3], [f32; 4]); 6] = [
        ("particles-point-fixed", 4, 0, 1, 0.008038847736625514, [1.0, 1.0, 0.008038847707211971], [3.0, 0.0, 0.0], [-0.9758834838867188, 0.5, 2.0, 1.0]),
        ("particles-point-fixed", 40, 3, 12, 0.031223006067309256, [1.0, 1.0, 0.7589634656906128], [3.0, 0.0, 0.0], [1.2768901586532593, 0.5, 2.0, 1.0]),
        ("particles-explosion-basic", 1, 0, 16, 0.0022222222222222222, [1.0, 0.7661547660827637, 0.0], [1.9938759803771973, 2.2368502616882324, 1.3765053749084473], [0.0, 1.0, 0.0, 1.0]),
        ("particles-bounded-floor", 40, 2, 7, 0.031223006067309256, [1.0, 1.879188895225525, 0.651789665222168], [0.0, 1.817145586013794, 0.0], [0.2999993562698364, 0.1694689393043518, -0.20000042021274567, 1.0]),
        ("particles-surface-box", 20, 5, 10, 0.024946185892110985, [1.0, 1.0, 0.18017223477363586], [1.0, 0.0, 0.0], [1.1801722049713135, 0.49847790598869324, -0.8939659595489502, 1.0]),
        ("particles-volume-cube", 30, 4, 15, 0.029126240996650475, [1.0, 0.831851065158844, 0.49148041009902954], [0.0, 1.1451361179351807, 0.0], [0.6128652095794678, 0.3715283274650574, 0.42287617921829224, 1.0]),
    ];

    for (name, frame, index, num, delta_time, lle, v, p) in spots {
        big(move || {
            let o = oracle();
            let script = json(&o.join("synthetic-rea-b06/scripts").join(format!("{name}.json")));
            let base = Baseline::load(name);
            let run = replay(&o.join("synthetic-rea-b06").join(format!("{name}.x3dv")), &script, base.seeds());
            let step = &run.steps[frame as usize - 1];
            let r = &run.dumps[&frame][index];

            assert_eq!(step.num_particles, num, "{name}");
            assert!((step.delta_time - delta_time).abs() <= 1e-12 * delta_time, "{name}: deltaTime {}", step.delta_time);
            assert_eq!([r[0], r[1], r[2]], lle, "{name}: life, lifetime, elapsedTime");

            for c in 0..3 {
                assert!((r[8 + c] - v[c]).abs() <= 3e-6 + 1e-6 * v[c].abs(), "{name}: velocity {c}: {} vs {}", r[8 + c], v[c]);
            }

            for c in 0..4 {
                assert!((r[24 + c] - p[c]).abs() <= 5e-6 + 2e-6 * p[c].abs(), "{name}: position {c}: {} vs {}", r[24 + c], p[c]);
            }
        });
    }
}
