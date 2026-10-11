//! Minimal scene model: just enough of X_ITE's X3DScene / X3DExecutionContext / X3DBaseNode /
//! X3DProtoDeclaration / X3DExternProtoDeclaration / X3DRoute to hold what the VRML parser
//! builds and to reproduce `scene .toVRMLString ()`.
//!
//! Not modelled (phase X3+): proto instance bodies (X3DPrototypeInstance.realize), event
//! propagation, node `initialize ()` side effects, Script execution, loading.

use crate::scene::info_array::{InfoArray, get_unique_name};
use crate::scene::nodes::{NodeTable, table};
use crate::fields::{FieldType, FieldValue, NodeId, access};

pub type CtxId = usize;
pub type ProtoId = usize;
pub type ExternId = usize;

/// An IS reference from a field inside a proto body to a field of the proto's interface.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct FieldRef {
    pub proto: ProtoId,
    pub index: usize,
}

#[derive(Clone, Debug)]
pub struct Field {
    pub name: String,
    pub access: u8,
    pub value: FieldValue,
    /// IS references, in insertion order (X3DField.getReferences, a set).
    pub references: Vec<FieldRef>,
    /// Field was assigned during parsing (X_ITE: modification time >= 0).
    pub modified: bool,
    pub input_routes: usize,
    pub output_routes: usize,
}

impl Field {
    pub fn new(name: &str, access: u8, value: FieldValue) -> Self {
        Field { name: name.to_string(), access, value, references: vec![], modified: false, input_routes: 0, output_routes: 0 }
    }

    pub fn ty(&self) -> FieldType {
        self.value.field_type()
    }

    pub fn is_initializable(&self) -> bool {
        access::is_initializable(self.access)
    }

    /// X3DField.isReference (accessType).
    pub fn is_reference(&self, access_type: u8) -> bool {
        access_type == self.access || access_type == access::INPUT_OUTPUT
    }

