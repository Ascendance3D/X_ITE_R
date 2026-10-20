//! Port of src/x_ite/Parser/XMLParser.js (XML mode; the HTMLParser overrides are DOM integration
//! only) into the `x3d-scene` model. Attribute values are read with the VRML value grammar of
//! [`Parser`], as XMLParser.js reads them with VRMLParser.prototype's value functions.
//! JSONParser.js converts X3D JSON into the same element tree (see json.rs).

use crate::dom::{Element, XNode};
use crate::lexer::{js_parse_float, js_parse_int};
use crate::parser::{ParseError, Parser};
use x3d_fields::value::unescape;
use x3d_fields::{FieldType, FieldValue, NodeId, access, jsmath};
use x3d_scene::*;

/// The `parents` stack of XMLParser.js: nodes, PROTO / EXTERNPROTO declarations, and fields
/// (a `<field>` or `<fieldValue>` whose child elements are its node values).
#[derive(Clone, Copy, Debug)]
enum Parent {
    Node(NodeId),
    Proto(ProtoId),
    Extern(ExternId),
    Field(FieldTarget),
}

#[derive(Clone, Copy, Debug)]
enum FieldTarget {
    Node(NodeId, FieldLoc),
    Proto(ProtoId, usize),
    Extern(ExternId, usize),
}

/// The node type a Placeholder stands for (Placeholder.#type): a concrete node type,
/// X3DPrototypeInstance, or X3DNode (unknown element name).
#[derive(Clone, Copy, Debug)]
enum UseType {
    Concrete(usize),
    Instance,
    Unknown,
}

pub(crate) struct XmlParser<'a> {
    p: Parser<'a>,
    parents: Vec<Parent>,
    placeholder_types: std::collections::HashMap<NodeId, UseType>,
    /// IS links whose node field sends events to the PROTO field (outputOnly, inputOutput), with
    /// the node field's value when it was connected.
    links: Vec<(NodeId, FieldLoc, ProtoId, usize, FieldValue)>,
    /// The input is an HTML element (XMLParser `HTMLParser` overrides): node and field names go
    /// through html_support.rs and the proto maps below.
    html: bool,
    /// `protoNames`: PROTO name and its upper case form → the name.
    proto_names: std::collections::HashMap<String, String>,
    /// `protoFields`: per PROTO or EXTERNPROTO, field name and lower case form → the name.
    proto_fields: std::collections::HashMap<(bool, usize), std::collections::HashMap<String, String>>,
}

/// GoldenGate with an XML input of `""` (whitespace-only text): `$.parseXML` throws, the input
/// becomes `null`, and XMLParser.xmlElement (null) yields an empty scene.
pub(crate) fn empty_scene(world_url: &str) -> Scene {
    let mut scene = Scene::new(world_url);

    scene.encoding = "XML".into();
    scene.profile = Some("Full".into());
    scene
}

/// XMLParser.parseIntoScene for the root element `<X3D>` (the caller checked the name).
pub(crate) fn parse_x3d_element(root: &Element, world_url: &str) -> Result<Scene, ParseError> {
    parse_x3d(root, world_url, false)
}

/// XMLParser.parseIntoScene for an HTML `<x3d>` element (`HTMLElement` input, see html.rs).
pub(crate) fn parse_x3d_html_element(root: &Element, world_url: &str) -> Result<Scene, ParseError> {
    parse_x3d(root, world_url, true)
}

fn parse_x3d(root: &Element, world_url: &str, html: bool) -> Result<Scene, ParseError> {
    let mut p = Parser::new("", world_url);

    p.name = "XML Parser";

    let mut parser = XmlParser {
        p,
        parents: vec![],
        placeholder_types: Default::default(),
        links: vec![],
        html,
        proto_names: Default::default(),
        proto_fields: Default::default(),
    };

    parser.p.scene.encoding = "XML".into();
    parser.p.scene.profile = Some("Full".into());
    parser.x3d_element(root);
    parser.propagate_links();
    parser.p.scene.setup_nodes();

    Ok(parser.p.scene)
}

/// X3DNode.getSourceText: the `url` field of the node types that keep source text in it.
fn source_text_index(scene: &Scene, id: NodeId) -> Option<usize> {
    let node = scene.node(id);

    match node.kind {
        NodeKind::Builtin(_) if matches!(scene.type_name(id).as_str(), "Script" | "ShaderPart" | "ShaderProgram" | "PackagedShader") => {
            node.fields.iter().position(|f| f.name == "url")
        }
        _ => None,
    }
}

