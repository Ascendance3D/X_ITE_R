// Pointing2.vs / Pointing2.fs of X_ITE (assets/shaders/webgl2), for one triangle drawn into the 1x1
// float pointing buffer. Differences forced by WebGPU: clip z is remapped to [0, w] and clip y is
// negated (the viewport origin is the top left), neither touches the interpolated values.

struct Uniforms {
    projection: mat4x4<f32>,
    model_view: mat4x4<f32>,
    instance: mat4x4<f32>,
    instance_normal: mat3x3<f32>,
    id: vec4<f32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;

struct VertexIn {
    @location(0) vertex: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec4<f32>,
}

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec4<f32>,
}

@vertex
fn vs_main(in: VertexIn) -> VertexOut {
    var out: VertexOut;
    var t_vertex = u.instance * in.vertex;
    t_vertex = u.model_view * t_vertex;
    out.vertex = t_vertex.xyz;
    out.normal = u.instance_normal * in.normal;
    out.tex_coord = in.tex_coord;
    let clip = u.projection * t_vertex;
    out.position = vec4<f32>(clip.x, -clip.y, 0.5 * (clip.z + clip.w), clip.w);
    return out;
}

struct FragA {
    @location(0) data0: vec4<f32>,
    @location(1) data1: vec4<f32>,
}

@fragment
fn fs_a(in: VertexOut) -> FragA {
    var out: FragA;
    out.data0 = vec4<f32>(in.vertex, u.id.x);
    out.data1 = vec4<f32>(in.normal, 0.0);
    return out;
}

@fragment
fn fs_b(in: VertexOut) -> @location(0) vec4<f32> {
    return in.tex_coord;
}
