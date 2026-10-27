//! EXTERNPROTO declarations at run time: X3DExternProtoDeclaration.js, the X3DUrlObject
//! bookkeeping it inherits (load state, `load`, `url`, `autoRefresh`), the part of FileLoader.js
//! that fetches one declaration's scene, and X3DPrototypeInstance.construct / update.
//!
//! What belongs to which authority:
//!
//! * VRML97 4.9 / 4.5.2 and X3D 4.0 (concepts, EXTERNPROTO): a URL list that is searched in
//!   order until one entry yields an interpretable PROTO (the `#name` suffix or else the first
//!   PROTO of the file); before the definition is loaded an instance takes the user's value or the
//!   type's default; the interface must be a subset of the loaded PROTO's. Nothing about reloading.
//! * X_ITE (not in either standard): `load`, `url`, `autoRefresh` and `autoRefreshTimeLimit` of the
//!   declaration (X3DUrlObject), re-instancing of the instances when the PROTO changes
//!   (`updateInstances` → `update ()` / `construct ()`), a scene cache keyed by URL, and the frame
//!   on which each of this happens.
//!
//! Where the pinned X_ITE (34cdf9f97) is unsafe, this port differs on purpose (tests in
//! `crates/x3d-script/tests/extern_reload.rs` name each one):
//!
//! * Load identity. Every request gets a generation. A scene that finished for an older
//!   generation (a slower earlier request, or one that was disabled meanwhile) is dropped; X_ITE
//!   lets it replace the current result for a frame, or activate a disabled declaration.
//! * One body per instance. X_ITE keeps both its `construct` and `update` interests after a failed
//!   load, so the next successful load builds the bodies twice. Here an instance is in exactly one
//!   of the two modes.
//! * A failed fetch is not cached. X_ITE caches the failure forever (a later request of the same
//!   URL then dies with a TypeError, or never completes); here the next request fetches again.
//! * A declaration that would wait for the scene that is waiting for it fails instead. X_ITE's cache
//!   makes `EXTERNPROTO P "same-file.wrl#P"` inside `same-file.wrl` wait forever (the oracle
//!   times out on such a world); here that candidate fails with an error and the load goes on.
//! * The instance's `metadata` field is reused on reload like every other field.

use crate::loader::FileLoader;
use crate::nodes::LoadState;
use crate::*;

/// FileLoader.MAX_CACHED_SCENES.
const MAX_CACHED_SCENES: usize = 64;

/// X3DUrlObject / X3DExternProtoDeclaration state of one declaration.
#[derive(Debug)]
pub struct ExternState {
    pub load_state: LoadState,
    /// The `load` field (default TRUE).
    pub load: bool,
    /// Identity of the current load request; bumped whenever a request is replaced or cancelled.
    pub generation: u64,
    /// `autoRefresh` (seconds, 0 = never) and `autoRefreshTimeLimit` (seconds, default 3600).
    pub auto_refresh: f64,
    pub auto_refresh_time_limit: f64,
    /// `Date .now ()` (ms) at construction and when the load state last became COMPLETE / FAILED.
    pub start_ms: f64,
    pub complete_ms: f64,
    /// Due time (ms) of the auto refresh timer, if one is set.
    pub timer: Option<f64>,
    /// The internal scene: root context, and whether it is in the scene cache.
    pub scene: Option<(CtxId, bool)>,
    fetch: Option<ExternFetch>,
}

/// The FileLoader of the current generation: the URL list and where it is.
#[derive(Debug)]
struct ExternFetch {
    request: u64,
    urls: Vec<String>,
    /// Index of the next candidate.
    next: usize,
    /// Failed candidates so far.
    attempts: usize,
}

/// A scene an EXTERNPROTO loaded that waits for its loadCount and the next frame.
#[derive(Clone, Debug)]
pub struct PendingExtern {
    /// The EXTERNPROTO declaration that loads the scene.
    pub ext: ExternId,
    /// The generation of the request that fetched it.
    pub request: u64,
    /// Root context of the loaded scene.
    pub ctx: CtxId,
    /// The internal node that owns the scene's `loadCount` field.
    pub scene_node: NodeId,
    /// The resolved URL (with its fragment).
    pub url: url::Url,
    /// The scene cache key (URL without fragment); `None` when the URL has a query.
    pub key: Option<String>,
}

/// One entry of FileLoader.#sceneCache.
#[derive(Debug)]
pub enum CacheEntry {
    /// Being fetched; declarations that asked for the same URL wait here.
    Pending(Vec<(ExternId, u64, url::Url)>),
    /// Fully loaded: root context of the merged scene, and the generation of its slot when it was
    /// cached. An entry whose context slot was released since names nothing (R0.3-E).
    Ready(CtxId, u32),
}

/// What became of one candidate URL.
enum Step {
    /// The candidate is in flight (waiting for a frame or for another declaration's fetch).
    Wait,
    /// The load finished.
    Done,
    /// The candidate failed (X_ITE logs it and tries the next one).
    Failed {
        error: String,
        resolved: Option<url::Url>,
    },
}

impl ExternState {
    fn new(now_ms: f64) -> Self {
        ExternState {
            load_state: LoadState::NotStarted,
            load: true,
            generation: 0,
            auto_refresh: 0.0,
            auto_refresh_time_limit: 3600.0,
            start_ms: now_ms,
            complete_ms: now_ms,
            timer: None,
            scene: None,
            fetch: None,
        }
    }
}

// --- X3DUrlObject --------------------------------------------------------------------------

