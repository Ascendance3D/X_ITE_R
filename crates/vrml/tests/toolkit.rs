const WORLD: &[u8] = b"#VRML V2.0 utf8
PROTO Spin [ field SFFloat speed 1 ] { Group { } }
DEF T Transform { children [ Shape { geometry Box { } } Spin { } ] }
DEF TS TimeSensor { loop TRUE }
DEF OI OrientationInterpolator { key [ 0 1 ] keyValue [ 0 1 0 0, 0 1 0 3.14 ] }
ROUTE TS.fraction_changed TO OI.set_fraction
ROUTE OI.value_changed TO T.set_rotation
";

#[test]
fn detects_versions() {
    assert_eq!(vrml::detect(WORLD), vrml::Version::Vrml2);
    assert_eq!(vrml::detect(b"#VRML V1.0 ascii\n"), vrml::Version::Vrml1);
    assert_eq!(
        vrml::detect(b"#X3D V4.0 utf8\n"),
        vrml::Version::X3d("4.0".into())
    );
    assert_eq!(vrml::detect(b"Shape {}"), vrml::Version::Unknown);
}

#[test]
fn summarizes_a_scene() {
    let scene = vrml::parse_bytes(WORLD, "memory:").unwrap();
    let s = vrml::summarize(&scene);
    assert_eq!(
        (s.root_nodes, s.protos, s.externprotos, s.routes),
        (3, 1, 0, 2)
    );
    assert_eq!(s.node_types.get("Box"), Some(&1));
    assert_eq!(s.node_types.get("TimeSensor"), Some(&1));
}

/// The pinned X_ITE has no VRML 1.0 parser, and `x3d-parse` still does not dispatch a
/// `#VRML V1.0` header to its VRML97 parser (`is_valid_vrml`). Since the `vrml1` feature of
/// `x3d-parse` is on by default, the header goes to the separate `x3d-vrml1` loader instead
/// (a port of the x_ite-vrml1-parser extension, not a pin parity item).
#[test]
fn reads_vrml1_through_the_vrml1_loader() {
    assert!(!vrml::vrml97::is_valid_vrml("#VRML V1.0 ascii\n"));

    let scene =
        vrml::parse_bytes(b"#VRML V1.0 ascii\nSeparator { Cube { } }\n", "memory:").unwrap();

    assert_eq!(vrml::summarize(&scene).node_types.get("Box"), Some(&1));
    assert!(vrml::parse_bytes(b"#VRML V1.0 ascii\nSeparator { Foo 1 }\n", "memory:").is_err());
}
