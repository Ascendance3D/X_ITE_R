//! REA ledger entry sub:parse.golden-gate: GoldenGate's dispatch of binary input (GLB2Parser, STLBParser, PLYBParser) through
//! `Browser.createX3DFromURL`, against pinned X_ITE 16.5.0 (`GoldenGate.js`, `FileLoader.js`). Baselines are
//! oracle/golden-gate-binary-baseline/gg-*.json, recorded three times from the pinned page by `oracle/tools/golden_gate_binary.py record`;
//! the authored inputs and the worlds are in oracle/synthetic-rea-golden-gate-binary (made by oracle/tools/gen_golden_gate_binary.py).
//!
//! Each world's Script loads its URL lists one after the other and records, per list, the scene head and `toVRMLString` of the scene
//! or the rejection (`Couldn't load X3D file.`) in a field of its own, so the scene text of every dumped frame carries it. The
//! loader's diagnostic for a failed candidate (`Couldn't load URL ..., Error: <parser error>`, the warning when a later URL follows) is
//! the sorted `log`. Compared like oracle/tools/x10_compare.py: the scene text of every dumped frame exact, the sorted log exact.
//! No rendering.
//!
//! gg-stl-nonfinite: binary and ASCII STL with NaN, +Inf and -Inf coordinates and normals. The scene text is compared exactly (a binary
//! file keeps every non-finite value; ASCII takes only a SIGNED `nan` / `inf` constant). The pinned page's log also holds crashes of its
//! MikkTSpace WebAssembly (see PINNED_WASM_CRASH); that is the one named difference, and nothing else in the log may differ.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// Upstream defect, not copied. A non-finite vertex position (NaN, +Inf, -Inf) in a TriangleSet makes the pinned page's
/// `MikkTSpace.generateTangents` (the WebAssembly tangent generator, called from `X3DGeometryNode.generateTangents` for every
/// geometry that is not a point or line set) crash: `RangeError: Maximum call stack size exceeded` (NaN: unbounded recursion in the
/// wasm), `RuntimeError: unreachable` (Inf), and once the wasm instance is damaged `RuntimeError: memory access out of bounds` for the
/// geometries after it. `generateTangents` catches the error, logs it only in the unbundled development source (`if (DEVELOPMENT)`
/// console.error), and leaves the geometry without tangents; the scene, its text and the load are unaffected. A crash on a finite file
/// is not required behavior, and the port has no such crash (no panic, same scene), so the port logs none of these lines.
/// Evidence: oracle/golden-gate-binary-baseline/gg-stl-nonfinite.json `log`; one case per file in a fresh page gives
/// nonfinite-coord-nan RangeError, nonfinite-coord-inf / -ninf / nonfinite-mixed `unreachable`, nonfinite-normal-* no error.
fn is_pinned_wasm_crash(line: &str) -> bool {
    line.starts_with("error: RangeError: Maximum call stack size exceeded")
        || line.starts_with("error: RuntimeError: unreachable")
        || line.starts_with("error: RuntimeError: memory access out of bounds")
}

/// The groups of the baseline directory (the json files in it, nothing nested).
fn groups(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root.join("oracle/golden-gate-binary-baseline"))
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".json").map(str::to_string)
        })
        .collect();

    names.sort();
    names
}

