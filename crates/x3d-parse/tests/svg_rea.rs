//! REA batch 08: the SVG parser (GoldenGate dispatch, SVGParser.js) against pinned X_ITE.
//! Baseline: `oracle/synthetic-rea-svg/svg-baseline.json`, written by
//! `node oracle/x_ite-svg.mjs synthetic-rea-svg synthetic-rea-svg/svg-baseline.json`.
//!
//! For every input X_ITE loaded, the Rust scene must print the same toVRMLString, toXMLString and
//! toJSONString text; an input X_ITE rejected must be rejected too, with the same message.
//!
//! One normalization: a gradient is a canvas texture, an `ImageTexture` url holding a PNG data
//! URL. The file bytes of a PNG depend on the deflate implementation, so each data URL is decoded
//! on both sides, the pixels are compared (limits below), and the URL is replaced by its size
//! before the text comparison. Nothing else is normalized.
//!
//! `SVG_ONLY=<substring>` restricts the run; `SVG_OUT=<dir>` writes `.rust` / `.xite` files for each
//! difference.

use std::io::Read;
use std::path::Path;

/// Pixel limits for a gradient texture. Measured on the pinned page; see the lane report.
const MAX_CHANNEL_DIFF: i32 = 1;
const MAX_DIFF_FRACTION: f64 = 0.001;

fn first_difference(expected: &str, actual: &str) -> String {
    let (e, a): (Vec<&str>, Vec<&str>) = (expected.split('\n').collect(), actual.split('\n').collect());

    for i in 0..e.len().max(a.len()) {
        if e.get(i) != a.get(i) {
            return format!("line {}:\n  X_ITE: {:.300}\n  Rust:  {:.300}", i + 1, e.get(i).unwrap_or(&"<EOF>"), a.get(i).unwrap_or(&"<EOF>"));
        }
    }

    String::new()
}

fn base64_decode(s: &str) -> Vec<u8> {
    let value = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        b'/' => 63,
        _ => 255,
    };
    let mut out = vec![];
    let mut buffer = 0u32;
    let mut bits = 0;

    for c in s.bytes() {
        let v = value(c);

        if v == 255 {
            continue;
        }

        buffer = buffer << 6 | v as u32;
        bits += 6;

        if bits >= 8 {
            bits -= 8;
            out.push((buffer >> bits) as u8 & 0xff);
        }
    }

    out
}

/// (width, height, RGBA) of an 8-bit RGBA, non-interlaced PNG.
fn decode_png(data: &[u8]) -> (usize, usize, Vec<u8>) {
    assert_eq!(&data[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);

    let mut pos = 8;
    let (mut w, mut h) = (0, 0);
    let mut idat = vec![];

    while pos < data.len() {
        let len = u32::from_be_bytes(data[pos..pos + 4].try_into().unwrap()) as usize;
        let kind = &data[pos + 4..pos + 8];
        let body = &data[pos + 8..pos + 8 + len];

        match kind {
            b"IHDR" => {
                w = u32::from_be_bytes(body[0..4].try_into().unwrap()) as usize;
                h = u32::from_be_bytes(body[4..8].try_into().unwrap()) as usize;
                assert_eq!((body[8], body[9], body[12]), (8, 6, 0), "only 8-bit RGBA, not interlaced");
            }
            b"IDAT" => idat.extend_from_slice(body),
            _ => {}
        }

        pos += 12 + len;
    }

    let mut raw = vec![];

    flate2::read::ZlibDecoder::new(&idat[..]).read_to_end(&mut raw).unwrap();

    let stride = w * 4;
    let mut out = vec![0u8; stride * h];

    for y in 0..h {
        let filter = raw[y * (stride + 1)];
        let line = &raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)];

        for i in 0..stride {
            let a = if i >= 4 { out[y * stride + i - 4] as i32 } else { 0 };
            let b = if y > 0 { out[(y - 1) * stride + i] as i32 } else { 0 };
            let c = if y > 0 && i >= 4 { out[(y - 1) * stride + i - 4] as i32 } else { 0 };
            let predictor = match filter {
                0 => 0,
                1 => a,
                2 => b,
                3 => (a + b) / 2,
                _ => {
                    let p = a + b - c;
                    let (pa, pb, pc) = ((p - a).abs(), (p - b).abs(), (p - c).abs());

                    if pa <= pb && pa <= pc {
                        a
                    } else if pb <= pc {
                        b
                    } else {
                        c
                    }
                }
            };

            out[y * stride + i] = (line[i] as i32 + predictor) as u8;
        }
    }

    (w, h, out)
}

