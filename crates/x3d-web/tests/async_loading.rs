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
        // A data URL needs no request: the page's fetcher (WebFetcher) decodes it itself.
        if url.scheme() == "data" {
            return x3d_parse::decode_data_url(url.as_str());
        }

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
        if url.scheme() == "data" {
            return match x3d_parse::decode_data_url(url.as_str()) {
                Ok(_) => FetchPoll::Loaded,
                Err(_) => FetchPoll::Failed,
            };
        }

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

#[test]
fn foreign_anchor_targets_are_opened_not_fetched() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nDEF Out Anchor { url \"http://other.test/world.wrl\" parameter \"target=_blank\" }\nDEF Page Anchor { url \"page.html\" }\n",
        &net,
    );

    assert!(poll(&mut s));
    assert!(s.go_live(NOW));

    for (name, urls, parameters) in [
        (
            "Out",
            "http://other.test/world.wrl",
            vec!["target=_blank".to_string()],
        ),
        ("Page", "page.html", vec![]),
    ] {
        let anchor = s.named_node(name).expect("DEF Anchor");
        let v = s.viewer().expect("live");

        v.rt.anchor_load(anchor, vec![urls.to_string()], parameters);

        for i in 1..4 {
            s.step(NOW + f64::from(i) * 0.016);
        }
    }

    assert!(net.requested().is_empty(), "fetched: {:?}", net.requested());
    assert_eq!(
        s.take_foreign(),
        [
            (
                "_blank".to_string(),
                "http://other.test/world.wrl".to_string()
            ),
            (
                String::new(),
                "http://example.test/worlds/page.html".to_string()
            ),
        ]
    );
}

#[test]
fn a_failure_does_not_end_the_wait_for_another_resource() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url \"parts/missing.wrl\" }\nInline { url \"parts/inner.wrl\" }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(
        "http://example.test/worlds/parts/missing.wrl",
        Err("Error: Not Found"),
    );
    assert!(!poll(&mut s), "loaded while the second Inline waits");
    assert!(!s.is_loaded());

    net.respond("http://example.test/worlds/parts/inner.wrl", Ok(INNER));
    assert!(poll(&mut s), "loaded after the second response");
    assert!(s.go_live(NOW));

    let log = s.take_log().join("\n");

    assert_eq!(log.matches("Couldn't load URL").count(), 1, "{log}");
}

// --- fallback after a response that does not load (FileLoader.loadDocumentError) --------------------------------

const GARBAGE: &[u8] = b"\x00\x01\x02 this is no file format \xff\xfe";

/// A glTF 2.0 triangle whose positions are in the external buffers `uris` (36 bytes each; the first is used).
fn gltf(uris: &[&str]) -> Vec<u8> {
    let buffers: Vec<String> = uris
        .iter()
        .map(|u| format!("{{\"uri\":\"{u}\",\"byteLength\":36}}"))
        .collect();

    format!(
        "{{\"asset\":{{\"version\":\"2.0\"}},\"buffers\":[{}],\"bufferViews\":[{{\"buffer\":0,\"byteLength\":36}}],\"accessors\":[{{\"bufferView\":0,\"componentType\":5126,\"count\":3,\"type\":\"VEC3\",\"min\":[0,0,0],\"max\":[1,1,0]}}],\"meshes\":[{{\"primitives\":[{{\"attributes\":{{\"POSITION\":0}}}}]}}],\"nodes\":[{{\"mesh\":0}}],\"scenes\":[{{\"nodes\":[0]}}],\"scene\":0}}",
        buffers.join(",")
    )
    .into_bytes()
}

fn triangle() -> Vec<u8> {
    [0.0f32, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0, 0.0]
        .iter()
        .flat_map(|f| f.to_le_bytes())
        .collect()
}

const W: &str = "http://example.test/worlds/";

fn w(path: &str) -> String {
    format!("{W}{path}")
}

#[test]
fn a_response_that_does_not_parse_moves_on_to_the_next_candidate() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url [ \"bad.wrl\" \"second.wrl\" \"third.wrl\" ] }\n",
        &net,
    );

    assert!(!poll(&mut s));
    assert_eq!(net.requested(), [w("bad.wrl")]);

    net.respond(&w("bad.wrl"), Ok(GARBAGE));
    assert!(!poll(&mut s), "loaded before the second candidate");
    assert_eq!(net.requested(), [w("bad.wrl"), w("second.wrl")]);

    net.respond(&w("second.wrl"), Ok(INNER));
    assert!(poll(&mut s));
    assert_eq!(
        net.requested(),
        [w("bad.wrl"), w("second.wrl")],
        "the third candidate is not fetched"
    );
    assert_eq!(net.read(), [w("bad.wrl"), w("second.wrl")]);

    let log = s.take_log().join("\n");

    assert!(
        log.contains(&format!(
            "Inline: Couldn't load URL '{}', trying URL 2 of 3.",
            w("bad.wrl")
        )),
        "{log}"
    );
    assert_eq!(log.matches("Couldn't load").count(), 1, "{log}");
}

#[test]
fn failures_of_each_kind_before_the_candidate_that_loads() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url [ \"a.wrl\" \"b.wrl\" \"c.png\" \"d.wrl\" ] }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(&w("a.wrl"), Err("Error: Internal Server Error"));
    assert!(!poll(&mut s));
    net.respond(&w("b.wrl"), Ok(GARBAGE));
    assert!(!poll(&mut s));
    // An image is no scene: no parser takes it.
    net.respond(&w("c.png"), Ok(PNG));
    assert!(!poll(&mut s));
    net.respond(&w("d.wrl"), Ok(INNER));
    assert!(poll(&mut s));

    assert_eq!(
        net.requested(),
        [w("a.wrl"), w("b.wrl"), w("c.png"), w("d.wrl")]
    );

    let log = s.take_log().join("\n");

    for (url, n) in [("a.wrl", 2), ("b.wrl", 3), ("c.png", 4)] {
        assert!(
            log.contains(&format!(
                "Inline: Couldn't load URL '{}', trying URL {n} of 4.",
                w(url)
            )),
            "{url}: {log}"
        );
    }

    assert!(log.contains("No suitable file handler found"), "{log}");
}

#[test]
fn every_candidate_fails() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url [ \"bad.wrl\" \"missing.wrl\" ] }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(&w("bad.wrl"), Ok(GARBAGE));
    assert!(!poll(&mut s));
    net.respond(&w("missing.wrl"), Err("Error: Not Found"));
    assert!(poll(&mut s), "a failed Inline ends the wait");
    assert!(s.go_live(NOW));

    let log = s.take_log().join("\n");

    assert!(
        log.contains("Inline: Couldn't load any of the 2 URLs, tried in this order:"),
        "{log}"
    );
}

#[test]
fn the_first_candidate_loads_and_the_others_are_not_fetched() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url [ \"parts/inner.wrl\" \"second.wrl\" ] }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(&w("parts/inner.wrl"), Ok(INNER));
    assert!(poll(&mut s));
    assert_eq!(net.requested(), [w("parts/inner.wrl")]);
    assert!(!s.take_log().join("\n").contains("Couldn't load"));
}

