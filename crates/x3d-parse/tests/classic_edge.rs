//! REA ledger entry sub:parse.vrml-classic: X3D classic encoding and VRML97 edge cases against pinned X_ITE 16.5.0.
//!
//! Baseline: `oracle/classic-edge-baseline/classic-edge.json`, recorded three times (identical) from the pinned page by
//! `python3 -I oracle/tools/classic_edge.py record` (`oracle/x_ite-classic-edge.mjs`). Every input is authored in
//! `oracle/tools/classic_edge.py` and stored in the baseline, so the test reads no other file. A `text` case went through
//! `Browser.createX3DFromString`, a `b64` case (exact bytes) through `Browser.createX3DFromURL`.
//!
//! Per case the port must give, exactly: the accepted or rejected verdict; the rejection text; `toVRMLString`,
//! `toXMLString` and `toJSONString`; the encoding, specification version and profile; components, units and meta data; the
//! number of root nodes; and the console output (level and text, in print order). The recorder leaves out X_ITE's own `info`
//! progress notices (`Done loading ...` of its component loader and FileLoader, `Initializing ...Shader`): they come in the
//! order the modules arrive and the port has no such output. Every `log`, `warn` and `error` line is kept.
//!
//! A difference that has a principled cause is listed in `classic-edge-known.tsv` with a hash of the port's output (field `*`:
//! the whole case): the test fails when the port's output changes, and when a listed case matches (the entry is then stale).
//! It is never a pass.
//!
//! `CLASSIC_EDGE_ONLY=<substring>` restricts the run; `CLASSIC_EDGE_OUT=<dir>` writes `.rust` / `.xite` files for each difference.

use serde_json::Value;
use std::collections::BTreeMap;
use std::path::Path;
use x3d_parse::LoadError;

/// The world URL of a `text` case: the pinned page (`createX3DFromString` takes the page's URL).
const PAGE_URL: &str = "http://oracle/x_ite-pin/src/dump.html";

/// What a case produced, in the baseline's own terms.
struct Outcome {
    ok: bool,
    error: String,
    log: Vec<String>,
    vrml: String,
    xml: String,
    json: String,
    encoding: String,
    spec_version: String,
    profile: String,
    components: Vec<(String, i64)>,
    units: Vec<(String, String, f64)>,
    meta: Vec<(String, String)>,
    root_nodes: usize,
}

fn rejected(error: String, log: Vec<String>) -> Outcome {
    Outcome {
        ok: false,
        error,
        log,
        vrml: String::new(),
        xml: String::new(),
        json: String::new(),
        encoding: String::new(),
        spec_version: String::new(),
        profile: String::new(),
        components: vec![],
        units: vec![],
        meta: vec![],
        root_nodes: 0,
    }
}

/// `GoldenGate.parseIntoScene` for a file that no parser handles.
fn no_handler(url: &str) -> String {
    format!("Couldn't parse X3D. No suitable file handler found for '{url}'.")
}

/// The console lines a parse leaves and the verdict, like the runtime's `browser_scene`: the parser's warnings and errors
/// in print order, then the rejection text (`X3DBrowser.createX3DFromString` rejects with the Error of the parse).
fn run(parsed: Result<x3d_scene::Scene, LoadError>, url: &str) -> Outcome {
    let scene = match parsed {
        Ok(scene) => scene,
        Err(LoadError::Parse(e))
            if e.line == 0
                && ["TypeError: ", "RangeError: ", "Error: "]
                    .iter()
                    .any(|p| e.message.starts_with(p)) =>
        {
            return rejected(e.message, vec![]);
        }
        Err(LoadError::Parse(e)) => {
            let mut log: Vec<String> = x3d_parse::take_error_warnings()
                .into_iter()
                .map(|w| level(&w))
                .collect();

            log.push(format!(
                "error: {}",
                x3d_parse::error_console_text(&e.message)
            ));
            return rejected(
                x3d_parse::take_error_detail().unwrap_or_else(|| format!("Error: \n{e}")),
                log,
            );
        }
        Err(LoadError::NoHandler) => return rejected(no_handler(url), vec![]),
        Err(e) => return rejected(format!("Error: {e}"), vec![]),
    };

    Outcome {
        ok: true,
        error: String::new(),
        log: scene.warnings.iter().map(|w| level(w)).collect(),
        vrml: scene.to_vrml_string(),
        xml: scene.to_xml_string(),
        json: scene.to_json_string(),
        encoding: scene.encoding.clone(),
        spec_version: scene.specification_version.clone(),
        profile: scene.profile.clone().unwrap_or_default(),
        components: scene
            .components
            .iter()
            .map(|(n, l)| (n.clone(), i64::from(*l)))
            .collect(),
        units: scene
            .units
            .iter()
            .map(|u| (u.category.clone(), u.name.clone(), u.conversion_factor))
            .collect(),
        meta: scene.meta.clone(),
        root_nodes: scene.contexts[0].root_nodes.len(),
    }
}

