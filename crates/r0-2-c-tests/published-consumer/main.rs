#![allow(unused_imports)]
//! R0.2-C1 published 0.1.0 consumer. Built by published-consumer.sh against the
//! extracted crates.io archives only. Every `use ... as _` line is a compiler check
//! that the named public path exists in the released crate.

// The 24 public module paths of vrml97 0.1.0 (23 file modules + 1 inline module).
use vrml97::fields as _;
use vrml97::fields::fdlibm as _;
use vrml97::fields::format as _;
use vrml97::fields::generator as _;
use vrml97::fields::jsmath as _;
use vrml97::fields::math as _;
use vrml97::fields::math::color as _;
use vrml97::fields::math::matrix as _;
use vrml97::fields::math::quaternion as _;
use vrml97::fields::math::rotation as _;
use vrml97::fields::math::vector as _;
use vrml97::fields::types as _;
use vrml97::fields::types::access as _;
use vrml97::fields::value as _;
use vrml97::parse as _;
use vrml97::parse::color as _;
use vrml97::parse::lexer as _;
use vrml97::parse::parser as _;
use vrml97::scene as _;
use vrml97::scene::info_array as _;
use vrml97::scene::nodes as _;
use vrml97::scene::scene as _;
use vrml97::scene::setup as _;
use vrml97::scene::vrml as _;

// The crate-root set of vrml97 0.1.0: 3 modules + 9 `pub use` names.
use vrml97::{LoadError, ParseError, Parser, Scene, decode_text, gunzip, is_valid_vrml, load_file, parse_bytes};

use std::any::TypeId;

const MINIMAL: &[u8] = b"#VRML V2.0 utf8\nShape { appearance Appearance { material Material { } } geometry Box { } }\n";
// Pinned X_ITE 34cdf9f97 toVRMLString () of MINIMAL (tests/fixtures/xite/ORACLE.json, minimal.wrl).
const MINIMAL_ORACLE: &str = "#X3D V4.1 utf8 X_ITE V16.5.0\n\nPROFILE Interchange\n\nCOMPONENT EnvironmentalSensor : 3\nCOMPONENT PointingDeviceSensor : 1\nCOMPONENT Scripting : 1\nCOMPONENT Sound : 3\nCOMPONENT Text : 2\n\nShape {\n  appearance Appearance {\n    material Material { }\n  }\n  geometry Box { }\n}\n";

fn check(name: &str, ok: bool) -> bool {
    println!("{} {name}", if ok { "PASS" } else { "FAIL" });
    ok
}

fn main() {
    let _: for<'a, 'b> fn(&'a [u8], &'b str) -> Result<Scene, LoadError> = parse_bytes;
    let _: fn(&std::path::Path) -> Result<Scene, LoadError> = load_file;
    let _: fn(&[u8]) -> String = decode_text;
    let _: fn(&[u8]) -> Vec<u8> = gunzip;
    let _: fn(&str) -> bool = is_valid_vrml;
    let _ = std::mem::size_of::<Parser>();
    let _: fn(ParseError) -> String = |e| e.message;

    let mut ok = true;

    // vrml97: root re-exports are the module items, not copies.
    ok &= check("vrml97::Scene is vrml97::scene::scene::Scene", TypeId::of::<Scene>() == TypeId::of::<vrml97::scene::scene::Scene>());
    ok &= check("vrml97::ParseError is vrml97::parse::parser::ParseError", TypeId::of::<ParseError>() == TypeId::of::<vrml97::parse::parser::ParseError>());
    ok &= check("vrml97::fields::access is vrml97::fields::types::access", vrml97::fields::access::INPUT_OUTPUT == vrml97::fields::types::access::INPUT_OUTPUT);

    // vrml97: behavior.
    let scene = parse_bytes(MINIMAL, "minimal.wrl").expect("minimal parses");
    ok &= check("vrml97 minimal.wrl equals pinned X_ITE toVRMLString", scene.to_vrml_string() == MINIMAL_ORACLE);
    ok &= check("vrml97 gunzip passthrough", gunzip(b"hello") == b"hello");
    ok &= check("vrml97 decode_text strips one BOM", decode_text(b"\xef\xbb\xbfhi") == "hi");

    // vrml: re-exports are the vrml97 types.
    ok &= check("vrml::Scene is vrml97::Scene", TypeId::of::<vrml::Scene>() == TypeId::of::<Scene>());
    ok &= check("vrml::vrml97::Scene is vrml97::Scene", TypeId::of::<vrml::vrml97::Scene>() == TypeId::of::<Scene>());
    ok &= check("vrml::detect VRML2", vrml::detect(MINIMAL) == vrml::Version::Vrml2);
    ok &= check("vrml::detect X3D 3.3", vrml::detect(b"#X3D V3.3 utf8\n") == vrml::Version::X3d("3.3".into()));
    ok &= check("vrml::detect VRML1", vrml::detect(b"#VRML V1.0 ascii\n") == vrml::Version::Vrml1);
    ok &= check("vrml::is_gzip", vrml::is_gzip(&[0x1f, 0x8b]) && !vrml::is_gzip(b"#V"));
    let s = vrml::parse_bytes(MINIMAL, "minimal.wrl").expect("vrml parses");
    let sum = vrml::summarize(&s);
    ok &= check("vrml::summarize minimal.wrl", sum.root_nodes == 1 && sum.nodes == 4 && sum.warnings == 0
        && sum.node_types.iter().map(|(k, v)| format!("{k}={v}")).collect::<Vec<_>>() == ["Appearance=1", "Box=1", "Material=1", "Shape=1"]);

    // x3d: re-exports and node table (counts from data/x_ite-nodes.json: 263 nodes, 39 components, 8 profiles).
    ok &= check("x3d::Scene is vrml97::Scene", TypeId::of::<x3d::Scene>() == TypeId::of::<Scene>());
    ok &= check("x3d::nodes count 263", x3d::nodes().count() == 263);
    ok &= check("x3d::components count 39", x3d::components().count() == 39);
    ok &= check("x3d::profiles count 8", x3d::profiles().count() == 8);
    let t = x3d::node("Transform").expect("Transform");
    let f = t.field("translation").expect("translation");
    ok &= check("x3d Transform.translation", t.component() == "Grouping" && f.access() == "inputOutput" && f.type_name() == "SFVec3f" && f.default_vrml() == "0 0 0");
    ok &= check("x3d::node unknown is None", x3d::node("NotANode").is_none());
    let _: Option<x3d::Node> = x3d::node("Box");
    let _: Option<x3d::Field> = x3d::node("Box").and_then(|n| n.field("size"));

    if !ok {
        std::process::exit(1);
    }
    println!("ALL PASS");
}
