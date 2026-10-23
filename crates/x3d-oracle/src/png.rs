//! The PNG writer of the Node tools (`encodePNG` in oracle/x_ite-anim.mjs): 8-bit RGBA, filter 0 on every
//! row, one IDAT compressed at zlib level 9, IHDR / IDAT / IEND only. zlib-rs at level 9 produces the same
//! deflate stream as Node's zlib, so the files are byte-identical, not only pixel-identical.

fn crc32(data: &[u8]) -> u32 {
    static TABLE: std::sync::OnceLock<[u32; 256]> = std::sync::OnceLock::new();
    let t = TABLE.get_or_init(|| {
        let mut t = [0u32; 256];
        for (n, e) in t.iter_mut().enumerate() {
            let mut c = n as u32;
            for _ in 0..8 {
                c = if c & 1 != 0 { 0xedb8_8320 ^ (c >> 1) } else { c >> 1 };
            }
            *e = c;
        }
        t
    });
    let mut c = 0xffff_ffffu32;
    for &b in data {
        c = t[((c ^ b as u32) & 0xff) as usize] ^ (c >> 8);
    }
    c ^ 0xffff_ffff
}

fn chunk(out: &mut Vec<u8>, kind: &[u8; 4], data: &[u8]) {
    out.extend_from_slice(&(data.len() as u32).to_be_bytes());
    let start = out.len();
    out.extend_from_slice(kind);
    out.extend_from_slice(data);
    let crc = crc32(&out[start..]);
    out.extend_from_slice(&crc.to_be_bytes());
}

/// zlib stream of `data` at level 9 (window 15, memLevel 8, default strategy), as `deflateSync (data, { level: 9 })`.
pub fn zlib9(data: &[u8]) -> Vec<u8> {
    let mut buf = vec![0u8; zlib_rs::compress_bound(data.len())];
    let (out, rc) = zlib_rs::compress_slice(&mut buf, data, zlib_rs::DeflateConfig::new(9));
    assert_eq!(rc, zlib_rs::ReturnCode::Ok, "zlib-rs compress failed");
    out.to_vec()
}

/// `rgba` is top-down, `w * h * 4` bytes.
pub fn encode(w: u32, h: u32, rgba: &[u8]) -> Vec<u8> {
    let stride = w as usize * 4;
    let mut raw = vec![0u8; (stride + 1) * h as usize];
    for y in 0..h as usize {
        raw[y * (stride + 1) + 1..(y + 1) * (stride + 1)].copy_from_slice(&rgba[y * stride..(y + 1) * stride]);
    }
    let mut ihdr = [0u8; 13];
    ihdr[0..4].copy_from_slice(&w.to_be_bytes());
    ihdr[4..8].copy_from_slice(&h.to_be_bytes());
    ihdr[8] = 8;
    ihdr[9] = 6;
    let mut out = vec![137, 80, 78, 71, 13, 10, 26, 10];
    chunk(&mut out, b"IHDR", &ihdr);
    chunk(&mut out, b"IDAT", &zlib9(&raw));
    chunk(&mut out, b"IEND", &[]);
    out
}

/// The canvas read back with `gl.readPixels` is bottom-up; the PNG is top-down.
/// The PNG of a page's `PIXELS` reply (`gl.readPixels`, bottom-up RGBA): an error when the size is 0 or the buffer is
/// not `w * h * 4` bytes (WebGL's drawing buffer is never empty, so the Node tools never wrote such a PNG).
pub fn encode_bottom_up(w: u32, h: u32, bottom_up: &[u8]) -> Result<Vec<u8>, String> {
    if w == 0 || h == 0 || bottom_up.len() != w as usize * h as usize * 4 {
        return Err(format!("x3d-oracle: pixels: {} bytes for {w} x {h}", bottom_up.len()));
    }

    Ok(encode(w, h, &flip_rows(bottom_up, w, h)))
}

pub fn flip_rows(bottom_up: &[u8], w: u32, h: u32) -> Vec<u8> {
    let stride = w as usize * 4;
    let mut out = vec![0u8; bottom_up.len()];
    for y in 0..h as usize {
        let src = (h as usize - 1 - y) * stride;
        if src + stride <= bottom_up.len() && (y + 1) * stride <= out.len() {
            out[y * stride..(y + 1) * stride].copy_from_slice(&bottom_up[src..src + stride]);
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crc_and_structure() {
        assert_eq!(crc32(b"IEND"), 0xae42_6082);
        let png = encode(2, 1, &[255, 0, 0, 255, 0, 255, 0, 128]);
        assert_eq!(&png[..8], &[137, 80, 78, 71, 13, 10, 26, 10]);
        assert_eq!(&png[png.len() - 12..], &[0, 0, 0, 0, b'I', b'E', b'N', b'D', 0xae, 0x42, 0x60, 0x82]);
    }

    #[test]
    fn zlib_level9_matches_reference_zlib() {
        // zlib 1.3 (Python's zlib.compress (b"abcabcabc" * 50 + bytes (range (256)), 9)).
        let data: Vec<u8> = b"abcabcabc".repeat(50).into_iter().chain(0..=255u8).collect();
        let z = zlib9(&data);
        let want = "78da4b4c4a4e1c45431931303231b3b0b2b173707271f3f0f2f10b080a098b888a894b484a49cbc8cac92b282a29aba8aaa96b686a69ebe8eae91b181a199b989a995b585a59dbd8dad93b383a39bbb8bab97b787a79fbf8faf9070406058784868547444645c7c4c6c52700ed48494d4bcfc8cccacec9cdcb2f282c2a2e292d2bafa8acaaaea9adab6f686c6a6e696d6befe8eceaeee9edeb9f3071d2e42953a74d9f3173d6ec3973e7cd5fb070d1e2254b972d5fb172d5ea356bd7addfb071d3e62d5bb76ddfb173d7ee3d7bf7ed3f70f0d0e123478f1d3f71f2d4e93367cf9dbf70f1d2e52b57af5dbf71f3d6ed3b77efdd7ff0f0d1e3274f9f3d7ff1f2d5eb376fdfbdfff0f1d3e72f5fbf7dfff1f3d7ef3f7ffffd0700c36e2bd4";
        let hex: String = z.iter().map(|b| format!("{b:02x}")).collect();
        assert_eq!(hex, want);
        let mut back = vec![0u8; data.len()];
        let (d, rc) = zlib_rs::decompress_slice(&mut back, &z, zlib_rs::InflateConfig::default());
        assert_eq!(rc, zlib_rs::ReturnCode::Ok);
        assert_eq!(d, &data[..]);
    }

    #[test]
    fn flips() {
        assert_eq!(flip_rows(&[1, 1, 1, 1, 2, 2, 2, 2], 1, 2), vec![2, 2, 2, 2, 1, 1, 1, 1]);
        assert_eq!(encode_bottom_up(1, 2, &[1, 1, 1, 1, 2, 2, 2, 2]), Ok(encode(1, 2, &[2, 2, 2, 2, 1, 1, 1, 1])));
        assert!(encode_bottom_up(1, 2, &[1, 1, 1, 1]).is_err());
        assert!(encode_bottom_up(0, 0, &[]).is_err());
    }
}