/// A `Scene::warnings` entry as a console line: `warn: ...` or `error: ...`.
fn level(entry: &str) -> String {
    let (is_error, text) = x3d_scene::console_level(entry);

    format!("{}: {text}", if is_error { "error" } else { "warn" })
}

fn first_difference(expected: &str, actual: &str) -> String {
    let (e, a): (Vec<&str>, Vec<&str>) =
        (expected.split('\n').collect(), actual.split('\n').collect());

    for i in 0..e.len().max(a.len()) {
        if e.get(i) != a.get(i) {
            return format!(
                "line {}:\n  X_ITE: {:?}\n  Rust:  {:?}",
                i + 1,
                e.get(i).unwrap_or(&"<EOF>"),
                a.get(i).unwrap_or(&"<EOF>")
            );
        }
    }

    String::new()
}

/// FNV-1a, 64 bit: a stable hash for the known-difference table (std's hashers are not stable across releases).
fn fnv(text: &str) -> String {
    let mut h: u64 = 0xcbf29ce484222325;

    for b in text.bytes() {
        h ^= u64::from(b);
        h = h.wrapping_mul(0x100000001b3);
    }

    format!("{h:016x}")
}

fn parser_log(lines: &[Value]) -> Vec<String> {
    lines
        .iter()
        .map(|l| l.as_str().unwrap().to_string())
        .collect()
}

fn pinned(entry: &Value) -> Outcome {
    let s = |k: &str| entry[k].as_str().unwrap_or("").to_string();
    let log = parser_log(entry["log"].as_array().unwrap());

    if !entry["ok"].as_bool().unwrap() {
        return rejected(s("error"), log);
    }

    Outcome {
        ok: true,
        error: String::new(),
        log,
        vrml: s("vrml"),
        xml: s("xml"),
        json: s("json"),
        encoding: s("encoding"),
        spec_version: s("specificationVersion"),
        profile: s("profile"),
        components: entry["components"]
            .as_array()
            .unwrap()
            .iter()
            .map(|c| (c[0].as_str().unwrap().to_string(), c[1].as_i64().unwrap()))
            .collect(),
        units: entry["units"]
            .as_array()
            .unwrap()
            .iter()
            .map(|u| {
                (
                    u[0].as_str().unwrap().to_string(),
                    u[1].as_str().unwrap().to_string(),
                    // JSON has no Infinity or NaN: the recorder writes those as the strings of String (number).
                    match u[2].as_str() {
                        Some("Infinity") => f64::INFINITY,
                        Some("-Infinity") => f64::NEG_INFINITY,
                        Some("NaN") => f64::NAN,
                        Some(other) => panic!("unit factor {other}"),
                        None => u[2].as_f64().unwrap(),
                    },
                )
            })
            .collect(),
        meta: entry["meta"]
            .as_array()
            .unwrap()
            .iter()
            .map(|m| {
                (
                    m[0].as_str().unwrap().to_string(),
                    m[1].as_str().unwrap().to_string(),
                )
            })
            .collect(),
        root_nodes: entry["rootNodes"].as_u64().unwrap() as usize,
    }
}

/// The compared parts of a case, as (field, pinned text, port text). Empty when the case matches.
fn differences(p: &Outcome, r: &Outcome) -> Vec<(&'static str, String, String)> {
    let mut out = vec![];
    let mut add = |field: &'static str, a: String, b: String| {
        if a != b {
            out.push((field, a, b));
        }
    };

    add("ok", p.ok.to_string(), r.ok.to_string());
    add("error", p.error.clone(), r.error.clone());
    add("log", p.log.join("\n"), r.log.join("\n"));
    add("vrml", p.vrml.clone(), r.vrml.clone());
    add("xml", p.xml.clone(), r.xml.clone());
    add("json", p.json.clone(), r.json.clone());
    add("encoding", p.encoding.clone(), r.encoding.clone());
    add(
        "specificationVersion",
        p.spec_version.clone(),
        r.spec_version.clone(),
    );
    add("profile", p.profile.clone(), r.profile.clone());
    add(
        "components",
        format!("{:?}", p.components),
        format!("{:?}", r.components),
    );
    add("units", format!("{:?}", p.units), format!("{:?}", r.units));
    add("meta", format!("{:?}", p.meta), format!("{:?}", r.meta));
    add(
        "rootNodes",
        p.root_nodes.to_string(),
        r.root_nodes.to_string(),
    );
    out
}

/// `name<TAB>field<TAB>fnv of the port's text<TAB>cause`, one line per known difference.
fn known() -> BTreeMap<(String, String), (String, String)> {
    let text = std::fs::read_to_string(
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/classic-edge-known.tsv"),
    )
    .unwrap();

    text.lines()
        .filter(|l| !l.starts_with('#') && !l.is_empty())
        .map(|l| {
            let f: Vec<&str> = l.splitn(4, '\t').collect();

            (
                (f[0].to_string(), f[1].to_string()),
                (f[2].to_string(), f[3].to_string()),
            )
        })
        .collect()
}

