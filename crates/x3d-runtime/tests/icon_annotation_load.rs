//! IconAnnotation image loading: an enhancement beyond pinned X_ITE 16.5.0 (owner decision 2026-11-12, ledger entry
//! node:IconAnnotation, ADR 0002). The dormant X_ITE class cannot initialize (X3DUrlObject.initialize reads `_load`,
//! which the class lacks) and its requestImmediateLoad is empty.
//!
//! Supporting evidence, not ISO: the Web3D X3D 4 working draft WD2, clause 43, says that browsers shall support JPEG and
//! PNG for IconAnnotation. The adopted ISO/IEC 19775-1:2023 text has no Annotation clause.
//!
//! The port fetches the url like an ImageTexture (X3DUrlObject requestImmediateLoad, FileLoader fallback): a candidate
//! that does not fetch or does not decode is skipped, the first image that decodes completes the load, and no candidate
//! left fails it. Nothing is displayed: the draft gives no normative layout. The node has no `load` field, so it loads as
//! with the X3DUrlObject default, TRUE.

use std::collections::HashMap;

use x3d_fields::NodeId;
use x3d_runtime::nodes::LoadState;
use x3d_runtime::{Fetcher, Runtime};

struct Files(HashMap<String, Vec<u8>>);

impl Fetcher for Files {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
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

fn files() -> Files {
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
    Files(map)
}

fn run(urls: &str) -> (Runtime, NodeId) {
    let text = format!(
        "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT Annotation:1\nDEF IA IconAnnotation {{ url [ {urls} ] }}\n"
    );
    let scene = x3d_parse::parse_bytes(text.as_bytes(), "http://oracle/t.x3dv").unwrap();
    let mut rt = Runtime::new(scene, Box::new(files()));

    rt.load_world();
    let node = rt.scene.get_named_node(x3d_scene::ROOT, "IA").unwrap();
    (rt, node)
}

fn load_state(rt: &Runtime, node: NodeId) -> LoadState {
    rt.url_load_state(node)
}

#[test]
fn png_and_jpeg_complete() {
    for url in ["\"icon.png\"", "\"icon.jpg\""] {
        let (rt, node) = run(url);
        assert_eq!(load_state(&rt, node), LoadState::Complete, "{url}");
    }
}

#[test]
fn fallback_skips_missing_and_undecodable_candidates() {
    let (rt, node) = run("\"missing.png\" \"not-an-image.png\" \"icon.jpg\"");
    assert_eq!(load_state(&rt, node), LoadState::Complete);

    let (rt, node) = run("\"missing.png\" \"not-an-image.png\"");
    assert_eq!(load_state(&rt, node), LoadState::Failed);
}

#[test]
fn empty_url_does_not_start() {
    let (rt, node) = run("");
    assert_eq!(load_state(&rt, node), LoadState::NotStarted);
}