fn id(s: Option<&str>) -> Option<&str> {
    s.filter(|s| !s.is_empty())
}

impl<'a> XmlParser<'a> {
    fn scene(&mut self) -> &mut Scene {
        &mut self.p.scene
    }

    fn ctx(&self) -> CtxId {
        self.p.ctx()
    }

    fn warn(&mut self, message: String) {
        self.p.warn(message);
    }

    fn x3d_element(&mut self, el: &'a Element) {
        // Profile
        let profile = id(el.attr("profile")).unwrap_or("Full").to_string();

        if self.p.scene.table.profile(&profile).is_some() {
            self.scene().profile = Some(profile);
        } else {
            // XMLParser.x3dElement: catch (error) { console .error (error); }
            self.p.scene.error(format!("Error: Profile '{profile}' is not supported."));
        }

        // Specification version
        if let Some(version) = id(el.attr("version")) {
            self.scene().specification_version = version.to_string();
        }

        for child in &el.children {
            if let XNode::Element(e) = child {
                if matches!(e.name.as_str(), "head" | "HEAD") {
                    self.head_element(e);
                }
            }
        }

        // `if (!this .xml) this .headElement (xmlElement)`: HTML hoists the head statements.
        if self.html {
            self.head_element(el);
        }

        for child in &el.children {
            if let XNode::Element(e) = child {
                if matches!(e.name.as_str(), "Scene" | "SCENE") {
                    self.children_elements(e);
                }
            }
        }

        self.p.setup_nodes();
    }

    fn head_element(&mut self, el: &'a Element) {
        for child in &el.children {
            let XNode::Element(e) = child else { continue };

            match e.name.as_str() {
                "component" | "COMPONENT" => self.component_element(e),
                "unit" | "UNIT" => self.unit_element(e),
                "meta" | "META" => self.meta_element(e),
                _ => {}
            }
        }
    }

    fn component_element(&mut self, el: &Element) {
        let (Some(name), Some(level)) = (el.attr("name"), el.attr("level")) else {
            let which = if el.attr("name").is_none() { "name" } else { "level" };
            return self.warn(format!("XML Parser: Bad component statement. Expected {which} attribute."));
        };

        // X3DBrowser.getComponent: unknown names throw (logged with console.log, not a warning);
        // the level is clamped to [1, supported], 0 or NaN meaning the supported level.
        let Some(supported) = self.p.scene.supported_component_level(name) else {
            return;
        };

        let level = jsmath::to_int32(js_parse_int(level));
        let level = if level == 0 { supported } else { level.clamp(1, supported) };

        self.scene().update_component(name, level);
    }

    fn unit_element(&mut self, el: &Element) {
        for a in ["category", "name", "conversionFactor"] {
            if el.attr(a).is_none() {
                return self.warn(format!("XML Parser: Bad unit statement. Expected {a} attribute."));
            }
        }

        let factor = js_parse_float(el.attr("conversionFactor").unwrap());

        self.scene().update_unit(el.attr("category").unwrap(), el.attr("name").unwrap(), factor);
    }

    fn meta_element(&mut self, el: &Element) {
        let (Some(key), Some(value)) = (el.attr("name"), el.attr("content")) else {
            let which = if el.attr("name").is_none() { "name" } else { "content" };
            return self.warn(format!("XML Parser: Bad meta statement. Expected {which} attribute."));
        };

        if key.is_empty() {
            return self.warn("Error: Couldn't add metadata: name is empty.".into());
        }

        self.scene().meta.push((key.to_string(), value.to_string()));
    }

    fn children_elements(&mut self, el: &'a Element) {
        for child in &el.children {
            self.child_element(child);
        }
    }

    fn child_element(&mut self, node: &'a XNode) {
        let el = match node {
            XNode::Comment(_) | XNode::Text(_) => return,
            XNode::CData(data) => return self.cdata_node(data),
            XNode::Element(e) => e,
        };

        match el.name.as_str() {
            "ExternProtoDeclare" | "EXTERNPROTODECLARE" => self.extern_proto_declare_element(el),
            "ProtoDeclare" | "PROTODECLARE" => self.proto_declare_element(el),
            "IS" => self.is_element(el),
            "ProtoInstance" | "PROTOINSTANCE" => self.proto_instance_element(el),
            "fieldValue" | "FIELDVALUE" => self.field_value_element(el),
            "field" | "FIELD" => self.field_element(el),
            "ROUTE" => self.route_element(el),
            "IMPORT" => self.import_element(el),
            "EXPORT" => self.export_element(el),
            _ => self.node_element(el),
        }
    }

