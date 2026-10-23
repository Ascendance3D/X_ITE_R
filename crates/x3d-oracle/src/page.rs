//! One Playwright-style page: a fresh browser context, the page session initialised with the commands
//! Playwright sends (focus emulation, viewport, font families, media features, request interception,
//! cache disabled), `context.route` aborting every non-local request, the request/finished/failed counting
//! the Node tools' `settle` relies on, console capture with Playwright's `ConsoleMessage.text ()` rules, and
//! `page.evaluate` / `page.goto` / `page.waitForFunction` with Playwright's error texts.

use crate::chrome::{CdpError, Chrome, Conn};
use crate::input::Cmd;
use crate::json::js_number;
use serde_json::{Value, json};
use std::collections::{HashMap, HashSet};
use std::path::PathBuf;
use std::sync::{Arc, Condvar, Mutex};
use std::time::{Duration, Instant};

#[derive(Default)]
struct Req {
    /// Network.requestWillBeSent seen: (url, has frame).
    sent: Option<(String, bool)>,
    /// Fetch.requestPaused seen: (interception id, session).
    paused: Option<(String, String)>,
    counted: bool,
}

#[derive(Default)]
pub struct State {
    context_id: Option<String>,
    page_session: Option<String>,
    page_target: Option<String>,
    workers: HashSet<String>,
    /// The route predicate: requests not starting with `base` are aborted.
    base: String,
    reqs: HashMap<String, Req>,
    pub in_flight: i64,
    pub console_enabled: bool,
    pub console: Vec<(String, String)>,
    lifecycle: Vec<(String, String, String)>,
    closed: Option<String>,
    pub page_errors: Vec<String>,
}

pub struct Page {
    pub chrome: Chrome,
    pub session: String,
    pub target: String,
    pub state: Arc<(Mutex<State>, Condvar)>,
    pub deadline: Instant,
}

