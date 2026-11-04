//! REA ledger entries sub:parse.gltf2 and sub:parse.glb2: a glTF or GLB whose external buffers, images or EXT_texture_video sources fail to
//! load, through `Browser.createX3DFromURL`, against pinned X_ITE 16.5.0 (`GLTF2Parser.js`, `GLB2Parser.js`, `FileLoader.js`, ImageTexture,
//! MovieTexture). Baselines are oracle/gltf-fetch-baseline/gf-*.json, recorded three times from the pinned page by
//! `oracle/tools/gltf_fetch.py record`; the authored inputs and the worlds are in oracle/synthetic-rea-gltf-fetch (made by
//! oracle/tools/gen_gltf_fetch.py).
//!
//! Each world's Script loads its URL lists one after the other (after the world itself loaded) and records, per list, the scene head and
//! `toVRMLString` of the scene or the rejection in a field of its own, so the scene text of every dumped frame carries it. The loaders'
//! diagnostics (the buffer's `Couldn't load URL ...`, the node's `ImageTexture: ...` or `Error loading movie ...`) are the sorted `log`.
//! Compared like oracle/tools/x10_compare.py: the scene text of every dumped frame exact, the sorted log exact, with one normalization:
//! the pinned page names a decoded blob by a random UUID (`Couldn't load image 'blob:http://oracle/<uuid>'`), which is replaced by `UUID`
//! on both sides. No rendering.
//!
//! One known difference is pinned, not passed: a decoded image that fails (`X3DSingleTextureNode.loadImage`) is named in the pinned page
//! by the blob URL it made of the fetched bytes (`blob:<origin>/<uuid>`, random), where the engine has no blob URL and names the file or
//! data URL itself (`image_texture_set_image`). [`KNOWN_UNBLOBBED`] lists, per group, the exact names the engine prints in place of the
//! blob; the pinned line count must match, and the test fails as soon as the engine prints any other name (a blob among them).

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn root() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap()
}

/// `blob:http://oracle/<uuid>` becomes `blob:http://oracle/UUID`: the page's name for a decoded blob is random.
fn normalize_blob(line: &str) -> String {
    const PREFIX: &str = "blob:http://oracle/";
    let mut out = String::new();
    let mut rest = line;

    while let Some(at) = rest.find(PREFIX) {
        let (before, after) = rest.split_at(at + PREFIX.len());
        let end = after
            .find(|c: char| !(c.is_ascii_hexdigit() || c == '-'))
            .unwrap_or(after.len());

        out.push_str(before);
        out.push_str("UUID");
        rest = &after[end..];
    }

    out.push_str(rest);
    out
}

/// Per group, what the engine prints inside `Couldn't load image '...'` where the pinned page prints a blob URL, and how often.
const KNOWN_UNBLOBBED: &[(&str, &[(&str, usize)])] = &[
    ("gf-buffer", &[]),
    (
        "gf-image",
        &[
            (
                "http://oracle/synthetic-rea-gltf-fetch/img/not-a-png.png",
                1,
            ),
            ("data:image/png;base64,", 1),
            (
                "data:image/png;base64,Z2FyYmFnZSBieXRlcywgbm90IGFuIGltYWdl",
                3,
            ),
            ("data:image/png;base64,bm90IGFuIGltYWdlIGF0IGFsbA==", 1),
        ],
    ),
    ("gf-video", &[]),
];

/// The engine's line with the name of a failed image replaced by the pinned page's blob form, and that name.
fn unblob(line: &str) -> (String, Option<String>) {
    const OPEN: &str = "Couldn't load image '";
    const CLOSE: &str = "': error.";

    let Some(at) = line.find(OPEN) else {
        return (line.to_string(), None);
    };
    let start = at + OPEN.len();
    let Some(len) = line[start..].find(CLOSE) else {
        return (line.to_string(), None);
    };
    let name = &line[start..start + len];

    if name.starts_with("blob:") {
        return (line.to_string(), None);
    }

    (
        format!(
            "{}blob:http://oracle/UUID{}",
            &line[..start],
            &line[start + len..]
        ),
        Some(name.to_string()),
    )
}

/// The groups of the baseline directory (the json files in it, nothing nested).
fn groups(root: &Path) -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(root.join("oracle/gltf-fetch-baseline"))
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
    let baseline = root.join(format!("oracle/gltf-fetch-baseline/{name}.json"));
    let js: Value = serde_json::from_str(&std::fs::read_to_string(&baseline).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("gltf-fetch-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(root)
        .args([
            &format!("oracle/synthetic-rea-gltf-fetch/{name}.wrl"),
            "oracle/synthetic-rea-gltf-fetch/gf.json",
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

    if text.contains(": ...\"") || !text.contains(": RES ") {
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

    // `log`: the warn / error lines, sorted (a blob's random UUID normalized, the pinned blob names compared with the known ones).
    let lines = |v: &Value| -> Vec<String> {
        v.as_array()
            .map(|a| {
                a.iter()
                    .filter_map(|s| s.as_str().map(normalize_blob))
                    .collect()
            })
            .unwrap_or_default()
    };
    let mut a = lines(&js["log"]);
    let mut named: Vec<String> = vec![];
    let mut b: Vec<String> = lines(&rs["log"])
        .iter()
        .map(|l| {
            let (line, name) = unblob(l);
            named.extend(name);
            line
        })
        .collect();

    a.sort();
    b.sort();

    if a != b {
        let only_js: Vec<&String> = a.iter().filter(|l| !b.contains(l)).collect();
        let only_rs: Vec<&String> = b.iter().filter(|l| !a.contains(l)).collect();
        fail(format!(
            "log differs ({} / {} lines)\n  only pinned: {only_js:?}\n  only port: {only_rs:?}",
            a.len(),
            b.len()
        ));
    }

    named.sort();

    let mut known: Vec<String> = KNOWN_UNBLOBBED
        .iter()
        .find(|(g, _)| *g == name)
        .unwrap_or_else(|| panic!("{name}: no entry in KNOWN_UNBLOBBED"))
        .1
        .iter()
        .flat_map(|(n, count)| std::iter::repeat_n(n.to_string(), *count))
        .collect();

    known.sort();

    if named != known {
        fail(format!(
            "the names the engine prints for failed images changed:\n  expected {known:?}\n  got      {named:?}"
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
fn gltf_buffer_fetch_matches_x_ite() {
    check("gf-buffer");
}

#[test]
fn gltf_image_fetch_matches_x_ite() {
    check("gf-image");
}

#[test]
fn gltf_video_fetch_matches_x_ite() {
    check("gf-video");
}

/// The baseline directory holds exactly the three groups, so a new baseline cannot be left without a test.
#[test]
fn gltf_fetch_baseline_groups_are_the_tested_ones() {
    assert_eq!(groups(&root()), ["gf-buffer", "gf-image", "gf-video"]);
}

#[test]
fn blob_uuid_is_normalized() {
    assert_eq!(
        normalize_blob(
            "a 'blob:http://oracle/56215cd8-6838-48e3-85f5-752ba8a4e16d': error. blob:http://oracle/ab-1 x"
        ),
        "a 'blob:http://oracle/UUID': error. blob:http://oracle/UUID x"
    );
}
