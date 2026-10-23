//! The DOM the XML parser walks: `DOMParser.parseFromString (string, "application/xml")` as
//! `$.parseXML` (src/lib/helper.js) uses it, and the elements JSONParser.js builds with
//! `document.createElementNS`. Only what XMLParser.js reads is kept: element names
//! (`nodeName`), attributes in document order (`attributes`, `getAttribute`), and child nodes
//! (`#text`, `#cdata-section`, `#comment`, elements).

use quick_xml::events::Event;

#[derive(Clone, Debug)]
pub(crate) enum XNode {
    Element(Element),
    // XMLParser reads only elements and CDATA sections; text and comments stay in the tree.
    #[allow(dead_code)]
    Text(String),
    CData(String),
    #[allow(dead_code)]
    Comment(String),
}

#[derive(Clone, Debug, Default)]
pub(crate) struct Element {
    pub name: String,
    pub attrs: Vec<(String, String)>,
    pub children: Vec<XNode>,
    /// An element of an HTML document (html.rs): `nodeName` is upper case, attribute names are
    /// lower case and `getAttribute` ignores the case of its argument.
    pub html: bool,
    /// An HTML `<script>` whose text is not well-formed XML (XMLParser.scriptElement throws).
    pub script_error: bool,
}

impl Element {
    pub fn new(name: &str) -> Element {
        Element {
            name: name.to_string(),
            ..Default::default()
        }
    }

    /// Element.getAttribute: `None` is `null`.
    pub fn attr(&self, name: &str) -> Option<&str> {
        if self.html {
            let lower = name.to_ascii_lowercase();

            return self
                .attrs
                .iter()
                .find(|(n, _)| *n == lower)
                .map(|(_, v)| v.as_str());
        }

        self.attrs
            .iter()
            .find(|(n, _)| n == name)
            .map(|(_, v)| v.as_str())
    }

    /// Element.setAttribute: an existing attribute keeps its position.
    pub fn set_attr(&mut self, name: &str, value: String) {
        match self.attrs.iter_mut().find(|(n, _)| n == name) {
            Some(a) => a.1 = value,
            None => self.attrs.push((name.to_string(), value)),
        }
    }
}

/// XML end-of-line handling (XML 1.0 section 2.11): `\r\n` and a lone `\r` become `\n`
/// before anything is parsed.
fn normalize_line_ends(text: &str) -> String {
    text.replace("\r\n", "\n").replace('\r', "\n")
}

fn general_ref(name: &str) -> Result<char, String> {
    if let Some(rest) = name.strip_prefix('#') {
        let code = match rest.strip_prefix('x') {
            Some(hex) => u32::from_str_radix(hex, 16),
            None => rest.parse::<u32>(),
        };

        return code
            .ok()
            .and_then(char::from_u32)
            .ok_or_else(|| format!("Invalid character reference '&{name};'."));
    }

    match name {
        "lt" => Ok('<'),
        "gt" => Ok('>'),
        "amp" => Ok('&'),
        "apos" => Ok('\''),
        "quot" => Ok('"'),
        _ => Err(format!("Entity '{name}' not defined.")),
    }
}

/// `$.parseXML (string)` and `documentElement`: the root element, or an error where the browser
/// would return a `parsererror` document ("Invalid XML.").
pub(crate) fn parse_document(text: &str) -> Result<Element, String> {
    let text = normalize_line_ends(text);
    let mut reader = quick_xml::Reader::from_str(&text);
    let config = reader.config_mut();

    config.trim_text(false);
    config.check_end_names = true;
    config.check_comments = true;

    let mut stack: Vec<Element> = vec![];
    let mut root: Option<Element> = None;
    let mut text_run: Option<String> = None;

    // Adjacent text and entity references form one text node.
    fn flush(stack: &mut [Element], run: &mut Option<String>) {
        if let Some(t) = run.take() {
            if let Some(top) = stack.last_mut() {
                top.children.push(XNode::Text(t));
            }
        }
    }

    let element_of = |e: &quick_xml::events::BytesStart| -> Result<Element, String> {
        let name = String::from_utf8_lossy(e.name().as_ref()).into_owned();
        let mut element = Element::new(&name);

        for a in e.attributes().with_checks(true) {
            let a = a.map_err(|e| e.to_string())?;
            let key = String::from_utf8_lossy(a.key.as_ref()).into_owned();

            // Namespace declarations are attributes in the DOM too, but no field is named like one.
            let value = a
                .normalized_value(quick_xml::XmlVersion::Implicit1_0)
                .map_err(|e| e.to_string())?
                .into_owned();

            if element.attr(&key).is_some() {
                return Err(format!("Attribute '{key}' redefined."));
            }

            element.attrs.push((key, value));
        }

        Ok(element)
    };

    loop {
        match reader.read_event().map_err(|e| e.to_string())? {
            Event::Start(e) => {
                flush(&mut stack, &mut text_run);

                if stack.is_empty() && root.is_some() {
                    return Err("Extra content at the end of the document.".into());
                }

                stack.push(element_of(&e)?);
            }
            Event::Empty(e) => {
                flush(&mut stack, &mut text_run);

                let element = element_of(&e)?;

                match stack.last_mut() {
                    Some(top) => top.children.push(XNode::Element(element)),
                    None if root.is_none() => root = Some(element),
                    None => return Err("Extra content at the end of the document.".into()),
                }
            }
            Event::End(_) => {
                flush(&mut stack, &mut text_run);

                let element = stack.pop().ok_or("Unexpected end tag.")?;

                match stack.last_mut() {
                    Some(top) => top.children.push(XNode::Element(element)),
                    None => root = Some(element),
                }
            }
            Event::Text(t) => {
                let s = t.decode().map_err(|e| e.to_string())?;

                if stack.is_empty() {
                    if !s.chars().all(|c| matches!(c, ' ' | '\t' | '\n')) {
                        return Err("Content outside the root element.".into());
                    }
                } else {
                    text_run.get_or_insert_with(String::new).push_str(&s);
                }
            }
            Event::GeneralRef(r) => {
                let name = String::from_utf8_lossy(&r).into_owned();
                let c = general_ref(&name)?;

                if stack.is_empty() {
                    return Err("Content outside the root element.".into());
                }

                text_run.get_or_insert_with(String::new).push(c);
            }
            Event::CData(c) => {
                flush(&mut stack, &mut text_run);

                let s = c.decode().map_err(|e| e.to_string())?.into_owned();

                match stack.last_mut() {
                    Some(top) => top.children.push(XNode::CData(s)),
                    None => return Err("CDATA outside the root element.".into()),
                }
            }
            Event::Comment(c) => {
                flush(&mut stack, &mut text_run);

                if let Some(top) = stack.last_mut() {
                    top.children.push(XNode::Comment(
                        c.decode().map_err(|e| e.to_string())?.into_owned(),
                    ));
                }
            }
            Event::Decl(_) | Event::PI(_) | Event::DocType(_) => flush(&mut stack, &mut text_run),
            Event::Eof => break,
        }
    }

    if !stack.is_empty() {
        return Err("Premature end of data.".into());
    }

    root.ok_or_else(|| "Document is empty.".to_string())
}
