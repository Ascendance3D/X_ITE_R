//! Concrete node classes registered after loading a `#VRML V2.0` scene must equal what X_ITE
//! reports (oracle/x_ite-nodes.mjs → `vrml2Available` = `browser .getConcreteNodes ()`).
//! This is component loading only; `createNode` additionally filters by specification version.

use x3d_parse::parse_bytes;
use x3d_scene::{ROOT, nodes::table};

#[test]
fn vrml2_registered_node_classes_match_x_ite() {
    let t = table();
    let scene = parse_bytes(b"#VRML V2.0 utf8\n", "file:///synthetic.wrl").unwrap();

    let mut available: Vec<String> = t.types.iter().filter(|ty| scene.component_loaded(&ty.component)).map(|ty| ty.name.clone()).collect();
    let mut expected = t.vrml2_available.clone();

    available.sort();
    expected.sort();

    assert_eq!(expected.len(), 146);
    assert_eq!(available, expected);
}

#[test]
fn vrml2_create_node_filters_by_specification_version() {
    let mut scene = parse_bytes(b"#VRML V2.0 utf8\n", "file:///synthetic.wrl").unwrap();

    // VRML97 node.
    assert!(scene.create_node(ROOT, "Transform").is_some());
    // Registered (Sound component is loaded) but X3D 4.0+ only, so not creatable from VRML 2.0.
    assert!(scene.create_node(ROOT, "AcousticProperties").is_none());
    // External component (NURBS) is not loaded for a VRML 2.0 scene.
    assert!(!scene.component_loaded("NURBS"));
    assert!(scene.create_node(ROOT, "NurbsPatchSurface").is_none());
}
