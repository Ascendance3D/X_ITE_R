//! The CFF items that were UNVERIFIED (ledger node:FontLibrary / sub:text.fonts): subroutine bias, subroutine nesting depth, `seac` (an `endchar` with
//! four arguments) in plain and CID-keyed CFF, and variable CFF2 fonts (fvar, avar, named default instance, `blend`). x3d-text against the pinned
//! X_ITE 16.5.0 (opentype.js 2.0.0). `oracle/rea-t-cff-items-baseline/fonts.json` is three identical recordings of `oracle/x_ite-cff-cid-fonts.mjs`
//! (`oracle/tools/rea_t_cff_items.py record`) over the fonts of `oracle/synthetic-rea-t-cff-items` (authored by `oracle/tools/gen_cff_items_fonts.py`;
//! no third-party font data). Per font and glyph: the advance before and after the path is built, the `getPath (0, 0, 1)` commands, the raw
//! commands, the libtess triangles, the exception of a failed path build and the `console` lines. X_ITE calls `glyph .getPath (0, 0, 1)` without the
//! font, so the HVAR advance is never applied, but the charstring still blends with the default instance of the font's `fvar` and rounds every
//! coordinate (`font .variation && coords`).

use serde_json::Value;
use x3d_text::font::Cmd;
use x3d_text::{Font, PrimitiveQuality};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/../../oracle");

fn baseline() -> Value {
    let data = std::fs::read(format!("{DIR}/rea-t-cff-items-baseline/fonts.json")).unwrap();

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
fn cff_items_fonts_match_x_ite() {
    let all = baseline();
    let fonts = all["fonts"].as_object().unwrap();
    let mut parsed = 0;
    let mut path_errors = std::collections::BTreeMap::new();

    for (file, info) in fonts {
        let bytes = std::fs::read(format!("{DIR}/synthetic-rea-t-cff-items/{file}")).unwrap();

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
                *path_errors.entry(file.clone()).or_insert(0) += 1;
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
            // Playwright names a console.warn "warning"; the engine and the runtime recorder say "warn".
            .map(|l| l.as_str().unwrap().replacen("warning: ", "warn: ", 1))
            .collect();
        let mut got = font.take_log();

        want.sort();
        got.sort();
        assert_eq!(got, want, "{file} console");
        parsed += 1;
    }

    // The glyphs whose path build throws in the pinned X_ITE (a TypeError: opentype.js reads `.path` of a glyph it did not find): C, D and E of the
    // CID-keyed fonts (a CID-keyed font has no standard-encoding glyph names: `seac` finds no component) and E of the plain font (it names the
    // accent `Z`, the font has none). C and D of the plain font compose A and B. No other glyph throws.
    assert_eq!(fonts.len(), 12);
    assert_eq!(parsed, 12);
    assert_eq!(
        path_errors,
        std::collections::BTreeMap::from([
            ("seac-cid-fd.otf".to_string(), 3),
            ("seac-cid.otf".to_string(), 3),
            ("seac-plain.otf".to_string(), 1),
        ])
    );
}

/// A malformed table is an error or an empty glyph, never a panic: every byte of every font but the two with thousands of subrs is replaced by
/// 0x00, 0xff and 0x80 in turn, and every glyph of a font that still parses has its path built.
#[test]
fn damaged_fonts_never_panic() {
    let mut parsed = 0;
    let mut rejected = 0;

    for entry in std::fs::read_dir(format!("{DIR}/synthetic-rea-t-cff-items")).unwrap() {
        let path = entry.unwrap().path();
        let name = path.file_name().unwrap().to_string_lossy().to_string();

        if !name.ends_with(".otf") || name.starts_with("bias-") {
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
        parsed > 1000 && rejected > 100,
        "{parsed} parsed, {rejected} rejected"
    );
}

/// The fonts discriminate what they are for (checked on the pinned recording, so a wrong bias, depth limit or default instance cannot pass by
/// drawing nothing): a call that lands on the wrong subroutine leaves a glyph with fewer points.
#[test]
fn recorded_fonts_discriminate() {
    let all = baseline();
    let fonts = &all["fonts"];
    let points = |font: &str, ch: &str| -> usize {
        fonts[font]["glyphs"][ch]["rawCommands"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["type"] != "Z")
            .count()
    };
    let xs = |font: &str, ch: &str| -> Vec<f64> {
        fonts[font]["glyphs"][ch]["rawCommands"]
            .as_array()
            .unwrap()
            .iter()
            .filter(|c| c["type"] != "Z")
            .map(|c| c["x"].as_f64().unwrap())
            .collect()
    };

    // Bias: glyph A (two local subrs) and B (two global subrs) draw 3 lines each whatever the subroutine count; H calls subr number N, which
    // does not exist (the 33899 font calls N - 2 instead and draws two lines more).
    for f in [
        "bias-1239.otf",
        "bias-1240.otf",
        "bias-33899.otf",
        "bias-33900.otf",
    ] {
        assert_eq!(points(f, "A"), 4, "{f} A");
        assert_eq!(points(f, "B"), 4, "{f} B");
        assert_eq!(points(f, "O"), 4, "{f} O");
        assert_eq!(
            points(f, "H"),
            if f == "bias-33899.otf" { 6 } else { 4 },
            "{f} H"
        );
    }

    // Depth: a chain of 9 subrs draws 9 lines, every longer chain stops at 10 (the 11th call is skipped with a warning).
    let lines: Vec<usize> = "ABCDEFGHIOV"
        .chars()
        .map(|c| points("depth.otf", &c.to_string()) - 1)
        .collect();

    assert_eq!(lines, [9, 10, 10, 10, 10, 10, 10, 10, 10, 10, 10]);

    // Variable CFF2: the named instance "Regular" (wght 700) is the default of var-regular, so the blend of glyph A (400 + 100 * 0.6) differs
    // from var-default (the axis default: 400 + 100 * 0 -> 400).
    assert_eq!(xs("var-default.otf", "A"), [20.0, 420.0, 220.0]);
    assert_eq!(xs("var-regular.otf", "A"), [20.0, 480.0, 280.0]);
    // avar maps the normalized 0.6 to 0.84: 20 + 400 + 100 * 0.84 = 504
    assert_eq!(xs("var-avar.otf", "A"), [20.0, 504.0, 304.0]);
    // Rounding: B has -10.5 and 40.5 on its path (Math.round: -10, 41) in every variable font.
    assert_eq!(xs("var-default.otf", "B")[0], -10.0);
    assert_eq!(xs("var-default.otf", "B")[1], 0.0);
}
