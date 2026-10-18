//! Suite 7 — Future type-authority gate (R0.2-D, Option B).
//!
//! Each test states one compatibility requirement that Option B must meet. The file is
//! compiled only with `--features future_gate`. Today it does not compile, because the
//! published `vrml97` is a separate copy; the compile errors are the gate's "before"
//! result, recorded by `tests/future_gate_contract.rs`.
//!
//! No test carries `#[ignore]`. A test that is not run is not a pass. The one command that
//! runs the whole gate is `crates/r0-2-c-tests/future-gate.sh`; it fails unless every
//! listed gate test runs and passes and none is ignored.
//!
//! Expected values come from the pinned X_ITE oracle (`fixtures/xite/ORACLE.json`), the
//! recorded Chrome vectors and the X_ITE Matrix4 oracle (see `fixtures/PROVENANCE.md`).
#![cfg(feature = "future_gate")]

use std::any::TypeId;

fn input(file: &str) -> Vec<u8> {
    r0_2_c_tests::fixture(&format!("xite/{file}"))
}

fn oracle_vrml(file: &str) -> String {
    let json: serde_json::Value = serde_json::from_slice(&r0_2_c_tests::fixture("xite/ORACLE.json")).unwrap();
    json["results"].as_array().unwrap().iter().find(|r| r["file"] == file).unwrap()["vrml"].as_str().unwrap().to_string()
}

/// 1. One type authority: the published Scene is the engine Scene.
#[test]
fn gate_one_type_authority() {
    assert_eq!(TypeId::of::<vrml97::Scene>(), TypeId::of::<x3d_scene::Scene>());
    let s: x3d_scene::Scene = vrml97::parse_bytes(&input("minimal.wrl"), "minimal.wrl").unwrap();
    assert_eq!(s.to_vrml_string(), oracle_vrml("minimal.wrl"));
}

/// 2. Every released module path and root name still resolves.
#[test]
fn gate_all_public_paths_accessible() {
    #[allow(unused_imports)]
    use vrml97::{
        LoadError, ParseError, Parser, Scene, decode_text, gunzip, is_valid_vrml, load_file, parse_bytes,
        fields as _, fields::fdlibm as _, fields::format as _, fields::generator as _, fields::jsmath as _,
        fields::math as _, fields::math::color as _, fields::math::matrix as _, fields::math::quaternion as _,
        fields::math::rotation as _, fields::math::vector as _, fields::types as _, fields::types::access as _,
        fields::value as _, parse as _, parse::color as _, parse::lexer as _, parse::parser as _, scene as _,
        scene::info_array as _, scene::nodes as _, scene::scene as _, scene::setup as _, scene::vrml as _,
    };
    let _: fn(&[u8]) -> String = decode_text;
    let _: fn(&[u8]) -> Vec<u8> = gunzip;
    let _: fn(&str) -> bool = is_valid_vrml;
    let _: fn(&std::path::Path) -> Result<Scene, LoadError> = load_file;
    let _: for<'a, 'b> fn(&'a [u8], &'b str) -> Result<Scene, LoadError> = parse_bytes;
    assert_eq!(TypeId::of::<vrml::Scene>(), TypeId::of::<Scene>());
    assert_eq!(TypeId::of::<x3d::Scene>(), TypeId::of::<Scene>());
}

/// 3. Re-exported names are the engine types, not copies.
#[test]
fn gate_reexports_are_internal_types() {
    assert_eq!(TypeId::of::<vrml97::ParseError>(), TypeId::of::<x3d_parse::ParseError>());
    assert_eq!(TypeId::of::<vrml97::LoadError>(), TypeId::of::<x3d_parse::LoadError>());
    assert_eq!(TypeId::of::<vrml97::Parser>(), TypeId::of::<x3d_parse::Parser>());
    assert_eq!(TypeId::of::<vrml97::fields::FieldValue>(), TypeId::of::<x3d_fields::value::FieldValue>());
    assert_eq!(TypeId::of::<vrml97::fields::FieldType>(), TypeId::of::<x3d_fields::types::FieldType>());
    assert_eq!(TypeId::of::<vrml97::fields::math::Matrix4>(), TypeId::of::<x3d_fields::math::matrix::Matrix4>());
    assert_eq!(TypeId::of::<vrml97::fields::math::Rotation4>(), TypeId::of::<x3d_fields::math::rotation::Rotation4>());
    assert_eq!(TypeId::of::<vrml97::fields::math::Vector3>(), TypeId::of::<x3d_fields::math::vector::Vector3>());
    assert_eq!(TypeId::of::<vrml97::scene::scene::NodeKind>(), TypeId::of::<x3d_scene::scene::NodeKind>());
    assert_eq!(TypeId::of::<vrml97::scene::scene::Route>(), TypeId::of::<x3d_scene::scene::Route>());
    assert_eq!(TypeId::of::<vrml97::scene::scene::ExecutionContext>(), TypeId::of::<x3d_scene::scene::ExecutionContext>());
    assert_eq!(TypeId::of::<vrml97::scene::nodes::NodeTable>(), TypeId::of::<x3d_scene::nodes::NodeTable>());
}

