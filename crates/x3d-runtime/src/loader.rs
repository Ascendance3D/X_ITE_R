//! FileLoader.js (loadDocument / loadDocumentAsync / loadDocumentError / printError) against a
//! local, read-only file root, plus the X3DUrlObject nodes that only fetch (textures, sounds),
//! Inline and EXTERNPROTO loading.
//!
//! The oracle serves `oracle/` over HTTP and aborts every other request, so:
//! * `http://oracle/<path>` → the file under the root, `Error: Not Found` if missing (404),
//!   `Error: Forbidden` outside the root (403);
//! * every other URL (remote hosts, `file:`, `urn:` …) → `TypeError: Failed to fetch`.

use crate::*;
use std::path::PathBuf;

pub trait Fetcher {
    /// `fetch (url)` + `checkResponse`: the body bytes, or `String (error)` of the rejection.
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String>;

    /// `fetch (url)` + `checkResponse`, with the parts of the response that FileLoader reads besides the body: the final URL,
    /// the status, the headers (`Content-Type` decides the foreign hand-off of [`LoadHooks::foreign`]). The error is the
    /// same text as [`Fetcher::fetch`] gives: a network failure, or `Error: <statusText>` of a status outside 200 to 299.
    /// A fetcher with no HTTP (the default) answers a fetched body as status 200 with no headers, so existing
    /// implementations keep working; an HTTP fetcher (crate `x3d-http`) overrides this and implements [`Fetcher::fetch`]
    /// with it.
    fn fetch_response(&self, url: &url::Url) -> Result<Response, String> {
        self.fetch(url).map(|body| Response::ok(url.clone(), body))
    }

    /// [`Fetcher::fetch_response`], except that the body of a 2xx response whose mime type is in `unread` is not read: the
    /// response comes back with an empty body. It is FileLoader.js's `foreign` hand-off, which decides from the
    /// `Content-Type` header alone and never reads the body of a page (`FileLoader.js` 304-312), so a page that is large,
    /// slow or cut short still goes to `foreign`. The default reads the body ([`Fetcher::fetch_response`]); an HTTP
    /// fetcher overrides this.
    fn fetch_response_unless(&self, url: &url::Url, unread: &[&str]) -> Result<Response, String> {
        let _ = unread;
        self.fetch_response(url)
    }

    /// The error message of a failed request, or `None` when the request succeeded. The default
    /// calls [`Fetcher::fetch`], so for a successful request it copies the body and drops it.
    /// A fetcher that holds a response cache overrides this to answer from the cached state
    /// without copying: `WebFetcher` does.
    fn fetch_error(&self, url: &url::Url) -> Option<String> {
        self.fetch(url).err()
    }

    /// An asynchronous fetcher (a web page's `fetch`) answers [`Fetcher::fetch`] only for a URL whose response has
    /// arrived. The runtime then asks [`Fetcher::poll`] before a load runs, and holds the load until
    /// [`Runtime::resume_fetches`]. A synchronous fetcher (the default) is always ready.
    fn is_async(&self) -> bool {
        false
    }

    /// Has the response of `url` arrived? `Pending` starts its request when there is none yet.
    fn poll(&self, _url: &url::Url) -> FetchPoll {
        FetchPoll::Loaded
    }

    /// Does the page play a video that this fetcher's page loads (a browser build)? A MovieTexture candidate that is
    /// no GIF or PNG then goes to a `<video>` element of the page ([`nodes::movie_texture::VideoOp`]), and fails or
    /// plays as the element does. Without it the video is not decoded.
    fn plays_video(&self) -> bool {
        false
    }

    /// A FileLoader of its own requests `url` (the page's createVrmlFromURL, see `browser_load.rs`): an asynchronous fetcher
    /// starts a request for it even when it holds a response or one is in flight; the response that comes replaces the one
    /// it holds.
    fn refresh(&self, _url: &url::Url) {}

    /// A load attempt of an Inline starts over (its url is set again): an asynchronous fetcher forgets the failed
    /// response it holds for `url`, so the attempt asks for the file again, as X_ITE's `fetch` of each Inline load does.
    /// A response that arrived or is in flight stays. EXTERNPROTO keeps its failure (X_ITE's scene cache holds it for
    /// good), so only the Inline's own url set calls this.
    fn forget_failure(&self, _url: &url::Url) {}
}

/// The mime types that `FileLoader.js` hands to `foreign` instead of reading them (`FOREIGN_MIME_TYPES`).
pub const FOREIGN_MIME_TYPES: [&str; 2] = ["text/html", "application/xhtml+xml"];

/// A fetch response that passed `checkResponse`: what FileLoader.js reads of the `Response` object.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Response {
    /// `response.url`: the URL of the last request, after the redirects. FileLoader.js does not use it: the scene's world
    /// URL and the base of its relative URLs are the URL that was requested.
    pub url: url::Url,
    /// `response.status`.
    pub status: u16,
    /// `response.statusText`.
    pub status_text: String,
    /// `response.headers`: the names are lower case, in the order received; a repeated name is a repeated entry.
    pub headers: Vec<(String, String)>,
    /// `response.redirected`: at least one redirect was followed.
    pub redirected: bool,
    /// The body, after the transfer decoding and the content decoding of the transport (`Content-Encoding`).
    pub body: Vec<u8>,
}

impl Response {
    /// A response of status 200 with no header and no redirect.
    pub fn ok(url: url::Url, body: Vec<u8>) -> Response {
        Response {
            url,
            status: 200,
            status_text: "OK".into(),
            headers: vec![],
            redirected: false,
            body,
        }
    }

    /// `response.ok`.
    pub fn is_ok(&self) -> bool {
        (200..300).contains(&self.status)
    }

    /// `checkResponse`: `Error: <statusText>`, or `Error: <status>` when the text is empty, for a response that is not ok.
    pub fn check(self) -> Result<Response, String> {
        if self.is_ok() {
            return Ok(self);
        }

        if self.status_text.is_empty() {
            Err(format!("Error: {}", self.status))
        } else {
            Err(format!("Error: {}", self.status_text))
        }
    }

    /// `response.headers.get (name)`: the values of a repeated header are joined with `, `; `None` without the header.
    pub fn header(&self, name: &str) -> Option<String> {
        let values: Vec<&str> = self
            .headers
            .iter()
            .filter(|(n, _)| n.eq_ignore_ascii_case(name))
            .map(|(_, v)| v.as_str())
            .collect();

        (!values.is_empty()).then(|| values.join(", "))
    }

    /// `response.headers.get ("Content-Type") ?.replace (/;.*$/, "")`: the text before the first `;`, as sent (the case is
    /// not changed, and `FOREIGN_MIME_TYPES` is compared exactly).
    pub fn mime_type(&self) -> Option<String> {
        self.header("content-type")
            .map(|v| v.split(';').next().unwrap_or_default().to_string())
    }
}

/// The state of a URL at an asynchronous [`Fetcher`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FetchPoll {
    /// The request is in flight.
    Pending,
    /// The response arrived: [`Fetcher::fetch`] gives the bytes.
    Loaded,
    /// The request failed: [`Fetcher::fetch`] gives the error.
    Failed,
}

/// Script URLs (`/^\s*(?:ecmascript|javascript|vrmlscript):/s`): no fetch.
pub(crate) fn is_script_url(candidate: &str) -> bool {
    strip_script_scheme(candidate).is_some()
}

pub(crate) fn strip_script_scheme(candidate: &str) -> Option<(&'static str, &str)> {
    let trimmed = candidate.trim_start_matches(is_js_space);
    for scheme in ["ecmascript:", "javascript:", "vrmlscript:"] {
        if trimmed.starts_with(scheme) {
            return Some((scheme, &trimmed[scheme.len()..]));
        }
    }
    None
}

/// What `fetch (url)` rejects with before it sends a request: a URL with a user name or a password
/// (`Request cannot be constructed from a URL that includes credentials`), else `Failed to fetch`. `href` is the `href`
/// of the URL as Chromium prints it.
pub(crate) fn fetch_rejection(url: &url::Url, href: &str) -> String {
    if url.username().is_empty() && url.password().is_none() {
        "TypeError: Failed to fetch".to_string()
    } else {
        format!(
            "TypeError: Failed to execute 'fetch' on 'Window': Request cannot be constructed from a URL that includes credentials: {href}"
        )
    }
}

/// Serves `<prefix><path>` from `root/<path>` like oracle/x_ite-dump.mjs's server.
pub struct LocalFetcher {
    pub root: PathBuf,
    /// e.g. `http://oracle/`
    pub prefix: String,
}

impl Fetcher for LocalFetcher {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        let href = url.as_str();

        // `fetch (dataURL)` needs no server: the browser decodes the URL itself.
        if url.scheme() == "data" {
            return x3d_parse::decode_data_url(href);
        }

        // The Request constructor rejects a URL with credentials, of any scheme, before a request is sent.
        if !url.username().is_empty() || url.password().is_some() {
            return Err(fetch_rejection(url, href));
        }

        if !href.starts_with(&self.prefix) {
            return Err("TypeError: Failed to fetch".into());
        }

        // decodeURIComponent (pathname), then path.normalize (join (ROOT, …)).
        let decoded = percent_encoding::percent_decode_str(url.path())
            .decode_utf8_lossy()
            .into_owned();
        let mut path = self.root.clone();

        for part in decoded.split('/') {
            match part {
                "" | "." => {}
                ".." => {
                    if !path.pop() || !path.starts_with(&self.root) {
                        return Err("Error: Forbidden".into());
                    }
                }
                p => path.push(p),
            }
        }

        if !path.starts_with(&self.root) {
            return Err("Error: Forbidden".into());
        }

        // The oracle server reads a case-sensitive file system: `PLAIN.wrl` is not `plain.wrl`, also where this one ignores case
        // (macOS APFS, Windows NTFS).
        if !exact_case(&self.root, &path) {
            return Err("Error: Not Found".into());
        }

        match std::fs::read(&path) {
            Ok(data) => Ok(data),
            Err(_) => Err("Error: Not Found".into()),
        }
    }
}

/// Does every component of `path` below `root` exist under exactly that name (byte for byte), as on a case-sensitive file
/// system? A missing component is false.
fn exact_case(root: &std::path::Path, path: &std::path::Path) -> bool {
    let Ok(rest) = path.strip_prefix(root) else {
        return false;
    };
    let mut dir = root.to_path_buf();

    for part in rest.components() {
        let name = part.as_os_str();
        let Ok(entries) = std::fs::read_dir(&dir) else {
            return false;
        };

        if !entries.flatten().any(|e| e.file_name() == name) {
            return false;
        }

        dir.push(name);
    }

    true
}

/// MovieTexture.loadData takes a GIF or a PNG file (by its path, or a `data:image/gif` or `data:image/png` URL) as an image
/// it decodes itself; any other candidate is a video.
pub(crate) fn movie_is_image(url: &url::Url) -> bool {
    let path = url.path();
    let href = url.as_str();

    path.ends_with(".gif")
        || href.starts_with("data:image/gif")
        || path.ends_with(".png")
        || href.starts_with("data:image/png")
}

/// `decodeURI`: decode escapes except reserved characters; `None` on a malformed sequence.
pub fn decode_uri(s: &str) -> Option<String> {
    decode_escapes(s, b";/?:@&=+$,#")
}

/// `decodeURIComponent`: decode every escape; `None` on a malformed sequence (where JavaScript throws URIError).
pub fn decode_uri_component(s: &str) -> Option<String> {
    decode_escapes(s, b"")
}

