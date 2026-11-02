//! The engine side of Browser/DOMIntegration.js (issue #146): an element added to the `<X3D>` element is parsed into the context
//! of its parent (`processAddedNode`), an attribute that changed is read into the field of its node (`processAttribute`), and the
//! node of a removed element is disposed (`processRemovedNode`). The page finds the element of a node: the parse reports what each
//! XML element made ([`XmlTarget`], `xmlElement [_node]` of XMLParser.js).

use crate::{CtxId, FKey, Runtime};
use x3d_fields::{FieldValue, NodeId};
use x3d_scene::{FieldLoc, NodeKind, XmlTarget};

impl Runtime {
    /// DOMIntegration.processAddedNode: `text` is the XML of one element. It is parsed with `XMLParser.childElement` into `ctx`
    /// (the nodes belong to the context of `parent` when there is one), below `parent` or among the root nodes of `ctx`, and the
    /// nodes made are set up like the nodes of a field's value. The result is what each element of `text` made, by document order.
    pub fn xml_child_into(
        &mut self,
        ctx: CtxId,
        parent: Option<NodeId>,
        text: &str,
    ) -> Result<Vec<(usize, XmlTarget)>, String> {
        let first_new = self.scene.nodes.len() as NodeId;
        let first_setup = self.scene.setup_order.len();
        let before = parent.map(|p| self.node_fields_snapshot(p));
        let mut scene = std::mem::replace(&mut self.scene, x3d_scene::Scene::new(""));

        // The nodes it creates are found as the range `first_new..`: none goes into a released slot.
        scene.append_only = true;

        let (mut scene, result) = x3d_parse::parse_child_into(scene, ctx, parent, text);

        scene.append_only = false;
        self.scene = scene;

        for w in std::mem::take(&mut self.scene.warnings) {
            self.log_warn(w);
        }

        let targets = std::mem::take(&mut self.scene.xml_targets);

        for n in first_new..self.scene.nodes.len() as NodeId {
            self.nrt_mut(n);
            self.construct_node(n);
        }

        self.rebuild_parents();

        let setup: Vec<NodeId> = self.scene.setup_order.drain(first_setup..).collect();

        for n in setup {
            self.setup_node(n);
        }

        self.register_dynamic(first_new..self.scene.nodes.len() as NodeId);

        // The routes of the new ROUTE elements (the parse added them to the context; the world connects them).
        for (_, target) in &targets {
            if let XmlTarget::Route(c, id) = *target {
                if matches!(self.scene.ctx(c).outer, x3d_scene::Outer::Proto(_)) {
                    continue;
                }

                if let Some(r) = self
                    .scene
                    .ctx(c)
                    .routes
                    .iter()
                    .find(|r| r.id == id)
                    .cloned()
                {
                    self.activate_route(&r);
                }
            }
        }

        // XMLParser.addNode: the node joins the field of its parent, or the root nodes, and the field sends its event.
        match (parent, before) {
            (Some(p), Some(before)) => {
                // The parse assigned the values (a node joined a field, a fieldValue): they are set like any value, so
                // the mirrors and the parents follow and the field sends its event.
                for (k, old) in before {
                    let now = self.field(k).value.clone();

                    if format!("{now:?}") != format!("{old:?}") {
                        self.set_value(k, now);
                    }
                }

                let _ = p;
            }
            _ => {
                let list = self.scene.ctx(ctx).root_nodes.clone();

                self.set_root_nodes(ctx, list);
            }
        }

        result.map(|()| targets)
    }

    /// The fields of `node` and what they hold.
    fn node_fields_snapshot(&self, node: NodeId) -> Vec<(FKey, FieldValue)> {
        let n = self.scene.node(node);
        let mut out = vec![];

        for (i, f) in n.fields.iter().enumerate() {
            out.push(((node, FieldLoc::Pre(i)), f.value.clone()));
        }

        for (i, f) in n.user_fields.iter().enumerate() {
            out.push(((node, FieldLoc::User(i)), f.value.clone()));
        }

        out
    }