#[test]
fn a_texture_that_does_not_decode_moves_on_to_the_next_candidate() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nShape { appearance Appearance { texture ImageTexture { url [ \"tex/bad.png\" \"tex/red.png\" ] } } geometry Box { } }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(&w("tex/bad.png"), Ok(GARBAGE));
    assert!(!poll(&mut s), "loaded before the second texture candidate");
    assert_eq!(net.requested(), [w("tex/bad.png"), w("tex/red.png")]);

    net.respond(&w("tex/red.png"), Ok(PNG));
    assert!(poll(&mut s));

    let log = s.take_log().join("\n");

    assert!(
        log.contains(&format!(
            "ImageTexture: Couldn't load URL '{}', trying URL 2 of 2.",
            w("tex/bad.png")
        )),
        "{log}"
    );
    assert_eq!(log.matches("Couldn't load URL").count(), 1, "{log}");
}

#[test]
fn a_gltf_waits_for_its_buffers_and_a_failed_buffer_moves_on() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url [ \"models/a.gltf\" \"models/b.gltf\" ] }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(
        &w("models/a.gltf"),
        Ok(&gltf(&["missing.bin", "other.bin"])),
    );
    assert!(!poll(&mut s), "loaded before the buffers");
    // `Promise .all`: both buffers are requested together, relative to the glTF file.
    assert_eq!(
        net.requested(),
        [
            w("models/a.gltf"),
            w("models/missing.bin"),
            w("models/other.bin")
        ]
    );

    net.respond(&w("models/missing.bin"), Err("Error: Not Found"));
    net.respond(&w("models/other.bin"), Ok(&triangle()));
    assert!(!poll(&mut s), "loaded before the second candidate");
    assert_eq!(
        net.requested(),
        [
            w("models/a.gltf"),
            w("models/missing.bin"),
            w("models/other.bin"),
            w("models/b.gltf")
        ],
        "each buffer is requested once"
    );

    net.respond(&w("models/b.gltf"), Ok(&gltf(&["tri.bin"])));
    assert!(
        !poll(&mut s),
        "loaded before the buffer of the second candidate"
    );
    net.respond(&w("models/tri.bin"), Ok(&triangle()));
    assert!(poll(&mut s));

    let log = s.take_log().join("\n");

    // The buffer's own FileLoader (of the world: no node type) logs its failure; the Inline logs nothing for it.
    assert!(
        log.contains(&format!(
            "Couldn't load URL '{}'. Error: Not Found",
            w("models/missing.bin")
        )),
        "{log}"
    );
    assert!(!log.contains("Inline: Couldn't load"), "{log}");
    assert!(!log.contains("other.bin"), "{log}");
}

#[test]
fn every_gltf_buffer_candidate_fails() {
    let net = TestFetcher::default();
    let mut s = begin("#VRML V2.0 utf8\nInline { url \"models/a.gltf\" }\n", &net);

    assert!(!poll(&mut s));
    net.respond(&w("models/a.gltf"), Ok(&gltf(&["missing.bin"])));
    assert!(!poll(&mut s));
    net.respond(&w("models/missing.bin"), Err("Error: Not Found"));
    assert!(poll(&mut s), "the failed Inline ends the wait");
    assert!(s.go_live(NOW));

    let log = s.take_log().join("\n");

    assert_eq!(
        log.matches("Couldn't load").count(),
        1,
        "only the buffer logs: {log}"
    );
}

#[test]
fn an_externproto_moves_on_after_a_file_that_does_not_parse() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nEXTERNPROTO ColorBox [ exposedField SFColor color ] [ \"bad.wrl#ColorBox\" \"protos.wrl#ColorBox\" ]\nDEF Instance ColorBox { color 0 1 0 }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(&w("bad.wrl"), Ok(GARBAGE));
    assert!(!poll(&mut s), "loaded before the second candidate");
    assert_eq!(net.requested(), [w("bad.wrl"), w("protos.wrl")]);

    net.respond(&w("protos.wrl"), Ok(PROTOS));
    assert!(poll(&mut s));
    assert!(s.go_live(NOW));

    let instance = s.named_node("Instance").expect("DEF Instance");

    assert_eq!(s.field(instance, "color").unwrap().value, "0 1 0");
}

#[test]
fn an_anchor_moves_on_after_a_world_that_does_not_parse() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nDEF Link Anchor { url [ \"bad.wrl\" \"next.wrl\" ] }\n",
        &net,
    );

    assert!(poll(&mut s));
    assert!(s.go_live(NOW));

    let anchor = s.named_node("Link").expect("DEF Link");

    s.viewer().expect("live").rt.anchor_load(
        anchor,
        vec!["bad.wrl".into(), "next.wrl".into()],
        vec![],
    );
    s.step(NOW + 0.016);
    assert_eq!(net.requested(), [w("bad.wrl")]);

    net.respond(&w("bad.wrl"), Ok(GARBAGE));
    s.step(NOW + 0.032);
    assert_eq!(net.requested(), [w("bad.wrl"), w("next.wrl")]);
    assert_eq!(
        net.read(),
        [w("bad.wrl")],
        "the second world is read after its response"
    );

    net.respond(&w("next.wrl"), Ok(INNER));

    for i in 3..8 {
        s.step(NOW + f64::from(i) * 0.016);
    }

    assert_eq!(net.read(), [w("bad.wrl"), w("next.wrl")]);

    let log = s.take_log().join("\n");

    assert!(
        log.contains(&format!(
            "Anchor: Couldn't load URL '{}', trying URL 2 of 2.",
            w("bad.wrl")
        )),
        "{log}"
    );
    assert_eq!(log.matches("Couldn't load").count(), 1, "{log}");
}

#[test]
fn a_request_replaced_while_it_waits_leaves_no_state() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nDEF Box Inline { url \"parts/inner.wrl\" }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(&w("parts/inner.wrl"), Ok(INNER));
    assert!(poll(&mut s));
    assert!(s.go_live(NOW));

    let inline = s.named_node("Box").expect("DEF Box");

    // The new url waits for its second candidate after the first did not parse.
    s.set_field(inline, "url", "[ \"bad.wrl\" \"next.wrl\" ]")
        .expect("url");
    s.step(NOW + 0.016);
    net.respond(&w("bad.wrl"), Ok(GARBAGE));
    s.step(NOW + 0.032);
    assert_eq!(net.requested().last(), Some(&w("next.wrl")));
    assert_eq!(s.viewer().expect("live").rt.fetch_resume.len(), 1);

    // A newer url replaces the request that waits: when a response comes, its state goes.
    s.set_field(inline, "url", "\"parts/inner.wrl\"")
        .expect("url");

    for i in 3..6 {
        s.step(NOW + f64::from(i) * 0.016);
    }

    net.respond(&w("next.wrl"), Ok(INNER));

    for i in 6..9 {
        s.step(NOW + f64::from(i) * 0.016);
    }

    let rt = &s.viewer().expect("live").rt;

    assert!(rt.fetch_resume.is_empty(), "{:?}", rt.fetch_resume);
    assert!(!rt.has_parked_fetches());
}

// --- web loadURL messages 01 ------------------------------------------------------------------------------------

use x3d_runtime::Runtime;
use x3d_runtime::loader::{LoadResult, Loaded};

const PT_SANS: &[u8] =
    include_bytes!("../../../oracle/synthetic-rea-b08-net/fonts/PTSans-Regular.ttf");
const NOT_FONT: &[u8] = include_bytes!("../../../oracle/synthetic-rea-b08-net/fonts/notfont.ttf");
const TONE: &[u8] = include_bytes!("../../../oracle/synthetic-rea-b05/media/tone22k-mono.wav");
const GIF: &[u8] = include_bytes!("browser/worlds/tex/green.gif");

/// The world FileLoader of the page's loadURL (WebBrowser.loadURL): its own runtime with the new world's fetcher.
fn page_loader(net: &TestFetcher) -> Runtime {
    let scene = x3d_parse::parse_string("#X3D V4.0 utf8\n", URL).expect("the empty scene");

    Runtime::new(scene, Box::new(net.clone()))
}

/// One step of the page's loadURL (WebBrowser.loadStep): the result, and whether a world parsed.
fn page_step(rt: &mut Runtime, urls: &[String]) -> (LoadResult, bool) {
    let mut parsed = false;
    let result = rt.load_page_document(URL, urls, |rt, loaded| {
        let Loaded::Data(data, url) = loaded else {
            return Err(Some("Error: no script".into()));
        };

        Session::parse_page(rt, &data, url.as_str(), Box::new(TestFetcher::default()))?;
        parsed = true;
        Ok(())
    });

    (result, parsed)
}

fn page_log(rt: &mut Runtime) -> Vec<String> {
    std::mem::take(&mut rt.log)
}

const WORLD: &[u8] = b"#VRML V2.0 utf8\nShape { geometry Box { } }\n";

#[test]
fn page_load_url_first_candidate_loads_and_no_other_is_requested() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("a.wrl"), w("never.wrl")];

    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Suspended, false));
    assert_eq!(net.requested(), [w("a.wrl")]);

    net.respond(&w("a.wrl"), Ok(WORLD));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Loaded, true));
    assert_eq!(net.requested(), [w("a.wrl")]);
    assert_eq!(page_log(&mut rt), Vec::<String>::new());
    assert!(rt.fetch_resume.is_empty());
}

#[test]
fn page_load_url_reports_each_failed_candidate_in_order() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![
        w("u2.wrl"),
        w("u3.wrl"),
        w("u4.wrl"),
        String::new(),
        w("ok.wrl"),
    ];

    assert_eq!(page_step(&mut rt, &urls).0, LoadResult::Suspended);
    net.respond(&w("u2.wrl"), Err("Error: Internal Server Error"));
    assert_eq!(page_step(&mut rt, &urls).0, LoadResult::Suspended);
    // The next candidate is requested only after the one before failed.
    assert_eq!(net.requested(), [w("u2.wrl"), w("u3.wrl")]);

    net.respond(&w("u3.wrl"), Ok(GARBAGE));
    assert_eq!(page_step(&mut rt, &urls).0, LoadResult::Suspended);
    net.respond(&w("u4.wrl"), Err("Error: Not Found"));
    assert_eq!(page_step(&mut rt, &urls).0, LoadResult::Suspended);
    assert_eq!(
        net.requested(),
        [w("u2.wrl"), w("u3.wrl"), w("u4.wrl"), w("ok.wrl")]
    );

    net.respond(&w("ok.wrl"), Ok(WORLD));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Loaded, true));

    // oracle/web-browser-loadurl-01-baseline, loadURL.severalThenLoads.
    assert_eq!(
        page_log(&mut rt),
        [
            format!(
                "warn: Couldn't load URL '{}', trying URL 2 of 5. Error: Internal Server Error",
                w("u2.wrl")
            ),
            format!(
                "warn: Couldn't load URL '{}', trying URL 3 of 5. Error: Couldn't parse X3D. No suitable file handler found for '{}'.",
                w("u3.wrl"),
                w("u3.wrl")
            ),
            format!(
                "warn: Couldn't load URL '{}', trying URL 4 of 5. Error: Not Found",
                w("u4.wrl")
            ),
            "warn: Couldn't load empty URL, trying URL 5 of 5. Error: Empty URL.".to_string(),
        ]
    );
}

