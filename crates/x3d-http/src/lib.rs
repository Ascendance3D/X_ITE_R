//! HTTP and HTTPS [`Fetcher`] for the FileLoader of `x3d-runtime` (native builds; the browser build has the page's `fetch`).
//!
//! FileLoader.js calls `fetch (url, options)` and `checkResponse`, then reads `Content-Type` (the `text/html` and
//! `application/xhtml+xml` hand-off to `foreign`) and the body. The browser does the rest: it follows redirects, decodes
//! `Content-Encoding`, and refuses what the Fetch standard refuses. This crate does the same with the HTTP client `ureq`:
//!
//! * Redirects are followed here, one hop at a time, never by the client. A status of 301, 302, 303, 307 or 308 with a
//!   `Location` header is a redirect; the request stays a GET. The 21st redirect fails the request (`max_redirects`, 20 as in
//!   Chromium). A `Location` that does not parse, that has a scheme other than `http` or `https` (`file:`, `data:`, `ftp:`,
//!   `javascript:`, `blob:` ...), or that carries a user name or a password, fails the request: the Fetch standard says
//!   "network error", and the text is `TypeError: Failed to fetch`. A redirect never leaves HTTP and HTTPS.
//! * A status outside 200 to 299 fails with `Error: <statusText>` (`checkResponse`), where the text is the canonical reason
//!   phrase of the status. The reason phrase that a server sent is not kept by the client; a server that sends a different
//!   phrase is a known difference (oracle/README.md, "Fetcher response contract").
//! * `Content-Encoding: gzip` is decoded by the client, as a browser does. A body that is itself a gzip file is decoded
//!   later by the loader (`gunzip`), once.
//! * Limits that a browser does not have, because a native program must have them: the body size
//!   ([`HttpOptions::max_body_bytes`]), a connect timeout, a timeout of each read, and a timeout of the whole request with
//!   all its redirects. Each refusal has a text of its own and does not look like a network error.
//!
//! [`HttpFetcher::get`] gives the response of any status with its final URL, status, reason, headers and (for 2xx) body.
//! [`Fetcher::fetch_response`] is `get` followed by `checkResponse`.
//!
//! [`OriginMap`] sends the requests for one origin to another (`http://oracle` to a local test server). It is how the
//! oracle comparisons reach a real server while the scene keeps its URLs.

use std::io::Read;
use std::time::{Duration, Instant};
use x3d_runtime::{Fetcher, Response};

#[cfg(feature = "test-server")]
pub mod route_server;

/// The text of every failure that the Fetch standard calls a network error.
pub const NETWORK_ERROR: &str = "TypeError: Failed to fetch";

/// Limits and options of an [`HttpFetcher`].
#[derive(Clone, Debug)]
pub struct HttpOptions {
    /// Redirects followed before the request fails. Chromium follows 20; the 21st fails.
    pub max_redirects: usize,
    /// The largest body that is read, in bytes (decoded size). A larger body fails the request.
    pub max_body_bytes: u64,
    /// The time to connect to one host.
    pub connect_timeout: Duration,
    /// The time to wait for a read (response head or body data) before the request fails.
    pub read_timeout: Duration,
    /// The time of the whole request, with all its redirects and its body.
    pub total_timeout: Duration,
    /// Use `HTTP_PROXY`, `HTTPS_PROXY` and `NO_PROXY` of the environment.
    pub proxy_from_env: bool,
    /// The `User-Agent` header.
    pub user_agent: String,
}

impl Default for HttpOptions {
    fn default() -> HttpOptions {
        HttpOptions {
            max_redirects: 20,
            max_body_bytes: 512 * 1024 * 1024,
            connect_timeout: Duration::from_secs(30),
            read_timeout: Duration::from_secs(60),
            total_timeout: Duration::from_secs(300),
            proxy_from_env: true,
            user_agent: format!("x3d-http/{}", env!("CARGO_PKG_VERSION")),
        }
    }
}