impl Runtime {
    fn now_ms(&self) -> f64 {
        self.now * 1000.0
    }

    fn xs(&mut self, e: ExternId) -> &mut ExternState {
        let now = self.now_ms();
        self.extern_states
            .entry(e)
            .or_insert_with(|| ExternState::new(now))
    }

    /// X3DUrlObject.checkLoadState.
    pub fn extern_load_state(&self, e: ExternId) -> LoadState {
        self.extern_states
            .get(&e)
            .map(|s| s.load_state)
            .unwrap_or_default()
    }

    /// Name of the PROTO the declaration holds (`getProtoDeclaration () .getName ()`).
    pub fn extern_proto_name(&self, e: ExternId) -> Option<String> {
        self.extern_loaded
            .get(&e)
            .copied()
            .flatten()
            .map(|p| self.scene.protos[p].name.clone())
    }

    /// `executionContext .externprotos .get (name)`.
    pub fn extern_find(&self, ctx: CtxId, name: &str) -> Option<ExternId> {
        self.scene.ctx(ctx).externprotos.get(name)
    }

    /// X3DUrlObject.setLoadState (the loading object count is derived from the states).
    fn extern_set_load_state(&mut self, e: ExternId, state: LoadState) {
        let now = self.now_ms();
        let st = self.xs(e);

        st.load_state = state;
        self.scene.externprotos[e].complete = state == LoadState::Complete;

        // X3DScene.addLoadingObject / removeLoadingObject: `_loadCount = size`.
        let root = self.scene.scene_root(self.scene.externprotos[e].ctx);

        self.update_load_count(root);

        if matches!(state, LoadState::Complete | LoadState::Failed) {
            let st = self.xs(e);
            st.complete_ms = now;

            let interval = st.auto_refresh.max(0.0);
            self.extern_set_auto_refresh_timer(e, interval);
        }
    }

    /// Replace the current request by nothing: late results of older generations are ignored and
    /// the loader of the old generation is dropped (FileLoader.abort).
    fn extern_cancel(&mut self, e: ExternId) {
        let request = self.next_request();
        let st = self.xs(e);

        st.generation = request;
        st.fetch = None;
    }

    /// A new request identity. Runtime-wide, not per declaration: a declaration slot that is released
    /// and handed out again (R0.3-E) never sees the number of a request made for the one before.
    fn next_request(&mut self) -> u64 {
        self.request_counter += 1;
        self.request_counter
    }

    /// X3DUrlObject.requestImmediateLoad for a declaration. The load itself starts when the
    /// `loadData` event is processed (see [`Runtime::process_phase`]): `_loadData = time` is a field
    /// event, and several requests before it is processed make one load.
    pub(crate) fn extern_request_load(&mut self, e: ExternId) {
        let st = self.xs(e);

        if st.load_state != LoadState::NotStarted || !st.load {
            return;
        }

        // The LoadUrlObjects option, checked before `load`.
        if !self.browser_options.load_url_objects() {
            return;
        }

        // `url .length === 0`: unloadData () and no load.
        if self.scene.externprotos[e].url.is_empty() {
            return;
        }

        self.extern_set_load_state(e, LoadState::InProgress);

        if !self.extern_load_events.contains(&e) {
            self.extern_load_events.push(e);
        }
    }

    /// X3DUrlObject.requestUnload (`load` became FALSE). X3DExternProtoDeclaration has no
    /// `unloadData`: the PROTO and the instances' bodies stay.
    fn extern_request_unload(&mut self, e: ExternId) {
        if matches!(
            self.extern_load_state(e),
            LoadState::NotStarted | LoadState::Failed
        ) {
            return;
        }

        self.extern_cancel(e);
        self.extern_set_load_state(e, LoadState::NotStarted);
    }

    /// The `url` field of the declaration was set (X3DUrlObject.set_url__).
    pub fn extern_set_url(&mut self, e: ExternId, url: Vec<String>) {
        self.scene.externprotos[e].url = url;

        if !self.xs(e).load {
            return;
        }

        self.extern_cancel(e);
        self.extern_set_load_state(e, LoadState::NotStarted);
        self.extern_request_load(e);
    }

    /// The `load` field was set (X3DUrlObject.set_load__).
    pub fn extern_set_load(&mut self, e: ExternId, value: bool) {
        self.xs(e).load = value;

        if value {
            self.extern_request_load(e);
        } else {
            self.extern_request_unload(e);
        }
    }

    /// X3DExecutionContext.removeExternProtoDeclaration: the declaration leaves its context and its
    /// load is cancelled, so nothing it started can reach its instances afterwards (X_ITE lets an
    /// in-flight load finish). The instances keep the body they have.
    pub fn extern_remove(&mut self, ctx: CtxId, name: &str) -> Option<ExternId> {
        let e = self.scene.ctx(ctx).externprotos.get(name)?;

        self.scene.ctx_mut(ctx).externprotos.remove(name);
        self.extern_cancel(e);
        self.extern_load_events.retain(|x| *x != e);
        self.xs(e).timer = None;

        if self.extern_load_state(e) == LoadState::InProgress {
            self.extern_set_load_state(e, LoadState::NotStarted);
        }

        Some(e)
    }

    /// A declaration whose context was disposed: no request, no timer, nothing in progress. A fetch
    /// that is in flight, or a scene that waits for its loadCount, is dropped when it comes back (its
    /// request is not current). X_ITE lets the load finish into the disposed scene.
    pub(crate) fn extern_close(&mut self, e: ExternId) {
        if !self.extern_states.contains_key(&e) {
            return;
        }

        self.extern_cancel(e);
        self.extern_load_events.retain(|x| *x != e);
        self.xs(e).timer = None;

        if self.extern_load_state(e) == LoadState::InProgress {
            self.extern_set_load_state(e, LoadState::NotStarted);
        }
    }

