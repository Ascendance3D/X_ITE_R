//! IconAnnotation image loading: an enhancement beyond pinned X_ITE 16.5.0 (owner decision 2026-11-12, ledger entry
//! node:IconAnnotation, ADR 0002). The dormant X_ITE class cannot initialize (X3DUrlObject.initialize reads `_load`,
//! which the class lacks) and its requestImmediateLoad is empty.
//!
//! Supporting evidence, not ISO: the Web3D X3D 4 working draft WD2, clause 43, says that browsers shall support JPEG and
//! PNG for IconAnnotation. The adopted ISO/IEC 19775-1:2023 text has no Annotation clause.
//!
//! The port fetches the url like an ImageTexture (X3DUrlObject requestImmediateLoad, FileLoader fallback): a candidate
//! that does not fetch, or that the candidate check rejects, is skipped. The first candidate that passes completes the
//! load, and no candidate left fails it. Nothing is displayed: the draft gives no normative layout. The node has no
//! `load` field, so it loads as with the X3DUrlObject default, TRUE.
//!
//! The candidate check is the one of ImageTexture (`check_image`): data in no image format fails the candidate, and an
//! SVG document does not. The check does not decode the pixels, so a damaged file with a valid image header passes, as
//! for ImageTexture. The tests cover JPEG and PNG, the formats the draft names, and data in no image format.

use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::Rc;

use x3d_fields::NodeId;
use x3d_runtime::nodes::LoadState;
use x3d_runtime::{Fetcher, Runtime};

type Requests = Rc<RefCell<Vec<String>>>;

/// The fetcher records each requested URL, in order.
struct Files(HashMap<String, Vec<u8>>, Requests);

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        self.1.borrow_mut().push(url.as_str().to_string());
        self.0
            .get(url.as_str())
            .cloned()
            .ok_or_else(|| "Error: Not Found".into())
    }
}

fn encoded(format: image::ImageFormat) -> Vec<u8> {
    let img = image::RgbImage::from_pixel(4, 2, image::Rgb([200, 40, 10]));
    let mut out = std::io::Cursor::new(vec![]);
    img.write_to(&mut out, format).unwrap();
    out.into_inner()
}

fn files(requests: Requests) -> Files {
    let mut map = HashMap::new();
    map.insert(
        "http://oracle/icon.png".to_string(),
        encoded(image::ImageFormat::Png),
    );
    map.insert(
        "http://oracle/icon.jpg".to_string(),
        encoded(image::ImageFormat::Jpeg),
    );
    map.insert(
        "http://oracle/not-an-image.png".to_string(),
        b"not an image".to_vec(),
    );
    Files(map, requests)
}

fn run(urls: &str) -> (Runtime, NodeId, Requests) {
    let text = format!(
        "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT Annotation:1\nDEF IA IconAnnotation {{ url [ {urls} ] }}\n"
    );
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let requests = Requests::default();
    let mut rt = Runtime::new(scene, Box::new(files(requests.clone())));

    rt.load_world();
    let node = rt.scene.get_named_node(x3d_scene::ROOT, "IA").unwrap();
    (rt, node, requests)
}

fn load_state(rt: &Runtime, node: NodeId) -> LoadState {
    rt.url_load_state(node)
}

#[test]
fn png_and_jpeg_complete() {
    for url in ["\"icon.png\"", "\"icon.jpg\""] {
        let (rt, node, _) = run(url);
        assert_eq!(load_state(&rt, node), LoadState::Complete, "{url}");
    }
}

#[test]
fn fallback_skips_missing_and_non_image_candidates() {
    let (rt, node, requests) = run("\"missing.png\" \"not-an-image.png\" \"icon.jpg\"");
    assert_eq!(load_state(&rt, node), LoadState::Complete);
    assert_eq!(
        requests.borrow().as_slice(),
        [
            "http://oracle/missing.png",
            "http://oracle/not-an-image.png",
            "http://oracle/icon.jpg",
        ],
        "each candidate is tried, in url order"
    );

    let (rt, node, _) = run("\"missing.png\" \"not-an-image.png\"");
    assert_eq!(load_state(&rt, node), LoadState::Failed);
}

#[test]
fn first_image_candidate_ends_the_fallback() {
    let (rt, node, requests) = run("\"icon.jpg\" \"icon.png\"");
    assert_eq!(load_state(&rt, node), LoadState::Complete);
    assert_eq!(
        requests.borrow().as_slice(),
        ["http://oracle/icon.jpg"],
        "no candidate after the first image is fetched"
    );
}

#[test]
fn empty_url_does_not_start() {
    let (rt, node, requests) = run("");
    assert_eq!(load_state(&rt, node), LoadState::NotStarted);
    assert!(requests.borrow().is_empty());
}

#[test]
fn candidates_are_the_ones_of_image_texture() {
    let mut damaged = encoded(image::ImageFormat::Png);
    damaged.truncate(20);
    let mut map = files(Requests::default()).0;
    map.insert("http://oracle/damaged.png".to_string(), damaged);
    map.insert(
        "http://oracle/icon.svg".to_string(),
        b"<?xml version=\"1.0\"?><svg xmlns=\"http://www.w3.org/2000/svg\" width=\"4\" height=\"2\"/>".to_vec(),
    );

    for urls in [
        "\"icon.png\"",
        "\"icon.jpg\"",
        "\"icon.svg\"",
        "\"damaged.png\"",
        "\"not-an-image.png\"",
        "\"missing.png\" \"not-an-image.png\" \"icon.jpg\"",
    ] {
        let text = format!(
            "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT Annotation:1\n\
             DEF IA IconAnnotation {{ url [ {urls} ] }}\n\
             Shape {{ appearance Appearance {{ texture DEF IT ImageTexture {{ url [ {urls} ] }} }} geometry Box {{}} }}\n"
        );
        let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
        let mut rt = Runtime::new(scene, Box::new(Files(map.clone(), Requests::default())));
        rt.load_world();
        let icon = rt.scene.get_named_node(x3d_scene::ROOT, "IA").unwrap();
        let texture = rt.scene.get_named_node(x3d_scene::ROOT, "IT").unwrap();
        assert_eq!(
            load_state(&rt, icon),
            load_state(&rt, texture),
            "IconAnnotation and ImageTexture for {urls}"
        );
    }
}
