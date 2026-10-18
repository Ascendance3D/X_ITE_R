//! The frame-stepped Inline scenario runner of inline_common, pointed at oracle/synthetic-rea-b08-gltf-inline (REA batch 08, glTF lane).

#![allow(dead_code)]

use serde_json::{Value, json};
use std::cell::RefCell;
use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::rc::Rc;
use x3d_runtime::{Fetcher, LocalFetcher, Runtime};
use x3d_script::compare::{ORACLE_PREFIX, encode_path};

pub const CASE_DIR: &str = "synthetic-rea-b08-gltf-inline";

pub fn oracle_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

pub fn case_dir() -> PathBuf {
    oracle_root().join(CASE_DIR)
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

pub fn scenario(name: &str) -> Value {
    let text = std::fs::read_to_string(case_dir().join(format!("{name}.json"))).unwrap_or_else(|e| panic!("{name}.json: {e}"));
    serde_json::from_str(&text).unwrap()
}

// --- the runner ---------------------------------------------------------------------------------

/// Files that a scenario `publish` action makes available at run time, over the files on disk
/// (the oracle driver serves the same overlay).
#[derive(Clone, Default)]
pub struct Overlay(Rc<RefCell<HashMap<String, Vec<u8>>>>);

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

/// A scenario that is running: the runtime and what the recorder needs to follow it.
pub struct Run {
    pub rt: Runtime,
    pub overlay: Overlay,
    pub t0: f64,
    pub dt: f64,
    seen: usize,
    last_vrml: Option<String>,
}

impl Run {
    /// Load the world of a scenario and settle it (step 0 is the state after this).
    pub fn start(world: &str, t0: f64, dt: f64) -> Result<Run, String> {
        x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);

        let root = oracle_root();
        let url = format!("{ORACLE_PREFIX}{}", encode_path(&format!("{CASE_DIR}/{world}")));
        let data = std::fs::read(case_dir().join(world)).map_err(|e| format!("{world}: {e}"))?;
        let scene = x3d_parse::parse_bytes(&data, &url).map_err(|e| format!("{e:?}"))?;
        let overlay = Overlay::default();
        let fetcher = Box::new(OverlayFetcher { inner: LocalFetcher { root, prefix: ORACLE_PREFIX.to_string() }, overlay: overlay.clone() });
        let mut rt = Runtime::new(scene, fetcher);

        rt.script_host = Some(Box::new(x3d_script::QuickJsHost::new()));
        rt.live = true;
        rt.now = t0;
        rt.record_console = true;
        rt.load_scene();
        rt.replace_world();
        rt.settle();

        // `browser .loadURL ()` resolves one frame after `replaceWorld` made the scene live: the
        // `_loadCount` event that checkLoadCount waits for is processed in that frame.
        rt.frame();
        rt.settle();

        Ok(Run { rt, overlay, t0, dt, seen: 0, last_vrml: None })
    }

    /// One step k (k >= 1): the actions of frame k at the previous frame's time, settle, one frame, settle.
    pub fn step(&mut self, k: u64, actions: &[Value]) -> Result<(), String> {
        let prev = self.t0 + (k - 1) as f64 * self.dt;
        let now = self.t0 + k as f64 * self.dt;

        for a in actions {
            self.rt.now = prev;
            apply(&mut self.rt, &self.overlay, a)?;
        }

        self.rt.settle();
        self.rt.now = now;
        self.rt.fire_timeouts(now);
        self.rt.frame();
        self.rt.settle();
        Ok(())
    }

    pub fn record(&mut self, k: u64) -> Value {
        record(&mut self.rt, k, &mut self.seen, &mut self.last_vrml)
    }
}

