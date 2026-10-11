//! `scene .toVRMLString ()`: X3DScene.toVRMLStream, X3DExecutionContext.toVRMLStream,
//! X3DNode.toVRMLStream, X3DProtoDeclaration / X3DExternProtoDeclaration / X3DRoute
//! toVRMLStream, and the name bookkeeping of X3DGenerator.js.

use crate::scene::*;
use std::collections::{HashMap, HashSet};
use x3d_fields::{FieldValue, Generator, NodeId, access};

#[derive(Default)]
struct CtxIndex {
    names: HashSet<String>,
    names_by_node: HashMap<NodeId, String>,
    nodes: HashSet<NodeId>,
    route_nodes: HashSet<NodeId>,
}

pub struct VrmlGenerator<'a> {
    pub g: Generator,
    scene: &'a Scene,
    /// `None` is the `null` execution context pushed by X3DObject.toVRMLString.
    stack: Vec<Option<CtxId>>,
    index: HashMap<CtxId, CtxIndex>,
}

impl<'a> VrmlGenerator<'a> {
    pub fn new(scene: &'a Scene) -> Self {
        VrmlGenerator { g: Generator::default(), scene, stack: vec![], index: HashMap::new() }
    }

    pub fn push_ctx(&mut self, ctx: Option<CtxId>) {
        self.stack.push(ctx);

        if let Some(c) = ctx {
            if self.index.contains_key(&c) {
                return;
            }

            self.index.insert(c, CtxIndex::default());
            self.g.needs_space = false;
        }
    }

    pub fn pop_ctx(&mut self) {
        let ctx = self.stack.pop().flatten();

        if self.stack.last().copied().flatten() == ctx && ctx.is_some() {
            return;
        }

        if let Some(c) = ctx {
            self.index.remove(&c);
        }
    }

    fn current(&self) -> Option<CtxId> {
        self.stack.last().copied().flatten()
    }

    fn cur_index(&mut self) -> &mut CtxIndex {
        let c = self.current().expect("execution context");
        self.index.get_mut(&c).unwrap()
    }

    /// X3DGenerator.NamedNodes.
    fn named_nodes(&mut self, ctx: CtxId) {
        let scene = self.scene;
        let index = self.cur_index();

        for &node in scene.ctx(ctx).named_nodes.values() {
            let name = &scene.node(node).name;

            if is_underscore_number(name) {
                continue;
            }

            index.names.insert(name.clone());
            index.names_by_node.insert(node, name.clone());
        }
    }

    /// X3DGenerator.Name.
    fn name(&mut self, node: NodeId) -> String {
        if let Some(n) = self.cur_index().names_by_node.get(&node) {
            return n.clone();
        }

        let name = self.scene.node(node).name.clone();

        if (name.is_empty() || is_underscore_number(&name)) && !self.needs_name(node) {
            return String::new();
        }

        let index = self.cur_index();
        let new_name = crate::info_array::get_unique_name(|n| index.names.contains(n), &name);

        index.names.insert(new_name.clone());
        index.names_by_node.insert(node, new_name.clone());
        new_name
    }

    /// X3DGenerator.NeedsName: clone count > 1 (known once the runtime ran), or routes.
    fn needs_name(&self, node: NodeId) -> bool {
        if self.scene.clone_counts.as_ref().is_some_and(|c| c.get(&node).is_some_and(|n| *n > 1)) {
            return true;
        }

        self.scene.has_routes(node)
    }

    fn is_shared_node(&self, node: NodeId) -> bool {
        self.current() != Some(self.scene.node(node).ctx)
    }

    // --- scene -------------------------------------------------------------------------------

    pub fn scene_to_vrml(mut self) -> String {
        let scene = self.scene;

        self.push_ctx(None);

        let g = &mut self.g;

        g.indent();
        g.push("#X3D V");
        g.push(LATEST_VERSION);
        g.space();
        g.push("utf8");
        g.space();
        g.push(&scene.table.browser_name);
        g.space();
        g.push("V");
        g.push(&scene.table.browser_version);
        g.push("\n");
        g.push("\n");

        if let Some(profile) = &scene.profile {
            g.indent();
            g.push("PROFILE");
            g.space();
            g.push(profile);
            g.brk();
            g.tidy_break();
        }

        if !scene.components.is_empty() {
            for (name, level) in &scene.components {
                g.indent();
                g.push("COMPONENT");
                g.space();
                g.push(name);
                g.tidy_space();
                g.push(":");
                g.tidy_space();
                g.push(&level.to_string());
                g.brk();
            }

            g.tidy_break();
        }

        if !scene.meta.is_empty() {
            for (key, value) in &scene.meta {
                g.indent();
                g.push("META");
                g.space();
                g.push(&format!("\"{}\"", x3d_fields::value::escape(key)));
                g.space();
                g.push(&format!("\"{}\"", x3d_fields::value::escape(value)));
                g.brk();
            }

            g.tidy_break();
        }

        self.push_ctx(Some(ROOT));
        self.context_to_vrml(ROOT);
        self.pop_ctx();
        self.pop_ctx();

        self.g.string
    }