#[test]
fn page_load_url_every_candidate_fails() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("u6.wrl"), w("u7.wrl")];

    page_step(&mut rt, &urls);
    net.respond(&w("u6.wrl"), Err("Error: Not Found"));
    page_step(&mut rt, &urls);
    net.respond(&w("u7.wrl"), Ok(GARBAGE));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Failed, false));

    let log = page_log(&mut rt);

    assert_eq!(log.len(), 2, "{log:?}");
    assert_eq!(
        log[1],
        "error: Couldn't load any of the 2 URLs, tried in this order:"
    );

    // One candidate: its own message (loadURL.oneFails).
    let mut rt = page_loader(&net);
    let one = vec![w("u6.wrl")];

    assert_eq!(page_step(&mut rt, &one).0, LoadResult::Failed);
    assert_eq!(
        page_log(&mut rt),
        [format!(
            "error: Couldn't load URL '{}'. Error: Not Found",
            w("u6.wrl")
        )]
    );
}

#[test]
fn page_load_url_waits_without_a_retry_loop() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("slow.wrl"), w("next.wrl")];

    for _ in 0..50 {
        assert_eq!(page_step(&mut rt, &urls).0, LoadResult::Suspended);
    }

    // One request, no read, no message while the response is in flight.
    assert_eq!(net.requested(), [w("slow.wrl")]);
    assert!(net.read().is_empty());
    assert!(page_log(&mut rt).is_empty());
}

#[test]
fn page_load_url_abort_ends_the_chain_at_the_candidate_in_flight() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let mut urls = vec![w("held.wrl"), w("next.wrl")];

    page_step(&mut rt, &urls);

    // FileLoader.abort (): `this .url .length = 0`, and the request rejects (loadURL.replacedWhilePending).
    let left = rt.abort_page_document().expect("the chain waits");

    urls.truncate(left);
    net.respond(
        &w("held.wrl"),
        Err("AbortError: signal is aborted without reason"),
    );
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Failed, false));
    assert_eq!(net.requested(), [w("held.wrl")]);
    assert_eq!(
        page_log(&mut rt),
        [format!(
            "error: Couldn't load URL '{}'. AbortError: signal is aborted without reason",
            w("held.wrl")
        )]
    );

    // After a failed candidate the abort counts it (loadURL.replacedAfterFailure).
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let mut urls = vec![w("u9.wrl"), w("held.wrl"), w("next.wrl")];

    page_step(&mut rt, &urls);
    net.respond(&w("u9.wrl"), Err("Error: Not Found"));
    page_step(&mut rt, &urls);
    urls.truncate(rt.abort_page_document().expect("the chain waits"));
    net.respond(
        &w("held.wrl"),
        Err("AbortError: signal is aborted without reason"),
    );
    assert_eq!(page_step(&mut rt, &urls).0, LoadResult::Failed);
    assert_eq!(net.requested(), [w("u9.wrl"), w("held.wrl")]);
    assert_eq!(
        page_log(&mut rt),
        [
            format!(
                "warn: Couldn't load URL '{}', trying URL 2 of 3. Error: Not Found",
                w("u9.wrl")
            ),
            "error: Couldn't load any of the 2 URLs, tried in this order:".to_string(),
        ]
    );
    assert!(rt.fetch_resume.is_empty());
}

/// A live world of `text`, after the responses `first` (the world waits for them).
fn live(text: &str, net: &TestFetcher, first: &[(&str, &[u8])]) -> Session {
    let mut s = begin(text, net);

    for (path, data) in first {
        poll(&mut s);
        net.respond(&w(path), Ok(data));
    }

    for i in 0..20 {
        if s.poll_load(NOW + f64::from(i) * 0.016) {
            break;
        }
    }

    assert!(s.go_live(NOW), "the world went live");
    s
}

fn steps(s: &mut Session, from: u32, n: u32) {
    for i in from..from + n {
        s.step(NOW + f64::from(i) * 0.016);
    }
}

#[test]
fn a_font_file_waits_for_its_response_then_the_next_family_is_fetched() {
    let net = TestFetcher::default();
    let mut s = live(
        "#X3D V4.0 utf8\nShape { geometry Text { string \"Hg\" fontStyle FontStyle { family [ \"fonts/f1.ttf\" \"fonts/notfont.ttf\" \"fonts/PTSans-Regular.ttf\" ] } } }\n",
        &net,
        &[],
    );

    steps(&mut s, 1, 10);
    assert_eq!(net.requested(), [w("fonts/f1.ttf")]);

    net.respond(&w("fonts/f1.ttf"), Err("Error: Not Found"));
    steps(&mut s, 11, 10);
    assert_eq!(net.requested(), [w("fonts/f1.ttf"), w("fonts/notfont.ttf")]);

    net.respond(&w("fonts/notfont.ttf"), Ok(NOT_FONT));
    steps(&mut s, 21, 10);
    // No busy loop: one request per file while it is in flight.
    assert_eq!(
        net.requested(),
        [
            w("fonts/f1.ttf"),
            w("fonts/notfont.ttf"),
            w("fonts/PTSans-Regular.ttf")
        ]
    );

    net.respond(&w("fonts/PTSans-Regular.ttf"), Ok(PT_SANS));
    steps(&mut s, 31, 10);

    let rt = &s.viewer().expect("live").rt;

    assert!(!rt.fonts_loading());
    assert!(rt.font_fetch_waits.is_empty());

    // oracle/web-browser-loadurl-01-baseline, font.fallback.
    let log: Vec<String> = s
        .take_log()
        .into_iter()
        .filter(|l| l.contains("font"))
        .collect();

    assert_eq!(
        log,
        [
            format!(
                "error: FontStyle: Couldn't load URL '{}'. Error: Not Found",
                w("fonts/f1.ttf")
            ),
            format!("warn: Error loading font '{}':", w("fonts/f1.ttf")),
            format!(
                "error: FontStyle: Couldn't load URL '{}'. Error: Unsupported OpenType signature not ",
                w("fonts/notfont.ttf")
            ),
            format!("warn: Error loading font '{}':", w("fonts/notfont.ttf")),
        ]
    );
}

#[test]
fn a_font_style_that_is_gone_ignores_its_late_response() {
    let net = TestFetcher::default();
    let mut s = live(
        "#X3D V4.0 utf8\nDEF T Transform { children Shape { geometry Text { string \"Hg\" fontStyle FontStyle { family \"fonts/late.ttf\" } } } }\n",
        &net,
        &[],
    );

    steps(&mut s, 1, 10);
    assert_eq!(net.requested(), [w("fonts/late.ttf")]);

    // A newer family: the load that waits is no longer the node's.
    let style = {
        let rt = &s.viewer().expect("live").rt;

        rt.font_fetch_waits[0].node
    };

    s.set_field(style, "family", "\"SANS\"").expect("family");
    steps(&mut s, 11, 10);
    net.respond(&w("fonts/late.ttf"), Ok(PT_SANS));
    steps(&mut s, 21, 10);

    let rt = &s.viewer().expect("live").rt;

    assert!(rt.font_fetch_waits.is_empty());
    assert!(!rt.fonts_loading());
    assert!(net.read().is_empty(), "{:?}", net.read());
}

#[test]
fn a_font_library_waits_for_the_file_after_one_that_does_not_parse() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#X3D V4.1 utf8\nFontLibrary { family \"Lib\" url [ \"fonts/notfont.ttf\" \"fonts/PTSans-Regular.ttf\" ] }\nShape { geometry Text { string \"Hg\" fontStyle FontStyle { family \"Lib\" } } }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(&w("fonts/notfont.ttf"), Ok(NOT_FONT));
    assert!(!poll(&mut s), "loaded before the second font file");
    assert_eq!(
        net.requested(),
        [w("fonts/notfont.ttf"), w("fonts/PTSans-Regular.ttf")]
    );

    net.respond(&w("fonts/PTSans-Regular.ttf"), Ok(PT_SANS));
    assert!(poll(&mut s));

    let log = s.take_log().join("\n");

    assert!(
        log.contains(&format!(
            "FontLibrary: Couldn't load URL '{}'.",
            w("fonts/notfont.ttf")
        )),
        "{log}"
    );
}

#[test]
fn a_movie_texture_waits_for_the_candidate_after_one_that_does_not_play() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#X3D V4.0 utf8\nShape { appearance Appearance { texture MovieTexture { url [ \"m1.avi\" \"tex/green.gif\" ] } } geometry Box { } }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(&w("m1.avi"), Ok(GARBAGE));
    assert!(!poll(&mut s), "loaded before the GIF");
    assert_eq!(net.requested(), [w("m1.avi"), w("tex/green.gif")]);

    net.respond(&w("tex/green.gif"), Ok(GIF));
    assert!(poll(&mut s));

    let log = s.take_log().join("\n");

    assert!(
        log.contains(&format!(
            "Error loading movie '{}': Couldn't load video '{}': error.",
            w("m1.avi"),
            w("m1.avi")
        )),
        "{log}"
    );
    assert!(!log.contains("tex/green.gif"), "{log}");
}

