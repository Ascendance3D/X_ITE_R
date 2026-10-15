//! Suites 4 and 5 — Parser and scene compatibility against the pinned X_ITE oracle.
//!
//! Every fixture in `tests/fixtures/xite/` was loaded by pinned X_ITE 34cdf9f97 (16.5.0)
//! through `oracle/x_ite-dump.mjs` in headless Chromium. `ORACLE.json` holds its
//! `toVRMLString ()` text, or its load error, and its sorted console warnings.
//!
//! The Rust parsers (`vrml97::parse_bytes`, `x3d_parse::parse_bytes`) parse only. They do
//! not fetch EXTERNPROTO or Inline URLs, so the oracle's `error:` lines from those fetches
//! are outside this contract. Parser warnings (`warn:` lines) are inside it.

use std::collections::HashMap;

struct Oracle {
    ok: bool,
    vrml: Option<String>,
    warns: Vec<String>,
    errors: Vec<String>,
}

fn oracle() -> HashMap<String, Oracle> {
    let json: serde_json::Value = serde_json::from_slice(&r0_2_c_tests::fixture("xite/ORACLE.json")).unwrap();
    assert_eq!(json["pinned"], "34cdf9f97");
    let mut map = HashMap::new();
    for r in json["results"].as_array().unwrap() {
        let log: Vec<String> = r["log"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()).collect();
        map.insert(
            r["file"].as_str().unwrap().to_string(),
            Oracle {
                ok: r["ok"].as_bool().unwrap(),
                vrml: r["vrml"].as_str().map(str::to_string),
                warns: log.iter().filter_map(|l| l.strip_prefix("warn: ").map(str::to_string)).collect(),
                errors: log.iter().filter_map(|l| l.strip_prefix("error: ").map(str::to_string)).collect(),
            },
        );
    }
    assert_eq!(map.len(), 22);
    map
}

fn input(file: &str) -> Vec<u8> {
    r0_2_c_tests::fixture(&format!("xite/{file}"))
}

/// Published and engine output both equal the X_ITE text and warnings exactly.
fn assert_matches_oracle(file: &str) {
    let o = &oracle()[file];
    assert!(o.ok, "{file}: oracle did not load");
    let want = o.vrml.as_deref().unwrap();
    let p = vrml97::parse_bytes(&input(file), file).unwrap_or_else(|e| panic!("{file}: published: {e}"));
    assert_eq!(p.to_vrml_string(), want, "{file}: published text");
    assert_eq!(p.warnings, o.warns, "{file}: published warnings");
    let e = x3d_parse::parse_bytes(&input(file), file).unwrap_or_else(|e| panic!("{file}: engine: {e}"));
    assert_eq!(e.to_vrml_string(), want, "{file}: engine text");
    assert_eq!(e.warnings, o.warns, "{file}: engine warnings");
}

/// Published and engine both fail with `message`; X_ITE fails with the same message text.
fn assert_fails_like_oracle(file: &str, message: &str, oracle_error: &str) {
    let o = &oracle()[file];
    assert!(!o.ok, "{file}: oracle loaded");
    assert!(o.errors.iter().any(|e| e == oracle_error), "{file}: oracle errors {:?}", o.errors);
    let p = vrml97::parse_bytes(&input(file), file).err().unwrap_or_else(|| panic!("{file}: published parsed"));
    assert_eq!(p.to_string(), message, "{file}: published error");
    let e = x3d_parse::parse_bytes(&input(file), file).err().unwrap_or_else(|| panic!("{file}: engine parsed"));
    assert_eq!(e.to_string(), message, "{file}: engine error");
}

// -------------------------------------------------------------------------
// Valid inputs: exact X_ITE text
// -------------------------------------------------------------------------

#[test]
fn parse_minimal_vrml97() {
    assert_matches_oracle("minimal.wrl");
}

#[test]
fn parse_empty_world() {
    assert_matches_oracle("empty.wrl");
}

