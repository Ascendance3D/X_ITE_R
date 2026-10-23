//! REA batch 06R: runtime field changes of a ParticleSystem and its emitter, replayed through the real
//! runtime against X_ITE 16.5.0 (oracle/tools/x_ite_particles_events.mjs sets the fields between frames;
//! scripts in oracle/synthetic-rea-b06/events, baselines in oracle/rea-batch06-baseline/particles-events-*).
//!
//! * point: emitter speed, variation, on (FALSE then TRUE), ParticleSystem particleLifetime,
//!   lifetimeVariation and createParticles change while particles fly.
//! * maxparticles: maxParticles 12 -> 20 -> 6 (set_maxParticles__ resizes the buffers, resets
//!   creationTime), enabled FALSE (isActive FALSE, numParticles 0, no animate) and TRUE again,
//!   maxParticles 0 (inactive) and 10.
//! * explosion: speed, createParticles and particleLifetime between bursts, maxParticles 16 -> 8.
//!
//! The per-step numParticles, createParticles, isActive, maxParticles and every particle of every
//! frame are compared with X_ITE's.

mod particles_b06_common;

use particles_b06_common::*;

fn run_events(name: &str) {
    let name = name.to_string();

    big(move || {
        let o = oracle();
        let script = json(
            &o.join("synthetic-rea-b06/events")
                .join(format!("particles-events-{name}.json")),
        );
        let base = Baseline::load(&format!("particles-events-{name}"));
        let run = replay(
            &o.join("synthetic-rea-b06/events")
                .join(format!("particles-events-{name}.x3dv")),
            &script,
            base.seeds(),
        );

        compare_steps(&name, &base, &run);

        let max = compare_dumps(&name, &base.dumps(), &run.dumps);

        println!(
            "MAXERR\tevents-{name}\t{}",
            max.iter()
                .map(|(g, m)| format!("{g}={m:.2e}"))
                .collect::<Vec<_>>()
                .join("\t")
        );
        assert_eq!(run.dumps.len(), script["frames"].as_u64().unwrap() as usize);
    });
}

#[test]
fn emitter_and_system_fields_change_while_particles_fly() {
    run_events("point");
}

#[test]
fn max_particles_and_enabled_change() {
    run_events("maxparticles");
}

#[test]
fn explosion_fields_change_between_bursts() {
    run_events("explosion");
}

/// X_ITE-derived numbers of the maxParticles scenario, written out from the baseline: the buffer keeps the
/// first particles on a resize, `enabled` FALSE empties it, isActive follows.
#[test]
fn max_particles_scenario_numbers() {
    big(|| {
        let o = oracle();
        let script = json(&o.join("synthetic-rea-b06/events/particles-events-maxparticles.json"));
        let base = Baseline::load("particles-events-maxparticles");
        let run = replay(
            &o.join("synthetic-rea-b06/events/particles-events-maxparticles.x3dv"),
            &script,
            base.seeds(),
        );
        let at = |f: usize| &run.steps[f - 1];

        // (frame, numParticles, maxParticles, isActive) as read from X_ITE.
        for (frame, num, max, active) in [
            (24, 8, 12, true),
            (25, 8, 20, true),
            (49, 20, 20, true),
            (50, 6, 6, true),
            (59, 6, 6, true),
            (60, 0, 6, false),
            (69, 0, 6, false),
            (70, 0, 6, true),
            (85, 0, 0, false),
            (90, 0, 10, true),
        ] {
            let s = at(frame);

            assert_eq!(
                (s.num_particles, s.max_particles, s.is_active),
                (num, max, active),
                "frame {frame}"
            );
        }

        // The oracle's creationTime is reset to `Date.now ()` by every set_maxParticles__: after frames 25, 50, 60, 70, 85, 90.
        for (frame, creation) in [
            (25, 0.8333333731),
            (50, 1.6666666269),
            (60, 2.0),
            (70, 2.3333333731),
            (85, 2.8333333731),
            (90, 3.0),
        ] {
            assert!(
                (at(frame).creation_time - 946684800.0 - creation).abs() < 1e-9,
                "frame {frame}: creationTime {}",
                at(frame).creation_time - 946684800.0
            );
        }
    });
}
