//! The published `vrml97` facade against the classic-edge baseline (`oracle/classic-edge-baseline/classic-edge.json`, recorded from
//! pinned X_ITE 16.5.0 by `oracle/tools/classic_edge.py`; the full comparison with the console text is
//! `crates/x3d-parse/tests/classic_edge.rs`). This test goes through the entry points a user of the crate has: `vrml97::parse_bytes`
//! (gunzip and text decoding included) and `vrml97::load_file` (a file on disk), and compares the verdict and `toVRMLString`.
//!
//! Left out: the `sweep-annotation` cases (a known difference of the full test), a `text` case that holds U+FEFF (`createX3DFromString` keeps a byte order mark that a file load removes) and the cases of
//! the `hdr-bom` family for the same reason.

use std::path::Path;

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

#[test]
fn the_facade_reads_the_classic_edge_cases_like_x_ite() {
    let path = Path::new(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle/classic-edge-baseline/classic-edge.json");
    let baseline: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();
    let dir = std::env::temp_dir().join(format!("classic-edge-facade-{}", std::process::id()));
    std::fs::create_dir_all(&dir).unwrap();

    let (mut checked, mut bad) = (0, vec![]);

    for entry in baseline["results"].as_array().unwrap() {
        let name = entry["name"].as_str().unwrap();
        let bytes = match (entry["text"].as_str(), entry["b64"].as_str()) {
            // The port supports the Annotation component, which X_ITE 16.5.0 does not register (classic-edge-known.tsv).
            _ if name.starts_with("sweep-annotation-") => continue,
            (Some(text), _) if text.contains('\u{feff}') => continue,
            (Some(_), _) if name.starts_with("hdr-bom") => continue,
            (Some(text), _) => text.as_bytes().to_vec(),
            (None, Some(b64)) => decode_base64(b64),
            _ => panic!("{name}: no input"),
        };

        // A text case ran through createX3DFromString, whose world is the page; a file load takes the file as the world.
        // The verdict and the printed scene do not depend on the world, except that an error text names it (not compared here).
        let file = dir.join(format!("{name}.x3dv"));

        std::fs::write(&file, &bytes).unwrap();

        let from_bytes = vrml97::parse_bytes(&bytes, "http://oracle/case");
        let from_file = vrml97::load_file(&file);

        // A byte case that FileLoader could not load, and a text case that was rejected, are both rejections.
        let expected_ok = entry["ok"].as_bool().unwrap();

        checked += 1;

        for (what, got) in [("parse_bytes", &from_bytes), ("load_file", &from_file)] {
            match got {
                Ok(scene) if expected_ok => {
                    if scene.to_vrml_string() != entry["vrml"].as_str().unwrap() {
                        bad.push(format!("{name} [{what}]: toVRMLString differs"));
                    }
                }
                Ok(_) => bad.push(format!(
                    "{name} [{what}]: X_ITE rejected it, the port accepted it"
                )),
                Err(e) if expected_ok => bad.push(format!(
                    "{name} [{what}]: X_ITE accepted it, the port rejected it: {e}"
                )),
                Err(_) => {}
            }
        }
    }

    let _ = std::fs::remove_dir_all(&dir);
    println!("{checked} cases, {} problems", bad.len());
    assert!(checked > 1500, "only {checked} cases");
    assert!(bad.is_empty(), "{}", bad.join("\n"));
}
