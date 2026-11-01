//! #73: glTF-level comparison of the compressed-geometry decoders (EXT_meshopt_compression,
//! KHR_draco_mesh_compression) against pinned X_ITE 16.5.0 on newly authored synthetic assets
//! (`oracle/synthetic-73-gltf-decoders`, see its PROVENANCE.txt). Baseline:
//! `oracle/gltf-decoders-73-baseline/gltf-baseline.json`, recorded with `oracle/x_ite-gltf.mjs`.
//! For every input X_ITE loaded, the Rust scene must print the same toVRMLString text; an input
//! X_ITE rejected must be rejected too.
//!
//! `GEN_73=1 cargo test -p x3d-parse --test gltf_decoders_73 generate_fixtures -- --ignored`
//! rewrites the fixtures (deterministic).

use draco_core::{
    DataType, EncoderBuffer, EncoderOptions, FaceIndex, GeometryAttributeType, Mesh, MeshEncoder,
    PointAttribute, PointCloud, PointCloudEncoder,
};
use std::path::{Path, PathBuf};

const B64: &[u8; 64] = b"ABCDEFGHIJKLMNOPQRSTUVWXYZabcdefghijklmnopqrstuvwxyz0123456789+/";

fn b64(data: &[u8]) -> String {
    let mut out = String::new();

    for c in data.chunks(3) {
        let n = (c[0] as u32) << 16
            | (*c.get(1).unwrap_or(&0) as u32) << 8
            | *c.get(2).unwrap_or(&0) as u32;

        for i in 0..4 {
            if i <= c.len() {
                out.push(B64[(n >> (18 - 6 * i) & 63) as usize] as char);
            } else {
                out.push('=');
            }
        }
    }

    out
}

fn fixture_dir() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../../oracle/synthetic-73-gltf-decoders")
}

fn f32s(v: &[f32]) -> Vec<u8> {
    v.iter().flat_map(|x| x.to_le_bytes()).collect()
}

// A fan of three triangles around vertex 0 (5 vertices), z = 0 plane, with normals and texcoords.
const POS: [f32; 15] = [
    0., 0., 0., 1., 0., 0., 1., 1., 0., 0., 1., 0., -1., 0.5, 0.25,
];
const NRM: [f32; 15] = [0., 0., 1., 0., 0., 1., 0., 0., 1., 0., 0., 1., 0., 0., 1.];
const UV: [f32; 10] = [0., 0., 1., 0., 1., 1., 0., 1., -1., 0.5];
const IDX: [u16; 9] = [0, 1, 2, 0, 2, 3, 0, 3, 4];

/// One meshopt-compressed bufferView: appends the encoded bytes to `buf`, returns the view JSON.
fn mo_view(
    buf: &mut Vec<u8>,
    encoded: &[u8],
    raw_len: usize,
    stride: usize,
    count: usize,
    mode: &str,
    filter: &str,
) -> String {
    let off = buf.len();

    buf.extend_from_slice(encoded);
    while buf.len() % 4 != 0 {
        buf.push(0);
    }

    format!(
        r#"{{"buffer":0,"byteLength":{raw_len},"extensions":{{"EXT_meshopt_compression":{{"buffer":0,"byteOffset":{off},"byteLength":{},"byteStride":{stride},"count":{count},"mode":"{mode}","filter":"{filter}"}}}}}}"#,
        encoded.len()
    )
}

fn mo_vertex(raw: &[u8], count: usize, stride: usize) -> Vec<u8> {
    let bound = meshoptimizer::encode_vertex_buffer_bound(count, stride).unwrap();
    let mut enc = vec![0u8; bound];
    let n = meshoptimizer::encode_vertex_buffer(&mut enc, bound, raw, count, stride).unwrap();

    enc.truncate(n);
    enc
}

