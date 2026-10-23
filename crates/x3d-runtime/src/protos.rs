//! PROTO instancing: X3DPrototypeInstance.initialize / realize and X3DNode.copy (instance),
//! SFNode.copy / MFNode.copy (instance), X3DField.addReference (IS relations).

use crate::*;

impl Runtime {
    /// The X3DProtoDeclaration an instance realizes, if available (an EXTERNPROTO only once loaded).
    pub fn instance_proto(&self, inst: NodeId) -> Option<ProtoId> {
        match self.scene.node(inst).kind {
            NodeKind::Instance(ProtoRef::Proto(p)) => Some(p),
            NodeKind::Instance(ProtoRef::Extern(e)) => {
                self.extern_loaded.get(&e).copied().flatten()
            }
            _ => None,
        }
    }

    /// Body execution context of a realized instance (the current one: a replaced body is gone).
    pub fn instance_body(&self, inst: NodeId) -> Option<CtxId> {
        self.instance_bodies.get(&inst).copied()
    }

    /// The type names of the root nodes of an instance's body (`None` without a body).
    pub fn instance_body_roots(&self, inst: NodeId) -> Option<Vec<Option<String>>> {
        let body = self.instance_body(inst)?;

        Some(
            self.scene
                .ctx(body)
                .root_nodes
                .iter()
                .map(|n| n.map(|n| self.type_name(n)))
                .collect(),
        )
    }

    /// X3DPrototypeInstance.initialize.
    pub(crate) fn instance_initialize(&mut self, inst: NodeId) {
        self.realize(inst);
    }

    /// X3DPrototypeInstance.realize.
    pub fn realize(&mut self, inst: NodeId) {
        let Some(p) = self.instance_proto(inst) else {
            return;
        };

        // X3DPrototypeInstance.realize: an extern proto that is not COMPLETE has no body.
        if let NodeKind::Instance(ProtoRef::Extern(e)) = self.scene.node(inst).kind {
            if self.extern_load_state(e) != nodes::LoadState::Complete {
                return;
            }
        }

        let decl_body = self.scene.protos[p].body;
        let parent = self.scene.protos[p].ctx;
        let body = self.scene.new_context(parent, Outer::Instance(inst));

        self.instance_bodies.insert(inst, body);

        // addExternProtos / addProtos
        let externprotos: Vec<(String, ExternId)> = self
            .scene
            .ctx(decl_body)
            .externprotos
            .values()
            .iter()
            .map(|e| (self.scene.externprotos[*e].name.clone(), *e))
            .collect();
        for (name, e) in externprotos {
            let _ = self.scene.ctx_mut(body).externprotos.add(&name, e);
        }

        let protos: Vec<(String, ProtoId)> = self
            .scene
            .ctx(decl_body)
            .protos
            .values()
            .iter()
            .map(|q| (self.scene.protos[*q].name.clone(), *q))
            .collect();
        for (name, q) in protos {
            let _ = self.scene.ctx_mut(body).protos.add(&name, q);
        }

        // copyNamedNodes
        let named: Vec<NodeId> = self.scene.ctx(decl_body).named_nodes.values().to_vec();
        for n in named {
            self.copy_node(n, inst, body);
        }

        // copyRootNodes (SFNode.copy: set up the copy)
        let roots = self.scene.ctx(decl_body).root_nodes.clone();
        for r in roots {
            let c = r.map(|n| self.copy_sfnode(n, inst, body));

            self.scene.ctx_mut(body).root_nodes.push(c);

            if let Some(c) = c {
                self.add_parent_pub(c, Holder::Root(body));
            }
        }

        // copyRoutes
        let routes = self.scene.ctx(decl_body).routes.clone();
        for r in routes {
            let src_name = self.scene.node(r.source_node).name.clone();
            let dst_name = self.scene.node(r.destination_node).name.clone();
            let src = self.scene.get_named_node(body, &src_name);
            let dst = self.scene.get_named_node(body, &dst_name);

            match (src, dst) {
                (Some(s), Some(d)) => {
                    if let Err(e) =
                        self.add_route(body, s, &r.source_field, d, &r.destination_field)
                    {
                        self.log_error(format!("Error: {e}"));
                    }
                }
                (None, _) => self.log_error(format!(
                    "Error: Unknown named or imported node '{src_name}'."
                )),
                (_, None) => self.log_error(format!(
                    "Error: Unknown named or imported node '{dst_name}'."
                )),
            }
        }

        // X3DChildObject .prototype .addEvent .call (this)
        self.add_node_parents_event(inst);
    }

