//! Suite 1 — Public API compilation (compile-only checks plus re-export identity).
//!
//! `use <module path> as _;` compiles only when that exact module path is public. One test
//! per path names the path itself, not a type re-exported somewhere else.
//!
//! vrml97 0.1.0 has 24 public module paths: 23 file-backed modules plus the inline
//! `fields::types::access`. The 23 file-backed paths are the R0.2-A section 4.5 table.
//! R0.2-B section 6 said "20" and claimed three `mod.rs` files collapse; that is wrong,
//! a `mod.rs` file *is* its parent module (`fields`, `fields::math`, `parse`, `scene`).
//! rustdoc on the published archive lists the same 24 (C1 evidence
//! `rustdoc-modules-published.txt`).
//!
//! The vrml97 crate root has 3 `pub mod` and 9 `pub use` names (R0.2-A/B said 8).
//! It exports no macro (R0.2-A section 4.8 said 2; both `macro_rules!` are crate-local).

#![allow(unused_imports)]

use std::any::TypeId;

macro_rules! module_path_test {
    ($name:ident, $($path:tt)+) => {
        #[test]
        fn $name() {
            use $($path)+ as _;
        }
    };
}

module_path_test!(path_vrml97_fields, vrml97::fields);
module_path_test!(path_vrml97_fields_fdlibm, vrml97::fields::fdlibm);
module_path_test!(path_vrml97_fields_format, vrml97::fields::format);
module_path_test!(path_vrml97_fields_generator, vrml97::fields::generator);
module_path_test!(path_vrml97_fields_jsmath, vrml97::fields::jsmath);
module_path_test!(path_vrml97_fields_math, vrml97::fields::math);
module_path_test!(path_vrml97_fields_math_color, vrml97::fields::math::color);
module_path_test!(
    path_vrml97_fields_math_complex,
    vrml97::fields::math::complex
);
module_path_test!(path_vrml97_fields_math_matrix, vrml97::fields::math::matrix);
module_path_test!(
    path_vrml97_fields_math_quaternion,
    vrml97::fields::math::quaternion
);
module_path_test!(
    path_vrml97_fields_math_rotation,
    vrml97::fields::math::rotation
);
module_path_test!(path_vrml97_fields_math_vector, vrml97::fields::math::vector);
module_path_test!(path_vrml97_fields_types, vrml97::fields::types);
module_path_test!(
    path_vrml97_fields_types_access,
    vrml97::fields::types::access
);
module_path_test!(path_vrml97_fields_value, vrml97::fields::value);
module_path_test!(path_vrml97_parse, vrml97::parse);
module_path_test!(path_vrml97_parse_color, vrml97::parse::color);
module_path_test!(path_vrml97_parse_lexer, vrml97::parse::lexer);
module_path_test!(path_vrml97_parse_parser, vrml97::parse::parser);
module_path_test!(path_vrml97_scene, vrml97::scene);
module_path_test!(path_vrml97_scene_info_array, vrml97::scene::info_array);
module_path_test!(path_vrml97_scene_nodes, vrml97::scene::nodes);
module_path_test!(path_vrml97_scene_scene, vrml97::scene::scene);
module_path_test!(path_vrml97_scene_setup, vrml97::scene::setup);
module_path_test!(path_vrml97_scene_vrml, vrml97::scene::vrml);

/// The 24 paths above, as text, in the same order.
const MODULE_PATHS: [&str; 25] = [
    "fields",
    "fields::fdlibm",
    "fields::format",
    "fields::generator",
    "fields::jsmath",
    "fields::math",
    "fields::math::color",
    "fields::math::complex",
    "fields::math::matrix",
    "fields::math::quaternion",
    "fields::math::rotation",
    "fields::math::vector",
    "fields::types",
    "fields::types::access",
    "fields::value",
    "parse",
    "parse::color",
    "parse::lexer",
    "parse::parser",
    "scene",
    "scene::info_array",
    "scene::nodes",
    "scene::scene",
    "scene::setup",
    "scene::vrml",
];