fn gltf_doc(
    buffer: &[u8],
    ext: &str,
    views: &[String],
    accessors: &str,
    primitive: &str,
) -> String {
    format!(
        r#"{{"asset":{{"version":"2.0","generator":"x_ite_r-73"}},"extensionsUsed":["{ext}"],"extensionsRequired":["{ext}"],
"scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],
"meshes":[{{"primitives":[{primitive}]}}],
"accessors":[{accessors}],
"bufferViews":[{}],
"buffers":[{{"byteLength":{},"uri":"data:application/octet-stream;base64,{}"}}]}}
"#,
        views.join(","),
        buffer.len(),
        b64(buffer)
    )
}

fn meshopt_triangles() -> String {
    let mut buf = vec![];
    let mut views = vec![];

    views.push(mo_view(
        &mut buf,
        &mo_vertex(&f32s(&POS), 5, 12),
        60,
        12,
        5,
        "ATTRIBUTES",
        "NONE",
    ));
    views.push(mo_view(
        &mut buf,
        &mo_vertex(&f32s(&NRM), 5, 12),
        60,
        12,
        5,
        "ATTRIBUTES",
        "NONE",
    ));
    views.push(mo_view(
        &mut buf,
        &mo_vertex(&f32s(&UV), 5, 8),
        40,
        8,
        5,
        "ATTRIBUTES",
        "NONE",
    ));

    let mut enc = vec![0u8; meshoptimizer::encode_index_buffer_bound(9, 5).unwrap()];
    let size = enc.len();
    let n = meshoptimizer::encode_index_buffer(&mut enc, size, &IDX, 9).unwrap();

    views.push(mo_view(&mut buf, &enc[..n], 18, 2, 9, "TRIANGLES", "NONE"));

    gltf_doc(
        &buf,
        "EXT_meshopt_compression",
        &views,
        r#"{"bufferView":0,"componentType":5126,"count":5,"type":"VEC3","min":[-1,0,0],"max":[1,1,0.25]},
{"bufferView":1,"componentType":5126,"count":5,"type":"VEC3"},
{"bufferView":2,"componentType":5126,"count":5,"type":"VEC2"},
{"bufferView":3,"componentType":5123,"count":9,"type":"SCALAR"}"#,
        r#"{"attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2},"indices":3}"#,
    )
}

fn meshopt_filters() -> String {
    let mut buf = vec![];
    let mut views = vec![];

    // POSITION: EXPONENTIAL, stride 12 (three floats sharing no exponent).
    let mut pos = vec![0u8; 5 * 12];

    meshoptimizer::encode_filter_exp(
        &mut pos,
        5,
        12,
        15,
        &POS,
        meshoptimizer::EncodeExpMode::Separate,
    )
    .unwrap();
    views.push(mo_view(
        &mut buf,
        &mo_vertex(&pos, 5, 12),
        60,
        12,
        5,
        "ATTRIBUTES",
        "EXPONENTIAL",
    ));

    // NORMAL: OCTAHEDRAL, 8-bit, stride 4 (x y z w as signed normalized bytes).
    let n4: Vec<f32> = NRM
        .chunks(3)
        .flat_map(|n| [n[0], n[1], n[2], 1.0])
        .collect();
    let mut nrm = vec![0u8; 5 * 4];

    meshoptimizer::encode_filter_oct(&mut nrm, 5, 4, 8, &n4).unwrap();
    views.push(mo_view(
        &mut buf,
        &mo_vertex(&nrm, 5, 4),
        20,
        4,
        5,
        "ATTRIBUTES",
        "OCTAHEDRAL",
    ));

    // COLOR_0: COLOR filter, 8-bit RGBA, stride 4.
    let rgba: [f32; 20] = [
        1., 0., 0., 1., 0., 1., 0., 1., 0., 0., 1., 0.5, 1., 1., 0., 1., 0.25, 0.5, 0.75, 1.,
    ];
    let mut col = vec![0u8; 5 * 4];

    meshoptimizer::encode_filter_color(&mut col, 5, 4, 8, &rgba).unwrap();
    views.push(mo_view(
        &mut buf,
        &mo_vertex(&col, 5, 4),
        20,
        4,
        5,
        "ATTRIBUTES",
        "COLOR",
    ));

    // INDICES (sequence) for a LINE_STRIP, u32.
    let seq: [u32; 6] = [0, 1, 2, 3, 4, 0];
    let mut enc = vec![0u8; meshoptimizer::encode_index_sequence_bound(6, 5)];
    let size = enc.len();
    let n = meshoptimizer::encode_index_sequence(&mut enc, size, &seq, 6).unwrap();

    views.push(mo_view(&mut buf, &enc[..n], 24, 4, 6, "INDICES", "NONE"));

    gltf_doc(
        &buf,
        "EXT_meshopt_compression",
        &views,
        r#"{"bufferView":0,"componentType":5126,"count":5,"type":"VEC3","min":[-1,0,0],"max":[1,1,0.25]},
{"bufferView":1,"componentType":5120,"normalized":true,"count":5,"type":"VEC3"},
{"bufferView":2,"componentType":5121,"normalized":true,"count":5,"type":"VEC4"},
{"bufferView":3,"componentType":5125,"count":6,"type":"SCALAR"}"#,
        r#"{"attributes":{"POSITION":0,"NORMAL":1,"COLOR_0":2},"indices":3,"mode":3}"#,
    )
}

