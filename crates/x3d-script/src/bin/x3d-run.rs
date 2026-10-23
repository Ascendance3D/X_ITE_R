//! x3d-run dump <file> [--oracle <dir>] [--no-scripts] [--format vrml|xml|json]
//!                                                         print scene.toVRMLString () (or XML / JSON) after load
//! x3d-run compare <baseline.json> <corpus-dir> [--known <tsv>] [--out <dir>] [--no-scripts] [--logs]
//!
//! The corpus directory must sit directly inside the oracle directory (served as http://oracle/).

use std::collections::HashMap;
use std::path::Path;
use std::process::ExitCode;
use x3d_script::compare::{DumpFormat, Outcome, compare_corpus, run_world_as};

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().collect();
    let flag = |f: &str| args.iter().any(|a| a == f);
    let opt = |f: &str| {
        args.iter()
            .position(|a| a == f)
            .and_then(|i| args.get(i + 1))
            .cloned()
    };
    let scripts = !flag("--no-scripts");

    match args.get(1).map(String::as_str) {
        Some("dump") if args.len() >= 3 => {
            let file = std::fs::canonicalize(&args[2]).expect("file");
            let oracle = opt("--oracle")
                .map(|o| std::fs::canonicalize(o).unwrap())
                .unwrap_or_else(|| file.parent().unwrap().parent().unwrap().to_path_buf());
            let format = match opt("--format").as_deref() {
                Some("xml") => DumpFormat::Xml,
                Some("json") => DumpFormat::Json,
                _ => DumpFormat::Vrml,
            };
            let r = std::thread::Builder::new()
                .stack_size(64 << 20)
                .spawn(move || run_world_as(&oracle, &file, scripts, format))
                .unwrap()
                .join()
                .unwrap();

            if flag("--log") {
                for l in &r.log {
                    eprintln!("{l}");
                }
            }

            match r.vrml {
                Ok(v) => {
                    print!("{v}");
                    ExitCode::SUCCESS
                }
                Err(e) => {
                    eprintln!("error: {e}");
                    ExitCode::FAILURE
                }
            }
        }
        Some("compare") if args.len() >= 4 => compare(
            &args[2],
            &args[3],
            opt("--known"),
            opt("--out"),
            scripts,
            flag("--logs"),
        ),
        _ => {
            eprintln!(
                "usage: x3d-run dump <file> [--oracle <dir>] [--no-scripts] [--log] [--format vrml|xml|json]\n       x3d-run compare <baseline.json> <corpus-dir> [--known <tsv>] [--out <dir>] [--no-scripts] [--logs]"
            );
            ExitCode::from(2)
        }
    }
}

fn compare(
    baseline: &str,
    corpus: &str,
    known: Option<String>,
    out_dir: Option<String>,
    scripts: bool,
    show_logs: bool,
) -> ExitCode {
    let mut known_map: HashMap<String, (String, String)> = HashMap::new();

    if let Some(k) = &known {
        if let Ok(text) = std::fs::read_to_string(k) {
            for line in text
                .lines()
                .filter(|l| !l.starts_with('#') && !l.trim().is_empty())
            {
                let parts: Vec<&str> = line.splitn(3, '\t').collect();
                if parts.len() == 3 {
                    known_map.insert(
                        parts[0].to_string(),
                        (parts[1].to_string(), parts[2].to_string()),
                    );
                }
            }
        }
    }

    let threads = std::thread::available_parallelism()
        .map(|n| n.get())
        .unwrap_or(8)
        .max(8);
    let results = match compare_corpus(Path::new(baseline), Path::new(corpus), scripts, threads) {
        Ok(r) => r,
        Err(e) => {
            eprintln!("error: {e}");
            return ExitCode::FAILURE;
        }
    };

    let (mut matched, mut mismatched, mut xfail, mut unexplained, mut timeouts) = (0, 0, 0, 0, 0);
    let (mut log_ok, mut log_total, mut log_unexplained) = (0, 0, 0);

    for r in &results {
        if !matches!(r.outcome, Outcome::OracleTimeout) {
            log_total += 1;
            if r.log_match == Some(true) {
                log_ok += 1;
            }
        }

        match &r.outcome {
            Outcome::Match => {
                matched += 1;
                println!("MATCH      {}", r.file);
            }
            Outcome::ExpectedFailure(e) => {
                xfail += 1;
                println!("XFAIL-OK   {}  ({e})", r.file);
            }
            Outcome::OracleTimeout => {
                timeouts += 1;
                println!("TIMEOUT    {}  (oracle timed out; not compared)", r.file);
            }
            Outcome::Mismatch {
                line,
                expected,
                actual,
            } => {
                mismatched += 1;

                match known_map.get(&r.file) {
                    Some((phase, cause)) => {
                        println!("MISMATCH   {}  [needs {phase}] {cause}", r.file)
                    }
                    None => {
                        unexplained += 1;
                        println!("MISMATCH   {}  [unexplained]", r.file);
                    }
                }

                println!(
                    "           line {line}\n             x_ite: {expected}\n             rust:  {actual}"
                );
            }
            Outcome::RustRejected(e) => {
                mismatched += 1;
                if !known_map.contains_key(&r.file) {
                    unexplained += 1;
                }
                println!("REJECTED   {}  (X_ITE parsed it) {e}", r.file);
            }
            Outcome::UnexpectedSuccess => {
                mismatched += 1;
                if !known_map.contains_key(&r.file) {
                    unexplained += 1;
                }
                println!("UNEXPECTED {}  (X_ITE rejected it, Rust parsed it)", r.file);
            }
        }

        if r.log_match == Some(false) && !matches!(r.outcome, Outcome::OracleTimeout) {
            // The known list explains log differences too (same file key).
            match known_map.get(&r.file) {
                Some((phase, cause)) => println!("LOGDIFF    {}  [needs {phase}] {cause}", r.file),
                None => {
                    log_unexplained += 1;
                    println!("LOGDIFF    {}  [unexplained]", r.file);
                }
            }
        }

        if show_logs && r.log_match == Some(false) {
            // Multiset difference of the two sorted logs.
            let (mut e, mut a) = (r.expected_log.clone(), r.rust_log.clone());
            let mut i = 0;

            while i < e.len() {
                if let Some(j) = a.iter().position(|x| *x == e[i]) {
                    a.remove(j);
                    e.remove(i);
                } else {
                    i += 1;
                }
            }

            for l in e {
                println!("           log -x_ite {l}");
            }
            for l in a {
                println!("           log +rust  {l}");
            }
        }

        if let Some(dir) = &out_dir {
            if !matches!(r.outcome, Outcome::Match | Outcome::ExpectedFailure(_)) {
                let name = r.file.replace('/', "__");
                let _ = std::fs::create_dir_all(dir);
                let _ = std::fs::write(
                    Path::new(dir).join(format!("{name}.rust")),
                    r.rust.as_deref().unwrap_or(""),
                );
                let _ = std::fs::write(
                    Path::new(dir).join(format!("{name}.xite")),
                    r.expected.as_deref().unwrap_or(""),
                );
            }
        }
    }

    println!(
        "\n{} files: {matched} exact match, {mismatched} mismatch ({unexplained} unexplained), {xfail} expected failure reproduced, {timeouts} oracle timeouts not compared\nlogs: {log_ok} of {log_total} sorted console logs identical ({log_unexplained} differences unexplained)",
        results.len()
    );

    if unexplained == 0 && log_unexplained == 0 {
        ExitCode::SUCCESS
    } else {
        ExitCode::FAILURE
    }
}
