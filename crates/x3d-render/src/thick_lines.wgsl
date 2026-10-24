// X3DLineGeometryNode.displayInstancedThick (LineTransform2.vs pass 0 with X3D_INSTANCING): the triangles of every
// segment and instance of an instanced line geometry, as gpu.rs thick_line_instance makes them on the CPU. One
// invocation per segment and instance, six vertices each. `main` writes the shadow passes' positions under the instance
// matrix (the system's space); `display` the DISPLAY pass's vertices in the instance's own space with the attributes
// of the line's vertices (gpu.rs line_style_vertices, x3d.wgsl VInStipple).

struct Params {
    // Float32Array (modelViewMatrix × projection) and its inverse (taken in doubles).
    mvp: mat4x4<f32>,
    inverse: mat4x4<f32>,
    viewport: vec4<f32>,
    half_width: f32,
    segments: u32,
    instances: u32,
    // x: 1 for a stippled line (`display`: x3d_LineStipple .z is the length so far).
    stippled: u32,
};

struct Input {
    params: Params,
    // The line's vertices (two per segment), then the instance matrices (four columns each).
    data: array<vec4<f32>>,
};

@group(0) @binding(0) var<storage, read> input: Input;
@group(0) @binding(1) var<storage, read_write> out: array<vec4<f32>>;
// `display`: 35 floats per vertex (gpu.rs STIPPLE_FLOATS_PER_VERTEX), and the line's vertices as gpu.rs
// geometry_vertices lays them out (31 floats each).
@group(0) @binding(2) var<storage, read_write> vertices: array<f32>;
@group(0) @binding(3) var<storage, read> line: array<f32>;

// gpu.rs inverse_f32: the adjugate over the determinant (the 2 × 2 sub-factor form).
fn inverse4(m: mat4x4<f32>) -> mat4x4<f32> {
    let s0 = m[0][0] * m[1][1] - m[1][0] * m[0][1];
    let s1 = m[0][0] * m[1][2] - m[1][0] * m[0][2];
    let s2 = m[0][0] * m[1][3] - m[1][0] * m[0][3];
    let s3 = m[0][1] * m[1][2] - m[1][1] * m[0][2];
    let s4 = m[0][1] * m[1][3] - m[1][1] * m[0][3];
    let s5 = m[0][2] * m[1][3] - m[1][2] * m[0][3];
    let c5 = m[2][2] * m[3][3] - m[3][2] * m[2][3];
    let c4 = m[2][1] * m[3][3] - m[3][1] * m[2][3];
    let c3 = m[2][1] * m[3][2] - m[3][1] * m[2][2];
    let c2 = m[2][0] * m[3][3] - m[3][0] * m[2][3];
    let c1 = m[2][0] * m[3][2] - m[3][0] * m[2][2];
    let c0 = m[2][0] * m[3][1] - m[3][0] * m[2][1];
    let det = s0 * c5 - s1 * c4 + s2 * c3 + s3 * c2 - s4 * c1 + s5 * c0;
    let inv = 1.0 / det;

    return mat4x4<f32>(
        vec4<f32>(
            (m[1][1] * c5 - m[1][2] * c4 + m[1][3] * c3) * inv,
            (-m[0][1] * c5 + m[0][2] * c4 - m[0][3] * c3) * inv,
            (m[3][1] * s5 - m[3][2] * s4 + m[3][3] * s3) * inv,
            (-m[2][1] * s5 + m[2][2] * s4 - m[2][3] * s3) * inv,
        ),
        vec4<f32>(
            (-m[1][0] * c5 + m[1][2] * c2 - m[1][3] * c1) * inv,
            (m[0][0] * c5 - m[0][2] * c2 + m[0][3] * c1) * inv,
            (-m[3][0] * s5 + m[3][2] * s2 - m[3][3] * s1) * inv,
            (m[2][0] * s5 - m[2][2] * s2 + m[2][3] * s1) * inv,
        ),
        vec4<f32>(
            (m[1][0] * c4 - m[1][1] * c2 + m[1][3] * c0) * inv,
            (-m[0][0] * c4 + m[0][1] * c2 - m[0][3] * c0) * inv,
            (m[3][0] * s4 - m[3][1] * s2 + m[3][3] * s0) * inv,
            (-m[2][0] * s4 + m[2][1] * s2 - m[2][3] * s0) * inv,
        ),
        vec4<f32>(
            (-m[1][0] * c3 + m[1][1] * c1 - m[1][2] * c0) * inv,
            (m[0][0] * c3 - m[0][1] * c1 + m[0][2] * c0) * inv,
            (-m[3][0] * s3 + m[3][1] * s1 - m[3][2] * s0) * inv,
            (m[2][0] * s3 - m[2][1] * s1 + m[2][2] * s0) * inv,
        ),
    );
}

