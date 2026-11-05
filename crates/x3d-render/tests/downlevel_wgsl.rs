//! The downlevel profile of the renderer's shaders (WebGL2): `x3d_render::gpu::downlevel_wgsl` removes every storage
//! buffer and the cube map array, and what is left is valid WGSL for a device without them. The browser smoke test
//! (crates/x3d-web/tests/preview/smoke.mjs --backend webgl2) draws with these shaders; this test keeps them valid
//! natively, without a GPU.

use x3d_render::gpu::{
    DOWNLEVEL_INSTANCES, DOWNLEVEL_SHADOW_INSTANCES, DOWNLEVEL_UNIFORM_BLOCK, downlevel_wgsl,
};

const X3D: &str = include_str!("../src/x3d.wgsl");
const SHADOW: &str = include_str!("../src/shadow.wgsl");
const DEPTH: &str = include_str!("../src/depth.wgsl");

/// What WebGL2 does not have: storage buffers (vertex and fragment), cube map arrays. Validated with every other
/// capability, as wgpu's GL backend asks naga for them.
fn validate(src: &str) -> Result<(), String> {
    let module = naga::front::wgsl::parse_str(src).map_err(|e| e.emit_to_string(src))?;
    let caps = naga::valid::Capabilities::all() - naga::valid::Capabilities::CUBE_ARRAY_TEXTURES;

    naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps)
        .validate(&module)
        .map(drop)
        .map_err(|e| e.emit_to_string(src))
}

#[test]
fn x3d_wgsl_downlevel_has_no_storage_and_no_cube_array_and_is_valid() {
    let s = downlevel_wgsl(X3D);

    assert!(!s.contains("var<storage"), "a storage buffer is left");
    assert!(
        !s.contains("texture_cube_array"),
        "the cube map array is left"
    );
    assert!(!s.contains("shadow_maps["));
    assert!(!s.contains("scatter_color["));
    assert!(!s.contains("scatter_depth["));
    assert!(s.contains(&format!(
        "var<uniform> instances: array<Instance, {DOWNLEVEL_INSTANCES}>;"
    )));
    assert_eq!(
        s.matches("env_sample_level(").count(),
        X3D.matches("textureSampleLevel(env_maps, env_s, ").count() + 1,
        "every environment sample uses env_sample_level (and its definition)"
    );
    validate(&s).unwrap();
}

#[test]
fn x3d_wgsl_unchanged_is_not_valid_without_cube_arrays() {
    // The test above would prove nothing if the full profile were valid without the capability too.
    assert!(validate(X3D).is_err());
}

#[test]
fn shadow_and_depth_wgsl_downlevel_read_instances_from_a_uniform_block() {
    for (name, src) in [("shadow.wgsl", SHADOW), ("depth.wgsl", DEPTH)] {
        let s = downlevel_wgsl(src);

        assert!(!s.contains("var<storage, read> instances"), "{name}");
        assert!(
            s.contains(&format!(
                "var<uniform> instances: array<mat4x4<f32>, {DOWNLEVEL_SHADOW_INSTANCES}>;"
            )),
            "{name}"
        );
        validate(&s).unwrap_or_else(|e| panic!("{name}: {e}"));
    }
}

#[test]
fn every_downlevel_uniform_block_fits_in_16_kib() {
    // WebGL2's minimum MAX_UNIFORM_BLOCK_SIZE, and the value of ANGLE on Metal (Apple M1, Chrome 156).
    let caps = naga::valid::Capabilities::all() - naga::valid::Capabilities::CUBE_ARRAY_TEXTURES;
    for (name, src) in [
        ("x3d.wgsl", X3D),
        ("shadow.wgsl", SHADOW),
        ("depth.wgsl", DEPTH),
    ] {
        let s = downlevel_wgsl(src);
        let module = naga::front::wgsl::parse_str(&s).unwrap();
        naga::valid::Validator::new(naga::valid::ValidationFlags::all(), caps)
            .validate(&module)
            .unwrap();
        for (_, var) in module.global_variables.iter() {
            if var.space == naga::AddressSpace::Uniform {
                let size = module.types[var.ty].inner.size(module.to_ctx());
                assert!(
                    size as usize <= DOWNLEVEL_UNIFORM_BLOCK,
                    "{name}: uniform {:?} has {size} bytes",
                    var.name
                );
            }
        }
    }
}
