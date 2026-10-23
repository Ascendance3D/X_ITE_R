//! Multisampling (X_ITE MultiSampleFramebuffer): the sample-count clamp and the edge-scene
//! baselines captured from the pinned X_ITE with antialiasing on (oracle/synthetic-msaa,
//! baselines oracle/render-msaa{4,8}-synth-baseline, written by oracle/x_ite-shot-msaa.mjs).
//!
//! With `set_gl_sample_orientation` the Vulkan resolve is byte-identical to ANGLE/GL on the
//! same NVIDIA GPU; on other GPUs the sample positions may differ, so the comparison allows a
//! few one-sample edge pixels (see oracle/README.md).

mod common;

use common::gpu::gpu;
use std::path::PathBuf;

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

fn decode(p: &std::path::Path) -> Option<Vec<u8>> {
    Some(image::open(p).ok()?.to_rgba8().into_raw())
}

#[test]
fn sample_count_clamps_like_max_samples() {
    let Ok(mut g) = gpu() else {
        return eprintln!("no adapter");
    };
    let r = &mut g.renderer;
    let supported = r.supported_samples().to_vec();
    assert_eq!(supported[0], 1);

    for request in [0u32, 1, 2, 3, 4, 5, 8, 16, 100] {
        r.set_sample_count(request);
        let n = r.sample_count();
        assert!(
            supported.contains(&n),
            "{request} -> {n} not in {supported:?}"
        );
        assert!(n <= request.max(1));
        assert!(
            supported.iter().all(|&s| s > request.max(1) || s <= n),
            "{request} -> {n} is not the highest supported {supported:?}"
        );
    }

    // The cycle visits every supported count and wraps.
    r.set_sample_count(1);
    let mut seen = vec![r.sample_count()];
    for _ in 0..supported.len() {
        seen.push(r.cycle_sample_count());
    }
    assert_eq!(&seen[..supported.len()], &supported[..]);
    assert_eq!(seen[supported.len()], 1);
}

#[test]
fn edge_scenes_match_the_x_ite_resolve() {
    let Ok(mut g) = gpu() else {
        return eprintln!("no adapter");
    };
    let root = oracle().canonicalize().unwrap();
    let mut checked = 0;

    for samples in [4u32, 8] {
        g.renderer.set_sample_count(samples);
        g.renderer.set_gl_sample_orientation(true);

        if g.renderer.sample_count() != samples {
            eprintln!("{samples}x not supported here");
            continue;
        }

        for name in ["edge-quad", "edge-sliver", "edge-sphere", "edge-fine-lines"] {
            let Some(want) = decode(&root.join(format!(
                "render-msaa{samples}-synth-baseline/{name}.wrl.png"
            ))) else {
                return eprintln!("baseline missing");
            };
            let shot = x3d_render::render_file(
                &mut g,
                &root,
                &root.join(format!("synthetic-msaa/{name}.wrl")),
                512,
                512,
            )
            .unwrap();
            let worst = shot
                .rgba
                .chunks_exact(4)
                .zip(want.chunks_exact(4))
                .map(|(a, b)| (0..3).map(|c| a[c].abs_diff(b[c])).max().unwrap())
                .max()
                .unwrap();
            let over = shot
                .rgba
                .chunks_exact(4)
                .zip(want.chunks_exact(4))
                .filter(|(a, b)| (0..3).any(|c| a[c].abs_diff(b[c]) > 8))
                .count();
            // On the reference machine (RTX 3060 Ti, ANGLE/GL vs Vulkan): exactly 0.
            assert!(
                over * 1000 <= 512 * 512 * 5,
                "{name} {samples}x: {over} pixels over 8 (worst {worst})"
            );
            checked += 1;
        }
    }

    eprintln!("{checked} edge scenes checked");
}

#[test]
fn multisampling_changes_edges_only() {
    let Ok(mut g) = gpu() else {
        return eprintln!("no adapter");
    };
    let root = oracle().canonicalize().unwrap();
    let file = root.join("synthetic-msaa/edge-quad.wrl");
    g.renderer.set_sample_count(1);
    let a = x3d_render::render_file(&mut g, &root, &file, 512, 512)
        .unwrap()
        .rgba;
    g.renderer.set_sample_count(4);
    let b = x3d_render::render_file(&mut g, &root, &file, 512, 512)
        .unwrap()
        .rgba;

    if g.renderer.sample_count() < 2 {
        return;
    }

    assert_ne!(a, b);
    // Interior and background pixels are identical; only partly covered pixels change.
    let (mut same, mut edge) = (0, 0);
    for (p, q) in a.chunks_exact(4).zip(b.chunks_exact(4)) {
        if p == q { same += 1 } else { edge += 1 }
    }
    assert!(
        edge > 0 && edge * 20 < same,
        "{edge} changed vs {same} same"
    );
}
