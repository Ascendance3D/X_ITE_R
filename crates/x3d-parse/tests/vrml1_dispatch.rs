//! Format dispatch of a `#VRML V1.0 ascii` header (the x_ite-vrml1-parser port; not a pin item).
//! Rejected by default, loaded only with `ParseOptions { vrml1: true }`.

use x3d_parse::ParseOptions;

const ON: ParseOptions = ParseOptions { vrml1: true };

#[test]
fn v1_is_rejected_by_default() {
    for input in [
        &b"#VRML V1.0 ascii\nSeparator { }\n"[..],
        b"#VRML V1.0 ascii\nCube { }\n",
    ] {
        let e = x3d_parse::parse_bytes(input, "file:///a.wrl")
            .err()
            .unwrap();

        assert!(matches!(e, x3d_parse::LoadError::NoHandler));
    }

    let e = x3d_parse::parse_string("#VRML V1.0 ascii\nSeparator { }\n", "file:///a.wrl")
        .err()
        .unwrap();

    assert!(matches!(e, x3d_parse::LoadError::NoHandler));
}

#[test]
fn v1_string_loads_when_enabled() {
    let scene = x3d_parse::parse_string_with(
        "#VRML V1.0 ascii\nSeparator { Cube { } }\n",
        "file:///a.wrl",
        &ON,
    )
    .unwrap();

    assert!(scene.to_vrml_string().contains("geometry Box"));
}

#[test]
fn v1_header_selects_the_vrml1_loader() {
    let scene = x3d_parse::parse_bytes_with(
        b"#VRML V1.0 ascii\nSeparator { Cube { } }\n",
        "file:///a.wrl",
        &ON,
    )
    .unwrap();

    assert_eq!(scene.encoding, "VRML");
    assert_eq!(scene.profile.as_deref(), Some("Interchange"));
    assert!(scene.to_vrml_string().contains("geometry Box"));
}

#[test]
fn v1_error_carries_the_line() {
    let e = x3d_parse::parse_bytes_with(
        b"#VRML V1.0 ascii\n\nCube { foo 1 }\n",
        "file:///a.wrl",
        &ON,
    )
    .err()
    .unwrap();

    match e {
        x3d_parse::LoadError::Parse(p) => {
            assert_eq!(p.line, 3);
            assert!(p.message.contains("Unknown field 'foo'"), "{}", p.message);
        }
        other => panic!("{other}"),
    }
}

#[test]
fn vrml2_is_unchanged() {
    assert!(!x3d_parse::is_valid_vrml("#VRML V1.0 ascii\n"));
    assert!(x3d_parse::is_valid_vrml("#VRML V2.0 utf8\n"));

    let scene = x3d_parse::parse_bytes(
        b"#VRML V2.0 utf8\nShape { geometry Box { } }\n",
        "file:///a.wrl",
    )
    .unwrap();

    assert_eq!(scene.specification_version, "2.0");
}
