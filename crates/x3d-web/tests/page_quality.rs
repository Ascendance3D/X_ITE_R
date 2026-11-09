//! The page's PrimitiveQuality and TextureQuality reach the engine (BrowserOptions belong to the browser and persist
//! across worlds): a top-level world parsed after the page set them parses SVG with them (SVGParser.js lines 113-141 read
//! them from its browser), a world starts with them, and the option a world Script sets carries to the next world.

use x3d_parse::SvgQuality;
use x3d_runtime::browser_options::OptionValue;
use x3d_web::session::{Session, init_page_options, set_page_quality};

const SVG: &[u8] = br#"<svg xmlns="http://www.w3.org/2000/svg" width="40" height="40">
<circle cx="20" cy="20" r="15" fill="red"/><path d="M0 0 C10 40 30 40 0 0" stroke="blue" fill="none"/>
<linearGradient id="g"><stop offset="0" stop-color="red"/><stop offset="1" stop-color="blue"/></linearGradient>
<rect width="40" height="40" fill="url(#g)"/></svg>"#;
const URL: &str = "http://example.test/q.svg";
const NOW: f64 = 946_684_800.0;

struct NoFetch;

impl x3d_runtime::Fetcher for NoFetch {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        Err(format!("no network in this test: {url}"))
    }
}

/// The scene text of SVG parsed with explicit options (the parser itself, no page).
fn reference(primitive: SvgQuality, texture: SvgQuality) -> String {
    x3d_parse::set_svg_quality(primitive, texture);

    x3d_parse::parse_bytes(SVG, URL).unwrap().to_vrml_string()
}

/// The text of the SVG as the page's top-level parse gives it.
fn page_parse() -> String {
    let world = Session::parse(SVG, URL, Box::new(NoFetch)).expect("the SVG parses");
    let mut session = Session::begin(world, NOW, 64, 64);

    session.print_scene("vrml").expect("prints")
}

/// The runtime of a world (its Scripts set browser options through its `browser_options`).
fn blank() -> x3d_runtime::Runtime {
    let scene = x3d_parse::parse_bytes(b"#VRML V2.0 utf8\n", URL).unwrap();

    x3d_runtime::Runtime::new(scene, Box::new(NoFetch))
}

#[test]
fn the_page_options_reach_the_top_level_svg_parse() {
    let low = reference(SvgQuality::Low, SvgQuality::Low);
    let high = reference(SvgQuality::High, SvgQuality::Medium);
    let medium = reference(SvgQuality::Medium, SvgQuality::Medium);

    assert_ne!(low, medium);
    assert_ne!(high, medium);

    set_page_quality("LOW", "LOW");
    assert_eq!(page_parse(), low);

    set_page_quality("HIGH", "MEDIUM");
    assert_eq!(page_parse(), high);

    // An option that is not LOW or HIGH is MEDIUM.
    set_page_quality("whatever", "");
    assert_eq!(page_parse(), medium);
}

#[test]
fn a_world_starts_with_the_page_options() {
    let mut rt = blank();

    init_page_options(
        &mut rt,
        &[
            ("PrimitiveQuality".into(), OptionValue::String("LOW".into())),
            ("TextureQuality".into(), OptionValue::String("HIGH".into())),
        ],
    );

    let o = &rt.browser_options;

    assert_eq!(o.get_option("PrimitiveQuality").unwrap().as_str(), "LOW");
    assert_eq!(o.get_option("TextureQuality").unwrap().as_str(), "HIGH");
}

/// A Script's setBrowserOption comes back to the page, which keeps it for the next world (X_ITE has one BrowserOptions).
#[test]
fn a_script_option_carries_to_the_next_world() {
    let high = reference(SvgQuality::High, SvgQuality::Medium);
    let mut rt = blank();

    rt.browser_options
        .set_script_option("PrimitiveQuality", OptionValue::String("HIGH".into()));

    let mut primitive = String::from("MEDIUM");

    for (name, value) in rt.browser_options.take_script_changes() {
        if name == "PrimitiveQuality" {
            primitive = value.as_str().to_string();
        }
    }

    set_page_quality(&primitive, "MEDIUM");
    assert_eq!(page_parse(), high);
}