    /// X3DExecutionContext.toVRMLStream.
    fn context_to_vrml(&mut self, ctx: CtxId) {
        let scene = self.scene;

        self.push_ctx(Some(ctx));
        self.named_nodes(ctx);

        // Extern protos, then protos (X3DInfoArray.toVRMLStream with the "Proto" extra break).
        for &e in scene.ctx(ctx).externprotos.values() {
            self.extern_proto_to_vrml(e);
            self.g.brk();
            self.g.tidy_break();
        }

        for &p in scene.ctx(ctx).protos.values() {
            self.proto_to_vrml(p);
            self.g.brk();
            self.g.tidy_break();
        }

        let root_nodes = &scene.ctx(ctx).root_nodes;
        let last = root_nodes.len().wrapping_sub(1);

        for (i, root) in root_nodes.iter().enumerate() {
            self.g.indent();

            match root {
                Some(n) => self.node_to_vrml(*n),
                None => self.g.null(),
            }

            self.g.tidy_break();

            if i != last {
                self.g.tidy_break();
            }
        }

        let routes = &scene.ctx(ctx).routes;

        if !routes.is_empty() {
            self.g.tidy_break();

            for r in routes {
                if self.route_to_vrml(r) {
                    self.g.brk();
                }
            }
        }

        self.pop_ctx();
    }

    /// X3DRoute.toVRMLStream; `false` where X_ITE throws (and the array printer skips it).
    fn route_to_vrml(&mut self, route: &Route) -> bool {
        {
            let index = self.cur_index();

            if !index.route_nodes.contains(&route.source_node) || !index.route_nodes.contains(&route.destination_node) {
                return false;
            }
        }

        let source = self.name(route.source_node);
        let destination = self.name(route.destination_node);
        let g = &mut self.g;

        g.indent();
        g.push("ROUTE");
        g.space();
        g.push(&source);
        g.push(".");
        g.push(&route.source_field);
        g.space();
        g.push("TO");
        g.space();
        g.push(&destination);
        g.push(".");
        g.push(&route.destination_field);
        true
    }

    fn interface_header(&mut self, keyword: &str, name: &str, fields: &[Field], values: bool) {
        let g = &mut self.g;

        g.indent();
        g.push(keyword);
        g.space();
        g.push(name);
        g.tidy_space();
        g.push("[");

        if fields.is_empty() {
            g.tidy_space();
        } else {
            let type_len = fields.iter().map(|f| f.ty().name().len()).max().unwrap();
            let access_len = fields.iter().map(|f| access::name(f.access).len()).max().unwrap();

            self.g.tidy_break();
            self.g.inc_indent();

            let last = fields.len() - 1;

            for (i, field) in fields.iter().enumerate() {
                self.g.indent();
                self.pad(access::name(field.access), access_len);
                self.g.space();
                self.pad(field.ty().name(), type_len);
                self.g.space();
                self.g.push(&field.name);

                if values && field.is_initializable() {
                    self.g.space();
                    self.value_to_vrml(&field.value);
                }

                if i == last {
                    self.g.tidy_break();
                } else {
                    self.g.brk();
                }
            }

            self.g.dec_indent();
            self.g.indent();
        }

        self.g.push("]");
    }

    /// `generator .string += text .padEnd (n, generator .TidySpace ())`.
    fn pad(&mut self, text: &str, n: usize) {
        self.g.tidy_space_lost();
        self.g.push(&format!("{text:<n$}"));
    }

    /// X3DExternProtoDeclaration.toVRMLStream.
    fn extern_proto_to_vrml(&mut self, e: ExternId) {
        let proto = &self.scene.externprotos[e];

        self.interface_header("EXTERNPROTO", &proto.name, &proto.fields, false);
        self.g.tidy_break();
        self.g.indent();
        FieldValue::MFString(proto.url.clone()).to_vrml_stream(&mut self.g);
    }

    /// X3DProtoDeclaration.toVRMLStream.
    fn proto_to_vrml(&mut self, p: ProtoId) {
        let proto = &self.scene.protos[p];

        self.interface_header("PROTO", &proto.name, &proto.fields, true);
        self.g.tidy_break();
        self.g.indent();
        self.g.push("{");
        self.g.tidy_break();
        self.g.inc_indent();
        self.context_to_vrml(proto.body);
        self.g.dec_indent();
        self.g.indent();
        self.g.push("}");
    }

