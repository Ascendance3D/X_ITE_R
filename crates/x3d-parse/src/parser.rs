//! Port of src/x_ite/Parser/VRMLParser.js (+ X3DParser.js, Placeholder.js) into the
//! `x3d-scene` model. Method names follow the JS so the two can be read side by side.

use crate::color::convert_color;
use crate::lexer::{self, constant_value};
use std::collections::HashMap;
use x3d_fields::math::*;
use x3d_fields::value::unescape;
use x3d_fields::{FieldType, FieldValue, Image, NodeId, access, jsmath};
use x3d_scene::*;

#[derive(Debug, Clone)]
pub struct ParseError {
    pub message: String,
    pub line: usize,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "Parser error at line {}: {}", self.line, self.message)
    }
}

type R<T> = Result<T, ParseError>;

thread_local! {
    /// The warnings (console.warn) the parse that just failed had printed.
    static ERROR_WARNINGS: std::cell::RefCell<Vec<String>> = const { std::cell::RefCell::new(Vec::new()) };
    /// VRMLParser.getError of the last parse that failed, see [`take_error_detail`].
    static ERROR_DETAIL: std::cell::RefCell<Option<String>> = const { std::cell::RefCell::new(None) };
}

/// The warnings the parse that just failed printed before it failed (the scene is gone, so the caller logs them).
pub fn take_error_warnings() -> Vec<String> {
    ERROR_WARNINGS.with(|w| std::mem::take(&mut *w.borrow_mut()))
}

/// The Error message VRMLParser.parseIntoScene threw for the parse that just failed (the line of the input and a caret),
/// once; [`ParseError::message`] is the original message the parser logs first.
pub fn take_error_detail() -> Option<String> {
    ERROR_DETAIL.with(|d| d.borrow_mut().take())
}

/// Result of `nodeStatement`/`node`: a node, `null`, or `false` (nothing parsed).
#[derive(Clone, Copy, Debug, PartialEq)]
enum Stmt {
    Node(NodeId),
    Null,
    No,
}

/// A call the pinned VRMLParser makes on its execution context.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ContextCall {
    /// `createNode`, for a node type.
    CreateNode,
    /// `getBrowser`, for a USE reference, or for the `X3DProtoDeclaration` / `X3DExternProtoDeclaration` of a PROTO.
    GetBrowser,
}

pub struct Parser<'a> {
    input: &'a [u8],
    /// The first call the parse made on its execution context (the first one fails when there is none).
    context_call: Option<ContextCall>,
    /// No execution context at all: `ROUTE` finds no named node (X_ITE: `getLocalNode` of undefined).
    no_context: bool,
    last_index: usize,
    line_number: usize,
    /// Text of the last successful terminal match (`this .result [0]`).
    result: String,
    value_f64: f64,
    value_string: String,
    pub scene: Scene,
    pub(crate) contexts: Vec<CtxId>,
    unknown_level: usize,
    /// X3DParser: per execution context, first DEF of each name.
    pub(crate) named_nodes: HashMap<CtxId, HashMap<String, NodeId>>,
    /// X3DParser: per execution context, the imported nodes a Placeholder may resolve to
    /// (X3DParser.getImportedNodes). Only the XML parser fills it.
    pub(crate) imported_names: HashMap<CtxId, HashMap<String, NodeId>>,
    /// X3DParser: per execution context, unresolved forward USEs in insertion order.
    pub(crate) placeholders: HashMap<CtxId, Vec<(String, NodeId)>>,
    /// The unit category of the predefined field whose value is being read (X3DField.getUnit).
    pub(crate) field_unit: Option<String>,
    /// X3DParser.getName: the prefix of Placeholder's warning.
    pub(crate) name: &'static str,
}

impl<'a> Parser<'a> {
    pub fn new(input: &'a str, world_url: &str) -> Self {
        Parser {
            input: input.as_bytes(),
            last_index: 0,
            line_number: 1,
            result: String::new(),
            value_f64: 0.0,
            value_string: String::new(),
            scene: Scene::new(world_url),
            contexts: vec![ROOT],
            unknown_level: 0,
            context_call: None,
            no_context: false,
            named_nodes: HashMap::new(),
            imported_names: HashMap::new(),
            placeholders: HashMap::new(),
            field_unit: None,
            name: "Parser",
        }
    }

    /// VRMLParser.setInput: read `input` from its start (the XML parser reads attribute values
    /// with the VRML value grammar).
    pub(crate) fn set_input(&mut self, input: &'a [u8]) {
        self.input = input;
        self.last_index = 0;
        self.line_number = 1;
    }

    /// X3DField.fromVRMLString (value) on a fresh field, without a scene (no unit conversion).
    pub fn parse_field_value(ty: FieldType, text: &str) -> Result<FieldValue, ParseError> {
        let mut parser = Parser::new(text, "");
        let current = FieldValue::default_for(ty);

        match parser.field_value(ty, &current)? {
            Some(v) => Ok(v),
            None => parser.err("Couldn't read value for field ''."),
        }
    }

    /// X3DField.fromVRMLString (value, executionContext) on a fresh SFNode, parsed into an
    /// existing scene (Script.js `new SFNode (vrmlSyntax)`). Returns the scene back, and
    /// `Ok(Some(value))` for a parsed value (`SFNode(None)` for NULL), `Ok(None)` when
    /// nothing could be read (X_ITE: "Couldn't read value for field ''.").
    pub fn parse_sfnode_into(
        scene: Scene,
        ctx: CtxId,
        text: &str,
    ) -> (Scene, Result<Option<FieldValue>, ParseError>) {
        let mut parser = Parser::new(text, "");

        parser.scene = scene;
        parser.contexts = vec![ctx];

        let result = parser.field_value(FieldType::SFNode, &FieldValue::SFNode(None));

        if result.is_ok() {
            parser.setup_nodes();
        }

        (parser.scene, result)
    }

    /// X3DField.fromVRMLString (value, scene) (VRMLParser.fieldValue) and X3DField.fromXMLString (value, scene)
    /// (XMLParser.fieldValue, attribute grammar: `xml`) on a field of type `ty`, the nodes of an SFNode or MFNode value
    /// parsed into `ctx` of `scene`. With a scene the field's `unit` converts the values (`parser .setUnits (!!scene)`).
    /// Returns the scene back, and `Ok(Some(value))` for a parsed value, `Ok(None)` when nothing could be read (X_ITE:
    /// "Couldn't read value for field '<name>'."). The XML grammar's MFString warning names the field: `name`.
    pub fn parse_field_into(
        scene: Scene,
        ctx: CtxId,
        ty: FieldType,
        text: &str,
        xml: bool,
        unit: Option<String>,
        name: &str,
    ) -> (Scene, Result<Option<FieldValue>, ParseError>) {
        let (scene, result, _) =
            Parser::parse_field_probe(scene, ctx, ty, text, xml, unit, name, false);

        (scene, result)
    }

    /// [`Parser::parse_field_into`], and the first call the parse made on its execution context. `x3d-runtime` calls this, so
    /// it is `pub`; use `parse_field_into` for a field with a scene. With `no_context` the parse stops at that call and
    /// `ROUTE` warns as X_ITE does with an undefined context. The call is `None` for XML text and for text with no node,
    /// PROTO or USE statement..
    // The arguments of `parse_field_into` and the one flag; a second entry point keeps that function's callers unchanged.
    #[allow(clippy::too_many_arguments)]
    pub fn parse_field_probe(
        scene: Scene,
        ctx: CtxId,
        ty: FieldType,
        text: &str,
        xml: bool,
        unit: Option<String>,
        name: &str,
        no_context: bool,
    ) -> (
        Scene,
        Result<Option<FieldValue>, ParseError>,
        Option<ContextCall>,
    ) {
        let mut parser = Parser::new(text, "");

        parser.scene = scene;
        parser.contexts = vec![ctx];
        parser.field_unit = unit;
        parser.no_context = no_context;

        let current = FieldValue::default_for(ty);

        let result = if xml {
            match ty {
                FieldType::SFNode => Ok(Some(FieldValue::SFNode(None))),
                FieldType::MFNode => Ok(Some(FieldValue::MFNode(vec![]))),
                FieldType::SFString => Ok(Some(FieldValue::SFString(x3d_fields::value::unescape(
                    text,
                )))),
                FieldType::MFString => {
                    let v = parser.string_values();

                    if v.is_empty() && !text.is_empty() {
                        parser.warn(format!("XML Parser: Field '{name}' is an MFString array and can have multiple values, so make sure to separate each individual string with double quotation marks!"));
                        Ok(Some(FieldValue::MFString(vec![text.to_string()])))
                    } else {
                        Ok(Some(FieldValue::MFString(v)))
                    }
                }
                _ if ty.name().starts_with("MF") => parser.sf_values(ty).map(Some),
                _ => parser.field_value(ty, &current),
            }
        } else {
            parser.field_value(ty, &current)
        };

        if result.is_ok() {
            parser.setup_nodes();
        }

        (parser.scene, result, parser.context_call)
    }