#[test]
fn parse_group_with_children() {
    assert_matches_oracle("group-children.wrl");
}

#[test]
fn parse_def_and_use() {
    assert_matches_oracle("def-use.wrl");
}

#[test]
fn parse_route_declaration() {
    assert_matches_oracle("route-valid.wrl");
}

#[test]
fn parse_route_to_unknown_field_warns_and_drops_route() {
    // The R0.2-C fixture routed to `T.set`, which does not exist. X_ITE warns and drops
    // the ROUTE; it is not a valid-ROUTE test. Kept under its real name.
    assert_matches_oracle("route-bad-field.wrl");
}

#[test]
fn parse_proto() {
    assert_matches_oracle("proto.wrl");
}

#[test]
fn parse_externproto_with_url() {
    assert_matches_oracle("externproto-url.wrl");
}

#[test]
fn parse_bom_is_stripped() {
    assert_matches_oracle("bom.wrl");
}

#[test]
fn parse_unknown_node_type_warns_and_prints_null() {
    // X_ITE VRMLParser.node: an unknown type is a console warning, its body is parsed into
    // a throw-away WorldInfo, and the statement yields NULL. It is not a parse failure.
    assert_matches_oracle("unknown-node.wrl");
    let s = vrml97::parse_bytes(&input("unknown-node.wrl"), "unknown-node.wrl").unwrap();
    assert_eq!(s.ctx(vrml97::scene::ROOT).root_nodes, vec![None]);
}

// -------------------------------------------------------------------------
// Invalid inputs: X_ITE fails to load; Rust returns Err with the same message
// -------------------------------------------------------------------------

#[test]
fn parse_unterminated_bracket_fails() {
    assert_fails_like_oracle("unterminated-bracket.wrl", "Parser error at line 3: Expected ']'.", "Error: Expected ']'.");
}

#[test]
fn parse_externproto_with_url_in_interface_fails() {
    // R0.2-C called this "EXTERNPROTO with a single-string URL". The input put the URL
    // inside the interface brackets, which is malformed; X_ITE rejects it too.
    assert_fails_like_oracle(
        "externproto-malformed.wrl",
        "Parser error at line 2: Expected a ']' at the end of EXTERNPROTO interface declaration.",
        "Error: Expected a ']' at the end of EXTERNPROTO interface declaration.",
    );
}

// -------------------------------------------------------------------------
// Known difference 1: EXPORT and IMPORT
// -------------------------------------------------------------------------

#[test]
fn parse_export_is_hard_error_published() {
    for (file, line) in [("export.x3dv.wrl", 4), ("export-vrml2.wrl", 3)] {
        assert!(oracle()[file].ok, "X_ITE accepts EXPORT in {file}");
        let e = vrml97::parse_bytes(&input(file), file).err().expect("published accepts EXPORT: known difference 1 closed; retire this record");
        assert_eq!(e.to_string(), format!("Parser error at line {line}: EXPORT is not supported by the Rust port yet (phase X4)."));
    }
}

#[test]
fn parse_import_is_hard_error_published() {
    assert!(oracle()["import.wrl"].ok, "X_ITE accepts IMPORT");
    let e = vrml97::parse_bytes(&input("import.wrl"), "import.wrl").err().expect("published accepts IMPORT: known difference 1 closed; retire this record");
    assert_eq!(e.to_string(), "Parser error at line 4: IMPORT is not supported by the Rust port yet (phase X4).");
}

#[test]
fn engine_parse_export_matches_xite() {
    for file in ["export.x3dv.wrl", "export-vrml2.wrl"] {
        let e = x3d_parse::parse_bytes(&input(file), file).unwrap();
        assert_eq!(e.to_vrml_string(), oracle()[file].vrml.as_deref().unwrap(), "{file}");
    }
}

