//! Issue #69, pass 1: GaussianSplats under the logarithmic depth buffer (`DepthMode::Logarithmic`) against pinned X_ITE
//! (16.5.0, 34cdf9f97) run with the `LogarithmicDepthBuffer` rendering property (`oracle/x_ite-shot-log.mjs`, page
//! `shot-log.html`) over the synthetic worlds of `oracle/synthetic-rea-b08-gs`: `oracle/render-b69-gs-log-baseline`.
//! GaussianSplats.vs.js calls `logarithmic (gl_Position)` and GaussianSplats.fs.js `logarithmic ()`, so the splats are depth
//! tested against opaque shapes in the same logarithmic depth. `REA_ONLY=<substring>` restricts the worlds.
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off.

use std::path::PathBuf;
use std::sync::Mutex;
use x3d_render::gpu::{DepthMode, Gpu};

/// The logarithmic mode switches a process-wide property (see tests/depth.rs).
static SERIAL: Mutex<()> = Mutex::new(());

#[test]
fn splats_in_logarithmic_depth_match_x_ite() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let only = std::env::var("REA_ONLY").unwrap_or_default();
    let baseline = "render-b69-gs-log-baseline";
    let mut names: Vec<String> = std::fs::read_dir(root.join(baseline))
        .unwrap()
        .filter_map(|e| {
            e.ok()?
                .file_name()
                .into_string()
                .ok()?
                .strip_suffix(".png")
                .map(str::to_string)
        })
        .filter(|n| n.contains(&only))
        .collect();
    let mut failures = vec![];

    names.sort();
    g.renderer.set_depth_mode(DepthMode::Logarithmic);

    for name in &names {
        let (w, h, want) =
            x3d_render::compare::load_png(&root.join(format!("{baseline}/{name}.png"))).unwrap();
        let shot = x3d_render::render_file(
            &mut g,
            &root,
            &root.join(format!("synthetic-rea-b08-gs/{name}")),
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
            "{name}: over {} ({:.3}%), max {}",
            s.pixels_over, s.pct_over, s.max_delta
        );

        if s.pixels_over * 1000 > (w * h) as usize * 5 {
            failures.push(format!(
                "{name}: {} pixels over 8 ({:.3}%), max {}",
                s.pixels_over, s.pct_over, s.max_delta
            ));
        }
    }

    g.renderer.set_depth_mode(DepthMode::Standard);
    assert!(!names.is_empty());
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
