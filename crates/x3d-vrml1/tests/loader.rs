//! Behavior of the loader that the package-output comparison does not state directly.

use x3d_vrml1::{is_vrml1, parse_bytes, parse_str};

fn vrml(text: &str) -> String {
    let mut s = parse_str(text, "file:///t.wrl").unwrap();

    s.clone_counts = Some(s.parse_clone_counts());
    s.to_vrml_string()
}

#[test]
fn header_detection() {
    assert!(is_vrml1("#VRML V1.0 ascii\n"));
    assert!(is_vrml1("#VRML V1.0 ascii"));
    assert!(!is_vrml1("#VRML V2.0 utf8\n"));
    assert!(!is_vrml1(" #VRML V1.0 ascii\n"));
    assert!(!is_vrml1("#X3D V3.3 utf8\n"));
}

#[test]
fn bytes_drop_a_byte_order_mark() {
    let scene = parse_bytes(b"\xef\xbb\xbf#VRML V1.0 ascii\nCube { }\n", "file:///t.wrl").unwrap();

    assert_eq!(scene.ctx(x3d_scene::ROOT).root_nodes.len(), 1);
}

#[test]
fn separator_restores_state_at_its_end() {
    let out = vrml(
        "#VRML V1.0 ascii\nSeparator { Material { diffuseColor 1 0 0 } Cube { } }\nSphere { }\n",
    );

    // The cube is red, the sphere after the Separator has the default material again.
    assert_eq!(out.matches("diffuseColor 1 0 0").count(), 1, "{out}");
    assert!(out.contains("material Material { }"), "{out}");
}

#[test]
fn group_does_not_restore_state() {
    let out = vrml("#VRML V1.0 ascii\nGroup { Material { diffuseColor 0 1 0 } }\nSphere { }\n");

    assert!(out.contains("diffuseColor 0 1 0"), "{out}");
}

#[test]
fn state_reaches_later_siblings_not_earlier_ones() {
    let out = vrml(
        "#VRML V1.0 ascii\nSeparator { Cube { } Material { diffuseColor 0 0 1 } Sphere { } }\n",
    );
    let cube = out.find("Box").unwrap();
    let blue = out.find("diffuseColor 0 0 1").unwrap();
    let sphere = out.find("Sphere").unwrap();

    // The default Material (printed first, shared) belongs to the cube; the blue one to the sphere.
    assert!(cube < blue && blue < sphere, "{out}");
}

#[test]
fn transforms_fold_into_the_enclosing_transform_node() {
    // The extension keeps one X3D Transform per Separator: a Translation moves all children.
    let out = vrml("#VRML V1.0 ascii\nSeparator { Cube { } Translation { translation 1 2 3 } }\n");

    assert!(out.contains("translation 1 2 3"), "{out}");
    // Outside any Separator a Translation has nothing to fold into.
    let out = vrml("#VRML V1.0 ascii\nTranslation { translation 1 2 3 }\nCube { }\n");

    assert!(!out.contains("translation"), "{out}");
}

#[test]
fn primitive_defaults_print_nothing() {
    let out = vrml("#VRML V1.0 ascii\nCube { }\nSphere { }\nCone { }\nCylinder { }\n");

    for t in ["Box { }", "Sphere { }", "Cone { }", "Cylinder { }"] {
        assert!(out.contains(t), "{t}: {out}");
    }
}

#[test]
fn def_use_of_a_material_shares_the_node() {
    let out = vrml(
        "#VRML V1.0 ascii\nDEF M Material { diffuseColor 1 1 0 }\nCube { }\nSeparator { USE M Sphere { } }\n",
    );

    assert_eq!(out.matches("diffuseColor 1 1 0").count(), 1, "{out}");
    assert!(out.contains("USE M"), "{out}");
}

#[test]
fn unknown_node_types_and_nodes_without_conversion_are_dropped() {
    let out = vrml(
        "#VRML V1.0 ascii\nFoo { fields [ SFLong n ] n 3 Cube { } }\nAsciiText { string \"x\" }\nCube { }\n",
    );

    assert_eq!(out.matches("Box").count(), 1, "{out}");
}

#[test]
fn error_positions_name_line_and_column() {
    let cases: &[(&str, &str, usize, usize)] = &[
        (
            "#VRML V1.0 ascii\nSeparator {\n  Cube { foo 1 }\n}\n",
            "Unknown field 'foo' in class 'Cube'.",
            3,
            10,
        ),
        (
            "#VRML V1.0 ascii\nCube { }\n}\nSphere { }\n",
            "Unknown statement.",
            3,
            1,
        ),
        (
            "#VRML V1.0 ascii\nSeparator {\n  Cube { width 1\n}\n",
            "Expected '}' at the end of node body.",
            5,
            1,
        ),
        (
            "#VRML V1.0 ascii\nCube { width }\n",
            "Couldn't read value for field 'width' of node Cube.",
            2,
            14,
        ),
        (
            "#VRML V1.0 ascii\nDEF { }\n",
            "No name given after DEF.",
            2,
            4,
        ),
        ("#VRML V1.0 ascii\nUSE\n", "No name given after USE.", 2, 4),
        (
            "#VRML V1.0 ascii\nCoordinate3 { point [ 1 2 3 }\n",
            "Expected ']'.",
            2,
            29,
        ),
    ];

    for (input, message, line, column) in cases {
        let e = parse_str(input, "file:///t.wrl")
            .err()
            .unwrap_or_else(|| panic!("{input:?} parsed"));

        assert_eq!(
            (e.message.as_str(), e.line, e.column),
            (*message, *line, *column),
            "{input:?}"
        );
    }
}

#[test]
fn error_display_has_line_and_column() {
    let e = parse_str("#VRML V1.0 ascii\nCube { foo 1 }\n", "file:///t.wrl")
        .err()
        .unwrap();

    assert_eq!(
        e.to_string(),
        "VRML 1.0 parser error at line 2, column 8: Unknown field 'foo' in class 'Cube'."
    );
}

#[test]
fn crlf_and_columns_count_characters() {
    let e = parse_str(
        "#VRML V1.0 ascii\r\nInfo { string \"\u{e9}\u{e9}\" foo 1 }\r\n",
        "file:///t.wrl",
    )
    .err()
    .unwrap();

    assert_eq!((e.line, e.column), (2, 20));
}
