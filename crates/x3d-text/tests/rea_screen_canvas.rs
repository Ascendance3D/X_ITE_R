//! node:FontLibrary, sub:text.fonts: the canvas raster of ScreenText against the pinned X_ITE 16.5.0 (34cdf9f97).
//!
//! oracle/rea-cff-screen-raster-baseline/<world>.canvas.json (oracle/tools/rea_cff_screen_raster.py, `x_ite-shot.mjs --canvas`,
//! three identical runs) holds, for every 2D canvas that ScreenText filled in a world of oracle/synthetic-rea-cff-screen-raster,
//! the page's own canvas calls (the transforms and the path of every glyph) and the alpha of every canvas pixel that
//! `getImageData` returned. The test gives the same calls to [`x3d_text::skia_edge::fill_path`] and
//! [`x3d_text::skia_edge::composite`], which ScreenText uses, and compares the alpha.
//!
//! Bound, as for the pictures: at most 0.5% of the canvas' pixels more than 8 off. Here the canvas is only the text box, so
//! the bound is far stricter than on a 512 × 512 picture.
//!
//! KNOWN_GAPS lists the canvases that stay over the bound (none now). A listed canvas must stay over the bound: when it comes
//! within it, the entry fails the test and must be removed. Each entry also has the most pixels over 8 that the canvas may
//! have (the measured number and a tenth more), so a listed canvas cannot get worse unseen.
//!
//! The six CFF canvases that were listed (206, 5, 17, 26, 43 and 22 pixels over 8, 0.71% to 1.24%) had glyphs whose cubic
//! curves bulge into the filled span: there Chrome's edge was about 0.1 pixel further left. The cause is Skia's walk of a
//! cubic edge in the non-convex scan (SkScan_AAAPath aaa_walk_edges): SkAnalyticCubicEdge::keepContinuous starts each next
//! line where the walk of the previous one ended, and the truncated steps add up (`skia_edge::walk`). With the walk they
//! are at 30, 0, 0, 1, 5 and 0.

use serde_json::Value;
use x3d_text::skia_edge::{PathCmd, composite, fill_path};

const DIR: &str = concat!(
    env!("CARGO_MANIFEST_DIR"),
    "/../../oracle/rea-cff-screen-raster-baseline"
);

/// (world, canvas width, canvas height, most pixels over 8): the canvases over the bound.
const KNOWN_GAPS: &[(&str, usize, usize, usize)] = &[];

/// 2D affine transform of the canvas: x' = a x + c y + e, y' = b x + d y + f.
#[derive(Clone, Copy)]
struct Ctm([f64; 6]);

impl Ctm {
    fn apply(&self, x: f64, y: f64) -> [f64; 2] {
        let m = &self.0;

        [m[0] * x + m[2] * y + m[4], m[1] * x + m[3] * y + m[5]]
    }
}

/// The alpha of a canvas after the page's calls.
fn replay(width: usize, height: usize, calls: &[Value]) -> Vec<u8> {
    let mut alpha = vec![0u8; width * height];
    let mut ctm = Ctm([1.0, 0.0, 0.0, 1.0, 0.0, 0.0]);
    let mut stack = vec![];
    let mut path: Vec<PathCmd> = vec![];

    for call in calls {
        let call = call.as_array().unwrap();
        let n = |i: usize| call[i].as_f64().unwrap();
        let m = ctm.0;

        match call[0].as_str().unwrap() {
            "save" => stack.push(ctm),
            "restore" => ctm = stack.pop().unwrap(),
            "translate" => {
                let [e, f] = ctm.apply(n(1), n(2));

                ctm = Ctm([m[0], m[1], m[2], m[3], e, f]);
            }
            "scale" => {
                ctm = Ctm([
                    m[0] * n(1),
                    m[1] * n(1),
                    m[2] * n(2),
                    m[3] * n(2),
                    m[4],
                    m[5],
                ]);
            }
            "beginPath" => path.clear(),
            "moveTo" => path.push(PathCmd::MoveTo(ctm.apply(n(1), n(2)))),
            "lineTo" => path.push(PathCmd::LineTo(ctm.apply(n(1), n(2)))),
            "quadraticCurveTo" => path.push(PathCmd::QuadTo(
                ctm.apply(n(1), n(2)),
                ctm.apply(n(3), n(4)),
            )),
            "bezierCurveTo" => path.push(PathCmd::CubicTo(
                ctm.apply(n(1), n(2)),
                ctm.apply(n(3), n(4)),
                ctm.apply(n(5), n(6)),
            )),
            "closePath" => path.push(PathCmd::ClosePath),
            "fill" => composite(&mut alpha, &fill_path(width, height, &path)),
            other => panic!("canvas call {other} is not handled"),
        }
    }

    alpha
}

