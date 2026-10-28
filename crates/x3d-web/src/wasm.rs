//! The wasm32 module behind `web/x_ite_r.mjs`: a [`WebBrowser`] owns the WebGPU device of one `<x3d-canvas>`, the
//! renderer, and its worlds ([`Session`]). The page's adapter fetches the world, runs the `requestAnimationFrame`
//! loop, forwards the input events and calls [`WebBrowser::frame`]; everything X3D happens here.
//!
//! A world is a session with an id. The current world is drawn; the others are scenes that load
//! (`createX3DFromString`, a `loadURL` in progress) and wait for [`WebBrowser::replace`]. Each world has its own
//! [`WebFetcher`]: the page's `fetch`, never a synchronous request. Its requests end with the world.

use crate::session::{Pointer, Session, Touch};
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
    current: Option<u32>,
    next_id: u32,
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
            current: None,
            next_id: 1,
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
        let id = self.next_id;

        self.next_id += 1;
        self.worlds.insert(
            id,
            World {
                session: Session::begin(world, now, self.config.width, self.config.height),
                cache,
            },
        );
        id
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
        let id = self.next_id;

        self.next_id += 1;
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
            self.worlds.remove(&old);
        }

        let w = self.worlds.get_mut(&id).ok_or_else(no_world)?;

        w.session.resize(self.config.width, self.config.height);
        w.session.go_live(now);

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

    /// The node id of a DEF name of scene `id`, or `undefined`.
    #[wasm_bindgen(js_name = namedNode)]
    pub fn named_node(&self, id: u32, name: &str) -> Option<u32> {
        self.worlds.get(&id)?.session.named_node(name)
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

    /// The events of the observed fields, as node, name, data, ... in event order (see
    /// [`Session::take_field_events`]).
    #[wasm_bindgen(js_name = takeFieldEvents)]
    pub fn take_field_events(&mut self, id: u32) -> js_sys::Array {
        match self.worlds.get_mut(&id) {
            Some(w) => field_list(w.session.take_field_events()),
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
            .map(|d| {
                let entry = js_sys::Array::of3(
                    &d.name.as_str().into(),
                    &f64::from(d.access).into(),
                    &d.type_name.into(),
                );

                entry.push(&d.default.as_ref().map_or(JsValue::UNDEFINED, data_js));
                JsValue::from(entry)
            })
            .collect()
    }

    /// Set the field from its VRML encoding (see [`Session::set_field`]).
    #[wasm_bindgen(js_name = setField)]
    pub fn set_field(&mut self, id: u32, node: u32, name: &str, vrml: &str) -> Result<(), JsError> {
        self.session_mut(id)?
            .set_field(node, name, vrml)
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

    /// X3DBrowser.viewAll (transitionTime).
    #[wasm_bindgen(js_name = viewAll)]
    pub fn view_all(&mut self, transition_time: f64) {
        if let Some(s) = self.current_mut() {
            s.view_all(transition_time);
        }
    }

    /// X3DBrowserContext.getActiveViewpoint of the current world: the node id, or `undefined` for the default one.
    #[wasm_bindgen(js_name = activeViewpoint)]
    pub fn active_viewpoint(&self) -> Option<u32> {
        self.worlds.get(&self.current?)?.session.active_viewpoint()
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
            return new;
        }

        self.worlds
            .get_mut(&id)
            .map_or(vec![], |w| w.session.take_log())
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
