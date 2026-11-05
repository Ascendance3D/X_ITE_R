//! The renderer's downlevel profile (WebGL2) on a real device: a native adapter asked for no storage buffers
//! (`max_storage_buffers_per_shader_stage` 0) is a downlevel device for `x3d_render::gpu::is_downlevel`, so the
//! renderer takes the same paths as in a WebGL2 browser: uniform blocks for the instances, windows of instances bound
//! per draw, the downlevel frame block.
//!
//! Each case renders one synthetic world on the full device and on the downlevel device and requires the same image
//! (at most 0.5% of the pixels more than 8 off, the bound of `x3d-shot compare`). Without the windowing a downlevel
//! draw reads past its window, and without the downlevel frame structs a volume pipeline is invalid.
//!
//! `DOWNLEVEL_DUMP=<dir>` writes both images of each case to `<dir>`.
//!
//! The cases need a GPU; without an adapter they are skipped.

mod common;

use std::path::{Path, PathBuf};
use x3d_render::gpu::{
    DOWNLEVEL_INSTANCES, DOWNLEVEL_SHADOW_INSTANCES, Gpu, Renderer, is_downlevel,
};

const W: u32 = 160;
const H: u32 = 120;

/// A device of the `X3D_BACKEND` adapter without storage buffers (the downlevel profile), one start at a time.
fn downlevel_gpu() -> Option<Gpu> {
    let full = common::gpu::gpu().ok()?;
    let mut desc = wgpu::InstanceDescriptor::new_without_display_handle();
    desc.backends =
        x3d_render::gpu::headless_backends(std::env::var("X3D_BACKEND").ok().as_deref()).ok()?;
    let instance = wgpu::Instance::new(desc);
    let adapter = pollster::block_on(instance.request_adapter(&wgpu::RequestAdapterOptions {
        power_preference: wgpu::PowerPreference::HighPerformance,
        ..Default::default()
    }))
    .ok()?;
    let mut limits = adapter.limits();
    limits.max_storage_buffers_per_shader_stage = 0;
    limits.max_storage_textures_per_shader_stage = 0;
    // WebGL2 guarantees 16 KiB uniform blocks; the downlevel blocks are sized for that (DOWNLEVEL_UNIFORM_BLOCK).
    limits.max_uniform_buffer_binding_size = limits.max_uniform_buffer_binding_size.min(16384);
    let (device, queue) = pollster::block_on(adapter.request_device(&wgpu::DeviceDescriptor {
        label: Some("x3d-render downlevel"),
        required_features: Renderer::wanted_features(&adapter),
        required_limits: limits,
        ..Default::default()
    }))
    .ok()?;
    assert!(is_downlevel(&device));
    drop(full);
    Some(Gpu {
        adapter_info: adapter.get_info(),
        renderer: Renderer::new(device.clone(), queue.clone()),
        device,
        queue,
    })
}

fn oracle() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
        .join("../../oracle")
        .canonicalize()
        .unwrap()
}

/// A world file in a directory of its own below `CARGO_TARGET_TMPDIR` (render_file reads a file).
///
/// The directory name holds the process ID and the file name, so two tests (threads of one process, or processes
/// of nextest) never share it. When the test passes, drop removes the directory. When the test fails (a panic), the
/// directory stays for the diagnosis and its path is printed.
struct World {
    dir: PathBuf,
    path: PathBuf,
}

impl World {
    fn new(name: &str, text: &str) -> Self {
        let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!(
            "x3d-render-downlevel-{}-{name}",
            std::process::id()
        ));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join(name);
        std::fs::write(&path, text).unwrap();
        Self { dir, path }
    }
}

impl Drop for World {
    fn drop(&mut self) {
        if std::thread::panicking() {
            eprintln!("world kept for diagnosis: {}", self.path.display());
        } else {
            let _ = std::fs::remove_dir_all(&self.dir);
        }
    }
}

/// The full and the downlevel image of `file`, and the share of pixels more than 8 off.
fn compare(file: &Path) -> Option<(f64, usize)> {
    let mut full = common::gpu::gpu().ok()?;
    let mut down = downlevel_gpu()?;
    let a = x3d_render::render_file(&mut full, &oracle(), file, W, H).unwrap();
    let b = x3d_render::render_file(&mut down, &oracle(), file, W, H).unwrap();
    if let Ok(dir) = std::env::var("DOWNLEVEL_DUMP") {
        let name = file.file_stem().unwrap().to_string_lossy();
        x3d_render::write_png(
            &Path::new(&dir).join(format!("{name}-full.png")),
            W,
            H,
            &a.rgba,
        )
        .unwrap();
        x3d_render::write_png(
            &Path::new(&dir).join(format!("{name}-downlevel.png")),
            W,
            H,
            &b.rgba,
        )
        .unwrap();
    }
    let off = a
        .rgba
        .chunks(4)
        .zip(b.rgba.chunks(4))
        .filter(|(p, q)| p.iter().zip(q.iter()).any(|(x, y)| x.abs_diff(*y) > 8))
        .count();
    let lit = a
        .rgba
        .chunks(4)
        .filter(|p| p[..3].iter().any(|&c| c > 40))
        .count();
    Some((off as f64 / (W * H) as f64, lit))
}

