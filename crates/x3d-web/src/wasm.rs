//! The wasm32 module behind `web/x_ite_r.mjs`: a [`WebBrowser`] owns the WebGPU device of one `<x3d-canvas>`, the
//! renderer, and its worlds ([`Session`]). The page's adapter fetches the world, runs the `requestAnimationFrame`
//! loop, forwards the input events and calls [`WebBrowser::frame`]; everything X3D happens here.
//!
//! A world is a session with an id. The current world is drawn; the others are scenes that load
//! (`createX3DFromString`, a `loadURL` in progress) and wait for [`WebBrowser::replace`]. Each world has its own
//! [`WebFetcher`]: the page's `fetch`, never a synchronous request. Its requests end with the world.

use crate::session::{Pointer, RouteInfo, Session, Touch};
use std::cell::RefCell;
use std::collections::HashMap;
use std::rc::{Rc, Weak};
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use wasm_bindgen::JsCast;
use wasm_bindgen::prelude::*;
use x3d_render::gpu::Renderer;
use x3d_runtime::FetchPoll;
use x3d_runtime::data::Data;

/// The responses of one world's fetches, by URL without its fragment.
struct FetchCache {
    entries: HashMap<String, Entry>,
    /// Aborts the requests in flight when the world goes (FileLoader.abort).
    controller: Option<web_sys::AbortController>,
    /// A response arrived since [`WebBrowser::loaded`] last looked.
    settled: bool,
    /// The resolve functions of [`WebBrowser::progress`] promises.
    waiters: Vec<js_sys::Function>,
}

enum Entry {
    Pending,
    Loaded(Rc<[u8]>),
    Failed(String),
}

impl FetchCache {
    fn new() -> Rc<RefCell<FetchCache>> {
        Rc::new(RefCell::new(FetchCache {
            entries: HashMap::new(),
            controller: web_sys::AbortController::new().ok(),
            settled: false,
            waiters: vec![],
        }))
    }

    fn wake(&mut self) {
        for resolve in std::mem::take(&mut self.waiters) {
            let _ = resolve.call0(&JsValue::NULL);
        }
    }
}

/// The world is gone: its requests are aborted and a page that waits for one continues (and sees the world gone).
impl Drop for FetchCache {
    fn drop(&mut self) {
        if let Some(c) = &self.controller {
            c.abort();
        }

        self.wake();
    }
}

fn cache_key(url: &url::Url) -> String {
    let mut url = url.clone();

    url.set_fragment(None);
    url.into()
}

/// `String (error)` of a rejection.
fn error_text(error: &JsValue) -> String {
    error
        .as_string()
        .unwrap_or_else(|| String::from(error.unchecked_ref::<js_sys::Object>().to_string()))
}

/// FileLoader's `fetch (url, { cache: "default", signal })` + `checkResponse`: the body, or `String (error)`.
async fn fetch_bytes(url: String, signal: Option<web_sys::AbortSignal>) -> Result<Vec<u8>, String> {
    let window = web_sys::window().ok_or("TypeError: Failed to fetch")?;
    let init = web_sys::RequestInit::new();

    init.set_cache(web_sys::RequestCache::Default);
    init.set_signal(signal.as_ref());

    let response =
        wasm_bindgen_futures::JsFuture::from(window.fetch_with_str_and_init(&url, &init))
            .await
            .map_err(|e| error_text(&e))?;
    let response: web_sys::Response = response.unchecked_into();

    if !response.ok() {
        // `new Error (response .statusText || response .status)`.
        let text = response.status_text();

        return Err(if text.is_empty() {
            format!("Error: {}", response.status())
        } else {
            format!("Error: {text}")
        });
    }

    let buffer =
        wasm_bindgen_futures::JsFuture::from(response.array_buffer().map_err(|e| error_text(&e))?)
            .await
            .map_err(|e| error_text(&e))?;

    Ok(js_sys::Uint8Array::new(&buffer).to_vec())
}

/// Serves a world's Inline, EXTERNPROTO, texture and Anchor URLs with the page's asynchronous `fetch`. The runtime
/// asks [`x3d_runtime::Fetcher::poll`] before a load runs and waits until the response is there; then
/// [`x3d_runtime::Fetcher::fetch`] answers from the cache. Repeated requests of a URL share one response.
struct WebFetcher {
    cache: Rc<RefCell<FetchCache>>,
}

impl WebFetcher {
    fn start(&self, key: String) {
        let weak: Weak<RefCell<FetchCache>> = Rc::downgrade(&self.cache);
        let signal = {
            let mut cache = self.cache.borrow_mut();

            cache.entries.insert(key.clone(), Entry::Pending);
            cache.controller.as_ref().map(|c| c.signal())
        };

        wasm_bindgen_futures::spawn_local(async move {
            let result = fetch_bytes(key.clone(), signal).await;

            // A response after the world went attaches to nothing.
            let Some(cache) = weak.upgrade() else {
                return;
            };
            let mut cache = cache.borrow_mut();

            cache.entries.insert(
                key,
                match result {
                    Ok(bytes) => Entry::Loaded(bytes.into()),
                    Err(error) => Entry::Failed(error),
                },
            );
            cache.settled = true;
            cache.wake();
        });
    }
}

impl x3d_runtime::Fetcher for WebFetcher {
    fn is_async(&self) -> bool {
        true
    }

    // The page plays the videos of MovieTextures in `<video>` elements (WebBrowser::take_video_ops).
    fn plays_video(&self) -> bool {
        true
    }

    fn poll(&self, url: &url::Url) -> FetchPoll {
        // `fetch (dataURL)` needs no request: the browser decodes the URL itself (see `fetch`). One that does not
        // decode rejects at once.
        if url.scheme() == "data" {
            return match x3d_parse::decode_data_url(url.as_str()) {
                Ok(_) => FetchPoll::Loaded,
                Err(_) => FetchPoll::Failed,
            };
        }

        let key = cache_key(url);
        let state = match self.cache.borrow().entries.get(&key) {
            Some(Entry::Pending) => Some(FetchPoll::Pending),
            Some(Entry::Loaded(_)) => Some(FetchPoll::Loaded),
            Some(Entry::Failed(_)) => Some(FetchPoll::Failed),
            None => None,
        };

        state.unwrap_or_else(|| {
            self.start(key);
            FetchPoll::Pending
        })
    }

    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        if url.scheme() == "data" {
            return x3d_parse::decode_data_url(url.as_str());
        }

        let key = cache_key(url);

        match self.cache.borrow().entries.get(&key) {
            Some(Entry::Loaded(bytes)) => return Ok(bytes.to_vec()),
            Some(Entry::Failed(error)) => return Err(error.clone()),
            _ => {}
        }

        // A URL the runtime did not ask for first. Every fallback chain (the world's, Inline, the textures, EXTERNPROTO,
        // Anchor, glTF buffers, fonts, MovieTexture, sounds) waits for its responses (FetchPoll), so this is a load
        // that does not wait: the page never blocks on a request, so this fetch fails now; the response serves later
        // requests.
        if !self.cache.borrow().entries.contains_key(&key) {
            self.start(key);
        }

        web_sys::console::warn_1(
            &format!("X_ITE_R: {url} was requested before it was loaded.").into(),
        );
        Err("TypeError: Failed to fetch".to_string())
    }

    fn refresh(&self, url: &url::Url) {
        if url.scheme() == "data" {
            return;
        }

        // The entry is Pending until a response comes; each response replaces the one before.
        self.start(cache_key(url));
    }

    fn forget_failure(&self, url: &url::Url) {
        if url.scheme() == "data" {
            return;
        }

        let key = cache_key(url);
        let mut cache = self.cache.borrow_mut();

        if matches!(cache.entries.get(&key), Some(Entry::Failed(_))) {
            cache.entries.remove(&key);
        }
    }

    /// `follow_buffers` only reaches this after `poll` says `Failed` or `Loaded`.
    /// Thus we do not need to `self.start(key)` for missing entries.
    fn fetch_error(&self, url: &url::Url) -> Option<String> {
        if url.scheme() == "data" {
            return x3d_parse::decode_data_url(url.as_str()).err();
        }
        let key = cache_key(url);
        match self.cache.borrow().entries.get(&key) {
            Some(Entry::Failed(error)) => Some(error.clone()),
            _ => None,
        }
    }
}

/// One world: the session and its fetches.
struct World {
    session: Session,
    cache: Rc<RefCell<FetchCache>>,
}

/// A loadURL of the page: the world's FileLoader ([`x3d_runtime::Runtime::load_page_document`]). It runs in a
/// runtime of its own, whose log has its messages, with the fetches of the world it makes.
struct PageLoad {
    rt: x3d_runtime::Runtime,
    /// The candidates; FileLoader.abort () leaves the one in flight as the last.
    urls: Vec<String>,
    base: String,
    cache: Rc<RefCell<FetchCache>>,
    /// How much of the log [`WebBrowser::take_log`] returned.
    log_seen: usize,
}

thread_local! {
    /// The id the next world or load of any `WebBrowser` of this page gets. One counter for all browsers, so a world id names
    /// one world in the whole page: the in-chain field operations ([`crate::chain`]) find their runtime by this id alone, and
    /// two canvases never share one.
    static NEXT_WORLD_ID: std::cell::Cell<u32> = const { std::cell::Cell::new(1) };
}

/// The node ids of a `rootNodes` list from the page: `null` and `undefined` are `None`; anything else must be an integer
/// number in `0 ..= u32::MAX`, so a bad value never turns into a valid node id by `as` conversion.
fn root_node_list(list: &js_sys::Array) -> Result<Vec<Option<u32>>, JsError> {
    list.iter()
        .map(|v| {
            if v.is_null() || v.is_undefined() {
                return Ok(None);
            }

            match v.as_f64() {
                Some(n)
                    if n.is_finite()
                        && n.fract() == 0.0
                        && (0.0..=f64::from(u32::MAX)).contains(&n) =>
                {
                    Ok(Some(n as u32))
                }
                _ => Err(JsError::new("rootNodes: a node id must be an integer")),
            }
        })
        .collect()
}

fn next_world_id() -> u32 {
    NEXT_WORLD_ID.with(|n| {
        let id = n.get();

        n.set(id + 1);
        id
    })
}

#[wasm_bindgen]
pub struct WebBrowser {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    renderer: Renderer,
    adapter_info: String,
    max_texture_dimension: u32,
    lost: Arc<AtomicBool>,
    worlds: HashMap<u32, World>,
    /// The page's loadURL calls whose world has not parsed yet, by id (the ids of worlds and loads differ).
    loads: HashMap<u32, PageLoad>,
    /// The latest `audioContext .currentTime` the page gave ([`WebBrowser::set_audio_clock`]); a world that starts
    /// later (replaceWorld while the canvas is detached) takes it, not the clock 0 of a new audio context.
    audio_clock: Option<f64>,
    /// The page's Mute and SoundIntensity options ([`WebBrowser::set_sound_options`]); a world that starts takes them.
    sound_options: Option<(bool, f64)>,
    /// The page's browser options the engine reads itself ([`WebBrowser::set_engine_option`]); a world that starts takes them.
    engine_options: Vec<(String, x3d_runtime::browser_options::OptionValue)>,
    /// The audio engines the page has been told about ([`WebBrowser::take_audio_ops`]), by `AudioEngine::uid`.
    audio_known: std::collections::HashSet<u64>,
    /// The buffers of the last [`WebBrowser::take_audio_ops`] ([`WebBrowser::audio_buffer`]).
    audio_payloads: x3d_runtime::sound::ops::Payloads,
    /// The last [`WebBrowser::take_video_ops`] told the page about video elements.
    video_known: bool,
    current: Option<u32>,
    /// X3DBrowserContext.isLive: false between endUpdate and beginUpdate, also over a replaceWorld.
    live: bool,
    /// The page's Script realm factory (`setScriptRealm`); none: no Script runs.
    script_realm: Option<js_sys::Function>,
    /// The page's console writer (`setLogSink`): a Script's `print` and `console` lines go to it in the Script's call.
    log_sink: Option<js_sys::Function>,
    /// The page's field callback sink (`setFieldEventSink`): see [`crate::chain`].
    field_event_sink: Option<js_sys::Function>,
    /// Worlds that a replace removed while the page may still hold functions or node objects of their Scripts
    /// (issue #202), by their Script host's key. The page tells when it dropped them all ([`WebBrowser::release_retired`]).
    retired: HashMap<u64, Session>,
    /// The last log lines of the world that a replace removed (what its Scripts' `shutdown` printed): the next
    /// [`WebBrowser::take_log`] of the current world gives them first.
    retired_log: Vec<String>,
}

/// A route as the array `routes` and its relatives give.
fn route_js(r: &RouteInfo) -> js_sys::Array {
    let a = js_sys::Array::new();

    a.push(&(r.id as f64).into());
    a.push(&r.source.into());
    a.push(&r.source_field.as_str().into());
    a.push(&r.destination.into());
    a.push(&r.destination_field.as_str().into());
    a.push(&r.raw_source_field.as_str().into());
    a.push(&r.raw_destination_field.as_str().into());
    a
}

