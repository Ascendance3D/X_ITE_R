//! `x3d-oracle particles`: oracle/tools/x_ite_particles_b07.mjs (default), x_ite_particles_events.mjs
//! (`--variant events`) and x_ite_particles.mjs (`--variant 06r`) in Rust: the seeded-particle determinism
//! protocol (Date.now pinned to t0 before load, Math.random = mulberry32 (seed) installed before page scripts and
//! reseeded after load + warmup, ParticleSystem.deltaTime reset, per-frame randomSeed / forceDraws / numParticles /
//! deltaTime in steps [], the 28-float transform-feedback readback into the .f32 sidecar at the dump frames).
//! Output: the same result JSON (`JSON.stringify (result, null, 1)`) and .f32 bytes as the Node tool.
//!
//! Additive (not in the Node tools): `--png-dir DIR` writes the WebGL drawing buffer at every dump frame as
//! DIR/<frame>.png (the PNG method of x_ite-anim.mjs), after the particle readback. `--aa` / `--ms N` load
//! anim-msaa.html (X_ITE antialiased, Multisampling N) instead of anim.html; `--png-size WxH` sets that page's
//! canvas size (default 512x512, the size of anim.html's canvas). Script key `pickProbes`
//! (`{ "frames": [k, ...], "points": [[x, y], ...] }`, pointer coordinates, y up) records `browser .touch` at each point
//! after each listed frame's step, dump and PNG, as `result .pickProbes ["k"]` (id, DEF name of the hit shape, point,
//! normal, texCoord at full precision); x3d-particles-drive does the same with its pointing pass. Script key
//! `collisionProbes` (`{ "frames": [k, ...], "directions": [[x, y, z], ...], "translations": [[x, y, z], ...],
//! "nodes": [DEF, ...] }`) records, after the pick probes of each listed frame, the frame's collision shapes, the closest
//! object along each direction, each translation constrained, the user pose and the named Collision nodes' isActive and
//! collideTime as `result .collisionProbes ["k"]`; x3d-particles-drive does the same with its collision pass.

use crate::anim::{Argv, relative, resolve};
use crate::json::{self, J, js_number};
use crate::page::Page;
use crate::particles_js as pj;
use crate::{Common, page_js};
use serde_json::Value;
use std::path::Path;

#[derive(Clone, Copy, PartialEq, Eq, Debug)]
pub enum Variant {
    R06,
    Events,
    B07,
}

const STRIDE: f64 = 28.0;

/// `--png-dir` only (not in the Node tools): size the drawing buffer to the canvas before the world loads.
const PNG_RESHAPE: &str =
    r#"() => { const b = document .getElementById ("c") .browser; b .reshape (); }"#;

/// `JSON.stringify (JSON.parse (token))` for a raw script value: numbers and strings in JavaScript's form.
fn restringify(v: &J) -> J {
    match v {
        J::Undef => J::Undef,
        J::Tok(t) => {
            if t.starts_with('"') {
                match serde_json::from_str::<String>(t) {
                    Ok(s) => J::str(&s),
                    Err(_) => J::Tok(t.clone()),
                }
            } else if t == "true" || t == "false" || t == "null" {
                J::Tok(t.clone())
            } else {
                J::num(t.parse::<f64>().unwrap_or(f64::NAN))
            }
        }
        J::Arr(a) => J::Arr(a.iter().map(restringify).collect()),
        J::Obj(m) => J::Obj(
            m.iter()
                .map(|(k, v)| {
                    (
                        json::escape(&serde_json::from_str::<String>(k).unwrap_or_default()),
                        restringify(v),
                    )
                })
                .collect(),
        ),
    }
}

/// `script .key ?? def`.
fn field(raw: &J, key: &str, def: J) -> J {
    match raw.member(key) {
        J::Undef => def,
        J::Tok(t) if t == "null" => def,
        v => restringify(&v),
    }
}

fn tok_num(v: &J) -> f64 {
    match v {
        J::Tok(t) => t.parse().unwrap_or(f64::NAN),
        _ => f64::NAN,
    }
}

fn tok_text(v: &J) -> String {
    match v {
        J::Tok(t) if t.starts_with('"') => serde_json::from_str::<String>(t).unwrap_or_default(),
        J::Tok(t) => t.clone(),
        J::Undef => "undefined".into(),
        other => other.compact(),
    }
}

