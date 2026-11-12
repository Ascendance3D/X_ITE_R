//! MovieTexture GIF / APNG (issue #90) against pinned X_ITE 16.5.0. Each baseline was recorded five times with
//! identical results by `oracle/tools/movie_anim.py record`:
//!
//! - oracle/movie-anim-baseline: GIF frames chosen from elapsedTime in equal steps (loop, no loop, speed 2, 0.5 and
//!   -1, stopTime, enabled FALSE), GIF disposal and transparency, and the APNG states that do not depend on the wall
//!   clock (frame 0 before the start, a 60 s first frame, speed 0). The world and its event script are
//!   oracle/synthetic-movie-anim (oracle/tools/gen_movie_anim.py): a key press starts every MovieTexture through a
//!   Script.
//! - oracle/movie-load-errors-baseline: the load states of PNG data that apng-js does not take as an animated PNG
//!   (IN_PROGRESS), of a PNG cut in its header (warning, next URL, FAILED) and of APNGs with cut or invalid frame
//!   data (COMPLETE), from oracle/synthetic-movie-load-errors (oracle/tools/gen_movie_load_errors.py).
//!
//! Each case of a baseline's cases.txt (`name world script msaa [text_tol]`) runs `x3d-drive` on the
//! world and its event script and compares the result with the oracle's `js.json` like
//! crates/x3d-viewer/tests/rea_batch09_inputs.rs: scene text exact (or within `text_tol`), poses within 1e-4, the
//! sorted log exact, the `log:` lines of the console in order (the X_ITE page keeps warnings and errors only in
//! the log), captured frames with at most 0.5% of the pixels more than 8 off. Without a GPU adapter the
//! image part is skipped and reported (x3d_evidence::no_gpu).
//!
//! One known difference is pinned in oracle/movie-load-errors-baseline, not passed ([`KNOWN_DIFFERENCES`]): the cases `static`,
//! `notpng` and `noframes`. X_ITE 16.5.0 hangs there (apng-js returns its error, `setAPNG` is not awaited, the node stays
//! IN_PROGRESS without a message, MovieTexture.js:98); the port fails the node like any other load error. The baseline is not
//! edited: the test requires the port's log and console to be the pinned ones plus exactly the listed lines, and fails as soon as
//! the port stops differing or differs in another way (docs/adr/0001-browser-build-known-differences.md, row 14).
//!
//! `REA_ONLY=<substring>` restricts the cases.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

fn quat(r: &[f64]) -> [f64; 4] {
    let n = (r[0] * r[0] + r[1] * r[1] + r[2] * r[2]).sqrt();
    let n = if n == 0.0 { 1.0 } else { n };
    let s = (r[3] / 2.0).sin() / n;
    [r[0] * s, r[1] * s, r[2] * s, (r[3] / 2.0).cos()]
}

/// x10_compare.py text_equal: identical, or identical after replacing every number and the
/// numbers within `tol`.
fn text_equal(a: &str, b: &str, tol: f64) -> bool {
    if a == b {
        return true;
    }

    if tol == 0.0 {
        return false;
    }

    fn split(s: &str) -> (String, Vec<f64>) {
        let (mut text, mut numbers) = (String::new(), vec![]);
        let bytes = s.as_bytes();
        let mut i = 0;

        while i < bytes.len() {
            let start = i;
            let digit = |j: usize| j < bytes.len() && bytes[j].is_ascii_digit();

            if digit(i) || (bytes[i] == b'-' && digit(i + 1)) {
                i += 1;
                while digit(i) {
                    i += 1;
                }
                if i < bytes.len() && bytes[i] == b'.' && digit(i + 1) {
                    i += 1;
                    while digit(i) {
                        i += 1;
                    }
                }
                if i < bytes.len() && (bytes[i] == b'e' || bytes[i] == b'E') {
                    let mut j = i + 1;
                    if j < bytes.len() && (bytes[j] == b'-' || bytes[j] == b'+') {
                        j += 1;
                    }
                    if digit(j) {
                        i = j;
                        while digit(i) {
                            i += 1;
                        }
                    }
                }
                numbers.push(s[start..i].parse().unwrap_or(f64::NAN));
                text.push('#');
            } else {
                let c = s[i..].chars().next().unwrap();
                text.push(c);
                i += c.len_utf8();
            }
        }

        (text, numbers)
    }

    let ((ta, na), (tb, nb)) = (split(a), split(b));

    ta == tb
        && na
            .iter()
            .zip(&nb)
            .all(|(x, y)| (x - y).abs() / x.abs().max(1.0) <= tol)
}

