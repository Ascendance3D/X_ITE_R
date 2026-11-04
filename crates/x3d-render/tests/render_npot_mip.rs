//! Issue #51 (ledger `sub:render.core`): mipmaps of textures whose width or height is not a power of two. The ten worlds
//! of oracle/synthetic-npot-mip draw ImageTexture images (odd and even sizes from 1x7 to 301x201, the default
//! TextureProperties and an explicit generateMipMaps TRUE, PNG with and without alpha, and the power-of-two control
//! 128x64) on quads that are minified by 1.4 to 24 and tilted. The oracle is the pinned X_ITE 16.5.0 (34cdf9f97) picture
//! of every world, recorded three times with identical pixels by oracle/tools/rea_npot_mip.py:
//! oracle/render-npot-mip-baseline/msaa0 (Multisampling 0) and .../msaa4 (Multisampling 4, the Rust render uses
//! `--gl-samples` orientation as `x3d-shot` does).
//!
//! The bound is the one of `x3d-shot compare`: at most 0.5% of the pixels more than 8 off. It holds for nine worlds. The
//! tenth, `npot-parity-shifted`, is a measured limit of the pinned driver, not of the port: for some sizes with one odd
//! and one even side the NVIDIA `gl.generateMipmap` shifts the even side by one texel (second test below), which no rule
//! of GL or X3D predicts. Its state is pinned here as measured (more than the bound, less than a ceiling) and the bound
//! is not widened for it. The test also prints the pixels more than 2 off. `NPOT_ONLY=<substring>` restricts the worlds;
//! `NPOT_DUMP=<dir>` writes the Rust pictures there.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

/// Worlds whose pinned picture holds the shifted mip chain of the driver (see the second test): the Rust picture differs
/// by more than the bound, as measured, and is pinned to that measurement instead of the bound.
const KNOWN_DRIVER_LIMIT: [&str; 1] = ["npot-parity-shifted.wrl"];

#[test]
fn npot_mipmaps_match_the_pinned_pictures() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let only = std::env::var("NPOT_ONLY").unwrap_or_default();
    let dump = std::env::var_os("NPOT_DUMP").map(PathBuf::from);
    let mut failures = vec![];
    let mut checked = 0;

    for samples in [0u32, 4] {
        g.renderer.set_sample_count(samples);
        g.renderer.set_gl_sample_orientation(samples > 1);

        if samples > 1 && g.renderer.sample_count() != samples {
            eprintln!("{samples}x not supported here");
            continue;
        }

        let baseline = root.join(format!("render-npot-mip-baseline/msaa{samples}"));
        let mut names: Vec<String> = std::fs::read_dir(&baseline)
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

        names.sort();

        for name in &names {
            let (w, h, want) =
                x3d_render::compare::load_png(&baseline.join(format!("{name}.png"))).unwrap();
            let shot = x3d_render::render_file(
                &mut g,
                &root,
                &root.join(format!("synthetic-npot-mip/{name}")),
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
            let s2 = x3d_render::compare::compare(w, h, &want, &rgba, 2);

            eprintln!(
                "{name} MSAA {samples}: over 8 {} ({:.3}%), over 2 {} ({:.3}%), max {}",
                s.pixels_over, s.pct_over, s2.pixels_over, s2.pct_over, s.max_delta
            );
            checked += 1;

            if let Some(dir) = &dump {
                std::fs::create_dir_all(dir).unwrap();
                image::save_buffer(
                    dir.join(format!("msaa{samples}-{name}.png")),
                    &rgba,
                    w,
                    h,
                    image::ColorType::Rgba8,
                )
                .unwrap();
            }

            let over_bound = s.pixels_over * 1000 > (w * h) as usize * 5;

            if KNOWN_DRIVER_LIMIT.contains(&name.as_str()) {
                // Measured with the pinned recording: 7274 (MSAA 0) and 7321 (MSAA 4) pixels, 2.8%, max 85.
                if !over_bound || s.pct_over > 3.5 || s.max_delta > 100 {
                    failures.push(format!(
                        "{name} MSAA {samples}: the recorded driver limit changed: {} pixels over 8 ({:.3}%), max {}",
                        s.pixels_over, s.pct_over, s.max_delta
                    ));
                }
            } else if over_bound {
                failures.push(format!(
                    "{name} MSAA {samples}: {} pixels over 8 ({:.3}%), max {}",
                    s.pixels_over, s.pct_over, s.max_delta
                ));
            }
        }
    }

    g.renderer.set_sample_count(0);
    g.renderer.set_gl_sample_orientation(false);
    eprintln!("{checked} shots checked");
    assert!(checked > 0);
    assert!(failures.is_empty(), "{}", failures.join("\n"));
}

// ---- The mip chain itself, against the levels the pinned driver builds (no GPU needed).

use x3d_render::texture::{Filter, Sampler, TextureImage, mip_chain};

fn base64(s: &str) -> Vec<u8> {
    let val = |c: u8| match c {
        b'A'..=b'Z' => c - b'A',
        b'a'..=b'z' => c - b'a' + 26,
        b'0'..=b'9' => c - b'0' + 52,
        b'+' => 62,
        _ => 63,
    } as u32;
    let mut out = vec![];

    for q in s.trim_end_matches('=').as_bytes().chunks(4) {
        let n = q.iter().fold(0u32, |n, &c| n << 6 | val(c)) << (6 * (4 - q.len()));
        out.extend_from_slice(&n.to_be_bytes()[1..q.len()]);
    }

    out
}