fn remote_object_preview(o: &Value) -> String {
    let s = |v: &Value| v.as_str().map(str::to_string);
    let string_of = |v: &Value| -> String {
        match v {
            Value::String(x) => x.clone(),
            Value::Number(n) => js_number(n.as_f64().unwrap_or(f64::NAN)),
            Value::Bool(b) => b.to_string(),
            Value::Null => "null".into(),
            _ => v.to_string(),
        }
    };
    let ty = o.get("type").and_then(Value::as_str).unwrap_or("");
    let unserializable = |u: &str| -> String {
        // parseUnserializableValue + String (): "-0" prints "0"; bigint ("5n") is undefined.
        match u {
            "NaN" | "Infinity" | "-Infinity" => u.into(),
            "-0" => "0".into(),
            _ => "undefined".into(),
        }
    };
    if o.get("objectId").is_none() {
        // String (potentiallyUnserializableValue (remoteObject)).
        if let Some(u) = o.get("unserializableValue").and_then(Value::as_str) {
            return unserializable(u);
        }
        return match o.get("value") {
            Some(v) => string_of(v),
            None => "undefined".into(),
        };
    }
    // renderPreview (remoteObject) || `JSHandle@${subtype || type}`.
    let preview = if ty == "undefined" {
        Some("undefined".to_string())
    } else if let Some(v) = o.get("value") {
        Some(string_of(v))
    } else if let Some(u) = o.get("unserializableValue").and_then(Value::as_str) {
        Some(u.to_string())
    } else if o.get("description").and_then(Value::as_str) == Some("Object") && o.get("preview").is_some() {
        let props = o["preview"]["properties"].as_array().cloned().unwrap_or_default();
        let toks: Vec<String> = props
            .iter()
            .map(|p| format!("{}: {}", s(&p["name"]).unwrap_or_default(), p.get("value").and_then(Value::as_str).unwrap_or("undefined")))
            .collect();
        Some(format!("{{{}}}", toks.join(", ")))
    } else if o.get("subtype").and_then(Value::as_str) == Some("array") && o.get("preview").is_some() {
        let mut entries: Vec<(i64, String)> = o["preview"]["properties"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .iter()
            .filter_map(|p| {
                let idx: f64 = p["name"].as_str()?.parse().ok()?;
                (idx >= 0.0 && idx.fract() == 0.0).then(|| (idx as i64, p.get("value").and_then(Value::as_str).unwrap_or("undefined").to_string()))
            })
            .collect();
        entries.sort_by_key(|e| e.0);
        let mut last = -1;
        let mut toks = Vec::new();
        for (i, v) in entries {
            let empty = i - last - 1;
            if empty == 1 {
                toks.push("empty".to_string());
            } else if empty > 1 {
                toks.push(format!("empty x {empty}"));
            }
            toks.push(v);
            last = i;
        }
        Some(format!("[{}]", toks.join(", ")))
    } else {
        o.get("description").and_then(Value::as_str).map(str::to_string)
    };
    match preview {
        Some(p) if !p.is_empty() => p,
        _ => format!("JSHandle@{}", o.get("subtype").and_then(Value::as_str).filter(|x| !x.is_empty()).unwrap_or(ty)),
    }
}

fn first_line(s: &str) -> &str {
    s.split('\n').next().unwrap_or("")
}

fn on_event(st: &Arc<(Mutex<State>, Condvar)>, conn: &Conn, v: &Value) {
    let method = v["method"].as_str().unwrap_or("");
    let params = &v["params"];
    let session = v.get("sessionId").and_then(Value::as_str).map(str::to_string);
    let (lock, cv) = &**st;
    let mut s = lock.lock().unwrap();
    match method {
        "Target.attachedToTarget" => {
            let info = &params["targetInfo"];
            let sid = params["sessionId"].as_str().unwrap_or("").to_string();
            let ty = info["type"].as_str().unwrap_or("");
            let ctx = info["browserContextId"].as_str();
            if session.is_none() && ty == "page" && s.page_session.is_none() && ctx.is_some() && ctx == s.context_id.as_deref() {
                s.page_session = Some(sid);
                s.page_target = info["targetId"].as_str().map(str::to_string);
                cv.notify_all();
            } else if ty == "worker" && session.is_some() {
                // FrameSession._onAttachedToTarget for a worker; NetworkManager.addSession.
                let w = Some(sid.as_str());
                conn.fire(w, "Runtime.enable", None);
                conn.fire(w, "Network.enable", None);
                conn.fire(w, "Network.setCacheDisabled", Some(json!({ "cacheDisabled": true })));
                conn.fire(w, "Fetch.enable", Some(json!({ "handleAuthRequests": true, "patterns": [{ "urlPattern": "*", "requestStage": "Request" }] })));
                conn.fire(w, "Runtime.runIfWaitingForDebugger", None);
                conn.fire(w, "Target.setAutoAttach", Some(json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true })));
                s.workers.insert(sid);
            } else {
                if params["waitingForDebugger"].as_bool() == Some(true) {
                    conn.fire(Some(&sid), "Runtime.runIfWaitingForDebugger", None);
                }
                conn.fire(session.as_deref(), "Target.detachFromTarget", Some(json!({ "sessionId": sid })));
            }
        }
        "Target.detachedFromTarget" => {
            let sid = params["sessionId"].as_str().unwrap_or("");
            s.workers.remove(sid);
            if Some(sid) == s.page_session.as_deref() {
                s.closed = Some("Target page, context or browser has been closed".into());
                cv.notify_all();
                drop(s);
                conn.fail_all("Target page, context or browser has been closed");
            }
        }
        "Inspector.targetCrashed" => {
            s.closed = Some("Target crashed".into());
            cv.notify_all();
            drop(s);
            conn.fail_all("Target crashed");
        }
        "Page.lifecycleEvent" => {
            s.lifecycle.push((
                params["frameId"].as_str().unwrap_or("").into(),
                params["loaderId"].as_str().unwrap_or("").into(),
                params["name"].as_str().unwrap_or("").into(),
            ));
            cv.notify_all();
        }
        "Fetch.requestPaused" => {
            let sid = session.clone().unwrap_or_default();
            let id = params["requestId"].as_str().unwrap_or("").to_string();
            let url = params["request"]["url"].as_str().unwrap_or("");
            match params.get("networkId").and_then(Value::as_str) {
                None => conn.fire(Some(&sid), "Fetch.continueRequest", Some(json!({ "requestId": id }))),
                Some(_) if url.starts_with("data:") => {}
                Some(nid) => {
                    let nid = nid.to_string();
                    s.reqs.entry(nid.clone()).or_default().paused = Some((id, sid));
                    pair(&mut s, conn, &nid);
                }
            }
        }
        "Network.requestWillBeSent" => {
            let url = params["request"]["url"].as_str().unwrap_or("").to_string();
            if !url.starts_with("data:") && params.get("redirectResponse").is_none() {
                let nid = params["requestId"].as_str().unwrap_or("").to_string();
                let in_worker = session.as_deref().is_some_and(|x| s.workers.contains(x));
                let has_frame = in_worker || params.get("frameId").and_then(Value::as_str).is_some_and(|f| !f.is_empty());
                s.reqs.entry(nid.clone()).or_default().sent = Some((url, has_frame));
                pair(&mut s, conn, &nid);
            }
        }
        "Network.loadingFinished" | "Network.loadingFailed" => {
            let nid = params["requestId"].as_str().unwrap_or("");
            if let Some(r) = s.reqs.remove(nid)
                && r.counted {
                    s.in_flight -= 1;
                }
        }
        "Runtime.consoleAPICalled"
            if params["executionContextId"].as_i64() != Some(0) => {
                let text: Vec<String> = params["args"].as_array().map(|a| a.iter().map(remote_object_preview).collect()).unwrap_or_default();
                let ty = params["type"].as_str().unwrap_or("").to_string();
                s.console.push((ty, text.join(" ")));
            }
        "Log.entryAdded" => {
            let e = &params["entry"];
            if e["source"].as_str() != Some("worker") {
                s.console.push((e["level"].as_str().unwrap_or("").into(), e["text"].as_str().unwrap_or("").into()));
            }
        }
        "Runtime.exceptionThrown" => {
            let d = &params["exceptionDetails"];
            let text = d["exception"]["description"].as_str().or(d["text"].as_str()).unwrap_or("exception");
            s.page_errors.push(first_line(text).to_string());
        }
        _ => {}
    }
}