const PREFIX: &str = "data:image/png;base64,";

/// Replaces every PNG data URL by `PNG <w>x<h>`; returns the decoded images in order.
fn extract_pngs(text: &str) -> (String, Vec<(usize, usize, Vec<u8>)>) {
    let mut out = String::new();
    let mut images = vec![];
    let mut rest = text;

    while let Some(i) = rest.find(PREFIX) {
        out.push_str(&rest[..i]);

        let body = &rest[i + PREFIX.len()..];
        let end = body.find(|c: char| !(c.is_ascii_alphanumeric() || c == '+' || c == '/' || c == '=')).unwrap_or(body.len());
        let (w, h, rgba) = decode_png(&base64_decode(&body[..end]));

        out.push_str(&format!("PNG {w}x{h}"));
        images.push((w, h, rgba));
        rest = &body[end..];
    }

    out.push_str(rest);
    (out, images)
}

/// (max channel difference, number of pixels that differ at all, pixel count)
fn compare_pixels(a: &(usize, usize, Vec<u8>), b: &(usize, usize, Vec<u8>)) -> (i32, usize, usize) {
    assert_eq!((a.0, a.1), (b.0, b.1), "texture size");

    let mut max = 0;
    let mut differing = 0;

    for (pa, pb) in a.2.chunks_exact(4).zip(b.2.chunks_exact(4)) {
        let d = (0..4).map(|k| (pa[k] as i32 - pb[k] as i32).abs()).max().unwrap();

        max = max.max(d);
        differing += (d > 0) as usize;
    }

    (max, differing, a.0 * a.1)
}

#[test]
fn svg_matches_x_ite() {
    check("synthetic-rea-svg", "svg-baseline.json", "default quality");
}

/// PrimitiveQuality and TextureQuality change the curve subdivision and the gradient texture size.
/// The page sets them (`SVG_QUALITY=<primitive>,<texture> node oracle/x_ite-svg.mjs ...`) and the
/// Rust parser is given the same options (the runtime does so through `sync_svg_quality`).
#[test]
fn svg_quality_options_match_x_ite() {
    use x3d_parse::SvgQuality::{High, Low};

    for (baseline, primitive, texture) in [("baseline-low-low.json", Low, Low), ("baseline-high-high.json", High, High), ("baseline-low-high.json", Low, High)] {
        x3d_parse::set_svg_quality(primitive, texture);
        check("synthetic-rea-svg-quality", baseline, baseline);
    }

    x3d_parse::set_svg_quality(x3d_parse::SvgQuality::Medium, x3d_parse::SvgQuality::Medium);
}

