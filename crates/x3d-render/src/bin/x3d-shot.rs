//! x3d-shot <file.wrl> <out.png> [--size 512x512] [--oracle <dir>]
//! x3d-shot batch <corpus-dir> <out-dir> [--size 512x512] [--only <substring>]
//! x3d-shot compare <baseline-dir> <rust-dir> [--threshold 8] [--max-pct 0.5] [--diff-dir /tmp/x3d-diff] [--json out.json]
//! x3d-shot info
//!
//! Renders offscreen with wgpu on Vulkan; never opens a window. The corpus must sit inside the
//! oracle directory (served by the oracle as http://oracle/), like `x3d-run`.

use std::path::{Path, PathBuf};
use std::process::ExitCode;
use x3d_render::compare;
use x3d_render::gpu::Gpu;

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let size = opt(&args, "--size").unwrap_or_else(|| "512x512".into());
    let (w, h) = size.split_once('x').map(|(a, b)| (a.parse().unwrap_or(512), b.parse().unwrap_or(512))).unwrap_or((512, 512));

    // QuickJS wants a big stack; run everything on a worker thread.
    let r = std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(move || {
            let opt = |f: &str| opt(&args, f);
            match args.get(1).map(String::as_str) {
            Some("info") => info(),
            Some("batch") if args.len() >= 4 => batch(&args[2], &args[3], w, h, opt("--only")),
            Some("compare") if args.len() >= 4 => compare_dirs(
                &args[2],
                &args[3],
                opt("--threshold").and_then(|t| t.parse().ok()).unwrap_or(8),
                opt("--max-pct").and_then(|t| t.parse().ok()).unwrap_or(0.5),
                opt("--diff-dir"),
                opt("--json"),
            ),
            Some(file) if args.len() >= 3 && !file.starts_with("--") => single(file, &args[2], w, h, opt("--oracle")),
            _ => {
                eprintln!(
                    "usage: x3d-shot <file.wrl> <out.png> [--size WxH] [--oracle dir] [--msaa N]\n       x3d-shot batch <corpus-dir> <out-dir> [--size WxH] [--only s] [--msaa N]\n       x3d-shot compare <baseline-dir> <rust-dir> [--threshold N] [--max-pct P] [--diff-dir d] [--json f]\n       x3d-shot info"
                );
                2
            }
        }})
        .unwrap()
        .join()
        .unwrap();

    ExitCode::from(r)
}

fn opt(args: &[String], f: &str) -> Option<String> {
    args.iter().position(|a| a == f).and_then(|i| args.get(i + 1)).cloned()
}

/// `--msaa N`: X_ITE's Multisampling (default here 0 = off, so the baselines stay valid).
fn msaa() -> u32 {
    let args: Vec<String> = std::env::args().collect();
    opt(&args, "--msaa").and_then(|v| v.parse().ok()).unwrap_or(0)
}

fn gpu() -> Gpu {
    match Gpu::new() {
        Ok(mut g) => {
            g.renderer.set_sample_count(msaa());
            // `--gl-samples`: mirror the draw so the MSAA sample pattern matches the GL oracle's.
            g.renderer.set_gl_sample_orientation(std::env::args().any(|a| a == "--gl-samples"));
            g
        }
        Err(e) => {
            eprintln!("error: {e}");
            std::process::exit(1);
        }
    }
}

fn info() -> u8 {
    let g = gpu();
    let i = &g.adapter_info;
    println!("supported sample counts: {:?}", g.renderer.supported_samples());
    println!("adapter: {} | backend {:?} | driver {} {} | type {:?}", i.name, i.backend, i.driver, i.driver_info, i.device_type);
    0
}

fn find_oracle_root(file: &Path) -> PathBuf {
    // The nearest ancestor named "oracle", else the grandparent like x3d-run.
    let mut p = file.parent();
    while let Some(d) = p {
        if d.file_name().is_some_and(|n| n == "oracle") {
            return d.to_path_buf();
        }
        p = d.parent();
    }
    file.parent().and_then(|p| p.parent()).unwrap_or(Path::new("/")).to_path_buf()
}

fn single(file: &str, out: &str, w: u32, h: u32, oracle: Option<String>) -> u8 {
    let file = match std::fs::canonicalize(file) {
        Ok(f) => f,
        Err(e) => {
            eprintln!("error: {file}: {e}");
            return 1;
        }
    };
    let root = oracle.map(|o| std::fs::canonicalize(o).unwrap()).unwrap_or_else(|| find_oracle_root(&file));
    let mut g = gpu();

    match x3d_render::render_file(&mut g, &root, &file, w, h) {
        Ok(shot) => {
            let rgba: Vec<u8> = shot.rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
            if let Err(e) = x3d_render::write_png(Path::new(out), w, h, &rgba) {
                eprintln!("error: {e}");
                return 1;
            }
            if !shot.unsupported.is_empty() {
                eprintln!("not rendered: {}", shot.unsupported.join(", "));
            }
            0
        }
        Err(e) => {
            eprintln!("error: {e}");
            1
        }
    }
}