    /// `loadNow ()` of the SAI object: start over from NOT_STARTED.
    pub fn extern_load_now(&mut self, e: ExternId) {
        self.extern_cancel(e);
        self.extern_set_load_state(e, LoadState::NotStarted);
        self.extern_request_load(e);
    }

    pub fn extern_set_auto_refresh(&mut self, e: ExternId, value: f64) {
        self.xs(e).auto_refresh = value;
        self.extern_auto_refresh_changed(e);
    }

    pub fn extern_set_auto_refresh_time_limit(&mut self, e: ExternId, value: f64) {
        self.xs(e).auto_refresh_time_limit = value;
        self.extern_auto_refresh_changed(e);
    }

    /// X3DUrlObject.set_autoRefresh__.
    fn extern_auto_refresh_changed(&mut self, e: ExternId) {
        if self.extern_load_state(e) != LoadState::Complete {
            return;
        }

        let now = self.now_ms();
        let st = self.xs(e);
        let interval =
            url_object::refresh_interval(st.auto_refresh, (now - st.complete_ms) / 1000.0);

        self.extern_set_auto_refresh_timer(e, interval);
    }

    /// X3DUrlObject.setAutoRefreshTimer: one timer, bounded by `autoRefreshTimeLimit`.
    fn extern_set_auto_refresh_timer(&mut self, e: ExternId, interval: f64) {
        let now = self.now_ms();
        let st = self.xs(e);

        st.timer = url_object::refresh_timer_due(
            now,
            st.start_ms,
            st.auto_refresh,
            st.auto_refresh_time_limit,
            interval,
        );
    }

    /// The due time (ms) and declaration of the earliest auto refresh timer due by `now` (seconds).
    pub(crate) fn extern_next_timer(&self, now: f64) -> Option<(f64, ExternId)> {
        self.extern_states
            .iter()
            .filter_map(|(e, s)| s.timer.map(|due| (due, *e)))
            .filter(|(due, _)| *due <= now * 1000.0)
            .min_by(|a, b| a.0.partial_cmp(&b.0).unwrap().then(a.1.cmp(&b.1)))
    }

    /// X3DUrlObject.performAutoRefresh.
    pub(crate) fn extern_perform_auto_refresh(&mut self, e: ExternId) {
        self.xs(e).timer = None;
        self.extern_cancel(e);
        self.extern_set_load_state(e, LoadState::NotStarted);
        self.extern_request_load(e);
    }

    /// Loading objects of the world scene (`scene ._loadCount`): the Inlines and EXTERNPROTO
    /// declarations whose load is in progress and whose scene is attached to the world.
    pub fn root_load_count(&self) -> i32 {
        let attached = |ctx: CtxId| {
            let mut root = self.scene.scene_root(ctx);

            for _ in 0..64 {
                if root == ROOT || root == self.scene.world {
                    return true;
                }

                match self.inline_scenes.iter().find(|(_, c)| **c == root) {
                    Some((inline, _)) => root = self.scene.scene_root(self.scene.node(*inline).ctx),
                    None => return false,
                }
            }

            false
        };

        let inlines = (0..self.scene.nodes.len() as NodeId)
            .filter(|&n| {
                self.builtin_name(n) == Some("Inline")
                    && self.url_load_state(n) == LoadState::InProgress
                    && attached(self.scene.node(n).ctx)
            })
            .count();
        let externs = self
            .extern_states
            .iter()
            .filter(|(e, s)| {
                s.load_state == LoadState::InProgress && attached(self.scene.externprotos[**e].ctx)
            })
            .count();

        (inlines + externs) as i32
    }
}

/// What is left of EXTERNPROTO loading (for tests and diagnostics): all zero when everything settled.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct ExternDiagnostics {
    /// Scenes parsed and set up that wait for their loadCount or the next frame.
    pub pending_scenes: usize,
    /// `loadData` events not processed yet.
    pub load_events: usize,
    /// Declarations whose load state is IN_PROGRESS.
    pub in_progress: usize,
    /// Scene cache entries that are being fetched.
    pub pending_cache: usize,
    /// Armed auto refresh timers.
    pub timers: usize,
    /// Instance bodies that were built and then replaced or dropped (never reused).
    pub disposed_bodies: usize,
}

impl Runtime {
    pub fn extern_diagnostics(&self) -> ExternDiagnostics {
        ExternDiagnostics {
            pending_scenes: self.pending_externs.len() + self.extern_frame_waiters.len(),
            load_events: self.extern_load_events.len(),
            in_progress: self
                .extern_states
                .values()
                .filter(|s| s.load_state == LoadState::InProgress)
                .count(),
            pending_cache: self
                .extern_cache
                .iter()
                .filter(|(_, e)| matches!(e, CacheEntry::Pending(_)))
                .count(),
            timers: self
                .extern_states
                .values()
                .filter(|s| s.timer.is_some())
                .count(),
            disposed_bodies: self.bodies_disposed,
        }
    }
}

// --- loadData: the FileLoader of a declaration -------------------------------------------------

