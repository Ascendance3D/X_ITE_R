//! x3d-particles-drive <world.x3dv> <script.json> --seeds-from <baseline js.json> --out <rs.json>
//!
//! Headless run of a ParticleSystem world through the real runtime (REA batch 06R), writing the
//! state of the transform feedback buffer that X_ITE keeps (28 float32 per particle) in the format
//! of oracle/tools/rea_b06_particles.py (see its docstring and
//! .reports/xite-rust-rea-batch-06r/particles-research.md section 7):
//!
//!   rs.json: { "ok": true, "stride": 28, "fileF32": "rs.f32",
//!              "steps": [ { "frame", "numParticles", "deltaTime", "creationTime", "createParticles" } ],
//!              "dumps": { "<frame>": { "numParticles": n, "offset": <first float of the records> } } }
//!   rs.f32:  little-endian float32, n * 28 per dump
//!
//! The script is the one x_ite_particles.mjs reads: dt, t0, frames, warmup, node (DEF name),
//! dump | dumpAll, filter ("born": only the records created in that frame). The per-frame
//! `randomSeed` of the emitter pass is X_ITE's `Math.random () * 0xffffffff`; the baseline's values
//! are injected so that both sides see the same random numbers.
//!
//! REA batch 07 additions (backward compatible; scripts and baselines without them run as before):
//! * `forceDraws` of each baseline step (the `Math.random` values WindPhysicsModel.addForce drew that frame, see
//!   oracle/tools/x_ite_particles_b07.mjs) are replayed frame by frame; the steps record how many were
//!   consumed (`forceDrawsUsed`) and the run fails if a frame used more or fewer than X_ITE.
//! * script `events` (the 06R format, values: bool, number, string, number arrays for vector / MF fields,
//!   `{"def": NAME | null}` for SFNode and `{"nodes": [...]}` for MFNode) are applied before the frame's step.
//! * steps carry `isActive`, `maxParticles`, `numForces`; `geometry.polyline` is the `polylines` texture.
//!
//! `--png-dir <dir>` renders the frame after each dump frame's step into `<dir>/<frame>.png` (`x3d-oracle particles
//! --png-dir`: anim.html's 512 x 512 canvas, `antialiased="false"`; `--png-size WxH` for another size); `--msaa N`
//! renders it like the MSAA page (`antialiased="true" multisampling="N"`). The simulation is the same with or without them.
//!
//! Script key `pickProbes` (`{ "frames": [k, ...], "points": [[x, y], ...] }`, see `x3d-oracle particles`): after each
//! listed frame's step, dump and PNG, `browser .touch` at each point through the GPU pointing pass, as
//! `pickProbes ["k"]`: id, the DEF name of the hit shape node, point, normal, texCoord. Without a GPU the ray cast
//! picks (its double values are not X_ITE's) and `pointingGpu` is false; `--cpu-pointing` picks that way on purpose. `--records-from <baseline js.json>` also picks
//! each listed frame with the baseline's particle records of that frame (its `.f32` dump) in place of the run's own,
//! as `pickProbesReference`, then puts the run's records back: the pointing pass apart from the simulation's float32
//! differences (REA batch 06R: within 1e-5 + 1e-5 * |reference|).

