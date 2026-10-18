//! Shared by the particles_b06_* tests: replay a baseline case of X_ITE (oracle/rea-batch06-baseline)
//! through the real runtime (Viewer3d, virtual time, the baseline's `Math.random` seeds) and compare
//! the transform feedback state with X_ITE's.
#![allow(dead_code)]

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::particles;
use x3d_scene::ROOT;
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

pub const STRIDE: usize = 28;

pub fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

pub fn big(f: impl FnOnce() + Send + 'static) {
    std::thread::Builder::new().stack_size(256 << 20).spawn(f).unwrap().join().unwrap();
}

pub fn json(path: &Path) -> Value {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_else(|e| panic!("{}: {e}", path.display()))).unwrap()
}

pub fn f32s(path: &Path) -> Vec<f32> {
    std::fs::read(path).unwrap_or_else(|e| panic!("{}: {e}", path.display())).chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect()
}

/// One frame of a run: what `rea_b06_particles.py` compares per step.
#[derive(Clone, Debug)]
pub struct Step {
    pub frame: u64,
    pub num_particles: usize,
    pub delta_time: f64,
    pub creation_time: f64,
    pub create_particles: bool,
    pub is_active: bool,
    pub max_particles: usize,
}

pub struct Run {
    pub steps: Vec<Step>,
    /// frame -> the dumped records (all of 0 .. numParticles, or only the born ones).
    pub dumps: BTreeMap<u64, Vec<[f32; STRIDE]>>,
}

/// A baseline: the X_ITE run (js.json and js.f32 or samples.f32) of a case.
pub struct Baseline {
    pub js: Value,
    pub dir: PathBuf,
}

impl Baseline {
    pub fn load(name: &str) -> Baseline {
        let dir = oracle().join("rea-batch06-baseline").join(name);

        Baseline { js: json(&dir.join("js.json")), dir }
    }

    pub fn steps(&self) -> &Vec<Value> {
        self.js["steps"].as_array().unwrap()
    }

    /// The recorded `randomSeed` uniform of every emitter pass, in order.
    pub fn seeds(&self) -> Vec<f64> {
        self.steps().iter().filter_map(|s| s["randomSeed"].as_f64()).collect()
    }

    /// frame -> records (exact cases: js.f32 with the dump index of js.json).
    pub fn dumps(&self) -> BTreeMap<u64, Vec<[f32; STRIDE]>> {
        let data = f32s(&self.dir.join(self.js["fileF32"].as_str().unwrap()));
        let mut out = BTreeMap::new();

        for (k, d) in self.js["dumps"].as_object().unwrap() {
            let (n, off) = (d["numParticles"].as_u64().unwrap() as usize, d["offset"].as_u64().unwrap() as usize);
            let rows = (0..n).map(|i| data[off + i * STRIDE..off + (i + 1) * STRIDE].try_into().unwrap()).collect();

            out.insert(k.parse().unwrap(), rows);
        }

        out
    }
}

fn set_field(v: &mut Viewer3d, node: NodeId, field: &str, value: &Value) {
    let k = v.rt.get_field(node, field).unwrap_or_else(|| panic!("no field {field}"));
    let new = match v.rt.value(k) {
        FieldValue::SFBool(_) => FieldValue::SFBool(value.as_bool().unwrap()),
        FieldValue::SFFloat(_) => FieldValue::SFFloat(value.as_f64().unwrap() as f32 as f64),
        FieldValue::SFInt32(_) => FieldValue::SFInt32(value.as_i64().unwrap() as i32),
        FieldValue::SFTime(_) => FieldValue::SFTime(value.as_f64().unwrap()),
        other => panic!("field {field} of type {other:?} is not supported by the events"),
    };

    v.rt.set_value(k, new);
}

pub struct Loaded {
    pub v: Viewer3d,
    pub ps: NodeId,
    pub t0: f64,
    pub dt: f64,
}

