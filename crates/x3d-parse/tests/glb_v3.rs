//! GLB container versions 2 and 3 (X_ITE 16.7.0 GLBParser.js). Not a pin parity item: the pinned
//! reference (16.5.0) only knows version 2. All files are synthetic and built here.

const MAGIC: u32 = 0x46546C67;
const JSON_CHUNK: u32 = 0x4e4f534a;
const BIN_CHUNK: u32 = 0x004e4942;

const TRIANGLE_JSON: &str = r#"{"asset":{"version":"2.0"},"scene":0,"scenes":[{"nodes":[0]}],
"nodes":[{"mesh":0}],"meshes":[{"primitives":[{"attributes":{"POSITION":0}}]}],
"accessors":[{"bufferView":0,"componentType":5126,"count":3,"type":"VEC3",
"min":[0,0,0],"max":[1,1,0]}],"bufferViews":[{"buffer":0,"byteLength":36}],
"buffers":[{"byteLength":36}]}"#;

fn triangle_bin() -> Vec<u8> {
    [0f32, 0., 0., 1., 0., 0., 0., 1., 0.]
        .iter()
        .flat_map(|v| v.to_le_bytes())
        .collect()
}

fn pad(mut v: Vec<u8>, align: usize, fill: u8) -> Vec<u8> {
    while v.len() % align != 0 {
        v.push(fill);
    }
    v
}

/// A version 2 chunk: length, type, data padded to 4.
fn chunk2(ty: u32, data: &[u8], fill: u8) -> Vec<u8> {
    let data = pad(data.to_vec(), 4, fill);
    let mut c = (data.len() as u32).to_le_bytes().to_vec();
    c.extend(ty.to_le_bytes());
    c.extend(data);
    c
}

/// A version 3 chunk: type, encoding, 64-bit length, data padded to 8.
fn chunk3(ty: u32, encoding: u32, data: &[u8], fill: u8) -> Vec<u8> {
    let padded = pad(data.to_vec(), 8, fill);
    let mut c = ty.to_le_bytes().to_vec();
    c.extend(encoding.to_le_bytes());
    c.extend((padded.len() as u64).to_le_bytes());
    c.extend(padded);
    c
}

fn glb2(chunks: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = chunks.concat();
    let mut f = MAGIC.to_le_bytes().to_vec();
    f.extend(2u32.to_le_bytes());
    f.extend(((12 + body.len()) as u32).to_le_bytes());
    f.extend(body);
    f
}

fn glb3(chunks: &[Vec<u8>]) -> Vec<u8> {
    let body: Vec<u8> = chunks.concat();
    let mut f = MAGIC.to_le_bytes().to_vec();
    f.extend(3u32.to_le_bytes());
    f.extend(((16 + body.len()) as u64).to_le_bytes());
    f.extend(body);
    f
}

fn load(data: &[u8]) -> Result<String, String> {
    x3d_parse::parse_bytes(data, "file:///t/a.glb")
        .map(|s| s.to_vrml_string())
        .map_err(|e| e.to_string())
}

fn v3_triangle() -> Vec<u8> {
    glb3(&[
        chunk3(JSON_CHUNK, 0, TRIANGLE_JSON.as_bytes(), b' '),
        chunk3(BIN_CHUNK, 0, &triangle_bin(), 0),
    ])
}

fn set_u64(f: &mut [u8], at: usize, v: u64) {
    f[at..at + 8].copy_from_slice(&v.to_le_bytes());
}

#[test]
fn v2_regression_triangle() {
    let f = glb2(&[
        chunk2(JSON_CHUNK, TRIANGLE_JSON.as_bytes(), b' '),
        chunk2(BIN_CHUNK, &triangle_bin(), 0),
    ]);
    let out = load(&f).unwrap();

    assert!(out.contains("0 1 0"), "{out}");
}

#[test]
fn v3_triangle_matches_v2() {
    let v2 = glb2(&[
        chunk2(JSON_CHUNK, TRIANGLE_JSON.as_bytes(), b' '),
        chunk2(BIN_CHUNK, &triangle_bin(), 0),
    ]);
    let out3 = load(&v3_triangle()).unwrap();

    assert!(out3.contains("0 1 0"), "{out3}");
    assert_eq!(load(&v2).unwrap(), out3);
}

#[test]
fn wrong_magic_rejected() {
    let mut f = v3_triangle();
    f[0] = b'X';
    assert!(load(&f).is_err());
}

#[test]
fn unknown_version_rejected() {
    for version in [0u32, 1, 4, 0x0300_0000] {
        let mut f = v3_triangle();
        f[4..8].copy_from_slice(&version.to_le_bytes());
        assert!(load(&f).is_err(), "version {version}");
    }
}

#[test]
fn v3_length_mismatch_rejected() {
    let good = v3_triangle();
    let len = good.len() as u64;

    for bad in [len + 8, len - 8, 0, u64::MAX, len | (1 << 32)] {
        let mut f = good.clone();
        set_u64(&mut f, 8, bad);
        assert!(load(&f).is_err(), "length {bad}");
    }
    // A 32-bit length field (the version 2 layout) does not satisfy version 3.
    let mut f = good.clone();
    f[8..12].copy_from_slice(&(good.len() as u32).to_le_bytes());
    f[12..16].copy_from_slice(&[1, 0, 0, 0]);
    assert!(load(&f).is_err());
}