    // --- nodes -------------------------------------------------------------------------------

    /// SFNode / MFNode / other values.
    fn value_to_vrml(&mut self, value: &FieldValue) {
        match value {
            FieldValue::SFNode(n) => self.sfnode_to_vrml(*n),
            FieldValue::MFNode(nodes) => match nodes.len() {
                0 => {
                    self.g.push("[");
                    self.g.tidy_space();
                    self.g.push("]");
                }
                1 => self.sfnode_to_vrml(nodes[0]),
                _ => {
                    self.g.push("[");
                    self.g.tidy_break();
                    self.g.inc_indent();

                    for n in nodes {
                        self.g.indent();
                        self.sfnode_to_vrml(*n);
                        self.g.tidy_break();
                    }

                    self.g.dec_indent();
                    self.g.indent();
                    self.g.push("]");
                }
            },
            _ => value.to_vrml_stream(&mut self.g),
        }
    }

    fn sfnode_to_vrml(&mut self, node: Option<NodeId>) {
        match node {
            Some(n) => self.node_to_vrml(n),
            None => self.g.null(),
        }
    }

    /// X3DBaseNode.getChangedFields (extended = false).
    fn changed_fields(&self, id: NodeId) -> Vec<usize> {
        let node = self.scene.node(id);
        let mut out = vec![];

        for (i, field) in node.fields.iter().enumerate() {
            if field.references.is_empty() {
                if !field.is_initializable() {
                    continue;
                }

                if self.scene.is_default_value(id, FieldLoc::Pre(i)) {
                    continue;
                }
            }

            out.push(i);
        }

        out
    }

    /// X3DNode.toVRMLStream.
    pub fn node_to_vrml(&mut self, id: NodeId) {
        let scene = self.scene;

        self.g.check_space();

        let name = self.name(id);

        if !name.is_empty() {
            if self.cur_index().nodes.contains(&id) {
                self.g.push("USE");
                self.g.space();
                self.g.push(&name);
                self.g.set_needs_space();
                return;
            }

            let index = self.cur_index();
            index.nodes.insert(id);
            index.route_nodes.insert(id);

            self.g.push("DEF");
            self.g.space();
            self.g.push(&name);
            self.g.space();
        }

        self.g.push(&scene.type_name(id));
        self.g.tidy_space();
        self.g.push("{");

        let fields = self.changed_fields(id);
        let node = scene.node(id);
        let user_fields = &node.user_fields;

        if node.can_user_defined_fields && !user_fields.is_empty() {
            let type_len = user_fields.iter().map(|f| f.ty().name().len()).max().unwrap();
            let access_len = user_fields.iter().map(|f| access::name(f.access).len()).max().unwrap();

            self.g.tidy_break();
            self.g.inc_indent();

            let last = user_fields.len() - 1;

            for (i, field) in user_fields.iter().enumerate() {
                self.user_defined_field_to_vrml(id, field, type_len, access_len);

                if i == last {
                    self.g.tidy_break();
                } else {
                    self.g.brk();
                }
            }

            self.g.dec_indent();

            if !fields.is_empty() {
                self.g.tidy_break();
            }
        }

        if fields.is_empty() {
            if !user_fields.is_empty() {
                self.g.indent();
            } else {
                self.g.tidy_space();
            }
        } else {
            if user_fields.is_empty() {
                self.g.tidy_break();
            }

            self.g.inc_indent();

            let last = fields.len() - 1;

            for (k, &i) in fields.iter().enumerate() {
                self.field_to_vrml(id, i);

                if k == last {
                    self.g.tidy_break();
                } else {
                    self.g.brk();
                }
            }

            self.g.dec_indent();
            self.g.indent();
        }

        self.g.push("}");
    }