fn attr(kind: GeometryAttributeType, comps: u8, values: &[f32], points: usize) -> PointAttribute {
    let mut a = PointAttribute::new();

    a.init(kind, comps, DataType::Float32, false, points);

    for (i, v) in values.iter().enumerate() {
        a.buffer_mut().write(i * 4, &v.to_le_bytes());
    }

    a
}

fn draco_options() -> EncoderOptions {
    let mut options = EncoderOptions::new();

    options.set_global_int("encoding_speed", 10);
    options.set_global_int("decoding_speed", 10);
    options
}

fn draco_mesh_bytes() -> Vec<u8> {
    let mut mesh = Mesh::new();

    mesh.add_attribute(attr(GeometryAttributeType::Position, 3, &POS, 5));
    mesh.add_attribute(attr(GeometryAttributeType::Normal, 3, &NRM, 5));
    mesh.add_attribute(attr(GeometryAttributeType::TexCoord, 2, &UV, 5));
    mesh.set_num_faces(3);

    for f in 0..3usize {
        mesh.set_face(
            FaceIndex(f as u32),
            [
                u32::from(IDX[f * 3]).into(),
                u32::from(IDX[f * 3 + 1]).into(),
                u32::from(IDX[f * 3 + 2]).into(),
            ],
        );
    }

    let mut encoder = MeshEncoder::new();
    let mut out = EncoderBuffer::new();

    encoder.set_mesh(mesh);
    encoder
        .encode(&draco_options(), &mut out)
        .expect("encode mesh");
    out.data().to_vec()
}

fn draco_cloud_bytes() -> Vec<u8> {
    let mut cloud = PointCloud::new();

    cloud.set_num_points(5);
    cloud.add_attribute(attr(GeometryAttributeType::Position, 3, &POS, 5));

    let mut encoder = PointCloudEncoder::new();
    let mut out = EncoderBuffer::new();

    encoder.set_point_cloud(cloud);
    encoder
        .encode(&draco_options(), &mut out)
        .expect("encode cloud");
    out.data().to_vec()
}

