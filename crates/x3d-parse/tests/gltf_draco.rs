//! KHR_draco_mesh_compression (#73): a synthetic glTF whose primitive carries a Draco-encoded
//! quad must print the scene the decoded geometry describes. The Draco stream is encoded here
//! with draco-core's own encoder (a dev-dependency); no external asset is used.

use draco_core::{
    DataType, EncoderBuffer, EncoderOptions, FaceIndex, GeometryAttributeType, Mesh, MeshEncoder,
    PointAttribute,
};

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

/// A unit quad in the z=0 plane as two triangles; the position attribute gets unique id 0.
fn quad() -> Vec<u8> {
    let mut mesh = Mesh::new();
    let mut pos = PointAttribute::new();

    pos.init(
        GeometryAttributeType::Position,
        3,
        DataType::Float32,
        false,
        4,
    );

    let points: [[f32; 3]; 4] = [[0., 0., 0.], [1., 0., 0.], [1., 1., 0.], [0., 1., 0.]];

    for (i, p) in points.iter().enumerate() {
        for (c, v) in p.iter().enumerate() {
            pos.buffer_mut().write((i * 3 + c) * 4, &v.to_le_bytes());
        }
    }

    mesh.add_attribute(pos);
    mesh.set_num_faces(2);
    mesh.set_face(FaceIndex(0), [0u32.into(), 1u32.into(), 2u32.into()]);
    mesh.set_face(FaceIndex(1), [0u32.into(), 2u32.into(), 3u32.into()]);

    let mut options = EncoderOptions::new();

    options.set_global_int("encoding_speed", 10);
    options.set_global_int("decoding_speed", 10);

    let mut encoder = MeshEncoder::new();
    let mut out = EncoderBuffer::new();

    encoder.set_mesh(mesh);
    encoder.encode(&options, &mut out).expect("encode");
    out.data().to_vec()
}

fn gltf(used: bool, buffer: &[u8]) -> String {
    let used = if used {
        r#""extensionsUsed":["KHR_draco_mesh_compression"],"#
    } else {
        ""
    };

    format!(
        r#"{{"asset":{{"version":"2.0"}},{used}
        "scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],
        "meshes":[{{"primitives":[{{"attributes":{{"POSITION":1}},"indices":0,
          "extensions":{{"KHR_draco_mesh_compression":{{"bufferView":0,"attributes":{{"POSITION":0}}}}}}}}]}}],
        "accessors":[
          {{"componentType":5125,"count":6,"type":"SCALAR"}},
          {{"componentType":5126,"count":4,"type":"VEC3","min":[0,0,0],"max":[1,1,0]}}],
        "bufferViews":[{{"buffer":0,"byteLength":{}}}],
        "buffers":[{{"byteLength":{},"uri":"data:application/octet-stream;base64,{}"}}]}}"#,
        buffer.len(),
        buffer.len(),
        b64(buffer)
    )
}

#[test]
fn draco_quad_decodes_to_its_geometry() {
    let scene = x3d_parse::parse_bytes(gltf(true, &quad()).as_bytes(), "file:///t/a.gltf")
        .expect("draco primitive decodes")
        .to_vrml_string();

    assert!(scene.contains("IndexedTriangleSet"), "{scene}");
    // Four unique corners and the two triangles, as encoded.
    assert!(
        scene.contains("index [\n      0,\n      1,\n      2,\n      0,\n      2,\n      3\n    ]"),
        "{scene}"
    );
    assert!(
        scene.contains(
            "point [\n        0 0 0,\n        1 0 0,\n        1 1 0,\n        0 1 0\n      ]"
        ),
        "{scene}"
    );
}

#[test]
fn garbage_draco_data_is_rejected() {
    let err = x3d_parse::parse_bytes(gltf(true, b"not draco!").as_bytes(), "file:///t/a.gltf")
        .unwrap_err()
        .to_string();

    assert!(err.contains("Invalid geometry type -1"), "{err}");
}

#[test]
fn draco_extension_not_declared_is_ignored() {
    // Without extensionsUsed X_ITE never loads the decoder: the (absent) accessor data is read
    // the normal way, so no Draco error is raised.
    let r = x3d_parse::parse_bytes(gltf(false, b"not draco!").as_bytes(), "file:///t/a.gltf");

    if let Err(e) = r {
        assert!(!e.to_string().contains("Invalid geometry type"), "{e}");
    }
}
