//! The asynchronous loading of the browser module, with a fetcher whose responses the test gives: a world waits for
//! its Inline, texture and EXTERNPROTO responses (X_ITE's loadCount), tries the MFString candidates in order, and never
//! reads a response that has not arrived.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use x3d_runtime::{FetchPoll, Fetcher};
use x3d_web::session::Session;

const URL: &str = "http://example.test/worlds/main.wrl";
const NOW: f64 = 946_684_800.0;

/// The responses of the test, by URL: `None` while the request is in flight.
#[derive(Default)]
struct Network {
    responses: HashMap<String, Option<Result<Vec<u8>, String>>>,
    /// The URLs in the order their requests started.
    requested: Vec<String>,
    /// The URLs whose response the runtime read.
    read: Vec<String>,
}

#[derive(Clone, Default)]
struct TestFetcher(Rc<RefCell<Network>>);

impl TestFetcher {
    fn respond(&self, url: &str, response: Result<&[u8], &str>) {
        self.0.borrow_mut().responses.insert(
            url.to_string(),
            Some(response.map(<[u8]>::to_vec).map_err(str::to_string)),
        );
    }

    fn requested(&self) -> Vec<String> {
        self.0.borrow().requested.clone()
    }

    fn read(&self) -> Vec<String> {
        self.0.borrow().read.clone()
    }
}

/// The request URL: a fragment (an EXTERNPROTO's `#name`) is no part of it, as with the page's `fetch`.
fn without_fragment(url: &url::Url) -> String {
    let mut url = url.clone();

    url.set_fragment(None);
    url.into()
}

impl Fetcher for TestFetcher {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        let mut net = self.0.borrow_mut();
        let key = without_fragment(url);

        net.read.push(key.clone());

        match net.responses.get(&key) {
            Some(Some(response)) => response.clone(),
            _ => panic!("the runtime read {key} before its response arrived"),
        }
    }

    fn is_async(&self) -> bool {
        true
    }

    fn poll(&self, url: &url::Url) -> FetchPoll {
        let mut net = self.0.borrow_mut();
        let key = without_fragment(url);

        match net.responses.get(&key) {
            None => {
                net.requested.push(key.clone());
                net.responses.insert(key, None);
                FetchPoll::Pending
            }
            Some(None) => FetchPoll::Pending,
            Some(Some(Ok(_))) => FetchPoll::Loaded,
            Some(Some(Err(_))) => FetchPoll::Failed,
        }
    }
}

const INNER: &[u8] = b"#VRML V2.0 utf8\nDEF InnerBox Shape { geometry Box { } }\n";
const PROTOS: &[u8] =
    b"#VRML V2.0 utf8\nPROTO ColorBox [ exposedField SFColor color 1 0 0 ] { Shape { appearance Appearance { material Material { diffuseColor IS color } } geometry Box { } } }\n";

const PNG: &[u8] = include_bytes!("browser/worlds/tex/red.png");

fn begin(world: &str, fetcher: &TestFetcher) -> Session {
    let parsed =
        Session::parse(world.as_bytes(), URL, Box::new(fetcher.clone())).expect("the world parses");

    Session::begin(parsed, NOW, 400, 300)
}

/// Poll the load a few times, like the page does after each response.
fn poll(s: &mut Session) -> bool {
    (0..4).any(|i| s.poll_load(NOW + f64::from(i) * 0.016))
}

