//! Metal rasterization of the pointing pass against the pinned X_ITE 16.5.0 recorded on the same physical M1 (issues #69 and #89).
//!
//! The worlds are oracle/synthetic-metal-coverage (made by oracle/tools/gen_metal_coverage.py): every probe sits on a pixel centre,
//! so a probe is a coverage sample of that pixel. The recordings are oracle/metal-coverage-apple-m1-baseline (made by
//! oracle/tools/record_metal_coverage.py, three identical runs each).
//!
//! * `ml-sprite-*`: point sprites (PointProperties, and a plain PointSet for gl_PointSize 2). ANGLE on Metal covers the pixel
//!   centres between the two corners `centre -+ size / 2` of a sprite, each rounded to 1/256 pixel (and inside the round point
//!   mask when there is one); gl_PointCoord is relative to `centre - size / 2` rounded to 1/256 pixel and divided by the
//!   difference of the corners rounded to 1/16 pixel (the oracle GPU of the NVIDIA recordings floors the size to 1/32 pixel
//!   instead). A size above ALIASED_POINT_SIZE_RANGE [1], 511, is drawn as 511 (ml-sprite-clamp). Ids, hit points and normals
//!   are bit for bit; the texCoord within SPRITE_TEX_ABS, except the probes of the sprites that sit on a rounding tie (see
//!   SPRITE_TIES).
//! * `ml-lines-*`: 1 px lines. The page covers on Metal exactly the pixels the NVIDIA recording covers (measured: the same
//!   probes hit in both), which the quad emulation of `pointing.wgsl` reproduces except at the ends of a line and at a few ties
//!   (GL's diamond-exit rule): LINE_ID_GAPS is that number per case, and the test fails when it changes.
//!
//! Only on a Metal device with `X3D_REFERENCE_PLATFORM=apple-m1`: the recordings are the M1's, and the Metal rules apply to Metal.
//! Anywhere else the test says so and passes without comparing (the Vulkan results are covered by rea_batch12 and the other
//! pointing tests, which this change leaves bit-identical).

mod common;

use serde_json::Value;
use std::path::PathBuf;
use std::process::Command;

/// The largest absolute difference of a sprite texCoord to the recording (a float32 step of a value near 1).
const SPRITE_TEX_ABS: f64 = 1.3e-7;

/// A sprite whose `centre - size / 2` lies within float32 noise of a rounding tie of the 1/256 pixel origin grid: the page and
/// Rust pick the neighbouring grid value, so every probe of the sprite may differ by one grid step of the texCoord, which is
/// 1 / (256 * size) (at most 2.0e-3 for the smallest size, 2) and, at the rim of the round point mask, by its hit.
const SPRITE_TIE_ABS: f64 = 2.0e-3;

/// (case, probe index ranges (first, last) of the sprites on a rounding tie), measured on the M1: 2 of the 256 sprites of
/// ml-sprite-size, 1 of ml-sprite-low and 6 of ml-sprite-pos; every other sprite of every case is exact.
const SPRITE_TIES: &[(&str, &[(usize, usize)])] = &[
    ("ml-sprite-size", &[(6055, 6110), (8175, 8238)]),
    ("ml-sprite-low", &[(9786, 9834)]),
    (
        "ml-sprite-pos",
        &[
            (7616, 7679),
            (8128, 8191),
            (8384, 8447),
            (8896, 8959),
            (9408, 9471),
            (9920, 9983),
        ],
    ),
];

/// (case, probes that hit in the recording and not in Rust or the other way round).
const LINE_ID_GAPS: &[(&str, usize)] = &[("ml-lines-a", 27), ("ml-lines-b", 38)];

fn probes(v: &Value) -> &Vec<Value> {
    v["pickProbes"].as_array().expect("pickProbes")
}

fn max_abs(a: &Value, b: &Value) -> f64 {
    match (a.as_array(), b.as_array()) {
        (Some(x), Some(y)) if x.len() == y.len() => x
            .iter()
            .zip(y)
            .map(|(p, q)| (p.as_f64().unwrap() - q.as_f64().unwrap()).abs())
            .fold(0.0, f64::max),
        _ => f64::INFINITY,
    }
}

