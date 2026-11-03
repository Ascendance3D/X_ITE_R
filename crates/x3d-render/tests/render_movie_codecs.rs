//! Native video decoding against pinned X_ITE 16.5.0 (issue #90): the first picture of every VP8 stream of
//! oracle/synthetic-movie-codecs (`oracle/tools/gen_movie_codecs.py`), drawn on an unlit rectangle, against the
//! baseline X_ITE recorded in oracle/render-movie-codecs-baseline (`node oracle/x_ite-shot.mjs
//! oracle/synthetic-movie-codecs oracle/render-movie-codecs-baseline`; headless Chrome decodes the video, X_ITE uploads
//! the `video` element).
//!
//! The bound is the one of `x3d-shot compare` and of the other still baselines, fixed before the first run: at most
//! 0.5% of the pixels more than 8 off (BR1.4's timing tolerance does not apply: a still has no timing). The inter
//! frames are covered by `crates/x3d-runtime/src/webm.rs`, which compares every picture of every stream plane by plane
//! with an independent decoder.

use std::path::PathBuf;
use x3d_render::gpu::Gpu;

#[test]
fn native_video_first_pictures_match_x_ite() {
    let Ok(mut g) = Gpu::new() else {
        return x3d_evidence::no_gpu("no adapter");
    };
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap();
    let baseline = root.join("render-movie-codecs-baseline");
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
        .collect();
    let mut failures = vec![];

    names.sort();
    assert!(!names.is_empty(), "no baselines found");

    // A world without its reference fails: an absent answer key never passes. The baseline PNGs are named after the
    // world file, so their stem already carries the `.wrl` (`vp8-crop.wrl.png`). A missing .webm makes `render_file`
    // fail and the unwrap below panic: the test fails, it does not skip.
    for e in std::fs::read_dir(root.join("synthetic-movie-codecs")).unwrap() {
        let world = e.unwrap().file_name().into_string().unwrap();

        if world.ends_with(".wrl") && !names.contains(&world) {
            failures.push(format!("{world}: no reference in the baseline"));
        }
    }

    // A reference whose X_ITE run logged anything does not picture the world as written.
    let index: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(baseline.join("index.json")).unwrap())
            .unwrap();

    for r in index["results"].as_array().unwrap() {
        if !r["log"].as_array().unwrap().is_empty() || r["ok"] != true {
            failures.push(format!(
                "{}: the reference run logged {:?}",
                r["file"], r["log"]
            ));
        }
    }

    for name in names {
        let (w, h, want) =
            x3d_render::compare::load_png(&baseline.join(format!("{name}.png"))).unwrap();
        let shot = x3d_render::render_file(
            &mut g,
            &root,
            &root.join(format!("synthetic-movie-codecs/{name}")),
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
            "{name}: {} pixels over 8 ({:.3}%), max {}",
            s.pixels_over, s.pct_over, s.max_delta
        );

        // KNOWN difference (BR1.7, BR1.8): Chrome decodes the Opus track, no Opus decoder is admitted here, so the
        // video plays silently and the one diagnostic says so. The picture is still compared.
        let opus = name == "vp8-opus.wrl"
            && shot.log.len() == 1
            && shot.log[0].contains("A_OPUS")
            && shot.log[0].contains("silently");

        if !opus && (!shot.log.is_empty() || !shot.unsupported.is_empty()) {
            failures.push(format!(
                "{name}: X_ITE logged nothing, Rust logged {:?} / unsupported {:?}",
                shot.log, shot.unsupported
            ));
        }

        if s.pixels_over * 1000 > (w * h) as usize * 5 {
            failures.push(format!(
                "{name}: {} pixels over 8 ({:.3}%), max {}",
                s.pixels_over, s.pct_over, s.max_delta
            ));
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
