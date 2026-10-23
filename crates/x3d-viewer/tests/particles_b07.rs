//! REA batch 07 (particles): ForcePhysicsModel, WindPhysicsModel and PolylineEmitter.
//!
//! 1. `oracle_cases_match_x_ite`: every case of oracle/synthetic-rea-b07-ps (95 worlds: defaults, non-default
//!    fields, 24 / 30 / 50 / 60 / 75 / 120 / 144 Hz, enabled toggling, run-time field changes, combinations,
//!    legacy specification versions, degenerate polylines, 20000 particles) is run through the real runtime
//!    (`x3d-particles-drive`, the binary of this package) with the Math.random stream X_ITE recorded
//!    (oracle/rea-batch07-ps-baseline, written by rea_b07_ps.py from pinned X_ITE 16.5.0, never from Rust)
//!    and compared with X_ITE: every particle of every dumped frame within the 06R limits, the per-step counts,
//!    the force texture, the polylines texture, the scene text and the console log.
//! 2. Lifecycle: the `polylines` cache and the particle buffers exist while a system runs and are gone
//!    with it (Inline load / reload / unload, removal from a live Group, Script-created instances), they
//!    follow coord / emitter changes without growing, and every counter is flat over 100 cycles.
//!
//! The comparison needs oracle/rea-batch07-ps-baseline; re-create it with
//! `python3 -I oracle/tools/gen_rea_b07_ps.py && python3 -I oracle/tools/rea_b07_ps.py baseline`.

mod particles_b06_common;

use particles_b06_common::{GROUPS, STRIDE, big, f32s, json, oracle};
use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::nodes::State;
use x3d_runtime::{Cb, Runtime};
use x3d_scene::{CtxId, ROOT};
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

static LOCK: Mutex<()> = Mutex::new(());

// ------------------------------------------------------------------------------------------------
// 1. the oracle comparison

/// The velocity / position tolerance of the cases generated with `tol: "strong"` (see rea_b07_ps.py TOL_STRONG).
const STRONG: [(&str, f64, f64); 2] = [("velocity", 1e-4, 2e-5), ("position", 1e-4, 2e-5)];

fn dumps(js: &Value, data: &[f32]) -> Vec<(u64, Vec<[f32; STRIDE]>)> {
    let mut out: Vec<(u64, Vec<[f32; STRIDE]>)> = js["dumps"]
        .as_object()
        .unwrap()
        .iter()
        .map(|(k, d)| {
            let (n, off) = (
                d["numParticles"].as_u64().unwrap() as usize,
                d["offset"].as_u64().unwrap() as usize,
            );

            (
                k.parse().unwrap(),
                (0..n)
                    .map(|i| {
                        data[off + i * STRIDE..off + (i + 1) * STRIDE]
                            .try_into()
                            .unwrap()
                    })
                    .collect(),
            )
        })
        .collect();

    out.sort_by_key(|d| d.0);
    out
}

/// A force texture value as the comparison sees it: float32, null for non-finite.
fn force_value(v: &Value) -> Option<f32> {
    v.as_f64().map(|x| x as f32)
}

struct Outcome {
    name: String,
    fails: Vec<String>,
    max: Vec<f64>,
}

