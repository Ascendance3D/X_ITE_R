//! Format dispatch of a `#VRML V1.0 ascii` header (the x_ite-vrml1-parser port; not a pin item).

#[cfg(feature = "vrml1")]
#[test]
fn v1_header_selects_the_vrml1_loader() {
    let scene = x3d_parse::parse_bytes(
        b"#VRML V1.0 ascii\nSeparator { Cube { } }\n",
        "file:///a.wrl",
    )
    .unwrap();

    assert_eq!(scene.encoding, "VRML");
    assert_eq!(scene.profile.as_deref(), Some("Interchange"));
    assert!(scene.to_vrml_string().contains("geometry Box"));
}

#[cfg(feature = "vrml1")]
#[test]
fn v1_error_carries_the_line() {
    let e = x3d_parse::parse_bytes(b"#VRML V1.0 ascii\n\nCube { foo 1 }\n", "file:///a.wrl")
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

#[cfg(not(feature = "vrml1"))]
#[test]
fn without_the_feature_v1_has_no_handler() {
    let e = x3d_parse::parse_bytes(b"#VRML V1.0 ascii\nCube { }\n", "file:///a.wrl")
        .err()
        .unwrap();

    assert!(matches!(e, x3d_parse::LoadError::NoHandler));
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