fn decode_escapes(s: &str, reserved: &[u8]) -> Option<String> {
    let bytes = s.as_bytes();
    let mut out: Vec<u8> = vec![];
    let mut i = 0;

    while i < bytes.len() {
        if bytes[i] != b'%' {
            out.push(bytes[i]);
            i += 1;
            continue;
        }

        let hex = |j: usize| -> Option<u8> {
            let h = bytes.get(j..j + 2)?;

            if !h.iter().all(u8::is_ascii_hexdigit) {
                return None;
            }

            u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()
        };

        let b0 = hex(i + 1)?;

        if b0 < 0x80 {
            if reserved.contains(&b0) {
                out.extend_from_slice(&bytes[i..i + 3]);
            } else {
                out.push(b0);
            }

            i += 3;
            continue;
        }

        let n = match b0 {
            0xc0..=0xdf => 2,
            0xe0..=0xef => 3,
            0xf0..=0xf7 => 4,
            _ => return None,
        };

        let mut seq = vec![b0];

        for k in 1..n {
            if bytes.get(i + 3 * k) != Some(&b'%') {
                return None;
            }

            seq.push(hex(i + 3 * k + 1)?);
        }

        std::str::from_utf8(&seq).ok()?;
        out.extend_from_slice(&seq);
        i += 3 * n;
    }

    String::from_utf8(out).ok()
}

/// FileLoader.truncate (string, 120).
fn truncate(s: &str) -> String {
    let units: Vec<u16> = s.encode_utf16().collect();

    if units.len() > 120 {
        format!("{}…", String::from_utf16_lossy(&units[..120]))
    } else {
        s.to_string()
    }
}

/// JavaScript's `\s`: Unicode `White_Space` without U+0085, and U+FEFF.
pub(crate) fn is_js_space(c: char) -> bool {
    c == '\u{feff}' || (c.is_whitespace() && c != '\u{85}')
}

/// FileLoader.loadDocumentAsync, the `dataAsString` branch: `url .match (/^\s*data:(.*?)(?:;charset=(.*?))?(?:;(base64))?,/s)`;
/// unless the header ends with `;base64` (case-sensitive: `;BASE64` is text), the text after the first comma is
/// `decodeURIComponent`ed (kept as it is when that throws: one malformed escape keeps every escape) and a leading
/// `ï»¿` (a UTF-8 BOM read as Latin-1) is removed.
pub(crate) fn data_url_text(candidate: &str) -> Option<String> {
    let rest = candidate
        .trim_start_matches(is_js_space)
        .strip_prefix("data:")?;
    let (header, data) = rest.split_once(',')?;

    if header.ends_with(";base64") {
        return None;
    }

    let decoded = decode_uri_component(data).unwrap_or_else(|| data.to_string());

    Some(
        decoded
            .strip_prefix("\u{ef}\u{bb}\u{bf}")
            .map(str::to_string)
            .unwrap_or(decoded),
    )
}

/// What a FileLoader candidate produced.
pub enum Loaded {
    /// `javascript:` / `ecmascript:` / `vrmlscript:` source (no resolved URL).
    Script(String),
    /// Fetched (and gunzipped) bytes with the resolved URL.
    Data(Vec<u8>, url::Url),
}

/// The callbacks of FileLoader.createX3DFromURL (url, parameter, callback, bindViewpoint, foreign).
#[derive(Default, Clone, Debug)]
pub struct LoadHooks {
    /// `bindViewpoint` was given: a URL with only a new fragment names a viewpoint of the referring document.
    pub bind_viewpoint: bool,
    /// `foreign` was given: a `target` parameter (not `_self`) or a page URL goes to another window or document.
    pub foreign: bool,
    /// The `parameter` field / argument (`target=_blank`, ...).
    pub parameters: Vec<String>,
}

/// How a FileLoader run ended.
#[derive(Clone, Debug, PartialEq)]
pub enum LoadResult {
    /// A candidate was accepted.
    Loaded,
    /// `bindViewpoint (name)`: the decoded fragment.
    Fragment(String),
    /// `foreign (href, target)`.
    Foreign(String, String),
    /// Every candidate failed (or there was none).
    Failed,
    /// An asynchronous [`Fetcher`] has not answered the next candidate yet: the load task waits and goes on
    /// from that candidate ([`Runtime::allow_fetch_wait`]). Nothing is settled.
    Suspended,
}

/// A load task whose fallback chain can wait for an asynchronous [`Fetcher`].
#[derive(Clone, Debug, PartialEq, Eq, Hash)]
pub enum FetchKey {
    /// `Task::UrlLoad` (node, generation).
    Url(NodeId, u64),
    /// `Task::BrowserLoad` (request).
    Browser(u64),
    /// [`Runtime::load_page_document`]: the page's loadURL, which is no task of the runtime.
    Page,
}

impl FetchKey {
    pub fn of(task: &Task) -> Option<FetchKey> {
        match *task {
            Task::UrlLoad(node, generation) => Some(FetchKey::Url(node, generation)),
            Task::BrowserLoad(id) => Some(FetchKey::Browser(id)),
            _ => None,
        }
    }

    /// The task that goes on with the chain; the page's loadURL has none (its caller steps it again).
    pub fn task(&self) -> Option<Task> {
        match *self {
            FetchKey::Url(node, generation) => Some(Task::UrlLoad(node, generation)),
            FetchKey::Browser(id) => Some(Task::BrowserLoad(id)),
            FetchKey::Page => None,
        }
    }
}

/// FileLoader.loadDocumentError → loadDocumentAsync (this .url .shift ()): a candidate failed after its response
/// came, and the next one is fetched now. With an asynchronous [`Fetcher`] its response comes later, so the chain
/// stops there and goes on from `index` (of `urls`) with `attempts` failed candidates when it is in.
#[derive(Clone, Debug)]
pub struct FetchResume {
    pub index: usize,
    pub attempts: usize,
    /// The candidates of the chain; a chain of other candidates (the url changed) starts again.
    pub urls: Vec<String>,
}

/// `new URL (candidate, base)` as Chromium gives it: the `url` crate follows the URL standard, which removes every tab
/// and newline, but Chromium keeps those after a `data:` scheme (`x3d_parse::chromium_url_input`). Every candidate
/// URL of the runtime is resolved here.
pub fn join_url(base: &url::Url, candidate: &str) -> Result<url::Url, url::ParseError> {
    base.join(&x3d_parse::chromium_url_input(candidate))
}

/// A URL that Chromium's `new URL (candidate, base)` parses and the `url` crate cannot represent: a space in the host
/// of a URL with a special scheme (`http`, `https`, `ws`, `wss`, `ftp`, `file`). The URL standard rejects it; Chromium
/// keeps it as `%20` (`https://exa mple.invalid/` is `https://exa%20mple.invalid/`), also for the characters that
/// IDNA maps to a space (U+00A0, U+2000 to U+200A, U+202F, U+205F, U+3000) and for a `%20` escape. No server has such a
/// host, so a fetch of it fails.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SpaceHostUrl {
    /// The URL with a placeholder character in the host where the space is. Its host is no host to fetch from.
    pub stand_in: url::Url,
    /// The `href` of Chromium's URL.
    pub href: String,
}

/// What `new URL (candidate, base)` of FileLoader.loadDocumentAsync gave for one candidate, with the facts that the `url` crate
/// cannot hold. [`Runtime::resolve_candidate`] makes it, and it goes with the candidate through the FileLoader chain, EXTERNPROTO and
/// Script loading, so that no later step has to look the URL up by its text: a candidate whose text equals the stand-in URL of
/// another candidate is an ordinary URL (`space_host` is false).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ResolvedCandidate {
    /// The URL to fetch from and to resolve against. For a host with a space it is the stand-in URL of [`SpaceHostUrl`].
    pub url: url::Url,
    /// The `href` Chromium gives (`url .as_str ()` unless `space_host`).
    pub href: String,
    /// The host holds a space: Chromium parses the URL, no server has such a host, and `fetch` rejects it.
    pub space_host: bool,
}

impl ResolvedCandidate {
    /// An ordinary URL: Chromium's `href` is the one of the `url` crate.
    pub fn plain(url: url::Url) -> ResolvedCandidate {
        ResolvedCandidate {
            href: url.as_str().to_string(),
            url,
            space_host: false,
        }
    }
}

impl std::ops::Deref for ResolvedCandidate {
    type Target = url::Url;

    fn deref(&self) -> &url::Url {
        &self.url
    }
}

/// The characters that IDNA (UTS 46, without the STD3 rules) maps to U+0020.
fn is_idna_space(c: char) -> bool {
    matches!(
        c,
        ' ' | '\u{a0}' | '\u{2000}'..='\u{200a}' | '\u{202f}' | '\u{205f}' | '\u{3000}'
    )
}

/// [`SpaceHostUrl`] for `candidate` resolved against `base`, when [`join_url`] fails only because the host holds a
/// space; `None` for every other failure and for a URL that [`join_url`] parses.
pub fn space_host_url(base: &url::Url, candidate: &str) -> Option<SpaceHostUrl> {
    // The URL standard strips C0 controls and spaces around the input and removes every tab and newline.
    let input: String = x3d_parse::chromium_url_input(candidate)
        .trim_matches(|c: char| c <= ' ')
        .chars()
        .filter(|c| !matches!(c, '\t' | '\n' | '\r'))
        .collect();
    let slash = |c: char| c == '/' || c == '\\';

    // Where the authority starts, in `input`: after the scheme and its slashes, or after the slashes of a scheme-relative URL.
    let scheme_end = input.find(':').filter(|&i| {
        input[..i].starts_with(|c: char| c.is_ascii_alphabetic())
            && input[..i]
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || matches!(c, '+' | '-' | '.'))
    });
    let (scheme, rest, prefix) = match scheme_end {
        Some(i) => (
            input[..i].to_ascii_lowercase(),
            &input[i + 1..],
            &input[..=i],
        ),
        None => (base.scheme().to_string(), input.as_str(), ""),
    };

    if !["http", "https", "ws", "wss", "ftp", "file"].contains(&scheme.as_str()) {
        return None;
    }

    let slashes = rest.chars().take_while(|&c| slash(c)).count();
    let authority_follows = if scheme == "file" {
        slashes == 2
    } else if scheme_end.is_some() && scheme == base.scheme() {
        // "special relative or authority state": `https:x` and `https:/x` are relative to the base.
        slashes >= 2
    } else if scheme_end.is_some() {
        true
    } else {
        slashes >= 2
    };

    if !authority_follows {
        return None;
    }

    let after = &rest[slashes..];
    let authority_len = after
        .find(|c: char| slash(c) || c == '?' || c == '#')
        .unwrap_or(after.len());
    let authority = &after[..authority_len];
    let host_port = authority.rsplit_once('@').map_or(authority, |(_, h)| h);
    let userinfo = &authority[..authority.len() - host_port.len()];

    if host_port.starts_with('[') {
        return None;
    }

    let host_len = host_port.find(':').unwrap_or(host_port.len());
    let (host, port) = host_port.split_at(host_len);

    // URL host parsing percent-decodes the host first (a `%20` is a space), then IDNA maps the characters.
    let decoded = percent_encoding::percent_decode_str(host)
        .decode_utf8()
        .ok()?;

    if !decoded.chars().any(is_idna_space) {
        return None;
    }

    // Chromium encodes a label with other non-ASCII characters differently when its space is a character that IDNA maps to a
    // space (`a<U+00A0>ä` is `xn--a%20-wia`, `a ä` is `xn--a%20-ooa`; measured). Not reproduced: `Invalid URL`.
    if decoded.chars().any(|c| c != ' ' && is_idna_space(c))
        && decoded.chars().any(|c| !c.is_ascii() && !is_idna_space(c))
    {
        return None;
    }

    // The forbidden host code points and the controls of the standard stay errors (`%2F`, `%25`, `%00` ...).
    if decoded.chars().any(|c| {
        c <= '\u{1f}'
            || c == '\u{7f}'
            || matches!(
                c,
                '#' | '/' | ':' | '<' | '>' | '?' | '@' | '[' | '\\' | ']' | '^' | '|' | '%'
            )
    }) {
        return None;
    }

    // A placeholder that the `url` crate keeps in a host, that the input does not use, and that the base does not hold.
    let placeholder = ['_', '~', '!', '$', '&', '\'', '(', ')', '+', ',', ';', '=']
        .into_iter()
        .find(|&c| !input.contains(c) && !base.as_str().contains(c))?;
    // Chromium escapes the space to `%20` before it encodes the label (`exä mple` is `xn--ex%20mple-w2a`), so the label
    // holds three characters where the space was: the punycode of the other characters depends on their number.
    let marked: String = decoded
        .chars()
        .map(|c| {
            if is_idna_space(c) {
                placeholder.to_string().repeat(3)
            } else {
                c.to_string()
            }
        })
        .collect();
    let stand_in_input = format!(
        "{prefix}{}{userinfo}{marked}{port}{}",
        &rest[..slashes],
        &after[authority_len..]
    );
    let stand_in = base.join(&stand_in_input).ok()?;
    let host = stand_in.host_str()?;
    let href = stand_in.as_str();

    // The host of the stand-in starts after `scheme://` and the user information, if any.
    let authority_start = href.find("://")? + 3;
    let host_start = if stand_in.username().is_empty() && stand_in.password().is_none() {
        authority_start
    } else {
        authority_start + href[authority_start..].find('@')? + 1
    };

    if !href[host_start..].starts_with(host) {
        return None;
    }

    let host_end = host_start + host.len();
    let href = format!(
        "{}{}{}",
        &href[..host_start],
        host.replace(&placeholder.to_string().repeat(3), "%20"),
        &href[host_end..]
    );

    Some(SpaceHostUrl { stand_in, href })
}

