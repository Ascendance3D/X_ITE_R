//! One loaded world of an `<x3d-canvas>`: the Rust runtime and the live frame loop of x3d-viewer, without a window,
//! a file system or a GPU. The wasm32 module ([`crate::wasm`]) draws its frames; the native tests drive it directly.
//!
//! The steps are those of `Viewer3d::load_attributes` (X3DBrowser.loadURL → FileLoader → replaceWorld), with the
//! world's bytes and URL from the page instead of a file, and no Script host (QuickJS does not build for the web).

use x3d_fields::{FieldValue, NodeId, access};
use x3d_render::traverse::Frame;
use x3d_runtime::{Fetcher, Runtime};
use x3d_viewer::viewer::Viewer3d;

/// A field of a node as the page sees it: X3DField.getTypeName, the access type and `toString ()`.
#[derive(Clone, Debug, PartialEq)]
pub struct FieldInfo {
    pub type_name: &'static str,
    pub access: &'static str,
    pub value: String,
}

pub struct Session {
    viewer: Viewer3d,
    world_url: String,
    /// How much of the runtime's log [`Session::take_log`] returned.
    log_seen: usize,
}

impl Session {
    /// Parse and set up the world `data` from `url`, then run replaceWorld up to the first live frame at `now` (seconds
    /// since the Unix epoch, X3DBrowser.getCurrentTime). `fetcher` serves the world's Inline, EXTERNPROTO and texture
    /// URLs. A world that does not parse is an error and no session is made.
    pub fn load(
        data: &[u8],
        url: &str,
        fetcher: Box<dyn Fetcher>,
        now: f64,
        width: u32,
        height: u32,
    ) -> Result<Session, String> {
        x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);
        x3d_render::traverse::install_particle_geometry();

        let scene = x3d_runtime::parse_bytes_with_fetcher(&*fetcher, data, url)
            .scene
            .map_err(|e| format!("{e:?}"))?;
        let mut rt = Runtime::new(scene, fetcher);

        rt.page_url = url.to_string();
        x3d_render::shader_gpu::install(&mut rt);
        rt.live = true;
        rt.now = now;
        rt.browser_options = x3d_runtime::browser_options::BrowserOptions::new(width, height);
        rt.load_scene();
        rt.replace_world();

