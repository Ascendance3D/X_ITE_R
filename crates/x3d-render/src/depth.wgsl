// Depth2.vs / Depth2.fs of X_ITE (assets/shaders/webgl2) with X3D_NORMAL_BUFFER, for the 16x16
// collision depth buffer. Differences forced by WebGPU: clip z is remapped to the window z GL
// would compute (the clip volume is 0 <= z <= w) and clip y is negated (the viewport origin is the
// top left), so fragment z and the pixel rows are GL's.

struct Draw {
    projection: mat4x4<f32>,
    model_view: mat4x4<f32>,
    instance: mat4x4<f32>,
    instance_normal: mat3x3<f32>,
    clip_planes: array<vec4<f32>, 6>,
    // x: shape index, y: number of clip planes
    params: vec4<f32>,
}

@group(0) @binding(0) var<uniform> draw: Draw;

struct VertexIn {
    @location(0) vertex: vec4<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex: vec3<f32>,
    @location(1) normal: vec3<f32>,
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    let t_vertex = draw.model_view * (draw.instance * in.vertex);
    out.vertex = t_vertex.xyz;
    out.normal = draw.instance_normal * in.normal;
    let clip = draw.projection * t_vertex;
    // GL's window z of a vertex is (z / w) * 0.5 + 0.5; the viewport transform below is the
    // identity on [0, 1], so the vertex z is set to that value (w is 1 for the orthographic
    // collision camera, so multiplying by w loses nothing).
    let window_z = (clip.z / clip.w) * 0.5 + 0.5;
    out.position = vec4<f32>(clip.x, -clip.y, window_z * clip.w, clip.w);
    return out;
}

struct FragOut {
    @location(0) data0: vec4<f32>,
    @location(1) data1: vec4<f32>,
}

@fragment
fn fs_main(in: VertexOut, @builtin(front_facing) front: bool) -> FragOut {
    // ClipPlanes2.glsl clip ()
    for (var i = 0u; i < u32(draw.params.y); i++) {
        let plane = draw.clip_planes[i];

        if (dot(in.vertex, plane.xyz) < plane.w) {
            discard;
        }
    }

    var out: FragOut;
    out.data0 = vec4<f32>(in.position.z, draw.params.x, draw.params.x, draw.params.x);
    out.data1 = vec4<f32>(in.normal, select(0.0, 1.0, front));
    return out;
}
