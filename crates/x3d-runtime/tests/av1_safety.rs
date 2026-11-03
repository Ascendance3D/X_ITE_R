//! AV1 malformed-media safety (BR1.8), proved in child processes (feature `av1`).
//!
//! rav1d's C interface is `extern "C"`: a panic inside it cannot unwind and aborts the process, so `catch_unwind` in
//! the same process proves nothing. Each case here therefore decodes one damaged file in a CHILD process (this test
//! binary run again with `AV1_SAFETY_CASE=<seed>`); the parent asserts that the child exited normally (a decode
//! result or an error, status 0), never by SIGABRT, SIGSEGV or any other signal, and reports the panic locations of
//! the children that did not.
//!
//! `AV1_SAFETY_SEEDS=<n>` sets how many damaged files are tried (default 300).
#![cfg(all(feature = "av1", unix))]

use std::os::unix::process::ExitStatusExt;
use std::process::Command;

fn fixture(name: &str) -> Vec<u8> {
    std::fs::read(format!(
        "{}/../../oracle/synthetic-movie-codecs/media/{name}.webm",
        env!("CARGO_MANIFEST_DIR")
    ))
    .unwrap()
}

/// SplitMix64.
struct Rng(u64);

impl Rng {
    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9E37_79B9_7F4A_7C15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xBF58_476D_1CE4_E5B9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94D0_49BB_1331_11EB);
        z ^ (z >> 31)
    }
}

const NAMES: [&str; 6] = [
    "av1-testsrc",
    "av1-parts",
    "av1-svt",
    "av1-10bit",
    "av1-444",
    "av1-grain",
];

/// The damaged file of one seed: a stream, then a mix of byte flips (anywhere), runs of random bytes, a cut, and
/// a swap of two blocks of bytes.
fn damaged(seed: u64) -> Vec<u8> {
    let mut rng = Rng(seed ^ 0xA5A5);
    let mut d = fixture(NAMES[(seed % NAMES.len() as u64) as usize]);

    for _ in 0..1 + rng.next() % 6 {
        let at = (rng.next() % d.len() as u64) as usize;

        match rng.next() % 4 {
            0 => d[at] ^= 1 << (rng.next() % 8),
            1 => {
                for b in d.iter_mut().skip(at).take(1 + (rng.next() % 12) as usize) {
                    *b = rng.next() as u8;
                }
            }
            2 => d.truncate(at.max(1)),
            _ => {
                let to = (rng.next() % d.len() as u64) as usize;
                let len = ((rng.next() % 32) as usize)
                    .min(d.len() - at)
                    .min(d.len() - to);
                let piece = d[at..at + len].to_vec();

                d[to..to + len].copy_from_slice(&piece);
            }
        }
    }

    d
}

/// The child: decode, and exit 0 whatever the result.
#[test]
fn child() {
    let Ok(seed) = std::env::var("AV1_SAFETY_CASE") else {
        return;
    };
    let d = damaged(seed.parse().unwrap());
    let _ = x3d_runtime::webm::decode(&d, 512 << 20);
}

#[test]
fn damaged_av1_files_never_crash_the_process() {
    let seeds: u64 = std::env::var("AV1_SAFETY_SEEDS")
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(300);
    let exe = std::env::current_exe().unwrap();
    let mut failures: Vec<(u64, String, String)> = vec![];
    let threads = 8;
    let results = std::sync::Mutex::new(&mut failures);

    std::thread::scope(|s| {
        for t in 0..threads {
            let (exe, results) = (&exe, &results);

            s.spawn(move || {
                for seed in (t..seeds).step_by(threads as usize) {
                    let out = Command::new(exe)
                        .args(["--exact", "child", "--nocapture", "--test-threads=1"])
                        .env("AV1_SAFETY_CASE", seed.to_string())
                        .env("RUST_BACKTRACE", "0")
                        .output()
                        .unwrap();

                    // A child that ran no test at all would exit 0 too.
                    let ran = String::from_utf8_lossy(&out.stdout).contains("1 passed");

                    if !out.status.success() || !ran {
                        let err = String::from_utf8_lossy(&out.stderr);
                        let at = err
                            .lines()
                            .find(|l| l.contains("panicked at"))
                            .unwrap_or("(no panic line)")
                            .to_string();
                        let how = match out.status.signal() {
                            Some(sig) => format!("signal {sig}"),
                            None => format!("exit {:?}, test ran: {ran}", out.status.code()),
                        };

                        results.lock().unwrap().push((seed, how, at));
                    }
                }
            });
        }
    });

    failures.sort();

    assert!(
        failures.is_empty(),
        "{} of {seeds} damaged files crashed the decoder process:\n{}",
        failures.len(),
        failures
            .iter()
            .map(|(s, h, a)| format!("seed {s}: {h}: {a}"))
            .collect::<Vec<_>>()
            .join("\n")
    );
}