use serde_json::{Value, json};
use std::path::{Path, PathBuf};
use x3d_fields::math::{Vector2, Vector3};
use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::particles::{self, STRIDE};
use x3d_scene::ROOT;
use x3d_viewer::viewer::{ORACLE_PAGE_ATTRIBUTES, Viewer3d};

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opt = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };

    if args.len() < 3 || opt("--out").is_none() {
        eprintln!(
            "usage: x3d-particles-drive <world.x3dv> <script.json> [--seeds-from <baseline js.json>] --out <rs.json>"
        );
        std::process::exit(2);
    }

    let world = PathBuf::from(&args[1]);
    let script: Value = serde_json::from_str(&std::fs::read_to_string(&args[2]).expect("script"))
        .expect("script json");
    let base: Option<Value> = opt("--seeds-from").map(|p| {
        serde_json::from_str(&std::fs::read_to_string(p).expect("baseline")).expect("baseline json")
    });
    // A step without a seed ran no emitter pass (the system was disabled or had stopped emitting): nothing to replay.
    let seeds = base.as_ref().map(|base| {
        base["steps"]
            .as_array()
            .expect("baseline steps")
            .iter()
            .filter_map(|s| s["randomSeed"].as_f64())
            .collect::<Vec<f64>>()
    });
    let force_draws: Option<Vec<Vec<f64>>> = base.as_ref().and_then(|base| {
        let steps = base["steps"].as_array()?;

        steps
            .iter()
            .any(|s| s.get("forceDraws").is_some())
            .then(|| {
                steps
                    .iter()
                    .map(|s| {
                        s["forceDraws"]
                            .as_array()
                            .map(|a| a.iter().filter_map(Value::as_f64).collect())
                            .unwrap_or_default()
                    })
                    .collect()
            })
    });
    let out = PathBuf::from(opt("--out").unwrap());
    let size = opt("--png-size").map(|s| {
        s.split_once('x')
            .and_then(|(w, h)| Some((w.parse().ok()?, h.parse().ok()?)))
            .expect("--png-size WxH")
    });
    let png_dir = opt("--png-dir").map(PathBuf::from);
    let size = size.unwrap_or(if png_dir.is_some() {
        (512, 512)
    } else {
        (256, 256)
    });
    let capture = Capture {
        png_dir,
        msaa: opt("--msaa"),
        size,
    };
    let result = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || run(&world, &script, seeds, force_draws, &out, &capture))
        .unwrap()
        .join()
        .unwrap();

    match result {
        Ok((json, f32s, out)) => {
            let f32_path = out.with_extension("f32");
            let bytes: Vec<u8> = f32s.iter().flat_map(|f: &f32| f.to_le_bytes()).collect();

            std::fs::write(&f32_path, bytes).expect("write f32");
            std::fs::write(&out, serde_json::to_string_pretty(&json).unwrap()).expect("write json");
        }
        Err((e, out)) => {
            std::fs::write(
                &out,
                serde_json::to_string_pretty(&json!({ "ok": false, "error": e })).unwrap(),
            )
            .expect("write json");
            eprintln!("{e}");
            std::process::exit(1);
        }
    }
}

type Output = (Value, Vec<f32>, PathBuf);

/// `--png-dir` and `--msaa`.
struct Capture {
    png_dir: Option<PathBuf>,
    msaa: Option<String>,
    size: (u32, u32),
}

