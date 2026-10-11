// WGSL translation of the X_ITE WebGL2 shaders used for VRML97 content
// (oracle/x_ite-pin/src/assets/shaders/webgl2):
//   Default2.vs.js + common/Vertex2.glsl.js           -> vs_main
//   Material2.fs.js + common/Material2.glsl.js        -> X3D_GOURAUD_MATERIAL path (mode 1)
//   Unlit2.fs.js + common/Unlit2.glsl.js              -> X3D_UNLIT_MATERIAL path (mode 0)
//   common/Lighting2.glsl.js (getAttenuation, getSpotFactor)
//   common/Normal2.glsl.js (getNormalInfo without normal texture: n = normalize (normal), negated for back faces)
//   common/Texture2.glsl.js (getTexCoord with TEXTURE_COORDINATE_GENERATOR mode 0, x3d_TextureMatrix, FLIP_Y,
//                            getTextureColor without multi-texturing = color * texture)
//   common/Fragment2.glsl.js (alpha modes) and pbr/ToneMapping2.glsl.js (toneMap with ToneMapping NONE,
//                            ColorSpace SRGB for Phong/Gouraud/Unlit materials: color * exposure, exposure 1).
// Clip space: GL z in [-w, w] is mapped to WebGPU [0, w] after x3d_ProjectionMatrix (same window depth as
// glDepthRange (0, 1)).

struct Light {
    color_type: vec4<f32>,       // rgb, type (1 directional, 2 point, 3 spot)
    direction_intensity: vec4<f32>,
    location_ambient: vec4<f32>,
    attenuation_radius: vec4<f32>,
    spot: vec4<f32>,             // cutOffAngle, beamWidth
    m0: vec4<f32>,               // matrix (mat3 columns)
    m1: vec4<f32>,
    m2: vec4<f32>,
};

struct FrameUniforms {
    projection: mat4x4<f32>,
    num_lights: vec4<u32>,
    lights: array<Light, 8>,
};

struct DrawUniforms {
    model_view: mat4x4<f32>,
    texture_matrix: mat4x4<f32>,
    n0: vec4<f32>,               // x3d_NormalMatrix columns
    n1: vec4<f32>,
    n2: vec4<f32>,
    diffuse_ambient: vec4<f32>,  // diffuseColor, ambientIntensity
    specular_shininess: vec4<f32>,
    emissive_transparency: vec4<f32>,
    // x: mode (0 unlit, 1 gouraud), y: lighting, z: color material, w: texture
    flags: vec4<u32>,
    // x: flip y, y: alpha mode opaque, z: geometry type, w: projection z scale off (background)
    flags2: vec4<u32>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniforms;
@group(1) @binding(0) var<uniform> draw: DrawUniforms;
@group(2) @binding(0) var tex: texture_2d<f32>;
@group(2) @binding(1) var samp: sampler;

struct VIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec4<f32>,
    @location(3) color: vec4<f32>,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec4<f32>,
    @location(3) color: vec4<f32>,
};

@vertex
fn vs_main(in: VIn) -> VOut {
    var out: VOut;
    let t_vertex = draw.model_view * vec4<f32>(in.position, 1.0);
    out.vertex = t_vertex.xyz;
    out.color = in.color;
    out.tex_coord = in.tex_coord;
    let normal_matrix = mat3x3<f32>(draw.n0.xyz, draw.n1.xyz, draw.n2.xyz);
    out.normal = normal_matrix * in.normal;

    var p = frame.projection * t_vertex;
    if (draw.flags2.w == 1u) {
        // X3DBackgroundNode.display: projectionScale zeroes gl_Position.z.
        p.z = 0.0;
    }
    p.z = 0.5 * (p.z + p.w);
    out.position = p;
    return out;
}

fn get_attenuation(attenuation: vec3<f32>, distance_to_light: f32) -> f32 {
    return 1.0 / max(dot(attenuation, vec3<f32>(1.0, distance_to_light, distance_to_light * distance_to_light)), 1.0);
}

fn get_spot_factor(point_to_light: vec3<f32>, direction: vec3<f32>, cut_off_angle: f32, beam_width: f32) -> f32 {
    let spot_angle = acos(clamp(dot(-point_to_light, direction), -1.0, 1.0));

    if (spot_angle >= cut_off_angle) {
        return 0.0;
    } else if (spot_angle <= beam_width) {
        return 1.0;
    }

    return (spot_angle - cut_off_angle) / (beam_width - cut_off_angle);
}

