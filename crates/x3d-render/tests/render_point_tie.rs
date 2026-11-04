//! Point sprites whose edges lie on pixel centers (issue #51, sub:render.core): the Rust picture against the pinned
//! X_ITE 16.5.0 (oracle/synthetic-point-tie, made by `oracle/tools/gen_point_tie.py`; keys
//! oracle/render-point-tie-baseline at MSAA 0 and oracle/render-msaa4-point-tie-baseline at Multisampling 4, each
//! recorded three times with identical pixels by x_ite-shot.mjs / x_ite-shot-msaa.mjs, 256 x 256).
//!
//! Every world is a 6 x 6 grid of PointSet points seen by an OrthoViewpoint, so the window position of each point is
//! exact. In one world the sprite size is fixed (1 to 9 pixels) and the sub-pixel position of the points runs through:
//! an edge exactly on a pixel center (the tie), the edge on a pixel corner, a little either side of the tie, and two
//! plain positions. `point-tie-control` and `point-tie-control3` have only plain positions. `point-tie-snap` moves a
//! sprite in steps of 1/1024 pixel across a tie, along x and along y: GL snaps the position of a point to 1/256 pixel
//! (a half step goes to the even one) and then fills the square [low, high) in both axes.
//!
//! Two tests:
//! * `point_tie_renders_stay_within_the_bound` is the check of `x3d-shot compare`: at most 0.5% of the pixels more than
//!   8 off. A tie moves `size` pixels per point, so this bound let the defect through (324 of 65536 pixels, 0.494%, in
//!   the worst world before the fix).
//! * `point_tie_renders_are_exact` asks for 0 pixels over 8 in every world at MSAA 0 and 4: the worlds are made of
//!   flat colored squares at exact positions, so a single pixel over 8 is a wrong tie.

mod common;

use common::gpu::gpu;
use std::path::PathBuf;

const WORLDS: [&str; 12] = [
    "point-tie-1",
    "point-tie-2",
    "point-tie-2p5",
    "point-tie-3",
    "point-tie-4",
    "point-tie-5",
    "point-tie-6",
    "point-tie-8p5",
    "point-tie-9",
    "point-tie-control",
    "point-tie-control3",
    "point-tie-snap",
];

fn key_dir(samples: u32) -> &'static str {
    match samples {
        0 => "render-point-tie-baseline",
        _ => "render-msaa4-point-tie-baseline",
    }
}

/// (world, MSAA, pixels over 8, max delta, picture size) for every world at MSAA 0 and 4.
fn measure_all() -> Option<Vec<(&'static str, u32, usize, u8, usize)>> {
    let Ok(mut g) = gpu() else {
        x3d_evidence::no_gpu("no adapter");
        return None;
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let mut out = vec![];

    for samples in [0u32, 4] {
        g.renderer.set_sample_count(samples);
        g.renderer.set_gl_sample_orientation(samples > 1);

        if samples > 1 && g.renderer.sample_count() != samples {
            eprintln!("{samples}x not supported here");
            continue;
        }

        for world in WORLDS {
            let key = root.join(format!("{}/{world}.wrl.png", key_dir(samples)));
            let (w, h, want) = x3d_render::compare::load_png(&key)
                .unwrap_or_else(|e| panic!("{world} MSAA {samples}: no answer key {key:?}: {e}"));
            let shot = x3d_render::render_file(
                &mut g,
                &root,
                &root.join(format!("synthetic-point-tie/{world}.wrl")),
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
fn point_tie_renders_stay_within_the_bound() {
    let Some(all) = measure_all() else { return };
    let failures: Vec<String> = all
        .iter()
        .filter(|(_, _, over, _, size)| over * 1000 > size * 5)
        .map(|(world, samples, over, max, size)| {
            format!("{world} MSAA {samples}: {over} of {size} pixels over 8 (max {max})")
        })
        .collect();

    assert!(failures.is_empty(), "{failures:#?}");
    assert!(all.len() >= WORLDS.len(), "MSAA 0 was not measured");
}

#[test]
fn point_tie_renders_are_exact() {
    let Some(all) = measure_all() else { return };
    let failures: Vec<String> = all
        .iter()
        .filter(|(_, _, over, _, _)| *over > 0)
        .map(|(world, samples, over, max, _)| {
            format!("{world} MSAA {samples}: {over} pixels over 8 (max {max})")
        })
        .collect();

    assert!(failures.is_empty(), "{failures:#?}");
    assert!(all.len() >= WORLDS.len(), "MSAA 0 was not measured");
}
