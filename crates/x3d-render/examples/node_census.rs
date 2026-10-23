//! Count node types per corpus after a full runtime load (PROTO bodies realized, Inlines loaded).
//! cargo run --example node_census -- <oracle-dir> <corpus-dir>
use std::collections::BTreeMap;

fn main() {
    let a: Vec<String> = std::env::args().collect();
    let oracle = std::fs::canonicalize(&a[1]).unwrap();
    let corpus = std::fs::canonicalize(&a[2]).unwrap();
    let mut files = vec![];
    walk(&corpus, &mut files);
    files.sort();
    let mut uses: BTreeMap<String, (usize, usize)> = BTreeMap::new();

    for f in &files {
        let f2 = f.clone();
        let o2 = oracle.clone();
        let names = std::thread::Builder::new()
            .stack_size(64 << 20)
            .spawn(move || {
                let Ok(rt) = x3d_render::load::load_world(&o2, &f2, true) else {
                    return vec![];
                };
                (0..rt.scene.nodes.len() as u32)
                    .filter_map(|n| rt.builtin_name(n).map(String::from))
                    .collect::<Vec<_>>()
            })
            .unwrap()
            .join()
            .unwrap_or_default();
        let mut seen = std::collections::HashSet::new();
        for n in names {
            let e = uses.entry(n.clone()).or_default();
            e.0 += 1;
            if seen.insert(n) {
                e.1 += 1;
            }
        }
    }
    let mut v: Vec<_> = uses.into_iter().collect();
    v.sort_by(|a, b| b.1.1.cmp(&a.1.1));
    println!("{} files", files.len());
    for (n, (c, f)) in v {
        println!("{f:5} files {c:7} nodes  {n}");
    }
}

fn walk(d: &std::path::Path, out: &mut Vec<std::path::PathBuf>) {
    for e in std::fs::read_dir(d).unwrap().flatten() {
        let p = e.path();
        if p.is_dir() {
            walk(&p, out)
        } else if p
            .extension()
            .is_some_and(|x| x == "wrl" || x == "wrz" || x == "gz")
        {
            out.push(p)
        }
    }
}
