//! `Sg::any_node` (one pass over a node's fields) gives what the per-name search gives: `any` of `Sg::node`.
//! Synthetic scene: Materials and PhysicalMaterials with and without texture nodes, a PROTO-wrapped texture,
//! and a NULL field.

use x3d_render::sg::Sg;
use x3d_runtime::{LocalFetcher, Runtime};

const NAMES: &[&str] = &[
    "ambientTexture",
    "diffuseTexture",
    "specularTexture",
    "emissiveTexture",
    "shininessTexture",
    "occlusionTexture",
    "normalTexture",
    "baseTexture",
    "metallicRoughnessTexture",
];

const WORLD: &str = r#"#X3D V4.0 utf8
PROFILE Full
PROTO Tex [ ] { ImageTexture { } }
PROTO Empty [ ] { }
Shape { appearance Appearance { material Material { } } geometry Box { } }
Shape { appearance Appearance { material Material { emissiveTexture ImageTexture { } } } geometry Box { } }
Shape { appearance Appearance { material Material { normalTexture Tex { } } } geometry Box { } }
Shape { appearance Appearance { material Material { diffuseTexture Empty { } } } geometry Box { } }
Shape { appearance Appearance { material PhysicalMaterial { baseTexture ImageTexture { } } } geometry Box { } }
Shape { appearance Appearance { material PhysicalMaterial { emissiveTexture NULL } } geometry Box { } }
Shape { appearance Appearance { material UnlitMaterial { emissiveTexture ImageTexture { } } } geometry Box { } }
"#;

#[test]
fn any_node_matches_the_search_by_name_on_every_node() {
    let scene = x3d_parse::Parser::new(WORLD, "http://test/w.x3dv")
        .parse()
        .expect("parse");
    let mut rt = Runtime::new(
        scene,
        Box::new(LocalFetcher {
            root: std::env::temp_dir(),
            prefix: "http://test/".into(),
        }),
    );
    rt.load_world();
    let sg = Sg::new(&rt);
    let (mut with, mut without) = (0, 0);

    for n in 0..rt.scene.nodes.len() as x3d_fields::NodeId {
        for names in [
            NAMES,
            &NAMES[3..4],
            &NAMES[..0],
            &["material", "texture"][..],
        ] {
            let want = names.iter().any(|f| sg.node(n, f).is_some());
            assert_eq!(sg.any_node(n, names), want, "node {n} {names:?}");

            if names.len() == NAMES.len() {
                if want { with += 1 } else { without += 1 }
            }
        }
    }

    // emissiveTexture, normalTexture (PROTO), baseTexture and UnlitMaterial's emissiveTexture hold a node.
    assert!(with >= 4, "materials with a texture: {with}");
    assert!(without > 0);
}