fn run(
    world: &Path,
    script: &Value,
    seeds: Option<Vec<f64>>,
    force_draws: Option<Vec<Vec<f64>>>,
    out: &Path,
    capture: &Capture,
) -> Result<Output, (String, PathBuf)> {
    let fail = |e: String| (e, out.to_path_buf());
    let t0 = script["t0"].as_f64().unwrap_or(946684800.0);
    let dt = script["dt"].as_f64().unwrap_or(1.0 / 60.0);
    let frames = script["frames"].as_u64().unwrap_or(1);
    let warmup = script["warmup"].as_u64().unwrap_or(8);
    let name = script["node"].as_str().unwrap_or("PS");
    let born_only = script["filter"].as_str() == Some("born");
    let dump: Vec<u64> = if script["dumpAll"].as_bool().unwrap_or(false) {
        (1..=frames).collect()
    } else {
        script["dump"]
            .as_array()
            .map(|a| a.iter().filter_map(Value::as_u64).collect())
            .unwrap_or_else(|| vec![frames])
    };

    // The load frames and the warm-up animate with random seeds nobody reads.
    particles::use_random_seeds();

    let mut attributes = ORACLE_PAGE_ATTRIBUTES.to_vec();

    if let Some(n) = capture.msaa.as_deref() {
        attributes.retain(|(a, _)| *a != "antialiased");
        attributes.extend([("antialiased", "true"), ("multisampling", n)]);
    }

    let mut v = Viewer3d::load_attributes(
        world,
        None,
        t0,
        capture.size.0,
        capture.size.1,
        Some(1.0 / dt),
        false,
        &attributes,
    )
    .map_err(fail)?;
    let mut gpu = match &capture.png_dir {
        Some(_) => {
            let mut g = x3d_render::gpu::Gpu::new()
                .map_err(|e| fail(format!("no GPU for --png-dir: {e}")))?;

            v.rendering_properties().apply(&mut g.renderer);
            Some(g)
        }
        None => None,
    };

    v.frame_rate = 1.0 / dt;

    let probe_frames: Vec<u64> = script
        .pointer("/pickProbes/frames")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_u64).collect())
        .unwrap_or_default();
    let probe_points: Vec<(f64, f64)> = script
        .pointer("/pickProbes/points")
        .and_then(Value::as_array)
        .map(|a| {
            a.iter()
                .map(|p| (p[0].as_f64().unwrap_or(0.0), p[1].as_f64().unwrap_or(0.0)))
                .collect()
        })
        .unwrap_or_default();
    let mut probes = serde_json::Map::new();
    let mut reference_probes = serde_json::Map::new();
    let reference = opt_records_from(script).map_err(fail)?;

    // `--cpu-pointing`: the ray cast's double values, like x3d-drive (the interactive viewer picks this way).
    if !probe_frames.is_empty() && !std::env::args().any(|a| a == "--cpu-pointing") {
        match x3d_render::pointing_gpu::PointingGpu::new_headless() {
            Ok(g) => v.pointing_gpu = Some(g),
            Err(e) => {
                eprintln!("x3d-particles-drive: no pointing GPU ({e}); picks use the ray cast")
            }
        }
    }

    for _ in 0..warmup {
        v.step(t0);
    }

    let vrml_load = v.rt.scene.to_vrml_string();

    // Back to the state before the first animateParticles, then the recorded seeds.
    let ps = v.rt.scene.contexts[ROOT]
        .named_nodes
        .get(name)
        .ok_or_else(|| fail(format!("no node named {name}")))?;

    if v.rt.particle_sim(ps).is_none() {
        return Err(fail(format!("{name} is not a ParticleSystem")));
    }

    v.rt.particle_system_reset(ps, t0);

    if let Some(seeds) = seeds {
        particles::set_seed_list(seeds);
    }

    let mut steps = vec![];
    let mut dumps = serde_json::Map::new();
    let mut data: Vec<f32> = vec![];

    let events = script["events"].as_array().cloned().unwrap_or_default();
    let mut log: Vec<String> = vec![];

    for k in 1..=frames {
        let now = t0 + k as f64 * dt;

        for e in events.iter().filter(|e| e["frame"].as_u64() == Some(k)) {
            apply_event(&mut v, e).map_err(fail)?;
        }

        let recorded = force_draws
            .as_ref()
            .map(|f| f.get(k as usize - 1).cloned().unwrap_or_default());

        if let Some(r) = &recorded {
            particles::set_force_draws(r.clone());
        }

        v.date_now = now;
        v.step(now);

        let used = recorded
            .as_ref()
            .map(|r| r.len() - particles::force_draws_left());

        if let (Some(r), Some(u)) = (&recorded, used) {
            if u != r.len() || particles::force_draws_exhausted() {
                log.push(format!(
                    "frame {k}: the run consumed {u}{} force draws, X_ITE drew {}",
                    if particles::force_draws_exhausted() {
                        " or more"
                    } else {
                        ""
                    },
                    r.len()
                ));
            }
        }

        let sim =
            v.rt.particle_sim(ps)
                .ok_or_else(|| fail("the ParticleSystem went away".into()))?;

        steps.push(json!({
            "frame": k,
            "time": now,
            "numParticles": sim.num_particles,
            "deltaTime": sim.delta_time,
            "creationTime": sim.creation_time,
            "createParticles": sim.create_particles,
            "isActive": v.rt.get_bool(ps, "isActive"),
            "maxParticles": sim.max_particles,
            "numForces": sim.num_forces,
            "forces": sim.forces.iter().flatten().collect::<Vec<_>>(),
            "forceDrawsUsed": used,
        }));

        if dump.contains(&k) {
            let offset = data.len();
            let mut count = 0;

            for p in sim.particles.iter().take(sim.num_particles) {
                if born_only && p[2] != 0.0 {
                    continue;
                }

                data.extend_from_slice(p);
                count += 1;
            }

            dumps.insert(
                k.to_string(),
                json!({ "numParticles": count, "total": sim.num_particles, "offset": offset }),
            );

            if let (Some(g), Some(dir)) = (gpu.as_mut(), &capture.png_dir) {
                let frame = v.build_frame();
                let (fw, fh) = v.framebuffer_size();
                let rgba = g.render(&frame, fw, fh);
                let rgba: Vec<u8> = rgba
                    .chunks_exact(4)
                    .flat_map(|p| [p[0], p[1], p[2], 255])
                    .collect();

                std::fs::create_dir_all(dir)
                    .map_err(|e| fail(format!("{}: {e}", dir.display())))?;
                x3d_render::write_png(&dir.join(format!("{k}.png")), fw, fh, &rgba)
                    .map_err(|e| fail(format!("png: {e}")))?;
            }
        }

        if probe_frames.contains(&k) {
            let hits: Vec<Value> = probe_points
                .iter()
                .map(|&(x, y)| probe(&mut v, x, y))
                .collect();

            probes.insert(k.to_string(), Value::Array(hits));

            if let Some((meta, records)) = &reference {
                let dump = &meta["dumps"][k.to_string()];
                let (n, offset) = (
                    dump["numParticles"].as_u64().unwrap_or(0) as usize,
                    dump["offset"].as_u64().unwrap_or(0) as usize,
                );
                let own_n = v.rt.particle_sim(ps).map_or(0, |s| s.num_particles);

                if dump.is_null() || n != own_n || offset + n * STRIDE > records.len() {
                    return Err(fail(format!(
                        "frame {k}: the reference dump has {n} particles at offset {offset}, the run {own_n}"
                    )));
                }

                let own: Vec<particles::Particle> =
                    v.rt.particle_records_mut(ps)
                        .map(|r| r[..n].to_vec())
                        .unwrap_or_default();

                for (i, p) in v.rt.particle_records_mut(ps).unwrap()[..n]
                    .iter_mut()
                    .enumerate()
                {
                    p.copy_from_slice(&records[offset + i * STRIDE..offset + (i + 1) * STRIDE]);
                }

                v.refresh_pointer_scan();
                let hits: Vec<Value> = probe_points
                    .iter()
                    .map(|&(x, y)| probe(&mut v, x, y))
                    .collect();
                reference_probes.insert(k.to_string(), Value::Array(hits));

                v.rt.particle_records_mut(ps).unwrap()[..n].copy_from_slice(&own);
                v.refresh_pointer_scan();
            }
        }
    }

    if particles::seed_list_exhausted() {
        return Err(fail(
            "the baseline has fewer randomSeed values than the run needed".into(),
        ));
    }

    if !log.is_empty() {
        return Err(fail(log.join("; ")));
    }

    particles::use_random_force_draws();

    let mut geometry = serde_json::Map::new();

    if let Some(t) = v.rt.particle_polyline_texture(ps) {
        geometry.insert("polyline".into(), json!({ "verticesIndex": t.vertices_index, "array": t.array.iter().flatten().collect::<Vec<_>>(), "note": "whole texture; the checker compares the first texels X_ITE sampled" }));
    }

    // The ParticleSystem must still be a ParticleSystem with a defined isActive.
    let _ = matches!(v.rt.pre_value(ps, "isActive"), Some(FieldValue::SFBool(_)));

    let mut console = v.rt.log.clone();

    console.sort();

    let file = out.with_extension("f32");
    let rel = file
        .file_name()
        .map(|n| n.to_string_lossy().to_string())
        .unwrap_or_else(|| "rs.f32".into());

    let mut result = json!({ "ok": true, "stride": STRIDE, "fileF32": rel, "steps": steps, "dumps": dumps, "geometry": geometry, "vrmlLoad": vrml_load, "vrmlEnd": v.rt.scene.to_vrml_string(), "console": console, "driver": "x3d-particles-drive" });

    if !probe_frames.is_empty() {
        result["pickProbes"] = Value::Object(probes);
        // false: the ray cast picked (no pointing GPU), whose double values are not X_ITE's.
        result["pointingGpu"] = Value::Bool(v.pointing_gpu.is_some());

        if reference.is_some() {
            result["pickProbesReference"] = Value::Object(reference_probes);
        }
    }

    Ok((result, data, out.to_path_buf()))
}

