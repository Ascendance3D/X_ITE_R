// WGSL translation of X_ITE's environment filter (oracle/x_ite-pin/src/x_ite/Browser/Lighting/Filter2.fs.js,
// from the glTF-Sample-Renderer ibl_filtering.frag): X3DLightingContext.filterEnvironmentTexture
// draws one face of one mip level of the filtered cube map per pass.

struct FilterUniforms {
    // x x3d_CurrentFaceEXT, y x3d_DistributionEXT (0 LAMBERTIAN, 1 GGX, 2 CHARLIE),
    // z x3d_SampleCountEXT, w x3d_TextureSizeEXT
    ints: vec4<i32>,
    // x x3d_RoughnessEXT, y x3d_LodBiasEXT, z x3d_IntensityEXT, w x3d_TextureLinearEXT
    floats: vec4<f32>,
    // x3d_FlipEXT, w the size of the mip level drawn
    flip: vec4<f32>,
};

@group(0) @binding(0) var<uniform> u: FilterUniforms;
@group(0) @binding(1) var source: texture_cube<f32>;
@group(0) @binding(2) var source_s: sampler;

const M_PI: f32 = 3.141592653589793;

struct VOut {
    @builtin(position) position: vec4<f32>,
};

// FullScreen2.vs: two triangles over the target.
@vertex
fn vs_main(@builtin(vertex_index) i: u32) -> VOut {
    var p = array<vec2<f32>, 6>(vec2<f32>(-1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(1.0, -1.0), vec2<f32>(1.0, 1.0));
    var out: VOut;
    out.position = vec4<f32>(p[i], 0.0, 1.0);
    return out;
}

fn generate_tbn(normal: vec3<f32>) -> mat3x3<f32> {
    var bitangent = vec3<f32>(0.0, 1.0, 0.0);
    let n_dot_up = dot(normal, vec3<f32>(0.0, 1.0, 0.0));
    let epsilon = 0.0000001;

    if (1.0 - abs(n_dot_up) <= epsilon) {
        if (n_dot_up > 0.0) {
            bitangent = vec3<f32>(0.0, 0.0, 1.0);
        } else {
            bitangent = vec3<f32>(0.0, 0.0, -1.0);
        }
    }

    let tangent = normalize(cross(bitangent, normal));
    bitangent = cross(normal, tangent);
    return mat3x3<f32>(tangent, bitangent, normal);
}

fn radical_inverse_vdc(bits_in: u32) -> f32 {
    var bits = bits_in;
    bits = (bits << 16u) | (bits >> 16u);
    bits = ((bits & 0x55555555u) << 1u) | ((bits & 0xAAAAAAAAu) >> 1u);
    bits = ((bits & 0x33333333u) << 2u) | ((bits & 0xCCCCCCCCu) >> 2u);
    bits = ((bits & 0x0F0F0F0Fu) << 4u) | ((bits & 0xF0F0F0F0u) >> 4u);
    bits = ((bits & 0x00FF00FFu) << 8u) | ((bits & 0xFF00FF00u) >> 8u);
    return f32(bits) * 2.3283064365386963e-10;
}

fn hammersley_2d(i: i32, n: i32) -> vec2<f32> {
    return vec2<f32>(f32(i) / f32(n), radical_inverse_vdc(u32(i)));
}

fn d_ggx(n_dot_h: f32, roughness: f32) -> f32 {
    let a = n_dot_h * roughness;
    let k = roughness / (1.0 - n_dot_h * n_dot_h + a * a);
    return k * k * (1.0 / M_PI);
}

fn d_charlie(sheen_roughness_in: f32, n_dot_h: f32) -> f32 {
    let sheen_roughness = max(sheen_roughness_in, 0.000001);
    let inv_r = 1.0 / sheen_roughness;
    let cos2h = n_dot_h * n_dot_h;
    let sin2h = 1.0 - cos2h;
    return (2.0 + inv_r) * pow(sin2h, inv_r * 0.5) / (2.0 * M_PI);
}

// getImportanceSample: direction (xyz) and pdf (w).
fn get_importance_sample(sample_index: i32, n: vec3<f32>, roughness: f32) -> vec4<f32> {
    let xi = hammersley_2d(sample_index, u.ints.z);
    var cos_theta: f32;
    var sin_theta: f32;
    var pdf: f32;
    let phi = 2.0 * M_PI * xi.x;

    switch (u.ints.y) {
        case 0: {
            cos_theta = sqrt(1.0 - xi.y);
            sin_theta = sqrt(xi.y);
            pdf = cos_theta / M_PI;
        }
        case 1: {
            let alpha = roughness * roughness;
            cos_theta = clamp(sqrt((1.0 - xi.y) / (1.0 + (alpha * alpha - 1.0) * xi.y)), 0.0, 1.0);
            sin_theta = sqrt(1.0 - cos_theta * cos_theta);
            pdf = d_ggx(cos_theta, alpha) / 4.0;
        }
        default: {
            let alpha = roughness * roughness;
            sin_theta = pow(xi.y, alpha / (2.0 * alpha + 1.0));
            cos_theta = sqrt(1.0 - sin_theta * sin_theta);
            pdf = d_charlie(alpha, cos_theta) / 4.0;
        }
    }

    let local_space_direction = normalize(vec3<f32>(sin_theta * cos(phi), sin_theta * sin(phi), cos_theta));
    let direction = generate_tbn(n) * local_space_direction;
    return vec4<f32>(direction, pdf);
}

fn compute_lod(pdf: f32) -> f32 {
    let size = f32(u.ints.w);
    return 0.5 * log2(6.0 * size * size / (f32(u.ints.z) * pdf));
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(2.2));
}

