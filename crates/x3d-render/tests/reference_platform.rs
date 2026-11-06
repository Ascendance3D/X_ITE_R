//! The choice of the pinned picture of an x3d-render test (`common::reference`): the NVIDIA picture by default, a platform
//! picture only when the platform is named and the file has one, and a named platform only on its own kind of adapter. No GPU needed.

mod common;

use common::reference::{adapter_matches, png_for};

#[test]
fn a_platform_picture_is_used_only_when_the_platform_is_named_and_the_file_has_one() {
    let root =
        std::env::temp_dir().join(format!("render-reference-platform-{}", std::process::id()));
    let own = root.join("render-x-baseline");
    let m1 = root.join("render-x-apple-m1-baseline");
    std::fs::create_dir_all(&own).unwrap();
    std::fs::create_dir_all(&m1).unwrap();
    for f in ["a.png", "b.png"] {
        std::fs::write(own.join(f), "").unwrap();
    }
    std::fs::write(m1.join("b.png"), "").unwrap();

    assert_eq!(
        png_for(&root, "render-x-baseline", "b.png", None),
        own.join("b.png")
    );
    assert_eq!(
        png_for(&root, "render-x-baseline", "b.png", Some("apple-m1")),
        m1.join("b.png")
    );
    assert_eq!(
        png_for(&root, "render-x-baseline", "a.png", Some("apple-m1")),
        own.join("a.png")
    );

    std::fs::remove_dir_all(&root).unwrap();
}

#[test]
fn apple_m1_needs_an_apple_metal_adapter() {
    use wgpu::Backend::{Gl, Metal, Vulkan};

    assert!(adapter_matches("apple-m1", Metal, 0x106b, "Apple M1"));
    assert!(adapter_matches(
        "apple-m1",
        Metal,
        0,
        "Apple Paravirtual device"
    ));
    assert!(!adapter_matches(
        "apple-m1",
        Vulkan,
        0x10de,
        "NVIDIA GeForce RTX 3060 Ti"
    ));
    assert!(!adapter_matches("apple-m1", Gl, 0x106b, "Apple M1"));
    assert!(!adapter_matches(
        "apple-m1",
        Metal,
        0x1002,
        "AMD Radeon Pro 5500M"
    ));
    assert!(!adapter_matches("intel-uhd", Metal, 0x106b, "Apple M1"));
}

#[test]
#[should_panic(expected = "unknown platform")]
fn an_unknown_platform_stops_the_test() {
    // SAFETY: this test binary has no other thread that reads the environment at the same time.
    unsafe { std::env::set_var("X3D_REFERENCE_PLATFORM", "no-such-platform") };
    let _ = common::reference::platform();
}