/// `--records-from <js.json>`: the baseline result and its `.f32` records (`fileF32` beside it). The dumps must hold
/// every particle (no `filter`).
fn opt_records_from(script: &Value) -> Result<Option<(Value, Vec<f32>)>, String> {
    let args: Vec<String> = std::env::args().collect();
    let Some(path) = args
        .iter()
        .position(|a| a == "--records-from")
        .and_then(|i| args.get(i + 1))
        .map(PathBuf::from)
    else {
        return Ok(None);
    };

    if script["filter"].is_string() {
        return Err("--records-from needs dumps of every particle (no filter)".into());
    }

    let meta: Value = serde_json::from_str(
        &std::fs::read_to_string(&path).map_err(|e| format!("{}: {e}", path.display()))?,
    )
    .map_err(|e| format!("{}: {e}", path.display()))?;
    let f32_path = path.with_file_name(meta["fileF32"].as_str().unwrap_or("js.f32"));
    let bytes = std::fs::read(&f32_path).map_err(|e| format!("{}: {e}", f32_path.display()))?;

    Ok(Some((
        meta,
        bytes
            .chunks_exact(4)
            .map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]]))
            .collect(),
    )))
}

/// One `pickProbes` entry: the hit at (x, y) as `x3d-oracle particles` records it (`hit .shapeNode .getName ()`).
fn probe(v: &mut Viewer3d, x: f64, y: f64) -> Value {
    let h = v.probe_pick(x, y);

    if !h.id {
        return json!({ "id": 0 });
    }

    let name = h
        .shape
        .as_ref()
        .and_then(|s| {
            x3d_viewer::walk::rt_at(&v.rt, &s.path)
                .and_then(|rt| rt.scene.nodes.get(s.node as usize))
                .map(|n| n.name.clone())
        })
        .unwrap_or_default();

    json!({ "id": 1, "shape": name, "point": [h.point.x, h.point.y, h.point.z], "normal": [h.normal.x, h.normal.y, h.normal.z], "texCoord": [h.tex_coord.x, h.tex_coord.y, h.tex_coord.z, h.tex_coord.w] })
}