/// The decoded fragment of `candidate` if FileLoader's `bindViewpoint` branch takes it: a URL that, resolved against
/// `base`, differs from the referring document only by a non-empty fragment.
pub fn viewpoint_fragment(base: &str, candidate: &str, hooks: &LoadHooks) -> Option<String> {
    if !hooks.bind_viewpoint || candidate.is_empty() {
        return None;
    }

    let referer = url::Url::parse(base).ok()?;
    let r = join_url(&referer, candidate).ok()?;

    if r.scheme() == "data" || r.fragment().is_none_or(str::is_empty) {
        return None;
    }

    let same = referer.scheme() == r.scheme()
        && referer.host_str() == r.host_str()
        && referer.port() == r.port()
        && referer.path() == r.path();

    same.then(|| {
        percent_encoding::percent_decode_str(r.fragment().unwrap_or_default())
            .decode_utf8()
            .map(|d| d.into_owned())
            .ok()
    })
    .flatten()
}

/// FileLoader.getTarget: the value of the first `target=value` parameter.
pub fn loader_target(parameters: &[String]) -> String {
    for parameter in parameters {
        let pair: Vec<&str> = parameter.split('=').collect();

        if pair.len() == 2 && pair[0] == "target" {
            return pair[1].to_string();
        }
    }

    String::new()
}

/// A fetched BufferAudioSource / AudioClip candidate whose decodeAudioData result is still pending, with what
/// FileLoader needs to go on if it rejects.
pub struct PendingAudioDecode {
    pub node: NodeId,
    pub bytes: Vec<u8>,
    /// The candidate as written and as resolved (for the warning).
    pub candidate: String,
    pub resolved: url::Url,
    /// The candidates after this one.
    pub rest: Vec<String>,
    /// The candidates before this one, all failed.
    pub attempts: usize,
}

/// A FileLoader instance: node type name for messages (`""` for the world) and the attempts.
pub struct FileLoader {
    pub type_name: String,
    pub base: String,
    pub data_as_string: bool,
}

impl Runtime {
    /// The next fallback chain of the running load task may wait for an asynchronous [`Fetcher`]
    /// ([`LoadResult::Suspended`]). The caller then settles nothing for that result: the task runs again.
    pub(crate) fn allow_fetch_wait(&mut self) {
        self.fetch_wait = self
            .running_fetch
            .clone()
            .filter(|_| self.fetcher.is_async());
    }

    /// The candidate of `urls` where the chain of the running load task goes on, if it waited.
    pub(crate) fn fetch_resume_of(&self, key: &FetchKey, urls: &[String]) -> Option<&FetchResume> {
        self.fetch_resume
            .get(key)
            .filter(|r| r.urls == urls && r.index < urls.len())
    }

    /// X3DExecutionContext.getBaseURL of a node's context (the scene's world URL).
    pub fn base_url(&self, node: NodeId) -> String {
        self.scene
            .world_url_of(self.scene.node(node).ctx)
            .to_string()
    }

    /// FileLoader.loadDocument with its fallback loop. `accept` is the data callback: an `Err`
    /// is a rejection of loadDocumentAsync and moves on to the next URL, like X_ITE.
    /// Returns whether a candidate was accepted.
    pub fn load_document(
        &mut self,
        loader: &FileLoader,
        urls: &[String],
        mut accept: impl FnMut(&mut Runtime, Loaded) -> Result<(), String>,
    ) -> bool {
        self.load_document_opt(loader, urls, |rt, loaded| accept(rt, loaded).map_err(Some))
    }

    pub(crate) fn load_document_opt(
        &mut self,
        loader: &FileLoader,
        urls: &[String],
        mut accept: impl FnMut(&mut Runtime, Loaded) -> Result<(), Option<String>>,
    ) -> bool {
        !matches!(
            self.load_document_from_opt(loader, urls, &LoadHooks::default(), 0, |rt, _, loaded| {
                accept(rt, loaded)
            }),
            LoadResult::Failed
        )
    }

    /// [`Runtime::load_document`] with the two callbacks X3DBrowser.loadURL and Anchor hand FileLoader.createX3DFromURL
    /// (`bindViewpoint` and `foreign`, see [`LoadHooks`]).
    pub fn load_document_hooks(
        &mut self,
        loader: &FileLoader,
        urls: &[String],
        hooks: &LoadHooks,
        mut accept: impl FnMut(&mut Runtime, Loaded) -> Result<(), String>,
    ) -> LoadResult {
        self.load_document_from_opt(loader, urls, hooks, 0, |rt, _, loaded| {
            accept(rt, loaded).map_err(Some)
        })
    }

    /// [`Runtime::load_document_hooks`] from a later point of the fallback chain: `attempts` candidates before
    /// `urls` have already failed (and were reported). `accept` also gets the candidate's index in `urls`.
    pub(crate) fn load_document_from(
        &mut self,
        loader: &FileLoader,
        urls: &[String],
        hooks: &LoadHooks,
        attempts: usize,
        mut accept: impl FnMut(&mut Runtime, usize, Loaded) -> Result<(), String>,
    ) -> LoadResult {
        self.load_document_from_opt(loader, urls, hooks, attempts, |rt, idx, loaded| {
            accept(rt, idx, loaded).map_err(Some)
        })
    }

    pub(crate) fn load_document_from_opt(
        &mut self,
        loader: &FileLoader,
        urls: &[String],
        hooks: &LoadHooks,
        attempts: usize,
        mut accept: impl FnMut(&mut Runtime, usize, Loaded) -> Result<(), Option<String>>,
    ) -> LoadResult {
        let mut attempts = attempts;
        let mut special: Option<LoadResult> = None;
        // Only the chain the caller allowed can wait; a load it starts meanwhile cannot.
        let wait = self.fetch_wait.take();
        let mut start = 0;

        if urls.is_empty() {
            // loadDocumentError () without an error prints nothing.
            return LoadResult::Failed;
        }

        if let Some(r) = wait.as_ref().and_then(|k| self.fetch_resume_of(k, urls)) {
            start = r.index;
            attempts = r.attempts;
        }

        for (i, candidate) in urls.iter().enumerate().skip(start) {
            let remaining = urls.len() - i - 1;
            let mut resolved: Option<ResolvedCandidate> = None;
            let mut not_in = false;

            let result: Result<(), Option<String>> = (|| {
                if candidate.is_empty() {
                    return Err(Some("Error: Empty URL.".to_string()));
                }

                // Script URLs: /^\s*(?:ecmascript|javascript|vrmlscript):/s
                if let Some((_, source)) = strip_script_scheme(candidate) {
                    return accept(self, i, Loaded::Script(source.to_string()));
                }

                let base =
                    url::Url::parse(&loader.base).map_err(|e| Some(format!("TypeError: {e}")))?;
                let r = self.resolve_candidate(&base, candidate).map_err(|_| {
                    Some("TypeError: Failed to construct 'URL': Invalid URL".to_string())
                })?;
                resolved = Some(r.clone());

                // `if (this .dataAsString)`: a data URL that is not base64 is decoded here and handed to the callback as text.
                if loader.data_as_string {
                    if let Some(text) = data_url_text(candidate) {
                        return accept(self, i, Loaded::Data(text.into_bytes(), r.url.clone()));
                    }
                }

                // Bind Viewpoint URLs: a URL that only adds a fragment to the referring document names a viewpoint.
                if hooks.bind_viewpoint
                    && r.scheme() != "data"
                    && r.fragment().is_some_and(|f| !f.is_empty())
                {
                    let same = url::Url::parse(&loader.base).is_ok_and(|referer| {
                        referer.scheme() == r.scheme()
                            && referer.host_str() == r.host_str()
                            && referer.port() == r.port()
                            && referer.path() == r.path()
                    });

                    if same {
                        let fragment = r.fragment().unwrap_or_default();
                        let name = percent_encoding::percent_decode_str(fragment)
                            .decode_utf8()
                            .map(|d| d.into_owned())
                            .map_err(|_| Some("URIError: URI malformed".to_string()))?;
                        special = Some(LoadResult::Fragment(name));
                        return Ok(());
                    }
                }

                // Foreign targets: another window, or foreign content (a page). Never loaded here.
                if hooks.foreign {
                    let target = loader_target(&hooks.parameters);

                    let href = r.href.clone();

                    if !target.is_empty() && target != "_self" {
                        special = Some(LoadResult::Foreign(href, target));
                        return Ok(());
                    }

                    if r.scheme() != "data"
                        && ["html", "htm", "xhtml"]
                            .iter()
                            .any(|e| href.ends_with(&format!(".{e}")))
                    {
                        special = Some(LoadResult::Foreign(href, target));
                        return Ok(());
                    }
                }

                // A base64 data URL is fetched like any other (the browser decodes it); one without a comma is no URL fetch can read.
                if loader.data_as_string && r.scheme() == "data" && !candidate.contains(',') {
                    return Err(Some("TypeError: Failed to fetch".to_string()));
                }

                // A host with a space (Chromium's `%20`) is no host of any server: `fetch` rejects it.
                if let Some(rejection) = self.url_rejection(&r) {
                    return Err(Some(rejection));
                }

                // `await fetch (...)`: the response of this candidate is not in yet. A data URL is not waited for,
                // like in fetch_task_ready.
                // The FileLoader of a page's createVrmlFromURL requests each candidate it reaches itself.
                if let Some(FetchKey::Browser(id)) = wait.as_ref()
                    && let Some(own) = self.page_browser_loads.get(id)
                    && r.scheme() != "data"
                    && i >= own.requested.get()
                {
                    own.requested.set(i + 1);
                    self.fetcher.refresh(&r);
                }

                if wait.is_some()
                    && r.scheme() != "data"
                    && self.fetcher.poll(&r) == FetchPoll::Pending
                {
                    not_in = true;
                    return Err(None);
                }

                // fetch (dataURL) of a ShaderPart that the text branch did not take: the shader text is carried in the URL.
                let data = if loader.type_name == "ShaderPart" && r.scheme() == "data" {
                    x3d_parse::decode_data_url(candidate).map_err(Some)?
                } else {
                    // `checkResponse (await fetch (this .resolvedURL, options))`, then the Content-Type of the response:
                    // `FOREIGN_MIME_TYPES` go to `foreign (this .resolvedURL .href, this .target)` and are not read.
                    // The href is the requested URL, not `response .url` after a redirect.
                    let unread: &[&str] = if hooks.foreign {
                        &FOREIGN_MIME_TYPES
                    } else {
                        &[]
                    };
                    let response = self
                        .fetcher
                        .fetch_response_unless(&r, unread)
                        .map_err(Some)?;

                    if hooks.foreign
                        && response
                            .mime_type()
                            .is_some_and(|m| FOREIGN_MIME_TYPES.contains(&m.as_str()))
                    {
                        special = Some(LoadResult::Foreign(
                            r.as_str().to_string(),
                            loader_target(&hooks.parameters),
                        ));
                        return Ok(());
                    }

                    response.body
                };

                self.buffers_pending = false;
                let outer = std::mem::replace(&mut self.accept_waits, wait.is_some());
                let accepted = accept(
                    self,
                    i,
                    Loaded::Data(x3d_parse::gunzip(&data), r.url.clone()),
                );
                self.accept_waits = outer;
                accepted
            })();

            // The candidate waits for its response, or for the buffers of its glTF (GLTF2Parser `await`s them
            // before it builds the scene): the chain goes on with this candidate when they are in.
            if let Some(key) = wait.as_ref()
                && (not_in || (result.is_err() && std::mem::take(&mut self.buffers_pending)))
            {
                self.fetch_resume.insert(
                    key.clone(),
                    FetchResume {
                        index: i,
                        attempts,
                        urls: urls.to_vec(),
                    },
                );
                self.fetch_suspended = true;
                return LoadResult::Suspended;
            }

            self.buffers_pending = false;

            match result {
                Ok(()) => return special.take().unwrap_or(LoadResult::Loaded),
                Err(error) => {
                    if let Some(err) = error {
                        attempts += 1;
                        self.print_load_error(
                            loader,
                            candidate,
                            resolved.as_ref(),
                            &err,
                            attempts,
                            remaining,
                        );
                    }

                    // The buffer FileLoaders of a glTF that failed log when their own fetch fails,
                    // after this FileLoader's result.
                    let world = FileLoader {
                        type_name: String::new(),
                        base: loader.base.clone(),
                        data_as_string: false,
                    };

                    for b in std::mem::take(&mut self.deferred_buffer_errors) {
                        self.print_load_error(
                            &world,
                            &b.candidate,
                            b.resolved.clone().map(ResolvedCandidate::plain).as_ref(),
                            &b.error,
                            1,
                            0,
                        );
                    }
                }
            }
        }

        LoadResult::Failed
    }

