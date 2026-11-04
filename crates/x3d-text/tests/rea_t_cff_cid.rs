//! CID-keyed CFF and CFF2 fonts (ledger node:FontLibrary / sub:text.fonts): x3d-text against the pinned X_ITE 16.5.0 (opentype.js 2.0.0).
//! The baseline `oracle/rea-t-cff-cid-baseline/fonts.json` is three identical recordings of `oracle/x_ite-cff-cid-fonts.mjs` over the fonts of
//! `oracle/synthetic-rea-t-cff-cid` (authored by `oracle/tools/gen_cff_cid_fonts.py`; no third-party font data). Per font: the parse (or the text
//! of the exception), the metrics, and per glyph the advance before and after the path is built, the `getPath (0, 0, 1)` commands, the raw
//! commands, the libtess triangles, the exception of a failed path build, and the `console` lines of the load and the builds. Every font is
//! parsed afresh here too: opentype.js lets the first path build of a glyph replace its advance by the charstring width.

use serde_json::Value;
use x3d_text::font::Cmd;
use x3d_text::{Font, FontError, PrimitiveQuality};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../oracle");

fn baseline() -> Value {
    let data = std::fs::read(format!("{DIR}/rea-t-cff-cid-baseline/fonts.json")).unwrap();

    serde_json::from_slice(&data).unwrap()
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

#[test]
fn cid_and_cff2_fonts_match_x_ite() {
    let all = baseline();
    let fonts = all["fonts"].as_object().unwrap();
    let mut parsed = 0;
    let mut rejected = 0;
    let mut path_errors = 0;

    for (file, info) in fonts {
        let bytes = std::fs::read(format!("{DIR}/synthetic-rea-t-cff-cid/{file}")).unwrap();

        if let Some(want) = info["parseError"].as_str() {
            match Font::parse(&bytes) {
                Err(FontError::OpenType(got)) => assert_eq!(got, want, "{file} parse error"),
                Err(other) => panic!("{file}: {other:?}, expected {want}"),
                Ok(_) => panic!("{file}: parsed, expected {want}"),
            }

            rejected += 1;
            continue;
        }

        let font = Font::parse(&bytes).unwrap_or_else(|e| panic!("{file}: {e:?}"));

        assert_eq!(
            font.units_per_em,
            info["unitsPerEm"].as_f64().unwrap(),
            "{file} unitsPerEm"
        );
        assert_eq!(
            font.ascender,
            info["ascender"].as_f64().unwrap(),
            "{file} ascender"
        );
        assert_eq!(
            font.descender,
            info["descender"].as_f64().unwrap(),
            "{file} descender"
        );
        assert_eq!(
            font.num_glyphs as f64,
            info["numGlyphs"].as_f64().unwrap(),
            "{file} numGlyphs"
        );
        assert_eq!(
            font.is_cid_keyed(),
            info["isCIDFont"].as_bool().unwrap(),
            "{file} isCIDFont"
        );

        let scale = 1.0 / font.units_per_em * 1.0;

        for (ch, g) in info["glyphs"].as_object().unwrap() {
            let c = ch.chars().next().unwrap();
            let gid = font.char_to_glyph(c as u16);
            let what = format!("{file} {ch:?}");

            assert_eq!(gid as f64, g["index"].as_f64().unwrap(), "{what} index");
            // The advance before the glyph's path is built: hmtx.
            assert_eq!(
                font.advance_width(gid),
                g["advanceBefore"].as_f64().unwrap(),
                "{what} advance before"
            );

            if let Some(want) = g["error"].as_str() {
                // The path build threw: no commands, the exception text, the advance as it was.
                assert_eq!(
                    font.glyph_error(gid).as_deref(),
                    Some(want),
                    "{what} exception"
                );
                assert!(
                    font.glyph_commands(gid).is_empty(),
                    "{what} commands after an exception"
                );
                assert_eq!(
                    font.advance_width(gid),
                    g["advanceAfter"].as_f64().unwrap(),
                    "{what} advance after"
                );
                path_errors += 1;
                continue;
            }

            assert_eq!(font.glyph_error(gid), None, "{what} must not throw");

            // PolygonText's libtess triangles (Medium quality), vertex by vertex.
            let tri = font.glyph_geometry(gid, PrimitiveQuality::Medium);
            let v = nums(&g["vertices"]);

            assert_eq!(tri.len() * 2, v.len(), "{what} vertex count");

            for (i, p) in tri.iter().enumerate() {
                assert_eq!([p[0], p[1]], [v[i * 2], v[i * 2 + 1]], "{what} vertex {i}");
            }

            // Glyph.getPath (0, 0, 1) commands: M, L, C; JavaScript flips y and PolygonText flips it back.
            let want = g["commands"].as_array().unwrap();
            let got = font.glyph_commands(gid);

            assert_eq!(got.len(), want.len(), "{what} command count");

            for (i, (have, w)) in got.iter().zip(want).enumerate() {
                let f = |k: &str| w[k].as_f64().unwrap();
                let (x, y) = (f("x"), -f("y"));

                match (*have, w["type"].as_str().unwrap()) {
                    (Cmd::M(hx, hy), "M") | (Cmd::L(hx, hy), "L") => {
                        assert_eq!((hx, hy), (x, y), "{what} command {i}");
                    }
                    (Cmd::C(a, b, c, d, hx, hy), "C") => {
                        assert_eq!(
                            (a, b, c, d, hx, hy),
                            (f("x1"), -f("y1"), f("x2"), -f("y2"), x, y),
                            "{what} command {i}"
                        );
                    }
                    (have, kind) => panic!("{what} command {i}: {have:?} vs {kind}"),
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
                    assert_eq!(hx, w["x"].as_f64().unwrap() * scale, "{what} raw x");
                }
            }

            // The charstring width replaces the hmtx advance once the path exists, when the charstring has a width argument or a stem hint.
            assert_eq!(
                font.advance_width(gid),
                g["advanceAfter"].as_f64().unwrap(),
                "{what} advance after"
            );
        }

        // The console lines of the load and of the path builds, as a multiset.
        let mut want: Vec<String> = info["console"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap().to_string())
            .collect();
        let mut got = font.take_log();

        want.sort();
        got.sort();
        assert_eq!(got, want, "{file} console");
        parsed += 1;
    }

    assert_eq!(fonts.len(), 11);
    assert_eq!((parsed, rejected), (7, 4));
    assert_eq!(
        path_errors, 9,
        "the CFF2 font without a VariationStore: every glyph throws"
    );
}

/// A malformed CFF table is an error or an empty glyph, never a panic: every byte of every font is replaced by 0x00, 0xff and 0x80 in turn, and
/// every glyph of a font that still parses has its path built.
#[test]
fn damaged_fonts_never_panic() {
    let mut parsed = 0;
    let mut rejected = 0;

    for entry in std::fs::read_dir(format!("{DIR}/synthetic-rea-t-cff-cid")).unwrap() {
        let path = entry.unwrap().path();

        if path.extension().is_none_or(|e| e != "otf") {
            continue;
        }

        let bytes = std::fs::read(&path).unwrap();

        for i in 0..bytes.len() {
            for v in [0x00, 0xff, 0x80] {
                let mut damaged = bytes.clone();

                damaged[i] = v;

                match Font::parse(&damaged) {
                    Ok(font) => {
                        for g in 0..font.num_glyphs {
                            let _ = font.glyph_commands(g);
                        }

                        parsed += 1;
                    }
                    Err(_) => rejected += 1,
                }
            }
        }
    }

    assert!(
        parsed > 1000 && rejected > 1000,
        "{parsed} parsed, {rejected} rejected"
    );
}
