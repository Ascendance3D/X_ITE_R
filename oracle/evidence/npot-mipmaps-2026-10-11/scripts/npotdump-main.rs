use x3d_render::texture::{TextureImage, mip_chain};
fn main() {
    let a: Vec<String> = std::env::args().collect();
    let img = image::open(&a[1]).unwrap().to_rgba8();
    let (w, h) = img.dimensions();
    let mut t = TextureImage::white();
    t.width = w; t.height = h; t.rgba = img.into_raw(); t.nearest = false;
    for (i, (lw, lh, d)) in mip_chain(&t).into_iter().enumerate() {
        std::fs::write(format!("{}.L{}.{}x{}.rgba", a[2], i, lw, lh), d).unwrap();
    }
}