/// Cross-check: every module path the facade exposes is in MODULE_PATHS, and no other
/// path is. This catches a new public module that the compile checks above would miss.
///
/// Since R0.2-D `crates/vrml97/src/lib.rs` is a facade. `pub use x3d_fields as fields;`
/// and `pub use x3d_scene as scene;` expose each engine crate's whole `pub mod` tree, so
/// this walks the engine source. `pub mod parse` re-exports a named list from `x3d_parse`;
/// only the `x3d_parse` modules in that list are public.
#[test]
fn module_path_list_is_complete() {
    fn walk(file: &std::path::Path, prefix: &str, out: &mut Vec<String>) {
        let dir = file.parent().unwrap();
        for line in std::fs::read_to_string(file).unwrap().lines() {
            let Some(rest) = line.trim_start().strip_prefix("pub mod ") else {
                continue;
            };
            let name = rest.trim_end_matches([';', '{', ' ']).trim();
            let path = format!("{prefix}::{name}");
            out.push(path.clone());
            if rest.trim_end().ends_with(';') {
                let sub = dir.join(name).join("mod.rs");
                walk(
                    &if sub.exists() {
                        sub
                    } else {
                        dir.join(format!("{name}.rs"))
                    },
                    &path,
                    out,
                );
            }
        }
    }
    let crates = r0_2_c_tests::workspace_root().join("crates");
    let lib = std::fs::read_to_string(crates.join("vrml97/src/lib.rs")).unwrap();
    let mut found = Vec::new();

    let aliases: Vec<(&str, &str)> = lib
        .lines()
        .filter_map(|l| {
            l.trim()
                .strip_prefix("pub use x3d_")?
                .strip_suffix(';')?
                .split_once(" as ")
        })
        .collect();
    assert_eq!(
        aliases,
        [("fields", "fields"), ("scene", "scene")],
        "crate-wide module aliases"
    );
    for (krate, name) in aliases {
        found.push(name.to_string());
        walk(
            &crates.join(format!("x3d-{krate}/src/lib.rs")),
            name,
            &mut found,
        );
    }

    assert!(
        lib.contains("\npub mod parse {\n"),
        "inline facade module parse"
    );
    found.push("parse".to_string());
    let block = lib
        .split("pub mod parse {")
        .nth(1)
        .unwrap()
        .split('}')
        .next()
        .unwrap();
    let listed: Vec<&str> = block
        .split("x3d_parse::{")
        .nth(1)
        .unwrap()
        .split([',', ' ', '\n'])
        .filter(|w| !w.is_empty())
        .collect();
    let engine = crates.join("x3d-parse/src/lib.rs");
    let mut engine_mods = Vec::new();
    walk(&engine, "parse", &mut engine_mods);
    for m in engine_mods {
        let top = m.split("::").nth(1).unwrap();
        if listed.contains(&top) {
            found.push(m);
        }
    }

    found.sort();
    let mut want: Vec<String> = MODULE_PATHS.iter().map(|s| s.to_string()).collect();
    want.sort();
    assert_eq!(found, want);
}

/// `x3d_parse::compare` is an oracle test helper that vrml97 never released. The facade
/// keeps it out of `vrml97::parse`.
#[test]
fn parse_compare_is_not_public() {
    r0_2_c_tests::assert_probe_fails(
        env!("CARGO_TARGET_TMPDIR"),
        "parse_compare_not_public",
        "use vrml97::parse::compare as _;",
        &["E0432"],
    );
}

// -------------------------------------------------------------------------
// vrml97 crate root: 9 `pub use` names, each the module item itself
// -------------------------------------------------------------------------

#[test]
fn root_vrml97_types_are_the_module_types() {
    assert_eq!(
        TypeId::of::<vrml97::Scene>(),
        TypeId::of::<vrml97::scene::scene::Scene>()
    );
    assert_eq!(
        TypeId::of::<vrml97::Scene>(),
        TypeId::of::<vrml97::scene::Scene>()
    );
    assert_eq!(
        TypeId::of::<vrml97::ParseError>(),
        TypeId::of::<vrml97::parse::parser::ParseError>()
    );
    assert_eq!(
        TypeId::of::<vrml97::Parser>(),
        TypeId::of::<vrml97::parse::parser::Parser>()
    );
    assert_eq!(
        TypeId::of::<vrml97::LoadError>(),
        TypeId::of::<vrml97::parse::LoadError>()
    );
}