#[test]
fn a_movie_texture_that_fails_reports_like_x_ite() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#X3D V4.0 utf8\nShape { appearance Appearance { texture MovieTexture { url \"m3.gif\" } } geometry Box { } }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(&w("m3.gif"), Err("Error: Not Found"));
    assert!(poll(&mut s));

    // oracle/web-browser-loadurl-01-baseline, movie.failed.
    assert_eq!(
        s.take_log(),
        [
            format!(
                "error: MovieTexture: Couldn't load URL '{}'. Error: Not Found",
                w("m3.gif")
            ),
            format!(
                "warn: Error loading movie '{}': MovieTexture: No suitable file handler found.",
                w("m3.gif")
            ),
            "error: Error: MovieTexture: No suitable file handler found.".to_string(),
        ]
    );
}

#[test]
fn an_audio_clip_waits_for_the_candidate_after_one_that_does_not_decode() {
    let net = TestFetcher::default();
    let mut s = live(
        "#X3D V4.0 utf8\nSound { source DEF A AudioClip { url [ \"a1.wav\" \"tone.wav\" ] } }\n",
        &net,
        &[("a1.wav", GARBAGE)],
    );

    steps(&mut s, 1, 10);
    assert_eq!(net.requested(), [w("a1.wav"), w("tone.wav")]);
    assert_eq!(net.read(), [w("a1.wav")], "read before the response");

    net.respond(&w("tone.wav"), Ok(TONE));
    steps(&mut s, 14, 10);

    let clip = s.named_node("A").expect("DEF A");

    let duration: f64 = s
        .field(clip, "duration_changed")
        .expect("duration")
        .value
        .parse()
        .expect("a number");

    // The recording rounds to 3 decimals (audio.decodeFallback: "0.272").
    assert_eq!(format!("{duration:.3}"), "0.272");

    // oracle/web-browser-loadurl-01-baseline, audio.decodeFallback.
    let log: Vec<String> = s
        .take_log()
        .into_iter()
        .filter(|l| l.contains("AudioClip"))
        .collect();

    assert_eq!(
        log,
        [format!(
            "warn: AudioClip: Couldn't load URL '{}', trying URL 2 of 2. EncodingError: Unable to decode audio data",
            w("a1.wav")
        )]
    );

    let rt = &s.viewer().expect("live").rt;

    assert!(rt.fetch_resume.is_empty());
    assert!(!rt.has_parked_fetches());
}

#[test]
fn every_audio_candidate_fails_to_decode() {
    let net = TestFetcher::default();
    let mut s = live(
        "#X3D V4.0 utf8\nSound { source DEF A AudioClip { url [ \"a3.wav\" \"a4.wav\" ] } }\n",
        &net,
        &[("a3.wav", GARBAGE)],
    );

    steps(&mut s, 1, 10);
    assert_eq!(net.read(), [w("a3.wav")]);
    net.respond(&w("a4.wav"), Ok(GARBAGE));
    steps(&mut s, 14, 10);

    let log: Vec<String> = s
        .take_log()
        .into_iter()
        .filter(|l| l.contains("AudioClip"))
        .collect();

    // oracle/web-browser-loadurl-01-baseline, audio.allFail.
    assert_eq!(
        log,
        [
            format!(
                "warn: AudioClip: Couldn't load URL '{}', trying URL 2 of 2. EncodingError: Unable to decode audio data",
                w("a3.wav")
            ),
            "error: AudioClip: Couldn't load any of the 2 URLs, tried in this order:".to_string(),
        ]
    );
    assert_eq!(net.requested(), [w("a3.wav"), w("a4.wav")]);
}

#[test]
fn a_world_dropped_while_its_audio_waits_takes_no_late_response() {
    let net = TestFetcher::default();
    let mut s = live(
        "#X3D V4.0 utf8\nSound { source AudioClip { url [ \"a5.wav\" \"held.wav\" ] } }\n",
        &net,
        &[("a5.wav", GARBAGE)],
    );

    steps(&mut s, 1, 10);
    assert_eq!(net.requested().last(), Some(&w("held.wav")));

    // The page replaces or disposes the world: the session goes, then the response comes.
    drop(s);
    net.respond(&w("held.wav"), Ok(TONE));
    assert_eq!(net.read(), [w("a5.wav")]);
}

// --- data: URLs (MiniMax P1 of PR #123) ----------------------------------------------------------------------------

fn base64(data: &[u8]) -> String {
    const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";
    let mut out = String::new();

    for chunk in data.chunks(3) {
        let n = chunk.iter().fold(0u32, |n, &b| n << 8 | b as u32) << (8 * (3 - chunk.len()));

        for k in 0..=chunk.len() {
            out.push(B64[(n >> (18 - 6 * k)) as usize & 63] as char);
        }

        out.push_str(&"=".repeat(3 - chunk.len()));
    }

    out
}

fn percent(data: &[u8]) -> String {
    data.iter().map(|b| format!("%{b:02x}")).collect()
}

/// A font, an image, a movie and a sound as `;BASE64` data URLs (oracle/web-browser-loadurl-01-baseline, dataURL.*):
/// each decodes, and no later candidate is requested.
#[test]
fn upper_case_base64_data_urls_decode_for_every_resource_type() {
    let net = TestFetcher::default();
    let text = format!(
        "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT Text:1\nCOMPONENT Texturing:3\nCOMPONENT Sound:2\n\
         Shape {{ appearance Appearance {{ texture ImageTexture {{ url [ \"data:image/gif;BASE64,{gif}\" \"i1.gif\" ] }} }} geometry Box {{ }} }}\n\
         Shape {{ appearance Appearance {{ texture MovieTexture {{ url [ \"data:image/gif;BASE64,{gif}\" \"m9.gif\" ] }} }} geometry Box {{ }} }}\n\
         Shape {{ geometry DEF T Text {{ string \"Hg\" fontStyle FontStyle {{ family [ \"data:font/ttf;BASE64,{font}\" \"f9.ttf\" ] }} }} }}\n\
         Sound {{ source DEF A AudioClip {{ url [ \"data:audio/wav;BASE64,{tone}\" \"a9.wav\" ] }} }}\n",
        gif = base64(GIF),
        font = base64(PT_SANS),
        tone = base64(TONE),
    );
    let mut s = live(&text, &net, &[]);

    steps(&mut s, 1, 20);
    assert_eq!(net.requested(), Vec::<String>::new());

    let clip = s.named_node("A").expect("DEF A");
    let duration: f64 = s
        .field(clip, "duration_changed")
        .expect("duration")
        .value
        .parse()
        .expect("a number");

    assert_eq!(format!("{duration:.3}"), "0.272");

    let text = s.named_node("T").expect("DEF T");

    let bounds: Vec<f64> = s
        .field(text, "textBounds")
        .expect("textBounds")
        .value
        .split_whitespace()
        .map(|v| v.parse().expect("a number"))
        .collect();

    // dataURL.font: "1.13 0.91" (PT Sans; the default font is narrower).
    assert_eq!(format!("{:.2} {:.2}", bounds[0], bounds[1]), "1.13 0.91");

    let log = s.take_log().join("\n");

    assert!(!log.contains("Couldn't load"), "{log}");

    let rt = &s.viewer().expect("live").rt;

    assert!(rt.font_fetch_waits.is_empty());
    assert!(rt.fetch_resume.is_empty());
    assert!(!rt.has_parked_fetches());
}

/// Binary bytes percent-encoded (dataURL.image, percent): the image decodes without a request.
#[test]
fn percent_encoded_binary_data_url_decodes() {
    let net = TestFetcher::default();
    let mut s = begin(
        &format!(
            "#VRML V2.0 utf8\nShape {{ appearance Appearance {{ texture ImageTexture {{ url [ \"data:image/gif,{}\" \"i2.gif\" ] }} }} geometry Box {{ }} }}\n",
            percent(GIF)
        ),
        &net,
    );

    assert!(poll(&mut s));
    assert_eq!(net.requested(), Vec::<String>::new());
    assert!(!s.take_log().join("\n").contains("Couldn't load"));
}

