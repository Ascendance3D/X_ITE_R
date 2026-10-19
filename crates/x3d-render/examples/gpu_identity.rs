//! Prints the adapter the renderer's headless `Gpu::new` picks (final-gate proof that it is not a software adapter).
fn main() {
    let gpu = x3d_render::gpu::Gpu::new().expect("gpu");
    println!("{:?}", gpu.adapter_info);
}