    /// X3DBrowser.loadURL of the page that embeds the browser: FileLoader.createX3DFromURL for the X3DWorld (its
    /// messages name no node type), the fallback chain of `urls` against `base`. It runs in a runtime of its own,
    /// whose fetcher is the new world's. With an asynchronous [`Fetcher`] the chain stops at a candidate whose
    /// response is not in ([`LoadResult::Suspended`]); call again with the same `urls` when a response arrived, and
    /// it goes on from that candidate. `accept` parses a candidate (`createX3DFromString`); an `Err` is its rejection,
    /// and `Err (None)` a rejection with no error (nothing is logged for it; a glTF whose buffers are not in yet, see
    /// [`Runtime::parse_candidate`]).
    pub fn load_page_document(
        &mut self,
        base: &str,
        urls: &[String],
        mut accept: impl FnMut(&mut Runtime, Loaded) -> Result<(), Option<String>>,
    ) -> LoadResult {
        let outer = (
            self.running_fetch.replace(FetchKey::Page),
            std::mem::take(&mut self.fetch_suspended),
            self.fetch_wait.take(),
        );
        let loader = FileLoader {
            type_name: String::new(),
            base: base.to_string(),
            data_as_string: true,
        };

        self.allow_fetch_wait();

        let result = self.load_document_from_opt(
            &loader,
            urls,
            &LoadHooks::default(),
            0,
            |rt, _, loaded| accept(rt, loaded),
        );

        if result != LoadResult::Suspended {
            self.fetch_resume.remove(&FetchKey::Page);
        }

        (self.running_fetch, self.fetch_suspended, self.fetch_wait) = outer;
        result
    }

    /// A loop of single-candidate loads (MovieTexture, FontLibrary) of the running load task `wait` reaches candidate
    /// `index` of `urls`, whose response is not in: the task stops and goes on from `index` when it is
    /// ([`LoadResult::Suspended`]). Returns whether it waits. A data URL needs no response.
    pub(crate) fn fetch_waits(
        &mut self,
        wait: Option<&FetchKey>,
        urls: &[String],
        index: usize,
        url: &url::Url,
    ) -> bool {
        let Some(key) = wait else {
            return false;
        };

        if url.scheme() == "data" || self.fetcher.poll(url) != FetchPoll::Pending {
            return false;
        }

        self.fetch_resume.insert(
            key.clone(),
            FetchResume {
                index,
                attempts: 0,
                urls: urls.to_vec(),
            },
        );
        self.fetch_suspended = true;
        true
    }

    /// FileLoader.abort () of [`Runtime::load_page_document`] (`this .url .length = 0`): the candidate in flight is
    /// the last one. Returns how many of the candidates are left, to pass with the next call, or `None` when the
    /// chain does not wait.
    pub fn abort_page_document(&mut self) -> Option<usize> {
        let r = self.fetch_resume.get_mut(&FetchKey::Page)?;

        r.urls.truncate(r.index + 1);
        Some(r.urls.len())
    }

    /// `new URL (candidate, base)` of FileLoader.loadDocumentAsync: [`join_url`], and for a host with a space (which
    /// Chromium parses and the `url` crate does not) the stand-in URL of [`space_host_url`] with Chromium's `href`
    /// ([`ResolvedCandidate`]). Nothing is remembered in the runtime: the answer carries the facts.
    pub(crate) fn resolve_candidate(
        &self,
        base: &url::Url,
        candidate: &str,
    ) -> Result<ResolvedCandidate, url::ParseError> {
        match join_url(base, candidate) {
            Ok(r) => Ok(ResolvedCandidate::plain(r)),
            Err(error) => match space_host_url(base, candidate) {
                Some(space_host) => Ok(ResolvedCandidate {
                    url: space_host.stand_in,
                    href: space_host.href,
                    space_host: true,
                }),
                None => Err(error),
            },
        }
    }

    /// What `fetch` rejects with before any [`Fetcher`] runs: a host with a space (no server has one), or a URL with
    /// a user name or a password (the `Request` constructor of the browser refuses it, whatever the scheme except `data`).
    /// `None` for any other URL. The check is here, not in one [`Fetcher`], so that every fetcher has the same rule.
    pub(crate) fn url_rejection(&self, resolved: &ResolvedCandidate) -> Option<String> {
        if resolved.space_host {
            return Some(fetch_rejection(&resolved.url, &resolved.href));
        }

        if resolved.scheme() != "data"
            && (!resolved.username().is_empty() || resolved.password().is_some())
        {
            return Some(fetch_rejection(&resolved.url, resolved.url.as_str()));
        }

        None
    }

    /// The subject of FileLoader.printError: the candidate's `href` as Chromium gives it ([`ResolvedCandidate::href`]).
    pub(crate) fn format_load_subject(
        candidate: &str,
        resolved: Option<&ResolvedCandidate>,
    ) -> String {
        let data_url = resolved.is_some_and(|r| r.scheme() == "data");
        let resolved_text = match resolved {
            Some(r) if !data_url => decode_uri(&r.href).unwrap_or_else(|| r.href.clone()),
            _ => String::new(),
        };
        if candidate.is_empty() {
            "empty URL".to_string()
        } else if data_url {
            "data URL".to_string()
        } else if !resolved_text.is_empty() {
            format!("URL '{resolved_text}'")
        } else {
            format!("URL '{}'", truncate(candidate))
        }
    }

    /// FileLoader.printError for the candidate that just failed: `attempts` counts it, `remaining`
    /// is the number of candidates after it.
    pub(crate) fn print_load_error(
        &mut self,
        loader: &FileLoader,
        candidate: &str,
        resolved: Option<&ResolvedCandidate>,
        error: &str,
        attempts: usize,
        remaining: usize,
    ) {
        let subject = Self::format_load_subject(candidate, resolved);
        let prefix = if loader.type_name.is_empty() {
            String::new()
        } else {
            format!("{}: ", loader.type_name)
        };

        if remaining > 0 {
            self.log_warn(format!(
                "{prefix}Couldn't load {subject}, trying URL {} of {}. {error}",
                attempts + 1,
                attempts + remaining
            ));
        } else if attempts == 1 {
            self.log_error(format!("{prefix}Couldn't load {subject}. {error}"));
        } else {
            self.log_error(format!(
                "{prefix}Couldn't load any of the {attempts} URLs, tried in this order:"
            ));
        }
    }

    /// Is the fetch task ready to run with an asynchronous [`Fetcher`]? FileLoader tries the candidates in turn, so
    /// the responses up to the first that arrived must be there; the candidates after it are not requested.
    pub(crate) fn fetch_task_ready(&self, task: &Task) -> bool {
        // A Script's fetch is of one candidate (Script.loadData's FileLoader): it runs when that response is in.
        if let Task::ScriptFetch(node, index, generation) = *task {
            return self.script_fetch_ready(node, index, generation);
        }

        // `world`: the parameters of a loadURL or Anchor load, whose foreign targets are never fetched.
        let (base, mut urls, world) = match *task {
            Task::UrlLoad(node, generation) => {
                if node as usize >= self.scene.nodes.len() || self.scene.free_nodes.contains(&node)
                {
                    return true;
                }

                let urls = if self.is_scene_loader(node) {
                    self.inline_request_urls(node, generation)
                } else {
                    self.url_strings(node)
                };

                (self.base_url(node), urls, None)
            }
            Task::ExternLoad(e, request) => match self.extern_pending_candidates(e, request) {
                Some((base, urls)) => (base, urls, None),
                None => return true,
            },
            Task::BrowserLoad(id) => match self.browser_loads.get(&id) {
                Some(req) if !self.world_state.aborted.contains(&id) => (
                    self.browser_load_base(id, req.parent),
                    req.urls.clone(),
                    req.world.clone(),
                ),
                _ => return true,
            },
            _ => return true,
        };
        let Ok(base) = url::Url::parse(&base) else {
            return true;
        };

        // A chain that waited goes on from the candidate it waits for: the candidates before it failed.
        let mut offset = 0;

        if let Some(r) = FetchKey::of(task).and_then(|k| self.fetch_resume_of(&k, &urls)) {
            urls.drain(..r.index);
            offset = r.index;
        }

        // The FileLoader of a page's createVrmlFromURL requests each candidate it reaches itself.
        let own = match *task {
            Task::BrowserLoad(id) => self.page_browser_loads.get(&id),
            _ => None,
        };

        // load_document_from_opt's foreign rule: a `target` other than `_self` goes to another window unfetched.
        if let Some(parameters) = &world {
            let target = loader_target(parameters);

            if !target.is_empty() && target != "_self" {
                return true;
            }
        }

        for (i, candidate) in urls.iter().enumerate() {
            if candidate.is_empty() || is_script_url(candidate) {
                continue;
            }

            let Ok(r) = join_url(&base, candidate) else {
                continue;
            };

            if let Some(own) = own
                && r.scheme() != "data"
                && offset + i >= own.requested.get()
            {
                own.requested.set(offset + i + 1);
                self.fetcher.refresh(&r);
            }

            // With the `Cache` option off an EXTERNPROTO asks for its file again at each load.
            if let Task::ExternLoad(_, request) = *task
                && r.scheme() != "data"
                && !self.browser_options.cache()
                && self
                    .extern_refreshed
                    .borrow_mut()
                    .insert((request, offset + i))
            {
                self.fetcher.refresh(&r);
            }

            // A MovieTexture video is requested by the page's video element, not by the engine: it is ready.
            if let Task::UrlLoad(node, _) = *task
                && self.fetcher.plays_video()
                && self.builtin_name(node) == Some("MovieTexture")
                && !movie_is_image(&r)
            {
                return true;
            }

            // `fetch (dataURL)` needs no request: a data URL that decodes is ready, and no later candidate is
            // requested; one that does not decode fails at once, like a failed response.
            if r.scheme() == "data" {
                if x3d_parse::decode_data_url(r.as_str()).is_ok() {
                    return true;
                }

                continue;
            }

            // A loadURL of a fragment of the referring document binds a viewpoint: no fetch.
            if world.is_some()
                && r.fragment().is_some_and(|f| !f.is_empty())
                && (r.scheme(), r.host_str(), r.port(), r.path())
                    == (base.scheme(), base.host_str(), base.port(), base.path())
            {
                return true;
            }

            // A page (foreign content) is opened, not fetched.
            if world.is_some()
                && ["html", "htm", "xhtml"]
                    .iter()
                    .any(|e| r.as_str().ends_with(&format!(".{e}")))
            {
                return true;
            }

            match self.fetcher.poll(&r) {
                FetchPoll::Loaded => return true,
                FetchPoll::Failed => continue,
                FetchPoll::Pending => return false,
            }
        }

        true
    }

