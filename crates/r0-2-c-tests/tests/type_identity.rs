//! Suite 2 — Type identity, current state (current-difference records).
//!
//! These tests record how the published `vrml97` types differ from the engine
//! (`x3d-fields`, `x3d-scene`, `x3d-parse`) types today. An absent item is proven with a
//! compile probe: a throw-away crate that names the item must fail with the recorded
//! rustc error code. A comment that "would not compile" proves nothing, so none is used.
//!
//! After R0.2-D (Option B) the records marked "difference" are expected to fail with a
//! message that names the closed difference. R0.2-D retires those records in the same
//! change and the future gate (`tests/future_gate.rs`) takes over.
//!
//! Field and variant facts: `crates/vrml97/src/scene/scene.rs` and
//! `crates/x3d-scene/src/scene.rs` at R0.2-C `35ce9ec`.

use r0_2_c_tests::{assert_probe_compiles, assert_probe_fails};
use std::any::TypeId;

const TMP: &str = env!("CARGO_TARGET_TMPDIR");

// -------------------------------------------------------------------------
// Distinct types today (difference)
// -------------------------------------------------------------------------

#[test]
fn scene_published_and_engine_are_distinct_types() {
    assert_ne!(TypeId::of::<vrml97::Scene>(), TypeId::of::<x3d_scene::Scene>());
}

#[test]
fn shared_shape_types_are_still_distinct_types() {
    // FieldValue, Matrix4, ParseError and LoadError have the same shape on both sides
    // (R0.2-B section 4.4) but are separate Rust types until Option B.
    assert_ne!(TypeId::of::<vrml97::fields::FieldValue>(), TypeId::of::<x3d_fields::value::FieldValue>());
    assert_ne!(TypeId::of::<vrml97::fields::math::Matrix4>(), TypeId::of::<x3d_fields::math::matrix::Matrix4>());
    assert_ne!(TypeId::of::<vrml97::ParseError>(), TypeId::of::<x3d_parse::ParseError>());
    assert_ne!(TypeId::of::<vrml97::LoadError>(), TypeId::of::<x3d_parse::LoadError>());
}

#[test]
fn published_scene_is_rejected_where_engine_scene_is_expected() {
    assert_probe_fails(
        TMP,
        "scene_not_interchangeable",
        r##"fn take(_: x3d_scene::Scene) {}
           let s = vrml97::parse_bytes(b"#VRML V2.0 utf8\n", "a.wrl").unwrap();
           take(s);"##,
        &["E0308"],
    );
}

#[test]
fn engine_parse_error_is_rejected_where_published_is_expected() {
    assert_probe_fails(
        TMP,
        "parse_error_not_interchangeable",
        r#"let _: vrml97::ParseError = x3d_parse::ParseError { message: String::new(), line: 0 };"#,
        &["E0308"],
    );
}

// -------------------------------------------------------------------------
// Matrix4: published lacks factor / get_transform (difference)
// -------------------------------------------------------------------------

#[test]
fn matrix4_published_has_set_transform() {
    let mut m = vrml97::fields::math::Matrix4::IDENTITY;
    let t = vrml97::fields::math::Vector3 { x: 1.0, y: 2.0, z: 3.0 };
    m.set_transform(Some(&t), None, None, None, None);
    // Translation lands in elements 12..15 (X_ITE Matrix4.js translate).
    assert_eq!(m.0, [1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 2.0, 3.0, 1.0]);
}

#[test]
fn matrix4_published_lacks_factor() {
    assert_probe_fails(
        TMP,
        "matrix4_lacks_factor",
        "let m = vrml97::fields::math::Matrix4::IDENTITY; m.factor(None, None, None, None);",
        &["E0599"],
    );
}

#[test]
fn matrix4_published_lacks_get_transform() {
    assert_probe_fails(
        TMP,
        "matrix4_lacks_get_transform",
        "let m = vrml97::fields::math::Matrix4::IDENTITY; m.get_transform(None, None, None, None, None);",
        &["E0599"],
    );
}

#[test]
fn matrix4_engine_has_factor_and_get_transform() {
    assert_probe_compiles(
        TMP,
        "matrix4_engine_has_factor",
        "let m = x3d_fields::math::matrix::Matrix4::IDENTITY;
         m.factor(None, None, None, None);
         m.get_transform(None, None, None, None, None);",
    );
}

// -------------------------------------------------------------------------
// FieldValue / FieldType: same type list, same default variant per type (shared contract)
// -------------------------------------------------------------------------

#[test]
fn field_types_and_default_variants_match_on_both_sides() {
    use vrml97::fields::FieldType as P;
    use x3d_fields::FieldType as E;
    assert_eq!(P::ALL.len(), 44);
    assert_eq!(E::ALL.len(), 44);
    for (p, e) in P::ALL.iter().zip(E::ALL.iter()) {
        assert_eq!(p.name(), e.name());
        let pv = format!("{:?}", vrml97::fields::FieldValue::default_for(*p));
        let ev = format!("{:?}", x3d_fields::FieldValue::default_for(*e));
        assert_eq!(pv, ev, "default value of {}", p.name());
        assert_eq!(vrml97::fields::FieldValue::default_for(*p).field_type(), *p);
    }
}

// -------------------------------------------------------------------------
// NodeKind: engine adds Imported (difference)
// -------------------------------------------------------------------------

#[test]
fn node_kind_published_lacks_imported_variant() {
    assert_probe_fails(
        TMP,
        "node_kind_lacks_imported",
        "let _ = vrml97::scene::scene::NodeKind::Imported(0);",
        &["E0599"],
    );
}