        Ok(Session {
            viewer: Viewer3d::from_runtime(rt, now, width, height),
            world_url: url.to_string(),
            log_seen: 0,
        })
    }

    /// X3DScene.worldURL.
    pub fn world_url(&self) -> &str {
        &self.world_url
    }

    fn rt(&self) -> &Runtime {
        &self.viewer.rt
    }

    /// Is `node` a node of the world now (not a released or out-of-range id from the page)?
    fn node_exists(&self, node: NodeId) -> bool {
        let scene = &self.rt().scene;

        (node as usize) < scene.nodes.len() && !scene.free_nodes.contains(&node)
    }

    /// X3DExecutionContext.getNamedNode (name) of the world; `None` where X_ITE throws "Named node not found".
    pub fn named_node(&self, name: &str) -> Option<NodeId> {
        let scene = &self.rt().scene;

        scene.get_named_node(scene.world, name)
    }

    /// X3DExecutionContext.rootNodes of the world (NULL entries kept).
    pub fn root_nodes(&self) -> Vec<Option<NodeId>> {
        let scene = &self.rt().scene;

        scene.ctx(scene.world).root_nodes.clone()
    }

    /// X3DNode.getNodeTypeName.
    pub fn type_name(&self, node: NodeId) -> Option<String> {
        self.node_exists(node).then(|| self.rt().type_name(node))
    }

    /// X3DNode.getNodeName.
    pub fn node_name(&self, node: NodeId) -> Option<String> {
        self.node_exists(node)
            .then(|| self.rt().scene.node(node).name.clone())
    }

    /// The nodes of an SFNode or MFNode field (`None` for NULL); `Err` for a field of another type.
    pub fn node_value(&self, node: NodeId, name: &str) -> Result<Vec<Option<NodeId>>, String> {
        let k = self.field_key(node, name)?;

        match &self.rt().field(k).value {
            FieldValue::SFNode(n) => Ok(vec![*n]),
            FieldValue::MFNode(list) => Ok(list.clone()),
            v => Err(format!("{} is not a node field", v.field_type().name())),
        }
    }

    /// The names of X3DNode.getFieldDefinitions, then the user-defined fields.
    pub fn field_names(&self, node: NodeId) -> Vec<String> {
        if !self.node_exists(node) {
            return vec![];
        }

        let n = self.rt().scene.node(node);

        n.fields
            .iter()
            .chain(&n.user_fields)
            .map(|f| f.name.clone())
            .collect()
    }

    /// X3DNode.getField (name): its type, access type and `toString ()` value.
    pub fn field(&self, node: NodeId, name: &str) -> Result<FieldInfo, String> {
        let k = self.field_key(node, name)?;
        let rt = self.rt();
        let f = rt.field(k);

        Ok(FieldInfo {
            type_name: f.ty().name(),
            access: access::name(f.access),
            value: value_string(rt, &f.value),
        })
    }

    /// `node [name] = value` (SFNode's proxy setter, X3DField.setValue) with the value in the VRML encoding: the field
    /// takes the value now and sends its event in the next frame's event cascade. An outputOnly field refuses it, like
    /// the proxy setter's `return false`.
    pub fn set_field(&mut self, node: NodeId, name: &str, vrml: &str) -> Result<(), String> {
        let k = self.field_key(node, name)?;
        let rt = &mut self.viewer.rt;
        let (ty, access) = {
            let f = rt.field(k);
            (f.ty(), f.access)
        };

        if access == access::OUTPUT_ONLY {
            return Err(format!("field '{name}' is outputOnly"));
        }

        let ctx = rt.scene.node(node).ctx;
        let value = match rt.field_from_string(ty, vrml, Some(ctx), false, None, name) {
            Ok(Some(v)) => v,
            Ok(None) => return Err(format!("no {} value in '{vrml}'", ty.name())),
            Err(e) => return Err(e.unwrap_or_else(|| format!("bad {} value", ty.name()))),
        };

        rt.set_value(k, value);
        Ok(())
    }

    fn field_key(&self, node: NodeId, name: &str) -> Result<x3d_runtime::FKey, String> {
        if !self.node_exists(node) {
            return Err("node is not in the world".to_string());
        }

        self.rt().get_field(node, name).ok_or_else(|| {
            format!(
                "Unknown field '{name}' in node class {}.",
                self.rt().type_name(node)
            )
        })
    }

    /// One frame of the browser's event loop at `now` (seconds since the Unix epoch).
    pub fn step(&mut self, now: f64) {
        self.viewer.date_now = now;
        self.viewer.step(now);
    }

    pub fn resize(&mut self, width: u32, height: u32) {
        self.viewer.resize(width.max(1), height.max(1));
    }

    /// The frame to draw: the DISPLAY traversal of the current state.
    pub fn build_frame(&mut self) -> Frame {
        self.viewer.build_frame()
    }

    /// The warnings and errors the runtime logged (parse warnings, failed loads, ...) since the last call.
    pub fn take_log(&mut self) -> Vec<String> {
        let log = &self.viewer.rt.log;
        let new = log[self.log_seen.min(log.len())..].to_vec();

        self.log_seen = log.len();
        new
    }

    /// The live frame loop (navigation, bindables, sensors) of the world.
    pub fn viewer(&mut self) -> &mut Viewer3d {
        &mut self.viewer
    }
}

/// The world goes like on X3DBrowser.replaceWorld: `shutdown` of its nodes, then its scenes are disposed.
impl Drop for Session {
    fn drop(&mut self) {
        self.viewer.rt.unload_world();
    }
}

/// X3DField.toString (): the VRML encoding, `TypeName { }` for node values.
fn value_string(rt: &Runtime, value: &FieldValue) -> String {
    match value {
        FieldValue::SFNode(_) | FieldValue::MFNode(_) => rt.scene.node_value_to_string(value),
        _ => rt.scene.node_value_to_vrml_string(value),
    }
}