    fn extern_proto_declare_element(&mut self, el: &'a Element) {
        let Some(name) = id(el.attr("name")) else { return };

        // this .parser .setInput (url ?? ""); this .parser .sfstringValues (this .url)
        self.p.set_input(el.attr("url").unwrap_or("").as_bytes());

        let url = self.p.string_values();

        if url.is_empty() {
            self.warn("XML Parser: Bad ExternProtoDeclare statement. Expected url attribute with value.".into());
        }

        let ctx = self.ctx();
        let e = self.scene().new_extern_proto(ctx, url);

        self.parents.push(Parent::Extern(e));
        self.proto_interface_element(el);
        self.parents.pop();
        self.add_proto_field_names((true, e));

        if let Some(existing) = self.p.scene.ctx(ctx).externprotos.get(name) {
            self.warn(format!("A extern proto named '{name}' is already defined and will be overridden."));

            let unique = self.p.scene.get_unique_extern_proto_name(ctx, name);
            self.scene().update_extern_proto_declaration(ctx, &unique, existing);
        }

        self.scene().update_extern_proto_declaration(ctx, name, e);
        self.add_proto_name(name);
    }

    fn proto_declare_element(&mut self, el: &'a Element) {
        let Some(name) = id(el.attr("name")) else { return };
        let ctx = self.ctx();
        let proto = self.scene().new_proto(ctx);

        let child = |names: [&str; 2]| {
            el.children.iter().find_map(|c| match c {
                XNode::Element(e) if names.contains(&e.name.as_str()) => Some(e),
                _ => None,
            })
        };

        if let Some(interface) = child(["ProtoInterface", "PROTOINTERFACE"]) {
            self.parents.push(Parent::Proto(proto));
            self.proto_interface_element(interface);
            self.parents.pop();
            self.add_proto_field_names((false, proto));
        }

        if let Some(body) = child(["ProtoBody", "PROTOBODY"]) {
            let body_ctx = self.p.scene.protos[proto].body;

            self.p.contexts.push(body_ctx);
            self.parents.push(Parent::Proto(proto));
            self.children_elements(body);
            self.parents.pop();
            self.p.setup_nodes();
            self.p.contexts.pop();
        }

        if let Some(existing) = self.p.scene.ctx(ctx).protos.get(name) {
            self.warn(format!("A proto named '{name}' is already defined and will be overridden."));

            let unique = self.p.scene.get_unique_proto_name(ctx, name);
            self.scene().update_proto_declaration(ctx, &unique, existing);
        }

        self.scene().update_proto_declaration(ctx, name, proto);
        self.add_proto_name(name);
    }

    fn proto_interface_element(&mut self, el: &'a Element) {
        for child in &el.children {
            if let XNode::Element(e) = child {
                if matches!(e.name.as_str(), "field" | "FIELD") {
                    self.field_element(e);
                }
            }
        }
    }

    fn field_element(&mut self, el: &'a Element) {
        let Some(parent) = self.parents.last().copied() else { return };

        let can = match parent {
            Parent::Node(n) => self.p.scene.node(n).can_user_defined_fields,
            Parent::Proto(_) | Parent::Extern(_) => true,
            Parent::Field(_) => false,
        };

        if !can {
            return;
        }

        let access_type = match el.attr("accessType") {
            Some("initializeOnly") => access::INITIALIZE_ONLY,
            Some("inputOnly") => access::INPUT_ONLY,
            Some("outputOnly") => access::OUTPUT_ONLY,
            Some("inputOutput") => access::INPUT_OUTPUT,
            _ => access::INITIALIZE_ONLY,
        };

        let Some(ty) = el.attr("type").and_then(FieldType::from_name) else { return };
        let Some(name) = id(el.attr("name")) else { return };

        let mut field = Field::new(name, access_type, FieldValue::default_for(ty));
        let initializable = access_type & access::INITIALIZE_ONLY != 0;

        if initializable {
            if let Some(v) = self.field_value(ty, &field.value, el.attr("value"), None) {
                field.value = v;
                field.modified = true;
            }
        }

        // node .addUserDefinedField (accessType, name, field), after the children were added
        // to the field; adding first and then filling it gives the same field.
        let target = match parent {
            Parent::Node(n) => FieldTarget::Node(n, FieldLoc::User(self.scene().add_user_defined_field(n, field))),
            Parent::Proto(p) => {
                let d = &mut self.p.scene.protos[p];
                Scene::add_interface_field(&mut d.fields, &mut d.has_metadata, field);
                FieldTarget::Proto(p, d.fields.len() - 1)
            }
            Parent::Extern(e) => {
                let d = &mut self.p.scene.externprotos[e];
                Scene::add_interface_field(&mut d.fields, &mut d.has_metadata, field);
                FieldTarget::Extern(e, d.fields.len() - 1)
            }
            Parent::Field(_) => unreachable!(),
        };

        if initializable {
            self.parents.push(Parent::Field(target));
            self.children_elements(el);
            self.parents.pop();
        }
    }