fn filter_color(n: vec3<f32>) -> vec3<f32> {
    var color = vec3<f32>(0.0);
    var weight = 0.0;
    let roughness = u.floats.x;
    let linear = u.floats.w == 1.0;

    for (var i = 0; i < u.ints.z; i = i + 1) {
        let importance_sample = get_importance_sample(i, n, roughness);
        let h = importance_sample.xyz;
        let pdf = importance_sample.w;
        var lod = compute_lod(pdf) + u.floats.y;

        if (u.ints.y == 0) {
            var lambertian = textureSampleLevel(source, source_s, h, lod).rgb;

            if (!linear) {
                lambertian = srgb_to_linear(lambertian);
            }

            color = color + lambertian;
        } else {
            let v = n;
            let l = normalize(reflect(-v, h));
            let n_dot_l = dot(n, l);

            if (n_dot_l > 0.0) {
                if (roughness == 0.0) {
                    lod = u.floats.y;
                }

                var sample_color = textureSampleLevel(source, source_s, l, lod).rgb;

                if (!linear) {
                    sample_color = srgb_to_linear(sample_color);
                }

                color = color + sample_color * n_dot_l;
                weight = weight + n_dot_l;
            }
        }
    }

    if (weight != 0.0) {
        color = color / weight;
    } else {
        color = color / f32(u.ints.z);
    }

    return color * u.floats.z;
}

fn get_normal(face: i32, t: vec2<f32>) -> vec3<f32> {
    var x: f32;
    var y: f32;
    var z: f32;

    switch (face) {
        case 0: { x = t.x; y = -t.y; z = 1.0; }
        case 1: { x = -t.x; y = -t.y; z = -1.0; }
        case 2: { x = -1.0; y = -t.y; z = t.x; }
        case 3: { x = 1.0; y = -t.y; z = -t.x; }
        case 4: { x = t.x; y = 1.0; z = t.y; }
        default: { x = t.x; y = -1.0; z = -t.y; }
    }

    return normalize(vec3<f32>(x, y, z)) * u.flip.xyz;
}

@fragment
fn fs_main(in: VOut) -> @location(0) vec4<f32> {
    // texCoord [-1, 1]: memory row r of the face is GL window row r (flip.w is the mip size).
    let tex_coord = in.position.xy / u.flip.w * 2.0 - 1.0;
    let normal = get_normal(u.ints.x, tex_coord);
    return vec4<f32>(filter_color(normal), 1.0);
}
