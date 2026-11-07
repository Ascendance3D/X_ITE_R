//! The page's calls into the engine from a field callback that runs inside the route chain (issue #202).
//!
//! X_ITE's X3DField.processEvent calls the field callbacks at once, between two routes, so a callback that writes a field
//! is seen by the next Script of the chain. The engine does the same with [`Runtime::page_event_sink`]: it hands the sink
//! the runtime in the middle of X3DRoutingContext [processEvents] and the page's callback runs there. The callback calls
//! back into the engine, but the `WebBrowser` object is borrowed by the export that runs the chain (`frame`), so those
//! calls go through the free functions of [`crate::wasm`] (`chain*`), which reach the runtime through [`enter`] and
//! [`with_runtime`], and through nothing else.
//!
//! The invariant that makes the one `unsafe` block of [`with_runtime`] sound:
//!
//! 1. The pointer is made from the exclusive borrow `rt` that [`enter`] receives, and `enter` does not use `rt` again until
//!    the callback returns, so for that time the pointer is the only way to the runtime.
//! 2. [`with_runtime`] takes the pointer out of its slot for the duration of its closure, so a second call inside the
//!    closure finds none (an error): there are never two `&mut Runtime`.
//! 3. The closures are the field operations below only: they read and write fields and drain the page's change lists.
//!    They do not process events, call a Script or touch [`Runtime::page_event_sink`], which the runtime took out for the
//!    call. A write queues its event ([`Runtime::set_value`]) on the same list the chain is draining, so it runs
//!    after the events already queued, as a Script's write does.
//! 4. The slot is restored (also on a panic) when `enter` returns, so the pointer never outlives the borrow.

use crate::session::FieldDefinition;
use std::cell::Cell;
use x3d_fields::{FieldType, FieldValue, NodeId};
use x3d_runtime::data::{self, Data};
use x3d_runtime::{FKey, Runtime};

thread_local! {
    /// The runtime in the middle of a route chain, with the id of its world in the `WebBrowser`.
    static ACTIVE: Cell<Option<(*mut Runtime, u32)>> = const { Cell::new(None) };
}

/// Restores the slot of [`ACTIVE`] when dropped.
struct Restore(Option<(*mut Runtime, u32)>);

impl Drop for Restore {
    fn drop(&mut self) {
        ACTIVE.with(|a| a.set(self.0));
    }
}

/// Run `f` (the page's callback) while the field operations of this module can reach `rt`, the runtime of world `world`.
pub fn enter<R>(rt: &mut Runtime, world: u32, f: impl FnOnce() -> R) -> R {
    let _restore = Restore(ACTIVE.with(|a| a.replace(Some((rt as *mut Runtime, world)))));

    f()
}

/// Is a callback of world `world` running (see [`enter`])?
pub fn is_active(world: u32) -> bool {
    ACTIVE.with(|a| a.get()).is_some_and(|(_, w)| w == world)
}

/// The world whose callback is running (see [`enter`]), if any.
pub fn active_world() -> Option<u32> {
    ACTIVE.with(|a| a.get()).map(|(_, w)| w)
}

/// Run `f` on the runtime of world `world` in the middle of its route chain. `Err`: no callback of that world is running, or
/// `with_runtime` is already running (a closure must not call it).
pub fn with_runtime<R>(world: u32, f: impl FnOnce(&mut Runtime) -> R) -> Result<R, String> {
    let taken = ACTIVE.with(|a| match a.get() {
        Some((p, w)) if w == world => a.take().map(|_| p),
        _ => None,
    });
    let Some(p) = taken else {
        return Err(
            "X_ITE_R: this is not available inside the field callback of this world.".to_string(),
        );
    };
    let _restore = Restore(Some((p, world)));

    // SAFETY: see the module documentation. `p` was made by `enter` from the exclusive borrow of the runtime, which `enter`
    // does not use until its callback returns; the slot is empty while `f` runs, so this is the only live `&mut Runtime`.
    let rt = unsafe { &mut *p };

    Ok(f(rt))
}

// --- the field operations of the page, on a runtime (the `Session` methods and the chain's exports share them) ----------

/// Is `node` a node of the world now (not a released or out-of-range id from the page)?
pub fn node_exists(rt: &Runtime, node: NodeId) -> bool {
    (node as usize) < rt.scene.nodes.len() && !rt.scene.free_nodes.contains(&node)
}