#[test]
fn screen_text_canvases_match_x_ite() {
    let mut files: Vec<String> = std::fs::read_dir(DIR)
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().into_string().ok())
        .filter(|n| n.ends_with(".canvas.json"))
        .collect();

    files.sort();
    assert_eq!(files.len(), 6, "{files:?}");

    let (mut failures, mut over_bound, mut compared, mut curves) = (vec![], vec![], 0, [0usize; 2]);

    for file in &files {
        let world = file.trim_end_matches(".canvas.json");
        let text = std::fs::read_to_string(format!("{DIR}/{file}")).unwrap();
        let record: Value = serde_json::from_str(&text).unwrap();
        let canvases = record["canvases"].as_array().unwrap();

        assert!(!canvases.is_empty(), "{world}: no canvas");

        for canvas in canvases {
            let (width, height) = (
                canvas["width"].as_u64().unwrap() as usize,
                canvas["height"].as_u64().unwrap() as usize,
            );
            let calls = canvas["calls"].as_array().unwrap();
            let want: Vec<u8> = canvas["alpha"]
                .as_array()
                .unwrap()
                .iter()
                .map(|a| a.as_u64().unwrap() as u8)
                .collect();

            assert_eq!(want.len(), width * height, "{world}: alpha size");

            for (i, name) in ["quadraticCurveTo", "bezierCurveTo"].iter().enumerate() {
                curves[i] += calls.iter().filter(|c| c[0] == *name).count();
            }

            let got = replay(width, height, calls);
            let diff = |(a, b): (&u8, &u8)| (*a as i32 - *b as i32).abs();
            let over = got.iter().zip(&want).filter(|p| diff(*p) > 8).count();
            let max = got.iter().zip(&want).map(diff).max().unwrap();

            eprintln!(
                "{world} {width}x{height}: {over} pixels over 8 ({:.3}%), max {max}",
                over as f64 * 100.0 / want.len() as f64
            );
            compared += 1;

            let gap = KNOWN_GAPS
                .iter()
                .find(|g| (g.0, g.1, g.2) == (world, width, height));

            if over * 1000 > want.len() * 5 {
                over_bound.push((world.to_string(), width, height));

                match gap {
                    None => failures.push(format!(
                        "{world} {width}x{height}: {over} of {} pixels over 8, max {max}",
                        want.len()
                    )),
                    Some(g) if over > g.3 => failures.push(format!(
                        "{world} {width}x{height}: {over} pixels over 8, more than the {} of the known gap",
                        g.3
                    )),
                    Some(_) => {}
                }
            } else if gap.is_some() {
                failures.push(format!(
                    "{world} {width}x{height}: within the bound now; remove it from KNOWN_GAPS"
                ));
            }
        }
    }

    for (world, width, height, _) in KNOWN_GAPS {
        if !over_bound.contains(&(world.to_string(), *width, *height)) {
            failures.push(format!(
                "{world} {width}x{height}: listed in KNOWN_GAPS, but no such canvas is over the bound"
            ));
        }
    }

    // Both curve types are in the recordings: TrueType quadratics and CFF cubics.
    assert!(curves[0] > 100 && curves[1] > 100, "curves {curves:?}");
    assert!(compared >= 6, "only {compared} canvases");
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
