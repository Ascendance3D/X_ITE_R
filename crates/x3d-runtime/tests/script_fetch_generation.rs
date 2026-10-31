//! A Script's fetch that completes after `unloadData` (a reload, `load` FALSE, close) or for a later load does nothing
//! (issue #133). The fetcher is asynchronous and the test gives the responses; the host counts the sources it runs.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use x3d_fields::{FieldValue, NodeId};
use x3d_runtime::{
    FetchPoll, Fetcher, Runtime, ScriptCall, ScriptError, ScriptHost, nodes::LoadState,
};

const BASE: &str = "http://example.test/w.wrl";

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

/// The sources that `create_context` ran, in order.
type Sources = Rc<RefCell<Vec<String>>>;

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

fn start(world: &str) -> (Runtime, TestFetcher, Sources) {
    let text = format!("#VRML V2.0 utf8\n{world}");
    let scene = x3d_parse::parse_bytes(text.as_bytes(), BASE).unwrap();
    let net = TestFetcher::default();
    let sources = Sources::default();
    let mut rt = Runtime::new(scene, Box::new(net.clone()));

    rt.script_host = Some(Box::new(CountHost(sources.clone())));
    rt.load_world();
    settle(&mut rt);
    (rt, net, sources)
}

fn settle(rt: &mut Runtime) {
    for _ in 0..6 {
        rt.process_events();
        rt.run_tasks();
        rt.resume_fetches();
    }
}

fn node(rt: &Runtime, name: &str) -> NodeId {
    rt.scene.get_named_node(x3d_scene::ROOT, name).unwrap()
}

fn set(rt: &mut Runtime, name: &str, field: &str, value: FieldValue) {
    let n = node(rt, name);
    let loc = rt.scene.get_field(n, field).unwrap();

    rt.set_value((n, loc), value);
    settle(rt);
}

fn state(rt: &Runtime, name: &str) -> LoadState {
    rt.url_load_state(node(rt, name))
}

fn url(s: &str) -> FieldValue {
    FieldValue::MFString(vec![s.to_string()])
}

const A: &str = "http://example.test/a.js";
const B: &str = "http://example.test/b.js";

#[test]
fn a_response_after_load_false_is_dropped() {
    let (mut rt, net, sources) = start("DEF S Script { url \"a.js\" }");

    assert_eq!(state(&rt, "S"), LoadState::InProgress);
    set(&mut rt, "S", "load", FieldValue::SFBool(false));
    net.respond(A, "// a");
    settle(&mut rt);

    assert!(sources.borrow().is_empty(), "{:?}", sources.borrow());
    assert_ne!(state(&rt, "S"), LoadState::Complete);
}

#[test]
fn a_response_after_the_scene_of_the_script_closed_is_dropped() {
    let (mut rt, net, sources) = start("DEF I Inline { url \"in.wrl\" }");

    net.respond(
        "http://example.test/in.wrl",
        "#VRML V2.0 utf8\nDEF S Script { url \"a.js\" }",
    );
    settle(&mut rt);

    let inner = (0..rt.scene.nodes.len() as NodeId)
        .find(|&n| rt.builtin_name(n) == Some("Script"))
        .expect("the Script of the Inline");

    // The Inline unloads its scene: the Script is closed while it is alive and its fetch still waits.
    set(&mut rt, "I", "load", FieldValue::SFBool(false));
    net.respond(A, "// a");
    settle(&mut rt);

    assert!(sources.borrow().is_empty(), "{:?}", sources.borrow());
    assert_ne!(rt.url_load_state(inner), LoadState::Complete);
}

#[test]
fn an_old_response_does_not_touch_a_later_load() {
    let (mut rt, net, sources) = start("DEF S Script { url \"a.js\" }");

    // A reload with another url while the first fetch waits.
    set(&mut rt, "S", "url", url("b.js"));
    net.respond(A, "// a");
    settle(&mut rt);

    assert!(sources.borrow().is_empty(), "{:?}", sources.borrow());
    assert_eq!(state(&rt, "S"), LoadState::InProgress);

    net.respond(B, "// b");
    settle(&mut rt);

    assert_eq!(*sources.borrow(), ["// b"]);
    assert_eq!(state(&rt, "S"), LoadState::Complete);
}

#[test]
fn a_response_of_an_earlier_load_of_the_same_url_runs_no_second_source() {
    let (mut rt, net, sources) = start("DEF S Script { url \"a.js\" }");

    // Off and on again with the same url: the request of the first load is not the second's.
    set(&mut rt, "S", "load", FieldValue::SFBool(false));
    set(&mut rt, "S", "load", FieldValue::SFBool(true));
    net.respond(A, "// a");
    settle(&mut rt);

    assert_eq!(*sources.borrow(), ["// a"], "one source runs, once");
}
