//! X3D and VRML97 node reference, plus reading and writing the classic encoding.
//!
//! Every node type X_ITE 16.5 knows (263, from VRML97 to X3D 4.1), with its component,
//! container field, specification range and fields: access type, field type and default
//! value. The data comes from [X_ITE](https://github.com/create3000/x_ite)'s own node
//! definitions, so it describes what a real X3D browser accepts.
//!
//! ```
//! let t = x3d::node("Transform").unwrap();
//! assert_eq!(t.component(), "Grouping");
//! let f = t.field("translation").unwrap();
//! assert_eq!((f.access(), f.type_name(), f.default_vrml().as_str()), ("inputOutput", "SFVec3f", "0 0 0"));
//! ```
//!
//! Classic-encoding files (`.wrl`, `.x3dv`) are read and written by [`vrml97`], re-exported
//! here as [`load_file`], [`parse_bytes`] and [`Scene`].

use vrml97::fields::{FieldValue, Generator, Style, access};
use vrml97::scene::{self, NodeType};

pub use vrml97;
pub use vrml97::{Scene, load_file, parse_bytes};

/// A node type.
#[derive(Clone, Copy)]
pub struct Node {
    inner: &'static NodeType,
}

/// A field of a node type.
#[derive(Clone, Copy)]
pub struct Field {
    inner: &'static scene::nodes::FieldDef,
}

/// Look up a node type by its exact name, e.g. `"IndexedFaceSet"`.
pub fn node(name: &str) -> Option<Node> {
    scene::table().get(name).map(|inner| Node { inner })
}

/// All node types, in X_ITE's registration order.
pub fn nodes() -> impl Iterator<Item = Node> {
    scene::table().types.iter().map(|inner| Node { inner })
}

/// All components with their highest supported level.
pub fn components() -> impl Iterator<Item = (&'static str, i32)> {
    scene::table()
        .components
        .iter()
        .map(|c| (c.name.as_str(), c.level))
}

/// All profiles, each with its (component, level) list.
pub fn profiles() -> impl Iterator<Item = (&'static str, &'static [(String, i32)])> {
    scene::table()
        .profiles
        .iter()
        .map(|p| (p.name.as_str(), p.components.as_slice()))
}

impl Node {
    pub fn name(&self) -> &'static str {
        &self.inner.name
    }

    /// The component that defines this node, e.g. `"Geometry3D"`.
    pub fn component(&self) -> &'static str {
        &self.inner.component
    }

    /// The default containerField, e.g. `"children"` or `"geometry"`.
    pub fn container_field(&self) -> &'static str {
        &self.inner.container_field
    }

    /// First and last specification version that has this node (`to` is `"Infinity"` when open).
    pub fn spec_range(&self) -> (&'static str, &'static str) {
        (&self.inner.from, &self.inner.to)
    }

    /// The node's fields, in X_ITE's order. (X_ITE gives VRML97 and X3D scenes the same
    /// field interface; VRML97-only spellings are accepted by the parser as aliases.)
    pub fn fields(&self) -> impl Iterator<Item = Field> {
        self.inner.x3d.fields.iter().map(|inner| Field { inner })
    }

    /// Look up a field by name.
    pub fn field(&self, name: &str) -> Option<Field> {
        self.fields().find(|f| f.name() == name)
    }
}

impl Field {
    pub fn name(&self) -> &'static str {
        &self.inner.name
    }

    /// `"initializeOnly"`, `"inputOnly"`, `"outputOnly"` or `"inputOutput"`.
    pub fn access(&self) -> &'static str {
        access::name(self.inner.access)
    }

    /// Field type name, e.g. `"MFVec3f"`.
    pub fn type_name(&self) -> &'static str {
        self.inner.ty.name()
    }

    /// The default value as X3D classic-encoding text, formatted like X_ITE
    /// (`"0 0 1 0"`, `"TRUE"`, `"NULL"`, `"[ ]"` for empty MF fields).
    pub fn default_vrml(&self) -> String {
        match &self.inner.default {
            FieldValue::SFNode(_) => "NULL".to_string(),
            FieldValue::MFNode(_) => "[ ]".to_string(),
            value => {
                let mut g = Generator::new(Style::Clean);
                value.to_vrml_stream(&mut g);
                g.string
            }
        }
    }

    /// The default value itself.
    pub fn default_value(&self) -> &'static FieldValue {
        &self.inner.default
    }
}