/// Run one scenario and return its trace: `{ case, ok, steps: [ { k, loadCount, console, inlines, vrml? } ] }`.
pub fn run_scenario(name: &str) -> Result<Value, String> {
    let sc = scenario(name);
    let t0 = sc["t0"].as_f64().unwrap_or(946684800.0);
    let dt = sc["dt"].as_f64().unwrap_or(0.1);
    let frames = sc["frames"].as_u64().unwrap_or(1);
    let actions = sc["actions"].as_array().cloned().unwrap_or_default();
    let world = sc["world"].as_str().ok_or("scenario has no world")?;

    let mut run = Run::start(world, t0, dt)?;
    let mut steps = vec![run.record(0)];

    for k in 1..=frames {
        let todo: Vec<Value> = actions.iter().filter(|a| a["frame"].as_u64() == Some(k)).cloned().collect();

        run.step(k, &todo)?;
        steps.push(run.record(k));
    }

    Ok(json!({ "case": name, "ok": true, "steps": steps, "cleanup": cleanup(&run.rt) }))
}

/// What of any load or scene is left behind (Rust only; X_ITE has no such counters).
pub fn cleanup(rt: &Runtime) -> Value {
    let d = rt.inline_diagnostics();

    json!({
        "unfinishedLoads": rt.has_unfinished_loads(),
        "pendingScenes": d.pending_scenes,
        "frameWaiters": d.frame_waiters,
        "inProgress": d.in_progress,
        "liveScenes": d.live_scenes,
        "orphanLiveScenes": d.orphan_live_scenes,
        "timers": d.timers,
        "tasks": rt.tasks.len() + rt.fetches.len(),
    })
}

/// The Inline `path` ("Mid/Leaf": the node Leaf in the scene that the Inline Mid loaded).
pub fn inline_at(rt: &Runtime, path: &str) -> Result<x3d_fields::NodeId, String> {
    let mut ctx = x3d_scene::ROOT;
    let mut node = None;

    for part in path.split('/') {
        let n = rt.scene.ctx(ctx).named_nodes.get(part).ok_or_else(|| format!("Named node '{part}' not found."))?;

        node = Some(n);

        if let Some(&c) = rt.inline_scenes.get(&n) {
            ctx = c;
        }
    }

    node.ok_or_else(|| "empty path".to_string())
}

fn strings(v: &Value) -> Vec<String> {
    v.as_array().map(|u| u.iter().filter_map(|s| s.as_str().map(String::from)).collect()).unwrap_or_default()
}

fn apply(rt: &mut Runtime, overlay: &Overlay, a: &Value) -> Result<(), String> {
    use x3d_fields::FieldValue;

    let kind = a["type"].as_str().unwrap_or("");

    if kind == "publish" {
        let from = a["from"].as_str().ok_or("publish needs from")?;
        let path = a["path"].as_str().ok_or("publish needs path")?;
        let data = std::fs::read(case_dir().join(from)).map_err(|e| format!("{from}: {e}"))?;

        overlay.0.borrow_mut().insert(format!("/{CASE_DIR}/{path}"), data);
        return Ok(());
    }

    let name = a["name"].as_str().ok_or("action needs an Inline name")?;
    let node = inline_at(rt, name)?;
    let field = |rt: &Runtime, f: &str| rt.get_field(node, f).ok_or_else(|| format!("no field {f}"));

    match kind {
        "url" => {
            let k = field(rt, "url")?;
            rt.set_value(k, FieldValue::MFString(strings(&a["url"])));
        }
        "load" => {
            let k = field(rt, "load")?;
            rt.set_value(k, FieldValue::SFBool(a["value"].as_bool().unwrap_or(true)));
        }
        "autoRefresh" => {
            let k = field(rt, "autoRefresh")?;
            rt.set_value(k, FieldValue::SFTime(a["value"].as_f64().unwrap_or(0.0)));
        }
        "autoRefreshTimeLimit" => {
            let k = field(rt, "autoRefreshTimeLimit")?;
            rt.set_value(k, FieldValue::SFTime(a["value"].as_f64().unwrap_or(0.0)));
        }
        "loadNow" => rt.inline_load_now(node),
        "unloadNow" => rt.inline_unload_now(node),
        "remove" => {
            let group = inline_at(rt, a["group"].as_str().ok_or("remove needs group")?)?;
            let k = field_of(rt, group, "children")?;
            let FieldValue::MFNode(list) = rt.value(k).clone() else {
                return Err("children is not an MFNode".into());
            };

            rt.set_value(k, FieldValue::MFNode(list.into_iter().filter(|c| *c != Some(node)).collect()));
        }
        other => return Err(format!("unknown action {other}")),
    }

    Ok(())
}