/// FrameManager.requestStarted / route handling once both halves of a request are known.
fn pair(s: &mut State, conn: &Conn, nid: &str) {
    let base = s.base.clone();
    let Some(r) = s.reqs.get_mut(nid) else { return };
    if r.sent.is_none() || r.paused.is_none() {
        return;
    }
    let (Some((url, has_frame)), Some((id, sid))) = (r.sent.clone(), r.paused.take()) else { return };
    let fire = |m: &str, p: Value| conn.fire(Some(&sid), m, Some(p));
    if !has_frame {
        fire("Fetch.continueRequest", json!({ "requestId": id }));
        return;
    }
    if url.ends_with("/favicon.ico") {
        fire("Fetch.failRequest", json!({ "requestId": id, "errorReason": "Aborted" }));
        return;
    }
    r.counted = true;
    s.in_flight += 1;
    if url.starts_with(&base) {
        fire("Fetch.continueRequest", json!({ "requestId": id }));
    } else {
        fire("Fetch.failRequest", json!({ "requestId": id, "errorReason": "Failed" }));
    }
}

fn api_error(api: &str, e: CdpError) -> String {
    match e {
        CdpError::Timeout(s) => s,
        CdpError::Closed(s) | CdpError::Protocol(s) => format!("{api}: {s}"),
    }
}

