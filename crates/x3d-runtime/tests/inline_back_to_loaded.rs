//! An Inline whose url is set back to a scene it already loaded while another request is held initializes the Scripts
//! of that scene once (ADR 0014; pinned X_ITE 16.5.0, Inline.loadData aborts the older FileLoader). The fetcher is
//! asynchronous and the test gives the responses; the host counts the Script sources it creates (one per Script
//! initialize).

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::{FetchPoll, Fetcher, Runtime, ScriptCall, ScriptError, ScriptHost};

const BASE: &str = "http://example.test/w.wrl";
const A: &str = "http://example.test/a.wrl";
const B: &str = "http://example.test/b.wrl";

#[derive(Default)]
struct Net {
    responses: HashMap<String, Option<Vec<u8>>>,
}

#[derive(Clone, Default)]
struct TestFetcher(Rc<RefCell<Net>>);

impl TestFetcher {
    fn respond(&self, url: &str, body: &str) {
        self.0
            .borrow_mut()
            .responses
            .insert(url.to_string(), Some(body.as_bytes().to_vec()));
    }
}

impl Fetcher for TestFetcher {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        match self.0.borrow().responses.get(url.as_str()) {
            Some(Some(body)) => Ok(body.clone()),
            _ => panic!("read {url} before its response arrived"),
        }
    }

    fn is_async(&self) -> bool {
        true
    }

    fn poll(&self, url: &url::Url) -> FetchPoll {
        let mut net = self.0.borrow_mut();

        match net.responses.get(url.as_str()) {
            None => {
                net.responses.insert(url.to_string(), None);
                FetchPoll::Pending
            }
            Some(None) => FetchPoll::Pending,
            Some(Some(_)) => FetchPoll::Loaded,
        }
    }
}

type Sources = Rc<RefCell<Vec<String>>>;

/// Records the source of every Script context that is created.
struct CountHost(Sources);

impl ScriptHost for CountHost {
    fn create_context(
        &mut self,
        _rt: &mut Runtime,
        _node: NodeId,
        source: &str,
        callbacks: &[String],
    ) -> Result<Vec<bool>, ScriptError> {
        self.0.borrow_mut().push(source.to_string());
        Ok(vec![false; callbacks.len()])
    }

    fn call(&mut self, _: &mut Runtime, _: NodeId, _: ScriptCall) -> Result<(), ScriptError> {
        Ok(())
    }
}

fn scene_text(name: &str) -> String {
    format!(
        "#VRML V2.0 utf8\nDEF Root{name} Transform {{ children Script {{ url \"javascript: // {name}\" }} }}\n"
    )
}

fn settle(rt: &mut Runtime) {
    for _ in 0..6 {
        rt.process_events();
        rt.run_tasks();
        rt.resume_fetches();
        rt.process_phase("finishEvents");
    }
}

/// Set the url and run the fetches that are parked before the `loadData` event of the new request is processed, as a page
/// does when a response (or a task) comes between the url set and the end of the frame's events.
fn set_url(rt: &mut Runtime, url: &str) {
    let n = rt.scene.get_named_node(x3d_scene::ROOT, "Inl").unwrap();
    let loc = rt.scene.get_field(n, "url").unwrap();

    rt.set_value((n, loc), FieldValue::MFString(vec![url.to_string()]));
    rt.resume_fetches();
    settle(rt);
}

fn count(sources: &Sources, name: &str) -> usize {
    sources
        .borrow()
        .iter()
        .filter(|s| s.ends_with(&format!("// {name}")))
        .count()
}

#[test]
fn a_url_set_back_to_a_loaded_scene_while_another_is_held_initializes_its_scripts_once() {
    let text = format!("#VRML V2.0 utf8\nDEF Inl Inline {{ url \"{A}\" }}\n");
    let scene = x3d_parse::parse_bytes(text.as_bytes(), BASE).unwrap();
    let net = TestFetcher::default();
    let sources = Sources::default();
    let mut rt = Runtime::new(scene, Box::new(net.clone()));

    rt.script_host = Some(Box::new(CountHost(sources.clone())));
    rt.load_world();
    settle(&mut rt);
    net.respond(A, &scene_text("A"));
    settle(&mut rt);
    assert_eq!(count(&sources, "A"), 1, "{:?}", sources.borrow());

    // B is requested and held; the url goes back to A, which the runtime already holds.
    set_url(&mut rt, B);
    set_url(&mut rt, A);
    assert_eq!(count(&sources, "A"), 2, "{:?}", sources.borrow());

    net.respond(B, &scene_text("B"));
    settle(&mut rt);
    assert_eq!(count(&sources, "A"), 2, "{:?}", sources.borrow());
    assert_eq!(count(&sources, "B"), 0, "{:?}", sources.borrow());
}
