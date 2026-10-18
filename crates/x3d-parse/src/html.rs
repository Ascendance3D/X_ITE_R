//! A small HTML front end for the X3D HTML integration: what a browser's HTML parser makes of
//! `div .innerHTML = text` (the fragment parsed in the `body` context), restricted to what an
//! `<x3d>` element needs. The result is an element tree whose names are upper case and whose
//! attribute names are lower case (`Element::html`), the input of the XMLParser's HTML mode
//! (`xml::parse_x3d_html_element`, with html_support.rs for the names).
//!
//! Implemented: start and end tags, quoted and unquoted attributes (the first of a duplicate
//! wins), character references in attribute values, comments, raw text elements (`script`,
//! `style`, `textarea`, `title`), void elements, `html`/`head`/`body` tags ignored as in the
//! body insertion mode, an end tag closing the nearest open element of that name.
//!
//! Not implemented: the rest of the HTML5 tree construction (implied end tags, foster parenting,
//! the adoption agency, tables, `<p>` closing, ...). X3D elements are unknown elements, which
//! the tree construction treats as plain containers, so none of that applies to them.

use crate::dom::{Element, XNode};
use crate::{LoadError, ParseError};
use x3d_scene::Scene;

const VOID: &[&str] = &["area", "base", "br", "col", "embed", "hr", "img", "input", "link", "meta", "param", "source", "track", "wbr"];
const RAW_TEXT: &[&str] = &["script", "style", "textarea", "title"];
const IGNORED: &[&str] = &["html", "head", "body"];

fn decode_references(s: &str) -> String {
    let mut out = String::new();
    let mut rest = s;

    while let Some(i) = rest.find('&') {
        out.push_str(&rest[..i]);

        let tail = &rest[i + 1..];
        let end = tail.find(|c: char| !(c.is_ascii_alphanumeric() || c == '#')).unwrap_or(tail.len());
        let name = &tail[..end];
        let decoded = match name {
            "amp" => Some('&'),
            "lt" => Some('<'),
            "gt" => Some('>'),
            "quot" => Some('"'),
            "apos" => Some('\''),
            "nbsp" => Some('\u{a0}'),
            _ => name.strip_prefix('#').and_then(|n| match n.strip_prefix(['x', 'X']) {
                Some(hex) => u32::from_str_radix(hex, 16).ok(),
                None => n.parse::<u32>().ok(),
            }).and_then(char::from_u32),
        };

        match decoded {
            Some(c) => {
                out.push(c);
                rest = &tail[end..];

                if rest.starts_with(';') {
                    rest = &rest[1..];
                }
            }
            None => {
                out.push('&');
                rest = tail;
            }
        }
    }

    out.push_str(rest);
    out
}

struct Open {
    element: Element,
    name: String,
}

/// The CDATA sections and elements of a script's text, read as XML like XMLParser.scriptElement
/// (`$.parseXML (element .outerHTML)`) does; `None` when the text is not well-formed.
fn script_children(text: &str) -> Option<Vec<XNode>> {
    let root = crate::dom::parse_document(&format!("<script>{text}</script>")).ok()?;

    // `for (const childNode of childNodes)` walks a live NodeList while `element .append (childNode)`
    // moves each element and CDATA section out of it, so the node after every moved one is skipped:
    // the iterator's index advances over a list that just got shorter.
    let mut live = root.children;
    let mut moved = vec![];
    let mut i = 0;

    while i < live.len() {
        if matches!(live[i], XNode::Element(_) | XNode::CData(_)) {
            moved.push(live.remove(i));
        }

        i += 1;
    }

    Some(moved)
}

