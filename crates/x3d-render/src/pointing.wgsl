// Pointing2.vs / Pointing2.fs of X_ITE (assets/shaders/webgl2), for the candidate primitives drawn into the 1x1 float pointing
// buffer. Differences forced by WebGPU: clip z is remapped to [0, w] and clip y is negated (the viewport origin is the top left),
// neither touches the interpolated values. WebGPU has no wide lines and no point sprites, so the vertex stage expands them:
//
//   mode 0  triangles, drawn as they are
//   mode 1  points (gl.POINTS): a square of gl_PointSize pixels, every corner with the point's own varyings; they are
//           interpolated like any varying (perspective-correct, not flat): the oracle GPU's GL points round them the
//           same way, so the values agree bit for bit; the fragment stage rebuilds gl_PointCoord from the window position
//   mode 2  lines (gl.LINES, aliased, gl.lineWidth w): the segment widened along the minor axis by `offset` (w / 2)
//           pixels; the corners at one end carry that end's varyings, so a fragment gets the values at its own
//           major-axis coordinate, as the oracle GPU's GL lines interpolate (a 1 px line covers the pixels whose
//           centre lies within half a pixel of the line along the minor axis)
//
// Points and lines (X3D_GEOMETRY_0D / 1D) have no texCoord attribute: texCoord0 is the constant (0, 0, 0, 1), or
// gl_PointCoord for points with X3D_STYLE_PROPERTIES.

struct Uniforms {
    projection: mat4x4<f32>,
    model_view: mat4x4<f32>,
    instance: mat4x4<f32>,
    instance_normal: mat3x3<f32>,
    // x3d_Id, mode, X3D_STYLE_PROPERTIES, X3D_TEXTURE (getPointSize returns the size unchanged)
    id: vec4<f32>,
    // gl.viewport: x, y, width, height
    viewport: vec4<f32>,
    // x3d_PointProperties: pointSizeScaleFactor, pointSizeMinValue, pointSizeMaxValue; X3D_TEXTURE || X3D_MATERIAL_TEXTURES
    point: vec4<f32>,
    // x3d_PointProperties.attenuation; the number of clip planes
    attenuation: vec4<f32>,
    clip_planes: array<vec4<f32>, 6>,
    // X3D_GEOMETRY_0D or X3D_GEOMETRY_1D
    flags: vec4<f32>,
    // x: 1 when the instance matrices come from `instances`, y: the first, z: vertices per instance (0: one instance
    // per instance_index), w: the draw's first vertex.
    instancing: vec4<u32>,
}

@group(0) @binding(0) var<uniform> u: Uniforms;
// The instance matrices of a ParticleSystem's particles (Instancing2.glsl x3d_InstanceMatrix): one draw per system,
// particle k under `instances [first + k]` with x3d_Id `id .x + k`.
@group(0) @binding(1) var<storage, read> instances: array<mat4x4<f32>>;

struct VertexIn {
    @location(0) vertex: vec4<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec4<f32>,
    @location(3) offset: vec2<f32>,
}

struct VertexOut {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec4<f32>,
    // Points: the window position of the point and pointSize.
    @location(3) @interpolate(flat) point: vec3<f32>,
    @location(4) @interpolate(flat) raster: f32,
    // gl_PointCoord: the corners' 0 and 1 interpolated linearly in screen space across the square, origin upper left.
    @location(5) @interpolate(linear) point_coord: vec2<f32>,
    // x3d_Id of the shape, its particle's with instances.
    @location(6) @interpolate(flat) id: f32,
}

// PointSize2.glsl getPointSize
fn get_point_size(vertex: vec3<f32>) -> f32 {
    let dl = length(vertex);
    var point_size = u.point.x;
    point_size /= dot(u.attenuation.xyz, vec3<f32>(1.0, dl, dl * dl));
    point_size = clamp(point_size, u.point.y, u.point.z);

    if u.id.w != 0.0 {
        return point_size;
    }

    return point_size + 1.0 - step(point_size, 1.0);
}