/// Load `world` like the driver: live at t0, the warm-up frames, then the ParticleSystem is reset to the
/// state before its first animate.
pub fn load(world: &Path, script: &Value) -> Loaded {
    let t0 = script["t0"].as_f64().unwrap_or(946684800.0);
    let dt = script["dt"].as_f64().unwrap_or(1.0 / 60.0);
    let name = script["node"].as_str().unwrap_or("PS");

    particles::use_random_seeds();

    let mut v = Viewer3d::load_attributes(world, None, t0, 256, 256, Some(1.0 / dt), false, ORACLE_PAGE_ATTRIBUTES).expect("load");

    v.frame_rate = 1.0 / dt;

    for _ in 0..script["warmup"].as_u64().unwrap_or(8) {
        v.step(t0);
    }

    let ps = v.rt.scene.contexts[ROOT].named_nodes.get(name).unwrap_or_else(|| panic!("no node {name}"));

    v.rt.particle_system_reset(ps, t0);
    Loaded { v, ps, t0, dt }
}

/// Run the script's frames: field `events` are applied before the frame's step. The seeds are the baseline's.
pub fn replay(world: &Path, script: &Value, seeds: Vec<f64>) -> Run {
    let mut l = load(world, script);
    let frames = script["frames"].as_u64().unwrap_or(1);
    let born_only = script["filter"].as_str() == Some("born");
    let dump_all = script["dumpAll"].as_bool().unwrap_or(false);
    let dump: Vec<u64> = script["dump"].as_array().map(|a| a.iter().filter_map(Value::as_u64).collect()).unwrap_or_else(|| vec![frames]);
    let events = script["events"].as_array().cloned().unwrap_or_default();
    let mut run = Run { steps: vec![], dumps: BTreeMap::new() };

    particles::set_seed_list(seeds);

    for k in 1..=frames {
        for e in events.iter().filter(|e| e["frame"].as_u64() == Some(k)) {
            let node = l.v.rt.scene.contexts[ROOT].named_nodes.get(e["node"].as_str().unwrap()).expect("event node");

            set_field(&mut l.v, node, e["field"].as_str().unwrap(), &e["value"]);
        }

        let now = l.t0 + k as f64 * l.dt;

        l.v.date_now = now;
        l.v.step(now);

        let sim = l.v.rt.particle_sim(l.ps).expect("ParticleSystem state");

        run.steps.push(Step {
            frame: k,
            num_particles: sim.num_particles,
            delta_time: sim.delta_time,
            creation_time: sim.creation_time,
            create_particles: sim.create_particles,
            is_active: l.v.rt.get_bool(l.ps, "isActive"),
            max_particles: sim.max_particles,
        });

        if dump_all || dump.contains(&k) {
            let rows = sim.particles.iter().take(sim.num_particles).filter(|p| !born_only || p[2] == 0.0).copied().collect();

            run.dumps.insert(k, rows);
        }
    }

    assert!(!particles::seed_list_exhausted(), "the run used more random seeds than the X_ITE baseline recorded");
    particles::use_random_seeds();
    run
}

/// The comparison of `rea_b06_particles.py` TOL: abs + rel * |reference|.
pub const GROUPS: [(&str, &[usize], f64, f64); 6] = [
    ("life", &[0], 0.0, 0.0),
    ("lifetime", &[1], 0.0, 0.0),
    ("elapsed", &[2], 0.0, 0.0),
    ("color", &[4, 5, 6, 7], 0.0, 0.0),
    ("velocity", &[8, 9, 10], 3e-6, 1e-6),
    ("position", &[24, 25, 26, 27], 5e-6, 2e-6),
];

