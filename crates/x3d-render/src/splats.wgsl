// WGSL translation of Browser/GaussianSplats/GaussianSplats.vs.js and GaussianSplats.fs.js of the
// pinned X_ITE (DISPLAY pass). crates/x3d-render/src/splats_gpu.rs composes the program: the frame
// uniform structs, the tone map and the fog functions are copied from x3d.wgsl at run time.
//
// The splat data are storage buffers indexed by splat index (X_ITE: float textures read with
// texelFetch at (index % textureWidth, index / textureWidth)); `indices` is the sorted index
// buffer that drives the instanced quads (the vertex attribute x3d_SplatIndex with divisor 1).

//@FRAME_STRUCTS@

@group(0) @binding(0) var<uniform> frame: FrameUniforms;

struct SplatUniforms {
    model_view: mat4x4<f32>,
    // inverse (x3d_ProjectionMatrix) of the uploaded projection
    inv_projection: mat4x4<f32>,
    // inverse (x3d_ModelViewMatrix) [3] .xyz: the camera in the splat space
    camera: vec4<f32>,
    // x3d_FocalLength (x, y), x3d_Viewport (width, height)
    focal_viewport: vec4<f32>,
    // x: x3d_ViewpointType, y: degrees + 1, z: X3D_COLORSPACE_LINEAR, w: 1 for the reversed depth
    mode: vec4<u32>,
    // x: number of clip planes, y: textureWidth, z: spherical harmonics width, w: spherical harmonics layers
    counts: vec4<u32>,
    clip_planes: array<vec4<f32>, 6>,
    // The splats' fog (renderContext .fogNode, else the layer's): color and type, visibilityStart / visibilityRange, matrix columns.
    fog_color_type: vec4<f32>,
    fog_range: vec4<f32>,
    fog_m0: vec4<f32>,
    fog_m1: vec4<f32>,
    fog_m2: vec4<f32>,
}

@group(1) @binding(0) var<uniform> u: SplatUniforms;
@group(1) @binding(1) var<storage, read> positions: array<vec4<f32>>;
@group(1) @binding(2) var<storage, read> orientations: array<vec4<f32>>;
@group(1) @binding(3) var<storage, read> scales: array<vec4<f32>>;
@group(1) @binding(4) var<storage, read> opacities: array<f32>;
@group(1) @binding(5) var<storage, read> spherical_harmonics: array<vec4<f32>>;
@group(1) @binding(6) var<storage, read> indices: array<u32>;

//@TONE_AND_FOG@

const SPLAT_SIGMA: f32 = 3.0;

const SH_C0: f32 = 0.28209479177387814;
const SH_C1_1: f32 = 0.4886025119029199;
const SH_C2_0: f32 = 1.0925484305920792;
const SH_C2_1: f32 = -1.0925484305920792;
const SH_C2_2: f32 = 0.31539156525252005;
const SH_C2_3: f32 = -1.0925484305920792;
const SH_C2_4: f32 = 0.5462742152960396;
const SH_C3_0: f32 = -0.5900435899266435;
const SH_C3_1: f32 = 2.890611442640554;
const SH_C3_2: f32 = -0.4570457994644658;
const SH_C3_3: f32 = 0.3731763325901154;
const SH_C3_4: f32 = -0.4570457994644658;
const SH_C3_5: f32 = 1.445305721320277;
const SH_C3_6: f32 = -0.5900435899266435;

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) color: vec4<f32>,
    @location(1) coord_xy: vec2<f32>,
    @location(2) conic: vec3<f32>,
    @location(3) vertex: vec3<f32>,
}

fn compute_cov3d(rotation: vec4<f32>, scale: vec3<f32>) -> mat3x3<f32> {
    // We have to take the inverse of the quaternion, because
    // the implementation of quat to mat3 differs from ours.
    let qx = rotation.x;
    let qy = rotation.y;
    let qz = rotation.z;
    let qw = -rotation.w;

    let yy = qy * qy;
    let zz = qz * qz;
    let xy = qx * qy;
    let zw = qz * qw;
    let xz = qx * qz;
    let yw = qy * qw;
    let xx = qx * qx;
    let yz = qy * qz;
    let xw = qx * qw;

    let r = mat3x3<f32>(
        vec3<f32>(1.0 - 2.0 * (yy + zz), 2.0 * (xy + zw), 2.0 * (xz - yw)),
        vec3<f32>(2.0 * (xy - zw), 1.0 - 2.0 * (zz + xx), 2.0 * (yz + xw)),
        vec3<f32>(2.0 * (xz + yw), 2.0 * (yz - xw), 1.0 - 2.0 * (yy + xx)),
    );

    let s = mat3x3<f32>(vec3<f32>(scale.x, 0.0, 0.0), vec3<f32>(0.0, scale.y, 0.0), vec3<f32>(0.0, 0.0, scale.z));
    let m = s * r;

    // Covariance is symmetric.
    return transpose(m) * m;
}