/// `Err` when no world `id` exists.
fn no_world() -> JsError {
    JsError::new("X_ITE_R: the scene is gone.")
}

#[wasm_bindgen]
impl WebBrowser {
    /// The device and surface of `canvas` (WebGPU). Fails when the page has no WebGPU adapter, or the adapter lacks
    /// what the renderer needs (the WebGPU default limits, a usable canvas format).
    pub async fn create(canvas: web_sys::HtmlCanvasElement) -> Result<WebBrowser, JsError> {
        // The page's Sound nodes are real browser nodes: the engine records its Web Audio calls for the page to make
        // (web/audio_mirror.mjs). Before the first world, because an engine makes its choice when it is created.
        x3d_runtime::sound::set_default_record_ops(true);

        let (width, height) = (canvas.width().max(1), canvas.height().max(1));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsError::new(&format!("create_surface: {e}")))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| JsError::new(&format!("no WebGPU adapter: {e}")))?;
        let limits = adapter.limits();
        let mut missing = vec![];

        // The renderer is written for the WebGPU default limits: an adapter below them cannot run it.
        wgpu::Limits::default().check_limits_with_fail_fn(&limits, false, |name, want, have| {
            missing.push(format!("{name} {have} < {want}"))
        });

        if !missing.is_empty() {
            return Err(JsError::new(&format!(
                "the WebGPU adapter is below the limits X_ITE_R needs: {}",
                missing.join(", ")
            )));
        }

        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("x3d-web"),
                required_features: Renderer::wanted_features(&adapter),
                required_limits: limits.clone(),
                ..Default::default()
            })
            .await
            .map_err(|e| JsError::new(&format!("request_device: {e}")))?;

        device.on_uncaptured_error(std::sync::Arc::new(|e| {
            web_sys::console::error_1(&format!("WebGPU: {e}").into());
        }));

        let lost = Arc::new(AtomicBool::new(false));
        let flag = lost.clone();

        device.set_device_lost_callback(move |reason, message| {
            flag.store(true, Ordering::SeqCst);
            web_sys::console::error_1(
                &format!("WebGPU device lost ({reason:?}): {message}").into(),
            );
        });

        // The shaders write display-encoded values: draw through a non-sRGB view (see x3d-view's window code).
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| x3d_render::gpu::target_format_ok(f.remove_srgb_suffix()))
            .ok_or_else(|| {
                JsError::new(&format!("no usable canvas format in {:?}", caps.formats))
            })?;
        let view_format = format.remove_srgb_suffix();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
                wgpu::CompositeAlphaMode::Opaque
            } else {
                caps.alpha_modes[0]
            },
            view_formats: if view_format != format {
                vec![view_format]
            } else {
                vec![]
            },
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        surface.configure(&device, &config);

        let mut renderer = Renderer::new(device.clone(), queue);
        let info = adapter.get_info();

        renderer.set_supported_samples(Renderer::supported_sample_counts(
            &adapter,
            &device,
            view_format,
        ));
        renderer.set_sample_count(crate::DEFAULT_MULTISAMPLING);

        Ok(WebBrowser {
            surface,
            device,
            config,
            view_format,
            renderer,
            adapter_info: format!("{} ({:?}, {:?})", info.name, info.backend, info.device_type),
            max_texture_dimension: limits.max_texture_dimension_2d,
            lost,
            worlds: HashMap::new(),
            loads: HashMap::new(),
            audio_clock: None,
            sound_options: None,
            engine_options: Vec::new(),
            audio_known: Default::default(),
            audio_payloads: Default::default(),
            video_known: false,
            current: None,
            live: true,
            script_realm: None,
            log_sink: None,
            field_event_sink: None,
            retired: HashMap::new(),
            retired_log: vec![],
        })
    }

    /// The WebGPU adapter: name, backend, device type.
    #[wasm_bindgen(getter, js_name = adapterInfo)]
    pub fn adapter_info(&self) -> String {
        self.adapter_info.clone()
    }

    /// The largest canvas side the device draws (X3DRenderingContext.limitFramebufferSize).
    #[wasm_bindgen(getter, js_name = maxTextureDimension)]
    pub fn max_texture_dimension(&self) -> u32 {
        self.max_texture_dimension
    }

    /// Is the WebGPU device lost? The browser then draws nothing more.
    #[wasm_bindgen(getter)]
    pub fn lost(&self) -> bool {
        self.lost.load(Ordering::SeqCst)
    }

    /// For tests: destroy the device, as a lost GPU does.
    #[wasm_bindgen(js_name = loseDevice)]
    pub fn lose_device(&self) {
        self.device.destroy();
    }

    fn add(
        &mut self,
        world: crate::session::ParsedWorld,
        cache: Rc<RefCell<FetchCache>>,
        now: f64,
    ) -> u32 {
        let id = next_world_id();
        let factory = self.script_realm.clone();
        let sink = self.log_sink.clone();
        let event_sink = self.field_event_sink.clone();
        let engine_options = self.engine_options.clone();
        let session =
            Session::begin_with_scripts(world, now, self.config.width, self.config.height, |rt| {
                // The page's options before the world's loads start (LoadUrlObjects is read when an Inline starts).
                for (name, value) in engine_options {
                    rt.browser_options.set_option(&name, value);
                }

                // The page's console and field callbacks run in the route chain (issue #202): the lines made before an
                // event go to the console first, then the page's callbacks of the event are called.
                if let Some(sink) = &sink {
                    let sink = sink.clone();

                    rt.console_sink = Some(Box::new(move |line| {
                        let _ = sink.call1(&JsValue::NULL, &JsValue::from_str(line));
                    }));
                }

                if let Some(event_sink) = event_sink {
                    if rt.console_sink.is_some() {
                        rt.page_event_sink = Some(Box::new(move |rt, k, value| {
                            let name = rt.field(k).name.clone();
                            let data = data_js(&x3d_runtime::data::value_data(value));
                            let args = js_sys::Array::of4(
                                &f64::from(id).into(),
                                &f64::from(k.0).into(),
                                &name.into(),
                                &data,
                            );

                            // The page's callbacks run with the runtime reachable through the `chain*` exports.
                            let halted = crate::chain::enter(rt, id, || {
                                event_sink.apply(&JsValue::NULL, &args)
                            });

                            // The page reports an error of a callback itself and answers `true`; an error that escapes it
                            // halts the same way.
                            !matches!(halted, Ok(v) if v.as_bool() == Some(false))
                        }));
                    }
                }

                let factory = factory.as_ref()?;

                match crate::script::PageJsHost::new(rt, factory) {
                    Ok(Some(host)) => Some(Box::new(host) as Box<dyn x3d_runtime::ScriptHost>),
                    Ok(None) => None,
                    Err(e) => {
                        rt.log_error(format!(
                            "X_ITE_R: the Scripts of this world cannot run: {e}"
                        ));
                        None
                    }
                }
            });

        self.worlds.insert(id, World { session, cache });
        id
    }

    /// The page's Script realm factory, or null (the default): no Script runs. For each world that starts after the
    /// call, the engine calls `factory (hostId)`, which returns the realm (`web/pkg/script.mjs` createScriptRealm) of
    /// the world's Scripts, or null when they must not run. A Script runs with the privileges of the page.
    #[wasm_bindgen(js_name = setScriptRealm)]
    pub fn set_script_realm(&mut self, factory: Option<js_sys::Function>) {
        self.script_realm = factory;
    }

    /// The page's console writer, or null: `sink (line)` takes a line `log: text`, `warn: text` or `error: text`. A Script's
    /// `print` and `console` lines of the worlds that start after the call go to it in the Script's call (when nothing waits
    /// before them), not when the engine returns (issue #202).
    #[wasm_bindgen(js_name = setLogSink)]
    pub fn set_log_sink(&mut self, sink: Option<js_sys::Function>) {
        self.log_sink = sink;
    }

    /// The page's field callback sink, or null (the default: the page takes the events after the frame,
    /// `takeFieldEvents`). For the worlds that start after the call (and only with a log sink, [`WebBrowser::set_log_sink`]),
    /// `sink (scene, node, fieldName, data)` is called in the route chain, where X_ITE's X3DField.processEvent calls the field
    /// callbacks, and answers `false`, or `true` when a callback threw (the event processing halts). While it runs the page
    /// reaches the world through the `chain*` functions only; the `WebBrowser` is borrowed by the call that runs the chain
    /// (issue #202, [`crate::chain`]).
    #[wasm_bindgen(js_name = setFieldEventSink)]
    pub fn set_field_event_sink(&mut self, sink: Option<js_sys::Function>) {
        self.field_event_sink = sink;
    }

    /// The page dropped every function and node object of the Scripts of the replaced world with the Script host key `host`:
    /// the engine releases the world it kept for them (issue #202). Unknown keys do nothing.
    #[wasm_bindgen(js_name = releaseRetired)]
    pub fn release_retired(&mut self, host: f64) {
        self.retired.remove(&(host as u64));
    }

    /// For tests: how many replaced worlds the engine keeps for functions and node objects the page holds.
    #[wasm_bindgen(getter, js_name = retiredWorlds)]
    pub fn retired_worlds(&self) -> u32 {
        self.retired.len() as u32
    }

    /// Parse the world `data` from `url` (FileLoader) and start its loads: the id of the new scene. The current world
    /// stays. A world that does not parse is an error.
    pub fn prepare(&mut self, data: &[u8], url: &str, now: f64) -> Result<u32, JsError> {
        let cache = FetchCache::new();
        let fetcher = Box::new(WebFetcher {
            cache: cache.clone(),
        });
        let world = Session::parse(data, url, fetcher).map_err(|e| JsError::new(&e))?;

        Ok(self.add(world, cache, now))
    }

    /// X3DBrowser.loadURL (url) of the page: start the world's FileLoader with the candidates `urls`, resolved against
    /// `base`. The id of the load: [`WebBrowser::load_step`] goes on with it, [`WebBrowser::take_log`] gives its
    /// messages, [`WebBrowser::progress`] waits for its responses and [`WebBrowser::discard`] ends it.
    #[wasm_bindgen(js_name = loadURL)]
    pub fn load_url(&mut self, urls: Vec<String>, base: &str) -> Result<u32, JsError> {
        let cache = FetchCache::new();
        let fetcher = Box::new(WebFetcher {
            cache: cache.clone(),
        });
        let scene = x3d_parse::parse_string("#X3D V4.0 utf8\n", base)
            .map_err(|e| JsError::new(&format!("{e:?}")))?;
        let id = next_world_id();

        self.loads.insert(
            id,
            PageLoad {
                rt: x3d_runtime::Runtime::new(scene, fetcher),
                urls,
                base: base.to_string(),
                cache,
                log_seen: 0,
            },
        );
        Ok(id)
    }

    /// Go on with load `id` with the responses that arrived: 0 while the response of a candidate is still to come,
    /// the id of the new scene (whose own loads go on, see [`WebBrowser::loaded`]), or -1 when every candidate
    /// failed (or the load is gone). The messages of the failed candidates are in the load's log.
    #[wasm_bindgen(js_name = loadStep)]
    pub fn load_step(&mut self, id: u32, now: f64) -> f64 {
        let Some(load) = self.loads.get_mut(&id) else {
            return -1.0;
        };

        load.cache.borrow_mut().settled = false;
        load.rt.follow_buffers();

        let cache = load.cache.clone();
        let base = load.base.clone();
        let mut parsed = None;
        let result = load
            .rt
            .load_page_document(&load.base, &load.urls, |rt, loaded| {
                let (data, url) = match loaded {
                    x3d_runtime::loader::Loaded::Data(data, url) => (data, url.to_string()),
                    // `createX3DFromString (this .resolvedURL, data)` without a resolved URL: `new URL (null, base)`.
                    x3d_runtime::loader::Loaded::Script(source) => (
                        source.into_bytes(),
                        url::Url::parse(&base)
                            .and_then(|b| b.join("null"))
                            .map_or_else(|_| base.clone(), String::from),
                    ),
                };
                let fetcher = Box::new(WebFetcher {
                    cache: cache.clone(),
                });

                parsed = Some(Session::parse_page(rt, &data, &url, fetcher)?);
                Ok(())
            });

        match (result, parsed) {
            (x3d_runtime::loader::LoadResult::Suspended, _) => 0.0,
            (x3d_runtime::loader::LoadResult::Loaded, Some(world)) => {
                f64::from(self.add(world, cache, now))
            }
            _ => -1.0,
        }
    }

    /// FileLoader.abort () of load `id`: the candidate in flight is the last one, and its request ends (AbortError,
    /// which the next [`WebBrowser::load_step`] reports). A candidate that waits for its glTF buffers goes on.
    #[wasm_bindgen(js_name = abortLoad)]
    pub fn abort_load(&mut self, id: u32) {
        let Some(load) = self.loads.get_mut(&id) else {
            return;
        };

        let mut buffers_wait = false;

        if let Some(n) = load.rt.abort_page_document() {
            load.urls.truncate(n);

            // A candidate whose response is in (or a data URL) waits for the buffers of its glTF. Their requests
            // are GLTF2Parser's own FileLoaders, which the abort does not end: the load goes on and is discarded.
            let candidate = url::Url::parse(&load.base)
                .ok()
                .zip(load.urls.last())
                .and_then(|(base, c)| x3d_runtime::loader::join_url(&base, c).ok());

            buffers_wait = candidate.is_some_and(|u| {
                u.scheme() == "data"
                    || matches!(
                        load.cache.borrow().entries.get(&cache_key(&u)),
                        Some(Entry::Loaded(_))
                    )
            });
        }

        if !buffers_wait && let Some(c) = &load.cache.borrow().controller {
            // The followed buffers' requests share the controller: they end and log nothing.
            load.rt.followed_buffers.clear();
            c.abort();
        }
    }

    /// The glTF buffers that load `id` follows after a candidate was rejected while they were in flight
    /// ([`x3d_runtime::Runtime::follow_buffers`]): the failures that came go to its log. Whether some are still in
    /// flight; the page keeps the load until they are in.
    #[wasm_bindgen(js_name = followLoad)]
    pub fn follow_load(&mut self, id: u32) -> bool {
        let Some(load) = self.loads.get_mut(&id) else {
            return false;
        };

        load.cache.borrow_mut().settled = false;
        load.rt.follow_buffers()
    }

    /// X3DBrowser.createX3DFromString of the page: parse `text` with the world URL `url` and start its loads. The
    /// error is the message of X3DBrowser's rejection.
    #[wasm_bindgen(js_name = prepareString)]
    pub fn prepare_string(&mut self, text: &str, url: &str, now: f64) -> Result<u32, JsError> {
        let cache = FetchCache::new();
        let fetcher = Box::new(WebFetcher {
            cache: cache.clone(),
        });
        let world = Session::parse_string(text, url, fetcher).map_err(|e| JsError::new(&e))?;

        Ok(self.add(world, cache, now))
    }

    /// Continue the loads of scene `id` with the responses that arrived: whether it is loaded. `false` for a scene
    /// that is gone.
    pub fn loaded(&mut self, id: u32, now: f64) -> bool {
        let Some(w) = self.worlds.get_mut(&id) else {
            return false;
        };

        w.cache.borrow_mut().settled = false;
        w.session.poll_load(now)
    }

    /// A Promise that resolves when a response of scene or load `id` arrives (at once when one arrived since
    /// [`WebBrowser::loaded`], or the scene is gone).
    pub fn progress(&self, id: u32) -> js_sys::Promise {
        let cache = self
            .worlds
            .get(&id)
            .map(|w| w.cache.clone())
            .or_else(|| self.loads.get(&id).map(|l| l.cache.clone()));

        js_sys::Promise::new(&mut |resolve, _| match &cache {
            Some(c) if !c.borrow().settled => c.borrow_mut().waiters.push(resolve),
            _ => {
                let _ = resolve.call0(&JsValue::NULL);
            }
        })
    }

    /// X3DBrowser.replaceWorld with the loaded scene `id` at the browser time `now`: the current world goes (its
    /// shutdown, its requests), then the new one goes live and is drawn with its own rendering properties. The page
    /// fires `shutdown` before this call, while the old world is still current.
    pub fn replace(&mut self, id: u32, now: f64) -> Result<(), JsError> {
        let w = self.worlds.get(&id).ok_or_else(no_world)?;

        if w.session.is_live() || !w.session.is_loaded() {
            return Err(JsError::new(if w.session.is_live() {
                "X_ITE_R: the scene is already a world."
            } else {
                "X_ITE_R: the scene is still loading."
            }));
        }

        if let Some(old) = self.current.take() {
            if let Some(mut w) = self.worlds.remove(&old) {
                w.session.unload();
                self.retired_log.extend(w.session.take_log());

                if let Some(key) = w.session.retire() {
                    self.retired.insert(key, w.session);
                }
            }
        }

        let w = self.worlds.get_mut(&id).ok_or_else(no_world)?;

        w.session.resize(self.config.width, self.config.height);
        w.session.set_live(self.live);

        // The page's Mute and SoundIntensity first: go_live activates the world's sound destinations, and they take the
        // gain of these options, not the default one (a world that starts muted is never heard).
        if let Some((mute, intensity)) = self.sound_options {
            w.session.set_sound_options(mute, intensity);
        }

        w.session.go_live(now);

        // The world's audio context starts with the clock 0: it takes the page's latest clock at once, so the first
        // frame after a replaceWorld while the canvas was detached is no jump from 0 (a clip would end in it).
        if let Some(time) = self.audio_clock {
            w.session.set_audio_clock(time);
        }

        if let Some(v) = w.session.viewer() {
            v.rendering_properties().apply(&mut self.renderer);
        }

        self.current = Some(id);
        Ok(())
    }

    /// Release scene or load `id` (not the current world): its loads end. A load's world that parsed stays.
    pub fn discard(&mut self, id: u32) {
        if self.current != Some(id) {
            self.worlds.remove(&id);
        }

        self.loads.remove(&id);
    }

    /// For tests: how many loadURL records ([`WebBrowser::load_url`]) are not discarded yet.
    #[wasm_bindgen(getter, js_name = loadRecords)]
    pub fn load_records(&self) -> u32 {
        self.loads.len() as u32
    }

    /// For tests: how many scenes (the world and those not discarded yet) the engine holds.
    #[wasm_bindgen(getter, js_name = sceneRecords)]
    pub fn scene_records(&self) -> u32 {
        self.worlds.len() as u32
    }

    /// The id of the current world.
    #[wasm_bindgen(getter)]
    pub fn current(&self) -> Option<u32> {
        self.current
    }

    #[wasm_bindgen(getter, js_name = hasWorld)]
    pub fn has_world(&self) -> bool {
        self.current.is_some()
    }

    /// Does scene `id` exist?
    pub fn exists(&self, id: u32) -> bool {
        self.worlds.contains_key(&id)
    }

    fn session(&self, id: u32) -> Result<&Session, JsError> {
        self.worlds
            .get(&id)
            .map(|w| &w.session)
            .ok_or_else(no_world)
    }

    fn session_mut(&mut self, id: u32) -> Result<&mut Session, JsError> {
        self.worlds
            .get_mut(&id)
            .map(|w| &mut w.session)
            .ok_or_else(no_world)
    }

    fn current_mut(&mut self) -> Option<&mut Session> {
        let id = self.current?;

        self.worlds.get_mut(&id).map(|w| &mut w.session)
    }

    /// The world URL of scene `id`.
    #[wasm_bindgen(js_name = worldURL)]
    pub fn world_url(&self, id: u32) -> Option<String> {
        self.worlds.get(&id).map(|w| w.session.world_url())
    }

    /// The world generation of scene `id`: it changes when an Anchor replaces the world inside the scene, and node
    /// handles of the old world must not reach the new one.
    #[wasm_bindgen(js_name = worldGeneration)]
    pub fn world_generation(&self, id: u32) -> Option<f64> {
        self.worlds
            .get(&id)
            .map(|w| w.session.world_generation() as f64)
    }

    /// X3DBrowser.createVrmlFromString of the page: parse `#VRML V2.0 utf8` + `text` into scene `id` (the current world, or
    /// an empty scene of the page's when there is no world yet) as nodes of its arena, see
    /// [`Session::create_vrml_from_string`], with the page's `base` URL. The ids of the new scene's root nodes (-1 is
    /// NULL). The error is the thrown Error's `String (error)`.
    #[wasm_bindgen(js_name = createVrmlFromString)]
    pub fn create_vrml_from_string(
        &mut self,
        id: u32,
        text: &str,
        base: &str,
    ) -> Result<Vec<i64>, JsError> {
        self.session_mut(id)?
            .create_vrml_from_string(text, base)
            .map(|nodes| nodes.into_iter().map(|n| n.map_or(-1, i64::from)).collect())
            .map_err(|e| JsError::new(&e))
    }

    /// X3DBrowser.createX3DFromURL (url, node, event) / createVrmlFromURL of the page: start the FileLoader of the X3DWorld
    /// in the runtime of the current world (scene `id`), with the candidates `urls` resolved against `base`. The id of the
    /// request; [`WebBrowser::take_scene_requests`] gives its outcome.
    #[wasm_bindgen(js_name = createX3DFromURL)]
    pub fn create_x3d_from_url(
        &mut self,
        id: u32,
        urls: Vec<String>,
        base: &str,
    ) -> Result<f64, JsError> {
        Ok(self.session_mut(id)?.create_x3d_from_url(urls, base) as f64)
    }

    /// The requests of [`WebBrowser::create_x3d_from_url`] of scene `id` that settled: `[request, nodes]` pairs, where
    /// `nodes` is the list of root node ids of the scene (-1 is NULL), or the message of the rejection (a string).
    #[wasm_bindgen(js_name = takeSceneRequests)]
    pub fn take_scene_requests(&mut self, id: u32) -> js_sys::Array {
        let list = js_sys::Array::new();

        if let Ok(session) = self.session_mut(id) {
            for (request, outcome) in session.take_scene_requests() {
                list.push(&(request as f64).into());

                match outcome {
                    Ok(nodes) => {
                        let ids = js_sys::Array::new();

                        for n in nodes {
                            ids.push(&n.map_or(-1.0, f64::from).into());
                        }

                        list.push(&ids);
                    }
                    Err(message) => {
                        list.push(&message.into());
                    }
                }
            }
        }

        list
    }

    /// X3DScene.profile and components of scene `id`: `[profile name or null, [name, level, name, level, ...]]`.
    #[wasm_bindgen(js_name = profileAndComponents)]
    pub fn profile_and_components(&self, id: u32) -> Result<js_sys::Array, JsError> {
        let (profile, components) = self.session(id)?.profile_and_components();
        let list = js_sys::Array::new();

        for (name, level) in components {
            list.push(&name.into());
            list.push(&f64::from(level).into());
        }

        Ok(js_sys::Array::of2(
            &profile.map_or(JsValue::NULL, |p| p.into()),
            &list,
        ))
    }

    /// What the elements of the XML text of scene `id` made, four numbers per element (see [`Session::xml_targets`]).
    #[wasm_bindgen(js_name = xmlTargets)]
    pub fn xml_targets(&self, id: u32) -> Vec<u32> {
        self.worlds
            .get(&id)
            .map_or(vec![], |w| w.session.xml_targets())
    }

    /// DOMIntegration.processAddedNode for scene `id`: the XML of one element parsed into the context of node `parent`
    /// (-1: among the root nodes of the world); what its elements made, as [`WebBrowser::xml_targets`].
    #[wasm_bindgen(js_name = xmlChild)]
    pub fn xml_child(&mut self, id: u32, parent: i32, text: &str) -> Result<Vec<u32>, JsError> {
        let parent = u32::try_from(parent).ok();

        self.session_mut(id)?
            .xml_child(parent, text)
            .map_err(|e| JsError::new(&e))
    }

    /// DOMIntegration.processAttribute for scene `id`: the attribute `name="value"` read into the field of node `node`.
    #[wasm_bindgen(js_name = xmlAttribute)]
    pub fn xml_attribute(
        &mut self,
        id: u32,
        node: u32,
        name: &str,
        value: &str,
    ) -> Result<(), JsError> {
        self.session_mut(id)?.xml_attribute(node, name, value);
        Ok(())
    }

    /// DOMIntegration.processRemovedNode for scene `id`: X3DNode.dispose of `node`.
    #[wasm_bindgen(js_name = disposeNode)]
    pub fn dispose_node(&mut self, id: u32, node: u32) -> Result<(), JsError> {
        self.session_mut(id)?.dispose_node(node);
        Ok(())
    }

    /// Route `route` of context `ctx` of scene `id` is deleted.
    #[wasm_bindgen(js_name = deleteRouteById)]
    pub fn delete_route_by_id(&mut self, id: u32, ctx: u32, route: u32) -> Result<(), JsError> {
        self.session_mut(id)?.delete_route_in(ctx as usize, route as usize);
        Ok(())
    }

    /// The node id of a DEF name of scene `id`, or `undefined`.
    #[wasm_bindgen(js_name = namedNode)]
    pub fn named_node(&self, id: u32, name: &str) -> Option<u32> {
        self.worlds.get(&id)?.session.named_node(name)
    }

    /// `updateNamedNode (name, node)` of scene `id` (also `addNamedNode`, after the page's checks): see
    /// [`Session::set_named_node`].
    #[wasm_bindgen(js_name = setNamedNode)]
    pub fn set_named_node(&mut self, id: u32, name: &str, node: u32) -> Result<(), JsError> {
        self.session_mut(id)?
            .set_named_node(name, node)
            .map_err(|e| JsError::new(&e))
    }

    /// `removeNamedNode (name)` of scene `id`.
    #[wasm_bindgen(js_name = removeNamedNode)]
    pub fn remove_named_node(&mut self, id: u32, name: &str) -> Result<(), JsError> {
        self.session_mut(id)?.remove_named_node(name);
        Ok(())
    }

    /// `createProto (name)` of scene `id`: the node id of the new instance, or undefined when the scene declares no such PROTO
    /// or EXTERNPROTO.
    #[wasm_bindgen(js_name = createProto)]
    pub fn create_proto(&mut self, id: u32, name: &str) -> Result<Option<u32>, JsError> {
        Ok(self.session_mut(id)?.create_proto(name))
    }

    // --- PROTO and EXTERNPROTO declarations of scene `id`. A declaration is its index in `protos` (`extern_` false) or
    // `externprotos` (true). ----------------------------------------------------------------------------------------

    /// The declarations of the scene in the order of `getProtoDeclarations ()` / `getExternProtoDeclarations ()`.
    #[wasm_bindgen(js_name = declIds)]
    pub fn decl_ids(&self, id: u32, extern_: bool) -> Vec<u32> {
        self.worlds.get(&id).map_or_else(Vec::new, |w| {
            w.session
                .decl_ids(extern_)
                .into_iter()
                .map(|i| i as u32)
                .collect()
        })
    }

    /// `getProtoDeclaration (name)` / `getExternProtoDeclaration (name)`: the index, or an error with X_ITE's text.
    #[wasm_bindgen(js_name = declGet)]
    pub fn decl_get(&self, id: u32, extern_: bool, name: &str) -> Result<u32, JsError> {
        self.session(id)?
            .decl_get(extern_, name)
            .map(|i| i as u32)
            .map_err(|e| JsError::new(&e))
    }

    /// `add`, `update` or `remove` of `(Extern)ProtoDeclaration (name, declaration)` (`op` is `add`, `update` or `remove`).
    /// `decl` is the index of the declaration the page passes, `-1` for an argument that is no declaration of that kind
    /// and `-2` for a declaration of another scene.
    #[wasm_bindgen(js_name = declOp)]
    pub fn decl_op(
        &mut self,
        id: u32,
        extern_: bool,
        op: &str,
        name: &str,
        decl: i32,
    ) -> Result<(), JsError> {
        let op = match op {
            "add" => x3d_runtime::DeclOp::Add,
            "remove" => x3d_runtime::DeclOp::Remove,
            _ => x3d_runtime::DeclOp::Update,
        };
        let decl = match decl {
            -1 => None,
            -2 => Some(usize::MAX),
            n => Some(n as usize),
        };

        self.session_mut(id)?
            .decl_op(extern_, op, name, decl)
            .map_err(|e| JsError::new(&e))
    }

    /// `getUniqueProtoName (name)` / `getUniqueExternProtoName (name)`.
    #[wasm_bindgen(js_name = declUniqueName)]
    pub fn decl_unique_name(&self, id: u32, extern_: bool, name: &str) -> Result<String, JsError> {
        Ok(self.session(id)?.decl_unique_name(extern_, name))
    }

    /// What the SAI object of a declaration reads, or undefined for an index the scene does not have: an object with
    /// `name`, `ctx` (the owning execution context), `hasMetadata`, `fields` (`[name, accessType, typeName, value]` each,
    /// `metadata` first), and for an EXTERNPROTO `urls`, `loadState` (a constant name), `load`, `autoRefresh`,
    /// `autoRefreshTimeLimit` and `loaded` (the index of the loaded PROTO, or undefined).
    #[wasm_bindgen(js_name = declInfo)]
    pub fn decl_info(&self, id: u32, extern_: bool, decl: u32) -> Result<JsValue, JsError> {
        let Some(info) = self.session(id)?.decl_info(extern_, decl as usize) else {
            return Ok(JsValue::UNDEFINED);
        };
        let o = js_sys::Object::new();
        let set = |k: &str, v: JsValue| {
            let _ = js_sys::Reflect::set(&o, &k.into(), &v);
        };
        let fields = js_sys::Array::new();

        for f in &info.fields {
            fields.push(&js_sys::Array::of4(
                &f.name.as_str().into(),
                &f64::from(f.access).into(),
                &f.type_name.into(),
                &data_js(&x3d_runtime::data::value_data(&f.value)),
            ));
        }

        set("name", info.name.as_str().into());
        set("ctx", (info.ctx as u32).into());
        set("hasMetadata", info.has_metadata.into());
        set("fields", fields.into());

        if let Some(urls) = &info.urls {
            set(
                "urls",
                urls.iter()
                    .map(|u| JsValue::from(u.as_str()))
                    .collect::<js_sys::Array>()
                    .into(),
            );
        }

        if let Some(state) = info.load_state {
            use x3d_runtime::nodes::LoadState::*;

            set(
                "loadState",
                match state {
                    NotStarted => "NOT_STARTED_STATE",
                    InProgress => "IN_PROGRESS_STATE",
                    Complete => "COMPLETE_STATE",
                    Failed => "FAILED_STATE",
                }
                .into(),
            );
        }

        if let Some((load, refresh, limit)) = info.url_fields {
            set("load", load.into());
            set("autoRefresh", refresh.into());
            set("autoRefreshTimeLimit", limit.into());
        }

        if let Some(p) = info.loaded {
            set("loaded", (p as u32).into());
        }

        Ok(o.into())
    }

    /// `createInstance ()` / `newInstance ()` of a declaration: the node id of the instance, in the declaration's context,
    /// not part of the scene.
    #[wasm_bindgen(js_name = declNewInstance)]
    pub fn decl_new_instance(
        &mut self,
        id: u32,
        extern_: bool,
        decl: u32,
    ) -> Result<Option<u32>, JsError> {
        Ok(self
            .session_mut(id)?
            .decl_new_instance(extern_, decl as usize))
    }

    /// `requestUpdateInstances ()` (`now` false) or `updateInstances ()` (`now` true).
    #[wasm_bindgen(js_name = declUpdateInstances)]
    pub fn decl_update_instances(
        &mut self,
        id: u32,
        extern_: bool,
        decl: u32,
        now: bool,
    ) -> Result<(), JsError> {
        let s = self.session_mut(id)?;

        if now {
            s.decl_update_instances(extern_, decl as usize)
        } else {
            s.decl_request_update(extern_, decl as usize)
        }
        .map_err(|e| JsError::new(&e))
    }

    /// `addUserDefinedField (accessType, name, field)` of a PROTO (`type_name` and `data` as the page's field object holds
    /// them) or `removeUserDefinedField (name)` (`type_name` empty).
    #[wasm_bindgen(js_name = declUserField)]
    pub fn decl_user_field(
        &mut self,
        id: u32,
        decl: u32,
        name: &str,
        access: u8,
        type_name: &str,
        data: JsValue,
    ) -> Result<(), JsError> {
        let s = self.session_mut(id)?;

        if type_name.is_empty() {
            return s
                .decl_remove_user_field(decl as usize, name)
                .map_err(|e| JsError::new(&e));
        }

        s.decl_add_user_field(decl as usize, name, access, type_name, &js_data(&data))
            .map_err(|e| JsError::new(&e))
    }

    /// `toVRMLString ()` / `toXMLString ()` / `toJSONString ()` of a declaration.
    #[wasm_bindgen(js_name = declString)]
    pub fn decl_string(
        &self,
        id: u32,
        extern_: bool,
        decl: u32,
        format: &str,
    ) -> Result<String, JsError> {
        self.session(id)?
            .decl_string(extern_, decl as usize, format)
            .map_err(|e| JsError::new(&e))
    }

    /// `body` of a PROTO: its execution context, or undefined.
    #[wasm_bindgen(js_name = declBody)]
    pub fn decl_body(&self, id: u32, decl: u32) -> Result<Option<u32>, JsError> {
        Ok(self.session(id)?.decl_body(decl as usize).map(|c| c as u32))
    }

    /// The root nodes of an execution context of the scene (a PROTO body), `0xffffffff` for a null entry.
    #[wasm_bindgen(js_name = ctxRootNodes)]
    pub fn ctx_root_nodes(&self, id: u32, ctx: u32) -> Result<Vec<u32>, JsError> {
        Ok(self
            .session(id)?
            .ctx_root_nodes(ctx as usize)
            .into_iter()
            .map(|n| n.unwrap_or(u32::MAX))
            .collect())
    }

    /// The named nodes of an execution context of the scene in the order of `getNamedNodes ()`.
    #[wasm_bindgen(js_name = ctxNamedNodes)]
    pub fn ctx_named_nodes(&self, id: u32, ctx: u32) -> Result<Vec<u32>, JsError> {
        Ok(self.session(id)?.ctx_named_nodes(ctx as usize))
    }

    /// `toVRMLString ()` / `toXMLString ()` / `toJSONString ()` of an execution context of the scene (a PROTO body).
    #[wasm_bindgen(js_name = ctxString)]
    pub fn ctx_string(&self, id: u32, ctx: u32, format: &str) -> Result<String, JsError> {
        Ok(self.session(id)?.ctx_string(ctx as usize, format))
    }

    /// The `url`, `load`, `autoRefresh`, `autoRefreshTimeLimit` of an EXTERNPROTO and `loadNow` / `unloadNow` (`what`).
    #[wasm_bindgen(js_name = externUrlOp)]
    pub fn extern_url_op(
        &mut self,
        id: u32,
        decl: u32,
        what: &str,
        urls: Vec<String>,
        number: f64,
    ) -> Result<bool, JsError> {
        Ok(self
            .session_mut(id)?
            .extern_url_op(decl as usize, what, urls, number))
    }

    /// The named nodes of scene `id` in the order of `getNamedNodes ()`.
    #[wasm_bindgen(js_name = namedNodeList)]
    pub fn named_node_list(&self, id: u32) -> Vec<u32> {
        self.worlds
            .get(&id)
            .map_or_else(Vec::new, |w| w.session.named_nodes())
    }

    /// The exported nodes of scene `id` in the order of `getExportedNodes ()`, as [name, local node, description] triples.
    #[wasm_bindgen(js_name = exportedNodeList)]
    pub fn exported_node_list(&self, id: u32) -> js_sys::Array {
        let list = js_sys::Array::new();

        if let Some(w) = self.worlds.get(&id) {
            for (name, local, description) in w.session.exported_nodes() {
                list.push(&js_sys::Array::of3(
                    &name.into(),
                    &local.into(),
                    &description.into(),
                ));
            }
        }

        list
    }

    /// `updateExportedNode (name, node, description)` of scene `id` (also `addExportedNode`, after the page's checks): see
    /// [`Session::set_exported_node`].
    #[wasm_bindgen(js_name = setExportedNode)]
    pub fn set_exported_node(
        &mut self,
        id: u32,
        name: &str,
        node: u32,
        description: &str,
    ) -> Result<(), JsError> {
        self.session_mut(id)?
            .set_exported_node(name, node, description)
            .map_err(|e| JsError::new(&e))
    }

    /// `removeExportedNode (name)` of scene `id`.
    #[wasm_bindgen(js_name = removeExportedNode)]
    pub fn remove_exported_node(&mut self, id: u32, name: &str) -> Result<(), JsError> {
        self.session_mut(id)?.remove_exported_node(name);
        Ok(())
    }

    /// The imported nodes of scene `id` in the order of `getImportedNodes ()`, as [imported name, Inline node, exported name,
    /// description, instance node] arrays.
    #[wasm_bindgen(js_name = importedNodeList)]
    pub fn imported_node_list(&self, id: u32) -> js_sys::Array {
        let list = js_sys::Array::new();

        if let Some(w) = self.worlds.get(&id) {
            for (name, inline, exported, description, instance) in w.session.imported_nodes() {
                let row = js_sys::Array::new();

                row.push(&name.into());
                row.push(&inline.into());
                row.push(&exported.into());
                row.push(&description.into());
                row.push(&instance.into());
                list.push(&row);
            }
        }

        list
    }

    /// The instance node of the imported name in scene `id`, or undefined.
    #[wasm_bindgen(js_name = importedInstance)]
    pub fn imported_instance(&self, id: u32, name: &str) -> Option<u32> {
        self.worlds.get(&id)?.session.imported_instance(name)
    }

    /// The node the Inline of the imported name exports in scene `id`, or undefined while the Inline does not provide it.
    #[wasm_bindgen(js_name = importedExportedNode)]
    pub fn imported_exported_node(&self, id: u32, name: &str) -> Option<u32> {
        self.worlds.get(&id)?.session.imported_exported_node(name)
    }

    /// `getUniqueImportName (name)` of scene `id`.
    #[wasm_bindgen(js_name = uniqueImportName)]
    pub fn unique_import_name(&self, id: u32, name: &str) -> String {
        self.worlds
            .get(&id)
            .map_or_else(|| name.to_string(), |w| w.session.unique_import_name(name))
    }

    /// `updateImportedNode (inline, exportedName, importedName, description)` of scene `id` (also `addImportedNode`, after the
    /// page's checks): see [`Session::set_imported_node`].
    #[wasm_bindgen(js_name = setImportedNode)]
    pub fn set_imported_node(
        &mut self,
        id: u32,
        inline: u32,
        exported: &str,
        imported: &str,
        description: &str,
    ) -> Result<(), JsError> {
        self.session_mut(id)?
            .set_imported_node(inline, exported, imported, description)
            .map_err(|e| JsError::new(&e))
    }

    /// `removeImportedNode (name)` of scene `id`.
    #[wasm_bindgen(js_name = removeImportedNode)]
    pub fn remove_imported_node(&mut self, id: u32, name: &str) -> Result<(), JsError> {
        self.session_mut(id)?.remove_imported_node(name);
        Ok(())
    }

    /// `renameImportedNode (old, new)` of scene `id`.
    #[wasm_bindgen(js_name = renameImportedNode)]
    pub fn rename_imported_node(&mut self, id: u32, old: &str, new: &str) -> Result<(), JsError> {
        self.session_mut(id)?
            .rename_imported_node(old, new)
            .map_err(|e| JsError::new(&e))
    }

    /// `X3DImportedNode.setDescription` of the record `name` of scene `id`.
    #[wasm_bindgen(js_name = setImportedDescription)]
    pub fn set_imported_description(
        &mut self,
        id: u32,
        name: &str,
        description: &str,
    ) -> Result<(), JsError> {
        self.session_mut(id)?
            .set_imported_description(name, description);
        Ok(())
    }

    /// The local node of the exported name in scene `id`, or undefined.
    #[wasm_bindgen(js_name = exportedNode)]
    pub fn exported_node(&self, id: u32, name: &str) -> Option<u32> {
        self.worlds.get(&id)?.session.exported_node(name)
    }

    /// The root nodes of scene `id` as node ids; -1 is NULL.
    #[wasm_bindgen(js_name = rootNodes)]
    pub fn root_nodes(&self, id: u32) -> Vec<i64> {
        self.worlds.get(&id).map_or(vec![], |w| {
            w.session
                .root_nodes()
                .into_iter()
                .map(|n| n.map_or(-1, i64::from))
                .collect()
        })
    }

    /// `rootNodes .assign (list)` of scene `id` (see [`Session::set_root_nodes`]): `list` holds node ids and nulls.
    #[wasm_bindgen(js_name = setRootNodes)]
    pub fn set_root_nodes(&mut self, id: u32, list: js_sys::Array) -> Result<(), JsError> {
        let list = root_node_list(&list)?;

        self.session_mut(id)?
            .set_root_nodes(list)
            .map_err(|e| JsError::new(&e))
    }

    /// X3DField.addFieldCallback / removeFieldCallback of the `rootNodes` field of scene `id`: its events come with
    /// [`WebBrowser::take_field_events`] as node `ROOT_NODES_EVENT` (4294967295).
    #[wasm_bindgen(js_name = observeRootNodes)]
    pub fn observe_root_nodes(&mut self, id: u32, on: bool) -> Result<(), JsError> {
        self.session_mut(id)?.observe_root_nodes(on);
        Ok(())
    }

    /// The routes of scene `id` in the order they were added, each as `[id, source, sourceField, destination,
    /// destinationField, givenSourceField, givenDestinationField]` (see [`Session::routes`]).
    #[wasm_bindgen(js_name = routes)]
    pub fn routes(&self, id: u32) -> js_sys::Array {
        self.worlds.get(&id).map_or_else(js_sys::Array::new, |w| {
            w.session.routes().iter().map(route_js).collect()
        })
    }

    /// X3DExecutionContext.getRoute: the route (see [`WebBrowser::routes`]) or `undefined`; an error where X_ITE throws.
    #[wasm_bindgen(js_name = getRoute)]
    pub fn get_route(
        &self,
        id: u32,
        source: u32,
        source_field: &str,
        destination: u32,
        destination_field: &str,
    ) -> Result<JsValue, JsError> {
        let session = &self.worlds.get(&id).ok_or_else(no_world)?.session;

        Ok(session
            .get_route(source, source_field, destination, destination_field)
            .map_err(|e| JsError::new(&e))?
            .map_or(JsValue::UNDEFINED, |r| route_js(&r).into()))
    }

    /// X3DExecutionContext.addRoute: the route that exists or the new one (see [`Session::add_route`]).
    #[wasm_bindgen(js_name = addRoute)]
    pub fn add_route(
        &mut self,
        id: u32,
        source: u32,
        source_field: &str,
        destination: u32,
        destination_field: &str,
    ) -> Result<js_sys::Array, JsError> {
        self.session_mut(id)?
            .add_route(source, source_field, destination, destination_field)
            .map(|r| route_js(&r))
            .map_err(|e| JsError::new(&e))
    }

    /// X3DExecutionContext.deleteRoute (route) by the id of the route; whether the world had it.
    #[wasm_bindgen(js_name = deleteRoute)]
    pub fn delete_route(&mut self, id: u32, route: u32) -> Result<bool, JsError> {
        Ok(self.session_mut(id)?.delete_route(route as usize))
    }

    #[wasm_bindgen(js_name = typeName)]
    pub fn type_name(&self, id: u32, node: u32) -> Option<String> {
        self.worlds.get(&id)?.session.type_name(node)
    }

    #[wasm_bindgen(js_name = nodeName)]
    pub fn node_name(&self, id: u32, node: u32) -> Option<String> {
        self.worlds.get(&id)?.session.node_name(node)
    }

    #[wasm_bindgen(js_name = fieldNames)]
    pub fn field_names(&self, id: u32, node: u32) -> Vec<String> {
        self.worlds
            .get(&id)
            .map_or(vec![], |w| w.session.field_names(node))
    }

    /// The node ids of an SFNode or MFNode field; -1 is NULL.
    #[wasm_bindgen(js_name = nodeValue)]
    pub fn node_value(&self, id: u32, node: u32, name: &str) -> Result<Vec<i64>, JsError> {
        let nodes = self
            .session(id)?
            .node_value(node, name)
            .map_err(|e| JsError::new(&e))?;

        Ok(nodes.into_iter().map(|n| n.map_or(-1, i64::from)).collect())
    }

    /// `[typeName, accessType, toString ()]` of the field.
    pub fn field(&self, id: u32, node: u32, name: &str) -> Result<Vec<String>, JsError> {
        let f = self
            .session(id)?
            .field(node, name)
            .map_err(|e| JsError::new(&e))?;

        Ok(vec![f.type_name.to_string(), f.access.to_string(), f.value])
    }

    // --- field objects of the page (web/x_ite_r.mjs: X_ITE's own field classes) ----------------------------------

    /// `[typeName, accessType, data]` of a node field (see [`Session::field_data`]); the page holds an object of it.
    #[wasm_bindgen(js_name = fieldData)]
    pub fn field_data(&mut self, id: u32, node: u32, name: &str) -> Result<js_sys::Array, JsError> {
        let (type_name, access, data) = self
            .session_mut(id)?
            .field_data(node, name)
            .map_err(|e| JsError::new(&e))?;

        Ok(js_sys::Array::of3(
            &type_name.into(),
            &f64::from(access).into(),
            &data_js(&data),
        ))
    }

    /// X3DField.addEvent of the page's object of a node field (see [`Session::set_field_data`]).
    #[wasm_bindgen(js_name = setFieldData)]
    pub fn set_field_data(
        &mut self,
        id: u32,
        node: u32,
        name: &str,
        data: JsValue,
    ) -> Result<(), JsError> {
        self.session_mut(id)?
            .set_field_data(node, name, &js_data(&data))
            .map_err(|e| JsError::new(&e))
    }

    /// The changed fields the page holds objects of, as node, name, data, ... (see [`Session::take_field_changes`]).
    #[wasm_bindgen(js_name = takeFieldChanges)]
    pub fn take_field_changes(&mut self, id: u32) -> js_sys::Array {
        match self.worlds.get_mut(&id) {
            Some(w) => field_list(w.session.take_field_changes()),
            None => js_sys::Array::new(),
        }
    }

    /// X3DField.addFieldCallback / removeFieldCallback: whether the page gets the events of the field.
    #[wasm_bindgen(js_name = observeField)]
    pub fn observe_field(
        &mut self,
        id: u32,
        node: u32,
        name: &str,
        on: bool,
    ) -> Result<(), JsError> {
        self.session_mut(id)?
            .observe_field(node, name, on)
            .map_err(|e| JsError::new(&e))
    }

    /// A page field callback threw: X_ITE processes no event again (see `Runtime::halt_events`).
    #[wasm_bindgen(js_name = haltEvents)]
    pub fn halt_events(&mut self, id: u32) -> Result<(), JsError> {
        self.session_mut(id)?.halt_events();
        Ok(())
    }

    /// The events of the observed fields, as node, name, data, ... in event order (see
    /// [`Session::take_field_events`]).
    #[wasm_bindgen(js_name = takeFieldEvents)]
    pub fn take_field_events(&mut self, id: u32) -> js_sys::Array {
        match self.worlds.get_mut(&id) {
            Some(w) => {
                // The lines of a replaced world come first in `takeLog` of the current world.
                let retired = if self.current == Some(id) {
                    self.retired_log.len()
                } else {
                    0
                };
                let list = js_sys::Array::new();

                for (node, name, data, before) in w.session.take_field_events_logged() {
                    list.push(&f64::from(node).into());
                    list.push(&name.into());
                    list.push(&data_js(&data));
                    list.push(&(before.saturating_add(retired) as f64).into());
                }

                list
            }
            None => js_sys::Array::new(),
        }
    }

    /// The nodes the runtime released since the last call (see [`Session::take_released_nodes`]).
    #[wasm_bindgen(js_name = takeReleasedNodes)]
    pub fn take_released_nodes(&mut self, id: u32) -> Vec<u32> {
        match self.worlds.get_mut(&id) {
            Some(w) => w.session.take_released_nodes(),
            None => vec![],
        }
    }

    /// X3DNode.getFieldDefinitions: `[name, accessType, typeName, default]` each; `default` is undefined for a
    /// user-defined field.
    #[wasm_bindgen(js_name = fieldDefinitions)]
    pub fn field_definitions(&self, id: u32, node: u32) -> js_sys::Array {
        let Some(w) = self.worlds.get(&id) else {
            return js_sys::Array::new();
        };

        w.session
            .field_definitions(node)
            .into_iter()
            .map(field_definition_js)
            .collect()
    }

    /// Set the field from its VRML encoding (see [`Session::set_field`]).
    #[wasm_bindgen(js_name = setField)]
    pub fn set_field(&mut self, id: u32, node: u32, name: &str, vrml: &str) -> Result<(), JsError> {
        self.session_mut(id)?
            .set_field(node, name, vrml)
            .map_err(|e| JsError::new(&e))
    }

    /// X3DObject.toVRMLString / toXMLString / toJSONString of the nodes of an SFNode or MFNode value (see
    /// [`Session::print_nodes`]); `kind` is `vrml`, `xml` or `json`.
    #[wasm_bindgen(js_name = printNodes)]
    pub fn print_nodes(
        &mut self,
        id: u32,
        kind: &str,
        type_name: &str,
        data: JsValue,
    ) -> Result<String, JsError> {
        let ty = x3d_fields::FieldType::from_name(type_name)
            .ok_or_else(|| JsError::new(&format!("unknown field type '{type_name}'")))?;

        self.session_mut(id)?
            .print_nodes(kind, ty, &js_data(&data))
            .map_err(|e| JsError::new(&e))
    }

    /// X3DScene.toVRMLString / toXMLString / toJSONString of the world `id` (see [`Session::print_scene`]).
    #[wasm_bindgen(js_name = printScene)]
    pub fn print_scene(&mut self, id: u32, kind: &str) -> Result<String, JsError> {
        self.session_mut(id)?
            .print_scene(kind)
            .map_err(|e| JsError::new(&e))
    }

    /// X3DBrowser.replaceWorld (null): the empty world, a scene no source set up (see [`Session::empty_world`]).
    #[wasm_bindgen(js_name = prepareEmpty)]
    pub fn prepare_empty(&mut self, url: &str, now: f64) -> u32 {
        let cache = FetchCache::new();
        let fetcher = Box::new(WebFetcher {
            cache: cache.clone(),
        });
        let world = Session::empty_world(url, fetcher);

        self.add(world, cache, now)
    }

    /// X3DField.fromVRMLString / fromXMLString (value, scene) (see [`Session::parse_field`]): the data of the value
    /// `text` reads as for a field of type `type_name` of the world `id`.
    #[wasm_bindgen(js_name = parseField)]
    pub fn parse_field(
        &mut self,
        id: u32,
        type_name: &str,
        text: &str,
        xml: bool,
        name: &str,
        unit: Option<String>,
    ) -> Result<JsValue, JsError> {
        let ty = x3d_fields::FieldType::from_name(type_name)
            .ok_or_else(|| JsError::new(&format!("unknown field type '{type_name}'")))?;

        self.session_mut(id)?
            .parse_field(ty, text, xml, name, unit)
            .map(|data| data_js(&data))
            .map_err(|e| JsError::new(&e))
    }

    // --- input of the current world ------------------------------------------------------------------------------

    /// A pointer event at pointer coordinates: `kind` 0 down, 1 up, 2 move, 3 dblclick; `button` is
    /// MouseEvent.button.
    pub fn pointer(&mut self, kind: u8, button: u8, x: f64, y: f64) {
        let event = match kind {
            0 => Pointer::Down { button, x, y },
            1 => Pointer::Up { button, x, y },
            3 => Pointer::DblClick { x, y },
            _ => Pointer::Move { x, y },
        };

        if let Some(s) = self.current_mut() {
            s.pointer(event);
        }
    }

    /// A wheel event: whether the viewer takes it (the page then calls `preventDefault ()`).
    pub fn wheel(&mut self, delta_y: f64, x: f64, y: f64) -> bool {
        self.current_mut().is_some_and(|s| s.wheel(delta_y, x, y))
    }

    /// A touch event: `kind` 0 start, 1 move, 2 end; `points` the touches still down as x, y pairs at pointer
    /// coordinates; `zoom_width` is `window .innerWidth` at pointer scale.
    pub fn touch(&mut self, kind: u8, points: &[f64], zoom_width: f64) {
        let event = match kind {
            0 => Touch::Start,
            1 => Touch::Move,
            _ => Touch::End,
        };
        let touches: Vec<(f64, f64)> = points.chunks_exact(2).map(|p| (p[0], p[1])).collect();

        if let Some(s) = self.current_mut() {
            s.touch(event, &touches, zoom_width);
        }
    }

    /// A keydown (`down`) or keyup of the element: `event .key` and `event .keyCode`.
    pub fn key(&mut self, down: bool, key: &str, code: u32) {
        if let Some(s) = self.current_mut() {
            s.key(down, key, code);
        }
    }

    /// The CSS cursor of the canvas.
    #[wasm_bindgen(getter)]
    pub fn cursor(&self) -> String {
        self.current
            .and_then(|id| self.worlds.get(&id))
            .map_or("default", |w| w.session.cursor())
            .to_string()
    }

    /// The viewer of the current world: "EXAMINE", "WALK", "FLY", "PLANE", "LOOKAT" or "NONE".
    #[wasm_bindgen(getter, js_name = navigationType)]
    pub fn navigation_type(&self) -> String {
        self.current
            .and_then(|id| self.worlds.get(&id))
            .map_or("NONE", |w| w.session.navigation_type())
            .to_string()
    }

    /// The browser events of the current world since the last call (an Anchor replaced the world).
    #[wasm_bindgen(js_name = takeEvents)]
    pub fn take_events(&mut self) -> Vec<u32> {
        self.current_mut()
            .map_or(vec![], |s| s.take_browser_events())
    }

    /// The requests of another window or page since the last call: target, url, target, url, ...
    #[wasm_bindgen(js_name = takeForeign)]
    pub fn take_foreign(&mut self) -> Vec<String> {
        self.current_mut().map_or(vec![], |s| {
            s.take_foreign()
                .into_iter()
                .flat_map(|(t, u)| [t, u])
                .collect()
        })
    }

    // --- viewpoints of the current world -------------------------------------------------------------------------

    /// X3DBrowser.firstViewpoint, previousViewpoint, nextViewpoint, lastViewpoint, or `changeViewpoint:<name>`.
    #[wasm_bindgen(js_name = viewpointRequest)]
    pub fn viewpoint_request(&mut self, request: &str) {
        if let Some(s) = self.current_mut() {
            s.viewpoint_request(request);
        }
    }

    /// The descriptions that the engine set itself since the last call, in order: a viewpoint that was bound
    /// (X3DViewpointNode.set_bound__) and a pointing device sensor that is over (set_over__), each `browser .setDescription`.
    #[wasm_bindgen(js_name = takeDescriptions)]
    pub fn take_descriptions(&mut self) -> Vec<String> {
        self.current_mut().map_or(vec![], |s| s.take_descriptions())
    }

    /// X3DLayerNode.getUserViewpoints of the active layer: the viewpoints with a description, as node ids.
    #[wasm_bindgen(js_name = userViewpoints)]
    pub fn user_viewpoints(&self) -> Vec<u32> {
        self.current
            .and_then(|id| self.worlds.get(&id))
            .map_or(vec![], |w| w.session.user_viewpoints())
    }

    /// `viewpoint .getDescriptions () .join (" » ")` of a viewpoint of the current world.
    #[wasm_bindgen(js_name = viewpointDescription)]
    pub fn viewpoint_description(&self, node: u32) -> String {
        self.current
            .and_then(|id| self.worlds.get(&id))
            .map_or(String::new(), |w| w.session.viewpoint_description(node))
    }

    /// `browser ._availableViewers` of the current world.
    #[wasm_bindgen(js_name = availableViewers)]
    pub fn available_viewers(&self) -> Vec<String> {
        self.current
            .and_then(|id| self.worlds.get(&id))
            .map_or(vec![], |w| w.session.available_viewers())
    }

    /// `browser .getCurrentViewer ()` of the current world.
    #[wasm_bindgen(js_name = currentViewer)]
    pub fn current_viewer(&self) -> String {
        self.current
            .and_then(|id| self.worlds.get(&id))
            .map_or("NONE".to_string(), |w| w.session.viewer_field())
    }

    /// `browser ._viewer = kind` of the current world.
    #[wasm_bindgen(js_name = setViewer)]
    pub fn set_viewer(&mut self, kind: &str) {
        if let Some(s) = self.current_mut() {
            s.set_viewer(kind);
        }
    }

    /// The world's first WorldInfo and META statements, for the "Show World Info" menu entry: `[hasWorldInfo, title,
    /// info (an array), meta (an array of [key, value] arrays)]`.
    #[wasm_bindgen(js_name = worldInfo)]
    pub fn world_info(&self) -> js_sys::Array {
        let out = js_sys::Array::new();

        let Some(w) = self.current.and_then(|id| self.worlds.get(&id)) else {
            return out;
        };

        let (info, meta) = w.session.world_info();
        let lines = js_sys::Array::new();
        let pairs = js_sys::Array::new();

        for (k, v) in meta {
            pairs.push(&js_sys::Array::of2(
                &JsValue::from_str(&k),
                &JsValue::from_str(&v),
            ));
        }

        match info {
            Some((title, list)) => {
                for l in list {
                    lines.push(&JsValue::from_str(&l));
                }

                out.push(&JsValue::TRUE);
                out.push(&JsValue::from_str(&title));
            }
            None => {
                out.push(&JsValue::FALSE);
                out.push(&JsValue::from_str(""));
            }
        }

        out.push(&lines);
        out.push(&pairs);
        out
    }

    /// The bound viewpoint's user position, orientation (axis, angle) and center of rotation, ten numbers (Shift+F8).
    #[wasm_bindgen(js_name = userPose)]
    pub fn user_pose(&mut self) -> Option<Vec<f64>> {
        self.current_mut()?.user_pose().map(|p| p.to_vec())
    }

    /// X3DBrowser.viewAll (transitionTime) of the active layer, or of the layer node `layer`.
    #[wasm_bindgen(js_name = viewAll)]
    pub fn view_all(&mut self, transition_time: f64, layer: Option<u32>) {
        if let Some(s) = self.current_mut() {
            s.view_all(layer, transition_time);
        }
    }

    /// X3DBrowserContext.setLive (beginUpdate, endUpdate): the live state of the browser, for the world and the
    /// worlds that follow.
    #[wasm_bindgen(js_name = setLive)]
    pub fn set_live(&mut self, live: bool) {
        self.live = live;

        if let Some(s) = self.current_mut() {
            s.set_live(live);
        }
    }

    /// X3DBrowserContext.getActiveViewpoint of the current world: the node id, or `undefined` for the default one.
    #[wasm_bindgen(js_name = activeViewpoint)]
    pub fn active_viewpoint(&self) -> Option<u32> {
        self.worlds.get(&self.current?)?.session.active_viewpoint()
    }

    /// Has a Sound node of any world or load of this browser made its audio context? The page then makes its
    /// AudioContext (X3DSoundContext.getAudioContext) and gives its clock to the engine ([`WebBrowser::set_audio_clock`]).
    #[wasm_bindgen(js_name = hasAudio)]
    pub fn has_audio(&self) -> bool {
        self.worlds.values().any(|w| w.session.has_audio())
            || self.loads.values().any(|l| l.rt.audio.is_some())
    }

    /// X3DSoundContext.startAudioElement: did a world or load start a media element since the last call? The page
    /// then calls the waiting AudioContext's resume () again (startAudioElements).
    #[wasm_bindgen(js_name = takeAudioStarts)]
    pub fn take_audio_starts(&mut self) -> bool {
        let mut started = false;

        for w in self.worlds.values_mut() {
            started |= w.session.take_audio_element_started();
        }

        for l in self.loads.values_mut() {
            started |= std::mem::take(&mut l.rt.audio_element_started);
        }

        started
    }

    /// The calls that the MovieTextures of the worlds and loads made on the page's `<video>` elements since the last call,
    /// and the tokens of the elements they want: `{ live: [token, ...], ops: [["load", token, url], ["play", token], ...] }`,
    /// or `undefined` when there is nothing and there was nothing last time. The page makes an element for each live token
    /// when its `load` call comes, makes the other calls on it, and removes the elements of the tokens that are not live
    /// (web/x_ite_r.mjs, #serviceVideos). The calls are `load` (token, url), `play`, `pause`, `rewind` (`currentTime = 0`),
    /// `loop` (token, bool) and `rate` (token, playbackRate, preservesPitch).
    #[wasm_bindgen(js_name = takeVideoOps)]
    pub fn take_video_ops(&mut self) -> JsValue {
        use x3d_runtime::nodes::movie_texture::VideoOp;

        let mut live: Vec<u64> = Vec::new();
        let mut ops: Vec<VideoOp> = Vec::new();

        for w in self.worlds.values_mut() {
            live.extend(w.session.video_tokens());
            ops.extend(w.session.take_video_ops());
        }

        for l in self.loads.values_mut() {
            live.extend(l.rt.video_tokens());
            ops.extend(l.rt.take_video_ops());
        }

        if live.is_empty() && ops.is_empty() && !self.video_known {
            return JsValue::UNDEFINED;
        }

        self.video_known = !live.is_empty();

        let call = |items: &[JsValue]| -> JsValue {
            let a = js_sys::Array::new();

            for i in items {
                a.push(i);
            }

            a.into()
        };
        let num = |t: u64| JsValue::from_f64(t as f64);
        let js_ops = js_sys::Array::new();

        for op in ops {
            js_ops.push(&match op {
                VideoOp::Load { token, url } => {
                    call(&["load".into(), num(token), url.as_str().into()])
                }
                VideoOp::Play(t) => call(&["play".into(), num(t)]),
                VideoOp::Pause(t) => call(&["pause".into(), num(t)]),
                VideoOp::Rewind(t) => call(&["rewind".into(), num(t)]),
                VideoOp::Loop(t, v) => call(&["loop".into(), num(t), v.into()]),
                VideoOp::Rate(t, r, p) => call(&["rate".into(), num(t), r.into(), p.into()]),
            });
        }

        let out = js_sys::Object::new();

        let _ = js_sys::Reflect::set(
            &out,
            &"live".into(),
            &live.iter().map(|t| num(*t)).collect::<js_sys::Array>(),
        );
        let _ = js_sys::Reflect::set(&out, &"ops".into(), &js_ops);
        out.into()
    }

    /// The page's video element `token` has its first frame: its size, `duration`, whether it has audio, and the pixels of
    /// that frame (RGBA, rows from the top).
    #[wasm_bindgen(js_name = videoLoaded)]
    pub fn video_loaded(
        &mut self,
        token: f64,
        width: u32,
        height: u32,
        duration: f64,
        has_sound: bool,
        rgba: &[u8],
    ) {
        let token = token as u64;

        for w in self.worlds.values_mut() {
            if w.session
                .video_loaded(token, (width, height), duration, has_sound, rgba.to_vec())
            {
                return;
            }
        }

        for l in self.loads.values_mut() {
            if l.rt
                .video_loaded(token, width, height, duration, has_sound, rgba.to_vec())
            {
                return;
            }
        }
    }

    /// The page's video element `token` failed: `event` is the type of its `error` or `abort` event.
    #[wasm_bindgen(js_name = videoFailed)]
    pub fn video_failed(&mut self, token: f64, event: &str) {
        let token = token as u64;

        for w in self.worlds.values_mut() {
            if w.session.video_failed(token, event) {
                return;
            }
        }

        for l in self.loads.values_mut() {
            if l.rt.video_failed(token, event) {
                return;
            }
        }
    }

    /// A frame of the page's video element `token`: its `currentTime`, and the pixels (RGBA, rows from the top) when the
    /// picture changed; `rgba` is empty when it did not.
    #[wasm_bindgen(js_name = videoFrame)]
    pub fn video_frame(&mut self, token: f64, current_time: f64, rgba: &[u8]) {
        let token = token as u64;

        for w in self.worlds.values_mut() {
            if w.session.video_frame(token, current_time, rgba.to_vec()) {
                return;
            }
        }

        for l in self.loads.values_mut() {
            if l.rt.video_frame(token, current_time, rgba.to_vec()) {
                return;
            }
        }
    }

    /// The `Shading` browser option as the renderer's mode (BrowserOptions.set_Shading__ with Shading.js): `POINT`,
    /// `WIREFRAME`, `FLAT`, `GOURAUD` or `PHONG`; any other text gives `GOURAUD`, like X_ITE.
    #[wasm_bindgen(js_name = setShading)]
    pub fn set_shading(&mut self, name: &str) {
        use x3d_render::gpu::ShadingMode;

        self.renderer.set_shading(match name {
            "POINT" => ShadingMode::Point,
            "WIREFRAME" => ShadingMode::Wireframe,
            "FLAT" => ShadingMode::Flat,
            "PHONG" => ShadingMode::Phong,
            _ => ShadingMode::Gouraud,
        });
    }

    /// The `Multisampling` rendering property: the sample count to draw with (0 or 1 draws without multisampling).
    /// The renderer uses the highest count the device supports that is not above the request.
    #[wasm_bindgen(js_name = setSampleCount)]
    pub fn set_sample_count(&mut self, samples: u32) {
        self.renderer.set_sample_count(samples);
    }

    /// The highest sample count the device supports: X_ITE's `MaxSamples`, the limit of `Multisampling`.
    #[wasm_bindgen(getter, js_name = maxSamples)]
    pub fn max_samples(&self) -> u32 {
        self.renderer
            .supported_samples()
            .iter()
            .copied()
            .max()
            .unwrap_or(1)
    }

    /// A browser option the shared engine reads itself, for the worlds and loads now and for those to come: the booleans
    /// `EnableInlineViewpoints` (X3DLayerNode.getUserViewpoints), `LoadUrlObjects` (X3DUrlObject.requestImmediateLoad) and
    /// `StraightenHorizon` (BrowserOptions.set_StraightenHorizon__, X3DBrowser.viewAll), and the number `Gravity`
    /// (X3DRenderObject.gravitate). The page gives the option's value already coerced to the field type. Returns whether
    /// the engine has the option.
    #[wasm_bindgen(js_name = setEngineOption)]
    pub fn set_engine_option(&mut self, name: &str, flag: bool, number: f64) -> bool {
        use x3d_runtime::browser_options::OptionValue;

        let value = match name {
            "EnableInlineViewpoints" | "LoadUrlObjects" | "StraightenHorizon" => {
                OptionValue::Bool(flag)
            }
            "Gravity" => OptionValue::Double(number),
            _ => return false,
        };

        self.engine_options.retain(|(n, _)| n != name);
        self.engine_options.push((name.to_string(), value.clone()));

        for w in self.worlds.values_mut() {
            w.session.set_engine_option(name, value.clone());
        }

        for l in self.loads.values_mut() {
            l.rt.browser_options.set_option(name, value.clone());
        }

        true
    }

    /// The page's `Mute` and `SoundIntensity` browser options, for the worlds and loads now and for those to come
    /// (BrowserOptions.set_SoundIntensity__: the gain of every sound destination is `!Mute * clamp (SoundIntensity, 0, 1)`).
    #[wasm_bindgen(js_name = setSoundOptions)]
    pub fn set_sound_options(&mut self, mute: bool, intensity: f64) {
        self.sound_options = Some((mute, intensity));

        for w in self.worlds.values_mut() {
            w.session.set_sound_options(mute, intensity);
        }

        for l in self.loads.values_mut() {
            crate::session::set_sound_options(&mut l.rt, mute, intensity);
        }
    }

    /// The calls that the audio engines of the worlds and loads made on their Web Audio nodes since the last call, as
    /// JSON: `{"engines": [{"uid": n, "ops": [["connect", 1, 0, 0, 0], ...]}], "gone": [uid, ...]}`, or `undefined`
    /// when there is nothing. The page makes each call on a real node of its AudioContext (web/audio_mirror.mjs).
    /// `gone` lists the engines that no longer exist (a replaced world, a discarded load): the page stops and
    /// disconnects their nodes. A buffer argument is `{"buffer": n}`; read it with [`WebBrowser::audio_buffer`].
    #[wasm_bindgen(js_name = takeAudioOps)]
    pub fn take_audio_ops(&mut self) -> Option<String> {
        let mut live = std::collections::HashSet::new();
        let mut engines = Vec::new();
        let mut taken = Vec::new();

        for w in self.worlds.values() {
            taken.extend(w.session.take_audio_ops());
        }

        for l in self.loads.values() {
            taken.extend(l.rt.audio.as_ref().map(|a| (a.uid(), a.take_ops())));
        }

        self.audio_payloads = Default::default();

        for (uid, ops) in taken {
            live.insert(uid);

            if self.audio_known.insert(uid) || !ops.is_empty() {
                let json = x3d_runtime::sound::ops::to_json(&ops, &mut self.audio_payloads);

                engines.push(format!("{{\"uid\":{uid},\"ops\":{json}}}"));
            }
        }

        let gone: Vec<String> = self
            .audio_known
            .iter()
            .filter(|uid| !live.contains(uid))
            .map(|uid| uid.to_string())
            .collect();

        self.audio_known.retain(|uid| live.contains(uid));

        if engines.is_empty() && gone.is_empty() {
            return None;
        }

        Some(format!(
            "{{\"engines\":[{}],\"gone\":[{}]}}",
            engines.join(","),
            gone.join(",")
        ))
    }

    /// Buffer `index` of the last [`WebBrowser::take_audio_ops`]: `[sampleRate, channel 0, channel 1, ...]`, each channel
    /// a Float32Array.
    #[wasm_bindgen(js_name = audioBuffer)]
    pub fn audio_buffer(&self, index: u32) -> Option<js_sys::Array> {
        let (data, rate) = self.audio_payloads.buffers.get(index as usize)?;
        let out = js_sys::Array::new();

        out.push(&JsValue::from_f64(f64::from(*rate)));

        for channel in data.iter() {
            out.push(&js_sys::Float32Array::from(channel.as_slice()));
        }

        Some(out)
    }

    /// Encoded file `index` of the last [`WebBrowser::take_audio_ops`] (`{"encoded": n}`): the bytes of the file an
    /// AudioBuffer was decoded from. The page decodes it itself (`decodeAudioData`), as X_ITE does.
    #[wasm_bindgen(js_name = audioBytes)]
    pub fn audio_bytes(&self, index: u32) -> Option<js_sys::Uint8Array> {
        let bytes = self.audio_payloads.encoded.get(index as usize)?;

        Some(js_sys::Uint8Array::from(bytes.as_slice()))
    }

    /// `audioContext .currentTime` of the page's AudioContext, for the audio contexts of every world and load.
    #[wasm_bindgen(js_name = setAudioClock)]
    pub fn set_audio_clock(&mut self, time: f64) {
        self.audio_clock = Some(time);

        for w in self.worlds.values_mut() {
            w.session.set_audio_clock(time);
        }

        for l in self.loads.values_mut() {
            if let Some(audio) = l.rt.audio.as_mut() {
                audio.set_clock(time);
            }
        }
    }

    /// The audio graph of the current world as JSON ([`Session::audio_graph_json`]), or `undefined` while no Sound
    /// node has made the audio context. For tests and diagnostics.
    #[wasm_bindgen(js_name = audioGraph)]
    pub fn audio_graph(&self) -> Option<String> {
        self.worlds.get(&self.current?)?.session.audio_graph_json()
    }

    // --- frames --------------------------------------------------------------------------------------------------

    /// Step the current world to `now` and draw it into the canvas, sized `width` x `height` device pixels. A lost
    /// device draws nothing.
    pub fn frame(&mut self, now: f64, width: u32, height: u32) -> Result<(), JsError> {
        if self.lost() {
            return Err(JsError::new("the WebGPU device is lost"));
        }

        let (width, height) = (width.max(1), height.max(1));

        if (width, height) != (self.config.width, self.config.height) {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);

            for w in self.worlds.values_mut() {
                w.session.resize(width, height);
            }
        }

        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            other => return Err(JsError::new(&format!("canvas texture: {other:?}"))),
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.view_format),
            ..Default::default()
        });
        let current = self.current.and_then(|id| self.worlds.get_mut(&id));
        let frame = match current.map(|w| &mut w.session) {
            Some(s) => {
                s.step(now);

                let changes = s.viewer().map(|v| v.take_rendering_changes());
                let frame = s.build_frame();

                if let Some(c) = changes {
                    c.apply(&mut self.renderer);
                }

                frame
            }
            None => crate::empty_frame(),
        };

        self.renderer
            .render(&frame, &view, self.view_format, width, height)
            .map_err(|e| JsError::new(&e))?;
        self.renderer.queue().present(texture);
        Ok(())
    }

    /// Draw the current state (no stepping) once more into an RGBA8 texture of the canvas size and read it back: a
    /// Promise of the pixels as the canvas shows them, top row first. For tests and diagnostics: a page that cannot
    /// read the WebGPU canvas (headless Chromium without a display compositor) still gets the picture the renderer
    /// made.
    #[wasm_bindgen(js_name = readPixels)]
    pub fn read_pixels(&mut self) -> Result<js_sys::Promise, JsError> {
        let (width, height) = (self.config.width, self.config.height);
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let current = self.current.and_then(|id| self.worlds.get_mut(&id));
        let frame = match current.map(|w| &mut w.session) {
            Some(s) => s.build_frame(),
            None => crate::empty_frame(),
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("x3d-web readPixels"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        self.renderer
            .render(
                &frame,
                &texture.create_view(&Default::default()),
                format,
                width,
                height,
            )
            .map_err(|e| JsError::new(&e))?;

        let row = width * 4;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("x3d-web readPixels"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());

        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            texture.size(),
        );
        self.renderer.queue().submit([encoder.finish()]);

        // An opaque canvas shows every pixel with alpha 1, whatever the framebuffer alpha is (the render oracle reads
        // X_ITE's opaque WebGL canvas the same way).
        let opaque = self.config.alpha_mode == wgpu::CompositeAlphaMode::Opaque;

        Ok(js_sys::Promise::new(&mut |resolve, reject| {
            let mapped = buffer.clone();

            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    if let Err(e) = result {
                        let _ = reject.call1(&JsValue::NULL, &JsError::new(&e.to_string()).into());
                        return;
                    }

                    let data = match mapped.slice(..).get_mapped_range() {
                        Ok(data) => data,
                        Err(e) => {
                            let _ =
                                reject.call1(&JsValue::NULL, &JsError::new(&e.to_string()).into());
                            return;
                        }
                    };
                    let mut rgba = Vec::with_capacity((row * height) as usize);

                    for y in 0..height as usize {
                        let start = y * padded as usize;

                        rgba.extend_from_slice(&data[start..start + row as usize]);
                    }

                    drop(data);
                    mapped.unmap();

                    if opaque {
                        rgba.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
                    }

                    let _ =
                        resolve.call1(&JsValue::NULL, &js_sys::Uint8Array::from(&rgba[..]).into());
                });
        }))
    }

    /// The runtime's log and Browser.print output of scene or load `id` since the last call.
    #[wasm_bindgen(js_name = takeLog)]
    pub fn take_log(&mut self, id: u32) -> Vec<String> {
        if let Some(load) = self.loads.get_mut(&id) {
            let new = load.rt.log[load.log_seen.min(load.rt.log.len())..].to_vec();

            load.log_seen = load.rt.log.len();
            load.rt.log_taken = load.log_seen;
            return new;
        }

        let mut log = if self.current == Some(id) {
            std::mem::take(&mut self.retired_log)
        } else {
            vec![]
        };

        if let Some(w) = self.worlds.get_mut(&id) {
            log.extend(w.session.take_log());
        }

        log
    }
}

