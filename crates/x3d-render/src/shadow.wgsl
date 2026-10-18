// X_ITE Depth2.vs.js / Depth2.fs.js for the shadow maps (X3DRenderObject.depth): the shape's
// x3d_ModelViewMatrix in the light's space, the light's projection, clip planes; depth only.
// Clip space: GL z in [-w, w] is mapped to WebGPU [0, w] (same window depth as glDepthRange (0, 1)).

struct ShadowDraw {
    projection: mat4x4<f32>,
    model_view: mat4x4<f32>,
    clip_planes: array<vec4<f32>, 6>,
    // x: number of clip planes, y: 1 for an instance of an InstancedShape.
    counts: vec4<u32>,
    // The instance matrix (x3d_InstanceMatrix).
    inst_matrix: mat4x4<f32>,
};

@group(0) @binding(0) var<uniform> draw: ShadowDraw;

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex: vec3<f32>,
};

@vertex
fn vs_shadow(@location(0) position: vec3<f32>) -> VOut {
    var out: VOut;
    var local = vec4<f32>(position, 1.0);

    if (draw.counts.y == 1u) {
        // Instancing2.glsl getInstanceVertex.
        local = draw.inst_matrix * local;
    }

    let t_vertex = draw.model_view * local;
    out.vertex = t_vertex.xyz;
    var p = draw.projection * t_vertex;
    p.z = 0.5 * (p.z + p.w);
    out.position = p;
    return out;
}

// RenderedTexture depthMap (TraverseType DEPTH, Depth2.fs.js without X3D_NORMAL_BUFFER):
// x3d_FragData0 = vec4 (vec3 (gl_FragCoord .z), 1.0).
@fragment
fn fs_depth_color(in: VOut) -> @location(0) vec4<f32> {
    for (var i = 0u; i < min(draw.counts.x, 6u); i = i + 1u) {
        let clip_plane = draw.clip_planes[i];

        if (dot(in.vertex, clip_plane.xyz) < clip_plane.w) {
            discard;
        }
    }

    return vec4<f32>(vec3<f32>(in.position.z), 1.0);
}

@fragment
fn fs_shadow(in: VOut) {
    for (var i = 0u; i < min(draw.counts.x, 6u); i = i + 1u) {
        let clip_plane = draw.clip_planes[i];

        if (dot(in.vertex, clip_plane.xyz) < clip_plane.w) {
            discard;
        }
    }
}
