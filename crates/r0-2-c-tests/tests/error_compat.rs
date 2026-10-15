//! Suite 6 — Public error compatibility.
//!
//! Every test exercises the error path it names and checks the exact variant and text.
//! A test that also passes on `Ok` proves nothing, so none is used.
//!
//! Parser message text: pinned X_ITE VRMLParser.js (see ORACLE.json,
//! unterminated-bracket.wrl: `Error: Expected ']'.`). The `Parser error at line N: `
//! prefix is X3DParser's format, which X_ITE prints for warnings (ORACLE.json,
//! unknown-node.wrl).

const UNTERMINATED: &[u8] = b"#VRML V2.0 utf8\nGroup { children [";
const MISSING: &str = "/nonexistent/r02c1/path/that/does/not/exist.wrl";

#[test]
fn parse_failure_is_load_error_parse_variant() {
    let e = vrml97::parse_bytes(UNTERMINATED, "test.wrl").err().expect("must fail");
    match &e {
        vrml97::LoadError::Parse(p) => {
            assert_eq!(p.message, "Expected ']'.");
            assert_eq!(p.line, 2);
        }
        other => panic!("expected LoadError::Parse, got {other:?}"),
    }
    assert_eq!(e.to_string(), "Parser error at line 2: Expected ']'.");
}

#[test]
fn parse_error_display_and_debug() {
    let e = vrml97::parse_bytes(UNTERMINATED, "test.wrl").err().unwrap();
    let vrml97::LoadError::Parse(p) = e else { panic!("not a parse error") };
    assert_eq!(format!("{p}"), "Parser error at line 2: Expected ']'.");
    assert_eq!(format!("{p:?}"), "ParseError { message: \"Expected ']'.\", line: 2 }");
}

#[test]
fn parse_error_clone_is_equal() {
    let e = vrml97::parse_bytes(UNTERMINATED, "test.wrl").err().unwrap();
    let vrml97::LoadError::Parse(p) = e else { panic!("not a parse error") };
    let q = p.clone();
    assert_eq!((q.message.as_str(), q.line), (p.message.as_str(), p.line));
}

#[test]
fn load_file_missing_is_io_not_found() {
    let e = vrml97::load_file(std::path::Path::new(MISSING)).err().expect("must fail");
    match &e {
        vrml97::LoadError::Io(io) => assert_eq!(io.kind(), std::io::ErrorKind::NotFound),
        other => panic!("expected LoadError::Io, got {other:?}"),
    }
    assert!(format!("{e:?}").starts_with("Io("));
}

#[test]
fn no_handler_error_text() {
    let e = vrml97::parse_bytes(&[0x1f, 0x8b, 0x00, 0x00], "t.wrl").err().unwrap();
    assert!(matches!(e, vrml97::LoadError::NoHandler));
    assert_eq!(e.to_string(), "Couldn't parse X3D. No suitable file handler found.");
}

#[test]
fn engine_errors_have_the_same_variants_and_text() {
    let e = x3d_parse::parse_bytes(UNTERMINATED, "test.wrl").err().unwrap();
    let x3d_parse::LoadError::Parse(p) = &e else { panic!("not a parse error") };
    assert_eq!((p.message.as_str(), p.line), ("Expected ']'.", 2));
    assert_eq!(e.to_string(), "Parser error at line 2: Expected ']'.");
    let e = x3d_parse::load_file(std::path::Path::new(MISSING)).err().unwrap();
    assert!(matches!(e, x3d_parse::LoadError::Io(ref io) if io.kind() == std::io::ErrorKind::NotFound));
}

#[test]
fn parse_and_load_errors_are_distinct_types() {
    use std::any::TypeId;
    assert_ne!(TypeId::of::<vrml97::ParseError>(), TypeId::of::<vrml97::LoadError>());
    // LoadError wraps ParseError; there is no From conversion in either direction.
}

#[test]
fn parse_bytes_returns_result_scene_load_error() {
    // R0.2-A section 4.7 implied parse_bytes returns ParseError. The released signature
    // returns LoadError (recorded as R0.2-C item 11).
    let _: for<'a, 'b> fn(&'a [u8], &'b str) -> Result<vrml97::Scene, vrml97::LoadError> = vrml97::parse_bytes;
    let _: for<'a, 'b> fn(&'a [u8], &'b str) -> Result<x3d_scene::Scene, x3d_parse::LoadError> = x3d_parse::parse_bytes;
}

#[test]
fn load_file_returns_result_scene_load_error() {
    let _: fn(&std::path::Path) -> Result<vrml97::Scene, vrml97::LoadError> = vrml97::load_file;
    let _: fn(&std::path::Path) -> Result<x3d_scene::Scene, x3d_parse::LoadError> = x3d_parse::load_file;
}

#[test]
fn errors_do_not_implement_std_error() {
    // Recorded fact of the 0.1.0 contract: neither crate implements std::error::Error, so
    // `?` into Box<dyn Error> does not work for these types on either side.
    let tmp = env!("CARGO_TARGET_TMPDIR");
    for ty in ["vrml97::ParseError", "vrml97::LoadError", "x3d_parse::ParseError", "x3d_parse::LoadError"] {
        r0_2_c_tests::assert_probe_fails(
            tmp,
            &format!("no_std_error_{}", ty.replace("::", "_")),
            &format!("fn e<T: std::error::Error>() {{}} e::<{ty}>();"),
            &["E0277"],
        );
    }
}