fn field_of(rt: &Runtime, node: x3d_fields::NodeId, f: &str) -> Result<x3d_runtime::FKey, String> {
    rt.get_field(node, f).ok_or_else(|| format!("no field {f}"))
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

/// The named Inlines of a scene and of the scenes they loaded, in X_ITE's order.
fn inlines(rt: &Runtime, ctx: usize, prefix: &str, out: &mut Vec<Value>) {
    for &n in rt.scene.ctx(ctx).named_nodes.values() {
        if rt.builtin_name(n) != Some("Inline") {
            continue;
        }

        let name = rt.scene.node(n).name.clone();
        let scene = rt.inline_scenes.get(&n).copied();
        let url = match rt.pre_value(n, "url") {
            Some(x3d_fields::FieldValue::MFString(v)) => v.clone(),
            _ => vec![],
        };

        out.push(json!({
            "path": format!("{prefix}{name}"),
            "loadState": state_name(rt.url_load_state(n)),
            "load": rt.get_bool(n, "load"),
            "url": url,
            "loaded": scene.is_some(),
            "roots": scene.map(|c| rt.scene.ctx(c).root_nodes.iter().map(|r| r.map(|n| rt.type_name(n))).collect::<Vec<_>>()).unwrap_or_default(),
            "children": scene.map(|c| rt.scene.ctx(c).root_nodes.len()).unwrap_or(0),
            "live": scene.map(|c| rt.ctx_live(c)),
        }));

        if let Some(c) = scene {
            inlines(rt, c, &format!("{prefix}{name}/"), out);
        }
    }
}

fn record(rt: &mut Runtime, k: u64, seen: &mut usize, last_vrml: &mut Option<String>) -> Value {
    let console: Vec<String> = rt.console[*seen..].iter().map(|l| l.split('\n').next().unwrap_or("").to_string()).collect();
    *seen = rt.console.len();

    let mut list = vec![];
    inlines(rt, x3d_scene::ROOT, "", &mut list);

    rt.publish_clone_counts();
    let vrml = rt.scene.to_vrml_string();

    let mut step = json!({
        "k": k,
        "loadCount": rt.root_load_count(),
        "console": console,
        "inlines": list,
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

// --- helpers for the safety tests ----------------------------------------------------------------

/// Run a world with the given actions (`[ { frame, type, name, ... } ]`) for `frames` steps and
/// return the runner and the recorded steps (step 0 first).
pub fn trace(world: &str, frames: u64, actions: &[Value]) -> (Run, Vec<Value>) {
    let mut run = Run::start(world, 946684800.0, 0.1).unwrap();
    let mut steps = vec![run.record(0)];

    for k in 1..=frames {
        let todo: Vec<Value> = actions.iter().filter(|a| a["frame"].as_u64() == Some(k)).cloned().collect();

        run.step(k, &todo).unwrap();
        steps.push(run.record(k));
    }

    (run, steps)
}

pub fn action(frame: u64, kind: &str, name: &str, extra: Value) -> Value {
    let mut a = json!({ "frame": frame, "type": kind, "name": name });

    if let Value::Object(m) = extra {
        for (k, v) in m {
            a[k] = v;
        }
    }

    a
}

/// All console lines of the steps `from..=to`.
pub fn lines(steps: &[Value], from: u64, to: u64) -> Vec<String> {
    steps
        .iter()
        .filter(|s| s["k"].as_u64().is_some_and(|k| k >= from && k <= to))
        .flat_map(|s| s["console"].as_array().unwrap().iter().map(|l| l.as_str().unwrap().to_string()))
        .collect()
}

/// The record of the Inline `path` in a step.
pub fn inline_of<'a>(step: &'a Value, path: &str) -> &'a Value {
    step["inlines"].as_array().unwrap().iter().find(|i| i["path"] == path).unwrap_or_else(|| panic!("no Inline {path} in step {}", step["k"]))
}