    /// The fetches that waited for an asynchronous [`Fetcher`] go back to the front of the queue, in request order,
    /// and the tasks run. A load whose responses have still not arrived waits again.
    pub fn resume_fetches(&mut self) {
        self.resume_font_fetches();

        // The glTF buffers that an Inline's rejected candidate left in flight (issue #128) log their own failures.
        self.follow_buffers();

        if self.parked_fetches.is_empty() {
            return;
        }

        for task in std::mem::take(&mut self.parked_fetches).into_iter().rev() {
            self.fetches.push_front(task);
        }

        self.run_tasks();
    }

    /// The FontStyle loops whose font file's response is in go on with that file (browser.loadFont).
    fn resume_font_fetches(&mut self) {
        for p in std::mem::take(&mut self.font_fetch_waits) {
            // A newer loadData of the node took over, or the node is gone.
            if !self.scene.is_live(p.node)
                || self.font_style_snapshot.get(&p.node)
                    != Some(&(p.family.clone(), p.style.clone()))
            {
                continue;
            }

            let waits = url::Url::parse(&self.base_url(p.node))
                .and_then(|b| join_url(&b, &p.family[p.index]))
                .is_ok_and(|u| self.fetcher.poll(&u) == FetchPoll::Pending);

            if waits {
                self.font_fetch_waits.push(p);
            } else {
                self.font_style_continue_from(p.node, p.family, p.index, p.style, true);
            }
        }
    }

    /// Does a load wait for an asynchronous [`Fetcher`]?
    pub fn has_parked_fetches(&self) -> bool {
        !self.parked_fetches.is_empty()
    }

    pub(crate) fn url_strings(&self, node: NodeId) -> Vec<String> {
        match self.pre_value(node, "url") {
            Some(FieldValue::MFString(v)) => v.clone(),
            _ => vec![],
        }
    }

    /// Inline.loadData / ImageTexture.loadData … : run the FileLoader for the node.
    pub(crate) fn url_load(&mut self, node: NodeId, generation: u64) {
        let type_name = self.type_name(node);

        if type_name == "MovieTexture" {
            return self.movie_load(node);
        }

        if type_name == "BufferAudioSource" || type_name == "AudioClip" {
            return self.buffer_load(node);
        }

        if type_name == "FontLibrary" {
            return self.font_library_load(node);
        }

        if matches!(type_name.as_str(), "FontStyle" | "ScreenFontStyle") {
            return self.font_style_load(node);
        }

        if type_name == "Inline" || type_name == "InlineGeometry" {
            // The request may have been replaced, cancelled or disposed since it was queued.
            if self.inline_request_is_current(node, generation) {
                self.inline_load(node, generation);
            }

            // The task of this request is done, run or not: its url snapshot goes.
            if let Some(s) = self.url_state_mut(node) {
                s.fetch_urls.retain(|(g, _)| *g != generation);
            }

            return;
        }

        if type_name == "ShaderPart" {
            return self.shader_part_load(node);
        }

        let loader = FileLoader {
            type_name: type_name.clone(),
            base: self.base_url(node),
            data_as_string: false,
        };
        let urls = self.url_strings(node);

        let image_texture = type_name == "ImageTexture";
        let atlas = type_name == "ImageTextureAtlas";
        let image_3d = type_name == "ImageTexture3D";

        self.allow_fetch_wait();

        let result =
            self.load_document_from(&loader, &urls, &LoadHooks::default(), 0, |rt, _, loaded| {
                match loaded {
                    Loaded::Script(_) => {
                        Err("Error: Couldn't parse X3D. No suitable file handler found.".into())
                    }
                    Loaded::Data(data, url) => {
                        // ImageTexture.setImage: setTextureData before COMPLETE_STATE.
                        if image_texture {
                            rt.image_texture_set_image(node, &data, &url)?;
                        }
                        if atlas {
                            return rt.image_texture_atlas_set_image(node, &data);
                        }
                        if image_3d {
                            return rt.image_texture_3d_set_data(node, &data);
                        }
                        Ok(())
                    }
                }
            });

        // The load goes on when the response of the next candidate is in (IN_PROGRESS_STATE meanwhile).
        if result == LoadResult::Suspended {
            return;
        }

        let ok = result != LoadResult::Failed;

        // ImageTexture.loadData with `data === null`: clearTexture, then FAILED_STATE.
        if !ok && image_texture {
            self.decoded_images.retain(|(n, ..)| *n != node);
            self.set_texture_2d_size(node, 1, 1);
        }

        // ImageTextureAtlas: clearTexture, updateOutputs (0, 0, 0, 0), FAILED_STATE; on success the
        // outputs and COMPLETE_STATE wait for the decode.
        if atlas {
            if ok {
                return;
            }
            self.image_texture_atlas_clear(node);
        }

        // ImageTexture3D: clearTexture, updateOutputs (0, 0, 0, 0), FAILED_STATE (the outputs and the
        // texture were set by the parser before COMPLETE_STATE).
        if image_3d && !ok {
            self.image_texture_3d_clear(node);
        }

        self.set_load_state(
            node,
            if ok {
                nodes::LoadState::Complete
            } else {
                nodes::LoadState::Failed
            },
        );
    }

    /// BufferAudioSource.loadData and AudioClip.loadData. `await this .setArrayBuffer (data)` (decodeAudioData)
    /// runs inside the FileLoader callback, so a file that does not decode rejects the candidate like a failed
    /// fetch: FileLoader reports it and tries the next URL. COMPLETE_STATE after the decode; no URL left:
    /// unloadData (AudioClip) or `setMediaElement (null)` (BufferAudioSource), FAILED_STATE.
    ///
    /// With an asynchronous [`Fetcher`] the chain waits for the responses of its candidates, also when it goes on
    /// after a candidate that did not decode ([`Runtime::buffer_load_waits`]).
    fn buffer_load(&mut self, node: NodeId) {
        let urls = self.url_strings(node);

        self.allow_fetch_wait();
        self.buffer_load_from(node, &urls, 0);
    }

    /// The fallback chain of [`Runtime::buffer_load`] from `urls`, after `attempts` failed candidates.
    ///
    /// decodeAudioData ends on the browser's decoder thread: the result of a fetched candidate waits in
    /// `pending_audio_decodes`, and the chain goes on from there. It is resolved after the events that are
    /// pending at the next prepareEvents (like an image decode, [`Runtime::apply_decoded_images`]), or, with
    /// `defer_audio_decodes`, at the start of the next frame ([`Runtime::flush_audio_decodes`]).
    fn buffer_load_from(&mut self, node: NodeId, urls: &[String], attempts: usize) {
        let loader = FileLoader {
            type_name: self.type_name(node),
            base: self.base_url(node),
            data_as_string: false,
        };
        let mut pending = None;

        let result = self.load_document_from(
            &loader,
            urls,
            &LoadHooks::default(),
            attempts,
            |_, i, loaded| match loaded {
                Loaded::Script(_) => Err("Error: No suitable file handler found.".into()),
                Loaded::Data(bytes, resolved) => {
                    // Every candidate before `i` failed.
                    pending = Some(PendingAudioDecode {
                        node,
                        bytes,
                        candidate: urls[i].clone(),
                        resolved,
                        rest: urls[i + 1..].to_vec(),
                        attempts: attempts + i,
                    });
                    Ok(())
                }
            },
        );

        match (pending, result) {
            (Some(p), _) => self.pending_audio_decodes.push(p),
            (None, LoadResult::Failed) => self.buffer_load_failed(node),
            (None, _) => {}
        }
    }

    /// `setArrayBuffer (data)` of the node; an error is the rejection of decodeAudioData.
    fn buffer_set_data(&mut self, node: NodeId, bytes: Vec<u8>) -> Result<(), String> {
        if self.builtin_name(node) == Some("AudioClip") {
            self.audio_clip_set_array_buffer(node, bytes)
        } else {
            self.buffer_set_array_buffer(node, bytes)
        }
    }

    /// After candidate `attempts` of `node` did not decode, FileLoader fetches the next one (`rest` are the candidates
    /// left); with an asynchronous [`Fetcher`] its response comes later. The load task then waits with the node's
    /// chain at that candidate, and goes on from there when the response is in ([`Runtime::resume_fetches`],
    /// [`Runtime::buffer_load`]). Returns whether it waits.
    fn buffer_load_waits(&mut self, node: NodeId, attempts: usize, rest: &[String]) -> bool {
        let urls = self.url_strings(node);
        let index = attempts + 1;

        // A chain of other candidates (the url changed) is not the node's current one.
        if !self.fetcher.is_async() || urls.get(index..) != Some(rest) {
            return false;
        }

        let key = FetchKey::Url(node, 0);

        self.fetch_resume.insert(
            key.clone(),
            FetchResume {
                index,
                attempts: index,
                urls,
            },
        );
        self.parked_fetches.extend(key.task());
        true
    }

    /// The FileLoader callback with `data === null`: every candidate failed.
    fn buffer_load_failed(&mut self, node: NodeId) {
        if self.builtin_name(node) == Some("AudioClip") {
            self.audio_clip_unload_data(node);
        } else {
            self.buffer_load_start(node);
        }

        self.set_load_state(node, nodes::LoadState::Failed);
    }

    /// The decodes held by `defer_audio_decodes`, resolved in request order at the start of a frame, then
    /// the tasks they queued.
    pub fn flush_audio_decodes(&mut self) {
        self.resolve_audio_decodes();
        self.run_tasks();
    }

    /// The end of `await this .setArrayBuffer (data)` for every pending decode, in request order: COMPLETE_STATE, or
    /// a rejection, which is FileLoader.loadDocumentError: the warning, then the next URL (whose decode is pending in
    /// turn), or FAILED_STATE when none is left.
    pub(crate) fn resolve_audio_decodes(&mut self) {
        for p in std::mem::take(&mut self.pending_audio_decodes) {
            if !self.sound_has_source(p.node) {
                continue;
            }

            match self.buffer_set_data(p.node, p.bytes) {
                Ok(()) => self.set_load_state(p.node, nodes::LoadState::Complete),
                Err(error) => {
                    let loader = FileLoader {
                        type_name: self.type_name(p.node),
                        base: self.base_url(p.node),
                        data_as_string: false,
                    };
                    self.print_load_error(
                        &loader,
                        &p.candidate,
                        Some(&ResolvedCandidate::plain(p.resolved.clone())),
                        &error,
                        p.attempts + 1,
                        p.rest.len(),
                    );

                    if p.rest.is_empty() {
                        self.buffer_load_failed(p.node);
                    } else if !self.buffer_load_waits(p.node, p.attempts, &p.rest) {
                        self.buffer_load_from(p.node, &p.rest, p.attempts + 1);
                    }
                }
            }
        }
    }

