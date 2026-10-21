//! VRML97 Sound / AudioClip (and BufferAudioSource, which shares AudioClip's loader) against pinned X_ITE 16.5.0
//! (34cdf9f97). Worlds: oracle/synthetic-rea-b05/{clip-*,sound-*,buffer-fa*-decode}.wrl, written by
//! `oracle/tools/gen_rea_audio_repair.py`; baselines: oracle/rea-batch05-baseline/NAME, written only by
//! `oracle/tools/rea_b05.py baseline NAME` from an X_ITE run (headless Chrome, OfflineAudioContext).
//!
//! Compared like `rea_b05.py check`: the scene text at every dumped frame, the sorted X_ITE log, the Script print
//! lines in order, the audio suspend frames, and every audio sample (tolerance 0). No GPU, no audio device.
//!
//! `REA_ONLY=<substring>` restricts the cases.

use serde_json::Value;
use std::path::{Path, PathBuf};
use std::process::Command;

const CASES: &[&str] = &[
    // URL fallback after a decode failure, failure states, LoadSensor.
    "clip-fallback-decode",
    "clip-fail",
    "clip-fail-all-decode",
    "clip-fail-missing",
    "clip-fallback-missing",
    "clip-empty-url",
    "clip-loadsensor-fallback",
    "clip-loadsensor-fail",
    "buffer-fallback-decode",
    "buffer-fail-decode",
    // Loading races: two url changes in one cascade, removal while a load is pending, LoadSensor order.
    "race-url-twice",
    "race-remove-pending",
    "race-loadsensor-url-twice",
    // A PROTO instance as Sound.source.
    "sound-proto-source",
    "sound-proto-fallback",
    "sound-invalid-source",
    "sound-source-change",
    // Playback and placement.
    "clip-play",
    "clip-loop-end",
    "clip-pitch",
    "clip-pause-resume",
    "sound-pan-left",
    "sound-pan-right",
    "sound-pan-center",
    "sound-nospatialize",
    "sound-intensity",
    "sound-direction",
    "sound-ellipsoid-asym",
    "sound-zero-axis",
    "sound-transform",
    "sound-two-nodes",
    "sound-children",
    "sound-lifecycle",
];

// Not compared: `race-url-next-frame` (BLOCKED in rea_b05.py: under CPU load X_ITE resolved both decodes in one
// frame, so the route carried only the last duration).
// Not compared: `clip-url-change`. Its url changes while the frames run, so the fetch of the second candidate
// is not gated, and X_ITE delivered the decoded clip one frame later under CPU load (946684800.24 instead of
// .224). The case is checked for its outcome in `vrml_audio_clip_url_change_falls_back`.

/// Measured differences that stay open (the same list as rea_b05.py KNOWN): the audio may differ from the given
/// sample on (the frame of the scene change, 768 samples a frame), everything else must match. Finding C
/// (`sound-lifecycle`, `sound-source-change`) is repaired: tests/audio_pull.rs in x3d-runtime.
const KNOWN: &[(&str, usize, &str)] = &[];

fn audio(result: &Value, dir: &Path) -> Vec<Vec<f32>> {
    let a = &result["audio"];
    let file = PathBuf::from(a["file"].as_str().unwrap());
    let file = if file.is_absolute() { file } else { dir.join(file) };
    let bytes = std::fs::read(file).unwrap();
    let n = a["length"].as_u64().unwrap() as usize;
    let samples: Vec<f32> = bytes.chunks_exact(4).map(|b| f32::from_le_bytes([b[0], b[1], b[2], b[3]])).collect();
    (0..a["channels"].as_u64().unwrap() as usize).map(|c| samples[c * n..(c + 1) * n].to_vec()).collect()
}

fn prints(result: &Value) -> Vec<String> {
    result["console"].as_array().map(|c| c.iter().filter_map(Value::as_str).filter(|l| l.starts_with("log: ")).map(str::to_string).collect()).unwrap_or_default()
}