    fn is_element(&mut self, el: &'a Element) {
        if !self.p.is_inside_proto_declaration() {
            return;
        }

        for child in &el.children {
            if let XNode::Element(e) = child {
                if matches!(e.name.as_str(), "connect" | "CONNECT") {
                    self.connect_element(e);
                }
            }
        }
    }

    fn connect_element(&mut self, el: &Element) {
        let Some(node_field_name) = el.attr("nodeField") else {
            return self.warn("XML Parser: Bad connect statement. Expected nodeField attribute.".into());
        };

        let Some(proto_field_name) = el.attr("protoField") else {
            return self.warn("XML Parser: Bad connect statement. Expected protoField attribute.".into());
        };

        let Some(Parent::Node(node)) = self.parents.last().copied() else { return };
        let Some(proto) = self.p.outer_proto() else { return };

        let Some(loc) = self.p.scene.get_field(node, node_field_name) else {
            let type_name = self.p.scene.type_name(node);
            return self.warn(format!("XML Parser: Couldn't create IS reference. Unknown field '{node_field_name}' in node class {type_name}."));
        };

        let Some(r) = self.p.proto_field_lookup(proto, proto_field_name) else {
            return self.warn(format!("XML Parser: Couldn't create IS reference. Unknown field '{proto_field_name}' in node class X3DProtoDeclaration."));
        };

        let reference = self.p.scene.protos[proto].fields[r].clone();
        let field = self.p.scene.field(node, loc);

        if field.ty() != reference.ty() {
            // XMLParser.isReference reads `this .proto`, which nothing ever assigns: the TypeError is what the warning carries.
            return self.warn("XML Parser: Couldn't create IS reference. Cannot read properties of undefined (reading 'getName')".into());
        }

        if !reference.is_reference(field.access) {
            let message = format!("Field '{}' and '{}' in PROTO {} are incompatible as an IS mapping.", field.name, reference.name, self.p.outer_name());
            return self.warn(format!("XML Parser: Couldn't create IS reference. {message}"));
        }

        let f = self.p.scene.field_mut(node, loc);
        let fr = FieldRef { proto, index: r };

        if !f.references.contains(&fr) {
            f.add_reference(fr);

            // X3DField.addReference: initializeOnly / inputOutput relations copy the value.
            if matches!(f.access & reference.access, access::INITIALIZE_ONLY | access::INPUT_OUTPUT) {
                f.value = reference.value.clone();
            }

            if matches!(f.access & reference.access, access::OUTPUT_ONLY | access::INPUT_OUTPUT) {
                let value = f.value.clone();
                self.links.push((node, loc, proto, r, value));
            }
        }
    }

    /// The node field of an IS link that XML gives more values after the `<connect>` (child
    /// elements of the node) sends its value to the PROTO field (X3DField.addFieldInterest).
    /// X_ITE delivers that event after the parse, so instances created during the parse keep
    /// the field's earlier value.
    fn propagate_links(&mut self) {
        for (node, loc, proto, r, connected) in std::mem::take(&mut self.links) {
            let value = self.p.scene.field(node, loc).value.clone();

            if !value.equals(&connected) {
                self.p.scene.protos[proto].fields[r].value = value;
            }
        }
    }

