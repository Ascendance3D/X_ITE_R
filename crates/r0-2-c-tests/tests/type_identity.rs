//! Suite 2 — Type identity after R0.2-D (Option B).
//!
//! The local `vrml97` is a facade: it re-exports `x3d-fields`, `x3d-scene` and
//! `x3d-parse`. These tests prove that each shared type has one compiled identity, and
//! record the exact engine shape that `vrml97` users now see. A shape is proven with a
//! compile probe: a throw-away crate that must compile, or must fail with the recorded
//! rustc error code. A comment that "would not compile" proves nothing, so none is used.
//!
//! Until R0.2-C1 this file recorded how the published 0.1.0 types differed. Those records
//! now run against the released archive in `published-consumer/` (main.rs and the compile
//! probes in published-consumer.sh), not against the local source.
//!
//! Field and variant facts: `crates/x3d-scene/src/scene.rs`.

use r0_2_c_tests::{assert_probe_compiles, assert_probe_fails};
use std::any::TypeId;

const TMP: &str = env!("CARGO_TARGET_TMPDIR");

// -------------------------------------------------------------------------
// One type authority (R0.2-C1 recorded two distinct types here)
// -------------------------------------------------------------------------

#[test]
fn scene_facade_is_the_engine_type() {
    assert_eq!(TypeId::of::<vrml97::Scene>(), TypeId::of::<x3d_scene::Scene>());
}

#[test]
fn shared_shape_types_are_one_type() {
    // R0.2-B section 4.4: same shape on both sides. Option B leaves one Rust type each.
    assert_eq!(TypeId::of::<vrml97::fields::FieldValue>(), TypeId::of::<x3d_fields::value::FieldValue>());
    assert_eq!(TypeId::of::<vrml97::fields::math::Matrix4>(), TypeId::of::<x3d_fields::math::matrix::Matrix4>());
    assert_eq!(TypeId::of::<vrml97::ParseError>(), TypeId::of::<x3d_parse::ParseError>());
    assert_eq!(TypeId::of::<vrml97::LoadError>(), TypeId::of::<x3d_parse::LoadError>());
}

#[test]
fn facade_scene_is_accepted_where_engine_scene_is_expected() {
    // A separate crate, not this test binary: proves the identity for any consumer.
    assert_probe_compiles(
        TMP,
        "scene_interchangeable",
        r##"fn take(_: x3d_scene::Scene) {}
           let s = vrml97::parse_bytes(b"#VRML V2.0 utf8\n", "a.wrl").unwrap();
           take(s);"##,
    );
}

#[test]
fn engine_parse_error_is_accepted_where_facade_is_expected() {
    assert_probe_compiles(
        TMP,
        "parse_error_interchangeable",
        r#"let _: vrml97::ParseError = x3d_parse::ParseError { message: String::new(), line: 0 };"#,
    );
}

// -------------------------------------------------------------------------
// Matrix4: factor / get_transform through the facade path (0.1.0 lacked both)
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
fn matrix4_facade_has_factor() {
    assert_probe_compiles(
        TMP,
        "matrix4_facade_has_factor",
        "let m = vrml97::fields::math::Matrix4::IDENTITY; m.factor(None, None, None, None);",
    );
}

