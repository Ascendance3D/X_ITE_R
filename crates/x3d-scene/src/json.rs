//! `scene .toJSONString ()`: X3DScene.toJSONStream, X3DExecutionContext.toJSONStream, X3DNode.toJSONStream,
//! X3DPrototypeInstance, X3DProtoDeclaration, X3DExternProtoDeclaration, X3DImportedNode,
//! X3DExportedNode, X3DImportedNodeInstance and X3DRoute toJSONStream, the JSONGenerator helpers
//! (InputOutput/JSONGenerator.js), and the value printers of Fields/*.js and Base/*Array*.js.
//!
//! The output stream and the name bookkeeping are the VRML generator's (vrml.rs), as in xml.rs.

use crate::scene::*;
use crate::vrml::VrmlGenerator;
use crate::xml::{is_source_url, must_output_pre, must_output_user, printed_value, source_text_index, type_name_of};
use x3d_fields::{FieldValue, Generator, Image as SfImage, NodeId, access};

/// JSONGenerator: no state beyond the output stream and the name bookkeeping.
pub struct JsonGenerator<'a> {
    base: VrmlGenerator<'a>,
}

impl Scene {
    /// `scene .toJSONString ()` with X_ITE's default (TIDY) generator options.
    pub fn to_json_string(&self) -> String {
        JsonGenerator::new(self).scene_to_json(ROOT)
    }

    /// X3DScene.toJSONStream of the scene whose root context is `root`.
    pub fn to_json_string_ctx(&self, root: CtxId) -> String {
        JsonGenerator::new(self).scene_to_json(root)
    }

    /// `field .toJSONString ()`: the field's `toJSONStream` outside any execution context.
    pub fn field_value_to_json_string(&self, value: &FieldValue) -> String {
        let mut g = JsonGenerator::new(self);

        g.base.push_ctx(self.first_node_ctx(value));
        g.field_value_to_json(value);
        g.base.pop_ctx();
        g.base.g.string
    }
}

/// JSONGenerator.EncodeString: the escapes of a JSON string.
pub fn encode_json_string(s: &str) -> String {
    let mut out = String::with_capacity(s.len());

    for c in s.chars() {
        match c {
            '\\' => out.push_str("\\\\"),
            '\r' => out.push_str("\\r"),
            '\n' => out.push_str("\\n"),
            '\t' => out.push_str("\\t"),
            '"' => out.push_str("\\\""),
            _ => out.push(c),
        }
    }

    out
}

/// JSONGenerator.Number: non-finite numbers are quoted strings.
fn json_number(text: String) -> String {
    match text.as_str() {
        "NaN" | "Infinity" | "-Infinity" => format!("\"{text}\""),
        _ => text,
    }
}

impl<'a> JsonGenerator<'a> {
    pub fn new(scene: &'a Scene) -> Self {
        JsonGenerator { base: VrmlGenerator::new(scene) }
    }

