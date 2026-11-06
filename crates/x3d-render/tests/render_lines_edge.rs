//! Line and point edges outside XR (issue #83, part of #51): points, a wide polyline and a thin rectangle seen by one
//! plain perspective Viewpoint, against the pinned X_ITE 16.5.0 (oracle/synthetic-lines-edge, made by
//! `oracle/tools/gen_lines_edge.py`; keys oracle/render-lines-edge-baseline at MSAA 0 and
//! oracle/render-msaa4-lines-edge-baseline at Multisampling 4, recorded three times with identical pixels by
//! x_ite-shot.mjs / x_ite-shot-msaa.mjs, 256 x 256).
//!
//! PR #402 measured a few single-pixel differences on the lines of the XR case `stereo-points-lines`. This file asks
//! whether they exist outside XR. They do: the same shapes under a plain Viewpoint differ at MSAA 0 too, and at MSAA 4
//! for three of the cameras (a one-sample step). So the difference is not XR-specific.
//!
//! Two tests:
//! * `lines_edge_renders_stay_within_the_bound` is the parity check of `x3d-shot compare`: at most 0.5% of the pixels
//!   more than 8 off. It passes.
//! * `lines_edge_known_pixel_differences_are_recorded` is NOT a parity check. It records the pixels that differ today
//!   (KNOWN) and fails when a case gets worse than the record, or when a case that is recorded as different becomes
//!   exact (then the record must be edited down). The causes, measured with controlled changes in the renderer:
//!   - thin lines (width 1, no LineProperties) are native Vulkan LINE_LIST lines: the NVIDIA rasterizer gives one extra
//!     pixel at the steps of a line where GL does not. The same lines drawn as the renderer's own aliased-line
//!     triangles (line_style_vertices) matched the key in all four scenes with thin lines (0 pixels over 8);
//!   - a point whose center is exactly on a pixel boundary (here y = 128.0, the point at y 0.1 of a camera at y 0.1)
//!     is a tie at the edge of its sprite: GL takes the row at the bottom of the picture, the triangles of the sprite
//!     take the top row (Vulkan top-left rule). Fixed by the point-sprite tie rule in x3d.wgsl (#409): lines-edge-points
//!     at MSAA 0 is exact and is no longer in KNOWN;
//!   - the ends of aliased wide lines (width 2 to 10 at MSAA 0): GL's diamond-exit rule draws the pixel that holds
//!     the start point, the parallelogram of line_style_vertices stops at the pixel centers.
//!
//! The renderer is not changed here: each fix touches every line of every world (see the PR for the numbers).

mod common;

use common::gpu::gpu;
use common::reference::{check_adapter, platform, png_for};
use std::path::PathBuf;

const WORLDS: [&str; 7] = [
    "lines-edge",
    "lines-edge-points",
    "lines-edge-wide",
    "lines-edge-thin",
    "lines-edge-cam2",
    "lines-edge-cam3",
    "lines-edge-cam4",
];

/// (world, MSAA, pixels over 8, max delta) measured on the reference machine (RTX 3060 Ti, ANGLE/GL key against Vulkan
/// with `set_gl_sample_orientation` at MSAA 4). Cases not listed are exact (0 pixels over 8).
const KNOWN: &[(&str, u32, usize, u8)] = &[
    ("lines-edge", 0, 46, 255),
    ("lines-edge-wide", 0, 8, 255),
    ("lines-edge-thin", 0, 20, 255),
    ("lines-edge-cam2", 0, 13, 255),
    ("lines-edge-cam3", 0, 13, 255),
    ("lines-edge-cam4", 0, 25, 255),
    ("lines-edge-cam2", 4, 3, 64),
    ("lines-edge-cam3", 4, 1, 64),
    ("lines-edge-cam4", 4, 2, 64),
];

/// The record of the Apple M1 (`X3D_REFERENCE_PLATFORM=apple-m1`, Rust on Metal against the M1 pictures of
/// oracle/render-lines-edge-apple-m1-baseline and oracle/render-msaa4-lines-edge-apple-m1-baseline, issue #461). Cases not listed are exact.
const KNOWN_APPLE_M1: &[(&str, u32, usize, u8)] = &[];