fn run_case(driver: &Path, tmp: &Path, name: &str) -> Outcome {
    let o = oracle();
    let base_dir = o.join("rea-batch07-ps-baseline").join(name);
    let script_path = o
        .join("synthetic-rea-b07-ps/scripts")
        .join(format!("{name}.json"));
    let script = json(&script_path);
    let out = tmp.join(format!("{name}.json"));
    let mut fails = vec![];
    let mut max = vec![0.0f64; GROUPS.len()];
    let run = Command::new(driver)
        .arg(o.join("synthetic-rea-b07-ps").join(format!("{name}.x3dv")))
        .arg(&script_path)
        .arg("--seeds-from")
        .arg(base_dir.join("js.json"))
        .arg("--out")
        .arg(&out)
        .output()
        .expect("driver");

    if !run.status.success() || !out.exists() {
        return Outcome {
            name: name.into(),
            fails: vec![format!(
                "driver failed: {}",
                String::from_utf8_lossy(&run.stderr)
            )],
            max,
        };
    }

    let (base, rs) = (json(&base_dir.join("js.json")), json(&out));
    let (bd, rd) = (
        dumps(&base, &f32s(&base_dir.join("js.f32"))),
        dumps(&rs, &f32s(&out.with_extension("f32"))),
    );
    let strong = script["tol"].as_str() == Some("strong");
    let (bs, rsteps) = (
        base["steps"].as_array().unwrap(),
        rs["steps"].as_array().unwrap(),
    );

    if bs.len() != rsteps.len() {
        fails.push(format!("steps: {} vs {}", bs.len(), rsteps.len()));
    }

    for (b, r) in bs.iter().zip(rsteps) {
        let f = b["frame"].as_u64().unwrap();

        for key in ["numParticles", "isActive", "maxParticles", "numForces"] {
            if b.get(key).is_some() && b[key] != r[key] {
                fails.push(format!(
                    "frame {f}: {key} X_ITE {}, port {}",
                    b[key], r[key]
                ));
            }
        }

        if b["createParticles"].as_bool() != r["createParticles"].as_bool() {
            fails.push(format!("frame {f}: createParticles"));
        }

        let drawn = b["forceDraws"].as_array().map_or(0, Vec::len);

        if r["forceDrawsUsed"]
            .as_u64()
            .map(|u| u as usize)
            .unwrap_or(0)
            != drawn
        {
            fails.push(format!(
                "frame {f}: force draws used {} vs {drawn}",
                r["forceDrawsUsed"]
            ));
        }

        let (bf, rf): (Vec<_>, Vec<_>) = (
            b["forces"]
                .as_array()
                .unwrap()
                .iter()
                .map(force_value)
                .collect(),
            r["forces"]
                .as_array()
                .unwrap()
                .iter()
                .map(force_value)
                .collect(),
        );

        if bf != rf {
            fails.push(format!(
                "frame {f}: force texture X_ITE {bf:?}, port {rf:?}"
            ));
        }

        let (dt, rt) = (
            b["deltaTime"].as_f64().unwrap(),
            r["deltaTime"].as_f64().unwrap(),
        );

        if (dt - rt).abs() > 1e-12 * dt.abs() {
            fails.push(format!("frame {f}: deltaTime {dt} vs {rt}"));
        }

        if (b["creationTime"].as_f64().unwrap() - r["creationTime"].as_f64().unwrap()).abs() > 1e-9
        {
            fails.push(format!("frame {f}: creationTime"));
        }
    }

    for (frame, rows) in &bd {
        let Some((_, got)) = rd.iter().find(|d| d.0 == *frame) else {
            fails.push(format!("no dump for frame {frame}"));
            continue;
        };

        if rows.len() != got.len() {
            fails.push(format!(
                "frame {frame}: {} records, X_ITE {}",
                got.len(),
                rows.len()
            ));
            continue;
        }

        for (i, (a, b)) in rows.iter().zip(got).enumerate() {
            for (gi, (g, cols, abs, rel)) in GROUPS.iter().enumerate() {
                let (abs, rel) = STRONG
                    .iter()
                    .find(|s| strong && s.0 == *g)
                    .map_or((*abs, *rel), |s| (s.1, s.2));

                for &c in *cols {
                    let (x, y) = (f64::from(a[c]), f64::from(b[c]));

                    if x.is_nan() != y.is_nan() {
                        fails.push(format!(
                            "frame {frame} particle {i} {g}[{c}]: X_ITE {x}, port {y}"
                        ));
                    } else if !x.is_nan() {
                        let d = (x - y).abs();

                        max[gi] = max[gi].max(d);

                        if d > abs + rel * x.abs() {
                            fails.push(format!("frame {frame} particle {i} {g}[{c}]: X_ITE {x}, port {y}, error {d:e}"));
                        }
                    }
                }
            }

            if fails.len() > 8 {
                break;
            }
        }
    }

    // The polylines texture: the length texels hold only .x for the shader (X_ITE reuses its array without clearing it).
    match (
        base["geometry"].get("polyline"),
        rs["geometry"].get("polyline"),
    ) {
        (None, None) => {}
        (Some(b), Some(r)) => {
            let vi = b["verticesIndex"].as_i64().unwrap();

            if vi != r["verticesIndex"].as_i64().unwrap() {
                fails.push(format!(
                    "polylines verticesIndex {vi} vs {}",
                    r["verticesIndex"]
                ));
            }

            for (i, x) in b["array"].as_array().unwrap().iter().enumerate() {
                if (i / 4) as i64 >= vi && vi >= 0 || i % 4 == 0 {
                    let (x, y) = (
                        x.as_f64().unwrap() as f32,
                        r["array"][i].as_f64().unwrap() as f32,
                    );

                    if x != y {
                        fails.push(format!("polylines texture float {i}: X_ITE {x}, port {y}"));
                    }
                }
            }
        }
        (b, r) => fails.push(format!(
            "polylines texture: X_ITE {}, port {}",
            b.is_some(),
            r.is_some()
        )),
    }

    for key in ["vrmlLoad", "vrmlEnd"] {
        if base[key] != rs[key] {
            fails.push(format!("scene text {key} differs"));
        }
    }

    let sorted = |v: &Value| {
        let mut l: Vec<String> = v
            .as_array()
            .map(|a| {
                a.iter()
                    .map(|x| x.as_str().unwrap_or("").to_string())
                    .collect()
            })
            .unwrap_or_default();

        l.sort();
        l
    };

    if sorted(&base["console"]) != sorted(&rs["console"]) {
        fails.push(format!(
            "console: X_ITE {:?}, port {:?}",
            sorted(&base["console"]),
            sorted(&rs["console"])
        ));
    }

    fails.truncate(8);
    Outcome {
        name: name.into(),
        fails,
        max,
    }
}