/// An InstancedShape of `n` boxes in a square grid, each with its own translation, `step` apart and `size` wide; and
/// the number of boxes on a side.
fn instanced_shape(n: usize, step: f64, size: f64) -> (usize, String) {
    let side = (n as f64).sqrt().ceil() as usize;
    let translations: Vec<String> = (0..n)
        .map(|k| {
            let (x, y) = ((k % side) as f64, (k / side) as f64);
            format!(
                "{:.4} {:.4} 0",
                (x - side as f64 / 2.0) * step,
                (y - side as f64 / 2.0) * step
            )
        })
        .collect();
    let shape = format!(
        "InstancedShape {{ translations [ {} ]\n  appearance Appearance {{ material Material {{ diffuseColor 0.9 0.5 0.2 }} }}\n  \
         geometry Box {{ size {size} {size} {size} }} }}\n",
        translations.join(", ")
    );
    (side, shape)
}

/// A world of `n` instanced boxes, seen whole.
fn instanced_grid(n: usize) -> String {
    let (side, shape) = instanced_shape(n, 0.5, 0.4);
    format!(
        "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT X_ITE:1\nViewpoint {{ position 0 0 {} }}\nNavigationInfo {{ headlight TRUE }}\n{shape}",
        side as f64 * 0.75
    )
}

#[test]
fn downlevel_instanced_draw_over_one_window_matches_the_full_device() {
    // More than two windows, and a count that ends inside a window.
    let n = DOWNLEVEL_INSTANCES * 2 + 37;
    let world = World::new("instanced.x3dv", &instanced_grid(n));
    let Some((off, lit)) = compare(&world.path) else {
        return x3d_evidence::no_gpu("no adapter");
    };
    assert!(
        lit > (W * H / 10) as usize,
        "the instances are drawn ({lit} lit pixels)"
    );
    assert!(
        off <= 0.005,
        "{:.2}% of the pixels differ between the full and the downlevel device",
        off * 100.0
    );
}

#[test]
fn downlevel_depth_map_with_more_instances_than_one_window_matches_the_full_device() {
    // A RenderedTexture depthMap pass (encode_depth_color) of more than DOWNLEVEL_SHADOW_INSTANCES instances.
    // The boxes are 1 m from the pass's viewpoint, where a box is about 40 levels darker than the clear color, so a box
    // that is drawn with another instance's matrix changes the depth image by much more than 8 levels.
    let n = DOWNLEVEL_SHADOW_INSTANCES + 120;
    let (_, body) = instanced_shape(n, 0.05, 0.04);
    let text = format!(
        "#X3D V4.1 utf8\nPROFILE Interchange\nCOMPONENT X_ITE:1\nCOMPONENT Texturing:4\nViewpoint {{ position 0 0 3 }}\n\
         Shape {{ appearance Appearance {{ material UnlitMaterial {{ }} texture RenderedTexture {{ depthMap TRUE width 128 height 128 \
         children [ Viewpoint {{ position 0 0 4 }} NavigationInfo {{ visibilityLimit 8 }} Transform {{ translation 0 0 3 children {body} }} ] }} }}\n  \
         geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -1.3 -1 0, 1.3 -1 0, 1.3 1 0, -1.3 1 0 ] }} \
         coordIndex [ 0 1 2 3 -1 ] texCoord TextureCoordinate {{ point [ 0 0, 1 0, 1 1, 0 1 ] }} }} }}\n"
    );
    let world = World::new("depth-map.x3dv", &text);
    let Some((off, lit)) = compare(&world.path) else {
        return x3d_evidence::no_gpu("no adapter");
    };
    assert!(lit > 0, "the depth map is drawn");
    assert!(
        off <= 0.005,
        "{:.2}% of the pixels differ between the full and the downlevel device",
        off * 100.0
    );
}

#[test]
fn downlevel_volume_pipeline_is_valid_and_matches_the_full_device() {
    let file = oracle().join("synthetic-rea-b07-vol/static/bd-default.wrl");
    let Some((off, lit)) = compare(&file) else {
        return x3d_evidence::no_gpu("no adapter");
    };
    assert!(lit > 0, "the volume is drawn");
    assert!(
        off <= 0.005,
        "{:.2}% of the pixels differ between the full and the downlevel device",
        off * 100.0
    );
}

#[test]
fn downlevel_rendered_texture_depth_map_matches_the_full_device() {
    let file = oracle().join("synthetic-rea-b03/rt-depth.wrl");
    let Some((off, _)) = compare(&file) else {
        return x3d_evidence::no_gpu("no adapter");
    };
    assert!(
        off <= 0.005,
        "{:.2}% of the pixels differ between the full and the downlevel device",
        off * 100.0
    );
}