#[test]
fn root_vrml97_functions_have_the_released_signatures() {
    let _: fn(&[u8]) -> String = vrml97::decode_text;
    let _: fn(&[u8]) -> Vec<u8> = vrml97::gunzip;
    let _: fn(&str) -> bool = vrml97::is_valid_vrml;
    let _: fn(&std::path::Path) -> Result<vrml97::Scene, vrml97::LoadError> = vrml97::load_file;
    let _: for<'a, 'b> fn(&'a [u8], &'b str) -> Result<vrml97::Scene, vrml97::LoadError> =
        vrml97::parse_bytes;
    // Same items through the module path.
    let _: fn(&[u8]) -> String = vrml97::parse::decode_text;
    let _: fn(&[u8]) -> Vec<u8> = vrml97::parse::gunzip;
    let _: fn(&str) -> bool = vrml97::parse::is_valid_vrml;
}

#[test]
fn root_vrml97_is_valid_vrml_matches_vrmlparser_isvalid() {
    // VRMLParser.isValid: VRML 1.0 is rejected, VRML 2.0 and X3D headers are accepted.
    assert!(vrml97::is_valid_vrml("#VRML V2.0 utf8\n"));
    assert!(vrml97::is_valid_vrml("#X3D V3.3 utf8\n"));
    assert!(!vrml97::is_valid_vrml("#VRML V1.0 ascii\n"));
}

#[test]
fn fields_reexports_are_the_module_items() {
    assert_eq!(
        TypeId::of::<vrml97::fields::FieldValue>(),
        TypeId::of::<vrml97::fields::value::FieldValue>()
    );
    assert_eq!(
        TypeId::of::<vrml97::fields::FieldType>(),
        TypeId::of::<vrml97::fields::types::FieldType>()
    );
    assert_eq!(
        TypeId::of::<vrml97::fields::Generator>(),
        TypeId::of::<vrml97::fields::generator::Generator>()
    );
    assert_eq!(
        TypeId::of::<vrml97::fields::math::Matrix4>(),
        TypeId::of::<vrml97::fields::math::matrix::Matrix4>()
    );
    assert_eq!(
        TypeId::of::<vrml97::fields::math::Matrix3>(),
        TypeId::of::<vrml97::fields::math::matrix::Matrix3>()
    );
    assert_eq!(
        TypeId::of::<vrml97::fields::math::Vector3>(),
        TypeId::of::<vrml97::fields::math::vector::Vector3>()
    );
    assert_eq!(
        TypeId::of::<vrml97::fields::math::Color3>(),
        TypeId::of::<vrml97::fields::math::color::Color3>()
    );
    assert_eq!(
        TypeId::of::<vrml97::fields::math::Quaternion>(),
        TypeId::of::<vrml97::fields::math::quaternion::Quaternion>()
    );
    assert_eq!(
        TypeId::of::<vrml97::fields::math::Rotation4>(),
        TypeId::of::<vrml97::fields::math::rotation::Rotation4>()
    );
    assert_eq!(
        vrml97::fields::access::INPUT_OUTPUT,
        vrml97::fields::types::access::INPUT_OUTPUT
    );
}

// -------------------------------------------------------------------------
// vrml crate root: 4 re-exports + Version, detect, is_gzip, Summary, summarize
// -------------------------------------------------------------------------

#[test]
fn root_vrml_reexports_are_vrml97_items() {
    assert_eq!(TypeId::of::<vrml::Scene>(), TypeId::of::<vrml97::Scene>());
    assert_eq!(
        TypeId::of::<vrml::vrml97::Scene>(),
        TypeId::of::<vrml97::Scene>()
    );
    let _: fn(&std::path::Path) -> Result<vrml::Scene, vrml97::LoadError> = vrml::load_file;
    let _: for<'a, 'b> fn(&'a [u8], &'b str) -> Result<vrml::Scene, vrml97::LoadError> =
        vrml::parse_bytes;
}