#[test]
fn oracle_cases_match_x_ite() {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());
    let o = oracle();
    let mut names: Vec<String> = std::fs::read_dir(o.join("synthetic-rea-b07-ps/scripts"))
        .expect("scripts")
        .filter_map(|e| {
            e.ok()?
                .file_name()
                .to_str()?
                .strip_suffix(".json")
                .map(String::from)
        })
        .filter(|n| n.starts_with("ps-"))
        .collect();

    names.sort();
    assert!(names.len() >= 90, "{} cases", names.len());

    let driver = PathBuf::from(env!("CARGO_BIN_EXE_x3d-particles-drive"));
    let tmp = std::env::temp_dir().join(format!("x3d-b07-oracle-{}", std::process::id()));

    std::fs::create_dir_all(&tmp).unwrap();

    let results: Mutex<Vec<Outcome>> = Mutex::new(vec![]);
    let next = Mutex::new(0usize);

    std::thread::scope(|s| {
        for _ in 0..8 {
            s.spawn(|| {
                loop {
                    let i = {
                        let mut n = next.lock().unwrap();
                        let i = *n;

                        *n += 1;
                        i
                    };

                    let Some(name) = names.get(i) else { break };
                    let r = run_case(&driver, &tmp, name);

                    results.lock().unwrap().push(r);
                }
            });
        }
    });

    let mut results = results.into_inner().unwrap();

    results.sort_by(|a, b| a.name.cmp(&b.name));

    let mut bad = vec![];
    let mut max = vec![0.0f64; GROUPS.len()];

    for r in &results {
        if !r.fails.is_empty() {
            bad.push(format!("{}: {}", r.name, r.fails.join(" | ")));
        }

        for (m, x) in max.iter_mut().zip(&r.max) {
            *m = m.max(*x);
        }
    }

    println!(
        "MAXERR\tps-oracle\t{} cases\t{}",
        results.len(),
        GROUPS
            .iter()
            .zip(&max)
            .map(|(g, m)| format!("{}={m:.2e}", g.0))
            .collect::<Vec<_>>()
            .join("\t")
    );
    let _ = std::fs::remove_dir_all(&tmp);
    assert!(
        bad.is_empty(),
        "{} of {} cases differ from X_ITE:\n{}",
        bad.len(),
        results.len(),
        bad.join("\n")
    );
}