impl Runtime {
    /// X3DExternProtoDeclaration.loadData: a new FileLoader (the old one is aborted) for the URL list.
    pub(crate) fn extern_load_data(&mut self, e: ExternId) {
        // A request that was cancelled before its event was processed does nothing.
        if self.extern_load_state(e) != LoadState::InProgress {
            return;
        }

        let urls = self.scene.externprotos[e].url.clone();
        let request = self.next_request();
        let st = self.xs(e);

        st.generation = request;
        let empty = urls.is_empty();

        st.fetch = Some(ExternFetch {
            request,
            urls,
            next: 0,
            attempts: 0,
        });

        if empty {
            // FileLoader.loadDocument: `loadDocumentError ()` without an error prints nothing.
            self.extern_exhausted(e, request);
        } else {
            self.tasks.push_back(Task::ExternLoad(e, request));
        }
    }

    /// The base URL and the candidates a declaration's fetch has not tried yet (`None` when request `request` is over).
    pub(crate) fn extern_pending_candidates(
        &self,
        e: ExternId,
        request: u64,
    ) -> Option<(String, Vec<String>)> {
        if !self.extern_fetch_is(e, request) {
            return None;
        }

        let fetch = self.extern_states.get(&e)?.fetch.as_ref()?;

        Some((
            self.scene
                .world_url_of(self.scene.externprotos[e].ctx)
                .to_string(),
            fetch.urls[fetch.next.min(fetch.urls.len())..].to_vec(),
        ))
    }

    fn extern_fetch_is(&self, e: ExternId, request: u64) -> bool {
        self.extern_states.get(&e).is_some_and(|s| {
            s.generation == request && s.fetch.as_ref().is_some_and(|f| f.request == request)
        })
    }

    /// FileLoader.loadDocument / loadDocumentError: try the candidates from where the loader is,
    /// until one is in flight, one succeeds, or all have failed.
    pub(crate) fn extern_run(&mut self, e: ExternId, request: u64) {
        loop {
            if !self.extern_fetch_is(e, request) {
                return;
            }

            let st = self.xs(e);
            let fetch = st.fetch.as_mut().unwrap();

            if fetch.next >= fetch.urls.len() {
                self.extern_exhausted(e, request);
                return;
            }

            let candidate = fetch.urls[fetch.next].clone();

            fetch.next += 1;

            match self.extern_candidate(e, request, &candidate) {
                Step::Wait | Step::Done => return,
                Step::Failed { error, resolved } => {
                    if !self.extern_fail_candidate(e, request, error, resolved) {
                        return;
                    }

                    // An asynchronous fetcher: the next candidate runs when its response is in
                    // (fetch_task_ready), at once if it is.
                    if self.fetcher.is_async() {
                        self.fetches.push_front(Task::ExternLoad(e, request));
                        return;
                    }
                }
            }
        }
    }

    /// FileLoader.printError for the candidate that just failed. Returns whether another candidate remains.
    fn extern_fail_candidate(
        &mut self,
        e: ExternId,
        request: u64,
        error: String,
        resolved: Option<url::Url>,
    ) -> bool {
        if !self.extern_fetch_is(e, request) {
            return false;
        }

        let loader = FileLoader {
            type_name: "X3DExternProtoDeclaration".into(),
            base: self
                .scene
                .world_url_of(self.scene.externprotos[e].ctx)
                .to_string(),
            data_as_string: false,
        };
        let fetch = self.xs(e).fetch.as_mut().unwrap();

        fetch.attempts += 1;

        let (attempts, remaining, candidate) = (
            fetch.attempts,
            fetch.urls.len() - fetch.next,
            fetch.urls[fetch.next - 1].clone(),
        );

        self.print_load_error(
            &loader,
            &candidate,
            resolved.as_ref(),
            &error,
            attempts,
            remaining,
        );

        if remaining == 0 {
            self.extern_exhausted(e, request);
            return false;
        }

        true
    }

    /// Every candidate failed: `callback (null)` → setInternalScene (null).
    fn extern_exhausted(&mut self, e: ExternId, request: u64) {
        let _ = self.extern_set_internal_scene(e, request, None);
    }

    /// FileLoader.loadDocumentAsync for one candidate of a declaration.
    fn extern_candidate(&mut self, e: ExternId, request: u64, candidate: &str) -> Step {
        if candidate.is_empty() {
            return Step::Failed {
                error: "Error: Empty URL.".into(),
                resolved: None,
            };
        }

        // Script URLs are no scenes.
        let trimmed = candidate.trim_start();

        if ["ecmascript:", "javascript:", "vrmlscript:"]
            .iter()
            .any(|s| trimmed.starts_with(s))
        {
            return Step::Failed {
                error: "Error: Couldn't parse X3D. No suitable file handler found.".into(),
                resolved: None,
            };
        }

        let base = match url::Url::parse(self.scene.world_url_of(self.scene.externprotos[e].ctx)) {
            Ok(b) => b,
            Err(err) => {
                return Step::Failed {
                    error: format!("TypeError: {err}"),
                    resolved: None,
                };
            }
        };
        let Ok(resolved) = base.join(candidate) else {
            return Step::Failed {
                error: "TypeError: Failed to construct 'URL': Invalid URL".into(),
                resolved: None,
            };
        };

        // Cached scenes: not for a URL with a query; the key has no fragment.
        let key = if resolved.query().is_none() {
            let mut k = resolved.clone();

            k.set_fragment(None);
            Some(k.to_string())
        } else {
            None
        };

        if let Some(key) = &key {
            if self.extern_wait_cycle(e, key) {
                return Step::Failed {
                    error: "Error: EXTERNPROTO URL refers to the scene that is loading it.".into(),
                    resolved: Some(resolved),
                };
            }

            // A cached scene that went with the world is no scene: it is fetched again. So is one whose
            // context slot was released and perhaps taken by another scene (generation).
            if self.extern_cache.iter().any(|(k, entry)| {
                k == key
                    && matches!(entry, CacheEntry::Ready(c, g) if self.extern_cache_stale(*c, *g))
            }) {
                self.extern_cache.retain(|(k, _)| k != key);
            }

            match self.extern_cache_get(key) {
                Some(CacheEntry::Ready(ctx, _)) => {
                    let ctx = *ctx;

                    return self.extern_deliver(e, request, ctx, &resolved, true);
                }
                Some(CacheEntry::Pending(waiters)) => {
                    waiters.push((e, request, resolved));
                    return Step::Wait;
                }
                None => self.extern_cache_add(key.clone()),
            }
        }

        let data = match self.fetcher.fetch(&resolved) {
            Ok(d) => d,
            Err(error) => {
                self.extern_cache_fail(key.as_deref(), &error, &resolved);
                return Step::Failed {
                    error,
                    resolved: Some(resolved),
                };
            }
        };

        match self.extern_scene(
            e,
            request,
            &x3d_parse::gunzip(&data),
            &resolved,
            key.clone(),
        ) {
            Ok(()) => Step::Wait,
            Err(error) => {
                self.extern_cache_fail(key.as_deref(), &error, &resolved);
                Step::Failed {
                    error,
                    resolved: Some(resolved),
                }
            }
        }
    }

