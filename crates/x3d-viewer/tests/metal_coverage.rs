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
//!   are bit for bit; the texCoord within SPRITE_TEX_ABS, except the probes of the sprites that sit on a rounding tie: those are a
//!   known unresolved difference, pinned as measured in TIE_DIFFS (see SPRITE_TIES), never accepted by a tolerance.
//! * `ml-lines-*`: 1 px lines. The page draws them as Metal line primitives (ANGLE's gl.LINES), and so does the pointing pass on a
//!   Metal device (`gpu::metal_native_lines`): every id, hit point and normal is bit for bit (before, the two-triangle emulation
//!   of GL's diamond-exit rule missed 65 hit/miss probes and 49 hit points were a float32 step off).
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
/// Rust pick the neighbouring grid value. These probes are a KNOWN UNRESOLVED DIFFERENCE, not a match and not within a
/// tolerance: the test pins what was measured (TIE_DIFFS) and fails when it changes in either direction, including when the
/// probes become exact (then the entry is stale and the difference is fixed).
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

/// (case, MSAA, probes of the tie sprites that differ in any field, of them probes that hit in only one of page and Rust, the
/// largest texCoord difference, the largest point or normal difference), measured on the M1.
const TIE_DIFFS: &[(&str, &str, usize, usize, f64, f64)] = &[
    ("ml-sprite-low", "0", 4, 0, 1.3297498226165771e-3, 0.0),
    ("ml-sprite-pos", "0", 43, 1, 9.765625e-4, 0.0),
    ("ml-sprite-size", "0", 13, 0, 1.0080933570861816e-3, 0.0),
    ("ml-sprite-size", "4", 13, 0, 1.0080933570861816e-3, 0.0),
];

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
    let baseline = root.join("oracle/metal-coverage-apple-m1-baseline");

    // The worlds are generated (they are not committed) and must be the bytes the recordings were made from.
    let worlds = std::env::temp_dir().join(format!("metal-coverage-worlds-{}", std::process::id()));
    let generated = Command::new("python3")
        .arg("-I")
        .arg(root.join("oracle/tools/gen_metal_coverage.py"))
        .arg(&worlds)
        .status()
        .expect("python3");
    assert!(generated.success(), "gen_metal_coverage.py failed");
    let pinned = std::fs::read_to_string(baseline.join("worlds.sha256")).expect("worlds.sha256");
    let mut pinned_names = vec![];

    for line in pinned.lines().filter(|l| !l.trim().is_empty()) {
        let (hash, name) = line.split_once("  ").expect("worlds.sha256 line");
        let out = Command::new("sha256sum")
            .arg(worlds.join(name))
            .output()
            .expect("sha256sum");
        let got = String::from_utf8_lossy(&out.stdout);
        assert!(
            got.starts_with(hash),
            "{name}: the generated world is not the one the recordings were made from (want {hash}, got {got})"
        );
        pinned_names.push(name.to_string());
    }

    assert_eq!(
        std::fs::read_dir(&worlds).unwrap().count(),
        pinned_names.len(),
        "the generator wrote files that worlds.sha256 does not list"
    );

    let table = baseline.join("cases.txt");
    let cases = std::fs::read_to_string(&table).expect("cases.txt");
    let only = std::env::var("REA_ONLY").ok();

    // Every listed case has its recording, its world and its script: a missing file is a failure, never a skipped case.
    let missing: Vec<String> = cases
        .lines()
        .filter(|l| !l.trim().is_empty())
        .flat_map(|l| {
            let f: Vec<&str> = l.split_whitespace().collect();
            assert!(f.len() == 4, "cases.txt: malformed line {l:?}");
            vec![
                format!(
                    "oracle/metal-coverage-apple-m1-baseline/{}-msaa{}.json",
                    f[0], f[3]
                ),
                f[1].to_string(),
                f[2].to_string(),
            ]
        })
        .filter(|p| {
            let path = if p.starts_with("oracle/synthetic-metal-coverage/") {
                worlds.join(p.rsplit('/').next().unwrap())
            } else {
                root.join(p)
            };
            !path.is_file()
        })
        .collect();
    assert!(
        missing.is_empty(),
        "cases.txt lists files that are not in the repository: {missing:?}"
    );

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
        let (world, script) = (
            worlds.join(world.rsplit('/').next().unwrap()),
            worlds.join(script.rsplit('/').next().unwrap()),
        );
        let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-drive"));
        cmd.current_dir(&root)
            .arg(&world)
            .arg(&script)
            .args(["--oracle", "oracle", "--out"])
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
            serde_json::from_str(&std::fs::read_to_string(&reference).expect("recording"))
                .expect("recording is JSON");
        let got: Value = match std::fs::read_to_string(&out)
            .ok()
            .and_then(|t| serde_json::from_str(&t).ok())
        {
            Some(v) => v,
            None => {
                failures.push(format!("{name} MSAA {msaa}: x3d-drive wrote no result"));
                continue;
            }
        };
        let _ = std::fs::remove_file(&out);
        let (want, got) = (probes(&want), probes(&got));
        let ties: &[(usize, usize)] = SPRITE_TIES
            .iter()
            .find(|(n, _)| *n == name)
            .map_or(&[], |(_, t)| t);
        let on_tie = |i: usize| ties.iter().any(|&(a, b)| (a..=b).contains(&i));
        let (mut id_gaps, mut worst) = (0, 0.0f64);
        let (mut tie_diffs, mut tie_id_gaps, mut tie_tex, mut tie_rest) =
            (0usize, 0usize, 0.0f64, 0.0f64);

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

                if on_tie(i) {
                    tie_id_gaps += 1;
                    tie_diffs += 1;
                } else {
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
                // The hit point of a 1 px line is interpolated by the rasterizer: bit for bit with the line primitive.
                worst = worst.max(rest);

                if rest != 0.0 {
                    failures.push(format!(
                        "{name} MSAA {msaa}: probe {i}: page {a} / Rust {b} (hit point or normal {rest:e} off)"
                    ));
                }
            } else if on_tie(i) {
                if tex != 0.0 || rest != 0.0 {
                    tie_diffs += 1;
                }

                tie_tex = tie_tex.max(tex);
                tie_rest = tie_rest.max(rest);
            } else if rest != 0.0 || tex > SPRITE_TEX_ABS {
                failures.push(format!(
                    "{name} MSAA {msaa}: probe {i}: page {a} / Rust {b} (texCoord {tex:e}, rest {rest:e})"
                ));
            }
        }

        if !ties.is_empty() {
            let measured = (tie_diffs, tie_id_gaps, tie_tex, tie_rest);
            let known = TIE_DIFFS.iter().find(|k| k.0 == name && k.1 == msaa);

            match known {
                Some(k) if (k.2, k.3, k.4, k.5) == measured => {}
                Some(k) => failures.push(format!(
                    "{name} MSAA {msaa}: the known tie differences changed: recorded {:?}, measured {measured:?}",
                    (k.2, k.3, k.4, k.5)
                )),
                None => failures.push(format!(
                    "{name} MSAA {msaa}: no recorded tie differences; measured {measured:?}"
                )),
            }
        }

        eprintln!(
            "{name} MSAA {msaa}: {} probes, {id_gaps} hit/miss gaps, hit point up to {worst:e} off, tie sprites: {tie_diffs} differing probes ({tie_id_gaps} hit/miss), texCoord up to {tie_tex:e}, point/normal up to {tie_rest:e}",
            want.len()
        );
        checked += 1;
    }

    let _ = std::fs::remove_dir_all(&worlds);
    assert!(only.is_some() || checked > 0, "no case");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}
