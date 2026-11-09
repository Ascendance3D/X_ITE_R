//! The SVG parser reads the options of its own browser (`SVGParser.js` lines 113-141: `scene .getBrowser ()`'s
//! PrimitiveQuality and TextureQuality, read in the constructor). Two runtimes on one thread, or a runtime whose
//! option changed, must not parse with the value another browser last set.

use x3d_parse::SvgQuality;
use x3d_runtime::browser_options::OptionValue;
use x3d_runtime::{Fetcher, Runtime, parse_bytes_with_fetcher};

const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">
<circle cx="20" cy="20" r="15" fill="red"/><path d="M0 0 C10 40 30 40 40 0" stroke="blue" fill="none"/>
<linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>
<rect width="40" height="40" fill="url(#g)"/></svg>"#;
const URL: &str = "http://oracle/q.svg";

struct NoFetch;

impl Fetcher for NoFetch {
    fn fetch(&self, _: &url::Url) -> Result<Vec<u8>, String> {
        Err("Failed".into())
    }
}

/// The text of the scene SVG gives with explicit options.
fn reference(primitive: SvgQuality, texture: SvgQuality) -> String {
    x3d_parse::set_svg_quality(primitive, texture);

    x3d_parse::parse_bytes(SVG, URL).unwrap().to_vrml_string()
}

fn runtime(primitive: &str, texture: &str) -> Runtime {
    let scene = x3d_parse::parse_bytes(b"#VRML V2.0 utf8\n", "http://oracle/t.wrl").unwrap();
    let mut rt = Runtime::new(scene, Box::new(NoFetch));

    rt.browser_options
        .set_option("PrimitiveQuality", OptionValue::String(primitive.into()));
    rt.browser_options
        .set_option("TextureQuality", OptionValue::String(texture.into()));
    rt
}

/// The page's loadURL candidate parse of `rt`.
fn parse_in(rt: &mut Runtime) -> String {
    rt.parse_candidate(SVG, URL).scene.unwrap().to_vrml_string()
}

#[test]
fn references_differ() {
    let low = reference(SvgQuality::Low, SvgQuality::Low);
    let med = reference(SvgQuality::Medium, SvgQuality::Medium);
    let high = reference(SvgQuality::High, SvgQuality::High);

    assert_ne!(low, med);
    assert_ne!(med, high);
}

#[test]
fn two_browsers_on_one_thread_do_not_leak() {
    let low = reference(SvgQuality::Low, SvgQuality::Low);
    let high = reference(SvgQuality::High, SvgQuality::High);
    let mut a = runtime("LOW", "LOW");
    let mut b = runtime("HIGH", "HIGH");

    assert_eq!(parse_in(&mut a), low);
    assert_eq!(parse_in(&mut b), high);
    assert_eq!(parse_in(&mut a), low);
}

#[test]
fn changed_option_reaches_the_next_top_level_parse() {
    let low = reference(SvgQuality::Low, SvgQuality::Low);
    let high = reference(SvgQuality::High, SvgQuality::Low);
    let mut rt = runtime("LOW", "LOW");

    assert_eq!(parse_in(&mut rt), low);
    rt.browser_options
        .set_option("PrimitiveQuality", OptionValue::String("HIGH".into()));
    assert_eq!(parse_in(&mut rt), high);
}

/// A world parsed before its runtime exists has the default options, whatever a browser set before.
#[test]
fn a_new_world_parse_does_not_see_another_browsers_options() {
    let med = reference(SvgQuality::Medium, SvgQuality::Medium);
    // Another browser on this thread parsed last, with LOW.
    x3d_parse::set_svg_quality(SvgQuality::Low, SvgQuality::Low);

    let got = parse_bytes_with_fetcher(&NoFetch, SVG, URL)
        .scene
        .unwrap()
        .to_vrml_string();

    assert_eq!(got, med);
}
