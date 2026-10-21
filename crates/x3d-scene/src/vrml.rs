//! `scene .toVRMLString ()`: X3DScene.toVRMLStream, X3DExecutionContext.toVRMLStream,
//! X3DNode.toVRMLStream, X3DProtoDeclaration / X3DExternProtoDeclaration / X3DRoute
//! toVRMLStream, and the name bookkeeping of X3DGenerator.js.

use crate::scene::*;
use std::collections::{HashMap, HashSet};
use x3d_fields::{FieldValue, Generator, NodeId, access};

#[derive(Default)]
pub(crate) struct CtxIndex {
    pub(crate) names: HashSet<String>,
    pub(crate) names_by_node: HashMap<NodeId, String>,
    pub(crate) nodes: HashSet<NodeId>,
    pub(crate) route_nodes: HashSet<NodeId>,
    /// X3DGenerator [importedNames]: the printed name of an imported node instance.
    pub(crate) imported_names: HashMap<NodeId, String>,
}

pub struct VrmlGenerator<'a> {
    pub g: Generator,
    pub(crate) scene: &'a Scene,
    /// `None` is the `null` execution context pushed by X3DObject.toVRMLString.
    stack: Vec<Option<CtxId>>,
    index: HashMap<CtxId, CtxIndex>,
    /// The predefined field whose value prints next (for its unit category).
    pub(crate) unit_field: Option<(NodeId, String)>,
    /// The field whose value prints next, for its raw components (Scene::raw_tokens).
    pub(crate) raw_field: Option<(NodeId, FieldLoc)>,
    /// Contexts on the stack at or below this depth only name nodes: a node's or a field's own `toXXXString` pushes a `null`
    /// execution context in X_ITE, so X3DGenerator.ToUnit leaves every value as it is.
    pub(crate) unit_base: usize,
}

