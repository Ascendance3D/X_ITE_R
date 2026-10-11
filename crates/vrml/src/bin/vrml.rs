//! `vrml check|fmt|info <file>...` — check, reformat and summarize VRML/X3D classic files.

use std::path::Path;
use std::process::ExitCode;

const USAGE: &str = "usage:
  vrml check <file>...   parse each file; print OK or the error; exit 1 if any fails
  vrml fmt <file>        print the scene re-written by X_ITE's writer (X3D classic encoding, #X3D V4.1 header)
  vrml info <file>...    print version, size and contents of each file";

fn main() -> ExitCode {
    let args: Vec<String> = std::env::args().skip(1).collect();

    let Some((cmd, files)) = args.split_first() else {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    };

    if files.is_empty() || !matches!(cmd.as_str(), "check" | "fmt" | "info") {
        eprintln!("{USAGE}");
        return ExitCode::from(2);
    }

    let mut failed = false;

    for file in files {
        let path = Path::new(file);
        let data = match std::fs::read(path) {
            Ok(d) => d,
            Err(e) => {
                eprintln!("{file}: {e}");
                failed = true;
                continue;
            }
        };

        let result = vrml::parse_bytes(&data, &format!("file://{}", path.display()));

        match (cmd.as_str(), result) {
            (_, Err(e)) => {
                println!("{file}: ERROR {e}");
                failed = true;
            }
            ("check", Ok(scene)) => {
                println!("{file}: OK ({} warnings)", scene.warnings.len());
                for w in &scene.warnings {
                    println!("  warning: {}", w.lines().next().unwrap_or(""));
                }
            }
            ("fmt", Ok(scene)) => print!("{}", scene.to_vrml_string()),
            (_, Ok(scene)) => {
                let s = vrml::summarize(&scene);
                println!("{file}");
                println!("  header      {:?}{}", vrml::detect(&data), if vrml::is_gzip(&data) { " (gzip)" } else { "" });
                println!("  bytes       {}", data.len());
                println!("  profile     {}", s.profile.as_deref().unwrap_or("-"));
                println!("  root nodes  {}", s.root_nodes);
                println!("  nodes       {}", s.nodes);
                println!("  PROTOs      {}  EXTERNPROTOs {}", s.protos, s.externprotos);
                println!("  ROUTEs      {}", s.routes);
                println!("  warnings    {}", s.warnings);
                let mut top: Vec<_> = s.node_types.iter().collect();
                top.sort_by(|a, b| b.1.cmp(a.1).then(a.0.cmp(b.0)));
                for (name, n) in top.iter().take(10) {
                    println!("    {n:>6}  {name}");
                }
            }
        }
    }

    if failed { ExitCode::FAILURE } else { ExitCode::SUCCESS }
}
