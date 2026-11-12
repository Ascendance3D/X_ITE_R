//! sub:render.transparency-sorting: the envelope reference of the five multisampled shots that pinned X_ITE 16.5.0 (34cdf9f97)
//! does not reproduce from run to run (owner decision 2026-11-11). These tests need no GPU: they check the stored X_ITE runs,
//! the envelope built from them, and that the comparison finds a difference (`support/envelope.rs`). The renderer itself is
//! measured against the envelopes in render_rea_i92_items and render_rea_oit_i92_items.
//!
//! The stored runs: `oracle/tools/rea_oit_msaa4_envelope.py record 8 [--i92]`, eight runs of each world, Chrome 156 on the
//! RTX 3060 Ti. `python3 -I -B oracle/tools/rea_oit_msaa4_envelope.py check [--i92]` rebuilds the envelope files twice and
//! compares them with the stored ones byte for byte.

#[path = "support/envelope.rs"]
mod envelope;

use envelope::{APPROVED, Envelope, Outside, THRESHOLD, VERSION_DIR, load_captures, load_envelope};
use sha2::{Digest, Sha256};
use std::path::{Path, PathBuf};

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap()
}

fn is_oit(baseline: &str) -> bool {
    baseline == "rea-oit-i92-render-items-baseline"
}

fn dir(baseline: &str) -> PathBuf {
    oracle().join(baseline).join(VERSION_DIR)
}

fn load(baseline: &str, world: &str) -> Envelope {
    load_envelope(&dir(baseline), baseline, world, is_oit(baseline), 4)
        .unwrap_or_else(|e| panic!("{baseline} {world}: {e}"))
}

fn sha256(data: &[u8]) -> String {
    Sha256::digest(data)
        .iter()
        .map(|b| format!("{b:02x}"))
        .collect()
}

/// A copy of the envelope directory of `baseline`, to change.
fn copy(baseline: &str, name: &str) -> PathBuf {
    fn all(from: &Path, to: &Path) {
        std::fs::create_dir_all(to).unwrap();

        for entry in std::fs::read_dir(from).unwrap() {
            let entry = entry.unwrap();
            let target = to.join(entry.file_name());

            if entry.file_type().unwrap().is_dir() {
                all(&entry.path(), &target);
            } else {
                std::fs::copy(entry.path(), target).unwrap();
            }
        }
    }

    let to = std::env::temp_dir().join(format!("rea-msaa4-envelope-{}-{name}", std::process::id()));
    let _ = std::fs::remove_dir_all(&to);
    all(&dir(baseline), &to);
    to
}

/// A pixel and channel of `e` with room for 9 levels above and below its interval: (x, y, channel, lowest, highest).
fn roomy(e: &Envelope) -> (u32, u32, usize, u8, u8) {
    for y in 0..e.height {
        for x in 0..e.width {
            for c in 0..3 {
                let (lo, hi) = e.interval(x, y, c);

                if lo >= 9 && hi <= 246 {
                    return (x, y, c, lo, hi);
                }
            }
        }
    }

    panic!("no channel with room");
}

/// The lowest picture of `e` (RGBA), with the channel `c` of the pixel `x`, `y` set to `value`.
fn with(e: &Envelope, x: u32, y: u32, c: usize, value: u8) -> Vec<u8> {
    let mut rgba = vec![];

    for py in 0..e.height {
        for px in 0..e.width {
            rgba.extend((0..3).map(|c| e.interval(px, py, c).0));
            rgba.push(255);
        }
    }

    rgba[((y * e.width + x) * 4) as usize + c] = value;
    rgba
}

#[test]
fn the_approved_shots_are_exactly_the_five_of_the_unstable_lists() {
    let mut listed = vec![];

    for baseline in [
        "rea-i92-render-items-baseline",
        "rea-oit-i92-render-items-baseline",
    ] {
        let worlds: Vec<String> = serde_json::from_str(
            &std::fs::read_to_string(oracle().join(baseline).join("msaa4/unstable.json")).unwrap(),
        )
        .unwrap();

        for world in worlds {
            listed.push((baseline.to_string(), world));
        }
    }

    listed.sort();

    let mut approved: Vec<(String, String)> = APPROVED
        .iter()
        .map(|(b, w)| (b.to_string(), w.to_string()))
        .collect();

    approved.sort();
    assert_eq!(approved.len(), 5);
    assert_eq!(listed, approved);
}

