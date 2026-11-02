//! A Viewpoint the page made (createVrmlFromString of an external browser) and put into the root nodes of a scene that
//! replaces the world is bound there (BindableList.getBound skips only bindables of scenes that have an execution
//! context; the page's scenes have none). Compared with pinned X_ITE in `tests/browser/ui02.test.mjs`.

use x3d_web::session::Session;

const NOW: f64 = 946_684_800.0;

struct NoFetch;

impl x3d_runtime::Fetcher for NoFetch {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        Err(format!("no network in this test: {url}"))
    }
}

#[test]
fn a_page_made_viewpoint_in_the_root_nodes_binds_when_the_scene_replaces_the_world() {
    let world = Session::empty_world("http://example.test/", Box::new(NoFetch));
    let mut s = Session::begin(world, NOW, 64, 48);

    assert!(s.poll_load(NOW));

    let nodes = s
        .create_vrml_from_string("Viewpoint { position 3 4 5 }", "http://example.test/")
        .expect("parses");

    s.set_root_nodes(nodes).expect("root nodes");
    assert!(s.go_live(NOW + 1.0));
    s.step(NOW + 2.0);

    assert_eq!(s.encoding(), "SCRIPTED");
    assert_eq!(
        s.viewpoint_text().as_deref(),
        Some("<Viewpoint\n    position='3 4 5'/>\n")
    );
}
