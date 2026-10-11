//! x3d-parse dump <file>                      print the scene like X_ITE's scene.toVRMLString ()
//! x3d-parse compare <baseline.json> <corpus> [--known <file>] [--out <dir>]
//!                                            compare every corpus file with the X0 oracle
//!
//! `--known` reads lines `<file>\t<phase>\t<cause>` that explain mismatches needing runtime
//! behavior. `--out` writes `<file>.rust` / `<file>.xite` for each mismatch.

use std::collections::HashMap;
use std::path::Path;
use std::process::ExitCode;
use x3d_parse::compare::{Outcome, compare_corpus};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();

    match args.get(1).map(String::as_str) {
        Some("dump") if args.len() >= 3 => dump(&args[2], args.iter().any(|a| a == "--warnings")),
        Some("compare") if args.len() >= 4 => compare(&args[2], &args[3], &args[4..]),
        _ => {
            eprintln!("usage: x3d-parse dump <file> [--warnings]\n       x3d-parse compare <baseline.json> <corpus-dir> [--known <file>] [--out <dir>]");
            ExitCode::from(2)
        }
    }
}

fn dump(path: &str, warnings: bool) -> ExitCode {
    match x3d_parse::load_file(Path::new(path)) {
        Ok(scene) => {
            if warnings {
                for w in &scene.warnings {
                    eprintln!("warn: {w}");
                }
            }

            print!("{}", scene.to_vrml_string());
            ExitCode::SUCCESS
        }
        Err(e) => {
            eprintln!("error: {e}");
            ExitCode::FAILURE
        }
    }
}

fn compare(baseline: &str, corpus: &str, rest: &[String]) -> ExitCode {
    let mut known: HashMap<String, (String, String)> = HashMap::new();
    let mut out_dir = None;
    let mut i = 0;

    while i < rest.len() {
        match rest[i].as_str() {
            "--known" if i + 1 < rest.len() => {
                if let Ok(text) = std::fs::read_to_string(&rest[i + 1]) {
                    for line in text.lines().filter(|l| !l.starts_with('#') && !l.trim().is_empty()) {
                        let parts: Vec<&str> = line.splitn(3, '\t').collect();
                        if parts.len() == 3 {
                            known.insert(parts[0].to_string(), (parts[1].to_string(), parts[2].to_string()));
                        }
                    }
                }
                i += 2;
            }
            "--out" if i + 1 < rest.len() => {
                out_dir = Some(rest[i + 1].clone());
                i += 2;
            }
            _ => i += 1,
        }
    }

    let results = match compare_corpus(Path::new(baseline), Path::new(corpus)) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let (mut matched, mut mismatched, mut expected_failures, mut unexplained) = (0, 0, 0, 0);

    for r in &results {
        match &r.outcome {
            Outcome::Match => {
                matched += 1;
                println!("MATCH      {}", r.file);
            }
            Outcome::ExpectedFailure(e) => {
                expected_failures += 1;
                println!("XFAIL-OK   {}  ({e})", r.file);
            }
            Outcome::Mismatch { line, expected, actual } => {
                mismatched += 1;

                match known.get(&r.file) {
                    Some((phase, cause)) => println!("MISMATCH   {}  [needs {phase}] {cause}", r.file),
                    None => {
                        unexplained += 1;
                        println!("MISMATCH   {}  [unexplained]", r.file);
                    }
                }

                println!("           line {line}\n             x_ite: {expected}\n             rust:  {actual}");

                if let Some(dir) = &out_dir {
                    let _ = std::fs::create_dir_all(dir);
                    let _ = std::fs::write(Path::new(dir).join(format!("{}.rust", r.file.replace('/', "__"))), r.rust_output.as_deref().unwrap_or(""));
                    let _ = std::fs::write(Path::new(dir).join(format!("{}.xite", r.file.replace('/', "__"))), r.expected_output.as_deref().unwrap_or(""));
                }
            }
            Outcome::RustRejected(e) => {
                mismatched += 1;
                unexplained += 1;
                println!("REJECTED   {}  (X_ITE parsed it) {e}", r.file);
            }
            Outcome::UnexpectedSuccess => {
                mismatched += 1;
                unexplained += 1;
                println!("UNEXPECTED {}  (X_ITE rejected it, Rust parsed it)", r.file);
            }
        }
    }

    println!(
        "\n{} files: {matched} exact match, {mismatched} mismatch ({unexplained} unexplained), {expected_failures} expected failure reproduced",
        results.len()
    );

    if unexplained == 0 { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}
