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
            return Err("Error: no script".into());
        };

        Session::parse_logged(
            &data,
            url.as_str(),
            Box::new(TestFetcher::default()),
            |line| rt.log.push(line),
        )?;
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