    pub(crate) fn add_parent_pub(&mut self, node: NodeId, holder: Holder) {
        let list = self.parents.entry(node).or_default();

        if !list.contains(&holder) {
            list.push(holder);
        }
    }

    /// X3DExecutionContext.addRoute + X3DRoute.connect: the route becomes a field interest.
    pub fn add_route(
        &mut self,
        ctx: CtxId,
        source: NodeId,
        source_field: &str,
        destination: NodeId,
        destination_field: &str,
    ) -> Result<(), String> {
        if let Some(index) =
            self.scene
                .add_route(ctx, source, source_field, destination, destination_field)?
        {
            let r = self.scene.ctx(ctx).routes[index].clone();
            self.activate_route(&r);
        }

        Ok(())
    }

    /// Connect the routes the parser created (all contexts except PROTO declaration bodies).
    pub fn connect_parsed_routes(&mut self) {
        for c in 0..self.scene.contexts.len() {
            if matches!(self.scene.contexts[c].outer, Outer::Proto(_)) {
                continue;
            }

            for r in self.scene.contexts[c].routes.clone() {
                self.activate_route(&r);
            }
        }
    }

    /// Is `ctx` (or an ancestor body) inside a PROTO declaration?
    fn inside_proto_declaration(&self, ctx: CtxId) -> bool {
        matches!(self.scene.ctx(ctx).outer, Outer::Proto(_))
    }

    /// SFNode.copy (instance): copy, then set up unless the instance lives in a PROTO declaration.
    pub fn copy_sfnode(&mut self, node: NodeId, inst: NodeId, body: CtxId) -> NodeId {
        let copy = self.copy_node(node, inst, body);
        let inst_ctx = self.scene.node(inst).ctx;

        if !self.is_initialized(copy) && !self.inside_proto_declaration(inst_ctx) {
            self.setup_node(copy);
        }

        copy
    }

    fn copy_value(&mut self, value: &FieldValue, inst: NodeId, body: CtxId) -> FieldValue {
        match value {
            FieldValue::SFNode(Some(n)) => {
                FieldValue::SFNode(Some(self.copy_sfnode(*n, inst, body)))
            }
            FieldValue::MFNode(list) => FieldValue::MFNode(
                list.iter()
                    .map(|n| n.map(|n| self.copy_sfnode(n, inst, body)))
                    .collect(),
            ),
            v => v.clone(),
        }
    }

    /// Create a fresh node of the same class in `ctx` (X3DNode.create).
    pub fn create_like(&mut self, src: NodeId, ctx: CtxId) -> NodeId {
        let n = match self.scene.node(src).kind {
            NodeKind::Builtin(t) => self.scene.instantiate_builtin(ctx, t),
            NodeKind::Instance(p) => self.scene.instantiate_proto(ctx, p),
            _ => self.scene.create_placeholder(ctx),
        };

        self.nrt_mut(n);
        self.construct_node(n);

        // Interface default values of an instance hold their SFNode values (field copy).
        let values: Vec<(usize, FieldValue)> = self
            .scene
            .node(n)
            .fields
            .iter()
            .map(|f| f.value.clone())
            .enumerate()
            .collect();
        for (i, v) in values {
            for m in Self::node_values_pub(&v) {
                self.add_parent_pub(m, Holder::Field((n, FieldLoc::Pre(i))));
            }
        }

        n
    }

    pub fn node_values_pub(value: &FieldValue) -> Vec<NodeId> {
        match value {
            FieldValue::SFNode(Some(n)) => vec![*n],
            FieldValue::MFNode(list) => list.iter().flatten().copied().collect(),
            _ => vec![],
        }
    }

    /// X3DNode.getNeedsName.
    fn needs_name(&self, node: NodeId) -> bool {
        if !self.scene.node(node).name.is_empty() {
            return false;
        }

        self.clone_count(node) > 1 || self.scene.has_routes(node)
    }

    fn reference_initializable(&self, r: &FieldRef) -> bool {
        self.scene.protos[r.proto].fields[r.index].is_initializable()
    }

