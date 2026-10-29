//! X_ITE's node, component and profile tables.
//!
//! `data/x_ite-nodes.json` is generated, not hand-written: oracle/x_ite-nodes.mjs instantiates
//! every concrete node class of the pinned X_ITE in headless chromium and dumps its field
//! definitions; oracle/tools/trim_nodes.py trims that to this file. X_ITE is the truth here,
//! not X3DUOM: e.g. DirectionalLight `global` starts TRUE in a VRML 2.0 scene although its
//! definition default is FALSE, so the node always prints `global TRUE` there.

use serde_json::Value;
use std::collections::HashMap;
use std::sync::OnceLock;
use x3d_fields::{FieldType, FieldValue};

#[derive(Clone, Debug)]
pub struct FieldDef {
    pub access: u8,
    pub name: String,
    pub ty: FieldType,
    /// The definition's value: what `isDefaultValue` compares against.
    pub default: FieldValue,
    /// The value a fresh instance starts with (differs from `default` for a few nodes).
    pub initial: FieldValue,
    /// X3DField.getUnit: the unit category ("length", "angle", "speed", ...), if any.
    pub unit: Option<String>,
}

#[derive(Clone, Debug)]
pub struct NodeVariant {
    pub can_user_defined_fields: bool,
    pub fields: Vec<FieldDef>,
    /// alias → field name (X3DBaseNode.addAlias).
    pub aliases: HashMap<String, String>,
}

#[derive(Clone, Debug)]
pub struct NodeType {
    pub name: String,
    pub component: String,
    pub container_field: String,
    pub from: String,
    pub to: String,
    /// Definitions as seen by a node created in a `#VRML V2.0` scene.
    pub vrml2: NodeVariant,
    /// Definitions as seen by a node created in an X3D scene.
    pub x3d: NodeVariant,
    /// Definitions as seen in an X3D 3.0 … 3.3 scene, where they differ from `x3d` (constructors
    /// that test `getSpecificationVersion () <= 3.x`), by dumped version.
    pub legacy: Vec<(f64, NodeVariant)>,
    /// Definitions as seen in an X3D 4.1 scene, where they differ from `x3d` (constructors that
    /// test `getSpecificationVersion () <= 4.0`, `x3d` being a 4.0 scene).
    pub newer: Vec<(f64, NodeVariant)>,
}

/// The X3D 3.x versions dumped for `NodeType::legacy`.
pub const LEGACY_VERSIONS: [f64; 4] = [3.0, 3.1, 3.2, 3.3];

impl NodeType {
    pub fn variant(&self, spec_version: &str) -> &NodeVariant {
        // X_ITE tests `getSpecificationVersion () == 2.0` (loose equality with a number).
        let Ok(v) = spec_version.trim().parse::<f64>() else {
            return &self.x3d;
        };

        if v == 2.0 {
            return &self.vrml2;
        }

        // A version above 4.0: the newest dumped scene not above it.
        if v > 4.0 {
            return self
                .newer
                .iter()
                .filter(|(k, _)| *k <= v)
                .next_back()
                .map(|(_, x)| x)
                .unwrap_or(&self.x3d);
        }

        // The dumped 3.x scene whose `<=` tests decide like this version's: the first one >= it.
        match LEGACY_VERSIONS.iter().find(|&&k| v <= k) {
            Some(&k) => self
                .legacy
                .iter()
                .find(|(l, _)| *l == k)
                .map(|(_, x)| x)
                .unwrap_or(&self.x3d),
            None => &self.x3d,
        }
    }
}

/// (alias, name) of Configuration/SupportedComponents.js and SupportedProfiles.js.
pub const COMPONENT_ALIASES: &[(&str, &str)] = &[
    ("H-Anim", "HAnim"),
    ("TextureProjector", "TextureProjection"),
    ("ProjectiveTextureMapping", "TextureProjection"),
];
pub const PROFILE_ALIASES: &[(&str, &str)] = &[("MPEG-4", "MPEG4Interactive")];

