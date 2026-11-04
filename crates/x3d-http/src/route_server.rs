//! A local HTTP server for tests (std only): it serves a table of routes on two ports of 127.0.0.1, so that a redirect can
//! cross origins. The table is the one that `oracle/x_ite-anim.mjs` serves for the pinned page (script key `httpRoutes`), so
//! both read the same file.
//!
//! A route is the path `/h/NAME`. Its JSON form (oracle/synthetic-rea-file-http/routes.json):
//! `{ "status": 200, "reason": "OK", "headers": { "Name": "value" | ["value", ...] }, "body": "text", "bodyHex": "0a0b",
//! "truncateTo": n, "hangup": true, "delayMs": n }`. `{origin}`, `{origin2}`, `{hostport}` and `{hostport2}` in a header value or in the body text
//! are replaced by the origins of the two ports and their host and port. `truncateTo` sends a `Content-Length` for the whole body and only the first
//! `n` bytes; `hangup` closes the connection without a response. The server closes each connection after its response.

use std::collections::BTreeMap;
use std::io::{Read, Write};
use std::net::{Shutdown, TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;

#[derive(Clone, Debug, Default)]
pub struct Route {
    pub status: u16,
    /// The reason phrase of the status line; the canonical phrase when `None`.
    pub reason: Option<String>,
    pub headers: Vec<(String, String)>,
    pub body: Vec<u8>,
    pub truncate_to: Option<usize>,
    pub hangup: bool,
    pub delay: Duration,
}

impl Route {
    pub fn new(status: u16, headers: &[(&str, &str)], body: &[u8]) -> Route {
        Route {
            status,
            headers: headers
                .iter()
                .map(|(n, v)| (n.to_string(), v.to_string()))
                .collect(),
            body: body.to_vec(),
            ..Route::default()
        }
    }

    fn from_json(value: &serde_json::Value) -> Result<Route, String> {
        let mut route = Route {
            status: value["status"].as_u64().unwrap_or(200) as u16,
            reason: value["reason"].as_str().map(str::to_string),
            ..Route::default()
        };

        if let Some(headers) = value["headers"].as_object() {
            for (name, v) in headers {
                let values = match v {
                    serde_json::Value::Array(a) => a.iter().filter_map(|x| x.as_str()).collect(),
                    other => vec![other.as_str().ok_or("a header value is a string")?],
                };

                for v in values {
                    route.headers.push((name.clone(), v.to_string()));
                }
            }
        }

        if let Some(text) = value["body"].as_str() {
            route.body = text.as_bytes().to_vec();
        }

        if let Some(hex) = value["bodyHex"].as_str() {
            route.body = (0..hex.len() / 2)
                .map(|i| u8::from_str_radix(&hex[2 * i..2 * i + 2], 16).map_err(|e| e.to_string()))
                .collect::<Result<_, _>>()?;
        }

        route.truncate_to = value["truncateTo"].as_u64().map(|n| n as usize);
        route.hangup = value["hangup"].as_bool().unwrap_or(false);
        route.delay = Duration::from_millis(value["delayMs"].as_u64().unwrap_or(0));
        Ok(route)
    }
}

/// The table of routes, by name (`/h/NAME`).
pub fn parse_routes(json: &str) -> Result<BTreeMap<String, Route>, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|e| e.to_string())?;

    value
        .as_object()
        .ok_or("the table is an object")?
        .iter()
        .map(|(name, v)| Ok((name.clone(), Route::from_json(v)?)))
        .collect()
}

pub struct RouteServer {
    /// `http://127.0.0.1:PORT` of the first server.
    pub origin: String,
    /// `http://127.0.0.1:PORT` of the second server (the other origin of a cross-origin redirect).
    pub origin2: String,
    log: Arc<Mutex<Vec<String>>>,
    stop: Arc<AtomicBool>,
    ports: [u16; 2],
}