/// 4. Matrix4.factor / getTransform through the published path give the X_ITE result
///    (node v24 value; this binary keeps the default Node math flavor).
#[test]
fn gate_matrix_factor_get_transform_present() {
    use vrml97::fields::math::{Matrix4, Rotation4, Vector3};
    let mut a = Matrix4::IDENTITY;
    a.set_transform(
        Some(&Vector3::new(1.0, 2.0, 3.0)),
        Some(&Rotation4::new(0.0, 1.0, 0.0, 0.5)),
        Some(&Vector3::new(2.0, 3.0, 4.0)),
        None,
        None,
    );
    for via_get_transform in [false, true] {
        let (mut t, mut r, mut s, mut so) = (Vector3::ZERO, Rotation4::identity(), Vector3::ZERO, Rotation4::identity());
        if via_get_transform {
            a.get_transform(Some(&mut t), Some(&mut r), Some(&mut s), Some(&mut so), None);
        } else {
            a.factor(Some(&mut t), Some(&mut r), Some(&mut s), Some(&mut so));
        }
        assert_eq!([t.x, t.y, t.z], [1.0, 2.0, 3.0]);
        assert_eq!([s.x, s.y, s.z], [2.0, 3.0, 4.0]);
        assert_eq!([r.x().to_bits(), r.y().to_bits(), r.z().to_bits(), r.angle().to_bits()], [0, 0x3ff000000000000d, 0, 0x3fdfffffffffffe7]);
    }
}

/// 5. NodeKind has the Imported variant; a five-arm match is exhaustive.
#[test]
fn gate_node_kind_has_imported_variant() {
    use vrml97::scene::scene::NodeKind::{self, *};
    let k: NodeKind = Imported(7);
    let n = match k {
        Builtin(_) | Instance(_) | Placeholder | Internal(_) => None,
        Imported(i) => Some(i),
    };
    assert_eq!(n, Some(7));
}

/// 6. Route has id, conn, raw_source_field, raw_destination_field.
#[test]
fn gate_route_has_extra_fields() {
    use vrml97::scene::{FieldLoc, scene::Route};
    let r = Route {
        source_node: 1,
        source_field: "fraction_changed".into(),
        destination_node: 2,
        destination_field: "set_fraction".into(),
        source_loc: FieldLoc::Pre(0),
        destination_loc: FieldLoc::Pre(0),
        id: 42,
        conn: None,
        raw_source_field: "fraction_changed".into(),
        raw_destination_field: "set_fraction".into(),
    };
    assert_eq!((r.id, r.raw_source_field.as_str(), r.raw_destination_field.as_str()), (42, "fraction_changed", "set_fraction"));
}

/// 7. Scene has scene_infos, imported, exported, route_counter, filled by the parser.
#[test]
fn gate_scene_has_extra_fields() {
    let s = vrml97::parse_bytes(&input("export.x3dv.wrl"), "export.x3dv.wrl").unwrap();
    assert_eq!(s.exported.len(), 1);
    assert_eq!(s.imported.len(), 0);
    let _ = (&s.scene_infos, s.route_counter);
    let s = vrml97::parse_bytes(&input("import.wrl"), "import.wrl").unwrap();
    assert_eq!(s.imported.len(), 1);
}

/// 8. ExecutionContext has imported and exported.
#[test]
fn gate_execution_context_has_imported_and_exported() {
    use vrml97::scene::{Outer, scene::ExecutionContext};
    let c = ExecutionContext {
        parent: None,
        outer: Outer::Scene,
        root_nodes: Vec::new(),
        named_nodes: Default::default(),
        protos: Default::default(),
        externprotos: Default::default(),
        routes: Vec::new(),
        imported: Default::default(),
        exported: Default::default(),
    };
    assert!(c.root_nodes.is_empty());
}

/// 9. EXPORT and IMPORT parse through the published path and print like X_ITE.
#[test]
fn gate_export_import_parses() {
    for file in ["export.x3dv.wrl", "export-vrml2.wrl"] {
        let s = vrml97::parse_bytes(&input(file), file).unwrap();
        assert_eq!(s.to_vrml_string(), oracle_vrml(file), "{file}");
    }
    // `global TRUE`: Inline's constructor sets it for X3D <= 3.3 (node table "legacy"); the
    // parse-only output now equals X_ITE's exactly (an earlier note blamed the Inline load).
    let s = vrml97::parse_bytes(&input("import.wrl"), "import.wrl").unwrap();
    assert_eq!(s.to_vrml_string(), oracle_vrml("import.wrl"));
}

/// 10. OrthoViewpoint.fieldOfView prints as X_ITE prints it (SFVec4f form).
#[test]
fn gate_orthoviewpoint_serializes_to_sfvec4f() {
    for file in ["ortho-v33.wrl", "ortho-v40.wrl"] {
        let s = vrml97::parse_bytes(&input(file), file).unwrap();
        assert_eq!(s.to_vrml_string(), oracle_vrml(file), "{file}");
    }
}

/// 11. (Added in C1, known difference 2.) Published number formatting equals Chrome.
#[test]
fn gate_float_format_matches_chrome() {
    // crates/x3d-fields/tests/chrome_format.rs (Chrome 155) and fixtures/js-oracle.json.
    let cases: &[(u64, usize, &str)] = &[
        (0x3fefffffffffffff, 16, "1"),
        (0x3feffffffffffffc, 15, "1"),
        (0x3feffffffffffffb, 15, "1"),
        (0x3feffffffffffffb, 16, "0.999999999999999"),
        (0x3feffffffffffffa, 16, "0.9999999999999993"),
        (0x3fc10cd84c924c23, 15, "0.133204495800414"),
    ];
    for (b, p, want) in cases {
        assert_eq!(vrml97::fields::format::intl_standard(f64::from_bits(*b), *p), *want);
    }
    assert_eq!(vrml97::fields::format::double_format(0.9999999999999994), "1");
}