    /// FileLoader.createX3DFromString for an EXTERNPROTO: the scene is parsed and its nodes are
    /// set up (like `inline_scene`); it waits for its loadCount and the next frame before
    /// `extern_attach`. Whether the file has the PROTO is decided then (setInternalScene).
    fn extern_scene(
        &mut self,
        e: ExternId,
        request: u64,
        data: &[u8],
        url: &url::Url,
        key: Option<String>,
    ) -> Result<(), String> {
        self.sync_svg_quality();

        let mut scene =
            x3d_parse::parse_bytes(data, url.as_str()).map_err(|err| format!("Error: {err}"))?;

        for w in std::mem::take(&mut scene.warnings) {
            self.log_warn(w);
        }

        let setup_order = std::mem::take(&mut scene.setup_order);
        let ctx_count = scene.contexts.len();

        // A scene that goes into the cache belongs to the browser, not to the declaration that
        // fetched it: X_ITE gives it `browser .getDefaultScene ()` as its execution context
        // (X3DExternProtoDeclaration.setInternalScene) and never disposes it with a user's scene.
        // Here that is the world: an Inline that declared it first may go, the next one finds it.
        // An uncached scene (a URL with a query) is the declaration's own and goes with it.
        let parent = if key.is_some() {
            self.scene.world
        } else {
            self.scene.externprotos[e].ctx
        };
        let m = self.scene.merge_extern(scene, parent);

        let (scene_node, count) = self.new_load_count_scene();
        self.pending_externs.push(PendingExtern {
            ext: e,
            request,
            ctx: m.root_ctx,
            scene_node,
            url: url.clone(),
            key,
        });

        // `new X3DScene (browser)`: setLive (false) until extern_attach.
        self.inline_live.insert(m.root_ctx, false);

        self.attach_merged_scene(m, ctx_count, setup_order);

        // FileLoader.setScene: `scene ._loadCount .addEvent ()`.
        self.add_event(count);
        Ok(())
    }

    /// FileLoader.set_loadCount__, after the frame (see `finish_frame`): the scene is cached and
    /// handed to setInternalScene; the declarations that waited for it follow.
    pub(crate) fn extern_attach(&mut self, p: PendingExtern) {
        let PendingExtern {
            ext,
            request,
            ctx,
            url,
            key,
            ..
        } = p;
        let waiters = match &key {
            Some(key) => self.extern_cache_ready(key, ctx),
            None => vec![],
        };

        match self.extern_deliver(ext, request, ctx, &url, key.is_some()) {
            Step::Failed { error, resolved } => {
                if self.extern_fail_candidate(ext, request, error, resolved) {
                    self.tasks.push_back(Task::ExternLoad(ext, request));
                }
            }
            Step::Wait | Step::Done => {}
        }

        for (we, wrequest, wurl) in waiters {
            match self.extern_deliver(we, wrequest, ctx, &wurl, true) {
                Step::Failed { error, resolved } => {
                    if self.extern_fail_candidate(we, wrequest, error, resolved) {
                        self.tasks.push_back(Task::ExternLoad(we, wrequest));
                    }
                }
                Step::Wait | Step::Done => {}
            }
        }
    }

    fn extern_deliver(
        &mut self,
        e: ExternId,
        request: u64,
        ctx: CtxId,
        url: &url::Url,
        cached: bool,
    ) -> Step {
        match self.extern_set_internal_scene(e, request, Some((ctx, url, cached))) {
            Ok(()) => Step::Done,
            Err(error) => Step::Failed {
                error,
                resolved: Some(url.clone()),
            },
        }
    }