fn nums(v: &Value) -> Vec<f64> {
    v.as_array()
        .map(|a| a.iter().filter_map(Value::as_f64).collect())
        .unwrap_or_default()
}

/// What the port prints in addition to the pinned page, per baseline: the warning and the error of the failed PNG loads (`log`)
/// and the events of the nodes that fail (`console`: width, height, duration and `loaded false` of the LoadSensor, which the
/// pinned page never sends because the node never leaves IN_PROGRESS). Each line is removed once, its last occurrence, before
/// the comparison; a line that is not there is a failure.
struct KnownDifferences {
    baseline: &'static str,
    reason: &'static str,
    log: &'static [&'static str],
    console: &'static [&'static str],
}

const KNOWN_DIFFERENCES: &[KnownDifferences] = &[KnownDifferences {
    baseline: "movie-load-errors-baseline",
    reason: "X_ITE does not await setAPNG: a static or non-PNG url hangs the node (MovieTexture.js:98); the port fails it (ADR 0001 row 14)",
    log: &[
        "error: Error: Not a PNG",
        "error: Error: Not an animated PNG",
        "error: Error: Not an animated PNG",
        "warn: Error loading movie 'http://oracle/synthetic-movie-load-errors/media/static.png': Not an animated PNG",
        "warn: Error loading movie 'http://oracle/synthetic-movie-load-errors/media/notpng.png': Not a PNG",
        "warn: Error loading movie 'http://oracle/synthetic-movie-load-errors/media/noframes.png': Not an animated PNG",
    ],
    console: &[
        "log: static width 0",
        "log: static height 0",
        "log: static duration -1",
        "log: static loaded false",
        "log: notpng width 0",
        "log: notpng height 0",
        "log: notpng duration -1",
        "log: notpng loaded false",
        "log: noframes width 0",
        "log: noframes height 0",
        "log: noframes duration -1",
        "log: noframes loaded false",
    ],
}];

/// `lines` without the last occurrence of each known extra line; `Err` names a line that is not there.
fn without_known(lines: &[Value], known: &[&str]) -> Result<Vec<Value>, String> {
    let mut rest = lines.to_vec();

    for k in known {
        let at = rest
            .iter()
            .rposition(|l| l.as_str() == Some(k))
            .ok_or_else(|| format!("the known difference line is gone: {k}"))?;

        rest.remove(at);
    }

    Ok(rest)
}

