//! VRML and X3D classic-encoding toolkit.
//!
//! - [`detect`] tells which VRML/X3D version a file declares, gzip or plain.
//! - [`summarize`] counts what a parsed scene contains.
//! - The `vrml` command-line tool (`cargo install vrml`) checks, re-writes (as X3D classic
//!   encoding, like X_ITE) and summarizes
//!   `.wrl` files.
//!
//! Parsing and writing are done by [`vrml97`], a faithful Rust port of X_ITE's parser and
//! writer, re-exported here.
//!
//! ```
//! assert_eq!(vrml::detect(b"#VRML V2.0 utf8\nShape {}"), vrml::Version::Vrml2);
//! assert_eq!(vrml::detect(b"#X3D V3.3 utf8\n"), vrml::Version::X3d("3.3".into()));
//! ```

use std::collections::BTreeMap;

pub use vrml97;
pub use vrml97::{Scene, load_file, parse_bytes};

/// The version a file declares in its header line.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Version {
    /// `#VRML V1.0 ascii` (not supported by the parser, like X_ITE).
    Vrml1,
    /// `#VRML V2.0 utf8` (VRML97).
    Vrml2,
    /// `#X3D V<version> utf8`.
    X3d(String),
    /// No recognized header.
    Unknown,
}

/// Detect the declared version. Gzip-compressed input is decompressed first.
pub fn detect(data: &[u8]) -> Version {
    let bytes = vrml97::gunzip(data);
    let text = vrml97::decode_text(&bytes[..bytes.len().min(256)]);
    let line = text.lines().next().unwrap_or("");

    if line.starts_with("#VRML V1.0") {
        Version::Vrml1
    } else if line.starts_with("#VRML V2.0") {
        Version::Vrml2
    } else if let Some(rest) = line.strip_prefix("#X3D V") {
        Version::X3d(rest.split_whitespace().next().unwrap_or("").to_string())
    } else {
        Version::Unknown
    }
}

/// True when the data starts with the gzip magic bytes.
pub fn is_gzip(data: &[u8]) -> bool {
    data.len() >= 2 && data[0] == 0x1f && data[1] == 0x8b
}

/// What a parsed scene contains.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Summary {
    pub specification_version: String,
    pub profile: Option<String>,
    pub root_nodes: usize,
    /// Every node instance, including those inside PROTO bodies.
    pub nodes: usize,
    /// Node count per type name, sorted by name.
    pub node_types: BTreeMap<String, usize>,
    pub protos: usize,
    pub externprotos: usize,
    pub routes: usize,
    pub warnings: usize,
}

/// Count the contents of a scene.
pub fn summarize(scene: &Scene) -> Summary {
    let mut node_types = BTreeMap::new();

    for (id, node) in scene.nodes.iter().enumerate() {
        // Unresolved forward USEs and runtime objects are not nodes of the file.
        if matches!(
            node.kind,
            vrml97::scene::NodeKind::Placeholder | vrml97::scene::NodeKind::Internal(_)
        ) {
            continue;
        }
        *node_types
            .entry(scene.type_name(id as vrml97::fields::NodeId))
            .or_insert(0) += 1;
    }

    Summary {
        specification_version: scene.specification_version.clone(),
        profile: scene.profile.clone(),
        root_nodes: scene.ctx(vrml97::scene::ROOT).root_nodes.len(),
        nodes: node_types.values().sum(),
        node_types,
        protos: scene.protos.len(),
        externprotos: scene.externprotos.len(),
        routes: scene.contexts.iter().map(|c| c.routes.len()).sum(),
        warnings: scene.warnings.len(),
    }
}