    /// X3DExternProtoDeclaration.setInternalScene. An `Err` is a thrown error (the candidate fails).
    fn extern_set_internal_scene(
        &mut self,
        e: ExternId,
        request: u64,
        scene: Option<(CtxId, &url::Url, bool)>,
    ) -> Result<(), String> {
        // A scene of an older request (or of a disabled declaration) is not used.
        if self.xs(e).generation != request {
            return Ok(());
        }

        // Remove the old scene (`if (!this [_scene] ?.[_cache]) this [_scene] .dispose ()`).
        let old = self.xs(e).scene.take();

        if let Some((old_ctx, old_cached)) = old {
            if !old_cached && scene.is_none_or(|(c, ..)| c != old_ctx) {
                self.dispose_scene(old_ctx);
            }
        }

        let Some((ctx, url, cached)) = scene else {
            self.xs(e).fetch = None;
            self.extern_set_load_state(e, LoadState::Failed);
            self.extern_set_proto(e, None);
            return Ok(());
        };

        self.xs(e).scene = Some((ctx, cached));

        // `scene .protos .get (hash)` or `scene .protos [0]`; a data: URL has no fragment.
        let fragment = if url.scheme() == "data" {
            ""
        } else {
            url.fragment().unwrap_or("")
        };
        let root = self.scene.ctx(ctx);
        let found = if fragment.is_empty() {
            root.protos.values().first().copied()
        } else {
            root.protos.get(fragment)
        };

        let Some(proto) = found else {
            return Err("Error: PROTO not found.".into());
        };

        // scene .setLive (true)
        self.inline_live.insert(ctx, true);
        self.refresh_live();

        self.xs(e).fetch = None;
        self.extern_set_load_state(e, LoadState::Complete);
        self.extern_set_proto(e, Some(proto));
        Ok(())
    }

    /// X3DExternProtoDeclaration.setProtoDeclaration: the declaration takes the PROTO's interface
    /// and every instance is told (`updateInstances`).
    fn extern_set_proto(&mut self, e: ExternId, proto: Option<ProtoId>) {
        self.extern_loaded.insert(e, proto);

        if let Some(p) = proto {
            let (fields, has_metadata) = (
                self.scene.protos[p].fields.clone(),
                self.scene.protos[p].has_metadata,
            );

            self.scene.externprotos[e].fields = fields;
            self.scene.externprotos[e].has_metadata = has_metadata;
        }

        for inst in self.extern_instances(e) {
            if self.instance_update.contains(&inst) {
                self.extern_instance_update(inst);
            } else {
                self.extern_instance_construct(inst);
            }
        }
    }

    /// Every live instance node of an EXTERNPROTO declaration, in the order they were created.
    fn extern_instances(&self, e: ExternId) -> Vec<NodeId> {
        (0..self.scene.nodes.len() as NodeId)
            .filter(|&n| {
                matches!(self.scene.node(n).kind, NodeKind::Instance(ProtoRef::Extern(x)) if x == e)
                    && !self.ctx_dead(self.scene.node(n).ctx)
            })
            .collect()
    }
}

// --- the scene cache (FileLoader.#sceneCache) -------------------------------------------------

impl Runtime {
    /// Would declaration `e` wait for a cache entry that is waiting for the scene `e` belongs to?
    /// The entry is pending and its owner's scene is parsed but waits for its loadCount; `e` is
    /// declared in that scene, or in a scene that waits for such a scene.
    fn extern_wait_cycle(&self, e: ExternId, key: &str) -> bool {
        if !self
            .extern_cache
            .iter()
            .any(|(k, entry)| k == key && matches!(entry, CacheEntry::Pending(_)))
        {
            return false;
        }

        let Some(owner) = self
            .pending_externs
            .iter()
            .find(|p| p.key.as_deref() == Some(key))
        else {
            return false;
        };
        let mut root = self.scene.scene_root(self.scene.externprotos[e].ctx);

        for _ in 0..64 {
            if root == owner.ctx {
                return true;
            }

            match self.pending_externs.iter().find(|p| p.ctx == root) {
                Some(p) => root = self.scene.scene_root(self.scene.externprotos[p.ext].ctx),
                None => return false,
            }
        }

        false
    }

    /// A `Ready` entry that names no cached scene: the scene was disposed or emptied, or its context
    /// slot was released since (its generation moved on).
    pub fn extern_cache_stale(&self, ctx: CtxId, generation: u32) -> bool {
        self.scene
            .resolve_ctx(CtxHandle {
                id: ctx,
                generation,
            })
            .is_none()
            || self.ctx_dead(ctx)
            || self.emptied_ctxs.contains(&ctx)
    }

    /// FileLoader.getScene: the entry moves to the end of the list.
    fn extern_cache_get(&mut self, key: &str) -> Option<&mut CacheEntry> {
        let i = self.extern_cache.iter().position(|(k, _)| k == key)?;
        let entry = self.extern_cache.remove(i);

        self.extern_cache.push(entry);
        self.extern_cache.last_mut().map(|(_, e)| e)
    }

    /// FileLoader.addScene: drops the least recently used entry at the limit.
    fn extern_cache_add(&mut self, key: String) {
        if self.extern_cache.len() >= MAX_CACHED_SCENES {
            self.extern_cache.remove(0);
        }

        self.extern_cache.push((key, CacheEntry::Pending(vec![])));
    }

    /// `cacheResolve (scene)`: the entry is ready; the waiting declarations are returned.
    fn extern_cache_ready(&mut self, key: &str, ctx: CtxId) -> Vec<(ExternId, u64, url::Url)> {
        let generation = self.scene.ctx_slots.generation(ctx);

        match self.extern_cache.iter_mut().find(|(k, _)| k == key) {
            Some((_, entry)) => {
                match std::mem::replace(entry, CacheEntry::Ready(ctx, generation)) {
                    CacheEntry::Pending(waiters) => waiters,
                    CacheEntry::Ready(..) => vec![],
                }
            }
            None => {
                if self.extern_cache.len() >= MAX_CACHED_SCENES {
                    self.extern_cache.remove(0);
                }

                self.extern_cache
                    .push((key.to_string(), CacheEntry::Ready(ctx, generation)));
                vec![]
            }
        }
    }