fn check(dir: &str, baseline_file: &str, what: &str) {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle").join(dir);
    let baseline: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(root.join(baseline_file)).unwrap()).unwrap();
    let only = std::env::var("SVG_ONLY").ok();
    let out = std::env::var("SVG_OUT").ok();
    let mut failures = vec![];
    let mut checks = 0;
    let mut worst = (0, 0usize, 0usize);
    let mut textures = 0;

    for entry in baseline["results"].as_array().unwrap() {
        let file = entry["file"].as_str().unwrap();

        if only.as_deref().is_some_and(|o| !file.contains(o)) {
            continue;
        }

        let data = std::fs::read(root.join(file)).unwrap();
        let url = format!("http://oracle/{dir}/{file}");
        let parsed = x3d_parse::parse_bytes(&data, &url);

        if !entry["ok"].as_bool().unwrap() {
            checks += 1;

            match parsed {
                Ok(_) => failures.push(format!("{file}: X_ITE rejected it ({}), Rust parsed it", entry["error"])),
                Err(e) => {
                    // X_ITE's rejection is "Couldn't load X3D file."; the cause is its one console
                    // error: `Couldn't load URL '<url>'. Error: <message>` with the url appended to a
                    // "No suitable file handler found" message.
                    let log = entry["log"].as_array().unwrap();
                    let actual = match &e {
                        x3d_parse::LoadError::Parse(p) => p.message.clone(),
                        other => format!("{other}"),
                    };
                    let message = actual.trim_end_matches('.');

                    if entry["error"].as_str() != Some("Couldn't load X3D file.") || log.len() != 1 || !log[0].as_str().unwrap().contains(message) {
                        failures.push(format!("{file}: error text\n  X_ITE: {} {log:?}\n  Rust:  {actual}", entry["error"]));
                    }
                }
            }

            continue;
        }

        let scene = match parsed {
            Ok(s) => s,
            Err(e) => {
                failures.push(format!("{file}: Rust rejected it: {e}"));
                continue;
            }
        };

        let encoding = entry["encoding"].as_str().unwrap_or("");

        checks += 1;

        if scene.encoding != encoding {
            failures.push(format!("{file}: encoding X_ITE {encoding}, Rust {}", scene.encoding));
        }

        let mut images: Option<Vec<((usize, usize, Vec<u8>), (usize, usize, Vec<u8>))>> = None;

        for (key, actual) in [("vrml", scene.to_vrml_string()), ("xml", scene.to_xml_string()), ("json", scene.to_json_string())] {
            let Some(expected) = entry[key].as_str() else { continue };

            checks += 1;

            let (expected_text, expected_images) = extract_pngs(expected);
            let (actual_text, actual_images) = extract_pngs(&actual);

            if expected_images.len() != actual_images.len() {
                failures.push(format!("{file} [{key}]: {} textures in X_ITE, {} in Rust", expected_images.len(), actual_images.len()));
            } else if key == "xml" {
                images = Some(expected_images.into_iter().zip(actual_images).collect());
            }

            if expected_text != actual_text {
                failures.push(format!("{file} [{key}]: {}", first_difference(&expected_text, &actual_text)));

                if let Some(dir) = &out {
                    let name = file.replace('/', "__");

                    std::fs::create_dir_all(dir).unwrap();
                    std::fs::write(Path::new(dir).join(format!("{name}.{key}.rust")), &actual_text).unwrap();
                    std::fs::write(Path::new(dir).join(format!("{name}.{key}.xite")), &expected_text).unwrap();
                }
            }
        }

        for (i, (e, a)) in images.unwrap_or_default().iter().enumerate() {
            let (max, differing, count) = compare_pixels(e, a);

            textures += 1;
            checks += 1;

            if worst.2 == 0 || max > worst.0 || (max == worst.0 && differing > worst.1) {
                worst = (max, differing, count);
            }

            if std::env::var("SVG_PIXELS").is_ok() && differing > 0 {
                let mut shown = 0;

                for (k, (pa, pb)) in e.2.chunks_exact(4).zip(a.2.chunks_exact(4)).enumerate() {
                    if pa != pb && shown < 12 {
                        println!("{file} pixel ({}, {}): X_ITE {pa:?} Rust {pb:?}", k % e.0, k / e.0);
                        shown += 1;
                    }
                }
            }

            if max > MAX_CHANNEL_DIFF || differing as f64 > MAX_DIFF_FRACTION * count as f64 {
                failures.push(format!("{file}: texture {i}: max channel difference {max}, {differing} of {count} pixels differ"));
            }
        }
    }

    println!("{what}: {checks} checks, {} failed; {textures} gradient textures, worst: max channel difference {} with {} of {} pixels differing", failures.len(), worst.0, worst.1, worst.2);
    assert!(failures.is_empty(), "{what}: {} of {checks} checks failed:\n{}", failures.len(), failures.join("\n"));
}
