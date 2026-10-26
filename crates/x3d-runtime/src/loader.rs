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
fn is_script_url(candidate: &str) -> bool {
    let trimmed = candidate.trim_start();

    ["ecmascript:", "javascript:", "vrmlscript:"]
        .iter()
        .any(|s| trimmed.starts_with(s))
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
            return x3d_parse::gltf_default_fetch(href, "");
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
    const RESERVED: &[u8] = b";/?:@&=+$,#";
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
            u8::from_str_radix(std::str::from_utf8(h).ok()?, 16).ok()
        };

        let b0 = hex(i + 1)?;

        if b0 < 0x80 {
            if RESERVED.contains(&b0) {
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

/// FileLoader.loadDocumentAsync, the `dataAsString` branch: `url .match (/^\s*data:(.*?)(?:;charset=(.*?))?(?:;(base64))?,/s)`;
/// unless the header ends with `;base64`, the text after the first comma is `decodeURIComponent`ed (kept as it is when
/// that fails) and a leading `ï»¿` (a UTF-8 BOM read as Latin-1) is removed.
pub(crate) fn data_url_text(candidate: &str) -> Option<String> {
    let rest = candidate.trim_start().strip_prefix("data:")?;
    let (header, data) = rest.split_once(',')?;

    if header.ends_with(";base64") {
        return None;
    }

    let decoded = percent_encoding::percent_decode_str(data)
        .decode_utf8()
        .map(|d| d.into_owned())
        .unwrap_or_else(|_| data.to_string());

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
}

/// The decoded fragment of `candidate` if FileLoader's `bindViewpoint` branch takes it: a URL that, resolved against
/// `base`, differs from the referring document only by a non-empty fragment.
pub fn viewpoint_fragment(base: &str, candidate: &str, hooks: &LoadHooks) -> Option<String> {
    if !hooks.bind_viewpoint || candidate.is_empty() {
        return None;
    }

    let referer = url::Url::parse(base).ok()?;
    let r = referer.join(candidate).ok()?;

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

        if urls.is_empty() {
            // loadDocumentError () without an error prints nothing.
            return LoadResult::Failed;
        }

        for (i, candidate) in urls.iter().enumerate() {
            let remaining = urls.len() - i - 1;
            let mut resolved: Option<url::Url> = None;

            let result: Result<(), Option<String>> = (|| {
                if candidate.is_empty() {
                    return Err(Some("Error: Empty URL.".to_string()));
                }

                // Script URLs: /^\s*(?:ecmascript|javascript|vrmlscript):/s
                let trimmed = candidate.trim_start();
                for scheme in ["ecmascript:", "javascript:", "vrmlscript:"] {
                    if trimmed.starts_with(scheme) {
                        let source = &trimmed[scheme.len()..];
                        return accept(self, i, Loaded::Script(source.to_string()));
                    }
                }

                let base =
                    url::Url::parse(&loader.base).map_err(|e| Some(format!("TypeError: {e}")))?;
                let r = base.join(candidate).map_err(|_| {
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

                // fetch (dataURL) of a ShaderPart: the shader text is carried in the URL (nodes/shaders.rs).
                let data = if loader.type_name == "ShaderPart" && r.scheme() == "data" {
                    nodes::shaders::fetch_data_url(candidate).map_err(Some)?
                } else {
                    self.fetcher.fetch(&r).map_err(Some)?
                };
                accept(self, i, Loaded::Data(x3d_parse::gunzip(&data), r))
            })();

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
        // `world`: the parameters of a loadURL or Anchor load, whose foreign targets are never fetched.
        let (base, urls, world) = match *task {
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
                    self.scene.world_url_of(req.parent).to_string(),
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

        // load_document_from_opt's foreign rule: a `target` other than `_self` goes to another window unfetched.
        if let Some(parameters) = &world {
            let target = loader_target(parameters);

            if !target.is_empty() && target != "_self" {
                return true;
            }
        }

        for candidate in &urls {
            if candidate.is_empty() || is_script_url(candidate) {
                continue;
            }

            let Ok(r) = base.join(candidate) else {
                continue;
            };

            if r.scheme() == "data" {
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
        if self.parked_fetches.is_empty() {
            return;
        }

        for task in std::mem::take(&mut self.parked_fetches).into_iter().rev() {
            self.fetches.push_front(task);
        }

        self.run_tasks();
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
        let ok = self.load_document(&loader, &urls, |rt, loaded| match loaded {
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
        });

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
    fn buffer_load(&mut self, node: NodeId) {
        let urls = self.url_strings(node);
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
                    } else {
                        self.buffer_load_from(p.node, &p.rest, p.attempts + 1);
                    }
                }
            }
        }
    }

    /// MovieTexture.loadData: one attempt per URL, each with its own FileLoader (GIF/PNG) or a
    /// `<video>` element (anything else).
    ///
    /// Decoding is not ported. A fetched GIF/PNG is taken as loaded. For video, headless
    /// Chromium fires `error` for a missing file and for formats it cannot play; an existing
    /// .webm/.ogv/.ogg/.mp4/.m4v is taken as playable (an assumption, not observed in the corpora).
    fn movie_load(&mut self, node: NodeId) {
        let base = self.base_url(node);
        let Ok(base) = url::Url::parse(&base) else {
            return;
        };

        for candidate in self.url_strings(node) {
            // `new URL (url, base)` is outside the try: an invalid URL rejects loadData.
            let Ok(file_url) = base.join(&candidate) else {
                return;
            };
            let decoded =
                decode_uri(file_url.as_str()).unwrap_or_else(|| file_url.as_str().to_string());
            let path = file_url.path().to_string();
            let href = file_url.as_str();
            let image = path.ends_with(".gif")
                || path.ends_with(".png")
                || href.starts_with("data:image/gif")
                || href.starts_with("data:image/png");

            let error = if image {
                let loader = FileLoader {
                    type_name: "MovieTexture".into(),
                    base: href.to_string(),
                    data_as_string: false,
                };
                let ok = self.load_document(&loader, &[href.to_string()], |_, _| Ok(()));

                if ok {
                    return;
                }

                "MovieTexture: No suitable file handler found.".to_string()
            } else {
                let playable = [".webm", ".ogv", ".ogg", ".mp4", ".m4v"]
                    .iter()
                    .any(|e| path.to_ascii_lowercase().ends_with(e));

                if playable && self.fetcher.fetch(&file_url).is_ok() {
                    return;
                }

                format!("Couldn't load video '{href}': error.")
            };

            self.log_warn(format!("Error loading movie '{decoded}': {error}"));
            self.log_error(format!("Error: {error}"));
        }

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
            let Ok(file_url) = base.join(f) else {
                return;
            };

            let path = file_url.path().to_ascii_lowercase();
            let font_file = [".woff2", ".woff", ".otf", ".ttf"]
                .iter()
                .any(|e| path.ends_with(e));

            if file_url.scheme() == "data" || font_file {
                if self.scene.spec_version_of(self.scene.world) >= "4.1" {
                    self.log_warn("Loading a font file via family field is deprecated, please use new FontLibrary node instead.");
                }

                let href = file_url.as_str().to_string();

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
}

/// `x3d_parse::parse_bytes`, with GLTF2Parser / GLB2Parser loading the buffers of a glTF through
/// `fetcher` (FileLoader resolves them against the scene's url).
pub fn parse_bytes_with_fetcher(fetcher: &dyn Fetcher, data: &[u8], url: &str) -> ParseBytesResult {
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
            .and_then(|b| b.join(uri.trim()))
            .map_err(|_| "TypeError: Failed to construct 'URL': Invalid URL".to_string());

        if uri.trim_start().starts_with("data:") {
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

        fetcher.fetch(&resolved).inspect_err(|e| {
            failed(uri, Some(resolved.clone()), e, BufferFailure::Fetch);
        })
    };

    let scene = x3d_parse::with_gltf_fetcher(&fetch, || x3d_parse::parse_bytes(data, url));

    ParseBytesResult {
        scene,
        buffer_errors: buffer_errors.into_inner(),
    }
}
