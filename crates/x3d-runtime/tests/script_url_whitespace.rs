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
    let scripts = ["javascript", "ecmascript", "vrmlscript"];

    let prefixes = ["\u{FEFF}", "\u{00A0}", "\u{2028}", "\u{3000}", " \t\n\r"];

    for script in scripts {
        for prefix in prefixes {
            let s = format!("{prefix}{script}:");

            // 1. Script node
            let (_, calls) = rt(&format!("Script {{ url \"{s}\" }}"));
            assert!(
                calls.borrow().is_empty(),
                "Prefix {prefix:?} {script} script should not fetch"
            );

            // 2. EXTERNPROTO
            let (_, calls) = rt(&format!("EXTERNPROTO P [] \"{s}\"\nP {{}}"));
            assert!(
                calls.borrow().is_empty(),
                "Prefix {prefix:?} {script} externproto should not fetch"
            );

            // 3. Inline
            let (_, calls) = rt(&format!("Inline {{ url \"{s}\" }}"));
            assert!(
                calls.borrow().is_empty(),
                "Prefix {prefix:?} {script} inline should not fetch"
            );

            // 4. browser_load (and preflight path tested since `browser_create_from_url` pushes a BrowserLoad task)
            let (mut browser_rt, calls) = rt("");
            browser_rt.browser_create_from_url(vec![s.clone()], x3d_scene::ROOT, None);
            for _ in 0..10 {
                browser_rt.run_tasks();
            }
            assert!(
                calls.borrow().is_empty(),
                "Prefix {prefix:?} {script} browser load should not fetch"
            );
        }

        let prefix = "\u{0085}";
        let s = format!("{prefix}{script}:");

        // 1. Script node
        let (_, calls) = rt(&format!("Script {{ url \"{s}\" }}"));
        assert!(
            !calls.borrow().is_empty(),
            "Prefix {prefix:?} {script} script should fetch"
        );
        assert!(
            calls.borrow().iter().any(|u| u.contains("javascript")
                || u.contains("ecmascript")
                || u.contains("vrmlscript")
                || u.contains("%C2%85")),
            "Should fetch exact url {}",
            s
        );

        // 2. EXTERNPROTO
        let (_, calls) = rt(&format!("EXTERNPROTO P [] \"{s}\"\nP {{}}"));
        assert!(
            !calls.borrow().is_empty(),
            "Prefix {prefix:?} {script} externproto should fetch"
        );
        assert!(
            calls.borrow().iter().any(|u| u.contains("javascript")
                || u.contains("ecmascript")
                || u.contains("vrmlscript")
                || u.contains("%C2%85")),
            "Should fetch exact url {}",
            s
        );

        // 3. Inline
        let (_, calls) = rt(&format!("Inline {{ url \"{s}\" }}"));
        assert!(
            !calls.borrow().is_empty(),
            "Prefix {prefix:?} {script} inline should fetch"
        );
        assert!(
            calls.borrow().iter().any(|u| u.contains("javascript")
                || u.contains("ecmascript")
                || u.contains("vrmlscript")
                || u.contains("%C2%85")),
            "Should fetch exact url {}",
            s
        );

        // 4. browser_load
        let (mut browser_rt, calls) = rt("");
        browser_rt.browser_create_from_url(vec![s.clone()], x3d_scene::ROOT, None);
        for _ in 0..10 {
            browser_rt.run_tasks();
        }
        assert!(
            !calls.borrow().is_empty(),
            "Prefix {prefix:?} {script} browser load should fetch"
        );
        assert!(
            calls.borrow().iter().any(|u| u.contains("javascript")
                || u.contains("ecmascript")
                || u.contains("vrmlscript")
                || u.contains("%C2%85")),
            "Should fetch exact url {}",
            s
        );
    }
}

#[test]
fn test_anchor_preflight() {
    // 5. Anchor.requestImmediateLoad: a script URL of an Anchor is not fetched (Runtime::anchor_load, the click path).
    let scripts = ["javascript", "ecmascript", "vrmlscript"];
    let prefixes = ["\u{FEFF}", "\u{00A0}", "\u{2028}", "\u{3000}", " \t\n\r"];

    let anchor_run = |url: &str| {
        let (mut browser_rt, calls) = rt("DEF A Anchor { }");
        let anchor = browser_rt
            .scene
            .get_named_node(browser_rt.scene.world, "A")
            .expect("DEF Anchor");

        calls.borrow_mut().clear();
        browser_rt.anchor_load(anchor, vec![url.to_string()], vec![]);

        for _ in 0..10 {
            browser_rt.run_tasks();
        }

        let fetched = calls.borrow().clone();
        fetched
    };

    for script in scripts {
        for prefix in prefixes {
            let s = format!("{prefix}{script}:");
            assert!(
                anchor_run(&s).is_empty(),
                "Prefix {prefix:?} {script} Anchor should not fetch"
            );
        }

        // Not JavaScript `\s`: the URL is a file to fetch.
        for prefix in ["\u{0085}", "\u{200B}", "\u{180E}"] {
            let s = format!("{prefix}{script}:");
            assert!(
                !anchor_run(&s).is_empty(),
                "Prefix {prefix:?} {script} Anchor should fetch"
            );
        }
    }
}