#[test]
fn engine_parse_import_matches_xite_without_inline_load() {
    // X_ITE loads the Inline and then prints `global TRUE` on it. The parser alone does
    // not load the Inline, so that one line is the only difference.
    let want = oracle()["import.wrl"].vrml.clone().unwrap();
    assert!(want.contains("  global TRUE\n"));
    let e = x3d_parse::parse_bytes(&input("import.wrl"), "import.wrl").unwrap();
    assert_eq!(e.to_vrml_string(), want.replace("  global TRUE\n", ""));
}

// -------------------------------------------------------------------------
// Known difference 9: OrthoViewpoint.fieldOfView
// -------------------------------------------------------------------------

#[test]
fn orthoviewpoint_fieldofview_published_prints_mffloat() {
    // R0.2-C asserted only that a plain Viewpoint printed "Viewpoint". This checks the
    // node the difference is about, in X3D 3.3 and 4.0 headers.
    for file in ["ortho-v33.wrl", "ortho-v40.wrl"] {
        let want = oracle()[file].vrml.clone().unwrap();
        assert!(want.ends_with("OrthoViewpoint {\n  fieldOfView -2 -1 2 1\n}\n"), "{file}: oracle");
        let p = vrml97::parse_bytes(&input(file), file).unwrap().to_vrml_string();
        assert_eq!(
            p,
            want.replace("fieldOfView -2 -1 2 1", "fieldOfView [\n    -2,\n    -1,\n    2,\n    1\n  ]"),
            "{file}: published"
        );
        assert_ne!(p, want, "known difference 9 closed for {file}; retire this record");
        let e = x3d_parse::parse_bytes(&input(file), file).unwrap().to_vrml_string();
        assert_eq!(e, want, "{file}: engine");
    }
}

// -------------------------------------------------------------------------
// Scene structure
// -------------------------------------------------------------------------

#[test]
fn scene_root_node_count_matches_input() {
    // Group { children [ Shape { } Shape { } ] }: one root node, a Group whose children
    // field holds exactly two Shape nodes, and no other nodes.
    assert_matches_oracle("root-count.wrl");
    let s = vrml97::parse_bytes(&input("root-count.wrl"), "root-count.wrl").unwrap();
    let roots = &s.ctx(vrml97::scene::ROOT).root_nodes;
    assert_eq!(roots.len(), 1);
    let g = roots[0].unwrap();
    assert_eq!(s.type_name(g), "Group");
    let loc = s.get_field(g, "children").unwrap();
    let children = match &s.field(g, loc).value {
        vrml97::fields::FieldValue::MFNode(v) => v.clone(),
        other => panic!("children is {other:?}"),
    };
    assert_eq!(children.len(), 2);
    for c in &children {
        assert_eq!(s.type_name(c.unwrap()), "Shape");
    }
    // Node storage order is a Rust detail, so compare the sorted type list.
    let mut types: Vec<String> = (0..s.nodes.len() as u32).map(|i| s.type_name(i)).collect();
    types.sort();
    assert_eq!(types, ["Group", "Shape", "Shape"]);
    assert!(s.warnings.is_empty());
}

#[test]
fn scene_to_vrml_string_header_is_x3d_4_1() {
    // X_ITE writes every scene as X3D 4.1 with its own version (ORACLE.json, all files).
    let s = vrml97::parse_bytes(b"#VRML V2.0 utf8\nGroup { }", "a.wrl").unwrap();
    assert_eq!(
        s.to_vrml_string(),
        "#X3D V4.1 utf8 X_ITE V16.5.0\n\nPROFILE Interchange\n\nCOMPONENT EnvironmentalSensor : 3\nCOMPONENT PointingDeviceSensor : 1\nCOMPONENT Scripting : 1\nCOMPONENT Sound : 3\nCOMPONENT Text : 2\n\nGroup { }\n"
    );
}