    // --- helpers ---------------------------------------------------------------------------

    fn err<T>(&self, message: impl Into<String>) -> R<T> {
        Err(ParseError {
            message: message.into(),
            line: self.line_number,
        })
    }

    /// The parse calls its execution context. With none the call throws in X_ITE, so the parse stops here (the
    /// warnings it made before stay, the ones it would make after never exist).
    fn context_call_made(&mut self, call: ContextCall) -> R<()> {
        self.context_call.get_or_insert(call);

        if self.no_context {
            return self.err("no execution context");
        }

        Ok(())
    }

    pub(crate) fn warn(&mut self, message: String) {
        self.scene.warn(message);
    }

    pub(crate) fn ctx(&self) -> CtxId {
        *self.contexts.last().unwrap()
    }

    pub(crate) fn outer_proto(&self) -> Option<ProtoId> {
        self.scene.outer_proto(self.ctx())
    }

    pub(crate) fn is_inside_proto_declaration(&self) -> bool {
        self.outer_proto().is_some()
    }

    pub(crate) fn outer_name(&self) -> String {
        self.outer_proto()
            .map(|p| self.scene.protos[p].name.clone())
            .unwrap_or_default()
    }

    /// Apply a terminal matcher at the current position (`Grammar.X.parse (this)`).
    fn accept(&mut self, m: impl Fn(&[u8], usize) -> Option<usize>) -> bool {
        match m(self.input, self.last_index) {
            Some(end) => {
                self.result =
                    String::from_utf8_lossy(&self.input[self.last_index..end]).into_owned();
                self.last_index = end;
                true
            }
            None => false,
        }
    }

    fn keyword(&mut self, word: &str) -> bool {
        self.accept(|i, p| lexer::literal(i, p, word))
    }

    fn lines(&mut self, s: &str) {
        self.line_number += s.bytes().filter(|b| *b == b'\n').count();
    }

    fn comments(&mut self) {
        while self.comment() {}
    }

    fn comment(&mut self) -> bool {
        if self.whitespaces() {
            return true;
        }

        self.accept(lexer::comment)
    }

    fn whitespaces(&mut self) -> bool {
        if self.accept(lexer::whitespaces) {
            let r = self.result.clone();
            self.lines(&r);
            return true;
        }

        false
    }

    // --- scene -----------------------------------------------------------------------------

    pub fn parse(mut self) -> Result<Scene, ParseError> {
        self.scene.encoding = "VRML".into();
        self.scene.profile = Some("Full".into());

        ERROR_DETAIL.with(|d| d.borrow_mut().take());
        ERROR_WARNINGS.with(|w| w.borrow_mut().clear());

        if let Err(e) = self.x3d_scene() {
            let detail = self.get_error(&e.message);

            ERROR_DETAIL.with(|d| *d.borrow_mut() = Some(detail));
            ERROR_WARNINGS.with(|w| *w.borrow_mut() = std::mem::take(&mut self.scene.warnings));
            return Err(e);
        }

        self.scene.setup_nodes();

        Ok(self.scene)
    }

    /// VRMLParser.getError: the message of the Error that parseIntoScene throws, with the line of the input where the
    /// parser stopped and a caret under the column.
    fn get_error(&self, message: &str) -> String {
        let text = String::from_utf8_lossy(self.input);
        let input: Vec<u16> = text.encode_utf16().collect();
        let len = input.len() as isize;
        let mut last_index =
            String::from_utf8_lossy(&self.input[..self.last_index.min(self.input.len())])
                .encode_utf16()
                .count() as isize;
        let unexpected = if last_index == len {
            "Unexpected end of file. "
        } else {
            ""
        };
        let at = |i: isize| input[i as usize];
        let newline = |c: u16| c == 10 || c == 13;

        // getLine
        let mut rest = vec![];

        while last_index < len && !newline(at(last_index)) {
            rest.push(at(last_index));
            last_index += 1;
        }

        // getLastLine (twice)
        let mut last_line = |last_index: &mut isize| {
            let mut line: Vec<u16> = vec![];

            *last_index = (*last_index).min(len - 1);

            if *last_index < len {
                *last_index -= 1;
            }

            while *last_index >= 0 && !newline(at(*last_index)) {
                line.insert(0, at(*last_index));
                *last_index -= 1;
            }

            line
        };

        let mut line = last_line(&mut last_index);
        let mut last = last_line(&mut last_index);
        let mut line_pos = line.len() as isize - rest.len() as isize + 1;

        if line.len() > 80 {
            let from = (line_pos - 40).max(0) as usize;
            let to = ((line_pos + 40).max(0) as usize).min(line.len());

            line = line[from.min(to)..to].to_vec();
            last = vec![];
            line_pos = 40;
        }

        let mut world = self.scene.world_url.clone();

        if world.starts_with("data:") {
            world = world[..world.find(',').unwrap_or(world.len())].to_string();
        }

        format!(
            "\n{rule}\nParser error at line {}:{line_pos}\nin '{world}'\n\n{}\n{}\n{}^\n{unexpected}{message}\n{rule}\n",
            self.line_number,
            String::from_utf16_lossy(&last),
            String::from_utf16_lossy(&line),
            " ".repeat((line_pos - 1).max(0) as usize),
            rule = "*".repeat(80),
        )
    }

    fn x3d_scene(&mut self) -> R<()> {
        const VRML: [&str; 5] = [
            "EnvironmentalSensor",
            "PointingDeviceSensor",
            "Scripting",
            "Sound",
            "Text",
        ];

        self.header_statement();
        self.profile_statement()?;
        self.component_statements()?;
        self.unit_statements()?;
        self.meta_statements()?;

        if self.scene.specification_version == "2.0" {
            self.scene.profile = Some("Interchange".into());

            for name in VRML {
                let level = self.scene.supported_component_level(name).unwrap_or(1);
                self.scene.update_component(name, level);
            }
        }

        let mut roots = vec![];

        self.statements(&mut roots)?;
        self.scene.ctx_mut(ROOT).root_nodes = roots;
        self.setup_nodes();

        if self.last_index < self.input.len() {
            return self.err("Unknown statement.");
        }

        Ok(())
    }

    /// `/^#(VRML|X3D) V(.*?) (utf8)\b(.*?)[\r\n]/y`
    fn header_statement(&mut self) -> bool {
        if self.last_index != 0 {
            return false;
        }

        let s = self.input;
        let after = if s.starts_with(b"#VRML V") {
            7
        } else if s.starts_with(b"#X3D V") {
            6
        } else {
            return false;
        };

        let is_term = |b: u8| b == b'\n' || b == b'\r';
        let mut k = after;

        while k < s.len() && !is_term(s[k]) {
            if s[k..].starts_with(b" utf8") {
                let e = k + 5;
                let boundary = e >= s.len() || !(s[e].is_ascii_alphanumeric() || s[e] == b'_');

                if boundary {
                    if let Some(nl) = s[e..].iter().position(|b| is_term(*b)) {
                        let end = e + nl + 1;
                        let version = String::from_utf8_lossy(&s[after..k]).into_owned();
                        let matched = String::from_utf8_lossy(&s[..end]).into_owned();

                        self.last_index = end;
                        self.lines(&matched);
                        self.scene.specification_version = version;
                        return true;
                    }

                    return false;
                }
            }

            k += 1;
        }

        false
    }

    fn profile_statement(&mut self) -> R<()> {
        self.comments();

        if self.keyword("PROFILE") {
            if self.id() {
                let name = self.result.clone();

                let Some(profile) = self.scene.table.profile(&name).map(|p| p.name.clone()) else {
                    return self.err(format!("Profile '{name}' is not supported."));
                };

                self.scene.profile = Some(profile);
                self.scene.profile_statement = true;
                return Ok(());
            }

            return self.err("Expected a profile name.");
        }

        Ok(())
    }

