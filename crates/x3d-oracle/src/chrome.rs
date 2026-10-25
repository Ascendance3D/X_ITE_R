//! Chrome over the DevTools protocol, the way Playwright launches it: `--remote-debugging-pipe` (commands on
//! fd 3, replies on fd 4, NUL-terminated JSON), Playwright's default switches, a fresh user data directory.
//!
//! The browser runs in its own process group. [`Chrome`]'s Drop (also on panic and on every error path)
//! asks the browser to close, then kills the whole group and removes the profile directory. On Linux the
//! browser also gets SIGKILL if this process dies (PR_SET_PDEATHSIG), and on every Unix it exits when the
//! pipe closes.

use serde_json::{Value, json};
use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::path::PathBuf;
use std::process::{Child, Command, Stdio};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::{Receiver, Sender, channel};
use std::sync::{Arc, Mutex};
use std::thread;
use std::time::{Duration, Instant};

/// playwright-core 1.62 `chromiumSwitches ()` + `_innerDefaultArgs` for headless launches.
pub const PLAYWRIGHT_SWITCHES: &[&str] = &[
    "--disable-field-trial-config",
    "--disable-background-networking",
    "--disable-background-timer-throttling",
    "--disable-backgrounding-occluded-windows",
    "--disable-back-forward-cache",
    "--disable-breakpad",
    "--disable-client-side-phishing-detection",
    "--disable-component-extensions-with-background-pages",
    "--disable-component-update",
    "--no-default-browser-check",
    "--disable-default-apps",
    "--disable-dev-shm-usage",
    "--disable-edgeupdater",
    "--disable-extensions",
    "--disable-features=AvoidUnnecessaryBeforeUnloadCheckSync,BoundaryEventDispatchTracksNodeRemoval,DestroyProfileOnBrowserClose,DialMediaRouteProvider,GlobalMediaControls,HttpsUpgrades,LensOverlay,MediaRouter,PaintHolding,ThirdPartyStoragePartitioning,BlockOriginHeaderModificationOnRedirect,Translate,AutoDeElevate,OptimizationHints,msForceBrowserSignIn,msEdgeUpdateLaunchServicesPreferredVersion",
    "--enable-features=CDPScreenshotNewSurface",
    "--allow-pre-commit-input",
    "--disable-hang-monitor",
    "--disable-ipc-flooding-protection",
    "--disable-popup-blocking",
    "--disable-prompt-on-repost",
    "--disable-renderer-backgrounding",
    "--disable-updater-scheduler",
    "--force-color-profile=srgb",
    "--metrics-recording-only",
    "--no-first-run",
    "--password-store=basic",
    "--use-mock-keychain",
    "--no-service-autorun",
    "--export-tagged-pdf",
    "--disable-search-engine-choice-screen",
    "--edge-skip-compat-layer-relaunch",
    "--disable-infobars",
    "--disable-search-engine-choice-screen",
    "--disable-sync",
    "--enable-unsafe-swiftshader",
    "--headless",
    "--hide-scrollbars",
    "--mute-audio",
    "--blink-settings=primaryHoverType=2,availableHoverTypes=2,primaryPointerType=4,availablePointerTypes=4",
    "--no-sandbox",
];

/// The `args` every oracle/x_ite-*.mjs passes to `chromium.launch`.
pub const ORACLE_ARGS: &[&str] = &[
    "--headless=new",
    "--no-sandbox",
    "--use-gl=angle",
    "--use-angle=gl",
    "--enable-gpu",
    "--ignore-gpu-blocklist",
];

#[derive(Debug, Clone)]
pub enum CdpError {
    /// The browser, context or page went away (pipe closed, target crashed or detached).
    Closed(String),
    /// A protocol error reply.
    Protocol(String),
    Timeout(String),
}

impl std::fmt::Display for CdpError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CdpError::Closed(s) | CdpError::Protocol(s) | CdpError::Timeout(s) => f.write_str(s),
        }
    }
}

type Pending = Arc<Mutex<HashMap<u64, Sender<Result<Value, CdpError>>>>>;
pub type EventHandler = Box<dyn FnMut(&Conn, &Value) + Send>;

/// The pipe connection. Cloneable handle; events are delivered on the reader thread.
#[derive(Clone)]
pub struct Conn {
    writer: Arc<Mutex<Box<dyn Write + Send>>>,
    next_id: Arc<AtomicU64>,
    pending: Pending,
    closed: Arc<AtomicBool>,
    trace: Option<Arc<Mutex<std::fs::File>>>,
}

