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
    // x: shape index (x3d_Id of its first instance), y: number of clip planes
    params: vec4<f32>,
    // x: 1 when the instance matrices come from `instances`, y: the first, z: vertices per instance (0: one instance
    // per instance_index).
    instancing: vec4<u32>,
}

@group(0) @binding(0) var<uniform> draw: Draw;
// The instance matrices of a ParticleSystem's particles (Instancing2.glsl x3d_InstanceMatrix).
@group(0) @binding(1) var<storage, read> instances: array<mat4x4<f32>>;

struct VertexIn {
    @location(0) vertex: vec4<f32>,
    @location(1) normal: vec3<f32>,
}

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex: vec3<f32>,
    @location(1) normal: vec3<f32>,
    // x3d_Id of the shape, its particle's with instances.
    @location(2) @interpolate(flat) id: f32,
}

@vertex
fn vs_main(
    in: VertexIn,
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOut {
    var out: VertexOut;
    var instance = draw.instance;
    out.id = draw.params.x;

    if (draw.instancing.x == 1u) {
        var k = instance_index;

        if (draw.instancing.z > 0u) {
            k = vertex_index / draw.instancing.z;
        }

        instance = instances[draw.instancing.y + k];
        out.id = draw.params.x + f32(k);
    }

    let t_vertex = draw.model_view * (instance * in.vertex);
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
    out.data0 = vec4<f32>(in.position.z, in.id, in.id, in.id);
    out.data1 = vec4<f32>(in.normal, select(0.0, 1.0, front));
    return out;
}