#[test]
fn root_vrml_detect() {
    let _: fn(&[u8]) -> vrml::Version = vrml::detect;
    assert_eq!(vrml::detect(b"#VRML V2.0 utf8\n"), vrml::Version::Vrml2);
    assert_eq!(vrml::detect(b"#VRML V1.0 ascii\n"), vrml::Version::Vrml1);
    assert_eq!(
        vrml::detect(b"#X3D V3.3 utf8\n"),
        vrml::Version::X3d("3.3".into())
    );
    assert_eq!(vrml::detect(b"Group { }"), vrml::Version::Unknown);
    assert_eq!(
        vrml::detect(&r0_2_c_tests::fixture("xite/minimal-gzip.wrl")),
        vrml::Version::Vrml2
    );
}

#[test]
fn root_vrml_is_gzip() {
    let _: fn(&[u8]) -> bool = vrml::is_gzip;
    assert!(vrml::is_gzip(&[0x1f, 0x8b]));
    assert!(!vrml::is_gzip(&[0x1f]));
    assert!(!vrml::is_gzip(b"#VRML"));
}

#[test]
fn root_vrml_summarize() {
    let _: fn(&vrml97::Scene) -> vrml::Summary = vrml::summarize;
    let s = vrml::parse_bytes(&r0_2_c_tests::fixture("xite/minimal.wrl"), "minimal.wrl").unwrap();
    let sum = vrml::summarize(&s);
    assert_eq!(sum.specification_version, "2.0");
    assert_eq!(
        (
            sum.root_nodes,
            sum.nodes,
            sum.protos,
            sum.externprotos,
            sum.routes,
            sum.warnings
        ),
        (1, 4, 0, 0, 0, 0)
    );
    let types: Vec<(&str, usize)> = sum
        .node_types
        .iter()
        .map(|(k, v)| (k.as_str(), *v))
        .collect();
    assert_eq!(
        types,
        [("Appearance", 1), ("Box", 1), ("Material", 1), ("Shape", 1)]
    );
}

// -------------------------------------------------------------------------
// x3d crate root: 4 re-exports + Node, Field, node, nodes, components, profiles
// Counts: crates/x3d-scene/data/x_ite-nodes.json has 263 nodes, 39 components, 8 profiles.
// -------------------------------------------------------------------------

#[test]
fn root_x3d_reexports_are_vrml97_items() {
    assert_eq!(TypeId::of::<x3d::Scene>(), TypeId::of::<vrml97::Scene>());
    assert_eq!(
        TypeId::of::<x3d::vrml97::Scene>(),
        TypeId::of::<vrml97::Scene>()
    );
    let _: fn(&std::path::Path) -> Result<x3d::Scene, vrml97::LoadError> = x3d::load_file;
    let _: for<'a, 'b> fn(&'a [u8], &'b str) -> Result<x3d::Scene, vrml97::LoadError> =
        x3d::parse_bytes;
}

#[test]
fn root_x3d_node_and_field() {
    let _: fn(&str) -> Option<x3d::Node> = x3d::node;
    let t = x3d::node("Transform").unwrap();
    assert_eq!(
        (t.name(), t.component(), t.container_field()),
        ("Transform", "Grouping", "children")
    );
    let f: x3d::Field = t.field("translation").unwrap();
    assert_eq!(
        (f.access(), f.type_name(), f.default_vrml().as_str()),
        ("inputOutput", "SFVec3f", "0 0 0")
    );
    assert!(x3d::node("NotANode").is_none());
}

#[test]
fn root_x3d_nodes_components_profiles() {
    // 284 since the table includes Texturing3D and VolumeRendering (21 node types X_ITE loads as external components).
    // 290 and 40 since REA batch 04 (owner decision 2026-10-17): the Annotation component (6 node
    // types) that X_ITE 16.5.0 ships but does not register (crates/x3d-scene/data/x_ite-nodes-annotation.json).
    assert_eq!(x3d::nodes().count(), 290);
    assert_eq!(x3d::components().count(), 40);
    assert_eq!(x3d::profiles().count(), 8);
    assert!(x3d::profiles().any(|(name, _)| name == "Interchange"));
}