    fn component_statements(&mut self) -> R<()> {
        while let Some((name, level)) = self.component_statement()? {
            self.scene.update_component(&name, level);
        }

        Ok(())
    }

    fn component_statement(&mut self) -> R<Option<(String, i32)>> {
        self.comments();

        if self.keyword("COMPONENT") {
            self.comments();

            if self.accept(lexer::component_name_id) {
                let name = self.result.clone();

                self.comments();

                if self.keyword(":") {
                    if self.int32() {
                        let Some(max) = self.scene.table.component(&name).map(|c| c.level) else {
                            let at = jsmath::to_int32(self.value_f64);
                            return self.err(format!(
                                "Component '{name}' at level '{at}' is not supported."
                            ));
                        };

                        // X3DBrowser.getComponent: Algorithm .clamp (level || component .level, 1, component .level).
                        let level = jsmath::to_int32(self.value_f64);
                        let level = if level == 0 { max } else { level }.clamp(1, max);

                        return Ok(Some((name, level)));
                    }

                    return self.err("Expected a component support level.");
                }

                return self.err("Expected a ':' after component name.");
            }

            return self.err("Expected a component name.");
        }

        Ok(None)
    }

    fn unit_statements(&mut self) -> R<()> {
        while self.unit_statement()? {}
        Ok(())
    }

    fn unit_statement(&mut self) -> R<bool> {
        self.comments();

        if self.keyword("UNIT") {
            if self.id() {
                let category = self.result.clone();

                if self.id() {
                    let name = self.result.clone();

                    if self.double() {
                        self.scene.update_unit(&category, &name, self.value_f64);
                        return Ok(true);
                    }

                    return self.err("Expected unit conversion factor.");
                }

                return self.err("Expected unit name identifier.");
            }

            return self.err("Expected category name identifier after UNIT statement.");
        }

        Ok(false)
    }

    fn meta_statements(&mut self) -> R<()> {
        while self.meta_statement()? {}
        Ok(())
    }

    fn meta_statement(&mut self) -> R<bool> {
        self.comments();

        if self.keyword("META") {
            if self.string() {
                let key = self.value_string.clone();

                if self.string() {
                    let value = self.value_string.clone();

                    // X3DScene.addMetaData
                    if key.is_empty() {
                        return self.err("Couldn't add metadata: name is empty.");
                    }

                    self.scene.meta.push((key, value));
                    return Ok(true);
                }

                return self.err("Expected metadata value.");
            }

            return self.err("Expected metadata key.");
        }

        Ok(false)
    }

    fn export_statement(&mut self) -> R<bool> {
        self.comments();

        if self.keyword("EXPORT") {
            if self.id() {
                let local_name = self.result.clone();
                let scene_ctx = ROOT;

                // this .getScene () .getLocalNode (name)
                let Some(node) = self.scene.get_local_node(scene_ctx, &local_name) else {
                    return self.err(format!("Unknown named or imported node '{local_name}'."));
                };

                self.comments();

                let exported_name = if self.keyword("AS") {
                    if self.id() {
                        self.result.clone()
                    } else {
                        return self.err("No name given after AS.");
                    }
                } else {
                    local_name.clone()
                };

                let mut description = String::new();

                self.comments();

                if self.keyword("DESCRIPTION") && self.string() {
                    description = self.value_string.clone();
                }

                // An existing export of that name moves to a unique name.
                if let Some(existing) = self.scene.get_exported_node(scene_ctx, &exported_name) {
                    let unique = self.scene.get_unique_export_name(&exported_name);
                    let _ = self
                        .scene
                        .update_exported_node(scene_ctx, &unique, existing, "");
                }

                if let Err(e) =
                    self.scene
                        .update_exported_node(scene_ctx, &exported_name, node, &description)
                {
                    return self.err(e);
                }

                return Ok(true);
            }

            return self.err("No name given after EXPORT.");
        }

        Ok(false)
    }

    fn import_statement(&mut self) -> R<bool> {
        self.comments();

        if self.keyword("IMPORT") {
            if self.id() {
                let inline_name = self.result.clone();
                let ctx = self.ctx();

                // this .getExecutionContext () .getNamedNode (name)
                let Some(inline) = self.scene.get_named_node(ctx, &inline_name) else {
                    return self.err(format!("Named node '{inline_name}' not found."));
                };

                self.comments();

                if self.keyword(".") {
                    if self.id() {
                        let exported_name = self.result.clone();

                        self.comments();

                        let imported_name = if self.keyword("AS") {
                            if self.id() {
                                self.result.clone()
                            } else {
                                return self.err("No name given after AS.");
                            }
                        } else {
                            exported_name.clone()
                        };

                        let mut description = String::new();

                        self.comments();

                        if self.keyword("DESCRIPTION") && self.string() {
                            description = self.value_string.clone();
                        }

                        // Rename existing node, then add the new imported node.
                        self.scene.rename_existing_node(ctx, &imported_name);

                        if let Err(e) = self.scene.add_imported_node(
                            ctx,
                            inline,
                            &exported_name,
                            &imported_name,
                            &description,
                        ) {
                            return self.err(e);
                        }

                        return Ok(true);
                    }

                    return self.err("Expected exported node name.");
                }

                return self.err("Expected a '.' after exported node name.");
            }

            return self.err("No name given after IMPORT statement.");
        }

        Ok(false)
    }

    fn statements(&mut self, field: &mut Vec<Option<NodeId>>) -> R<()> {
        while self.statement(field)? {}
        Ok(())
    }

    fn statement(&mut self, field: &mut Vec<Option<NodeId>>) -> R<bool> {
        if self.proto_statement()? {
            return Ok(true);
        }

        if self.route_statement()? {
            return Ok(true);
        }

        if self.import_statement()? {
            return Ok(true);
        }

        if self.export_statement()? {
            return Ok(true);
        }

        match self.node_statement()? {
            Stmt::Node(n) => {
                field.push(Some(n));
                Ok(true)
            }
            Stmt::Null => {
                field.push(None);
                Ok(true)
            }
            Stmt::No => Ok(false),
        }
    }

    fn node_statement(&mut self) -> R<Stmt> {
        self.comments();

        if self.keyword("DEF") {
            if self.id() {
                let name = self.result.clone();
                return self.node(&name);
            }

            return self.err("No name given after DEF.");
        }

        if self.keyword("USE") {
            if self.id() {
                let name = self.result.clone();
                let ctx = self.ctx();

                self.context_call_made(ContextCall::GetBrowser)?;

                if let Some(n) = self.scene.get_local_node(ctx, &name) {
                    return Ok(Stmt::Node(n));
                }

                // Forward reference: Placeholder.
                let list = self.placeholders.entry(ctx).or_default();

                if let Some((_, p)) = list.iter().find(|(n, _)| *n == name) {
                    return Ok(Stmt::Node(*p));
                }

                let p = self.scene.create_placeholder(ctx);
                self.placeholders.entry(ctx).or_default().push((name, p));
                return Ok(Stmt::Node(p));
            }

            return self.err("No name given after USE.");
        }

        if self.keyword("NULL") || self.keyword("null") {
            return Ok(Stmt::Null);
        }

        self.node("")
    }

    fn proto_statement(&mut self) -> R<bool> {
        if self.proto()? {
            return Ok(true);
        }

        if self.externproto()? {
            return Ok(true);
        }

        Ok(false)
    }

    fn proto_statements(&mut self) -> R<()> {
        while self.proto_statement()? {}
        Ok(())
    }

    fn proto(&mut self) -> R<bool> {
        self.comments();

        if self.keyword("PROTO") {
            if self.id() {
                let node_type_id = self.result.clone();

                self.comments();

                if self.keyword("[") {
                    let interface = self.interface_declarations()?;

                    self.comments();

                    if self.keyword("]") {
                        self.comments();

                        if self.keyword("{") {
                            let ctx = self.ctx();
                            self.context_call_made(ContextCall::GetBrowser)?;
                            let proto = self.scene.new_proto(ctx);

                            {
                                let p = &mut self.scene.protos[proto];
                                for field in interface {
                                    Scene::add_interface_field(
                                        &mut p.fields,
                                        &mut p.has_metadata,
                                        field,
                                    );
                                }
                            }

                            let body = self.scene.protos[proto].body;

                            self.contexts.push(body);

                            let mut roots = vec![];
                            let result = self.proto_body(&mut roots);

                            self.scene.ctx_mut(body).root_nodes = roots;
                            result?;
                            self.setup_nodes();
                            self.contexts.pop();

                            self.comments();

                            if self.keyword("}") {
                                if let Some(existing) =
                                    self.scene.ctx(ctx).protos.get(&node_type_id)
                                {
                                    self.warn(format!("A proto named '{node_type_id}' is already defined and will be overridden."));

                                    let unique =
                                        self.scene.get_unique_proto_name(ctx, &node_type_id);
                                    self.scene.update_proto_declaration(ctx, &unique, existing);
                                }

                                self.scene
                                    .update_proto_declaration(ctx, &node_type_id, proto);
                                return Ok(true);
                            }

                            return self.err("Expected a '}' at the end of PROTO body.");
                        }

                        return self.err("Expected a '{' at the beginning of PROTO body.");
                    }

                    return self.err("Expected a ']' at the end of PROTO interface declaration.");
                }

                return self.err("Expected a '[' at the beginning of PROTO interface declaration.");
            }

            return self.err("Invalid PROTO definition name.");
        }

        Ok(false)
    }

