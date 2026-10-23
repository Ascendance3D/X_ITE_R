//! Depth modes (`gpu::DepthMode`).
//!
//! * `Standard` and `Logarithmic` against X_ITE (the pinned build, headless Chrome on the same
//!   GPU, `oracle/render-zfight-baseline` and `oracle/render-zfight-log-baseline`, written by
//!   `oracle/x_ite-shot.mjs` and `oracle/x_ite-shot-log.mjs` over `oracle/synthetic-zfight`).
//! * `Reversed` is not in X_ITE: it must resolve near-coplanar surfaces better than the standard
//!   buffer and leave a scene without such surfaces unchanged.

use std::path::PathBuf;
use std::sync::Mutex;
use x3d_render::gpu::{DepthMode, Gpu};

/// The logarithmic mode switches a process-wide property (far plane 1e10, like X_ITE's browser-wide
/// `LogarithmicDepthBuffer`), so the tests of this file must not overlap.
static SERIAL: Mutex<()> = Mutex::new(());

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap()
}

fn decode(p: &std::path::Path) -> Vec<u8> {
    image::open(p)
        .unwrap_or_else(|e| panic!("{}: {e}", p.display()))
        .to_rgba8()
        .into_raw()
}

fn render(g: &mut Gpu, mode: DepthMode, dir: &str, name: &str) -> Vec<u8> {
    g.renderer.set_depth_mode(mode);
    let root = oracle();
    let rgba = x3d_render::render_file(g, &root, &root.join(format!("{dir}/{name}.wrl")), 512, 512)
        .unwrap()
        .rgba;
    g.renderer.set_depth_mode(DepthMode::Standard);
    rgba
}

fn over(a: &[u8], b: &[u8]) -> usize {
    a.chunks_exact(4)
        .zip(b.chunks_exact(4))
        .filter(|(p, q)| (0..3).any(|c| p[c].abs_diff(q[c]) > 8))
        .count()
}

#[test]
fn standard_and_log_match_x_ite() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(mut g) = Gpu::new() else {
        return eprintln!("no adapter");
    };

    for (mode, baseline) in [
        (DepthMode::Standard, "render-zfight-baseline"),
        (DepthMode::Logarithmic, "render-zfight-log-baseline"),
    ] {
        for name in ["tilt-pair", "far-range"] {
            let want = decode(&oracle().join(format!("{baseline}/{name}.wrl.png")));
            let got = render(&mut g, mode, "synthetic-zfight", name);
            let n = over(&got, &want);
            assert!(
                n * 1000 <= 512 * 512 * 5,
                "{mode:?} {name}: {n} pixels over 8"
            );
        }
    }
}

#[test]
fn log_shows_what_standard_cannot() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(mut g) = Gpu::new() else {
        return eprintln!("no adapter");
    };
    let px = |rgba: &[u8], x: usize, y: usize| {
        [
            rgba[(y * 512 + x) * 4],
            rgba[(y * 512 + x) * 4 + 1],
            rgba[(y * 512 + x) * 4 + 2],
        ]
    };
    let std = render(&mut g, DepthMode::Standard, "synthetic-zfight", "far-range");
    let log = render(
        &mut g,
        DepthMode::Logarithmic,
        "synthetic-zfight",
        "far-range",
    );
    // Center: red (nearer) over green (5 units behind at z = -1e4). Standard 24-bit depth ties them
    // and the later green wins; the log buffer resolves it. The corner shows the blue quad at
    // z = -2e5, beyond the standard far plane (1e5) but inside the log one (1e10).
    assert_eq!(px(&std, 256, 256), [0, 255, 0]);
    assert_eq!(px(&log, 256, 256), [255, 0, 0]);
    assert_eq!(px(&std, 10, 10), [0, 0, 0]);
    assert_eq!(px(&log, 10, 10), [0, 0, 255]);
}

/// Pixels on the wrong side of the plane crossing at x = 0 (screen column 256) in tilt-pair.
fn wrong_side(rgba: &[u8]) -> usize {
    let mut n = 0;
    for y in 100..412 {
        for x in 20..492 {
            let p = &rgba[(y * 512 + x) * 4..(y * 512 + x) * 4 + 3];
            let (red, green) = (p == [255, 0, 0], p == [0, 255, 0]);
            if (x < 256 && green) || (x >= 256 && red) {
                n += 1;
            }
        }
    }
    n
}

#[test]
fn reversed_resolves_near_coplanar_surfaces() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(mut g) = Gpu::new() else {
        return eprintln!("no adapter");
    };
    let std = wrong_side(&render(
        &mut g,
        DepthMode::Standard,
        "synthetic-zfight",
        "tilt-pair",
    ));
    let rev = wrong_side(&render(
        &mut g,
        DepthMode::Reversed,
        "synthetic-zfight",
        "tilt-pair",
    ));
    eprintln!("wrong-side pixels: standard {std}, reversed {rev}");
    assert!(
        std > 1000,
        "the standard buffer should misplace the crossing (got {std})"
    );
    assert!(rev * 20 < std, "reversed {rev} vs standard {std}");
}

#[test]
fn reversed_leaves_other_scenes_alone() {
    let _serial = SERIAL.lock().unwrap_or_else(|e| e.into_inner());
    let Ok(mut g) = Gpu::new() else {
        return eprintln!("no adapter");
    };

    for name in ["edge-quad", "edge-sliver", "edge-sphere", "edge-fine-lines"] {
        let a = render(&mut g, DepthMode::Standard, "synthetic-msaa", name);
        let b = render(&mut g, DepthMode::Reversed, "synthetic-msaa", name);
        assert_eq!(over(&a, &b), 0, "{name}");
    }
}