    fn proto_instance_element(&mut self, el: &'a Element) {
        if self.use_attribute(el) {
            return;
        }

        let Some(name) = id(el.attr("name")) else { return };
        let ctx = self.ctx();

        let Some(node) = self.scene().create_proto_instance(ctx, name) else {
            return self.warn(format!("XML Parser:  Unknown proto or externproto type '{name}'."));
        };

        self.def_attribute(el, node);
        self.add_node(el, Some(node));
        self.parents.push(Parent::Node(node));
        self.children_elements(el);

        if !self.p.is_inside_proto_declaration() {
            self.scene().setup_order.push(node);
        }

        self.parents.pop();
    }

    fn field_value_element(&mut self, el: &'a Element) {
        let Some(Parent::Node(node)) = self.parents.last().copied() else { return };

        if !matches!(self.p.scene.node(node).kind, NodeKind::Instance(_)) {
            return;
        }

        let Some(name) = id(el.attr("name")) else { return };

        let Some(loc) = self.p.scene.get_field(node, name) else {
            let type_name = self.p.scene.type_name(node);
            return self.warn(format!("XML Parser: Couldn't assign field value. Unknown field '{name}' in node class {type_name}."));
        };

        let field = self.p.scene.field(node, loc);

        if field.access & access::INITIALIZE_ONLY == 0 {
            return;
        }

        let (ty, mut current) = (field.ty(), field.value.clone());

        if ty == FieldType::MFNode {
            current = FieldValue::MFNode(vec![]);
            let f = self.p.scene.field_mut(node, loc);
            f.value = current.clone();
            f.modified = true;
        }

        if let Some(v) = self.field_value(ty, &current, el.attr("value"), None) {
            let f = self.p.scene.field_mut(node, loc);
            f.value = v;
            f.modified = true;
        }

        self.parents.push(Parent::Field(FieldTarget::Node(node, loc)));
        self.children_elements(el);
        self.parents.pop();
    }

    fn node_element(&mut self, el: &'a Element) {
        if self.use_attribute(el) {
            return;
        }

        let ctx = self.ctx();
        // nodeNameToCamelCase and protoNameToCamelCase: `undefined` reads as the name "undefined".
        let (node_name, proto_name) = if self.html {
            (self.node_name_to_camel_case(&el.name).unwrap_or("undefined").to_string(), self.proto_names.get(&el.name).cloned().unwrap_or_else(|| "undefined".into()))
        } else {
            (el.name.clone(), el.name.clone())
        };
        let node = match self.scene().create_node(ctx, &node_name) {
            Some(n) => Some(n),
            None => self.scene().create_proto_instance(ctx, &proto_name),
        };

        let Some(node) = node else {
            if el.name == "NULL" {
                return self.add_node(el, None);
            }

            // XMLParser.nodeElement throws this Error and its catch prints the Error object with console.error (DEVELOPMENT).
            return self.p.scene.error(format!(
                "Error: Unknown node type '{}'. You probably have insufficient component/profile statements and/or an inappropriate specification version.",
                el.name
            ));
        };

        // DOMIntegration: Script node support for HTML.
        if el.name == "SCRIPT" && el.script_error {
            return self.warn("XML Parser: Invalid XML.".into());
        }

        self.def_attribute(el, node);
        self.add_node(el, Some(node));
        self.parents.push(Parent::Node(node));
        self.node_attributes(el, node);
        self.children_elements(el);

        if !self.p.is_inside_proto_declaration() {
            self.scene().setup_order.push(node);
        }

        self.parents.pop();
    }

    fn route_element(&mut self, el: &Element) {
        let attrs = ["fromNode", "fromField", "toNode", "toField"];

        for a in attrs {
            if el.attr(a).is_none() {
                return self.warn(format!("XML Parser: Bad ROUTE specification: Expected {a} attribute."));
            }
        }

        let [from, from_field, to, to_field] = attrs.map(|a| el.attr(a).unwrap());
        let ctx = self.ctx();

        let result = match (self.p.scene.get_local_node(ctx, from), self.p.scene.get_local_node(ctx, to)) {
            (None, _) => Err(format!("Unknown named or imported node '{from}'.")),
            (_, None) => Err(format!("Unknown named or imported node '{to}'.")),
            (Some(f), Some(t)) => self.scene().add_route(ctx, f, from_field, t, to_field).map(|_| ()),
        };

        if let Err(e) = result {
            self.warn(format!("XML Parser: {e}"));
        }
    }