/// Requirement 1, and the measured variation of each shot.
#[test]
fn every_stored_x_ite_run_is_inside_its_own_envelope() {
    for (baseline, world) in APPROVED {
        let e = load(baseline, world);
        let (w, h, captures) =
            load_captures(&dir(baseline), baseline, world, is_oit(baseline)).unwrap();
        let (pixels, spread) = e.variation();

        assert_eq!(captures.len(), 8, "{baseline} {world}");
        assert_eq!((w, h), (512, 512), "{baseline} {world}");
        assert!(spread <= 1, "{baseline} {world}: spread {spread}");
        eprintln!(
            "{baseline} {world}: {} stored runs, {pixels} pixels vary, largest spread {spread}",
            captures.len()
        );

        for (i, capture) in captures.iter().enumerate() {
            assert_eq!(
                e.outside(w, h, capture).unwrap(),
                Outside {
                    pixels_outside: 0,
                    pixels_over: 0,
                    max_distance: 0
                },
                "{baseline} {world} run {}",
                i + 1
            );
        }
    }
}

/// Requirement 2. `load_envelope` has already compared the stored envelope pictures with the stored runs.
#[test]
fn the_same_stored_runs_give_the_same_envelope_and_the_same_file() {
    for (baseline, world) in APPROVED {
        let (w, h, captures) =
            load_captures(&dir(baseline), baseline, world, is_oit(baseline)).unwrap();
        let (a, b) = (
            Envelope::of(w, h, &captures).unwrap(),
            Envelope::of(w, h, &captures).unwrap(),
        );

        assert_eq!(a, b);
        assert_eq!(a, load(baseline, world));

        // The order of the runs does not matter.
        let reversed: Vec<Vec<u8>> = captures.iter().rev().cloned().collect();

        assert_eq!(a, Envelope::of(w, h, &reversed).unwrap());

        let file = |e: &Envelope| {
            let lowest = with(e, 0, 0, 0, e.interval(0, 0, 0).0);
            let mut png = std::io::Cursor::new(vec![]);

            image::RgbaImage::from_raw(e.width, e.height, lowest)
                .unwrap()
                .write_to(&mut png, image::ImageFormat::Png)
                .unwrap();
            sha256(&png.into_inner())
        };

        assert_eq!(file(&a), file(&b), "{baseline} {world}");
    }
}

/// Requirements 3 to 6, on every approved shot.
#[test]
fn a_value_outside_the_interval_is_counted_and_fails_over_the_threshold() {
    assert_eq!(THRESHOLD, 8);

    let clean = |outside: usize, over: usize, max: u8| Outside {
        pixels_outside: outside,
        pixels_over: over,
        max_distance: max,
    };

    for (baseline, world) in APPROVED {
        let e = load(baseline, world);
        let (x, y, c, lo, hi) = roomy(&e);
        let (w, h) = (e.width, e.height);
        let measure = |value: u8| e.outside(w, h, &with(&e, x, y, c, value)).unwrap();

        // Inside the interval, at both ends.
        assert_eq!(measure(lo), clean(0, 0, 0), "{baseline} {world}");
        assert_eq!(measure(hi), clean(0, 0, 0), "{baseline} {world}");
        // 1 level outside: within the threshold, but reported.
        assert_eq!(measure(hi + 1), clean(1, 0, 1), "{baseline} {world}");
        // 8 levels outside: still within the threshold, and reported.
        assert_eq!(measure(hi + 8), clean(1, 0, 8), "{baseline} {world}");
        assert_eq!(measure(lo - 8), clean(1, 0, 8), "{baseline} {world}");
        // 9 levels outside: a failure.
        assert_eq!(measure(hi + 9), clean(1, 1, 9), "{baseline} {world}");
        assert_eq!(measure(lo - 9), clean(1, 1, 9), "{baseline} {world}");

        // A changed channel fails in each of the three channels, and a far value has its full distance.
        for channel in 0..3 {
            let (_, top) = e.interval(x, y, channel);

            if top <= 200 {
                let out = e.outside(w, h, &with(&e, x, y, channel, 255)).unwrap();

                assert_eq!(
                    out,
                    clean(1, 1, 255 - top),
                    "{baseline} {world} channel {channel}"
                );
            }
        }

        // Alpha is not compared.
        let mut alpha = with(&e, x, y, c, lo);

        alpha[3] = 0;
        assert_eq!(e.outside(w, h, &alpha).unwrap(), clean(0, 0, 0));
    }
}

/// Requirement 7.
#[test]
fn a_picture_of_another_size_is_an_error() {
    for (baseline, world) in APPROVED {
        let e = load(baseline, world);
        let picture = with(&e, 0, 0, 0, 0);

        assert!(e.outside(e.width, e.height, &picture).is_ok());
        assert!(e.outside(e.width / 2, e.height * 2, &picture).is_err());
        assert!(e.outside(e.width, e.height - 1, &picture).is_err());
        assert!(
            e.outside(e.width, e.height, &picture[..picture.len() - 4])
                .is_err()
        );
        assert!(
            e.outside(256, 256, &vec![0; 256 * 256 * 4]).is_err(),
            "{baseline} {world}"
        );
    }
}

