//! The frame-stepped EXTERNPROTO scenario runner shared by the R0.3-B tests (see `extern_reload.rs`).

#![allow(dead_code)]

use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use x3d_runtime::{Fetcher, LocalFetcher, Runtime};
use x3d_script::compare::{ORACLE_PREFIX, encode_path};

pub fn oracle_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

pub fn case_dir() -> PathBuf {
    oracle_root().join("synthetic-externproto-reload")
}

/// The scenarios, by name.
pub fn case_names() -> Vec<String> {
    let mut names: Vec<String> = std::fs::read_dir(case_dir())
        .unwrap()
        .filter_map(|e| e.ok()?.file_name().to_str()?.strip_suffix(".json").map(String::from))
        .collect();

    names.sort();
    names
}

// --- the runner ---------------------------------------------------------------------------------

/// Files that a scenario `publish` action makes available at run time, over the files on disk
/// (the oracle driver serves the same overlay).
#[derive(Clone, Default)]
struct Overlay(Rc<RefCell<HashMap<String, Vec<u8>>>>);

struct OverlayFetcher {
    inner: LocalFetcher,
    overlay: Overlay,
}

impl Fetcher for OverlayFetcher {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        if url.as_str().starts_with(&self.inner.prefix) {
            if let Some(data) = self.overlay.0.borrow().get(url.path()) {
                return Ok(data.clone());
            }
        }

        self.inner.fetch(url)
    }
}

/// Run one scenario and return its trace: `{ case, ok, steps: [ { k, loadCount, console, externs, instances, vrml? } ] }`.
pub fn run_scenario(name: &str) -> Result<Value, String> {
    x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);

    let root = oracle_root();
    let dir = case_dir();
    let text = std::fs::read_to_string(dir.join(format!("{name}.json"))).map_err(|e| format!("{name}.json: {e}"))?;
    let scenario: Value = serde_json::from_str(&text).map_err(|e| e.to_string())?;
    let t0 = scenario["t0"].as_f64().unwrap_or(946684800.0);
    let dt = scenario["dt"].as_f64().unwrap_or(0.1);
    let frames = scenario["frames"].as_u64().unwrap_or(1);
    let actions = scenario["actions"].as_array().cloned().unwrap_or_default();
    let world = scenario["world"].as_str().ok_or("scenario has no world")?;

    let url = format!("{ORACLE_PREFIX}{}", encode_path(&format!("synthetic-externproto-reload/{world}")));
    let data = std::fs::read(dir.join(world)).map_err(|e| format!("{world}: {e}"))?;
    let scene = x3d_parse::parse_bytes(&data, &url).map_err(|e| format!("{e:?}"))?;

    let overlay = Overlay::default();
    let fetcher = Box::new(OverlayFetcher { inner: LocalFetcher { root: root.clone(), prefix: ORACLE_PREFIX.to_string() }, overlay: overlay.clone() });
    let mut rt = Runtime::new(scene, fetcher);

    rt.script_host = Some(Box::new(x3d_script::QuickJsHost::new()));
    rt.live = true;
    rt.now = t0;
    rt.record_console = true;
    rt.load_scene();
    rt.replace_world();
    rt.settle();

    let mut steps = vec![];
    let mut seen = 0usize;
    let mut last_vrml: Option<String> = None;

    steps.push(record(&mut rt, 0, &mut seen, &mut last_vrml));

    for k in 1..=frames {
        let prev = t0 + (k - 1) as f64 * dt;
        let now = t0 + k as f64 * dt;

        for a in actions.iter().filter(|a| a["frame"].as_u64() == Some(k)) {
            rt.now = prev;
            apply(&mut rt, &overlay, a)?;
        }

        rt.settle();
        rt.now = now;
        rt.fire_timeouts(now);
        rt.frame();
        rt.settle();
        steps.push(record(&mut rt, k, &mut seen, &mut last_vrml));
    }

    // What of any load is left behind.
    let d = rt.extern_diagnostics();
    let cleanup = json!({
        "unfinishedLoads": rt.has_unfinished_loads(),
        "pendingScenes": d.pending_scenes,
        "loadEvents": d.load_events,
        "inProgress": d.in_progress,
        "pendingCache": d.pending_cache,
        "timers": d.timers,
        "disposedBodies": d.disposed_bodies,
        "tasks": rt.tasks.len() + rt.fetches.len(),
    });

    Ok(json!({ "case": name, "ok": true, "steps": steps, "cleanup": cleanup }))
}