/// Why a request produced no response.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum HttpError {
    /// The Fetch standard's network error: DNS, connect, TLS, protocol, a redirect that is refused, too many redirects,
    /// a scheme other than `http` and `https`, a URL with credentials. The text is the reason, for a log; it is
    /// [`NETWORK_ERROR`] that FileLoader shows.
    Network(String),
    /// [`HttpOptions::connect_timeout`], [`HttpOptions::read_timeout`] or [`HttpOptions::total_timeout`] ran out.
    Timeout,
    /// The connection failed while the body was read (cut short, or a content encoding that does not decode). The page's
    /// `blob ()` rejects with `TypeError: network error` for it, not with `Failed to fetch`.
    Body(String),
    /// The body is larger than [`HttpOptions::max_body_bytes`].
    BodyTooLarge(u64),
}

impl HttpError {
    /// The error text that FileLoader's diagnostic carries.
    pub fn message(&self) -> String {
        match self {
            HttpError::Network(_) => NETWORK_ERROR.to_string(),
            HttpError::Body(_) => "TypeError: network error".to_string(),
            HttpError::Timeout => "Error: The request timed out".to_string(),
            HttpError::BodyTooLarge(limit) => {
                format!("Error: The response body is larger than {limit} bytes")
            }
        }
    }
}

/// Requests for one origin that go to another origin: `from` and `to` are origins (`http://oracle`, `http://127.0.0.1:8080`).
/// The URL of the request and of the scene does not change; only the connection does. A redirect `Location` is resolved
/// against the URL that was really requested.
#[derive(Clone, Debug, Default)]
pub struct OriginMap(pub Vec<(url::Url, url::Url)>);

impl OriginMap {
    /// Parses `FROM=TO` (two origins).
    pub fn parse(pair: &str) -> Result<(url::Url, url::Url), String> {
        let (from, to) = pair
            .split_once('=')
            .ok_or_else(|| format!("{pair}: expected FROM=TO"))?;
        let parse = |s: &str| {
            let url = url::Url::parse(s).map_err(|e| format!("{s}: {e}"))?;

            if is_http(&url)
                && url.path() == "/"
                && url.query().is_none()
                && url.fragment().is_none()
            {
                Ok(url)
            } else if is_http(&url) {
                Err(format!("{s}: an origin has no path, query or fragment"))
            } else {
                Err(format!(
                    "{s}: an origin map maps http and https origins only"
                ))
            }
        };

        Ok((parse(from)?, parse(to)?))
    }

    fn rewrite(&self, url: &url::Url) -> url::Url {
        for (from, to) in &self.0 {
            if url.origin() == from.origin() {
                let mut mapped = url.clone();

                if mapped.set_scheme(to.scheme()).is_ok()
                    && mapped.set_host(to.host_str()).is_ok()
                    && mapped.set_port(to.port()).is_ok()
                {
                    return mapped;
                }
            }
        }

        url.clone()
    }
}

/// The HTTP and HTTPS fetcher. See the crate documentation.
pub struct HttpFetcher {
    agent: ureq::Agent,
    pub options: HttpOptions,
    pub origin_map: OriginMap,
}

fn is_http(url: &url::Url) -> bool {
    matches!(url.scheme(), "http" | "https") && url.host().is_some()
}

fn has_credentials(url: &url::Url) -> bool {
    !url.username().is_empty() || url.password().is_some()
}

/// The statuses of the Fetch standard's "redirect status".
fn is_redirect_status(status: u16) -> bool {
    matches!(status, 301 | 302 | 303 | 307 | 308)
}

impl HttpFetcher {
    pub fn new(options: HttpOptions) -> HttpFetcher {
        let mut builder = ureq::Agent::config_builder()
            .http_status_as_error(false)
            // The redirects are followed in `get`, where each hop is checked.
            .max_redirects(0)
            .timeout_connect(Some(options.connect_timeout))
            .timeout_per_read(Some(options.read_timeout))
            .user_agent(options.user_agent.clone());

        if !options.proxy_from_env {
            builder = builder.proxy(None);
        }

        HttpFetcher {
            agent: builder.build().into(),
            options,
            origin_map: OriginMap::default(),
        }
    }

