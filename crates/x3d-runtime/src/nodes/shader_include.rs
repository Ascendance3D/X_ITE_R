//! `#include <Name>` in a ShaderPart (X_ITE's ShaderCompiler.process). The files are the pinned
//! X_ITE's WebGL 2 ShaderRegistry includes, written by `oracle/tools/gen_shader_includes.mjs` in the
//! order of the registry, which gives each its source file number (from 1) for `#line`.

const INCLUDES: [(&str, &str); 26] = [
    ("ClipPlanes", include_str!("shader_includes/01-ClipPlanes.glsl")),
    ("Fog", include_str!("shader_includes/02-Fog.glsl")),
    ("Fragment", include_str!("shader_includes/03-Fragment.glsl")),
    ("Hatch", include_str!("shader_includes/04-Hatch.glsl")),
    ("Instancing", include_str!("shader_includes/05-Instancing.glsl")),
    ("Lighting", include_str!("shader_includes/06-Lighting.glsl")),
    ("Line2", include_str!("shader_includes/07-Line2.glsl")),
    ("Logarithmic", include_str!("shader_includes/08-Logarithmic.glsl")),
    ("Material", include_str!("shader_includes/09-Material.glsl")),
    ("Normal", include_str!("shader_includes/10-Normal.glsl")),
    ("OIT", include_str!("shader_includes/11-OIT.glsl")),
    ("Perlin", include_str!("shader_includes/12-Perlin.glsl")),
    ("Point", include_str!("shader_includes/13-Point.glsl")),
    ("PointSize", include_str!("shader_includes/14-PointSize.glsl")),
    ("Shadow", include_str!("shader_includes/15-Shadow.glsl")),
    ("Stipple", include_str!("shader_includes/16-Stipple.glsl")),
    ("Texture", include_str!("shader_includes/17-Texture.glsl")),
    ("Unlit", include_str!("shader_includes/18-Unlit.glsl")),
    ("Utils", include_str!("shader_includes/19-Utils.glsl")),
    ("Vertex", include_str!("shader_includes/20-Vertex.glsl")),
    ("BRDF", include_str!("shader_includes/21-BRDF.glsl")),
    ("IBL", include_str!("shader_includes/22-IBL.glsl")),
    ("MaterialInfo", include_str!("shader_includes/23-MaterialInfo.glsl")),
    ("PBR", include_str!("shader_includes/24-PBR.glsl")),
    ("Punctual", include_str!("shader_includes/25-Punctual.glsl")),
    ("ToneMapping", include_str!("shader_includes/26-ToneMapping.glsl")),
];

fn file_number(name: &str) -> usize {
    INCLUDES.iter().position(|(n, _)| *n == name).map_or(0, |i| i + 1)
}

/// The name of an `#include <name>` line: the whole line, blanks around, the name without `/`.
fn include_name(line: &str) -> Option<&str> {
    let rest = line.trim_start().strip_prefix("#include")?.trim_start().strip_prefix('<')?;
    let name = rest.trim_end().strip_suffix('>')?;

    (!name.is_empty() && !name.contains('/')).then_some(name)
}

/// ShaderCompiler.process: each include line becomes `#line 1 N`, the file (itself processed), a
/// newline and `#line <next line> <parent>`. An unknown name gives an empty file number 0.
pub fn process(source: &str) -> String {
    process_from(source, 0)
}