#[test]
fn classic_edge_cases_match_x_ite() {
    let path = std::env::var("CLASSIC_EDGE_BASELINE").unwrap_or_else(|_| {
        format!(
            "{}/../../oracle/classic-edge-baseline/classic-edge.json",
            env!("CARGO_MANIFEST_DIR")
        )
    });
    let baseline: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let only = std::env::var("CLASSIC_EDGE_ONLY").ok();
    let out = std::env::var("CLASSIC_EDGE_OUT").ok();
    let known = known();
    let mut failures = vec![];
    let mut used = vec![];
    let (mut cases, mut exact, mut known_diffs) = (0, 0, 0);

    for entry in baseline["results"].as_array().unwrap() {
        let name = entry["name"].as_str().unwrap();

        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }

        cases += 1;

        let rust = match (entry["text"].as_str(), entry["b64"].as_str()) {
            (Some(text), _) => run(x3d_parse::parse_string(text, PAGE_URL), PAGE_URL),
            (None, Some(b64)) => {
                let bytes = decode_base64(b64);
                let url = format!("http://oracle/__case/{}", encode_uri_component(name));
                let mut outcome = run(x3d_parse::parse_bytes(&bytes, &url), &url);

                // FileLoader.createX3DFromURL: a url that does not load prints its error (the whole rejection of the parse) and
                // rejects with `Couldn't load X3D file.`.
                if !outcome.ok {
                    outcome.log.push(format!(
                        "error: Couldn't load URL '{url}'. Error: {}",
                        outcome.error
                    ));
                    outcome.error = "Couldn't load X3D file.".into();
                }

                outcome
            }
            _ => panic!("{name}: no input"),
        };
        let diffs = differences(&pinned(entry), &rust);

        if diffs.is_empty() {
            exact += 1;
            continue;
        }

        // `*` in the table: the whole case differs (the hash covers every differing part, in order).
        let whole = (name.to_string(), "*".to_string());

        if let Some((hash, _)) = known.get(&whole) {
            let all: String = diffs
                .iter()
                .map(|(f, _, a)| format!("{f}\0{a}\0"))
                .collect();

            used.push(whole);

            if *hash == fnv(&all) {
                known_diffs += 1;
            } else {
                failures.push(format!(
                    "{name} [*]: the port's output changed from the known difference ({hash} -> {})",
                    fnv(&all)
                ));
            }

            continue;
        }

        for (field, expected, actual) in diffs {
            let key = (name.to_string(), field.to_string());

            match known.get(&key) {
                Some((hash, _)) if *hash == fnv(&actual) => {
                    used.push(key);
                    known_diffs += 1;
                }
                Some((hash, _)) => {
                    used.push(key);
                    failures.push(format!(
                        "{name} [{field}]: the port's output changed from the known difference ({hash} -> {}): {}",
                        fnv(&actual),
                        first_difference(&expected, &actual)
                    ));
                }
                None => {
                    failures.push(format!(
                        "{name} [{field}] (port hash {}): {}",
                        fnv(&actual),
                        first_difference(&expected, &actual)
                    ));

                    if let Some(dir) = &out {
                        std::fs::create_dir_all(dir).unwrap();
                        std::fs::write(
                            Path::new(dir).join(format!("{name}.{field}.rust")),
                            &actual,
                        )
                        .unwrap();
                        std::fs::write(
                            Path::new(dir).join(format!("{name}.{field}.xite")),
                            &expected,
                        )
                        .unwrap();
                    }
                }
            }
        }
    }

    // A listed difference whose case now matches is stale.
    if only.is_none() {
        for key in known.keys().filter(|k| !used.contains(k)) {
            failures.push(format!(
                "{} [{}]: listed as a known difference, but the port matches X_ITE now",
                key.0, key.1
            ));
        }
    }

    println!(
        "{cases} cases, {exact} exact, {known_diffs} known differences, {} failures",
        failures.len()
    );
    assert!(
        failures.is_empty(),
        "{} problems in {cases} cases:\n{}",
        failures.len(),
        failures.join("\n")
    );

    if only.is_none() {
        assert_eq!(cases, baseline["results"].as_array().unwrap().len());
    }
}

/// The case names are plain ASCII words and hyphens, which `encodeURIComponent` leaves alone.
fn encode_uri_component(name: &str) -> &str {
    assert!(name.bytes().all(|b| b.is_ascii_alphanumeric() || b == b'-'));
    name
}

fn decode_base64(text: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => panic!("bad base64"),
    };
    let mut out = vec![];
    let (mut acc, mut bits) = (0u32, 0);

    for c in text.bytes().filter(|c| *c != b'=') {
        acc = (acc << 6) | u32::from(value(c));
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push((acc >> bits) as u8);
            acc &= (1 << bits) - 1;
        }
    }

    out
}
