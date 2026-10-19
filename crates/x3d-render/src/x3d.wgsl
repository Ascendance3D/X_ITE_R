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
    shadow_color: vec4<f32>,     // shadowColor, shadowIntensity (0 without a shadow map)
    shadow_params: vec4<f32>,    // shadowBias, shadowMapSize, first value, row stride (u32 bits)
    shadow_matrix: mat4x4<f32>,
};

struct Projector {
    color: vec4<f32>,            // color, intensity
    location: vec4<f32>,         // location (view space), texture linear (1)
    params: vec4<f32>,           // nearParameter, farParameter
    matrix: mat4x4<f32>,
};

struct FrameUniforms {
    projection: mat4x4<f32>,
    num_lights: vec4<u32>,
    lights: array<Light, 192>,
    // x3d_Fog: rgb color, type (0 none, 1 linear, 2 exponential); visibilityStart, visibilityRange;
    // matrix (mat3 columns).
    fog_color_type: vec4<f32>,
    fog_range: vec4<f32>,
    fog_m0: vec4<f32>,
    fog_m1: vec4<f32>,
    fog_m2: vec4<f32>,
    // viewport width, height, flip (1: the target is drawn upside down), 1 for the Shading FLAT
    viewport: vec4<f32>,
    // ToneMapping2.glsl: x ToneMapping (0 NONE, 1 ACES_NARKOWICZ, 2 ACES_HILL,
    // 3 ACES_HILL_EXPOSURE_BOOST, 4 KHR_PBR_NEUTRAL), y x3d_Exposure; IBL2.glsl: z 1 with an
    // EnvironmentLight, w specularTextureLevels.
    tone: vec4<f32>,
    // x3d_EnvironmentLightSource: color, intensity; rotation (mat3 columns).
    env_color: vec4<f32>,
    env_m0: vec4<f32>,
    env_m1: vec4<f32>,
    env_m2: vec4<f32>,
    // the width and height of the layer's viewport (gl.viewport)
    layer: vec4<f32>,
    projectors: array<Projector, 32>,
    // x: 1 in the transmission pass (X3D_TRANSMISSION_PASS, X3D_LINEAR_OUTPUT).
    // y: 1 in the volume scatter pass (X3D_VOLUME_SCATTER_PASS, X3D_LINEAR_OUTPUT).
    pass_flags: vec4<f32>,
    // The inverse of the (GL) projection matrix.
    inv_projection: mat4x4<f32>,
};

struct DrawUniforms {
    model_view: mat4x4<f32>,
    // x3d_TextureMatrix [X3D_NUM_TEXTURE_TRANSFORMS]
    texture_matrices: array<mat4x4<f32>, 4>,
    n0: vec4<f32>,               // x3d_NormalMatrix columns
    n1: vec4<f32>,
    n2: vec4<f32>,
    diffuse_ambient: vec4<f32>,  // diffuseColor, ambientIntensity
    specular_shininess: vec4<f32>,
    emissive_transparency: vec4<f32>,
    // x: mode (0 unlit, 1 lit Material), y: lighting, z: color material, w: texture
    flags: vec4<u32>,
    // x: flip y, y: alpha mode opaque, z: geometry type, w: projection z scale off (background)
    flags2: vec4<u32>,
    // x: number of clip planes, y: depth range on, z: X3D_PHONG_MATERIAL
    flags3: vec4<u32>,
    // DepthMode depthRange: near, far
    depth_range: vec4<f32>,
    // ClipPlanes2.glsl x3d_ClipPlane [X3D_NUM_CLIP_PLANES]: normal, distanceFromOrigin
    clip_planes: array<vec4<f32>, 6>,
    // x: style (1 PointProperties, 2 LineProperties, 3 FillProperties), y: filled / linetype, z: hatched
    style: vec4<u32>,
    // point: pointSizeScaleFactor, min, max; line: lineStippleScale, width; fill: hatchColor
    style_a: vec4<f32>,
    // point: attenuation
    style_b: vec4<f32>,
    // Texture2.glsl X3D_TEXTURE units (MultiTexture: at most maxTextures 4): x binding (0 none,
    // 1..8 the 2D slot + 1, 9..12 the cube slot + 1), y X3D_TEXTURE_i_FLIP_Y, z x3d_MultiTexture
    // mode, w alphaMode.
    tex_units: array<vec4<u32>, 4>,
    // x source, y function.
    tex_units2: array<vec4<u32>, 4>,
    // x3d_TextureCoordinateGenerator per coordinate channel: mode, parameter [0..2]; parameter [3..5].
    tex_gen: array<vec4<f32>, 8>,
    // x3d_MultiTextureColor
    multi_color: vec4<f32>,
    // x X3D_NUM_TEXTURES, y X3D_MULTI_TEXTURING, z X3D_NUM_TEXTURE_COORDINATES,
    // w X3D_NUM_TEXTURE_TRANSFORMS
    tex_counts: vec4<u32>,
    // MaterialTextures.js: base (diffuse), emissive, metallicRoughness (specularGlossiness),
    // occlusion, normal, then the extension textures (traverse.rs EXTENSION_TEXTURES): x binding
    // (0 none, as tex_units), y FLIP_Y, z textureTransformMapping, w textureCoordinateMapping.
    mat_tex: array<vec4<u32>, 21>,
    // x3d_Material: metallic (glossiness), roughness, normalScale, occlusionStrength.
    pbr: vec4<f32>,
    // x 0 Phong/Unlit, 1 Physical, 2 SpecularGlossiness; y X3D_COLORSPACE_LINEAR; z X3D_TANGENTS;
    // w X3D_NORMALS (IBL needs normals).
    pbr_flags: vec4<u32>,
    // x: the number of the shape's lights; their indices into frame.lights, four per vector.
    light_count: vec4<u32>,
    light_index: array<vec4<u32>, 2>,
    // x: X3D_NUM_TEXTURE_PROJECTORS; frame.projectors indices; their texture slots.
    projector_count: vec4<u32>,
    projector_index: vec4<u32>,
    projector_slot: vec4<u32>,
    // PhysicalMaterial extensions: x the X3D_*_MATERIAL_EXT bits (traverse.rs EXT_*).
    ext_flags: vec4<u32>,
    // 0 emissiveStrength, ior, specular, specularStrength; 1 specularColor, clearcoat;
    // 2 sheenColor, sheenRoughness; 3 clearcoatRoughness, iridescence, iridescenceIor,
    // iridescenceThicknessMinimum; 4 iridescenceThicknessMaximum, transmission, thickness,
    // attenuationDistance; 5 attenuationColor, diffuseTransmission; 6 diffuseTransmissionColor;
    // 7 anisotropy (cos, sin, strength); 8 dispersion, retroreflection.
    // 9 multiscatterColor, x3d_ScatterMaterialIdEXT.
    ext: array<vec4<f32>, 10>,
    // InstancedShape (X3D_INSTANCING, Instancing2.glsl): x 1 for an instance of it; x3d_InstanceMatrix and
    // the columns of x3d_InstanceNormalMatrix.
    inst_flags: vec4<u32>,
    inst_matrix: mat4x4<f32>,
    inst_n0: vec4<f32>,
    inst_n1: vec4<f32>,
    inst_n2: vec4<f32>,
    // X3DFogObject / FogContainer of the draw (the bound Fog or the innermost LocalFog): color and type,
    // visibilityStart / visibilityRange, the fog matrix columns; x of fog_flags: X3D_FOG_COORDS.
    fog_color_type: vec4<f32>,
    fog_range: vec4<f32>,
    fog_m0: vec4<f32>,
    fog_m1: vec4<f32>,
    fog_m2: vec4<f32>,
    fog_flags: vec4<u32>,
};

@group(0) @binding(0) var<uniform> frame: FrameUniforms;
// IBL2.glsl: the EnvironmentLight's filtered diffuse (layer 0), specular (1) and sheen (2) cube
// maps; the transmission buffer (TransmissionMaterialExtension); lut_ggx.png, lut_charlie.png and
// lut_sheen_E.png (layers 0, 1, 2).
@group(0) @binding(1) var env_maps: texture_cube_array<f32>;
@group(0) @binding(2) var env_s: sampler;
@group(0) @binding(3) var transmission_tex: texture_2d<f32>;
@group(0) @binding(4) var transmission_s: sampler;
@group(0) @binding(5) var luts: texture_2d_array<f32>;
@group(0) @binding(6) var lut_s: sampler;
@group(0) @binding(7) var<storage, read> shadow_maps: array<f32>;
// The volume scatter buffer of the frame (VolumeScatterMaterialExtension): RGBA32F values and window depth.
@group(0) @binding(8) var<storage, read> scatter_color: array<vec4<f32>>;
@group(0) @binding(9) var<storage, read> scatter_depth: array<f32>;
@group(1) @binding(0) var<uniform> draw: DrawUniforms;
// The texture slots of a draw: 8 two-dimensional and 4 cube textures, each with its own sampler.
@group(2) @binding(0) var t2d0: texture_2d<f32>;
@group(2) @binding(1) var s2d0: sampler;
@group(2) @binding(2) var t2d1: texture_2d<f32>;
@group(2) @binding(3) var s2d1: sampler;
@group(2) @binding(4) var t2d2: texture_2d<f32>;
@group(2) @binding(5) var s2d2: sampler;
@group(2) @binding(6) var t2d3: texture_2d<f32>;
@group(2) @binding(7) var s2d3: sampler;
@group(2) @binding(8) var t2d4: texture_2d<f32>;
@group(2) @binding(9) var s2d4: sampler;
@group(2) @binding(10) var t2d5: texture_2d<f32>;
@group(2) @binding(11) var s2d5: sampler;
@group(2) @binding(12) var t2d6: texture_2d<f32>;
@group(2) @binding(13) var s2d6: sampler;
@group(2) @binding(14) var t2d7: texture_2d<f32>;
@group(2) @binding(15) var s2d7: sampler;
@group(2) @binding(16) var tc0: texture_cube<f32>;
@group(2) @binding(17) var sc0: sampler;
@group(2) @binding(18) var tc1: texture_cube<f32>;
@group(2) @binding(19) var sc1: sampler;
@group(2) @binding(20) var tc2: texture_cube<f32>;
@group(2) @binding(21) var sc2: sampler;
@group(2) @binding(22) var tc3: texture_cube<f32>;
@group(2) @binding(23) var sc3: sampler;
@group(3) @binding(0) var style_tex: texture_2d<f32>;
@group(3) @binding(1) var style_samp: sampler;

struct VIn {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec4<f32>,
    @location(3) color: vec4<f32>,
    // texCoord1 .. texCoord3 (multiTexCoords [1..3]) and x3d_Tangent.
    @location(5) tex_coord1: vec4<f32>,
    @location(6) tex_coord2: vec4<f32>,
    @location(7) tex_coord3: vec4<f32>,
    @location(8) tangent: vec4<f32>,
    // x3d_FogDepth (FogCoordinate).
    @location(9) fog_depth: f32,
};

struct VOut {
    @builtin(position) position: vec4<f32>,
    @location(0) vertex: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec4<f32>,
    @location(3) color: vec4<f32>,
    // PointSize2.glsl: gl_PointCoord (x, y) and pointSize; Stipple2.glsl: midPoint (x, y).
    @location(6) point: vec3<f32>,
    // Stipple2.glsl: startPoint (x, y), lengthSoFar.
    @location(7) @interpolate(flat) stipple: vec3<f32>,
    @location(8) tex_coord1: vec4<f32>,
    @location(9) tex_coord2: vec4<f32>,
    @location(10) tex_coord3: vec4<f32>,
    // Vertex2.glsl localVertex, localNormal (x3d_Vertex, x3d_Normal).
    @location(11) local_vertex: vec3<f32>,
    @location(12) local_normal: vec3<f32>,
    // Vertex2.glsl TBN (X3D_NORMALS and X3D_TANGENTS): tangent, bitangent.
    @location(13) tbn_t: vec3<f32>,
    @location(14) tbn_b: vec3<f32>,
    // Fog2.glsl fogDepth (X3D_FOG_COORDS).
    @location(15) fog_depth: f32,
};

// Vertex2.glsl with the x3d_LineStipple attribute (LineProperties): start point, length so far.
struct VInStipple {
    @location(0) position: vec3<f32>,
    @location(1) normal: vec3<f32>,
    @location(2) tex_coord: vec4<f32>,
    @location(3) color: vec4<f32>,
    @location(4) stipple: vec4<f32>,
    @location(5) tex_coord1: vec4<f32>,
    @location(6) tex_coord2: vec4<f32>,
    @location(7) tex_coord3: vec4<f32>,
    @location(8) tangent: vec4<f32>,
    @location(9) fog_depth: f32,
};

@vertex
fn vs_stipple(in: VInStipple) -> VOut {
    var out = vertex_common(VIn(in.position, in.normal, in.tex_coord, in.color, in.tex_coord1, in.tex_coord2, in.tex_coord3, in.tangent, in.fog_depth));
    out.point = vec3<f32>(in.tex_coord.xy, 0.0);
    out.stipple = in.stipple.xyz;
    return out;
}

// PointSize2.glsl getPointSize
fn get_point_size(vertex: vec3<f32>) -> f32 {
    let dL = length(vertex);
    var point_size = draw.style_a.x;
    point_size = point_size / dot(draw.style_b.xyz, vec3<f32>(1.0, dL, dL * dL));
    point_size = clamp(point_size, draw.style_a.y, draw.style_a.z);

    if (draw.flags.w == 1u) {
        return point_size;
    }

    return point_size + 1.0 - step(point_size, 1.0);
}

@vertex
fn vs_main(in: VIn) -> VOut {
    return vertex_common(in);
}