    fn proto_body(&mut self, roots: &mut Vec<Option<NodeId>>) -> R<()> {
        self.proto_statements()?;

        match self.root_node_statement()? {
            Stmt::Node(n) => roots.push(Some(n)),
            Stmt::Null => roots.push(None),
            Stmt::No => {}
        }

        self.statements(roots)
    }

    fn root_node_statement(&mut self) -> R<Stmt> {
        self.comments();

        if self.keyword("DEF") {
            if self.id() {
                let name = self.result.clone();
                let node = self.node(&name)?;

                if node != Stmt::No {
                    return Ok(node);
                }

                return self.err("Expected node type name after DEF.");
            }

            return self.err("No name given after DEF.");
        }

        self.node("")
    }

    fn interface_declarations(&mut self) -> R<Vec<Field>> {
        let mut out = vec![];

        while let Some(f) = self.interface_declaration()? {
            out.push(f);
        }

        Ok(out)
    }

    fn unknown_type_error<T>(&mut self) -> R<T> {
        self.id();
        let r = self.result.clone();
        self.err(format!("Unknown event or field type: '{r}'."))
    }

    /// Shared body of the interface declaration forms. `with_value` parses an initial value.
    fn declaration(&mut self, access_type: u8, with_value: bool) -> R<Option<Field>> {
        if self.field_type() {
            let ty = FieldType::from_name(&self.result).unwrap();

            if self.id() {
                let field_id = self.result.clone();
                let mut value = FieldValue::default_for(ty);

                if with_value {
                    match self.field_value(ty, &value)? {
                        Some(v) => value = v,
                        None => {
                            return self
                                .err(format!("Couldn't read value for field '{field_id}'."));
                        }
                    }
                }

                let mut field = Field::new(&field_id, access_type, value);
                field.modified = with_value;
                return Ok(Some(field));
            }

            return self.err("Expected a name for field.");
        }

        self.unknown_type_error()
    }

    fn restricted_interface_declaration(&mut self) -> R<Option<Field>> {
        self.comments();

        if self.keyword("inputOnly") || self.keyword("eventIn") {
            return self.declaration(access::INPUT_ONLY, false);
        }

        if self.keyword("outputOnly") || self.keyword("eventOut") {
            return self.declaration(access::OUTPUT_ONLY, false);
        }

        if self.keyword("initializeOnly") || self.keyword("field") {
            return self.declaration(access::INITIALIZE_ONLY, true);
        }

        Ok(None)
    }

    fn interface_declaration(&mut self) -> R<Option<Field>> {
        if let Some(f) = self.restricted_interface_declaration()? {
            return Ok(Some(f));
        }

        self.comments();

        if self.keyword("inputOutput") || self.keyword("exposedField") {
            return self.declaration(access::INPUT_OUTPUT, true);
        }

        Ok(None)
    }

    fn externproto(&mut self) -> R<bool> {
        self.comments();

        if self.keyword("EXTERNPROTO") {
            if self.id() {
                let node_type_id = self.result.clone();

                self.comments();

                if self.keyword("[") {
                    let interface = self.extern_interface_declarations()?;

                    self.comments();

                    if self.keyword("]") {
                        if let Some(FieldValue::MFString(url)) = self.mfstring_value()? {
                            let ctx = self.ctx();
                            self.context_call_made(ContextCall::GetBrowser)?;
                            let e = self.scene.new_extern_proto(ctx, url);

                            {
                                let p = &mut self.scene.externprotos[e];
                                for field in interface {
                                    Scene::add_interface_field(
                                        &mut p.fields,
                                        &mut p.has_metadata,
                                        field,
                                    );
                                }
                            }

                            if let Some(existing) =
                                self.scene.ctx(ctx).externprotos.get(&node_type_id)
                            {
                                self.warn(format!("A extern proto named '{node_type_id}' is already defined and will be overridden."));

                                let unique =
                                    self.scene.get_unique_extern_proto_name(ctx, &node_type_id);
                                self.scene
                                    .update_extern_proto_declaration(ctx, &unique, existing);
                            }

                            self.scene
                                .update_extern_proto_declaration(ctx, &node_type_id, e);
                            return Ok(true);
                        }

                        return self.err(format!("Expected a URL list after EXTERNPROTO interface declaration '{node_type_id}'."));
                    }

                    return self
                        .err("Expected a ']' at the end of EXTERNPROTO interface declaration.");
                }

                return self
                    .err("Expected a '[' at the beginning of EXTERNPROTO interface declaration.");
            }

            return self.err("Invalid EXTERNPROTO definition name.");
        }

        Ok(false)
    }

    fn extern_interface_declarations(&mut self) -> R<Vec<Field>> {
        let mut out = vec![];

        while let Some(f) = self.extern_interface_declaration()? {
            out.push(f);
        }

        Ok(out)
    }

    fn extern_interface_declaration(&mut self) -> R<Option<Field>> {
        self.comments();

        if self.keyword("inputOnly") || self.keyword("eventIn") {
            return self.declaration(access::INPUT_ONLY, false);
        }

        if self.keyword("outputOnly") || self.keyword("eventOut") {
            return self.declaration(access::OUTPUT_ONLY, false);
        }

        if self.keyword("initializeOnly") || self.keyword("field") {
            return self.declaration(access::INITIALIZE_ONLY, false);
        }

        if self.keyword("inputOutput") || self.keyword("exposedField") {
            return self.declaration(access::INPUT_OUTPUT, false);
        }

        Ok(None)
    }

    fn route_statement(&mut self) -> R<bool> {
        self.comments();

        if self.keyword("ROUTE") {
            if self.id() {
                let from_node_id = self.result.clone();

                self.comments();

                if self.keyword(".") {
                    if self.id() {
                        let event_out_id = self.result.clone();

                        self.comments();

                        if self.keyword("TO") {
                            if self.id() {
                                let to_node_id = self.result.clone();

                                self.comments();

                                if self.keyword(".") {
                                    if self.id() {
                                        let event_in_id = self.result.clone();
                                        let ctx = self.ctx();
                                        let line = self.line_number;

                                        // getLocalNode: a named node of this context or an imported node.
                                        let from = self.scene.get_local_node(ctx, &from_node_id);
                                        let to = self.scene.get_local_node(ctx, &to_node_id);

                                        let result = match (from, to) {
                                            _ if self.no_context => Err(
                                                "TypeError: Cannot read properties of undefined (reading 'getLocalNode')"
                                                    .to_string(),
                                            ),
                                            (None, _) => Err(format!(
                                                "Unknown named or imported node '{from_node_id}'."
                                            )),
                                            (_, None) => Err(format!(
                                                "Unknown named or imported node '{to_node_id}'."
                                            )),
                                            (Some(f), Some(t)) => self.scene.add_route(
                                                ctx,
                                                f,
                                                &event_out_id,
                                                t,
                                                &event_in_id,
                                            ),
                                        };

                                        // console .warn (`Parser error at line ${n}.`, error): the
                                        // dump prints the Error argument as String (error).
                                        if let Err(e) = result {
                                            let e = if e.starts_with("TypeError: ") {
                                                e
                                            } else {
                                                format!("Error: {e}")
                                            };

                                            self.warn(format!("Parser error at line {line}. {e}"));
                                        }

                                        return Ok(true);
                                    }

                                    return self
                                        .err("Bad ROUTE specification: Expected a field name.");
                                }

                                return self.err(
                                    "Bad ROUTE specification: Expected a '.' after node name.",
                                );
                            }

                            return self.err("Bad ROUTE specification: Expected a node name.");
                        }

                        return self.err("Bad ROUTE specification: Expected a 'TO'.");
                    }

                    return self.err("Bad ROUTE specification: Expected a field name.");
                }

                return self.err("Bad ROUTE specification: Expected a '.' after node name.");
            }

            return self.err("Bad ROUTE specification: Expected a node name.");
        }

        Ok(false)
    }

