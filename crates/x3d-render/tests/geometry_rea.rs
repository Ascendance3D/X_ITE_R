//! REA full-port geometry oracle: oracle/synthetic-rea/geometry-baseline.json is written by
//! oracle/x_ite-geometry-rea.mjs from oracle/synthetic-rea/geometry-cases.json (pinned X_ITE,
//! X3D 4.0 scenes). Every array X3DGeometryNode builds must match element by element: vertices,
//! normals, colors, every texture coordinate channel, fog depths, tangents, bbox, type, solid, ccw.

use std::path::PathBuf;
use x3d_runtime::{LocalFetcher, Runtime};

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle")
}

fn build(header: &str, vrml: &str) -> Option<x3d_render::geometry::Geometry> {
    x3d_fields::jsmath::set_flavor(x3d_fields::jsmath::MathFlavor::Chrome);
    let source = format!("{header}DEF G {vrml}");
    let scene = x3d_parse::Parser::new(&source, "http://oracle/geometry.x3dv").parse().expect("parse");
    let mut rt = Runtime::new(scene, Box::new(LocalFetcher { root: oracle(), prefix: "http://oracle/".into() }));
    rt.load_world();
    let root = rt.scene.ctx(x3d_scene::ROOT).root_nodes[0].expect("node");
    x3d_render::traverse::geometry_of(&rt, root)
}

fn floats(v: &serde_json::Value) -> Vec<f64> {
    v.as_array().map(|a| a.iter().map(|x| x.as_f64().unwrap_or(f64::NAN)).collect()).unwrap_or_default()
}

fn close(name: &str, what: &str, got: &[f32], want: &[f64]) -> Result<(), String> {
    if got.len() != want.len() {
        return Err(format!("{name}: {what} length {} != {}", got.len(), want.len()));
    }
    for (i, (g, w)) in got.iter().zip(want).enumerate() {
        // Both sides are Float32 values: compare exactly (NaN equals NaN).
        if !((*g as f64) == *w || (g.is_nan() && w.is_nan())) {
            return Err(format!("{name}: {what}[{i}] {g} != {w}"));
        }
    }
    Ok(())
}

#[test]
fn rea_geometry_matches_x_ite() {
    let json: serde_json::Value = serde_json::from_slice(&std::fs::read(oracle().join("synthetic-rea/geometry-baseline.json")).unwrap()).unwrap();
    let cases = json["cases"].as_object().unwrap();
    let mut errors = vec![];

    for (name, c) in cases {
        let Some(g) = build(c["header"].as_str().unwrap(), c["vrml"].as_str().unwrap()) else {
            errors.push(format!("{name}: no geometry"));
            continue;
        };
        let check = || -> Result<(), String> {
            if g.geometry_type as i64 != c["geometryType"].as_i64().unwrap() {
                return Err(format!("{name}: geometryType {} != {}", g.geometry_type, c["geometryType"]));
            }
            if g.solid != c["solid"].as_bool().unwrap() || g.ccw != c["ccw"].as_bool().unwrap() {
                return Err(format!("{name}: solid/ccw {} {} != {} {}", g.solid, g.ccw, c["solid"], c["ccw"]));
            }
            close(name, "vertices", &g.vertices, &floats(&c["vertices"]))?;
            close(name, "normals", &g.normals, &floats(&c["normals"]))?;
            close(name, "colors", &g.colors, &floats(&c["colors"]))?;
            let channels = c["multiTexCoords"].as_array().unwrap();
            let want0 = channels.first().map(floats).unwrap_or_default();
            close(name, "multiTexCoords[0]", &g.tex_coords, &want0)?;
            let extra = channels.len().saturating_sub(1);
            if g.tex_coords_extra.len() != extra {
                return Err(format!("{name}: {} extra texture coordinate channels != {extra}", g.tex_coords_extra.len()));
            }
            for (k, ch) in channels.iter().skip(1).enumerate() {
                close(name, &format!("multiTexCoords[{}]", k + 1), &g.tex_coords_extra[k], &floats(ch))?;
            }
            let attribs = c["attribs"].as_array().cloned().unwrap_or_default();
            if g.attribs.len() != attribs.len() {
                return Err(format!("{name}: {} attrib arrays != {}", g.attribs.len(), attribs.len()));
            }
            for (k, a) in attribs.iter().enumerate() {
                close(name, &format!("attribs[{k}]"), &g.attribs[k].values, &floats(a))?;
            }
            close(name, "fogDepths", &g.fog_depths, &floats(&c["fogDepths"]))?;
            close(name, "tangents", &g.tangents, &floats(&c["tangents"]))?;
            let gmin = [g.min.x as f32, g.min.y as f32, g.min.z as f32];
            let gmax = [g.max.x as f32, g.max.y as f32, g.max.z as f32];
            close(name, "min", &gmin, &floats(&c["min"]))?;
            close(name, "max", &gmax, &floats(&c["max"]))
        };
        if let Err(e) = check() {
            errors.push(e);
        }
    }

    assert!(errors.is_empty(), "{} of {} cases differ:\n{}", errors.len(), cases.len(), errors.join("\n"));
}
