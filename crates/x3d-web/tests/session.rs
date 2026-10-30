//! The platform-neutral part of the browser module: the world the page loads is parsed and owned by the Rust runtime,
//! its fields read and set like through X_ITE's SFNode, and a world that does not parse is an error.

use x3d_fields::NodeId;
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

/// A page callback that throws ends X_ITE's event processing for good (X3DRoutingContext `_active` stays set): the later
/// frames still run, but no event is processed, so nothing reaches the page and no route moves.
#[test]
fn halted_events_stay_halted_and_later_frames_run() {
    let mut s = load(ROUTED).unwrap();
    let t = s.named_node("T").unwrap();
    let t2 = s.named_node("T2").unwrap();

    s.observe_field(t, "translation", true).unwrap();
    s.set_field_data(t, "translation", &vec3(1.0, 0.0, 0.0))
        .unwrap();
    s.step(NOW + 0.1);
    assert_eq!(s.take_field_events().len(), 1);

    s.halt_events();
    s.set_field_data(t, "translation", &vec3(2.0, 0.0, 0.0))
        .unwrap();
    s.step(NOW + 0.2);
    s.step(NOW + 0.3);

    assert_eq!(s.take_field_events(), []);
    // The route did not run: T2 still holds the value of the first event.
    assert_eq!(
        s.field_data(t2, "translation").unwrap().2,
        vec3(1.0, 0.0, 0.0)
    );
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

// --- released nodes (PR #147 review): a released node's id may name another node later ---------------------------

/// A world that gives the page a node of an Inline: the Inline's G exports its child IT, and a route moves G's
/// children into the world's O when the page adds a node to G (through Src).
fn inline_world() -> Vec<u8> {
    let inner = "#X3D V4.0 utf8\nDEF G Group { children DEF IT Transform { translation 1 2 3 } }\nEXPORT G\n";
    let url: String = inner
        .chars()
        .map(|c| match c {
            '#' => "%23".to_string(),
            ' ' => "%20".to_string(),
            '\n' => "%0A".to_string(),
            c => c.to_string(),
        })
        .collect();

    format!(
        "#X3D V4.0 utf8
DEF I Inline {{ url \"data:model/x3d-vrml,{url}\" }}
IMPORT I.G
DEF Src Group {{ }}
DEF O Group {{ }}
DEF X Transform {{ }}
ROUTE Src.children_changed TO G.addChildren
ROUTE G.children_changed TO O.set_children
"
    )
    .into_bytes()
}

/// Load the Inline world and step until O holds the Inline's IT; IT's id.
fn inline_node(s: &mut Session, t: &mut f64) -> NodeId {
    let x = s.named_node("X").unwrap();
    let src = s.named_node("Src").unwrap();
    let o = s.named_node("O").unwrap();

    for _ in 0..20 {
        *t += 0.1;
        s.step(*t);
    }

    s.set_field_data(
        src,
        "children",
        &Data::List(vec![Data::Number(f64::from(x))]),
    )
    .unwrap();

    for _ in 0..3 {
        *t += 0.1;
        s.step(*t);
    }

    let children = s.node_value(o, "children").unwrap();
    let it = children[0].expect("O holds the Inline's IT");

    assert_eq!(s.node_name(it).as_deref(), Some("IT"));
    it
}

/// Unload the Inline and empty O and Src: nothing refers to IT any more, the sweep releases it.
fn release_inline(s: &mut Session, t: &mut f64) {
    let i = s.named_node("I").unwrap();

    for name in ["Src", "O"] {
        let n = s.named_node(name).unwrap();

        s.set_field_data(n, "children", &Data::List(vec![]))
            .unwrap();
    }

    s.set_field_data(i, "load", &Data::Bool(false)).unwrap();

    for _ in 0..3 {
        *t += 0.1;
        s.step(*t);
    }
}

#[test]
fn a_released_node_s_queued_events_are_dropped() {
    let mut s = load(&inline_world()).unwrap();
    let mut t = NOW;
    let it = inline_node(&mut s, &mut t);
    let i = s.named_node("I").unwrap();

    s.take_released_nodes();
    s.observe_field(it, "translation", true).unwrap();
    s.take_field_events();

    // The event of IT and the release of IT happen in the same frame: the page takes the events after it.
    s.set_field_data(it, "translation", &vec3(5.0, 5.0, 5.0))
        .unwrap();

    for name in ["Src", "O"] {
        let n = s.named_node(name).unwrap();

        s.set_field_data(n, "children", &Data::List(vec![]))
            .unwrap();
    }

    s.set_field_data(i, "load", &Data::Bool(false)).unwrap();
    t += 0.1;
    s.step(t);

    assert!(s.type_name(it).is_none(), "the sweep released IT");
    assert!(s.take_released_nodes().contains(&it));
    assert_eq!(s.take_field_events(), []);
}

#[test]
fn a_released_id_that_names_a_new_node_is_reported_and_keeps_no_events() {
    let mut s = load(&inline_world()).unwrap();
    let mut t = NOW;
    let it = inline_node(&mut s, &mut t);
    let i = s.named_node("I").unwrap();

    s.observe_field(it, "translation", true).unwrap();
    release_inline(&mut s, &mut t);

    assert!(s.take_released_nodes().contains(&it));
    assert!(
        s.set_field_data(it, "translation", &vec3(1.0, 1.0, 1.0))
            .is_err()
    );

    // The Inline loads again: its new nodes take the released slots, IT's id among them.
    s.set_field_data(i, "load", &Data::Bool(true)).unwrap();

    for _ in 0..20 {
        t += 0.1;
        s.step(t);
    }

    assert!(
        s.type_name(it).is_some(),
        "the id of the old IT names a node of the new Inline scene"
    );

    // The new node is not observed: the page's callbacks of the old IT do not get its events.
    s.set_field_data(it, "children", &Data::List(vec![])).ok();
    s.set_field_data(it, "translation", &vec3(7.0, 7.0, 7.0))
        .ok();
    t += 0.1;
    s.step(t);
    assert_eq!(s.take_field_events(), []);
}

#[test]
fn field_data_of_the_wrong_shape_or_with_bad_node_ids_is_refused() {
    let mut s = load(ROUTED).unwrap();
    let t = s.named_node("T").unwrap();
    let m = s.named_node("M").unwrap();
    let n = |x: f64| Data::Number(x);
    let list = |v: Vec<Data>| Data::List(v);

    // Too few components, a string for a number, a number for a boolean.
    assert!(
        s.set_field_data(t, "translation", &list(vec![n(1.0), n(2.0)]))
            .is_err()
    );
    assert!(
        s.set_field_data(
            t,
            "translation",
            &list(vec![n(1.0), n(2.0), Data::String("3".into())])
        )
        .is_err()
    );
    assert!(s.set_field_data(t, "translation", &n(1.0)).is_err());
    assert!(
        s.set_field_data(m, "diffuseColor", &list(vec![n(1.0); 4]))
            .is_err()
    );
    assert!(s.set_field_data(t, "visible", &n(1.0)).is_err());

    // Node ids: out of range, fractional, negative, not a number; then a node that is not in the world.
    for bad in [
        n(1e12),
        n(1.5),
        n(-1.0),
        n(f64::NAN),
        Data::String("1".into()),
        n(f64::from(u32::MAX)),
    ] {
        assert!(
            s.set_field_data(t, "children", &list(vec![bad.clone()]))
                .is_err(),
            "{bad:?}"
        );
    }

    // Nothing was changed by the refused data.
    assert_eq!(s.field(t, "translation").unwrap().value, "0 0 0");
    assert!(s.node_value(t, "children").unwrap().is_empty());

    // The right shapes are taken.
    s.set_field_data(t, "translation", &vec3(1.0, 2.0, 3.0))
        .unwrap();
    s.set_field_data(t, "children", &list(vec![n(f64::from(m)), Data::Null]))
        .unwrap();
    assert_eq!(s.node_value(t, "children").unwrap(), [Some(m), None]);
}

#[test]
fn nodes_print_in_full_in_the_three_text_forms() {
    let s = load(BOX).unwrap();
    let material = s.named_node("BoxMaterial").unwrap();
    let node = Data::Number(f64::from(material));
    let print = |kind: &str| {
        s.print_nodes(kind, x3d_fields::FieldType::SFNode, &node)
            .unwrap()
    };

    assert!(print("vrml").starts_with("DEF BoxMaterial Material {\n  "));
    assert!(print("xml").starts_with("<Material DEF='BoxMaterial'"));
    assert!(print("json").starts_with("{ \"Material\":"));
    assert!(
        s.print_nodes("yaml", x3d_fields::FieldType::SFNode, &node)
            .is_err()
    );
}

#[test]
fn the_scene_prints_in_the_three_text_forms() {
    let s = load(BOX).unwrap();

    let vrml = s.print_scene("vrml").unwrap();
    assert!(vrml.starts_with("#X3D V4.1 utf8 X_ITE V16.5.0\n"), "{vrml}");
    assert!(vrml.contains("DEF BoxMaterial Material {"), "{vrml}");
    assert!(s.print_scene("xml").unwrap().starts_with("<?xml"));
    assert!(s.print_scene("json").unwrap().starts_with('{'));
    assert!(s.print_scene("yaml").is_err());
}

#[test]
fn the_empty_world_prints_without_a_profile() {
    let vrml = x3d_web::session::Session::print_empty_scene("vrml").unwrap();
    assert_eq!(vrml, "#X3D V4.1 utf8 X_ITE V16.5.0\n\n");
    assert!(
        x3d_web::session::Session::print_empty_scene("xml")
            .unwrap()
            .contains("profile='Full'")
    );
    assert!(x3d_web::session::Session::print_empty_scene("yaml").is_err());
}

#[test]
fn a_node_that_is_not_in_the_world_is_not_printed() {
    let s = load(BOX).unwrap();
    let list = Data::List(vec![Data::Number(f64::from(u32::MAX))]);

    assert!(
        s.print_nodes("vrml", x3d_fields::FieldType::MFNode, &list)
            .is_err()
    );
    assert!(
        s.print_nodes(
            "vrml",
            x3d_fields::FieldType::MFNode,
            &Data::List(vec![Data::String("x".into())])
        )
        .is_err()
    );
}

#[test]
fn a_released_node_is_not_printed() {
    let mut s = load(&inline_world()).unwrap();
    let mut t = NOW;
    let it = inline_node(&mut s, &mut t);

    assert!(
        s.print_nodes(
            "vrml",
            x3d_fields::FieldType::SFNode,
            &Data::Number(f64::from(it))
        )
        .is_ok()
    );

    release_inline(&mut s, &mut t);

    assert!(
        s.print_nodes(
            "vrml",
            x3d_fields::FieldType::SFNode,
            &Data::Number(f64::from(it))
        )
        .is_err(),
        "a released id is no node of the world"
    );
}

#[test]
fn values_read_from_text_make_their_nodes_in_the_world() {
    let mut s = load(BOX).unwrap();
    let before = s.root_nodes().len();
    let read = |s: &mut Session, ty, text: &str, xml| s.parse_field(ty, text, xml, "f", None);

    assert_eq!(
        read(&mut s, x3d_fields::FieldType::SFBool, "TRUE", false),
        Ok(Data::Bool(true))
    );
    assert_eq!(
        read(&mut s, x3d_fields::FieldType::SFBool, "x", false),
        Err("Couldn't read value for field 'f'.".to_string())
    );

    let node = read(&mut s, x3d_fields::FieldType::SFNode, "Group { }", false).unwrap();
    let Data::Number(id) = node else {
        panic!("a node id, got {node:?}");
    };

    assert_eq!(s.type_name(id as NodeId).as_deref(), Some("Group"));
    assert_eq!(s.root_nodes().len(), before, "the node is not in the scene");
    assert_eq!(
        read(&mut s, x3d_fields::FieldType::SFNode, "NULL", false),
        Ok(Data::Null)
    );
    assert!(read(&mut s, x3d_fields::FieldType::SFNode, "<Group/>", true).is_ok());
}

#[test]
fn only_node_field_types_are_printed() {
    let s = load(BOX).unwrap();
    let node = Data::Number(f64::from(s.named_node("BoxMaterial").unwrap()));

    for ty in [
        x3d_fields::FieldType::SFString,
        x3d_fields::FieldType::SFInt32,
        x3d_fields::FieldType::MFFloat,
    ] {
        let error = s.print_nodes("vrml", ty, &node).unwrap_err();

        assert!(error.contains("not a node field type"), "{error}");
    }
}

const LOOSE: &[u8] = b"#VRML V2.0 utf8
DEF A Transform { }
DEF B Transform { }
DEF C Transform { }
DEF G Group { }
DEF PI PositionInterpolator { key [ 0 1 ] keyValue [ 0 0 0, 1 1 1 ] }
ROUTE A.translation_changed TO B.set_translation
";

#[test]
fn a_route_added_by_the_page_carries_events_and_deleting_it_stops_them() {
    let mut s = load(LOOSE).unwrap();
    let (a, b, c) = (
        s.named_node("A").unwrap(),
        s.named_node("B").unwrap(),
        s.named_node("C").unwrap(),
    );

    let parsed = s.routes();
    assert_eq!(parsed.len(), 1);
    assert_eq!(
        (
            parsed[0].source,
            parsed[0].source_field.as_str(),
            parsed[0].destination,
            parsed[0].destination_field.as_str()
        ),
        (a, "translation_changed", b, "set_translation")
    );

    // Any spelling of an inputOutput field names the same fields; the route prints X_ITE's names.
    let route = s.add_route(a, "translation", c, "translation").unwrap();

    assert_eq!(route.source_field, "translation_changed");
    assert_eq!(route.destination_field, "set_translation");
    assert_eq!(route.raw_source_field, "translation");
    assert_eq!(s.routes().len(), 2);

    // The same route again is the route that exists.
    assert_eq!(
        s.add_route(a, "translation_changed", c, "set_translation")
            .unwrap(),
        route
    );
    assert_eq!(
        s.get_route(a, "translation", c, "translation"),
        Ok(Some(route.clone()))
    );
    assert_eq!(s.routes().len(), 2);

    s.field_data(c, "translation").unwrap();
    s.set_field_data(a, "translation", &vec3(1.0, 2.0, 3.0))
        .unwrap();
    s.step(NOW + 0.1);
    assert_eq!(
        s.take_field_changes(),
        [(c, "translation".to_string(), vec3(1.0, 2.0, 3.0))]
    );

    assert!(s.delete_route(route.id));
    assert!(!s.delete_route(route.id));
    assert_eq!(s.get_route(a, "translation", c, "translation"), Ok(None));

    s.set_field_data(a, "translation", &vec3(4.0, 5.0, 6.0))
        .unwrap();
    s.step(NOW + 0.2);
    assert_eq!(s.take_field_changes(), []);
    assert_eq!(s.routes().len(), 1);
}

#[test]
fn a_bad_route_is_an_error_with_the_text_x_ite_throws() {
    let mut s = load(LOOSE).unwrap();
    let (a, c, g, pi) = (
        s.named_node("A").unwrap(),
        s.named_node("C").unwrap(),
        s.named_node("G").unwrap(),
        s.named_node("PI").unwrap(),
    );

    let message = |r: Result<x3d_web::session::RouteInfo, String>| r.unwrap_err();

    assert_eq!(
        message(s.add_route(a, "nope", c, "translation")),
        "Unknown field 'nope' in node class Transform."
    );
    assert_eq!(
        message(s.add_route(a, "translation", c, "nope")),
        "Unknown field 'nope' in node class Transform."
    );
    assert_eq!(
        message(s.add_route(a, "translation", c, "rotation")),
        "Bad ROUTE specification: source field type must match destination field type of fields named \"translation\" and \"rotation\"."
    );
    assert_eq!(
        message(s.add_route(g, "addChildren", g, "removeChildren")),
        "Bad ROUTE specification: source field \"addChildren\" must be an output."
    );
    assert_eq!(
        message(s.add_route(a, "bboxSize", c, "bboxSize")),
        "Bad ROUTE specification: source field \"bboxSize\" must be an output."
    );
    assert_eq!(
        message(s.add_route(a, "translation", c, "bboxSize")),
        "Bad ROUTE specification: destination field \"bboxSize\" must be an input."
    );
    assert_eq!(
        message(s.add_route(pi, "value_changed", pi, "set_fraction")),
        "Bad ROUTE specification: source field type must match destination field type of fields named \"value_changed\" and \"set_fraction\"."
    );
    assert_eq!(
        s.get_route(a, "translation", c, "rotation").unwrap_err(),
        "Bad ROUTE specification: source field type must match destination field type of fields named \"translation\" and \"rotation\"."
    );

    // A node that is not in the world.
    assert!(s.add_route(9999, "translation", c, "translation").is_err());
    assert_eq!(s.routes().len(), 1);
}

#[test]
fn createvrmlfromstring_makes_nodes_of_the_world() {
    let mut s = load(BOX).unwrap();
    let roots = s.root_nodes().len();
    let nodes = s
        .create_vrml_from_string("DEF N Transform { translation 1 2 3 } Shape { }", URL)
        .expect("the text parses");
    let types: Vec<String> = nodes
        .iter()
        .map(|n| s.type_name(n.expect("no NULL root")).unwrap())
        .collect();

    assert_eq!(types, ["Transform", "Shape"]);

    // The nodes are the world's own, but not its root nodes, and a DEF name is the new scene's.
    assert_eq!(s.root_nodes().len(), roots);
    assert_eq!(s.named_node("N"), None);

    // Text that does not parse is an error, and the world is as before.
    let error = s
        .create_vrml_from_string("Transform {", URL)
        .expect_err("an open node");

    assert!(
        error.contains("Expected '}' at the end of node body."),
        "{error}"
    );
    assert_eq!(s.root_nodes().len(), roots);
}

/// Serves one VRML file at `parts/p.wrl` next to the example world; every other URL fails.
struct OneFile;

impl x3d_runtime::Fetcher for OneFile {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        if url.as_str() == "http://example.test/example/parts/p.wrl" {
            Ok(b"#VRML V2.0 utf8\nDEF P Transform { translation 1 2 3 }\nShape { }\n".to_vec())
        } else {
            Err("Error: Not Found".to_string())
        }
    }
}

#[test]
fn createvrmlfromurl_loads_into_the_world_runtime() {
    let mut s = Session::load(BOX, URL, Box::new(OneFile), NOW, 512, 384).unwrap();
    let roots = s.root_nodes().len();

    // The candidates resolve against the page's base URL, not the world's (`../parts/p.wrl` against the world's URL
    // would be http://example.test/parts/p.wrl, which fails); the first fails and is reported like FileLoader's.
    let request = s.create_x3d_from_url(
        vec!["missing.wrl".to_string(), "../parts/p.wrl".to_string()],
        "http://example.test/example/sub/page.html",
    );
    let missing = s.create_x3d_from_url(
        vec!["missing.wrl".to_string()],
        "http://example.test/example/sub/page.html",
    );

    assert!(
        s.take_scene_requests().is_empty(),
        "nothing settles before a frame"
    );

    let mut settled = vec![];

    for i in 1..10 {
        s.step(NOW + f64::from(i) * 0.1);
        settled.extend(s.take_scene_requests());
    }

    assert_eq!(settled.len(), 2, "{settled:?}");

    let (_, nodes) = settled.iter().find(|(id, _)| *id == request).unwrap();
    let nodes = nodes.as_ref().expect("the second candidate loads");
    let types: Vec<String> = nodes
        .iter()
        .map(|n| s.type_name(n.expect("no NULL root")).unwrap())
        .collect();

    // The nodes are the world's own (they can go into a field of it), but not its root nodes.
    assert_eq!(types, ["Transform", "Shape"]);
    assert_eq!(s.node_name(nodes[0].unwrap()).as_deref(), Some("P"));
    assert_eq!(s.root_nodes().len(), roots);

    let (_, failed) = settled.iter().find(|(id, _)| *id == missing).unwrap();

    assert_eq!(failed.as_ref().unwrap_err(), "Couldn't load X3D file.");

    let log = s.take_log().join("\n");

    assert!(
        log.contains(
            "Couldn't load URL 'http://example.test/example/sub/missing.wrl', trying URL 2 of 2."
        ),
        "{log}"
    );
}

#[test]
fn the_world_has_the_profile_and_components_of_its_statements() {
    let s = load(
        b"#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT Text:1\nCOMPONENT Shape:2\nGroup { }\n",
    )
    .unwrap();

    assert_eq!(
        s.profile_and_components(),
        (
            Some("Interchange".to_string()),
            vec![("Text".to_string(), 1), ("Shape".to_string(), 2)]
        )
    );
}

/// X3DScene.rootNodes of the page (issue #137): `rootNodes .assign (list)` changes the world at once, and the `rootNodes`
/// event comes in the order of the events, after a node field set before it and before one set after it.
#[test]
fn the_root_nodes_of_the_page_change_the_world_and_send_their_event_in_order() {
    use x3d_web::session::ROOT_NODES_EVENT;

    let mut s = load(ROUTED).unwrap();
    let t = s.named_node("T").unwrap();
    let t2 = s.named_node("T2").unwrap();
    let m = s.named_node("M").unwrap();
    let roots = s.root_nodes();
    let ids = |list: &[Option<NodeId>]| {
        Data::List(
            list.iter()
                .map(|n| n.map_or(Data::Null, |n| Data::Number(f64::from(n))))
                .collect(),
        )
    };

    s.observe_field(t, "translation", true).unwrap();
    s.observe_root_nodes(true);

    // A node field, then the root nodes, in one frame.
    s.set_field_data(t, "translation", &vec3(1.0, 0.0, 0.0))
        .unwrap();

    let mut more = roots.clone();
    more.push(Some(m));
    more.push(None);
    s.set_root_nodes(more.clone()).unwrap();

    // The world holds the list at once: the scene prints it.
    assert_eq!(s.root_nodes(), more);
    assert!(s.print_scene("vrml").unwrap().contains("USE M"));

    s.step(NOW + 0.1);

    let name = || "translation".to_string();

    assert_eq!(
        s.take_field_events(),
        [
            (t, name(), vec3(1.0, 0.0, 0.0)),
            (ROOT_NODES_EVENT, "rootNodes".to_string(), ids(&more)),
        ]
    );

    // Two changes before a frame are one event, with the value at the time of the callbacks.
    s.set_root_nodes(vec![Some(t)]).unwrap();
    s.set_root_nodes(vec![Some(t2), Some(t)]).unwrap();
    s.step(NOW + 0.2);
    assert_eq!(
        s.take_field_events(),
        [(
            ROOT_NODES_EVENT,
            "rootNodes".to_string(),
            ids(&[Some(t2), Some(t)])
        )]
    );

    // Not observed: no event. A node id that is not in the world is refused and changes nothing.
    s.observe_root_nodes(false);
    s.set_root_nodes(vec![]).unwrap();
    s.step(NOW + 0.3);
    assert_eq!(s.take_field_events(), []);
    assert!(s.set_root_nodes(vec![Some(100_000)]).is_err());
    assert_eq!(s.root_nodes(), []);
}
