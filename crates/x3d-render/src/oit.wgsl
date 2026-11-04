// OITCompose2.fs.js on a full-screen quad (MultiSampleFramebuffer.compose): the weighted blended order independent
// transparency composite. The resolved accumulation targets are read with texelFetch.

// sum (rgb * a * w), prod (1 - a)
@group(0) @binding(0) var accum_tex: texture_2d<f32>;
// sum (a * w)
@group(0) @binding(1) var alpha_tex: texture_2d<f32>;

// X3DRenderingContext.getFullscreenVertexArrayObject: two triangles, (-1 1) (-1 -1) (1 -1) (-1 1) (1 -1) (1 1).
@vertex
fn vs_compose(@builtin(vertex_index) i: u32) -> @builtin(position) vec4<f32> {
    var p = array<vec2<f32>, 6>(
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(-1.0, -1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(-1.0, 1.0),
        vec2<f32>(1.0, -1.0),
        vec2<f32>(1.0, 1.0),
    );
    return vec4<f32>(p[i], 0.0, 1.0);
}

@fragment
fn fs_compose(@builtin(position) position: vec4<f32>) -> @location(0) vec4<f32> {
    let frag_coord = vec2<i32>(position.xy);
    let accum = textureLoad(accum_tex, frag_coord, 0);

    if (accum.a >= 1.0) {
        discard;
    }

    let alpha = textureLoad(alpha_tex, frag_coord, 0).r;
    let revealage = 1.0 - accum.a;

    return vec4<f32>(revealage * accum.rgb / clamp(alpha, 0.001, 50000.0), revealage);
}