    /// The fetch of a cached URL failed: the entry is dropped (X_ITE would keep the failure), and
    /// the declarations that waited for it fail the same candidate.
    fn extern_cache_fail(&mut self, key: Option<&str>, error: &str, resolved: &url::Url) {
        let Some(key) = key else {
            return;
        };
        let Some(i) = self.extern_cache.iter().position(|(k, _)| k == key) else {
            return;
        };

        let (_, entry) = self.extern_cache.remove(i);

        if let CacheEntry::Pending(waiters) = entry {
            for (we, wrequest, _) in waiters {
                if self.extern_fail_candidate(
                    we,
                    wrequest,
                    error.to_string(),
                    Some(resolved.clone()),
                ) {
                    self.tasks.push_back(Task::ExternLoad(we, wrequest));
                }
            }
        }
    }
}

// --- X3DPrototypeInstance.construct / update ----------------------------------------------------

impl Runtime {
    /// `this [_body] ?.dispose ()`: the body is disposed like a scene (see [`Runtime::dispose_scene`]) and
    /// goes dead. X_ITE leaves its nodes to the garbage collector; here the sweep releases them.
    pub(crate) fn instance_dispose_body(&mut self, inst: NodeId) {
        let Some(body) = self.instance_bodies.remove(&inst) else {
            return;
        };

        // The body is disposed like a scene: its Scripts get `shutdown ()` (4.12.3), its routes go,
        // its nodes are released at the next sweep.
        self.dispose_scene(body);

        if self.dead_ctxs.insert(body) {
            self.bodies_disposed += 1;
        }
        self.refresh_live();
    }

    /// Is `ctx` a disposed instance body or a disposed Inline scene, or inside one?
    pub fn ctx_dead(&self, ctx: CtxId) -> bool {
        let mut c = ctx;

        for _ in 0..256 {
            if self.dead_ctxs.contains(&c) || self.disposed_scenes.contains(&c) {
                return true;
            }

            match self.scene.ctx(c).outer {
                Outer::Instance(n) => c = self.scene.node(n).ctx,
                _ => match self.scene.ctx(c).parent {
                    Some(p) => c = p,
                    None => return false,
                },
            }
        }

        false
    }

    /// X3DPrototypeInstance.construct for an instance of an EXTERNPROTO.
    pub(crate) fn extern_instance_construct(&mut self, inst: NodeId) {
        let NodeKind::Instance(ProtoRef::Extern(e)) = self.scene.node(inst).kind else {
            return;
        };

        self.instance_dispose_body(inst);

        // Not complete: no body; the instance waits for the load (and asks for it).
        if self.extern_load_state(e) != LoadState::Complete {
            if self.is_initialized(inst) {
                self.add_node_parents_event(inst);
            }

            self.instance_update.remove(&inst);
            self.extern_request_load(e);
            return;
        }

        if let Some(proto) = self.extern_loaded.get(&e).copied().flatten() {
            self.extern_instance_defaults(inst, proto);
        }

        if self.is_initialized(inst) {
            self.realize(inst);
        }

        self.instance_update.insert(inst);
    }

    /// X3DPrototypeInstance.construct, the field part for a loaded EXTERNPROTO: an interface field
    /// that was not set takes the PROTO's default (`assign`: with an event, and the field counts as
    /// modified from then on). A field the declaration lacks is added.
    pub(crate) fn extern_instance_defaults(&mut self, inst: NodeId, proto: ProtoId) {
        for pf in self.scene.protos[proto].fields.clone() {
            let Some(i) = self
                .scene
                .node(inst)
                .fields
                .iter()
                .position(|f| f.name == pf.name)
            else {
                // addPredefinedField
                self.scene.node_mut(inst).fields.push(Field::new(
                    &pf.name,
                    pf.access,
                    pf.value.clone(),
                ));
                continue;
            };

            let k = (inst, FieldLoc::Pre(i));
            let f = &self.scene.node(inst).fields[i];
            let has_references = !f.references.is_empty()
                || self.frt.get(&k).is_some_and(|r| !r.references.is_empty());
            let skip = f.access != pf.access
                || f.ty() != pf.ty()
                || !f.is_initializable()
                || f.modified
                || has_references
                || f.value.equals(&pf.value);

            if !skip {
                self.set_value(k, pf.value.clone());
            }
        }
    }