impl<'a> VrmlGenerator<'a> {
    pub fn new(scene: &'a Scene) -> Self {
        VrmlGenerator { g: Generator::default(), scene, stack: vec![], index: HashMap::new(), unit_field: None, raw_field: None, unit_base: 0 }
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

    pub(crate) fn current(&self) -> Option<CtxId> {
        self.stack.last().copied().flatten()
    }

    pub(crate) fn cur_index(&mut self) -> &mut CtxIndex {
        let c = self.current().expect("execution context");
        self.index.get_mut(&c).unwrap()
    }

    /// X3DGenerator.NamedNodes.
    pub(crate) fn named_nodes(&mut self, ctx: CtxId) {
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
    pub(crate) fn name(&mut self, node: NodeId) -> String {
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
    pub(crate) fn needs_name(&self, node: NodeId) -> bool {
        if self.scene.clone_counts.as_ref().is_some_and(|c| c.get(&node).is_some_and(|n| *n > 1)) {
            return true;
        }

        if self.scene.has_routes(node) {
            return true;
        }

        // An Inline that an IMPORT refers to, and an exported node.
        let Some(ctx) = self.current() else {
            return false;
        };
        let c = self.scene.ctx(ctx);

        c.imported.values().iter().any(|&i| self.scene.imported[i].inline == node) || c.exported.values().iter().any(|&e| self.scene.exported[e].local == node)
    }

    /// X3DGenerator.ImportedName.
    pub(crate) fn imported_name(&mut self, instance: NodeId) -> String {
        if let Some(n) = self.cur_index().imported_names.get(&instance) {
            return n.clone();
        }

        let name = self.scene.node(instance).name.clone();
        let index = self.cur_index();
        let new_name = crate::info_array::get_unique_name(|n| index.names.contains(n), &name);

        index.names.insert(new_name.clone());
        index.imported_names.insert(instance, new_name.clone());
        new_name
    }

    /// X3DImportedNode.toVRMLStream; `false` where X_ITE throws (the array printer skips it).
    fn imported_to_vrml(&mut self, id: ImportedId) -> bool {
        let scene = self.scene;
        let imported = &scene.imported[id];

        if !self.cur_index().nodes.contains(&imported.inline) {
            return false;
        }

        self.cur_index().route_nodes.insert(imported.instance);

        let imported_name = self.imported_name(imported.instance);
        let inline_name = self.name(imported.inline);
        let g = &mut self.g;

        g.indent();
        g.push("IMPORT");
        g.space();
        g.push(&inline_name);
        g.push(".");
        g.push(&imported.exported_name);

        if imported_name != imported.exported_name {
            g.space();
            g.push("AS");
            g.space();
            g.push(&imported_name);
        }

        if !imported.description.is_empty() {
            g.space();
            g.push("DESCRIPTION");
            g.space();
            g.push("\"");
            g.push(&imported.description);
            g.push("\"");
        }

        true
    }

    /// X3DExportedNode.toVRMLStream.
    fn exported_to_vrml(&mut self, id: ExportedId) {
        let scene = self.scene;
        let exported = &scene.exported[id];
        let local_name = self.name(exported.local);
        let g = &mut self.g;

        g.indent();
        g.push("EXPORT");
        g.space();
        g.push(&local_name);

        if exported.exported_name != local_name {
            g.space();
            g.push("AS");
            g.space();
            g.push(&exported.exported_name);
        }

        if !exported.description.is_empty() {
            g.space();
            g.push("DESCRIPTION");
            g.space();
            g.push("\"");
            g.push(&exported.description);
            g.push("\"");
        }
    }

    pub(crate) fn is_shared_node(&self, node: NodeId) -> bool {
        self.current() != Some(self.scene.node(node).ctx)
    }

    // --- scene -------------------------------------------------------------------------------

    pub fn scene_to_vrml(self) -> String {
        self.scene_to_vrml_ctx(ROOT)
    }

    /// X3DScene.toVRMLStream of the scene whose root context is `root` (the world's, or an
    /// Inline's merged scene).
    pub fn scene_to_vrml_ctx(mut self, root: CtxId) -> String {
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

        if let Some(profile) = scene.profile_of(root) {
            g.indent();
            g.push("PROFILE");
            g.space();
            g.push(profile);
            g.brk();
            g.tidy_break();
        }

        if !scene.components_of(root).is_empty() {
            for (name, level) in scene.components_of(root) {
                g.indent();
                g.push("COMPONENT");
                g.space();
                g.push(name);
                g.tidy_space();
                g.push(":");
                g.tidy_space();
                g.push(&scene.component_level_text(root, name, *level));
                g.brk();
            }

            g.tidy_break();
        }

        // UnitInfo.toVRMLStream for the units whose conversion factor is not 1.
        let units: Vec<_> = scene.units_of(root).iter().filter(|u| u.conversion_factor != 1.0).collect();

        if !units.is_empty() {
            for u in units {
                g.indent();
                g.push("UNIT");
                g.space();
                g.push(&u.category);
                g.space();
                g.push(&u.name);
                g.space();
                g.push(&x3d_fields::jsmath::number_to_string(u.conversion_factor));
                g.brk();
            }

            g.tidy_break();
        }

        if !scene.meta_of(root).is_empty() {
            for (key, value) in scene.meta_of(root) {
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

        self.push_ctx(Some(root));
        self.context_to_vrml(root);

        if !scene.ctx(root).exported.is_empty() {
            self.g.tidy_break();

            for &e in scene.ctx(root).exported.values() {
                self.exported_to_vrml(e);
                self.g.brk();
            }
        }

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

        let imported = &scene.ctx(ctx).imported;

        if !imported.is_empty() {
            self.g.tidy_break();

            for &i in imported.values() {
                if self.imported_to_vrml(i) {
                    self.g.brk();
                }
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

        let source = if scene_is_imported(self.scene, route.source_node) { self.imported_name(route.source_node) } else { self.name(route.source_node) };
        let destination =
            if scene_is_imported(self.scene, route.destination_node) { self.imported_name(route.destination_node) } else { self.name(route.destination_node) };
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
            _ => {
                let converted = match self.take_raw(value) {
                    Some(RawOut::Texts(texts)) => return write_raw(&mut self.g, &texts, value, false),
                    Some(RawOut::Value(v)) => Some(v),
                    None => None,
                };

                self.apply_units();
                converted.as_ref().unwrap_or(value).to_vrml_stream(&mut self.g);
                self.g.set_unit_factors(1.0, 1.0);
            }
        }
    }

    /// Does X3DGenerator.ToUnit convert here (an execution context that is not only the naming frame of a standalone print)?
    pub(crate) fn units_apply(&self) -> bool {
        self.stack.len() > self.unit_base && self.current().is_some()
    }

    /// The raw component texts of the value printing next (`raw_field`, taken), when ToUnit leaves the components as they are:
    /// no execution context, or a field without a unit category. X3DGenerator.FloatFormat then writes `String (value)`.
    pub(crate) fn take_raw(&mut self, value: &FieldValue) -> Option<RawOut> {
        let (id, loc) = self.raw_field.take()?;
        let raw = self.scene.raw_tokens.get(&(id, loc))?;
        let unit = match loc {
            FieldLoc::Pre(i) => self.scene.field_has_unit(id, &self.scene.node(id).fields[i].name),
            _ => false,
        };
        let number = unit && self.units_apply();
        let supplied = match &mut *self.scene.raw_print.borrow_mut() {
            RawPrint::Log(log) => {
                log.push(RawRequest { host: raw.host, token: raw.token, number });
                None
            }
            RawPrint::Supply(queue) => queue.pop_front().flatten(),
            RawPrint::Off => None,
        };

        if number {
            // ToUnit divides each component: a raw one is its ToNumber now.
            return supplied.map(|components| RawOut::Value(with_numbers(value, &components)));
        }

        self.unit_field = None;

        let texts = match supplied {
            Some(components) => components.into_iter().map(|c| if let Some(RawComponent::Text(t)) = c { Some(t) } else { None }).collect(),
            None => raw.texts.to_vec(),
        };

        Some(RawOut::Texts(texts))
    }

    /// Generator.ToUnit for the next value: `this .ExecutionContext () ?.toUnit (category, value) ?? value`,
    /// with the category of `unit_field` (taken) and the angle unit.
    pub(crate) fn apply_units(&mut self) {
        let unit_field = self.unit_field.take();
        let (unit, angle) = match self.current() {
            Some(ctx) if self.stack.len() > self.unit_base => match &unit_field {
                Some((n, name)) => self.scene.unit_factors(ctx, Some(*n), name),
                None => self.scene.unit_factors(ctx, None, ""),
            },
            _ => (1.0, 1.0),
        };

        self.g.set_unit_factors(unit, angle);
    }

    fn sfnode_to_vrml(&mut self, node: Option<NodeId>) {
        match node {
            Some(n) => self.node_to_vrml(n),
            None => self.g.null(),
        }
    }

    /// X3DBaseNode.getChangedFields (extended = false).
    pub(crate) fn changed_fields(&self, id: NodeId) -> Vec<usize> {
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

        // X3DImportedNodeInstance.toVRMLStream: `USE importedName`.
        if scene.is_imported(id) {
            self.g.check_space();
            let name = self.imported_name(id);
            self.g.push("USE");
            self.g.space();
            self.g.push(&name);
            self.g.set_needs_space();
            return;
        }

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

    pub(crate) fn reference_name(&self, r: &FieldRef) -> &'a str {
        &self.scene.protos[r.proto].fields[r.index].name
    }

    pub(crate) fn reference_initializable(&self, r: &FieldRef) -> bool {
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

                self.raw_field = self.scene.node(id).user_fields.iter().position(|f| std::ptr::eq(f, field)).map(|k| (id, FieldLoc::User(k)));
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
                self.raw_field = Some((id, FieldLoc::Pre(i)));
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
                self.raw_field = Some((id, FieldLoc::Pre(i)));
                self.field_value_to_vrml(id, &field.name, &field.value);
            }
        }
    }

    /// X3DBaseNode.getConvertedFields → OrthoViewpoint.convertFields: the MFFloat is written as an SFVec4f
    /// (`new Fields .SFVec4f (... this ._fieldOfView)`, whatever its length; see [`crate::xml::field_of_view_vec4`]).
    fn field_value_to_vrml(&mut self, id: NodeId, name: &str, value: &FieldValue) {
        if let FieldValue::MFFloat(v) = value {
            if name == "fieldOfView" && self.scene.type_name(id) == "OrthoViewpoint" {
                FieldValue::SFVec4f(crate::xml::field_of_view_vec4(v)).to_vrml_stream(&mut self.g);
                return;
            }
        }

        if !matches!(value, FieldValue::SFNode(_) | FieldValue::MFNode(_)) {
            self.unit_field = Some((id, name.to_string()));
        }

        self.value_to_vrml(value);
    }
}

fn scene_is_imported(scene: &Scene, node: NodeId) -> bool {
    scene.is_imported(node)
}

fn is_underscore_number(name: &str) -> bool {
    name.len() > 1 && name.starts_with('_') && name[1..].bytes().all(|b| b.is_ascii_digit())
}

impl Scene {
    /// `scene .toVRMLString ()` with X_ITE's default (TIDY) generator options.
    pub fn to_vrml_string(&self) -> String {
        if self.world != ROOT {
            return self.to_vrml_string_ctx(self.world);
        }

        VrmlGenerator::new(self).scene_to_vrml()
    }

    /// `toVRMLString ()` of the scene whose root context is `root` (an Inline's scene).
    pub fn to_vrml_string_ctx(&self, root: CtxId) -> String {
        VrmlGenerator::new(self).scene_to_vrml_ctx(root)
    }
}

impl Scene {
    /// `node .toString ()`: the node printed in its own execution context.
    pub fn node_to_vrml_string(&self, node: NodeId) -> String {
        let mut g = VrmlGenerator::new(self);
        let ctx = self.node(node).ctx;

        g.push_ctx(Some(ctx));
        g.unit_base = 1;
        g.node_to_vrml(node);
        g.pop_ctx();
        g.g.string
    }

    /// The execution context of the first node a node-valued field holds (what a field's `toXXXString` prints names against).
    pub(crate) fn first_node_ctx(&self, value: &FieldValue) -> Option<CtxId> {
        let first = match value {
            FieldValue::SFNode(n) => *n,
            FieldValue::MFNode(list) => list.iter().flatten().next().copied(),
            _ => None,
        };

        first.map(|n| self.node(n).ctx)
    }

    /// `field .toVRMLString ()` of a node-valued field: the nodes in full, named by their own execution context.
    pub fn node_value_to_vrml_string(&self, value: &FieldValue) -> String {
        let mut g = VrmlGenerator::new(self);

        g.push_ctx(self.first_node_ctx(value));
        g.unit_base = 1;
        g.value_to_vrml(value);
        g.pop_ctx();
        g.g.string
    }

    /// `field .toString ()` of a node-valued field: X3DNode.toStream prints only `TypeName { }`.
    pub fn node_value_to_string(&self, value: &FieldValue) -> String {
        let one = |n: &Option<NodeId>| match n {
            Some(n) => format!("{} {{ }}", self.type_name(*n)),
            None => "NULL".to_string(),
        };

        match value {
            FieldValue::SFNode(n) => one(n),
            FieldValue::MFNode(list) => match list.len() {
                0 => "[ ]".to_string(),
                1 => one(&list[0]),
                _ => format!("[\n{}\n]", list.iter().map(|n| format!("  {}", one(n))).collect::<Vec<_>>().join("\n")),
            },
            _ => String::new(),
        }
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


/// SFVecPrototypeTemplate / SFColor toStream (or toJSONStreamValue, `json`) of a value with raw components: `String (raw)` for each
/// one that is not a number (X3DGenerator.FloatFormat, JSONGenerator.Number), the format of the number for the others.
pub(crate) fn write_raw(g: &mut Generator, texts: &[Option<String>], value: &FieldValue, json: bool) {
    let (numbers, double): (Vec<f64>, bool) = match value {
        FieldValue::SFColor(c) => (c.components().to_vec(), false),
        FieldValue::SFColorRGBA(c) => (c.components().to_vec(), false),
        FieldValue::SFVec2f(v) => (v.to_array().to_vec(), false),
        FieldValue::SFVec3f(v) => (v.to_array().to_vec(), false),
        FieldValue::SFVec4f(v) => (v.to_array().to_vec(), false),
        FieldValue::SFVec2d(v) => (v.to_array().to_vec(), true),
        FieldValue::SFVec3d(v) => (v.to_array().to_vec(), true),
        FieldValue::SFVec4d(v) => (v.to_array().to_vec(), true),
        _ => unreachable!("raw components belong to vector and color values"),
    };

    let last = numbers.len() - 1;

    for (i, v) in numbers.iter().enumerate() {
        let text = match texts.get(i).cloned().flatten() {
            Some(t) => t,
            None if double => g.double_format(*v),
            None => g.float_format(*v),
        };

        g.push(&if json { crate::json::json_number(text) } else { text });

        if i != last {
            if json {
                g.push(",");
                g.tidy_space();
            } else {
                g.space();
            }
        }
    }
}

/// What [`VrmlGenerator::take_raw`] gives a printer: the texts of a value printed without unit conversion, or the value with
/// the ToNumber of its raw components (it is then printed as any value, through ToUnit).
pub(crate) enum RawOut {
    Texts(Vec<Option<String>>),
    Value(FieldValue),
}

/// `value` with the raw components that have a number replaced by it.
fn with_numbers(value: &FieldValue, components: &[Option<RawComponent>]) -> FieldValue {
    let n = |i: usize, x: f64| match components.get(i) {
        Some(Some(RawComponent::Number(v))) => *v,
        _ => x,
    };

    match value {
        FieldValue::SFVec2f(v) => FieldValue::SFVec2f(x3d_fields::math::Vector2::new(n(0, v.x), n(1, v.y))),
        FieldValue::SFVec2d(v) => FieldValue::SFVec2d(x3d_fields::math::Vector2::new(n(0, v.x), n(1, v.y))),
        FieldValue::SFVec3f(v) => FieldValue::SFVec3f(x3d_fields::math::Vector3::new(n(0, v.x), n(1, v.y), n(2, v.z))),
        FieldValue::SFVec3d(v) => FieldValue::SFVec3d(x3d_fields::math::Vector3::new(n(0, v.x), n(1, v.y), n(2, v.z))),
        FieldValue::SFVec4f(v) => FieldValue::SFVec4f(x3d_fields::math::Vector4::new(n(0, v.x), n(1, v.y), n(2, v.z), n(3, v.w))),
        FieldValue::SFVec4d(v) => FieldValue::SFVec4d(x3d_fields::math::Vector4::new(n(0, v.x), n(1, v.y), n(2, v.z), n(3, v.w))),
        other => other.clone(),
    }
}

impl Scene {
    /// `declaration .toVRMLString ()` / `toXMLString ()` / `toJSONString ()` of a PROTO (`extern_` false) or EXTERNPROTO
    /// declaration: X3DObject's generator with a `null` execution context, then the declaration's toXXXStream.
    pub fn proto_declaration_string(&self, extern_: bool, id: usize, format: &str) -> String {
        match format {
            "xml" => crate::xml::proto_declaration_xml(self, extern_, id),
            "json" => crate::json::proto_declaration_json(self, extern_, id),
            _ => {
                let mut g = VrmlGenerator::new(self);

                g.push_ctx(None);
                g.unit_base = 1;

                if extern_ {
                    g.extern_proto_to_vrml(id);
                } else {
                    g.proto_to_vrml(id);
                }

                g.pop_ctx();
                g.g.string
            }
        }
    }
}