fn draco_doc(stream: &[u8], accessors: &str, primitive: &str) -> String {
    gltf_doc(
        stream,
        "KHR_draco_mesh_compression",
        &[format!(r#"{{"buffer":0,"byteLength":{}}}"#, stream.len())],
        accessors,
        primitive,
    )
}

fn draco_triangles() -> String {
    draco_doc(
        &draco_mesh_bytes(),
        r#"{"componentType":5125,"count":9,"type":"SCALAR"},
{"componentType":5126,"count":5,"type":"VEC3","min":[-1,0,0],"max":[1,1,0.25]},
{"componentType":5126,"count":5,"type":"VEC3"},
{"componentType":5126,"count":5,"type":"VEC2"}"#,
        r#"{"attributes":{"POSITION":1,"NORMAL":2,"TEXCOORD_0":3},"indices":0,"extensions":{"KHR_draco_mesh_compression":{"bufferView":0,"attributes":{"POSITION":0,"NORMAL":1,"TEXCOORD_0":2}}}}"#,
    )
}

fn draco_points() -> String {
    draco_doc(
        &draco_cloud_bytes(),
        r#"{"componentType":5126,"count":5,"type":"VEC3","min":[-1,0,0],"max":[1,1,0.25]}"#,
        r#"{"attributes":{"POSITION":0},"mode":0,"extensions":{"KHR_draco_mesh_compression":{"bufferView":0,"attributes":{"POSITION":0}}}}"#,
    )
}

fn fixtures() -> Vec<(&'static str, String)> {
    vec![
        ("meshopt-triangles.gltf", meshopt_triangles()),
        ("meshopt-filters.gltf", meshopt_filters()),
        ("draco-triangles.gltf", draco_triangles()),
        ("draco-points.gltf", draco_points()),
    ]
}

#[test]
#[ignore]
fn generate_fixtures() {
    if std::env::var("GEN_73").is_err() {
        return;
    }

    std::fs::create_dir_all(fixture_dir()).unwrap();

    for (name, text) in fixtures() {
        std::fs::write(fixture_dir().join(name), text).unwrap();
    }
}

#[test]
fn fixtures_are_current() {
    for (name, text) in fixtures() {
        let on_disk = std::fs::read_to_string(fixture_dir().join(name)).unwrap();

        assert_eq!(on_disk, text, "{name} is stale: rerun generate_fixtures");
    }
}

#[test]
fn compressed_geometry_matches_x_ite() {
    let baseline: serde_json::Value = serde_json::from_str(
        &std::fs::read_to_string(
            Path::new(env!("CARGO_MANIFEST_DIR"))
                .join("../../oracle/gltf-decoders-73-baseline/gltf-baseline.json"),
        )
        .unwrap(),
    )
    .unwrap();
    let mut failures = vec![];
    let results = baseline["results"].as_array().unwrap();

    assert!(results.len() >= 4);

    for entry in results {
        let file = entry["file"].as_str().unwrap();

        if !file.ends_with(".gltf") {
            continue;
        }

        let path = fixture_dir().join(file);
        let url = format!("file://{}", path.canonicalize().unwrap().display());
        let parsed = x3d_parse::parse_bytes(&std::fs::read(&path).unwrap(), &url);

        if !entry["ok"].as_bool().unwrap() {
            if parsed.is_ok() {
                failures.push(format!("{file}: X_ITE rejected it, Rust parsed it"));
            }

            continue;
        }

        match parsed {
            Err(e) => failures.push(format!("{file}: X_ITE loaded it, Rust failed: {e}")),
            Ok(scene) => {
                let expected = entry["vrml"].as_str().unwrap();
                let actual = scene.to_vrml_string();

                if expected != actual {
                    let at = expected
                        .lines()
                        .zip(actual.lines())
                        .position(|(a, b)| a != b)
                        .map_or(0, |i| i + 1);

                    failures.push(format!(
                        "{file}: toVRMLString differs at line {at}:\n  X_ITE: {:?}\n  Rust:  {:?}",
                        expected.lines().nth(at.saturating_sub(1)),
                        actual.lines().nth(at.saturating_sub(1))
                    ));
                }
            }
        }
    }

    assert!(failures.is_empty(), "{}", failures.join("\n"));
}
