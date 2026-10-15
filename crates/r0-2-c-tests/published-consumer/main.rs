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

    ok &= historical_differences();

    if !ok {
        std::process::exit(1);
    }
    println!("ALL PASS");
}

// -------------------------------------------------------------------------
// Historical difference records of vrml97 0.1.0 (moved here from the local suite in R0.2-D).
//
// R0.2-D made the local vrml97 a facade over the engine crates, so the local suite now
// checks parity. These records keep the proof of how the *released* 0.1.0 differs from
// X_ITE, against the released archive only. Items that 0.1.0 lacks (Matrix4::factor,
// NodeKind::Imported, the four Scene fields, the extra Route and ExecutionContext fields)
// are proven by the compile probes in published-consumer.sh, because a missing item
// cannot be named in this program.
// -------------------------------------------------------------------------

// Byte-identical to crates/r0-2-c-tests/tests/fixtures/xite/{export.x3dv,export-vrml2,import,ortho-v33}.wrl.
const EXPORT_X3DV: &[u8] = b"#X3D V3.3 utf8\nPROFILE Interchange\nDEF G Group { }\nEXPORT G\n";
const EXPORT_VRML2: &[u8] = b"#VRML V2.0 utf8\nDEF G Group { }\nEXPORT G\n";
const IMPORT: &[u8] = b"#X3D V3.3 utf8\nPROFILE Interchange\nDEF I Inline { url \"import-child.wrl\" }\nIMPORT I.G AS N\n";
const ORTHO_V33: &[u8] = b"#X3D V3.3 utf8\nPROFILE Immersive\nCOMPONENT Navigation:3\nOrthoViewpoint { fieldOfView [ -2 -1 2 1 ] }\n";
// Pinned X_ITE 34cdf9f97 text of ortho-v33.wrl (ORACLE.json) with fieldOfView as 0.1.0 prints it.
const ORTHO_V33_PUBLISHED: &str = "#X3D V4.1 utf8 X_ITE V16.5.0\n\nPROFILE Immersive\n\nCOMPONENT Navigation : 3\n\nOrthoViewpoint {\n  fieldOfView [\n    -2,\n    -1,\n    2,\n    1\n  ]\n}\n";

// Chrome 155 Math.sin / cos / tan vectors where glibc differs (crates/x3d-fields/tests/chrome_math.rs).
const CHROME_SIN: &[(u64, u64)] = &[
    (0xbfc4d644da65bda0, 0xbfc4bebd80c34c7b),
    (0xc00b18b162781378, 0x3fcf1ad94ab666a3),
    (0x400b03976beb5720, 0xbfcdd2edfb5f0b65),
    (0xc00e05b6fbe3c732, 0x3fe25cf82689c635),
    (0x400a0f052caa5114, 0xbfbd904e7e0da65f),
    (0xc00a38529536ac0c, 0x3fc157c1859ab0c5),
    (0xbfd5d5a5f1f14bf0, 0xbfd569d825235469),
    (0xc004e6e382b5e6b4, 0xbfe02539fd704e81),
];
const CHROME_COS: &[(u64, u64)] = &[
    (0x3feb22eaf76f67b0, 0x3fe52acdc024418d),
    (0x3ff1ce01251878f4, 0x3fdc4c4d7e78b1e5),
    (0x3ff32921a973f558, 0x3fd756637149ff05),
    (0x3fefcbd013106548, 0x3fe175fafb8f0d95),
    (0xbffeb29b61d06a98, 0xbfd5d04d3011209b),
    (0xbfff312e5d66aa34, 0xbfd7a98a46e8f627),
    (0x4001cc7917628b0e, 0xbfe37804ff5a92dd),
    (0xbfe99fe047ff1b30, 0x3fe646eae0aed533),
];
const CHROME_TAN: &[(u64, u64)] = &[
    (0x3fc691e7d2715320, 0x3fc6ce8a79d39c49),
    (0x3ff505a153b86bac, 0x400e73211f7a4d3f),
    (0xbfe016adbc016408, 0xbfe198cd3b51e153),
    (0xc0084b532498b29c, 0x3fbaee49c2144175),
    (0x3fbf4657f99eca80, 0x3fbf6e69f691d8dd),
    (0x40087dbf85a5cc16, 0xbfb492c54374c738),
    (0xbfe613ad768d9308, 0xbfea67cdf2f38a55),
    (0xbffb19e9b3961d90, 0x40202c90eb667c05),
];

