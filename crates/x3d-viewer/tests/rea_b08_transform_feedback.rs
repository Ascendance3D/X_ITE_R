//! REA batch 08 shaders lane: `ComposedShader.setTransformFeedbackVaryings` against pinned X_ITE (no GPU needed).
//!
//! X_ITE sets it only for its own shaders (`browser .createShader ({ transformFeedbackVaryings })`, no field) and
//! hands it to `gl.transformFeedbackVaryings (program, varyings, INTERLEAVED_ATTRIBS)` before the link. The
//! oracle (`oracle/tools/rea_b08_transform_feedback.mjs`, headless Chrome on the GPU) wrote
//! oracle/rea-batch08-shaders-baseline/transform-feedback.json: per case `isValid` and the console warnings.
//! Here the same vertex and fragment shader is linked by the Rust runtime with the same list
//! (`Runtime::composed_set_transform_feedback_varyings`, then `activate`, which relinks) and `isValid` and the
//! warnings must be the same.

use serde_json::Value;
use std::path::{Path, PathBuf};
use x3d_fields::FieldValue;
use x3d_scene::ROOT;
use x3d_viewer::viewer::Viewer3d;

#[test]
fn transform_feedback_varyings_match_x_ite() {
    std::thread::Builder::new()
        .stack_size(256 << 20)
        .spawn(|| {
            let oracle = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle");
            let baseline: Value = serde_json::from_str(&std::fs::read_to_string(oracle.join("rea-batch08-shaders-baseline/transform-feedback.json")).expect("baseline missing: comparison NOT RUN")).unwrap();
            let (vs, fs) = (baseline["vertex"].as_str().unwrap(), baseline["fragment"].as_str().unwrap());
            let dir = std::env::temp_dir().join(format!("rea-b08-tf-{}", std::process::id()));
            std::fs::create_dir_all(&dir).unwrap();
            let main = dir.join("main.wrl");
            let world = format!(
                "#X3D V4.0 utf8\nPROFILE Full\nViewpoint {{ position 0 0 6 }}\nShape {{ appearance Appearance {{ shaders DEF CS ComposedShader {{ language \"GLSL\" parts [ ShaderPart {{ type \"VERTEX\" url \"data:x-shader/x-vertex,{vs}\" }} ShaderPart {{ type \"FRAGMENT\" url \"data:x-shader/x-fragment,{fs}\" }} ] }} }} geometry Sphere {{ }} }}\n"
            );
            std::fs::write(&main, world).unwrap();

            let mut v = Viewer3d::load(&main, Some(Path::new("/")), 100.0, 64, 64).expect("load");
            let mut t = 100.0;
            let mut steps = |v: &mut Viewer3d, n: usize| {
                for _ in 0..n {
                    t += 1.0 / 60.0;
                    v.step(t);
                }
            };
            steps(&mut v, 10);
            let cs = v.rt.scene.contexts[ROOT].named_nodes.get("CS").unwrap_or_else(|| panic!("no CS: {:?}", v.rt.log));
            let mut wrong = vec![];
            let cases = baseline["cases"].as_array().unwrap();

            for c in cases {
                let name = c["name"].as_str().unwrap();
                let varyings: Vec<String> = c["varyings"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().to_string()).collect();
                let want_valid = c["valid"].as_bool().unwrap();
                // X_ITE names the node `<name>Shader`; the world names it CS.
                let want_warnings: Vec<String> = c["warnings"].as_array().unwrap().iter().map(|s| s.as_str().unwrap().replace(&format!("'{name}Shader'"), "'CS'")).collect();

                v.rt.composed_set_transform_feedback_varyings(cs, varyings);
                let before = v.rt.log.len();
                let k = v.rt.get_field(cs, "activate").unwrap();
                v.rt.set_value(k, FieldValue::SFBool(true));
                steps(&mut v, 5);

                let valid = v.rt.shader_is_valid_flag(cs);
                let warnings: Vec<String> = v.rt.log[before..].iter().filter_map(|l| l.strip_prefix("warn: ").map(str::to_string)).collect();

                if valid != want_valid || warnings != want_warnings {
                    wrong.push(format!("{name}: Rust isValid {valid} {warnings:?}, X_ITE isValid {want_valid} {want_warnings:?}"));
                }
            }

            let _ = std::fs::remove_dir_all(&dir);
            assert!(cases.len() >= 10, "baseline has {} cases", cases.len());
            assert!(wrong.is_empty(), "{} of {} differ:\n{}", wrong.len(), cases.len(), wrong.join("\n"));
        })
        .unwrap()
        .join()
        .unwrap();
}
