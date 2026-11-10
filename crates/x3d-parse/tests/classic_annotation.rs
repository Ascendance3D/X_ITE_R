//! REA ledger entry sub:parse.vrml-classic, ADR 0002 (owner decision 2026-10-17, "Do both"): the Annotation component in the
//! classic encoding.
//!
//! Pinned X_ITE 16.5.0 ships the Annotation classes but `SupportedComponents.js` does not register the component, so its
//! parser rejects `COMPONENT Annotation:1` and `:2` (oracle/classic-edge-baseline/classic-edge.json, cases
//! `sweep-annotation-1` and `-2`, kept as known rows in classic-edge-known.tsv). The port registers it. This test pins
//! both sides of the decision for the classic encoding:
//!
//! 1. The port accepts the component statement. In an `#X3D V4.0` world it parses all six nodes with no message, and each
//!    node has the fields of the dormant X_ITE class (crates/x3d-scene/data/x_ite-nodes-annotation.json, dumped from the
//!    pinned classes). In the recorded `#X3D V3.3` cases the nodes stay unknown, because the dormant classes are
//!    `from: "4.0"` and X_ITE's version rule (the "inappropriate specification version" message) applies to them as to
//!    every registered node.
//! 2. Where X_ITE's own rules apply the port still equals X_ITE: without the component statement the nodes are unknown
//!    (case `sweep-annotation-no-component`, exact in classic_edge.rs), and a level above the class table's level 2 is
//!    clamped to 2, as X3DBrowser.getComponent does for every supported component (`Algorithm .clamp (level ||
//!    component .level, 1, component .level)`, called by VRMLParser.componentStatement).
//!
//! The runtime behavior of the nodes (scene text, log, poses and images against the dormant classes, and the two stand-ins)
//! is compared by crates/x3d-viewer/tests/rea_batch04.rs.

use serde_json::Value;

const PAGE_URL: &str = "http://oracle/x_ite-pin/src/dump.html";
const NODES: [&str; 6] = [
    "AnnotationLayer",
    "AnnotationTarget",
    "GroupAnnotation",
    "IconAnnotation",
    "TextAnnotation",
    "URLAnnotation",
];

fn world(version: &str, profile: &str, statement: &str) -> String {
    let mut lines = vec![
        format!("#X3D V{version} utf8"),
        format!("PROFILE {profile}"),
    ];

    if !statement.is_empty() {
        lines.push(statement.to_string());
    }

    lines.extend(NODES.iter().map(|node| format!("{node} {{ }}")));
    lines.join("\n")
}

fn baseline_case(name: &str) -> Value {
    let path = format!(
        "{}/../../oracle/classic-edge-baseline/classic-edge.json",
        env!("CARGO_MANIFEST_DIR")
    );
    let baseline: Value = serde_json::from_str(&std::fs::read_to_string(path).unwrap()).unwrap();

    baseline["results"]
        .as_array()
        .unwrap()
        .iter()
        .find(|c| c["name"] == name)
        .unwrap_or_else(|| panic!("{name}: not in the baseline"))
        .clone()
}

fn table() -> Value {
    serde_json::from_str(include_str!(
        "../../x3d-scene/data/x_ite-nodes-annotation.json"
    ))
    .unwrap()
}

fn dormant(table: &Value, node: &str) -> Value {
    table["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .find(|n| n["typeName"] == node)
        .unwrap_or_else(|| panic!("{node}: not in x_ite-nodes-annotation.json"))
        .clone()
}

fn unknown_by_version(warnings: &[String]) -> bool {
    warnings.len() == NODES.len()
        && warnings.iter().zip(NODES).all(|(w, node)| {
            w.contains(&format!("Unknown node type or proto '{node}'"))
                && w.contains("inappropriate specification version")
        })
}

#[test]
fn recorded_v3_3_cases_accept_the_component_and_keep_x_ite_s_version_rule() {
    let table = table();

    for node in NODES {
        assert_eq!(dormant(&table, node)["from"], "4.0", "{node}: from");
    }

    for level in [1, 2] {
        let name = format!("sweep-annotation-{level}");
        let text = world("3.3", "Core", &format!("COMPONENT Annotation:{level}"));

        // The input is the recorded case, so the pinned rejection is X_ITE's answer to this exact text.
        let case = baseline_case(&name);
        assert_eq!(case["text"].as_str().unwrap(), text, "{name}: input");
        assert_eq!(case["ok"], false, "{name}: pinned X_ITE rejects it");
        assert!(
            case["error"].as_str().unwrap().contains(&format!(
                "Component 'Annotation' at level '{level}' is not supported."
            )),
            "{name}: pinned rejection text"
        );

        let scene = x3d_parse::parse_string(&text, PAGE_URL)
            .unwrap_or_else(|e| panic!("{name}: the port rejects the component: {e:?}"));

        assert_eq!(
            scene.components,
            vec![("Annotation".to_string(), level)],
            "{name}: components"
        );
        assert!(
            scene.contexts[0].root_nodes.iter().all(Option::is_none),
            "{name}: V3.3 roots"
        );
        assert!(
            unknown_by_version(&scene.warnings),
            "{name}: {:?}",
            scene.warnings
        );
    }
}

#[test]
fn v4_0_component_loads_the_dormant_classes() {
    let table = table();

    for level in [1, 2, 3] {
        let text = world("4.0", "Core", &format!("COMPONENT Annotation:{level}"));
        let scene = x3d_parse::parse_string(&text, PAGE_URL)
            .unwrap_or_else(|e| panic!("level {level}: the port rejects the component: {e:?}"));

        // Level 3 is above the class table's level 2: clamped, as X3DBrowser.getComponent clamps every supported component.
        assert_eq!(
            scene.components,
            vec![("Annotation".to_string(), level.min(2))],
            "level {level}: components"
        );
        assert!(
            scene.warnings.is_empty(),
            "level {level}: {:?}",
            scene.warnings
        );

        let roots = &scene.contexts[0].root_nodes;
        assert_eq!(roots.len(), NODES.len(), "level {level}: root nodes");

        for (slot, node) in roots.iter().zip(NODES) {
            let id = slot.unwrap_or_else(|| panic!("level {level}: {node} is NULL"));
            assert_eq!(scene.type_name(id), node, "level {level}: type");

            for field in dormant(&table, node)["x3d"]["fields"].as_array().unwrap() {
                let field = field[1].as_str().unwrap();
                assert!(
                    scene.get_field(id, field).is_some(),
                    "level {level}: {node} has no field {field} of the dormant class"
                );
            }
        }
    }
}

#[test]
fn without_the_component_the_nodes_are_unknown_as_in_x_ite() {
    // X_ITE's Full profile has no Annotation component: the recorded case (exact in classic_edge.rs) and a V4.0 twin.
    let case = baseline_case("sweep-annotation-no-component");
    assert_eq!(case["ok"], true);
    assert_eq!(case["rootNodes"], NODES.len());

    for text in [
        case["text"].as_str().unwrap().to_string(),
        world("4.0", "Full", ""),
    ] {
        let scene = x3d_parse::parse_string(&text, PAGE_URL).unwrap();
        assert_eq!(scene.contexts[0].root_nodes.len(), NODES.len(), "{text}");
        assert!(
            scene.contexts[0].root_nodes.iter().all(Option::is_none),
            "{text}"
        );
        assert_eq!(scene.warnings.len(), NODES.len(), "{:?}", scene.warnings);
    }
}