fn image(w: u32, h: u32, rgba: Vec<u8>) -> TextureImage {
    TextureImage {
        width: w,
        height: h,
        rgba,
        transparent: false,
        flip_y: false,
        repeat_s: true,
        repeat_t: true,
        nearest: false,
        no_mipmaps: false,
        url: String::new(),
        sampler: Sampler::new(
            Filter::Linear,
            Filter::Linear,
            Some(Filter::Nearest),
            true,
            true,
        ),
        cube: false,
        generated: None,
        volume: false,
        depth: 1,
        float: false,
    }
}

/// The weights of texel `x` of a halved side of `n` texels, as the pinned driver does it for an odd `n` (the 3-tap
/// polyphase filter, the same as `texture::mip_chain`) and for an even `n` next to an odd side: with `shift`, the two
/// source texels 2x - 1 and 2x (the first texel twice for x = 0), not 2x and 2x + 1.
fn taps(n: usize, x: usize, shift: bool) -> Vec<(usize, f64)> {
    match n {
        1 => vec![(0, 1.0)],
        n if n % 2 == 1 => {
            let (k, d, xf) = ((n / 2) as f64, n as f64, x as f64);
            vec![
                (2 * x, (k - xf) / d),
                (2 * x + 1, k / d),
                (2 * x + 2, (xf + 1.0) / d),
            ]
        }
        _ if shift => vec![((2 * x).saturating_sub(1), 0.5), (2 * x, 0.5)],
        _ => vec![(2 * x, 0.5), (2 * x + 1, 0.5)],
    }
}

/// One halving step like `taps`, from RGBA8 rows, rounding once.
fn halve(w: usize, h: usize, src: &[u8], shift: bool) -> (usize, usize, Vec<u8>) {
    let (nw, nh) = ((w / 2).max(1), (h / 2).max(1));
    // An even side is shifted only next to an odd side.
    let (shift_x, shift_y) = (
        shift && w % 2 == 0 && h % 2 == 1,
        shift && h % 2 == 0 && w % 2 == 1,
    );
    let mut dst = vec![0u8; nw * nh * 4];

    for y in 0..nh {
        for x in 0..nw {
            for c in 0..4 {
                let mut sum = 0.0;

                for (sy, wy) in taps(h, y, shift_y) {
                    for (sx, wx) in taps(w, x, shift_x) {
                        sum += wx * wy * src[(sy * w + sx) * 4 + c] as f64;
                    }
                }

                dst[(y * nw + x) * 4 + c] = sum.round().clamp(0.0, 255.0) as u8;
            }
        }
    }

    (nw, nh, dst)
}

fn max_diff(a: &[u8], b: &[u8]) -> u8 {
    assert_eq!(a.len(), b.len());
    a.iter()
        .zip(b)
        .map(|(a, b)| a.abs_diff(*b))
        .max()
        .unwrap_or(0)
}

/// oracle/render-npot-mip-baseline/driver-levels.json is `gl.generateMipmap` of the pinned recorder's headless Chrome
/// (ANGLE on the NVIDIA GPU, oracle/tools/npot_mip_probe.mjs) for noise textures. Every level of every size is rebuilt
/// from the level above it (so one level cannot hide in the next) by the Rust filter (`standard`) and by the shifted
/// variant. Measured: the driver gives the standard result for the levels of a size with two even or two odd sides
/// (and the odd-odd levels of every size), and for some sizes with one odd and one even side it shifts the even side
/// by one texel (`taps`). Which sizes it shifts looks like a rounding of the driver's own float arithmetic (for a
/// power-of-two even side it is whether the float32 of 1 / (odd side / 2) is below the exact value; other even sides
/// mix in their own rounding), so the port does not copy it: it is not a rule of GL or of X3D, and another driver
/// builds the standard chain.
#[test]
fn mip_chain_matches_the_pinned_driver_levels_except_the_shifted_even_side() {
    let root =
        PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/render-npot-mip-baseline");
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(root.join("driver-levels.json")).unwrap())
            .unwrap();
    // Level 1 of these sizes is the shifted one in the driver (the first level that differs from the standard chain).
    let shifted_at_level_1 = ["75x50", "50x75", "51x16", "16x51", "7x6", "6x7"];
    let mut shifted_seen = 0;

    for case in json["cases"].as_array().unwrap() {
        let name = format!("{}x{}", case["w"], case["h"]);
        let levels: Vec<(usize, usize, Vec<u8>)> = case["levels"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| {
                (
                    l["w"].as_u64().unwrap() as usize,
                    l["h"].as_u64().unwrap() as usize,
                    base64(l["rgba"].as_str().unwrap()),
                )
            })
            .collect();

        for k in 1..levels.len() {
            let (w, h, ref above) = levels[k - 1];
            let (_, _, ref driver) = levels[k];
            let rust = &mip_chain(&image(w as u32, h as u32, above.clone()))[1].2;
            let standard = max_diff(rust, driver);
            let (_, _, shifted) = halve(w, h, above, true);
            let shift = max_diff(&shifted, driver);
            eprintln!("{name} level {k} ({w}x{h}): Rust {standard}, shifted {shift}");

            // The driver always builds one of the two (to one level of rounding).
            assert!(
                standard.min(shift) <= 1,
                "{name} level {k}: neither chain matches"
            );

            if k == 1 && shifted_at_level_1.contains(&name.as_str()) {
                assert!(
                    standard > 8,
                    "{name} level 1: the driver stopped shifting (Rust {standard})"
                );
                assert!(shift <= 1, "{name} level 1: not the shifted box ({shift})");
                shifted_seen += 1;
            } else if k == 1 {
                assert!(
                    standard <= 1,
                    "{name} level 1: Rust {standard} off the driver"
                );
            }

            // A level with two even or two odd sides is never shifted.
            if (w % 2) == (h % 2) {
                assert!(
                    standard <= 1,
                    "{name} level {k}: Rust {standard} off the driver"
                );
            }
        }
    }

    assert_eq!(shifted_seen, shifted_at_level_1.len());
}