    fn reference_name(&self, r: &FieldRef) -> &'a str {
        &self.scene.protos[r.proto].fields[r.index].name
    }

    fn reference_initializable(&self, r: &FieldRef) -> bool {
        self.scene.protos[r.proto].fields[r.index].is_initializable()
    }

    /// X3DNode.toVRMLStreamUserDefinedField.
    fn user_defined_field_to_vrml(&mut self, id: NodeId, field: &Field, type_len: usize, access_len: usize) {
        let shared = self.is_shared_node(id);

        if field.references.is_empty() || self.current().is_none() || shared {
            self.g.indent();
            self.pad(access::name(field.access), access_len);
            self.g.space();
            self.pad(field.ty().name(), type_len);
            self.g.space();
            self.g.push(&field.name);

            if field.is_initializable() {
                self.g.space();
                self.value_to_vrml(&field.value);
            }
        } else {
            let mut initializable_references = false;
            let count = field.references.len();

            for (index, r) in field.references.iter().enumerate() {
                initializable_references |= self.reference_initializable(r);

                self.g.indent();
                self.pad(access::name(field.access), access_len);
                self.g.space();
                self.pad(field.ty().name(), type_len);
                self.g.space();
                self.g.push(&field.name);
                self.g.space();
                self.g.push("IS");
                self.g.space();
                let rn = self.reference_name(r);
                self.g.push(rn);

                if index + 1 != count {
                    self.g.brk();
                }
            }

            if field.access == access::INPUT_OUTPUT && !initializable_references && !field.value.is_default_value() {
                self.g.brk();
                self.g.indent();
                self.pad(access::name(field.access), access_len);
                self.g.space();
                self.pad(field.ty().name(), type_len);
                self.g.space();
                self.g.push(&field.name);

                if field.is_initializable() {
                    self.g.space();
                    self.value_to_vrml(&field.value);
                }
            }
        }
    }

    /// X3DNode.toVRMLStreamField.
    fn field_to_vrml(&mut self, id: NodeId, i: usize) {
        let scene = self.scene;
        let field = &scene.node(id).fields[i];
        let shared = self.is_shared_node(id);

        if field.references.is_empty() || self.current().is_none() || shared {
            if field.is_initializable() {
                self.g.indent();
                self.g.push(&field.name);
                self.g.space();
                self.field_value_to_vrml(id, &field.name, &field.value);
            }
        } else {
            let mut initializable_references = false;
            let count = field.references.len();

            for (index, r) in field.references.iter().enumerate() {
                initializable_references |= self.reference_initializable(r);

                self.g.indent();
                self.g.push(&field.name);
                self.g.space();
                self.g.push("IS");
                self.g.space();
                let rn = self.reference_name(r);
                self.g.push(rn);

                if index + 1 != count {
                    self.g.brk();
                }
            }

            if field.access == access::INPUT_OUTPUT && !initializable_references && !scene.is_default_value(id, FieldLoc::Pre(i)) {
                self.g.brk();
                self.g.indent();
                self.g.push(&field.name);
                self.g.space();
                self.field_value_to_vrml(id, &field.name, &field.value);
            }
        }
    }

    /// X3DBaseNode.getConvertedFields → OrthoViewpoint.convertFields: for specification versions up to 4.0
    /// the field is an MFFloat, but it is written as an SFVec4f (`new Fields .SFVec4f (... this ._fieldOfView)`).
    fn field_value_to_vrml(&mut self, id: NodeId, name: &str, value: &FieldValue) {
        if let FieldValue::MFFloat(v) = value {
            if name == "fieldOfView" && v.len() == 4 && self.scene.type_name(id) == "OrthoViewpoint" {
                let v4 = x3d_fields::math::Vector4::new(v[0] as f64, v[1] as f64, v[2] as f64, v[3] as f64);
                FieldValue::SFVec4f(v4).to_vrml_stream(&mut self.g);
                return;
            }
        }

        self.value_to_vrml(value);
    }
}

fn is_underscore_number(name: &str) -> bool {
    name.len() > 1 && name.starts_with('_') && name[1..].bytes().all(|b| b.is_ascii_digit())
}

impl Scene {
    /// `scene .toVRMLString ()` with X_ITE's default (TIDY) generator options.
    pub fn to_vrml_string(&self) -> String {
        VrmlGenerator::new(self).scene_to_vrml()
    }
}

impl Scene {
    /// `node .toString ()`: the node printed in its own execution context.
    pub fn node_to_vrml_string(&self, node: NodeId) -> String {
        let mut g = VrmlGenerator::new(self);
        let ctx = self.node(node).ctx;

        g.push_ctx(Some(ctx));
        g.node_to_vrml(node);
        g.pop_ctx();
        g.g.string
    }

    /// Remove a route (X3DExecutionContext.deleteRoute); the removed route, if any.
    pub fn delete_route(&mut self, ctx: CtxId, source: NodeId, source_field: &str, destination: NodeId, destination_field: &str) -> Option<Route> {
        let src = self.get_field(source, source_field)?;
        let dst = self.get_field(destination, destination_field)?;
        let i = self.contexts[ctx].routes.iter().position(|r| {
            r.source_node == source && r.source_loc == src && r.destination_node == destination && r.destination_loc == dst
        })?;
        let r = self.contexts[ctx].routes.remove(i);

        self.field_mut(source, src).output_routes -= 1;
        self.field_mut(destination, dst).input_routes -= 1;
        Some(r)
    }
}
