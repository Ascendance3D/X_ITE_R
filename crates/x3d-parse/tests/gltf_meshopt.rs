//! EXT_meshopt_compression (#73): a synthetic glTF whose position view is meshopt-encoded must
//! print the same scene as the identical glTF with the raw bytes. The fixture is generated here.

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

fn gltf(buffer: &[u8], view_ext: &str, view_len: usize) -> String {
    format!(
        r#"{{"asset":{{"version":"2.0"}},"extensionsUsed":["EXT_meshopt_compression"],
        "scene":0,"scenes":[{{"nodes":[0]}}],"nodes":[{{"mesh":0}}],
        "meshes":[{{"primitives":[{{"attributes":{{"POSITION":0}},"mode":0}}]}}],
        "accessors":[{{"bufferView":0,"componentType":5126,"count":4,"type":"VEC3",
          "min":[0,0,0],"max":[1,1,1]}}],
        "bufferViews":[{{"buffer":0,"byteLength":{view_len}{view_ext}}}],
        "buffers":[{{"byteLength":{},"uri":"data:application/octet-stream;base64,{}"}}]}}"#,
        buffer.len(),
        b64(buffer)
    )
}

#[test]
fn meshopt_view_matches_raw_view() {
    let positions: [f32; 12] = [0., 0., 0., 1., 0., 0., 0., 1., 0., 0., 0., 1.];
    let raw: Vec<u8> = positions.iter().flat_map(|v| v.to_le_bytes()).collect();

    let bound = meshoptimizer::encode_vertex_buffer_bound(4, 12).unwrap();
    let mut enc = vec![0u8; bound];
    let n = meshoptimizer::encode_vertex_buffer(&mut enc, bound, &raw, 4, 12).unwrap();
    enc.truncate(n);

    let ext = format!(
        r#","extensions":{{"EXT_meshopt_compression":{{"buffer":0,"byteOffset":0,
        "byteLength":{n},"byteStride":12,"count":4,"mode":"ATTRIBUTES"}}}}"#
    );

    let plain = x3d_parse::parse_bytes(gltf(&raw, "", 48).as_bytes(), "file:///t/a.gltf").unwrap();
    let packed = x3d_parse::parse_bytes(gltf(&enc, &ext, 48).as_bytes(), "file:///t/a.gltf")
        .expect("meshopt view decodes");

    assert_eq!(plain.to_vrml_string(), packed.to_vrml_string());
    assert!(packed.to_vrml_string().contains("0 1 0"));
}

#[test]
fn malformed_meshopt_data_is_rejected() {
    let ext = r#","extensions":{"EXT_meshopt_compression":{"buffer":0,"byteOffset":0,
        "byteLength":4,"byteStride":12,"count":4,"mode":"ATTRIBUTES"}}"#;
    let err = x3d_parse::parse_bytes(
        gltf(&[0x51, 0, 0, 0], ext, 48).as_bytes(),
        "file:///t/a.gltf",
    )
    .unwrap_err()
    .to_string();

    assert!(err.contains("Malformed buffer data: -1"), "{err}");
}