fn get_material_color(vertex: vec3<f32>, N: vec3<f32>, ambient_color: vec3<f32>, diffuse_color: vec3<f32>, specular_color: vec3<f32>, shininess: f32) -> vec3<f32> {
    let V = normalize(-vertex);
    var final_color = vec3<f32>(0.0);
    let n = min(frame.num_lights.x, 8u);

    for (var i = 0u; i < n; i = i + 1u) {
        let light = frame.lights[i];
        let light_type = u32(light.color_type.w);
        let m = mat3x3<f32>(light.m0.xyz, light.m1.xyz, light.m2.xyz);
        let vL = light.location_ambient.xyz - vertex;
        let dL = length(m * vL);
        let radius = light.attenuation_radius.w;

        if (dL <= radius || radius < 0.0) {
            let di = light_type == 1u;
            let d = light.direction_intensity.xyz;
            let c = light.attenuation_radius.xyz;
            var L: vec3<f32>;
            if (di) { L = -d; } else { L = normalize(vL); }
            let H = normalize(L + V);

            let light_angle = max(dot(N, L), 0.0);
            let diffuse_term = diffuse_color * light_angle;
            var specular_factor = 1.0;
            if (shininess > 0.0) {
                specular_factor = pow(max(dot(N, H), 0.0), shininess * 128.0);
            }
            let specular_term = specular_color * specular_factor;

            var attenuation_factor = 1.0;
            if (!di) { attenuation_factor = get_attenuation(c, dL); }
            var spot_factor = 1.0;
            if (light_type == 3u) { spot_factor = get_spot_factor(L, d, light.spot.x, light.spot.y); }
            let attenuation_spot_factor = attenuation_factor * spot_factor;
            let ambient_term = light.location_ambient.w * ambient_color;
            let diffuse_specular_term = light.direction_intensity.w * (diffuse_term + specular_term);

            final_color = final_color + attenuation_spot_factor * light.color_type.xyz * (ambient_term + diffuse_specular_term);
        }
    }

    return final_color;
}

fn get_texture(in: VOut) -> vec4<f32> {
    var tc = draw.texture_matrix * in.tex_coord;
    let stp = tc.xyz / tc.w;
    var st = stp.xy;
    if (draw.flags2.x == 1u) {
        st.y = 1.0 - st.y;
    }
    return textureSample(tex, samp, st);
}

@fragment
fn fs_main(in: VOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    var final_color: vec4<f32>;
    let color_material = draw.flags.z == 1u;
    let textured = draw.flags.w == 1u;
    let tex_color = get_texture(in); // uniform control flow for textureSample

    if (draw.flags.x == 1u) {
        // getMaterialColor, X3D_GOURAUD_MATERIAL
        var alpha = 1.0 - draw.emissive_transparency.w;
        var diffuse_color = draw.diffuse_ambient.xyz;

        if (color_material) {
            diffuse_color = in.color.rgb;
            alpha = alpha * in.color.a;
        }

        if (draw.flags.y == 1u) {
            let ambient_color = draw.diffuse_ambient.w * diffuse_color;
            var N = normalize(in.normal);
            if (!front_facing && draw.flags2.z >= 2u) {
                N = -N;
            }
            final_color = vec4<f32>(get_material_color(in.vertex, N, ambient_color, diffuse_color, draw.specular_shininess.xyz, draw.specular_shininess.w), alpha);
        } else {
            final_color = vec4<f32>(vec3<f32>(0.0), alpha);
        }

        final_color = vec4<f32>(final_color.rgb + draw.emissive_transparency.xyz, final_color.a);

        if (textured) {
            final_color = final_color * tex_color;
        }
    } else {
        // Unlit2.glsl getEmissiveColor
        let alpha = 1.0 - draw.emissive_transparency.w;
        var emissive_color: vec4<f32>;

        if (color_material) {
            emissive_color = vec4<f32>(in.color.rgb, in.color.a * alpha);
        } else {
            emissive_color = vec4<f32>(draw.emissive_transparency.xyz, alpha);
        }

        if (textured) {
            emissive_color = emissive_color * tex_color;
        }

        final_color = emissive_color;
    }

    if (draw.flags2.y == 1u) {
        final_color.a = 1.0;
    }

    return final_color;
}