    fn node(&mut self, node_name_id: &str) -> R<Stmt> {
        if !self.id() {
            return Ok(Stmt::No);
        }

        let node_type_id = self.result.clone();
        let ctx = self.ctx();

        self.context_call_made(ContextCall::CreateNode)?;

        let base_node = match self.scene.create_node(ctx, &node_type_id) {
            Some(n) => Some(n),
            None => self.scene.create_proto_instance(ctx, &node_type_id),
        };

        let Some(base_node) = base_node else {
            // Parse unknown node into a throw-away WorldInfo.
            if self.unknown_level == 0 {
                let line = self.line_number;
                self.warn(format!(
                    "Parser error at line {line}: Unknown node type or proto '{node_type_id}'. You probably have insufficient component/profile statements, and/or an inappropriate specification version."
                ));
            }

            self.comments();

            if self.keyword("{") {
                self.unknown_level += 1;

                let world_info = self.scene.create_node(ctx, "WorldInfo").expect("WorldInfo");
                let result = self.node_body(world_info);

                self.unknown_level -= 1;
                result?;

                self.comments();

                if self.keyword("}") {
                    return Ok(Stmt::Null);
                }
            }

            return Ok(Stmt::No);
        };

        if !node_name_id.is_empty() {
            self.named_nodes
                .entry(ctx)
                .or_default()
                .entry(node_name_id.to_string())
                .or_insert(base_node);
            self.scene.rename_existing_node(ctx, node_name_id);
            self.scene.update_named_node(ctx, node_name_id, base_node);
        }

        self.comments();

        if self.keyword("{") {
            if self.scene.node(base_node).can_user_defined_fields {
                self.script_body(base_node)?;
            } else {
                self.node_body(base_node)?;
            }

            self.comments();

            if self.keyword("}") {
                if !self.is_inside_proto_declaration() {
                    self.scene.setup_order.push(base_node);
                }

                return Ok(Stmt::Node(base_node));
            }

            return self.err("Expected '}' at the end of node body.");
        }

        self.err("Expected '{' at the beginning of node body.")
    }

    fn script_body(&mut self, node: NodeId) -> R<()> {
        while self.script_body_element(node)? {}
        Ok(())
    }

    fn access_type_of(name: &str) -> Option<u8> {
        Some(match name {
            "field" | "initializeOnly" => access::INITIALIZE_ONLY,
            "eventIn" | "inputOnly" => access::INPUT_ONLY,
            "eventOut" | "outputOnly" => access::OUTPUT_ONLY,
            "exposedField" | "inputOutput" => access::INPUT_OUTPUT,
            _ => return None,
        })
    }