// --- field data (x3d_runtime::data) ---------------------------------------------------------------------------------

fn data_js(d: &Data) -> JsValue {
    match d {
        Data::Null => JsValue::NULL,
        Data::Bool(b) => JsValue::from_bool(*b),
        Data::Number(x) => JsValue::from_f64(*x),
        Data::String(s) => JsValue::from_str(s),
        Data::List(items) => items.iter().map(data_js).collect::<js_sys::Array>().into(),
    }
}

fn js_data(v: &JsValue) -> Data {
    if let Some(b) = v.as_bool() {
        Data::Bool(b)
    } else if let Some(x) = v.as_f64() {
        Data::Number(x)
    } else if let Some(s) = v.as_string() {
        Data::String(s)
    } else if js_sys::Array::is_array(v) {
        Data::List(js_sys::Array::from(v).iter().map(|x| js_data(&x)).collect())
    } else {
        Data::Null
    }
}

fn field_list(fields: Vec<(u32, String, Data)>) -> js_sys::Array {
    let list = js_sys::Array::new();

    for (node, name, data) in fields {
        list.push(&f64::from(node).into());
        list.push(&name.into());
        list.push(&data_js(&data));
    }

    list
}

/// X_ITE prints a parser warning when it is made, so a warning made before a failure comes first (`[1 2` for an MFVec3f:
/// "Array length must be multiple of components size", then "Expected ']'.").
fn log_parser_warnings(warnings: &[String]) {
    for warning in warnings {
        web_sys::console::warn_1(&warning.into());
    }
}