fn project(p: vec4<f32>) -> vec3<f32> {
    let v = input.params.mvp * p;
    let w2 = 2.0 * v.w;
    let vp = input.params.viewport;

    return vec3<f32>((v.x / w2 + 0.5) * vp.z + vp.x, (v.y / w2 + 0.5) * vp.w + vp.y, v.z / w2 + 0.5);
}

// The corner in the particle's own space (ViewVolume.unProjectPointMatrix), then under the instance matrix.
fn corner(m: mat4x4<f32>, unproject: mat4x4<f32>, x: f32, y: f32, z: f32) -> vec4<f32> {
    let vp = input.params.viewport;
    let v = unproject * vec4<f32>((x - vp.x) / vp.z * 2.0 - 1.0, (y - vp.y) / vp.w * 2.0 - 1.0, z * 2.0 - 1.0, 1.0);

    return m * vec4<f32>(v.xyz / v.w, 1.0);
}

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let segments = input.params.segments;
    let index = id.x + id.y * groups.x * 64u;

    if (index >= segments * input.params.instances) {
        return;
    }

    let i = index / segments;
    let k = index % segments;
    let base = 2u * segments + 4u * i;
    let m = mat4x4<f32>(input.data[base], input.data[base + 1u], input.data[base + 2u], input.data[base + 3u]);
    let unproject = inverse4(m) * input.params.inverse;
    let p0 = project(m * input.data[2u * k]);
    let p1 = project(m * input.data[2u * k + 1u]);
    let neg0 = select(1.0, -1.0, p0.z > 1.0);
    let neg1 = select(1.0, -1.0, p1.z > 1.0);
    // normalize (): v * inversesqrt (dot (v, v)).
    let d = p1.xy - p0.xy;
    let r = 1.0 / sqrt(d.x * d.x + d.y * d.y);
    let o = vec2<f32>(-(d.y * r), d.x * r) * input.params.half_width;
    let o1 = o * neg1;
    let o0 = o * neg0;
    let first = index * 6u;

    out[first] = corner(m, unproject, p0.x + o1.x, p0.y + o1.y, p0.z);
    out[first + 1u] = corner(m, unproject, p0.x - o1.x, p0.y - o1.y, p0.z);
    out[first + 2u] = corner(m, unproject, p1.x - o1.x, p1.y - o1.y, p1.z);
    out[first + 3u] = corner(m, unproject, p0.x + o0.x, p0.y + o0.y, p0.z);
    out[first + 4u] = corner(m, unproject, p1.x - o0.x, p1.y - o0.y, p1.z);
    out[first + 5u] = corner(m, unproject, p1.x + o0.x, p1.y + o0.y, p1.z);
}

// The corner in the instance's own space.
fn local_corner(unproject: mat4x4<f32>, x: f32, y: f32, z: f32) -> vec4<f32> {
    let vp = input.params.viewport;
    let v = unproject * vec4<f32>((x - vp.x) / vp.z * 2.0 - 1.0, (y - vp.y) / vp.w * 2.0 - 1.0, z * 2.0 - 1.0, 1.0);

    return vec4<f32>(v.xyz / v.w, 1.0);
}

