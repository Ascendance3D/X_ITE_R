//! X3DField.fromVRMLString / fromXMLString without a scene (issue #137, PR #150 QA). The expected values are the
//! pinned X_ITE 16.5.0 recording `fieldText.noScene` in oracle/web-browser-fields-02-baseline.

use x3d_fields::{FieldType, FieldValue};
use x3d_runtime::{NoSceneError, field_from_string_without_scene};

fn read(
    ty: FieldType,
    text: &str,
    xml: bool,
) -> (Result<Option<FieldValue>, NoSceneError>, Vec<String>) {
    field_from_string_without_scene(ty, text, xml, "")
}

#[test]
fn a_vrml_node_value_that_needs_a_node_needs_a_scene() {
    for (ty, text) in [
        (FieldType::SFNode, "Transform { }"),
        (FieldType::SFNode, "Bogus { }"),
        (FieldType::MFNode, "[ Group { } ]"),
        (FieldType::MFNode, "Group { }"),
        (FieldType::MFNode, "[ Bogus { } ]"),
    ] {
        let (result, warnings) = read(ty, text, false);

        assert!(matches!(result, Err(NoSceneError::NeedsScene)), "{text}");
        assert!(warnings.is_empty(), "{text}");
    }
}

#[test]
fn a_vrml_node_value_with_no_node_stands() {
    assert!(matches!(
        read(FieldType::SFNode, "NULL", false).0,
        Ok(Some(FieldValue::SFNode(None)))
    ));
    assert!(matches!(
        read(FieldType::MFNode, "[ ]", false).0,
        Ok(Some(FieldValue::MFNode(v))) if v.is_empty()
    ));
}

#[test]
fn a_vrml_node_value_with_no_node_statement_reads_nothing() {
    // X_ITE: `Error: Couldn't read value for field ''.`, not the TypeError, and no warning.
    for (ty, text) in [(FieldType::SFNode, ""), (FieldType::SFNode, "}")] {
        let (result, warnings) = read(ty, text, false);

        assert!(matches!(result, Ok(None)), "{text:?}: {result:?}");
        assert!(warnings.is_empty(), "{text:?}");
    }
}

#[test]
fn an_xml_node_value_reads_no_node_and_never_needs_a_scene() {
    for text in ["<Transform/>", "", "NULL", "<Bogus"] {
        let (result, warnings) = read(FieldType::SFNode, text, true);

        assert!(
            matches!(result, Ok(Some(FieldValue::SFNode(None)))),
            "{text:?}"
        );
        assert!(warnings.is_empty(), "{text:?}");
    }

    for text in ["<Group/><Transform/>", "", "[ ]", "<Bogus"] {
        let (result, warnings) = read(FieldType::MFNode, text, true);

        assert!(
            matches!(result, Ok(Some(FieldValue::MFNode(ref v))) if v.is_empty()),
            "{text:?}: {result:?}"
        );
        assert!(warnings.is_empty(), "{text:?}");
    }
}

#[test]
fn a_value_that_cannot_be_read_gives_no_warning() {
    // X_ITE 16.5.0 prints nothing before it throws `Couldn't read value for field ''.`
    for (ty, text, xml) in [
        (FieldType::SFVec3f, "1 2 x", false),
        (FieldType::SFInt32, "abc", false),
        (FieldType::SFBool, "x", true),
    ] {
        let (result, warnings) = read(ty, text, xml);

        assert!(matches!(result, Ok(None)), "{text}: {result:?}");
        assert!(warnings.is_empty(), "{text}");
    }
}

#[test]
fn the_warnings_of_a_value_that_stands_are_returned() {
    let (result, warnings) = read(FieldType::MFString, "a b", true);

    assert!(matches!(result, Ok(Some(FieldValue::MFString(ref v))) if v == &["a b"]));
    assert_eq!(warnings.len(), 1);
    assert!(warnings[0].contains("is an MFString array and can have multiple values"));
}