    fn import_element(&mut self, el: &Element) {
        let inline_name = el.attr("inlineDEF");
        let exported_name = id(el.attr("importedDEF")).or(el.attr("exportedDEF"));

        let Some(inline_name) = inline_name else {
            return self.warn("XML Parser: Bad IMPORT statement: Expected inlineDEF attribute.".into());
        };

        let Some(exported_name) = exported_name else {
            return self.warn("XML Parser: Bad IMPORT statement: Expected importedDEF attribute.".into());
        };

        let local_name = id(el.attr("AS")).unwrap_or(exported_name);
        let description = id(el.attr("DESCRIPTION")).unwrap_or("");
        let ctx = self.ctx();

        let Some(inline) = self.p.scene.get_named_node(ctx, inline_name) else {
            return self.warn(format!("XML Parser: Named node '{inline_name}' not found."));
        };

        self.scene().rename_existing_node(ctx, local_name);

        match self.scene().add_imported_node(ctx, inline, exported_name, local_name, description) {
            Ok(imp) => {
                let instance = self.p.scene.imported[imp].instance;
                self.p.imported_names.entry(ctx).or_default().entry(local_name.to_string()).or_insert(instance);
            }
            Err(e) => self.warn(format!("XML Parser: {e}")),
        }
    }

    fn export_element(&mut self, el: &Element) {
        let Some(local_name) = el.attr("localDEF") else {
            return self.warn("XML Parser: Bad EXPORT statement: Expected localDEF attribute.".into());
        };

        let exported_name = id(el.attr("AS")).unwrap_or(local_name);
        let description = id(el.attr("DESCRIPTION")).unwrap_or("");
        let ctx = self.ctx();

        let Some(local) = self.p.scene.get_local_node(ctx, local_name) else {
            return self.warn(format!("XML Parser: Unknown named or imported node '{local_name}'."));
        };

        // An existing export of that name moves to a unique name.
        if let Some(existing) = self.p.scene.get_exported_node(ROOT, exported_name) {
            let unique = self.p.scene.get_unique_export_name(exported_name);
            let _ = self.scene().update_exported_node(ROOT, &unique, existing, "");
        }

        if let Err(e) = self.scene().update_exported_node(ROOT, exported_name, local, description) {
            self.warn(format!("XML Parser: {e}"));
        }
    }

    fn cdata_node(&mut self, data: &str) {
        let Some(Parent::Node(node)) = self.parents.last().copied() else { return };
        let Some(i) = source_text_index(&self.p.scene, node) else { return };
        let f = &mut self.scene().node_mut(node).fields[i];

        if let FieldValue::MFString(v) = &mut f.value {
            v.push(data.to_string());
            f.modified = true;
        }
    }

    fn use_attribute(&mut self, el: &'a Element) -> bool {
        let Some(name) = id(el.attr("USE")) else { return false };

        let node_name = if self.html { self.node_name_to_camel_case(&el.name).unwrap_or("undefined").to_string() } else { el.name.clone() };

        let ty = if node_name == "ProtoInstance" {
            UseType::Instance
        } else {
            self.p.scene.table.by_name.get(node_name.as_str()).map(|t| UseType::Concrete(*t)).unwrap_or(UseType::Unknown)
        };

        let ctx = self.ctx();

        if let Some(node) = self.p.scene.get_local_node(ctx, name) {
            self.check_node_type(node, name, ty, el.attr("name"));
            self.add_node(el, Some(node));
            return true;
        }

        let existing = self.p.placeholders.get(&ctx).and_then(|l| l.iter().find(|(n, _)| n == name)).map(|(_, p)| *p);

        let placeholder = match existing {
            Some(p) => p,
            None => {
                let p = self.scene().create_placeholder(ctx);
                self.p.placeholders.entry(ctx).or_default().push((name.to_string(), p));
                self.placeholder_types.insert(p, ty);
                p
            }
        };

        self.add_node(el, Some(placeholder));
        true
    }