fn compute_cov2d(view_splat_center: vec3<f32>, cov3d: mat3x3<f32>) -> vec3<f32> {
    let fx = u.focal_viewport.x;
    let fy = u.focal_viewport.y;
    var j: mat3x3<f32>;

    if (u.mode.x == 0u) {
        // Perspective camera
        let x = view_splat_center.x;
        let y = view_splat_center.y;
        let z = view_splat_center.z;
        let zz = z * z;

        j = mat3x3<f32>(
            vec3<f32>(fx / z, 0.0, -(fx * x) / zz),
            vec3<f32>(0.0, fy / z, -(fy * y) / zz),
            vec3<f32>(0.0, 0.0, 0.0),
        );
    } else {
        // Ortho camera
        j = mat3x3<f32>(vec3<f32>(fx, 0.0, 0.0), vec3<f32>(0.0, fy, 0.0), vec3<f32>(0.0, 0.0, 0.0));
    }

    let mv = mat3x3<f32>(u.model_view[0].xyz, u.model_view[1].xyz, u.model_view[2].xyz);
    let w = transpose(mv);
    let t = w * j;

    // Covariance is symmetric, so transpose would have no effect.
    var cov = transpose(t) * cov3d * t;

    // Apply low-pass filter: every Gaussian should be at least one pixel wide/high.
    cov[0][0] = cov[0][0] + 0.3;
    cov[1][1] = cov[1][1] + 0.3;

    return vec3<f32>(cov[0][0], cov[0][1], cov[1][1]);
}

// texelFetch (x3d_SphericalHarmonicsTexture, ivec3 (texelCoord, layer), 0) .rgb; zero for a texture
// that does not exist or a coordinate outside of it.
fn sh_fetch(texel: vec2<u32>, layer: u32) -> vec3<f32> {
    let width = u.counts.z;

    if (layer >= u.counts.w || texel.x >= width || texel.y >= width) {
        return vec3<f32>(0.0);
    }

    return spherical_harmonics[(layer * width + texel.y) * width + texel.x].xyz;
}

