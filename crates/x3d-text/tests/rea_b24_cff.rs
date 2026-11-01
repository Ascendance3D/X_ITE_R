//! OpenType-CFF fonts (REA issue 74, ledger node:FontLibrary / sub:text.fonts): x3d-text against the pinned
//! X_ITE 16.5.0 (opentype.js 2.0.0). The baseline `oracle/synthetic-rea-b24-cff-baseline.json` is written by
//! `oracle/x_ite-cff-fonts.mjs` from the fonts of `oracle/synthetic-rea-b24-cff` (authored by
//! `oracle/tools/gen_cff_fonts.py`; no third-party font data). Every layout there loaded a fresh font, so
//! every test here parses a fresh [`Font`] too: opentype.js lets the first path build of a glyph replace its
//! advance by the charstring width.

use serde_json::Value;
use x3d_text::font::Cmd;
use x3d_text::{Font, FontStyleParams, PrimitiveQuality, TextCompression, TextParams, layout};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../oracle");

fn baseline() -> Value {
    let data = std::fs::read(format!("{DIR}/synthetic-rea-b24-cff-baseline.json")).unwrap();

    serde_json::from_slice(&data).unwrap()
}

fn fresh_font() -> Font {
    Font::parse(&std::fs::read(format!("{DIR}/synthetic-rea-b24-cff/cff-basic.otf")).unwrap())
        .unwrap()
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

fn font_style(src: &str) -> FontStyleParams {
    let mut fs = FontStyleParams::default();
    let toks: Vec<&str> = src.split_whitespace().collect();
    let mut i = 0;

    while i < toks.len() {
        match toks[i] {
            "size" => fs.size = toks[i + 1].parse().unwrap(),
            "spacing" => fs.spacing = toks[i + 1].parse().unwrap(),
            "horizontal" => fs.horizontal = toks[i + 1] == "TRUE",
            "leftToRight" => fs.left_to_right = toks[i + 1] == "TRUE",
            "topToBottom" => fs.top_to_bottom = toks[i + 1] == "TRUE",
            "justify" => {
                // justify [ "MIDDLE" "MIDDLE" ]
                fs.justify = vec![];
                i += 2;

                while toks[i] != "]" {
                    fs.justify.push(toks[i].trim_matches('"').to_string());
                    i += 1;
                }

                i += 1;
                continue;
            }
            other => panic!("unknown FontStyle field {other}"),
        }

        i += 2;
    }

    fs
}

fn text_fields(src: &str) -> (Vec<f64>, f64) {
    let (mut length, mut max_extent) = (vec![], 0.0);
    let toks: Vec<&str> = src.split_whitespace().collect();
    let mut i = 0;

    while i < toks.len() {
        match toks[i] {
            "length" => {
                i += 2;

                while toks[i] != "]" {
                    length.push(toks[i].parse().unwrap());
                    i += 1;
                }

                i += 1;
            }
            "maxExtent" => {
                max_extent = toks[i + 1].parse().unwrap();
                i += 2;
            }
            other => panic!("unknown Text field {other}"),
        }
    }

    (length, max_extent)
}

#[test]
fn cff_font_metrics_kerning_and_glyph_outlines() {
    let o = baseline();
    let info = &o["fonts"]["cff-basic"];
    let font = fresh_font();

    assert_eq!(font.units_per_em, info["unitsPerEm"].as_f64().unwrap());
    assert_eq!(font.ascender, info["ascender"].as_f64().unwrap());
    assert_eq!(font.descender, info["descender"].as_f64().unwrap());
    assert_eq!(font.num_glyphs as f64, info["numGlyphs"].as_f64().unwrap());

    // Kerning (GPOS) over the characters of the recording.
    let chars: Vec<char> = "ABOSiHFQ\u{c9}fWV Z".chars().collect();
    let expected = info["kerning"].as_object().unwrap();
    let mut seen = 0;

    for a in &chars {
        for b in &chars {
            let k = font.kerning(font.char_to_glyph(*a as u16), font.char_to_glyph(*b as u16));
            let key = format!("{a}{b}");

            match expected.get(&key) {
                Some(v) => {
                    assert_eq!(k, v.as_f64().unwrap(), "kerning {key:?}");
                    seen += 1;
                }
                None => assert_eq!(k, 0.0, "kerning {key:?} should be 0"),
            }
        }
    }

    assert_eq!(seen, expected.len());
    assert!(seen >= 3);

    let scale = 1.0 / font.units_per_em * 1.0;
    let mut checked = 0;

    for (ch, g) in info["glyphs"].as_object().unwrap() {
        let c = ch.chars().next().unwrap();
        let gid = font.char_to_glyph(c as u16);

        assert_eq!(gid as f64, g["index"].as_f64().unwrap(), "{ch:?} index");
        // The advance before the glyph's path is built: hmtx.
        assert_eq!(
            font.advance_width(gid),
            g["advanceBefore"].as_f64().unwrap(),
            "{ch:?} advance before"
        );

        // Glyph.getPath (0, 0, 1) commands: M, L, C; JavaScript flips y and PolygonText flips it back.
        let want = g["commands"].as_array().unwrap();
        let got = font.glyph_commands(gid);

        assert_eq!(got.len(), want.len(), "{ch:?} command count");

        for (i, (have, w)) in got.iter().zip(want).enumerate() {
            let f = |k: &str| w[k].as_f64().unwrap();
            let (x, y) = (f("x"), -f("y"));

            match (*have, w["type"].as_str().unwrap()) {
                (Cmd::M(hx, hy), "M") | (Cmd::L(hx, hy), "L") => {
                    assert_eq!((hx, hy), (x, y), "{ch:?} command {i}");
                }
                (Cmd::C(a, b, c, d, hx, hy), "C") => {
                    assert_eq!(
                        (a, b, c, d, hx, hy),
                        (f("x1"), -f("y1"), f("x2"), -f("y2"), x, y),
                        "{ch:?} command {i}"
                    );
                }
                (have, kind) => panic!("{ch:?} command {i}: {have:?} vs {kind}"),
            }
        }

        // The raw charstring outline in font units, scaled the way getPath does.
        for (have, w) in got.iter().zip(
            g["rawCommands"]
                .as_array()
                .unwrap()
                .iter()
                .filter(|w| w["type"] != "Z"),
        ) {
            if let Cmd::M(hx, _) | Cmd::L(hx, _) | Cmd::C(_, _, _, _, hx, _) = *have {
                assert_eq!(hx, w["x"].as_f64().unwrap() * scale, "{ch:?} raw x");
            }
        }

        // PolygonText's libtess triangles (Medium quality), vertex by vertex.
        let tri = font.glyph_geometry(gid, PrimitiveQuality::Medium);
        let v = nums(&g["vertices"]);

        assert_eq!(tri.len() * 2, v.len(), "{ch:?} vertex count");

        for (i, p) in tri.iter().enumerate() {
            // Exact: serde_json's `float_roundtrip` parses the recorded doubles to the last bit.
            assert_eq!([p[0], p[1]], [v[i * 2], v[i * 2 + 1]], "{ch:?} vertex {i}");
        }

        // The charstring width replaces the hmtx advance once the path exists (opentype.js quirk).
        assert_eq!(
            font.advance_width(gid),
            g["advanceAfter"].as_f64().unwrap(),
            "{ch:?} advance after"
        );
        checked += 1;
    }

    assert_eq!(checked, 14);
}

#[test]
fn cff_text_layouts_match_x_ite() {
    let o = baseline();
    let mut checked = 0;

    for (name, l) in o["layouts"].as_object().unwrap() {
        let strings: Vec<String> = l["strings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        let fs = font_style(l["fontStyle"].as_str().unwrap());
        let (length, max_extent) = text_fields(l["text"].as_str().unwrap());
        let font = std::rc::Rc::new(fresh_font());
        let text = TextParams::from_strings(&strings, &length, max_extent);
        let out = layout(
            Some(&font),
            &text,
            &fs,
            PrimitiveQuality::Medium,
            TextCompression::CharSpacing,
        );

        assert_eq!(l["loadState"].as_i64().unwrap(), 1008, "{name} load state");

        let same = |a: &[f64], b: &[f64], what: &str| {
            assert_eq!(a.len(), b.len(), "{name} {what} length");

            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                // X_ITE keeps these arrays as Float32Array.
                assert!(
                    (x - y).abs() <= 1e-6 * (1.0 + y.abs()),
                    "{name} {what}[{i}]: {x} vs {y}"
                );
            }
        };

        same(&out.vertices, &nums(&l["vertices"]), "vertices");
        same(&out.tex_coords, &nums(&l["texCoords"]), "texCoords");
        same(&out.normals, &nums(&l["normals"]), "normals");
        same(&out.min, &nums(&l["min"]), "min");
        same(&out.max, &nums(&l["max"]), "max");
        same(&out.origin, &nums(&l["origin"]), "origin");
        same(&out.text_bounds, &nums(&l["textBounds"]), "textBounds");

        let lb: Vec<f64> = l["lineBounds"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(nums)
            .collect();
        let mine: Vec<f64> = out.line_bounds.iter().flat_map(|v| v.to_vec()).collect();

        same(&mine, &lb, "lineBounds");
        checked += 1;
    }

    assert_eq!(checked, 12);
}

#[test]
fn bare_cff_and_non_font_data_do_not_parse() {
    // opentype.js: "Unsupported OpenType signature" for a bare CFF file (no SFNT wrapper), like the baseline's failing case.
    let o = baseline();
    let bare = std::fs::read(format!("{DIR}/synthetic-rea-b24-cff/cff-bare.cff")).unwrap();

    assert_eq!(o["failing"]["cff-bare.cff"]["libraryLoadState"], 1009);
    assert!(Font::parse(&bare).is_err());
    assert!(Font::parse(b"not a font at all").is_err());
}
