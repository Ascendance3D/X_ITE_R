//! The platform-neutral part of the browser module: the world the page loads is parsed and owned by the Rust runtime,
//! its fields read and set like through X_ITE's SFNode, and a world that does not parse is an error.

use x3d_runtime::data::Data;
use x3d_web::session::{FieldInfo, Session};

const BOX: &[u8] = include_bytes!("../web/example/box.wrl");
const URL: &str = "http://example.test/example/box.wrl";
const NOW: f64 = 946_684_800.0;

/// Fails every fetch: the example world has no Inline, texture or EXTERNPROTO.
struct NoFetch;

impl x3d_runtime::Fetcher for NoFetch {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        Err(format!("no network in this test: {url}"))
    }
}

fn load(data: &[u8]) -> Result<Session, String> {
    Session::load(data, URL, Box::new(NoFetch), NOW, 512, 384)
}

#[test]
fn the_runtime_owns_the_loaded_world() {
    let s = load(BOX).expect("the example world loads");

    assert_eq!(s.world_url(), URL);

    let roots: Vec<String> = s
        .root_nodes()
        .into_iter()
        .map(|n| s.type_name(n.expect("no NULL root")).unwrap())
        .collect();

    assert_eq!(roots, ["Viewpoint", "Transform"]);

    let material = s.named_node("BoxMaterial").expect("DEF BoxMaterial");

    assert_eq!(s.type_name(material).as_deref(), Some("Material"));
    assert_eq!(s.node_name(material).as_deref(), Some("BoxMaterial"));
    assert!(s.field_names(material).iter().any(|f| f == "diffuseColor"));
    assert_eq!(s.named_node("NoSuchNode"), None);
}

#[test]
fn node_fields_give_the_nodes_of_the_scene_graph() {
    let s = load(BOX).unwrap();
    let transform = s.root_nodes()[1].unwrap();
    let shape = s.node_value(transform, "children").unwrap()[0].unwrap();
    let appearance = s.node_value(shape, "appearance").unwrap()[0].unwrap();

    assert_eq!(s.type_name(shape).as_deref(), Some("Shape"));
    assert_eq!(
        s.node_value(appearance, "material").unwrap(),
        [s.named_node("BoxMaterial")]
    );
    assert_eq!(
        s.field(shape, "appearance").unwrap().value,
        "Appearance { }"
    );
    assert!(s.node_value(shape, "bboxSize").is_err());
}

#[test]
fn fields_read_like_x3dfield_to_string() {
    let s = load(BOX).unwrap();
    let material = s.named_node("BoxMaterial").unwrap();
    let camera = s.named_node("Camera").unwrap();

    assert_eq!(
        s.field(material, "diffuseColor"),
        Ok(FieldInfo {
            type_name: "SFColor",
            access: "inputOutput",
            value: "0.2 0.4 0.9".to_string(),
        })
    );
    assert_eq!(s.field(camera, "position").unwrap().value, "0 0 6");
    assert_eq!(s.field(camera, "description").unwrap().value, "\"Front\"");
    assert_eq!(s.field(camera, "isBound").unwrap().access, "outputOnly");
    assert_eq!(s.field(camera, "isBound").unwrap().value, "TRUE");
    assert!(s.field(material, "noSuchField").is_err());
}

#[test]
fn a_field_update_is_stored_and_survives_the_next_frame() {
    let mut s = load(BOX).unwrap();
    let material = s.named_node("BoxMaterial").unwrap();

    s.set_field(material, "diffuseColor", "1 0 0").unwrap();
    assert_eq!(s.field(material, "diffuseColor").unwrap().value, "1 0 0");

    s.step(NOW + 0.1);
    assert_eq!(s.field(material, "diffuseColor").unwrap().value, "1 0 0");

    let frame = s.build_frame();

    assert!(!frame.items.is_empty(), "the Box is drawn");
}

#[test]
fn bad_updates_are_refused() {
    let mut s = load(BOX).unwrap();
    let material = s.named_node("BoxMaterial").unwrap();
    let camera = s.named_node("Camera").unwrap();

    assert!(
        s.set_field(camera, "isBound", "FALSE").is_err(),
        "outputOnly"
    );
    // X3DField.fromString reads CSS color names ("red"), but not these.
    assert!(s.set_field(material, "diffuseColor", "x y z").is_err());
    assert!(s.set_field(material, "diffuseColor", "1 0").is_err());
    assert!(s.set_field(material, "noSuchField", "1").is_err());
    assert!(s.set_field(u32::MAX, "diffuseColor", "1 0 0").is_err());
    assert_eq!(
        s.field(material, "diffuseColor").unwrap().value,
        "0.2 0.4 0.9"
    );
}

#[test]
fn a_world_that_does_not_parse_is_an_error() {
    assert!(load(b"#VRML V2.0 utf8\nTransform { children [ Box { size 1 1 } ").is_err());
    assert!(load(b"not a world").is_err());
}