    /// X3DPrototypeInstance.update: the declaration's interface changed. Fields of the same name
    /// and type are reused (and with them their routes), then `construct ()`.
    pub(crate) fn extern_instance_update(&mut self, inst: NodeId) {
        let NodeKind::Instance(ProtoRef::Extern(e)) = self.scene.node(inst).kind else {
            return;
        };

        // The new definitions: metadata (unless the interface redefines it), then the interface.
        let decl = &self.scene.externprotos[e];
        let mut defs: Vec<Field> = vec![];

        if decl.has_metadata {
            defs.push(Field::new(
                "metadata",
                access::INPUT_OUTPUT,
                FieldValue::SFNode(None),
            ));
        }

        defs.extend(
            decl.fields
                .iter()
                .map(|f| Field::new(&f.name, f.access, f.value.clone())),
        );

        let old = self.scene.node(inst).fields.clone();
        let reuse: Vec<Option<usize>> = defs
            .iter()
            .map(|d| {
                old.iter()
                    .position(|o| o.name == d.name && o.ty() == d.ty())
            })
            .collect();

        // The routes of the reused fields, then the routes of the fields that are gone. They are
        // deleted now (in the old field layout) and added again after the fields are replaced; a
        // re-added route goes to the end of its context's list.
        let mut readd: Vec<(CtxId, Route, bool, usize)> = vec![];

        for (j, r) in reuse.iter().enumerate() {
            if let Some(i) = r {
                for (input, c, route) in self.instance_routes(inst, FieldLoc::Pre(*i)) {
                    self.delete_route(
                        c,
                        route.source_node,
                        &route.source_field,
                        route.destination_node,
                        &route.destination_field,
                    );
                    readd.push((c, route, input, j));
                }
            }
        }

        for (i, _) in old.iter().enumerate() {
            if !reuse.contains(&Some(i)) {
                for (_, c, route) in self.instance_routes(inst, FieldLoc::Pre(i)) {
                    self.delete_route(
                        c,
                        route.source_node,
                        &route.source_field,
                        route.destination_node,
                        &route.destination_field,
                    );
                }
            }
        }

        // The fields that are reused keep their value, their `modified` flag and their runtime
        // state, but not the connections to the old body.
        let old = self.scene.node(inst).fields.clone();
        let mut map: Vec<Option<usize>> = vec![None; old.len()];
        let mut fields = defs.clone();
        let mut assign: Vec<usize> = vec![];

        for (j, r) in reuse.iter().enumerate() {
            let Some(i) = r else {
                continue;
            };

            let mut f = old[*i].clone();

            f.access = defs[j].access;

            if f.is_initializable() && !f.modified {
                assign.push(j);
            }

            fields[j] = f;
            map[*i] = Some(j);
        }

        self.remap_instance_fields(inst, &map);
        self.scene.node_mut(inst).fields = fields;
        self.touch(inst);

        for j in 0..defs.len() {
            if reuse[j].is_some() {
                let k = (inst, FieldLoc::Pre(j));
                let references = self
                    .frt
                    .get(&k)
                    .map(|r| r.references.clone())
                    .unwrap_or_default();

                // Remove the field interests (the old body's IS relations), then add the references again.
                if let Some(r) = self.frt.get_mut(&k) {
                    r.field_interests.clear();
                    r.references.clear();
                }

                for reference in references {
                    self.add_reference(k, reference);
                }
            }
        }

        // `oldField .assign (newField); oldField .setModificationTime (-1)`
        for j in assign {
            let k = (inst, FieldLoc::Pre(j));

            self.set_value(k, defs[j].value.clone());
            self.scene.field_mut(inst, FieldLoc::Pre(j)).modified = false;
        }

        // Reconnect the routes.
        for (c, route, input, j) in readd {
            let name = self.scene.node(inst).fields[j].name.clone();
            let result = if input {
                self.add_route(c, route.source_node, &route.source_field, inst, &name)
            } else {
                self.add_route(
                    c,
                    inst,
                    &name,
                    route.destination_node,
                    &route.destination_field,
                )
            };

            // A route that cannot be made again is dropped (`catch { }`).
            let _ = result;
        }

        self.extern_instance_construct(inst);
    }

    /// The routes that end (`true`) or start (`false`) at a field of `inst`, in the order of their contexts.
    fn instance_routes(&self, inst: NodeId, loc: FieldLoc) -> Vec<(bool, CtxId, Route)> {
        let mut out = vec![];

        for (c, ctx) in self.scene.contexts.iter().enumerate() {
            for r in &ctx.routes {
                if r.conn.is_none() {
                    continue;
                }

                if r.destination_node == inst && r.destination_loc == loc {
                    out.push((true, c, r.clone()));
                }

                if r.source_node == inst && r.source_loc == loc {
                    out.push((false, c, r.clone()));
                }
            }
        }

        out
    }

    /// Move the runtime state of the predefined fields of `inst` to their new positions
    /// (`map [old] = Some (new)`) and drop the state of the fields that are gone.
    fn remap_instance_fields(&mut self, inst: NodeId, map: &[Option<usize>]) {
        let remap = |k: FKey| -> Option<FKey> {
            match k {
                (n, FieldLoc::Pre(i)) if n == inst => {
                    map.get(i).copied().flatten().map(|j| (n, FieldLoc::Pre(j)))
                }
                other => Some(other),
            }
        };

        let old = std::mem::take(&mut self.frt);

        for (k, mut rt) in old {
            let Some(nk) = remap(k) else {
                continue;
            };

            rt.field_interests = rt
                .field_interests
                .iter()
                .filter_map(|x| remap(*x))
                .collect();
            rt.references = rt.references.iter().filter_map(|x| remap(*x)).collect();
            self.frt.insert(nk, rt);
        }

        for holders in self.parents.values_mut() {
            *holders = holders
                .iter()
                .filter_map(|h| match h {
                    Holder::Field(k) => remap(*k).map(Holder::Field),
                    other => Some(*other),
                })
                .collect();
        }

        let tainted = std::mem::take(&mut self.tainted_fields);

        self.tainted_fields = tainted
            .into_iter()
            .filter_map(|(k, mut ev)| {
                let nk = remap(k)?;

                ev.origin = remap(ev.origin)?;
                ev.visited = ev.visited.iter().filter_map(|x| remap(*x)).collect();
                Some((nk, ev))
            })
            .collect();
    }
}
