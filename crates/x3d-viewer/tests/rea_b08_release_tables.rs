//! Batch 08 integration: runtime tables of the Batch 08 nodes are released with their nodes and
//! execution contexts (no GPU needed).
//!
//! * The shader host (`Runtime::shaders`: ShaderPart sources, ComposedShader state, Appearance
//!   selection, selection counts) is keyed by NodeId, and NodeIds are reused: a released node must
//!   leave no entry, or a new node in the same slot would inherit it.
//! * FontLibrary fonts are kept per execution context (X_ITE: a WeakMap on the context); a released
//!   Inline context leaves no font table, so a new context in the same slot starts empty.
//!
//! An Inline switches between a ComposedShader world and a FontLibrary world 20 times; the tables
//! stay bounded and are empty after the last unload.

use std::path::{Path, PathBuf};
use x3d_fields::FieldValue;
use x3d_scene::ROOT;
use x3d_viewer::viewer::Viewer3d;

fn oracle_url(p: &Path) -> String {
    format!("http://oracle{}", p.canonicalize().unwrap().display())
}

/// Parts, ComposedShaders, Appearances that list a shader, selection counts.
fn shader_tables(v: &Viewer3d) -> [usize; 4] {
    let s = &v.rt.shaders;
    [
        s.parts.len(),
        s.composed.len(),
        s.appearances
            .values()
            .filter(|a| !a.nodes.is_empty())
            .count(),
        s.selected_count.len(),
    ]
}

#[test]
fn shader_and_font_library_tables_are_released_with_their_nodes() {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(|| {
            let oracle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle");
            let shader = oracle_url(&oracle.join("synthetic-rea-b08-shaders/basic.wrl"));
            let fonts = oracle_url(&oracle.join("synthetic-rea-b08-net/fl-basic-a.wrl"));
            let dir = std::env::temp_dir().join(format!("rea-b08-release-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let main = dir.join("main.wrl");
            std::fs::write(
                &main,
                "#VRML V2.0 utf8\nViewpoint { position 0 0 6 }\nDEF Inl Inline { load FALSE }\n",
            )
            .unwrap();

            let mut v = Viewer3d::load(&main, Some(Path::new("/")), 100.0, 64, 64).expect("load");
            let mut t = 100.0;
            let mut steps = |v: &mut Viewer3d, n: usize| {
                for _ in 0..n {
                    t += 1.0 / 60.0;
                    v.step(t);
                }
            };
            let inl = v.rt.scene.contexts[ROOT].named_nodes.get("Inl").unwrap();
            let mut first_shader = None;

            for cycle in 0..20 {
                let url = if cycle % 2 == 0 { &shader } else { &fonts };
                let k = v.rt.get_field(inl, "url").unwrap();
                v.rt.set_value(k, FieldValue::MFString(vec![url.clone()]));
                let k = v.rt.get_field(inl, "load").unwrap();
                v.rt.set_value(k, FieldValue::SFBool(true));
                steps(&mut v, 30);

                if cycle % 2 == 0 {
                    // The shader world compiled and linked its parts and selected the shader.
                    let tables = shader_tables(&v);
                    assert!(
                        tables[0] >= 2 && tables[1] >= 1 && tables[2] >= 1,
                        "cycle {cycle}: shader world not set up {tables:?}"
                    );
                    assert_eq!(
                        *first_shader.get_or_insert(tables),
                        tables,
                        "cycle {cycle}: shader tables grew"
                    );
                    assert!(
                        v.rt.font_libraries.is_empty(),
                        "cycle {cycle}: fonts of a released context are still registered"
                    );
                } else {
                    assert!(
                        !v.rt.font_libraries.is_empty(),
                        "cycle {cycle}: font world registered no family"
                    );
                    assert_eq!(
                        shader_tables(&v),
                        [0; 4],
                        "cycle {cycle}: shader state of released nodes kept"
                    );
                }

                let k = v.rt.get_field(inl, "load").unwrap();
                v.rt.set_value(k, FieldValue::SFBool(false));
                steps(&mut v, 30);
            }

            assert_eq!(
                shader_tables(&v),
                [0; 4],
                "shader tables after the last unload"
            );
            assert!(
                v.rt.shaders.appearances.is_empty(),
                "Appearance entries after the last unload"
            );
            assert!(
                v.rt.font_libraries.is_empty(),
                "font tables after the last unload"
            );
            let _ = std::fs::remove_dir_all(&dir);
        })
        .unwrap()
        .join()
        .unwrap();
}