/// Parses an HTML fragment into top-level nodes.
pub(crate) fn parse_fragment(text: &str) -> Vec<XNode> {
    let bytes = text.as_bytes();
    let mut pos = 0;
    let mut stack: Vec<Open> = vec![];
    let mut roots: Vec<XNode> = vec![];

    fn push(stack: &mut [Open], roots: &mut Vec<XNode>, node: XNode) {
        match stack.last_mut() {
            Some(top) => top.element.children.push(node),
            None => roots.push(node),
        }
    }

    fn close(stack: &mut Vec<Open>, roots: &mut Vec<XNode>) {
        if let Some(open) = stack.pop() {
            push(stack, roots, XNode::Element(open.element));
        }
    }

    while pos < bytes.len() {
        if bytes[pos] != b'<' {
            let end = text[pos..].find('<').map(|i| pos + i).unwrap_or(text.len());

            push(&mut stack, &mut roots, XNode::Text(decode_references(&text[pos..end])));
            pos = end;
            continue;
        }

        let rest = &text[pos..];

        if rest.starts_with("<!--") {
            let end = rest[4..].find("-->").map(|i| pos + 4 + i + 3).unwrap_or(text.len());
            let body = &text[pos + 4..end.saturating_sub(3).max(pos + 4)];

            push(&mut stack, &mut roots, XNode::Comment(body.to_string()));
            pos = end;
            continue;
        }

        if rest.starts_with("<!") || rest.starts_with("<?") {
            // A doctype, `<![CDATA[` (a bogus comment in HTML content) or a processing instruction.
            pos = rest.find('>').map(|i| pos + i + 1).unwrap_or(text.len());
            continue;
        }

        if rest.starts_with("</") {
            let name_end = rest[2..].find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/').map(|i| 2 + i).unwrap_or(rest.len());
            let name = rest[2..name_end].to_ascii_lowercase();

            pos = rest.find('>').map(|i| pos + i + 1).unwrap_or(text.len());

            if IGNORED.contains(&name.as_str()) {
                continue;
            }

            if let Some(i) = stack.iter().rposition(|o| o.name == name) {
                while stack.len() > i {
                    close(&mut stack, &mut roots);
                }
            }

            continue;
        }

        // A start tag needs an ASCII letter after `<`; anything else is text.
        if !rest[1..].starts_with(|c: char| c.is_ascii_alphabetic()) {
            push(&mut stack, &mut roots, XNode::Text("<".into()));
            pos += 1;
            continue;
        }

        let name_end = rest[1..].find(|c: char| c.is_ascii_whitespace() || c == '>' || c == '/').map(|i| 1 + i).unwrap_or(rest.len());
        let name = rest[1..name_end].to_ascii_lowercase();
        let mut i = name_end;
        let mut attrs: Vec<(String, String)> = vec![];
        let r = rest.as_bytes();

        // Attributes.
        loop {
            while i < r.len() && (r[i].is_ascii_whitespace() || r[i] == b'/') {
                i += 1;
            }

            if i >= r.len() || r[i] == b'>' {
                break;
            }

            let start = i;

            while i < r.len() && !r[i].is_ascii_whitespace() && r[i] != b'=' && r[i] != b'>' && !(r[i] == b'/' && start != i) {
                i += 1;
            }

            let attr_name = rest[start..i].to_ascii_lowercase();

            while i < r.len() && r[i].is_ascii_whitespace() {
                i += 1;
            }

            let mut value = String::new();

            if i < r.len() && r[i] == b'=' {
                i += 1;

                while i < r.len() && r[i].is_ascii_whitespace() {
                    i += 1;
                }

                if i < r.len() && (r[i] == b'"' || r[i] == b'\'') {
                    let quote = r[i];
                    let from = i + 1;
                    let to = rest[from..].find(quote as char).map(|k| from + k).unwrap_or(rest.len());

                    value = decode_references(&rest[from..to]);
                    i = (to + 1).min(rest.len());
                } else {
                    let from = i;

                    while i < r.len() && !r[i].is_ascii_whitespace() && r[i] != b'>' {
                        i += 1;
                    }

                    value = decode_references(&rest[from..i]);
                }
            }

            if !attr_name.is_empty() && !attrs.iter().any(|(n, _)| *n == attr_name) {
                attrs.push((attr_name, value));
            }
        }

        pos += (i + 1).min(rest.len());

        if IGNORED.contains(&name.as_str()) {
            continue;
        }

        let mut element = Element::new(&name.to_ascii_uppercase());

        element.html = true;
        element.attrs = attrs;

        if RAW_TEXT.contains(&name.as_str()) {
            // Everything up to the matching end tag is text.
            let lower = text[pos..].to_ascii_lowercase();
            let (body_end, after) = match lower.find(&format!("</{name}")) {
                Some(k) => (pos + k, text[pos + k..].find('>').map(|g| pos + k + g + 1).unwrap_or(text.len())),
                None => (text.len(), text.len()),
            };
            let body = &text[pos..body_end];

            if name == "script" {
                match script_children(body) {
                    Some(children) => element.children = children,
                    None => element.script_error = true,
                }
            } else {
                element.children.push(XNode::Text(body.to_string()));
            }

            push(&mut stack, &mut roots, XNode::Element(element));
            pos = after;
            continue;
        }

        if VOID.contains(&name.as_str()) {
            push(&mut stack, &mut roots, XNode::Element(element));
        } else {
            stack.push(Open { element, name });
        }
    }

    while !stack.is_empty() {
        close(&mut stack, &mut roots);
    }

    roots
}

fn find_x3d(nodes: &[XNode]) -> Option<&Element> {
    for node in nodes {
        if let XNode::Element(e) = node {
            if e.name == "X3D" {
                return Some(e);
            }

            if let Some(found) = find_x3d(&e.children) {
                return Some(found);
            }
        }
    }

    None
}

/// `div .innerHTML = html; browser .importDocument (div .querySelector ("x3d"))`: the scene of
/// the first `<x3d>` element of the fragment, parsed by the XMLParser in its HTML mode.
pub fn parse_html_x3d(html: &str, world_url: &str) -> Result<Scene, LoadError> {
    let nodes = parse_fragment(html);
    let Some(x3d) = find_x3d(&nodes) else {
        return Err(LoadError::Parse(ParseError { message: "No x3d element found.".into(), line: 0 }));
    };
    let mut scene = crate::xml::parse_x3d_html_element(x3d, world_url).map_err(LoadError::Parse)?;

    scene.clone_counts = Some(scene.parse_clone_counts());
    Ok(scene)
}