impl Conn {
    fn trace(&self, dir: &str, text: &str) {
        if let Some(t) = &self.trace {
            let _ = writeln!(t.lock().unwrap(), "{dir} {text}");
        }
    }

    fn write(
        &self,
        id: u64,
        session: Option<&str>,
        method: &str,
        params: Option<Value>,
    ) -> Result<(), CdpError> {
        let mut msg = json!({ "id": id, "method": method });
        if let Some(p) = params {
            msg["params"] = p;
        }
        if let Some(s) = session {
            msg["sessionId"] = json!(s);
        }
        let text = msg.to_string();
        self.trace("SEND", &text);
        let mut w = self.writer.lock().unwrap();
        let r = w
            .write_all(text.as_bytes())
            .and_then(|_| w.write_all(&[0]))
            .and_then(|_| w.flush());
        r.map_err(|_| CdpError::Closed("Target page, context or browser has been closed".into()))
    }

    /// Sends a command and returns a receiver for its reply.
    pub fn send(
        &self,
        session: Option<&str>,
        method: &str,
        params: Option<Value>,
    ) -> Result<Receiver<Result<Value, CdpError>>, CdpError> {
        if self.closed.load(Ordering::SeqCst) {
            return Err(CdpError::Closed(
                "Target page, context or browser has been closed".into(),
            ));
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let (tx, rx) = channel();
        self.pending.lock().unwrap().insert(id, tx);
        if let Err(e) = self.write(id, session, method, params) {
            self.pending.lock().unwrap().remove(&id);
            return Err(e);
        }
        Ok(rx)
    }

    /// Sends a command whose reply is ignored (from event handlers, which must not block).
    pub fn fire(&self, session: Option<&str>, method: &str, params: Option<Value>) {
        if self.closed.load(Ordering::SeqCst) {
            return;
        }
        let id = self.next_id.fetch_add(1, Ordering::SeqCst);
        let _ = self.write(id, session, method, params);
    }

    pub fn wait(
        rx: Receiver<Result<Value, CdpError>>,
        deadline: Instant,
        what: &str,
    ) -> Result<Value, CdpError> {
        let left = deadline.saturating_duration_since(Instant::now());
        match rx.recv_timeout(left) {
            Ok(r) => r,
            Err(std::sync::mpsc::RecvTimeoutError::Timeout) => Err(CdpError::Timeout(format!(
                "x3d-oracle: timeout waiting for {what}"
            ))),
            Err(_) => Err(CdpError::Closed(
                "Target page, context or browser has been closed".into(),
            )),
        }
    }

    pub fn call(
        &self,
        session: Option<&str>,
        method: &str,
        params: Option<Value>,
        deadline: Instant,
    ) -> Result<Value, CdpError> {
        let rx = self.send(session, method, params)?;
        Conn::wait(rx, deadline, method)
    }

    pub fn is_closed(&self) -> bool {
        self.closed.load(Ordering::SeqCst)
    }

    /// Fails every outstanding call (target crashed / detached).
    pub fn fail_all(&self, why: &str) {
        for (_, tx) in self.pending.lock().unwrap().drain() {
            let _ = tx.send(Err(CdpError::Closed(why.into())));
        }
    }
}

/// The browser process, the command pipe (its fd 3) and the reply pipe (its fd 4).
type Spawned = (Child, Box<dyn Write + Send>, Box<dyn Read + Send>);

pub struct Chrome {
    child: Option<Child>,
    pub conn: Conn,
    profile: PathBuf,
    stderr_tail: Arc<Mutex<VecDeque<String>>>,
}

fn reader_loop(mut input: impl Read, conn: Conn, mut on_event: EventHandler) {
    let mut buf = Vec::new();
    // buf [..scanned] holds no NUL: a large reply (AUDIO_DONE, PIXELS) arriving in many reads is scanned once.
    let mut scanned = 0;
    let mut chunk = vec![0u8; 1 << 16];
    loop {
        let n = match input.read(&mut chunk) {
            Ok(0) | Err(_) => break,
            Ok(n) => n,
        };
        buf.extend_from_slice(&chunk[..n]);
        while let Some(rel) = buf[scanned..].iter().position(|&b| b == 0) {
            let end = scanned + rel;
            scanned = 0;
            let msg: Vec<u8> = buf.drain(..=end).collect();
            let text = String::from_utf8_lossy(&msg[..msg.len() - 1]);
            conn.trace("RECV", &text);
            let Ok(v) = serde_json::from_str::<Value>(&text) else {
                continue;
            };
            if let Some(id) = v.get("id").and_then(Value::as_u64) {
                if let Some(tx) = conn.pending.lock().unwrap().remove(&id) {
                    let r = match v.get("error") {
                        Some(e) => Err(CdpError::Protocol(
                            e.get("message")
                                .and_then(Value::as_str)
                                .unwrap_or("protocol error")
                                .to_string(),
                        )),
                        None => Ok(v.get("result").cloned().unwrap_or(Value::Null)),
                    };
                    let _ = tx.send(r);
                }
            } else if v.get("method").is_some() {
                on_event(&conn, &v);
            }
        }
        scanned = buf.len();
    }
    conn.closed.store(true, Ordering::SeqCst);
    conn.fail_all("Target page, context or browser has been closed");
}

impl Chrome {
    /// Launches `exe` with Playwright's switches followed by `args` (the Node tools' launch args).
    pub fn launch(
        exe: &str,
        args: &[&str],
        trace: Option<PathBuf>,
        on_event: EventHandler,
    ) -> Result<Chrome, String> {
        remove_stale_profiles();
        let profile = std::env::temp_dir().join(format!(
            "x3d-oracle-profile-{}-{}",
            std::process::id(),
            std::time::SystemTime::now()
                .duration_since(std::time::UNIX_EPOCH)
                .map(|d| d.as_nanos())
                .unwrap_or(0)
        ));
        std::fs::create_dir_all(&profile)
            .map_err(|e| format!("x3d-oracle: cannot create {}: {e}", profile.display()))?;
        let mut cmd = Command::new(exe);
        cmd.args(PLAYWRIGHT_SWITCHES)
            .args(args)
            .arg(format!("--user-data-dir={}", profile.display()))
            .arg("--remote-debugging-pipe")
            .arg("--no-startup-window")
            .stdin(Stdio::null())
            .stdout(Stdio::null())
            .stderr(Stdio::piped());
        let (child, writer, reader) = spawn_with_pipes(cmd).map_err(|e| {
            let _ = std::fs::remove_dir_all(&profile);
            format!("x3d-oracle: cannot launch {exe}: {e}")
        })?;
        let mut child = child;
        let stderr_tail = Arc::new(Mutex::new(VecDeque::new()));
        if let Some(err) = child.stderr.take() {
            let tail = stderr_tail.clone();
            thread::Builder::new()
                .name("chrome-stderr".into())
                .spawn(move || {
                    for line in BufReader::new(err).lines().map_while(Result::ok) {
                        let mut t = tail.lock().unwrap();
                        t.push_back(line);
                        if t.len() > 40 {
                            t.pop_front();
                        }
                    }
                })
                .map_err(|e| e.to_string())?;
        }
        let trace = match trace {
            Some(p) => Some(Arc::new(Mutex::new(
                std::fs::File::create(&p)
                    .map_err(|e| format!("x3d-oracle: {}: {e}", p.display()))?,
            ))),
            None => None,
        };
        let conn = Conn {
            writer: Arc::new(Mutex::new(writer)),
            next_id: Arc::new(AtomicU64::new(1)),
            pending: Arc::new(Mutex::new(HashMap::new())),
            closed: Arc::new(AtomicBool::new(false)),
            trace,
        };
        let c2 = conn.clone();
        thread::Builder::new()
            .name("cdp-reader".into())
            .spawn(move || reader_loop(reader, c2, on_event))
            .map_err(|e| e.to_string())?;
        Ok(Chrome {
            child: Some(child),
            conn,
            profile,
            stderr_tail,
        })
    }

