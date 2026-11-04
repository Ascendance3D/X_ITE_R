//! REA ledger entry sub:io.file-loader: the FileLoader's HTTP side against pinned X_ITE 16.5.0: redirects (301, 302, 303, 307, 308, relative
//! and absolute `Location`, chains, a loop, 20 and 21 hops, a redirect to a scheme that must not be followed, to a URL with
//! credentials, across origins), failed statuses, content types, headers, `Content-Encoding`, a cut body, a closed connection, and the
//! `text/html` hand-off to `foreign` (`Browser.loadURL`: `window .open (href, target)` or `location = href`).
//!
//! The baselines are oracle/file-http-baseline/fh-*.json, recorded three times from the pinned page by `oracle/tools/file_http.py record`.
//! The pinned page and the port read one route table, oracle/synthetic-rea-file-http/routes.json: the recorder serves it from Node, this
//! test serves it from `x3d_http::route_server` (std only, 127.0.0.1, two ports) and `x3d-drive --http-map` sends the requests under `/h/`
//! to it over HTTP (crate `x3d-http`, ureq). The scene text of every dumped frame, the sorted log and the list of hand-offs
//! (`foreign`) are compared exactly. No rendering, no GPU.
//!
//! `fh-diff` holds the cases where the port differs from the page; `known_differences_are_pinned` fails on any change of either side.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;
use x3d_http::route_server::RouteServer;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

fn groups(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root.join("oracle/file-http-baseline"))
        .unwrap()
        .filter_map(|e| {
            let name = e.unwrap().file_name().into_string().unwrap();
            name.strip_suffix(".json").map(str::to_string)
        })
        .collect();

    names.sort();
    names
}

/// Run `x3d-drive` on the group's world with the route table served over HTTP; the port's result.
fn drive(root: &Path, name: &str) -> (Value, RouteServer) {
    let baseline = root.join(format!("oracle/file-http-baseline/{name}.json"));
    let routes =
        std::fs::read_to_string(root.join("oracle/synthetic-rea-file-http/routes.json")).unwrap();
    let server = RouteServer::from_json(&routes).unwrap();
    let out = std::env::temp_dir().join(format!("file-http-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let script = if name == "fh-foreign" {
        "oracle/synthetic-rea-file-http/fh-foreign.json"
    } else {
        "oracle/synthetic-rea-file-http/fh.json"
    };
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(root)
        .args([
            &format!("oracle/synthetic-rea-file-http/{name}.wrl"),
            script,
            "--oracle",
            "oracle",
            "--http-map",
            &format!("http://oracle={}", server.origin),
        ])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(&baseline)
        .status()
        .unwrap();

    assert!(status.success(), "{name}: x3d-drive exited with {status}");

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let _ = std::fs::remove_dir_all(&out);
    (rs, server)
}

fn baseline(root: &Path, name: &str) -> Value {
    serde_json::from_str(
        &std::fs::read_to_string(root.join(format!("oracle/file-http-baseline/{name}.json")))
            .unwrap(),
    )
    .unwrap()
}

fn last_frame(v: &Value) -> String {
    let frames = v["vrml"].as_object().unwrap();
    let last = frames
        .keys()
        .max_by_key(|k| k.parse::<u32>().unwrap())
        .unwrap();

    frames[last].as_str().unwrap().to_string()
}

fn compare_group(root: &Path, name: &str) -> Vec<String> {
    let js = baseline(root, name);
    let (rs, _server) = drive(root, name);
    let mut failures = vec![];
    let mut fail = |what: String| failures.push(format!("{name}: {what}"));

    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        fail(format!("ok js {} rs {}", js["ok"], rs["ok"]));
        return failures;
    }

    // The baseline is not vacuous: every list of the world finished, and the cases are loaded and failed ones.
    let text = last_frame(&js);

    if text.contains(": ...\"") || !text.contains(": REJ ") {
        fail("the baseline's last frame is not a finished run".to_string());
    }

    if name != "fh-foreign" && !text.contains(": RES ") {
        fail("the baseline has no loaded scene".to_string());
    }

    if name == "fh-foreign" && js["foreign"].as_array().is_none_or(|f| f.len() < 15) {
        fail("the baseline has hardly any hand-off".to_string());
    }

    let mut bad = vec![];

    for (k, a) in js["vrml"].as_object().unwrap() {
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

    let strings = |v: &Value| -> Vec<String> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(str::to_string))
                    .collect()
            })
            .unwrap_or_default()
    };

    if js["log"] != rs["log"] {
        let (a, b) = (strings(&js["log"]), strings(&rs["log"]));
        let only_js: Vec<&String> = a.iter().filter(|l| !b.contains(l)).collect();
        let only_rs: Vec<&String> = b.iter().filter(|l| !a.contains(l)).collect();
        fail(format!(
            "log differs ({} / {} lines)\n  only pinned: {only_js:?}\n  only port: {only_rs:?}",
            a.len(),
            b.len()
        ));
    }

    // The hand-offs to `foreign`, in order: (target, href).
    if name == "fh-foreign" && js["foreign"] != rs["foreign"] {
        fail(format!(
            "foreign differs\n  pinned: {}\n  port:   {}",
            js["foreign"], rs["foreign"]
        ));
    }

    failures
}

