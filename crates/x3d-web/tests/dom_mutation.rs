//! The engine side of DOMIntegration (issue #146, part 2): an element added below a node is parsed into the node's field,
//! an attribute is read into a field with its event, and a disposed node leaves the names, the routes and the fields that
//! hold it. The page side is compared with pinned X_ITE in `tests/browser/dom02.test.mjs`.

use x3d_web::session::Session;

const NOW: f64 = 946_684_800.0;

struct NoFetch;

impl x3d_runtime::Fetcher for NoFetch {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        Err(format!("no network in this test: {url}"))
    }
}

const WORLD: &[u8] = b"<X3D profile='Full'><Scene>
<Transform DEF='A'><Group DEF='G'/></Transform>
<Transform DEF='B'/>
<ROUTE fromNode='A' fromField='translation_changed' toNode='B' toField='set_translation'/>
</Scene></X3D>";

fn load() -> Session {
    Session::load(
        WORLD,
        "http://example.test/w.x3d",
        Box::new(NoFetch),
        NOW,
        64,
        48,
    )
    .expect("loads")
}

fn text(s: &mut Session) -> String {
    s.print_scene("vrml").expect("prints")
}

#[test]
fn the_parse_reports_what_each_element_made() {
    let s = load();
    let t = s.xml_targets();

    // X3D, Scene, Transform A, Group G, Transform B, ROUTE: the elements are numbered in document order.
    let kinds: Vec<(u32, u32)> = t.chunks(4).map(|q| (q[0], q[1])).collect();

    assert_eq!(kinds, [(1, 1), (2, 0), (3, 0), (4, 0), (5, 2)]);
}

#[test]
fn an_added_element_joins_its_parent() {
    let mut s = load();
    let g = s.named_node("G").unwrap();
    let made = s
        .xml_child(Some(g), None, "<Transform DEF='C' translation='1 2 3'/>")
        .expect("parses");

    assert_eq!(made.len(), 4, "one element made one node");
    assert!(s.named_node("C").is_some());
    assert!(text(&mut s).contains("DEF C Transform"));

    // Without a parent the node is a root node of the world.
    s.xml_child(None, None, "<Group DEF='R'/>").expect("parses");
    assert_eq!(s.root_nodes().len(), 3);
}

#[test]
fn an_added_route_is_connected() {
    let mut s = load();

    s.xml_child(None, None, "<Transform DEF='C'/>").unwrap();
    s.xml_child(
        None,
        None,
        "<ROUTE fromNode='A' fromField='translation_changed' toNode='C' toField='set_translation'/>",
    )
    .unwrap();

    let a = s.named_node("A").unwrap();

    s.xml_attribute(a, "translation", "4 5 6");
    s.step(NOW + 0.1);

    assert_eq!(text(&mut s).matches("translation 4 5 6").count(), 3);
}

#[test]
fn an_attribute_is_read_with_its_event() {
    let mut s = load();
    let a = s.named_node("A").unwrap();

    s.xml_attribute(a, "translation", "7 8 9");
    s.xml_attribute(a, "nothing", "1");
    s.xml_attribute(a, "translation", "a b c");
    s.step(NOW + 0.1);

    // The route carried the event to B, and a value that does not read changed nothing.
    assert_eq!(text(&mut s).matches("translation 7 8 9").count(), 2);
}

#[test]
fn a_disposed_node_leaves_everything_that_holds_it() {
    let mut s = load();
    let (a, b, g) = (
        s.named_node("A").unwrap(),
        s.named_node("B").unwrap(),
        s.named_node("G").unwrap(),
    );

    s.dispose_node(g);
    assert!(s.named_node("G").is_none());
    assert!(!text(&mut s).contains("Group"), "left the children of A");

    s.dispose_node(b);
    assert!(s.routes().is_empty(), "its routes are gone");
    assert_eq!(s.root_nodes(), [Some(a)]);
}

#[test]
fn a_removed_scene_element_disposes_its_context() {
    let mut s = load();

    s.dispose_context(0);

    assert!(s.named_node("A").is_none() && s.named_node("B").is_none());
    assert!(s.routes().is_empty());
    assert!(s.root_nodes().is_empty());
}

#[test]
fn an_inline_reports_its_load_state_and_its_document() {
    let data = "data:model/x3d+xml,%3CX3D%20profile%3D%27Full%27%3E%3CScene%3E%3CGroup%20DEF%3D%27In%27%2F%3E%3C%2FScene%3E%3C%2FX3D%3E";
    let world = format!(
        "<X3D profile='Full'><Scene><Inline DEF='I' url='\"{data}\"'/><Inline DEF='J' load='false' url='\"{data}\"'/></Scene></X3D>"
    );
    let mut s = Session::load(
        world.as_bytes(),
        "http://example.test/w.x3d",
        Box::new(NoFetch),
        NOW,
        64,
        48,
    )
    .expect("loads");

    for i in 0..50 {
        s.poll_load(NOW + f64::from(i));
        s.step(NOW + f64::from(i));
    }

    let (i, j) = (s.named_node("I").unwrap(), s.named_node("J").unwrap());

    assert_eq!(s.load_state(i), Some(2), "COMPLETE_STATE");
    assert_eq!(s.load_state(j), Some(0), "NOT_STARTED_STATE");
    assert!(s.inline_xml(j).is_none());

    let (xml, targets) = s.inline_xml(i).expect("the Inline's document");

    assert!(xml.contains("DEF='In'"), "{xml}");
    assert!(!targets.is_empty());
}