/// The record of the reference in use.
fn known() -> &'static [(&'static str, u32, usize, u8)] {
    match platform().as_deref() {
        Some("apple-m1") => KNOWN_APPLE_M1,
        _ => KNOWN,
    }
}

/// How many pixels more than the record a case may show before the test fails (other GPUs differ by a tie or two).
const MARGIN: usize = 8;

fn key_dir(samples: u32) -> &'static str {
    match samples {
        0 => "render-lines-edge-baseline",
        _ => "render-msaa4-lines-edge-baseline",
    }
}

/// (world, samples, pixels over 8, max delta, picture size) for every world at MSAA 0 and 4.
fn measure_all() -> Option<Vec<(&'static str, u32, usize, u8, usize)>> {
    let Ok(mut g) = gpu() else {
        x3d_evidence::no_gpu("no adapter");
        return None;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let platform = platform();
    check_adapter(platform.as_deref(), &g.adapter_info);
    let mut out = vec![];

    for samples in [0u32, 4] {
        g.renderer.set_sample_count(samples);
        g.renderer.set_gl_sample_orientation(samples > 1);

        if samples > 1 && g.renderer.sample_count() != samples {
            eprintln!("{samples}x not supported here");
            continue;
        }

        for world in WORLDS {
            let key = png_for(
                &root,
                key_dir(samples),
                &format!("{world}.wrl.png"),
                platform.as_deref(),
            );
            let (w, h, want) = x3d_render::compare::load_png(&key)
                .unwrap_or_else(|e| panic!("{world} MSAA {samples}: no answer key {key:?}: {e}"));
            let shot = x3d_render::render_file(
                &mut g,
                &root,
                &root.join(format!("synthetic-lines-edge/{world}.wrl")),
                w,
                h,
            )
            .unwrap();
            let rgba: Vec<u8> = shot
                .rgba
                .chunks_exact(4)
                .flat_map(|p| [p[0], p[1], p[2], 255])
                .collect();
            let s = x3d_render::compare::compare(w, h, &want, &rgba, 8);

            eprintln!(
                "{world} MSAA {samples}: {} pixels over 8 ({:.3}%), max {}",
                s.pixels_over, s.pct_over, s.max_delta
            );
            out.push((world, samples, s.pixels_over, s.max_delta, (w * h) as usize));
        }
    }

    Some(out)
}

#[test]
fn lines_edge_renders_stay_within_the_bound() {
    let Some(all) = measure_all() else { return };
    let mut failures = vec![];

    for (world, samples, over, max, size) in &all {
        if over * 1000 > size * 5 {
            failures.push(format!(
                "{world} MSAA {samples}: {over} of {size} pixels over 8 (max {max})"
            ));
        }
    }

    assert!(failures.is_empty(), "{failures:#?}");
    assert!(all.len() >= WORLDS.len(), "MSAA 0 was not measured");
}

/// Records known differences. It does not show parity: see the file header.
#[test]
fn lines_edge_known_pixel_differences_are_recorded() {
    let Some(all) = measure_all() else { return };
    let mut failures = vec![];

    for (world, samples, over, max, _) in &all {
        match known()
            .iter()
            .find(|(w, s, _, _)| w == world && s == samples)
        {
            Some((_, _, known, known_max)) => {
                if *over > known + MARGIN || max > known_max {
                    failures.push(format!(
                        "{world} MSAA {samples}: {over} pixels over 8 (max {max}), recorded {known} (max {known_max})"
                    ));
                }

                if *over == 0 {
                    failures.push(format!(
                        "{world} MSAA {samples}: now exact; remove it from KNOWN"
                    ));
                }
            }
            None => {
                if *over > 0 {
                    failures.push(format!(
                        "{world} MSAA {samples}: {over} pixels over 8, recorded exact"
                    ));
                }
            }
        }
    }

    assert!(failures.is_empty(), "{failures:#?}");
}
