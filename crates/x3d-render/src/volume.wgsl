// WGSL translation of Browser/VolumeRendering/VolumeStyle.vs.js and VolumeStyle.fs.js of the
// pinned X_ITE. crates/x3d-render/src/volume.rs composes the program: the frame uniform structs
// (copied from x3d.wgsl at run time), the texture slots, the style functions and
// `get_texture_color`, around this template.
//
// The planes of X3DVolumeDataNode.getPoints are generated from the vertex index: plane i of
// `planes` at z = size * (i / (planes - 1) - 0.5), corners (+,+) (-,+) (-,-) (+,-) of a QuadSet,
// each quad drawn as the triangles (0, 1, 2) and (0, 2, 3).

//@FRAME_STRUCTS@

@group(0) @binding(0) var<uniform> frame: FrameUniforms;

struct VolumeUniforms {
    model_view: mat4x4<f32>,
    tex_matrix: mat4x4<f32>,
    // x3d_TextureNormalMatrix columns
    nm0: vec4<f32>,
    nm1: vec4<f32>,
    nm2: vec4<f32>,
    // planes, size
    params: vec4<f32>,
    // x clip planes, y lights, z X3D_COLORSPACE_LINEAR
    counts: vec4<u32>,
    light_index: array<vec4<u32>, 2>,
    clip_planes: array<vec4<f32>, 6>,
    v: array<vec4<f32>, //@VEC4S@>,
}

@group(1) @binding(0) var<uniform> u: VolumeUniforms;
//@SLOTS@

const M_PI: f32 = 3.141592653589793;
const M_SQRT2: f32 = 1.4142135623730951;
const M_SQRT1_2: f32 = 0.7071067811865476;

fn normal_matrix() -> mat3x3<f32> {
    return mat3x3<f32>(u.nm0.xyz, u.nm1.xyz, u.nm2.xyz);
}

// getSpotFactor of VolumeStyle.fs.js.
fn spot_factor_of(cut_off_angle: f32, beam_width: f32, L: vec3<f32>, d: vec3<f32>) -> f32 {
    let spot_angle = acos(clamp(dot(-L, d), -1.0, 1.0));

    if (spot_angle >= cut_off_angle) {
        return 0.0;
    } else if (spot_angle <= beam_width) {
        return 1.0;
    }

    return (spot_angle - cut_off_angle) / (beam_width - cut_off_angle);
}

//@TONE_AND_FOG@

//@PROGRAM@

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex: vec3<f32>,
    @location(1) tex_coord: vec4<f32>,
}

@vertex
fn vs_main(@builtin(vertex_index) vi: u32) -> VOut {
    var corners = array<vec2<f32>, 4>(vec2<f32>(1.0, 1.0), vec2<f32>(-1.0, 1.0), vec2<f32>(-1.0, -1.0), vec2<f32>(1.0, -1.0));
    var order = array<u32, 6>(0u, 1u, 2u, 0u, 2u, 3u);
    let plane = vi / 6u;
    let corner = corners[order[vi % 6u]];
    let size = u.params.y;
    let z = f32(plane) / (u.params.x - 1.0) - 0.5;
    let point = vec4<f32>(size * 0.5 * corner.x, size * 0.5 * corner.y, size * z, 1.0);
    let position = u.model_view * point;
    var out: VOut;
    out.vertex = position.xyz;
    out.tex_coord = u.tex_matrix * point;
    var clip = frame.projection * position;
    clip.z = 0.5 * (clip.z + clip.w);
    out.position = clip;
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

    var final_color = get_texture_color(in.tex_coord.xyz / in.tex_coord.w, in.vertex);

    if (final_color.a < 1.0 / 255.0) {
        discard;
    }

    if (u32(frame.fog_color_type.w) != 0u) {
        final_color = vec4<f32>(mix(frame.fog_color_type.xyz, final_color.rgb, get_fog_interpolant(in.vertex)), final_color.a);
    }

    final_color = vec4<f32>(tone_map(final_color.rgb, u.counts.z == 1u), final_color.a);
    return final_color;
}