fn apply(rt: &mut Runtime, overlay: &Overlay, a: &Value) -> Result<(), String> {
    let kind = a["type"].as_str().unwrap_or("");

    if kind == "publish" {
        let from = a["from"].as_str().ok_or("publish needs from")?;
        let path = a["path"].as_str().ok_or("publish needs path")?;
        let data = std::fs::read(case_dir().join(from)).map_err(|e| format!("{from}: {e}"))?;

        overlay.0.borrow_mut().insert(format!("/synthetic-externproto-reload/{path}"), data);
        return Ok(());
    }

    let name = a["name"].as_str().ok_or("action needs a declaration name")?;

    if kind == "remove" {
        return rt.extern_remove(x3d_scene::ROOT, name).map(|_| ()).ok_or_else(|| format!("no EXTERNPROTO '{name}'"));
    }

    let e = rt.extern_find(x3d_scene::ROOT, name).ok_or_else(|| format!("no EXTERNPROTO '{name}'"))?;

    match kind {
        "url" => rt.extern_set_url(e, a["url"].as_array().map(|u| u.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default()),
        "load" => rt.extern_set_load(e, a["value"].as_bool().unwrap_or(true)),
        "autoRefresh" => rt.extern_set_auto_refresh(e, a["value"].as_f64().unwrap_or(0.0)),
        "autoRefreshTimeLimit" => rt.extern_set_auto_refresh_time_limit(e, a["value"].as_f64().unwrap_or(0.0)),
        "loadNow" => rt.extern_load_now(e),
        other => return Err(format!("unknown action {other}")),
    }

    Ok(())
}

fn state_name(s: x3d_runtime::nodes::LoadState) -> &'static str {
    use x3d_runtime::nodes::LoadState::*;

    match s {
        NotStarted => "NOT_STARTED",
        InProgress => "IN_PROGRESS",
        Complete => "COMPLETE",
        Failed => "FAILED",
    }
}

fn record(rt: &mut Runtime, k: u64, seen: &mut usize, last_vrml: &mut Option<String>) -> Value {
    let console: Vec<String> = rt.console[*seen..].iter().map(|l| l.split('\n').next().unwrap_or("").to_string()).collect();
    *seen = rt.console.len();

    let root = x3d_scene::ROOT;
    let externs: Vec<Value> = rt
        .scene
        .ctx(root)
        .externprotos
        .values()
        .iter()
        .map(|&e| {
            let d = &rt.scene.externprotos[e];

            json!({
                "name": d.name,
                "loadState": state_name(rt.extern_load_state(e)),
                "url": d.url,
                "proto": rt.extern_proto_name(e),
                "fields": d.fields.iter().map(|f| f.name.clone()).collect::<Vec<_>>(),
            })
        })
        .collect();

    let mut instances = serde_json::Map::new();

    for &n in rt.scene.ctx(root).named_nodes.values() {
        if !matches!(rt.scene.node(n).kind, x3d_scene::NodeKind::Instance(_)) {
            continue;
        }

        instances.insert(rt.scene.node(n).name.clone(), json!({ "type": rt.type_name(n), "body": rt.instance_body_roots(n) }));
    }

    rt.publish_clone_counts();
    let vrml = rt.scene.to_vrml_string();

    let mut step = json!({
        "k": k,
        "loadCount": rt.root_load_count(),
        "console": console,
        "externs": externs,
        "instances": instances,
    });

    if last_vrml.as_deref() != Some(vrml.as_str()) {
        step["vrml"] = json!(vrml);
    }

    *last_vrml = Some(vrml);
    step
}

/// Run on a thread with a big stack (QuickJS).
pub fn big_stack<T: Send + 'static>(f: impl FnOnce() -> T + Send + 'static) -> T {
    std::thread::Builder::new().stack_size(64 << 20).spawn(f).unwrap().join().unwrap()
}
