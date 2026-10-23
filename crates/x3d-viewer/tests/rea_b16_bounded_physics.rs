//! BoundedPhysicsModel replay (lane XITE_R_BOUNDED_PHYSICS_REPLAY_01) against pinned X_ITE 16.5.0.
//!
//! The worlds and scripts are oracle/synthetic-rea-b16-bounded (oracle/tools/gen_rea_b16_bounded.py). A case's X_ITE
//! baseline is oracle/rea-batch16-bounded-baseline/NAME/ (`js.json`, `js.f32`: every frame's transform feedback
//! buffer), recorded by `oracle/tools/rea_b16_bounded.py record` with `x3d-oracle particles` (three identical runs).
//! Each case is replayed through the real runtime with the baseline's seeds and its field events
//! (particles_b06_common::replay) and compared with the batch 06R tolerances (compare_steps, compare_dumps).

mod particles_b06_common;

use particles_b06_common::*;

fn run_case(name: &str) {
    let name = name.to_string();

    big(move || {
        let o = oracle();
        let dir = o.join("rea-batch16-bounded-baseline").join(&name);
        let base = Baseline {
            js: json(&dir.join("js.json")),
            dir,
        };
        let script = json(
            &o.join("synthetic-rea-b16-bounded/scripts")
                .join(format!("{name}.json")),
        );
        let run = replay(
            &o.join("synthetic-rea-b16-bounded")
                .join(format!("{name}.x3dv")),
            &script,
            base.seeds(),
        );

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
    hits_31 => "bp-hits-31",
    hits_32 => "bp-hits-32",
    hits_33 => "bp-hits-33",
    hits_64 => "bp-hits-64",
    hits_31_below => "bp-hits-31-below",
    hits_32_below => "bp-hits-32-below",
    hits_33_below => "bp-hits-33-below",
    hits_34_below => "bp-hits-34-below",
    hits_40_below => "bp-hits-40-below",
    hits_48_below => "bp-hits-48-below",
    hits_64_below => "bp-hits-64-below",
    hits_128_below => "bp-hits-128-below",
    hits_64_floor_last => "bp-hits-64-floor-last",
    hits_64_mixed => "bp-hits-64-mixed",
    damping_zero => "bp-damping-zero",
    damping_zero_box => "bp-damping-zero-box",
    damping_zero_mixed => "bp-damping-zero-mixed",
    damping_change => "bp-damping-change",
    tilted => "bp-tilted",
    transformed => "bp-transformed",
    normals => "bp-normals",
    normals_sphere => "bp-normals-sphere",
    enabled_toggle => "bp-enabled-toggle",
    enabled_late => "bp-enabled-late",
    rebuild => "bp-rebuild",
    replace => "bp-replace",
}

/// The particle state of `frames` steps of a loaded case (every particle of every frame).
fn states(l: &mut Loaded, frames: u64) -> Vec<Vec<[f32; STRIDE]>> {
    (1..=frames).map(|k| step_state(l, k)).collect()
}

fn step_state(l: &mut Loaded, k: u64) -> Vec<[f32; STRIDE]> {
    let now = l.t0 + k as f64 * l.dt;

    l.v.date_now = now;
    l.v.step(now);

    let sim = l.v.rt.particle_sim(l.ps).expect("ParticleSystem state");

    sim.particles
        .iter()
        .take(sim.num_particles)
        .copied()
        .collect()
}

/// Two viewers with bounded worlds, stepped in turn, give what each gives alone, bit for bit: the bounce state (the
/// bounded texture, the hit arrays) belongs to each ParticleSystem. The cases have no random quantity, so the seeds
/// do not matter.
#[test]
fn independent_viewers() {
    big(|| {
        let o = oracle();
        let open = |name: &str| {
            let script = json(
                &o.join("synthetic-rea-b16-bounded/scripts")
                    .join(format!("{name}.json")),
            );

            load(
                &o.join("synthetic-rea-b16-bounded")
                    .join(format!("{name}.x3dv")),
                &script,
            )
        };
        let (a, b) = ("bp-hits-64-below", "bp-tilted");
        let alone_a = states(&mut open(a), 60);
        let alone_b = states(&mut open(b), 60);
        let (mut va, mut vb) = (open(a), open(b));
        let (mut both_a, mut both_b) = (vec![], vec![]);

        for k in 1..=60 {
            both_a.push(step_state(&mut va, k));
            both_b.push(step_state(&mut vb, k));
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

        // Both bounced: an upward velocity somewhere.
        for (name, s) in [(a, &both_a), (b, &both_b)] {
            assert!(
                s.iter().flatten().any(|p| p[9] > 0.0),
                "{name}: no particle bounced"
            );
        }
    });
}