    pub(crate) fn proto_field_lookup(&self, proto: ProtoId, name: &str) -> Option<usize> {
        let fields = &self.scene.protos[proto].fields;
        let direct = |n: &str| fields.iter().position(|f| f.name == n);

        if let Some(i) = direct(name) {
            return Some(i);
        }

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

    fn script_body_element(&mut self, node: NodeId) -> R<bool> {
        let (last_index, line_number) = (self.last_index, self.line_number);

        if self.id() {
            if let Some(access_type) = Self::access_type_of(&self.result) {
                if self.field_type() {
                    let ty = FieldType::from_name(&self.result).unwrap();

                    if self.id() {
                        let field_id = self.result.clone();

                        self.comments();

                        if self.keyword("IS") {
                            let Some(proto) = self.outer_proto() else {
                                return self.err("IS statement outside PROTO definition.");
                            };

                            if !self.id() {
                                return self.err("No name give after IS statement.");
                            }

                            let is_id = self.result.clone();

                            let Some(r) = self.proto_field_lookup(proto, &is_id) else {
                                let line = self.line_number;
                                let outer = self.outer_name();
                                self.warn(format!("Parser error at line {line}: No such event or field '{is_id}' inside PROTO {outer} interface declaration."));
                                return Ok(true);
                            };

                            let reference = self.scene.protos[proto].fields[r].clone();

                            if ty != reference.ty() {
                                let outer = self.outer_name();
                                return self.err(format!("Field '{field_id}' and '{}' in PROTO '{outer}' have different types.", reference.name));
                            }

                            if !reference.is_reference(access_type) {
                                let outer = self.outer_name();
                                return self.err(format!("Field '{field_id}' and '{}' in PROTO '{outer}' are incompatible as an IS mapping.", reference.name));
                            }

                            let index = match self.scene.get_user_defined_field(node, &field_id) {
                                Some(i) => i,
                                None => self.scene.add_user_defined_field(
                                    node,
                                    Field::new(&field_id, access_type, FieldValue::default_for(ty)),
                                ),
                            };

                            let field = &mut self.scene.node_mut(node).user_fields[index];

                            if access_type == field.access && reference.ty() == field.ty() {
                                if !field.references.contains(&FieldRef { proto, index: r }) {
                                    field.add_reference(FieldRef { proto, index: r });

                                    // X3DField.addReference: initializeOnly / inputOutput relations
                                    // process an event from the reference (copy its value).
                                    if matches!(
                                        field.access & reference.access,
                                        access::INITIALIZE_ONLY | access::INPUT_OUTPUT
                                    ) {
                                        field.value = reference.value.clone();
                                    }
                                }

                                return Ok(true);
                            }

                            return self.err(format!("Couldn't add field '{field_id}', field already exists with different access type or data type."));
                        }
                    }
                }
            }
        }

        self.last_index = last_index;
        self.line_number = line_number;

        if let Some(field) = self.interface_declaration()? {
            if let Some(i) = self.scene.get_user_defined_field(node, &field.name) {
                let existing = &mut self.scene.node_mut(node).user_fields[i];

                if existing.access == field.access && existing.ty() == field.ty() {
                    existing.value = field.value;
                    existing.modified = true;
                    return Ok(true);
                }

                return self.err(format!("Couldn't set value for field '{}', field already exists with different access type or data type.", field.name));
            }

            self.scene.add_user_defined_field(node, field);
            return Ok(true);
        }

        self.node_body_element(node)
    }

    fn node_body(&mut self, node: NodeId) -> R<()> {
        while self.node_body_element(node)? {}
        Ok(())
    }

    fn node_body_element(&mut self, node: NodeId) -> R<bool> {
        if self.proto_statement()? {
            return Ok(true);
        }

        if self.route_statement()? {
            return Ok(true);
        }

        if !self.id() {
            return Ok(false);
        }

        let field_id = self.result.clone();

        let Some(index) = self.scene.get_predefined_field(node, &field_id) else {
            // An alias of a private child object (GeoViewpoint navType and headlight up to 3.2).
            if let Some(p) = self.scene.child_alias(node, &field_id) {
                self.comments();

                let field = &self.scene.node(node).private_fields[p];
                let (ty, current) = (field.ty(), field.value.clone());

                if let Some(v) = self.field_value(ty, &current)? {
                    let f = &mut self.scene.node_mut(node).private_fields[p];
                    f.value = v;
                    f.modified = true;
                    return Ok(true);
                }

                return self.err(format!("Couldn't read value for field '{field_id}'."));
            }

            // Parse unknown field value.
            let (last_index, line_number) = (self.last_index, self.line_number);

            if self.unknown_value()? {
                if self.unknown_level == 0 {
                    let type_name = self.scene.type_name(node);
                    self.warn(format!("Parser error at line {line_number}: Unknown field '{field_id}' in class '{type_name}'."));
                }

                return Ok(true);
            }

            self.last_index = last_index;
            self.line_number = line_number;

            let type_name = self.scene.type_name(node);
            return self.err(format!(
                "Unknown field '{field_id}' in class '{type_name}'."
            ));
        };

        self.comments();

        if self.keyword("IS") {
            let Some(proto) = self.outer_proto() else {
                return self.err("IS statement outside PROTO definition.");
            };

            if !self.id() {
                return self.err("No name give after IS statement.");
            }

            let is_id = self.result.clone();

            let Some(r) = self.proto_field_lookup(proto, &is_id) else {
                let line = self.line_number;
                let outer = self.outer_name();
                self.warn(format!("Parser error at line {line}: No such event or field '{is_id}' inside PROTO {outer}"));
                return Ok(true);
            };

            let reference = self.scene.protos[proto].fields[r].clone();
            let field = &self.scene.node(node).fields[index];

            if field.ty() == reference.ty() {
                if reference.is_reference(field.access) {
                    let f = &mut self.scene.node_mut(node).fields[index];

                    if !f.references.contains(&FieldRef { proto, index: r }) {
                        f.add_reference(FieldRef { proto, index: r });

                        // X3DField.addReference: processEvent (Events .from (reference)).
                        if matches!(
                            f.access & reference.access,
                            access::INITIALIZE_ONLY | access::INPUT_OUTPUT
                        ) {
                            f.value = reference.value.clone();
                        }
                    }

                    return Ok(true);
                }

                let outer = self.outer_name();
                return self.err(format!(
                    "Field '{}' and '{}' in PROTO {outer} are incompatible as an IS mapping.",
                    field.name, reference.name
                ));
            }

            let outer = self.outer_name();
            return self.err(format!(
                "Field '{}' and '{}' in PROTO {outer} have different types.",
                field.name, reference.name
            ));
        }

        let field = &self.scene.node(node).fields[index];

        if field.is_initializable() {
            let (ty, current) = (field.ty(), field.value.clone());
            let field_name = field.name.clone();

            self.field_unit = self.field_unit_of(node, &field_name);

            let parsed = self.field_value(ty, &current);
            self.field_unit = None;

            if let Some(v) = parsed? {
                let f = &mut self.scene.node_mut(node).fields[index];
                f.value = v;
                f.modified = true;
                return Ok(true);
            }

            return self.err(format!("Couldn't read value for field '{field_id}'."));
        }

        // Parse value of a inputOnly or outputOnly, and output a warning.
        let access_type = field.access;

        if !self.unknown_value()? {
            return self.err(format!("Couldn't read value for field '{field_id}'."));
        }

        let line = self.line_number;
        self.warn(format!(
            "Parser error at line {line}: Couldn't assign value to {} field '{field_id}'.",
            access::name(access_type)
        ));

        Ok(true)
    }

    /// X3DField.getUnit of a predefined field of `node` (only builtin node types define units).
    pub(crate) fn field_unit_of(&self, node: NodeId, field_name: &str) -> Option<String> {
        match self.scene.node(node).kind {
            NodeKind::Builtin(t) => {
                let table = self.scene.table;
                let spec = &self.scene.specification_version;
                table.types[t]
                    .variant(spec)
                    .fields
                    .iter()
                    .find(|d| d.name == field_name)
                    .and_then(|d| d.unit.clone())
            }
            _ => None,
        }
    }

    // --- terminals ---------------------------------------------------------------------------

    fn id(&mut self) -> bool {
        self.comments();
        self.accept(lexer::id)
    }

    fn field_type(&mut self) -> bool {
        self.comments();
        self.accept(lexer::field_type)
    }

    fn bool_(&mut self) -> Option<bool> {
        self.comments();

        if self.keyword("TRUE") || self.keyword("true") {
            return Some(true);
        }

        if self.keyword("FALSE") || self.keyword("false") {
            return Some(false);
        }

        None
    }

    fn double(&mut self) -> bool {
        self.comments();

        if self.accept(lexer::double) {
            self.value_f64 = lexer::parse_float(&self.result);
            return true;
        }

        if let Some((end, negative, name)) = lexer::constant(self.input, self.last_index) {
            self.result = String::from_utf8_lossy(&self.input[self.last_index..end]).into_owned();
            self.last_index = end;

            let v = constant_value(&name);
            self.value_f64 = if negative { -v } else { v };
            return true;
        }

        false
    }

    fn int32(&mut self) -> bool {
        self.comments();

        if self.accept(lexer::int32) {
            self.value_f64 = lexer::parse_int(&self.result);
            return true;
        }

        false
    }

    /// VRMLParser.string: handles `\"` by checking for an odd number of trailing backslashes.
    pub(crate) fn string(&mut self) -> bool {
        self.comments();

        if self.input.get(self.last_index) != Some(&b'"') {
            return false;
        }

        self.last_index += 1;

        let mut value: Vec<u8> = vec![];

        loop {
            let start = self.last_index;
            let mut end = start;

            while end < self.input.len() && self.input[end] != b'"' {
                end += 1;
            }

            if end == start {
                break;
            }

            value.extend_from_slice(&self.input[start..end]);
            self.last_index = end;

            let trailing = value.iter().rev().take_while(|b| **b == b'\\').count();

            if trailing == 0 || trailing % 2 == 0 {
                break;
            }

            if self.input.get(self.last_index) == Some(&b'"') {
                self.last_index += 1;
            }

            value.push(b'"');
        }

        if self.input.get(self.last_index) == Some(&b'"') {
            self.last_index += 1;

            let text = String::from_utf8_lossy(&value).into_owned();

            self.lines(&text);
            self.value_string = unescape(&text);
            return true;
        }

        false
    }

    // --- field values ----------------------------------------------------------------------

    /// VRMLParser.fieldValue: `Some (new value)` or `None` (the JS `false`).
    /// X3DParser.fromUnit (category, value) with this scene's units.
    fn from_unit(&self, category: &str, value: f64) -> f64 {
        let factor = x3d_scene::scene::unit_factor(&self.scene.units, category);
        if factor == 1.0 { value } else { value * factor }
    }

    /// fromUnit (field .getUnit (), value) for every number of the field being read.
    fn field_units(&self, mut v: Vec<f64>) -> Vec<f64> {
        if let Some(u) = &self.field_unit {
            let factor = x3d_scene::scene::unit_factor(&self.scene.units, u);
            if factor != 1.0 {
                for x in v.iter_mut() {
                    *x *= factor;
                }
            }
        }
        v
    }

    fn field_unit_value(&self, value: f64) -> f64 {
        match &self.field_unit {
            Some(u) => self.from_unit(u, value),
            None => value,
        }
    }

    pub(crate) fn field_value(
        &mut self,
        ty: FieldType,
        current: &FieldValue,
    ) -> R<Option<FieldValue>> {
        use FieldType as T;
        use FieldValue as V;

        Ok(match ty {
            T::SFBool => self.bool_().map(V::SFBool),
            T::SFColor => self
                .sfcolor_value()?
                .map(|c| V::SFColor(Color3::new(c[0], c[1], c[2]))),
            T::SFColorRGBA => self
                .sfcolorrgba_value()?
                .map(|c| V::SFColorRGBA(Color4::new(c[0], c[1], c[2], c[3]))),
            T::SFDouble => self
                .double()
                .then(|| V::SFDouble(self.field_unit_value(self.value_f64))),
            T::SFFloat => self
                .double()
                .then(|| V::SFFloat(self.field_unit_value(self.value_f64))),
            T::SFTime => self
                .double()
                .then(|| V::SFTime(self.field_unit_value(self.value_f64))),
            T::SFImage => self.sfimage_value().map(V::SFImage),
            T::SFInt32 => self
                .int32()
                .then(|| V::SFInt32(jsmath::to_int32(self.value_f64))),
            T::SFMatrix3d | T::SFMatrix3f => self.doubles_exact(9).map(|v| {
                let m = Matrix3(v.try_into().unwrap());
                if ty == T::SFMatrix3d {
                    V::SFMatrix3d(m)
                } else {
                    V::SFMatrix3f(m)
                }
            }),
            T::SFMatrix4d | T::SFMatrix4f => self.doubles_exact(16).map(|v| {
                let m = Matrix4(v.try_into().unwrap());
                if ty == T::SFMatrix4d {
                    V::SFMatrix4d(m)
                } else {
                    V::SFMatrix4f(m)
                }
            }),
            T::SFNode => match self.node_statement()? {
                Stmt::Node(n) => Some(V::SFNode(Some(n))),
                Stmt::Null => Some(V::SFNode(None)),
                Stmt::No => None,
            },
            T::SFQuaternion => self
                .doubles_exact(4)
                .map(|v| V::SFQuaternion(Quaternion::new(v[0], v[1], v[2], v[3]))),
            T::SFRotation => self.doubles_exact(4).map(|v| {
                // Each component setter calls Rotation4.set with the raw values.
                let mut r = match current {
                    V::SFRotation(r) => *r,
                    _ => Rotation4::identity(),
                };
                r.set_x(v[0]);
                r.set_y(v[1]);
                r.set_z(v[2]);
                r.set_angle(self.from_unit("angle", v[3]));
                V::SFRotation(r)
            }),
            T::SFString => self
                .string()
                .then(|| V::SFString(self.value_string.clone())),
            T::SFVec2d | T::SFVec2f => self.doubles_exact(2).map(|v| {
                let v = Vector2::from_slice(&self.field_units(v));
                if ty == T::SFVec2d {
                    V::SFVec2d(v)
                } else {
                    V::SFVec2f(v)
                }
            }),
            T::SFVec3d | T::SFVec3f => self.doubles_exact(3).map(|v| {
                let v = Vector3::from_slice(&self.field_units(v));
                if ty == T::SFVec3d {
                    V::SFVec3d(v)
                } else {
                    V::SFVec3f(v)
                }
            }),
            T::SFVec4d | T::SFVec4f => self.doubles_exact(4).map(|v| {
                let v = Vector4::from_slice(&self.field_units(v));
                if ty == T::SFVec4d {
                    V::SFVec4d(v)
                } else {
                    V::SFVec4f(v)
                }
            }),
            T::MFNode => self.mfnode_value()?,
            T::MFString => self.mfstring_value()?,
            T::MFBool => self.mfbool_value()?,
            T::MFImage => self.mfimage_value()?,
            T::MFColor => self.mf_typed(
                ty,
                3,
                |p| p.sfcolor_value().map(|c| c.map(|c| c[..3].to_vec())),
                true,
            )?,
            T::MFColorRGBA => self.mf_typed(
                ty,
                4,
                |p| p.sfcolorrgba_value().map(|c| c.map(|c| c.to_vec())),
                true,
            )?,
            T::MFInt32 => self.mf_int32()?,
            T::MFDouble | T::MFFloat | T::MFTime => {
                self.mf_typed(ty, 1, |p| Ok(p.double().then(|| vec![p.value_f64])), false)?
            }
            T::MFMatrix3d | T::MFMatrix3f => {
                self.mf_typed(ty, 9, |p| Ok(p.doubles_exact(9)), false)?
            }
            T::MFMatrix4d | T::MFMatrix4f => {
                self.mf_typed(ty, 16, |p| Ok(p.doubles_exact(16)), false)?
            }
            T::MFQuaternion => self.mf_typed(ty, 4, |p| Ok(p.doubles_exact(4)), false)?,
            T::MFRotation => self.mf_typed(ty, 4, |p| Ok(p.doubles_exact(4)), false)?,
            T::MFVec2d | T::MFVec2f => self.mf_typed(ty, 2, |p| Ok(p.doubles_exact(2)), false)?,
            T::MFVec3d | T::MFVec3f => self.mf_typed(ty, 3, |p| Ok(p.doubles_exact(3)), false)?,
            T::MFVec4d | T::MFVec4f => self.mf_typed(ty, 4, |p| Ok(p.doubles_exact(4)), false)?,
        })
    }

    /// `n` consecutive doubles (sfvec*Value, sfrotationValue, sfmatrix*Value). Like X_ITE,
    /// consumed input is *not* restored on failure.
    fn doubles_exact(&mut self, n: usize) -> Option<Vec<f64>> {
        let mut v = Vec::with_capacity(n);

        for _ in 0..n {
            if !self.double() {
                return None;
            }
            v.push(self.value_f64);
        }

        Some(v)
    }

    /// sfcolorValue: three doubles, else an HTML/CSS color. Returns raw (unclamped) values; the
    /// caller's Color3 clamps.
    fn sfcolor_value(&mut self) -> R<Option<[f64; 4]>> {
        let last_index = self.last_index;

        if let Some(v) = self.doubles_exact(3) {
            return Ok(Some([v[0], v[1], v[2], 1.0]));
        }

        self.last_index = last_index;
        self.html_color()
    }

    fn sfcolorrgba_value(&mut self) -> R<Option<[f64; 4]>> {
        let last_index = self.last_index;

        if let Some(v) = self.doubles_exact(4) {
            return Ok(Some([v[0], v[1], v[2], v[3]]));
        }

        self.last_index = last_index;
        self.html_color()
    }

    fn html_color(&mut self) -> R<Option<[f64; 4]>> {
        self.comments();

        if self.accept(lexer::html_color) {
            let text = self.result.clone();
            let css = if text.len() >= 2 && (text.starts_with("0x") || text.starts_with("0X")) {
                format!("#{}", &text[2..])
            } else {
                text.clone()
            };
            let color = convert_color(&css);

            // X3DParser.checkColorName
            if color[..3].iter().all(|v| *v == 1.0)
                && color[3] == 1.0
                && !text.eq_ignore_ascii_case("white")
                && text.bytes().all(|b| b.is_ascii_alphabetic())
            {
                return self.err("Invalid color name.");
            }

            return Ok(Some(color));
        }

        Ok(None)
    }

    fn sfimage_value(&mut self) -> Option<Image> {
        if !self.int32() {
            return None;
        }
        let width = self.value_f64;

        if !self.int32() {
            return None;
        }
        let height = self.value_f64;

        if !self.int32() {
            return None;
        }
        let comp = self.value_f64;

        let size = width * height;
        let mut array = vec![];
        let mut i = 0.0;

        while i < size {
            if !self.int32() {
                return None;
            }
            array.push(jsmath::to_int32(self.value_f64));
            i += 1.0;
        }

        Some(Image::new(width, height, comp, array))
    }

    /// Typed-array MF fields: single value without brackets (pushed through the SF temporary,
    /// which clamps colors), or a bracketed flat list of numbers / values.
    fn mf_typed(
        &mut self,
        ty: FieldType,
        components: usize,
        single: impl Fn(&mut Self) -> R<Option<Vec<f64>>>,
        colors: bool,
    ) -> R<Option<FieldValue>> {
        if let Some(v) = single(self)? {
            let mut value = FieldValue::default_for(ty);
            let v = if colors {
                v.iter().map(|x| jsmath::clamp(*x, 0.0, 1.0)).collect()
            } else {
                v
            };
            let v = self.mf_units(ty, colors, v);
            value.push_numbers(&v[..components]);
            return Ok(Some(value));
        }

        if self.keyword("[") {
            let value = self.typed_values(ty, components, colors)?;

            if self.keyword("]") {
                return Ok(Some(value));
            }

            return self.err("Expected ']'.");
        }

        Ok(None)
    }

    /// fromUnit: each rotation's angle, or every number of a field with a unit.
    fn mf_units(&self, ty: FieldType, colors: bool, mut v: Vec<f64>) -> Vec<f64> {
        if ty == FieldType::MFRotation {
            for a in v.iter_mut().skip(3).step_by(4) {
                *a = self.from_unit("angle", *a);
            }
            v
        } else if colors {
            v
        } else {
            self.field_units(v)
        }
    }

    /// The value list between the brackets of a typed-array MF field (sfcolorValues,
    /// sfdoubleValues, sfvecValues, sfmatrixValues, sfquaternionValues, sfrotationValues).
    fn typed_values(&mut self, ty: FieldType, components: usize, colors: bool) -> R<FieldValue> {
        let mut value = FieldValue::default_for(ty);
        let mut array = vec![];

        if colors {
            // sfcolorValues: single doubles, or whole colors (incl. HTML colors) pushed as components.
            loop {
                if self.double() {
                    array.push(self.value_f64);
                    continue;
                }

                let color = if components == 3 {
                    self.sfcolor_value()?
                } else {
                    self.sfcolorrgba_value()?
                };

                match color {
                    Some(c) => {
                        for k in 0..components {
                            array.push(jsmath::clamp(c[k], 0.0, 1.0));
                        }
                    }
                    None => break,
                }
            }
        } else {
            // sfdoubleValues / sfvecValues / sfmatrixValues, and for quaternions/rotations the
            // element-wise loops, which consume the same doubles.
            while self.double() {
                array.push(self.value_f64);
            }

            if matches!(ty, FieldType::MFRotation | FieldType::MFQuaternion) {
                let n = array.len() - array.len() % 4;
                array.truncate(n);
            }
        }

        // `field .setValue (array)` (sfcolorValues, sfvecValues, sfmatrixValues): X3DTypedArrayField.set warns and drops an
        // incomplete last element.
        if array.len() % components != 0
            && !matches!(ty, FieldType::MFRotation | FieldType::MFQuaternion)
        {
            self.warn(format!(
                "Array length must be multiple of components size, which is {components}."
            ));
        }

        let array = self.mf_units(ty, colors, array);
        value.set_flat_numbers(&array);
        Ok(value)
    }

    /// The sf…Values functions the XML parser reads an MF attribute with: the value list without
    /// brackets. The field is always set to what was read, even nothing. An MFNode attribute
    /// clears the field.
    pub(crate) fn sf_values(&mut self, ty: FieldType) -> R<FieldValue> {
        use FieldType as T;

        Ok(match ty {
            T::MFNode => FieldValue::MFNode(vec![]),
            T::MFString => FieldValue::MFString(self.string_values()),
            T::MFBool => FieldValue::MFBool(self.bool_values()),
            T::MFImage => FieldValue::MFImage(self.image_values()),
            T::MFInt32 => FieldValue::MFInt32(self.int32_values()),
            T::MFColor => self.typed_values(ty, 3, true)?,
            T::MFColorRGBA => self.typed_values(ty, 4, true)?,
            T::MFDouble | T::MFFloat | T::MFTime => self.typed_values(ty, 1, false)?,
            T::MFMatrix3d | T::MFMatrix3f => self.typed_values(ty, 9, false)?,
            T::MFMatrix4d | T::MFMatrix4f => self.typed_values(ty, 16, false)?,
            T::MFQuaternion | T::MFRotation => self.typed_values(ty, 4, false)?,
            T::MFVec2d | T::MFVec2f => self.typed_values(ty, 2, false)?,
            T::MFVec3d | T::MFVec3f => self.typed_values(ty, 3, false)?,
            T::MFVec4d | T::MFVec4f => self.typed_values(ty, 4, false)?,
            _ => unreachable!("sf_values on an SF type"),
        })
    }

    fn int32_values(&mut self) -> Vec<i32> {
        let mut array = vec![];

        while self.int32() {
            array.push(jsmath::to_int32(self.value_f64));
        }

        array
    }

    fn bool_values(&mut self) -> Vec<bool> {
        let mut array = vec![];

        while let Some(b) = self.bool_() {
            array.push(b);
        }

        array
    }

    pub(crate) fn string_values(&mut self) -> Vec<String> {
        let mut array = vec![];

        while self.string() {
            array.push(self.value_string.clone());
        }

        array
    }

    fn image_values(&mut self) -> Vec<Image> {
        let mut array = vec![];

        while let Some(i) = self.sfimage_value() {
            array.push(i);
        }

        array
    }

    fn mf_int32(&mut self) -> R<Option<FieldValue>> {
        if self.int32() {
            return Ok(Some(FieldValue::MFInt32(vec![jsmath::to_int32(
                self.value_f64,
            )])));
        }

        if self.keyword("[") {
            let array = self.int32_values();

            if self.keyword("]") {
                return Ok(Some(FieldValue::MFInt32(array)));
            }

            return self.err("Expected ']'.");
        }

        Ok(None)
    }

    fn mfbool_value(&mut self) -> R<Option<FieldValue>> {
        if let Some(b) = self.bool_() {
            return Ok(Some(FieldValue::MFBool(vec![b])));
        }

        if self.keyword("[") {
            let array = self.bool_values();

            if self.keyword("]") {
                return Ok(Some(FieldValue::MFBool(array)));
            }

            return self.err("Expected ']'.");
        }

        Ok(None)
    }

    fn mfstring_value(&mut self) -> R<Option<FieldValue>> {
        if self.string() {
            return Ok(Some(FieldValue::MFString(vec![self.value_string.clone()])));
        }

        if self.keyword("[") {
            let array = self.string_values();

            if self.keyword("]") {
                return Ok(Some(FieldValue::MFString(array)));
            }

            return self.err("Expected ']'.");
        }

        Ok(None)
    }

    fn mfimage_value(&mut self) -> R<Option<FieldValue>> {
        if let Some(i) = self.sfimage_value() {
            return Ok(Some(FieldValue::MFImage(vec![i])));
        }

        if self.keyword("[") {
            let array = self.image_values();

            if self.keyword("]") {
                return Ok(Some(FieldValue::MFImage(array)));
            }

            return self.err("Expected ']'.");
        }

        Ok(None)
    }

    fn mfnode_value(&mut self) -> R<Option<FieldValue>> {
        match self.node_statement()? {
            Stmt::Node(n) => return Ok(Some(FieldValue::MFNode(vec![Some(n)]))),
            Stmt::Null => return Ok(Some(FieldValue::MFNode(vec![None]))),
            Stmt::No => {}
        }

        if self.keyword("[") {
            let mut nodes = vec![];

            self.statements(&mut nodes)?;

            if self.keyword("]") {
                return Ok(Some(FieldValue::MFNode(nodes)));
            }

            return self.err("Expected ']'.");
        }

        Ok(None)
    }

    // --- unknown values ----------------------------------------------------------------------

    fn unknown_value(&mut self) -> R<bool> {
        self.unknown_level += 1;
        let result = self.unknown_value_inner();
        self.unknown_level -= 1;
        result
    }

    fn unknown_value_inner(&mut self) -> R<bool> {
        self.comments();

        if self.keyword("IS") && self.is_inside_proto_declaration() && self.id() {
            return Ok(true);
        }

        self.mfunknown_value()
    }

    fn sfunknown_value(&mut self) -> R<bool> {
        if self.bool_().is_some() {
            return Ok(true);
        }

        // sfdoubleValues (new MFFloat): as many doubles as there are.
        let mut any = false;

        while self.double() {
            any = true;
        }

        if any {
            return Ok(true);
        }

        if self.string() {
            return Ok(true);
        }

        Ok(self.node_statement()? != Stmt::No)
    }

    fn mfunknown_value(&mut self) -> R<bool> {
        if self.sfunknown_value()? {
            return Ok(true);
        }

        if self.keyword("[") {
            while self.sfunknown_value()? {}

            if self.keyword("]") {
                return Ok(true);
            }

            return self.err("Expected ']'.");
        }

        Ok(false)
    }

    // --- placeholders ------------------------------------------------------------------------

    /// X3DParser.setupNodes: resolve forward USEs of the current execution context.
    pub(crate) fn setup_nodes(&mut self) {
        let ctx = self.ctx();
        let Some(list) = self.placeholders.remove(&ctx) else {
            return;
        };

        for (name, placeholder) in list {
            let target = self
                .named_nodes
                .get(&ctx)
                .and_then(|m| m.get(&name))
                .copied()
                .or_else(|| {
                    self.imported_names
                        .get(&ctx)
                        .and_then(|m| m.get(&name))
                        .copied()
                });

            match target {
                Some(node) => self.replace_node_references(placeholder, node),
                None => {
                    self.warn(format!(
                        "{}: Named or imported node '{name}' not found.",
                        self.name
                    ));
                    self.dispose_placeholder(placeholder);
                }
            }
        }
    }

    /// Placeholder.replaceWithNode → this .dispose () → X3DNode.dispose: removed from every
    /// MFNode (including root nodes), SFNode parents set to NULL.
    fn dispose_placeholder(&mut self, p: NodeId) {
        let remove = |v: &mut FieldValue| match v {
            FieldValue::SFNode(Some(n)) if *n == p => *v = FieldValue::SFNode(None),
            FieldValue::MFNode(list) => list.retain(|n| *n != Some(p)),
            _ => {}
        };

        for node in &mut self.scene.nodes {
            for f in node.fields.iter_mut().chain(node.user_fields.iter_mut()) {
                remove(&mut f.value);
            }
        }

        for proto in &mut self.scene.protos {
            for f in &mut proto.fields {
                remove(&mut f.value);
            }
        }

        for c in &mut self.scene.contexts {
            c.root_nodes.retain(|n| *n != Some(p));
        }
    }

    fn replace_node_references(&mut self, from: NodeId, to: NodeId) {
        let replace = |v: &mut FieldValue| match v {
            FieldValue::SFNode(Some(n)) if *n == from => *n = to,
            FieldValue::MFNode(list) => {
                for n in list.iter_mut().flatten() {
                    if *n == from {
                        *n = to;
                    }
                }
            }
            _ => {}
        };

        for node in &mut self.scene.nodes {
            for f in node.fields.iter_mut().chain(node.user_fields.iter_mut()) {
                replace(&mut f.value);
            }
        }

        for p in &mut self.scene.protos {
            for f in &mut p.fields {
                replace(&mut f.value);
            }
        }

        for c in &mut self.scene.contexts {
            for n in c.root_nodes.iter_mut().flatten() {
                if *n == from {
                    *n = to;
                }
            }
        }
    }
}