fn vertex_common(in: VIn) -> VOut {
    var out: VOut;
    // Vertex2.glsl: tVertex = getInstanceVertex (x3d_Vertex), tNormal = getInstanceNormal (x3d_Normal),
    // tTangent = getInstanceNormal (x3d_Tangent .xyz); localVertex and localNormal are the instanced ones.
    var local_pos = vec4<f32>(in.position, 1.0);
    var local_normal = in.normal;
    var local_tangent = in.tangent.xyz;

    if (draw.inst_flags.x == 1u) {
        let instance_normal = mat3x3<f32>(draw.inst_n0.xyz, draw.inst_n1.xyz, draw.inst_n2.xyz);
        local_pos = draw.inst_matrix * local_pos;
        local_normal = instance_normal * local_normal;
        local_tangent = instance_normal * local_tangent;
    }

    let t_vertex = draw.model_view * local_pos;
    out.vertex = t_vertex.xyz;
    out.color = in.color;
    out.tex_coord = in.tex_coord;
    out.tex_coord1 = in.tex_coord1;
    out.tex_coord2 = in.tex_coord2;
    out.tex_coord3 = in.tex_coord3;
    out.local_vertex = local_pos.xyz;
    out.local_normal = local_normal;
    out.point = vec3<f32>(0.0, 0.0, 1.0);
    out.stipple = vec3<f32>(0.0);
    let normal_matrix = mat3x3<f32>(draw.n0.xyz, draw.n1.xyz, draw.n2.xyz);
    out.normal = normal_matrix * local_normal;
    // tangent = x3d_NormalMatrix * x3d_Tangent .xyz; bitangent = cross (normal, tangent) * x3d_Tangent .w.
    out.tbn_t = normal_matrix * local_tangent;
    out.tbn_b = cross(out.normal, out.tbn_t) * in.tangent.w;
    out.fog_depth = in.fog_depth;

    var p = frame.projection * t_vertex;

    if (draw.style.x == 1u) {
        // A point sprite corner: gl_PointSize pixels around the point, clamped to
        // ALIASED_POINT_SIZE_RANGE; gl_PointCoord has its origin at the upper left.
        // getInstancePointSize: pointSize * x3d_InstanceMatrix [0] .x.
        let point_size = get_point_size(t_vertex.xyz) * select(1.0, draw.inst_matrix[0].x, draw.inst_flags.x == 1u);
        let size = clamp(point_size, 1.0, 2047.0);
        let corner = in.tex_coord.xy;
        p = vec4<f32>(p.xy + corner * size / frame.layer.xy * p.w, p.zw);
        let up = select(corner.y, -corner.y, frame.viewport.z == 1.0);
        let pc = vec2<f32>(0.5 + 0.5 * corner.x, 0.5 - 0.5 * up);
        out.point = vec3<f32>(pc, point_size);
        // Point2.glsl setPointTexCoords
        out.tex_coord = vec4<f32>(pc.x, 1.0 - pc.y, 0.0, 1.0);
    }
    if (draw.flags2.w == 1u) {
        // X3DBackgroundNode.display: projectionScale zeroes gl_Position.z.
        p.z = 0.0;
    }
    if (draw.flags3.y == 1u) {
        // gl.depthRange (n, f): window z = n + (f - n) * (z / w + 1) / 2.
        let n = draw.depth_range.x;
        let f = draw.depth_range.y;
        p.z = 2.0 * n * p.w + (f - n) * (p.z + p.w) - p.w;
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
    let n = min(draw.light_count.x, 8u);

    for (var i = 0u; i < n; i = i + 1u) {
        let light = frame.lights[draw.light_index[i / 4u][i % 4u]];
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
            var diffuse_specular_term = light.direction_intensity.w * (diffuse_term + specular_term);

            // X3D_SHADOWS
            if (light_angle > 0.0) {
                diffuse_specular_term = mix(diffuse_specular_term, light.shadow_color.xyz, get_shadow_intensity(light, vertex));
            }

            final_color = final_color + attenuation_spot_factor * light.color_type.xyz * (ambient_term + diffuse_specular_term);
        }
    }

    return final_color;
}

// Shadow2.glsl with X3D_PCF_FILTERING. x3d_ShadowMap [i] is a DEPTH_COMPONENT24 texture with
// NEAREST filtering and CLAMP_TO_EDGE: texel floor (uv * size), rows from the bottom.
// `place`: the map's first value and row stride in shadow_maps (rows from the top).
fn get_shadow_depth(place: vec2<u32>, size: f32, uv: vec2<f32>) -> f32 {
    let s = i32(size);
    let i = clamp(i32(floor(uv.x * size)), 0, s - 1);
    let j = clamp(i32(floor(uv.y * size)), 0, s - 1);
    let d = shadow_maps[place.x + u32(s - 1 - j) * place.y + u32(i)];
    return floor(d * 16777215.0 + 0.5) / 16777215.0;
}

fn texture2d_compare(layer: vec2<u32>, size: f32, uv: vec2<f32>, compare: f32) -> f32 {
    let shadow_depth = get_shadow_depth(layer, size, uv);
    return (1.0 - step(1.0, shadow_depth)) * step(shadow_depth, compare);
}

// https://gist.github.com/tschw/da10c43c467ce8afd0c4
fn cube_to_uv_compact(v_in: vec3<f32>, texel_size_y: f32) -> vec2<f32> {
    var v = v_in;
    var abs_v = abs(v);
    let scale_to_cube = 1.0 / max(abs_v.x, max(abs_v.y, abs_v.z));
    abs_v = abs_v * scale_to_cube;
    v = v * (scale_to_cube * (1.0 - 2.0 * texel_size_y));
    var planar = v.xy;
    let almost_a_texel = 1.5 * texel_size_y;
    let almost_one = 1.0 - almost_a_texel;

    if (abs_v.z >= almost_one) {
        if (v.z > 0.0) {
            planar.x = 4.0 - v.x;
        }
    } else if (abs_v.x >= almost_one) {
        let sign_x = sign(v.x);
        planar.x = v.z * sign_x + 2.0 * sign_x;
    } else if (abs_v.y >= almost_one) {
        let sign_y = sign(v.y);
        planar.x = (v.x + 0.5 + sign_y) * 2.0;
        planar.y = v.z * sign_y - 2.0;
    }

    return vec2<f32>(0.125, 0.25) * planar + vec2<f32>(0.375, 0.75);
}