/// X3DScene.toVRMLString / toXMLString / toJSONString of the empty world a canvas has before any load.
#[wasm_bindgen(js_name = printEmptyScene)]
pub fn print_empty_scene(kind: &str) -> Result<String, JsError> {
    Session::print_empty_scene(kind).map_err(|e| JsError::new(&e))
}

/// X3DField.fromVRMLString / fromXMLString (value) without a scene, which needs no world: `[data, warning, ...]`. A value
/// that needs a node is a `TypeError`, as in X_ITE (the parser has no execution context: `scene` is `undefined` or
/// `null`, named by `receiver`).
#[wasm_bindgen(js_name = parseFieldWithoutScene)]
pub fn parse_field_without_scene(
    type_name: &str,
    text: &str,
    xml: bool,
    name: &str,
    receiver: &str,
) -> Result<js_sys::Array, JsError> {
    let ty = x3d_fields::FieldType::from_name(type_name)
        .ok_or_else(|| JsError::new(&format!("unknown field type '{type_name}'")))?;
    let (result, warnings) = x3d_runtime::field_from_string_without_scene(ty, text, xml, name);

    let data = match result {
        Ok(Some(value)) => x3d_runtime::data::value_data(&value),
        Ok(None) => {
            log_parser_warnings(&warnings);
            return Err(JsError::new(&format!(
                "Couldn't read value for field '{name}'."
            )));
        }
        Err(x3d_runtime::NoSceneError::Message(message)) => {
            log_parser_warnings(&warnings);
            return Err(JsError::new(&message));
        }
        Err(x3d_runtime::NoSceneError::NeedsScene) => {
            log_parser_warnings(&warnings);
            return Err(JsError::new(&format!(
                "TypeError: Cannot read properties of {receiver} (reading 'createNode')"
            )));
        }
        Err(x3d_runtime::NoSceneError::NeedsSceneBrowser) => {
            log_parser_warnings(&warnings);
            return Err(JsError::new(&format!(
                "TypeError: Cannot read properties of {receiver} (reading 'getBrowser')"
            )));
        }
    };

    let out = js_sys::Array::of1(&data_js(&data));

    for warning in warnings {
        out.push(&warning.into());
    }

    Ok(out)
}