fn walk(d: &Path, out: &mut Vec<PathBuf>) {
    if let Ok(rd) = std::fs::read_dir(d) {
        for e in rd.flatten() {
            let p = e.path();
            if p.is_dir() {
                walk(&p, out);
            } else if p.extension().is_some_and(|x| x == "wrl" || x == "wrz" || x == "gz") {
                out.push(p);
            }
        }
    }
}

fn batch(corpus: &str, out_dir: &str, w: u32, h: u32, only: Option<String>) -> u8 {
    let corpus = std::fs::canonicalize(corpus).expect("corpus");
    let root = corpus.parent().unwrap().to_path_buf();
    let out_dir = PathBuf::from(out_dir);
    let mut files = vec![];
    walk(&corpus, &mut files);
    files.sort();
    if let Some(o) = &only {
        files.retain(|f| f.to_string_lossy().contains(o.as_str()));
    }

    let mut g = gpu();
    let mut index = vec![];
    let t0 = std::time::Instant::now();

    for f in &files {
        let rel = f.strip_prefix(&corpus).unwrap().to_string_lossy().to_string();
        let out = out_dir.join(format!("{rel}.png"));
        match x3d_render::render_file(&mut g, &root, f, w, h) {
            Ok(shot) => {
                let rgba: Vec<u8> = shot.rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2], 255]).collect();
                let _ = x3d_render::write_png(&out, w, h, &rgba);
                println!("ok   {rel}{}", if shot.unsupported.is_empty() { String::new() } else { format!("  [not rendered: {}]", shot.unsupported.join(", ")) });
                index.push(serde_json::json!({ "file": rel, "ok": true, "unsupported": shot.unsupported }));
            }
            Err(e) => {
                println!("FAIL {rel}  {e}");
                index.push(serde_json::json!({ "file": rel, "ok": false, "error": e }));
            }
        }
    }

    let i = &g.adapter_info;
    let json = serde_json::json!({
        "adapter": format!("{} ({:?}, {} {})", i.name, i.backend, i.driver, i.driver_info),
        "size": [w, h],
        "results": index,
    });
    let _ = std::fs::create_dir_all(&out_dir);
    let _ = std::fs::write(out_dir.join("index.json"), serde_json::to_string_pretty(&json).unwrap());
    eprintln!("{} files in {:.1}s", files.len(), t0.elapsed().as_secs_f64());
    0
}

fn compare_dirs(baseline: &str, rust: &str, threshold: u8, max_pct: f64, diff_dir: Option<String>, json: Option<String>) -> u8 {
    let baseline = PathBuf::from(baseline);
    let rust = PathBuf::from(rust);
    let mut files = vec![];
    fn walk_png(d: &Path, out: &mut Vec<PathBuf>) {
        if let Ok(rd) = std::fs::read_dir(d) {
            for e in rd.flatten() {
                let p = e.path();
                if p.is_dir() {
                    walk_png(&p, out);
                } else if p.extension().is_some_and(|x| x == "png") {
                    out.push(p);
                }
            }
        }
    }
    walk_png(&baseline, &mut files);
    files.sort();

    let (mut pass, mut fail, mut missing) = (0, 0, 0);
    let mut rows = vec![];

    for f in &files {
        let rel = f.strip_prefix(&baseline).unwrap();
        let r = rust.join(rel);
        let name = rel.to_string_lossy().to_string();
        let (Ok((w, h, a)), Ok((w2, h2, b))) = (compare::load_png(f), compare::load_png(&r)) else {
            missing += 1;
            println!("MISSING {name}");
            rows.push(serde_json::json!({ "file": name, "status": "missing" }));
            continue;
        };
        if (w, h) != (w2, h2) {
            fail += 1;
            println!("SIZE    {name}");
            continue;
        }
        let s = compare::compare(w, h, &a, &b, threshold);
        let ok = s.pct_over <= max_pct;
        if ok {
            pass += 1;
        } else {
            fail += 1;
            if let Some(d) = &diff_dir {
                let out = Path::new(d).join(format!("{name}.diff.png"));
                let img = compare::diff_image(w, h, &a, &b, threshold);
                let _ = x3d_render::write_png(&out, w * 3, h, &img);
            }
        }
        println!(
            "{} {name}  max {:3}  over {:6} ({:6.3}%)  diff {:6}  psnr {:6.2}  ssim {:.4}",
            if ok { "PASS   " } else { "FAIL   " },
            s.max_delta,
            s.pixels_over,
            s.pct_over,
            s.pixels_diff,
            s.psnr,
            s.ssim
        );
        rows.push(serde_json::json!({
            "file": name, "status": if ok { "pass" } else { "fail" }, "maxDelta": s.max_delta, "pixelsOver": s.pixels_over,
            "pctOver": s.pct_over, "pixelsDiff": s.pixels_diff, "meanAbs": s.mean_abs, "psnr": if s.psnr.is_finite() { s.psnr } else { 999.0 }, "ssim": s.ssim,
        }));
    }

    println!("\n{pass} within tolerance, {fail} over, {missing} missing (threshold {threshold}, max {max_pct}% of pixels)");

    if let Some(j) = json {
        let _ = std::fs::write(j, serde_json::to_string_pretty(&serde_json::json!({ "threshold": threshold, "maxPct": max_pct, "pass": pass, "fail": fail, "missing": missing, "results": rows })).unwrap());
    }

    0
}