#[derive(Clone, Debug)]
pub struct ComponentDef {
    pub name: String,
    pub title: String,
    pub level: i32,
    pub external: bool,
    pub dependencies: Vec<String>,
}

#[derive(Clone, Debug)]
pub struct ProfileDef {
    pub name: String,
    pub title: String,
    pub components: Vec<(String, i32)>,
}

#[derive(Debug)]
pub struct NodeTable {
    pub pinned: String,
    pub browser_name: String,
    pub browser_version: String,
    pub types: Vec<NodeType>,
    pub by_name: HashMap<String, usize>,
    pub components: Vec<ComponentDef>,
    pub profiles: Vec<ProfileDef>,
    /// Concrete node names registered after X_ITE loaded a VRML 2.0 scene (cross-check data).
    pub vrml2_available: Vec<String>,
}

impl NodeTable {
    pub fn get(&self, name: &str) -> Option<&NodeType> {
        self.by_name.get(name).map(|i| &self.types[*i])
    }

    /// SupportedComponents.js `alias` and SupportedProfiles.js `alias`: another name of a supported component or profile.
    pub fn component(&self, name: &str) -> Option<&ComponentDef> {
        let name = COMPONENT_ALIASES
            .iter()
            .find(|a| a.0 == name)
            .map_or(name, |a| a.1);

        self.components.iter().find(|c| c.name == name)
    }

    pub fn profile(&self, name: &str) -> Option<&ProfileDef> {
        let name = PROFILE_ALIASES
            .iter()
            .find(|a| a.0 == name)
            .map_or(name, |a| a.1);

        self.profiles.iter().find(|p| p.name == name)
    }
}

fn variant(json: &Value) -> NodeVariant {
    let initial = json["initial"].as_object().cloned().unwrap_or_default();
    let units = json["units"].as_object().cloned().unwrap_or_default();

    let fields = json["fields"]
        .as_array()
        .expect("fields")
        .iter()
        .map(|f| {
            let access = f[0].as_u64().expect("access") as u8;
            let name = f[1].as_str().expect("name").to_string();
            let ty = FieldType::from_name(f[2].as_str().expect("type")).expect("field type");
            let default =
                FieldValue::from_json(ty, &f[3]).unwrap_or_else(|| panic!("default of {name}"));
            let initial = match initial.get(&name) {
                Some(v) => {
                    FieldValue::from_json(ty, v).unwrap_or_else(|| panic!("initial of {name}"))
                }
                None => default.clone(),
            };

            let unit = units
                .get(&name)
                .and_then(|u| u.as_str())
                .map(str::to_string);

            FieldDef {
                access,
                name,
                ty,
                default,
                initial,
                unit,
            }
        })
        .collect();

    let aliases = json["aliases"]
        .as_object()
        .map(|m| {
            m.iter()
                .map(|(k, v)| (k.clone(), v.as_str().unwrap().to_string()))
                .collect()
        })
        .unwrap_or_default();

    NodeVariant {
        can_user_defined_fields: json["udf"].as_bool().unwrap_or(false),
        fields,
        aliases,
    }
}