    pub fn add_reference(&mut self, r: FieldRef) {
        if !self.references.contains(&r) {
            self.references.push(r);
        }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ProtoRef {
    Proto(ProtoId),
    Extern(ExternId),
}

#[derive(Clone, Debug, PartialEq)]
pub enum NodeKind {
    /// Index into the node table.
    Builtin(usize),
    Instance(ProtoRef),
    /// A forward `USE` that was never resolved (Placeholder.js).
    Placeholder,
    /// A runtime object that is not a node of the scene graph (the browser), never printed.
    Internal(&'static str),
}

#[derive(Clone, Debug)]
pub struct Node {
    pub kind: NodeKind,
    pub ctx: CtxId,
    pub name: String,
    pub fields: Vec<Field>,
    pub user_fields: Vec<Field>,
    /// Private child objects (X3DBaseNode.addChildObject), owned by the runtime.
    pub private_fields: Vec<Field>,
    pub can_user_defined_fields: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Outer {
    Scene,
    Proto(ProtoId),
    /// Body of an X3DPrototypeInstance (the instance node).
    Instance(NodeId),
    /// Root context of a scene loaded by an Inline (the Inline node), or of a scene created
    /// at runtime (createVrmlFromString); `None` for the latter.
    Inline(Option<NodeId>),
}

#[derive(Clone, Debug)]
pub struct Route {
    pub source_node: NodeId,
    pub source_field: String,
    pub destination_node: NodeId,
    pub destination_field: String,
    /// Resolved fields (X3DRoute [_sourceField] / [_destinationField]).
    pub source_loc: FieldLoc,
    pub destination_loc: FieldLoc,
}

#[derive(Clone, Debug)]
pub struct ExecutionContext {
    pub parent: Option<CtxId>,
    pub outer: Outer,
    pub root_nodes: Vec<Option<NodeId>>,
    pub named_nodes: InfoArray<NodeId>,
    pub protos: InfoArray<ProtoId>,
    pub externprotos: InfoArray<ExternId>,
    pub routes: Vec<Route>,
}

#[derive(Clone, Debug)]
pub struct ProtoDecl {
    pub name: String,
    pub ctx: CtxId,
    pub body: CtxId,
    /// Interface (user-defined) fields in declaration order.
    pub fields: Vec<Field>,
    /// `metadata` stays a predefined field unless the interface redefines it.
    pub has_metadata: bool,
}

#[derive(Clone, Debug)]
pub struct ExternProtoDecl {
    pub name: String,
    pub ctx: CtxId,
    pub fields: Vec<Field>,
    pub has_metadata: bool,
    pub url: Vec<String>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum FieldLoc {
    Pre(usize),
    User(usize),
    /// Private child object (not visible to getField, never printed).
    Priv(usize),
}

#[derive(Debug)]
pub struct Scene {
    pub table: &'static NodeTable,
    pub specification_version: String,
    pub encoding: String,
    pub world_url: String,
    pub profile: Option<String>,
    /// Ordered like X_ITE's ComponentInfoArray (update keeps the position).
    pub components: Vec<(String, i32)>,
    pub meta: Vec<(String, String)>,
    pub contexts: Vec<ExecutionContext>,
    pub nodes: Vec<Node>,
    pub protos: Vec<ProtoDecl>,
    pub externprotos: Vec<ExternProtoDecl>,
    /// Console output X_ITE would have produced (warnings), for diagnostics.
    pub warnings: Vec<String>,
    /// Nodes parsed outside PROTO declarations, in the order X3DParser.getNodes () collects
    /// them (when the node body closes). X3DParser.setupNodes sets them up in this order.
    pub setup_order: Vec<NodeId>,
    /// Clone counts > 1 (X3DNode.getCloneCount), filled in by the runtime before printing.
    pub clone_counts: Option<std::collections::HashMap<NodeId, usize>>,
}

pub const ROOT: CtxId = 0;

/// X3DScene's LATEST_VERSION.
pub const LATEST_VERSION: &str = "4.1";

impl Scene {
    pub fn new(world_url: &str) -> Scene {
        Scene {
            table: table(),
            specification_version: LATEST_VERSION.to_string(),
            encoding: "SCRIPTED".to_string(),
            world_url: world_url.to_string(),
            profile: None,
            components: vec![],
            meta: vec![],
            contexts: vec![ExecutionContext {
                parent: None,
                outer: Outer::Scene,
                root_nodes: vec![],
                named_nodes: InfoArray::default(),
                protos: InfoArray::default(),
                externprotos: InfoArray::default(),
                routes: vec![],
            }],
            nodes: vec![],
            protos: vec![],
            externprotos: vec![],
            warnings: vec![],
            setup_order: vec![],
            clone_counts: None,
        }
    }

    pub fn warn(&mut self, message: String) {
        self.warnings.push(message);
    }

    // --- profile / components -------------------------------------------------------------

    pub fn update_component(&mut self, name: &str, level: i32) {
        match self.components.iter_mut().find(|c| c.0 == name) {
            Some(c) => c.1 = level,
            None => self.components.push((name.to_string(), level)),
        }
    }

    /// X3DBrowser.getComponent (name) without a level: the supported (maximum) level.
    pub fn supported_component_level(&self, name: &str) -> Option<i32> {
        self.table.component(name).map(|c| c.level)
    }

    /// X3DScene.hasComponent: profile components or scene components.
    pub fn has_component(&self, name: &str) -> bool {
        if let Some(p) = self.profile.as_deref().and_then(|p| self.table.profile(p)) {
            if p.components.iter().any(|c| c.0 == name) {
                return true;
            }
        }

        self.components.iter().any(|c| c.0 == name)
    }

    /// Concrete node classes registered in the browser once this scene's components are loaded:
    /// every non-external component, plus external ones requested by the profile or COMPONENT
    /// statements (and their dependencies). X3DBrowser.loadComponents.
    /// `browser .getConcreteNodes () .has (name)`: node types of the components loaded so far.
    pub fn is_concrete_node(&self, name: &str) -> bool {
        self.table.get(name).is_some_and(|t| self.component_loaded(&t.component))
    }

    pub fn component_loaded(&self, name: &str) -> bool {
        let Some(c) = self.table.component(name) else {
            return false;
        };

        if !c.external {
            return true;
        }

        let mut requested: Vec<String> = vec![];
        let profile = self.profile.as_deref().unwrap_or("Full");

        if let Some(p) = self.table.profile(profile) {
            requested.extend(p.components.iter().map(|c| c.0.clone()));
        }

        requested.extend(self.components.iter().map(|c| c.0.clone()));

        let mut seen: Vec<String> = vec![];

        while let Some(n) = requested.pop() {
            if seen.contains(&n) {
                continue;
            }

            if let Some(c) = self.table.component(&n) {
                requested.extend(c.dependencies.iter().cloned());
            }

            seen.push(n);
        }

        seen.iter().any(|n| n == name)
    }

    // --- execution contexts ---------------------------------------------------------------

    pub fn new_context(&mut self, parent: CtxId, outer: Outer) -> CtxId {
        self.contexts.push(ExecutionContext {
            parent: Some(parent),
            outer,
            root_nodes: vec![],
            named_nodes: InfoArray::default(),
            protos: InfoArray::default(),
            externprotos: InfoArray::default(),
            routes: vec![],
        });

        self.contexts.len() - 1
    }

    pub fn ctx(&self, id: CtxId) -> &ExecutionContext {
        &self.contexts[id]
    }

    pub fn ctx_mut(&mut self, id: CtxId) -> &mut ExecutionContext {
        &mut self.contexts[id]
    }

    // --- nodes ----------------------------------------------------------------------------

    pub fn node(&self, id: NodeId) -> &Node {
        &self.nodes[id as usize]
    }

    pub fn node_mut(&mut self, id: NodeId) -> &mut Node {
        &mut self.nodes[id as usize]
    }

    fn push_node(&mut self, node: Node) -> NodeId {
        self.nodes.push(node);
        (self.nodes.len() - 1) as NodeId
    }

    /// X3DExecutionContext.createNode (typeName, false): `None` if the type is not available.
    pub fn create_node(&mut self, ctx: CtxId, type_name: &str) -> Option<NodeId> {
        let index = *self.table.by_name.get(type_name)?;
        let ty = &self.table.types[index];

        if !self.component_loaded(&ty.component) {
            return None;
        }

        // String comparison, like X_ITE (`this .getSpecificationVersion () < from`).
        let version = self.specification_version.as_str();

        if version < ty.from.as_str() || version > ty.to.as_str() {
            return None;
        }

        if !self.has_component(&ty.component) {
            let url = self.world_url.clone();
            self.warn(format!("Node type '{type_name}' does not match component/profile statements in '{url}'."));
        }

        Some(self.instantiate_builtin(ctx, index))
    }

    pub fn instantiate_builtin(&mut self, ctx: CtxId, index: usize) -> NodeId {
        let ty = &self.table.types[index];
        let variant = ty.variant(&self.specification_version);
        let fields = variant.fields.iter().map(|d| Field::new(&d.name, d.access, d.initial.clone())).collect();

        self.push_node(Node {
            kind: NodeKind::Builtin(index),
            ctx,
            name: String::new(),
            fields,
            user_fields: vec![],
            private_fields: vec![],
            can_user_defined_fields: variant.can_user_defined_fields,
        })
    }

    pub fn create_placeholder(&mut self, ctx: CtxId) -> NodeId {
        self.push_node(Node {
            kind: NodeKind::Placeholder,
            ctx,
            name: String::new(),
            fields: vec![],
            user_fields: vec![],
            private_fields: vec![],
            can_user_defined_fields: false,
        })
    }

    /// X3DExecutionContext.createProto (name, false).
    pub fn create_proto_instance(&mut self, ctx: CtxId, name: &str) -> Option<NodeId> {
        let mut c = ctx;

        loop {
            let found = match self.contexts[c].protos.get(name) {
                Some(p) => Some(ProtoRef::Proto(p)),
                None => self.contexts[c].externprotos.get(name).map(ProtoRef::Extern),
            };

            if let Some(proto) = found {
                return Some(self.instantiate_proto(ctx, proto));
            }

            if self.contexts[c].outer == Outer::Scene && self.contexts[c].parent.is_none() {
                break;
            }

            match self.contexts[c].parent {
                Some(p) => c = p,
                None => break,
            }
        }

        None
    }

    pub fn instantiate_proto(&mut self, ctx: CtxId, proto: ProtoRef) -> NodeId {
        let (has_metadata, interface) = match proto {
            ProtoRef::Proto(p) => (self.protos[p].has_metadata, &self.protos[p].fields),
            ProtoRef::Extern(e) => (self.externprotos[e].has_metadata, &self.externprotos[e].fields),
        };

        let mut fields = vec![];

        if has_metadata {
            fields.push(Field::new("metadata", access::INPUT_OUTPUT, FieldValue::SFNode(None)));
        }

        for f in interface {
            fields.push(Field::new(&f.name, f.access, f.value.clone()));
        }

        self.push_node(Node {
            kind: NodeKind::Instance(proto),
            ctx,
            name: String::new(),
            fields,
            user_fields: vec![],
            private_fields: vec![],
            can_user_defined_fields: false,
        })
    }

    pub fn type_name(&self, id: NodeId) -> String {
        match &self.node(id).kind {
            NodeKind::Builtin(i) => self.table.types[*i].name.clone(),
            NodeKind::Instance(ProtoRef::Proto(p)) => self.protos[*p].name.clone(),
            NodeKind::Instance(ProtoRef::Extern(e)) => self.externprotos[*e].name.clone(),
            // Placeholder extends X3DNode and inherits its static typeName.
            NodeKind::Placeholder => "X3DNode".to_string(),
            NodeKind::Internal(name) => name.to_string(),
        }
    }

    // --- fields ---------------------------------------------------------------------------

    fn find_in(fields: &[Field], aliases: Option<&std::collections::HashMap<String, String>>, name: &str) -> Option<usize> {
        let direct = |n: &str| -> Option<usize> {
            if let Some(i) = fields.iter().position(|f| f.name == n) {
                return Some(i);
            }

            let target = aliases?.get(n)?;
            fields.iter().position(|f| &f.name == target)
        };

        if let Some(i) = direct(name) {
            return Some(i);
        }

        // X3DBaseNode.js: getFieldFromArray — set_x and x_changed resolve to inputOutput x.
        if let Some(rest) = name.strip_prefix("set_") {
            let i = direct(rest)?;
            return (fields[i].access == access::INPUT_OUTPUT).then_some(i);
        }

        if let Some(rest) = name.strip_suffix("_changed") {
            let i = direct(rest)?;
            return (fields[i].access == access::INPUT_OUTPUT).then_some(i);
        }

        None
    }

    fn aliases(&self, id: NodeId) -> Option<&std::collections::HashMap<String, String>> {
        match &self.node(id).kind {
            NodeKind::Builtin(i) => Some(&self.table.types[*i].variant(&self.specification_version).aliases),
            _ => None,
        }
    }

    /// X3DBaseNode.getPredefinedField.
    pub fn get_predefined_field(&self, id: NodeId, name: &str) -> Option<usize> {
        Self::find_in(&self.node(id).fields, self.aliases(id), name)
    }

    /// X3DBaseNode.getUserDefinedField.
    pub fn get_user_defined_field(&self, id: NodeId, name: &str) -> Option<usize> {
        Self::find_in(&self.node(id).user_fields, None, name)
    }

    /// X3DBaseNode.getField: user-defined first, then predefined.
    pub fn get_field(&self, id: NodeId, name: &str) -> Option<FieldLoc> {
        if let Some(i) = self.get_user_defined_field(id, name) {
            return Some(FieldLoc::User(i));
        }

        self.get_predefined_field(id, name).map(FieldLoc::Pre)
    }

    pub fn field(&self, id: NodeId, loc: FieldLoc) -> &Field {
        match loc {
            FieldLoc::Pre(i) => &self.node(id).fields[i],
            FieldLoc::User(i) => &self.node(id).user_fields[i],
            FieldLoc::Priv(i) => &self.node(id).private_fields[i],
        }
    }

    pub fn field_mut(&mut self, id: NodeId, loc: FieldLoc) -> &mut Field {
        match loc {
            FieldLoc::Pre(i) => &mut self.node_mut(id).fields[i],
            FieldLoc::User(i) => &mut self.node_mut(id).user_fields[i],
            FieldLoc::Priv(i) => &mut self.node_mut(id).private_fields[i],
        }
    }

    /// X3DBaseNode.addUserDefinedField: replaces (and moves to the end) a field of that name.
    pub fn add_user_defined_field(&mut self, id: NodeId, field: Field) -> usize {
        let node = self.node_mut(id);

        if let Some(i) = node.user_fields.iter().position(|f| f.name == field.name) {
            node.user_fields.remove(i);
        }

        node.user_fields.push(field);
        node.user_fields.len() - 1
    }

    /// X3DBaseNode.isDefaultValue (node-level: compares with the field definition).
    pub fn is_default_value(&self, id: NodeId, loc: FieldLoc) -> bool {
        let node = self.node(id);
        let field = self.field(id, loc);

        match (&node.kind, loc) {
            (NodeKind::Builtin(t), FieldLoc::Pre(_)) => {
                let def = self.table.types[*t].variant(&self.specification_version).fields.iter().find(|d| d.name == field.name);

                match def {
                    Some(d) => d.default.equals(&field.value),
                    None => !field.modified,
                }
            }
            (NodeKind::Instance(ProtoRef::Proto(p)), FieldLoc::Pre(_)) => {
                match self.protos[*p].fields.iter().find(|f| f.name == field.name) {
                    Some(f) => f.value.equals(&field.value),
                    None if field.name == "metadata" => field.value.equals(&FieldValue::SFNode(None)),
                    None => !field.modified,
                }
            }
            // X3DPrototypeInstance.isDefaultValue: an extern proto that never loaded only
            // knows whether a field was set.
            (NodeKind::Instance(ProtoRef::Extern(_)), _) => !field.modified,
            // User-defined fields are their own definition.
            (_, FieldLoc::User(_)) => true,
            _ => !field.modified,
        }
    }

    // --- named nodes ------------------------------------------------------------------------

    pub fn get_named_node(&self, ctx: CtxId, name: &str) -> Option<NodeId> {
        self.contexts[ctx].named_nodes.get(name)
    }

    pub fn get_unique_name(&self, ctx: CtxId, name: &str) -> String {
        let named = &self.contexts[ctx].named_nodes;
        get_unique_name(|n| named.has(n), name)
    }

    /// X3DExecutionContext.updateNamedNode.
    pub fn update_named_node(&mut self, ctx: CtxId, name: &str, node: NodeId) {
        self.remove_named_node(ctx, name);

        let old_name = self.node(node).name.clone();

        self.contexts[ctx].named_nodes.update(&old_name, name, node);
        self.node_mut(node).name = name.to_string();
    }

    pub fn remove_named_node(&mut self, ctx: CtxId, name: &str) {
        if let Some(n) = self.contexts[ctx].named_nodes.get(name) {
            self.node_mut(n).name = String::new();
        }

        self.contexts[ctx].named_nodes.remove(name);
    }

    /// X3DParser.renameExistingNode.
    pub fn rename_existing_node(&mut self, ctx: CtxId, name: &str) {
        if let Some(existing) = self.get_named_node(ctx, name) {
            let unique = self.get_unique_name(ctx, name);
            self.update_named_node(ctx, &unique, existing);
        }
    }

    // --- protos -----------------------------------------------------------------------------

    pub fn new_proto(&mut self, ctx: CtxId) -> ProtoId {
        let id = self.protos.len();
        let body = self.new_context(ctx, Outer::Proto(id));

        self.protos.push(ProtoDecl { name: String::new(), ctx, body, fields: vec![], has_metadata: true });
        id
    }

    pub fn new_extern_proto(&mut self, ctx: CtxId, url: Vec<String>) -> ExternId {
        self.externprotos.push(ExternProtoDecl { name: String::new(), ctx, fields: vec![], has_metadata: true, url });
        self.externprotos.len() - 1
    }

    /// X3DBaseNode.addUserDefinedField on a proto declaration.
    pub fn add_interface_field(fields: &mut Vec<Field>, has_metadata: &mut bool, field: Field) {
        if field.name == "metadata" {
            *has_metadata = false;
        }

        if let Some(i) = fields.iter().position(|f| f.name == field.name) {
            fields.remove(i);
        }

        fields.push(field);
    }

    /// X3DExecutionContext.updateProtoDeclaration (with its console warnings).
    pub fn update_proto_declaration(&mut self, ctx: CtxId, name: &str, proto: ProtoId) {
        if self.contexts[ctx].externprotos.get(name).is_some() {
            self.warnings.push(format!("Added proto '{name}' will override extern proto with same name."));
        }

        if self.is_concrete_node(name) {
            self.warnings.push(format!("Added proto '{name}', but will not override built-in node of same type if profile/components include this node."));
        }

        let old_name = self.protos[proto].name.clone();

        self.contexts[ctx].protos.update(&old_name, name, proto);
        self.protos[proto].name = name.to_string();
    }

    /// X3DExecutionContext.updateExternProtoDeclaration (with its console warnings).
    pub fn update_extern_proto_declaration(&mut self, ctx: CtxId, name: &str, proto: ExternId) {
        if self.contexts[ctx].protos.get(name).is_some() {
            self.warnings.push(format!("Added extern proto '{name}' will be overridden by proto with same name."));
        }

        if self.is_concrete_node(name) {
            self.warnings.push(format!("Added extern proto '{name}', but will not override built-in node of same type if profile/components include this node."));
        }

        let old_name = self.externprotos[proto].name.clone();

        self.contexts[ctx].externprotos.update(&old_name, name, proto);
        self.externprotos[proto].name = name.to_string();
    }

    pub fn get_unique_proto_name(&self, ctx: CtxId, name: &str) -> String {
        let protos = &self.contexts[ctx].protos;
        get_unique_name(|n| protos.has(n), name)
    }

    pub fn get_unique_extern_proto_name(&self, ctx: CtxId, name: &str) -> String {
        let protos = &self.contexts[ctx].externprotos;
        get_unique_name(|n| protos.has(n), name)
    }

    /// The proto whose body is `ctx`, if any (X3DParser.getOuterNode + isInsideProtoDeclaration).
    pub fn outer_proto(&self, ctx: CtxId) -> Option<ProtoId> {
        match self.contexts[ctx].outer {
            Outer::Proto(p) => Some(p),
            _ => None,
        }
    }

    // --- routes -----------------------------------------------------------------------------

    /// X3DExecutionContext.addRoute (with X3DRoute.checkFields). Returns an error message for
    /// the cases where X_ITE throws.
    pub fn add_route(&mut self, ctx: CtxId, source: NodeId, source_field: &str, destination: NodeId, destination_field: &str) -> Result<Option<usize>, String> {
        let src = self
            .get_field(source, source_field)
            .ok_or_else(|| format!("Unknown field '{source_field}' in node class {}.", self.type_name(source)))?;
        let dst = self
            .get_field(destination, destination_field)
            .ok_or_else(|| format!("Unknown field '{destination_field}' in node class {}.", self.type_name(destination)))?;

        let (sf, df) = (self.field(source, src), self.field(destination, dst));

        if sf.ty() != df.ty() {
            return Err(format!(
                "Bad ROUTE specification: source field type must match destination field type of fields named \"{}\" and \"{}\".",
                sf.name, df.name
            ));
        }

        if !access::is_output(sf.access) {
            return Err(format!("Bad ROUTE specification: source field \"{}\" must be an output.", sf.name));
        }

        if !access::is_input(df.access) {
            return Err(format!("Bad ROUTE specification: destination field \"{}\" must be an input.", df.name));
        }

        if self.contexts[ctx].routes.iter().any(|r| {
            r.source_node == source && r.source_loc == src && r.destination_node == destination && r.destination_loc == dst
        }) {
            return Ok(None);
        }

        // X3DRoute.getSourceField / getDestinationField.
        let source_name = if sf.access == access::INPUT_OUTPUT { format!("{}_changed", sf.name) } else { sf.name.clone() };
        let destination_name = if df.access == access::INPUT_OUTPUT { format!("set_{}", df.name) } else { df.name.clone() };

        self.field_mut(source, src).output_routes += 1;
        self.field_mut(destination, dst).input_routes += 1;

        self.contexts[ctx].routes.push(Route {
            source_node: source,
            source_field: source_name,
            destination_node: destination,
            destination_field: destination_name,
            source_loc: src,
            destination_loc: dst,
        });

        Ok(Some(self.contexts[ctx].routes.len() - 1))
    }

    /// X3DNode.hasRoutes.
    pub fn has_routes(&self, id: NodeId) -> bool {
        let node = self.node(id);

        if node.fields.iter().chain(node.user_fields.iter()).any(|f| f.input_routes > 0 || f.output_routes > 0) {
            return true;
        }

        self.contexts[node.ctx].routes.iter().any(|r| r.source_node == id || r.destination_node == id)
    }
}

/// Id offsets of a scene merged into another (see [`Scene::merge`]).
#[derive(Clone, Copy, Debug)]
pub struct MergeMap {
    pub node_offset: NodeId,
    pub ctx_offset: CtxId,
    pub proto_offset: ProtoId,
    pub extern_offset: ExternId,
    /// The merged scene's root execution context.
    pub root_ctx: CtxId,
}

impl MergeMap {
    pub fn node(&self, n: NodeId) -> NodeId {
        n + self.node_offset
    }
}

impl Scene {
    /// Append another scene (an X3DScene created at runtime, e.g. by createVrmlFromString or an
    /// Inline) to this one so both share one node arena. Its root context becomes a child of
    /// `parent` with `Outer::Inline (outer)`.
    pub fn merge(&mut self, mut other: Scene, parent: CtxId, outer: Option<NodeId>) -> MergeMap {
        let m = MergeMap {
            node_offset: self.nodes.len() as NodeId,
            ctx_offset: self.contexts.len(),
            proto_offset: self.protos.len(),
            extern_offset: self.externprotos.len(),
            root_ctx: self.contexts.len(),
        };

        let remap_value = |v: &mut FieldValue| match v {
            FieldValue::SFNode(Some(n)) => *n += m.node_offset,
            FieldValue::MFNode(list) => {
                for n in list.iter_mut().flatten() {
                    *n += m.node_offset;
                }
            }
            _ => {}
        };

        let remap_field = |f: &mut Field| {
            remap_value(&mut f.value);

            for r in &mut f.references {
                r.proto += m.proto_offset;
            }
        };

        for node in &mut other.nodes {
            node.ctx += m.ctx_offset;

            if let NodeKind::Instance(p) = &mut node.kind {
                *p = match *p {
                    ProtoRef::Proto(i) => ProtoRef::Proto(i + m.proto_offset),
                    ProtoRef::Extern(i) => ProtoRef::Extern(i + m.extern_offset),
                };
            }

            for f in node.fields.iter_mut().chain(node.user_fields.iter_mut()).chain(node.private_fields.iter_mut()) {
                remap_field(f);
            }
        }

        for (i, c) in other.contexts.iter_mut().enumerate() {
            c.parent = if i == 0 { Some(parent) } else { c.parent.map(|p| p + m.ctx_offset) };
            c.outer = match c.outer {
                Outer::Scene => Outer::Inline(outer),
                Outer::Proto(p) => Outer::Proto(p + m.proto_offset),
                Outer::Instance(n) => Outer::Instance(n + m.node_offset),
                Outer::Inline(n) => Outer::Inline(n.map(|n| n + m.node_offset)),
            };

            for r in c.root_nodes.iter_mut().flatten() {
                *r += m.node_offset;
            }

            c.named_nodes.map_values(|n| n + m.node_offset);
            c.protos.map_values(|p| p + m.proto_offset);
            c.externprotos.map_values(|e| e + m.extern_offset);

            for r in &mut c.routes {
                r.source_node += m.node_offset;
                r.destination_node += m.node_offset;
            }
        }

        for p in &mut other.protos {
            p.ctx += m.ctx_offset;
            p.body += m.ctx_offset;

            for f in &mut p.fields {
                remap_field(f);
            }
        }

        for e in &mut other.externprotos {
            e.ctx += m.ctx_offset;

            for f in &mut e.fields {
                remap_field(f);
            }
        }

        self.nodes.append(&mut other.nodes);
        self.contexts.append(&mut other.contexts);
        self.protos.append(&mut other.protos);
        self.externprotos.append(&mut other.externprotos);

        m
    }
}