    fn scene(&self) -> &'a Scene {
        self.base.scene
    }

    fn g(&mut self) -> &mut Generator {
        &mut self.base.g
    }

    // --- JSONGenerator helpers ---------------------------------------------------------------

    fn remove_comma(&mut self) {
        // `this .string .replace (/,\s*$/s, "")`: only a comma at the very end (after whitespace) goes.
        let s = &mut self.base.g.string;
        let trimmed = s.trim_end().len();

        if s[..trimmed].ends_with(',') {
            s.truncate(trimmed - 1);
        }
    }

    /// JSONGenerator.stringProperty.
    fn string_property(&mut self, key: &str, value: &str, comma: bool) {
        let encoded = encode_json_string(value);
        let g = self.g();

        if comma {
            g.push(",");
        }

        g.tidy_break();
        g.indent();
        g.push("\"");
        g.push(key);
        g.push("\":");
        g.tidy_space();
        g.push("\"");
        g.push(&encoded);
        g.push("\"");
    }

    /// JSONGenerator.numberProperty: the number is written as is.
    fn number_property(&mut self, key: &str, value: &str, comma: bool) {
        let g = self.g();

        if comma {
            g.push(",");
        }

        g.tidy_break();
        g.indent();
        g.push("\"");
        g.push(key);
        g.push("\":");
        g.tidy_space();
        g.push(value);
    }

    /// JSONGenerator.beginObject. An empty name is an unnamed object (array element).
    fn begin_object(&mut self, name: Option<&str>, comma: bool, main: bool) {
        let g = self.g();

        if comma {
            g.push(",");
        }

        match name {
            Some(name) if main => {
                g.push("{");
                g.tidy_space();
                g.push("\"");
                g.push(name);
                g.push("\"");
                g.push(":");
                g.tidy_break();
                g.inc_indent();
                g.indent();
                g.push("{");
                g.inc_indent();
            }
            Some(name) => {
                g.tidy_break();
                g.indent();
                g.push("\"");
                g.push(name);
                g.push("\"");
                g.push(":");
                g.tidy_space();
                g.push("{");
                g.inc_indent();
            }
            None => {
                g.tidy_break();
                g.indent();
                g.push("{");
                g.inc_indent();
            }
        }
    }

    fn end_object(&mut self) {
        let g = self.g();

        g.tidy_break();
        g.dec_indent();
        g.indent();
        g.push("}");
    }

    /// JSONGenerator.beginArray: a named array value, opened with `[`.
    fn begin_array(&mut self, name: Option<&str>, comma: bool) {
        self.begin_value(name, comma);
        self.g().push("[");
        self.g().inc_indent();
    }

    fn end_array(&mut self) {
        let g = self.g();

        g.tidy_break();
        g.dec_indent();
        g.indent();
        g.push("]");
    }

    /// JSONGenerator.beginValue: `"name": ` (an unnamed value has no key).
    fn begin_value(&mut self, name: Option<&str>, comma: bool) {
        let g = self.g();

        if comma {
            g.push(",");
        }

        g.tidy_break();
        g.indent();

        if let Some(name) = name {
            g.push("\"");
            g.push(name);
            g.push("\":");
            g.tidy_space();
        }
    }

    fn add_node(&mut self, id: NodeId) {
        let index = self.base.cur_index();

        index.nodes.insert(id);
        index.route_nodes.insert(id);
    }

    /// X3DInfoArray.toJSONStream: each entry gets a comma when one came before it; an entry that throws
    /// before printing has its comma removed. Returns the comma state.
    fn info_array_json<T: Copy>(&mut self, items: &[T], mut comma: bool, mut each: impl FnMut(&mut Self, T) -> bool) -> bool {
        for &item in items {
            if comma {
                self.g().push(",");
            }

            if each(self, item) {
                comma = true;
            } else {
                self.remove_comma();
            }
        }

        comma
    }

    // --- scene -------------------------------------------------------------------------------

    /// X3DScene.toJSONStream, with the toJSONString frame.
    fn scene_to_json(mut self, scene_root: CtxId) -> String {
        let scene = self.scene();

        self.base.push_ctx(None);

        self.begin_object(Some("X3D"), false, true);
        self.string_property("encoding", "UTF-8", false);
        self.string_property("@profile", scene.profile_of(scene_root).unwrap_or("Full"), true);
        self.string_property("@version", LATEST_VERSION, true);
        self.string_property("@xsd:noNamespaceSchemaLocation", &format!("https://www.web3d.org/specifications/x3d-{LATEST_VERSION}.xsd"), true);
        self.string_property("JSON schema", &format!("https://www.web3d.org/specifications/x3d-{LATEST_VERSION}-JSONSchema.json"), true);

        // Head: meta data, then components, then units whose conversion factor is not 1.

        let components = scene.components_of(scene_root);
        let metadata = scene.meta_of(scene_root);
        let units: Vec<_> = scene.units_of(scene_root).iter().filter(|u| u.conversion_factor != 1.0).cloned().collect();

        if !components.is_empty() || !units.is_empty() || !metadata.is_empty() {
            let mut head_last_property = false;

            self.begin_object(Some("head"), true, false);

            if !metadata.is_empty() {
                self.begin_array(Some("meta"), head_last_property);

                for (i, (key, value)) in metadata.iter().enumerate() {
                    self.begin_object(None, i != 0, false);
                    self.string_property("@name", key, false);
                    self.string_property("@content", value, true);
                    self.end_object();
                }

                self.end_array();

                head_last_property = true;
            }

            if !components.is_empty() {
                self.begin_array(Some("component"), head_last_property);

                // ComponentInfo.toJSONStream: an unnamed object with a name and a numeric level.
                let mut comma = false;

                for (name, level) in components {
                    if comma {
                        self.g().push(",");
                    }

                    self.begin_object(None, false, false);
                    self.string_property("@name", name, false);
                    self.number_property("@level", &level.to_string(), true);
                    self.end_object();
                    comma = true;
                }

                self.end_array();

                head_last_property = true;
            }

            if !units.is_empty() {
                self.begin_array(Some("unit"), head_last_property);

                // UnitInfoArray.toJSONStream: units with factor 1 throw and their comma is removed.
                for (i, u) in units.iter().enumerate() {
                    if i != 0 {
                        self.g().push(",");
                    }

                    self.begin_object(None, false, false);
                    self.string_property("@category", &u.category, false);
                    self.string_property("@name", &u.name, true);
                    self.number_property("@conversionFactor", &x3d_fields::jsmath::number_to_string(u.conversion_factor), true);
                    self.end_object();
                }

                self.end_array();
            }

            self.end_object();
        }

        // Scene

        self.begin_object(Some("Scene"), true, false);
        self.begin_array(Some("-children"), false);

        self.base.push_ctx(Some(scene_root));

        let comma = self.context_to_json(scene_root);
        let exported: Vec<ExportedId> = scene.ctx(scene_root).exported.values().to_vec();

        self.info_array_json(&exported, comma, |this, e| {
            this.exported_to_json(e);
            true
        });

        self.base.pop_ctx();

        self.end_array();
        self.end_object();

        // X3D end

        self.end_object();
        self.end_object();
        self.g().tidy_break();

        self.base.pop_ctx();
        self.base.pop_ctx();

        std::mem::take(&mut self.base.g.string)
    }

    /// X3DExecutionContext.toJSONStream. Returns the comma state, which the caller uses for the exports.
    fn context_to_json(&mut self, ctx: CtxId) -> bool {
        let scene = self.scene();

        self.base.push_ctx(Some(ctx));
        self.base.named_nodes(ctx);

        let externprotos = scene.ctx(ctx).externprotos.values().to_vec();
        let protos = scene.ctx(ctx).protos.values().to_vec();

        let mut comma = self.info_array_json(&externprotos, false, |this, e| {
            this.extern_proto_to_json(e);
            true
        });

        comma = self.info_array_json(&protos, comma, |this, p| {
            this.proto_to_json(p);
            true
        });

        // Root nodes: a null root is the literal `null`.

        for root in &scene.ctx(ctx).root_nodes {
            if comma {
                self.g().push(",");
            }

            self.g().tidy_break();
            self.g().indent();

            match root {
                Some(n) => self.node_to_json(*n),
                None => self.g().push("null"),
            }

            comma = true;
        }

        // The imports and the routes are given the comma state from before them (X_ITE ignores the
        // value that the imports return).

        let imported = scene.ctx(ctx).imported.values().to_vec();
        let routes = scene.ctx(ctx).routes.clone();

        self.info_array_json(&imported, comma, |this, i| this.imported_to_json(i));
        self.info_array_json(&routes.iter().enumerate().map(|(i, _)| i).collect::<Vec<_>>(), comma, |this, i| this.route_to_json(&routes[i]));

        self.base.pop_ctx();

        comma
    }

    // --- nodes -------------------------------------------------------------------------------

    /// SFNode.toJSONStream / toJSONStreamValue: the node, or `null`.
    fn sfnode_to_json(&mut self, node: Option<NodeId>) {
        match node {
            Some(n) => self.node_to_json(n),
            None => self.g().push("null"),
        }
    }

    /// MFNode.toJSONStream (ArrayFields.js).
    fn mfnode_to_json(&mut self, nodes: &[Option<NodeId>]) {
        if nodes.is_empty() {
            self.g().push("[");
            self.g().tidy_space();
            self.g().push("]");
            return;
        }

        let last = nodes.len() - 1;

        self.g().push("[");
        self.g().tidy_break();
        self.g().inc_indent();

        for (i, node) in nodes.iter().enumerate() {
            if i != last {
                self.g().indent();
                self.sfnode_to_json(*node);
                self.g().push(",");
                self.g().tidy_break();
            } else {
                self.g().indent();
                self.sfnode_to_json(*node);
            }
        }

        self.g().tidy_break();
        self.g().dec_indent();
        self.g().indent();
        self.g().push("]");
    }

    fn node_to_json(&mut self, id: NodeId) {
        let scene = self.scene();

        match &scene.node(id).kind {
            NodeKind::Imported(imp) => self.imported_instance_to_json(id, *imp),
            NodeKind::Instance(_) => self.proto_instance_to_json(id),
            _ => self.builtin_node_to_json(id),
        }
    }

    /// X3DImportedNodeInstance.toJSONStream: `{ "Type": { "@USE": "name" } }`.
    fn imported_instance_to_json(&mut self, id: NodeId, imp: ImportedId) {
        let scene = self.scene();
        let type_name = type_name_of(scene, id);

        self.begin_object(Some(type_name.as_str()), false, true);
        self.string_property("@USE", &scene.imported[imp].imported_name, false);
        self.end_object();
        self.end_object();
    }

    /// X3DNode.toJSONStream.
    fn builtin_node_to_json(&mut self, id: NodeId) {
        let scene = self.scene();
        let shared = self.base.is_shared_node(id);
        let name = self.base.name(id);
        let type_name = scene.type_name(id);

        self.begin_object(Some(type_name.as_str()), false, true);

        if !name.is_empty() && self.base.cur_index().nodes.contains(&id) {
            self.string_property("@USE", &name, false);
            self.end_object();
            self.end_object();
            return;
        }

        let mut comma = false;

        if !name.is_empty() {
            self.add_node(id);
            self.string_property("@DEF", &name, false);
            comma = true;
        }

        let node = scene.node(id);
        let in_ctx = self.base.current().is_some();

        // X3DNode.toJSONStream takes the source text only when it is exactly one script url.
        let source = source_text_index(scene, id).filter(|&i| {
            matches!(&node.fields[i].value, FieldValue::MFString(v) if v.len() == 1 && is_source_url(&v[0]))
        });

        let fields = self.base.changed_fields(id);
        let mut references: Vec<FieldLoc> = vec![];

        if !fields.is_empty() {
            let mut output: Vec<usize> = vec![];

            for &i in &fields {
                let field = &node.fields[i];
                let must = in_ctx && must_output_pre(scene, id, i);

                if field.references.is_empty() || !in_ctx || shared || must {
                    if must {
                        references.push(FieldLoc::Pre(i));
                    }

                    if source != Some(i) && field.is_initializable() {
                        output.push(i);
                    }
                } else {
                    references.push(FieldLoc::Pre(i));
                }
            }

            for (k, &i) in output.iter().enumerate() {
                let field = &node.fields[i];
                let value = printed_value(scene, id, i);
                let comma_before = comma || k != 0;

                let key = match value {
                    FieldValue::SFNode(_) | FieldValue::MFNode(_) => format!("-{}", field.name),
                    _ => format!("@{}", field.name),
                };

                self.begin_value(Some(&key), comma_before);

                if !matches!(value, FieldValue::SFNode(_) | FieldValue::MFNode(_)) {
                    self.base.unit_field = Some((id, field.name.clone()));
                }

                self.field_value_to_json(&value);
            }

            comma = comma || !output.is_empty();
        }

        // User-defined fields.

        if node.can_user_defined_fields && !node.user_fields.is_empty() {
            self.begin_array(Some("field"), comma);

            for (k, field) in node.user_fields.iter().enumerate() {
                self.begin_object(None, k != 0, false);
                self.string_property("@accessType", access::name(field.access), false);
                self.string_property("@type", field.ty().name(), true);
                self.string_property("@name", &field.name, true);

                let must = in_ctx && must_output_user(scene, field);

                if field.references.is_empty() || !in_ctx || shared || must {
                    if must && in_ctx {
                        references.push(FieldLoc::User(k));
                    }

                    if field.is_initializable() && !field.value.is_default_value() {
                        match &field.value {
                            FieldValue::SFNode(n) => {
                                self.begin_array(Some("-children"), true);
                                self.g().tidy_break();
                                self.g().indent();
                                self.sfnode_to_json(*n);
                                self.end_array();
                            }
                            FieldValue::MFNode(nodes) => {
                                self.begin_value(Some("-children"), true);
                                self.mfnode_to_json(nodes);
                            }
                            value => {
                                self.begin_value(Some("@value"), true);
                                self.field_value_to_json(value);
                            }
                        }
                    }
                } else if in_ctx {
                    references.push(FieldLoc::User(k));
                }

                self.end_object();
            }

            self.end_array();
            comma = true;
        }

        // Source text, one line per array element.

        if let Some(i) = source {
            if let FieldValue::MFString(v) = &node.fields[i].value {
                self.begin_array(Some("#sourceCode"), comma);

                for (k, line) in v[0].split('\n').enumerate() {
                    if k != 0 {
                        self.g().push(",");
                    }

                    self.g().tidy_break();
                    self.g().list_indent();
                    self.g().push("\"");
                    self.g().push(&encode_json_string(line));
                    self.g().push("\"");
                }

                self.end_array();
                comma = true;
            }
        }

        // IS references.

        if !references.is_empty() && !shared {
            self.begin_object(Some("IS"), comma, false);
            self.begin_array(Some("connect"), false);

            for (ri, loc) in references.iter().enumerate() {
                let field = scene.field(id, *loc);

                for (pi, r) in field.references.iter().enumerate() {
                    self.begin_object(None, ri != 0 || pi != 0, false);
                    self.string_property("@nodeField", &field.name, false);
                    self.string_property("@protoField", &scene.protos[r.proto].fields[r.index].name, true);
                    self.end_object();
                }
            }

            self.end_array();
            self.end_object();
        }

        self.end_object();
        self.end_object();
    }

    /// X3DPrototypeInstance.toJSONStream.
    fn proto_instance_to_json(&mut self, id: NodeId) {
        let scene = self.scene();
        let shared = self.base.is_shared_node(id);
        let name = self.base.name(id);
        let type_name = scene.type_name(id);

        self.begin_object(Some("ProtoInstance"), false, true);

        if !name.is_empty() && self.base.cur_index().nodes.contains(&id) {
            self.string_property("@name", &type_name, false);
            self.string_property("@USE", &name, true);
            self.end_object();
            self.end_object();
            return;
        }

        if !name.is_empty() {
            self.add_node(id);
            self.string_property("@DEF", &name, false);
        }

        self.string_property("@name", &type_name, !name.is_empty());

        let node = scene.node(id);
        let in_ctx = self.base.current().is_some();
        let fields = self.base.changed_fields(id);

        if !fields.is_empty() {
            let mut references: Vec<usize> = vec![];
            let mut output: Vec<usize> = vec![];

            self.begin_array(Some("fieldValue"), true);

            for &i in &fields {
                let field = &node.fields[i];
                let must = in_ctx && must_output_pre(scene, id, i);

                if field.references.is_empty() || !in_ctx || shared || must {
                    if must {
                        references.push(i);
                    }

                    output.push(i);
                } else {
                    references.push(i);
                }
            }

            for (k, &i) in output.iter().enumerate() {
                let field = &node.fields[i];
                let value = printed_value(scene, id, i);

                self.begin_object(None, k != 0, false);
                self.string_property("@name", &field.name, false);

                match &value {
                    FieldValue::SFNode(n) => {
                        self.begin_array(Some("-children"), true);
                        self.g().tidy_break();
                        self.g().indent();
                        self.sfnode_to_json(*n);
                        self.end_array();
                    }
                    FieldValue::MFNode(nodes) => {
                        self.begin_value(Some("-children"), true);
                        self.mfnode_to_json(nodes);
                    }
                    _ => {
                        self.begin_value(Some("@value"), true);
                        self.field_value_to_json(&value);
                    }
                }

                self.end_object();
            }

            self.end_array();

            if !references.is_empty() && !shared {
                self.begin_object(Some("IS"), true, false);
                self.begin_array(Some("connect"), false);

                for (ri, &i) in references.iter().enumerate() {
                    let field = &node.fields[i];

                    for (pi, r) in field.references.iter().enumerate() {
                        self.begin_object(None, ri != 0 || pi != 0, false);
                        self.string_property("@nodeField", &field.name, false);
                        self.string_property("@protoField", &scene.protos[r.proto].fields[r.index].name, true);
                        self.end_object();
                    }
                }

                self.end_array();
                self.end_object();
            }
        }

        self.end_object();
        self.end_object();
    }

    // --- PROTO and EXTERNPROTO declarations -------------------------------------------------

    /// X3DProtoDeclaration.toJSONStream.
    fn proto_to_json(&mut self, p: ProtoId) {
        let scene = self.scene();
        let proto = &scene.protos[p];

        self.g().tidy_break();
        self.g().indent();

        self.begin_object(Some("ProtoDeclare"), false, true);
        self.string_property("@name", &proto.name, false);

        self.begin_object(Some("ProtoInterface"), true, false);

        if !proto.fields.is_empty() {
            self.begin_array(Some("field"), false);

            for (k, field) in proto.fields.iter().enumerate() {
                self.begin_object(None, k != 0, false);
                self.string_property("@accessType", access::name(field.access), false);
                self.string_property("@type", field.ty().name(), true);
                self.string_property("@name", &field.name, true);

                if !field.value.is_default_value() && field.is_initializable() {
                    match &field.value {
                        FieldValue::SFNode(n) => {
                            self.begin_array(Some("-children"), true);
                            self.g().tidy_break();
                            self.g().indent();
                            self.sfnode_to_json(*n);
                            self.end_array();
                        }
                        FieldValue::MFNode(nodes) => {
                            self.begin_value(Some("-children"), true);
                            self.mfnode_to_json(nodes);
                        }
                        value => {
                            self.begin_value(Some("@value"), true);
                            self.field_value_to_json(value);
                        }
                    }
                }

                self.end_object();
            }

            self.end_array();
        }

        self.end_object();

        // ProtoBody

        self.begin_object(Some("ProtoBody"), true, false);
        self.begin_array(Some("-children"), false);

        self.context_to_json(proto.body);

        self.end_array();
        self.end_object();

        // The ProtoDeclare object was opened as the main object, which takes two braces.
        self.end_object();
        self.end_object();
    }

    /// X3DExternProtoDeclaration.toJSONStream.
    fn extern_proto_to_json(&mut self, e: ExternId) {
        let scene = self.scene();
        let proto = &scene.externprotos[e];

        self.g().tidy_break();
        self.g().indent();

        self.begin_object(Some("ExternProtoDeclare"), false, true);
        self.string_property("@name", &proto.name, false);

        if !proto.fields.is_empty() {
            self.begin_array(Some("field"), true);

            for (k, field) in proto.fields.iter().enumerate() {
                self.begin_object(None, k != 0, false);
                self.string_property("@accessType", access::name(field.access), false);
                self.string_property("@type", field.ty().name(), true);
                self.string_property("@name", &field.name, true);
                self.end_object();
            }

            self.end_array();
        }

        // URL

        self.g().push(",");
        self.g().tidy_break();
        self.g().indent();
        self.g().push("\"@url\":");
        self.g().tidy_space();

        self.field_value_to_json(&FieldValue::MFString(proto.url.clone()));

        self.end_object();
        self.end_object();
    }

    // --- IMPORT, EXPORT and ROUTE -----------------------------------------------------------

    /// X3DImportedNode.toJSONStream; `false` where X_ITE throws (the Inline node is not printed yet).
    fn imported_to_json(&mut self, imp: ImportedId) -> bool {
        let scene = self.scene();
        let record = &scene.imported[imp];

        if !self.base.cur_index().nodes.contains(&record.inline) {
            return false;
        }

        self.g().tidy_break();
        self.g().indent();

        self.add_route_node(record.instance);

        self.begin_object(Some("IMPORT"), false, true);
        let inline_name = self.base.name(record.inline);
        self.string_property("@inlineDEF", &inline_name, false);
        self.string_property("@importedDEF", &record.exported_name, true);

        let imported_name = self.base.imported_name(record.instance);

        if imported_name != record.exported_name {
            self.string_property("@AS", &imported_name, true);
        }

        if !record.description.is_empty() {
            self.string_property("@DESCRIPTION", &record.description, true);
        }

        self.end_object();
        self.end_object();

        true
    }

    fn add_route_node(&mut self, id: NodeId) {
        self.base.cur_index().route_nodes.insert(id);
    }

    /// X3DExportedNode.toJSONStream.
    fn exported_to_json(&mut self, e: ExportedId) {
        let scene = self.scene();
        let exported = &scene.exported[e];
        let local_name = self.base.name(exported.local);

        self.g().tidy_break();
        self.g().indent();

        self.begin_object(Some("EXPORT"), false, true);
        self.string_property("@localDEF", &local_name, false);

        if exported.exported_name != local_name {
            self.string_property("@AS", &exported.exported_name, true);
        }

        if !exported.description.is_empty() {
            self.string_property("@DESCRIPTION", &exported.description, true);
        }

        self.end_object();
        self.end_object();
    }

    /// X3DRoute.toJSONStream; `false` where X_ITE throws (an end node is not printed yet).
    fn route_to_json(&mut self, route: &Route) -> bool {
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

        self.g().tidy_break();
        self.g().indent();

        self.begin_object(Some("ROUTE"), false, true);
        self.string_property("@fromNode", &source, false);
        self.string_property("@fromField", &route.source_field, true);
        self.string_property("@toNode", &destination, true);
        self.string_property("@toField", &route.destination_field, true);
        self.end_object();
        self.end_object();

        true
    }

    // --- values ------------------------------------------------------------------------------

    /// A field's `toJSONStream`: vectors, colors, rotations, matrices and images are bracketed, and
    /// multi-valued fields are lists.
    fn field_value_to_json(&mut self, value: &FieldValue) {
        match value {
            FieldValue::SFNode(n) => self.sfnode_to_json(*n),
            FieldValue::MFNode(nodes) => self.mfnode_to_json(nodes),
            FieldValue::SFImage(image) => {
                self.g().push("[");
                self.g().list_break();
                self.g().inc_indent();
                self.g().list_indent();
                self.image_value_to_json(image);
                self.g().dec_indent();
                self.g().list_break();
                self.g().list_indent();
                self.g().push("]");
            }
            _ => {
                self.base.apply_units();

                match value.elements() {
                    Some(elements) => self.list_to_json(&elements),
                    None if is_bracketed(value) => {
                        self.g().push("[");
                        self.g().tidy_space();
                        self.value_to_json(value);
                        self.g().tidy_space();
                        self.g().push("]");
                    }
                    None => self.value_to_json(value),
                }

                self.g().set_unit_factors(1.0, 1.0);
            }
        }
    }

    /// The JSON list of a multi-valued field: each element's `toJSONStreamValue`, one per line.
    fn list_to_json(&mut self, elements: &[FieldValue]) {
        if elements.is_empty() {
            self.g().push("[");
            self.g().tidy_space();
            self.g().push("]");
            return;
        }

        let last = elements.len() - 1;

        self.g().push("[");
        self.g().list_break();
        self.g().inc_indent();

        for (i, element) in elements.iter().enumerate() {
            if i != last {
                self.g().list_indent();
                self.value_to_json(element);
                self.g().push(",");
                self.g().list_break();
            } else {
                self.g().list_indent();
                self.value_to_json(element);
            }
        }

        self.g().list_break();
        self.g().dec_indent();
        self.g().list_indent();
        self.g().push("]");
    }

    /// A single value's `toJSONStreamValue`.
    fn value_to_json(&mut self, value: &FieldValue) {
        match value {
            FieldValue::SFBool(b) => self.g().push(if *b { "true" } else { "false" }),
            FieldValue::SFInt32(i) => self.g().push(&i.to_string()),
            FieldValue::SFFloat(f) => {
                let f = self.g().to_unit(*f);
                let text = json_number(self.g().float_format(f));
                self.g().push(&text);
            }
            FieldValue::SFDouble(f) | FieldValue::SFTime(f) => {
                let f = self.g().to_unit(*f);
                let text = json_number(self.g().double_format(f));
                self.g().push(&text);
            }
            FieldValue::SFString(s) => {
                let text = encode_json_string(s);

                self.g().push("\"");
                self.g().push(&text);
                self.g().push("\"");
            }
            FieldValue::SFImage(image) => self.image_value_to_json(image),
            FieldValue::SFNode(n) => self.sfnode_to_json(*n),
            _ => {
                let (numbers, double) = numbers_of(value);
                // ToUnit (field .getUnit (), …) on every number (fields with a unit are vectors or
                // scalars), and ToUnit ("angle", …) on a rotation's angle.
                let mut numbers: Vec<f64> = numbers.iter().map(|v| self.g().to_unit(*v)).collect();

                if matches!(value, FieldValue::SFRotation(_)) {
                    numbers[3] = self.g().to_unit_angle(numbers[3]);
                }

                for (i, v) in numbers.iter().enumerate() {
                    let text = json_number(if double { self.g().double_format(*v) } else { self.g().float_format(*v) });

                    self.g().push(&text);

                    if i != numbers.len() - 1 {
                        self.g().push(",");
                        self.g().tidy_space();
                    }
                }
            }
        }
    }

    /// SFImage.toJSONStreamValue: `width, height, comp,` and the pixels, one row per line.
    fn image_value_to_json(&mut self, image: &SfImage) {
        let (width, height) = (image.width, image.height);

        self.g().push(&width.to_string());
        self.g().push(",");
        self.g().tidy_space();
        self.g().push(&height.to_string());
        self.g().push(",");
        self.g().tidy_space();
        self.g().push(&image.comp.to_string());
        self.g().push(",");

        if width != 0 && height != 0 {
            let (width, height) = (width as usize, height as usize);
            let length = image.array.len();

            self.g().list_break();
            self.g().inc_indent();

            for y in 0..height {
                self.g().list_indent();

                let s = y * width;

                for x in 0..width {
                    self.g().push(&(image.array[x + s] as u32).to_string());

                    if x + s != length - 1 {
                        self.g().push(",");
                    }

                    if x != width - 1 {
                        self.g().tidy_space();
                    }
                }

                if y != height - 1 {
                    self.g().list_break();
                }
            }

            self.g().dec_indent();
        }
    }
}