    /// Panics when an origin of `map` is not `http` or `https` ([`OriginMap::parse`] gives the error instead): a request
    /// never leaves HTTP and HTTPS.
    pub fn with_origin_map(mut self, map: Vec<(url::Url, url::Url)>) -> HttpFetcher {
        assert!(
            map.iter().all(|(from, to)| is_http(from) && is_http(to)),
            "an origin map maps http and https origins only"
        );
        self.origin_map = OriginMap(map);
        self
    }

    /// `fetch (url)` for a GET request. The response has the status, the reason, the headers and the final URL of any
    /// status. The body is read (up to [`HttpOptions::max_body_bytes`]) only for a status of 200 to 299: the response of
    /// any other status (a redirect without `Location` included) has an empty body, because `checkResponse` rejects it
    /// before FileLoader reads a body.
    pub fn get(&self, url: &url::Url) -> Result<Response, HttpError> {
        self.get_unless(url, &[])
    }

    /// [`HttpFetcher::get`], except that the body of a 2xx response whose mime type (the `Content-Type` before `;`) is in
    /// `unread` is not read: the `foreign` hand-off of FileLoader.js decides from the header alone.
    pub fn get_unless(&self, url: &url::Url, unread: &[&str]) -> Result<Response, HttpError> {
        let network = |why: &str| HttpError::Network(why.to_string());

        if !is_http(url) {
            return Err(network("the scheme is not http or https"));
        }

        if has_credentials(url) {
            return Err(network("the URL includes credentials"));
        }

        let start = Instant::now();
        let mut current = self.origin_map.rewrite(url);
        let mut redirects = 0;

        loop {
            let remaining = self
                .options
                .total_timeout
                .checked_sub(start.elapsed())
                .filter(|d| !d.is_zero())
                .ok_or(HttpError::Timeout)?;
            let response = self
                .agent
                .get(current.as_str())
                .config()
                .timeout_global(Some(remaining))
                .build()
                .call()
                .map_err(map_error)?;
            let status = response.status().as_u16();
            let location = response
                .headers()
                .get("location")
                .map(|v| decode_header(v.as_bytes()).trim().to_string())
                // An empty `Location` is no redirect: the page shows `Error: Found` for a 302 with `Location:`.
                .filter(|v| !v.is_empty());

            if let (true, Some(location)) = (is_redirect_status(status), location) {
                if redirects >= self.options.max_redirects {
                    return Err(network("too many redirects"));
                }

                let next = current
                    .join(&location)
                    .map_err(|_| network("the Location does not parse"))?;

                if !is_http(&next) {
                    return Err(network("a redirect to a scheme other than http and https"));
                }

                if has_credentials(&next) {
                    return Err(network("a redirect to a URL with credentials"));
                }

                redirects += 1;
                current = self.origin_map.rewrite(&next);
                continue;
            }

            let headers: Vec<(String, String)> = response
                .headers()
                .iter()
                .map(|(n, v)| (n.as_str().to_string(), decode_header(v.as_bytes())))
                .collect();
            let ok = response.status().is_success();
            let status_text = response
                .status()
                .canonical_reason()
                .unwrap_or_default()
                .to_string();
            let mut body = vec![];
            let skip_body = response
                .headers()
                .get("content-type")
                .map(|v| decode_header(v.as_bytes()))
                .is_some_and(|v| unread.contains(&v.split(';').next().unwrap_or_default()));

            if ok && !skip_body {
                let max = self.options.max_body_bytes;
                let declared = response
                    .headers()
                    .get("content-length")
                    .and_then(|v| v.to_str().ok())
                    .and_then(|v| v.trim().parse::<u64>().ok());

                let encoded = response.headers().contains_key("content-encoding");

                // `Content-Length` is the decoded size only without a content encoding (a gzip body is counted on the wire).
                if !encoded && declared.is_some_and(|n| n > max) {
                    return Err(HttpError::BodyTooLarge(max));
                }

                // The decoded body is limited: a small gzip body can be very large when it is decoded.
                let mut reader = response
                    .into_body()
                    .into_with_config()
                    .limit(u64::MAX)
                    .reader();
                let mut buffer = vec![0u8; 64 * 1024];

                loop {
                    let n = reader.read(&mut buffer).map_err(map_read_error)?;

                    if n == 0 {
                        break;
                    }

                    body.extend_from_slice(&buffer[..n]);

                    // The limit is the decoded size (the client's own limit, 10 MB by default, is turned off above).
                    if body.len() as u64 > max {
                        return Err(HttpError::BodyTooLarge(max));
                    }
                }
            }

            // `response .url` is the URL that was requested last, as the page sees it, not the mapped one.
            let final_url = if redirects == 0 {
                url.clone()
            } else {
                reverse_map(&self.origin_map, &current)
            };

            return Ok(Response {
                url: final_url,
                status,
                status_text,
                headers,
                redirected: redirects > 0,
                body,
            });
        }
    }
}

