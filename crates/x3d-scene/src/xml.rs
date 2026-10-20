//! `scene .toXMLString ()`: X3DScene.toXMLStream, X3DExecutionContext.toXMLStream, X3DNode.toXMLStream,
//! X3DPrototypeInstance, X3DProtoDeclaration, X3DExternProtoDeclaration, X3DImportedNode,
//! X3DExportedNode, X3DImportedNodeInstance and X3DRoute toXMLStream, the XMLGenerator helpers
//! (InputOutput/XMLGenerator.js), and the value printers of Fields/*.js and Base/*Array*.js.
//!
//! The output stream and the name bookkeeping are the VRML generator's (vrml.rs). This module
//! supplies the XML layout and XMLGenerator's container-field stack.

use crate::scene::*;
use crate::vrml::VrmlGenerator;
use x3d_fields::math::Vector4;
use x3d_fields::{FieldValue, Generator, NodeId, access};

/// XMLGenerator: `containerFields`. `None` is a `null` pushed by the caller, which prints no
/// containerField attribute (as does an empty stack).
pub struct XmlGenerator<'a> {
    pub(crate) base: VrmlGenerator<'a>,
    container_fields: Vec<Option<String>>,
}

impl Scene {
    /// `scene .toXMLString ()` with X_ITE's default (TIDY) generator options.
    pub fn to_xml_string(&self) -> String {
        XmlGenerator::new(self).scene_to_xml(ROOT)
    }

    /// X3DScene.toXMLStream of the scene whose root context is `root` (an Inline's or a script-made scene).
    pub fn to_xml_string_ctx(&self, root: CtxId) -> String {
        XmlGenerator::new(self).scene_to_xml(root)
    }

    /// `field .toXMLString ()`: the field's `toXMLStream` outside any execution context.
    pub fn field_value_to_xml_string(&self, value: &FieldValue) -> String {
        let mut g = XmlGenerator::new(self);

        g.base.push_ctx(self.first_node_ctx(value));
        g.base.unit_base = 1;

        match value {
            FieldValue::SFNode(n) => g.sfnode_to_xml(*n),
            FieldValue::MFNode(nodes) => g.mfnode_to_xml(nodes),
            _ => g.value_field_to_xml(value, false),
        }

        g.base.pop_ctx();
        g.base.g.string
    }
}

/// XMLGenerator.EncodeString: the escapes of a value printed inside a `'`-quoted attribute or an
/// element. Quotes take the VRML backslash escape, as X_ITE does.
pub fn encode_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());

    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("&#xD;"),
            '\n' => out.push_str("&#xA;"),
            '\t' => out.push_str("&#x9;"),
            '"' => out.push_str("\\\""),
            '\'' => out.push_str("&apos;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            _ => out.push(c),
        }
    }

    out
}

/// XMLGenerator.EncodeSourceText: script source keeps its line breaks and tabs.
pub fn encode_source_text(s: &str) -> String {
    let mut out = String::with_capacity(s.len());

    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '"' => out.push_str("\\\""),
            '\'' => out.push_str("&apos;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '&' => out.push_str("&amp;"),
            _ => out.push(c),
        }
    }

    out
}

/// A url that X_ITE takes as script source: `/^\s*(?:data|ecmascript|javascript|vrmlscript):/s`.
pub(crate) fn is_source_url(s: &str) -> bool {
    let t = s.trim_start();

    ["data:", "ecmascript:", "javascript:", "vrmlscript:"].iter().any(|p| t.starts_with(p))
}

/// X3DNode.getSourceText: the `url` field of a Script or a shader node, when the node type has one.
pub(crate) fn source_text_index(scene: &Scene, id: NodeId) -> Option<usize> {
    let node = scene.node(id);

    match &node.kind {
        NodeKind::Builtin(_) if matches!(scene.type_name(id).as_str(), "Script" | "ShaderPart" | "ShaderProgram" | "PackagedShader") => {
            node.fields.iter().position(|f| f.name == "url")
        }
        _ => None,
    }
}