impl Page {
    /// `chromium.launch`, `browser.newContext ({ viewport, deviceScaleFactor: 1 })`, `context.route (non-local => abort)`,
    /// `context.newPage ()`.
    pub fn open(exe: &str, args: &[&str], width: u32, height: u32, base: &str, trace: Option<PathBuf>, deadline: Instant) -> Result<Page, String> {
        let state = Arc::new((Mutex::new(State { base: base.into(), ..Default::default() }), Condvar::new()));
        let st = state.clone();
        let chrome = Chrome::launch(exe, args, trace, Box::new(move |conn, v| on_event(&st, conn, v)))?;
        let conn = chrome.conn.clone();
        let call = |m: &str, p: Option<Value>| conn.call(None, m, p, deadline).map_err(|e| format!("browserType.launch: {e}"));
        call("Browser.getVersion", None)?;
        call("Target.setAutoAttach", Some(json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true })))?;
        let ctx = call("Target.createBrowserContext", Some(json!({ "disposeOnDetach": true })))?;
        state.0.lock().unwrap().context_id = ctx["browserContextId"].as_str().map(str::to_string);
        call("Target.createTarget", Some(json!({ "url": "about:blank", "browserContextId": ctx["browserContextId"] })))?;
        let (session, target) = {
            let (lock, cv) = &*state;
            let mut s = lock.lock().unwrap();
            while s.page_session.is_none() {
                let left = deadline.saturating_duration_since(Instant::now());
                if left.is_zero() || conn.is_closed() {
                    return Err("browserContext.newPage: Target page, context or browser has been closed".into());
                }
                s = cv.wait_timeout(s, left.min(Duration::from_millis(100))).unwrap().0;
            }
            (s.page_session.clone().unwrap(), s.page_target.clone().unwrap_or_default())
        };
        let sess = Some(session.as_str());
        let window = conn.call(sess, "Browser.getWindowForTarget", None, deadline).ok();
        // FrameSession._initialize: sent together, awaited together.
        let mut cmds: Vec<(&str, Option<Value>)> = vec![
            ("Page.enable", None),
            ("Page.getFrameTree", None),
            ("Log.enable", Some(json!({}))),
            ("Page.setLifecycleEventsEnabled", Some(json!({ "enabled": true }))),
            ("Runtime.enable", Some(json!({}))),
            ("Network.enable", None),
            ("Network.setCacheDisabled", Some(json!({ "cacheDisabled": true }))),
            ("Fetch.enable", Some(json!({ "handleAuthRequests": true, "patterns": [{ "urlPattern": "*", "requestStage": "Request" }] }))),
            ("Target.setAutoAttach", Some(json!({ "autoAttach": true, "waitForDebuggerOnStart": true, "flatten": true }))),
            ("Emulation.setFocusEmulationEnabled", Some(json!({ "enabled": true }))),
        ];
        if let Some(w) = window.as_ref().and_then(|w| w.get("windowId")) {
            cmds.push(("Browser.setWindowBounds", Some(json!({ "windowId": w, "bounds": { "width": width, "height": height } }))));
        }
        cmds.extend([
            (
                "Emulation.setDeviceMetricsOverride",
                Some(json!({ "mobile": false, "width": width, "height": height, "screenWidth": width, "screenHeight": height,
                             "deviceScaleFactor": 1, "screenOrientation": { "angle": 0, "type": "landscapePrimary" } })),
            ),
            (
                "Page.setFontFamilies",
                Some(json!({ "fontFamilies": { "standard": "Times New Roman", "fixed": "Monospace", "serif": "Times New Roman",
                             "sansSerif": "Arial", "cursive": "Comic Sans MS", "fantasy": "Impact" } })),
            ),
            (
                "Emulation.setEmulatedMedia",
                Some(json!({ "media": "", "features": [
                    { "name": "prefers-color-scheme", "value": "light" }, { "name": "prefers-reduced-motion", "value": "no-preference" },
                    { "name": "forced-colors", "value": "none" }, { "name": "prefers-contrast", "value": "no-preference" } ] })),
            ),
            ("Runtime.runIfWaitingForDebugger", None),
        ]);
        let mut rxs = Vec::new();
        for (m, p) in cmds {
            rxs.push((m, conn.send(sess, m, p).map_err(|e| format!("browserContext.newPage: {e}"))?));
        }
        for (m, rx) in rxs {
            match Conn::wait(rx, deadline, m) {
                Ok(_) => {}
                // Browser.setWindowBounds is _sendMayFail-like in headless; everything else must succeed.
                Err(CdpError::Protocol(_)) if m == "Browser.setWindowBounds" => {}
                Err(e) => return Err(format!("browserContext.newPage: {e}")),
            }
        }
        Ok(Page { chrome, session, target, state, deadline })
    }

    fn conn(&self) -> Conn {
        self.chrome.conn.clone()
    }

    pub fn set_console(&self, on: bool) {
        self.state.0.lock().unwrap().console_enabled = on;
    }

    pub fn in_flight(&self) -> i64 {
        self.state.0.lock().unwrap().in_flight
    }

    pub fn closed_reason(&self) -> Option<String> {
        self.state.0.lock().unwrap().closed.clone()
    }

    /// `page.addInitScript (fn, arg)`: source `(${fn})(${JSON.stringify (arg)})`.
    pub fn add_init_script(&self, source: &str) -> Result<(), String> {
        self.conn()
            .call(Some(&self.session), "Page.addScriptToEvaluateOnNewDocument", Some(json!({ "source": source })), self.deadline)
            .map(|_| ())
            .map_err(|e| api_error("page.addInitScript", e))
    }

    /// `page.goto (url)` (waitUntil "load", 30 s).
    pub fn goto(&self, url: &str) -> Result<(), String> {
        let conn = self.conn();
        let nav_deadline = (Instant::now() + Duration::from_secs(30)).min(self.deadline);
        let r = conn
            .call(Some(&self.session), "Page.navigate", Some(json!({ "url": url, "frameId": self.target, "referrerPolicy": "unsafeUrl" })), nav_deadline)
            .map_err(|e| api_error("page.goto", e))?;
        if let Some(err) = r.get("errorText").and_then(Value::as_str) {
            return Err(format!("page.goto: {err} at {url}"));
        }
        let loader = r["loaderId"].as_str().unwrap_or("").to_string();
        let (lock, cv) = &*self.state;
        let mut s = lock.lock().unwrap();
        loop {
            if s.lifecycle.iter().any(|(f, l, n)| *f == self.target && *l == loader && n == "load") {
                return Ok(());
            }
            if let Some(c) = &s.closed {
                return Err(format!("page.goto: {c}"));
            }
            let now = Instant::now();
            if now >= nav_deadline {
                return Err(if nav_deadline < self.deadline { "page.goto: Timeout 30000ms exceeded.".into() } else { "x3d-oracle: timeout waiting for page.goto".into() });
            }
            s = cv.wait_timeout(s, (nav_deadline - now).min(Duration::from_millis(100))).unwrap().0;
        }
    }

    /// `page.evaluate (func, arg)`. `func` is the JavaScript function source, `arg` a JSON text or None.
    /// Returns the JSON text of the result (`JSON.stringify`), None for undefined.
    pub fn evaluate(&self, func: &str, arg: Option<&str>) -> Result<Option<String>, String> {
        let expression = format!(
            "(() => {{ const __pwSerialize = v => {{ try {{ return JSON.stringify (v); }} catch {{ return undefined; }} }}; \
             const __pwResult = ({func}) ({}); \
             return __pwResult && typeof __pwResult === \"object\" && typeof __pwResult .then === \"function\" ? __pwResult .then (__pwSerialize) : __pwSerialize (__pwResult); }})()",
            arg.unwrap_or("")
        );
        let r = self
            .conn()
            .call(
                Some(&self.session),
                "Runtime.evaluate",
                Some(json!({ "expression": expression, "returnByValue": true, "awaitPromise": true, "userGesture": true })),
                self.deadline,
            )
            .map_err(|e| api_error("page.evaluate", e))?;
        if let Some(d) = r.get("exceptionDetails") {
            let text = d["exception"]["description"]
                .as_str()
                .map(str::to_string)
                .or_else(|| d["exception"].get("value").map(|v| v.as_str().map(str::to_string).unwrap_or_else(|| v.to_string())))
                .or_else(|| d["text"].as_str().map(str::to_string))
                .unwrap_or_else(|| "evaluation failed".into());
            return Err(format!("page.evaluate: {}", first_line(&text)));
        }
        Ok(r["result"].get("value").and_then(Value::as_str).map(str::to_string))
    }

    /// `page.waitForFunction (expr, null, { timeout })`, polling.
    pub fn wait_for_function(&self, func: &str, timeout_ms: u64) -> Result<(), String> {
        let until = (Instant::now() + Duration::from_millis(timeout_ms)).min(self.deadline);
        loop {
            let v = self.evaluate(func, None).map_err(|e| e.replacen("page.evaluate", "page.waitForFunction", 1))?;
            if matches!(v.as_deref(), Some(t) if !matches!(t, "false" | "0" | "\"\"" | "null")) {
                return Ok(());
            }
            if Instant::now() >= until {
                return Err(if until < self.deadline {
                    format!("page.waitForFunction: Timeout {timeout_ms}ms exceeded.")
                } else {
                    "x3d-oracle: timeout waiting for page.waitForFunction".into()
                });
            }
            std::thread::sleep(Duration::from_millis(10));
        }
    }

    /// Input commands, pipelined (sent back to back, then all replies awaited), as Playwright does for a click.
    pub fn input(&self, cmds: Vec<Cmd>, api: &str) -> Result<(), String> {
        let conn = self.conn();
        let mut rxs = Vec::new();
        for c in cmds {
            rxs.push(conn.send(Some(&self.session), c.method, Some(c.params)).map_err(|e| api_error(api, e))?);
        }
        for rx in rxs {
            Conn::wait(rx, self.deadline, api).map_err(|e| api_error(api, e))?;
        }
        Ok(())
    }

    /// Playwright's console lines since `from`: (type, text).
    pub fn console_since(&self, from: usize) -> Vec<(String, String)> {
        self.state.0.lock().unwrap().console[from..].to_vec()
    }

    /// `context.close (); browser.close ()`.
    pub fn close(&mut self) {
        let conn = self.conn();
        if !conn.is_closed() {
            let ctx = self.state.0.lock().unwrap().context_id.clone();
            if let Some(ctx) = ctx
                && let Ok(rx) = conn.send(None, "Target.disposeBrowserContext", Some(json!({ "browserContextId": ctx }))) {
                    let _ = Conn::wait(rx, Instant::now() + Duration::from_secs(5), "Target.disposeBrowserContext");
                }
        }
        self.chrome.close();
    }

    pub fn page_errors(&self) -> Vec<String> {
        self.state.0.lock().unwrap().page_errors.clone()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn console_previews() {
        assert_eq!(remote_object_preview(&json!({ "type": "string", "value": "a b" })), "a b");
        assert_eq!(remote_object_preview(&json!({ "type": "number", "value": 1.5 })), "1.5");
        assert_eq!(remote_object_preview(&json!({ "type": "number", "value": 3 })), "3");
        assert_eq!(remote_object_preview(&json!({ "type": "number", "unserializableValue": "-0" })), "0");
        assert_eq!(remote_object_preview(&json!({ "type": "undefined" })), "undefined");
        assert_eq!(remote_object_preview(&json!({ "type": "object", "subtype": "null", "value": null })), "null");
        assert_eq!(
            remote_object_preview(&json!({ "type": "object", "objectId": "1", "description": "Object",
                "preview": { "properties": [ { "name": "a", "value": "1" }, { "name": "b", "value": "x" } ] } })),
            "{a: 1, b: x}"
        );
        assert_eq!(
            remote_object_preview(&json!({ "type": "object", "subtype": "array", "objectId": "1", "description": "Array(4)",
                "preview": { "properties": [ { "name": "0", "value": "1" }, { "name": "3", "value": "4" } ] } })),
            "[1, empty x 2, 4]"
        );
        assert_eq!(remote_object_preview(&json!({ "type": "object", "subtype": "error", "objectId": "1", "description": "Error: x\n    at y" })), "Error: x\n    at y");
        assert_eq!(remote_object_preview(&json!({ "type": "function", "objectId": "1" })), "JSHandle@function");
    }
}