fn load() -> NodeTable {
    let json: Value =
        serde_json::from_str(include_str!("../data/x_ite-nodes.json")).expect("node table JSON");

    // REA batch 04 (owner decision 2026-10-17): the Annotation component, which X_ITE 16.5.0 ships
    // but does not register. Dumped from those classes by oracle/x_ite-nodes-annotation.mjs.
    let annotation: Value =
        serde_json::from_str(include_str!("../data/x_ite-nodes-annotation.json"))
            .expect("annotation node table JSON");

    let types: Vec<NodeType> = json["nodes"]
        .as_array()
        .unwrap()
        .iter()
        .chain(annotation["nodes"].as_array().unwrap().iter())
        .map(|n| NodeType {
            name: n["typeName"].as_str().unwrap().to_string(),
            component: n["component"].as_str().unwrap().to_string(),
            container_field: n["containerField"].as_str().unwrap_or("").to_string(),
            from: n["from"].as_str().unwrap().to_string(),
            to: n["to"].as_str().unwrap().to_string(),
            vrml2: variant(&n["vrml2"]),
            x3d: variant(&n["x3d"]),
            legacy: n["legacy"]
                .as_object()
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| (k.parse::<f64>().expect("legacy version"), variant(v)))
                        .collect()
                })
                .unwrap_or_default(),
            newer: n["newer"]
                .as_object()
                .map(|m| {
                    m.iter()
                        .map(|(k, v)| (k.parse::<f64>().expect("newer version"), variant(v)))
                        .collect()
                })
                .unwrap_or_default(),
        })
        .collect();

    let mut types = types;
    iso_overrides(&mut types);

    let by_name = types
        .iter()
        .enumerate()
        .map(|(i, t)| (t.name.clone(), i))
        .collect();

    let components = json["components"]
        .as_array()
        .unwrap()
        .iter()
        .chain(annotation["components"].as_array().unwrap().iter())
        .map(|c| ComponentDef {
            name: c["name"].as_str().unwrap().to_string(),
            title: c["title"].as_str().unwrap().to_string(),
            level: c["level"].as_i64().unwrap() as i32,
            external: c["external"].as_bool().unwrap(),
            dependencies: c["dependencies"]
                .as_array()
                .unwrap()
                .iter()
                .map(|d| d.as_str().unwrap().to_string())
                .collect(),
        })
        .collect();

    let profiles = json["profiles"]
        .as_array()
        .unwrap()
        .iter()
        .map(|p| ProfileDef {
            name: p["name"].as_str().unwrap().to_string(),
            title: p["title"].as_str().unwrap().to_string(),
            components: p["components"]
                .as_array()
                .unwrap()
                .iter()
                .map(|c| {
                    (
                        c["name"].as_str().unwrap().to_string(),
                        c["level"].as_i64().unwrap() as i32,
                    )
                })
                .collect(),
        })
        .collect();

    NodeTable {
        pinned: json["pinned"].as_str().unwrap_or("").to_string(),
        browser_name: json["browserName"].as_str().unwrap().to_string(),
        browser_version: json["browserVersion"].as_str().unwrap().to_string(),
        types,
        by_name,
        components,
        profiles,
        vrml2_available: json["vrml2Available"]
            .as_array()
            .unwrap()
            .iter()
            .map(|s| s.as_str().unwrap().to_string())
            .collect(),
    }
}

/// Field defaults where ISO/IEC 19775-1 and X_ITE 16.5.0 differ and the owner chose ISO (REA
/// batch 04, 2026-10-17): NurbsSet bboxDisplay is TRUE in ISO 27.4.8, FALSE in X_ITE.
fn iso_overrides(types: &mut [NodeType]) {
    const OVERRIDES: &[(&str, &str, bool)] = &[("NurbsSet", "bboxDisplay", true)];

    for (type_name, field, value) in OVERRIDES {
        for t in types.iter_mut().filter(|t| t.name == *type_name) {
            let variants = std::iter::once(&mut t.vrml2)
                .chain(std::iter::once(&mut t.x3d))
                .chain(t.legacy.iter_mut().map(|(_, v)| v))
                .chain(t.newer.iter_mut().map(|(_, v)| v));

            for v in variants {
                for f in v.fields.iter_mut().filter(|f| f.name == *field) {
                    f.default = FieldValue::SFBool(*value);
                    f.initial = FieldValue::SFBool(*value);
                }
            }
        }
    }
}

pub fn table() -> &'static NodeTable {
    static TABLE: OnceLock<NodeTable> = OnceLock::new();
    TABLE.get_or_init(load)
}