#[test]
fn the_world_waits_for_its_inline_and_texture() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url \"parts/inner.wrl\" }\nShape { appearance Appearance { texture ImageTexture { url \"tex/red.png\" } } geometry Box { } }\n",
        &net,
    );

    assert!(!poll(&mut s), "loaded before the responses");
    assert!(
        net.requested()
            .contains(&"http://example.test/worlds/parts/inner.wrl".to_string()),
        "{:?}",
        net.requested()
    );

    net.respond("http://example.test/worlds/parts/inner.wrl", Ok(INNER));
    // The texture's request may start only after the Inline's (X_ITE: both start at once; the order is free).
    poll(&mut s);
    assert!(
        net.requested()
            .contains(&"http://example.test/worlds/tex/red.png".to_string()),
        "{:?}",
        net.requested()
    );
    assert!(!s.is_loaded(), "loaded before the texture");

    net.respond("http://example.test/worlds/tex/red.png", Ok(PNG));
    assert!(poll(&mut s), "loaded after the responses");
    assert!(s.go_live(NOW));
    assert!(
        s.take_log().iter().all(|l| !l.contains("Couldn't load")),
        "no load errors"
    );
}

#[test]
fn a_failed_response_ends_the_wait_with_x_ite_s_message() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url \"parts/missing.wrl\" }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(
        "http://example.test/worlds/parts/missing.wrl",
        Err("Error: Not Found"),
    );
    assert!(poll(&mut s));

    let log = s.take_log().join("\n");

    assert!(
        log.contains(
            "Inline: Couldn't load URL 'http://example.test/worlds/parts/missing.wrl'. Error: Not Found"
        ),
        "{log}"
    );
}

#[test]
fn mfstring_candidates_are_tried_in_order() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url [ \"first.wrl\" \"second.wrl\" ] }\n",
        &net,
    );

    assert!(!poll(&mut s));
    assert_eq!(net.requested(), ["http://example.test/worlds/first.wrl"]);

    net.respond(
        "http://example.test/worlds/first.wrl",
        Err("Error: Internal Server Error"),
    );
    assert!(!poll(&mut s), "loaded before the second candidate");
    assert_eq!(
        net.requested(),
        [
            "http://example.test/worlds/first.wrl",
            "http://example.test/worlds/second.wrl"
        ]
    );

    net.respond("http://example.test/worlds/second.wrl", Ok(INNER));
    assert!(poll(&mut s));

    let log = s.take_log().join("\n");

    assert!(
        log.contains("Couldn't load URL 'http://example.test/worlds/first.wrl', trying URL 2 of 2. Error: Internal Server Error"),
        "{log}"
    );
}

#[test]
fn the_world_waits_for_an_externproto_file() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nEXTERNPROTO ColorBox [ exposedField SFColor color ] \"protos.wrl#ColorBox\"\nDEF Instance ColorBox { color 0 1 0 }\n",
        &net,
    );

    assert!(!poll(&mut s));
    assert_eq!(net.requested(), ["http://example.test/worlds/protos.wrl"]);

    net.respond("http://example.test/worlds/protos.wrl", Ok(PROTOS));
    assert!(poll(&mut s));
    assert!(s.go_live(NOW));

    let instance = s.named_node("Instance").expect("DEF Instance");

    assert_eq!(s.type_name(instance).as_deref(), Some("ColorBox"));
    assert_eq!(s.field(instance, "color").unwrap().value, "0 1 0");
}

#[test]
fn one_request_per_url_and_no_read_before_the_response() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url \"parts/inner.wrl\" }\nInline { url \"parts/inner.wrl\" }\n",
        &net,
    );

    for _ in 0..3 {
        assert!(!poll(&mut s));
    }

    assert_eq!(
        net.requested(),
        ["http://example.test/worlds/parts/inner.wrl"]
    );
    assert!(
        net.read().is_empty(),
        "read before the response: {:?}",
        net.read()
    );

    net.respond("http://example.test/worlds/parts/inner.wrl", Ok(INNER));
    assert!(poll(&mut s));
}

#[test]
fn a_world_without_resources_is_loaded_at_once() {
    let net = TestFetcher::default();
    let mut s = begin("#VRML V2.0 utf8\nShape { geometry Box { } }\n", &net);

    assert!(poll(&mut s));
    assert!(net.requested().is_empty());
    assert!(s.go_live(NOW));
    assert!(s.is_live());
}
