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
    assert_eq!(vrml::detect(b"#X3D V4.0 utf8\n"), vrml::Version::X3d("4.0".into()));
    assert_eq!(vrml::detect(b"Shape {}"), vrml::Version::Unknown);
}

#[test]
fn summarizes_a_scene() {
    let scene = vrml::parse_bytes(WORLD, "memory:").unwrap();
    let s = vrml::summarize(&scene);
    assert_eq!((s.root_nodes, s.protos, s.externprotos, s.routes), (3, 1, 0, 2));
    assert_eq!(s.node_types.get("Box"), Some(&1));
    assert_eq!(s.node_types.get("TimeSensor"), Some(&1));
}

#[test]
fn rejects_vrml1() {
    assert!(vrml::parse_bytes(b"#VRML V1.0 ascii\nSeparator { }\n", "memory:").is_err());
}