/// Run `x3d-drive` on the group's world and compare it with the baseline; the failures are returned.
fn compare_group(root: &Path, name: &str) -> Vec<String> {
    let baseline = root.join(format!("oracle/golden-gate-binary-baseline/{name}.json"));
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    let out =
        std::env::temp_dir().join(format!("golden-gate-binary-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(root)
        .args([
            &format!("oracle/synthetic-rea-golden-gate-binary/{name}.wrl"),
            "oracle/synthetic-rea-golden-gate-binary/gg.json",
            "--oracle",
            "oracle",
        ])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(&baseline)
        .status()
        .unwrap();
    let mut failures = vec![];
    let mut fail = |what: String| failures.push(format!("{name}: {what}"));

    if !status.success() {
        fail(format!("x3d-drive exited with {status}"));
        return failures;
    }

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&out);

    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        fail(format!("ok js {} rs {}", js["ok"], rs["ok"]));
        return failures;
    }

    // The baseline is not vacuous: every list of the world finished, in the last dumped frame, and the dumped frames are all there.
    let frames = js["vrml"].as_object().unwrap();
    let last = frames
        .keys()
        .max_by_key(|k| k.parse::<u32>().unwrap())
        .unwrap();
    let text = frames[last].as_str().unwrap();

    if text.contains(": ...\"") || !text.contains(": RES ") || !text.contains(": REJ ") {
        fail(format!("the baseline's frame {last} is not a finished run"));
    }

    let mut bad = vec![];

    for (k, a) in frames {
        if rs["vrml"].get(k) != Some(a) {
            bad.push(k.clone());
        }
    }

    if !bad.is_empty() {
        bad.sort_by_key(|k| k.parse::<u32>().unwrap_or(0));
        let k = &bad[0];
        let (a, b) = (
            js["vrml"][k].as_str().unwrap(),
            rs["vrml"][k].as_str().unwrap_or(""),
        );
        let first = a
            .lines()
            .zip(b.lines())
            .position(|(x, y)| x != y)
            .unwrap_or_else(|| a.lines().count().min(b.lines().count()));
        fail(format!(
            "scene text differs at {} dumped frames, first {k}, first differing line {}:\n  pinned: {:?}\n  port:   {:?}",
            bad.len(),
            first + 1,
            a.lines().nth(first),
            b.lines().nth(first)
        ));
    }

    // `log`: the sorted warn / error lines.
    let strip = |v: &Value| -> Value {
        if name != "gg-stl-nonfinite" {
            return v.clone();
        }
        Value::Array(
            v.as_array()
                .unwrap()
                .iter()
                .filter(|l| !is_pinned_wasm_crash(l.as_str().unwrap()))
                .cloned()
                .collect(),
        )
    };
    if name == "gg-stl-nonfinite" {
        let crashes = |v: &Value| {
            v.as_array()
                .unwrap()
                .iter()
                .filter(|l| is_pinned_wasm_crash(l.as_str().unwrap()))
                .count()
        };
        if crashes(&js["log"]) == 0 {
            fail("the pinned log has no MikkTSpace crash: the named difference is gone".into());
        }
        if crashes(&rs["log"]) != 0 {
            fail("the port logs a crash line".into());
        }
    }
    let (js_log, rs_log) = (strip(&js["log"]), strip(&rs["log"]));
    if js_log != rs_log {
        let strings = |v: &Value| -> Vec<String> {
            v.as_array()
                .map(|a| {
                    a.iter()
                        .filter_map(|s| s.as_str().map(str::to_string))
                        .collect()
                })
                .unwrap_or_default()
        };
        let (a, b) = (strings(&js_log), strings(&rs_log));
        let only_js: Vec<&String> = a.iter().filter(|l| !b.contains(l)).collect();
        let only_rs: Vec<&String> = b.iter().filter(|l| !a.contains(l)).collect();
        fail(format!(
            "log differs ({} / {} lines)\n  only pinned: {only_js:?}\n  only port: {only_rs:?}",
            a.len(),
            b.len()
        ));
    }

    failures
}

fn check(group: &str) {
    let root = root();
    let failures = compare_group(&root, group);

    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn golden_gate_stl_binary_matches_x_ite() {
    check("gg-stl");
}

#[test]
fn golden_gate_stl_nonfinite_matches_x_ite() {
    check("gg-stl-nonfinite");
}

#[test]
fn golden_gate_ply_binary_matches_x_ite() {
    check("gg-ply");
}

#[test]
fn golden_gate_ply_splat_matches_x_ite() {
    check("gg-ply-splat");
}

#[test]
fn golden_gate_glb_matches_x_ite() {
    check("gg-glb");
}

#[test]
fn golden_gate_dispatch_matches_x_ite() {
    check("gg-dispatch");
}

/// The baseline directory holds exactly the five groups, so a new baseline cannot be left without a test.
#[test]
fn golden_gate_baseline_groups_are_the_tested_ones() {
    assert_eq!(
        groups(&root()),
        [
            "gg-dispatch",
            "gg-glb",
            "gg-ply",
            "gg-ply-splat",
            "gg-stl",
            "gg-stl-nonfinite"
        ]
    );
}