pub fn run(argv: &Argv, common: &Common) -> Result<(String, bool, String), String> {
    let pos = argv.positional(true);
    let (file_arg, script_arg) = match (pos.first(), pos.get(1)) {
        (Some(f), Some(s)) => (f.clone(), s.clone()),
        _ => return Err("usage: x3d-oracle particles <world inside oracle/> <script.json> [--out result.json] [--variant b07|events|06r] [--png-dir dir] [--aa] [--ms N]".into()),
    };
    let variant = match argv.opt("--variant").as_deref() {
        None | Some("b07") => Variant::B07,
        Some("events") => Variant::Events,
        Some("06r") | Some("r06") => Variant::R06,
        Some(v) => return Err(format!("unknown --variant {v:?} (b07, events, 06r)")),
    };
    let (init_js, norm_js, step_js, dump_js, geom_js, info_js) = match variant {
        Variant::R06 => (
            pj::INIT_R06,
            pj::NORMALIZE_R06,
            pj::STEP_R06,
            pj::DUMP_R06,
            pj::GEOMETRY_R06,
            pj::INFO_R06,
        ),
        Variant::Events => (
            pj::INIT_EV,
            pj::NORMALIZE_EV,
            pj::STEP_EV,
            pj::DUMP_EV,
            pj::GEOMETRY_EV,
            pj::INFO_EV,
        ),
        Variant::B07 => (
            pj::INIT_B07,
            pj::NORMALIZE_B07,
            pj::STEP_B07,
            pj::DUMP_B07,
            pj::GEOMETRY_B07,
            pj::INFO_B07,
        ),
    };
    let script_text =
        std::fs::read_to_string(&script_arg).map_err(|e| format!("ENOENT: {script_arg}: {e}"))?;
    let script: Value = serde_json::from_str(&script_text)
        .map_err(|e| format!("SyntaxError: {script_arg}: {e}"))?;
    let raw = json::parse(&script_text).map_err(|e| format!("SyntaxError: {script_arg}: {e}"))?;
    let out_path = resolve(&argv.opt("--out").unwrap_or_else(|| "particles.json".into()));
    let out_str = out_path.to_string_lossy().into_owned();

    let t0_j = field(&raw, "t0", J::num(946684800.0));
    let dt_j = field(&raw, "dt", J::num(1.0 / 60.0));
    let frames_j = field(&raw, "frames", J::num(1.0));
    let warmup_j = field(&raw, "warmup", J::num(8.0));
    let seed_j = field(&raw, "seed", J::num(1.0));
    let node_j = field(&raw, "node", J::str("PS"));
    let (t0, dt, frames) = (tok_num(&t0_j), tok_num(&dt_j), tok_num(&frames_j));
    let dump: Vec<f64> = if json::truthy(script.get("dumpAll")) {
        (1..=frames.max(0.0) as u64).map(|k| k as f64).collect()
    } else {
        match script.get("dump").filter(|v| !v.is_null()) {
            Some(Value::Array(a)) => a.iter().filter_map(Value::as_f64).collect(),
            Some(_) => vec![],
            None => vec![frames],
        }
    };
    let probe_frames: Vec<f64> = script
        .pointer("/pickProbes/frames")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    let probe_points = script
        .pointer("/pickProbes/points")
        .map(Value::to_string)
        .unwrap_or_else(|| "[]".into());
    let collision_frames: Vec<f64> = script
        .pointer("/collisionProbes/frames")
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default();
    let collision_arg = {
        let part = |k: &str| {
            script
                .pointer(&format!("/collisionProbes/{k}"))
                .map(Value::to_string)
                .unwrap_or_else(|| "[]".into())
        };

        format!(
            "{{\"directions\":{},\"translations\":{},\"nodes\":{}}}",
            part("directions"),
            part("translations"),
            part("nodes")
        )
    };
    let born = script.get("filter").and_then(Value::as_str) == Some("born");
    let png_dir = argv.opt("--png-dir").unwrap_or_default();
    let ms = argv.opt("--ms").unwrap_or_default();
    let msaa_page = argv.0.iter().any(|a| a == "--aa") || !ms.is_empty();
    let (cw, ch) = argv
        .opt("--png-size")
        .and_then(|s| {
            s.split_once('x')
                .and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)))
        })
        .unwrap_or((512, 512));

    let mut result = J::obj();
    result.set("ok", J::bool(false));
    result.set("dt", dt_j.clone());
    result.set("t0", t0_j.clone());
    result.set("frames", frames_j.clone());
    result.set("warmup", warmup_j.clone());
    result.set("seed", seed_j.clone());
    result.set("stride", J::num(STRIDE));
    result.set("node", node_j.clone());
    result.set("steps", J::Arr(vec![]));
    result.set("dumps", J::obj());
    result.set("log", J::Arr(vec![]));
    let mut chunks: Vec<u8> = Vec::new();
    let mut floats: f64 = 0.0;

    let server = crate::server::Server::start(common.root.clone())
        .map_err(|e| format!("x3d-oracle: http server: {e}"))?;
    let base = format!("http://127.0.0.1:{}", server.port);
    let mut page = Page::open(
        &common.chrome,
        crate::chrome::ORACLE_ARGS,
        256,
        256,
        &base,
        common.trace.clone(),
        common.deadline,
    )?;
    crate::watchdog(&page, common);

    let parse = |t: Option<String>| -> Result<J, String> {
        match t {
            None => Ok(J::Undef),
            Some(t) => json::parse(&t).map_err(|e| format!("x3d-oracle: bad page JSON: {e}")),
        }
    };
    let log_push = |result: &mut J, line: String| {
        if let Some(J::Arr(a)) = result.get_mut("log") {
            a.push(J::str(&line));
        }
    };

    let outcome = (|| -> Result<(), String> {
        page.add_init_script(&format!(
            "({init_js})([{},{}])",
            seed_j.compact(),
            js_number(t0 * 1000.0)
        ))?;
        let url = if msaa_page {
            format!(
                "{base}/x_ite-pin/src/anim-msaa.html?w={cw}&h={ch}&aa=1{}",
                if ms.is_empty() {
                    String::new()
                } else {
                    format!("&ms={ms}")
                }
            )
        } else {
            format!("{base}/x_ite-pin/src/anim.html")
        };
        page.goto(&url)?;
        page.wait_for_function(page_js::READY, 30000)?;
        if !png_dir.is_empty() {
            // The protocol makes ResizeObserver inert, so the drawing buffer would stay at the 1x1 of X_ITE's first
            // reshape (the canvas is not laid out yet). One reshape before the world loads sizes it to the canvas;
            // no particle system exists yet, so the protocol's JSON and .f32 are unchanged (checked on every case).
            page.evaluate(PNG_RESHAPE, None)?;
        }

        let rel = relative(&common.root, &resolve(&file_arg));
        let world = format!(
            "{base}/{}",
            rel.split('/')
                .map(json::encode_uri_component)
                .collect::<Vec<_>>()
                .join("/")
        );
        let load_arg = format!(
            "[{},{},{},{}]",
            json::escape(&world),
            t0_j.compact(),
            dt_j.compact(),
            warmup_j.compact()
        );
        let loaded_text = page
            .evaluate(page_js::LOAD, Some(&load_arg))?
            .unwrap_or_else(|| "{}".into());
        let loaded: Value = serde_json::from_str(&loaded_text).unwrap_or(Value::Null);
        if !json::truthy(loaded.get("ok")) {
            return Err(match loaded.get("error") {
                None | Some(Value::Null) => String::new(),
                Some(Value::String(s)) => s.clone(),
                Some(v) => v.to_string(),
            });
        }
        page.evaluate(page_js::SET_DT, Some(&dt_j.compact()))?;

        if variant == Variant::B07 {
            result.set("vrmlLoad", parse(page.evaluate(page_js::VRML, None)?)?);
        }
        let init = parse(page.evaluate(
            norm_js,
            Some(&format!(
                "[{},{},{}]",
                node_j.compact(),
                seed_j.compact(),
                t0_j.compact()
            )),
        )?)?;
        result.set("init", init);
        let info = parse(page.evaluate(info_js, None)?)?;
        let stride = info.member("stride");
        result.set("info", info);
        if tok_num(&stride) != STRIDE {
            return Err(format!("unexpected particle stride {}", tok_text(&stride)));
        }

        let events = raw.member("events");
        let mut k = 1.0f64;
        while k <= frames {
            let t = t0 + k * dt;
            let arg = if variant == Variant::R06 {
                format!("[{},{}]", js_number(t), node_j.compact())
            } else {
                let evs: Vec<J> = match &events {
                    J::Arr(a) => a
                        .iter()
                        .filter(|e| {
                            tok_num(&e.member("frame")) == k
                                && matches!(e.member("frame"), J::Tok(ref f) if !f.starts_with('"'))
                        })
                        .cloned()
                        .collect(),
                    _ => vec![],
                };
                format!(
                    "[{},{},{}]",
                    js_number(t),
                    node_j.compact(),
                    J::Arr(evs).compact()
                )
            };
            let step = parse(page.evaluate(step_js, Some(&arg))?)?;
            let calls = step.member("calls");
            if tok_num(&calls) != 1.0 {
                log_push(
                    &mut result,
                    format!(
                        "frame {}: {} emitter.animate calls",
                        js_number(k),
                        tok_text(&calls)
                    ),
                );
            }
            let seeds = match step.member("randomSeeds") {
                J::Arr(a) => a,
                _ => vec![],
            };
            if seeds.len() != 1 {
                log_push(
                    &mut result,
                    format!(
                        "frame {}: {} randomSeed uniform writes",
                        js_number(k),
                        seeds.len()
                    ),
                );
            }
            let mut s = J::obj();
            s.set("frame", J::num(k));
            s.set("time", J::num(t));
            s.set(
                "randomSeed",
                seeds.first().cloned().unwrap_or(J::Tok("null".into())),
            );
            for f in [
                "numParticles",
                "deltaTime",
                "creationTime",
                "createParticles",
            ] {
                s.set(f, step.member(f));
            }
            if variant != Variant::R06 {
                s.set("isActive", step.member("isActive"));
                s.set("maxParticles", step.member("maxParticles"));
            }
            if variant == Variant::B07 {
                s.set("numForces", step.member("numForces"));
                s.set("forces", step.member("forces"));
                s.set("forceDraws", step.member("forceDraws"));
                if let J::Arr(l) = step.member("eventLog")
                    && !l.is_empty()
                {
                    s.set("eventLog", J::Arr(l));
                }
            }
            if let Some(J::Arr(a)) = result.get_mut("steps") {
                a.push(s);
            }

            if dump.contains(&k) {
                let t = page
                    .evaluate(dump_js, Some(if born { "true" } else { "false" }))?
                    .unwrap_or_default();
                let d = parse(Some(t.clone()))?;
                let v: Value = serde_json::from_str(&t).unwrap_or(Value::Null);
                let buf = json::base64_decode(v["b64"].as_str().unwrap_or(""));
                let mut e = J::obj();
                e.set("numParticles", d.member("n"));
                e.set("total", d.member("total"));
                e.set("offset", J::num(floats));
                result.get_mut("dumps").unwrap().set(&js_number(k), e);
                floats += (buf.len() / 4) as f64;
                chunks.extend(buf);

                if !png_dir.is_empty() {
                    let t = page.evaluate(page_js::PIXELS, None)?.unwrap_or_default();
                    let p: Value =
                        serde_json::from_str(&t).map_err(|e| format!("x3d-oracle: pixels: {e}"))?;
                    let (pw, ph) = (
                        p["width"].as_u64().unwrap_or(0) as u32,
                        p["height"].as_u64().unwrap_or(0) as u32,
                    );
                    let png = crate::png::encode_bottom_up(
                        pw,
                        ph,
                        &json::base64_decode(p["rgba"].as_str().unwrap_or("")),
                    )?;
                    std::fs::create_dir_all(&png_dir).map_err(|e| format!("{png_dir}: {e}"))?;
                    let path = Path::new(&png_dir).join(format!("{}.png", js_number(k)));
                    std::fs::write(&path, png).map_err(|e| format!("{}: {e}", path.display()))?;
                }
            }
            if probe_frames.contains(&k) {
                let probes = parse(page.evaluate(pj::PICK_PROBES_NAMED, Some(&probe_points))?)?;
                if !matches!(result.member("pickProbes"), J::Obj(_)) {
                    result.set("pickProbes", J::obj());
                }
                result
                    .get_mut("pickProbes")
                    .unwrap()
                    .set(&js_number(k), probes);
            }
            if collision_frames.contains(&k) {
                let probes =
                    parse(page.evaluate(pj::COLLISION_PROBES_NAMED, Some(&collision_arg))?)?;
                if !matches!(result.member("collisionProbes"), J::Obj(_)) {
                    result.set("collisionProbes", J::obj());
                }
                result
                    .get_mut("collisionProbes")
                    .unwrap()
                    .set(&js_number(k), probes);
            }
            k += 1.0;
        }

        result.set("geometry", parse(page.evaluate(geom_js, None)?)?);
        if variant == Variant::B07 {
            result.set("vrmlEnd", parse(page.evaluate(page_js::VRML, None)?)?);
            result.set("console", parse(page.evaluate(pj::GET_LOG_OPT, None)?)?);
        }
        let calls = parse(page.evaluate(pj::CALLS, None)?)?;
        result.set("animateCalls", calls.clone());
        if tok_num(&calls) != frames {
            log_push(
                &mut result,
                format!(
                    "animate called {} times in {} steps",
                    tok_text(&calls),
                    tok_text(&frames_j)
                ),
            );
        }
        result.set("ok", J::bool(true));
        Ok(())
    })();

    if let Err(e) = &outcome {
        result.set("error", J::str(e));
    }
    // finally: result .log .push (... getLog), then the browser closes.
    if let Ok(Some(t)) = page.evaluate(pj::GET_LOG_OPT, None)
        && let (Ok(J::Arr(lines)), Some(J::Arr(a))) = (json::parse(&t), result.get_mut("log"))
    {
        a.extend(lines);
    }
    for err in page.page_errors() {
        eprintln!("x3d-oracle: page error: {err}");
    }
    if let Some(c) = page.closed_reason() {
        eprintln!("x3d-oracle: browser: {c}");
        for l in page.chrome.stderr_tail() {
            eprintln!("x3d-oracle: chrome: {l}");
        }
    }
    page.close();

    let f32_path = format!("{}.f32", out_str.strip_suffix(".json").unwrap_or(&out_str));
    std::fs::write(&f32_path, &chunks).map_err(|e| format!("{f32_path}: {e}"))?;
    result.set("fileF32", J::str(f32_path.rsplit('/').next().unwrap_or("")));
    let ok = outcome.is_ok();
    std::fs::write(&out_path, result.pretty_indent(1).unwrap())
        .map_err(|e| format!("{out_str}: {e}"))?;
    let line = match &outcome {
        Ok(()) => format!("ok {out_str} ({} floats)", js_number(floats)),
        Err(e) => format!("FAILED: {e}"),
    };
    Ok((String::new(), ok, line))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn particles_js_is_verbatim() {
        let r06 = include_str!("../../../oracle/tools/x_ite_particles.mjs");
        let ev = include_str!("../../../oracle/tools/x_ite_particles_events.mjs");
        let b07 = include_str!("../../../oracle/tools/x_ite_particles_b07.mjs");
        for (src, consts) in [
            (
                r06,
                [
                    pj::INIT_R06,
                    pj::NORMALIZE_R06,
                    pj::STEP_R06,
                    pj::DUMP_R06,
                    pj::GEOMETRY_R06,
                    pj::INFO_R06,
                ],
            ),
            (
                ev,
                [
                    pj::INIT_EV,
                    pj::NORMALIZE_EV,
                    pj::STEP_EV,
                    pj::DUMP_EV,
                    pj::GEOMETRY_EV,
                    pj::INFO_EV,
                ],
            ),
            (
                b07,
                [
                    pj::INIT_B07,
                    pj::NORMALIZE_B07,
                    pj::STEP_B07,
                    pj::DUMP_B07,
                    pj::GEOMETRY_B07,
                    pj::INFO_B07,
                ],
            ),
        ] {
            for c in consts.iter().chain(
                [
                    pj::GET_LOG_OPT,
                    pj::CALLS,
                    page_js::READY,
                    page_js::LOAD,
                    page_js::SET_DT,
                ]
                .iter(),
            ) {
                assert!(src.contains(c), "drift: {}", &c[..c.len().min(60)]);
            }
        }
        assert!(b07.contains(page_js::VRML));
    }

    #[test]
    fn restringify_matches_json_roundtrip() {
        let v = json::parse(r#"{"a":1.50,"b":"A","c":[1e2,true,null]}"#).unwrap();
        assert_eq!(
            restringify(&v).compact(),
            r#"{"a":1.5,"b":"A","c":[100,true,null]}"#
        );
    }
}