/// The bytes of a header value are Latin-1 for a page (`Headers`).
fn decode_header(bytes: &[u8]) -> String {
    bytes.iter().map(|&b| b as char).collect()
}

fn reverse_map(map: &OriginMap, url: &url::Url) -> url::Url {
    for (from, to) in &map.0 {
        if url.origin() == to.origin() {
            let mut unmapped = url.clone();

            if unmapped.set_scheme(from.scheme()).is_ok()
                && unmapped.set_host(from.host_str()).is_ok()
                && unmapped.set_port(from.port()).is_ok()
            {
                return unmapped;
            }
        }
    }

    url.clone()
}

fn map_error(error: ureq::Error) -> HttpError {
    match error {
        ureq::Error::Timeout(_) => HttpError::Timeout,
        ureq::Error::BodyExceedsLimit(limit) => HttpError::BodyTooLarge(limit),
        other => HttpError::Network(other.to_string()),
    }
}

fn map_read_error(error: std::io::Error) -> HttpError {
    // ureq passes its own error through the `io::Error` of the reader.
    match error
        .get_ref()
        .and_then(|e| e.downcast_ref::<ureq::Error>())
    {
        Some(ureq::Error::BodyExceedsLimit(limit)) => HttpError::BodyTooLarge(*limit),
        Some(ureq::Error::Timeout(_)) => HttpError::Timeout,
        _ if error.kind() == std::io::ErrorKind::TimedOut => HttpError::Timeout,
        _ => HttpError::Body(error.to_string()),
    }
}

impl Fetcher for HttpFetcher {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        self.fetch_response(url).map(|r| r.body)
    }

    fn fetch_response(&self, url: &url::Url) -> Result<Response, String> {
        self.get(url).map_err(|e| e.message())?.check()
    }

    fn fetch_response_unless(&self, url: &url::Url, unread: &[&str]) -> Result<Response, String> {
        self.get_unless(url, unread)
            .map_err(|e| e.message())?
            .check()
    }
}

/// A fetcher for the oracle comparisons: the `LocalFetcher` serves `http://oracle/`, except the paths under `prefix`
/// (`/h/`) of a mapped origin; those go to HTTP (and so do the redirects they meet). Every other URL fails as it does at
/// the oracle page (`TypeError: Failed to fetch`), because the recorder aborts every request outside its own servers.
pub struct OracleHttpFetcher {
    pub local: x3d_runtime::LocalFetcher,
    pub http: HttpFetcher,
    /// The path prefix of the routes of a mapped origin.
    pub prefix: String,
}

impl OracleHttpFetcher {
    fn goes_to_http(&self, url: &url::Url) -> bool {
        self.http
            .origin_map
            .0
            .iter()
            .any(|(from, _)| url.origin() == from.origin() && url.path().starts_with(&self.prefix))
    }
}

impl Fetcher for OracleHttpFetcher {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        if self.goes_to_http(url) {
            self.http.fetch(url)
        } else {
            self.local.fetch(url)
        }
    }

    fn fetch_response(&self, url: &url::Url) -> Result<Response, String> {
        if self.goes_to_http(url) {
            self.http.fetch_response(url)
        } else {
            self.local.fetch_response(url)
        }
    }

    fn fetch_response_unless(&self, url: &url::Url, unread: &[&str]) -> Result<Response, String> {
        if self.goes_to_http(url) {
            self.http.fetch_response_unless(url, unread)
        } else {
            self.local.fetch_response_unless(url, unread)
        }
    }
}