/// Set the field of an `events` entry like `x_ite_particles_b07.mjs` does (SAI-less: straight into the field).
fn apply_event(v: &mut Viewer3d, e: &Value) -> Result<(), String> {
    let named = |v: &Viewer3d, n: &str| -> Result<NodeId, String> {
        v.rt.scene.contexts[ROOT]
            .named_nodes
            .get(n)
            .ok_or_else(|| format!("event: no node named {n}"))
    };
    let node = named(v, e["node"].as_str().unwrap_or(""))?;
    let field = e["field"].as_str().unwrap_or("");
    let k =
        v.rt.get_field(node, field)
            .ok_or_else(|| format!("event: no field {field}"))?;
    let val = &e["value"];
    let nums = |val: &Value| -> Vec<f64> {
        val.as_array()
            .map(|a| a.iter().filter_map(Value::as_f64).collect())
            .unwrap_or_default()
    };
    let new = match v.rt.value(k) {
        FieldValue::SFBool(_) => FieldValue::SFBool(val.as_bool().ok_or("event: bool expected")?),
        FieldValue::SFFloat(_) => {
            FieldValue::SFFloat(val.as_f64().ok_or("event: number expected")?)
        }
        FieldValue::SFInt32(_) => {
            FieldValue::SFInt32(val.as_i64().ok_or("event: integer expected")? as i32)
        }
        FieldValue::SFTime(_) => FieldValue::SFTime(val.as_f64().ok_or("event: number expected")?),
        FieldValue::SFString(_) => {
            FieldValue::SFString(val.as_str().ok_or("event: string expected")?.to_string())
        }
        FieldValue::SFVec3f(_) => {
            let a = nums(val);

            if a.len() != 3 {
                return Err("event: three numbers expected".into());
            }

            FieldValue::SFVec3f(Vector3::new(a[0], a[1], a[2]))
        }
        FieldValue::SFVec2f(_) => {
            let a = nums(val);

            if a.len() != 2 {
                return Err("event: two numbers expected".into());
            }

            FieldValue::SFVec2f(Vector2::new(a[0], a[1]))
        }
        FieldValue::MFVec3f(_) => {
            FieldValue::MFVec3f(nums(val).iter().map(|&x| x as f32).collect())
        }
        FieldValue::MFVec3d(_) => FieldValue::MFVec3d(nums(val)),
        FieldValue::MFInt32(_) => {
            FieldValue::MFInt32(nums(val).iter().map(|&x| x as i32).collect())
        }
        FieldValue::SFNode(_) => FieldValue::SFNode(match val["def"].as_str() {
            Some(n) => Some(named(v, n)?),
            None => None,
        }),
        FieldValue::MFNode(_) => FieldValue::MFNode(
            val["nodes"]
                .as_array()
                .ok_or("event: nodes expected")?
                .iter()
                .map(|n| named(v, n.as_str().unwrap_or("")).map(Some))
                .collect::<Result<_, _>>()?,
        ),
        other => {
            return Err(format!(
                "event: field {field} of type {other:?} is not supported"
            ));
        }
    };

    v.rt.set_value(k, new);
    Ok(())
}
