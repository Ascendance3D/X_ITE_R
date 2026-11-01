//! `KHR_draco_mesh_compression`: the Draco decode behind `dracoDecodeMesh` of
//! `GLTF2Parser.js` (lines 1970-2059), which X_ITE runs through `draco_decoder_gltf.js`.
//! The codec is the `draco-core` crate (Apache-2.0, pure Rust, `unsafe` forbidden), so it also
//! builds for wasm32.

use draco_core::{DecoderBuffer, Mesh, MeshDecoder, PointCloud};

/// What `dracoDecodeMesh` hands to its callbacks.
pub(super) struct Decoded {
    /// `indicesCallback` value: `GetTrianglesUInt32Array`, only for a triangular mesh.
    pub indices: Option<Vec<u32>>,
    /// `attributeCallback (key, array)` in `attributes` order: `GetAttributeDataArrayForAllPoints`
    /// with `DT_FLOAT32`, `num_points * num_components` values.
    pub attributes: Vec<(String, Vec<f32>)>,
}

/// `draco .TRIANGULAR_MESH` is 1 and `draco .POINT_CLOUD` is 0.
const POINT_CLOUD: i32 = 0;
const TRIANGULAR_MESH: i32 = 1;

/// `decoder .GetEncodedGeometryType (buffer)`: the type byte of the 11-byte header, or -1
/// (INVALID_GEOMETRY_TYPE) when the header does not parse.
fn encoded_geometry_type(data: &[u8]) -> i32 {
    if data.len() < 8 || &data[..5] != b"DRACO" {
        return -1;
    }

    i32::from(data[7])
}

/// `dracoDecodeMesh (draco, dataView, attributes, ...)`. `attributes` is the extension's
/// `attributes` object as (name, unique id) pairs.
pub(super) fn decode_mesh(data: &[u8], attributes: &[(String, u32)]) -> Result<Decoded, String> {
    let kind = encoded_geometry_type(data);
    let mut buffer = DecoderBuffer::new(data);
    let mut out = Decoded {
        indices: None,
        attributes: vec![],
    };

    let mut mesh = Mesh::new();
    let (geometry, num_points): (&PointCloud, usize) = match kind {
        TRIANGULAR_MESH => {
            MeshDecoder::new()
                .decode(&mut buffer, &mut mesh)
                .map_err(|e| format!("Error: {e}"))?;

            out.indices = Some(
                mesh.faces()
                    .iter()
                    .flat_map(|f| f.iter().map(|p| p.0))
                    .collect(),
            );
            (&mesh, mesh.num_points())
        }
        // The draco_decoder_gltf.js X_ITE ships answers a point cloud with an error (observed for a
        // sequentially encoded cloud): "Unsupported geometry type." (#73).
        POINT_CLOUD => return Err("Error: Unsupported geometry type.".into()),
        other => return Err(format!("Error: Invalid geometry type {other}.")),
    };

    for (key, id) in attributes {
        // `GetAttributeByUniqueId` is null for an unknown id and `num_components ()` throws.
        let Some(attribute) = geometry.attribute_by_unique_id(*id) else {
            return Err(
                "TypeError: Cannot read properties of null (reading 'num_components')".into(),
            );
        };
        let components = attribute.num_components() as usize;

        out.attributes
            .push((key.clone(), attribute.read_f32s(num_points, components)));
    }

    Ok(out)
}