#[test]
fn node_kind_four_arm_match_is_exhaustive_only_for_published() {
    // The hazard R0.2-A section 7.3 names: a four-arm match is exhaustive for the
    // published enum and not exhaustive for the engine enum.
    assert_probe_compiles(
        TMP,
        "node_kind_match_published",
        "use vrml97::scene::scene::NodeKind::*;
         let k = vrml97::scene::scene::NodeKind::Placeholder;
         match k { Builtin(_) | Instance(_) | Placeholder | Internal(_) => {} }",
    );
    assert_probe_fails(
        TMP,
        "node_kind_match_engine",
        "use x3d_scene::scene::NodeKind::*;
         let k = x3d_scene::scene::NodeKind::Placeholder;
         match k { Builtin(_) | Instance(_) | Placeholder | Internal(_) => {} }",
        &["E0004"],
    );
}

// -------------------------------------------------------------------------
// Route: 6 fields published, 10 in the engine (difference)
// -------------------------------------------------------------------------

const ROUTE_SIX: &str = "source_node: 0, source_field: String::new(), destination_node: 0,
    destination_field: String::new(), source_loc: {P}::FieldLoc::Pre(0), destination_loc: {P}::FieldLoc::Pre(0),";
const ROUTE_EXTRA: &str = "id: 0, conn: None, raw_source_field: String::new(), raw_destination_field: String::new(),";

#[test]
fn route_published_is_exactly_the_six_field_literal() {
    let six = ROUTE_SIX.replace("{P}", "vrml97::scene");
    assert_probe_compiles(TMP, "route_published_six", &format!("let _ = vrml97::scene::scene::Route {{ {six} }};"));
    assert_probe_fails(
        TMP,
        "route_published_ten",
        &format!("let _ = vrml97::scene::scene::Route {{ {six} {ROUTE_EXTRA} }};"),
        &["E0560"],
    );
}

#[test]
fn route_engine_is_exactly_the_ten_field_literal() {
    let six = ROUTE_SIX.replace("{P}", "x3d_scene");
    assert_probe_compiles(TMP, "route_engine_ten", &format!("let _ = x3d_scene::scene::Route {{ {six} {ROUTE_EXTRA} }};"));
    assert_probe_fails(TMP, "route_engine_six", &format!("let _ = x3d_scene::scene::Route {{ {six} }};"), &["E0063"]);
}

// -------------------------------------------------------------------------
// ExecutionContext: 7 fields published, 9 in the engine (difference).
// R0.2-A section 7.3 said "4 fields"; the source has 7.
// -------------------------------------------------------------------------

const CTX_SEVEN: &str = "parent: None, outer: {P}::Outer::Scene, root_nodes: Vec::new(),
    named_nodes: Default::default(), protos: Default::default(), externprotos: Default::default(), routes: Vec::new(),";
const CTX_EXTRA: &str = "imported: Default::default(), exported: Default::default(),";

#[test]
fn execution_context_published_is_exactly_the_seven_field_literal() {
    let seven = CTX_SEVEN.replace("{P}", "vrml97::scene");
    assert_probe_compiles(TMP, "ctx_published_seven", &format!("let _ = vrml97::scene::scene::ExecutionContext {{ {seven} }};"));
    assert_probe_fails(
        TMP,
        "ctx_published_nine",
        &format!("let _ = vrml97::scene::scene::ExecutionContext {{ {seven} {CTX_EXTRA} }};"),
        &["E0560"],
    );
}

#[test]
fn execution_context_engine_is_exactly_the_nine_field_literal() {
    let seven = CTX_SEVEN.replace("{P}", "x3d_scene");
    assert_probe_compiles(TMP, "ctx_engine_nine", &format!("let _ = x3d_scene::scene::ExecutionContext {{ {seven} {CTX_EXTRA} }};"));
    assert_probe_fails(TMP, "ctx_engine_seven", &format!("let _ = x3d_scene::scene::ExecutionContext {{ {seven} }};"), &["E0063"]);
}

// -------------------------------------------------------------------------
// Scene: engine adds scene_infos, imported, exported, route_counter (difference)
// -------------------------------------------------------------------------

#[test]
fn scene_published_lacks_the_four_engine_fields() {
    for field in ["scene_infos", "imported", "exported", "route_counter"] {
        assert_probe_fails(
            TMP,
            &format!("scene_published_lacks_{field}"),
            &format!("let s = vrml97::parse_bytes(b\"#VRML V2.0 utf8\\n\", \"a.wrl\").unwrap(); let _ = &s.{field};"),
            &["E0609"],
        );
    }
}

#[test]
fn scene_engine_has_the_four_engine_fields() {
    let s = x3d_parse::parse_bytes(b"#VRML V2.0 utf8\n", "a.wrl").unwrap();
    assert_eq!(s.scene_infos.len(), 0);
    assert_eq!(s.imported.len(), 0);
    assert_eq!(s.exported.len(), 0);
    assert_eq!(s.route_counter, 0);
}

// -------------------------------------------------------------------------
// Error types. Neither LoadError implements Clone. R0.2-C section 9.1 said the engine
// one does; both sources say `#[derive(Debug)]` only.
// -------------------------------------------------------------------------

#[test]
fn load_error_is_not_clone_on_either_side() {
    assert_probe_fails(TMP, "load_error_published_not_clone", "fn c<T: Clone>() {} c::<vrml97::LoadError>();", &["E0277"]);
    assert_probe_fails(TMP, "load_error_engine_not_clone", "fn c<T: Clone>() {} c::<x3d_parse::LoadError>();", &["E0277"]);
}

#[test]
fn parse_error_is_clone_debug_display_on_both_sides() {
    fn c<T: Clone + std::fmt::Debug + std::fmt::Display>() {}
    c::<vrml97::ParseError>();
    c::<x3d_parse::ParseError>();
}