#[allow(clippy::too_many_arguments)]
fn compare_case(
    root: &Path,
    baseline: &str,
    name: &str,
    world: &str,
    script: &str,
    msaa: u32,
    text_tol: f64,
    gpu: bool,
    failures: &mut Vec<String>,
) {
    let dir = root.join(format!("oracle/{baseline}/{name}-msaa{msaa}"));
    let js: Value =
        serde_json::from_str(&std::fs::read_to_string(dir.join("js.json")).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!(
        "{baseline}-{}-{name}-msaa{msaa}",
        std::process::id()
    ));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let png_dir = out.join("rs");
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_x3d-drive"));
    // These baselines are the pinned X_ITE 16.5.0 timing, not the ISO 19775-1 default (docs/movie-texture-timing.md).
    cmd.current_dir(root)
        .env("X3D_MOVIE_TIMING", "pinned")
        .args([world, script, "--oracle", "oracle"])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(dir.join("js.json"));

    let images = gpu && dir.join("js").is_dir();

    if gpu && !images {
        failures.push(format!(
            "{name} MSAA {msaa}: no X_ITE frames in {}",
            dir.display()
        ));
        return;
    }

    if images {
        cmd.arg("--png-dir").arg(&png_dir);
    }

    if msaa > 0 {
        cmd.args(["--msaa", &msaa.to_string(), "--gl-samples"]);
    }

    let status = cmd.status().unwrap();
    let mut fail = |what: String| failures.push(format!("{name} MSAA {msaa}: {what}"));

    if !status.success() {
        return fail(format!("x3d-drive exited with {status}"));
    }

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();

    if js["ok"] != Value::Bool(true) || rs["ok"] != Value::Bool(true) {
        return fail(format!("ok js {} rs {}", js["ok"], rs["ok"]));
    }

    for (k, a) in js["vrml"].as_object().unwrap() {
        let equal = match (a.as_str(), rs["vrml"].get(k).and_then(Value::as_str)) {
            (Some(a), Some(b)) => text_equal(a, b, text_tol),
            _ => rs["vrml"].get(k) == Some(a),
        };

        if !equal {
            fail(format!("scene text differs at frame {k}"));
        }
    }

    let prints = |r: &Value| -> Vec<Value> {
        r["console"].as_array().map_or(vec![], |c| {
            c.iter()
                .filter(|l| l.as_str().is_some_and(|l| l.starts_with("log: ")))
                .cloned()
                .collect()
        })
    };
    let known = KNOWN_DIFFERENCES.iter().find(|k| k.baseline == baseline);
    let mut rs_log = rs["log"].as_array().cloned().unwrap_or_default();
    let mut rs_prints = prints(&rs);

    if let Some(known) = known {
        match without_known(&rs_log, known.log) {
            Ok(l) => rs_log = l,
            Err(e) => fail(format!("{e} ({})", known.reason)),
        }

        match without_known(&rs_prints, known.console) {
            Ok(p) => rs_prints = p,
            Err(e) => fail(format!("{e} ({})", known.reason)),
        }
    }

    if js["log"].as_array().cloned().unwrap_or_default() != rs_log {
        fail(format!("log differs: {} / {}", js["log"], rs["log"]));
    }

    if prints(&js) != rs_prints {
        fail(format!(
            "console differs: {:?} / {:?}",
            prints(&js),
            prints(&rs)
        ));
    }

    for (k, a) in js["poses"].as_object().unwrap() {
        let Some(b) = rs["poses"].get(k) else {
            fail(format!("pose at frame {k} missing"));
            continue;
        };
        let d = |key: &str| {
            nums(&a[key])
                .iter()
                .zip(nums(&b[key]))
                .map(|(p, q)| (p - q).abs())
                .fold(0.0, f64::max)
        };
        let (qa, qb) = (
            quat(&nums(&a["orientation"])),
            quat(&nums(&b["orientation"])),
        );
        let dot = qa.iter().zip(qb).map(|(p, q)| p * q).sum::<f64>().abs();
        let angle = 2.0 * dot.min(1.0).acos();
        let fov = (a["fieldOfViewScale"].as_f64().unwrap_or(1.0)
            - b["fieldOfViewScale"].as_f64().unwrap_or(1.0))
        .abs();
        let viewpoint = a["viewpoint"] == b["viewpoint"] || b["viewpoint"] == "<default>";

        if d("position") > 1e-4
            || d("centerOfRotation") > 1e-4
            || angle > 1e-4
            || fov > 1e-4
            || a["viewer"] != b["viewer"]
            || !viewpoint
        {
            fail(format!(
                "pose frame {k}: dpos {:.2e} dcenter {:.2e} dangle {angle:.2e} dfov {fov:.2e} viewer {}/{}",
                d("position"),
                d("centerOfRotation"),
                a["viewer"],
                b["viewer"]
            ));
        }
    }

    if images {
        for entry in std::fs::read_dir(dir.join("js")).unwrap() {
            let p = entry.unwrap().path();
            let file = p.file_name().unwrap().to_string_lossy().to_string();
            let (w, h, want) = x3d_render::compare::load_png(&p).unwrap();

            match x3d_render::compare::load_png(&png_dir.join(&file)) {
                Ok((rw, rh, got)) if (rw, rh) == (w, h) => {
                    let s = x3d_render::compare::compare(w, h, &want, &got, 8);

                    eprintln!(
                        "{name} MSAA {msaa} {file}: {} pixels over 8 ({:.3}%), max {}",
                        s.pixels_over, s.pct_over, s.max_delta
                    );

                    if s.pixels_over * 1000 > (w * h) as usize * 5 {
                        fail(format!(
                            "{file}: {} pixels over 8 ({:.3}%), max {}",
                            s.pixels_over, s.pct_over, s.max_delta
                        ));
                    }
                }
                Ok((rw, rh, _)) => fail(format!("{file}: size {rw}x{rh}, X_ITE {w}x{h}")),
                Err(e) => fail(format!("{file}: {e}")),
            }
        }
    }

    let _ = std::fs::remove_dir_all(&out);
}