// ------------------------------------------------------------------------------------------------
// 2. lifecycle

const T0: f64 = 946684800.0;
const DT: f64 = 1.0 / 60.0;
const HEADER: &str = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:2\n";

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

#[derive(Debug, PartialEq, Eq, Clone)]
struct Resources {
    systems: usize,
    buffer_floats: usize,
    /// Texels cached for polylines / surfaces / bounded models of all systems.
    cache_texels: usize,
    animate_interests: usize,
    live_watchers: usize,
    live_nodes: usize,
    arena: usize,
    nrt: usize,
    frt: usize,
    contexts: usize,
}

fn resources(rt: &Runtime) -> Resources {
    let (mut systems, mut floats, mut texels) = (0, 0, 0);

    for node in 0..rt.nrt.len() as NodeId {
        if let Some(sim) = rt.particle_sim(node) {
            systems += 1;
            floats += sim.particles.len() * STRIDE;
            texels += rt.particle_cache_texels(node);
        }
    }

    let sensor = rt.private_field(rt.browser_node, "sensorEvents").unwrap();
    let s = &rt.scene;

    Resources {
        systems,
        buffer_floats: floats,
        cache_texels: texels,
        animate_interests: rt.frt.get(&sensor).map_or(0, |f| {
            f.interests
                .iter()
                .filter(|i| matches!(i.cb, Cb::Particle(_)))
                .count()
        }),
        live_watchers: rt.live_watchers.len(),
        live_nodes: s.nodes.len() - s.free_nodes.len(),
        arena: s.nodes.len(),
        nrt: rt.nrt.len(),
        frt: rt.frt.len(),
        contexts: s.contexts.len(),
    }
}

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
        }
    }

    for (i, n) in rt.nrt.iter().enumerate() {
        if let State::ParticleSystem(_) = n.state {
            assert!(
                rt.builtin_name(i as NodeId) == Some("ParticleSystem"),
                "{when}: particle state on node {i} of another type"
            );
        }
    }
}

struct World {
    v: Viewer3d,
    t: f64,
    dir: PathBuf,
}

impl World {
    fn new(tag: &str, files: &[(&str, String)]) -> World {
        let dir = std::env::temp_dir().join(format!("x3d-b07-{}-{tag}", std::process::id()));

        std::fs::create_dir_all(&dir).unwrap();

        for (n, text) in files {
            std::fs::write(dir.join(n), text).unwrap();
        }

        let mut v = Viewer3d::load_attributes(
            &dir.join(files[0].0),
            Some(&dir),
            T0,
            64,
            64,
            Some(1.0 / DT),
            false,
            ORACLE_PAGE_ATTRIBUTES,
        )
        .expect("load");

        v.frame_rate = 1.0 / DT;

        let mut w = World { v, t: T0, dir };

        w.steps(6);
        w
    }

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

    fn set(&mut self, node: NodeId, field: &str, value: FieldValue) {
        let k = self
            .v
            .rt
            .get_field(node, field)
            .unwrap_or_else(|| panic!("no field {field}"));

        self.v.rt.set_value(k, value);
    }

    fn res(&self) -> Resources {
        resources(&self.v.rt)
    }
}