/// Requirement 8.
#[test]
fn a_missing_or_changed_stored_run_or_envelope_picture_is_an_error() {
    for (baseline, world) in APPROVED {
        let oit = is_oit(baseline);
        let name = format!("{}-{world}", if oit { "oit" } else { "off" });
        let copy = copy(baseline, &name);
        let load = || load_envelope(&copy, baseline, world, oit, 4);
        let raw = copy.join(format!("raw/run-3/{world}.png"));
        let original = std::fs::read(&raw).unwrap();

        assert!(load().is_ok(), "{baseline} {world}: the unchanged copy");

        // One changed byte of a stored run.
        let mut changed = original.clone();
        let last = changed.len() - 20;

        changed[last] ^= 1;
        std::fs::write(&raw, &changed).unwrap();
        assert!(
            load().unwrap_err().contains("SHA-256"),
            "{baseline} {world}"
        );

        // A stored run of another world in its place (a valid picture).
        let other = APPROVED
            .iter()
            .find(|(b, w)| b == baseline && w != world)
            .unwrap()
            .1;

        std::fs::copy(copy.join(format!("raw/run-3/{other}.png")), &raw).unwrap();
        assert!(
            load().unwrap_err().contains("SHA-256"),
            "{baseline} {world}"
        );

        // A missing stored run.
        std::fs::remove_file(&raw).unwrap();
        assert!(load().is_err(), "{baseline} {world}");
        std::fs::write(&raw, &original).unwrap();
        assert!(load().is_ok(), "{baseline} {world}: restored");

        // A stored envelope picture that is not the one of the stored runs.
        let max = copy.join(format!("{world}.max.png"));
        let min = std::fs::read(copy.join(format!("{world}.min.png"))).unwrap();
        let kept = std::fs::read(&max).unwrap();

        if min != kept {
            std::fs::write(&max, &min).unwrap();
            assert!(load().is_err(), "{baseline} {world}: max replaced by min");
            std::fs::write(&max, &kept).unwrap();
        }

        // A manifest that lists one run less.
        let manifest = copy.join("manifest.json");
        let mut json: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(&manifest).unwrap()).unwrap();

        json["worlds"][world]["captures"]
            .as_array_mut()
            .unwrap()
            .pop();
        std::fs::write(&manifest, json.to_string()).unwrap();
        assert!(load().is_err(), "{baseline} {world}: a run left out");

        std::fs::remove_dir_all(&copy).unwrap();
    }
}

/// Requirement 9.
#[test]
fn a_shot_that_is_not_approved_has_no_envelope() {
    for (baseline, world) in APPROVED {
        let oit = is_oit(baseline);
        let d = dir(baseline);

        // Another world of the same baseline, another Multisampling value, another baseline, the other option value.
        assert!(load_envelope(&d, baseline, "i92-tr-order.wrl", oit, 4).is_err());
        assert!(load_envelope(&d, baseline, world, oit, 0).is_err());
        assert!(load_envelope(&d, baseline, world, oit, 2).is_err());
        assert!(load_envelope(&d, baseline, world, oit, 8).is_err());
        assert!(load_envelope(&d, "rea-batch08-oit-baseline", world, oit, 4).is_err());
        assert!(load_envelope(&d, baseline, world, !oit, 4).is_err());
        assert!(!envelope::approved(baseline, world, 0));
        assert!(envelope::approved(baseline, world, 4));
    }

    // An approved world name in the directory of the other baseline.
    assert!(
        load_envelope(
            &dir("rea-oit-i92-render-items-baseline"),
            "rea-i92-render-items-baseline",
            "i92-tr-many.wrl",
            false,
            4
        )
        .is_err()
    );
}

/// Requirement 10: with Multisampling 0 each of the five worlds keeps its one recorded picture, the file that index.json
/// describes, and no shot with Multisampling 0 has an envelope.
#[test]
fn the_multisampling_0_references_are_the_recorded_pictures() {
    for (baseline, world) in APPROVED {
        let base = oracle().join(baseline);
        let index: serde_json::Value =
            serde_json::from_str(&std::fs::read_to_string(base.join("png/index.json")).unwrap())
                .unwrap();
        let entry = index["results"]
            .as_array()
            .unwrap()
            .iter()
            .find(|r| r["file"] == *world)
            .unwrap_or_else(|| panic!("{baseline} png: no {world}"));
        let png = std::fs::read(base.join(format!("png/{world}.png"))).unwrap();

        assert_eq!(entry["ok"], true, "{baseline} {world}");
        assert_eq!(
            entry["pngSha256"].as_str(),
            Some(sha256(&png).as_str()),
            "{baseline} {world}"
        );
        assert!(!envelope::approved(baseline, world, 0));
        assert!(!base.join("png-envelope-v1").exists());
        assert!(!base.join(format!("msaa4/{world}.png")).exists());
    }
}