/// X3DNode.toXMLStream / toJSONStream `cdata`: the source text, when it holds a script url.
pub(crate) fn source_cdata(scene: &Scene, id: NodeId) -> Option<usize> {
    let i = source_text_index(scene, id)?;

    match &scene.node(id).fields[i].value {
        FieldValue::MFString(v) if !v.is_empty() && v.iter().any(|s| is_source_url(s)) => Some(i),
        _ => None,
    }
}

/// X3DBaseNode.getConvertedFields: the value a predefined field prints as. OrthoViewpoint's
/// fieldOfView is written as an SFVec4f (see vrml.rs field_value_to_vrml).
pub(crate) fn printed_value(scene: &Scene, id: NodeId, i: usize) -> FieldValue {
    let field = &scene.node(id).fields[i];

    if let FieldValue::MFFloat(v) = &field.value {
        if field.name == "fieldOfView" && v.len() == 4 && scene.type_name(id) == "OrthoViewpoint" {
            return FieldValue::SFVec4f(Vector4::new(v[0] as f64, v[1] as f64, v[2] as f64, v[3] as f64));
        }
    }

    field.value.clone()
}

/// X3DField.references: the interface field of the PROTO a reference points to is initializable.
pub(crate) fn reference_initializable(scene: &Scene, r: &FieldRef) -> bool {
    scene.protos[r.proto].fields[r.index].is_initializable()
}

/// The XML and JSON `mustOutputValue` of a predefined field: an inputOutput field whose references are
/// all non-initializable must print its value, unless it is at its default.
pub(crate) fn must_output_pre(scene: &Scene, id: NodeId, i: usize) -> bool {
    let field = &scene.node(id).fields[i];

    field.access == access::INPUT_OUTPUT
        && !field.references.is_empty()
        && !field.references.iter().any(|r| reference_initializable(scene, r))
        && !scene.is_default_value(id, FieldLoc::Pre(i))
}

/// `mustOutputValue` of a user-defined field: as [`must_output_pre`], but without the default check
/// (X3DNode.toXMLStream and toJSONStream test a user field's value only where they print it).
pub(crate) fn must_output_user(scene: &Scene, field: &Field) -> bool {
    field.access == access::INPUT_OUTPUT && !field.references.is_empty() && !field.references.iter().any(|r| reference_initializable(scene, r))
}

/// The exported node an IMPORT refers to: the Inline's loaded root context exports it. `None` when the
/// Inline has not loaded.
pub(crate) fn imported_local(scene: &Scene, imp: ImportedId) -> Option<NodeId> {
    let record = &scene.imported[imp];
    let ctx = scene.contexts.iter().position(|c| c.outer == Outer::Inline(Some(record.inline)))?;

    scene.get_exported_node(ctx, &record.exported_name)
}

/// X3DImportedNodeInstance delegates getTypeName and getContainerField to the exported node.
pub(crate) fn type_name_of(scene: &Scene, id: NodeId) -> String {
    match &scene.node(id).kind {
        NodeKind::Imported(imp) => match imported_local(scene, *imp) {
            Some(local) => scene.type_name(local),
            None => "X3DImportedNodeInstance".to_string(),
        },
        _ => scene.type_name(id),
    }
}

/// `this .getContainerField ()`: the node type's container field (`None` is undefined).
pub(crate) fn container_field_of(scene: &Scene, id: NodeId) -> Option<String> {
    match &scene.node(id).kind {
        NodeKind::Builtin(t) => Some(scene.table.types[*t].container_field.clone()),
        NodeKind::Imported(imp) => imported_local(scene, *imp).and_then(|local| container_field_of(scene, local)),
        // X3DPrototypeInstance has the static containerField "children".
        NodeKind::Instance(_) => Some("children".to_string()),
        _ => None,
    }
}

impl<'a> XmlGenerator<'a> {
    pub fn new(scene: &'a Scene) -> Self {
        XmlGenerator { base: VrmlGenerator::new(scene), container_fields: vec![] }
    }

