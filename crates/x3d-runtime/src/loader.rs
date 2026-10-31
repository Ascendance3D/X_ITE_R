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

    /// A FileLoader of its own requests `url` (the page's createVrmlFromURL, see `browser_load.rs`): an asynchronous fetcher
    /// starts a request for it even when it holds a response or one is in flight; the response that comes replaces the one
    /// it holds.
    fn refresh(&self, _url: &url::Url) {}
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

        match std::fs::read(&path) {
            Ok(data) => Ok(data),
            Err(_) => Err("Error: Not Found".into()),
        }
    }
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
            let mut resolved: Option<url::Url> = None;
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
                let r = join_url(&base, candidate).map_err(|_| {
                    Some(format!("TypeError: Failed to construct 'URL': Invalid URL"))
                })?;
                resolved = Some(r.clone());

                // `if (this .dataAsString)`: a data URL that is not base64 is decoded here and handed to the callback as text.
                if loader.data_as_string {
                    if let Some(text) = data_url_text(candidate) {
                        return accept(self, i, Loaded::Data(text.into_bytes(), r));
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

                    if !target.is_empty() && target != "_self" {
                        special = Some(LoadResult::Foreign(r.as_str().to_string(), target));
                        return Ok(());
                    }

                    if r.scheme() != "data"
                        && ["html", "htm", "xhtml"]
                            .iter()
                            .any(|e| r.as_str().ends_with(&format!(".{e}")))
                    {
                        special = Some(LoadResult::Foreign(r.as_str().to_string(), target));
                        return Ok(());
                    }
                }

                // A base64 data URL is fetched like any other (the browser decodes it); one without a comma is no URL fetch can read.
                if loader.data_as_string && r.scheme() == "data" && !candidate.contains(',') {
                    return Err(Some("TypeError: Failed to fetch".to_string()));
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
                    self.fetcher.fetch(&r).map_err(Some)?
                };

                self.buffers_pending = false;
                let outer = std::mem::replace(&mut self.accept_waits, wait.is_some());
                let accepted = accept(self, i, Loaded::Data(x3d_parse::gunzip(&data), r));
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
                            b.resolved.as_ref(),
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

    pub(crate) fn format_load_subject(candidate: &str, resolved: Option<&url::Url>) -> String {
        let data_url = resolved.is_some_and(|r| r.scheme() == "data");
        let resolved_text = match resolved {
            Some(r) if !data_url => {
                decode_uri(r.as_str()).unwrap_or_else(|| r.as_str().to_string())
            }
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
        resolved: Option<&url::Url>,
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
            Task::UrlLoad(node, _) => {
                if node as usize >= self.scene.nodes.len() || self.scene.free_nodes.contains(&node)
                {
                    return true;
                }

                (self.base_url(node), self.url_strings(node), None)
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
                        Some(&p.resolved),
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

            if self.fetch_waits(wait.as_ref(), &urls, index, &file_url) {
                return;
            }

            let decoded =
                decode_uri(file_url.as_str()).unwrap_or_else(|| file_url.as_str().to_string());
            let path = file_url.path().to_string();
            let href = file_url.as_str();
            let gif = path.ends_with(".gif") || href.starts_with("data:image/gif");
            let png = path.ends_with(".png") || href.starts_with("data:image/png");

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
                    // setAPNG's unhandled rejection, or the browser's out-of-memory error: IN_PROGRESS.
                    Err(DecodeError::NotAnimated) => return,
                    Err(DecodeError::TooLarge(error)) => {
                        self.log_error(format!("RangeError: {error}"));
                        return;
                    }
                }
            } else {
                let playable = [".webm", ".ogv", ".ogg", ".mp4", ".m4v"]
                    .iter()
                    .any(|e| path.to_ascii_lowercase().ends_with(e));

                if playable && self.fetcher.fetch(&file_url).is_ok() {
                    return;
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
                                        Err(_) => {
                                            return Err(crate::font_library::font_error_text(
                                                &data,
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
        let parsed = parse_bytes_waiting(&*self.fetcher, data, url, self.accept_waits);

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
                self.print_load_error(&world, &candidate, Some(&resolved), &e, 1, 0);
            }
        }

        !self.followed_buffers.is_empty()
    }
}

/// `x3d_parse::parse_bytes`, with GLTF2Parser / GLB2Parser loading the buffers of a glTF through
/// `fetcher` (FileLoader resolves them against the scene's url).
pub fn parse_bytes_with_fetcher(fetcher: &dyn Fetcher, data: &[u8], url: &str) -> ParseBytesResult {
    parse_bytes_waiting(fetcher, data, url, false)
}

/// [`parse_bytes_with_fetcher`]; with `wait`, a buffer whose response an asynchronous `fetcher` has not given yet
/// is requested and the result has `buffers_pending`. Every buffer is asked for (`Promise .all`), so their
/// requests go out together.
pub(crate) fn parse_bytes_waiting(
    fetcher: &dyn Fetcher,
    data: &[u8],
    url: &str,
    wait: bool,
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

    let scene = x3d_parse::with_gltf_fetcher(&fetch, || x3d_parse::parse_bytes(data, url));

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

#[cfg(test)]
mod tests {
    use super::*;

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