/// Configuration/SupportedComponents.js and SupportedProfiles.js of the pinned X_ITE (the engine's node table):
/// `[components, profiles]`, each component `[name, title, level, external, [dependency, ...]]`, each profile
/// `[name, title, [name, level, name, level, ...]]`, in X_ITE's order. The aliases are the page's to add.
#[wasm_bindgen(js_name = configTable)]
pub fn config_table() -> js_sys::Array {
    let table = x3d_scene::nodes::table();
    let components = js_sys::Array::new();
    let profiles = js_sys::Array::new();

    for c in &table.components {
        let dependencies = js_sys::Array::new();

        for d in &c.dependencies {
            dependencies.push(&d.into());
        }

        let e = js_sys::Array::new();

        e.push(&c.name.as_str().into());
        e.push(&c.title.as_str().into());
        e.push(&f64::from(c.level).into());
        e.push(&c.external.into());
        e.push(&dependencies);
        components.push(&e);
    }

    for p in &table.profiles {
        let list = js_sys::Array::new();

        for (name, level) in &p.components {
            list.push(&name.as_str().into());
            list.push(&f64::from(*level).into());
        }

        let e = js_sys::Array::new();

        e.push(&p.name.as_str().into());
        e.push(&p.title.as_str().into());
        e.push(&list);
        profiles.push(&e);
    }

    js_sys::Array::of2(&components, &profiles)
}