/// Base64 that does not decode fails at once like a failed response, and the next candidate is fetched
/// (dataURL.imageBadBase64).
#[test]
fn a_data_url_that_does_not_decode_moves_on_to_the_next_candidate() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nShape { appearance Appearance { texture ImageTexture { url [ \"data:image/gif;base64,R0lG@@@@\" \"tex/green.gif\" ] } } geometry Box { } }\n",
        &net,
    );

    assert!(!poll(&mut s));
    assert_eq!(net.requested(), [w("tex/green.gif")]);

    net.respond(&w("tex/green.gif"), Ok(GIF));
    assert!(poll(&mut s));
    assert_eq!(
        s.take_log()
            .into_iter()
            .filter(|l| l.contains("Couldn't load"))
            .collect::<Vec<_>>(),
        [
            "warn: ImageTexture: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch"
        ]
    );
}

/// The world's loadURL of data URLs (FileLoader's dataAsString branch; dataURL.world*): the results of pinned X_ITE.
#[test]
fn page_load_url_data_urls_like_x_ite() {
    let doc = "#VRML V2.0 utf8\nDEF Data Shape { geometry Box { } }\n";
    let encoded: String = doc
        .bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();
    let cases = [
        // worldPercent, worldBase64: the world loads.
        (format!("data:model/vrml,{encoded}"), None),
        (
            format!("data:model/vrml;base64,{}", base64(doc.as_bytes())),
            None,
        ),
        // worldBase64Upper: `;(base64)` is case-sensitive, so the base64 text is the document.
        (
            format!("data:model/vrml;BASE64,{}", base64(doc.as_bytes())),
            Some("Error: Couldn't parse X3D. No suitable file handler found for 'data:' URL."),
        ),
        // worldBadBase64: fetch rejects.
        (
            "data:model/vrml;base64,QQ=@".to_string(),
            Some("TypeError: Failed to fetch"),
        ),
        // worldBadPercent: decodeURIComponent throws, the text stays encoded.
        (
            format!("data:model/vrml,{encoded}%zz"),
            Some("Error: Couldn't parse X3D. No suitable file handler found for 'data:' URL."),
        ),
    ];

    for (url, error) in cases {
        let net = TestFetcher::default();
        let mut rt = page_loader(&net);
        let urls = vec![url.clone(), w("w.wrl")];
        let (result, parsed) = page_step(&mut rt, &urls);

        match error {
            None => {
                assert_eq!((result, parsed), (LoadResult::Loaded, true), "{url}");
                assert_eq!(net.requested(), Vec::<String>::new(), "{url}");
                assert_eq!(page_log(&mut rt), Vec::<String>::new(), "{url}");
            }
            Some(error) => {
                assert_eq!(result, LoadResult::Suspended, "{url}");
                assert_eq!(net.requested(), [w("w.wrl")], "{url}");
                assert_eq!(
                    page_log(&mut rt),
                    [format!(
                        "warn: Couldn't load data URL, trying URL 2 of 2. {error}"
                    )],
                    "{url}"
                );
            }
        }
    }
}

// --- FontStyle loads that overlap (MiniMax P2 of PR #123) -----------------------------------------------------------

/// X3DFontStyleNode.loadData: a newer load of the same family and style while the older one waits for the font file.
/// X_ITE's loads share browser.loadFont's promise (one request); each applies its font unless a newer one already
/// did (`count < this .loadCount`), so both end with the same font. X_ITE_R keeps both waits and both apply it.
#[test]
fn two_font_loads_of_the_same_family_and_style_end_with_one_font() {
    let net = TestFetcher::default();
    let mut s = live(
        "#X3D V4.0 utf8\nShape { geometry DEF T Text { string \"Hg\" fontStyle DEF F FontStyle { family \"fonts/PTSans-Regular.ttf\" style \"PLAIN\" } } }\n",
        &net,
        &[],
    );

    steps(&mut s, 1, 10);
    assert_eq!(net.requested(), [w("fonts/PTSans-Regular.ttf")]);

    let style = s.named_node("F").expect("DEF F");

    // The same family and style again: a second loadData.
    s.set_field(style, "family", "\"fonts/PTSans-Regular.ttf\"")
        .expect("family");
    steps(&mut s, 11, 10);

    let waits = s.viewer().expect("live").rt.font_fetch_waits.len();

    net.respond(&w("fonts/PTSans-Regular.ttf"), Ok(PT_SANS));
    steps(&mut s, 21, 10);

    // One request, read once the response is in.
    assert_eq!(net.requested(), [w("fonts/PTSans-Regular.ttf")]);
    // Both loads wait for the one response.
    assert_eq!(waits, 2);

    let text = s.named_node("T").expect("DEF T");
    let bounds = s.field(text, "textBounds").expect("textBounds").value;
    let rt = &s.viewer().expect("live").rt;

    assert!(rt.font_fetch_waits.is_empty());
    assert!(!rt.fonts_loading());

    // The same bounds as the font.fallback recording, where PT Sans loads once: "1.13 0.91".
    let bounds: Vec<f64> = bounds
        .split_whitespace()
        .map(|v| v.parse().expect("a number"))
        .collect();

    assert_eq!(format!("{:.2} {:.2}", bounds[0], bounds[1]), "1.13 0.91");
    assert!(!s.take_log().join("\n").contains("Couldn't load"));
}

// --- FileLoader.abort () (MiniMax P2 of PR #123) --------------------------------------------------------------------

/// An abort while a candidate is pending: the AbortError is reported once, no later candidate is fetched, the resume
/// entry is gone after it, and no task waits. A second abort changes nothing.
#[test]
fn page_load_url_abort_reports_once_and_leaves_no_state() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let mut urls = vec![w("held.wrl"), w("next.wrl"), w("last.wrl")];

    assert_eq!(page_step(&mut rt, &urls).0, LoadResult::Suspended);

    let left = rt.abort_page_document().expect("the chain waits");

    assert_eq!(left, 1);
    // Twice: the chain still ends at the candidate in flight.
    assert_eq!(rt.abort_page_document(), Some(1));
    urls.truncate(left);

    // Steps before the abort's rejection arrives: no message, no new request.
    assert_eq!(page_step(&mut rt, &urls).0, LoadResult::Suspended);
    assert!(page_log(&mut rt).is_empty());

    net.respond(
        &w("held.wrl"),
        Err("AbortError: signal is aborted without reason"),
    );
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Failed, false));
    assert_eq!(
        page_log(&mut rt),
        [format!(
            "error: Couldn't load URL '{}'. AbortError: signal is aborted without reason",
            w("held.wrl")
        )]
    );
    assert_eq!(net.requested(), [w("held.wrl")]);
    assert!(rt.fetch_resume.is_empty());
    assert!(!rt.has_parked_fetches());
    // After the abort completed there is no chain to abort.
    assert_eq!(rt.abort_page_document(), None);
}

// --- PR #123 final review ------------------------------------------------------------------------------------------

/// GoldenGate.parseIntoScene names no data URL (`worldURL .startsWith ("data:")`). X3DScene.worldURL is a URL that a
/// URL parser serialized (`String (new URL (…))`), so its scheme is in lower case: `DATA:` and `Data:` are data URLs
/// too. Other URLs keep their text in the message.
#[test]
fn a_world_that_no_parser_handles_names_a_data_url_in_any_scheme_case() {
    const DATA: &str = "Error: Couldn't parse X3D. No suitable file handler found for 'data:' URL.";
    let named = |url: &str| {
        format!("Error: Couldn't parse X3D. No suitable file handler found for '{url}'.")
    };
    let parse = |url: &str| {
        Session::parse(GARBAGE, url, Box::new(TestFetcher::default()))
            .err()
            .expect("no parser handles the file")
    };
    let parse_string = |url: &str| {
        Session::parse_string(
            "this is no file format",
            url,
            Box::new(TestFetcher::default()),
        )
        .err()
        .expect("no parser handles the text")
    };

    for url in [
        "data:,x",
        "DATA:,x",
        "Data:text/plain,x",
        " dAtA:;base64,QUJD",
    ] {
        assert_eq!(parse(url), DATA, "{url}");
        assert_eq!(parse_string(url), DATA, "{url}");
    }

    for url in [URL, "datafile.wrl", "dat:,x", "data-x:,y"] {
        assert_eq!(parse(url), named(url), "{url}");
        assert_eq!(parse_string(url), named(url), "{url}");
    }
}

/// The page load's resume entry has a key of its own (`FetchKey::Page`). A task run of its runtime keeps it, and a
/// createX3DFromURL request with the largest id (the old `FetchKey::Browser (u64::MAX)` sentinel) does not replace it.
#[test]
fn a_page_load_keeps_its_own_resume_entry() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("p1.wrl"), w("p2.wrl"), w("p3.wrl")];

    assert_eq!(page_step(&mut rt, &urls).0, LoadResult::Suspended);
    net.respond(&w("p1.wrl"), Err("Error: Not Found"));
    assert_eq!(page_step(&mut rt, &urls).0, LoadResult::Suspended);

    // A task run between two steps.
    rt.run_tasks();

    // A request of the runtime with the id u64::MAX whose chain waits at its second candidate (the first one loaded
    // but did not parse).
    rt.browser_request_id = u64::MAX - 1;
    rt.browser_create_from_url(vec![w("b1.wrl"), w("b2.wrl")], 0, None);
    net.respond(&w("b1.wrl"), Ok(GARBAGE));
    rt.run_tasks();
    assert_eq!(rt.fetch_resume.len(), 2, "{:?}", rt.fetch_resume);
    assert!(rt.has_parked_fetches());

    // The page chain is still at its second candidate: an abort ends it there.
    assert_eq!(rt.abort_page_document(), Some(2));

    let urls = &urls[..2];

    net.respond(
        &w("p2.wrl"),
        Err("AbortError: signal is aborted without reason"),
    );
    assert_eq!(page_step(&mut rt, urls), (LoadResult::Failed, false));
    assert!(!net.requested().contains(&w("p3.wrl")));

    // The request's own chain still waits.
    assert_eq!(rt.fetch_resume.len(), 1, "{:?}", rt.fetch_resume);
    assert_eq!(rt.abort_page_document(), None);
}

/// Repeated page loads of one WebBrowser each have a runtime of their own; a replaced load leaves no entry in the
/// next one.
#[test]
fn repeated_page_loads_share_no_resume_entry() {
    let net = TestFetcher::default();
    let mut first = page_loader(&net);
    let mut second = page_loader(&net);
    let a = vec![w("r1.wrl"), w("r2.wrl")];
    let b = vec![w("r3.wrl")];

    assert_eq!(page_step(&mut first, &a).0, LoadResult::Suspended);
    assert_eq!(page_step(&mut second, &b).0, LoadResult::Suspended);
    assert_eq!(first.abort_page_document(), Some(1));
    assert_eq!(second.abort_page_document(), Some(1));

    net.respond(&w("r3.wrl"), Ok(WORLD));
    assert_eq!(page_step(&mut second, &b), (LoadResult::Loaded, true));
    assert!(second.fetch_resume.is_empty());
    assert_eq!(first.fetch_resume.len(), 1);
}

