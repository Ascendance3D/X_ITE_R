//! REA batch 06R: the 8 statistical ParticleSystem cases (20000 births each) through the real
//! runtime against X_ITE's samples (oracle/rea-batch06-baseline/particles-*-stat/samples.f32, the
//! born records of the GPU run, subsampled to 20000 rows). With X_ITE's `randomSeed`s the port
//! draws the same random numbers, so beyond the two-sample Kolmogorov-Smirnov test of
//! `rea_b06_particles.py` (D <= 2.5 sqrt ((n + m) / (n m)), 0.025 at 20000 vs 20000) the samples can be
//! compared row by row. Measured against the GPU (identical to the numpy float32 emulation of the
//! GLSL, i.e. the GPU's sin / cos / pow / fma): lifetime and color exact, velocity at most 1.4e-5 (cone-wide),
//! position at most 6.2e-5 (volume: a line / plane intersection divides by a small number).

mod particles_b06_common;

use particles_b06_common::*;

/// Row-wise bounds written down from the measured maxima (see the module comment).
const VELOCITY_BOUND: f64 = 2e-5;
const POSITION_BOUND: f64 = 1e-4;

fn run_stat(name: &str) {
    let name = name.to_string();

    big(move || {
        let o = oracle();
        let script = json(&o.join("synthetic-rea-b06/scripts").join(format!("{name}.json")));
        let base = Baseline::load(&name);
        let run = replay(&o.join("synthetic-rea-b06").join(format!("{name}.x3dv")), &script, base.seeds());

        compare_steps(&name, &base, &run);

        // Births per frame: exact.
        let stats = json(&base.dir.join("stats.json"));

        for (frame, n) in stats["per_frame_births"].as_object().unwrap() {
            let got = run.dumps.get(&frame.parse().unwrap()).map_or(0, |d| d.len());

            assert_eq!(got as u64, n.as_u64().unwrap(), "{name}: frame {frame}: births");
        }

        // The samples, row by row.
        let ours = compact(&run.dumps, 20000);
        let theirs: Vec<[f32; 13]> = f32s(&base.dir.join("samples.f32")).chunks_exact(13).map(|c| c.try_into().unwrap()).collect();

        assert_eq!(ours.len(), theirs.len(), "{name}: samples");

        let mut worst = [0.0f64; 13];

        for (r, (a, b)) in ours.iter().zip(&theirs).enumerate() {
            for c in 0..13 {
                let (x, y) = (f64::from(a[c]), f64::from(b[c]));

                assert_eq!(x.is_nan(), y.is_nan(), "{name}: row {r} column {c}: port {x}, X_ITE {y}");

                if !x.is_nan() {
                    worst[c] = worst[c].max((x - y).abs());
                }
            }
        }

        // frame, lifetime, rgba: exact; velocity, position: the measured bounds.
        assert!(worst[..6].iter().all(|&d| d == 0.0), "{name}: frame / lifetime / color differ: {:?}", &worst[..6]);
        assert!(worst[6..9].iter().all(|&d| d <= VELOCITY_BOUND), "{name}: velocity {:?}", &worst[6..9]);
        assert!(worst[9..13].iter().all(|&d| d <= POSITION_BOUND), "{name}: position {:?}", &worst[9..13]);

        // The Kolmogorov-Smirnov test on the features, as the check script does.
        let visible = |rows: &[[f32; 13]]| rows.iter().filter(|r| !r[9].is_nan()).copied().collect::<Vec<_>>();
        let (va, vb) = (visible(&ours), visible(&theirs));
        let speed = |r: &[f32; 13]| (f64::from(r[6]).powi(2) + f64::from(r[7]).powi(2) + f64::from(r[8]).powi(2)).sqrt();
        let features: [(&str, Box<dyn Fn(&[f32; 13]) -> f64>); 8] = [
            ("lifetime", Box::new(|r| f64::from(r[1]))),
            ("speed", Box::new(speed)),
            ("vx", Box::new(|r| f64::from(r[6]))),
            ("vy", Box::new(|r| f64::from(r[7]))),
            ("vz", Box::new(|r| f64::from(r[8]))),
            ("px", Box::new(|r| f64::from(r[9]))),
            ("py", Box::new(|r| f64::from(r[10]))),
            ("pz", Box::new(|r| f64::from(r[11]))),
        ];
        let (n, m) = (va.len() as f64, vb.len() as f64);
        let crit = 2.5 * ((n + m) / (n * m)).sqrt();
        let mut line = format!("KS\t{name}\tn={}\tcrit={crit:.4}", va.len());

        assert_eq!(va.len(), vb.len(), "{name}: visible samples");

        for (label, f) in &features {
            let d = ks2(&va.iter().map(f).collect::<Vec<_>>(), &vb.iter().map(f).collect::<Vec<_>>());

            line += &format!("\t{label}={d:.5}");
            assert!(d <= crit, "{name}: KS {label}: D {d} > {crit}");
        }

        println!("{line}\tvelocity_max={:.2e}\tposition_max={:.2e}", worst[6..9].iter().fold(0.0f64, |a, &b| a.max(b)), worst[9..13].iter().fold(0.0f64, |a, &b| a.max(b)));
    });
}

macro_rules! cases {
    ($($fn_name:ident => $case:literal),* $(,)?) => { $( #[test] fn $fn_name() { run_stat($case); } )* };
}

cases! {
    point_spherical_stat => "particles-point-spherical-stat",
    cone_basic_stat => "particles-cone-basic-stat",
    cone_wide_stat => "particles-cone-wide-stat",
    explosion_stat => "particles-explosion-stat",
    surface_ifs_twosided_stat => "particles-surface-ifs-twosided-stat",
    surface_box_stat => "particles-surface-box-stat",
    volume_cube_stat => "particles-volume-cube-stat",
    volume_twocubes_stat => "particles-volume-twocubes-stat",
}