fn compute_color_from_sh(texel: vec2<u32>, splat_center: vec3<f32>) -> vec3<f32> {
    let degrees = i32(u.mode.y) - 1;

    // Degree 0
    let sh0 = sh_fetch(texel, 0u);
    var color = sh0 * SH_C0;

    if (degrees >= 1) {
        // Degree 1
        let sh1_0 = sh_fetch(texel, 1u);
        let sh1_1 = sh_fetch(texel, 2u);
        let sh1_2 = sh_fetch(texel, 3u);

        let view_dir = normalize(splat_center - u.camera.xyz); // local-frame direction
        let x = view_dir.x;
        let y = view_dir.y;
        let z = view_dir.z;

        color += SH_C1_1 * (-y * sh1_0 + z * sh1_1 - x * sh1_2);

        if (degrees >= 2) {
            let sh2_0 = sh_fetch(texel, 4u);
            let sh2_1 = sh_fetch(texel, 5u);
            let sh2_2 = sh_fetch(texel, 6u);
            let sh2_3 = sh_fetch(texel, 7u);
            let sh2_4 = sh_fetch(texel, 8u);

            let xx = x * x;
            let yy = y * y;
            let zz = z * z;
            let xy = x * y;
            let yz = y * z;
            let xz = x * z;

            color += SH_C2_0 * xy * sh2_0 +
                     SH_C2_1 * yz * sh2_1 +
                     SH_C2_2 * (2.0 * zz - xx - yy) * sh2_2 +
                     SH_C2_3 * xz * sh2_3 +
                     SH_C2_4 * (xx - yy) * sh2_4;

            if (degrees >= 3) {
                let sh3_0 = sh_fetch(texel, 9u);
                let sh3_1 = sh_fetch(texel, 10u);
                let sh3_2 = sh_fetch(texel, 11u);
                let sh3_3 = sh_fetch(texel, 12u);
                let sh3_4 = sh_fetch(texel, 13u);
                let sh3_5 = sh_fetch(texel, 14u);
                let sh3_6 = sh_fetch(texel, 15u);

                color += SH_C3_0 * y * (3.0 * xx - yy) * sh3_0 +
                         SH_C3_1 * xy * z * sh3_1 +
                         SH_C3_2 * y * (4.0 * zz - xx - yy) * sh3_2 +
                         SH_C3_3 * z * (2.0 * zz - 3.0 * xx - 3.0 * yy) * sh3_3 +
                         SH_C3_4 * x * (4.0 * zz - xx - yy) * sh3_4 +
                         SH_C3_5 * z * (xx - yy) * sh3_5 +
                         SH_C3_6 * x * (xx - 3.0 * yy) * sh3_6;
            }
        }
    }

    color += 0.5;

    return color;
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32, @builtin(instance_index) ii: u32) -> VOut {
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0),
    );
    let corner = corners[vi];
    var out: VOut;
    out.color = vec4<f32>(0.0);
    out.coord_xy = vec2<f32>(0.0);
    out.conic = vec3<f32>(0.0);
    out.vertex = vec3<f32>(0.0);
    out.position = vec4<f32>(0.0, 0.0, 2.0, 1.0);

    // Texel Coord

    let index = indices[ii];
    let texture_width = u.counts.y;
    let texel = vec2<u32>(index % texture_width, index / texture_width);

    // Position

    let splat_center = positions[index].xyz;
    let view_splat_center = u.model_view * vec4<f32>(splat_center, 1.0); // g_pos_view
    var clip_splat_center = frame.projection * view_splat_center; // g_pos_screen

    clip_splat_center = clip_splat_center / clip_splat_center.w; // perspective division

    // Early Culling
    if (any(abs(clip_splat_center.xyz) > vec3<f32>(1.3))) {
        return out;
    }

    let splat_orientation = orientations[index];
    let splat_scale = scales[index].xyz;
    let opacity = opacities[index];

    // Orientation Quaternion must be normalized.
    let cov3d = compute_cov3d(normalize(splat_orientation), splat_scale);
    let cov2d = compute_cov2d(view_splat_center.xyz / view_splat_center.w, cov3d);

    let a = cov2d.x; // Variance x
    let b = cov2d.y; // Covariance xy
    let c = cov2d.z; // Variance y

    // Invert covariance (EWA algorithm)
    let det = a * c - b * b;

    if (det == 0.0) {
        return out;
    }

    // Calculate the inverse of the covariance matrix.
    out.conic = vec3<f32>(c, -b, a) / det;

    let quad_pixel_size = SPLAT_SIGMA * sqrt(vec2<f32>(a, c)); // screen space half quad height and width
    let quad_ndc_size = quad_pixel_size / u.focal_viewport.zw * 2.0; // in ndc space

    clip_splat_center = vec4<f32>(clip_splat_center.xy + corner * quad_ndc_size, clip_splat_center.zw);

    // Discard splats whose projected size exceeds the screen.
    let min_screen = min(u.focal_viewport.z, u.focal_viewport.w);
    let max_quad_size = max(quad_pixel_size.x, quad_pixel_size.y);

    if (max_quad_size > min_screen) {
        return out;
    }

    out.coord_xy = corner * quad_pixel_size;

    // GL clip z (-1 .. 1) to the WebGPU range; the reversed depth projection already has it.
    var p = clip_splat_center;

    if (u.mode.w == 0u) {
        p.z = 0.5 * (p.z + p.w);
    }

    out.position = p;

    let inv_clip_splat_center = u.inv_projection * clip_splat_center;

    out.vertex = inv_clip_splat_center.xyz / inv_clip_splat_center.w;
    out.color = vec4<f32>(compute_color_from_sh(texel, splat_center), opacity);

    return out;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // ClipPlanes2.glsl clip ()
    for (var i = 0u; i < min(u.counts.x, 6u); i = i + 1u) {
        let clip_plane = u.clip_planes[i];

        if (dot(in.vertex, clip_plane.xyz) < clip_plane.w) {
            discard;
        }
    }

    // Equation 4
    let exponent = -0.5 * (in.conic.x * in.coord_xy.x * in.coord_xy.x + in.conic.z * in.coord_xy.y * in.coord_xy.y) - in.conic.y * in.coord_xy.x * in.coord_xy.y;

    if (exponent > 0.0) {
        discard;
    }

    let alpha = min(0.99, exp(exponent) * in.color.a); // opacity modulated by Gaussian falloff

    if (alpha < 1.0 / 255.0) {
        discard;
    }

    var final_color = vec4<f32>(in.color.rgb, alpha);

    if (u32(u.fog_color_type.w) != 0u) {
        let visibility_start = u.fog_range.x;
        let visibility_range = u.fog_range.y - visibility_start;
        let d_v = max(length(mat3x3<f32>(u.fog_m0.xyz, u.fog_m1.xyz, u.fog_m2.xyz) * in.vertex) - visibility_start, 0.0);
        var interpolant = exp(-d_v / max(visibility_range - d_v, 0.001));

        if (u32(u.fog_color_type.w) == 1u) {
            interpolant = max(visibility_range - d_v, 0.0) / visibility_range;
        }

        final_color = vec4<f32>(mix(u.fog_color_type.xyz, final_color.rgb, interpolant), final_color.a);
    }

    final_color = vec4<f32>(tone_map(final_color.rgb, u.mode.z == 1u), final_color.a);
    return final_color;
}