#[test]
fn scene_specification_version_is_recorded() {
    let s = vrml97::parse_bytes(b"#VRML V2.0 utf8\n", "a.wrl").unwrap();
    // VRMLParser.js: setSpecificationVersion from the header, setEncoding ("VRML").
    assert_eq!(s.specification_version, "2.0");
    assert_eq!(s.encoding, "VRML");
    let s = vrml97::parse_bytes(b"#X3D V3.3 utf8\nPROFILE Interchange\n", "a.x3dv").unwrap();
    assert_eq!(s.specification_version, "3.3");
    assert_eq!(s.profile.as_deref(), Some("Interchange"));
}

// -------------------------------------------------------------------------
// Gzip input
// -------------------------------------------------------------------------

#[test]
fn parse_gzip_input_decodes_to_the_plain_scene() {
    let gz = input("minimal-gzip.wrl");
    assert_eq!(&gz[..2], &[0x1f, 0x8b]);
    assert_eq!(vrml97::gunzip(&gz), input("minimal.wrl"));
    assert_eq!(x3d_parse::gunzip(&gz), input("minimal.wrl"));
    assert_matches_oracle("minimal-gzip.wrl");
    assert_eq!(oracle()["minimal-gzip.wrl"].vrml, oracle()["minimal.wrl"].vrml);
}

#[test]
fn parse_truncated_gzip_fails_like_xite() {
    // $.gunzip returns the raw bytes when decompression fails, and no parser accepts them.
    // X_ITE appends the URL to the message; the Rust error does not carry the URL.
    let raw = input("truncated-gzip.wrl");
    assert_eq!(vrml97::gunzip(&raw), raw);
    assert_eq!(x3d_parse::gunzip(&raw), raw);
    let o = &oracle()["truncated-gzip.wrl"];
    assert!(!o.ok);
    assert!(o.errors[0].contains("Couldn't parse X3D. No suitable file handler found for '"));
    for msg in [
        vrml97::parse_bytes(&raw, "t.wrl").err().unwrap().to_string(),
        x3d_parse::parse_bytes(&raw, "t.wrl").err().unwrap().to_string(),
    ] {
        assert_eq!(msg, "Couldn't parse X3D. No suitable file handler found.");
    }
}

#[test]
fn gunzip_passthrough_on_non_gzip() {
    assert_eq!(vrml97::gunzip(b"hello"), b"hello");
    assert_eq!(x3d_parse::gunzip(b"hello"), b"hello");
}

// -------------------------------------------------------------------------
// Text decoding: new TextDecoder () .decode (bytes) — fixtures/js-oracle.json
// (Chromium 151 and node 24 agree on every case)
// -------------------------------------------------------------------------

const DECODE: &[(&[u8], &str)] = &[
    (&[0xff, 0xfe, 0xfd], "\u{fffd}\u{fffd}\u{fffd}"),
    (&[0x61, 0xe2, 0x82, 0x63], "a\u{fffd}c"),
    (&[0xef, 0xbb, 0xbf, 0x68, 0x69], "hi"),
    (&[0xef, 0xbb, 0xbf, 0xef, 0xbb, 0xbf, 0x68], "\u{feff}h"),
    (&[0xc0, 0x80], "\u{fffd}\u{fffd}"),
];

#[test]
fn decode_text_replaces_invalid_utf8() {
    for (bytes, want) in DECODE {
        assert_eq!(vrml97::decode_text(bytes), *want, "published {bytes:x?}");
        assert_eq!(x3d_parse::decode_text(bytes), *want, "engine {bytes:x?}");
    }
}

#[test]
fn decode_text_in_a_scene_matches_xite() {
    assert_matches_oracle("invalid-utf8.wrl");
}

#[test]
fn decode_text_strips_bom() {
    assert_eq!(vrml97::decode_text(b"\xef\xbb\xbfhello"), "hello");
    assert_eq!(x3d_parse::decode_text(b"\xef\xbb\xbfhello"), "hello");
}