/// X3DImportedNodeInstance delegates getNodeTypeName, getField and getFieldDefinitions to the node its Inline exports,
/// while the Inline provides it (`$.try (() => importedNode .getExportedNode ())`); any other node is itself.
pub fn inner_node(rt: &Runtime, node: NodeId) -> NodeId {
    if !node_exists(rt, node) {
        return node;
    }

    if let x3d_scene::NodeKind::Imported(i) = rt.scene.node(node).kind {
        let rec = &rt.scene.imported[i];

        if let Some(&scene) = rt.inline_scenes.get(&rec.inline) {
            if let Some(n) = rt.scene.get_exported_node(scene, &rec.exported_name) {
                return n;
            }
        }
    }

    node
}

pub fn field_key(rt: &Runtime, node: NodeId, name: &str) -> Result<FKey, String> {
    if !node_exists(rt, node) {
        return Err("node is not in the world".to_string());
    }

    let node = inner_node(rt, node);

    rt.get_field(node, name).ok_or_else(|| {
        format!(
            "Unknown field '{name}' in node class {}.",
            rt.type_name(node)
        )
    })
}

pub fn field_names(rt: &Runtime, node: NodeId) -> Vec<String> {
    if !node_exists(rt, node) {
        return vec![];
    }

    let node = inner_node(rt, node);

    let n = rt.scene.node(node);

    n.fields
        .iter()
        .chain(&n.user_fields)
        .map(|f| f.name.clone())
        .collect()
}

pub fn field_data(
    rt: &mut Runtime,
    node: NodeId,
    name: &str,
) -> Result<(&'static str, u8, Data), String> {
    let k = field_key(rt, node, name)?;

    rt.page_mirror.entry(k).or_insert(false);

    let f = rt.field(k);

    Ok((f.ty().name(), f.access, data::value_data(&f.value)))
}

pub fn set_field_data(
    rt: &mut Runtime,
    node: NodeId,
    name: &str,
    value: &Data,
) -> Result<(), String> {
    let k = field_key(rt, node, name)?;
    let ty: FieldType = rt.field(k).ty();

    data::check_data(ty, value).map_err(|e| format!("field '{name}': {e}"))?;

    if let Some(n) = data::data_nodes(ty, value)
        .into_iter()
        .find(|&n| !node_exists(rt, n))
    {
        return Err(format!("field '{name}': node {n} is not in the world"));
    }

    let value = data::data_value(ty, value);

    rt.set_value(k, value);
    // The page's object holds this value already.
    rt.page_clean(k);
    Ok(())
}

pub fn take_field_changes(rt: &mut Runtime) -> Vec<(NodeId, String, Data)> {
    rt.take_page_dirty()
        .into_iter()
        .map(|k| {
            let f = rt.field(k);

            (k.0, f.name.clone(), data::value_data(&f.value))
        })
        .collect()
}

pub fn world_generation(rt: &Runtime) -> u64 {
    rt.world_state.generation
}

pub fn observe_field(rt: &mut Runtime, node: NodeId, name: &str, on: bool) -> Result<(), String> {
    // DOMIntegration listens to `_loadState` of the Inline nodes, a private field.
    let k = match field_key(rt, node, name) {
        Ok(k) => k,
        Err(e) if name == "loadState" && node_exists(rt, node) => {
            rt.private_field(inner_node(rt, node), name).ok_or(e)?
        }
        Err(e) => return Err(e),
    };

    if on {
        rt.page_observed.insert(k);
    } else {
        rt.page_observed.remove(&k);
    }

    Ok(())
}

pub fn type_name(rt: &Runtime, node: NodeId) -> Option<String> {
    let node = inner_node(rt, node);

    node_exists(rt, node).then(|| rt.type_name(node))
}

pub fn node_name(rt: &Runtime, node: NodeId) -> Option<String> {
    node_exists(rt, node).then(|| rt.scene.node(node).name.clone())
}

/// `Session::parse_field` of the world in the middle of its chain: X3DField.fromVRMLString / fromXMLString (value, scene) in a
/// field callback (a node it makes is a node of the world).
pub fn parse_field(
    rt: &mut Runtime,
    ty: FieldType,
    text: &str,
    xml: bool,
    name: &str,
    unit: Option<String>,
) -> Result<Data, String> {
    let ctx = Some(rt.scene.world);

    match rt.field_from_string(ty, text, ctx, xml, unit, name) {
        Ok(Some(value)) => Ok(data::value_data(&value)),
        Ok(None) => Err(format!("Couldn't read value for field '{name}'.")),
        Err(Some(message)) => Err(message),
        Err(None) => Err("bad field value".to_string()),
    }
}

