//! WebP texture bytes decode through the same `image::load_from_memory` path as PNG and JPEG
//! (ImageTexture, glTF images). Synthetic 2x2 images, encoded in the test (#73).

use image::{ImageFormat, Rgba, RgbaImage};

const PIXELS: [[u8; 4]; 4] = [
    [255, 0, 0, 255],
    [0, 255, 0, 128],
    [0, 0, 255, 255],
    [10, 20, 30, 0],
];

fn encode(format: ImageFormat) -> Vec<u8> {
    let mut img = RgbaImage::new(2, 2);
    for (i, p) in PIXELS.iter().enumerate() {
        img.put_pixel(i as u32 % 2, i as u32 / 2, Rgba(*p));
    }
    let mut out = Vec::new();
    img.write_to(&mut std::io::Cursor::new(&mut out), format)
        .unwrap();
    out
}

#[test]
fn lossless_webp_decodes_to_exact_pixels() {
    let bytes = encode(ImageFormat::WebP);
    assert_eq!(&bytes[..4], b"RIFF");
    assert_eq!(&bytes[8..12], b"WEBP");
    let img = image::load_from_memory(&bytes).unwrap().to_rgba8();
    assert_eq!(img.dimensions(), (2, 2));
    for (i, p) in PIXELS.iter().enumerate() {
        let got = img.get_pixel(i as u32 % 2, i as u32 / 2).0;
        // Fully transparent texels may lose their color; alpha must survive.
        if p[3] == 0 {
            assert_eq!(got[3], 0);
        } else {
            assert_eq!(&got, p);
        }
    }
}

#[test]
fn corrupt_webp_fails_like_corrupt_png() {
    let mut webp = encode(ImageFormat::WebP);
    webp.truncate(20);
    assert!(image::load_from_memory(&webp).is_err());

    let mut png = encode(ImageFormat::Png);
    png.truncate(20);
    assert!(image::load_from_memory(&png).is_err());

    let mut bad = b"RIFF\x04\0\0\0WEBP".to_vec();
    bad.extend_from_slice(b"junk");
    assert!(image::load_from_memory(&bad).is_err());
}