/// Parser/HTMLSupport.js of the pinned X_ITE (the engine's node table): `[nodeTypeNames, fieldNames]`. The node type names
/// of every concrete node, and the names of their initializeOnly fields (aliases included), once each. The page reads HTML
/// markup with them: `TRANSFORM` is `Transform`, `diffusecolor` is `diffuseColor`.
#[wasm_bindgen(js_name = htmlSupport)]
pub fn html_support() -> js_sys::Array {
    let table = x3d_scene::nodes::table();
    let types = js_sys::Array::new();
    let fields = js_sys::Array::new();
    let mut seen_fields = std::collections::HashSet::new();

    for t in &table.types {
        types.push(&t.name.as_str().into());

        let variants = [&t.vrml2, &t.x3d]
            .into_iter()
            .chain(t.legacy.iter().map(|(_, v)| v))
            .chain(t.newer.iter().map(|(_, v)| v));

        for v in variants {
            let names = v
                .fields
                .iter()
                .filter(|f| f.access & 1 != 0)
                .map(|f| f.name.as_str())
                .chain(v.aliases.keys().map(String::as_str));

            for name in names {
                if seen_fields.insert(name.to_string()) {
                    fields.push(&name.into());
                }
            }
        }
    }

    js_sys::Array::of2(&types, &fields)
}

