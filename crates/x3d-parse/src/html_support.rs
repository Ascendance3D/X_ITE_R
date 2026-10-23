//! Port of src/x_ite/Parser/HTMLSupport.js: the two global maps the XMLParser consults when its
//! input is an HTML element (DOM integration; an XML document never uses them).
//!
//! * node type names: `TypeName` and `TYPENAME` (the upper case form is what an HTML parser
//!   produces for `nodeName`) map to `TypeName`. `ProtoInstance` is registered first.
//! * field names: for every field with the `initializeOnly` bit (so `initializeOnly` and
//!   `inputOutput`, not `inputOnly` or `outputOnly`) and for every alias of such a field,
//!   `fieldName` and `fieldname` map to `fieldName`. The map is global, not per node; a node that
//!   does not own the field then rejects the name later (`getPredefinedField` throws, the
//!   XMLParser ignores it).
//!
//! In X_ITE both maps are filled as nodes are registered (`ConcreteNodesArray.add`) and aliases as
//! nodes are constructed (`X3DBaseNode.addAlias`); with every node registered the result is the
//! set built here from the node table (X3D variants), in table order.

use std::collections::HashMap;
use std::sync::OnceLock;
use x3d_fields::access;

pub(crate) struct HtmlSupport {
    node_type_names: HashMap<String, String>,
    field_names: HashMap<String, String>,
}

impl HtmlSupport {
    fn add_node_type_name(&mut self, type_name: &str) {
        self.node_type_names
            .insert(type_name.to_string(), type_name.to_string());
        self.node_type_names
            .insert(type_name.to_uppercase(), type_name.to_string());
    }

    fn add_field_name(&mut self, name: &str) {
        self.field_names.insert(name.to_string(), name.to_string());
        self.field_names
            .insert(name.to_lowercase(), name.to_string());
    }

    /// `HTMLSupport .getNodeTypeName (typeName)`
    pub fn node_type_name(&self, type_name: &str) -> Option<&str> {
        self.node_type_names.get(type_name).map(String::as_str)
    }

    /// `HTMLSupport .getFieldName (name)`
    pub fn field_name(&self, name: &str) -> Option<&str> {
        self.field_names.get(name).map(String::as_str)
    }
}

fn build() -> HtmlSupport {
    let mut support = HtmlSupport {
        node_type_names: HashMap::new(),
        field_names: HashMap::new(),
    };

    support.add_node_type_name("ProtoInstance");

    for t in &x3d_scene::table().types {
        // addConcreteNode: a type name that is known already is skipped.
        if support.node_type_names.contains_key(&t.name) {
            continue;
        }

        support.add_node_type_name(&t.name);

        for f in &t.x3d.fields {
            if f.access & access::INITIALIZE_ONLY != 0 {
                support.add_field_name(&f.name);
            }
        }

        // X3DBaseNode.addAlias: only when the field is initializable.
        let mut aliases: Vec<(&String, &String)> = t.x3d.aliases.iter().collect();

        aliases.sort();

        for (alias, target) in aliases {
            if t.x3d
                .fields
                .iter()
                .any(|f| &f.name == target && f.access & access::INITIALIZE_ONLY != 0)
            {
                support.add_field_name(alias);
            }
        }
    }

    support
}

pub(crate) fn get() -> &'static HtmlSupport {
    static SUPPORT: OnceLock<HtmlSupport> = OnceLock::new();

    SUPPORT.get_or_init(build)
}