fn historical_differences() -> bool {
    use vrml97::fields::{format, jsmath};
    let mut ok = true;

    // Known difference 1: 0.1.0 rejects EXPORT and IMPORT; X_ITE accepts both.
    for (name, input, line) in [("export.x3dv.wrl", EXPORT_X3DV, 4), ("export-vrml2.wrl", EXPORT_VRML2, 3)] {
        let got = parse_bytes(input, name).err().map(|e| e.to_string());
        let want = format!("Parser error at line {line}: EXPORT is not supported by the Rust port yet (phase X4).");
        ok &= check(&format!("0.1.0 difference 1: {name} EXPORT is a hard error"), got.as_deref() == Some(want.as_str()));
    }
    let got = parse_bytes(IMPORT, "import.wrl").err().map(|e| e.to_string());
    ok &= check(
        "0.1.0 difference 1: import.wrl IMPORT is a hard error",
        got.as_deref() == Some("Parser error at line 4: IMPORT is not supported by the Rust port yet (phase X4)."),
    );

    // Known difference 2: number formatting. X_ITE / Chrome give "1" for 0.9999999999999994 at 15 digits.
    ok &= check("0.1.0 difference 2: DoubleFormat(0.9999999999999994) is 0.999999999999999", format::double_format(0.9999999999999994) == "0.999999999999999");
    let intl: &[(u64, usize, &str)] = &[
        (0x3fefffffffffffff, 16, "0.9999999999999999"),
        (0x3feffffffffffffe, 16, "0.9999999999999998"),
        (0x3feffffffffffffd, 16, "0.9999999999999997"),
        (0x3feffffffffffffc, 16, "0.9999999999999996"),
        (0x3feffffffffffffb, 15, "0.999999999999999"),
        (0x3feffffffffffffb, 16, "0.9999999999999994"),
    ];
    for (b, p, published) in intl {
        ok &= check(
            &format!("0.1.0 difference 2: intl_standard({:e}, {p}) is {published}", f64::from_bits(*b)),
            format::intl_standard(f64::from_bits(*b), *p) == *published,
        );
    }

    // Known difference 3: the 0.1.0 Chrome flavor is the platform libm, not Chrome.
    jsmath::set_flavor(jsmath::MathFlavor::Chrome);
    let mut misses = 0;
    let mut platform = true;
    for (table, f, p) in [
        (CHROME_SIN, jsmath::sin as fn(f64) -> f64, f64::sin as fn(f64) -> f64),
        (CHROME_COS, jsmath::cos, f64::cos),
        (CHROME_TAN, jsmath::tan, f64::tan),
    ] {
        for (x, chrome) in table {
            let got = f(f64::from_bits(*x)).to_bits();
            misses += (got != *chrome) as usize;
            platform &= got == p(f64::from_bits(*x)).to_bits();
        }
    }
    jsmath::set_flavor(jsmath::MathFlavor::Node);
    ok &= check("0.1.0 difference 3: Chrome-flavor sin/cos/tan equal the platform libm", platform);
    ok &= check(&format!("0.1.0 difference 3: Chrome-flavor math misses Chrome on {misses} of 24 vectors (> 0)"), misses > 0);

    // Known difference 9: OrthoViewpoint.fieldOfView prints as MFFloat; X_ITE prints `fieldOfView -2 -1 2 1`.
    let got = parse_bytes(ORTHO_V33, "ortho-v33.wrl").ok().map(|s| s.to_vrml_string());
    ok &= check("0.1.0 difference 9: ortho-v33.wrl fieldOfView prints as MFFloat", got.as_deref() == Some(ORTHO_V33_PUBLISHED));

    // Shape records. Each literal and match below compiles only against the exact 0.1.0
    // shape: a struct literal must name every field, a match must cover every variant.
    use vrml97::scene::{FieldLoc, Outer, scene::{ExecutionContext, NodeKind, Route}};
    let r = Route {
        source_node: 0,
        source_field: String::new(),
        destination_node: 0,
        destination_field: String::new(),
        source_loc: FieldLoc::Pre(0),
        destination_loc: FieldLoc::Pre(0),
    };
    ok &= check("0.1.0 shape: Route is exactly the six-field literal", r.source_field.is_empty());
    let c = ExecutionContext {
        parent: None,
        outer: Outer::Scene,
        root_nodes: Vec::new(),
        named_nodes: Default::default(),
        protos: Default::default(),
        externprotos: Default::default(),
        routes: Vec::new(),
    };
    ok &= check("0.1.0 shape: ExecutionContext is exactly the seven-field literal", c.root_nodes.is_empty());
    let k = NodeKind::Placeholder;
    let four = match k {
        NodeKind::Builtin(_) | NodeKind::Instance(_) | NodeKind::Internal(_) => false,
        NodeKind::Placeholder => true,
    };
    ok &= check("0.1.0 shape: a four-arm NodeKind match is exhaustive", four);

    ok
}