#[test]
fn matrix4_facade_has_get_transform() {
    assert_probe_compiles(
        TMP,
        "matrix4_facade_has_get_transform",
        "let m = vrml97::fields::math::Matrix4::IDENTITY; m.get_transform(None, None, None, None, None);",
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
// NodeKind: the engine enum has Imported (0.1.0 did not)
// -------------------------------------------------------------------------

#[test]
fn node_kind_facade_has_imported_variant() {
    assert_probe_compiles(TMP, "node_kind_facade_has_imported", "let _ = vrml97::scene::scene::NodeKind::Imported(0);");
}

#[test]
fn node_kind_four_arm_match_is_not_exhaustive_through_the_facade() {
    // The hazard R0.2-A section 7.3 names, now a documented source break: a four-arm
    // match was exhaustive for 0.1.0 and is not exhaustive for the facade enum.
    assert_probe_fails(
        TMP,
        "node_kind_match_facade",
        "use vrml97::scene::scene::NodeKind::*;
         let k = vrml97::scene::scene::NodeKind::Placeholder;
         match k { Builtin(_) | Instance(_) | Placeholder | Internal(_) => {} }",
        &["E0004"],
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
// Route: 10 fields (0.1.0 had 6)
// -------------------------------------------------------------------------

const ROUTE_SIX: &str = "source_node: 0, source_field: String::new(), destination_node: 0,
    destination_field: String::new(), source_loc: {P}::FieldLoc::Pre(0), destination_loc: {P}::FieldLoc::Pre(0),";
const ROUTE_EXTRA: &str = "id: 0, conn: None, raw_source_field: String::new(), raw_destination_field: String::new(),";

#[test]
fn route_facade_is_exactly_the_ten_field_literal() {
    let six = ROUTE_SIX.replace("{P}", "vrml97::scene");
    assert_probe_compiles(TMP, "route_facade_ten", &format!("let _ = vrml97::scene::scene::Route {{ {six} {ROUTE_EXTRA} }};"));
    assert_probe_fails(TMP, "route_facade_six", &format!("let _ = vrml97::scene::scene::Route {{ {six} }};"), &["E0063"]);
}

#[test]
fn route_engine_is_exactly_the_ten_field_literal() {
    let six = ROUTE_SIX.replace("{P}", "x3d_scene");
    assert_probe_compiles(TMP, "route_engine_ten", &format!("let _ = x3d_scene::scene::Route {{ {six} {ROUTE_EXTRA} }};"));
    assert_probe_fails(TMP, "route_engine_six", &format!("let _ = x3d_scene::scene::Route {{ {six} }};"), &["E0063"]);
}

// -------------------------------------------------------------------------
// ExecutionContext: 9 fields (0.1.0 had 7; R0.2-A section 7.3 said 4).
// -------------------------------------------------------------------------

const CTX_SEVEN: &str = "parent: None, outer: {P}::Outer::Scene, root_nodes: Vec::new(),
    named_nodes: Default::default(), protos: Default::default(), externprotos: Default::default(), routes: Vec::new(),";
const CTX_EXTRA: &str = "imported: Default::default(), exported: Default::default(),";

#[test]
fn execution_context_facade_is_exactly_the_nine_field_literal() {
    let seven = CTX_SEVEN.replace("{P}", "vrml97::scene");
    assert_probe_compiles(TMP, "ctx_facade_nine", &format!("let _ = vrml97::scene::scene::ExecutionContext {{ {seven} {CTX_EXTRA} }};"));
    assert_probe_fails(TMP, "ctx_facade_seven", &format!("let _ = vrml97::scene::scene::ExecutionContext {{ {seven} }};"), &["E0063"]);
}

#[test]
fn execution_context_engine_is_exactly_the_nine_field_literal() {
    let seven = CTX_SEVEN.replace("{P}", "x3d_scene");
    assert_probe_compiles(TMP, "ctx_engine_nine", &format!("let _ = x3d_scene::scene::ExecutionContext {{ {seven} {CTX_EXTRA} }};"));
    assert_probe_fails(TMP, "ctx_engine_seven", &format!("let _ = x3d_scene::scene::ExecutionContext {{ {seven} }};"), &["E0063"]);
}

// -------------------------------------------------------------------------
// Scene: scene_infos, imported, exported, route_counter (0.1.0 lacked all four)
// -------------------------------------------------------------------------

#[test]
fn scene_facade_has_the_four_engine_fields() {
    let s = vrml97::parse_bytes(b"#VRML V2.0 utf8\n", "a.wrl").unwrap();
    assert_eq!(s.scene_infos.len(), 0);
    assert_eq!(s.imported.len(), 0);
    assert_eq!(s.exported.len(), 0);
    assert_eq!(s.route_counter, 0);
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
    // One type since R0.2-D; both paths are kept so the record names both.
    assert_probe_fails(TMP, "load_error_published_not_clone", "fn c<T: Clone>() {} c::<vrml97::LoadError>();", &["E0277"]);
    assert_probe_fails(TMP, "load_error_engine_not_clone", "fn c<T: Clone>() {} c::<x3d_parse::LoadError>();", &["E0277"]);
}

#[test]
fn parse_error_is_clone_debug_display_on_both_sides() {
    fn c<T: Clone + std::fmt::Debug + std::fmt::Display>() {}
    c::<vrml97::ParseError>();
    c::<x3d_parse::ParseError>();
}