// --- PR #123 merge gate -----------------------------------------------------------------------------------------

use x3d_runtime::loader::FetchKey;

/// Each FontStyle loop that waits for a font file has that file's request in flight: [`Fetcher::poll`] starts it
/// before the loop parks in `font_fetch_waits`, and its response is still held.
fn assert_each_font_wait_has_its_request(s: &mut Session, net: &TestFetcher) {
    let rt = &s.viewer().expect("live").rt;
    let n = net.0.borrow();

    for p in &rt.font_fetch_waits {
        let url = w(&p.family[p.index]);

        assert!(n.requested.contains(&url), "{url} waits without a request");
        assert_eq!(
            n.responses.get(&url),
            Some(&None),
            "{url} waits after its response"
        );
    }
}

/// The font request starts before its response is released, and no font wait exists without its request: after a
/// failure (the next family), a repeated loadData, a cancelled load and a success (MiniMax, PR #123 merge gate).
#[test]
fn a_waiting_font_always_has_its_request_in_flight() {
    let net = TestFetcher::default();
    let mut s = live(
        "#X3D V4.0 utf8\nShape { geometry Text { string \"Hg\" fontStyle DEF A FontStyle { family [ \"fonts/a1.ttf\" \"fonts/a2.ttf\" ] } } }\nShape { geometry Text { string \"Hg\" fontStyle DEF B FontStyle { family \"fonts/b.ttf\" } } }\n",
        &net,
        &[],
    );
    let a = s.named_node("A").expect("DEF A");
    let b = s.named_node("B").expect("DEF B");
    let count =
        |net: &TestFetcher, path: &str| net.requested().iter().filter(|u| **u == w(path)).count();

    // Both requests are in flight while the responses are held; nothing was read.
    steps(&mut s, 1, 10);
    assert_eq!(s.viewer().expect("live").rt.font_fetch_waits.len(), 2);
    assert_eq!(count(&net, "fonts/a1.ttf"), 1);
    assert_eq!(count(&net, "fonts/b.ttf"), 1);
    assert_each_font_wait_has_its_request(&mut s, &net);
    assert!(net.read().is_empty(), "{:?}", net.read());

    // A repeated loadData of A shares the request in flight.
    s.set_field(a, "family", "[ \"fonts/a1.ttf\" \"fonts/a2.ttf\" ]")
        .expect("family");
    steps(&mut s, 11, 10);
    assert_eq!(count(&net, "fonts/a1.ttf"), 1);
    assert_each_font_wait_has_its_request(&mut s, &net);

    // A failure: the next family's request starts at once.
    net.respond(&w("fonts/a1.ttf"), Err("Error: Not Found"));
    steps(&mut s, 21, 10);
    assert_eq!(count(&net, "fonts/a2.ttf"), 1);
    assert_each_font_wait_has_its_request(&mut s, &net);

    // B is cancelled (a newer family): its wait goes, and its late response is not read.
    s.set_field(b, "family", "\"SANS\"").expect("family");
    steps(&mut s, 31, 10);
    net.respond(&w("fonts/b.ttf"), Ok(PT_SANS));
    steps(&mut s, 41, 10);
    assert_each_font_wait_has_its_request(&mut s, &net);
    assert!(!net.read().contains(&w("fonts/b.ttf")), "{:?}", net.read());

    // A success ends the last wait.
    net.respond(&w("fonts/a2.ttf"), Ok(PT_SANS));
    steps(&mut s, 51, 10);

    let rt = &s.viewer().expect("live").rt;

    assert!(rt.font_fetch_waits.is_empty());
    assert!(!rt.fonts_loading());
    assert_eq!(count(&net, "fonts/a1.ttf"), 1);
    assert_eq!(count(&net, "fonts/a2.ttf"), 1);
    assert_eq!(count(&net, "fonts/b.ttf"), 1);
}

/// `decodeURIComponent` in Chromium 151 (PR #123 merge gate probe): the decoded text, or `None` where it throws
/// `URIError: URI malformed`.
const DECODE_URI_COMPONENT: &[(&str, Option<&str>)] = &[
    ("%E0%A4%A", None),
    ("%C3%A9", Some("é")),
    ("%E0%A4%A4", Some("त")),
    ("%C3%A9%E0%A4%A", None),
    ("%C3%A9%zz", None),
    ("%41%E0%A4%A%42", None),
    ("é", Some("é")),
    ("é%C3%A9", Some("éé")),
    ("é%E0%A4%A", None),
    ("%FF", None),
    ("%80", None),
    ("%C3", None),
    ("%C3%28", None),
    ("%C3%A9%C3", None),
    ("%C0%AF", None),
    ("%C1%BF", None),
    ("%E0%80%80", None),
    ("%ED%A0%80", None),
    ("%ED%BF%BF", None),
    ("%F4%90%80%80", None),
    ("%F5%80%80%80", None),
    ("%F8%80%80%80%80", None),
    ("%F0%9F%98%80", Some("😀")),
    ("%F0%9F%98", None),
    ("%E2%82%AC", Some("€")),
    ("%E2%82", None),
    ("%e2%82%ac", Some("€")),
    ("%+1", None),
    ("%-1", None),
    ("% 1", None),
    ("%1", None),
    ("%", None),
    ("%%", None),
    ("%2C%3B%2F%3F%23", Some(",;/?#")),
    ("a%20b", Some("a b")),
    ("€%E2%82%AC", Some("€€")),
    ("%EF%BB%BF#VRML", Some("\u{feff}#VRML")),
    ("😀%F0%9F%98%80", Some("😀😀")),
    ("%00", Some("\0")),
];

#[test]
fn decode_uri_component_matches_chromium() {
    for (input, decoded) in DECODE_URI_COMPONENT {
        assert_eq!(
            x3d_runtime::loader::decode_uri_component(input).as_deref(),
            *decoded,
            "{input}"
        );
    }
}

/// The world text of a `data:` candidate (FileLoader's dataAsString branch): `decodeURIComponent (data)`, or the text
/// as it is where that throws. A malformed UTF-8 escape keeps every escape and gives no replacement character.
#[test]
fn a_data_url_world_keeps_its_text_where_decode_uri_component_throws() {
    for (input, decoded) in DECODE_URI_COMPONENT {
        let net = TestFetcher::default();
        let mut rt = page_loader(&net);
        let mut text = None;
        let result =
            rt.load_page_document(URL, &[format!("data:model/vrml,{input}")], |_, loaded| {
                if let Loaded::Data(data, _) = loaded {
                    text = Some(String::from_utf8(data).expect("UTF-8"));
                }

                Err(Some("Error: no parser".into()))
            });

        assert_eq!(result, LoadResult::Failed, "{input}");
        assert_eq!(text.as_deref(), Some(decoded.unwrap_or(input)), "{input}");
        assert!(!text.unwrap_or_default().contains('\u{fffd}'), "{input}");
        assert!(net.requested().is_empty());
    }
}

/// A rejection of `accept` (createX3DFromString throws) is a failed candidate, not an escape from the chain: a load
/// that goes on to a held candidate keeps its resume entry, the final failure removes it, and the runtime starts the
/// next page load (MiniMax, PR #123 merge gate).
#[test]
fn a_page_load_whose_documents_are_rejected_leaves_no_resume_entry() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("j1.wrl"), w("j2.wrl")];
    let reject = |rt: &mut Runtime, urls: &[String]| {
        rt.load_page_document(URL, urls, |_, _| Err(Some("Error: rejected".into())))
    };

    assert_eq!(reject(&mut rt, &urls), LoadResult::Suspended);
    assert!(rt.fetch_resume.contains_key(&FetchKey::Page));

    // j1 is rejected; j2 is held, so the entry stays.
    net.respond(&w("j1.wrl"), Ok(WORLD));
    assert_eq!(reject(&mut rt, &urls), LoadResult::Suspended);
    assert_eq!(rt.fetch_resume[&FetchKey::Page].index, 1);

    // j2 is rejected too: the final failure leaves no entry.
    net.respond(&w("j2.wrl"), Ok(WORLD));
    assert_eq!(reject(&mut rt, &urls), LoadResult::Failed);
    assert!(rt.fetch_resume.is_empty(), "{:?}", rt.fetch_resume);
    assert!(
        page_log(&mut rt)
            .iter()
            .any(|l| l.contains("Error: rejected"))
    );

    // Every candidate rejected without a wait (a data URL needs no response).
    let data = vec!["data:model/vrml,%23VRML%20V2.0%20utf8".to_string()];

    assert_eq!(reject(&mut rt, &data), LoadResult::Failed);
    assert!(rt.fetch_resume.is_empty(), "{:?}", rt.fetch_resume);

    // The runtime starts another page load, and it loads.
    let next = vec![w("j3.wrl")];

    assert_eq!(page_step(&mut rt, &next).0, LoadResult::Suspended);
    net.respond(&w("j3.wrl"), Ok(WORLD));
    assert_eq!(page_step(&mut rt, &next), (LoadResult::Loaded, true));
    assert!(rt.fetch_resume.is_empty(), "{:?}", rt.fetch_resume);
    assert_eq!(net.requested(), [w("j1.wrl"), w("j2.wrl"), w("j3.wrl")]);
}

