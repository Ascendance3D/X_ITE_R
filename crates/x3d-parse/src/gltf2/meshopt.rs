//! meshoptimizer's decodeGltfBuffer (the decoder X_ITE loads as meshopt_decoder.js, built from
//! meshoptimizer 1.3). The codec is the `meshopt-pure` crate (MIT, a pure Rust port of
//! meshoptimizer 1.3, no C++ and no dependencies), so it also builds for wasm32.

use meshoptimizer as mo;

/// `decodeGltfBuffer (target, count, size, source, mode, filter)` of meshopt_decoder.js
/// (`decode`, line 48; `decodeGltfBuffer`, line 179), returning `target`. `len` is the length
/// of the `Uint8Array` target X_ITE allocates (`count * size` for conforming data).
pub(super) fn decode_gltf_buffer(
    len: usize,
    count: usize,
    stride: usize,
    encoded: &[u8],
    mode: &str,
    filter: &str,
) -> Result<Vec<u8>, String> {
    // `decoders [mode]` / `instance .exports [undefined]` is undefined: `fun (...)` throws.
    if !matches!(mode, "ATTRIBUTES" | "TRIANGLES" | "INDICES") {
        return Err("TypeError: fun is not a function".into());
    }

    // `count4 = (count + 3) & ~3`: the filters run over the padded count.
    let count4 = (count + 3) & !3;
    let work = count4.saturating_mul(stride);
    let mut heap = vec![0u8; work.max(count.saturating_mul(stride))];
    let used = count.saturating_mul(stride);

    let result = match mode {
        "ATTRIBUTES" => {
            mo::decode_vertex_buffer(&mut heap[..used], count, stride, encoded, encoded.len())
        }
        "TRIANGLES" | "INDICES" => decode_indices(
            mode == "TRIANGLES",
            &mut heap[..used],
            count,
            stride,
            encoded,
        ),
        _ => unreachable!(),
    };

    if let Err(e) = result {
        return Err(malformed(e));
    }

    // Unknown names (and NONE) have no export, so no filter runs.
    let filtered = match filter {
        "OCTAHEDRAL" => Some(mo::decode_filter_oct(&mut heap, count4, stride)),
        "QUATERNION" => Some(mo::decode_filter_quat(&mut heap, count4, stride)),
        "EXPONENTIAL" => Some(mo::decode_filter_exp(&mut heap, count4, stride)),
        "COLOR" => Some(mo::decode_filter_color(&mut heap, count4, stride)),
        _ => None,
    };

    if let Some(Err(e)) = filtered {
        return Err(malformed(e));
    }

    // `target .set (heap .subarray (tp, tp + count * size))` into a Uint8Array of `len` bytes.
    if used > len {
        return Err("RangeError: offset is out of bounds".into());
    }

    let mut target = vec![0u8; len];

    target[..used].copy_from_slice(&heap[..used]);
    Ok(target)
}

fn decode_indices(
    triangles: bool,
    out: &mut [u8],
    count: usize,
    size: usize,
    encoded: &[u8],
) -> Result<(), mo::Error> {
    macro_rules! run {
        ($t:ty) => {{
            let mut idx = vec![<$t>::default(); count];
            if triangles {
                mo::decode_index_buffer(&mut idx, count, encoded, encoded.len())?;
            } else {
                mo::decode_index_sequence(&mut idx, count, encoded, encoded.len())?;
            }
            for (chunk, v) in out.chunks_exact_mut(size).zip(idx) {
                chunk.copy_from_slice(&v.to_le_bytes());
            }
            Ok(())
        }};
    }

    match size {
        2 => run!(u16),
        4 => run!(u32),
        _ => Err(mo::Error::InvalidArgument("index size must be 2 or 4")),
    }
}

fn malformed(e: mo::Error) -> String {
    match e {
        mo::Error::Decode(code) => format!("Error: Malformed buffer data: {code}"),
        // The wasm build has no assertions; the nearest observable result is a failure status.
        _ => format!("Error: Malformed buffer data: {e}"),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn vertex_attributes_round_trip() {
        let src: Vec<u8> = (0..8 * 12).map(|i| (i * 7) as u8).collect();
        let bound = mo::encode_vertex_buffer_bound(12, 8).unwrap();
        let mut enc = vec![0u8; bound];
        let n = mo::encode_vertex_buffer(&mut enc, bound, &src, 12, 8).unwrap();
        let out = decode_gltf_buffer(96, 12, 8, &enc[..n], "ATTRIBUTES", "NONE").unwrap();
        assert_eq!(out, src);
    }

    #[test]
    fn triangles_and_indices_round_trip() {
        let idx: Vec<u16> = vec![0, 1, 2, 2, 1, 3, 3, 1, 4];
        let bytes: Vec<u8> = idx.iter().flat_map(|v| v.to_le_bytes()).collect();
        let mut enc = vec![0u8; mo::encode_index_buffer_bound(9, 5).unwrap()];
        let size = enc.len();
        let n = mo::encode_index_buffer(&mut enc, size, &idx, 9).unwrap();
        assert_eq!(
            decode_gltf_buffer(18, 9, 2, &enc[..n], "TRIANGLES", "NONE").unwrap(),
            bytes
        );

        let seq: Vec<u32> = vec![5, 6, 6, 9, 2];
        let bytes: Vec<u8> = seq.iter().flat_map(|v| v.to_le_bytes()).collect();
        let mut enc = vec![0u8; mo::encode_index_sequence_bound(5, 10)];
        let size = enc.len();
        let n = mo::encode_index_sequence(&mut enc, size, &seq, 5).unwrap();
        assert_eq!(
            decode_gltf_buffer(20, 5, 4, &enc[..n], "INDICES", "NONE").unwrap(),
            bytes
        );
    }

    #[test]
    fn errors_match_the_js_decoder() {
        assert_eq!(
            decode_gltf_buffer(8, 2, 4, &[], "BOGUS", "NONE").unwrap_err(),
            "TypeError: fun is not a function"
        );
        assert_eq!(
            decode_gltf_buffer(8, 2, 4, &[0x51], "ATTRIBUTES", "NONE").unwrap_err(),
            "Error: Malformed buffer data: -1"
        );
        assert_eq!(
            decode_gltf_buffer(8, 2, 4, &[], "ATTRIBUTES", "NONE").unwrap_err(),
            "Error: Malformed buffer data: -2"
        );
    }

    #[test]
    fn octahedral_filter_runs_over_padded_count() {
        // Three 4-byte vertices, all-zero attribute data encodes to a valid stream.
        let src = vec![0u8; 12];
        let bound = mo::encode_vertex_buffer_bound(3, 4).unwrap();
        let mut enc = vec![0u8; bound];
        let n = mo::encode_vertex_buffer(&mut enc, bound, &src, 3, 4).unwrap();
        let out = decode_gltf_buffer(12, 3, 4, &enc[..n], "ATTRIBUTES", "OCTAHEDRAL").unwrap();
        assert_eq!(out.len(), 12);
    }
}