    /// MovieTexture.loadData: one attempt per URL, each with its own FileLoader (GIF/PNG) or a
    /// `<video>` element (anything else).
    ///
    /// A GIF is decoded as libgif does (`load_raw`; a file it cannot parse rejects with its on_error type). A PNG
    /// goes to apng-js parseAPNG: data that is not an animated PNG returns parseAPNG's error, which setAPNG, not
    /// awaited, rejects outside the try: the load ends without a warning and the node stays IN_PROGRESS. Video is
    /// not decoded: for video, headless Chromium fires `error` for a missing file and for formats it cannot play;
    /// an existing .webm/.ogv/.ogg/.mp4/.m4v is taken as playable (an assumption, not observed in the corpora).
    ///
    /// With an asynchronous [`Fetcher`] each candidate waits for its response (`await this .loadDocument (fileURL)`,
    /// the `<video>` element's request): the loop stops there and goes on from that candidate when it is in.
    fn movie_load(&mut self, node: NodeId) {
        let base = self.base_url(node);
        let Ok(base) = url::Url::parse(&base) else {
            return;
        };
        let urls = self.url_strings(node);

        // A newer load of the node takes over: a video element of an older one is not wanted any more.
        self.pending_videos.retain(|_, p| p.node != node);
        self.allow_fetch_wait();

        let wait = self.fetch_wait.take();
        let start = wait
            .as_ref()
            .and_then(|k| self.fetch_resume_of(k, &urls))
            .map_or(0, |r| r.index);

        for (index, candidate) in urls.iter().enumerate().skip(start) {
            // `new URL (url, base)` is outside the try: an invalid URL rejects loadData.
            let Ok(file_url) = join_url(&base, candidate) else {
                return;
            };

            let decoded =
                decode_uri(file_url.as_str()).unwrap_or_else(|| file_url.as_str().to_string());
            let path = file_url.path().to_string();
            let href = file_url.as_str();
            let gif = path.ends_with(".gif") || href.starts_with("data:image/gif");
            let png = path.ends_with(".png") || href.starts_with("data:image/png");

            // The page's video element requests a video itself: the engine has no response to wait for.
            if (gif || png || !self.fetcher.plays_video())
                && self.fetch_waits(wait.as_ref(), &urls, index, &file_url)
            {
                return;
            }

            let (name, error) = if gif || png {
                let loader = FileLoader {
                    type_name: "MovieTexture".into(),
                    base: href.to_string(),
                    data_as_string: false,
                };
                let mut data = None;
                let ok = self.load_document(&loader, &[href.to_string()], |_, loaded| {
                    if let Loaded::Data(bytes, _) = loaded {
                        data = Some(bytes);
                    }
                    Ok(())
                });

                use nodes::movie_texture::{DecodeError, MovieMedia};

                let media = match data.filter(|_| ok) {
                    Some(data) if gif => MovieMedia::decode_gif(&data),
                    Some(data) => MovieMedia::decode_apng(&data),
                    None => Err(DecodeError::Fail(
                        "Error",
                        "MovieTexture: No suitable file handler found.".into(),
                    )),
                };

                match media {
                    Ok(media) => return self.movie_set_frames(node, media),
                    Err(DecodeError::Fail(name, error)) => (name, error),
                    // The browser's out-of-memory error: IN_PROGRESS (not changed).
                    Err(DecodeError::TooLarge(error)) => {
                        self.log_error(format!("RangeError: {error}"));
                        return;
                    }
                }
            } else {
                if self.fetcher.plays_video() {
                    return self.movie_load_video(node, urls.clone(), index, file_url.clone());
                }

                // MPEG-1 (ISO/IEC 19775-1 18.4.2): decoded here; a failure is the usual unplayable-video error below.
                #[cfg(feature = "native-video")]
                if !self.mpeg1_unplayable
                    && crate::mpeg1::has_mpeg1_extension(&path)
                    && let Ok(data) = self.fetcher.fetch(&file_url)
                {
                    match nodes::movie_texture::MovieMedia::decode_mpeg1(&data) {
                        Ok(media) => return self.movie_set_frames(node, media),
                        Err(e) => self.log_warn(format!("MPEG-1 movie '{decoded}': {e}")),
                    }
                }

                let (playable, container) = video_source_kind(&path, href);

                if playable && let Ok(data) = self.fetcher.fetch(&file_url) {
                    use crate::video_types::WebmError;
                    use nodes::movie_texture::MovieMedia;

                    match container {
                        // Another playable file (Ogg) is taken as playing.
                        VideoContainer::Other => return,
                        // ADR 0015: an MP4 is decoded by the platform decoder of this build. Every failure is the video
                        // element's `error`: the reason, X_ITE's two messages, the next candidate, FAILED_STATE.
                        VideoContainer::Mp4 => match MovieMedia::decode_platform(&data) {
                            Ok(media) => return self.movie_set_frames(node, media),
                            Err(e) => self.log_warn(format!(
                                "MovieTexture: the video '{decoded}' is not decoded by this build ({e:?})."
                            )),
                        },
                        VideoContainer::Webm => match MovieMedia::decode_webm(&data) {
                            Ok(media) => return self.movie_set_frames(node, media),
                            // A video this build does not decode (another codec, a picture beyond the limits): it
                            // may play in the browser. No frame is shown, the scene goes on, the node stays
                            // IN_PROGRESS (as for every other undecoded video) and the log says why.
                            Err(
                                e @ (WebmError::NoVideoTrack
                                | WebmError::TooLarge
                                | WebmError::NoDecoder
                                | WebmError::Unsupported { .. }),
                            ) => {
                                self.log_warn(format!(
                                    "MovieTexture: the video '{decoded}' is not decoded by this build ({e:?}); no frame is shown."
                                ));
                                return;
                            }
                            // Not a WebM, or a stream that does not decode: the browser's video element fires
                            // `error`, and the next candidate is tried.
                            Err(WebmError::NotWebm | WebmError::Decode(_)) => {}
                        },
                    }
                }

                ("Error", format!("Couldn't load video '{href}': error."))
            };

            self.log_warn(format!("Error loading movie '{decoded}': {error}"));
            self.log_error(format!("{name}: {error}"));
        }

        self.movie_clear(node);
        self.set_load_state(node, nodes::LoadState::Failed);
    }

    /// X3DFontStyleNode.initialize: `family` and `style` request a new load (`set_url__`), and so does the
    /// initialize itself: loadData starts after `await $.sleep (0)`, a macrotask.
    pub(crate) fn font_style_initialize(&mut self, node: NodeId) {
        self.interest(node, "family", Cb::FontStyleReload);
        self.interest(node, "style", Cb::FontStyleReload);
        self.interest(node, "justify", Cb::FontStyleJustify);
        self.font_style_set_justify(node);
        self.font_style_request(node, false);
    }

    /// X3DFontStyleNode.set_justify__: the alignments are worked out from the fields as they are now and kept until `justify`
    /// changes again (a later change of `horizontal`, `leftToRight` or `topToBottom` does not call it).
    pub(crate) fn font_style_set_justify(&mut self, node: NodeId) {
        let a = self
            .text_font_style_params_for_alignments(node)
            .compute_alignments();

        self.font_style_alignments.insert(node, a);
    }

    /// X3DFontStyleNode.loadData (font selection only): the snapshot of `family` with "SERIF" appended.
    pub(crate) fn font_style_load(&mut self, node: NodeId) {
        let (family, style) = self.font_style_families(node);

        // browser.loadFont caches the default font file this load takes.
        if let Some(key) = family.iter().find_map(|f| {
            x3d_text::fonts::default_font_file(f, &style).map(|(name, _)| name.to_string())
        }) {
            self.loaded_font_files.insert(key);
        }

        self.font_style_snapshot
            .insert(node, (family.clone(), style.clone()));
        self.font_style_continue(node, family, 0, style);
    }

    /// The family loop of X3DFontStyleNode.loadData from entry `from`. Each family name is tried in order: a default
    /// family (SERIF, SANS, TYPEWRITER) is X_ITE's own font and always loads; then the FontLibrary table of the
    /// node's execution context, where X_ITE waits for as long as a FontLibrary is still loading (the loop is
    /// parked in `pending_font_styles` and goes on when a FontLibrary load ends); a family that looks like a font
    /// file URL is fetched through browser.loadFont (cached per URL); anything else warns.
    pub(crate) fn font_style_continue(
        &mut self,
        node: NodeId,
        family: Vec<String>,
        from: usize,
        style: String,
    ) {
        self.font_style_continue_from(node, family, from, style, false);
    }

    /// [`Runtime::font_style_continue`]; `fetched`: entry `from` is a font file whose response this loop waited for
    /// (its deprecation warning is out already).
    ///
    /// With an asynchronous [`Fetcher`] `await browser .loadFont (fileURL)` waits for the font file's response: the
    /// loop is parked in `font_fetch_waits` and goes on with that file when it is in ([`Runtime::resume_fetches`]).
    fn font_style_continue_from(
        &mut self,
        node: NodeId,
        family: Vec<String>,
        from: usize,
        style: String,
        fetched: bool,
    ) {
        let Ok(base) = url::Url::parse(&self.base_url(node)) else {
            return;
        };

        for (index, f) in family.iter().enumerate().skip(from) {
            if matches!(f.as_str(), "SERIF" | "SANS" | "TYPEWRITER") {
                return self.font_style_chose(node, None);
            }

            // `browser .getFont (executionContext, fontFamily)`: `this .font` is what this load found; a library that
            // registers later changes nothing until the next load.
            if let Some(font) = self.font_library_font(self.scene.node(node).ctx, f) {
                return self.font_style_chose(node, Some(font));
            }

            if self.font_library_loading() {
                self.pending_font_styles
                    .push(crate::font_library::PendingFontStyle {
                        node,
                        family: family.clone(),
                        index,
                        style,
                    });
                return;
            }

            // `new URL (fontFamily, base)` throws for an invalid URL: loadData rejects silently.
            let Ok(file_url) = join_url(&base, f) else {
                return;
            };

            let path = file_url.path().to_ascii_lowercase();
            let font_file = [".woff2", ".woff", ".otf", ".ttf"]
                .iter()
                .any(|e| path.ends_with(e));

            if file_url.scheme() == "data" || font_file {
                if !(fetched && index == from)
                    && self.scene.spec_version_of(self.scene.world) >= "4.1"
                {
                    self.log_warn("Loading a font file via family field is deprecated, please use new FontLibrary node instead.");
                }

                let href = file_url.as_str().to_string();

                if !self.font_cache.contains_key(&href)
                    && self.fetcher.is_async()
                    && file_url.scheme() != "data"
                    && self.fetcher.poll(&file_url) == FetchPoll::Pending
                {
                    self.font_fetch_waits
                        .push(crate::font_library::PendingFontStyle {
                            node,
                            family: family.clone(),
                            index,
                            style,
                        });
                    return;
                }

                let ok = match self.font_cache.get(&href) {
                    Some(ok) => *ok,
                    None => {
                        let loader = FileLoader {
                            type_name: self.type_name(node),
                            base: href.clone(),
                            data_as_string: false,
                        };
                        // browser.loadFont: `OpenType .parse (await decompressFont (data))`. A file that
                        // does not parse throws inside the loader's async callback, so the promise never
                        // resolves: the FontStyle stays without a font and no further family is tried.
                        let mut parsed: Option<
                            Result<std::rc::Rc<x3d_text::Font>, x3d_text::FontError>,
                        > = None;
                        let ok = self.load_document(
                            &loader,
                            std::slice::from_ref(&href),
                            |_, loaded| {
                                if let Loaded::Data(data, _) = loaded {
                                    // `OpenType .parse` throws inside the loader's callback: loadDocumentError, next url.
                                    match crate::font_library::parse_font_file(&data) {
                                        Ok(font) => parsed = Some(Ok(std::rc::Rc::new(font))),
                                        Err(e) => {
                                            return Err(crate::font_library::font_error_text(
                                                &data, &e,
                                            ));
                                        }
                                    }
                                }

                                Ok(())
                            },
                        );

                        if let Some(result) = parsed {
                            x3d_text::fonts::register_url_font(&href, result.ok());
                        }

                        if !ok && file_url.scheme() != "data" {
                            let decoded = decode_uri(&href).unwrap_or_else(|| href.clone());
                            self.log_warn(format!("Error loading font '{decoded}':"));
                        }

                        self.font_cache.insert(href, ok);
                        ok
                    }
                };

                if ok {
                    return self.font_style_chose(node, None);
                }
            } else {
                self.log_warn(format!(
                    "Couldn't find font family '{f}' with style '{style}'."
                ));
            }
        }

        self.font_style_chose(node, None);
    }
}

/// How a glTF buffer failed to load (GLTF2Parser.bufferObject, GLTF2Parser.js:602-619).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BufferFailure {
    /// `new URL (buffer .uri, base)` threw (:613): no FileLoader logs it, and `Promise .all`
    /// rejects with this error before any fetch has finished.
    InvalidUrl,
    /// The buffer's own FileLoader for the World (:614) failed; it logs the error itself.
    Fetch,
}

pub struct BufferFetchError {
    pub candidate: String,
    pub resolved: Option<url::Url>,
    pub error: String,
    pub kind: BufferFailure,
}

pub struct ParseBytesResult {
    pub scene: Result<Scene, x3d_parse::LoadError>,
    /// Every buffer that failed, in buffer order.
    pub buffer_errors: Vec<BufferFetchError>,
    /// A buffer's response is not in yet ([`parse_bytes_waiting`]): `scene` is no result, parse again later.
    pub buffers_pending: bool,
    /// The buffers (URI, resolved URL) whose response is not in yet, in buffer order.
    pub pending_buffers: Vec<(String, url::Url)>,
}

