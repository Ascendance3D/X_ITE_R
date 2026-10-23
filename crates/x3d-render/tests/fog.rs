//! Fog against the pinned X_ITE (oracle/x_ite-fog.json, written by oracle/x_ite-fog.mjs):
//! the fogType / visibility values X3DFogObject computes, and the fog factor of the real
//! Fog2.glsl.js shader chunk evaluated on the GPU.

use serde_json::Value;
use std::path::PathBuf;
use x3d_render::fog::{fog_interpolant, fog_type};

fn oracle() -> Option<Value> {
    let path = PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("../../oracle/x_ite-fog.json");

    serde_json::from_slice(&std::fs::read(path).ok()?).ok()
}

#[test]
fn fog_node_type_and_visibility() {
    let Some(o) = oracle() else {
        eprintln!("oracle/x_ite-fog.json missing; run node oracle/x_ite-fog.mjs");
        return;
    };
    let cases = o["nodes"].as_array().unwrap();

    for c in cases {
        let (t, start, range) = fog_type(
            c["fogType"].as_str().unwrap(),
            c["start"].as_f64().unwrap(),
            c["range"].as_f64().unwrap(),
            false,
        );

        assert_eq!(t as f64, c["jsFogType"].as_f64().unwrap(), "{c}");
        assert_eq!(start, c["jsStart"].as_f64().unwrap(), "{c}");
        assert_eq!(range, c["jsRange"].as_f64().unwrap(), "{c}");
    }

    eprintln!("{} fog nodes", cases.len());
}

#[test]
fn fog_interpolant_matches_the_shader() {
    let Some(o) = oracle() else {
        eprintln!("oracle/x_ite-fog.json missing; run node oracle/x_ite-fog.mjs");
        return;
    };
    let samples = o["interpolant"].as_array().unwrap();
    let mut worst = 0.0f64;

    for s in samples {
        let m: Vec<f32> = s["matrix"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();
        let v: Vec<f32> = s["vertex"]
            .as_array()
            .unwrap()
            .iter()
            .map(|v| v.as_f64().unwrap() as f32)
            .collect();

        // length (x3d_Fog .matrix * vertex), column-major mat3 like GLSL.
        let t = [
            m[0] * v[0] + m[3] * v[1] + m[6] * v[2],
            m[1] * v[0] + m[4] * v[1] + m[7] * v[2],
            m[2] * v[0] + m[5] * v[1] + m[8] * v[2],
        ];
        let distance = (t[0] * t[0] + t[1] * t[1] + t[2] * t[2]).sqrt();
        let mine = fog_interpolant(
            s["type"].as_u64().unwrap() as u32,
            s["start"].as_f64().unwrap() as f32,
            s["range"].as_f64().unwrap() as f32,
            distance,
        ) as f64;
        let theirs = s["value"].as_f64().unwrap();
        let err = (mine - theirs).abs();

        worst = worst.max(err);
        assert!(err <= 2e-5 + 1e-5 * theirs.abs(), "{s}: {mine} vs {theirs}");
    }

    eprintln!(
        "{} interpolant samples, worst error {worst:e}",
        samples.len()
    );
}
