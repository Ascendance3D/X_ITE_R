//! ROUTE parse warnings (issue #137, PR #150 QA): only the no-context `TypeError` text skips the `Error: ` prefix.

use x3d_fields::FieldType;
use x3d_parse::Parser;
use x3d_scene::Scene;

fn warnings(text: &str, no_context: bool) -> Vec<String> {
    let (scene, _, _) = Parser::parse_field_probe(
        Scene::new(""),
        0,
        FieldType::MFNode,
        text,
        false,
        None,
        "",
        no_context,
    );

    scene.warnings
}

#[test]
fn a_route_error_with_a_context_keeps_its_error_prefix() {
    // Pinned X_ITE prints the Error object: `Parser error at line 1. Error: Unknown named or imported node 'a'.`
    assert_eq!(
        warnings("[ ROUTE a.b TO c.d ]", false),
        ["Parser error at line 1. Error: Unknown named or imported node 'a'."]
    );
}

#[test]
fn a_route_error_without_a_context_is_the_type_error_and_comes_before_the_node() {
    assert_eq!(
        warnings("[ ROUTE a.b TO c.d Group { } ]", true),
        [
            "Parser error at line 1. TypeError: Cannot read properties of undefined (reading 'getLocalNode')"
        ]
    );
}