    fn check_node_type(&mut self, node: NodeId, name: &str, ty: UseType, type_name: Option<&str>) {
        let scene = &self.p.scene;

        if matches!(scene.node(node).kind, NodeKind::Imported(_)) {
            return;
        }

        let message = match ty {
            UseType::Unknown => None,
            UseType::Instance => match scene.node(node).kind {
                NodeKind::Instance(_) => {
                    let actual = scene.type_name(node);
                    (type_name != Some(actual.as_str())).then(|| format!("XML Parser: DEF/USE mismatch, '{name}', name {} != {actual}.", type_name.unwrap_or("null")))
                }
                _ => Some(format!("XML Parser: DEF/USE mismatch, '{name}', referenced node is not of type X3DPrototypeInstance.")),
            },
            UseType::Concrete(t) => match scene.node(node).kind {
                NodeKind::Builtin(k) if k == t => None,
                _ => Some(format!("XML Parser: DEF/USE mismatch, '{name}', {} != {}.", scene.table.types[t].name, scene.type_name(node))),
            },
        };

        if let Some(m) = message {
            self.warn(m);
        }
    }

    fn def_attribute(&mut self, el: &Element, node: NodeId) {
        let Some(name) = id(el.attr("DEF")) else { return };
        let ctx = self.ctx();

        self.p.named_nodes.entry(ctx).or_default().entry(name.to_string()).or_insert(node);
        self.scene().rename_existing_node(ctx, name);
        self.scene().update_named_node(ctx, name, node);
    }