/// The `toJSONStream` of a single vector-like value is bracketed (SFVec, SFColor, SFRotation, ...).
fn is_bracketed(value: &FieldValue) -> bool {
    use FieldValue as V;

    matches!(
        value,
        V::SFColor(_)
            | V::SFColorRGBA(_)
            | V::SFMatrix3d(_)
            | V::SFMatrix3f(_)
            | V::SFMatrix4d(_)
            | V::SFMatrix4f(_)
            | V::SFQuaternion(_)
            | V::SFRotation(_)
            | V::SFVec2d(_)
            | V::SFVec2f(_)
            | V::SFVec3d(_)
            | V::SFVec3f(_)
            | V::SFVec4d(_)
            | V::SFVec4f(_)
    )
}

/// The components of a vector-like single value, and whether they print with DoubleFormat.
fn numbers_of(value: &FieldValue) -> (Vec<f64>, bool) {
    use FieldValue as V;

    match value {
        V::SFColor(c) => (c.components().to_vec(), false),
        V::SFColorRGBA(c) => (c.components().to_vec(), false),
        V::SFMatrix3f(m) => (m.0.to_vec(), false),
        V::SFMatrix3d(m) => (m.0.to_vec(), true),
        V::SFMatrix4f(m) => (m.0.to_vec(), false),
        V::SFMatrix4d(m) => (m.0.to_vec(), true),
        V::SFQuaternion(q) => (vec![q.x, q.y, q.z, q.w], true),
        V::SFRotation(r) => (r.raw().to_vec(), true),
        V::SFVec2f(v) => (v.to_array().to_vec(), false),
        V::SFVec3f(v) => (v.to_array().to_vec(), false),
        V::SFVec4f(v) => (v.to_array().to_vec(), false),
        V::SFVec2d(v) => (v.to_array().to_vec(), true),
        V::SFVec3d(v) => (v.to_array().to_vec(), true),
        V::SFVec4d(v) => (v.to_array().to_vec(), true),
        _ => unreachable!("not a vector-like value"),
    }
}