    pub fn pid(&self) -> Option<u32> {
        self.child.as_ref().map(Child::id)
    }

    pub fn stderr_tail(&self) -> Vec<String> {
        self.stderr_tail.lock().unwrap().iter().cloned().collect()
    }

    /// `browser.close ()`: Browser.close, then wait for the process; the group is killed after a grace period.
    pub fn close(&mut self) {
        if !self.conn.is_closed()
            && let Ok(rx) = self.conn.send(None, "Browser.close", None)
        {
            let _ = Conn::wait(rx, Instant::now() + Duration::from_secs(5), "Browser.close");
        }
        if let Some(mut child) = self.child.take() {
            let until = Instant::now() + Duration::from_secs(5);
            loop {
                match child.try_wait() {
                    Ok(Some(_)) => break,
                    Ok(None) if Instant::now() < until => thread::sleep(Duration::from_millis(20)),
                    _ => {
                        kill_group(&mut child);
                        let _ = child.wait();
                        break;
                    }
                }
            }
            // Helpers (GPU, renderer, zygote) share the browser's process group.
            kill_group_id(child.id());
        }
        let _ = std::fs::remove_dir_all(&self.profile);
    }

    /// Immediate SIGKILL of the browser's process group (timeouts).
    pub fn kill(&mut self) {
        if let Some(child) = self.child.as_mut() {
            kill_group(child);
        }
    }
}

impl Drop for Chrome {
    fn drop(&mut self) {
        if let Some(mut child) = self.child.take() {
            kill_group(&mut child);
            let _ = child.wait();
        }
        let _ = std::fs::remove_dir_all(&self.profile);
    }
}

/// Profile directories left by a run that was killed (SIGKILL cannot run Drop): removed when their owner is gone.
fn remove_stale_profiles() {
    let Ok(dir) = std::fs::read_dir(std::env::temp_dir()) else {
        return;
    };
    for e in dir.flatten() {
        let name = e.file_name().to_string_lossy().into_owned();
        let Some(pid) = name
            .strip_prefix("x3d-oracle-profile-")
            .and_then(|r| r.split('-').next())
            .and_then(|p| p.parse::<u32>().ok())
        else {
            continue;
        };
        if pid != std::process::id() && !process_alive(pid) {
            let _ = std::fs::remove_dir_all(e.path());
        }
    }
}

#[cfg(unix)]
fn process_alive(pid: u32) -> bool {
    // SAFETY: signal 0 only checks existence.
    unsafe {
        libc::kill(pid as libc::pid_t, 0) == 0
            || std::io::Error::last_os_error().raw_os_error() == Some(libc::EPERM)
    }
}

#[cfg(not(unix))]
fn process_alive(_pid: u32) -> bool {
    true
}

#[cfg(unix)]
fn kill_group_id(pid: u32) {
    // SAFETY: plain syscall; the group id is the browser's pid (process_group (0)).
    unsafe {
        libc::kill(-(pid as libc::pid_t), libc::SIGKILL);
    }
}

#[cfg(not(unix))]
fn kill_group_id(_pid: u32) {}

fn kill_group(child: &mut Child) {
    kill_group_id(child.id());
    let _ = child.kill();
}

/// Hands the browser fd 3 (commands) and fd 4 (replies).
#[cfg(unix)]
fn spawn_with_pipes(mut cmd: Command) -> std::io::Result<Spawned> {
    use std::os::fd::AsRawFd;
    use std::os::unix::process::CommandExt;

    let (cmd_read, cmd_write) = std::io::pipe()?;
    let (resp_read, resp_write) = std::io::pipe()?;
    let (child_in, child_out) = (cmd_read.as_raw_fd(), resp_write.as_raw_fd());
    cmd.process_group(0);
    // SAFETY: only async-signal-safe calls (fcntl, dup2, close, prctl) between fork and exec.
    unsafe {
        cmd.pre_exec(move || {
            let a = libc::fcntl(child_in, libc::F_DUPFD, 10);
            let b = libc::fcntl(child_out, libc::F_DUPFD, 10);
            if a < 0 || b < 0 || libc::dup2(a, 3) < 0 || libc::dup2(b, 4) < 0 {
                return Err(std::io::Error::last_os_error());
            }
            libc::close(a);
            libc::close(b);
            #[cfg(target_os = "linux")]
            libc::prctl(libc::PR_SET_PDEATHSIG, libc::SIGKILL);
            Ok(())
        });
    }
    let child = cmd.spawn()?;
    drop(cmd_read);
    drop(resp_write);
    Ok((child, Box::new(cmd_write), Box::new(resp_read)))
}

#[cfg(not(unix))]
fn spawn_with_pipes(_cmd: Command) -> std::io::Result<Spawned> {
    Err(std::io::Error::other(
        "the --remote-debugging-pipe transport (fd 3 / fd 4) is implemented for Unix only; see crates/x3d-oracle/README.md",
    ))
}