impl Runtime {
    /// GLTF2Parser for the candidate `data` of the running fallback chain (the `accept` of [`Runtime::load_document`]
    /// and [`Runtime::load_page_document`]): its buffers load through this runtime's fetcher. While a buffer's
    /// response is not in, the result has `buffers_pending`, and the chain waits for it when the caller's `accept`
    /// rejects with no error (`await buffersArray`: the scene is built when the buffers are in).
    pub fn parse_candidate(&mut self, data: &[u8], url: &str) -> ParseBytesResult {
        // SVGParser reads the options of its own browser: this runtime's, not the last one that parsed on this thread.
        self.sync_svg_quality();

        let parsed = parse_bytes_waiting(&*self.fetcher, data, url, self.accept_waits, true);

        if parsed.buffers_pending {
            self.buffers_pending = true;
        }

        parsed
    }

    /// The rejection of a candidate whose glTF `failures` made GLTF2Parser throw `message`, or `None` when `message`
    /// is no buffer's error. The buffer FileLoaders' own messages come after the candidate's result.
    pub fn gltf_buffer_rejection(
        &mut self,
        failures: Vec<BufferFetchError>,
        message: &str,
    ) -> Option<Option<String>> {
        if !failures.iter().any(|f| f.error == message) {
            return None;
        }

        // GLTF2Parser.js:600, `Promise .all`: an invalid buffer URL rejects it at once with that error, which the
        // candidate's FileLoader logs. Otherwise the first buffer FileLoader to fail rejects it with no error
        // (FileLoader.js:26), and the candidate's FileLoader logs nothing (FileLoader.js:382-383).
        let thrown = failures
            .iter()
            .find(|f| f.kind == BufferFailure::InvalidUrl)
            .map(|f| f.error.clone());

        // Each buffer FileLoader logs its own failure when its fetch fails. A data URL is decoded without a request,
        // so it fails before any requested file (recorded order in inline-buffer-errors-baseline.json); otherwise in
        // buffer order.
        let mut logged: Vec<_> = failures
            .into_iter()
            .filter(|f| f.kind == BufferFailure::Fetch)
            .collect();

        logged.sort_by_key(|f| !f.resolved.as_ref().is_some_and(|r| r.scheme() == "data"));
        self.deferred_buffer_errors.extend(logged);

        Some(thrown)
    }

    /// The rejection of a page candidate whose glTF has a buffer that failed while others are still in flight, or
    /// `None` when no buffer failed yet (the chain waits for them). `Promise .all` (GLTF2Parser.js:600) rejects at the
    /// first buffer that fails, so the candidate does not wait for the others. Their World FileLoaders are not
    /// aborted: they are followed ([`Runtime::follow_buffers`]).
    pub fn gltf_buffer_failed_first(
        &mut self,
        parsed: &mut ParseBytesResult,
    ) -> Option<Option<String>> {
        if !parsed.buffers_pending || parsed.buffer_errors.is_empty() {
            return None;
        }

        let message = parsed.buffer_errors[0].error.clone();

        self.buffers_pending = false;
        self.followed_buffers
            .extend(std::mem::take(&mut parsed.pending_buffers));
        self.gltf_buffer_rejection(std::mem::take(&mut parsed.buffer_errors), &message)
    }

    /// The followed buffers ([`Runtime::followed_buffers`]) whose response came: each that failed logs its
    /// FileLoader's message, as GLTF2Parser's buffer FileLoaders do after their candidate was rejected. Returns
    /// whether some are still in flight.
    pub fn follow_buffers(&mut self) -> bool {
        let world = FileLoader {
            type_name: String::new(),
            base: String::new(),
            data_as_string: false,
        };

        for (candidate, resolved) in std::mem::take(&mut self.followed_buffers) {
            if self.fetcher.poll(&resolved) == FetchPoll::Pending {
                self.followed_buffers.push((candidate, resolved));
            } else if let Some(e) = self.fetcher.fetch_error(&resolved) {
                self.print_load_error(
                    &world,
                    &candidate,
                    Some(&ResolvedCandidate::plain(resolved)),
                    &e,
                    1,
                    0,
                );
            }
        }

        !self.followed_buffers.is_empty()
    }
}

/// `x3d_parse::parse_bytes`, with GLTF2Parser / GLB2Parser loading the buffers of a glTF through
/// `fetcher` (FileLoader resolves them against the scene's url).
///
/// The world has no runtime yet, so it has no browser options of its own: the SVG parser takes the defaults
/// (MEDIUM, MEDIUM), not what another browser on this thread set last.
pub fn parse_bytes_with_fetcher(fetcher: &dyn Fetcher, data: &[u8], url: &str) -> ParseBytesResult {
    x3d_parse::reset_svg_quality();
    parse_bytes_waiting(fetcher, data, url, false, false)
}

/// [`parse_bytes_with_fetcher`]; with `wait`, a buffer whose response an asynchronous `fetcher` has not given yet
/// is requested and the result has `buffers_pending`. Every buffer is asked for (`Promise .all`), so their
/// requests go out together. `gunzipped`: `data` is what FileLoader's `$.gunzip` returned, so the parsers do not gunzip
/// it again.
pub(crate) fn parse_bytes_waiting(
    fetcher: &dyn Fetcher,
    data: &[u8],
    url: &str,
    wait: bool,
    gunzipped: bool,
) -> ParseBytesResult {
    let wait = wait && fetcher.is_async();
    let pending = std::cell::RefCell::new(Vec::new());
    let buffer_errors = std::cell::RefCell::new(Vec::new());
    let failed = |uri: &str, resolved: Option<url::Url>, error: &str, kind: BufferFailure| {
        buffer_errors.borrow_mut().push(BufferFetchError {
            candidate: uri.to_string(),
            resolved,
            error: error.to_string(),
            kind,
        });
    };

    let fetch = |uri: &str, base: &str| -> Result<Vec<u8>, String> {
        let resolved_res = url::Url::parse(base)
            .and_then(|b| join_url(&b, uri.trim()))
            .map_err(|_| "TypeError: Failed to construct 'URL': Invalid URL".to_string());

        if x3d_parse::is_data_url(uri) {
            let res = x3d_parse::gltf_default_fetch(uri, base);
            if let Err(e) = &res {
                failed(uri, resolved_res.ok(), e, BufferFailure::Fetch);
            }
            return res;
        }

        let resolved = match resolved_res {
            Ok(r) => r,
            Err(e) => {
                failed(uri, None, &e, BufferFailure::InvalidUrl);
                return Err(e);
            }
        };

        if wait && fetcher.poll(&resolved) == FetchPoll::Pending {
            pending.borrow_mut().push((uri.to_string(), resolved));
            return Err("TypeError: Failed to fetch".to_string());
        }

        fetcher.fetch(&resolved).inspect_err(|e| {
            failed(uri, Some(resolved.clone()), e, BufferFailure::Fetch);
        })
    };

    let scene = x3d_parse::with_gltf_fetcher(&fetch, || {
        if gunzipped {
            x3d_parse::parse_bytes_gunzipped(data, url)
        } else {
            x3d_parse::parse_bytes(data, url)
        }
    });

    let pending_buffers = pending.into_inner();

    if !pending_buffers.is_empty() {
        // The messages of this parse are not the result: the next parse gives them.
        x3d_parse::take_error_warnings();
        x3d_parse::take_error_detail();
    }

    ParseBytesResult {
        scene,
        buffer_errors: buffer_errors.into_inner(),
        buffers_pending: !pending_buffers.is_empty(),
        pending_buffers,
    }
}

/// The container of a MovieTexture video source, as far as the native build decodes it.
#[derive(Clone, Copy, Debug, PartialEq)]
enum VideoContainer {
    Webm,
    /// MP4, M4V or M4A (ISO/IEC 14496-12, ADR 0015).
    Mp4,
    /// Another file the page's element may play (Ogg), or a file that it does not play.
    Other,
}