    /// The node fields of `node` and what they hold.
    fn node_field_values(&self, node: NodeId) -> Vec<(FKey, FieldValue)> {
        let mut out = self.node_fields_snapshot(node);

        out.retain(|(_, v)| matches!(v, FieldValue::SFNode(_) | FieldValue::MFNode(_)));
        out
    }

    /// DOMIntegration.processAttribute: XMLParser.nodeAttribute (attribute, node). The value is read into the field of the
    /// attribute's name when that field is initializable, and set like any other value, so its event follows.
    pub fn xml_attribute(&mut self, node: NodeId, name: &str, value: &str) {
        if !self.scene.is_live(node) {
            return;
        }

        let ctx = self.scene.node(node).ctx;
        let scene = std::mem::replace(&mut self.scene, x3d_scene::Scene::new(""));
        let (scene, result) = x3d_parse::parse_attribute_into(scene, ctx, node, name, value);

        self.scene = scene;

        for w in std::mem::take(&mut self.scene.warnings) {
            self.log_warn(w);
        }

        if let Some((loc, value)) = result {
            self.set_value((node, loc), value);
        }
    }

    /// X3DNode.dispose: the node leaves its context's names, its routes are deleted, and it is removed from every field and root
    /// node list that holds it (the MFNode fields first, then the SFNode fields).
    pub fn dispose_node(&mut self, node: NodeId) {
        if !self.scene.is_live(node) || matches!(self.scene.node(node).kind, NodeKind::Internal(_))
        {
            return;
        }

        let ctx = self.scene.node(node).ctx;
        let name = self.scene.node(node).name.clone();

        if !name.is_empty() {
            self.scene.remove_named_node(ctx, &name);
        }

        for c in 0..self.scene.contexts.len() {
            let ids: Vec<usize> = self
                .scene
                .ctx(c)
                .routes
                .iter()
                .filter(|r| r.source_node == node || r.destination_node == node)
                .map(|r| r.id)
                .collect();

            for id in ids {
                self.delete_route_by_id(c, id);
            }
        }

        for c in 0..self.scene.contexts.len() {
            if self.scene.ctx(c).root_nodes.contains(&Some(node)) {
                let list = self
                    .scene
                    .ctx(c)
                    .root_nodes
                    .iter()
                    .copied()
                    .filter(|n| *n != Some(node))
                    .collect();

                self.set_root_nodes(c, list);
            }
        }

        let mut keys: Vec<(FKey, FieldValue)> = vec![];

        for n in 0..self.scene.nodes.len() as NodeId {
            if !self.scene.is_live(n) || matches!(self.scene.node(n).kind, NodeKind::Internal(_)) {
                continue;
            }

            keys.extend(self.node_field_values(n));
        }

        for (k, value) in &keys {
            if let FieldValue::MFNode(list) = value {
                if list.contains(&Some(node)) {
                    let list = list.iter().copied().filter(|n| *n != Some(node)).collect();

                    self.set_value(*k, FieldValue::MFNode(list));
                }
            }
        }

        for (k, value) in keys {
            if matches!(value, FieldValue::SFNode(Some(n)) if n == node) {
                self.set_value(k, FieldValue::SFNode(None));
            }
        }

        self.rebuild_parents();
    }

    /// X3DExecutionContext.dispose for the `Scene` element that was removed (DOMIntegration.processRemovedNode): the routes are
    /// deleted, the names, imported nodes and declarations are cleared, and the root nodes are gone.
    pub fn dispose_context(&mut self, ctx: CtxId) {
        if ctx >= self.scene.contexts.len() {
            return;
        }

        let routes: Vec<usize> = self.scene.ctx(ctx).routes.iter().map(|r| r.id).collect();

        for id in routes {
            self.delete_route_by_id(ctx, id);
        }

        let c = self.scene.ctx_mut(ctx);

        c.named_nodes = Default::default();
        c.imported = Default::default();
        c.protos = Default::default();
        c.externprotos = Default::default();

        self.set_root_nodes(ctx, vec![]);
        self.dynamic_dirty_mark();
    }
}