    fn scene(&self) -> &'a Scene {
        self.base.scene
    }

    fn g(&mut self) -> &mut Generator {
        &mut self.base.g
    }

    // --- XMLGenerator helpers ----------------------------------------------------------------

    fn opening_tag(&mut self, name: &str) {
        let g = self.g();

        g.indent();
        g.push("<");
        g.push(name);
        g.push(">");
    }

    fn closing_tag(&mut self, name: &str) {
        let g = self.g();

        g.indent();
        g.push("</");
        g.push(name);
        g.push(">");
    }

    fn open_tag(&mut self, name: &str) {
        let g = self.g();

        g.indent();
        g.push("<");
        g.push(name);
    }

    fn end_tag(&mut self) {
        let g = self.g();

        g.push(">");
        g.tidy_break();
    }

    /// XMLGenerator.closeTag with `closingTags` false: every element without content is `/>`.
    fn close_tag(&mut self, _name: &str) {
        self.g().push("/>");
    }

    fn attribute(&mut self, name: &str, value: &str) {
        let encoded = encode_string(value);
        let g = self.g();

        g.space();
        g.push(name);
        g.push("='");
        g.push(&encoded);
        g.push("'");
    }

    /// XMLGenerator.containerField. `default` is `None` where X_ITE passes nothing or an undefined value.
    fn container_field(&mut self, default: Option<&str>) {
        let Some(Some(name)) = self.container_fields.last().cloned() else {
            return;
        };

        if default != Some(name.as_str()) {
            self.attribute("containerField", &name);
        }
    }

    fn push_container(&mut self, field: Option<String>) {
        self.container_fields.push(field);
    }

    fn pop_container(&mut self) {
        self.container_fields.pop();
    }

    fn add_node(&mut self, id: NodeId) {
        let index = self.base.cur_index();

        index.nodes.insert(id);
        index.route_nodes.insert(id);
    }

    // --- X3DScene / X3DExecutionContext ----------------------------------------------------

    /// X3DScene.toXMLStream, with the toXMLString frame (a `null` execution context below the scene).
    fn scene_to_xml(mut self, scene_root: CtxId) -> String {
        let scene = self.scene();

        self.base.push_ctx(None);

        self.g().indent();
        self.g().push("<?xml version=\"1.0\" encoding=\"UTF-8\"?>");
        self.g().tidy_break();
        self.g().indent();
        self.g().push("<!DOCTYPE X3D PUBLIC \"ISO//Web3D//DTD X3D ");
        self.g().push(LATEST_VERSION);
        self.g().push("//EN\" \"https://www.web3d.org/specifications/x3d-");
        self.g().push(LATEST_VERSION);
        self.g().push(".dtd\">");
        self.g().tidy_break();

        self.open_tag("X3D");
        self.attribute("profile", scene.profile_of(scene_root).unwrap_or("Full"));
        self.attribute("version", LATEST_VERSION);
        self.attribute("xmlns:xsd", "http://www.w3.org/2001/XMLSchema-instance");
        self.attribute("xsd:noNamespaceSchemaLocation", &format!("https://www.web3d.org/specifications/x3d-{LATEST_VERSION}.xsd"));
        self.end_tag();
        self.g().inc_indent();

        // <head>: components, then units whose conversion factor is not 1, then meta.

        let components = scene.components_of(scene_root);
        let metadata = scene.meta_of(scene_root);
        let units: Vec<_> = scene.units_of(scene_root).iter().filter(|u| u.conversion_factor != 1.0).collect();

        if !components.is_empty() || !units.is_empty() || !metadata.is_empty() {
            self.opening_tag("head");
            self.g().tidy_break();
            self.g().inc_indent();

            for (name, level) in components {
                self.open_tag("component");
                self.attribute("name", name);
                self.attribute("level", &scene.component_level_text(scene_root, name, *level));
                self.close_tag("component");
                self.g().tidy_break();
            }

            for u in &units {
                self.open_tag("unit");
                self.attribute("category", &u.category);
                self.attribute("name", &u.name);
                self.attribute("conversionFactor", &x3d_fields::jsmath::number_to_string(u.conversion_factor));
                self.close_tag("unit");
                self.g().tidy_break();
            }

            for (key, value) in metadata {
                self.open_tag("meta");
                self.attribute("name", key);
                self.attribute("content", value);
                self.close_tag("meta");
                self.g().tidy_break();
            }

            self.g().dec_indent();
            self.closing_tag("head");
            self.g().tidy_break();
        }

        // <Scene>

        let root = scene.ctx(scene_root);

        if !root.externprotos.is_empty() || !root.protos.is_empty() || !root.root_nodes.is_empty() {
            self.opening_tag("Scene");
            self.g().tidy_break();
            self.g().inc_indent();

            self.base.push_ctx(Some(scene_root));
            self.context_to_xml(scene_root);

            for &e in root.exported.values() {
                self.exported_to_xml(e);
                self.g().tidy_break();
            }

            self.base.pop_ctx();
            self.g().dec_indent();
            self.closing_tag("Scene");
        } else {
            self.open_tag("Scene");
            self.close_tag("Scene");
        }

        self.g().tidy_break();
        self.g().dec_indent();
        self.closing_tag("X3D");
        self.g().tidy_break();

        self.base.pop_ctx();
        self.base.pop_ctx();

        std::mem::take(&mut self.base.g.string)
    }

    /// X3DExecutionContext.toXMLStream.
    fn context_to_xml(&mut self, ctx: CtxId) {
        let scene = self.scene();

        self.base.push_ctx(Some(ctx));
        self.base.named_nodes(ctx);

        // Each InfoArray entry is printed and followed by a TidyBreak, unless it throws (then nothing).

        for &e in scene.ctx(ctx).externprotos.values() {
            self.extern_proto_to_xml(e);
            self.g().tidy_break();
        }

        for &p in scene.ctx(ctx).protos.values() {
            self.proto_to_xml(p);
            self.g().tidy_break();
        }

        // Root nodes: a null root throws in X3DNode.toXMLStream's caller and prints nothing.

        // The root nodes are an MFNode: one node per line, no break after the last, then one more break.

        let root_nodes = &scene.ctx(ctx).root_nodes;

        if !root_nodes.is_empty() {
            self.mfnode_to_xml(root_nodes);
            self.g().tidy_break();
        }

        for &i in scene.ctx(ctx).imported.values() {
            if self.imported_to_xml(i) {
                self.g().tidy_break();
            }
        }

        for route in &scene.ctx(ctx).routes {
            if self.route_to_xml(route) {
                self.g().tidy_break();
            }
        }

        self.base.pop_ctx();
    }

    // --- nodes -------------------------------------------------------------------------------

    /// SFNode.toXMLStream.
    fn sfnode_to_xml(&mut self, node: Option<NodeId>) {
        match node {
            Some(n) => self.node_to_xml(n),
            None => self.g().push("<!-- NULL -->"),
        }
    }

    /// MFNode.toXMLStream: one node per line; a null element is an empty NULL element.
    fn mfnode_to_xml(&mut self, nodes: &[Option<NodeId>]) {
        let last = nodes.len().wrapping_sub(1);

        for (i, node) in nodes.iter().enumerate() {
            match node {
                Some(n) => self.node_to_xml(*n),
                None => {
                    self.open_tag("NULL");
                    self.container_field(None);
                    self.close_tag("NULL");
                }
            }

            if i != last {
                self.g().tidy_break();
            }
        }
    }

    /// The node dispatch of the scene graph: X3DImportedNodeInstance, X3DPrototypeInstance, X3DNode.
    fn node_to_xml(&mut self, id: NodeId) {
        let scene = self.scene();

        match &scene.node(id).kind {
            NodeKind::Imported(imp) => self.imported_instance_to_xml(id, *imp),
            NodeKind::Instance(_) => self.proto_instance_to_xml(id),
            _ => self.builtin_node_to_xml(id),
        }
    }

    /// X3DImportedNodeInstance.toXMLStream: `<Type USE='importedName'/>`.
    fn imported_instance_to_xml(&mut self, id: NodeId, imp: ImportedId) {
        let scene = self.scene();
        let type_name = type_name_of(scene, id);
        let container = container_field_of(scene, id);

        self.open_tag(&type_name);
        self.attribute("USE", &scene.imported[imp].imported_name);
        self.container_field(container.as_deref());
        self.close_tag(&type_name);
    }

    /// X3DNode.toXMLStream.
    fn builtin_node_to_xml(&mut self, id: NodeId) {
        let scene = self.scene();
        let shared = self.base.is_shared_node(id);
        let name = self.base.name(id);
        let type_name = scene.type_name(id);
        let container = container_field_of(scene, id);

        if !name.is_empty() && self.base.cur_index().nodes.contains(&id) {
            self.open_tag(&type_name);
            self.attribute("USE", &name);
            self.container_field(container.as_deref());
            self.close_tag(&type_name);
            return;
        }

        self.open_tag(&type_name);

        if !name.is_empty() {
            self.add_node(id);
            self.attribute("DEF", &name);
        }

        self.container_field(container.as_deref());

        let node = scene.node(id);
        let in_ctx = self.base.current().is_some();
        let source = source_text_index(scene, id);
        let cdata = source_cdata(scene, id);
        let fields = self.base.changed_fields(id);

        let mut references: Vec<FieldLoc> = vec![];
        let mut children: Vec<usize> = vec![];

        self.g().inc_indent();
        self.g().inc_indent();

        for &i in &fields {
            let field = &node.fields[i];
            let value = printed_value(scene, id, i);
            let must = in_ctx && must_output_pre(scene, id, i);

            if field.references.is_empty() || !in_ctx || shared || must {
                if must {
                    references.push(FieldLoc::Pre(i));
                }

                if field.is_initializable() {
                    match value {
                        FieldValue::SFNode(_) | FieldValue::MFNode(_) => children.push(i),
                        _ => {
                            // The CDATA source text is not an attribute.
                            if cdata == Some(i) {
                                continue;
                            }

                            self.g().attrib_break();
                            self.g().list_indent();
                            self.g().push(&field.name);
                            self.g().push("='");
                            self.base.unit_field = Some((id, field.name.clone()));
                            self.base.raw_field = Some((id, FieldLoc::Pre(i)));
                            self.value_field_to_xml(&value, source == Some(i));
                            self.g().push("'");
                        }
                    }
                }
            } else {
                references.push(FieldLoc::Pre(i));
            }
        }

        self.g().dec_indent();
        self.g().dec_indent();

        let has_user_fields = node.can_user_defined_fields && !node.user_fields.is_empty();

        if !has_user_fields && (references.is_empty() || shared) && children.is_empty() && cdata.is_none() {
            self.close_tag(&type_name);
            return;
        }

        self.end_tag();
        self.g().inc_indent();

        if node.can_user_defined_fields {
            for (k, field) in node.user_fields.iter().enumerate() {
                self.user_field_to_xml(scene, id, k, field, in_ctx, shared, &mut references);
            }
        }

        if !references.is_empty() && !shared {
            self.opening_tag("IS");
            self.g().tidy_break();
            self.g().inc_indent();

            for loc in &references {
                let field = scene.field(id, *loc);

                for r in &field.references {
                    self.open_tag("connect");
                    self.attribute("nodeField", &field.name);
                    self.attribute("protoField", &scene.protos[r.proto].fields[r.index].name);
                    self.close_tag("connect");
                    self.g().tidy_break();
                }
            }

            self.g().dec_indent();
            self.closing_tag("IS");
            self.g().tidy_break();
        }

        for &i in &children {
            let field = &node.fields[i];

            self.push_container(Some(field.name.clone()));

            match &field.value {
                FieldValue::MFNode(nodes) => self.mfnode_to_xml(nodes),
                FieldValue::SFNode(n) => self.sfnode_to_xml(*n),
                _ => unreachable!("children are node fields"),
            }

            self.g().tidy_break();
            self.pop_container();
        }

        if let Some(c) = cdata {
            if let FieldValue::MFString(values) = &node.fields[c].value {
                for value in values {
                    self.g().push("<![CDATA[");
                    self.g().push(value);
                    self.g().push("]]>");
                    self.g().brk();
                }
            }
        }

        self.g().dec_indent();
        self.closing_tag(&type_name);
    }

    /// X3DNode.toXMLStreamUserDefinedField: one `<field>` element of a node with user-defined fields.
    #[allow(clippy::too_many_arguments)]
    fn user_field_to_xml(&mut self, scene: &'a Scene, id: NodeId, k: usize, field: &Field, in_ctx: bool, shared: bool, references: &mut Vec<FieldLoc>) {
        self.open_tag("field");
        self.attribute("accessType", access::name(field.access));
        self.attribute("type", field.ty().name());
        self.attribute("name", &field.name);

        let must = in_ctx && must_output_user(scene, field);

        if field.references.is_empty() || !in_ctx || shared || must {
            if must {
                references.push(FieldLoc::User(k));
            }

            if !field.is_initializable() || (field.value.is_default_value() && !scene.raw_tokens.contains_key(&(id, FieldLoc::User(k)))) {
                self.close_tag("field");
                self.g().tidy_break();
            } else {
                self.base.raw_field = Some((id, FieldLoc::User(k)));
                self.declared_value_to_xml(&field.value);
            }
        } else {
            if in_ctx {
                references.push(FieldLoc::User(k));
            }

            self.close_tag("field");
            self.g().tidy_break();
        }
    }

    /// The value of a declared field (user-defined or PROTO interface), printed inside its `<field>`:
    /// nodes become child elements, other values a `value` attribute.
    fn declared_value_to_xml(&mut self, value: &FieldValue) {
        match value {
            FieldValue::SFNode(_) | FieldValue::MFNode(_) => {
                self.end_tag();
                self.g().inc_indent();
                self.push_container(None);

                match value {
                    FieldValue::MFNode(nodes) => self.mfnode_to_xml(nodes),
                    FieldValue::SFNode(n) => self.sfnode_to_xml(*n),
                    _ => unreachable!(),
                }

                self.g().tidy_break();
                self.g().dec_indent();
                self.closing_tag("field");
                self.g().tidy_break();
                self.pop_container();
            }
            _ => {
                self.g().space();
                self.g().push("value='");
                self.value_field_to_xml(value, false);
                self.g().push("'");
                self.close_tag("field");
                self.g().tidy_break();
            }
        }
    }

    /// X3DPrototypeInstance.toXMLStream.
    fn proto_instance_to_xml(&mut self, id: NodeId) {
        let scene = self.scene();
        let shared = self.base.is_shared_node(id);
        let name = self.base.name(id);
        let type_name = scene.type_name(id);
        let container = container_field_of(scene, id);

        if !name.is_empty() && self.base.cur_index().nodes.contains(&id) {
            self.open_tag("ProtoInstance");
            self.attribute("name", &type_name);
            self.attribute("USE", &name);
            self.container_field(container.as_deref());
            self.close_tag("ProtoInstance");
            return;
        }

        self.open_tag("ProtoInstance");
        self.attribute("name", &type_name);

        if !name.is_empty() {
            self.add_node(id);
            self.attribute("DEF", &name);
        }

        self.container_field(container.as_deref());

        let fields = self.base.changed_fields(id);

        if fields.is_empty() {
            self.close_tag("ProtoInstance");
            return;
        }

        self.end_tag();
        self.g().inc_indent();

        let in_ctx = self.base.current().is_some();
        let node = scene.node(id);
        let mut references: Vec<usize> = vec![];

        for &i in &fields {
            let field = &node.fields[i];
            let value = printed_value(scene, id, i);
            let must = in_ctx && must_output_pre(scene, id, i);

            if field.references.is_empty() || !in_ctx || shared || must {
                if must {
                    references.push(i);
                }

                match &value {
                    FieldValue::MFNode(nodes) => {
                        self.open_tag("fieldValue");
                        self.attribute("name", &field.name);

                        if nodes.is_empty() {
                            self.close_tag("fieldValue");
                            self.g().tidy_break();
                        } else {
                            self.end_tag();
                            self.g().inc_indent();
                            self.push_container(None);
                            self.mfnode_to_xml(nodes);
                            self.g().tidy_break();
                            self.g().dec_indent();
                            self.closing_tag("fieldValue");
                            self.g().tidy_break();
                            self.pop_container();
                        }
                    }
                    FieldValue::SFNode(n) => {
                        self.open_tag("fieldValue");
                        self.attribute("name", &field.name);

                        match n {
                            None => {
                                self.attribute("value", "null");
                                self.close_tag("fieldValue");
                                self.g().tidy_break();
                            }
                            Some(_) => {
                                self.end_tag();
                                self.g().inc_indent();
                                self.push_container(None);
                                self.sfnode_to_xml(*n);
                                self.g().tidy_break();
                                self.g().dec_indent();
                                self.closing_tag("fieldValue");
                                self.g().tidy_break();
                            }
                        }

                        // X_ITE pops here whether or not it pushed (the null branch pushes nothing).
                        self.pop_container();
                    }
                    _ => {
                        self.open_tag("fieldValue");
                        self.attribute("name", &field.name);
                        self.g().space();
                        self.g().push("value='");
                        self.value_field_to_xml(&value, false);
                        self.g().push("'");
                        self.close_tag("fieldValue");
                        self.g().tidy_break();
                    }
                }
            } else {
                references.push(i);
            }
        }

        if !references.is_empty() && !shared {
            self.opening_tag("IS");
            self.g().tidy_break();
            self.g().inc_indent();

            for &i in &references {
                let field = &node.fields[i];

                for r in &field.references {
                    self.open_tag("connect");
                    self.attribute("nodeField", &field.name);
                    self.attribute("protoField", &scene.protos[r.proto].fields[r.index].name);
                    self.close_tag("connect");
                    self.g().tidy_break();
                }
            }

            self.g().dec_indent();
            self.closing_tag("IS");
            self.g().tidy_break();
        }

        self.g().dec_indent();
        self.closing_tag("ProtoInstance");
    }

    // --- PROTO and EXTERNPROTO declarations -------------------------------------------------

    /// X3DProtoDeclaration.toXMLStream.
    fn proto_to_xml(&mut self, p: ProtoId) {
        let scene = self.scene();
        let proto = &scene.protos[p];

        self.open_tag("ProtoDeclare");
        self.attribute("name", &proto.name);
        self.end_tag();

        if !proto.fields.is_empty() {
            self.g().inc_indent();
            self.opening_tag("ProtoInterface");
            self.g().tidy_break();
            self.g().inc_indent();

            for field in &proto.fields {
                self.open_tag("field");
                self.attribute("accessType", access::name(field.access));
                self.attribute("type", field.ty().name());
                self.attribute("name", &field.name);

                if field.value.is_default_value() || !field.is_initializable() {
                    self.close_tag("field");
                    self.g().tidy_break();
                } else {
                    self.declared_value_to_xml(&field.value);
                }
            }

            self.g().dec_indent();
            self.closing_tag("ProtoInterface");
            self.g().tidy_break();
            self.g().dec_indent();
        }

        self.g().inc_indent();
        self.opening_tag("ProtoBody");
        self.g().tidy_break();
        self.g().inc_indent();

        self.context_to_xml(proto.body);

        self.g().dec_indent();
        self.closing_tag("ProtoBody");
        self.g().tidy_break();
        self.g().dec_indent();

        self.closing_tag("ProtoDeclare");
    }

    /// X3DExternProtoDeclaration.toXMLStream.
    fn extern_proto_to_xml(&mut self, e: ExternId) {
        let scene = self.scene();
        let proto = &scene.externprotos[e];

        self.open_tag("ExternProtoDeclare");
        self.attribute("name", &proto.name);

        self.g().space();
        self.g().push("url='");
        self.value_field_to_xml(&FieldValue::MFString(proto.url.clone()), false);
        self.g().push("'");

        if !proto.fields.is_empty() {
            self.end_tag();
            self.g().inc_indent();

            for field in &proto.fields {
                self.open_tag("field");
                self.attribute("accessType", access::name(field.access));
                self.attribute("type", field.ty().name());
                self.attribute("name", &field.name);
                self.close_tag("field");
                self.g().tidy_break();
            }

            self.g().dec_indent();
            self.closing_tag("ExternProtoDeclare");
        } else {
            self.close_tag("ExternProtoDeclare");
        }
    }

    // --- IMPORT, EXPORT and ROUTE -----------------------------------------------------------

    /// X3DImportedNode.toXMLStream; `false` where X_ITE throws (the Inline node is not printed yet).
    fn imported_to_xml(&mut self, imp: ImportedId) -> bool {
        let scene = self.scene();
        let record = &scene.imported[imp];

        if !self.base.cur_index().nodes.contains(&record.inline) {
            return false;
        }

        self.add_route_node(record.instance);

        let imported_name = self.base.imported_name(record.instance);
        let inline_name = self.base.name(record.inline);

        self.open_tag("IMPORT");
        self.attribute("inlineDEF", &inline_name);
        self.attribute("importedDEF", &record.exported_name);

        if imported_name != record.exported_name {
            self.attribute("AS", &imported_name);
        }

        if !record.description.is_empty() {
            self.attribute("DESCRIPTION", &record.description);
        }

        self.close_tag("IMPORT");

        true
    }

    fn add_route_node(&mut self, id: NodeId) {
        self.base.cur_index().route_nodes.insert(id);
    }

    /// X3DExportedNode.toXMLStream.
    fn exported_to_xml(&mut self, e: ExportedId) {
        let scene = self.scene();
        let exported = &scene.exported[e];
        let local_name = self.base.name(exported.local);

        self.open_tag("EXPORT");
        self.attribute("localDEF", &local_name);

        if exported.exported_name != local_name {
            self.attribute("AS", &exported.exported_name);
        }

        if !exported.description.is_empty() {
            self.attribute("DESCRIPTION", &exported.description);
        }

        self.close_tag("EXPORT");
    }

    /// X3DRoute.toXMLStream; `false` where X_ITE throws (an end node is not printed yet).
    fn route_to_xml(&mut self, route: &Route) -> bool {
        let scene = self.scene();

        {
            let index = self.base.cur_index();

            if !index.route_nodes.contains(&route.source_node) || !index.route_nodes.contains(&route.destination_node) {
                return false;
            }
        }

        let source = if scene.is_imported(route.source_node) { self.base.imported_name(route.source_node) } else { self.base.name(route.source_node) };
        let destination =
            if scene.is_imported(route.destination_node) { self.base.imported_name(route.destination_node) } else { self.base.name(route.destination_node) };

        self.open_tag("ROUTE");
        self.attribute("fromNode", &source);
        self.attribute("fromField", &route.source_field);
        self.attribute("toNode", &destination);
        self.attribute("toField", &route.destination_field);
        self.close_tag("ROUTE");

        true
    }
}

