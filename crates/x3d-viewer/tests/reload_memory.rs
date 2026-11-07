//! Issue #462 section D: a viewer that loads world after world keeps no memory of the old ones. The renderer kept the
//! meshes of each world it drew until they aged out (KEEP_FRAMES frames), so 35 reloads of a synthetic 1,000-shape
//! world with 5 frames each grew the live heap by about 91 MB; `Renderer::release_unreferenced` on unload drops them.
//! The test binary counts live heap bytes with its own global allocator.

mod common;

use std::alloc::{GlobalAlloc, Layout, System};
use std::sync::atomic::{AtomicIsize, Ordering};
use x3d_viewer::embed::{EmbeddedViewer, LoadOptions, spawn_big_stack};
use x3d_viewer::viewer::Viewer3d;

struct Counting;

static LIVE: AtomicIsize = AtomicIsize::new(0);

unsafe impl GlobalAlloc for Counting {
    // Counted only when the system allocator succeeded: a failed realloc leaves the old block live.
    unsafe fn alloc(&self, l: Layout) -> *mut u8 {
        let p = unsafe { System.alloc(l) };
        if !p.is_null() {
            LIVE.fetch_add(l.size() as isize, Ordering::Relaxed);
        }
        p
    }

    unsafe fn dealloc(&self, p: *mut u8, l: Layout) {
        unsafe { System.dealloc(p, l) };
        LIVE.fetch_sub(l.size() as isize, Ordering::Relaxed);
    }

    unsafe fn realloc(&self, p: *mut u8, l: Layout, n: usize) -> *mut u8 {
        let q = unsafe { System.realloc(p, l, n) };
        if !q.is_null() {
            LIVE.fetch_add(n as isize - l.size() as isize, Ordering::Relaxed);
        }
        q
    }
}

#[global_allocator]
static ALLOCATOR: Counting = Counting;

const T0: f64 = 946684800.0;
const MIB: isize = 1 << 20;

/// 1,000 separate red Boxes (newly authored, synthetic).
fn world(name: &str) -> std::path::PathBuf {
    let dir = std::path::PathBuf::from(env!("CARGO_TARGET_TMPDIR")).join(name);
    std::fs::create_dir_all(&dir).unwrap();
    let file = dir.join("w.x3dv");
    let mut s = String::from("#X3D V4.0 utf8\nPROFILE Interchange\nTransform { children [\n");

    for i in 0..1000 {
        s += &format!(
            "Transform {{ translation {} {} 0 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 1 0 0 }} }} geometry Box {{ size 0.1 0.1 0.1 }} }} }}\n",
            i % 40,
            i / 40
        );
    }

    s += "] }\n";
    std::fs::write(&file, s).unwrap();
    file
}

/// Live bytes after the 5th load and after the 40th: one world is about 3.6 MB.
fn growth(after: &[isize]) -> isize {
    after[39] - after[4]
}

#[test]
fn reloads_in_one_embedded_viewer_keep_no_old_world() {
    let Some((device, queue)) = common::gpu::device() else {
        return x3d_evidence::no_gpu("no adapter, skipped");
    };
    let file = world("reload_memory_embedded");

    spawn_big_stack(move || {
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: None,
            size: wgpu::Extent3d {
                width: 256,
                height: 256,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        let mut v = EmbeddedViewer::new(device, queue, (256, 256));
        let mut after = vec![];

        for c in 0..40 {
            v.load(&file, LoadOptions::default()).unwrap();

            for i in 1..=5 {
                let now = T0 + c as f64 + i as f64 / 60.0;
                v.frame(now, &view, format, 256, 256).unwrap();
            }

            after.push(LIVE.load(Ordering::Relaxed));
        }

        assert!(
            growth(&after) < 4 * MIB,
            "live heap grew {} KiB from the 5th to the 40th load",
            growth(&after) / 1024
        );
    });
}

/// The same without a GPU: each load makes a new viewer and drops the old one.
#[test]
fn new_viewers_for_each_load_keep_no_old_world() {
    let file = world("reload_memory_cpu");
    let mut after = vec![];

    for c in 0..40 {
        let mut v = Viewer3d::load_opts(&file, None, T0, 64, 64, None, true).expect("load");

        for i in 1..=5 {
            v.step(T0 + c as f64 + i as f64 / 60.0);
        }

        drop(v);
        after.push(LIVE.load(Ordering::Relaxed));
    }

    assert!(
        growth(&after) < 4 * MIB,
        "live heap grew {} KiB from the 5th to the 40th load",
        growth(&after) / 1024
    );
}
