//! Scenes a Script asks the browser for: X3DBrowser.createX3DFromString, createX3DFromURL and
//! createVrmlFromURL (Browser/X3DBrowser.js, InputOutput/FileLoader.js).
//!
//! Like an Inline's scene (see `inline.rs`), the X3DScene they return is merged into the runtime's node arena as a
//! context of its own ([`x3d_scene::Scene::merge_extern`]), a child of the context of the calling Script
//! (`scene .setExecutionContext (currentScene)`). The timeline is FileLoader's:
//!
//! 1. `createX3DFromString` parses at once. `createX3DFromURL` fetches in a macrotask (`Task::BrowserLoad`).
//! 2. The scene's nodes are set up (a Script's `initialize ()` runs), then `scene ._loadCount` gets an event;
//!    `set_loadCount__` waits for the count to reach 0 (nested Inlines) and then for `browser .nextFrame ()`.
//! 3. X3DBrowser `scene .setLive (true)`, then the promise (or the `createVrmlFromURL` event) settles.
//!
//! The promise itself lives in JavaScript; [`ScriptHost::settle`] hands the outcome back.

use crate::loader::{FileLoader, Loaded};
use crate::*;

/// A request that has not been parsed yet (`createX3DFromURL`).
#[derive(Clone, Debug)]
pub struct BrowserLoad {
    pub script: Option<NodeId>,
    pub parent: CtxId,
    pub urls: Vec<String>,
}

/// A scene that was parsed and set up and waits for its loadCount to reach 0.
#[derive(Clone, Debug)]
pub struct PendingBrowserScene {
    pub id: u64,
    pub script: Option<NodeId>,
    pub ctx: CtxId,
    pub scene_node: NodeId,
}

impl Runtime {
    /// X3DBrowser.createX3DFromString: parse `text` in the Script's scene. `Err` is the message of the rejection.
    pub fn browser_create_from_string(&mut self, text: &str, parent: CtxId, script: Option<NodeId>) -> Result<u64, String> {
        let id = self.next_browser_request();
        let world = self.scene.world_url_of(parent).to_string();

        self.browser_scene(id, script, parent, text.as_bytes(), &world)?;
        Ok(id)
    }

    /// X3DBrowser.createX3DFromURL: the fetch is a macrotask.
    pub fn browser_create_from_url(&mut self, urls: Vec<String>, parent: CtxId, script: Option<NodeId>) -> u64 {
        let id = self.next_browser_request();

        self.browser_loads.insert(id, BrowserLoad { script, parent, urls });
        self.tasks.push_back(Task::BrowserLoad(id));
        id
    }

    fn next_browser_request(&mut self) -> u64 {
        self.browser_request_id += 1;
        self.browser_request_id
    }

    /// `Task::BrowserLoad`: FileLoader.createX3DFromURL (url, null, callback).
    pub(crate) fn browser_load(&mut self, id: u64) {
        let Some(req) = self.browser_loads.remove(&id) else {
            return;
        };

        let type_name = req.script.map(|s| self.type_name(s)).unwrap_or_default();
        let loader = FileLoader { type_name, base: self.scene.world_url_of(req.parent).to_string(), data_as_string: true };
        let base = loader.base.clone();

        let ok = self.load_document(&loader, &req.urls, |rt, loaded| {
            let (data, url) = match loaded {
                Loaded::Script(source) => (source.into_bytes(), format!("{}null", base.rsplit_once('/').map_or(String::new(), |(d, _)| format!("{d}/")))),
                Loaded::Data(data, url) => (data, url.to_string()),
            };

            rt.browser_scene(id, req.script, req.parent, &data, &url)
        });

        // loadDocumentError: `callback (null)`.
        if !ok {
            self.browser_settle(id, Err("Couldn't load X3D file.".into()));
        }
    }

    /// FileLoader.createX3DFromString for the browser: parse into a new X3DScene, set up its nodes and wait for its loadCount.
    fn browser_scene(&mut self, id: u64, script: Option<NodeId>, parent: CtxId, data: &[u8], url: &str) -> Result<(), String> {
        self.sync_svg_quality();

        let mut scene = match crate::loader::parse_bytes_with_fetcher(&*self.fetcher, data, url) {
            Ok(scene) => scene,
            Err(x3d_parse::LoadError::Parse(e)) if e.line == 0 && ["TypeError: ", "RangeError: ", "Error: "].iter().any(|p| e.message.starts_with(p)) => {
                return Err(e.message);
            }
            Err(x3d_parse::LoadError::Parse(e)) => {
                // VRMLParser (DEVELOPMENT) prints the original error before it is rethrown.
                self.log_error(format!("Error: {}", e.message));
                return Err(format!("Error: \n{e}"));
            }
            // GoldenGate.parseIntoScene.
            Err(x3d_parse::LoadError::NoHandler) => return Err(format!("Couldn't parse X3D. No suitable file handler found for '{url}'.")),
            Err(e) => return Err(format!("Error: {e}")),
        };

        for w in std::mem::take(&mut scene.warnings) {
            self.log_warn(w);
        }

        let setup_order = std::mem::take(&mut scene.setup_order);
        let ctx_count = scene.contexts.len();
        let m = self.scene.merge_extern(scene, parent);
        let (scene_node, count) = self.new_load_count_scene();

        self.pending_browser.push(PendingBrowserScene { id, script, ctx: m.root_ctx, scene_node });

        // `new X3DScene (browser)`: setLive (false); the Browser makes it live once it is delivered.
        self.inline_live.insert(m.root_ctx, false);
        self.attach_merged_scene(m, ctx_count, setup_order);

        // FileLoader.setScene: `scene ._loadCount .addEvent ()`.
        self.add_event(count);
        Ok(())
    }

    /// FileLoader.set_loadCount__ for a browser scene whose count reached 0: wait for the next frame.
    pub(crate) fn browser_scene_loaded(&mut self, scene_node: NodeId) -> bool {
        let Some(i) = self.pending_browser.iter().position(|p| p.scene_node == scene_node) else {
            return false;
        };

        let k = self.private_field(scene_node, "loadCount").unwrap();

        if !matches!(self.value(k), FieldValue::SFInt32(0)) {
            return true;
        }

        self.remove_interest(k, scene_node, Cb::SceneLoadCount);

        let p = self.pending_browser.remove(i);
        self.browser_frame_waiters.push(p);
        true
    }

    /// The continuation of `await browser .nextFrame ()`: `scene .setLive (true)`, then the promise settles.
    pub(crate) fn browser_finish_frame(&mut self) {
        for w in std::mem::take(&mut self.browser_frame_waiters) {
            self.inline_live.insert(w.ctx, true);
            self.refresh_live();
            self.browser_settle(w.id, Ok(w.ctx));
        }
    }

    fn browser_settle(&mut self, id: u64, outcome: Result<CtxId, String>) {
        let Some(mut host) = self.script_host.take() else {
            return;
        };

        host.settle(self, id, outcome);
        self.script_host = Some(host);
        self.dynamic_dirty = true;
    }

    /// Is a browser scene still loading?
    pub(crate) fn browser_loads_pending(&self) -> bool {
        !self.browser_loads.is_empty() || !self.pending_browser.is_empty() || !self.browser_frame_waiters.is_empty()
    }
}
