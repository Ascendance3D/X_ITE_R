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
    // X3D_STYLE_PROPERTIES of a POINT particle (PointProperties.setShaderUniforms): pointSizeScaleFactor, pointSizeMinValue,
    // pointSizeMaxValue
    point: vec4<f32>,
    // x3d_PointProperties.attenuation
    attenuation: vec4<f32>,
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

// The POINT particles with PointProperties (X3D_GEOMETRY_0D and X3D_STYLE_PROPERTIES of Depth2.vs / Depth2.fs). WebGPU has no
// point sprites, so the vertex stage expands each point into two triangles (six vertices per instance), as
// pointing.wgsl does for the pointing pass:
//
//   gl_PointSize = pointSize = getPointSize (vertex)       vertex: the model-view space of the viewpoint (not the camera's)
//   if (getPointColor (vec4 (1.0), gl_PointCoord) .a < 0.5) discard;
//
// The depth shader has no X3D_TEXTURE and no X3D_MATERIAL_TEXTURES (X3DRenderingContext.createDepthShader), so a texture on the
// appearance neither lifts the size nor lifts the round point mask. The point is the origin of its particle (ParticleSystem POINT
// geometry, `0 0 0 1`) under x3d_InstanceMatrix (one instance per particle), with the normal (0, 0, 1) of a shader without X3D_NORMALS.
//
// The oracle GPU rasterizes gl_PointSize in steps of 1/32 pixel, rounded down, at least 1 pixel (a pointSize of 0.6 covers
// what a 1 px point covers, measured against the pinned page), and the window position of the point on the
// rasterizer's 1/256 pixel grid (measured from gl_PointCoord, pointing.wgsl). The point is not clipped by its center: a
// sprite whose center lies outside the 16 x 16 buffer still covers the pixels it reaches (measured against the pinned page).

const BUFFER: f32 = 16.0;
// ALIASED_POINT_SIZE_RANGE of the oracle GPU: gl_PointSize is clamped to [1, 2047] (the fragment shader's pointSize is not).
const MAX_POINT_SIZE: f32 = 2047.0;

struct SpriteOut {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) @interpolate(flat) id: f32,
    // pointSize (flat): the round point mask reads it, the raster size is rounded.
    @location(3) @interpolate(flat) size: f32,
    // gl_PointCoord: the corners' 0 and 1 interpolated linearly in screen space, origin upper left.
    @location(4) @interpolate(linear) point_coord: vec2<f32>,
}

// PointSize2.glsl getPointSize without X3D_TEXTURE
fn get_point_size(vertex: vec3<f32>) -> f32 {
    let dl = length(vertex);
    var point_size = draw.point.x;
    point_size /= dot(draw.attenuation.xyz, vec3<f32>(1.0, dl, dl * dl));
    point_size = clamp(point_size, draw.point.y, draw.point.z);

    return point_size + 1.0 - step(point_size, 1.0);
}

@vertex
fn vs_sprite(
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> SpriteOut {
    var out: SpriteOut;
    var corners = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, 1.0),
        vec2<f32>(-1.0, 1.0),
    );
    let offset = corners[vertex_index % 6u];
    var instance = draw.instance;
    out.id = draw.params.x;

    if (draw.instancing.x == 1u) {
        instance = instances[draw.instancing.y + instance_index];
        out.id = draw.params.x + f32(instance_index);
    }

    let t_vertex = draw.model_view * (instance * vec4<f32>(0.0, 0.0, 0.0, 1.0));
    out.vertex = t_vertex.xyz;
    out.normal = draw.instance_normal * vec3<f32>(0.0, 0.0, 1.0);
    let clip = draw.projection * t_vertex;
    let window_z = (clip.z / clip.w) * 0.5 + 0.5;

    let size = get_point_size(t_vertex.xyz);
    out.size = size;
    out.point_coord = vec2<f32>((offset.x + 1.0) * 0.5, (1.0 - offset.y) * 0.5);

    let ndc = clip.xy / clip.w;
    let centre = (ndc + 1.0) * 0.5 * BUFFER;
    let raster = clamp(floor(size * 32.0) / 32.0, 1.0, MAX_POINT_SIZE);
    let window = round(centre * 256.0) / 256.0;
    let snapped = window / BUFFER * 2.0 - 1.0;
    let xy = (snapped + offset * raster / BUFFER) * clip.w;
    out.position = vec4<f32>(xy.x, -xy.y, window_z * clip.w, clip.w);
    return out;
}

@fragment
fn fs_sprite(in: SpriteOut) -> FragOut {
    // ClipPlanes2.glsl clip ()
    for (var i = 0u; i < u32(draw.params.y); i++) {
        let plane = draw.clip_planes[i];

        if (dot(in.vertex, plane.xyz) < plane.w) {
            discard;
        }
    }

    // Point2.glsl getPointColor (vec4 (1.0), gl_PointCoord) .a < 0.5
    var alpha = 1.0;

    if (in.size > 1.0) {
        alpha *= clamp(in.size * (0.5 - distance(vec2<f32>(0.5), in.point_coord)), 0.0, 1.0);
    } else {
        alpha *= in.size;
    }

    if (alpha < 0.5) {
        discard;
    }

    // gl_FrontFacing of a point is TRUE (the sprite's triangles may wind either way).
    var out: FragOut;
    out.data0 = vec4<f32>(in.position.z, in.id, in.id, in.id);
    out.data1 = vec4<f32>(in.normal, 1.0);
    return out;
}