#[test]
fn v3_short_header_rejected() {
    let mut f = MAGIC.to_le_bytes().to_vec();
    f.extend(3u32.to_le_bytes());
    f.extend(12u32.to_le_bytes()); // fewer than the 16 header bytes
    assert!(load(&f).is_err());
}

#[test]
fn v3_truncated_chunk_header() {
    // 8 trailing bytes: valid length field, but less than a 16-byte chunk header.
    let f = glb3(&[vec![0u8; 8]]);
    assert!(
        load(&f)
            .unwrap_err()
            .ends_with("Error: GLB Parser: Invalid chuck header length.")
    );

    // A v3 file whose only content is a v2-sized (8 byte) chunk header.
    let f = glb3(&[
        chunk3(JSON_CHUNK, 0, TRIANGLE_JSON.as_bytes(), b' '),
        vec![0u8; 15],
    ]);
    assert!(
        load(&f)
            .unwrap_err()
            .ends_with("Error: GLB Parser: Invalid chuck header length.")
    );
}

#[test]
fn v3_chunk_length_beyond_file() {
    let mut f = glb3(&[chunk3(JSON_CHUNK, 0, TRIANGLE_JSON.as_bytes(), b' ')]);
    let n = f.len() as u64;
    set_u64(&mut f, 16 + 8, n); // chunk length larger than what follows
    assert!(
        load(&f)
            .unwrap_err()
            .ends_with("Error: GLB Parser: Invalid chunk length.")
    );

    set_u64(&mut f, 16 + 8, u64::MAX);
    assert!(
        load(&f)
            .unwrap_err()
            .ends_with("Error: GLB Parser: Invalid chunk length.")
    );
}

#[test]
fn v3_non_plain_encoding_is_skipped() {
    // X_ITE 16.7.0 neither errors nor decompresses: a chunk with encoding != 0 is ignored.
    // Non-plain JSON chunk: no JSON, so the scene is empty.
    let f = glb3(&[
        chunk3(JSON_CHUNK, 1, TRIANGLE_JSON.as_bytes(), b' '),
        chunk3(BIN_CHUNK, 0, &triangle_bin(), 0),
    ]);
    let out = load(&f).unwrap();
    assert!(!out.contains("Shape"), "{out}");

    // Non-plain buffer chunk: the buffer is missing, so the geometry cannot be read, while the
    // same file with a plain buffer chunk loads.
    let f = glb3(&[
        chunk3(JSON_CHUNK, 0, TRIANGLE_JSON.as_bytes(), b' '),
        chunk3(BIN_CHUNK, 2, &triangle_bin(), 0),
    ]);
    let out = load(&f).unwrap();
    assert!(!out.contains("0 1 0"), "{out}");
}

#[test]
fn v3_unknown_chunk_type_and_second_json_chunk() {
    let other = chunk3(0x1234_5678, 0, b"ignored", 0);
    let f = glb3(&[
        other.clone(),
        chunk3(JSON_CHUNK, 0, b"{\"not\":\"gltf\"}", b' '),
        chunk3(JSON_CHUNK, 0, TRIANGLE_JSON.as_bytes(), b' '),
        other,
        chunk3(BIN_CHUNK, 0, &triangle_bin(), 0),
    ]);
    assert!(load(&f).unwrap().contains("0 1 0"));
}

#[test]
fn v3_alignment_uses_8_bytes() {
    // The JSON chunk declares its unpadded length (not a multiple of 8); the next chunk header
    // sits at the next 8-byte boundary, as GLBParser aligns after every chunk.
    let json = TRIANGLE_JSON.as_bytes();
    let mut json_padded = json.to_vec();
    while json_padded.len() % 8 != 3 {
        json_padded.push(b' ');
    }
    let declared = json_padded.len();
    let mut c = JSON_CHUNK.to_le_bytes().to_vec();
    c.extend(0u32.to_le_bytes());
    c.extend((declared as u64).to_le_bytes());
    c.extend(&json_padded);
    c.extend(vec![0u8; (8 - declared % 8) % 8]);
    let f = glb3(&[c, chunk3(BIN_CHUNK, 0, &triangle_bin(), 0)]);
    assert!(load(&f).unwrap().contains("0 1 0"));

    // A 4-byte (version 2) alignment does not resync a version 3 file: the next chunk header
    // is read at the wrong place and the container fails.
    let mut c = JSON_CHUNK.to_le_bytes().to_vec();
    c.extend(0u32.to_le_bytes());
    c.extend((declared as u64).to_le_bytes());
    c.extend(&json_padded);
    c.extend(vec![0u8; (4 - declared % 4) % 4]);
    let f = glb3(&[c, chunk3(BIN_CHUNK, 0, &triangle_bin(), 0)]);
    assert!(load(&f).is_err() || !load(&f).unwrap().contains("0 1 0"));
}

#[test]
fn v3_trailing_padding_inside_final_chunk_ok() {
    // A file ending exactly at the end of the final (padded) chunk, and one ending with an empty
    // chunk, both load.
    let f = glb3(&[
        chunk3(JSON_CHUNK, 0, TRIANGLE_JSON.as_bytes(), b' '),
        chunk3(BIN_CHUNK, 0, &triangle_bin(), 0),
        chunk3(BIN_CHUNK, 0, b"", 0),
    ]);
    assert!(load(&f).unwrap().contains("0 1 0"));
}