/// What a MovieTexture source that is not a GIF or a PNG is, from its URL: (a video the page's element may play, its
/// container). `path` and `href` are the path and the text of the parsed URL, so the scheme is lower case. A data URL
/// has no file name: its media type says what it is (a glTF EXT_texture_video source in a data URL or in a buffer view
/// of a GLB, which the pinned parser gives to the node as a data URL). Any other URL is known by its extension.
fn video_source_kind(path: &str, href: &str) -> (bool, VideoContainer) {
    let (playable, container) = match href.strip_prefix("data:") {
        Some(rest) => {
            let media_type = rest
                .split([';', ','])
                .next()
                .unwrap_or("")
                .trim()
                .to_ascii_lowercase();
            let container = match media_type.as_str() {
                "video/webm" => VideoContainer::Webm,
                "video/mp4" | "audio/mp4" => VideoContainer::Mp4,
                _ => VideoContainer::Other,
            };

            (
                ["video/webm", "video/ogg", "video/mp4", "audio/mp4"]
                    .contains(&media_type.as_str()),
                container,
            )
        }
        None => {
            let lower = path.to_ascii_lowercase();
            let container = if lower.ends_with(".webm") {
                VideoContainer::Webm
            } else if [".mp4", ".m4v", ".m4a"].iter().any(|e| lower.ends_with(e)) {
                VideoContainer::Mp4
            } else {
                VideoContainer::Other
            };

            (
                [".webm", ".ogv", ".ogg", ".mp4", ".m4v", ".m4a"]
                    .iter()
                    .any(|e| lower.ends_with(e)),
                container,
            )
        }
    };

    (
        playable,
        if playable {
            container
        } else {
            VideoContainer::Other
        },
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The kind of a video source: the media type of a data URL (any case, with or without parameters), the extension of
    /// any other URL. The URL is parsed first, as the loader does, so an upper-case scheme is a data URL too.
    #[test]
    fn video_source_kind_reads_a_data_url_media_type_and_a_file_extension() {
        let kind = |text: &str| {
            let u = url::Url::parse(text).unwrap();

            video_source_kind(u.path(), u.as_str())
        };

        assert_eq!(
            kind("data:video/webm;base64,GkXfow=="),
            (true, VideoContainer::Webm)
        );
        assert_eq!(
            kind("DATA:Video/WebM;base64,GkXfow=="),
            (true, VideoContainer::Webm)
        );
        assert_eq!(kind("data:video/webm,x"), (true, VideoContainer::Webm));
        assert_eq!(
            kind("data:video/webm;codecs=vp9;base64,GkXfow=="),
            (true, VideoContainer::Webm)
        );
        assert_eq!(
            kind("data:video/mp4;base64,AAAA"),
            (true, VideoContainer::Mp4)
        );
        assert_eq!(
            kind("data:audio/mp4;base64,AAAA"),
            (true, VideoContainer::Mp4)
        );
        assert_eq!(
            kind("data:video/ogg;base64,AAAA"),
            (true, VideoContainer::Other)
        );
        assert_eq!(
            kind("data:application/octet-stream;base64,GkXfow=="),
            (false, VideoContainer::Other)
        );
        assert_eq!(kind("data:,x"), (false, VideoContainer::Other));
        // The data of a data URL is not a file name.
        assert_eq!(
            kind("data:text/plain,a.webm"),
            (false, VideoContainer::Other)
        );
        assert_eq!(
            kind("http://oracle/media/a.webm"),
            (true, VideoContainer::Webm)
        );
        assert_eq!(
            kind("http://oracle/media/A.WEBM?x=1"),
            (true, VideoContainer::Webm)
        );
        assert_eq!(
            kind("http://oracle/media/a.mp4"),
            (true, VideoContainer::Mp4)
        );
        assert_eq!(
            kind("http://oracle/media/a.m4v"),
            (true, VideoContainer::Mp4)
        );
        assert_eq!(
            kind("http://oracle/media/a.M4A"),
            (true, VideoContainer::Mp4)
        );
        assert_eq!(
            kind("http://oracle/media/a.ogv"),
            (true, VideoContainer::Other)
        );
        assert_eq!(
            kind("http://oracle/media/a.mov"),
            (false, VideoContainer::Other)
        );
        assert_eq!(
            kind("http://oracle/media/webm"),
            (false, VideoContainer::Other)
        );
    }

    /// LocalFetcher answers as the case-sensitive oracle server on every file system: a name that differs only in case is
    /// "Not Found", in a file name and in a directory name (macOS run of PR #435: `text/PLAIN.wrl` was read as `plain.wrl`).
    #[test]
    fn local_fetcher_names_are_case_sensitive() {
        let root =
            std::env::temp_dir().join(format!("x3d-localfetcher-case-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&root);
        std::fs::create_dir_all(root.join("text")).unwrap();
        std::fs::write(root.join("text/plain.wrl"), b"#VRML V2.0 utf8\n").unwrap();

        let fetcher = LocalFetcher {
            root: root.clone(),
            prefix: "http://oracle/".into(),
        };
        let get = |p: &str| fetcher.fetch(&url::Url::parse(&format!("http://oracle/{p}")).unwrap());
        let results = (
            get("text/plain.wrl"),
            get("text/PLAIN.wrl"),
            get("TEXT/plain.wrl"),
            get("text/missing.wrl"),
        );

        std::fs::remove_dir_all(&root).unwrap();
        assert_eq!(results.0, Ok(b"#VRML V2.0 utf8\n".to_vec()));
        assert_eq!(results.1, Err("Error: Not Found".to_string()));
        assert_eq!(results.2, Err("Error: Not Found".to_string()));
        assert_eq!(results.3, Err("Error: Not Found".to_string()));
    }

    /// The credential rule is the runtime's, not one fetcher's: a fetcher that must not run for a URL with a user name or a
    /// password is not called (PR #434 review). `data:` URLs have no credentials to refuse.
    #[test]
    fn url_rejection_refuses_credentials_before_any_fetcher_runs() {
        struct Panics;

        impl Fetcher for Panics {
            fn fetch(&self, _: &url::Url) -> Result<Vec<u8>, String> {
                panic!("the fetcher ran");
            }
        }

        let scene = x3d_parse::parse_bytes(b"#VRML V2.0 utf8\n", "http://oracle/t.wrl").unwrap();
        let rt = Runtime::new(scene, Box::new(Panics));

        for (url, rejected) in [
            ("https://user:pw@example.invalid/x", true),
            ("https://user@example.invalid/x", true),
            ("foo://:pw@example.invalid/x", true),
            ("https://example.invalid/x", false),
            ("data:text/plain,a", false),
        ] {
            let url = url::Url::parse(url).unwrap();
            let got = rt.url_rejection(&ResolvedCandidate::plain(url.clone()));

            assert_eq!(got.is_some(), rejected, "{url}");

            if rejected {
                assert!(got.unwrap().contains("includes credentials"), "{url}");
            }
        }
    }

    /// A candidate with a space in its host and, later, a candidate that is exactly the stand-in URL of the first: the first is a
    /// space host (rejected before any fetcher), the second is an ordinary URL that reaches the fetcher. The runtime keeps no
    /// table of stand-in URLs, so the second cannot be taken for the first (PR #434 review).
    #[test]
    fn a_candidate_equal_to_a_stand_in_url_is_an_ordinary_candidate() {
        use std::cell::RefCell;
        use std::rc::Rc;

        struct Records(Rc<RefCell<Vec<String>>>);

        impl Fetcher for Records {
            fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
                self.0.borrow_mut().push(url.to_string());
                Err("Error: Not Found".into())
            }
        }

        let fetched = Rc::new(RefCell::new(vec![]));
        let scene = x3d_parse::parse_bytes(b"#VRML V2.0 utf8\n", "http://oracle/t.wrl").unwrap();
        let mut rt = Runtime::new(scene, Box::new(Records(fetched.clone())));
        let base = url::Url::parse("http://oracle/t.wrl").unwrap();
        let spaced = rt
            .resolve_candidate(&base, "https://exa mple.invalid/x.wrl")
            .unwrap();

        assert!(spaced.space_host);
        assert_eq!(spaced.href, "https://exa%20mple.invalid/x.wrl");
        assert!(rt.url_rejection(&spaced).is_some());

        // The text of the stand-in URL, as a candidate of its own.
        let same_text = rt.resolve_candidate(&base, spaced.url.as_str()).unwrap();

        assert_eq!(same_text.url, spaced.url);
        assert!(!same_text.space_host);
        assert_eq!(same_text.href, same_text.url.as_str());
        assert_eq!(rt.url_rejection(&same_text), None);

        // Through the FileLoader chain: the first candidate never reaches the fetcher, the second does.
        let loader = FileLoader {
            type_name: String::new(),
            base: base.to_string(),
            data_as_string: true,
        };
        let urls = vec![
            "https://exa mple.invalid/x.wrl".to_string(),
            spaced.url.to_string(),
        ];

        rt.load_document(&loader, &urls, |_, _| Ok(()));
        assert_eq!(*fetched.borrow(), [spaced.url.to_string()]);
        // The log names Chromium's href for the first.
        assert!(
            rt.log
                .iter()
                .any(|l| l.contains("URL 'https://exa mple.invalid/x.wrl', trying URL 2 of 2")),
            "{:?}",
            rt.log
        );
    }

    /// Chromium 156.0.8078.47 `new URL (input, "http://127.0.0.1:8000/a/b.wrl") .href` for a space in the host (measured with
    /// the browser of the pinned recordings): the `url` crate rejects each of these.
    #[test]
    fn space_host_url_gives_the_href_of_chromium() {
        let base = url::Url::parse("http://127.0.0.1:8000/a/b.wrl").unwrap();

        for (input, href) in [
            (
                "https://exa mple.invalid/x.wrl",
                "https://exa%20mple.invalid/x.wrl",
            ),
            (
                "https://exa%20mple.invalid/x.wrl",
                "https://exa%20mple.invalid/x.wrl",
            ),
            (
                "https://exa%C2%A0mple.invalid/x",
                "https://exa%20mple.invalid/x",
            ),
            (
                "https://exa  mple.invalid/x",
                "https://exa%20%20mple.invalid/x",
            ),
            ("http:// a/x", "http://%20a/x"),
            ("http://a /x", "http://a%20/x"),
            ("http://a b.c:80/x", "http://a%20b.c/x"),
            ("ws://a b:81/x?q=1#f", "ws://a%20b:81/x?q=1#f"),
            ("ftp://a b/", "ftp://a%20b/"),
            ("file://a b/x", "file://a%20b/x"),
            ("http://a:b@c d/x", "http://a:b@c%20d/x"),
            (
                "https://us er:pw@exa mple.invalid/x",
                "https://us%20er:pw@exa%20mple.invalid/x",
            ),
            ("HTTPS://EXA MPLE.INVALID/X", "https://exa%20mple.invalid/X"),
            ("http://a\tb c/", "http://ab%20c/"),
            (
                "https://exa\u{a0}mple.invalid/x",
                "https://exa%20mple.invalid/x",
            ),
            (
                "https://exa\u{3000}mple.invalid/x",
                "https://exa%20mple.invalid/x",
            ),
            (
                "https://exa\u{2003}mple.invalid/x",
                "https://exa%20mple.invalid/x",
            ),
            ("//exa mple.invalid/x", "http://exa%20mple.invalid/x"),
            ("//a b", "http://a%20b/"),
            ("\\\\exa mple.invalid/x", "http://exa%20mple.invalid/x"),
            (
                "https:exa mple.invalid/x.wrl",
                "https://exa%20mple.invalid/x.wrl",
            ),
            (
                "https:/exa mple.invalid/x.wrl",
                "https://exa%20mple.invalid/x.wrl",
            ),
            ("http:\\\\exa mple\\x", "http://exa%20mple/x"),
            // The label is encoded after the space became `%20`.
            (
                "https://ex\u{e4} mple.invalid/x",
                "https://xn--ex%20mple-w2a.invalid/x",
            ),
            ("https://a \u{e4}.invalid/", "https://xn--a%20-ooa.invalid/"),
            ("https://\u{e4} .invalid/x", "https://xn--%20-pla.invalid/x"),
        ] {
            assert!(join_url(&base, input).is_err(), "{input:?}");

            let space_host = space_host_url(&base, input).unwrap_or_else(|| panic!("{input:?}"));

            assert_eq!(space_host.href, href, "{input:?}");
            assert_eq!(decode_uri(&space_host.href).unwrap().contains("%20"), false);
        }
    }

    /// The same Chromium rejects these (or reads them as a path): no stand-in, and `join_url` is the answer.
    #[test]
    fn space_host_url_is_none_when_chromium_fails_or_the_space_is_not_in_the_host() {
        let base = url::Url::parse("http://127.0.0.1:8000/a/b.wrl").unwrap();

        for input in [
            "https://exa%2520mple.invalid/x",
            "http://a%2Fb c/x",
            "http://a%00b c/x",
            "http://[::1 ]/x",
            "http://a:80 b/x",
            "http://a b:8o/x",
            "https://exa<mple.invalid/x",
            "https://exa mple<.invalid/x",
            // Chromium: https://xn--a%20-wia.invalid/ and https://xn--%20x-koa.invalid/ (not reproduced)
            "https://a\u{a0}\u{e4}.invalid/",
            "https://\u{e4}\u{a0}x.invalid/",
            "foo://exa mple.invalid/x",
            "blob:https://exa mple.invalid/x",
            "https://user:pw@exa.invalid/x",
            "a b/c",
            "http:exa mple/x",
            "http:/exa mple/x",
            "text/with space.wrl",
            "https://exa.invalid/with space",
            "",
        ] {
            assert_eq!(space_host_url(&base, input), None, "{input:?}");
        }
    }

    /// Differences between Chromium 156.0.8078.47 and the `url` crate in the host that are not fixed (measured, REA
    /// sub:io.file-loader): the test fails when the crate's answer changes. Chromium escapes `*` in a host (`%2A`), which only
    /// shows in the `href` of a `foreign` target; and it rejects `#` and `?` in the host of a `file:` URL, where the
    /// crate starts the fragment and the query.
    #[test]
    fn host_differences_that_are_not_fixed_are_pinned() {
        let base = url::Url::parse("http://127.0.0.1:8000/a/b.wrl").unwrap();

        assert_eq!(
            join_url(&base, "https://a*b.invalid/x").unwrap().as_str(),
            "https://a*b.invalid/x" // Chromium: https://a%2Ab.invalid/x
        );
        assert_eq!(
            join_url(&base, "file://a#b.invalid/x").unwrap().as_str(),
            "file://a/#b.invalid/x" // Chromium: Invalid URL
        );
        assert_eq!(
            join_url(&base, "file://a?b.invalid/x").unwrap().as_str(),
            "file://a/?b.invalid/x" // Chromium: Invalid URL
        );
    }

    // The prefixes of the scripturl01 recording: JavaScript `\s` (U+FEFF in, U+0085 out) and three that are not.
    // The same table is in x3d-runtime/src/loader.rs and x3d-scene/src/xml.rs, so the two `is_js_space` cannot drift.
    const JS_SPACE: [(char, bool); 11] = [
        (' ', true),
        ('\t', true),
        ('\u{FEFF}', true),
        ('\u{00A0}', true),
        ('\u{2028}', true),
        ('\u{3000}', true),
        ('\u{2003}', true),
        ('\u{0085}', false),
        ('\u{200B}', false),
        ('\u{180E}', false),
        ('x', false),
    ];

    #[test]
    fn js_space_table() {
        for (c, space) in JS_SPACE {
            assert_eq!(is_js_space(c), space, "{c:?}");
        }
    }

    #[test]
    fn test_script_url_whitespace() {
        assert!(is_script_url("\u{FEFF}javascript:"));
        assert!(!is_script_url("\u{0085}javascript:"));
        assert!(!is_script_url("\u{200B}javascript:"));
        assert!(!is_script_url("\u{180E}javascript:"));
        assert!(is_script_url("\u{00A0}javascript:"));
        assert!(is_script_url("\u{2028}javascript:"));
        assert!(is_script_url("\u{3000}javascript:"));
        assert!(is_script_url(" \t\n\rjavascript:"));
    }
}