impl<'a> XmlGenerator<'a> {
    /// A field value as an attribute value or element content (`toXMLStream` of Fields/*.js): a single
    /// value, or the elements of a multi-valued field, separated by `, `, MFString elements quoted.
    fn value_field_to_xml(&mut self, value: &FieldValue, source_text: bool) {
        if let Some(texts) = self.base.take_raw() {
            crate::vrml::write_raw(self.g(), &texts, value, false);
            return;
        }

        self.base.apply_units();
        self.value_field_to_xml_units(value, source_text);
        self.g().set_unit_factors(1.0, 1.0);
    }

    fn value_field_to_xml_units(&mut self, value: &FieldValue, source_text: bool) {
        match value.elements() {
            Some(elements) => {
                let quoted = matches!(value, FieldValue::MFString(_));
                let last = elements.len().wrapping_sub(1);

                for (i, element) in elements.iter().enumerate() {
                    if quoted {
                        self.g().push("\"");
                    }

                    self.value_to_xml(element, source_text);

                    if quoted {
                        self.g().push("\"");
                    }

                    if i != last {
                        self.g().comma();
                        self.g().tidy_space();
                    }
                }
            }
            None => self.value_to_xml(value, source_text),
        }
    }

    /// A single (SF) value's toXMLStream. Only SFBool and SFString differ from the VRML text.
    fn value_to_xml(&mut self, value: &FieldValue, source_text: bool) {
        match value {
            FieldValue::SFBool(b) => self.g().push(if *b { "true" } else { "false" }),
            FieldValue::SFString(s) => {
                let text = if source_text { encode_source_text(s) } else { encode_string(s) };

                self.g().push(&text);
            }
            _ => value.to_vrml_stream(self.g()),
        }
    }
}