impl Drop for World {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

fn locked(f: impl FnOnce() + Send + 'static) {
    let _g = LOCK.lock().unwrap_or_else(|e| e.into_inner());

    big(f);
}

/// A scene with a PolylineEmitter system (wind + force + a floor) and a second system with a WindPhysicsModel on a
/// PointEmitter. `variant` changes the polylines (different texture sizes).
fn polyline_scene(variant: u32) -> String {
    let (index, points) = if variant == 0 {
        ("0 1 2 3 -1", "0 0 0, 1 0 0, 1 2 0, 3 2 1")
    } else {
        (
            "0 1 -1 2 3 -1 4 5 -1 6 7 8 9 10 -1",
            "0 0 0, 1 0 0, 0 1 0, 3 1 0, 0 2 0, 6 2 0, 0 3 0, 1 3 1, 2 3 3, 3 4 3, 5 5 5",
        )
    };

    format!(
        "{HEADER}\nGroup {{ children [
  DEF P1 ParticleSystem {{ maxParticles 60 particleLifetime 1 emitter DEF EM PolylineEmitter {{ direction 0 1 0 speed 1 mass 1 surfaceArea 2 coordIndex [ {index} ] coord DEF C Coordinate {{ point [ {points} ] }} }}
    physics [ WindPhysicsModel {{ direction 0 0 0 speed 1.5 gustiness 0.3 turbulence 0.3 }} ForcePhysicsModel {{ force 0 -9.8 0 }} BoundedPhysicsModel {{ damping 0.5 geometry Box {{ size 4 4 4 }} }} ] }}
  DEF P2 ParticleSystem {{ maxParticles 20 particleLifetime 1 emitter PointEmitter {{ direction 0 1 0 speed 1 mass 1 surfaceArea 1 }}
    physics [ WindPhysicsModel {{ }} ] }}
] }}\n"
    )
}

