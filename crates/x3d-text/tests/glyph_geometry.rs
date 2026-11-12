//! Every libtess glyph triangle vertex of the 12 X_ITE default fonts (19 characters each, 228 glyphs)
//! against pinned X_ITE 16.5.0 (PolygonText.getGlyphGeometry, PrimitiveQuality MEDIUM).
//!
//! The reference is the versioned fixture tests/data/glyph-geometry (manifest.json, vertices.f64le.gz),
//! written by oracle/tools/glyph_fixture.py from `oracle/x_ite-text.mjs --all-glyphs`. It is not the
//! protected answer key oracle/x_ite-text.json (tests/oracle.rs), so this test always runs.

use serde_json::Value;
use sha2::{Digest, Sha256};
use std::io::Read;
use x3d_text::fonts::default_font_file;
use x3d_text::{Font, PrimitiveQuality};

const DIR: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/data/glyph-geometry");

fn hex(b: &[u8]) -> String {
    Sha256::digest(b)
        .iter()
        .map(|x| format!("{x:02x}"))
        .collect()
}

/// The manifest and the decoded coordinates, after the fixture's own hashes are checked.
fn fixture() -> (Value, Vec<f64>) {
    let manifest: Value =
        serde_json::from_slice(&std::fs::read(format!("{DIR}/manifest.json")).unwrap()).unwrap();
    let gz = std::fs::read(format!("{DIR}/vertices.f64le.gz")).unwrap();

    assert_eq!(hex(&gz), manifest["vertices_gz_sha256"].as_str().unwrap());

    let mut raw = vec![];
    flate2::read::GzDecoder::new(&gz[..])
        .read_to_end(&mut raw)
        .unwrap();

    assert_eq!(hex(&raw), manifest["vertices_raw_sha256"].as_str().unwrap());

    let coords: Vec<f64> = raw
        .chunks_exact(8)
        .map(|c| f64::from_le_bytes(c.try_into().unwrap()))
        .collect();

    assert_eq!(
        coords.len() as u64,
        manifest["coordinates"].as_u64().unwrap()
    );

    (manifest, coords)
}

/// Compares x3d-text with the expected coordinates. Returns one line per glyph that differs
/// (index, advance, triangle count or any coordinate bit) and the number of glyphs compared.
fn mismatches(manifest: &Value, coords: &[f64]) -> (Vec<String>, usize) {
    let (mut bad, mut compared) = (vec![], 0);

    for f in manifest["fonts"].as_array().unwrap() {
        let name = f["name"].as_str().unwrap();
        let (family, style) = name.split_once('-').unwrap();
        let (file, data) = default_font_file(family, style).unwrap();

        assert_eq!(file, f["file"].as_str().unwrap(), "{name} font file");
        assert_eq!(
            hex(data),
            f["file_sha256"].as_str().unwrap(),
            "{name} font bytes"
        );

        let font = Font::parse(data).unwrap();

        for g in f["glyphs"].as_array().unwrap() {
            let ch = g["char"].as_str().unwrap();
            let gid = font.char_to_glyph(ch.chars().next().unwrap() as u16);
            let tri = font.glyph_geometry(gid, PrimitiveQuality::Medium);
            let (offset, count) = (
                g["offset"].as_u64().unwrap() as usize,
                g["count"].as_u64().unwrap() as usize,
            );
            let want = &coords[offset..offset + count];
            compared += 1;

            if gid as u64 != g["index"].as_u64().unwrap()
                || font.advance_width(gid) != g["advance"].as_f64().unwrap()
            {
                bad.push(format!("{name} {ch:?}: index or advance"));
                continue;
            }

            if tri.len() * 2 != count {
                bad.push(format!(
                    "{name} {ch:?}: {} points, want {}",
                    tri.len(),
                    count / 2
                ));
                continue;
            }

            let first = tri
                .iter()
                .flatten()
                .zip(want)
                .position(|(a, b)| a.to_bits() != b.to_bits());

            if let Some(i) = first {
                let got = tri[i / 2][i % 2];
                bad.push(format!(
                    "{name} {ch:?}: coordinate {i} is {got:e}, want {:e}",
                    want[i]
                ));
            }
        }
    }

    (bad, compared)
}

#[test]
fn every_default_font_glyph_matches_pinned_x_ite_bit_for_bit() {
    let (manifest, coords) = fixture();
    let (bad, compared) = mismatches(&manifest, &coords);

    assert_eq!(compared, 228);
    assert!(
        bad.is_empty(),
        "{} glyphs differ:\n{}",
        bad.len(),
        bad.join("\n")
    );
}

/// Negative control: one changed expected coordinate (the last bit of one point) is found, and only there.
#[test]
fn one_changed_expected_point_fails_the_comparison() {
    let (manifest, mut coords) = fixture();
    let g = &manifest["fonts"][4]["glyphs"][7]; // SANS-PLAIN '&'
    let i = g["offset"].as_u64().unwrap() as usize + 5;

    coords[i] = f64::from_bits(coords[i].to_bits() ^ 1);

    let (bad, _) = mismatches(&manifest, &coords);

    assert_eq!(bad.len(), 1, "{bad:?}");
    assert!(
        bad[0].starts_with("SANS-PLAIN \"&\": coordinate 5 "),
        "{}",
        bad[0]
    );
}