fn process_from(source: &str, parent: usize) -> String {
    let mut out = String::with_capacity(source.len() + 1);

    for (i, line) in source.split('\n').enumerate() {
        match include_name(line) {
            Some(name) => {
                let number = file_number(name);
                let text = INCLUDES.iter().find(|(n, _)| *n == name).map_or("", |(_, t)| *t);

                out.push_str(&format!("#line 1 {number}\n"));
                out.push_str(&process_from(text, number));
                out.push('\n');
                out.push_str(&format!("#line {} {parent}\n", i + 2));
            }
            None => {
                out.push_str(line);
                out.push('\n');
            }
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `ShaderCompiler.process` of the pinned X_ITE on these inputs (`oracle/shader-include/process-cases.json`,
    /// written by `oracle/tools/gen_shader_include_cases.mjs`).
    #[test]
    fn process_matches_the_pinned_compiler() {
        let cases: &[(&str, &str)] = &[
        ("void main () { }\n", "void main () { }\n\n"),
        ("a\n#include <Utils>\nb\n", "a\n#line 1 19\n\n#if defined (X3D_SKINNING) || defined (X3D_INSTANCING)\nvec4\ntexelFetch (const in sampler2D _sampler, const in int index, const in int lod)\n{\n   int   x = textureSize (_sampler, lod) .x;\n   ivec2 p = ivec2 (index % x, index / x);\n   vec4  t = texelFetch (_sampler, p, lod);\n\n   return t;\n}\n#endif\n\n#if !defined (X3D_SKINNING)\n   #define getSkinVertex(vertex,normal,tangent) (vertex)\n   #define getSkinNormal(normal) (normal)\n   #define getSkinTangent(tangent) (tangent)\n#endif\n\n\n#line 3 0\nb\n\n"),
        ("#include <Nope>\nx", "#line 1 0\n\n\n#line 2 0\nx\n"),
        ("  #include   <Fog>  \nmore\n#include <ClipPlanes>", "#line 1 2\n\n#if defined (X3D_FOG)\n#if defined (X3D_VERTEX_SHADER) && defined (X3D_FOG_COORDS)\nin float x3d_FogDepth;\n\nout float fogDepth;\n\nvoid\nfog ()\n{\n   fogDepth = x3d_FogDepth;\n}\n#endif\n#if defined (X3D_FRAGMENT_SHADER)\n\nuniform x3d_FogParameters x3d_Fog;\n\n#if defined (X3D_FOG_COORDS)\n   in float fogDepth;\n#endif\n\nfloat\ngetFogInterpolant ()\n{\n   // Returns 0.0 for fog color and 1.0 for material color.\n\n   #if defined (X3D_FOG_COORDS)\n      return clamp (1.0 - fogDepth, 0.0, 1.0);\n   #else\n      float visibilityStart = x3d_Fog .visibilityStart;\n      float visibilityRange = x3d_Fog .visibilityRange - visibilityStart;\n      float dV              = max (length (x3d_Fog .matrix * vertex) - visibilityStart, 0.0);\n\n      #if defined (X3D_FOG_LINEAR)\n         return max (visibilityRange - dV, 0.0) / visibilityRange;\n      #elif defined (X3D_FOG_EXPONENTIAL)\n         return exp (-dV / max (visibilityRange - dV, 0.001));\n      #endif\n   #endif\n}\n\nvec3\ngetFogColor (const in vec3 color)\n{\n   return mix (x3d_Fog .color, color, getFogInterpolant ());\n}\n#endif\n#endif\n\n\n#line 2 0\nmore\n#line 1 1\n\n#if defined (X3D_CLIP_PLANES)\n\nuniform vec4 x3d_ClipPlane [X3D_NUM_CLIP_PLANES];\n\nvoid\nclip ()\n{\n   for (int i = 0; i < X3D_NUM_CLIP_PLANES; ++ i)\n   {\n      vec4 clipPlane = x3d_ClipPlane [i];\n\n      if (dot (vertex, clipPlane .xyz) < clipPlane .w)\n         discard;\n   }\n}\n\n#endif\n\n\n#line 4 0\n"),
        ("#include <a/b>\nz", "#include <a/b>\nz\n"),
        ("#include <>\nz", "#include <>\nz\n"),
        ("#include<Perlin>\r\nq", "#line 1 12\n\n//https://gist.github.com/patriciogonzalezvivo/670c22f3966e662d2f83\n\nfloat rand (const in vec2 co) { return fract (sin (dot (co.xy, vec2 (12.9898,78.233))) * 43758.5453); }\nfloat rand (const in vec2 co, const in float l) { return rand (vec2 (rand (co), l)); }\nfloat rand (const in vec2 co, const in float l, const in float t) { return rand (vec2 (rand (co, l), t)); }\n\nfloat\nperlin (const in vec2 p, const in float dim, const in float time)\n{\n   const float M_PI = 3.14159265358979323846;\n\n   vec2 pos   = floor (p * dim);\n   vec2 posx  = pos + vec2 (1.0, 0.0);\n   vec2 posy  = pos + vec2 (0.0, 1.0);\n   vec2 posxy = pos + vec2 (1.0);\n\n   float c   = rand (pos,   dim, time);\n   float cx  = rand (posx,  dim, time);\n   float cy  = rand (posy,  dim, time);\n   float cxy = rand (posxy, dim, time);\n\n   vec2 d = fract (p * dim);\n\n   d = -0.5 * cos (d * M_PI) + 0.5;\n\n   float ccx    = mix (c,   cx,    d.x);\n   float cycxy  = mix (cy,  cxy,   d.x);\n   float center = mix (ccx, cycxy, d.y);\n\n   return center * 2.0 - 1.0;\n}\n\nvec3\nperlin (const in vec3 p)\n{\n   return vec3 (perlin (p.xy, 1.0, 0.0),\n                perlin (p.yz, 1.0, 0.0),\n                perlin (p.zx, 1.0, 0.0));\n}\n\n\n#line 2 0\nq\n"),
        ("// #include <Utils>\nx", "// #include <Utils>\nx\n"),
        ("#include <Utils> // c\nx", "#include <Utils> // c\nx\n"),
        ];

        for (input, want) in cases {
            assert_eq!(process(input), *want, "input {input:?}");
        }
    }

    #[test]
    fn every_registry_file_is_numbered_in_order() {
        assert_eq!(file_number("ClipPlanes"), 1);
        assert_eq!(file_number("ToneMapping"), INCLUDES.len());
        assert_eq!(file_number("Missing"), 0);
    }
}
