//! meshoptimizer's decodeGltfBuffer (the decoder X_ITE loads as meshopt_decoder.js).

pub(super) fn decode_gltf_buffer(
    _len: usize,
    _count: usize,
    _stride: usize,
    _encoded: &[u8],
    _mode: &str,
    _filter: &str,
) -> Result<Vec<u8>, String> {
    Err("Error: Meshopt decoder is not available.".into())
}
