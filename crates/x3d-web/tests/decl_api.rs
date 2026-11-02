//! The PROTO and EXTERNPROTO declaration API of a scene (issue #137): the operations behind `X3DScene`'s
//! `getProtoDeclaration`, `addProtoDeclaration`, `updateProtoDeclaration`, `removeProtoDeclaration`, the EXTERNPROTO
//! forms, `getUniqueProtoName`, and the declaration objects (interface, body, `newInstance`, text forms).
//! The error texts are X_ITE 16.5.0's (`X3DExecutionContext.js`).

use x3d_runtime::DeclOp;
use x3d_web::session::Session;

const NOW: f64 = 946_684_800.0;

struct NoFetch;

impl x3d_runtime::Fetcher for NoFetch {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        Err(format!("no network in this test: {url}"))
    }
}

const WORLD: &[u8] = b"#VRML V2.0 utf8
PROTO Pa [ field SFFloat size 2 inputOutput SFString label \"x\" ] { Group { children Shape { geometry Sphere { radius IS size } } } }
PROTO Pb [ ] { Group { } }
EXTERNPROTO Ea [ field SFFloat size ] \"urn:nope#Ea\"
Group { }
";

fn load() -> Session {
    Session::load(
        WORLD,
        "http://example.test/w.wrl",
        Box::new(NoFetch),
        NOW,
        64,
        48,
    )
    .expect("loads")
}

#[test]
fn lists_and_lookups() {
    let s = load();
    let protos = s.decl_ids(false);

    assert_eq!(protos.len(), 2);
    assert_eq!(s.decl_info(false, protos[0]).unwrap().name, "Pa");
    assert_eq!(s.decl_get(false, "Pb"), Ok(protos[1]));
    assert_eq!(
        s.decl_get(false, "Nope"),
        Err("Proto declaration 'Nope' not found.".into())
    );
    assert_eq!(
        s.decl_get(true, "Nope"),
        Err("Extern proto declaration 'Nope' not found.".into())
    );
    assert_eq!(s.decl_ids(true).len(), 1);
    assert_eq!(s.decl_unique_name(false, "Pa"), "Pa_1");
    assert_eq!(s.decl_unique_name(false, "Free"), "Free");
    assert_eq!(s.decl_unique_name(true, "Ea"), "Ea_1");
}

#[test]
fn the_interface_has_metadata_first() {
    let s = load();
    let info = s
        .decl_info(false, s.decl_get(false, "Pa").unwrap())
        .unwrap();
    let names: Vec<_> = info
        .fields
        .iter()
        .map(|f| (f.name.as_str(), f.type_name))
        .collect();

    assert_eq!(
        names,
        [
            ("metadata", "SFNode"),
            ("size", "SFFloat"),
            ("label", "SFString")
        ]
    );
    assert!(!info.is_extern);
    assert!(s.decl_body(s.decl_get(false, "Pa").unwrap()).is_some());
}

#[test]
fn add_update_remove_and_their_errors() {
    let mut s = load();
    let pa = s.decl_get(false, "Pa").unwrap();
    let pb = s.decl_get(false, "Pb").unwrap();

    assert_eq!(
        s.decl_op(false, DeclOp::Add, "Pa", Some(pa)),
        Err("Couldn't add proto declaration: proto 'Pa' already in use.".into())
    );
    assert_eq!(
        s.decl_op(false, DeclOp::Add, "Pc", None),
        Err("Couldn't add proto declaration: proto must be of type X3DProtoDeclaration.".into())
    );
    assert_eq!(
        s.decl_op(false, DeclOp::Update, "Pc", Some(usize::MAX)),
        Err(
            "Couldn't update proto declaration: proto does not belong to this execution context."
                .into()
        )
    );
    assert_eq!(
        s.decl_op(false, DeclOp::Update, "", Some(pb)),
        Err("Couldn't update proto declaration: proto name is empty.".into())
    );

    s.decl_op(false, DeclOp::Update, "Renamed", Some(pb))
        .unwrap();
    assert_eq!(s.decl_get(false, "Renamed"), Ok(pb));
    assert!(s.decl_get(false, "Pb").is_err());

    s.decl_op(false, DeclOp::Remove, "Renamed", None).unwrap();
    assert_eq!(s.decl_ids(false), [pa]);

    s.decl_op(false, DeclOp::Add, "Again", Some(pb)).unwrap();
    assert_eq!(s.decl_info(false, pb).unwrap().name, "Again");
    assert_eq!(
        s.decl_op(false, DeclOp::Add, "Other", Some(pb)),
        Err("Couldn't add proto declaration: proto 'Again' already added.".into())
    );
}

#[test]
fn extern_protos_report_their_load_state() {
    let mut s = load();
    let ea = s.decl_get(true, "Ea").unwrap();
    let info = s.decl_info(true, ea).unwrap();

    assert!(info.is_extern);
    assert_eq!(info.urls.as_deref(), Some(&["urn:nope#Ea".to_string()][..]));
    assert!(info.load_state.is_some());
    assert!(s.extern_url_op(ea, "loadNow", vec![], 0.0));
    assert!(!s.extern_url_op(ea, "nonsense", vec![], 0.0));
    assert!(!s.extern_url_op(99, "loadNow", vec![], 0.0));
}

#[test]
fn instances_and_text_forms() {
    let mut s = load();
    let pa = s.decl_get(false, "Pa").unwrap();
    let node = s.decl_new_instance(false, pa).expect("an instance");

    assert_eq!(s.type_name(node).as_deref(), Some("Pa"));

    let vrml = s.decl_string(false, pa, "vrml").expect("vrml");

    assert!(vrml.starts_with("PROTO Pa"), "{vrml}");
    assert!(
        s.decl_string(false, pa, "xml")
            .expect("xml")
            .contains("ProtoDeclare")
    );

    s.decl_add_user_field(
        pa,
        "extra",
        3,
        "SFBool",
        &x3d_runtime::data::Data::Bool(true),
    )
    .unwrap();
    assert!(
        s.decl_info(false, pa)
            .unwrap()
            .fields
            .iter()
            .any(|f| f.name == "extra")
    );
    s.decl_remove_user_field(pa, "extra");
    assert!(
        !s.decl_info(false, pa)
            .unwrap()
            .fields
            .iter()
            .any(|f| f.name == "extra")
    );
    assert!(s.decl_info(false, 99).is_none());
}

#[test]
fn an_index_that_is_not_in_the_world_is_an_error_not_a_panic() {
    let mut s = load();
    let past = 999;
    let value = x3d_runtime::data::Data::Number(1.0);

    assert!(
        s.decl_add_user_field(past, "f", 3, "SFFloat", &value)
            .is_err()
    );
    assert!(s.decl_remove_user_field(past, "f").is_err());
    assert!(s.decl_string(false, past, "vrml").is_err());
    assert!(s.decl_string(true, past, "vrml").is_err());
    assert!(s.decl_request_update(false, past).is_err());
    assert!(s.decl_update_instances(true, past).is_err());
}

#[test]
fn removing_an_extern_proto_goes_through_the_cancelling_removal() {
    let mut s = load();
    let ea = s.decl_get(true, "Ea").expect("Ea");

    s.decl_op(true, DeclOp::Remove, "Ea", Some(ea))
        .expect("removed");
    assert!(s.decl_get(true, "Ea").is_err());
}

#[test]
fn an_interface_change_keeps_the_is_references_of_the_body_on_their_fields() {
    let mut s = load();
    let pa = s.decl_get(false, "Pa").unwrap();
    let value = x3d_runtime::data::Data::Number(9.0);

    // `label` comes before `size`: removing it moves `size`, and the body's `radius IS size` follows it.
    s.decl_remove_user_field(pa, "label").unwrap();
    assert!(
        s.decl_string(false, pa, "vrml")
            .unwrap()
            .contains("radius IS size"),
        "after a field before it was removed"
    );

    // A field of the same name replaces the old one at the end; the reference stays on `size`.
    s.decl_add_user_field(pa, "size", 3, "SFFloat", &value)
        .unwrap();
    s.decl_add_user_field(pa, "other", 3, "SFFloat", &value)
        .unwrap();
    assert!(
        s.decl_string(false, pa, "vrml")
            .unwrap()
            .contains("radius IS size")
    );

    // A reference to a field that is gone ends with it.
    s.decl_remove_user_field(pa, "size").unwrap();
    assert!(
        !s.decl_string(false, pa, "vrml")
            .unwrap()
            .contains("radius IS")
    );
}