@vertex
fn vs_main(
    in: VertexIn,
    @builtin(vertex_index) vertex_index: u32,
    @builtin(instance_index) instance_index: u32,
) -> VertexOut {
    var out: VertexOut;
    var instance = u.instance;
    out.id = u.id.x;

    if u.instancing.x == 1u {
        var k = instance_index;

        if u.instancing.z > 0u {
            // vertex_index is first_vertex + i (WebGPU draw()), and the draw's vertices follow the earlier draws' in
            // the pass's one buffer, so the particle is counted from this draw's own first vertex.
            k = (vertex_index - u.instancing.w) / u.instancing.z;
        }

        instance = instances[u.instancing.y + k];
        out.id = u.id.x + f32(k);
    }

    var t_vertex = instance * in.vertex;
    t_vertex = u.model_view * t_vertex;
    out.vertex = t_vertex.xyz;
    out.normal = u.instance_normal * in.normal;
    out.tex_coord = in.tex_coord;
    var clip = u.projection * t_vertex;
    out.point = vec3<f32>(0.0);
    out.raster = 0.0;
    out.point_coord = vec2<f32>((in.offset.x + 1.0) * 0.5, (1.0 - in.offset.y) * 0.5);

    if u.id.y == 1.0 {
        // gl_PointSize = pointSize = max (getPointSize (vertex), 2.0) with X3D_STYLE_PROPERTIES, else 2.0
        var size = 2.0;

        if u.id.z != 0.0 {
            size = max(get_point_size(out.vertex), 2.0);
        }

        // The oracle GPU rasterizes gl_PointSize in steps of 1/32 pixel, rounded down (measured from gl_PointCoord);
        // the shader's own pointSize varying keeps the exact value.
        let raster = floor(size * 32.0) / 32.0;
        // The window position of the point snapped to the rasterizer's 1/256 pixel grid (also measured from gl_PointCoord).
        let ndc = clip.xy / clip.w;
        let window = round((u.viewport.xy + (ndc + 1.0) * 0.5 * u.viewport.zw) * 256.0) / 256.0;
        let snapped = (window - u.viewport.xy) / u.viewport.zw * 2.0 - 1.0;
        out.point = vec3<f32>(window, size);
        out.raster = raster;
        clip = vec4<f32>((snapped + in.offset * raster / u.viewport.zw) * clip.w, clip.z, clip.w);
    } else if u.id.y == 2.0 {
        clip = vec4<f32>(clip.xy + in.offset * 2.0 / u.viewport.zw * clip.w, clip.z, clip.w);
    }

    if u.flags.y != 0.0 {
        // gl.depthRange (n, f): window z = n + (f - n) * (z / w + 1) / 2, as in x3d.wgsl.
        let n = u.flags.z;
        let f = u.flags.w;
        clip.z = 2.0 * n * clip.w + (f - n) * (clip.z + clip.w) - clip.w;
    }

    out.position = vec4<f32>(clip.x, -clip.y, 0.5 * (clip.z + clip.w), clip.w);
    return out;
}

// ClipPlanes2.glsl clip (), Point2.glsl getPointColor and the gl_PointCoord texCoord0 of Pointing2.fs. The fragment centre of
// the 1x1 buffer is (0.5, 0.5) in GL window coordinates; gl_PointCoord has its origin at the upper left.
fn fragment(in: VertexOut) -> vec4<f32> {

    for (var i = 0; i < i32(u.attenuation.w); i++) {
        let plane = u.clip_planes[i];

        if dot(in.vertex, plane.xyz) < plane.w {
            discard;
        }
    }

    var tex_coord = in.tex_coord;

    if u.flags.x != 0.0 {
        tex_coord = vec4<f32>(0.0, 0.0, 0.0, 1.0);
    }

    if u.id.y == 1.0 && u.id.z != 0.0 {
        let size = in.point.z;
        let coord = in.point_coord;

        if u.point.w == 0.0 {
            var alpha = 1.0;

            if size > 1.0 {
                alpha *= clamp(size * (0.5 - distance(vec2<f32>(0.5), coord)), 0.0, 1.0);
            } else {
                alpha *= size;
            }

            if alpha < 0.5 {
                discard;
            }
        }

        tex_coord = vec4<f32>(coord.x, 1.0 - coord.y, 0.0, 1.0);
    }

    return tex_coord;
}

struct FragA {
    @location(0) data0: vec4<f32>,
    @location(1) data1: vec4<f32>,
}

@fragment
fn fs_a(in: VertexOut) -> FragA {
    var out: FragA;
    _ = fragment(in);
    out.data0 = vec4<f32>(in.vertex, in.id);
    out.data1 = vec4<f32>(in.normal, 0.0);
    return out;
}

@fragment
fn fs_b(in: VertexOut) -> @location(0) vec4<f32> {
    return fragment(in);
}