// --- the page's calls from a field callback in the route chain (issue #202, crate::chain) --------------------------------

fn chain_run<R>(world: u32, f: impl FnOnce(&mut x3d_runtime::Runtime) -> R) -> Result<R, JsError> {
    crate::chain::with_runtime(world, f).map_err(|e| JsError::new(&e))
}

/// `WebBrowser.exists` for the world whose callback is running.
#[wasm_bindgen(js_name = chainExists)]
pub fn chain_exists(world: u32) -> bool {
    crate::chain::is_active(world)
}

/// `WebBrowser.worldGeneration` inside a field callback.
#[wasm_bindgen(js_name = chainWorldGeneration)]
pub fn chain_world_generation(world: u32) -> Result<f64, JsError> {
    chain_run(world, |rt| crate::chain::world_generation(rt) as f64)
}

/// `WebBrowser.takeReleasedNodes` inside a field callback.
#[wasm_bindgen(js_name = chainTakeReleasedNodes)]
pub fn chain_take_released_nodes(world: u32) -> Result<Vec<u32>, JsError> {
    chain_run(world, |rt| rt.take_page_released())
}

/// `WebBrowser.fieldNames` inside a field callback.
#[wasm_bindgen(js_name = chainFieldNames)]
pub fn chain_field_names(world: u32, node: u32) -> Result<Vec<String>, JsError> {
    chain_run(world, |rt| crate::chain::field_names(rt, node))
}