#[test]
fn metal_coverage_matches_the_pinned_page_on_the_same_m1() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let gpu = match x3d_render::gpu::Gpu::new() {
        Ok(g) => g,
        Err(e) => return x3d_evidence::no_gpu(e),
    };
    let metal = gpu.adapter_info.backend == wgpu::Backend::Metal;
    let platform = common::reference::platform();

    if !metal || platform.as_deref() != Some("apple-m1") {
        eprintln!(
            "metal_coverage: not compared (backend {:?}, X3D_REFERENCE_PLATFORM {:?}): the recordings are the M1's, the rules are Metal's",
            gpu.adapter_info.backend, platform
        );
        return;
    }

    drop(gpu);
    let table = root.join("oracle/metal-coverage-apple-m1-baseline/cases.txt");
    let cases = std::fs::read_to_string(&table).expect("cases.txt");
    let only = std::env::var("REA_ONLY").ok();
    let mut failures = vec![];
    let mut checked = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, world, script, msaa) = (f[0], f[1], f[2], f[3]);

        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }

        let reference = root.join(format!(
            "oracle/metal-coverage-apple-m1-baseline/{name}-msaa{msaa}.json"
        ));
        let out = std::env::temp_dir().join(format!(
            "metal-coverage-{}-{name}-msaa{msaa}.json",
            std::process::id()
        ));
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-drive"));
        cmd.current_dir(&root)
            .args([world, script, "--oracle", "oracle", "--out"])
            .arg(&out)
            .arg("--warmup-from")
            .arg(&reference);

        if msaa != "0" {
            cmd.args(["--msaa", msaa, "--gl-samples"]);
        }

        let status = cmd.status().unwrap();

        if !status.success() {
            failures.push(format!(
                "{name} MSAA {msaa}: x3d-drive exited with {status}"
            ));
            continue;
        }

        let want: Value =
            serde_json::from_str(&std::fs::read_to_string(&reference).unwrap()).unwrap();
        let got: Value = serde_json::from_str(&std::fs::read_to_string(&out).unwrap()).unwrap();
        let _ = std::fs::remove_file(&out);
        let (want, got) = (probes(&want), probes(&got));
        let ties: &[(usize, usize)] = SPRITE_TIES
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(&[], |(_, t)| t);
        let line_gap = LINE_ID_GAPS
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, g)| *g);
        let on_tie = |i: usize| ties.iter().any(|&(a, b)| (a..=b).contains(&i));
        let (mut id_gaps, mut worst, mut worst_tie) = (0, 0.0f64, 0.0f64);

        if want.len() != got.len() {
            failures.push(format!(
                "{name} MSAA {msaa}: {} probes recorded, {} run",
                want.len(),
                got.len()
            ));
            continue;
        }

        for (i, (a, b)) in want.iter().zip(got).enumerate() {
            if a["id"] != b["id"] {
                id_gaps += 1;

                if line_gap.is_none() && !on_tie(i) {
                    failures.push(format!(
                        "{name} MSAA {msaa}: probe {i}: page {a} / Rust {b}"
                    ));
                }

                continue;
            }

            if a["id"] == 0 {
                continue;
            }

            let tex = max_abs(&a["texCoord"], &b["texCoord"]);
            let rest = max_abs(&a["point"], &b["point"]).max(max_abs(&a["normal"], &b["normal"]));

            if name.starts_with("ml-lines") {
                // The hit point of a 1 px line is interpolated by the rasterizer: float32 steps (pk-lines allows 398 ulps).
                worst = worst.max(rest);
            } else if on_tie(i) {
                worst_tie = worst_tie.max(tex);
            } else if rest != 0.0 || tex > SPRITE_TEX_ABS {
                failures.push(format!(
                    "{name} MSAA {msaa}: probe {i}: page {a} / Rust {b} (texCoord {tex:e}, rest {rest:e})"
                ));
            }
        }

        if let Some(gap) = line_gap {
            if id_gaps != gap {
                failures.push(format!(
                    "{name} MSAA {msaa}: {id_gaps} probes hit in only one of page and Rust, recorded {gap}"
                ));
            }
        }

        if worst_tie > SPRITE_TIE_ABS {
            failures.push(format!(
                "{name} MSAA {msaa}: a tie sprite differs by {worst_tie:e}"
            ));
        }

        eprintln!(
            "{name} MSAA {msaa}: {} probes, {id_gaps} hit/miss gaps, hit point up to {worst:e} off, tie sprites up to {worst_tie:e}",
            want.len()
        );
        checked += 1;
    }

    assert!(only.is_some() || checked > 0, "no case");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