// --- raw tab, LF and CR in data URLs (XITE_R_WEB_DATA_URL_COMPAT_01) ------------------------------------------------

/// The percent escapes of `doc`, but a tab, LF and CR in its title (between quotes) stay raw characters.
fn escaped_with_raw_tabs(doc: &str) -> String {
    let mut quoted = false;

    doc.bytes()
        .map(|b| match b {
            b'"' => {
                quoted = !quoted;
                "%22".to_string()
            }
            b'\t' | b'\n' | b'\r' if quoted => (b as char).to_string(),
            b if b.is_ascii_alphanumeric() => (b as char).to_string(),
            b => format!("%{b:02X}"),
        })
        .collect()
}

/// The document the page's loadURL hands to the parser for the first candidate `url`, or `None` when it requests the
/// next candidate (a parse that fails is not counted).
fn page_document(url: &str) -> Option<String> {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let mut document = None;
    let urls = vec![url.to_string(), w("w.wrl")];

    rt.load_page_document(URL, &urls, |_, loaded| {
        let Loaded::Data(data, _) = loaded else {
            return Err(Some("Error: no script".into()));
        };

        document = Some(String::from_utf8(data).expect("UTF-8"));
        Ok(())
    });

    assert_eq!(
        net.requested(),
        if document.is_some() {
            vec![]
        } else {
            vec![w("w.wrl")]
        },
        "{url:?}"
    );

    document
}

/// The world's loadURL. A raw tab, LF or CR after `data:` in any case, also after leading tabs and newlines, is part
/// of the document (FileLoader's text branch takes only `data:`; the browser's fetch decodes the others). After a
/// leading space or with a tab in the scheme, Chrome 156 removes them like the URL standard. `;<TAB>base64` is no
/// base64 flag after `DATA:`, but is one after `da<TAB>ta:`.
#[test]
fn page_load_url_keeps_a_raw_tab_after_the_data_scheme() {
    let doc = "#VRML V2.0 utf8\nDEF Info WorldInfo { title \"a\tb\nc\rd\" }\n";
    let body = escaped_with_raw_tabs(doc);
    let removed = "#VRML V2.0 utf8\nDEF Info WorldInfo { title \"abcd\" }\n";
    let cases = [
        (format!("data:model/vrml,{body}"), Some(doc)),
        (format!("DATA:model/vrml,{body}"), Some(doc)),
        (format!("\t\nData:model/vrml,{body}"), Some(doc)),
        (format!("da\tta:model/vrml,{body}"), Some(removed)),
        (format!(" DATA:model/vrml,{body}"), Some(removed)),
        (
            format!("D\nA\rTA:model/vrml;base64,{}", base64(doc.as_bytes())),
            Some(doc),
        ),
        (
            format!("da\tta:model/vrml;\tbase64,{}", base64(doc.as_bytes())),
            Some(doc),
        ),
        (
            format!("DATA:model/vrml;\tbase64,{}", base64(doc.as_bytes())),
            Some(&*base64(doc.as_bytes())),
        ),
        ("DA\tTA:model/vrml;base64,QQ=\t@".to_string(), None),
    ];

    for (url, expected) in cases {
        assert_eq!(page_document(&url).as_deref(), expected, "{url:?}");
    }
}

/// Images: after `data:`, `;<TAB>base64` is no base64 flag, so the GIF does not decode and the next candidate is
/// requested; after `da<TAB>ta:` or ` data:` the tab is removed and the GIF decodes. A tab in the base64 body changes
/// nothing, and no other candidate is requested.
#[test]
fn image_data_urls_with_raw_tabs_like_chromium() {
    let gif = base64(GIF);
    let (head, tail) = gif.split_at(8);
    let image = |url: &str, next: &str| {
        format!(
            "#VRML V2.0 utf8\nShape {{ appearance Appearance {{ texture ImageTexture {{ url [ \"{url}\" \"{next}\" ] }} }} geometry Box {{ }} }}\n"
        )
    };

    for url in [
        format!("DA\tTA:image/gif;base64,{head}\t{tail}"),
        format!("data:image/gif;base64,{head}\n{tail}"),
        format!("data:image/gif\t;base64,{gif}"),
        format!("da\tta:image/gif;\tbase64,{gif}"),
        format!(" data:image/gif;\tbase64,{gif}"),
    ] {
        let net = TestFetcher::default();
        let mut s = begin(&image(&url, "tex/i1.gif"), &net);

        assert!(poll(&mut s), "{url:?}");
        assert_eq!(net.requested(), Vec::<String>::new(), "{url:?}");
        assert!(
            !s.take_log().join("\n").contains("Couldn't load"),
            "{url:?}"
        );
    }

    let net = TestFetcher::default();
    let mut s = begin(
        &image(&format!("data:image/gif;\tbase64,{gif}"), "tex/i9.gif"),
        &net,
    );

    assert!(!poll(&mut s));
    assert_eq!(net.requested(), [w("tex/i9.gif")]);
    net.respond(&w("tex/i9.gif"), Ok(GIF));
    assert!(poll(&mut s));
    assert_eq!(net.requested(), [w("tex/i9.gif")]);

    // A data URL that does not decode, then a file: one request.
    let net = TestFetcher::default();
    let mut s = begin(
        &image("DA\tTA:image/gif;base64,R0lG@@@@", "tex/green.gif"),
        &net,
    );

    assert!(!poll(&mut s));
    assert_eq!(net.requested(), [w("tex/green.gif")]);
    net.respond(&w("tex/green.gif"), Ok(GIF));
    assert!(poll(&mut s));
    assert_eq!(
        s.take_log()
            .into_iter()
            .filter(|l| l.contains("Couldn't load"))
            .collect::<Vec<_>>(),
        [
            "warn: ImageTexture: Couldn't load data URL, trying URL 2 of 2. TypeError: Failed to fetch"
        ]
    );
}

/// An http(s) or relative candidate still loses every tab and newline (the URL standard and Chromium agree).
#[test]
fn http_candidates_drop_raw_tabs_and_newlines() {
    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nShape { appearance Appearance { texture ImageTexture { url [ \"te\tx/gr\neen.gif\" \"http://example.test/worlds/tex/b\tad.png\" ] } } geometry Box { } }\n",
        &net,
    );

    assert!(!poll(&mut s));
    assert_eq!(net.requested(), [w("tex/green.gif")]);
    net.respond(&w("tex/green.gif"), Ok(GIF));
    assert!(poll(&mut s));
    assert_eq!(net.requested(), [w("tex/green.gif")]);
}

/// The 36 bytes of a triangle with a 0x09 byte (x of the second point is `f32::from_bits (0x3f800009)`).
fn triangle_with_a_tab_byte() -> Vec<u8> {
    [
        0.0f32,
        0.0,
        0.0,
        f32::from_bits(0x3f80_0009),
        0.0,
        0.0,
        0.0,
        1.0,
        0.0,
    ]
    .iter()
    .flat_map(|f| f.to_le_bytes())
    .collect()
}

/// A glTF buffer as a data URL: the scheme in any case, and a raw tab (JSON `\t`) in the percent-encoded body in place
/// of the byte 0x09, which Chrome keeps after `data:`. Only the glTF file is requested; a buffer that does not decode
/// fails.
#[test]
fn gltf_data_buffers_with_raw_tabs_like_chromium() {
    let bytes = triangle_with_a_tab_byte();
    let body: String = bytes
        .iter()
        .map(|&b| {
            if b == 9 {
                "\\t".to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect();

    for uri in [
        format!("data:application/octet-stream,{body}"),
        format!("DATA:application/octet-stream,{body}"),
        format!("\\tDATA:application/octet-stream,{body}"),
        format!("Da\\nta:application/octet-stream;base64,{}", base64(&bytes)),
    ] {
        let net = TestFetcher::default();
        let mut s = begin(
            "#VRML V2.0 utf8\nInline { url [ \"models/t.gltf\" \"models/u.gltf\" ] }\n",
            &net,
        );

        assert!(!poll(&mut s));
        net.respond(&w("models/t.gltf"), Ok(&gltf(&[&uri])));
        assert!(poll(&mut s), "{uri:?}");
        assert_eq!(net.requested(), [w("models/t.gltf")], "{uri:?}");
        assert!(!s.take_log().join("\n").contains("Couldn't"), "{uri:?}");
    }

    let net = TestFetcher::default();
    let mut s = begin(
        "#VRML V2.0 utf8\nInline { url [ \"models/t.gltf\" \"models/u.gltf\" ] }\n",
        &net,
    );

    assert!(!poll(&mut s));
    net.respond(
        &w("models/t.gltf"),
        Ok(&gltf(&["DA\\tTA:application/octet-stream;base64,AA@@"])),
    );
    poll(&mut s);

    let log = s.take_log().join("\n");

    assert!(
        log.contains("Couldn't load URL 'data URL'. TypeError: Failed to fetch")
            || log.contains("Couldn't load data URL"),
        "{log}"
    );
    assert!(!net.requested().iter().any(|u| u.contains("AA@@")));
}

// --- a glTF as the page's world (finding F2 of PR #124) --------------------------------------------------------------

/// oracle/web-browser-gltf-world-01-baseline, gltfWorld.external and gltfWorld.two: the world waits for every buffer
/// of its glTF (`Promise .all`: they are requested together) and parses when they are in.
#[test]
fn a_gltf_world_waits_for_its_buffers() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("models/a.gltf")];

    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Suspended, false));
    net.respond(&w("models/a.gltf"), Ok(&gltf(&["one.bin", "two.bin"])));
    assert_eq!(
        page_step(&mut rt, &urls),
        (LoadResult::Suspended, false),
        "parsed before its buffers"
    );
    assert_eq!(
        net.requested(),
        [w("models/a.gltf"), w("models/one.bin"), w("models/two.bin")]
    );

    net.respond(&w("models/two.bin"), Ok(&triangle()));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Suspended, false));
    net.respond(&w("models/one.bin"), Ok(&triangle()));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Loaded, true));
    assert_eq!(page_log(&mut rt), Vec::<String>::new());
    assert!(rt.fetch_resume.is_empty());
    // Each buffer is requested once; a parse reads the responses that are in.
    assert_eq!(
        net.requested(),
        [w("models/a.gltf"), w("models/one.bin"), w("models/two.bin")]
    );
}