    /// X3DNode.copy (instance) for an X3DPrototypeInstance body.
    pub fn copy_node(&mut self, src: NodeId, inst: NodeId, body: CtxId) -> NodeId {
        let name = self.scene.node(src).name.clone();

        if !name.is_empty() {
            if let Some(existing) = self.scene.get_named_node(body, &name) {
                return existing;
            }
        }

        if matches!(
            self.scene.node(src).kind,
            NodeKind::Placeholder | NodeKind::Internal(_)
        ) {
            return src;
        }

        let copy = self.create_like(src, body);

        // Support editing of protos.
        if self.needs_name(src) {
            let ctx = self.scene.node(src).ctx;
            let unique = self.scene.get_unique_name(ctx, "");
            self.scene.update_named_node(ctx, &unique, src);
        }

        let name = self.scene.node(src).name.clone();

        if !name.is_empty() {
            self.scene.update_named_node(body, &name, copy);
        }

        // Default fields
        let src_fields = self.scene.node(src).fields.clone();

        for sf in &src_fields {
            let Some(di) = self.scene.get_predefined_field(copy, &sf.name) else {
                self.log_error(format!(
                    "Error: Unknown predefined field '{}' in node class {}.",
                    sf.name,
                    self.type_name(copy)
                ));
                continue;
            };
            let dk = (copy, FieldLoc::Pre(di));
            let init_refs = sf
                .references
                .iter()
                .any(|r| self.reference_initializable(r));

            if sf.is_initializable() && !init_refs {
                let v = self.copy_value(&sf.value, inst, body);
                self.set_raw(dk, v);
            }

            for r in &sf.references {
                let rname = self.scene.protos[r.proto].fields[r.index].name.clone();

                match self.get_field(inst, &rname) {
                    Some(rk) => self.add_reference(dk, rk),
                    None => {
                        let t = self.type_name(inst);
                        self.log_error(format!("Error: Unknown field '{rname}' in node class {t}."))
                    }
                }
            }

            self.scene.field_mut(copy, FieldLoc::Pre(di)).modified = sf.modified;
        }

        // User-defined fields
        let src_user = self.scene.node(src).user_fields.clone();

        for sf in &src_user {
            let init_refs = sf
                .references
                .iter()
                .any(|r| self.reference_initializable(r));
            let value = if init_refs {
                FieldValue::default_for(sf.ty())
            } else {
                self.copy_value(&sf.value, inst, body)
            };
            let mut field = Field::new(&sf.name, sf.access, FieldValue::default_for(sf.ty()));
            field.modified = sf.modified;

            let ui = self.scene.add_user_defined_field(copy, field);
            let dk = (copy, FieldLoc::User(ui));
            self.set_raw(dk, value);

            for r in &sf.references {
                let rname = self.scene.protos[r.proto].fields[r.index].name.clone();

                match self.get_field(inst, &rname) {
                    Some(rk) => self.add_reference(dk, rk),
                    None => {
                        let (t, n) = (self.type_name(inst), self.scene.node(inst).name.clone());
                        self.log_error(format!(
                            "No reference '{rname}' inside execution context {t} '{n}'."
                        ))
                    }
                }
            }

            self.scene.field_mut(copy, FieldLoc::User(ui)).modified = sf.modified;
        }

        copy
    }

    /// X3DField.addReference (reference): create the IS relationship.
    pub fn add_reference(&mut self, k: FKey, reference: FKey) {
        if self
            .frt
            .get(&k)
            .is_some_and(|f| f.references.contains(&reference))
        {
            return;
        }

        self.frt_mut(k).references.push(reference);

        let from = Event {
            origin: reference,
            visited: vec![reference],
        };

        match self.field(k).access & self.field(reference).access {
            access::INITIALIZE_ONLY => {
                self.add_field_interest(reference, k);
                self.process_event(k, from);
            }
            access::INPUT_ONLY => self.add_field_interest(reference, k),
            access::OUTPUT_ONLY => self.add_field_interest(k, reference),
            access::INPUT_OUTPUT => {
                self.add_field_interest(reference, k);
                self.add_field_interest(k, reference);
                self.process_event(k, from);
            }
            _ => {}
        }
    }
}