#[test]
fn polyline_cache_follows_the_scene_over_100_inline_reloads() {
    locked(|| {
        let mut w = World::new(
            "inline",
            &[
                (
                    "main.x3dv",
                    format!("{HEADER}\nDEF Inl Inline {{ url \"a.x3dv\" }}\n"),
                ),
                ("a.x3dv", polyline_scene(0)),
                ("b.x3dv", polyline_scene(1)),
            ],
        );
        let inl = w.named(ROOT, "Inl");
        let mut unloaded = None;
        let mut loaded = None;
        let mut texels = [0usize; 2];

        assert_eq!(w.res().systems, 2);
        assert!(
            w.res().cache_texels > 0,
            "the polylines (and the bounded box) are cached while the system runs"
        );
        assert_no_stale_ids(&w.v.rt, "loaded");

        for c in 1..=100 {
            w.set(inl, "load", FieldValue::SFBool(false));
            w.steps(3);

            let r = w.res();

            assert_eq!(
                (
                    r.systems,
                    r.buffer_floats,
                    r.cache_texels,
                    r.animate_interests
                ),
                (0, 0, 0, 0),
                "cycle {c}: an unloaded scene keeps particle state or texels: {r:?}"
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

            w.set(
                inl,
                "url",
                FieldValue::MFString(vec![if c % 2 == 0 { "a.x3dv" } else { "b.x3dv" }.into()]),
            );
            w.set(inl, "load", FieldValue::SFBool(true));
            w.steps(8);

            let r = w.res();

            assert_eq!((r.systems, r.animate_interests), (2, 2), "cycle {c}: {r:?}");
            assert!(
                r.cache_texels > 0,
                "cycle {c}: the polylines are cached again"
            );
            assert_no_stale_ids(&w.v.rt, "reloaded");

            texels[(c % 2) as usize] = r.cache_texels;

            let ctx = w.v.rt.inline_scenes[&inl];
            let p1 = w.named(ctx, "P1");
            let sim = w.v.rt.particle_sim(p1).unwrap();

            assert!(
                sim.num_particles > 0 && sim.num_forces == 2,
                "cycle {c}: wind and force rows: {}",
                sim.num_forces
            );
            assert!(
                w.v.rt
                    .particle_polyline_texture(p1)
                    .is_some_and(|t| t.vertices_index > 0)
            );

            if [10, 50, 100].contains(&c) {
                println!(
                    "MEM\tps-b07-inline-cycle\t{c}\tsystems={}\tbuffer_floats={}\tcache_texels={}\tanimate_interests={}\tlive_watchers={}\tlive_nodes={}\tarena={}\tnrt={}\tfrt={}\trss_kib={}",
                    r.systems,
                    r.buffer_floats,
                    r.cache_texels,
                    r.animate_interests,
                    r.live_watchers,
                    r.live_nodes,
                    r.arena,
                    r.nrt,
                    r.frt,
                    rss_kib()
                );
            }

            if c == 10 || c == 11 {
                loaded = Some((c % 2, r.clone()));
            }

            if let Some((parity, b)) = &loaded {
                if c % 2 == *parity {
                    assert_eq!(&r, b, "cycle {c}: a table grew after the warm-up (loaded)");
                }
            }
        }

        assert_ne!(
            texels[0], texels[1],
            "the two scenes have different polylines"
        );
    });
}

/// The cache holds the polylines of the CURRENT coord / coordIndex only: it follows size changes both ways, goes with a
/// switch to an emitter without a texture and comes back, and a hidden (empty) polyline has a one texel texture.
#[test]
fn polyline_cache_tracks_coord_changes_without_growing() {
    locked(|| {
        let text = format!(
            "{HEADER}
DEF P1 ParticleSystem {{ maxParticles 30 particleLifetime 1 emitter DEF EM PolylineEmitter {{ direction 0 1 0 speed 1 coordIndex [ 0 1 2 -1 ] coord DEF C Coordinate {{ point [ 0 0 0, 1 0 0, 2 0 0 ] }} }} }}
Shape {{ geometry IndexedLineSet {{ coordIndex [ 0 1 -1 ] coord DEF C2 Coordinate {{ point [ 0 0 0, 5 0 0 ] }} }} }}
DEF P2 ParticleSystem {{ maxParticles 5 emitter DEF PE PointEmitter {{ }} }}\n"
        );
        let mut w = World::new("coords", &[("w.x3dv", text)]);
        let (p1, em, c, c2, pe) = (
            w.named(ROOT, "P1"),
            w.named(ROOT, "EM"),
            w.named(ROOT, "C"),
            w.named(ROOT, "C2"),
            w.named(ROOT, "PE"),
        );
        let size = |vertices: usize| {
            let n = vertices / 2 + 1 + vertices;

            (n as f64).sqrt().ceil() as usize * (n as f64).sqrt().ceil() as usize
        };
        let texels = |w: &World| w.v.rt.particle_polyline_texture(p1).map(|t| t.array.len());

        // 2 segments = 4 vertices.
        assert_eq!(texels(&w), Some(size(4)));

        // Grow and shrink the point list (index 0..N-1) many times: always exactly the current polyline.
        for round in 0..200usize {
            let n = 2 + (round * 7) % 40;
            let points: Vec<f32> = (0..n)
                .flat_map(|i| [i as f32, (i % 3) as f32, 0.0])
                .collect();
            let index: Vec<i32> = (0..n as i32).chain([-1]).collect();

            w.set(c, "point", FieldValue::MFVec3f(points));
            w.set(em, "set_coordIndex", FieldValue::MFInt32(index));
            w.steps(1);
            assert_eq!(
                texels(&w),
                Some(size((n - 1) * 2)),
                "round {round}: {n} points"
            );
        }

        // Another coord node, then none: no vertices, a one texel texture.
        w.set(em, "coord", FieldValue::SFNode(Some(c2)));
        w.set(em, "set_coordIndex", FieldValue::MFInt32(vec![0, 1, -1]));
        w.steps(1);
        assert_eq!(texels(&w), Some(size(2)), "C2 has one segment");
        w.set(em, "coord", FieldValue::SFNode(None));
        w.steps(1);
        assert_eq!(texels(&w), Some(1));
        assert!(w.v.rt.particle_polyline_texture(p1).unwrap().vertices_index < 0);

        // The emitter replaced by a PointEmitter: no polylines at all.
        w.set(p1, "emitter", FieldValue::SFNode(Some(pe)));
        w.steps(1);
        assert_eq!(texels(&w), None);
        assert_eq!(
            w.v.rt.particle_cache_texels(p1),
            0,
            "the polylines were dropped with the emitter"
        );

        // And back.
        w.set(p1, "emitter", FieldValue::SFNode(Some(em)));
        w.set(em, "coord", FieldValue::SFNode(Some(c)));
        w.steps(1);
        assert!(texels(&w).is_some_and(|t| t > 1));
    });
}

const SCRIPT: &str = "
  function mk () { var S = Browser .currentScene, sh = S .createNode ('Shape'), ps = S .createNode ('ParticleSystem'), em = S .createNode ('PolylineEmitter'), co = S .createNode ('Coordinate'), wm = S .createNode ('WindPhysicsModel'), fm = S .createNode ('ForcePhysicsModel');
    co .point = new MFVec3f (new SFVec3f (0, 0, 0), new SFVec3f (1, 0, 0), new SFVec3f (1, 2, 0), new SFVec3f (3, 2, 1));
    em .coordIndex = new MFInt32 (0, 1, 2, 3, -1); em .coord = co; em .speed = 1; em .mass = 1; em .surfaceArea = 2; em .direction = new SFVec3f (0, 1, 0);
    wm .speed = 1.5; wm .direction = new SFVec3f (0, 0, 0); fm .force = new SFVec3f (0, -9.8, 0);
    ps .maxParticles = 30; ps .particleLifetime = 1; ps .emitter = em; ps .physics = new MFNode (wm, fm); sh .geometry = ps; return sh; }
  function add (v)   { grp .children = new MFNode (mk ()); }
  function clear (v) { grp .children = new MFNode (); }
";

fn script_world() -> String {
    format!(
        "{HEADER}\nDEF GRP Group {{ }}\nDEF SC Script {{ directOutput TRUE inputOnly SFBool add inputOnly SFBool clear initializeOnly SFNode grp USE GRP\n url \"ecmascript:{SCRIPT}\" }}\n"
    )
}

fn call(w: &mut World, name: &str) {
    let sc = w.named(ROOT, "SC");

    w.set(sc, name, FieldValue::SFBool(true));
    w.steps(2);
}

fn system_in(w: &World) -> NodeId {
    let grp = w.named(ROOT, "GRP");
    let shape = match w.v.rt.pre_value(grp, "children") {
        Some(FieldValue::MFNode(c)) => c.iter().flatten().copied().next().expect("a child"),
        o => panic!("children: {o:?}"),
    };

    match w.v.rt.pre_value(shape, "geometry") {
        Some(FieldValue::SFNode(Some(ps))) => *ps,
        o => panic!("geometry: {o:?}"),
    }
}

/// A Script builds a polyline system, puts it into a live Group and clears the Group, 100 times: the system, its
/// polylines texture, its emitter, coord and both physics models go with the Group's children.
#[test]
fn script_created_polyline_systems_are_released_100_cycles() {
    locked(|| {
        let mut w = World::new("script", &[("w.x3dv", script_world())]);

        // The first release makes the runtime's tombstone node (once): that is the baseline.
        call(&mut w, "add");
        call(&mut w, "clear");

        let base = w.res();
        let mut at10 = None;
        let mut rss10 = 0;

        for c in 1..=100 {
            call(&mut w, "add");
            w.steps(3);

            let live = w.res();
            let ps = system_in(&w);

            assert_eq!(
                live.systems,
                base.systems + 1,
                "cycle {c}: the system in the Group runs"
            );
            assert!(
                live.cache_texels > base.cache_texels && live.buffer_floats > base.buffer_floats,
                "cycle {c}: polylines and particles exist while it runs"
            );
            assert_eq!(
                w.v.rt.particle_sim(ps).unwrap().num_forces,
                2,
                "cycle {c}: wind and force"
            );
            assert!(w.v.rt.particle_sim(ps).unwrap().num_particles > 0);

            call(&mut w, "clear");

            assert!(
                !w.v.rt.scene.is_live(ps) && w.v.rt.particle_sim(ps).is_none(),
                "cycle {c}: the removed system {ps} still lives"
            );
            assert_eq!(
                w.v.rt.particle_cache_texels(ps),
                0,
                "cycle {c}: the polylines of the removed system are gone"
            );
            assert_no_stale_ids(&w.v.rt, "cleared");

            if c == 10 {
                at10 = Some(w.res());
                rss10 = rss_kib();
            }

            if [10, 50, 100].contains(&c) {
                let r = w.res();

                println!(
                    "MEM\tps-b07-script-cycle\t{c}\tsystems={}\tbuffer_floats={}\tcache_texels={}\tanimate_interests={}\tlive_watchers={}\tlive_nodes={}\tarena={}\tnrt={}\tfrt={}\trss_kib={}",
                    r.systems,
                    r.buffer_floats,
                    r.cache_texels,
                    r.animate_interests,
                    r.live_watchers,
                    r.live_nodes,
                    r.arena,
                    r.nrt,
                    r.frt,
                    rss_kib()
                );
            }

            if let Some(b) = &at10 {
                assert_eq!(&w.res(), b, "cycle {c}: a counter grew after the warm-up");
            }
        }

        let b = at10.unwrap();

        assert_eq!(
            (
                b.systems,
                b.buffer_floats,
                b.cache_texels,
                b.animate_interests,
                b.live_watchers
            ),
            (
                base.systems,
                base.buffer_floats,
                base.cache_texels,
                base.animate_interests,
                base.live_watchers
            )
        );
        assert_eq!(
            b.live_nodes, base.live_nodes,
            "every node of every cycle is gone (system, emitter, coord, 2 models, shape)"
        );

        let grown = rss_kib().saturating_sub(rss10);

        assert!(
            grown <= 1024,
            "RSS grew by {grown} KiB over the last 90 cycles"
        );
    });
}

/// A system written into the world (not made by a Script) and removed from a live Group: like X_ITE (oracle case
/// ps-polyline-group-removal) it keeps running, because its liveness is that of its execution context, and nothing
/// grows; it is not released (a DEF name refers to it). Its animation is compared with X_ITE by the oracle test.
#[test]
fn a_world_system_removed_from_its_group_runs_on_without_growing() {
    locked(|| {
        let text = format!(
            "{HEADER}
DEF GRP Group {{ children [ DEF P1 ParticleSystem {{ maxParticles 30 particleLifetime 1 emitter PolylineEmitter {{ direction 0 1 0 speed 1 mass 1 surfaceArea 2 coordIndex [ 0 1 2 -1 ] coord Coordinate {{ point [ 0 0 0, 1 0 0, 2 0 0 ] }} }}
  physics [ WindPhysicsModel {{ direction 0 0 0 speed 1.5 }} ] }} ] }}\n"
        );
        let mut w = World::new("group", &[("w.x3dv", text)]);
        let (grp, p1) = (w.named(ROOT, "GRP"), w.named(ROOT, "P1"));

        w.steps(20);

        let before = w.res();

        assert_eq!((before.systems, before.animate_interests), (1, 1));
        assert!(w.v.rt.particle_sim(p1).unwrap().num_particles > 0 && before.cache_texels > 0);

        let born = w.v.rt.particle_sim(p1).unwrap().particles.clone();

        for round in 0..50 {
            w.set(grp, "children", FieldValue::MFNode(vec![]));
            w.steps(3);
            assert_eq!(
                w.res().systems,
                1,
                "round {round}: the named system is kept"
            );
            w.set(grp, "children", FieldValue::MFNode(vec![Some(p1)]));
            w.steps(3);
        }

        let after = w.res();

        assert_eq!(
            (
                after.systems,
                after.buffer_floats,
                after.cache_texels,
                after.animate_interests,
                after.live_nodes,
                after.nrt,
                after.frt
            ),
            (
                before.systems,
                before.buffer_floats,
                before.cache_texels,
                before.animate_interests,
                before.live_nodes,
                before.nrt,
                before.frt
            )
        );
        assert_ne!(
            w.v.rt.particle_sim(p1).unwrap().particles,
            born,
            "it ran all the time"
        );
        assert_no_stale_ids(&w.v.rt, "removed and added");
    });
}