/// gltfWorld.missing and gltfWorld.oneMissing: a buffer that fails rejects the candidate with no error of its own;
/// only the buffer's FileLoader logs. The load fails ("Couldn't load X3D file." of the page).
#[test]
fn a_gltf_world_whose_buffer_fails_logs_only_the_buffer() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("models/a.gltf")];

    page_step(&mut rt, &urls);
    net.respond(&w("models/a.gltf"), Ok(&gltf(&["tri.bin", "gone.bin"])));
    page_step(&mut rt, &urls);
    net.respond(&w("models/tri.bin"), Ok(&triangle()));
    net.respond(&w("models/gone.bin"), Err("Error: Not Found"));

    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Failed, false));
    assert_eq!(
        page_log(&mut rt),
        [format!(
            "error: Couldn't load URL '{}'. Error: Not Found",
            w("models/gone.bin")
        )]
    );
    assert!(rt.fetch_resume.is_empty());
}

/// gltfWorld.badBase64: a data URL buffer whose base64 does not decode fails like a request.
#[test]
fn a_gltf_world_whose_data_url_buffer_does_not_decode_fails() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("models/a.gltf")];

    page_step(&mut rt, &urls);
    net.respond(
        &w("models/a.gltf"),
        Ok(&gltf(&["data:application/octet-stream;base64,@@@@"])),
    );

    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Failed, false));
    assert_eq!(
        page_log(&mut rt),
        ["error: Couldn't load data URL. TypeError: Failed to fetch"]
    );
    assert_eq!(net.requested(), [w("models/a.gltf")]);
}

/// gltfWorld.fallback: after a glTF whose buffer failed, the next candidate loads; the failed candidate gives no
/// "trying URL 2 of 2" warning (its rejection has no error).
#[test]
fn a_gltf_world_whose_buffer_fails_moves_on_to_the_next_candidate() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("models/a.gltf"), w("models/b.gltf")];

    page_step(&mut rt, &urls);
    net.respond(&w("models/a.gltf"), Ok(&gltf(&["gone.bin"])));
    page_step(&mut rt, &urls);
    net.respond(&w("models/gone.bin"), Err("Error: Not Found"));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Suspended, false));
    net.respond(&w("models/b.gltf"), Ok(&gltf(&["tri.bin"])));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Suspended, false));
    net.respond(&w("models/tri.bin"), Ok(&triangle()));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Loaded, true));

    assert_eq!(
        page_log(&mut rt),
        [format!(
            "error: Couldn't load URL '{}'. Error: Not Found",
            w("models/gone.bin")
        )]
    );
}

/// A data URL buffer that decodes needs no request (gltfWorld.data).
#[test]
fn a_gltf_world_with_a_data_url_buffer_loads_at_once() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("models/a.gltf")];
    // triangle () in base64.
    let data =
        "data:application/octet-stream;base64,AAAAAAAAAAAAAAAAAACAPwAAAAAAAAAAAAAAAAAAgD8AAAAA";

    page_step(&mut rt, &urls);
    net.respond(&w("models/a.gltf"), Ok(&gltf(&[data])));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Loaded, true));
    assert_eq!(net.requested(), [w("models/a.gltf")]);
    assert_eq!(page_log(&mut rt), Vec::<String>::new());
}

// --- a glTF world buffer that fails while another is in flight (CodeRabbit finding of PR #126) ----------------------

/// oracle/web-browser-gltf-world-02-baseline, gltfWorldHeld.failThenHeld and gltfWorldHeld.heldThenFail: `Promise .all`
/// rejects at the first buffer that fails, so the candidate fails while the other buffer is still in flight. That
/// buffer's FileLoader goes on: it is followed, and it logs nothing when it loads.
#[test]
fn a_gltf_world_buffer_that_fails_rejects_while_another_is_in_flight() {
    for buffers in [["gone.bin", "held.bin"], ["held.bin", "gone.bin"]] {
        let net = TestFetcher::default();
        let mut rt = page_loader(&net);
        let urls = vec![w("models/a.gltf")];

        page_step(&mut rt, &urls);
        net.respond(&w("models/a.gltf"), Ok(&gltf(&buffers)));
        assert_eq!(page_step(&mut rt, &urls), (LoadResult::Suspended, false));
        net.respond(&w("models/gone.bin"), Err("Error: Not Found"));

        assert_eq!(
            page_step(&mut rt, &urls),
            (LoadResult::Failed, false),
            "{buffers:?}: the candidate waited for the buffer in flight"
        );
        assert_eq!(
            page_log(&mut rt),
            [format!(
                "error: Couldn't load URL '{}'. Error: Not Found",
                w("models/gone.bin")
            )]
        );
        assert!(rt.fetch_resume.is_empty());
        assert!(
            rt.follow_buffers(),
            "{buffers:?}: the held buffer is followed"
        );

        net.respond(&w("models/held.bin"), Ok(&triangle()));
        assert!(!rt.follow_buffers());
        assert_eq!(page_log(&mut rt), Vec::<String>::new());
    }
}

/// gltfWorldHeld.bothFailSecondFirst and gltfWorldHeld.bothFailFirstFirst: when both buffers fail, each FileLoader
/// logs when its own fetch fails, so the messages come in the order of the failures, not in buffer order.
#[test]
fn gltf_world_buffers_that_fail_log_in_the_order_they_fail() {
    for (buffers, first, second) in [
        (["late.bin", "gone.bin"], "gone.bin", "late.bin"),
        (["gone.bin", "late.bin"], "gone.bin", "late.bin"),
    ] {
        let net = TestFetcher::default();
        let mut rt = page_loader(&net);
        let urls = vec![w("models/a.gltf")];
        let line = |name: &str| {
            format!(
                "error: Couldn't load URL '{}'. Error: Not Found",
                w(&format!("models/{name}"))
            )
        };

        page_step(&mut rt, &urls);
        net.respond(&w("models/a.gltf"), Ok(&gltf(&buffers)));
        page_step(&mut rt, &urls);
        net.respond(&w(&format!("models/{first}")), Err("Error: Not Found"));
        assert_eq!(page_step(&mut rt, &urls), (LoadResult::Failed, false));
        assert_eq!(page_log(&mut rt), [line(first)], "{buffers:?}");

        net.respond(&w(&format!("models/{second}")), Err("Error: Not Found"));
        assert!(!rt.follow_buffers());
        assert_eq!(page_log(&mut rt), [line(second)], "{buffers:?}");
    }
}

/// gltfWorldHeld.fallback: the next candidate is requested while the failed candidate's other buffer is in flight,
/// and it becomes the world before that buffer comes.
#[test]
fn a_gltf_world_buffer_that_fails_moves_on_while_another_is_in_flight() {
    let net = TestFetcher::default();
    let mut rt = page_loader(&net);
    let urls = vec![w("models/a.gltf"), w("models/b.gltf")];

    page_step(&mut rt, &urls);
    net.respond(&w("models/a.gltf"), Ok(&gltf(&["gone.bin", "held.bin"])));
    page_step(&mut rt, &urls);
    net.respond(&w("models/gone.bin"), Err("Error: Not Found"));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Suspended, false));
    assert!(net.requested().contains(&w("models/b.gltf")));

    net.respond(&w("models/b.gltf"), Ok(&gltf(&["tri.bin"])));
    page_step(&mut rt, &urls);
    net.respond(&w("models/tri.bin"), Ok(&triangle()));
    assert_eq!(page_step(&mut rt, &urls), (LoadResult::Loaded, true));
    assert!(rt.follow_buffers(), "the held buffer is still followed");

    net.respond(&w("models/held.bin"), Ok(&triangle()));
    assert!(!rt.follow_buffers());
    assert_eq!(
        page_log(&mut rt),
        [format!(
            "error: Couldn't load URL '{}'. Error: Not Found",
            w("models/gone.bin")
        )]
    );
}
