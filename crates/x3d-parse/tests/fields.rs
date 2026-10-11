//! Every SF/MF field type: parse with the VRML parser, print with the VRML generator, and
//! compare with X_ITE's `field .fromVRMLString (input)` / `field .toVRMLString ()`
//! (oracle/x_ite-fields.mjs → oracle/x_ite-fields.json, cases are synthetic).

use x3d_fields::{FieldType, FieldValue, Generator};
use x3d_parse::parser::Parser;

fn oracle() -> serde_json::Value {
    let path = concat!(env!("CARGO_MANIFEST_DIR"), "/../../oracle/x_ite-fields.json");
    serde_json::from_str(&std::fs::read_to_string(path).expect("run oracle/x_ite-fields.mjs first")).unwrap()
}

fn print(value: &FieldValue) -> String {
    let mut g = Generator::default();
    value.to_vrml_stream(&mut g);
    g.string
}

#[test]
fn every_field_type_parses_and_prints_like_x_ite() {
    let o = oracle();
    let cases = o["fields"].as_array().unwrap();
    let mut failures = vec![];
    let mut types = std::collections::BTreeSet::new();

    for case in cases {
        let ty_name = case["type"].as_str().unwrap();
        let input = case["input"].as_str().unwrap();
        let ty = FieldType::from_name(ty_name).unwrap();

        types.insert(ty_name.to_string());

        match Parser::parse_field_value(ty, input) {
            Err(e) => {
                if case["ok"].as_bool().unwrap() {
                    failures.push(format!("{ty_name} {input:?}: rust error {e}"));
                }
            }
            Ok(value) => {
                if !case["ok"].as_bool().unwrap() {
                    failures.push(format!("{ty_name} {input:?}: x_ite error {}, rust parsed", case["error"]));
                    continue;
                }

                let vrml = print(&value);
                let expected = case["vrml"].as_str().unwrap();

                if vrml != expected {
                    failures.push(format!("{ty_name} {input:?}: x_ite {expected:?} rust {vrml:?}"));
                }

                if ty.is_multi() {
                    let n = case["raw"].as_array().unwrap().len();

                    if value.len() != n {
                        failures.push(format!("{ty_name} {input:?}: x_ite {n} elements, rust {}", value.len()));
                    }
                }

                if value.is_default_value() != case["isDefault"].as_bool().unwrap() {
                    failures.push(format!("{ty_name} {input:?}: isDefaultValue differs"));
                }

                if value.equals(&FieldValue::default_for(ty)) != case["equalsDefault"].as_bool().unwrap() {
                    failures.push(format!("{ty_name} {input:?}: equals (default) differs"));
                }
            }
        }
    }

    // All 42 non-node field types are covered (SFNode/MFNode are exercised by the corpus test).
    assert_eq!(types.len(), 42, "{types:?}");
    assert!(failures.is_empty(), "{} of {} cases differ:\n{}", failures.len(), cases.len(), failures.join("\n"));
}
