//! VolumeEmitter intersection limit (lane XITE_R_PARTICLE_EMITTER_HIT_LIMIT_01) against pinned X_ITE 16.5.0.
//!
//! The worlds and scripts are oracle/synthetic-rea-b17-emitter-hits (oracle/tools/gen_rea_b17_emitter_hits.py): a
//! VolumeEmitter whose emission lines meet 31 to 128 quads, more than the GLSL intersection array (ARRAY_SIZE = 32)
//! holds. A case's X_ITE baseline is oracle/rea-batch17-emitter-hits-baseline/NAME/ (`js.json`, `js.f32`: every
//! frame's transform feedback buffer), recorded by `oracle/tools/rea_b17_emitter_hits.py record` with
//! `x3d-oracle particles` (three identical runs). Each case is replayed through the real runtime with the baseline's
//! seeds (particles_b06_common::replay) and compared with the batch 06R tolerances (compare_steps, compare_dumps).

mod particles_b06_common;

use particles_b06_common::*;

const WORLDS: &str = "synthetic-rea-b17-emitter-hits";
const BASELINES: &str = "rea-batch17-emitter-hits-baseline";

fn script(name: &str) -> serde_json::Value {
    json(
        &oracle()
            .join(WORLDS)
            .join("scripts")
            .join(format!("{name}.json")),
    )
}

fn world(name: &str) -> std::path::PathBuf {
    oracle().join(WORLDS).join(format!("{name}.x3dv"))
}

fn baseline(name: &str) -> Baseline {
    let dir = oracle().join(BASELINES).join(name);

    Baseline {
        js: json(&dir.join("js.json")),
        dir,
    }
}

fn run_case(name: &str) {
    let name = name.to_string();

    big(move || {
        let base = baseline(&name);
        let run = replay(&world(&name), &script(&name), base.seeds());

        compare_steps(&name, &base, &run);

        let max = compare_dumps(&name, &base.dumps(), &run.dumps);

        println!(
            "MAXERR\t{name}\t{}",
            max.iter()
                .map(|(g, m)| format!("{g}={m:.2e}"))
                .collect::<Vec<_>>()
                .join("\t")
        );
        assert!(!run.dumps.is_empty());
    });
}

macro_rules! cases {
    ($($fn_name:ident => $case:literal),* $(,)?) => { $( #[test] fn $fn_name() { run_case($case); } )* };
}

cases! {
    hits_31 => "ve-hits-31",
    hits_32 => "ve-hits-32",
    hits_33 => "ve-hits-33",
    hits_34 => "ve-hits-34",
    hits_40 => "ve-hits-40",
    hits_48 => "ve-hits-48",
    hits_64 => "ve-hits-64",
    hits_128 => "ve-hits-128",
    hits_33_down => "ve-hits-33-down",
    hits_33_shuffle => "ve-hits-33-shuffle",
    hits_33_uneven => "ve-hits-33-uneven",
    hits_40_down => "ve-hits-40-down",
    hits_40_shuffle => "ve-hits-40-shuffle",
    hits_40_uneven => "ve-hits-40-uneven",
    hits_64_down => "ve-hits-64-down",
    hits_64_shuffle => "ve-hits-64-shuffle",
    hits_64_uneven => "ve-hits-64-uneven",
    hits_128_down => "ve-hits-128-down",
    hits_128_shuffle => "ve-hits-128-shuffle",
    hits_128_uneven => "ve-hits-128-uneven",
    speed_33 => "ve-speed-33",
    speed_64 => "ve-speed-64",
    speed_128 => "ve-speed-128",
    speed_64_down => "ve-speed-64-down",
    speed_64_shuffle => "ve-speed-64-shuffle",
    speed_128_uneven => "ve-speed-128-uneven",
    speed_64_bounded => "ve-speed-64-bounded",
}

/// Step `l` to frame `k` with the baseline's seed of that frame; the particle state after it.
fn step_state(l: &mut Loaded, k: u64, seed: f64) -> Vec<[f32; STRIDE]> {
    let now = l.t0 + k as f64 * l.dt;

    x3d_runtime::particles::set_seed_list(vec![seed]);
    l.v.date_now = now;
    l.v.step(now);
    x3d_runtime::particles::use_random_seeds();

    let sim = l.v.rt.particle_sim(l.ps).expect("ParticleSystem state");

    sim.particles
        .iter()
        .take(sim.num_particles)
        .copied()
        .collect()
}

/// Two viewers with VolumeEmitters past ARRAY_SIZE, stepped in turn, give what each gives alone, bit for bit: the
/// intersection arrays, the BVH texture and the random state belong to each ParticleSystem. Each step gets its
/// frame's seed from the case's baseline.
#[test]
fn independent_viewers() {
    big(|| {
        let (a, b) = ("ve-hits-64-shuffle", "ve-hits-128-uneven");
        let open = |name: &str| load(&world(name), &script(name));
        let (seeds_a, seeds_b) = (baseline(a).seeds(), baseline(b).seeds());
        let alone = |name: &str, seeds: &[f64]| {
            let mut l = open(name);

            (1..=40)
                .map(|k| step_state(&mut l, k, seeds[k as usize - 1]))
                .collect::<Vec<_>>()
        };
        let alone_a = alone(a, &seeds_a);
        let alone_b = alone(b, &seeds_b);
        let (mut va, mut vb) = (open(a), open(b));
        let (mut both_a, mut both_b) = (vec![], vec![]);

        for k in 1..=40u64 {
            both_a.push(step_state(&mut va, k, seeds_a[k as usize - 1]));
            both_b.push(step_state(&mut vb, k, seeds_b[k as usize - 1]));
        }

        let bits = |s: &Vec<Vec<[f32; STRIDE]>>| {
            s.iter()
                .flatten()
                .flatten()
                .map(|x| x.to_bits())
                .collect::<Vec<_>>()
        };

        assert_eq!(bits(&alone_a), bits(&both_a), "{a}");
        assert_eq!(bits(&alone_b), bits(&both_b), "{b}");
        assert!(alone_a.iter().any(|s| !s.is_empty()), "{a}: no particles");
        assert!(alone_b.iter().any(|s| !s.is_empty()), "{b}: no particles");
    });
}