fn get_point_light_rotations(vector: vec3<f32>) -> mat4x4<f32> {
    let a = abs(vector.xyz);
    let left = mat4x4<f32>(0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    let right = mat4x4<f32>(0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    let front = mat4x4<f32>(-1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    let back = mat4x4<f32>(1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    let bottom = mat4x4<f32>(1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, -1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);
    let top = mat4x4<f32>(1.0, 0.0, 0.0, 0.0, 0.0, 0.0, -1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0);

    if (a.x > a.y) {
        if (a.x > a.z) {
            if (vector.x > 0.0) { return right; }
            return left;
        } else {
            if (vector.z > 0.0) { return front; }
            return back;
        }
    } else {
        if (a.y > a.z) {
            if (vector.y > 0.0) { return top; }
            return bottom;
        } else {
            if (vector.z > 0.0) { return front; }
            return back;
        }
    }
}

fn get_shadow_intensity(light: Light, vertex: vec3<f32>) -> f32 {
    let intensity = light.shadow_color.w;

    if (intensity <= 0.0) {
        return 0.0;
    }

    let bias = light.shadow_params.x;
    let size = light.shadow_params.y;
    let layer = vec2<u32>(bitcast<u32>(light.shadow_params.z), bitcast<u32>(light.shadow_params.w));

    if (u32(light.color_type.w) == 2u) {
        let bias_matrix = mat4x4<f32>(0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.5, 0.5, 0.5, 1.0);
        let projection_matrix = mat4x4<f32>(1.0, 0.0, 0.0, 0.0, 0.0, 0.5, 0.0, 0.0, 0.0, 0.0, -1.000025000312504, -1.0, 0.0, 0.0, -0.25000312503906297, 0.0);
        let texel_size = vec2<f32>(1.0) / (size * vec2<f32>(4.0, 2.0));
        var shadow_coord = light.shadow_matrix * vec4<f32>(vertex, 1.0);
        let light_to_position = shadow_coord.xyz;

        shadow_coord = bias_matrix * (projection_matrix * (get_point_light_rotations(light_to_position) * shadow_coord));
        shadow_coord.z = shadow_coord.z - bias;
        shadow_coord = vec4<f32>(shadow_coord.xyz / shadow_coord.w, shadow_coord.w);

        let offset = vec2<f32>(-1.0, 1.0) * (texel_size.y * 42.0);
        let z = shadow_coord.z;
        let ty = texel_size.y;
        let value = (
            texture2d_compare(layer, size, cube_to_uv_compact(light_to_position + offset.xyy, ty), z) +
            texture2d_compare(layer, size, cube_to_uv_compact(light_to_position + offset.yyy, ty), z) +
            texture2d_compare(layer, size, cube_to_uv_compact(light_to_position + offset.xyx, ty), z) +
            texture2d_compare(layer, size, cube_to_uv_compact(light_to_position + offset.yyx, ty), z) +
            texture2d_compare(layer, size, cube_to_uv_compact(light_to_position, ty), z) +
            texture2d_compare(layer, size, cube_to_uv_compact(light_to_position + offset.xxy, ty), z) +
            texture2d_compare(layer, size, cube_to_uv_compact(light_to_position + offset.yxy, ty), z) +
            texture2d_compare(layer, size, cube_to_uv_compact(light_to_position + offset.xxx, ty), z) +
            texture2d_compare(layer, size, cube_to_uv_compact(light_to_position + offset.yxx, ty), z)
        ) * (1.0 / 9.0);

        return intensity * value;
    }

    let texel_size = vec2<f32>(1.0) / vec2<f32>(size);
    var shadow_coord = light.shadow_matrix * vec4<f32>(vertex, 1.0);
    shadow_coord.z = shadow_coord.z - bias;
    shadow_coord = vec4<f32>(shadow_coord.xyz / shadow_coord.w, shadow_coord.w);

    if (any(abs(shadow_coord.xy - 0.5) > vec2<f32>(0.5))) {
        return 0.0;
    }

    let d0 = -texel_size;
    let d1 = texel_size;
    let uv = shadow_coord.xy;
    let z = shadow_coord.z;
    let value = (
        texture2d_compare(layer, size, uv + d0, z) +
        texture2d_compare(layer, size, uv + vec2<f32>(0.0, d0.y), z) +
        texture2d_compare(layer, size, uv + vec2<f32>(d1.x, d0.y), z) +
        texture2d_compare(layer, size, uv + vec2<f32>(d0.x, 0.0), z) +
        texture2d_compare(layer, size, uv, z) +
        texture2d_compare(layer, size, uv + vec2<f32>(d1.x, 0.0), z) +
        texture2d_compare(layer, size, uv + vec2<f32>(d0.x, d1.y), z) +
        texture2d_compare(layer, size, uv + vec2<f32>(0.0, d1.y), z) +
        texture2d_compare(layer, size, uv + d1, z)
    ) * (1.0 / 9.0);

    return intensity * value;
}

// Perlin2.glsl
fn rand1(co: vec2<f32>) -> f32 {
    return fract(sin(fma(co.y, 78.233, co.x * 12.9898)) * 43758.5453);
}

fn rand3(co: vec2<f32>, l: f32, t: f32) -> f32 {
    return rand1(vec2<f32>(rand1(vec2<f32>(rand1(co), l)), t));
}

fn perlin2(p: vec2<f32>, dim: f32, time: f32) -> f32 {
    let M_PI = 3.14159265358979323846;
    let pos = floor(p * dim);
    let c = rand3(pos, dim, time);
    let cx = rand3(pos + vec2<f32>(1.0, 0.0), dim, time);
    let cy = rand3(pos + vec2<f32>(0.0, 1.0), dim, time);
    let cxy = rand3(pos + vec2<f32>(1.0), dim, time);
    var d = fract(p * dim);
    d = -0.5 * cos(d * M_PI) + 0.5;
    let ccx = mix(c, cx, d.x);
    let cycxy = mix(cy, cxy, d.x);
    let center = mix(ccx, cycxy, d.y);
    return center * 2.0 - 1.0;
}

fn perlin3(p: vec3<f32>) -> vec3<f32> {
    return vec3<f32>(perlin2(p.xy, 1.0, 0.0), perlin2(p.yz, 1.0, 0.0), perlin2(p.zx, 1.0, 0.0));
}

// The values getTexCoord reads from the fragment (Texture2.glsl): the interpolated channels, the
// normals and vertices, and frontFacing.
struct TexInput {
    channels: array<vec4<f32>, 4>,
    normal: vec3<f32>,
    local_normal: vec3<f32>,
    vertex: vec3<f32>,
    local_vertex: vec3<f32>,
    front_facing: bool,
};

// Texture2.glsl getTexCoord (textureCoordinateMapping): the TextureCoordinateGenerator modes.
fn get_tex_coord_channel(t: TexInput, mapping: u32) -> vec4<f32> {
    let gen0 = draw.tex_gen[mapping * 2u];
    let gen1 = draw.tex_gen[mapping * 2u + 1u];
    let mode = u32(gen0.x);
    let points = draw.flags2.z < 2u;
    let N = select(select(-t.normal, t.normal, t.front_facing), t.normal, points);
    let local_N = select(select(-t.local_normal, t.local_normal, t.front_facing), t.local_normal, points);

    switch (mode) {
        case 1u: {
            // SPHERE
            return vec4<f32>(normalize(N).xy * 0.5 + 0.5, 0.0, 1.0);
        }
        case 2u: {
            // CAMERA_SPACE_NORMAL
            return vec4<f32>(normalize(N), 1.0);
        }
        case 3u, 7u: {
            // CAMERA_SPACE_POSITION, COORD_EYE
            return vec4<f32>(t.vertex, 1.0);
        }
        case 4u: {
            // CAMERA_SPACE_REFLECTION_VECTOR
            return vec4<f32>(reflect(normalize(t.vertex), -normalize(N)), 1.0);
        }
        case 5u: {
            // SPHERE_LOCAL
            return vec4<f32>(normalize(local_N).xy * 0.5 + 0.5, 0.0, 1.0);
        }
        case 6u: {
            // COORD
            return vec4<f32>(t.local_vertex, 1.0);
        }
        case 8u: {
            // NOISE
            return vec4<f32>(perlin3(t.local_vertex * gen0.yzw + gen1.xyz), 1.0);
        }
        case 9u: {
            // NOISE_EYE
            return vec4<f32>(perlin3(t.vertex * gen0.yzw + gen1.xyz), 1.0);
        }
        case 10u: {
            // SPHERE_REFLECT
            return vec4<f32>(refract(normalize(t.vertex), -normalize(N), gen0.y), 1.0);
        }
        case 11u: {
            // SPHERE_REFLECT_LOCAL
            return vec4<f32>(refract(normalize(t.local_vertex - vec3<f32>(gen0.z, gen0.w, gen1.x)), -normalize(local_N), gen0.y), 1.0);
        }
        default: {
            var channels = t.channels;
            return channels[mapping];
        }
    }
}

// Texture2.glsl getTexCoord (textureTransformMapping, textureCoordinateMapping).
fn get_tex_coord(t: TexInput, transform: u32, mapping: u32) -> vec3<f32> {
    var tex_coord = get_tex_coord_channel(t, mapping);

    // X3D_GEOMETRY_2D: flip X on back faces, before the texture matrix.
    if (draw.flags2.z == 2u && !t.front_facing) {
        tex_coord.x = 1.0 - tex_coord.x;
    }

    tex_coord = draw.texture_matrices[transform] * tex_coord;
    return tex_coord.xyz / tex_coord.w;
}

// A 2D texture slot (bindings 1..8) or a cube texture slot (9..12); 0 is X_ITE's default white.
fn sample_slot(binding: u32, tex_coord: vec3<f32>) -> vec4<f32> {
    let st = tex_coord.xy;

    switch (binding) {
        case 1u: { return textureSample(t2d0, s2d0, st); }
        case 2u: { return textureSample(t2d1, s2d1, st); }
        case 3u: { return textureSample(t2d2, s2d2, st); }
        case 4u: { return textureSample(t2d3, s2d3, st); }
        case 5u: { return textureSample(t2d4, s2d4, st); }
        case 6u: { return textureSample(t2d5, s2d5, st); }
        case 7u: { return textureSample(t2d6, s2d6, st); }
        case 8u: { return textureSample(t2d7, s2d7, st); }
        case 9u: { return textureSample(tc0, sc0, tex_coord); }
        case 10u: { return textureSample(tc1, sc1, tex_coord); }
        case 11u: { return textureSample(tc2, sc2, tex_coord); }
        case 12u: { return textureSample(tc3, sc3, tex_coord); }
        default: { return vec4<f32>(1.0); }
    }
}

// Texture2.glsl getTextureProjectorColor (X3D_TEXTURE_PROJECTION). Every projector's texture is
// sampled (uniform control flow); a fragment outside its stp box or facing away keeps 1.
fn get_texture_projector_color(vertex: vec3<f32>, normal: vec3<f32>, front_facing: bool, linear: bool) -> vec3<f32> {
    var current_color = vec3<f32>(1.0);
    // X3D_GEOMETRY_0D / 1D: the normal; 2D / 3D: negated for back faces.
    var N = normal;

    if (draw.flags2.z >= 2u && !front_facing) {
        N = -normal;
    }

    for (var i = 0u; i < min(draw.projector_count.x, 4u); i = i + 1u) {
        let projector = frame.projectors[draw.projector_index[i]];
        let tex_coord = projector.matrix * vec4<f32>(vertex, 1.0);
        var stp = tex_coord.xyz / tex_coord.w;
        stp.z = clamp(stp.z, projector.params.x, projector.params.y);
        let outside = any(abs(stp - 0.5) > vec3<f32>(0.5));
        // We do not need to normalize p, as we only need the sign of the dot product.
        let p = projector.location.xyz - vertex;
        let away = dot(N, p) < 0.0;
        var T = sample_slot(draw.projector_slot[i], vec3<f32>(stp.xy, 0.0));
        let texture_linear = projector.location.w == 1.0;

        if (!linear && texture_linear) {
            T = vec4<f32>(linear_to_srgb(T.rgb), T.a);
        } else if (linear && !texture_linear) {
            T = vec4<f32>(srgb_to_linear(T.rgb), T.a);
        }

        if (!outside && !away) {
            current_color = current_color * mix(vec3<f32>(1.0), T.rgb * projector.color.xyz, T.a * projector.color.w);
        }
    }

    return current_color;
}

// Texture2.glsl getTexture (i, min (i, X3D_NUM_TEXTURE_TRANSFORMS - 1), min (i, X3D_NUM_TEXTURE_COORDINATES - 1)).
fn get_texture(t: TexInput, i: u32) -> vec4<f32> {
    let unit = draw.tex_units[i];
    var tex_coord = get_tex_coord(t, min(i, max(draw.tex_counts.w, 1u) - 1u), min(i, max(draw.tex_counts.z, 1u) - 1u));

    // X3D_TEXTURE_i_FLIP_Y: after the texture matrix.
    if (unit.y == 1u) {
        tex_coord.y = 1.0 - tex_coord.y;
    }

    return sample_slot(unit.x, tex_coord);
}

// Texture2.glsl getTextureColor (diffuseColor, specularColor), with the unit colors sampled up front.
fn get_texture_color(diffuse_color: vec4<f32>, specular_color: vec4<f32>, textures: array<vec4<f32>, 4>) -> vec4<f32> {
    if (draw.tex_counts.y == 0u) {
        return diffuse_color * textures[0];
    }

    var current_color = diffuse_color;
    var colors = textures;
    let factor = draw.multi_color;

    for (var i = 0u; i < min(draw.tex_counts.x, 4u); i = i + 1u) {
        let unit = draw.tex_units[i];
        let unit2 = draw.tex_units2[i];
        var arg1 = colors[i];
        let arg2 = current_color;

        // Source
        switch (unit2.x) {
            case 1u: { arg1 = diffuse_color; }
            case 2u: { arg1 = specular_color; }
            case 3u: { arg1 = factor; }
            default: {}
        }

        // Function
        switch (unit2.y) {
            case 1u: { arg1 = 1.0 - arg1; }
            case 2u: { arg1.a = arg2.a; }
            default: {}
        }

        // RGB
        switch (unit.z) {
            case 0u: { current_color = vec4<f32>(arg1.rgb, current_color.a); }
            case 1u: { current_color = vec4<f32>(arg1.rgb * arg2.rgb, current_color.a); }
            case 2u: { current_color = vec4<f32>((arg1.rgb * arg2.rgb) * 2.0, current_color.a); }
            case 3u: { current_color = vec4<f32>((arg1.rgb * arg2.rgb) * 4.0, current_color.a); }
            case 4u: { current_color = vec4<f32>(arg1.rgb + arg2.rgb, current_color.a); }
            case 5u: { current_color = vec4<f32>(arg1.rgb + arg2.rgb - 0.5, current_color.a); }
            case 6u: { current_color = vec4<f32>((arg1.rgb + arg2.rgb - 0.5) * 2.0, current_color.a); }
            case 7u: { current_color = vec4<f32>(arg1.rgb + (1.0 - arg1.rgb) * arg2.rgb, current_color.a); }
            case 8u: { current_color = vec4<f32>(arg1.rgb - arg2.rgb, current_color.a); }
            case 9u: { current_color = vec4<f32>(arg1.rgb * diffuse_color.a + arg2.rgb * (1.0 - diffuse_color.a), current_color.a); }
            case 10u: { current_color = vec4<f32>(arg1.rgb * arg1.a + arg2.rgb * (1.0 - arg1.a), current_color.a); }
            case 11u: { current_color = vec4<f32>(arg1.rgb * factor.a + arg2.rgb * (1.0 - factor.a), current_color.a); }
            case 12u: { current_color = vec4<f32>(arg1.rgb * arg2.a + arg2.rgb * (1.0 - arg2.a), current_color.a); }
            case 13u: { current_color = vec4<f32>(arg1.rgb + arg1.a * arg2.rgb, current_color.a); }
            case 14u: { current_color = vec4<f32>((1.0 - arg1.a) * arg2.rgb + arg1.rgb, current_color.a); }
            case 15u: { current_color = vec4<f32>((1.0 - arg1.rgb) * arg2.rgb + arg1.a, current_color.a); }
            case 16u: { current_color = vec4<f32>(vec3<f32>(dot(arg1.rgb * 2.0 - 1.0, arg2.rgb * 2.0 - 1.0)), current_color.a); }
            case 17u: { current_color = vec4<f32>(arg1.rgb, current_color.a); }
            case 18u: { current_color = vec4<f32>(arg2.rgb, current_color.a); }
            default: {}
        }

        // Alpha
        switch (unit.w) {
            case 0u: { current_color.a = arg1.a; }
            case 1u: { current_color.a = arg1.a * arg2.a; }
            case 2u: { current_color.a = (arg1.a * arg2.a) * 2.0; }
            case 3u: { current_color.a = (arg1.a * arg2.a) * 4.0; }
            case 4u: { current_color.a = arg1.a + arg2.a; }
            case 5u: { current_color.a = arg1.a + arg2.a - 0.5; }
            case 6u: { current_color.a = (arg1.a + arg2.a - 0.5) * 2.0; }
            case 7u: { current_color.a = arg1.a + (1.0 - arg1.a) * arg2.a; }
            case 8u: { current_color.a = arg1.a - arg2.a; }
            case 9u: { current_color.a = arg1.a * diffuse_color.a + arg2.a * (1.0 - diffuse_color.a); }
            case 10u: { current_color.a = arg1.a * arg1.a + arg2.a * (1.0 - arg1.a); }
            case 11u: { current_color.a = arg1.a * factor.a + arg2.a * (1.0 - factor.a); }
            case 12u: { current_color.a = arg1.a * arg2.a + arg2.a * (1.0 - arg2.a); }
            case 13u: { current_color.a = arg1.a + arg1.a * arg2.a; }
            case 14u, 15u: { current_color.a = (1.0 - arg1.a) * arg2.a + arg1.a; }
            case 16u: { current_color.a = dot(arg1.rgb * 2.0 - 1.0, arg2.rgb * 2.0 - 1.0); }
            case 17u: { current_color.a = arg1.a; }
            case 18u: { current_color.a = arg2.a; }
            default: {}
        }
    }

    return current_color;
}

// --- Physical materials: pbr/BRDF2, Punctual2, IBL2, MaterialInfo2, PBR2, SpecularGlossiness2
// (glTF-Sample-Renderer via X_ITE), without material extensions.

const M_PI: f32 = 3.1415926535897932384626433832795;

// ToneMapping2.glsl linearToSRGB / sRGBToLinear (GAMMA 2.2).
fn linear_to_srgb(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(1.0 / 2.2));
}

fn srgb_to_linear(color: vec3<f32>) -> vec3<f32> {
    return pow(color, vec3<f32>(2.2));
}

fn tone_map_aces_narkowicz(color: vec3<f32>) -> vec3<f32> {
    let A = 2.51;
    let B = 0.03;
    let C = 2.43;
    let D = 0.59;
    let E = 0.14;
    return clamp((color * (A * color + B)) / (color * (C * color + D) + E), vec3<f32>(0.0), vec3<f32>(1.0));
}

fn rrt_and_odt_fit(color: vec3<f32>) -> vec3<f32> {
    let a = color * (color + 0.0245786) - 0.000090537;
    let b = color * (0.983729 * color + 0.4329510) + 0.238081;
    return a / b;
}

fn tone_map_aces_hill(color_in: vec3<f32>) -> vec3<f32> {
    let aces_input = mat3x3<f32>(vec3<f32>(0.59719, 0.07600, 0.02840), vec3<f32>(0.35458, 0.90834, 0.13383), vec3<f32>(0.04823, 0.01566, 0.83777));
    let aces_output = mat3x3<f32>(vec3<f32>(1.60475, -0.10208, -0.00327), vec3<f32>(-0.53108, 1.10813, -0.07276), vec3<f32>(-0.07367, -0.00605, 1.07602));
    var color = aces_input * color_in;
    color = rrt_and_odt_fit(color);
    color = aces_output * color;
    return clamp(color, vec3<f32>(0.0), vec3<f32>(1.0));
}

fn tone_map_khronos_pbr_neutral(color_in: vec3<f32>) -> vec3<f32> {
    let start_compression = 0.8 - 0.04;
    let desaturation = 0.15;
    var color = color_in;
    let x = min(color.r, min(color.g, color.b));
    let offset = select(0.04, x - 6.25 * x * x, x < 0.08);
    color = color - offset;
    let peak = max(color.r, max(color.g, color.b));

    if (peak < start_compression) {
        return color;
    }

    let d = 1.0 - start_compression;
    let new_peak = 1.0 - d * d / (peak + d - start_compression);
    color = color * (new_peak / peak);
    let g = 1.0 - 1.0 / (desaturation * (peak - new_peak) + 1.0);
    return mix(color, new_peak * vec3<f32>(1.0), g);
}

// ToneMapping2.glsl toneMap.
fn tone_map(color_in: vec3<f32>, linear: bool) -> vec3<f32> {
    // X3D_LINEAR_OUTPUT (the transmission pass): sRGB colors to linear, no exposure or tone map.
    if (frame.pass_flags.x == 1.0 || frame.pass_flags.y == 1.0) {
        return select(srgb_to_linear(color_in), color_in, linear);
    }

    var color = color_in * frame.tone.y;

    switch (u32(frame.tone.x)) {
        case 1u: { color = tone_map_aces_narkowicz(color); }
        case 2u: { color = tone_map_aces_hill(color); }
        case 3u: { color = tone_map_aces_hill(color / 0.6); }
        case 4u: { color = tone_map_khronos_pbr_neutral(color); }
        default: {}
    }

    if (linear) {
        color = linear_to_srgb(color);
    }

    return color;
}

fn f_schlick(f0: vec3<f32>, f90: vec3<f32>, v_dot_h: f32) -> vec3<f32> {
    return f0 + (f90 - f0) * pow(clamp(1.0 - v_dot_h, 0.0, 1.0), 5.0);
}

fn v_ggx(n_dot_l: f32, n_dot_v: f32, alpha_roughness: f32) -> f32 {
    let a2 = alpha_roughness * alpha_roughness;
    let ggxv = n_dot_l * sqrt(n_dot_v * n_dot_v * (1.0 - a2) + a2);
    let ggxl = n_dot_v * sqrt(n_dot_l * n_dot_l * (1.0 - a2) + a2);
    let ggx = ggxv + ggxl;

    if (ggx > 0.0) {
        return 0.5 / ggx;
    }

    return 0.0;
}

fn d_ggx(n_dot_h: f32, alpha_roughness: f32) -> f32 {
    let a2 = alpha_roughness * alpha_roughness;
    let f = (n_dot_h * n_dot_h) * (a2 - 1.0) + 1.0;
    return a2 / (M_PI * f * f);
}

// Punctual2.glsl
fn get_range_attenuation(radius: f32, distance_to_light: f32) -> f32 {
    if (radius <= 0.0) {
        return 1.0 / pow(distance_to_light, 2.0);
    }

    return max(min(1.0 - pow(distance_to_light / radius, 4.0), 1.0), 0.0) / pow(distance_to_light, 2.0);
}

fn get_spot_attenuation(point_to_light: vec3<f32>, spot_direction: vec3<f32>, outer_cone_cos: f32, inner_cone_cos: f32) -> f32 {
    let actual_cos = dot(normalize(spot_direction), normalize(-point_to_light));

    if (actual_cos > outer_cone_cos) {
        if (actual_cos < inner_cone_cos) {
            let angular_attenuation = (actual_cos - outer_cone_cos) / (inner_cone_cos - outer_cone_cos);
            return angular_attenuation * angular_attenuation;
        }

        return 1.0;
    }

    return 0.0;
}

fn get_light_intensity(light: Light, point_to_light: vec3<f32>, distance_to_light: f32) -> vec3<f32> {
    var intensity = light.direction_intensity.w;
    let light_type = u32(light.color_type.w);

    if (light_type != 1u) {
        intensity = intensity * get_attenuation(light.attenuation_radius.xyz, distance_to_light);
        intensity = intensity * get_range_attenuation(light.attenuation_radius.w, distance_to_light);
    }

    if (light_type == 3u) {
        intensity = intensity * get_spot_attenuation(point_to_light, light.direction_intensity.xyz, cos(light.spot.x), cos(light.spot.y));
    }

    return intensity * light.color_type.xyz;
}

// IBL2.glsl
fn get_diffuse_light(n: vec3<f32>) -> vec3<f32> {
    let rotation = mat3x3<f32>(frame.env_m0.xyz, frame.env_m1.xyz, frame.env_m2.xyz);
    let texture_color = textureSampleLevel(env_maps, env_s, rotation * n, 0, 0.0).rgb;
    return texture_color * frame.env_color.rgb * frame.env_color.w;
}

fn get_specular_light(reflection: vec3<f32>, lod: f32) -> vec3<f32> {
    let rotation = mat3x3<f32>(frame.env_m0.xyz, frame.env_m1.xyz, frame.env_m2.xyz);
    let texture_color = textureSampleLevel(env_maps, env_s, rotation * reflection, 1, lod).rgb;
    return texture_color * frame.env_color.rgb * frame.env_color.w;
}

fn get_ibl_ggx_fresnel(n: vec3<f32>, v: vec3<f32>, roughness: f32, f0: vec3<f32>, specular_weight: f32) -> vec3<f32> {
    let n_dot_v = clamp(dot(n, v), 0.0, 1.0);
    let brdf_sample_point = clamp(vec2<f32>(n_dot_v, roughness), vec2<f32>(0.0), vec2<f32>(1.0));
    let f_ab = textureSampleLevel(luts, lut_s, brdf_sample_point, 0, 0.0).rg;
    let fr = max(vec3<f32>(1.0 - roughness), f0) - f0;
    let k_s = f0 + fr * pow(1.0 - n_dot_v, 5.0);
    let fss_ess = specular_weight * (k_s * f_ab.x + f_ab.y);
    let ems = 1.0 - (f_ab.x + f_ab.y);
    let f_avg = specular_weight * (f0 + (1.0 - f0) / 21.0);
    let fms_ems = ems * fss_ess * f_avg / (1.0 - f_avg * ems);
    return fss_ess + fms_ems;
}

fn get_ibl_radiance_ggx(n: vec3<f32>, v: vec3<f32>, roughness: f32) -> vec3<f32> {
    let lod = roughness * frame.tone.w;
    let reflection = normalize(reflect(-v, n));
    return get_specular_light(reflection, lod);
}

// PBR2.glsl / SpecularGlossiness2.glsl getMaterialColor after getBaseColor and getNormalInfo.
// `f0` is f0_dielectric (0.04, or min (specular, 1)); `metallic` 0 for SpecularGlossiness.
fn get_pbr_color(vertex: vec3<f32>, n: vec3<f32>, base_color: vec4<f32>, f0: vec3<f32>, metallic: f32, perceptual_roughness_in: f32, emissive: vec3<f32>, occlusion: f32, lighting: bool, ibl: bool, no_normal_lines: bool) -> vec4<f32> {
    var color = vec3<f32>(0.0);
    let v = normalize(-vertex);
    let n_dot_v = clamp(dot(n, v), 0.0, 1.0);
    let perceptual_roughness = clamp(perceptual_roughness_in, 0.0, 1.0);
    let metal = clamp(metallic, 0.0, 1.0);
    let alpha_roughness = perceptual_roughness * perceptual_roughness;
    let f90 = vec3<f32>(1.0);

    if (ibl) {
        let f_diffuse = get_diffuse_light(n) * base_color.rgb;
        let f_specular = get_ibl_radiance_ggx(n, v, perceptual_roughness);
        let f_metal_fresnel_ibl = get_ibl_ggx_fresnel(n, v, perceptual_roughness, base_color.rgb, 1.0);
        let f_metal_brdf_ibl = f_metal_fresnel_ibl * f_specular;
        let f_dielectric_fresnel_ibl = get_ibl_ggx_fresnel(n, v, perceptual_roughness, f0, 1.0);
        let f_dielectric_brdf_ibl = mix(f_diffuse, f_specular, f_dielectric_fresnel_ibl);

        color = mix(f_dielectric_brdf_ibl, f_metal_brdf_ibl, metal);
        // X3D_OCCLUSION_TEXTURE (1 without one).
        color = color * (1.0 + draw.pbr.w * (occlusion - 1.0));
    }

    if (lighting) {
        let count = min(draw.light_count.x, 8u);

        for (var i = 0u; i < count; i = i + 1u) {
            let light = frame.lights[draw.light_index[i / 4u][i % 4u]];
            var point_to_light: vec3<f32>;
            var distance_to_light: f32;

            if (u32(light.color_type.w) != 1u) {
                let m = mat3x3<f32>(light.m0.xyz, light.m1.xyz, light.m2.xyz);
                point_to_light = light.location_ambient.xyz - vertex;
                distance_to_light = length(m * point_to_light);
            } else {
                point_to_light = -light.direction_intensity.xyz;
                distance_to_light = -1.0;
            }

            let radius = light.attenuation_radius.w;

            if (distance_to_light <= radius || radius < 0.0) {
                let l = normalize(point_to_light);
                let h = normalize(l + v);
                let n_dot_l = clamp(dot(n, l), 0.0, 1.0);
                let n_dot_h = clamp(dot(n, h), 0.0, 1.0);
                let v_dot_h = clamp(dot(v, h), 0.0, 1.0);
                let dielectric_fresnel = f_schlick(f0, f90, abs(v_dot_h));
                let metal_fresnel = f_schlick(base_color.rgb, vec3<f32>(1.0), abs(v_dot_h));
                let intensity = get_light_intensity(light, l, distance_to_light);
                // X3D_SHADOWS: PBR2.glsl mixes the light's color, SpecularGlossiness2.glsl the
                // diffuse light intensity.
                let shadow = get_shadow_intensity(light, vertex);
                let specular_glossiness = draw.pbr_flags.x == 2u;
                let diffuse_intensity = select(intensity, mix(intensity, light.shadow_color.xyz, shadow), specular_glossiness);
                let l_diffuse = diffuse_intensity * n_dot_l * (base_color.rgb / M_PI);
                let l_specular = intensity * n_dot_l * vec3<f32>(v_ggx(n_dot_l, n_dot_v, alpha_roughness) * d_ggx(n_dot_h, alpha_roughness));
                let l_metal_brdf = metal_fresnel * l_specular;
                let l_dielectric_brdf = mix(l_diffuse, l_specular, dielectric_fresnel);
                let l_color = mix(l_dielectric_brdf, l_metal_brdf, metal);

                color = color + select(mix(l_color, light.shadow_color.xyz, shadow), l_color, specular_glossiness);
            }
        }
    }

    // Points or lines without normals: base color + emissive, no lighting.
    if (no_normal_lines) {
        color = emissive + base_color.rgb;
    } else {
        color = emissive + color;
    }

    return vec4<f32>(color, base_color.a);
}

// ---- PhysicalMaterial extensions (X_ITE, experimental): PBR2.glsl getMaterialColor with the
// X3D_*_MATERIAL_EXT options of the material, MaterialInfo2, BRDF2, IBL2, Punctual2 and
// Iridescence2.glsl. Volume scatter is not drawn.

const EXT_EMISSIVE_STRENGTH: u32 = 1u;
const EXT_IOR: u32 = 2u;
const EXT_SPECULAR: u32 = 4u;
const EXT_CLEARCOAT: u32 = 8u;
const EXT_SHEEN: u32 = 16u;
const EXT_IRIDESCENCE: u32 = 32u;
const EXT_ANISOTROPY: u32 = 64u;
const EXT_TRANSMISSION: u32 = 128u;
const EXT_VOLUME: u32 = 256u;
const EXT_DIFFUSE_TRANSMISSION: u32 = 512u;
const EXT_DISPERSION: u32 = 1024u;
const EXT_RETROREFLECTION: u32 = 2048u;
const EXT_VOLUME_SCATTER: u32 = 4096u;

fn has_ext(bit: u32) -> bool {
    return (draw.ext_flags.x & bit) != 0u;
}

fn has_mat(kind: u32) -> bool {
    return draw.mat_tex[kind].x != 0u;
}

fn max3(value: vec3<f32>) -> f32 {
    return max(max(value.x, value.y), value.z);
}

fn rgb_mix(base: vec3<f32>, layer: vec3<f32>, rgb_alpha: vec3<f32>) -> vec3<f32> {
    let rgb_alpha_max = max(rgb_alpha.r, max(rgb_alpha.g, rgb_alpha.b));
    return (1.0 - rgb_alpha_max) * base + rgb_alpha * layer;
}

// F_Schlick (float f0, float f90, float VdotH) and the overloads with f90 1.0 (x^5 by products).
fn f_schlick_f(f0: f32, f90: f32, v_dot_h: f32) -> f32 {
    let x = clamp(1.0 - v_dot_h, 0.0, 1.0);
    let x2 = x * x;
    let x5 = x * x2 * x2;
    return f0 + (f90 - f0) * x5;
}

fn f_schlick_v1(f0: vec3<f32>, v_dot_h: f32) -> vec3<f32> {
    let x = clamp(1.0 - v_dot_h, 0.0, 1.0);
    let x2 = x * x;
    let x5 = x * x2 * x2;
    return f0 + (1.0 - f0) * x5;
}

// BRDF_specularGGXAnisotropy (V_GGX_anisotropic, D_GGX_anisotropic).
fn d_ggx_anisotropic(n_dot_h: f32, t_dot_h: f32, b_dot_h: f32, at: f32, ab: f32) -> f32 {
    let a2 = at * ab;
    let f = vec3<f32>(ab * t_dot_h, at * b_dot_h, a2 * n_dot_h);
    let w2 = a2 / dot(f, f);
    return a2 * w2 * w2 / M_PI;
}

fn v_ggx_anisotropic(n_dot_l: f32, n_dot_v: f32, b_dot_v: f32, t_dot_v: f32, t_dot_l: f32, b_dot_l: f32, at: f32, ab: f32) -> f32 {
    let ggxv = n_dot_l * length(vec3<f32>(at * t_dot_v, ab * b_dot_v, n_dot_v));
    let ggxl = n_dot_v * length(vec3<f32>(at * t_dot_l, ab * b_dot_l, n_dot_l));
    let v = 0.5 / (ggxv + ggxl);
    return clamp(v, 0.0, 1.0);
}

fn brdf_specular_ggx_anisotropy(alpha_roughness: f32, anisotropy: f32, n: vec3<f32>, v: vec3<f32>, l: vec3<f32>, h: vec3<f32>, t: vec3<f32>, b: vec3<f32>) -> vec3<f32> {
    let at = mix(alpha_roughness, 1.0, anisotropy * anisotropy);
    let ab = clamp(alpha_roughness, 0.001, 1.0);
    let n_dot_l = clamp(dot(n, l), 0.0, 1.0);
    let n_dot_h = clamp(dot(n, h), 0.001, 1.0);
    let n_dot_v = dot(n, v);
    let vis = v_ggx_anisotropic(n_dot_l, n_dot_v, dot(b, v), dot(t, v), dot(t, l), dot(b, l), at, ab);
    let d = d_ggx_anisotropic(n_dot_h, dot(t, h), dot(b, h), at, ab);
    return vec3<f32>(vis * d);
}

// Sheen (Charlie).
fn lambda_sheen_numeric_helper(x: f32, alpha_g: f32) -> f32 {
    let one_minus_alpha_sq = (1.0 - alpha_g) * (1.0 - alpha_g);
    let a = mix(21.5473, 25.3245, one_minus_alpha_sq);
    let b = mix(3.82987, 3.32435, one_minus_alpha_sq);
    let c = mix(0.19823, 0.16801, one_minus_alpha_sq);
    let d = mix(-1.97760, -1.27393, one_minus_alpha_sq);
    let e = mix(-4.32054, -4.85967, one_minus_alpha_sq);
    return a / (1.0 + b * pow(x, c)) + d * x + e;
}

fn lambda_sheen(cos_theta: f32, alpha_g: f32) -> f32 {
    if (abs(cos_theta) < 0.5) {
        return exp(lambda_sheen_numeric_helper(cos_theta, alpha_g));
    }

    return exp(2.0 * lambda_sheen_numeric_helper(0.5, alpha_g) - lambda_sheen_numeric_helper(1.0 - cos_theta, alpha_g));
}

fn v_sheen(n_dot_l: f32, n_dot_v: f32, sheen_roughness_in: f32) -> f32 {
    let sheen_roughness = max(sheen_roughness_in, 0.000001);
    let alpha_g = sheen_roughness * sheen_roughness;
    return clamp(1.0 / ((1.0 + lambda_sheen(n_dot_v, alpha_g) + lambda_sheen(n_dot_l, alpha_g)) * (4.0 * n_dot_v * n_dot_l)), 0.0, 1.0);
}

fn d_charlie(sheen_roughness_in: f32, n_dot_h: f32) -> f32 {
    let sheen_roughness = max(sheen_roughness_in, 0.000001);
    let alpha_g = sheen_roughness * sheen_roughness;
    let inv_r = 1.0 / alpha_g;
    let cos2h = n_dot_h * n_dot_h;
    let sin2h = 1.0 - cos2h;
    return (2.0 + inv_r) * pow(sin2h, inv_r * 0.5) / (2.0 * M_PI);
}

fn albedo_sheen_scaling_lut(n_dot_v: f32, sheen_roughness: f32) -> f32 {
    return textureSampleLevel(luts, lut_s, vec2<f32>(n_dot_v, sheen_roughness), 2, 0.0).r;
}

fn get_sheen_light(reflection: vec3<f32>, lod: f32) -> vec3<f32> {
    let rotation = mat3x3<f32>(frame.env_m0.xyz, frame.env_m1.xyz, frame.env_m2.xyz);
    let texture_color = textureSampleLevel(env_maps, env_s, rotation * reflection, 2, lod).rgb;
    return texture_color * frame.env_color.rgb * frame.env_color.w;
}

fn get_ibl_radiance_charlie(n: vec3<f32>, v: vec3<f32>, sheen_roughness: f32, sheen_color: vec3<f32>) -> vec3<f32> {
    let n_dot_v = clamp(dot(n, v), 0.0, 1.0);
    // sheenTextureLevels: the levels of the generated sheen texture (the specular texture size).
    let lod = sheen_roughness * frame.tone.w;
    let reflection = normalize(reflect(-v, n));
    let brdf_sample_point = clamp(vec2<f32>(n_dot_v, sheen_roughness), vec2<f32>(0.0), vec2<f32>(1.0));
    let brdf = textureSampleLevel(luts, lut_s, brdf_sample_point, 1, 0.0).b;
    let sheen_light = get_sheen_light(reflection, lod);
    return sheen_light * sheen_color * brdf;
}

fn get_ibl_radiance_anisotropy(n: vec3<f32>, v: vec3<f32>, roughness: f32, anisotropy: f32, anisotropy_direction: vec3<f32>) -> vec3<f32> {
    let anisotropic_tangent = cross(anisotropy_direction, v);
    let anisotropic_normal = cross(anisotropic_tangent, anisotropy_direction);
    let bend_factor = 1.0 - anisotropy * (1.0 - roughness);
    let bend_factor_pow4 = bend_factor * bend_factor * bend_factor * bend_factor;
    let bent_normal = normalize(mix(anisotropic_normal, n, bend_factor_pow4));
    let lod = roughness * frame.tone.w;
    let reflection = normalize(reflect(-v, bent_normal));
    return get_specular_light(reflection, lod);
}

// Punctual2.glsl transmission and volume.
fn apply_ior_to_roughness(roughness: f32, ior: f32) -> f32 {
    return roughness * clamp(ior * 2.0 - 2.0, 0.0, 1.0);
}

fn get_punctual_radiance_transmission(normal: vec3<f32>, view: vec3<f32>, point_to_light: vec3<f32>, alpha_roughness: f32, base_color: vec3<f32>, ior: f32) -> vec3<f32> {
    let transmission_roughness = apply_ior_to_roughness(alpha_roughness, ior);
    let n = normalize(normal);
    let v = normalize(view);
    let l = normalize(point_to_light);
    let l_mirror = normalize(l + 2.0 * n * dot(-l, n));
    let h = normalize(l_mirror + v);
    let d = d_ggx(clamp(dot(n, h), 0.0, 1.0), transmission_roughness);
    let vis = v_ggx(clamp(dot(n, l_mirror), 0.0, 1.0), clamp(dot(n, v), 0.0, 1.0), transmission_roughness);
    return base_color * d * vis;
}

fn apply_volume_attenuation(radiance: vec3<f32>, transmission_distance: f32, attenuation_color: vec3<f32>, attenuation_distance: f32) -> vec3<f32> {
    if (attenuation_distance == 0.0) {
        return radiance;
    }

    let attenuation_coefficient = log(attenuation_color) / attenuation_distance;
    let transmittance = exp(attenuation_coefficient * transmission_distance);
    return transmittance * radiance;
}

fn get_volume_transmission_ray(n: vec3<f32>, v: vec3<f32>, thickness: f32, ior: f32) -> vec3<f32> {
    let refraction_vector = refract(-v, normalize(n), 1.0 / ior);
    let m = draw.model_view;
    let model_scale = vec3<f32>(length(m[0].xyz), length(m[1].xyz), length(m[2].xyz));
    return normalize(refraction_vector) * thickness * model_scale;
}

// IBL2.glsl getTransmissionSample: the transmission buffer rows are in the target's order, the
// coordinates in GL's (lower left origin).
fn get_transmission_sample(frag_coord: vec2<f32>, roughness: f32, ior: f32) -> vec3<f32> {
    let framebuffer_size = max(frame.layer.x, frame.layer.y);
    let framebuffer_lod = log2(framebuffer_size) * apply_ior_to_roughness(roughness, ior);
    return textureSampleLevel(transmission_tex, transmission_s, vec2<f32>(frag_coord.x, 1.0 - frag_coord.y), framebuffer_lod).rgb;
}

fn get_ibl_volume_refraction(n: vec3<f32>, v: vec3<f32>, perceptual_roughness: f32, base_color: vec3<f32>, position: vec3<f32>, ior: f32, thickness: f32, attenuation_color: vec3<f32>, attenuation_distance: f32, dispersion: f32) -> vec3<f32> {
    var transmitted_light: vec3<f32>;
    var transmission_ray_length: f32;

    if (has_ext(EXT_DISPERSION)) {
        // Dispersion spreads out the ior values for each r, g, b channel (X3D_DISPERSION_MATERIAL_EXT).
        let half_spread = (ior - 1.0) * 0.025 * dispersion;
        let iors = vec3<f32>(ior - half_spread, ior, ior + half_spread);

        for (var i = 0; i < 3; i = i + 1) {
            let transmission_ray = get_volume_transmission_ray(n, v, thickness, iors[i]);
            // The length of the last (blue) ray wins, as in IBL2.glsl.
            transmission_ray_length = length(transmission_ray);
            let refracted_ray_exit = position + transmission_ray;
            let ndc_pos = frame.projection * vec4<f32>(refracted_ray_exit, 1.0);
            let refraction_coords = (ndc_pos.xy / ndc_pos.w + 1.0) / 2.0;
            transmitted_light[i] = get_transmission_sample(refraction_coords, perceptual_roughness, iors[i])[i];
        }
    } else {
        let transmission_ray = get_volume_transmission_ray(n, v, thickness, ior);
        transmission_ray_length = length(transmission_ray);
        let refracted_ray_exit = position + transmission_ray;
        let ndc_pos = frame.projection * vec4<f32>(refracted_ray_exit, 1.0);
        let refraction_coords = (ndc_pos.xy / ndc_pos.w + 1.0) / 2.0;
        transmitted_light = get_transmission_sample(refraction_coords, perceptual_roughness, ior);
    }

    let attenuated_color = apply_volume_attenuation(transmitted_light, transmission_ray_length, attenuation_color, attenuation_distance);
    return attenuated_color * base_color;
}

// Iridescence2.glsl
fn sq3(v: vec3<f32>) -> vec3<f32> {
    return v * v;
}

fn fresnel0_to_ior(fresnel0: vec3<f32>) -> vec3<f32> {
    let sqrt_f0 = sqrt(fresnel0);
    return (vec3<f32>(1.0) + sqrt_f0) / (vec3<f32>(1.0) - sqrt_f0);
}

fn eval_sensitivity(opd: f32, shift: vec3<f32>) -> vec3<f32> {
    let xyz_to_rec709 = mat3x3<f32>(
        vec3<f32>(3.2404542, -0.9692660, 0.0556434),
        vec3<f32>(-1.5371385, 1.8760108, -0.2040259),
        vec3<f32>(-0.4985314, 0.0415560, 1.0572252),
    );
    let val = vec3<f32>(5.4856e-13, 4.4201e-13, 5.2481e-13);
    let pos = vec3<f32>(1.6810e+06, 1.7953e+06, 2.2084e+06);
    let var_ = vec3<f32>(4.3278e+09, 9.3046e+09, 6.6121e+09);
    let phase = 2.0 * M_PI * opd * 1.0e-9;
    var xyz = val * sqrt(2.0 * M_PI * var_) * cos(pos * phase + shift) * exp(-(phase * phase) * var_);
    xyz.x = xyz.x + 9.7470e-14 * sqrt(2.0 * M_PI * 4.5282e+09) * cos(2.2399e+06 * phase + shift[0]) * exp(-4.5282e+09 * (phase * phase));
    xyz = xyz / 1.0685e-7;
    return xyz_to_rec709 * xyz;
}

fn eval_iridescence(outside_ior: f32, eta2: f32, cos_theta1: f32, thin_film_thickness: f32, base_f0: vec3<f32>) -> vec3<f32> {
    let iridescence_ior = mix(outside_ior, eta2, smoothstep(0.0, 0.03, thin_film_thickness));
    let r = outside_ior / iridescence_ior;
    let sin_theta2_sq = r * r * (1.0 - cos_theta1 * cos_theta1);
    let cos_theta2_sq = 1.0 - sin_theta2_sq;

    if (cos_theta2_sq < 0.0) {
        return vec3<f32>(1.0);
    }

    let cos_theta2 = sqrt(cos_theta2_sq);
    let r0_ = (iridescence_ior - outside_ior) / (iridescence_ior + outside_ior);
    let r0 = r0_ * r0_;
    let r12 = f_schlick_f(r0, 1.0, cos_theta1);
    let t121 = 1.0 - r12;
    var phi12 = 0.0;

    if (iridescence_ior < outside_ior) {
        phi12 = M_PI;
    }

    let phi21 = M_PI - phi12;
    let base_ior = fresnel0_to_ior(clamp(base_f0, vec3<f32>(0.0), vec3<f32>(0.9999)));
    let r1 = sq3((base_ior - vec3<f32>(iridescence_ior)) / (base_ior + vec3<f32>(iridescence_ior)));
    let r23 = f_schlick_v1(r1, cos_theta2);
    var phi23 = vec3<f32>(0.0);

    if (base_ior[0] < iridescence_ior) {
        phi23[0] = M_PI;
    }
    if (base_ior[1] < iridescence_ior) {
        phi23[1] = M_PI;
    }
    if (base_ior[2] < iridescence_ior) {
        phi23[2] = M_PI;
    }

    let opd = 2.0 * iridescence_ior * thin_film_thickness * cos_theta2;
    let phi = vec3<f32>(phi21) + phi23;
    let r123 = clamp(r12 * r23, vec3<f32>(1e-5), vec3<f32>(0.9999));
    let r123_sqrt = sqrt(r123);
    let rs = t121 * t121 * r23 / (vec3<f32>(1.0) - r123);
    var i = r12 + rs;
    var cm = rs - t121;

    for (var m = 1; m <= 2; m = m + 1) {
        cm = cm * r123_sqrt;
        let sm = 2.0 * eval_sensitivity(f32(m) * opd, f32(m) * phi);
        i = i + cm * sm;
    }

    return max(i, vec3<f32>(0.0));
}

// PBR2.glsl getMaterialColor with the MaterialInfo2.glsl get*Info of the extensions (uninitialized
// MaterialInfo members are 0, as ANGLE initializes locals).
fn get_pbr_ext_color(vertex: vec3<f32>, n: vec3<f32>, t: vec3<f32>, b: vec3<f32>, ng: vec3<f32>, base_color: vec4<f32>, mats: array<vec4<f32>, 21>, lighting: bool, ibl: bool, no_normal_lines: bool, frag_coord: vec2<f32>) -> vec4<f32> {
    var color = vec3<f32>(0.0);
    let v = normalize(-vertex);
    let n_dot_v = clamp(dot(n, v), 0.0, 1.0);
    // PBR2.glsl v_retro, defined with X3D_USE_IBL or X3D_LIGHTING.
    let v_retro = normalize(reflect(-v, n));

    var ior = 1.5;
    var f0_dielectric = vec3<f32>(0.04);
    var specular_weight = 1.0;
    var f90_dielectric = vec3<f32>(1.0);

    // getIorInfo
    if (has_ext(EXT_IOR)) {
        let x = draw.ext[0].y;
        f0_dielectric = vec3<f32>(pow((x - 1.0) / (x + 1.0), 2.0));
        ior = x;
    }

    // getMetallicRoughnessInfo
    var metallic = draw.pbr.x * mats[2].b;
    var perceptual_roughness = draw.pbr.y * mats[2].g;

    // getSheenInfo
    var sheen_color = vec3<f32>(0.0);
    var sheen_roughness = 0.0;

    if (has_ext(EXT_SHEEN)) {
        sheen_color = draw.ext[2].xyz * mats[10].rgb;
        sheen_roughness = draw.ext[2].w * mats[11].a;
    }

    // getClearCoatInfo
    var clearcoat = 0.0;
    var clearcoat_roughness = 0.0;
    var clearcoat_f0 = vec3<f32>(0.0);
    var clearcoat_f90 = vec3<f32>(0.0);
    var clearcoat_normal = vec3<f32>(0.0);

    if (has_ext(EXT_CLEARCOAT)) {
        clearcoat = draw.ext[1].w * mats[7].r;
        clearcoat_roughness = draw.ext[3].x * mats[8].g;
        clearcoat_f0 = vec3<f32>(pow((ior - 1.0) / (ior + 1.0), 2.0));
        clearcoat_f90 = vec3<f32>(1.0);
        clearcoat_normal = ng;

        if (has_mat(9u)) {
            let cn = mats[9].rgb * 2.0 - vec3<f32>(1.0);
            clearcoat_normal = mat3x3<f32>(t, b, ng) * normalize(cn);
        }

        clearcoat_roughness = clamp(clearcoat_roughness, 0.0, 1.0);
    }

    // getSpecularInfo
    if (has_ext(EXT_SPECULAR)) {
        let specular_color = draw.ext[0].w * draw.ext[1].xyz;
        var specular_texture = vec4<f32>(1.0);

        if (has_mat(5u)) {
            specular_texture.a = mats[5].a;
        }

        if (has_mat(6u)) {
            specular_texture = vec4<f32>(mats[6].rgb, specular_texture.a);
        }

        f0_dielectric = min(f0_dielectric * specular_color * specular_texture.rgb, vec3<f32>(1.0));
        specular_weight = draw.ext[0].z * specular_texture.a;
        f90_dielectric = vec3<f32>(specular_weight);
    }

    // getTransmissionInfo
    var transmission = 0.0;

    if (has_ext(EXT_TRANSMISSION)) {
        transmission = draw.ext[4].y * mats[15].r;
    }

    // getTransmissionInfo: x3d_DispersionEXT (0 without a DispersionMaterialExtension).
    var dispersion = 0.0;

    if (has_ext(EXT_DISPERSION)) {
        dispersion = draw.ext[8].x;
    }

    // getRetroreflectionInfo
    var retroreflection = 0.0;

    if (has_ext(EXT_RETROREFLECTION)) {
        retroreflection = draw.ext[8].y * mats[19].r;
    }

    // getVolumeInfo
    var thickness = 0.0;
    var attenuation_distance = 0.0;
    var attenuation_color = vec3<f32>(0.0);

    if (has_ext(EXT_VOLUME)) {
        thickness = draw.ext[4].z * mats[16].g;
        attenuation_distance = draw.ext[4].w;
        attenuation_color = draw.ext[5].xyz;
    }

    // getIridescenceInfo
    var iridescence = 0.0;
    var iridescence_ior = 0.0;
    var iridescence_thickness = 0.0;

    if (has_ext(EXT_IRIDESCENCE)) {
        iridescence = draw.ext[3].y * mats[12].r;
        iridescence_ior = draw.ext[3].z;
        iridescence_thickness = draw.ext[4].x;

        if (has_mat(13u)) {
            iridescence_thickness = mix(draw.ext[3].w, draw.ext[4].x, mats[13].g);
        }
    }

    // getDiffuseTransmissionInfo
    var diffuse_transmission = 0.0;
    var diffuse_transmission_color = vec3<f32>(0.0);

    if (has_ext(EXT_DIFFUSE_TRANSMISSION)) {
        diffuse_transmission = draw.ext[5].w * mats[17].a;
        diffuse_transmission_color = draw.ext[6].xyz * mats[18].rgb;
    }

    // getAnisotropyInfo
    var anisotropic_t = vec3<f32>(0.0);
    var anisotropic_b = vec3<f32>(0.0);
    var anisotropy_strength = 0.0;

    if (has_ext(EXT_ANISOTROPY)) {
        var direction = vec2<f32>(1.0, 0.0);
        var strength_factor = 1.0;

        if (has_mat(14u)) {
            direction = mats[14].xy * 2.0 - vec2<f32>(1.0);
            strength_factor = mats[14].z;
        }

        let rotation = draw.ext[7].xy;
        direction = mat2x2<f32>(rotation, vec2<f32>(-rotation.y, rotation.x)) * direction;
        anisotropic_t = mat3x3<f32>(t, b, n) * normalize(vec3<f32>(direction, 0.0));
        anisotropic_b = cross(ng, anisotropic_t);
        anisotropy_strength = clamp(draw.ext[7].z * strength_factor, 0.0, 1.0);
    }

    perceptual_roughness = clamp(perceptual_roughness, 0.0, 1.0);
    metallic = clamp(metallic, 0.0, 1.0);
    let alpha_roughness = perceptual_roughness * perceptual_roughness;

    var clearcoat_fresnel = vec3<f32>(0.0);
    var albedo_sheen_scaling = 1.0;
    var diffuse_transmission_thickness = 1.0;
    var iridescence_fresnel_dielectric = vec3<f32>(0.0);
    var iridescence_fresnel_metallic = vec3<f32>(0.0);

    if (has_ext(EXT_IRIDESCENCE) && (ibl || lighting)) {
        iridescence_fresnel_dielectric = eval_iridescence(1.0, iridescence_ior, n_dot_v, iridescence_thickness, f0_dielectric);
        iridescence_fresnel_metallic = eval_iridescence(1.0, iridescence_ior, n_dot_v, iridescence_thickness, base_color.rgb);

        if (iridescence_thickness == 0.0) {
            iridescence = 0.0;
        }
    }

    if (has_ext(EXT_DIFFUSE_TRANSMISSION) && has_ext(EXT_VOLUME)) {
        let m = draw.model_view;
        diffuse_transmission_thickness = thickness * (length(m[0].xyz) + length(m[1].xyz) + length(m[2].xyz)) / 3.0;
    }

    // VolumeScatterMaterialExtension: used for weighting absorption and scattering.
    var multiscatter_color = vec3<f32>(0.0);
    var single_scatter = vec3<f32>(0.0);

    if (has_ext(EXT_VOLUME_SCATTER)) {
        multiscatter_color = draw.ext[9].xyz * mats[20].rgb;
        single_scatter = multi_to_single_scatter(multiscatter_color);
    }

    if (has_ext(EXT_CLEARCOAT)) {
        clearcoat_fresnel = f_schlick(clearcoat_f0, clearcoat_f90, clamp(dot(clearcoat_normal, v), 0.0, 1.0));
    }

    if (ibl) {
        var f_diffuse = get_diffuse_light(n) * base_color.rgb;

        if (has_ext(EXT_DIFFUSE_TRANSMISSION)) {
            var diffuse_transmission_ibl = get_diffuse_light(-n) * diffuse_transmission_color;

            if (has_ext(EXT_VOLUME)) {
                diffuse_transmission_ibl = apply_volume_attenuation(diffuse_transmission_ibl, diffuse_transmission_thickness, attenuation_color, attenuation_distance);
            }

            if (has_ext(EXT_VOLUME_SCATTER)) {
                diffuse_transmission_ibl = diffuse_transmission_ibl * (vec3<f32>(1.0) - single_scatter);
            }

            f_diffuse = mix(f_diffuse, diffuse_transmission_ibl, diffuse_transmission);
        }

        if (has_ext(EXT_TRANSMISSION)) {
            let f_specular_transmission = get_ibl_volume_refraction(n, v, perceptual_roughness, base_color.rgb, vertex, ior, thickness, attenuation_color, attenuation_distance, dispersion);
            f_diffuse = mix(f_diffuse, f_specular_transmission, transmission);
        }

        var f_specular_metal: vec3<f32>;

        if (has_ext(EXT_ANISOTROPY)) {
            f_specular_metal = get_ibl_radiance_anisotropy(n, v, perceptual_roughness, anisotropy_strength, anisotropic_b);
        } else {
            f_specular_metal = get_ibl_radiance_ggx(n, v, perceptual_roughness);
        }

        let f_specular_dielectric = f_specular_metal;
        let f_metal_fresnel_ibl = get_ibl_ggx_fresnel(n, v, perceptual_roughness, base_color.rgb, 1.0);
        var f_metal_brdf_ibl = f_metal_fresnel_ibl * f_specular_metal;
        let f_dielectric_fresnel_ibl = get_ibl_ggx_fresnel(n, v, perceptual_roughness, f0_dielectric, specular_weight);
        var f_dielectric_brdf_ibl = mix(f_diffuse, f_specular_dielectric, f_dielectric_fresnel_ibl);

        if (has_ext(EXT_IRIDESCENCE)) {
            f_metal_brdf_ibl = mix(f_metal_brdf_ibl, f_specular_metal * iridescence_fresnel_metallic, iridescence);
            f_dielectric_brdf_ibl = mix(f_dielectric_brdf_ibl, rgb_mix(f_diffuse, f_specular_dielectric, iridescence_fresnel_dielectric), iridescence);
        }

        if (has_ext(EXT_RETROREFLECTION)) {
            // Retroreflective variant of the metallic and dielectric BRDFs (MRM model): the specular
            // lobe's view direction is v_retro.
            var f_specular_retro: vec3<f32>;

            if (has_ext(EXT_ANISOTROPY)) {
                f_specular_retro = get_ibl_radiance_anisotropy(n, v_retro, perceptual_roughness, anisotropy_strength, anisotropic_b);
            } else {
                f_specular_retro = get_ibl_radiance_ggx(n, v_retro, perceptual_roughness);
            }

            var f_metal_brdf_retro = f_metal_fresnel_ibl * f_specular_retro;
            var f_dielectric_brdf_retro = mix(f_diffuse, f_specular_retro, f_dielectric_fresnel_ibl);

            if (has_ext(EXT_IRIDESCENCE)) {
                f_metal_brdf_retro = mix(f_metal_brdf_retro, f_specular_retro * iridescence_fresnel_metallic, iridescence);
                f_dielectric_brdf_retro = mix(f_dielectric_brdf_retro, rgb_mix(f_diffuse, f_specular_retro, iridescence_fresnel_dielectric), iridescence);
            }

            f_metal_brdf_ibl = mix(f_metal_brdf_ibl, f_metal_brdf_retro, retroreflection);
            f_dielectric_brdf_ibl = mix(f_dielectric_brdf_ibl, f_dielectric_brdf_retro, retroreflection);
        }

        var clearcoat_brdf = vec3<f32>(0.0);
        var f_sheen = vec3<f32>(0.0);

        if (has_ext(EXT_CLEARCOAT)) {
            clearcoat_brdf = get_ibl_radiance_ggx(clearcoat_normal, v, clearcoat_roughness);
        }

        if (has_ext(EXT_SHEEN)) {
            f_sheen = get_ibl_radiance_charlie(n, v, sheen_roughness, sheen_color);
            albedo_sheen_scaling = 1.0 - max3(sheen_color) * albedo_sheen_scaling_lut(n_dot_v, sheen_roughness);
        }

        color = mix(f_dielectric_brdf_ibl, f_metal_brdf_ibl, metallic);
        color = f_sheen + color * albedo_sheen_scaling;
        color = mix(color, clearcoat_brdf, clearcoat * clearcoat_fresnel);
        // X3D_OCCLUSION_TEXTURE (1 without one).
        color = color * (1.0 + draw.pbr.w * (mats[3].r - 1.0));
    }

    if (lighting) {
        let count = min(draw.light_count.x, 8u);

        for (var i = 0u; i < count; i = i + 1u) {
            let light = frame.lights[draw.light_index[i / 4u][i % 4u]];
            var point_to_light: vec3<f32>;
            var distance_to_light: f32;
            let m = mat3x3<f32>(light.m0.xyz, light.m1.xyz, light.m2.xyz);

            if (u32(light.color_type.w) != 1u) {
                point_to_light = light.location_ambient.xyz - vertex;
                distance_to_light = length(m * point_to_light);
            } else {
                point_to_light = -light.direction_intensity.xyz;
                distance_to_light = -1.0;
            }

            let radius = light.attenuation_radius.w;

            if (distance_to_light <= radius || radius < 0.0) {
                var l = normalize(point_to_light);
                let h = normalize(l + v);
                let n_dot_l = clamp(dot(n, l), 0.0, 1.0);
                let n_dot_h = clamp(dot(n, h), 0.0, 1.0);
                let v_dot_h = clamp(dot(v, h), 0.0, 1.0);
                var dielectric_fresnel = f_schlick(f0_dielectric * specular_weight, f90_dielectric, abs(v_dot_h));
                let metal_fresnel = f_schlick(base_color.rgb, vec3<f32>(1.0), abs(v_dot_h));
                let light_intensity = get_light_intensity(light, l, distance_to_light);
                var l_diffuse = light_intensity * n_dot_l * (base_color.rgb / M_PI);

                if (has_ext(EXT_DIFFUSE_TRANSMISSION)) {
                    l_diffuse = l_diffuse * (1.0 - diffuse_transmission);

                    if (dot(n, l) < 0.0) {
                        let diffuse_n_dot_l = clamp(dot(-n, l), 0.0, 1.0);
                        var l_diffuse_btdf = light_intensity * diffuse_n_dot_l * (diffuse_transmission_color / M_PI);
                        let l_mirror = normalize(reflect(l, n));
                        let diffuse_v_dot_h = clamp(dot(v, normalize(l_mirror + v)), 0.0, 1.0);
                        dielectric_fresnel = f_schlick(f0_dielectric * specular_weight, f90_dielectric, abs(diffuse_v_dot_h));

                        if (has_ext(EXT_VOLUME)) {
                            l_diffuse_btdf = apply_volume_attenuation(l_diffuse_btdf, diffuse_transmission_thickness, attenuation_color, attenuation_distance);
                        }

                        if (has_ext(EXT_VOLUME_SCATTER)) {
                            l_diffuse_btdf = l_diffuse_btdf * (vec3<f32>(1.0) - single_scatter);
                        }

                        l_diffuse = l_diffuse + l_diffuse_btdf * diffuse_transmission;
                    }
                }

                if (has_ext(EXT_TRANSMISSION)) {
                    // The point where the light ray leaves the volume again.
                    let transmission_ray = get_volume_transmission_ray(n, v, thickness, ior);
                    point_to_light = point_to_light - transmission_ray;
                    l = normalize(point_to_light);
                    var transmitted_light = light_intensity * get_punctual_radiance_transmission(n, v, l, alpha_roughness, base_color.rgb, ior);

                    if (has_ext(EXT_VOLUME)) {
                        transmitted_light = apply_volume_attenuation(transmitted_light, length(transmission_ray), attenuation_color, attenuation_distance);
                    }

                    l_diffuse = mix(l_diffuse, transmitted_light, transmission);
                }

                let intensity = get_light_intensity(light, l, length(m * point_to_light));
                var l_specular_metal: vec3<f32>;

                if (has_ext(EXT_ANISOTROPY)) {
                    l_specular_metal = intensity * n_dot_l * brdf_specular_ggx_anisotropy(alpha_roughness, anisotropy_strength, n, v, l, h, anisotropic_t, anisotropic_b);
                } else {
                    l_specular_metal = intensity * n_dot_l * vec3<f32>(v_ggx(n_dot_l, n_dot_v, alpha_roughness) * d_ggx(n_dot_h, alpha_roughness));
                }

                let l_specular_dielectric = l_specular_metal;
                var l_metal_brdf = metal_fresnel * l_specular_metal;
                var l_dielectric_brdf = mix(l_diffuse, l_specular_dielectric, dielectric_fresnel);

                if (has_ext(EXT_IRIDESCENCE)) {
                    l_metal_brdf = mix(l_metal_brdf, l_specular_metal * iridescence_fresnel_metallic, iridescence);
                    l_dielectric_brdf = mix(l_dielectric_brdf, rgb_mix(l_diffuse, l_specular_dielectric, iridescence_fresnel_dielectric), iridescence);
                }

                if (has_ext(EXT_RETROREFLECTION)) {
                    // NdotV_retro == NdotV: only the half vector is recomputed.
                    let h_retro = normalize(l + v_retro);
                    let n_dot_h_retro = clamp(dot(n, h_retro), 0.0, 1.0);
                    let v_dot_h_retro = clamp(dot(v_retro, h_retro), 0.0, 1.0);
                    let dielectric_fresnel_retro = f_schlick(f0_dielectric * specular_weight, f90_dielectric, abs(v_dot_h_retro));
                    let metal_fresnel_retro = f_schlick(base_color.rgb, vec3<f32>(1.0), abs(v_dot_h_retro));
                    var l_specular_retro: vec3<f32>;

                    if (has_ext(EXT_ANISOTROPY)) {
                        l_specular_retro = intensity * n_dot_l * brdf_specular_ggx_anisotropy(alpha_roughness, anisotropy_strength, n, v_retro, l, h_retro, anisotropic_t, anisotropic_b);
                    } else {
                        l_specular_retro = intensity * n_dot_l * vec3<f32>(v_ggx(n_dot_l, n_dot_v, alpha_roughness) * d_ggx(n_dot_h_retro, alpha_roughness));
                    }

                    var l_metal_brdf_retro = metal_fresnel_retro * l_specular_retro;
                    var l_dielectric_brdf_retro = mix(l_diffuse, l_specular_retro, dielectric_fresnel_retro);

                    if (has_ext(EXT_IRIDESCENCE)) {
                        l_metal_brdf_retro = mix(l_metal_brdf_retro, l_specular_retro * iridescence_fresnel_metallic, iridescence);
                        l_dielectric_brdf_retro = mix(l_dielectric_brdf_retro, rgb_mix(l_diffuse, l_specular_retro, iridescence_fresnel_dielectric), iridescence);
                    }

                    l_metal_brdf = mix(l_metal_brdf, l_metal_brdf_retro, retroreflection);
                    l_dielectric_brdf = mix(l_dielectric_brdf, l_dielectric_brdf_retro, retroreflection);
                }

                var l_clearcoat_brdf = vec3<f32>(0.0);
                var l_sheen = vec3<f32>(0.0);
                var l_albedo_sheen_scaling = 1.0;

                if (has_ext(EXT_CLEARCOAT)) {
                    let cn_dot_l = clamp(dot(clearcoat_normal, l), 0.0, 1.0);
                    let cn_dot_v = clamp(dot(clearcoat_normal, v), 0.0, 1.0);
                    let cn_dot_h = clamp(dot(clearcoat_normal, h), 0.0, 1.0);
                    let ca = clearcoat_roughness * clearcoat_roughness;
                    l_clearcoat_brdf = intensity * cn_dot_l * vec3<f32>(v_ggx(cn_dot_l, cn_dot_v, ca) * d_ggx(cn_dot_h, ca));
                }

                if (has_ext(EXT_SHEEN)) {
                    l_sheen = intensity * n_dot_l * sheen_color * d_charlie(sheen_roughness, n_dot_h) * v_sheen(n_dot_l, n_dot_v, sheen_roughness);
                    l_albedo_sheen_scaling = min(1.0 - max3(sheen_color) * albedo_sheen_scaling_lut(n_dot_v, sheen_roughness), 1.0 - max3(sheen_color) * albedo_sheen_scaling_lut(n_dot_l, sheen_roughness));
                }

                var l_color = mix(l_dielectric_brdf, l_metal_brdf, metallic);
                l_color = l_sheen + l_color * l_albedo_sheen_scaling;
                l_color = mix(l_color, l_clearcoat_brdf, clearcoat * clearcoat_fresnel);
                // X3D_SHADOWS
                l_color = mix(l_color, light.shadow_color.xyz, get_shadow_intensity(light, vertex));
                color = color + l_color;
            }
        }
    }

    // Subsurface scattering is calculated based on fresnel weighted diffuse terms.
    if (has_ext(EXT_VOLUME_SCATTER)) {
        let l_color = get_subsurface_scattering(attenuation_distance, diffuse_transmission_color, multiscatter_color, frag_coord);
        color = color + l_color * (1.0 - metallic) * (1.0 - clearcoat * clearcoat_fresnel) * (1.0 - iridescence) * (1.0 - transmission);
    }

    // getEmissiveColor: emissiveColor × emissiveStrength × emissiveTexture.
    var f_emissive = draw.emissive_transparency.xyz * mats[1].rgb;

    if (has_ext(EXT_EMISSIVE_STRENGTH)) {
        f_emissive = draw.emissive_transparency.xyz * draw.ext[0].x * mats[1].rgb;
    }

    if (no_normal_lines) {
        color = f_emissive + base_color.rgb;
    } else {
        color = f_emissive * (1.0 - clearcoat * clearcoat_fresnel) + color;
    }

    return vec4<f32>(color, base_color.a);
}

// X_ITE VolumeScatterMaterialExtension (Scatter2.glsl, SubsurfaceScattering2.glsl, ScatterSamples.js).
// The scatter buffer (RGBA32F, one row per target row in the target's order) and its depth
// (window z) are storage buffers: scatter_color / scatter_depth (frame group 8 and 9).
const SCATTER_MIN_RADIUS: f32 = 0.0013946067774668336;
const SCATTER_SAMPLES: array<vec3<f32>, 55> = array<vec3<f32>, 55>(
    vec3<f32>(3.1415927410125732, 0.0013946067774668336, 0.9697510004043579),
    vec3<f32>(5.541555881500244, 0.004235472530126572, 0.9939025044441223),
    vec3<f32>(7.941519260406494, 0.00714764092117548, 1.0191227197647095),
    vec3<f32>(10.341482162475586, 0.010134723037481308, 1.0454820394515991),
    vec3<f32>(12.741445541381836, 0.013199646957218647, 1.0730485916137695),
    vec3<f32>(15.141408920288086, 0.01634618267416954, 1.1019017696380615),
    vec3<f32>(17.541372299194336, 0.019578173756599426, 1.1321262121200562),
    vec3<f32>(19.941335678100586, 0.022899718955159187, 1.1638133525848389),
    vec3<f32>(22.341299057006836, 0.026315193623304367, 1.1970630884170532),
    vec3<f32>(24.741262435913086, 0.029829273000359535, 1.2319833040237427),
    vec3<f32>(27.141225814819336, 0.033446960151195526, 1.2686922550201416),
    vec3<f32>(29.541187286376953, 0.03717361390590668, 1.3073185682296753),
    vec3<f32>(31.941150665283203, 0.041014984250068665, 1.3480027914047241),
    vec3<f32>(34.34111404418945, 0.04497724026441574, 1.3908989429473877),
    vec3<f32>(36.7410774230957, 0.049067024141550064, 1.4361759424209595),
    vec3<f32>(39.14104080200195, 0.05329149216413498, 1.484019160270691),
    vec3<f32>(41.5410041809082, 0.0576583594083786, 1.5346332788467407),
    vec3<f32>(43.94096755981445, 0.062175966799259186, 1.5882434844970703),
    vec3<f32>(46.3409309387207, 0.06685333698987961, 1.645099401473999),
    vec3<f32>(48.74089431762695, 0.07170025259256363, 1.705477237701416),
    vec3<f32>(51.1408576965332, 0.07672734558582306, 1.7696845531463623),
    vec3<f32>(53.54082107543945, 0.08194618672132492, 1.8380637168884277),
    vec3<f32>(55.9407844543457, 0.0873693898320198, 1.9109975099563599),
    vec3<f32>(58.34074783325195, 0.09301074594259262, 1.9889150857925415),
    vec3<f32>(60.7407112121582, 0.09888537228107452, 2.0722994804382324),
    vec3<f32>(63.14067459106445, 0.10500984638929367, 2.161695718765259),
    vec3<f32>(65.54063415527344, 0.11140245944261551, 2.257721424102783),
    vec3<f32>(67.94059753417969, 0.11808337271213531, 2.3610801696777344),
    vec3<f32>(70.34056091308594, 0.12507496774196625, 2.4725754261016846),
    vec3<f32>(72.74052429199219, 0.13240206241607666, 2.5931313037872314),
    vec3<f32>(75.14048767089844, 0.1400924175977707, 2.7238144874572754),
    vec3<f32>(77.54045104980469, 0.14817705750465393, 2.8658652305603027),
    vec3<f32>(79.94041442871094, 0.15669092535972595, 3.02073335647583),
    vec3<f32>(82.34037780761719, 0.16567350924015045, 3.190126657485962),
    vec3<f32>(84.74034118652344, 0.1751696914434433, 3.3760721683502197),
    vec3<f32>(87.14030456542969, 0.18523076176643372, 3.580996036529541),
    vec3<f32>(89.54026794433594, 0.19591574370861053, 3.8078298568725586),
    vec3<f32>(91.94023132324219, 0.20729303359985352, 4.060152530670166),
    vec3<f32>(94.34019470214844, 0.21944254636764526, 4.342384338378906),
    vec3<f32>(96.74015808105469, 0.23245854675769806, 4.660052299499512),
    vec3<f32>(99.14012145996094, 0.24645330011844635, 5.02017068862915),
    vec3<f32>(101.54008483886719, 0.26156213879585266, 5.431780815124512),
    vec3<f32>(103.94004821777344, 0.2779504656791687, 5.906754970550537),
    vec3<f32>(106.34001159667969, 0.29582348465919495, 6.461002826690674),
    vec3<f32>(108.73997497558594, 0.31544047594070435, 7.116368770599365),
    vec3<f32>(111.13993835449219, 0.3371359705924988, 7.903692245483398),
    vec3<f32>(113.53990173339844, 0.36135146021842957, 8.86793327331543),
    vec3<f32>(115.93986511230469, 0.3886941373348236, 10.077631950378418),
    vec3<f32>(118.33982849121094, 0.4200162887573242, 11.641927719116211),
    vec3<f32>(120.73979187011719, 0.4565882980823517, 13.746795654296875),
    vec3<f32>(123.13975524902344, 0.5004180073738098, 16.736520767211914),
    vec3<f32>(125.53971862792969, 0.5549649596214294, 21.32761001586914),
    vec3<f32>(127.93968200683594, 0.6270133852958679, 29.298946380615234),
    vec3<f32>(130.3396453857422, 0.733019232749939, 46.6138916015625),
    vec3<f32>(132.73960876464844, 0.9366055130958557, 113.31675720214844),
);

// glTF specification for converting multi-scatter color to single scatter color.
fn multi_to_single_scatter(multiscatter_color: vec3<f32>) -> vec3<f32> {
    let s = vec3<f32>(4.09712) + 4.20863 * multiscatter_color - sqrt(vec3<f32>(9.59217) + (vec3<f32>(41.6808) + 17.7126 * multiscatter_color) * multiscatter_color);
    return vec3<f32>(1.0) - s * s;
}

fn burley_setup(radius: vec3<f32>, albedo: vec3<f32>) -> vec3<f32> {
    let a = albedo - 0.8;
    let s = 1.9 - albedo + 3.5 * (a * a);
    let l = 0.25 * (1.0 / M_PI) * radius;
    return l / s;
}

fn burley_eval(d: vec3<f32>, r: f32) -> vec3<f32> {
    let exp_r_3_d = exp(-r / (3.0 * d));
    let exp_r_d = exp_r_3_d * exp_r_3_d * exp_r_3_d;
    return (exp_r_d + exp_r_3_d) / (4.0 * d);
}

// textureLod (x3d_ScatterSamplerEXT, uv, 0.0): NEAREST, CLAMP_TO_EDGE; `uv` in GL's lower left origin.
fn scatter_index(uv: vec2<f32>) -> u32 {
    let w = i32(frame.viewport.x);
    let h = i32(frame.viewport.y);
    let x = clamp(i32(floor(uv.x * f32(w))), 0, w - 1);
    let y = clamp(i32(floor(uv.y * f32(h))), 0, h - 1);
    let row = select(h - 1 - y, y, frame.viewport.z == 1.0);
    return u32(row * w + x);
}

fn scatter_view_position(uv: vec2<f32>, depth: f32) -> vec3<f32> {
    let upos = frame.inv_projection * vec4<f32>(uv * 2.0 - 1.0, depth * 2.0 - 1.0, 1.0);
    return upos.xyz / upos.w;
}

fn get_subsurface_scattering(attenuation_distance: f32, diffuse_color: vec3<f32>, multiscatter_color: vec3<f32>, frag_coord: vec2<f32>) -> vec3<f32> {
    let scatter_distance = attenuation_distance * multiscatter_color;
    let max_color = max3(scatter_distance);
    let texel_size = 1.0 / frame.viewport.xy;
    let uv = frag_coord * texel_size;
    let center_index = scatter_index(uv);
    let center_sample = scatter_color[center_index];
    let center_depth_window = scatter_depth[center_index];
    let clip_uv = uv * 2.0 - 1.0;
    let frag_view_position = scatter_view_position(uv, center_depth_window);
    // Position of the next texel to the right (the offset is added to the clip space coordinate).
    let upos = frame.inv_projection * vec4<f32>(clip_uv.x + texel_size.x, clip_uv.y, center_depth_window * 2.0 - 1.0, 1.0);
    let offset_view_position = upos.xyz / upos.w;
    let m_per_pixel = distance(frag_view_position, offset_view_position);
    let max_radius_pixels = max_color / m_per_pixel;

    if (max_radius_pixels <= 1.0) {
        return center_sample.rgb;
    }

    var total_weight = vec3<f32>(0.0);
    var total_diffuse = vec3<f32>(0.0);
    let clamped_scatter_distance = max(vec3<f32>(SCATTER_MIN_RADIUS), scatter_distance / max_color) * max_color;
    let d = burley_setup(clamped_scatter_distance, vec3<f32>(1.0));

    for (var i = 0; i < 55; i = i + 1) {
        let scatter_sample = SCATTER_SAMPLES[i];
        let fab_angle = scatter_sample.x;
        let r = scatter_sample.y * max_radius_pixels * texel_size.x;
        let rcp_pdf = scatter_sample.z;
        let sample_coords = vec2<f32>(cos(fab_angle), sin(fab_angle)) * r;
        let sample_uv = uv + sample_coords;
        let sample_index = scatter_index(sample_uv);
        let texture_sample = scatter_color[sample_index];

        // Check if sample originates from same mesh/material.
        if (center_sample.w != texture_sample.w) {
            continue;
        }

        let sample_view_position = scatter_view_position(sample_uv, scatter_depth[sample_index]);
        let sample_distance = distance(sample_view_position, frag_view_position);
        let weight = burley_eval(d, sample_distance) * rcp_pdf;

        total_weight = total_weight + weight;
        total_diffuse = total_diffuse + weight * texture_sample.rgb;
    }

    total_weight = max(total_weight, vec3<f32>(0.0001));
    return total_diffuse / total_weight * diffuse_color;
}

// Scatter2.glsl getMaterialColor: the diffuse light of the front, modulated by the single scatter
// color, and the diffuse transmission additionally by the absorption (1 - singleScatter); alpha is the
// material id (x3d_ScatterMaterialIdEXT, draw.ext[9].w).
fn get_scatter_pass_color(vertex: vec3<f32>, n: vec3<f32>, base_color: vec4<f32>, mats: array<vec4<f32>, 21>, lighting: bool, ibl: bool) -> vec4<f32> {
    let v = normalize(-vertex);
    let n_dot_v = clamp(dot(n, v), 0.0, 1.0);
    var f0_dielectric = vec3<f32>(0.04);
    var specular_weight = 1.0;
    var f90_dielectric = vec3<f32>(1.0);

    if (has_ext(EXT_IOR)) {
        let x = draw.ext[0].y;
        f0_dielectric = vec3<f32>(pow((x - 1.0) / (x + 1.0), 2.0));
    }

    var perceptual_roughness = draw.pbr.y * mats[2].g;
    var sheen_color = vec3<f32>(0.0);
    var sheen_roughness = 0.0;

    if (has_ext(EXT_SHEEN)) {
        sheen_color = draw.ext[2].xyz * mats[10].rgb;
        sheen_roughness = draw.ext[2].w * mats[11].a;
    }

    if (has_ext(EXT_SPECULAR)) {
        let specular_color = draw.ext[0].w * draw.ext[1].xyz;
        var specular_texture = vec4<f32>(1.0);

        if (has_mat(5u)) {
            specular_texture.a = mats[5].a;
        }

        if (has_mat(6u)) {
            specular_texture = vec4<f32>(mats[6].rgb, specular_texture.a);
        }

        f0_dielectric = min(f0_dielectric * specular_color * specular_texture.rgb, vec3<f32>(1.0));
        specular_weight = draw.ext[0].z * specular_texture.a;
        f90_dielectric = vec3<f32>(specular_weight);
    }

    var thickness = 0.0;
    var attenuation_distance = 0.0;
    var attenuation_color = vec3<f32>(0.0);

    if (has_ext(EXT_VOLUME)) {
        thickness = draw.ext[4].z * mats[16].g;
        attenuation_distance = draw.ext[4].w;
        attenuation_color = draw.ext[5].xyz;
    }

    var diffuse_transmission = 0.0;
    var diffuse_transmission_color = vec3<f32>(0.0);

    if (has_ext(EXT_DIFFUSE_TRANSMISSION)) {
        diffuse_transmission = draw.ext[5].w * mats[17].a;
        diffuse_transmission_color = draw.ext[6].xyz * mats[18].rgb;
    }

    let single_scatter = multi_to_single_scatter(draw.ext[9].xyz * mats[20].rgb);
    perceptual_roughness = clamp(perceptual_roughness, 0.0, 1.0);

    var front_color = vec3<f32>(0.0);
    var albedo_sheen_scaling = 1.0;
    var diffuse_transmission_thickness = 1.0;

    if (has_ext(EXT_DIFFUSE_TRANSMISSION) && has_ext(EXT_VOLUME)) {
        let m = draw.model_view;
        diffuse_transmission_thickness = thickness * (length(m[0].xyz) + length(m[1].xyz) + length(m[2].xyz)) / 3.0;
    }

    if (ibl) {
        var f_diffuse = vec3<f32>(0.0);

        if (has_ext(EXT_DIFFUSE_TRANSMISSION)) {
            f_diffuse = get_diffuse_light(n) * diffuse_transmission_color * single_scatter;
            var diffuse_transmission_ibl = get_diffuse_light(-n) * diffuse_transmission_color;

            if (has_ext(EXT_VOLUME)) {
                diffuse_transmission_ibl = apply_volume_attenuation(diffuse_transmission_ibl, diffuse_transmission_thickness, attenuation_color, attenuation_distance);
            }

            f_diffuse = f_diffuse + diffuse_transmission_ibl * (1.0 - single_scatter) * single_scatter;
            f_diffuse = f_diffuse * diffuse_transmission;
        }

        if (has_ext(EXT_SHEEN)) {
            albedo_sheen_scaling = 1.0 - max3(sheen_color) * albedo_sheen_scaling_lut(n_dot_v, sheen_roughness);
        }

        let f_dielectric_fresnel_ibl = get_ibl_ggx_fresnel(n, v, perceptual_roughness, f0_dielectric, specular_weight);
        front_color = front_color + mix(f_diffuse, vec3<f32>(0.0), f_dielectric_fresnel_ibl) * albedo_sheen_scaling;
    }

    if (lighting) {
        let count = min(draw.light_count.x, 8u);

        for (var i = 0u; i < count; i = i + 1u) {
            let light = frame.lights[draw.light_index[i / 4u][i % 4u]];
            var point_to_light: vec3<f32>;
            var distance_to_light: f32;
            let m = mat3x3<f32>(light.m0.xyz, light.m1.xyz, light.m2.xyz);

            if (u32(light.color_type.w) != 1u) {
                point_to_light = light.location_ambient.xyz - vertex;
                distance_to_light = length(m * point_to_light);
            } else {
                point_to_light = -light.direction_intensity.xyz;
                distance_to_light = -1.0;
            }

            let radius = light.attenuation_radius.w;

            if (distance_to_light <= radius || radius < 0.0) {
                let l = normalize(point_to_light);
                let h = normalize(l + v);
                let n_dot_l = clamp(dot(n, l), 0.0, 1.0);
                let v_dot_h = clamp(dot(v, h), 0.0, 1.0);
                var dielectric_fresnel = f_schlick(f0_dielectric * specular_weight, f90_dielectric, abs(v_dot_h));
                var l_diffuse = vec3<f32>(0.0);

                if (has_ext(EXT_DIFFUSE_TRANSMISSION)) {
                    let light_intensity = get_light_intensity(light, l, distance_to_light);
                    l_diffuse = light_intensity * n_dot_l * (diffuse_transmission_color / M_PI) * single_scatter;

                    if (dot(n, l) < 0.0) {
                        let diffuse_n_dot_l = clamp(dot(-n, l), 0.0, 1.0);
                        var diffuse_btdf = light_intensity * diffuse_n_dot_l * (diffuse_transmission_color / M_PI);
                        let l_mirror = normalize(reflect(l, n));
                        let diffuse_v_dot_h = clamp(dot(v, normalize(l_mirror + v)), 0.0, 1.0);
                        dielectric_fresnel = f_schlick(f0_dielectric * specular_weight, f90_dielectric, abs(diffuse_v_dot_h));

                        if (has_ext(EXT_VOLUME)) {
                            diffuse_btdf = apply_volume_attenuation(diffuse_btdf, diffuse_transmission_thickness, attenuation_color, attenuation_distance);
                        }

                        l_diffuse = l_diffuse + diffuse_btdf * (1.0 - single_scatter) * single_scatter;
                    }

                    l_diffuse = l_diffuse * diffuse_transmission;
                }

                // Aggregated in one texture: the sheen scaling is applied here (IBL: in the pass above).
                if (has_ext(EXT_SHEEN)) {
                    albedo_sheen_scaling = min(1.0 - max3(sheen_color) * albedo_sheen_scaling_lut(n_dot_v, sheen_roughness), 1.0 - max3(sheen_color) * albedo_sheen_scaling_lut(n_dot_l, sheen_roughness));
                }

                let l_dielectric_brdf = mix(l_diffuse, vec3<f32>(0.0), dielectric_fresnel);
                front_color = front_color + l_dielectric_brdf * albedo_sheen_scaling;
            }
        }
    }

    return vec4<f32>(front_color, draw.ext[9].w);
}

// The fog interpolant (Fog2.glsl) of the draw's fog (uniform x3d_Fog set by FogContainer.setShaderUniforms).
fn get_draw_fog_interpolant(vertex: vec3<f32>) -> f32 {
    let visibility_start = draw.fog_range.x;
    let visibility_range = draw.fog_range.y - visibility_start;
    let fog_matrix = mat3x3<f32>(draw.fog_m0.xyz, draw.fog_m1.xyz, draw.fog_m2.xyz);
    let d_v = max(length(fog_matrix * vertex) - visibility_start, 0.0);

    if (u32(draw.fog_color_type.w) == 1u) {
        return max(visibility_range - d_v, 0.0) / visibility_range;
    }

    return exp(-d_v / max(visibility_range - d_v, 0.001));
}

// Fog2.glsl getFogInterpolant: 0 is the fog color, 1 the material color.
fn get_fog_interpolant(vertex: vec3<f32>) -> f32 {
    let visibility_start = frame.fog_range.x;
    let visibility_range = frame.fog_range.y - visibility_start;
    let fog_matrix = mat3x3<f32>(frame.fog_m0.xyz, frame.fog_m1.xyz, frame.fog_m2.xyz);
    let d_v = max(length(fog_matrix * vertex) - visibility_start, 0.0);

    if (u32(frame.fog_color_type.w) == 1u) {
        return max(visibility_range - d_v, 0.0) / visibility_range;
    }

    return exp(-d_v / max(visibility_range - d_v, 0.001));
}

@fragment
fn fs_main(in: VOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {
    var final_color: vec4<f32>;
    let color_material = draw.flags.z == 1u;
    let textured = draw.flags.w == 1u;
    // gl_FragCoord: origin at the lower left.
    let frag_coord = vec2<f32>(in.position.x, select(frame.viewport.y - in.position.y, in.position.y, frame.viewport.z == 1.0));

    // Normal2.glsl generateFlatNormal: normalize (cross (dFdx (vertex), dFdy (vertex))), with dFdy
    // along gl_FragCoord.y (uniform control flow for the derivatives).
    let dy_vertex = select(-dpdyFine(in.vertex), dpdyFine(in.vertex), frame.viewport.z == 1.0);
    let flat_normal = normalize(cross(dpdxFine(in.vertex), dy_vertex));

    // Stipple2.glsl stipple (): the distance along the line in pixels × lineStippleScale.
    let start_point = in.stipple.xy;
    let line_direction = normalize(in.point.xy - start_point);
    let closest = line_direction * dot(frag_coord - start_point, line_direction) + start_point;
    let stipple_s = (in.stipple.z + length(closest - start_point)) * draw.style_a.x;
    let is_line_style = draw.style.x == 2u;

    // Lines with LineProperties: texCoord = (s, 0, 0, 1).
    let tex_input = TexInput(
        array<vec4<f32>, 4>(select(in.tex_coord, vec4<f32>(stipple_s, 0.0, 0.0, 1.0), is_line_style), in.tex_coord1, in.tex_coord2, in.tex_coord3),
        in.normal,
        in.local_normal,
        in.vertex,
        in.local_vertex,
        front_facing,
    );
    // X3D_COLORSPACE_LINEAR: Texture2.glsl getTexture and the sRGB material textures convert
    // to linear, toneMap converts back.
    let linear = draw.pbr_flags.y == 1u;

    // The texture unit colors, sampled in uniform control flow (textureSample).
    var textures = array<vec4<f32>, 4>(vec4<f32>(1.0), vec4<f32>(1.0), vec4<f32>(1.0), vec4<f32>(1.0));
    for (var i = 0u; i < min(draw.tex_counts.x, 4u); i = i + 1u) {
        let c = get_texture(tex_input, i);
        textures[i] = select(c, vec4<f32>(srgb_to_linear(c.rgb), c.a), linear);
    }

    // Texture2.glsl getTextureProjectorColor, sampled in uniform control flow.
    let projector_color = get_texture_projector_color(in.vertex, in.normal, front_facing, linear);

    // MaterialTextures.js get<Name>Texture: base and emissive (and SpecularGlossiness's diffuse
    // and specularGlossiness) textures are sRGB.
    // The extension textures (5..18): specularColor, sheenColor and diffuseTransmissionColor are
    // sRGB.
    var mats: array<vec4<f32>, 21>;
    for (var k = 0u; k < 21u; k = k + 1u) {
        mats[k] = vec4<f32>(1.0);
    }
    var normal_uv = in.vertex;
    // Material (Phong): ambient and shininess textures in kinds 5 and 6.
    let mat_count = select(select(5u, 7u, draw.pbr_flags.x == 0u), 21u, draw.ext_flags.x != 0u);
    for (var k = 0u; k < mat_count; k = k + 1u) {
        let u = draw.mat_tex[k];
        if (u.x != 0u) {
            // An index past X3D_NUM_TEXTURE_TRANSFORMS / X3D_NUM_TEXTURE_COORDINATES reads the last element (ANGLE clamps).
            var tc = get_tex_coord(tex_input, min(u.z, max(draw.tex_counts.w, 1u) - 1u), min(u.w, max(draw.tex_counts.z, 1u) - 1u));
            if (u.y == 1u) {
                tc.y = 1.0 - tc.y;
            }
            let c = sample_slot(u.x, tc);
            // Material (Phong): diffuse, emissive, specular and ambient are sRGB; shininess, occlusion
            // and normal are not.
            let srgb = select(k < 2u || (k == 2u && draw.pbr_flags.x == 2u) || k == 6u || k == 10u || k == 18u || k == 20u, k < 3u || k == 5u, draw.pbr_flags.x == 0u);
            mats[k] = select(c, vec4<f32>(srgb_to_linear(c.rgb), c.a), linear && srgb);
            if (k == 4u) {
                normal_uv = tc;
            }
        }
    }

    // Normal2.glsl getNormalInfo: the geometrical TBN from the derivatives of the normal texture
    // coordinates (or the vertex) and of the vertex, unless X3D_NORMALS and X3D_TANGENTS.
    let gl_dy = select(-1.0, 1.0, frame.viewport.z == 1.0);
    var uv_dx = dpdxFine(normal_uv.xy);
    var uv_dy = dpdyFine(normal_uv.xy) * gl_dy;
    let vertex_dx = dpdxFine(in.vertex);
    let vertex_dy = dpdyFine(in.vertex) * gl_dy;

    // Hatch2.glsl: the hatch texture repeats every 32 px (× ContentScale 1); Stipple2.glsl: the
    // row of the linetype.
    let hatch_t = frag_coord / 32.0;
    let linetype_row = (f32(draw.style.y) * 2.0 + 1.0) / (f32(textureDimensions(style_tex).y) * 2.0);
    let style_uv = select(vec2<f32>(hatch_t.x, 1.0 - hatch_t.y), vec2<f32>(stipple_s, linetype_row), is_line_style);
    let style_color = textureSample(style_tex, style_samp, style_uv);

    // linetype 1 is solid; 16 returns without a texture lookup.
    if (is_line_style && draw.style.y != 1u && draw.style.y != 16u && style_color.a != 1.0) {
        discard;
    }

    // ClipPlanes2.glsl clip ()
    for (var i = 0u; i < min(draw.flags3.x, 6u); i = i + 1u) {
        let clip_plane = draw.clip_planes[i];

        if (dot(in.vertex, clip_plane.xyz) < clip_plane.w) {
            discard;
        }
    }

    // Normal2.glsl getNormalInfo.
    let normals = draw.pbr_flags.w == 1u && !(frame.viewport.w == 1.0);
    var t: vec3<f32>;
    var b: vec3<f32>;
    var ng: vec3<f32>;

    if (normals && draw.pbr_flags.z == 1u) {
        t = normalize(in.tbn_t);
        b = normalize(in.tbn_b);
        ng = normalize(in.normal);
    } else {
        if (length(uv_dx) <= 1e-2) {
            uv_dx = vec2<f32>(1.0, 0.0);
        }
        if (length(uv_dy) <= 1e-2) {
            uv_dy = vec2<f32>(0.0, 1.0);
        }
        let t_ = (uv_dy.y * vertex_dx - uv_dx.y * vertex_dy) / (uv_dx.x * uv_dy.y - uv_dy.x * uv_dx.y);
        // Normals are either present as vertex attributes or approximated (generateFlatNormal
        // for 2D and 3D geometry).
        ng = select(normalize(in.normal), flat_normal, !normals && draw.flags2.z >= 2u);
        t = normalize(t_ - ng * dot(ng, t_));
        b = cross(ng, t);
    }

    if (!front_facing) {
        t = -t;
        b = -b;
        ng = -ng;
    }


    if (draw.pbr_flags.x != 0u) {
        // MaterialInfo2.glsl / SpecularGlossiness2.glsl getBaseColor.
        let alpha = 1.0 - draw.emissive_transparency.w;
        var base_color = vec4<f32>(draw.diffuse_ambient.xyz, alpha);

        if (color_material) {
            base_color = base_color * in.color;
        }

        if (draw.mat_tex[0].x != 0u) {
            base_color = base_color * mats[0];
        } else if (textured) {
            base_color = get_texture_color(base_color, vec4<f32>(vec3<f32>(1.0), alpha), textures);
        }

        // X3D_TEXTURE_PROJECTION
        base_color = vec4<f32>(base_color.rgb * projector_color, base_color.a);

        var n = ng;

        if (draw.mat_tex[4].x != 0u) {
            var ntex = mats[4].rgb * 2.0 - vec3<f32>(1.0);
            ntex = ntex * vec3<f32>(vec2<f32>(draw.pbr.z), 1.0);
            ntex = normalize(ntex);
            n = normalize(mat3x3<f32>(t, b, ng) * ntex);
        }

        var f0 = vec3<f32>(0.04);
        var metallic = draw.pbr.x;
        var roughness = draw.pbr.y;

        if (draw.pbr_flags.x == 1u) {
            // getMetallicRoughnessInfo: roughness in g, metallic in b.
            metallic = metallic * mats[2].b;
            roughness = roughness * mats[2].g;
        } else {
            // getSpecularGlossinessInfo.
            let glossiness = draw.pbr.x * mats[2].a;
            f0 = min(draw.specular_shininess.xyz * mats[2].rgb, vec3<f32>(1.0));
            roughness = 1.0 - glossiness;
            metallic = 0.0;
        }

        let emissive = draw.emissive_transparency.xyz * mats[1].rgb;
        let ibl = frame.tone.z == 1.0 && draw.pbr_flags.w == 1u;
        let lines = draw.flags2.z < 2u && !normals;

        if (draw.ext_flags.x != 0u) {
            if (frame.pass_flags.y == 1.0) {
                final_color = get_scatter_pass_color(in.vertex, n, base_color, mats, draw.flags.y == 1u, ibl);
            } else {
                final_color = get_pbr_ext_color(in.vertex, n, t, b, ng, base_color, mats, draw.flags.y == 1u, ibl, lines, frag_coord);
            }
        } else {
            final_color = get_pbr_color(in.vertex, n, base_color, f0, metallic, roughness, emissive, mats[3].r, draw.flags.y == 1u, ibl, lines);
        }
    } else if (draw.flags.x == 1u) {
        // getMaterialColor, X3D_GOURAUD_MATERIAL or X3D_PHONG_MATERIAL (flags3.z)
        let phong = draw.flags3.z == 1u;
        var alpha = 1.0 - draw.emissive_transparency.w;
        var diffuse_color = draw.diffuse_ambient.xyz;

        if (color_material) {
            diffuse_color = in.color.rgb;
            alpha = alpha * in.color.a;
        }

        // X3D_PHONG_MATERIAL getDiffuseColor: X3D_DIFFUSE_TEXTURE, else getTextureColor (diffuseColor, specularColor).
        if (phong && has_mat(0u)) {
            let c = vec4<f32>(diffuse_color, alpha) * mats[0];
            diffuse_color = c.rgb;
            alpha = c.a;
        } else if (phong && textured) {
            let c = get_texture_color(vec4<f32>(diffuse_color, alpha), vec4<f32>(draw.specular_shininess.xyz, alpha), textures);
            diffuse_color = c.rgb;
            alpha = c.a;
        }

        // X3D_TEXTURE_PROJECTION: X3D_PHONG_MATERIAL before lighting; the X3D_GOURAUD_MATERIAL
        // path multiplies diffuseColor after its last use (no effect).
        if (phong) {
            diffuse_color = diffuse_color * projector_color;
        }

        if (draw.flags.y == 1u) {
            // getAmbientColor, getSpecularColor, getShininessFactor: the Material textures.
            var ambient_color = draw.diffuse_ambient.w * diffuse_color;
            var specular_color = draw.specular_shininess.xyz;
            var shininess = draw.specular_shininess.w;

            if (has_mat(5u)) {
                ambient_color = ambient_color * mats[5].rgb;
            }

            if (has_mat(2u)) {
                specular_color = specular_color * mats[2].rgb;
            }

            if (has_mat(6u)) {
                shininess = shininess * mats[6].a;
            }

            // Shading FLAT: X3DMaterialNode leaves out X3D_NORMALS for 2D and 3D geometry.
            var N = select(normalize(in.normal), flat_normal, frame.viewport.w == 1.0 && draw.flags2.z >= 2u);
            if (!front_facing && draw.flags2.z >= 2u) {
                N = -N;
            }
            // getNormalVector: X3D_NORMAL_TEXTURE.
            if (has_mat(4u)) {
                var ntex = mats[4].rgb * 2.0 - vec3<f32>(1.0);
                ntex = ntex * vec3<f32>(vec2<f32>(draw.pbr.z), 1.0);
                ntex = normalize(ntex);
                N = normalize(mat3x3<f32>(t, b, ng) * ntex);
            }

            final_color = vec4<f32>(get_material_color(in.vertex, N, ambient_color, diffuse_color, specular_color, shininess), alpha);
        } else {
            final_color = vec4<f32>(vec3<f32>(0.0), alpha);
        }

        // X3D_OCCLUSION_TEXTURE
        if (has_mat(3u)) {
            final_color = vec4<f32>(mix(final_color.rgb, final_color.rgb * mats[3].r, draw.pbr.w), final_color.a);
        }

        var emissive_color = draw.emissive_transparency.xyz;

        if (has_mat(1u)) {
            emissive_color = emissive_color * mats[1].rgb;
        }

        final_color = vec4<f32>(final_color.rgb + emissive_color, final_color.a);

        if (!phong && has_mat(0u)) {
            final_color = final_color * mats[0];
        } else if (textured && !phong) {
            final_color = get_texture_color(final_color, vec4<f32>(draw.specular_shininess.xyz, alpha), textures);
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
            emissive_color = get_texture_color(emissive_color, vec4<f32>(vec3<f32>(1.0), alpha), textures);
        }

        // X3D_TEXTURE_PROJECTION
        final_color = vec4<f32>(emissive_color.rgb * projector_color, emissive_color.a);
    }

    if (draw.flags2.y == 1u) {
        final_color.a = 1.0;
    }

    if (draw.style.x == 1u) {
        // Point2.glsl getPointColor (without texture)
        if (!textured) {
            let point_size = in.point.z;

            if (point_size > 1.0) {
                final_color.a = final_color.a * clamp(point_size * (0.5 - distance(vec2<f32>(0.5), in.point.xy)), 0.0, 1.0);
            } else {
                final_color.a = final_color.a * point_size;
            }
        }
    } else if (draw.style.x == 3u) {
        // Hatch2.glsl getHatchColor
        var hatch_color = select(vec4<f32>(0.0), final_color, draw.style.y == 1u);

        if (draw.style.z == 1u) {
            let hatch = vec4<f32>(style_color.rgb * draw.style_a.xyz, style_color.a);
            hatch_color = mix(hatch_color, hatch, hatch.a);
        }

        final_color = hatch_color;
    }

    // Fragment2.glsl: finalColor .rgb = getFogColor (finalColor .rgb); the Background has no fog.
    if (u32(draw.fog_color_type.w) != 0u && draw.flags2.w == 0u) {
        var interpolant: f32;

        if (draw.fog_flags.x != 0u) {
            // Fog2.glsl X3D_FOG_COORDS.
            interpolant = clamp(1.0 - in.fog_depth, 0.0, 1.0);
        } else {
            interpolant = get_draw_fog_interpolant(in.vertex);
        }

        final_color = vec4<f32>(mix(draw.fog_color_type.xyz, final_color.rgb, interpolant), final_color.a);
    }

    // finalColor .rgb = toneMap (finalColor .rgb), also for the Background (the default
    // material's shader).
    final_color = vec4<f32>(tone_map(final_color.rgb, linear), final_color.a);

    return final_color;
}
