use std::cell::RefCell;
use std::rc::Rc;
use x3d_runtime::{Fetcher, Runtime};

#[derive(Default)]
struct DummyFetch {
    calls: Rc<RefCell<Vec<String>>>,
}

impl Fetcher for DummyFetch {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        self.calls.borrow_mut().push(url.to_string());
        Err("Failed".into())
    }
}

fn rt(body: &str) -> (Runtime, Rc<RefCell<Vec<String>>>) {
    let text = format!("#VRML V2.0 utf8\n{body}");
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.wrl").unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let fetch = DummyFetch {
        calls: calls.clone(),
    };
    let mut rt = Runtime::new(scene, Box::new(fetch));
    rt.load_world();

    for _ in 0..10 {
        rt.run_tasks();
    }

    (rt, calls)
}

#[test]
fn test_script_url_paths() {
    // 1. Script node
    let (_, calls) = rt("Script { url \"\u{FEFF}javascript:\" }");
    assert!(calls.borrow().is_empty(), "U+FEFF script should not fetch");

    let (_, calls) = rt("Script { url \"\u{0085}javascript:\" }");
    assert!(!calls.borrow().is_empty(), "U+0085 script should fetch");

    // 2. EXTERNPROTO
    let (_, calls) = rt("EXTERNPROTO P [] \"\u{FEFF}javascript:\"\nP {}");
    assert!(
        calls.borrow().is_empty(),
        "U+FEFF externproto should not fetch"
    );

    let (_, calls) = rt("EXTERNPROTO P [] \"\u{0085}javascript:\"\nP {}");
    assert!(
        !calls.borrow().is_empty(),
        "U+0085 externproto should fetch"
    );

    // 3. Inline
    let (_, calls) = rt("Inline { url \"\u{FEFF}javascript:\" }");
    assert!(calls.borrow().is_empty(), "U+FEFF inline should not fetch");

    let (_, calls) = rt("Inline { url \"\u{0085}javascript:\" }");
    assert!(!calls.borrow().is_empty(), "U+0085 inline should fetch");
}

#[test]
fn test_browser_load() {
    let text = "#VRML V2.0 utf8\n";
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.wrl").unwrap();
    let calls = Rc::new(RefCell::new(Vec::new()));
    let fetch = DummyFetch {
        calls: calls.clone(),
    };
    let mut rt = Runtime::new(scene, Box::new(fetch));
    rt.load_world();

    rt.browser_create_from_url(
        vec!["\u{FEFF}javascript:".to_string()],
        x3d_scene::ROOT,
        None,
    );
    for _ in 0..10 {
        rt.run_tasks();
    }
    assert!(
        calls.borrow().is_empty(),
        "U+FEFF browser load should not fetch"
    );

    rt.browser_create_from_url(
        vec!["\u{0085}javascript:".to_string()],
        x3d_scene::ROOT,
        None,
    );
    for _ in 0..10 {
        rt.run_tasks();
    }
    assert!(
        !calls.borrow().is_empty(),
        "U+0085 browser load should fetch"
    );
}
