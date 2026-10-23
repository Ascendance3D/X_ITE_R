#[test]
fn table_is_complete() {
    // 263 + the 21 Texturing3D / VolumeRendering node types of X_ITE's external components
    // + the 6 Annotation node types X_ITE 16.5.0 ships but does not register (REA batch 04).
    assert_eq!(x3d::nodes().count(), 290);
    assert!(x3d::components().any(|(c, _)| c == "Geometry3D"));
    assert!(x3d::profiles().any(|(p, _)| p == "Full"));
}

#[test]
fn known_fields() {
    let ifs = x3d::node("IndexedFaceSet").unwrap();
    let crease = ifs.field("creaseAngle").unwrap();
    assert_eq!(
        (
            crease.access(),
            crease.type_name(),
            crease.default_vrml().as_str()
        ),
        ("initializeOnly", "SFFloat", "0")
    );
    let solid = ifs.field("solid").unwrap();
    assert_eq!(solid.default_vrml(), "TRUE");
    let set = ifs.field("set_coordIndex").unwrap();
    assert_eq!(set.access(), "inputOnly");

    let vp = x3d::node("Viewpoint").unwrap();
    assert_eq!(vp.field("position").unwrap().default_vrml(), "0 0 10");
    assert_eq!(vp.field("orientation").unwrap().default_vrml(), "0 0 1 0");
    assert_eq!(
        x3d::node("Shape")
            .unwrap()
            .field("geometry")
            .unwrap()
            .default_vrml(),
        "NULL"
    );
    assert_eq!(
        x3d::node("Group")
            .unwrap()
            .field("children")
            .unwrap()
            .default_vrml(),
        "[ ]"
    );
    assert_eq!(x3d::node("Box").unwrap().container_field(), "geometry");
    assert!(x3d::node("NoSuchNode").is_none());
}

#[test]
fn reads_classic_encoding() {
    let s = x3d::parse_bytes(
        b"#X3D V4.0 utf8\nPROFILE Interchange\nTransform { translation 1 2 3 }\n",
        "memory:",
    )
    .unwrap();
    assert!(s.to_vrml_string().contains("translation 1 2 3"));
}