/// `WebBrowser.fieldData` inside a field callback.
#[wasm_bindgen(js_name = chainFieldData)]
pub fn chain_field_data(world: u32, node: u32, name: &str) -> Result<js_sys::Array, JsError> {
    let (type_name, access, data) =
        chain_run(world, |rt| crate::chain::field_data(rt, node, name))?
            .map_err(|e| JsError::new(&e))?;

    Ok(js_sys::Array::of3(
        &type_name.into(),
        &f64::from(access).into(),
        &data_js(&data),
    ))
}

/// `WebBrowser.setFieldData` inside a field callback: the field takes the value and its event is queued with the events
/// the chain is processing.
#[wasm_bindgen(js_name = chainSetFieldData)]
pub fn chain_set_field_data(
    world: u32,
    node: u32,
    name: &str,
    data: JsValue,
) -> Result<(), JsError> {
    chain_run(world, |rt| {
        crate::chain::set_field_data(rt, node, name, &js_data(&data))
    })?
    .map_err(|e| JsError::new(&e))
}

/// `WebBrowser.takeFieldChanges` inside a field callback.
#[wasm_bindgen(js_name = chainTakeFieldChanges)]
pub fn chain_take_field_changes(world: u32) -> Result<js_sys::Array, JsError> {
    chain_run(world, |rt| field_list(crate::chain::take_field_changes(rt)))
}

/// `WebBrowser.observeField` inside a field callback (X3DField.addFieldCallback): the next events of the field reach the sink,
/// also those still queued in the chain.
#[wasm_bindgen(js_name = chainObserveField)]
pub fn chain_observe_field(world: u32, node: u32, name: &str, on: bool) -> Result<(), JsError> {
    chain_run(world, |rt| crate::chain::observe_field(rt, node, name, on))?
        .map_err(|e| JsError::new(&e))
}

/// `WebBrowser.typeName` inside a field callback.
#[wasm_bindgen(js_name = chainTypeName)]
pub fn chain_type_name(world: u32, node: u32) -> Result<Option<String>, JsError> {
    chain_run(world, |rt| crate::chain::type_name(rt, node))
}

/// `WebBrowser.nodeName` inside a field callback.
#[wasm_bindgen(js_name = chainNodeName)]
pub fn chain_node_name(world: u32, node: u32) -> Result<Option<String>, JsError> {
    chain_run(world, |rt| crate::chain::node_name(rt, node))
}

/// `WebBrowser.namedNode` inside a field callback.
#[wasm_bindgen(js_name = chainNamedNode)]
pub fn chain_named_node(world: u32, name: &str) -> Result<Option<u32>, JsError> {
    chain_run(world, |rt| crate::chain::named_node(rt, name))
}

/// `WebBrowser.rootNodes` inside a field callback.
#[wasm_bindgen(js_name = chainRootNodes)]
pub fn chain_root_nodes(world: u32) -> Result<Vec<i64>, JsError> {
    chain_run(world, |rt| {
        crate::chain::root_nodes(rt)
            .into_iter()
            .map(|n| n.map_or(-1, i64::from))
            .collect()
    })
}

/// `WebBrowser.setRootNodes` inside a field callback: `rootNodes .assign (list)` of the world, as a Script's does.
#[wasm_bindgen(js_name = chainSetRootNodes)]
pub fn chain_set_root_nodes(world: u32, list: js_sys::Array) -> Result<(), JsError> {
    let list = root_node_list(&list)?;

    chain_run(world, |rt| crate::chain::set_root_nodes(rt, list))?.map_err(|e| JsError::new(&e))
}

/// `WebBrowser.nodeValue` inside a field callback.
#[wasm_bindgen(js_name = chainNodeValue)]
pub fn chain_node_value(world: u32, node: u32, name: &str) -> Result<Vec<i64>, JsError> {
    let nodes = chain_run(world, |rt| crate::chain::node_value(rt, node, name))?
        .map_err(|e| JsError::new(&e))?;

    Ok(nodes.into_iter().map(|n| n.map_or(-1, i64::from)).collect())
}

/// `WebBrowser.routes` inside a field callback.
#[wasm_bindgen(js_name = chainRoutes)]
pub fn chain_routes(world: u32) -> Result<js_sys::Array, JsError> {
    chain_run(world, |rt| {
        crate::chain::routes(rt).iter().map(route_js).collect()
    })
}

/// `WebBrowser.getRoute` inside a field callback.
#[wasm_bindgen(js_name = chainGetRoute)]
pub fn chain_get_route(
    world: u32,
    source: u32,
    source_field: &str,
    destination: u32,
    destination_field: &str,
) -> Result<JsValue, JsError> {
    let route = chain_run(world, |rt| {
        crate::chain::get_route(rt, source, source_field, destination, destination_field)
    })?
    .map_err(|e| JsError::new(&e))?;

    Ok(route.map_or(JsValue::UNDEFINED, |r| route_js(&r).into()))
}

/// `WebBrowser.addRoute` inside a field callback.
#[wasm_bindgen(js_name = chainAddRoute)]
pub fn chain_add_route(
    world: u32,
    source: u32,
    source_field: &str,
    destination: u32,
    destination_field: &str,
) -> Result<js_sys::Array, JsError> {
    chain_run(world, |rt| {
        crate::chain::add_route(rt, source, source_field, destination, destination_field)
    })?
    .map(|r| route_js(&r))
    .map_err(|e| JsError::new(&e))
}

/// `WebBrowser.deleteRoute` inside a field callback.
#[wasm_bindgen(js_name = chainDeleteRoute)]
pub fn chain_delete_route(world: u32, route: u32) -> Result<bool, JsError> {
    chain_run(world, |rt| crate::chain::delete_route(rt, route as usize))
}

/// `WebBrowser.fieldDefinitions` inside a field callback.
#[wasm_bindgen(js_name = chainFieldDefinitions)]
pub fn chain_field_definitions(world: u32, node: u32) -> Result<js_sys::Array, JsError> {
    chain_run(world, |rt| {
        crate::chain::field_definitions(rt, node)
            .into_iter()
            .map(field_definition_js)
            .collect()
    })
}

fn field_definition_js(d: crate::session::FieldDefinition) -> JsValue {
    let entry = js_sys::Array::of3(
        &d.name.as_str().into(),
        &f64::from(d.access).into(),
        &d.type_name.into(),
    );

    entry.push(&d.default.as_ref().map_or(JsValue::UNDEFINED, data_js));
    JsValue::from(entry)
}
