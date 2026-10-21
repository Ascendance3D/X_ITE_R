//! Run the full runtime on an oracle corpus and compare with an X_ITE baseline
//! (oracle/x_ite-dump.mjs output). Corpus files are read in place, never copied.

use std::path::{Path, PathBuf};
use x3d_runtime::{LocalFetcher, Runtime};

/// The oracle serves its own directory (oracle/) as `http://oracle/`.
pub const ORACLE_PREFIX: &str = "http://oracle/";

pub struct RunResult {
    pub vrml: Result<String, String>,
    pub log: Vec<String>,
}

/// The serialization the dump returns: scene.toVRMLString (), toXMLString () or toJSONString ().
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum DumpFormat {
    Vrml,
    Xml,
    Json,
}

/// Load one world like dump.html's `window .dump (url)`.
pub fn run_world(oracle_root: &Path, file: &Path, scripts: bool) -> RunResult {
    run_world_as(oracle_root, file, scripts, DumpFormat::Vrml)
}

/// [`run_world`], serialized as `format`.
pub fn run_world_as(oracle_root: &Path, file: &Path, scripts: bool, format: DumpFormat) -> RunResult {
    // The dump oracle runs in headless Chromium.
    x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);

    let rel = file.strip_prefix(oracle_root).unwrap_or(file);
    let rel = rel.to_string_lossy().replace('\\', "/");
    let url = format!("{ORACLE_PREFIX}{}", encode_path(&rel));
    let fetcher = Box::new(LocalFetcher { root: oracle_root.to_path_buf(), prefix: ORACLE_PREFIX.to_string() });
    let mut log = vec![];

    let data = match std::fs::read(file) {
        Ok(d) => d,
        Err(_) => {
            log.push(format!("error: Couldn't load URL '{url}'. Error: Not Found"));
            return RunResult { vrml: Err("Couldn't load X3D file.".into()), log };
        }
    };

    let scene = match x3d_parse::parse_bytes(&data, &url) {
        Ok(s) => s,
        Err(e) => {
            // FileLoader prints the parser's multi-line message (first line "Error: "), and
            // VRMLParser (DEVELOPMENT) prints the original error.
            log.push(format!("error: Couldn't load URL '{url}'. Error: "));

            match e {
                x3d_parse::LoadError::Parse(p) => log.push(format!("error: Error: {}", p.message)),
                other => log.push(format!("error: Error: {other}")),
            }

            log.sort();
            return RunResult { vrml: Err("Couldn't load X3D file.".into()), log };
        }
    };

    let mut rt = Runtime::new(scene, fetcher);

    if scripts {
        rt.script_host = Some(Box::new(crate::QuickJsHost::new()));
    }

    rt.load_world();
    rt.finish_font_loads();

    let vrml = match format {
        DumpFormat::Vrml => rt.print_raw(|s| s.to_vrml_string()),
        DumpFormat::Xml => rt.print_raw(|s| s.to_xml_string()),
        DumpFormat::Json => rt.print_raw(|s| s.to_json_string()),
    };
    let mut log = rt.log;
    log.sort();

    RunResult { vrml: Ok(vrml), log }
}

/// `new URL (path, base)` percent-encodes spaces and a few other characters in paths.
pub fn encode_path(p: &str) -> String {
    let mut out = String::new();

    for c in p.chars() {
        match c {
            ' ' => out.push_str("%20"),
            '"' => out.push_str("%22"),
            '<' => out.push_str("%3C"),
            '>' => out.push_str("%3E"),
            '`' => out.push_str("%60"),
            '#' => out.push_str("%23"),
            '?' => out.push_str("%3F"),
            '{' => out.push_str("%7B"),
            '}' => out.push_str("%7D"),
            c if (c as u32) < 0x20 || (c as u32) > 0x7e => {
                let mut b = [0u8; 4];
                for byte in c.encode_utf8(&mut b).bytes() {
                    out.push_str(&format!("%{byte:02X}"));
                }
            }
            c => out.push(c),
        }
    }

    out
}

#[derive(Debug, Clone, PartialEq)]
pub enum Outcome {
    Match,
    Mismatch { line: usize, expected: String, actual: String },
    RustRejected(String),
    ExpectedFailure(String),
    UnexpectedSuccess,
    /// The oracle timed out on this file; its answer is unreliable.
    OracleTimeout,
}