fn check(group: &str) {
    let failures = compare_group(&root(), group);

    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn http_redirects_match_x_ite() {
    check("fh-redirect");
}

#[test]
fn http_statuses_content_types_and_encodings_match_x_ite() {
    check("fh-status");
}

#[test]
fn text_html_hand_off_to_foreign_matches_x_ite() {
    check("fh-foreign");
}

/// The result strings of a one-line-per-case world: `"N: text"`.
fn results(frame: &str) -> Vec<String> {
    frame
        .lines()
        .filter_map(|l| {
            let t = l.trim().trim_end_matches(',');
            let t = t.strip_prefix('"')?.strip_suffix('"')?;
            t.split_once(": ")
                .filter(|(n, _)| n.chars().all(|c| c.is_ascii_digit()))
                .map(|(n, rest)| format!("{n}: {rest}"))
        })
        .collect()
}

/// Where the port differs from the pinned page, and what each says. A change of either side fails this test. Each is a difference of
/// the platform, not of the loader: the browser's CORS rule has no counterpart in a program with no origin, and the HTTP client keeps the
/// canonical reason phrase of a status, not the phrase that the server sent (the page shows the phrase on the wire).
#[test]
fn known_differences_are_pinned() {
    let root = root();
    let (rs, server) = drive(&root, "fh-diff");
    let js = baseline(&root, "fh-diff");
    let (pinned, port) = (results(&last_frame(&js)), results(&last_frame(&rs)));

    assert_eq!(pinned.len(), 4);
    assert_eq!(port.len(), 4, "{port:?}");
    // The page refuses a redirect to another origin whose answer has no `Access-Control-Allow-Origin`; the port has no origin.
    assert_eq!(pinned[0], "0: REJ Couldn't load X3D file.");
    assert_eq!(port[0], "0: RES h/r-cross-nocors title other");
    // The reason phrases: the page says what the server sent, the port the canonical phrase of the status.
    assert_eq!(pinned[1], "1: REJ Couldn't load X3D file.");
    assert_eq!(pinned[2], "2: REJ Couldn't load X3D file.");
    assert_eq!(pinned[3], "3: REJ Couldn't load X3D file.");
    assert_eq!(port[1..], pinned[1..]);
    assert_eq!(
        js["log"],
        serde_json::json!([
            "error: Script: Couldn't load URL 'http://oracle/h/r-cross-nocors'. TypeError: Failed to fetch",
            "error: Script: Couldn't load URL 'http://oracle/h/s-reason'. Error: Gone Fishing",
            "error: Script: Couldn't load URL 'http://oracle/h/s-reason-empty'. Error: 404",
            "error: Script: Couldn't load URL 'http://oracle/h/s418'. Error: I'm a Teapot"
        ])
    );
    assert_eq!(
        rs["log"],
        serde_json::json!([
            "error: Script: Couldn't load URL 'http://oracle/h/s-reason'. Error: Not Found",
            "error: Script: Couldn't load URL 'http://oracle/h/s-reason-empty'. Error: Not Found",
            "error: Script: Couldn't load URL 'http://oracle/h/s418'. Error: I'm a teapot"
        ])
    );
    // The port followed the cross-origin redirect: the second server was asked.
    assert!(server.requests().iter().any(|r| r == "GET /h/other"));
}

/// The baseline directory holds exactly the four groups, so a new baseline cannot be left without a test.
#[test]
fn file_http_baseline_groups_are_the_tested_ones() {
    assert_eq!(
        groups(&root()),
        ["fh-diff", "fh-foreign", "fh-redirect", "fh-status"]
    );
}