// One vertex of `display`: the position, the normal and the attributes after the texture coordinate of the line's
// vertex `end` of segment `k`, the mid point in the texture coordinate's place, x3d_LineStipple last.
fn emit(at: u32, position: vec4<f32>, k: u32, end: u32, mid: vec2<f32>, stipple: vec3<f32>) {
    let o = at * 35u;
    let b = (2u * k + end) * 31u;

    vertices[o] = position.x;
    vertices[o + 1u] = position.y;
    vertices[o + 2u] = position.z;
    vertices[o + 3u] = line[b + 3u];
    vertices[o + 4u] = line[b + 4u];
    vertices[o + 5u] = line[b + 5u];
    vertices[o + 6u] = mid.x;
    vertices[o + 7u] = mid.y;
    vertices[o + 8u] = 0.0;
    vertices[o + 9u] = 1.0;

    for (var j = 0u; j < 21u; j = j + 1u) {
        vertices[o + 10u + j] = line[b + 10u + j];
    }

    vertices[o + 31u] = stipple.x;
    vertices[o + 32u] = stipple.y;
    vertices[o + 33u] = stipple.z;
    vertices[o + 34u] = 0.0;
}

@compute @workgroup_size(64)
fn display(@builtin(global_invocation_id) id: vec3<u32>, @builtin(num_workgroups) groups: vec3<u32>) {
    let segments = input.params.segments;
    let index = id.x + id.y * groups.x * 64u;

    if (index >= segments * input.params.instances) {
        return;
    }

    let i = index / segments;
    let k = index % segments;
    let base = 2u * segments + 4u * i;
    let m = mat4x4<f32>(input.data[base], input.data[base + 1u], input.data[base + 2u], input.data[base + 3u]);
    let unproject = inverse4(m) * input.params.inverse;
    let p0 = project(m * input.data[2u * k]);
    let p1 = project(m * input.data[2u * k + 1u]);
    let neg0 = select(1.0, -1.0, p0.z > 1.0);
    let neg1 = select(1.0, -1.0, p1.z > 1.0);
    let d = p1.xy - p0.xy;
    let r = 1.0 / sqrt(d.x * d.x + d.y * d.y);
    let o = vec2<f32>(-(d.y * r), d.x * r) * input.params.half_width;
    let o1 = o * neg1;
    let o0 = o * neg0;

    // updateLengthSoFar: the window length of the segments before this one, without the instance matrix.
    var length_so_far = 0.0;

    if (input.params.stippled == 1u) {
        for (var j = 0u; j < k; j = j + 1u) {
            let s = project(input.data[2u * j + 1u]).xy - project(input.data[2u * j]).xy;
            length_so_far = length_so_far + sqrt(s.x * s.x + s.y * s.y);
        }
    }

    let stipple = vec3<f32>(p0.xy, length_so_far);
    let first = index * 6u;

    // Triangle 1 (the sign of end 1): p0 + o, p0 - o, p1 - o. Triangle 2 (that of end 0): p0 + o, p1 - o, p1 + o.
    emit(first, local_corner(unproject, p0.x + o1.x, p0.y + o1.y, p0.z), k, 0u, p1.xy, stipple);
    emit(first + 1u, local_corner(unproject, p0.x - o1.x, p0.y - o1.y, p0.z), k, 0u, p1.xy, stipple);
    emit(first + 2u, local_corner(unproject, p1.x - o1.x, p1.y - o1.y, p1.z), k, 1u, p0.xy, stipple);
    emit(first + 3u, local_corner(unproject, p0.x + o0.x, p0.y + o0.y, p0.z), k, 0u, p1.xy, stipple);
    emit(first + 4u, local_corner(unproject, p1.x - o0.x, p1.y - o0.y, p1.z), k, 1u, p0.xy, stipple);
    emit(first + 5u, local_corner(unproject, p1.x + o0.x, p1.y + o0.y, p1.z), k, 1u, p0.xy, stipple);
}