impl RouteServer {
    pub fn start(routes: BTreeMap<String, Route>) -> RouteServer {
        let listeners = [(); 2].map(|_| TcpListener::bind("127.0.0.1:0").expect("bind"));
        let ports = listeners
            .each_ref()
            .map(|l| l.local_addr().expect("address").port());
        let origins = ports.map(|p| format!("http://127.0.0.1:{p}"));
        let log = Arc::new(Mutex::new(vec![]));
        let stop = Arc::new(AtomicBool::new(false));
        let routes = Arc::new(routes);

        for listener in listeners {
            let (log, stop, routes, origins) =
                (log.clone(), stop.clone(), routes.clone(), origins.clone());

            std::thread::spawn(move || {
                for stream in listener.incoming() {
                    if stop.load(Ordering::SeqCst) {
                        break;
                    }

                    let Ok(stream) = stream else { continue };
                    let (log, routes, origins) = (log.clone(), routes.clone(), origins.clone());

                    std::thread::spawn(move || serve(stream, &routes, &origins, &log));
                }
            });
        }

        RouteServer {
            origin: origins[0].clone(),
            origin2: origins[1].clone(),
            log,
            stop,
            ports,
        }
    }

    pub fn from_json(json: &str) -> Result<RouteServer, String> {
        Ok(RouteServer::start(parse_routes(json)?))
    }

    /// The requests so far, as `"GET /h/name"`, in order of arrival, for both ports.
    pub fn requests(&self) -> Vec<String> {
        self.log.lock().unwrap().clone()
    }
}

impl Drop for RouteServer {
    fn drop(&mut self) {
        self.stop.store(true, Ordering::SeqCst);

        // Wake each accept loop so that it sees the flag.
        for port in self.ports {
            let _ = TcpStream::connect(("127.0.0.1", port));
        }
    }
}

fn reason_of(status: u16) -> &'static str {
    match status {
        200 => "OK",
        201 => "Created",
        204 => "No Content",
        206 => "Partial Content",
        300 => "Multiple Choices",
        301 => "Moved Permanently",
        302 => "Found",
        303 => "See Other",
        304 => "Not Modified",
        307 => "Temporary Redirect",
        308 => "Permanent Redirect",
        400 => "Bad Request",
        403 => "Forbidden",
        404 => "Not Found",
        410 => "Gone",
        418 => "I'm a teapot",
        500 => "Internal Server Error",
        502 => "Bad Gateway",
        503 => "Service Unavailable",
        _ => "unknown",
    }
}

fn serve(
    mut stream: TcpStream,
    routes: &BTreeMap<String, Route>,
    origins: &[String; 2],
    log: &Mutex<Vec<String>>,
) {
    let _ = stream.set_read_timeout(Some(Duration::from_secs(10)));
    let mut head = vec![];
    let mut byte = [0u8; 1];

    while !head.ends_with(b"\r\n\r\n") {
        match stream.read(&mut byte) {
            Ok(1) => head.push(byte[0]),
            _ => return,
        }
    }

    let text = String::from_utf8_lossy(&head).into_owned();
    let first = text.lines().next().unwrap_or_default();
    let mut parts = first.split(' ');
    let (method, target) = (
        parts.next().unwrap_or_default(),
        parts.next().unwrap_or_default(),
    );
    let path = target.split(['?', '#']).next().unwrap_or_default();

    log.lock().unwrap().push(format!("{method} {path}"));

    let fill = |s: &str| {
        s.replace("{origin2}", &origins[1])
            .replace("{hostport2}", &origins[1]["http://".len()..])
            .replace("{origin}", &origins[0])
            .replace("{hostport}", &origins[0]["http://".len()..])
    };
    let route = path
        .strip_prefix("/h/")
        .and_then(|name| routes.get(name))
        .cloned()
        .unwrap_or_else(|| Route::new(404, &[], b""));

    if route.hangup {
        let _ = stream.shutdown(Shutdown::Both);
        return;
    }

    std::thread::sleep(route.delay);

    let body = match std::str::from_utf8(&route.body) {
        Ok(s) if s.contains('{') => fill(s).into_bytes(),
        _ => route.body.clone(),
    };
    let reason = route
        .reason
        .clone()
        .unwrap_or_else(|| reason_of(route.status).to_string());
    let mut out = format!("HTTP/1.1 {} {reason}\r\n", route.status).into_bytes();

    for (name, value) in &route.headers {
        out.extend(format!("{name}: {}\r\n", fill(value)).bytes());
    }

    if !route
        .headers
        .iter()
        .any(|(n, _)| n.eq_ignore_ascii_case("content-length"))
    {
        out.extend(format!("Content-Length: {}\r\n", body.len()).bytes());
    }

    out.extend(b"Connection: close\r\n\r\n");

    let sent = route.truncate_to.map_or(body.len(), |n| n.min(body.len()));

    out.extend(&body[..sent]);
    let _ = stream.write_all(&out);
    let _ = stream.flush();
    let _ = stream.shutdown(Shutdown::Both);
}