/// Returns the differences of one case (empty: it matches), whether only the audio differs, and the first
/// different audio sample.
fn compare(root: &Path, name: &str) -> (Vec<String>, bool, Option<usize>) {
    let base = root.join(format!("oracle/rea-batch05-baseline/{name}"));
    let js: Value = serde_json::from_str(&std::fs::read_to_string(base.join("js.json")).unwrap()).unwrap();
    let out = std::env::temp_dir().join(format!("vrml-audio-oracle-{}-{name}", std::process::id()));
    std::fs::create_dir_all(&out).unwrap();
    let rs_json = out.join("rs.json");
    let status = Command::new(env!("CARGO_BIN_EXE_x3d-drive"))
        .current_dir(root)
        .arg(format!("oracle/synthetic-rea-b05/{name}.wrl"))
        .arg(format!("oracle/synthetic-rea-b05/scripts/{name}.json"))
        .args(["--oracle", "oracle"])
        .arg("--out")
        .arg(&rs_json)
        .arg("--warmup-from")
        .arg(base.join("js.json"))
        .status()
        .unwrap();

    if !status.success() {
        return (vec![format!("x3d-drive exited with {status}")], false, None);
    }

    let rs: Value = serde_json::from_str(&std::fs::read_to_string(&rs_json).unwrap()).unwrap();
    let mut bad = vec![];

    if js["vrml"] != rs["vrml"] {
        bad.push("scene text".to_string());
    }
    if js["log"] != rs["log"] {
        bad.push(format!("log: X_ITE {} Rust {}", js["log"], rs["log"]));
    }
    if prints(&js) != prints(&rs) {
        bad.push(format!("prints: X_ITE {:?} Rust {:?}", prints(&js), prints(&rs)));
    }
    if js["audioSuspend"] != rs["audioSuspend"] {
        bad.push("audio suspend frames".to_string());
    }

    let others = bad.is_empty();
    let mut first_audio: Option<usize> = None;
    let (a, b) = (audio(&js, &base), audio(&rs, &out));

    if a.len() != b.len() || a.iter().zip(&b).any(|(x, y)| x.len() != y.len()) {
        bad.push("audio shape".to_string());
    } else {
        for (c, (x, y)) in a.iter().zip(&b).enumerate() {
            let first = x.iter().zip(y).position(|(p, q)| p != q);

            if let Some(i) = first {
                first_audio = Some(first_audio.map_or(i, |f| f.min(i)));
                let max = x.iter().zip(y).map(|(p, q)| (p - q).abs()).fold(0.0f32, f32::max);
                bad.push(format!("audio channel {c}: first different sample {i}, max difference {max:.3e}"));
            }
        }
    }

    let _ = std::fs::remove_dir_all(&out);
    (bad, others, first_audio)
}

#[test]
fn vrml_audio_matches_x_ite() {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..").canonicalize().unwrap();
    let only = std::env::var("REA_ONLY").unwrap_or_default();
    let names: Vec<&str> = CASES.iter().copied().filter(|n| n.contains(&only)).collect();
    let results: Vec<(&str, Vec<String>, bool, Option<usize>)> = std::thread::scope(|s| {
        let handles: Vec<_> = names.chunks(names.len().div_ceil(8).max(1)).map(|chunk| s.spawn(|| chunk.iter().map(|&n| {
            let (bad, others, first) = compare(&root, n);
            (n, bad, others, first)
        }).collect::<Vec<_>>())).collect();
        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });
    let mut failures = vec![];

    for (name, bad, others, first) in results {
        match KNOWN.iter().find(|(k, ..)| *k == name) {
            Some((_, from, reason)) if others && first.is_none_or(|f| f >= *from) => {
                if !bad.is_empty() {
                    eprintln!("KNOWN {name}: {reason}: {}", bad.join("; "));
                }
            }
            _ if !bad.is_empty() => failures.push(format!("{name}: {}", bad.join("; "))),
            _ => {}
        }
    }

    assert!(failures.is_empty(), "{} of {} cases differ from X_ITE:\n{}", failures.len(), names.len(), failures.join("\n"));
}