/// Max abs error per group over all dumps, panicking with the first violation of the tolerance.
pub fn compare_dumps(label: &str, reference: &BTreeMap<u64, Vec<[f32; STRIDE]>>, got: &BTreeMap<u64, Vec<[f32; STRIDE]>>) -> Vec<(&'static str, f64)> {
    let mut max = vec![0.0f64; GROUPS.len()];

    for (frame, rows) in reference {
        let g = got.get(frame).unwrap_or_else(|| panic!("{label}: no dump for frame {frame}"));

        assert_eq!(rows.len(), g.len(), "{label}: frame {frame}: {} records, X_ITE has {}", g.len(), rows.len());

        for (i, (a, b)) in rows.iter().zip(g).enumerate() {
            for (gi, (name, cols, abs, rel)) in GROUPS.iter().enumerate() {
                for &c in *cols {
                    let (x, y) = (f64::from(a[c]), f64::from(b[c]));

                    assert_eq!(x.is_nan(), y.is_nan(), "{label}: frame {frame} particle {i} {name}[{c}]: X_ITE {x}, port {y}");

                    if x.is_nan() {
                        continue;
                    }

                    let d = (x - y).abs();

                    max[gi] = max[gi].max(d);
                    assert!(d <= abs + rel * x.abs(), "{label}: frame {frame} particle {i} {name}[{c}]: X_ITE {x}, port {y}, error {d:e}");
                }
            }
        }
    }

    GROUPS.iter().map(|g| g.0).zip(max).collect()
}

/// The per-step quantities: exact for the counts, 1e-12 relative for deltaTime, 1e-9 for creationTime.
pub fn compare_steps(label: &str, baseline: &Baseline, run: &Run) {
    let b = baseline.steps();

    assert_eq!(b.len(), run.steps.len(), "{label}: step count");

    for (x, s) in b.iter().zip(&run.steps) {
        let f = s.frame;

        assert_eq!(x["numParticles"].as_u64().unwrap() as usize, s.num_particles, "{label}: frame {f}: numParticles");
        assert_eq!(x["createParticles"].as_bool().unwrap(), s.create_particles, "{label}: frame {f}: createParticles");

        let (dt, ct) = (x["deltaTime"].as_f64().unwrap(), x["creationTime"].as_f64().unwrap());

        assert!((dt - s.delta_time).abs() <= 1e-12 * dt.abs(), "{label}: frame {f}: deltaTime X_ITE {dt}, port {}", s.delta_time);
        assert!((ct - s.creation_time).abs() <= 1e-9, "{label}: frame {f}: creationTime X_ITE {ct}, port {}", s.creation_time);

        if let Some(active) = x["isActive"].as_bool() {
            assert_eq!(active, s.is_active, "{label}: frame {f}: isActive");
        }

        if let Some(max) = x["maxParticles"].as_u64() {
            assert_eq!(max as usize, s.max_particles, "{label}: frame {f}: maxParticles");
        }
    }
}

/// `rea_b06_particles.py compact`: the born records as (frame, lifetime, rgba, v, p) rows, at most
/// `cap`, evenly subsampled (numpy `linspace (0, n - 1, cap) .astype (int)`).
pub fn compact(dumps: &BTreeMap<u64, Vec<[f32; STRIDE]>>, cap: usize) -> Vec<[f32; 13]> {
    let mut rows = vec![];

    for (frame, d) in dumps {
        for p in d {
            rows.push([*frame as f32, p[1], p[4], p[5], p[6], p[7], p[8], p[9], p[10], p[24], p[25], p[26], p[27]]);
        }
    }

    if rows.len() > cap {
        let step = (rows.len() - 1) as f64 / (cap - 1) as f64;

        rows = (0..cap).map(|i| rows[if i == cap - 1 { rows.len() - 1 } else { (i as f64 * step) as usize }]).collect();
    }

    rows
}

/// Two-sample Kolmogorov-Smirnov statistic.
pub fn ks2(a: &[f64], b: &[f64]) -> f64 {
    let (mut a, mut b) = (a.to_vec(), b.to_vec());

    a.sort_by(|x, y| x.partial_cmp(y).unwrap());
    b.sort_by(|x, y| x.partial_cmp(y).unwrap());

    let (n, m) = (a.len() as f64, b.len() as f64);
    let (mut i, mut j, mut d) = (0usize, 0usize, 0.0f64);

    while i < a.len() && j < b.len() {
        let x = a[i].min(b[j]);

        while i < a.len() && a[i] <= x {
            i += 1;
        }

        while j < b.len() && b[j] <= x {
            j += 1;
        }

        d = d.max((i as f64 / n - j as f64 / m).abs());
    }

    d
}
