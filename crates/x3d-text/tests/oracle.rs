//! Compares x3d-text with the pinned X_ITE (oracle/x_ite-text.json, written by
//! oracle/x_ite-text.mjs): font metrics, kerning, libtess glyph triangles and Text layouts.

use serde_json::Value;
use x3d_text::fonts::{default_font_file, select_default_font};
use x3d_text::{Font, FontStyleParams, PrimitiveQuality, TextCompression, TextParams, layout};

/// The verified answer key (oracle/answer-keys.tsv), or None after a permitted named skip
/// (x3d_evidence::require_key). An absent key fails in strict mode; a differing key always fails.
#[track_caller]
fn oracle() -> Option<Value> {
    let path = x3d_evidence::require_key("x_ite-text.json")?;
    let data = std::fs::read(path).unwrap();

    Some(serde_json::from_slice(&data).unwrap())
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

/// Parses the FontStyle field strings used by oracle/x_ite-text.mjs.
fn parse_font_style(src: &str) -> (Vec<String>, String, FontStyleParams) {
    let mut family = vec!["SERIF".to_string()];
    let mut style = "PLAIN".to_string();
    let mut fs = FontStyleParams::default();
    let toks: Vec<String> = {
        let mut t = vec![];
        let mut cur = String::new();
        let mut in_str = false;

        for c in src.chars() {
            if in_str {
                if c == '"' {
                    t.push(format!("\"{cur}\""));
                    cur.clear();
                    in_str = false;
                } else {
                    cur.push(c);
                }
            } else if c == '"' {
                in_str = true;
            } else if c.is_whitespace() {
                if !cur.is_empty() {
                    t.push(std::mem::take(&mut cur));
                }
            } else if c == '[' || c == ']' {
                if !cur.is_empty() {
                    t.push(std::mem::take(&mut cur));
                }

                t.push(c.to_string());
            } else {
                cur.push(c);
            }
        }

        if !cur.is_empty() {
            t.push(cur);
        }

        t
    };
    let mut i = 0;

    let read_strings = |i: &mut usize| -> Vec<String> {
        let mut out = vec![];

        if toks[*i] == "[" {
            *i += 1;

            while toks[*i] != "]" {
                out.push(toks[*i].trim_matches('"').to_string());
                *i += 1;
            }

            *i += 1;
        } else {
            out.push(toks[*i].trim_matches('"').to_string());
            *i += 1;
        }

        out
    };

    while i < toks.len() {
        let key = toks[i].clone();

        i += 1;

        match key.as_str() {
            "family" => family = read_strings(&mut i),
            "style" => style = read_strings(&mut i).remove(0),
            "justify" => fs.justify = read_strings(&mut i),
            "size" => {
                fs.size = toks[i].parse().unwrap();
                i += 1;
            }
            "spacing" => {
                fs.spacing = toks[i].parse().unwrap();
                i += 1;
            }
            "horizontal" => {
                fs.horizontal = toks[i] == "TRUE";
                i += 1;
            }
            "leftToRight" => {
                fs.left_to_right = toks[i] == "TRUE";
                i += 1;
            }
            "topToBottom" => {
                fs.top_to_bottom = toks[i] == "TRUE";
                i += 1;
            }
            other => panic!("unknown FontStyle field {other}"),
        }
    }

    (family, style, fs)
}

fn parse_text_fields(src: &str) -> (Vec<f64>, f64) {
    let mut length = vec![];
    let mut max_extent = 0.0;
    let toks: Vec<&str> = src.split_whitespace().collect();
    let mut i = 0;

    while i < toks.len() {
        match toks[i] {
            "length" => {
                i += 2; // "[" 
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

fn load_font(name: &str) -> Font {
    let (family, style) = name.split_once('-').unwrap();
    let (_, data) = default_font_file(family, style).unwrap();

    Font::parse(data).unwrap()
}

#[test]
fn font_metrics_kerning_and_glyphs() {
    let Some(o) = oracle() else {
        return;
    };
    let fonts = o["fonts"].as_object().unwrap();
    let mut glyph_checks = 0;
    let mut vertex_checks = 0;

    for (name, info) in fonts {
        let font = load_font(name);

        assert_eq!(
            font.units_per_em,
            info["unitsPerEm"].as_f64().unwrap(),
            "{name} unitsPerEm"
        );
        assert_eq!(
            font.ascender,
            info["ascender"].as_f64().unwrap(),
            "{name} ascender"
        );
        assert_eq!(
            font.descender,
            info["descender"].as_f64().unwrap(),
            "{name} descender"
        );
        assert_eq!(
            font.num_glyphs as f64,
            info["numGlyphs"].as_f64().unwrap(),
            "{name} numGlyphs"
        );

        // Kerning over the same character set.
        let mut chars: Vec<char> = "AVTWYoaeLrP.,fgijl 0123yvkwtT".chars().collect();
        chars.sort();
        chars.dedup();
        let expected = info["kerning"].as_object().unwrap();
        let mut seen = 0;

        for a in &chars {
            for b in &chars {
                let k = font.kerning(font.char_to_glyph(*a as u16), font.char_to_glyph(*b as u16));
                let key = format!("{a}{b}");

                match expected.get(&key) {
                    Some(v) => {
                        assert_eq!(k, v.as_f64().unwrap(), "{name} kerning {key:?}");
                        seen += 1;
                    }
                    None => assert_eq!(k, 0.0, "{name} kerning {key:?} should be 0"),
                }
            }
        }

        assert_eq!(seen, expected.len(), "{name}");

        for (ch, g) in info["glyphs"].as_object().unwrap() {
            let c = ch.chars().next().unwrap();
            let gid = font.char_to_glyph(c as u16);

            assert_eq!(
                gid as f64,
                g["index"].as_f64().unwrap(),
                "{name} {ch:?} index"
            );
            assert_eq!(
                font.advance_width(gid),
                g["advance"].as_f64().unwrap(),
                "{name} {ch:?} advance"
            );

            let tri = font.glyph_geometry(gid, PrimitiveQuality::Medium);

            assert_eq!(
                tri.len() as f64 / 3.0,
                g["triangles"].as_f64().unwrap(),
                "{name} {ch:?} triangle count"
            );

            let area: f64 = tri
                .chunks(3)
                .map(|c| {
                    ((c[1][0] - c[0][0]) * (c[2][1] - c[0][1])
                        - (c[2][0] - c[0][0]) * (c[1][1] - c[0][1]))
                        / 2.0
                })
                .sum();

            assert!(
                (area - g["area"].as_f64().unwrap()).abs() < 1e-9,
                "{name} {ch:?} area {area} vs {}",
                g["area"]
            );
            glyph_checks += 1;

            if let Some(v) = g["vertices"].as_array() {
                let v: Vec<f64> = v.iter().map(|x| x.as_f64().unwrap()).collect();

                assert_eq!(v.len(), tri.len() * 2, "{name} {ch:?} vertex count");

                for (i, p) in tri.iter().enumerate() {
                    assert!(
                        (p[0] - v[i * 2]).abs() < 1e-9 && (p[1] - v[i * 2 + 1]).abs() < 1e-9,
                        "{name} {ch:?} vertex {i}: {p:?} vs ({}, {})",
                        v[i * 2],
                        v[i * 2 + 1]
                    );
                }

                vertex_checks += 1;
            }
        }
    }

    eprintln!("{glyph_checks} glyphs checked (counts, area), {vertex_checks} with every vertex");
}

#[test]
fn text_layouts() {
    let Some(o) = oracle() else {
        return;
    };
    let mut checked = 0;

    for (name, l) in o["layouts"].as_object().unwrap() {
        let strings: Vec<String> = l["strings"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect();
        let (family, style, fs) = parse_font_style(l["fontStyle"].as_str().unwrap());
        let (length, max_extent) = parse_text_fields(l["text"].as_str().unwrap());
        let font = select_default_font(&family, &style);
        let text = TextParams::from_strings(&strings, &length, max_extent);
        let out = layout(
            font.as_deref(),
            &text,
            &fs,
            PrimitiveQuality::Medium,
            TextCompression::CharSpacing,
        );
        let close = |a: &[f64], b: &[f64], what: &str| {
            assert_eq!(a.len(), b.len(), "{name} {what} length");

            for (i, (x, y)) in a.iter().zip(b).enumerate() {
                assert!(
                    (x - y).abs() <= 1e-6 * (1.0 + y.abs()),
                    "{name} {what}[{i}]: {x} vs {y}"
                );
            }
        };

        close(&out.vertices, &nums(&l["vertices"]), "vertices");
        close(&out.tex_coords, &nums(&l["texCoords"]), "texCoords");
        close(&out.normals, &nums(&l["normals"]), "normals");

        if !out.vertices.is_empty()
            || l["vertices"].as_array().unwrap().is_empty() && !strings.is_empty()
        {
            close(&out.min, &nums(&l["min"]), "min");
            close(&out.max, &nums(&l["max"]), "max");
        }

        close(&out.origin, &nums(&l["origin"]), "origin");
        close(&out.text_bounds, &nums(&l["textBounds"]), "textBounds");

        let lb: Vec<f64> = l["lineBounds"]
            .as_array()
            .unwrap()
            .iter()
            .flat_map(|v| nums(v))
            .collect();
        let mine: Vec<f64> = out.line_bounds.iter().flat_map(|v| v.to_vec()).collect();

        close(&mine, &lb, "lineBounds");
        checked += 1;
    }

    eprintln!("{checked} layouts checked");
}