pub fn named_node(rt: &Runtime, name: &str) -> Option<NodeId> {
    rt.scene.get_named_node(rt.scene.world, name)
}

pub fn root_nodes(rt: &Runtime) -> Vec<Option<NodeId>> {
    rt.scene.ctx(rt.scene.world).root_nodes.clone()
}

/// `rootNodes .assign (list)` of the world, as `Session::set_root_nodes`: the world holds `list` at once and the page's
/// `rootNodes` field queues its event with the others (nothing is processed here).
pub fn set_root_nodes(rt: &mut Runtime, list: Vec<Option<NodeId>>) -> Result<(), String> {
    if let Some(n) = list.iter().flatten().find(|&&n| !node_exists(rt, n)) {
        return Err(format!("rootNodes: node {n} is not in the world"));
    }

    let world = rt.scene.world;

    rt.set_root_nodes(world, list);
    Ok(())
}

pub fn node_value(rt: &Runtime, node: NodeId, name: &str) -> Result<Vec<Option<NodeId>>, String> {
    let k = field_key(rt, node, name)?;

    match &rt.field(k).value {
        FieldValue::SFNode(n) => Ok(vec![*n]),
        FieldValue::MFNode(list) => Ok(list.clone()),
        v => Err(format!("{} is not a node field", v.field_type().name())),
    }
}

pub fn routes(rt: &Runtime) -> Vec<crate::session::RouteInfo> {
    rt.scene
        .ctx(rt.scene.world)
        .routes
        .iter()
        .map(crate::session::RouteInfo::of)
        .collect()
}

pub fn get_route(
    rt: &Runtime,
    source: NodeId,
    source_field: &str,
    destination: NodeId,
    destination_field: &str,
) -> Result<Option<crate::session::RouteInfo>, String> {
    if !node_exists(rt, source) || !node_exists(rt, destination) {
        return Err("node is not in the world".to_string());
    }

    let scene = &rt.scene;

    scene.route_ends(source, source_field, destination, destination_field)?;

    Ok(scene
        .find_route_index(
            scene.world,
            source,
            source_field,
            destination,
            destination_field,
        )
        .map(|i| crate::session::RouteInfo::of(&scene.ctx(scene.world).routes[i])))
}

/// X3DExecutionContext.addRoute: the route that exists, else a new one. Like a Script's `addRoute` it only changes the routing
/// tables; the events already queued stay.
pub fn add_route(
    rt: &mut Runtime,
    source: NodeId,
    source_field: &str,
    destination: NodeId,
    destination_field: &str,
) -> Result<crate::session::RouteInfo, String> {
    if let Some(route) = get_route(rt, source, source_field, destination, destination_field)? {
        return Ok(route);
    }

    let world = rt.scene.world;

    rt.add_route(world, source, source_field, destination, destination_field)?;

    // The route is the last one of the world's list.
    Ok(crate::session::RouteInfo::of(
        rt.scene
            .ctx(world)
            .routes
            .last()
            .expect("add_route made a route"),
    ))
}

pub fn delete_route(rt: &mut Runtime, id: usize) -> bool {
    let world = rt.scene.world;

    rt.delete_route_by_id(world, id)
}

pub fn field_definitions(rt: &Runtime, node: NodeId) -> Vec<FieldDefinition> {
    if !node_exists(rt, node) {
        return vec![];
    }

    let node = inner_node(rt, node);

    let scene = &rt.scene;
    let n = scene.node(node);
    let defaults = match &n.kind {
        x3d_scene::NodeKind::Builtin(t) => Some(
            &scene.table.types[*t]
                .variant(scene.spec_version_of(n.ctx))
                .fields,
        ),
        _ => None,
    };
    let definition = |f: &x3d_scene::Field, default: Option<Data>| FieldDefinition {
        name: f.name.clone(),
        access: f.access,
        type_name: f.ty().name(),
        default,
    };

    n.fields
        .iter()
        .map(|f| {
            let default = defaults
                .and_then(|d| d.iter().find(|d| d.name == f.name))
                .map(|d| data::value_data(&d.default));

            definition(f, default)
        })
        .chain(n.user_fields.iter().map(|f| definition(f, None)))
        .collect()
}
