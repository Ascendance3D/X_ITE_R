//! x3d-zfight <file.wrl> <script.json> --frames 75,132 [--out dir] [--jitter 1e-4] [--n 12] [--depth standard|log|reversed]
//!
//! Debug tool (never part of an oracle path). Replays the scripted input like x3d-drive, and at
//! each requested frame renders an object-ID image (`Renderer::set_debug_ids`: draw index and
//! triangle index per pixel, no antialiasing) with the camera moved by tiny view-space offsets
//! (`--jitter` world units, `--n` random directions). A pixel whose winning (draw, triangle)
//! changes under such a sub-pixel move although it is not next to a silhouette (all 8
//! neighbors share the baseline (draw, triangle)) is a depth-fighting pixel: its depth test is decided by
//! rounding, not geometry. The tool prints the fighting pixel count and the contested pairs, and
//! writes `<out>/<frame>-{base,flicker}.png`.
//!
//! `--color-dir DIR --jitters FILE` instead writes shaded PNGs (`DIR/<frame>/<i>.png`, no MSAA)
//! with the camera's position offset moved by each world-space vector of FILE (JSON
//! `[[dx,dy,dz],...]`; entry 0 should be zero). `oracle/x_ite-zfight.mjs` writes the same layout
//! from X_ITE, and `oracle/tools/zfight_compare.py` compares the two.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::PathBuf;
use x3d_render::gpu::{DepthMode, Gpu};
use x3d_viewer::script::apply;
use x3d_viewer::viewer::Viewer3d;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let opt = |f: &str| args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned();
    let file = PathBuf::from(&args[1]);
    let script: Value = serde_json::from_str(&std::fs::read_to_string(&args[2]).unwrap()).unwrap();
    let frames: Vec<u64> = opt("--frames").unwrap().split(',').map(|v| v.parse().unwrap()).collect();
    let out = PathBuf::from(opt("--out").unwrap_or("/tmp/zfight".into()));
    let jitter: f64 = opt("--jitter").map(|v| v.parse().unwrap()).unwrap_or(1e-4);
    let n: usize = opt("--n").map(|v| v.parse().unwrap()).unwrap_or(12);
    let mode = opt("--depth").map(|v| DepthMode::parse(&v).expect("depth mode")).unwrap_or_default();
    let color_dir = opt("--color-dir").map(PathBuf::from);
    let jitters: Option<Vec<[f64; 3]>> = opt("--jitters").map(|f| serde_json::from_str(&std::fs::read_to_string(f).unwrap()).unwrap());
    let (size, t0, dt) = (
        script["size"].as_array().map(|a| (a[0].as_u64().unwrap() as u32, a[1].as_u64().unwrap() as u32)).unwrap_or((512, 512)),
        script["t0"].as_f64().unwrap_or(946684800.0),
        script["dt"].as_f64().unwrap_or(1.0 / 60.0),
    );
    let events = script["events"].as_array().cloned().unwrap_or_default();
    let last = *frames.iter().max().unwrap();

    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            let mut v = Viewer3d::load(&file, None, t0, size.0, size.1).expect("load");
            v.frame_rate = 1.0 / dt;
            for _ in 0..8 {
                v.step(t0);
            }
            let mut gpu = Gpu::new().expect("gpu");
            gpu.renderer.set_depth_mode(mode);
            gpu.renderer.set_debug_ids(true);
            std::fs::create_dir_all(&out).unwrap();
            let mut seed = 12345u64;
            let mut rnd = move || {
                seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
                ((seed >> 33) as f64 / (1u64 << 31) as f64) * 2.0 - 1.0
            };

            for k in 1..=last {
                let prev = t0 + (k - 1) as f64 * dt;
                for e in events.iter().filter(|e| e["frame"].as_u64() == Some(k)) {
                    v.date_now = prev + e["t"].as_f64().unwrap_or(0.0);
                    apply(&mut v, e);
                }
                v.date_now = t0 + k as f64 * dt;
                v.step(t0 + k as f64 * dt);

                if !frames.contains(&k) {
                    continue;
                }

                if let (Some(dir), Some(jf)) = (&color_dir, &jitters) {
                    gpu.renderer.set_debug_ids(false);
                    let base_off = v.layer.vp(v.layer.viewpoint()).position_offset;
                    for (i, d) in jf.iter().enumerate() {
                        let vp = v.layer.viewpoint();
                        let mut o = base_off;
                        o.x += d[0];
                        o.y += d[1];
                        o.z += d[2];
                        v.layer.vp(vp).position_offset = o;
                        let base = v.base();
                        v.layer.vp(vp).update(&base);
                        let f = v.build_frame();
                        let rgba = gpu.render(&f, size.0, size.1);
                        let rgba: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
                        x3d_render::write_png(&dir.join(format!("{k}/{i}.png")), size.0, size.1, &rgba).unwrap();
                    }
                    let vp = v.layer.viewpoint();
                    v.layer.vp(vp).position_offset = base_off;
                    let base = v.base();
                    v.layer.vp(vp).update(&base);
                    gpu.renderer.set_debug_ids(true);
                    continue;
                }

                let mut frame = v.build_frame();
                let orig: Vec<_> = frame.items.iter().map(|i| i.model_view).collect();
                let (w, h) = size;
                let decode = |rgba: &[u8], i: usize| -> (u32, u32) {
                    let p = &rgba[i * 4..i * 4 + 4];
                    (p[0] as u32 | (p[1] as u32) << 8, p[2] as u32 | (p[3] as u32) << 8)
                };
                let base = gpu.render(&frame, w, h);
                let draws: Vec<_> = gpu.renderer.debug_draws().to_vec();
                let mut changed = vec![0u32; (w * h) as usize];
                let mut partner: BTreeMap<(u32, u32), u32> = BTreeMap::new();
                let mut tris: BTreeMap<(u32, u32, u32), u32> = BTreeMap::new();
                let mut records: BTreeMap<usize, ((u32, u32), (u32, u32))> = BTreeMap::new();

                for _ in 0..n {
                    let d = [rnd() * jitter, rnd() * jitter, rnd() * jitter];
                    for (it, m) in frame.items.iter_mut().zip(&orig) {
                        let mut mm = *m;
                        mm.0[12] += d[0];
                        mm.0[13] += d[1];
                        mm.0[14] += d[2];
                        it.model_view = mm;
                    }
                    let img = gpu.render(&frame, w, h);
                    for y in 1..h as usize - 1 {
                        for x in 1..w as usize - 1 {
                            let i = y * w as usize + x;
                            let b = decode(&base, i);
                            let c = decode(&img, i);
                            if b == c {
                                continue;
                            }
                            // not a silhouette pixel: all 8 neighbors share the baseline draw
                            let mut interior = true;
                            for (dx, dy) in [(-1i32, -1i32), (0, -1), (1, -1), (-1, 0), (1, 0), (-1, 1), (0, 1), (1, 1)] {
                                let j = ((y as i32 + dy) as usize) * w as usize + (x as i32 + dx) as usize;
                                if decode(&base, j) != b {
                                    interior = false;
                                }
                            }
                            if !interior || b.0 == 0 || c.0 == 0 {
                                continue;
                            }
                            changed[i] += 1;
                            records.entry(i).or_insert((b, c));
                            *partner.entry((b.0, c.0)).or_default() += 1;
                            *tris.entry((b.0, b.1, c.0)).or_default() += 1;
                        }
                    }
                }

                for (it, m) in frame.items.iter_mut().zip(&orig) {
                    it.model_view = *m;
                }

                let fight: usize = changed.iter().filter(|&&c| c > 0).count();
                println!("frame {k} depth={mode:?}: {w}x{h}, {} draws, {fight} fighting pixels (of {n} jitters of {jitter})", draws.len());
                let mut pairs: Vec<_> = partner.iter().collect();
                pairs.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
                let name = |id: u32| -> String {
                    match draws.get(id as usize - 1) {
                        Some(Some((node, _))) => {
                            let it = frame.items.iter().position(|i| i.geometry_node == *node && orig_eq(&i.model_view, &draws[id as usize - 1].as_ref().unwrap().1));
                            let col = it.map(|i| frame.items[i].material.diffuse).unwrap_or([0.0; 3]);
                            format!("draw{id}(geom node {node:?} diffuse {:.2},{:.2},{:.2})", col[0], col[1], col[2])
                        }
                        _ => format!("draw{id}(background/none)"),
                    }
                };
                for ((a, b), c) in pairs.iter().take(12) {
                    println!("  {} <-> {}: {c} flips", name(*a), name(*b));
                }
                let mut tl: Vec<_> = tris.iter().collect();
                tl.sort_by_key(|(_, c)| std::cmp::Reverse(**c));
                for ((d, t, other), c) in tl.iter().take(12) {
                    println!("  draw{d} tri{t} vs draw{other}: {c}");
                }

                // Analytic depth of the two competing triangles along each fighting pixel's ray.
                let proj = frame.projection;
                let (near, far) = (proj.0[14] / (proj.0[10] - 1.0), proj.0[14] / (proj.0[10] + 1.0));
                let tri_plane = |draw: u32, tri: u32| -> Option<([f64; 3], [f64; 3])> {
                    let (node, mv) = draws.get(draw as usize - 1)?.as_ref()?;
                    let it = frame.items.iter().find(|i| i.geometry_node == *node && i.model_view.0 == mv.0)?;
                    let g = &it.geometry;
                    let pts: Vec<[f64; 3]> = (0..3)
                        .map(|k| {
                            let v = &g.vertices[(tri as usize * 3 + k) * 4..(tri as usize * 3 + k) * 4 + 3];
                            let m = &mv.0;
                            let (x, y, z) = (v[0] as f64, v[1] as f64, v[2] as f64);
                            [m[0] * x + m[4] * y + m[8] * z + m[12], m[1] * x + m[5] * y + m[9] * z + m[13], m[2] * x + m[6] * y + m[10] * z + m[14]]
                        })
                        .collect();
                    let e1 = [pts[1][0] - pts[0][0], pts[1][1] - pts[0][1], pts[1][2] - pts[0][2]];
                    let e2 = [pts[2][0] - pts[0][0], pts[2][1] - pts[0][1], pts[2][2] - pts[0][2]];
                    let n = [e1[1] * e2[2] - e1[2] * e2[1], e1[2] * e2[0] - e1[0] * e2[2], e1[0] * e2[1] - e1[1] * e2[0]];
                    let l = (n[0] * n[0] + n[1] * n[1] + n[2] * n[2]).sqrt();
                    Some((pts[0], [n[0] / l, n[1] / l, n[2] / l]))
                };
                let mut deltas: Vec<(f64, f64, f64, f64)> = vec![];
                for (&i, &(b, c)) in &records {
                    let (px, py) = ((i % w as usize) as f64 + 0.5, (i / w as usize) as f64 + 0.5);
                    let (nx, ny) = (2.0 * px / w as f64 - 1.0, 1.0 - 2.0 * py / h as f64);
                    let dir = [nx / proj.0[0], ny / proj.0[5], -1.0];
                    let (Some(pa), Some(pb)) = (tri_plane(b.0, b.1), tri_plane(c.0, c.1)) else { continue };
                    let dist = |(p0, n): ([f64; 3], [f64; 3])| {
                        let t = (n[0] * p0[0] + n[1] * p0[1] + n[2] * p0[2]) / (n[0] * dir[0] + n[1] * dir[1] + n[2] * dir[2]);
                        t
                    };
                    let (ta, tb) = (dist(pa), dist(pb)); // view-space z = -t (dir.z = -1)
                    let dot = pa.1[0] * pb.1[0] + pa.1[1] * pb.1[1] + pa.1[2] * pb.1[2];
                    let z = ta.abs();
                    let step = z * z * (far - near) / (far * near * 16777216.0);
                    deltas.push(((ta - tb).abs(), z, step, dot.abs().min(1.0).acos().to_degrees()));
                }
                if !deltas.is_empty() {
                    deltas.sort_by(|a, b| a.0.partial_cmp(&b.0).unwrap());
                    let q = |f: f64| deltas[((deltas.len() - 1) as f64 * f) as usize];
                    println!(
                        "  analytic (near {near:.4} far {far:.4}): |dz| of the two planes min/median/max = {:.2e}/{:.2e}/{:.2e}; 24-bit step at the median pixel {:.2e} (z={:.2}); median |dz|/step {:.2}; plane angle median {:.4} deg",
                        deltas[0].0,
                        q(0.5).0,
                        deltas[deltas.len() - 1].0,
                        q(0.5).2,
                        q(0.5).1,
                        q(0.5).0 / q(0.5).2,
                        q(0.5).3
                    );
                }

                // Fighting pixels by pair of geometry nodes (same node twice = two instances of it).
                let mut by_node: BTreeMap<(u32, u32), usize> = BTreeMap::new();
                for &(b, c) in records.values() {
                    let node = |d: u32| draws.get(d as usize - 1).and_then(|x| x.as_ref()).map(|(n, _)| format!("{n:?}").parse::<u32>().unwrap_or(0)).unwrap_or(0);
                    let (x, y) = (node(b.0), node(c.0));
                    *by_node.entry((x.min(y), x.max(y))).or_default() += 1;
                }
                let list: Vec<String> = by_node.iter().map(|((a, b), c)| format!("geom{a}-geom{b}:{c}")).collect();
                println!("  by geometry node pair: {}", list.join(" "));

                let mut png = vec![0u8; (w * h * 4) as usize];
                for i in 0..(w * h) as usize {
                    let (d, _) = decode(&base, i);
                    let g = if d == 0 { 0 } else { 60 + (d * 37 % 120) as u8 };
                    let c = if changed[i] > 0 { [255, 0, 0] } else { [g, g, g] };
                    png[i * 4..i * 4 + 4].copy_from_slice(&[c[0], c[1], c[2], 255]);
                }
                x3d_render::write_png(&out.join(format!("{k}-flicker.png")), w, h, &png).unwrap();
            }
        })
        .unwrap()
        .join()
        .unwrap();
}

fn orig_eq(a: &x3d_fields::math::Matrix4, b: &x3d_fields::math::Matrix4) -> bool {
    a.0 == b.0
}