fn compare_baseline(baseline: &str) {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../..")
        .canonicalize()
        .unwrap();
    let cases = std::fs::read_to_string(root.join(format!("oracle/{baseline}/cases.txt"))).unwrap();
    let only = std::env::var("REA_ONLY").ok();
    // One GPU at a time: the cases run in order in this test.
    let gpu = x3d_render::gpu::Gpu::new().is_ok();
    if !gpu {
        x3d_evidence::no_gpu("image comparisons skipped");
    }
    let mut failures = vec![];
    let mut checked = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let (name, world, script, msaa) = (f[0], f[1], f[2], f[3].parse::<u32>().unwrap());
        let text_tol = f.get(4).map_or(0.0, |t| t.parse::<f64>().unwrap());

        if only.as_deref().is_some_and(|o| !name.contains(o)) {
            continue;
        }

        compare_case(
            &root,
            baseline,
            name,
            world,
            script,
            msaa,
            text_tol,
            gpu,
            &mut failures,
        );
        checked += 1;
    }

    eprintln!("{checked} cases checked (images: {gpu})");
    assert!(
        failures.is_empty(),
        "{} failures:\n{}",
        failures.len(),
        failures.join("\n")
    );
}

#[test]
fn movie_anim_cases_match_x_ite() {
    compare_baseline("movie-anim-baseline");
}

#[test]
fn movie_load_errors_match_x_ite() {
    compare_baseline("movie-load-errors-baseline");
}

/// WebM pictures at fixed movie times (VP8 ramp, test pattern and parts, VP9), MSAA 0 and 4, against the pinned page
/// (oracle/movie-video-baseline, five identical recordings; oracle/tools/gen_movie_video.py). The recorder sets the
/// element's currentTime (`movieSeek`); `x3d-drive` calls `Runtime::movie_seek` at the same frame. Each node plays at
/// speed 0, so the picture is the one the seek chose. AV1 is not in this world (see `movie_video_av1_frames_match_x_ite`).
#[test]
fn movie_video_frames_match_x_ite() {
    compare_baseline("movie-video-baseline");
}

/// The same pictures through glTF: three files whose textures have only an EXT_texture_video source (external WebM files, data
/// URLs, buffer views of a GLB), MSAA 0 and 4, against the pinned page (oracle/gltf-video-baseline, five identical recordings;
/// oracle/tools/gen_gltf_video.py). The pinned parser makes a MovieTexture with `loop TRUE` of each texture, and a glTF file
/// cannot set `speed`, so the nodes play at speed 1 from the load; the recorder pauses each video element and sets its
/// currentTime (`movieSeek`), and `x3d-drive` does the same at the same frame.
#[test]
fn gltf_video_frames_match_x_ite() {
    compare_baseline("gltf-video-baseline");
}

/// The same seeks on AV1 streams (av1-testsrc, av1-parts, av1-svt, av1-grain, av1-10bit, av1-444), MSAA 0 and 4, against
/// the pinned page, which plays AV1 itself (oracle/movie-video-av1-baseline, five identical recordings; the `movie-video-av1`
/// world of oracle/tools/gen_movie_video.py). X_ITE_R decodes AV1 only with the `av1` feature (rav1d, off by default), so
/// the comparison runs with `--features av1` (CI passes `x3d-viewer/av1`) and is reported as ignored, never as passed, without it. The baseline
/// must be there in both cases: a missing key fails (`movie_video_av1_baseline_is_present`), and with the feature the
/// comparison fails on a missing key and on a world whose AV1 pictures are not drawn.
#[test]
#[cfg_attr(
    not(feature = "av1"),
    ignore = "needs --features av1 (rav1d is off by default)"
)]
fn movie_video_av1_frames_match_x_ite() {
    compare_baseline("movie-video-av1-baseline");
}

#[test]
fn movie_video_av1_baseline_is_present() {
    let root = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../..");
    let dir = root.join("oracle/movie-video-av1-baseline");
    let cases =
        std::fs::read_to_string(dir.join("cases.txt")).expect("cases.txt of the AV1 baseline");
    let mut seen = 0;

    for line in cases.lines().filter(|l| !l.trim().is_empty()) {
        let f: Vec<&str> = line.split_whitespace().collect();
        let case = dir.join(format!("{}-msaa{}", f[0], f[3]));

        assert!(
            case.join("js.json").is_file(),
            "missing {}",
            case.join("js.json").display()
        );

        let pngs = std::fs::read_dir(case.join("js")).expect("js dir").count();

        assert_eq!(pngs, 5, "{}: 5 pictures expected", case.display());
        seen += 1;
    }

    assert_eq!(seen, 2, "MSAA 0 and MSAA 4");
}
