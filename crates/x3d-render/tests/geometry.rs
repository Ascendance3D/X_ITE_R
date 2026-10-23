//! Geometry generation against the pinned X_ITE: oracle/x_ite-geometry.json is written by
//! oracle/x_ite-geometry.mjs (X3DGeometryNode arrays after rebuild, headless Chrome). Each case is
//! a synthetic geometry node; vertex order, normals (creaseAngle), texture coordinates and colors
//! must match element by element.

use std::path::PathBuf;
use x3d_runtime::{LocalFetcher, Runtime};

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

fn build(vrml: &str) -> x3d_render::geometry::Geometry {
    x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);
    let source = format!("#VRML V2.0 utf8\nDEF G {vrml}");
    let scene = x3d_parse::Parser::new(&source, "http://oracle/geometry.wrl")
        .parse()
        .expect("parse");
    let mut rt = Runtime::new(
        scene,
        Box::new(LocalFetcher {
            root: oracle(),
            prefix: "http://oracle/".into(),
        }),
    );
    rt.load_world();
    let root = rt.scene.ctx(x3d_scene::ROOT).root_nodes[0].expect("node");
    x3d_render::traverse::geometry_of(&rt, root).expect("geometry")
}

fn floats(v: &serde_json::Value) -> Vec<f64> {
    v.as_array()
        .unwrap()
        .iter()
        .map(|x| x.as_f64().unwrap())
        .collect()
}

fn close(name: &str, what: &str, got: &[f32], want: &[f64], eps: f64) -> Result<(), String> {
    if got.len() != want.len() {
        return Err(format!(
            "{name}: {what} length {} != {}",
            got.len(),
            want.len()
        ));
    }
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        if (*g as f64 - w).abs() > eps {
            return Err(format!("{name}: {what}[{i}] {g} != {w}"));
        }
    }
    Ok(())
}

#[test]
fn geometry_matches_x_ite() {
    let json: serde_json::Value =
        serde_json::from_slice(&std::fs::read(oracle().join("x_ite-geometry.json")).unwrap())
            .unwrap();
    let cases = json["cases"].as_object().unwrap();
    let mut errors = vec![];

    for (name, c) in cases {
        let g = build(c["vrml"].as_str().unwrap());
        let check = || -> Result<(), String> {
            if g.geometry_type as i64 != c["geometryType"].as_i64().unwrap() {
                return Err(format!(
                    "{name}: geometryType {} != {}",
                    g.geometry_type, c["geometryType"]
                ));
            }
            if g.solid != c["solid"].as_bool().unwrap() || g.ccw != c["ccw"].as_bool().unwrap() {
                return Err(format!(
                    "{name}: solid/ccw {} {} != {} {}",
                    g.solid, g.ccw, c["solid"], c["ccw"]
                ));
            }
            // Float32Array values from X_ITE; ours are f32 computed in f64 like JS.
            close(name, "vertices", &g.vertices, &floats(&c["vertices"]), 1e-6)?;
            close(name, "normals", &g.normals, &floats(&c["normals"]), 1e-6)?;
            close(
                name,
                "texCoords",
                &g.tex_coords,
                &floats(&c["texCoords"]),
                1e-6,
            )?;
            close(name, "colors", &g.colors, &floats(&c["colors"]), 1e-6)?;
            let (min, max) = (floats(&c["min"]), floats(&c["max"]));
            let gmin = [g.min.x as f32, g.min.y as f32, g.min.z as f32];
            let gmax = [g.max.x as f32, g.max.y as f32, g.max.z as f32];
            close(name, "min", &gmin, &min, 1e-6)?;
            close(name, "max", &gmax, &max, 1e-6)
        };
        if let Err(e) = check() {
            errors.push(e);
        }
    }

    assert!(
        errors.is_empty(),
        "{} of {} cases differ:\n{}",
        errors.len(),
        cases.len(),
        errors.join("\n")
    );
}