/// The browser build has no Script engine yet (QuickJS does not build for wasm32-unknown-unknown): a Script fails to
/// load with an error in the log, and the rest of the world runs. This keeps the gap visible (web/API.md).
#[test]
fn a_script_does_not_run_and_says_so() {
    let world = b"#VRML V2.0 utf8
DEF M Material { diffuseColor 0 0 1 }
DEF S Script { eventOut SFColor c url \"javascript: function initialize () { c = new SFColor (1, 0, 0); }\" }
ROUTE S.c TO M.set_diffuseColor
Shape { appearance Appearance { material USE M } geometry Box { } }
";
    let mut s = load(world).unwrap();

    s.step(NOW + 0.1);

    let m = s.named_node("M").unwrap();

    assert_eq!(s.field(m, "diffuseColor").unwrap().value, "0 0 1");
    assert!(
        s.take_log()
            .iter()
            .any(|l| l.contains("Script") && l.contains("no script engine"))
    );
}

// --- field objects of the page (issue #137) -------------------------------------------------------------------------

const ROUTED: &[u8] = b"#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Scripting:1
DEF T Transform { }
DEF T2 Transform { }
DEF M Material { }
DEF E Script { field SFInt32 n 5 }
ROUTE T.translation_changed TO T2.set_translation
";

fn vec3(x: f64, y: f64, z: f64) -> Data {
    Data::List(vec![Data::Number(x), Data::Number(y), Data::Number(z)])
}

#[test]
fn a_field_object_of_the_page_sends_its_event_and_learns_the_routed_value() {
    let mut s = load(ROUTED).unwrap();
    let t = s.named_node("T").unwrap();
    let t2 = s.named_node("T2").unwrap();

    // The page holds objects of both fields.
    assert_eq!(
        s.field_data(t, "translation").unwrap(),
        (
            "SFVec3f",
            x3d_fields::access::INPUT_OUTPUT,
            vec3(0.0, 0.0, 0.0)
        )
    );
    s.field_data(t2, "translation").unwrap();

    s.set_field_data(t, "translation", &vec3(1.0, 2.0, 3.0))
        .unwrap();

    // The page's own object holds the value it sent: no change to report yet.
    assert_eq!(s.take_field_changes(), []);

    s.step(NOW + 0.1);

    // The route changed T2 in the frame; T is unchanged since the page set it.
    assert_eq!(
        s.take_field_changes(),
        [(t2, "translation".to_string(), vec3(1.0, 2.0, 3.0))]
    );
    assert_eq!(s.take_field_changes(), []);
}

#[test]
fn observed_fields_give_each_event_with_its_value_in_order() {
    let mut s = load(ROUTED).unwrap();
    let t = s.named_node("T").unwrap();
    let t2 = s.named_node("T2").unwrap();

    s.observe_field(t, "translation", true).unwrap();
    s.observe_field(t2, "set_translation", true).unwrap();

    s.set_field_data(t, "translation", &vec3(1.0, 0.0, 0.0))
        .unwrap();
    s.step(NOW + 0.1);
    s.set_field_data(t, "translation", &vec3(2.0, 0.0, 0.0))
        .unwrap();
    s.step(NOW + 0.2);

    let name = || "translation".to_string();

    assert_eq!(
        s.take_field_events(),
        [
            (t, name(), vec3(1.0, 0.0, 0.0)),
            (t2, name(), vec3(1.0, 0.0, 0.0)),
            (t, name(), vec3(2.0, 0.0, 0.0)),
            (t2, name(), vec3(2.0, 0.0, 0.0)),
        ]
    );

    s.observe_field(t2, "translation", false).unwrap();
    s.set_field_data(t, "translation", &vec3(3.0, 0.0, 0.0))
        .unwrap();
    s.step(NOW + 0.3);

    assert_eq!(s.take_field_events(), [(t, name(), vec3(3.0, 0.0, 0.0))]);
    assert!(s.observe_field(t, "noSuchField", true).is_err());
}

#[test]
fn field_definitions_hold_class_defaults_and_user_fields() {
    let mut s = load(ROUTED).unwrap();
    let m = s.named_node("M").unwrap();
    let e = s.named_node("E").unwrap();

    // A changed field does not change the class's definition.
    s.set_field_data(m, "transparency", &Data::Number(0.5))
        .unwrap();

    let defs = s.field_definitions(m);
    let transparency = defs.iter().find(|d| d.name == "transparency").unwrap();

    assert_eq!(transparency.type_name, "SFFloat");
    assert_eq!(transparency.default, Some(Data::Number(0.0)));
    assert_eq!(
        defs.iter()
            .find(|d| d.name == "diffuseColor")
            .unwrap()
            .default,
        Some(vec3(0.8, 0.8, 0.8))
    );

    let n = s
        .field_definitions(e)
        .into_iter()
        .find(|d| d.name == "n")
        .unwrap();

    assert_eq!((n.type_name, n.default), ("SFInt32", None));
    assert!(s.field_definitions(u32::MAX).is_empty());
}