pub struct FileResult {
    pub file: String,
    pub outcome: Outcome,
    pub rust: Option<String>,
    pub expected: Option<String>,
    pub log_match: Option<bool>,
    pub rust_log: Vec<String>,
    pub expected_log: Vec<String>,
}

fn first_difference(expected: &str, actual: &str) -> (usize, String, String) {
    let e: Vec<&str> = expected.split('\n').collect();
    let a: Vec<&str> = actual.split('\n').collect();

    for i in 0..e.len().max(a.len()) {
        let (x, y) = (e.get(i).copied(), a.get(i).copied());

        if x != y {
            return (i + 1, x.unwrap_or("<EOF>").to_string(), y.unwrap_or("<EOF>").to_string());
        }
    }

    (0, String::new(), String::new())
}

pub fn compare_corpus(baseline: &Path, corpus: &Path, scripts: bool, threads: usize) -> Result<Vec<FileResult>, String> {
    let json: serde_json::Value =
        serde_json::from_str(&std::fs::read_to_string(baseline).map_err(|e| e.to_string())?).map_err(|e| e.to_string())?;
    let entries: Vec<serde_json::Value> = json["results"].as_array().ok_or("baseline has no results")?.clone();
    let corpus = std::fs::canonicalize(corpus).map_err(|e| e.to_string())?;
    let oracle_root: PathBuf = corpus.parent().ok_or("corpus has no parent")?.to_path_buf();
    let next = std::sync::atomic::AtomicUsize::new(0);

    let mut results: Vec<FileResult> = std::thread::scope(|s| {
        let handles: Vec<_> = (0..threads)
            .map(|_| {
                let (entries, next, corpus, oracle_root) = (&entries, &next, &corpus, &oracle_root);

                // QuickJS needs a big native stack for deep scripts.
                std::thread::Builder::new()
                    .stack_size(64 << 20)
                    .spawn_scoped(s, move || {
                        let mut out = vec![];

                        loop {
                            let i = next.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
                            let Some(entry) = entries.get(i) else { break };
                            out.push(compare_one(entry, corpus, oracle_root, scripts));
                        }

                        out
                    })
                    .unwrap()
            })
            .collect();

        handles.into_iter().flat_map(|h| h.join().unwrap()).collect()
    });

    results.sort_by(|a, b| a.file.cmp(&b.file));
    Ok(results)
}

fn compare_one(entry: &serde_json::Value, corpus: &Path, oracle_root: &Path, scripts: bool) -> FileResult {
    let file = entry["file"].as_str().unwrap_or("").to_string();
    let ok = entry["ok"].as_bool().unwrap_or(false);
    let error = entry["error"].as_str().unwrap_or("");
    let expected = entry["vrml"].as_str().map(String::from);
    let mut expected_log: Vec<String> = entry["log"].as_array().map(|a| a.iter().filter_map(|v| v.as_str().map(String::from)).collect()).unwrap_or_default();

    // dump.html returns `log .sort ()`, but a message logged between the sort and the
    // serialization of the result can leave the array unsorted (x1 331-couch.wrl). The order
    // carries no information, so compare as sorted multisets.
    expected_log.sort();

    let r = run_world(oracle_root, &corpus.join(&file), scripts);
    let log_match = Some(r.log == expected_log);

    let outcome = if error.starts_with("TIMEOUT") {
        Outcome::OracleTimeout
    } else {
        match (ok, &r.vrml) {
            (true, Ok(out)) => {
                let exp = expected.clone().unwrap_or_default();

                if *out == exp {
                    Outcome::Match
                } else {
                    let (line, expected, actual) = first_difference(&exp, out);
                    Outcome::Mismatch { line, expected, actual }
                }
            }
            (true, Err(e)) => Outcome::RustRejected(e.clone()),
            (false, Err(e)) => Outcome::ExpectedFailure(e.clone()),
            (false, Ok(_)) => Outcome::UnexpectedSuccess,
        }
    };

    FileResult { file, outcome, rust: r.vrml.ok(), expected, log_match, rust_log: r.log, expected_log }
}