    /// `HTMLSupport .getNodeTypeName`
    fn node_name_to_camel_case(&self, name: &str) -> Option<&'static str> {
        crate::html_support::get().node_type_name(name)
    }

    /// `HTMLParser.addProtoName`
    fn add_proto_name(&mut self, name: &str) {
        if self.html {
            self.proto_names.insert(name.to_string(), name.to_string());
            self.proto_names.insert(name.to_uppercase(), name.to_string());
        }
    }

    /// `HTMLParser.addProtoFieldNames`
    fn add_proto_field_names(&mut self, key: (bool, usize)) {
        if !self.html {
            return;
        }

        let fields = if key.0 { &self.p.scene.externprotos[key.1].fields } else { &self.p.scene.protos[key.1].fields };
        let mut map = std::collections::HashMap::new();

        for f in fields {
            if matches!(f.name.as_str(), "DEF" | "USE" | "containerField" | "def" | "use" | "containerfield") {
                continue;
            }

            map.insert(f.name.clone(), f.name.clone());
            map.insert(f.name.to_lowercase(), f.name.clone());
        }

        self.proto_fields.insert(key, map);
    }

    /// `HTMLParser.attributeToCamelCase`
    fn attribute_to_camel_case(&self, node: NodeId, name: &str) -> Option<String> {
        if let NodeKind::Instance(proto) = self.p.scene.node(node).kind {
            let key = match proto {
                ProtoRef::Proto(p) => (false, p),
                ProtoRef::Extern(e) => (true, e),
            };

            return self.proto_fields.get(&key)?.get(name).cloned();
        }

        crate::html_support::get().field_name(name).map(String::from)
    }

    fn node_attributes(&mut self, el: &'a Element, node: NodeId) {
        for (name, value) in &el.attrs {
            // attributeToCamelCase: an HTML attribute name that maps to no field is ignored.
            let name = if self.html {
                match self.attribute_to_camel_case(node, name) {
                    Some(n) => n,
                    None => continue,
                }
            } else {
                name.clone()
            };
            let name = &name;
            let Some(i) = self.p.scene.get_predefined_field(node, name) else {
                // An alias of a private child object (GeoViewpoint navType and headlight up to 3.2).
                if let Some(p) = self.p.scene.child_alias(node, name) {
                    let field = &self.p.scene.node(node).private_fields[p];
                    let (ty, current) = (field.ty(), field.value.clone());

                    if let Some(v) = self.field_value(ty, &current, Some(value), None) {
                        let f = &mut self.scene().node_mut(node).private_fields[p];
                        f.value = v;
                        f.modified = true;
                    }
                }

                continue;
            };
            let field = &self.p.scene.node(node).fields[i];

            if !field.is_initializable() {
                continue;
            }

            let (ty, current) = (field.ty(), field.value.clone());
            let field_name = field.name.clone();
            let unit = self.p.field_unit_of(node, &field_name);

            if let Some(v) = self.field_value(ty, &current, Some(value), unit) {
                let f = &mut self.scene().node_mut(node).fields[i];
                f.value = v;
                f.modified = true;
            }
        }
    }

    /// XMLParser.fieldValue: the new value, or `None` when the attribute is missing, the value
    /// did not parse, or reading it threw (the callers ignore or log that).
    fn field_value(&mut self, ty: FieldType, current: &FieldValue, value: Option<&'a str>, unit: Option<String>) -> Option<FieldValue> {
        let value = value?;
        let ctx = self.ctx();

        self.p.contexts.push(ctx);
        self.p.set_input(value.as_bytes());
        self.p.field_unit = unit;

        let result = match ty {
            FieldType::SFNode => Ok(Some(FieldValue::SFNode(None))),
            FieldType::SFString => Ok(Some(FieldValue::SFString(unescape(value)))),
            FieldType::MFString => {
                let v = self.p.string_values();

                if v.is_empty() && !value.is_empty() {
                    self.p.warn("XML Parser: Field is an MFString array and can have multiple values, so make sure to separate each individual string with double quotation marks!".into());
                    Ok(Some(FieldValue::MFString(vec![value.to_string()])))
                } else {
                    Ok(Some(FieldValue::MFString(v)))
                }
            }
            _ if ty.name().starts_with("MF") => self.p.sf_values(ty).map(Some),
            _ => self.p.field_value(ty, current),
        };

        self.p.field_unit = None;
        self.p.contexts.pop();

        result.ok().flatten()
    }

    /// XMLParser.addNode: into the current context's root nodes, a field parent, or the
    /// parent node's `containerField` field.
    fn add_node(&mut self, el: &Element, node: Option<NodeId>) {
        let parent = self.parents.last().copied();

        let parent = match parent {
            None | Some(Parent::Proto(_)) => {
                let ctx = self.ctx();
                self.scene().ctx_mut(ctx).root_nodes.push(node);
                return;
            }
            Some(Parent::Extern(_)) => return,
            Some(p) => p,
        };

        let (target_node, loc): (Option<NodeId>, FieldLoc) = match parent {
            Parent::Field(FieldTarget::Node(n, loc)) => (Some(n), loc),
            Parent::Field(FieldTarget::Proto(p, i)) => return self.push_node_value(&mut |s| &mut s.protos[p].fields[i], node),
            Parent::Field(FieldTarget::Extern(e, i)) => return self.push_node_value(&mut |s| &mut s.externprotos[e].fields[i], node),
            Parent::Node(n) => {
                let container_field = id(el.attr("containerField")).map(String::from).or_else(|| node.and_then(|n| self.container_field_of(n)));

                let Some(container_field) = container_field else {
                    return self.warn("XML Parser: Node must have a container field attribute.".into());
                };

                let Some(loc) = self.p.scene.get_field(n, &container_field) else {
                    let what = node.map(|n| self.p.scene.type_name(n)).unwrap_or_else(|| "null".into());
                    let type_name = self.p.scene.type_name(n);
                    return self.warn(format!(
                        "XML Parser: Container field for node {what} not found. Unknown field '{container_field}' in node class {type_name}."
                    ));
                };

                (Some(n), loc)
            }
            _ => unreachable!(),
        };

        let n = target_node.unwrap();
        self.push_node_value(&mut |s| s.field_mut(n, loc), node);
    }

    fn push_node_value(&mut self, field: &mut dyn FnMut(&mut Scene) -> &mut Field, node: Option<NodeId>) {
        let f = field(&mut self.p.scene);

        match &mut f.value {
            FieldValue::SFNode(v) => *v = node,
            FieldValue::MFNode(v) => v.push(node),
            _ => return,
        }

        f.modified = true;
    }

    /// `node .getContainerField ()`, for a Placeholder its type's.
    fn container_field_of(&self, node: NodeId) -> Option<String> {
        let scene = &self.p.scene;

        match &scene.node(node).kind {
            NodeKind::Builtin(t) => Some(scene.table.types[*t].container_field.clone()),
            NodeKind::Instance(_) => Some("children".into()),
            NodeKind::Placeholder => match self.placeholder_types.get(&node) {
                Some(UseType::Concrete(t)) => Some(scene.table.types[*t].container_field.clone()),
                Some(UseType::Instance) => Some("children".into()),
                _ => None,
            },
            NodeKind::Imported(imp) => {
                let record = &scene.imported[*imp];
                let ctx = scene.contexts.iter().position(|c| c.outer == Outer::Inline(Some(record.inline)))?;
                let local = scene.get_exported_node(ctx, &record.exported_name)?;
                self.container_field_of(local)
            }
            #[allow(unreachable_patterns)]
            _ => None,
        }
    }
}
