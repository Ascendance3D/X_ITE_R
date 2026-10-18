#!/usr/bin/env python3
"""REA batch 08 shaders lane worlds for oracle/synthetic-rea-b08-shaders.

  python3 -I oracle/tools/gen_rea_b08_shaders.py oracle/synthetic-rea-b08-shaders

ComposedShader, ShaderPart (and ShaderProgram / ProgramShader / PackagedShader, which the pinned
X_ITE 16.5.0 implements as stubs). Every world and every GLSL source is authored here; nothing is
copied from a historical world. Static worlds are compared as pictures (X_ITE vs the Rust renderer,
MSAA 0 and 4); the *-runtime worlds with their *.json scripts are stepped cases.

A `data:` URL ends at a `#` (it is the fragment), so a preprocessor line in a data URL is written
`%23`; a literal `%` is `%25`. Shader files in shaders/ are plain files.
"""
import json, os, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "oracle/synthetic-rea-b08-shaders"
os.makedirs(OUT, exist_ok=True)
os.makedirs(os.path.join(OUT, "shaders"), exist_ok=True)

HEAD = "#X3D V4.0 utf8\nPROFILE Full\n"
CHECKER = "../synthetic-rea-b03/proj-checker.png"
ENVCUBE = "../synthetic-rea-b02/pbr/env.png"

worlds, scripts, files = {}, {}, {}

# ---------------------------------------------------------------------------------------------
# GLSL building blocks (GLSL ES 1.00, what a world without #version gets in WebGL 2)

VS = """attribute vec4 x3d_Vertex;
attribute vec3 x3d_Normal;
attribute vec4 x3d_TexCoord0;
attribute vec4 x3d_Color;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
uniform mat3 x3d_NormalMatrix;
varying vec3 vN;
varying vec3 vP;
varying vec2 vT;
varying vec4 vC;
void main()
{
   vec4 p = x3d_ModelViewMatrix * x3d_Vertex;
   vP = p.xyz;
   vN = normalize (x3d_NormalMatrix * x3d_Normal);
   vT = x3d_TexCoord0.st;
   vC = x3d_Color;
   gl_Position = x3d_ProjectionMatrix * p;
}
"""

FS_HEAD = """precision mediump float;
varying vec3 vN;
varying vec3 vP;
varying vec2 vT;
varying vec4 vC;
"""


def durl(src, kind):
    """A data: URL for a shader source (see the module doc for % and #)."""
    enc = src.replace("%", "%25").replace("#", "%23")
    assert '"' not in enc and "\\" not in enc
    # a #version line must be the first line of the source
    return f"data:x-shader/x-{kind},{enc}" if enc.startswith("%23version") else f"data:x-shader/x-{kind},\n{enc}"


def part(kind, src, extra=""):
    t = "VERTEX" if kind == "vertex" else "FRAGMENT"
    return f'ShaderPart {{ type "{t}" {extra} url "{durl(src, kind)}" }}'


def part_file(kind, urls, extra=""):
    t = "VERTEX" if kind == "vertex" else "FRAGMENT"
    return f'ShaderPart {{ type "{t}" {extra} url [ {" ".join(chr(34) + u + chr(34) for u in urls)} ] }}'


def composed(parts, fields="", lang="GLSL", name="", extra=""):
    d = f"DEF {name} " if name else ""
    return f'{d}ComposedShader {{ language "{lang}" {extra} {fields} parts [ {" ".join(parts)} ] }}'


def shape(shaders, geometry="Box { size 2 2 2 }", material="Material { diffuseColor 1 0 0 }", extra_app="", texture=""):
    sh = f"shaders [ {' '.join(shaders)} ]" if shaders else ""
    tx = f"texture {texture}" if texture else ""
    mt = f"material {material}" if material else ""
    return f"Shape {{ appearance Appearance {{ {mt} {tx} {extra_app} {sh} }} geometry {geometry} }}"


def world(*parts, vp="Viewpoint { position 0 0 4 }", head=HEAD, nav="NavigationInfo { headlight FALSE }"):
    return head + vp + "\n" + nav + "\n" + "\n".join(parts) + "\n"


def std(fs_body, fields="", fs_extra="", vs=VS):
    return composed([part("vertex", vs), part("fragment", FS_HEAD + fs_extra + "void main()\n{\n" + fs_body + "\n}\n")], fields)


# ---------------------------------------------------------------------------------------------
# 1. Basics: ES 1.00, ES 3.00, user fields of every type, arrays.

worlds["basic.wrl"] = world(shape([std("gl_FragColor = vec4 (tint * (0.3 + 0.7 * abs (vN.z)), 1.0);", "field SFColor tint 0.2 0.8 0.4", "uniform vec3 tint;\n")]))

VS300 = """#version 300 es
in vec4 x3d_Vertex;
in vec3 x3d_Normal;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
uniform mat3 x3d_NormalMatrix;
out vec3 vN;
void main()
{
   vN = normalize (x3d_NormalMatrix * x3d_Normal);
   gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;
}
"""
FS300 = """#version 300 es
precision mediump float;
in vec3 vN;
uniform vec3 tint;
out vec4 fragColor;
void main()
{
   fragColor = vec4 (tint * (0.2 + 0.8 * max (vN.y, 0.0)), 1.0);
}
"""
worlds["es300.wrl"] = world(shape([composed([part("vertex", VS300), part("fragment", FS300)], "field SFColor tint 0.9 0.5 0.1")], "Sphere { radius 1.2 }"))

# every SF type of addShaderFields, each visible in the picture
FS_SF = """uniform float f;
uniform int i;
uniform bool b;
uniform vec2 v2;
uniform vec3 v3;
uniform vec4 v4;
uniform mat3 m3;
uniform mat4 m4;
uniform mat3 rot;
uniform vec3 col;
uniform vec4 cola;
"""
BODY_SF = """vec3 c = vec3 (f, float (i) / 10.0, b ? 1.0 : 0.0);
   c += vec3 (v2, 0.0) * 0.3 + v3 * 0.3 + v4.xyz * v4.w * 0.3;
   c += (m3 * vec3 (0.1, 0.2, 0.3)) * 0.5 + (m4 * vec4 (0.1, 0.2, 0.3, 1.0)).xyz * 0.2;
   c += (rot * vec3 (0.0, 0.0, 1.0)) * 0.4;
   c *= col;
   gl_FragColor = vec4 (c, cola.a) * vec4 (1.0, 1.0, 1.0, 1.0) + vec4 (0.0, 0.0, 0.0, 0.0) * cola;"""
SF_FIELDS = """field SFFloat f 0.25
      field SFInt32 i 3
      field SFBool b TRUE
      field SFVec2f v2 0.5 0.25
      field SFVec3f v3 0.1 0.6 0.2
      field SFVec4f v4 0.9 0.3 0.1 0.7
      field SFMatrix3f m3 1 0 0 0 1 0 0 0 1
      field SFMatrix4f m4 1 0 0 0 0 1 0 0 0 0 1 0 0.2 0.1 0 1
      field SFRotation rot 0 1 0 0.8
      field SFColor col 1 0.9 0.8
      field SFColorRGBA cola 0.1 0.2 0.3 1"""
worlds["fields-sf.wrl"] = world(shape([std(BODY_SF, SF_FIELDS, FS_SF)], "Sphere { radius 1.3 }"))

# SFDouble, SFTime, SFVec2d, SFVec3d, SFVec4d, SFMatrix3d, SFMatrix4d, SFQuaternion? (X3D has no SFQuaternion field)
FS_SFD = """uniform float d;
uniform float t;
uniform vec2 v2;
uniform vec3 v3;
uniform vec4 v4;
uniform mat3 m3;
uniform mat4 m4;
"""
BODY_SFD = """vec3 c = vec3 (d, t / 4.0, 0.2) + vec3 (v2, 0.0) * 0.4 + v3 * 0.3 + v4.xyz * 0.2;
   c += (m3 * vec3 (0.2, 0.0, 0.5)).zxy * 0.3 + (m4 * vec4 (0.5, 0.1, 0.2, 1.0)).xyz * 0.2;
   gl_FragColor = vec4 (c, 1.0);"""
worlds["fields-sfd.wrl"] = world(shape([std(BODY_SFD, """field SFDouble d 0.3
      field SFTime t 2
      field SFVec2d v2 0.3 0.6
      field SFVec3d v3 0.4 0.1 0.2
      field SFVec4d v4 0.3 0.5 0.1 1
      field SFMatrix3d m3 0.5 0 0 0 1 0 0 0 1.5
      field SFMatrix4d m4 1 0 0 0 0 1 0 0 0 0 1 0 0.1 0.1 0.1 1""", FS_SFD)], "Sphere { radius 1.3 }"))

# MF fields: arrays of every uniform class (extra elements ignored, missing elements 0)
FS_MF = """uniform float f[4];
uniform int i[3];
uniform vec2 v2[2];
uniform vec3 v3[3];
uniform vec4 v4[2];
uniform mat3 m3[2];
uniform mat4 m4[1];
uniform bool b[3];
"""
BODY_MF = """vec3 c = vec3 (f[0] + f[1] + f[2] + f[3], float (i[0] + i[1] + i[2]) / 10.0, b[0] ? 0.3 : 0.0);
   c += v3[0] * 0.3 + v3[1] * 0.2 + v3[2] * 0.1 + vec3 (v2[0] + v2[1], 0.0) * 0.3;
   c += (v4[0] + v4[1]).xyz * 0.2 + (m3[0] * m3[1] * vec3 (1.0, 0.0, 0.0)) * 0.2;
   c += (m4[0] * vec4 (0.0, 0.0, 0.0, 1.0)).xyz;
   c += b[2] ? vec3 (0.0, 0.2, 0.0) : vec3 (0.0);
   gl_FragColor = vec4 (c, 1.0);"""
worlds["fields-mf.wrl"] = world(shape([std(BODY_MF, """field MFFloat f [ 0.1 0.2 0.15 0.05 0.9 ]
      field MFInt32 i [ 1 2 ]
      field MFVec2f v2 [ 0.1 0.2 0.3 0.1 ]
      field MFVec3f v3 [ 0.5 0.1 0.1 0.1 0.5 0.1 ]
      field MFColor c3 [ 1 0 0 ]
      field MFVec4f v4 [ 0.1 0.1 0.1 0.1 0.2 0.1 0.1 0.1 0.3 0.3 0.3 0.3 ]
      field MFMatrix3f m3 [ 1 0 0 0 1 0 0 0 1  0.5 0 0 0 0.5 0 0 0 0.5 ]
      field MFMatrix4f m4 [ 1 0 0 0 0 1 0 0 0 0 1 0 0.1 0.2 0.3 1 ]
      field MFBool b [ TRUE FALSE TRUE ]""", FS_MF)], "Sphere { radius 1.3 }"))

# a field that does not match its uniform keeps the default (0): SFVec3f into a vec4, SFFloat into a vec3
worlds["fields-mismatch.wrl"] = world(shape([std("gl_FragColor = vec4 (0.3 + a.xyz * 0.5 + b, 1.0);", """field SFVec3f a 1 1 1
      field SFFloat b 1""", "uniform vec4 a;\nuniform vec3 b;\n")], "Box { size 2 2 2 }"))

# ---------------------------------------------------------------------------------------------
# 2. Lights, material, fog, clip planes, style properties

FS_LIGHT = """struct Dummy { float x; };
uniform x3d_LightSourceParameters x3d_LightSource [x3d_MaxLights];
uniform x3d_MaterialParameters x3d_Material;
"""
BODY_LIGHT = """vec3 N = normalize (vN);
   vec3 V = normalize (-vP);
   vec3 c = x3d_Material.emissiveColor;
   for (int i = 0; i < x3d_MaxLights; ++ i)
   {
      if (x3d_LightSource [i] .type == x3d_NoneLight)
         break;
      vec3 L;
      float att = 1.0;
      if (x3d_LightSource [i] .type == x3d_DirectionalLight)
         L = normalize (-x3d_LightSource [i] .direction);
      else
      {
         vec3 d = x3d_LightSource [i] .location - vP;
         L = normalize (d);
         att = 1.0 / max (dot (x3d_LightSource [i] .attenuation, vec3 (1.0, length (d), length (d) * length (d))), 1.0);
         if (x3d_LightSource [i] .type == x3d_SpotLight)
         {
            float a = acos (clamp (dot (-L, x3d_LightSource [i] .direction), -1.0, 1.0));
            if (a >= x3d_LightSource [i] .cutOffAngle) att = 0.0;
         }
      }
      float diff = max (dot (N, L), 0.0);
      vec3 H = normalize (L + V);
      float spec = pow (max (dot (N, H), 0.0), x3d_Material.shininess * 128.0);
      c += att * x3d_LightSource [i] .intensity * x3d_LightSource [i] .color * (x3d_Material.diffuseColor * diff + x3d_Material.specularColor * spec);
      c += x3d_Material.ambientIntensity * x3d_LightSource [i] .ambientIntensity * x3d_Material.diffuseColor;
   }
   gl_FragColor = vec4 (c, 1.0 - x3d_Material.transparency);"""
LIGHTS = """DirectionalLight { direction -0.4 -0.6 -0.7 intensity 0.8 color 1 0.95 0.9 }
PointLight { location 2 1.5 1.5 intensity 1.2 color 0.4 0.6 1 radius 20 attenuation 1 0 0.05 }
SpotLight { location -2 2 2 direction 0.6 -0.6 -0.6 intensity 1 color 1 0.3 0.3 radius 20 cutOffAngle 0.9 beamWidth 0.4 attenuation 1 0 0 }"""
MAT = "Material { diffuseColor 0.8 0.4 0.2 specularColor 0.7 0.7 0.7 shininess 0.4 ambientIntensity 0.3 emissiveColor 0.02 0.02 0.05 }"
worlds["lights.wrl"] = world(LIGHTS, "Transform { translation -1.3 0 0 children " + shape([std(BODY_LIGHT, "", FS_LIGHT)], "Sphere { radius 1 }", MAT) + "}",
                             "Transform { translation 1.3 0 0 children " + shape([std(BODY_LIGHT, "", FS_LIGHT)], "Box { size 1.4 1.4 1.4 }", MAT) + "}",
                             vp="Viewpoint { position 0 0 5 }")
worlds["lights-headlight.wrl"] = world(shape([std(BODY_LIGHT, "", FS_LIGHT)], "Sphere { radius 1.2 }", MAT), nav="NavigationInfo { headlight TRUE }")
worlds["lights-local.wrl"] = world(
    "Transform { translation -1.3 0 0 children [ DirectionalLight { direction 0 -1 -1 intensity 1 global FALSE color 0.2 1 0.2 } " + shape([std(BODY_LIGHT, "", FS_LIGHT)], "Sphere { radius 0.9 }", MAT) + " ] }",
    "Transform { translation 1.3 0 0 children " + shape([std(BODY_LIGHT, "", FS_LIGHT)], "Sphere { radius 0.9 }", MAT) + "}",
    "PointLight { location 0 2 2 intensity 1 color 1 1 0.5 radius 30 }", vp="Viewpoint { position 0 0 5 }")
worlds["material-unlit.wrl"] = world(shape([std("gl_FragColor = vec4 (x3d_Material.emissiveColor + x3d_Material.diffuseColor * 0.1, 1.0 - x3d_Material.transparency);", "", "uniform x3d_MaterialParameters x3d_Material;\n")], "Sphere { radius 1.2 }", "UnlitMaterial { emissiveColor 0.9 0.5 0.1 transparency 0.25 }"))
worlds["material-none.wrl"] = world(shape([std("gl_FragColor = vec4 (x3d_Material.emissiveColor * 0.5 + x3d_Material.diffuseColor, 1.0 - x3d_Material.transparency);", "", "uniform x3d_MaterialParameters x3d_Material;\n")], "Sphere { radius 1.2 }", ""))
worlds["material-physical.wrl"] = world(shape([std("gl_FragColor = vec4 (x3d_Material.baseColor * (1.0 - x3d_Material.metallic * 0.5) + vec3 (x3d_Material.roughness * 0.3) + x3d_Material.emissiveColor, 1.0);", "", "struct PM { vec3 baseColor; float metallic; float roughness; vec3 emissiveColor; };\nuniform PM x3d_Material;\n")], "Sphere { radius 1.2 }", "PhysicalMaterial { baseColor 0.2 0.5 0.9 metallic 0.8 roughness 0.4 emissiveColor 0.1 0 0 }"))
worlds["material-legacy-names.wrl"] = world(shape([std("gl_FragColor = vec4 (x3d_FrontMaterial.diffuseColor + x3d_FrontMaterial.emissiveColor + vec3 (x3d_FrontMaterial.shininess * 0.2), 1.0 - x3d_FrontMaterial.transparency);", "", "struct FM { float ambientIntensity; vec3 diffuseColor; vec3 specularColor; vec3 emissiveColor; float shininess; float transparency; };\nuniform FM x3d_FrontMaterial;\n")], "Sphere { radius 1.2 }", MAT))

FS_FOG = "uniform x3d_FogParameters x3d_Fog;\n"
BODY_FOG = """float d = length (vP);
   float f = clamp ((x3d_Fog.visibilityRange - d) / x3d_Fog.visibilityRange, 0.0, 1.0);
   vec3 c = mix (x3d_Fog.color, vec3 (0.7, 0.4, 0.2) * (0.3 + 0.7 * abs (vN.z)), f);
   gl_FragColor = vec4 (c, 1.0);"""
worlds["fog.wrl"] = world("Fog { color 0.2 0.3 0.8 visibilityRange 9 fogType \"LINEAR\" }",
                          "Transform { translation -1.5 0 0 children " + shape([std(BODY_FOG, "", FS_FOG)], "Sphere { radius 1 }") + "}",
                          "Transform { translation 1.5 0 -3 children " + shape([std(BODY_FOG, "", FS_FOG)], "Sphere { radius 1 }") + "}", vp="Viewpoint { position 0 0 5 }")
worlds["fog-none.wrl"] = world(shape([std(BODY_FOG, "", FS_FOG)], "Sphere { radius 1.2 }"))

FS_CLIP = "uniform vec4 x3d_ClipPlane [x3d_MaxClipPlanes];\n"
BODY_CLIP = """for (int i = 0; i < x3d_MaxClipPlanes; ++ i)
   {
      if (x3d_ClipPlane [i] == vec4 (0.0))
         break;
      if (dot (vP, x3d_ClipPlane [i] .xyz) < x3d_ClipPlane [i] .w)
         discard;
   }
   gl_FragColor = vec4 (0.9, 0.6, 0.2, 1.0) * (0.3 + 0.7 * abs (vN.z));"""
worlds["clip.wrl"] = world("Transform { children [ ClipPlane { plane 1 0 0 0.2 } ", shape([std(BODY_CLIP, "", FS_CLIP)], "Sphere { radius 1.3 }"), "] }")

# discard / gl_FragCoord / derivatives
worlds["fragcoord.wrl"] = world(shape([std("gl_FragColor = vec4 (fract (gl_FragCoord.x / 32.0), fract (gl_FragCoord.y / 32.0), 0.5, 1.0);")], "Sphere { radius 1.3 }"))
worlds["discard.wrl"] = world(shape([std("if (fract (vT.s * 6.0) < 0.5) discard;\n   gl_FragColor = vec4 (0.3, 0.9, 0.5, 1.0);")], "Box { size 2 2 2 }"))

# ---------------------------------------------------------------------------------------------
# 3. Textures

FS_TEX2D = "uniform sampler2D x3d_Texture2D [x3d_MaxTextures];\n"
worlds["texture-appearance.wrl"] = world(shape([std("gl_FragColor = texture2D (x3d_Texture2D [0], vT) * vec4 (1.0, 0.9, 0.9, 1.0);", "", FS_TEX2D)], "Box { size 2 2 2 }",
                                              texture=f'ImageTexture {{ url "{CHECKER}" }}'))
worlds["texture-none.wrl"] = world(shape([std("gl_FragColor = texture2D (x3d_Texture2D [0], vT) * vec4 (0.5, 0.7, 1.0, 1.0);", "", FS_TEX2D)], "Box { size 2 2 2 }"))
worlds["texture-pixel.wrl"] = world(shape([std("gl_FragColor = texture2D (x3d_Texture2D [0], vT);", "", FS_TEX2D)], "Box { size 2 2 2 }",
                                           texture="PixelTexture { image 2 2 3 0xff0000 0x00ff00 0x0000ff 0xffff00 }"))
worlds["texture-field.wrl"] = world(shape([std("gl_FragColor = texture2D (map, vT) * 0.6 + texture2D (map2, vT * 2.0) * 0.4;", f"""field SFNode map ImageTexture {{ url "{CHECKER}" }}
      field SFNode map2 PixelTexture {{ image 2 2 3 0xff0000 0x00ff00 0x0000ff 0xffff00 }}""", "uniform sampler2D map;\nuniform sampler2D map2;\n")], "Box { size 2 2 2 }"))
worlds["texture-array-field.wrl"] = world(shape([std("gl_FragColor = texture2D (maps [0], vT) * 0.5 + texture2D (maps [1], vT) * 0.5;", f"""field MFNode maps [ ImageTexture {{ url "{CHECKER}" }} PixelTexture {{ image 2 2 3 0xff0000 0x00ff00 0x0000ff 0xffff00 }} ]""", "uniform sampler2D maps [2];\n")], "Box { size 2 2 2 }"))
worlds["texture-missing.wrl"] = world(shape([std("gl_FragColor = texture2D (map, vT) + vec4 (0.0, 0.0, 0.2, 0.0);", "field SFNode map ImageTexture { url \"does-not-exist.png\" }", "uniform sampler2D map;\n")], "Box { size 2 2 2 }"))
worlds["texture-unset.wrl"] = world(shape([std("gl_FragColor = texture2D (map, vT) + vec4 (0.0, 0.0, 0.2, 1.0);", "", "uniform sampler2D map;\n")], "Box { size 2 2 2 }"))
worlds["multitexture.wrl"] = world(shape([std("gl_FragColor = texture2D (x3d_Texture2D [0], vT) * texture2D (x3d_Texture2D [1], vT * 3.0) * 1.5;", "", FS_TEX2D)], "Box { size 2 2 2 }",
                                          texture=f'MultiTexture {{ texture [ ImageTexture {{ url "{CHECKER}" }} PixelTexture {{ image 2 2 3 0xffffff 0x808080 0x808080 0xffffff }} ] }}'))
FS_CUBE = "uniform samplerCube x3d_TextureCube [x3d_MaxTextures];\n"
worlds["texture-cube.wrl"] = world(shape([std("gl_FragColor = textureCube (x3d_TextureCube [0], reflect (normalize (vP), vN));", "", FS_CUBE)], "Sphere { radius 1.3 }",
                                          texture=f'ImageCubeMapTexture {{ url "{ENVCUBE}" }}'))
worlds["texture-cube-field.wrl"] = world(shape([std("gl_FragColor = textureCube (env, reflect (normalize (vP), vN)) * 0.8;", f'field SFNode env ImageCubeMapTexture {{ url "{ENVCUBE}" }}', "uniform samplerCube env;\n")], "Sphere { radius 1.3 }"))
worlds["texture-cube-unset.wrl"] = world(shape([std("gl_FragColor = textureCube (env, reflect (normalize (vP), vN)) * 0.5 + vec4 (0.0, 0.0, 0.25, 0.0);", "", "uniform samplerCube env;\n")], "Sphere { radius 1.3 }"))
FS3D = """#version 300 es
precision mediump float;
precision mediump sampler3D;
in vec2 vT;
uniform sampler3D vol;
out vec4 fragColor;
void main() { fragColor = texture (vol, vec3 (vT, 0.5)) * 0.5 + vec4 (0.0, 0.0, 0.25, 0.0); }
"""
VS3D = """#version 300 es
in vec4 x3d_Vertex;
in vec4 x3d_TexCoord0;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
out vec2 vT;
void main() { vT = x3d_TexCoord0.st; gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex; }
"""
worlds["texture-3d-unset.wrl"] = world(shape([composed([part("vertex", VS3D), part("fragment", FS3D)], "field SFNode vol NULL")], "Box { size 2 2 2 }"))
worlds["texture-dim-mismatch.wrl"] = world(shape([std("gl_FragColor = texture2D (x3d_Texture2D [0], vT) + vec4 (0.2, 0.0, 0.0, 0.0);", "", FS_TEX2D)], "Box { size 2 2 2 }",
                                                  texture=f'ImageCubeMapTexture {{ url "{ENVCUBE}" }}'))
worlds["texmatrix.wrl"] = world(shape([std("gl_FragColor = texture2D (x3d_Texture2D [0], (x3d_TextureMatrix [0] * vec4 (vT, 0.0, 1.0)).st);", "", FS_TEX2D + "uniform mat4 x3d_TextureMatrix [x3d_MaxTextures];\n")], "Box { size 2 2 2 }",
                                          texture=f'ImageTexture {{ url "{CHECKER}" }}', extra_app="textureTransform TextureTransform { translation 0.25 0.1 scale 2 2 rotation 0.4 center 0.5 0.5 }"))
worlds["texture-lod.wrl"] = world(shape([std("gl_FragColor = texture2DProj (x3d_Texture2D [0], vec3 (vT, 1.0)) * 0.7 + 0.3 * texture2D (x3d_Texture2D [0], vT + 0.1);", "", FS_TEX2D)], "Box { size 2 2 2 }",
                                           texture=f'ImageTexture {{ url "{CHECKER}" }}'))

# ---------------------------------------------------------------------------------------------
# 4. Attributes

worlds["attrib-color.wrl"] = world(shape([std("gl_FragColor = vC * (0.4 + 0.6 * abs (vN.z));")], "IndexedFaceSet { coord Coordinate { point [ -1 -1 0 1 -1 0 1 1 0 -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] color Color { color [ 1 0 0 0 1 0 0 0 1 1 1 0 ] } }"))
worlds["attrib-nocolor.wrl"] = world(shape([std("gl_FragColor = vec4 (vC.rgb + vec3 (0.2, 0.4, 0.8), 1.0) * (0.4 + 0.6 * abs (vN.z));")], "IndexedFaceSet { coord Coordinate { point [ -1 -1 0 1 -1 0 1 1 0 -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] }"))
VS_ATTR = """attribute vec4 x3d_Vertex;
attribute float weight;
attribute vec3 tint;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
varying vec3 c;
void main()
{
   c = tint * weight;
   gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;
}
"""
FS_ATTR = "precision mediump float;\nvarying vec3 c;\nvoid main() { gl_FragColor = vec4 (c, 1.0); }\n"
worlds["attrib-float.wrl"] = world(shape([composed([part("vertex", VS_ATTR), part("fragment", FS_ATTR)])],
    "IndexedFaceSet { coord Coordinate { point [ -1 -1 0 1 -1 0 1 1 0 -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] "
    "attrib [ FloatVertexAttribute { name \"weight\" numComponents 1 value [ 0.2 1 0.6 0.4 ] } FloatVertexAttribute { name \"tint\" numComponents 3 value [ 1 0 0 0 1 0 0 0 1 1 1 0 ] } ] }"))
worlds["attrib-missing.wrl"] = world(shape([composed([part("vertex", VS_ATTR), part("fragment", FS_ATTR)])],
    "IndexedFaceSet { coord Coordinate { point [ -1 -1 0 1 -1 0 1 1 0 -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] attrib [ FloatVertexAttribute { name \"weight\" numComponents 1 value [ 0.2 1 0.6 0.4 ] } ] }"))
VS_MAT = """attribute vec4 x3d_Vertex;
attribute mat3 rot;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
varying vec3 c;
void main()
{
   c = rot * vec3 (1.0, 0.5, 0.25);
   gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;
}
"""
worlds["attrib-matrix3.wrl"] = world(shape([composed([part("vertex", VS_MAT), part("fragment", FS_ATTR)])],
    "IndexedFaceSet { coord Coordinate { point [ -1 -1 0 1 -1 0 1 1 0 -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] "
    "attrib [ Matrix3VertexAttribute { name \"rot\" value [ 1 0 0 0 1 0 0 0 1  0 1 0 0 0 1 1 0 0  0.5 0 0 0 0.5 0 0 0 0.5  0 0 1 1 0 0 0 1 0 ] } ] }"))
worlds["texcoord-multi.wrl"] = world(shape([composed([part("vertex", """attribute vec4 x3d_Vertex;
attribute vec4 x3d_TexCoord0;
attribute vec4 x3d_TexCoord1;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
varying vec2 a;
varying vec2 b;
void main()
{
   a = x3d_TexCoord0.st;
   b = x3d_TexCoord1.st;
   gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;
}
"""), part("fragment", "precision mediump float;\nvarying vec2 a;\nvarying vec2 b;\nvoid main() { gl_FragColor = vec4 (a, b.x, 1.0) ; }\n")])],
    "IndexedFaceSet { coord Coordinate { point [ -1 -1 0 1 -1 0 1 1 0 -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] texCoord MultiTextureCoordinate { texCoord [ TextureCoordinate { point [ 0 0 1 0 1 1 0 1 ] } TextureCoordinate { point [ 1 0.5 0.5 0 0 1 0.2 0.8 ] } ] } }"))
worlds["legacy-texcoord-name.wrl"] = world(shape([composed([part("vertex", VS.replace("x3d_TexCoord0", "x3d_TexCoord")), part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (vT, 0.5, 1.0); }\n")])], "Box { size 2 2 2 }"))

# geometry kinds drawn with one shader
G = "gl_FragColor = vec4 (vec3 (0.3, 0.7, 0.4) * (0.3 + 0.7 * abs (vN.z)) + vT.xyx * 0.2, 1.0);"
geos = ["Sphere { radius 0.6 }", "Cone { height 1.2 bottomRadius 0.6 }", "Cylinder { height 1.2 radius 0.5 }", "Box { size 1 1 1 }",
        "ElevationGrid { xDimension 3 zDimension 3 xSpacing 0.4 zSpacing 0.4 height [ 0 0.3 0 0.3 0.6 0.3 0 0.3 0 ] }",
        "IndexedTriangleSet { coord Coordinate { point [ -0.5 -0.5 0 0.5 -0.5 0 0 0.6 0 ] } index [ 0 1 2 ] }",
        "Extrusion { crossSection [ 1 1 1 -1 -1 -1 -1 1 1 1 ] spine [ 0 0 0 0 1 0 ] scale [ 0.4 0.4 0.4 0.4 ] }",
        "Text { string [ \"A\" ] }"]
sh1 = std(G)
items = []
for i, g in enumerate(geos[:7]):
    x = (i % 4 - 1.5) * 1.3
    y = 0.8 - (i // 4) * 1.6
    items.append(f"Transform {{ translation {x} {y} 0 children Shape {{ appearance Appearance {{ shaders [ USE SH ] }} geometry {g} }} }}")
worlds["geometries.wrl"] = world("Transform { children Shape { appearance Appearance { shaders [ DEF SH " + sh1 + " ] } geometry Sphere { radius 0.01 } } }", *items, vp="Viewpoint { position 0 0 6 }")
worlds["lines-points.wrl"] = world(
    "Transform { translation -1 0 0 children Shape { appearance Appearance { shaders [ " + composed([part("vertex", VS), part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (1.0, 0.8, 0.2, 1.0); }\n")]) + " ] } geometry IndexedLineSet { coord Coordinate { point [ -0.8 -0.8 0 0.8 -0.8 0 0.8 0.8 0 -0.8 0.8 0 ] } coordIndex [ 0 1 2 3 0 -1 ] } } }",
    "Transform { translation 1.3 0 0 children Shape { appearance Appearance { shaders [ " + composed([part("vertex", VS), part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (0.2, 1.0, 0.8, 1.0); }\n")]) + " ] } geometry PointSet { coord Coordinate { point [ -0.5 -0.5 0 0.5 -0.5 0 0 0.5 0 ] } } } }", vp="Viewpoint { position 0 0 5 }")

# two sided and blended
worlds["two-sided.wrl"] = world(
    shape([std("gl_FragColor = vec4 (x3d_Material.diffuseColor * (0.3 + 0.7 * abs (vN.z)), 1.0);", "", "uniform x3d_MaterialParameters x3d_Material;\n")],
          "IndexedFaceSet { solid FALSE coord Coordinate { point [ -1 -1 0 1 -1 0 1 1 0 -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] }", "Material { diffuseColor 1 0.2 0.2 }", "backMaterial Material { diffuseColor 0.2 0.2 1 }"),
    vp="Viewpoint { position 0.5 0.3 4 orientation 0 1 0 0.3 }")
worlds["two-sided-rotated.wrl"] = world(
    "Transform { rotation 0 1 0 2.6 children " + shape([std("gl_FragColor = vec4 (x3d_Material.diffuseColor * (0.3 + 0.7 * abs (vN.z)), 1.0);", "", "uniform x3d_MaterialParameters x3d_Material;\n")],
          "IndexedFaceSet { solid FALSE coord Coordinate { point [ -1 -1 0 1 -1 0 1 1 0 -1 1 0 ] } coordIndex [ 0 1 2 3 -1 ] }", "Material { diffuseColor 1 0.2 0.2 }", "backMaterial Material { diffuseColor 0.2 0.2 1 }") + "}")
worlds["transparent.wrl"] = world(
    "Transform { translation 0 0 -1 children Shape { appearance Appearance { material Material { diffuseColor 1 1 0 } } geometry Box { size 3 3 0.2 } } }",
    shape([std("gl_FragColor = vec4 (0.2, 0.6, 1.0, 1.0 - x3d_Material.transparency);", "", "uniform x3d_MaterialParameters x3d_Material;\n")], "Sphere { radius 1.2 }", "Material { diffuseColor 0.2 0.6 1 transparency 0.5 }"))
worlds["blendmode.wrl"] = world(
    "Transform { translation 0 0 -1 children Shape { appearance Appearance { material Material { diffuseColor 1 1 0 } } geometry Box { size 3 3 0.2 } } }",
    shape([std("gl_FragColor = vec4 (0.2, 0.6, 1.0, 0.6);")], "Sphere { radius 1.2 }", "Material { diffuseColor 0.2 0.6 1 }", "blendMode BlendMode { }"))
worlds["depth-order.wrl"] = world(
    "Transform { translation 0 0 -1 children " + shape([std("gl_FragColor = vec4 (1.0, 0.3, 0.2, 1.0);")], "Box { size 3 3 0.4 }") + "}",
    "Transform { translation 0.4 0.3 0 children " + shape([std("gl_FragColor = vec4 (0.2, 0.5, 1.0, 1.0);")], "Sphere { radius 1 }") + "}")
worlds["depthmode.wrl"] = world(
    "Transform { translation 0 0 -1 children " + shape([std("gl_FragColor = vec4 (1.0, 0.3, 0.2, 1.0);")], "Box { size 3 3 0.4 }") + "}",
    shape([std("gl_FragColor = vec4 (0.2, 0.5, 1.0, 1.0);")], "Sphere { radius 1.1 }", extra_app="depthMode DepthMode { depthFunc \"GREATER\" }"))
worlds["layers.wrl"] = world(
    shape([std("gl_FragColor = vec4 (1.0, 0.3, 0.2, 1.0) * (0.3 + 0.7 * abs (vN.z));")], "Sphere { radius 1 }"),
    "LayerSet { order [ 0 1 ] layers [ Layer { viewport Viewport { clipBoundary [ 0.1 0.6 0.1 0.6 ] } children [ Viewpoint { position 0 0 4 } Transform { translation 0 0 -1 children " + shape([std("gl_FragColor = vec4 (0.2, 0.5, 1.0, 1.0);")], "Box { size 3 3 0.4 }") + " } ] } ] }")
worlds["transform-scale.wrl"] = world(
    "Transform { scale -1 1 1 children " + shape([std("gl_FragColor = vec4 (0.7, 0.9, 0.3, 1.0) * (0.3 + 0.7 * abs (vN.z));")], "Cone { height 2 bottomRadius 0.9 }") + "}")

# ---------------------------------------------------------------------------------------------
# 5. Selection, fallback and the stubs: first valid wins, otherwise the default shading

GOOD1 = std("gl_FragColor = vec4 (0.9, 0.2, 0.2, 1.0);")
GOOD2 = std("gl_FragColor = vec4 (0.2, 0.9, 0.2, 1.0);")
BAD_SYNTAX = composed([part("vertex", VS), part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (1.0) foo; }\n")])
NOFRAG = composed([part("vertex", VS)])
worlds["select-first-valid.wrl"] = world(shape([BAD_SYNTAX, NOFRAG, GOOD1, GOOD2]))
# measured: ANGLE links two vertex parts (each with its own main) and the program draws
worlds["select-two-vertex.wrl"] = world(shape([composed([part("vertex", VS), part("vertex", VS), part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (0.2, 1.0, 0.3, 1.0); }\n")])], "Sphere { radius 1.2 }", "Material { diffuseColor 0.9 0.6 0.1 }"))
worlds["select-second.wrl"] = world(shape([GOOD2, GOOD1]))
worlds["select-all-invalid.wrl"] = world(shape([BAD_SYNTAX, NOFRAG], material="Material { diffuseColor 0.2 0.5 0.9 }"), nav="NavigationInfo { headlight TRUE }")
worlds["select-none.wrl"] = world(shape([], material="Material { diffuseColor 0.2 0.5 0.9 }"), nav="NavigationInfo { headlight TRUE }")
worlds["select-use.wrl"] = world(
    "Transform { translation -1.3 0 0 children " + shape(["DEF SH " + GOOD1], "Sphere { radius 0.9 }") + "}",
    "Transform { translation 1.3 0 0 children " + shape(["USE SH"], "Box { size 1.5 1.5 1.5 }") + "}", vp="Viewpoint { position 0 0 5 }")
worlds["select-language.wrl"] = world(shape([composed([part("vertex", VS), part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (1.0, 0.0, 0.0, 1.0); }\n")], lang="HLSL"), GOOD2]), "Sphere { radius 1.2 }")
worlds["select-nolang.wrl"] = world(shape([composed([part("vertex", VS), part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (1.0, 0.0, 0.0, 1.0); }\n")], lang=""), GOOD2]), "Sphere { radius 1.2 }")
worlds["stub-nodes.wrl"] = world(
    "Transform { translation -1.5 1 0 children " + shape(["ProgramShader { language \"GLSL\" programs [ ShaderProgram { type \"VERTEX\" url \"data:x-shader/x-vertex,void main(){}\" } ] }"], "Sphere { radius 0.7 }", "Material { diffuseColor 1 0 0 }") + "}",
    "Transform { translation 0 1 0 children " + shape(["PackagedShader { language \"GLSL\" url \"data:x-shader/x-vertex,void main(){}\" }"], "Sphere { radius 0.7 }", "Material { diffuseColor 0 1 0 }") + "}",
    "Transform { translation 1.5 1 0 children " + shape(["ShaderProgram { type \"FRAGMENT\" url \"data:x-shader/x-fragment,void main(){}\" }"], "Sphere { radius 0.7 }", "Material { diffuseColor 0 0 1 }") + "}",
    "Transform { translation 0 -1 0 children " + shape(["ProgramShader { language \"GLSL\" }", "PackagedShader { language \"GLSL\" }", GOOD2], "Sphere { radius 0.7 }", "Material { diffuseColor 1 1 0 }") + "}",
    vp="Viewpoint { position 0 0 6 }", nav="NavigationInfo { headlight TRUE }")

# ---------------------------------------------------------------------------------------------
# 6. Invalid parts and load errors (the pictures fall back to the default shading; the console text is compared by rea_b08_shaders.py)

def bad(name, parts, lang="GLSL", extra=""):
    worlds[f"bad-{name}.wrl"] = world(shape([composed(parts, lang=lang, extra=extra)], "Sphere { radius 1.2 }", "Material { diffuseColor 0.9 0.6 0.1 }"), nav="NavigationInfo { headlight TRUE }")

FSOK = FS_HEAD + "void main() { gl_FragColor = vec4 (0.2, 1.0, 0.3, 1.0); }\n"
bad("syntax", [part("vertex", VS), part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (1.0) foo; }\n")])
bad("syntax-late", [part("vertex", VS), part("fragment", FS_HEAD + "\n\n// a comment\nvoid main()\n{\n   vec4 c = vec4 (1.0);\n   gl_FragColor = c bar;\n}\n")])
bad("undeclared", [part("vertex", VS), part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (undeclared_value); }\n")])
bad("missing-fragment", [part("vertex", VS)])
bad("missing-vertex", [part("fragment", FSOK)])
bad("wrong-type", [part("vertex", VS), part("vertex", FSOK)])
bad("type-swapped", [part("fragment", VS), part("vertex", FSOK)])
bad("no-parts", [])
bad("link-varying", [part("vertex", VS), part("fragment", FS_HEAD.replace("varying vec3 vN;", "varying vec3 vN;\nvarying vec3 notWritten;") + "void main() { gl_FragColor = vec4 (notWritten, 1.0); }\n")])
bad("empty-url", ["ShaderPart { type \"VERTEX\" url [ ] }", part("fragment", FSOK)])
bad("missing-file", [part_file("vertex", ["shaders/not-there.vs"]), part("fragment", FSOK)])
bad("missing-file-3", [part_file("vertex", ["shaders/not-there.vs", "shaders/nor-this.vs", "shaders/nope.vs"]), part("fragment", FSOK)])
bad("unknown-type", [part("vertex", VS), 'ShaderPart { type "GEOMETRY" url "data:x-shader/x-geometry,\nvoid main() {}\n" }', part("fragment", FSOK)])
bad("tess-type", [part("vertex", VS), 'ShaderPart { type "TESS_CONTROL" url "data:x-shader/x-tess,\nvoid main() {}\n" }', part("fragment", FSOK)])
bad("load-false", [part("vertex", VS, "load FALSE"), part("fragment", FSOK)])


# files: shader sources in plain files, with url fallback
files["shaders/basic.vs"] = VS
files["shaders/orange.fs"] = FS_HEAD + "void main() { gl_FragColor = vec4 (1.0, 0.5, 0.0, 1.0) * (0.3 + 0.7 * abs (vN.z)); }\n"
files["shaders/broken.fs"] = FS_HEAD + "void main() { gl_FragColor = vec4 (1.0, 0.5, 0.0, 1.0) * ; }\n"
files["shaders/preproc.fs"] = """precision mediump float;
varying vec3 vN;
#define BRIGHT 0.9
#ifdef X3D_FRAGMENT_SHADER
#define BASE vec3 (0.2, 0.4, BRIGHT)
#else
#define BASE vec3 (1.0, 0.0, 0.0)
#endif
#if defined (X3D_FOG) && x3d_MaxTextures == 4
const float K = 1.0;
#else
const float K = 0.0;
#endif
void main() { gl_FragColor = vec4 (BASE * (0.3 + 0.7 * abs (vN.z)) * K, 1.0); }
"""
worlds["file-url.wrl"] = world(shape([composed([part_file("vertex", ["shaders/basic.vs"]), part_file("fragment", ["shaders/orange.fs"])])], "Sphere { radius 1.2 }"))
worlds["file-fallback.wrl"] = world(shape([composed([part_file("vertex", ["shaders/missing.vs", "shaders/basic.vs"]), part_file("fragment", ["shaders/broken.fs", "shaders/missing.fs", "shaders/orange.fs"])])], "Sphere { radius 1.2 }"))
worlds["file-preproc.wrl"] = world(shape([composed([part_file("vertex", ["shaders/basic.vs"]), part_file("fragment", ["shaders/preproc.fs"])])], "Sphere { radius 1.2 }"))
worlds["data-base64.wrl"] = world(shape([composed([part("vertex", VS), 'ShaderPart { type "FRAGMENT" url "data:x-shader/x-fragment;base64,' + __import__("base64").b64encode(FSOK.encode()).decode() + '" }'])], "Sphere { radius 1.2 }"))

# ---------------------------------------------------------------------------------------------
# 7. X3D XML encoding (CDATA shader sources), and a PROTO that carries a shader

XML_HEAD = '<?xml version="1.0" encoding="UTF-8"?>\n<X3D profile="Full" version="4.0">\n<Scene>\n<Viewpoint position="0 0 4"/>\n<NavigationInfo headlight="false"/>\n'
worlds["xml-basic.x3d"] = XML_HEAD + f"""<Shape>
  <Appearance>
    <Material diffuseColor="1 0 0"/>
    <ComposedShader language="GLSL">
      <field name="tint" type="SFColor" accessType="inputOutput" value="0.9 0.5 0.2"/>
      <ShaderPart type="VERTEX" containerField="parts"><![CDATA[
{VS}]]></ShaderPart>
      <ShaderPart type="FRAGMENT" containerField="parts"><![CDATA[
{FS_HEAD}uniform vec3 tint;
#define SCALE 0.7
void main() {{ gl_FragColor = vec4 (tint * (0.3 + SCALE * abs (vN.z)), 1.0); }}
]]></ShaderPart>
    </ComposedShader>
  </Appearance>
  <Sphere radius="1.2"/>
</Shape>
</Scene>
</X3D>
"""
worlds["xml-fields.x3d"] = XML_HEAD + f"""<Shape>
  <Appearance>
    <ComposedShader language="GLSL" DEF="CS">
      <field name="f" type="SFFloat" accessType="inputOutput" value="0.4"/>
      <field name="v3" type="SFVec3f" accessType="inputOutput" value="0.1 0.5 0.9"/>
      <field name="map" type="SFNode" accessType="inputOutput"><ImageTexture url='"{CHECKER}"'/></field>
      <field name="arr" type="MFFloat" accessType="inputOutput" value="0.1 0.2 0.3"/>
      <ShaderPart type="VERTEX" containerField="parts" url='"data:x-shader/x-vertex,{durl(VS, "vertex").split(",", 1)[1].replace(chr(10), "%0A").replace(" ", "%20")}"'/>
      <ShaderPart type="FRAGMENT" containerField="parts"><![CDATA[
{FS_HEAD}uniform float f;
uniform vec3 v3;
uniform sampler2D map;
uniform float arr [3];
void main() {{ gl_FragColor = vec4 (v3 * f + texture2D (map, vT).rgb * 0.5 + vec3 (arr [0], arr [1], arr [2]) * 0.3, 1.0); }}
]]></ShaderPart>
    </ComposedShader>
  </Appearance>
  <Box size="2 2 2"/>
</Shape>
</Scene>
</X3D>
"""
worlds["xml-invalid.x3d"] = XML_HEAD + f"""<Shape>
  <Appearance>
    <Material diffuseColor="0.2 0.6 0.9"/>
    <ComposedShader language="GLSL">
      <ShaderPart type="VERTEX" containerField="parts"><![CDATA[
{VS}]]></ShaderPart>
      <ShaderPart type="FRAGMENT" containerField="parts"><![CDATA[
{FS_HEAD}void main() {{ gl_FragColor = vec4 (1.0) oops; }}
]]></ShaderPart>
    </ComposedShader>
  </Appearance>
  <Sphere radius="1.2"/>
</Shape>
</Scene>
</X3D>
"""

# ---------------------------------------------------------------------------------------------
# 8. Stepped runtime worlds: one key per step (KeySensor, as the batch 07 *-runtime worlds)

def runtime_json(frames, keys, start=20, step=8):
    events = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507},
              {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
    caps = []
    for i, k in enumerate(keys):
        f = start + i * step
        events.append({"frame": f, "type": "key", "key": k})
        caps.append(f + 4)
    return {"settle": True, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames,
            "capture": [12] + caps, "dump": [10] + caps + [frames - 1], "events": events}


KEYHEAD = """DEF K KeySensor { }
"""

VS_RT = VS
FS_RT = FS_HEAD + """uniform vec3 tint;
uniform float amount;
uniform bool flag;
uniform float arr [2];
uniform int mode;
void main()
{
   vec3 c = tint * (0.3 + 0.7 * abs (vN.z)) * amount + vec3 (arr [0], arr [1], 0.0);
   if (flag) c = c.gbr;
   if (mode == 1) c *= 0.5;
   gl_FragColor = vec4 (c, 1.0);
}
"""
FS_RT2 = FS_HEAD + "uniform vec3 tint;\nvoid main() { gl_FragColor = vec4 (tint.bgr * (0.4 + 0.6 * abs (vN.z)), 1.0); }\n"
worlds["fields-runtime.wrl"] = world(
    "DEF TIM TimeSensor { cycleInterval 1.2 loop TRUE }",
    "DEF INT ScalarInterpolator { key [ 0 0.5 1 ] keyValue [ 0.2 1 0.2 ] }",
    "ROUTE TIM.fraction_changed TO INT.set_fraction",
    "Transform { translation -1.4 0.8 0 children Shape { appearance Appearance { shaders [ DEF CS ComposedShader { language \"GLSL\" field SFColor tint 0.9 0.4 0.2 field SFFloat amount 1 field SFBool flag FALSE field MFFloat arr [ 0 0 ] field SFInt32 mode 0 parts [ "
    + part("vertex", VS_RT) + " " + part("fragment", FS_RT) + " ] } ] } geometry Sphere { radius 0.6 } } }",
    "Transform { translation 1.4 0.8 0 children Shape { appearance Appearance { shaders [ ComposedShader { language \"GLSL\" field SFColor tint 0.2 0.9 0.4 field SFFloat amount 0.5 field SFBool flag FALSE field MFFloat arr [ 0 0 ] field SFInt32 mode 0 parts [ "
    + part("vertex", VS_RT) + " " + part("fragment", FS_RT) + " ] } ] } geometry Box { size 1 1 1 } } }",
    "DEF RT Transform { translation 0 -1 0 children Shape { appearance Appearance { shaders [ DEF CS3 ComposedShader { language \"GLSL\" field SFColor tint 0.2 0.4 0.9 field SFFloat amount 1 field SFBool flag FALSE field MFFloat arr [ 0 0 ] field SFInt32 mode 0 parts [ "
    + part("vertex", VS_RT) + " " + part("fragment", FS_RT) + " ] } ] } geometry Sphere { radius 0.6 } } }",
    "ROUTE INT.value_changed TO CS3.amount",
    KEYHEAD,
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  initializeOnly SFNode cs USE CS
  initializeOnly SFNode cs3 USE CS3
  url "javascript:
function on_key (k)
{
   if (k == 'a') cs .tint = new SFColor (0.1, 0.9, 0.9);
   if (k == 'b') cs .amount = 0.4;
   if (k == 'c') cs .flag = true;
   if (k == 'd') cs .arr = new MFFloat (0.3, 0.1);
   if (k == 'e') cs .mode = 1;
   if (k == 'f') cs .tint = new SFColor (0.9, 0.1, 0.1);
   if (k == 'g') cs3 .flag = true;
   if (k == 'h') { cs .flag = false; cs .mode = 0; }
}
"
}
ROUTE K.keyPress TO R.on_key""", vp="Viewpoint { position 0 0 6 }")
scripts["fields-runtime"] = runtime_json(200, list("abcdefgh"))

# the shader changes: url, type, parts, activate, language, shaders field, node removal, Script-created shader
worlds["parts-runtime.wrl"] = world(
    "DEF G Group { children [ DEF T1 Transform { translation -1.5 0 0 children Shape { appearance DEF AP1 Appearance { material Material { diffuseColor 0.5 0.5 0.5 } shaders [ DEF CSA ComposedShader { language \"GLSL\" parts [ DEF VA "
    + part("vertex", VS) + " DEF FA ShaderPart { type \"FRAGMENT\" url \"data:x-shader/x-fragment,\n" + (FS_HEAD + "void main() { gl_FragColor = vec4 (1.0, 0.3, 0.2, 1.0); }\n").replace("%", "%25") + "\" } ] } ] } geometry Sphere { radius 0.6 } } } ] }",
    "Transform { translation 1.5 0 0 children Shape { appearance DEF AP2 Appearance { material Material { diffuseColor 0.2 0.3 0.8 } shaders [ DEF CSB ComposedShader { language \"GLSL\" parts [ USE VA, DEF FB " + part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (0.2, 1.0, 0.3, 1.0); }\n") + " ] } ] } geometry Box { size 1 1 1 } } }",
    "Transform { translation 0 -1.3 0 children Shape { appearance DEF AP3 Appearance { material Material { diffuseColor 0.8 0.8 0.2 } } geometry Cone { height 1 bottomRadius 0.5 } } }",
    KEYHEAD,
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  initializeOnly SFNode csa USE CSA
  initializeOnly SFNode csb USE CSB
  initializeOnly SFNode fa USE FA
  initializeOnly SFNode fb USE FB
  initializeOnly SFNode ap3 USE AP3
  initializeOnly SFNode g USE G
  initializeOnly SFNode t1 USE T1
  url "javascript:
var made = null;
function on_key (k)
{
   if (k == 'a') fa .url = new MFString ('data:x-shader/x-fragment,precision mediump float; void main() { gl_FragColor = vec4 (1.0, 1.0, 0.2, 1.0); }');
   if (k == 'b') fa .url = new MFString ('data:x-shader/x-fragment,precision mediump float; void main() { gl_FragColor = vec4 (1.0) oops; }');
   if (k == 'c') fa .url = new MFString ('data:x-shader/x-fragment,precision mediump float; void main() { gl_FragColor = vec4 (0.8, 0.2, 1.0, 1.0); }');
   if (k == 'd') csb .parts = new MFNode (csb .parts [0]);
   if (k == 'e') csb .parts = new MFNode (csb .parts [0], fb);
   if (k == 'f') csa .activate = true;
   if (k == 'g') fb .load = false;
   if (k == 'h') fb .load = true;
   if (k == 'i') { made = Browser .currentScene .createNode ('ComposedShader'); made .language = 'GLSL'; made .parts = new MFNode (csb .parts [0], fb); ap3 .shaders = new MFNode (made); }
   if (k == 'j') ap3 .shaders = new MFNode ();
   if (k == 'k') g .removeChildren = new MFNode (t1);
   if (k == 'l') g .addChildren = new MFNode (t1);
   if (k == 'm') csa .language = 'HLSL';
   if (k == 'n') csa .language = 'GLSL';
   if (k == 'o') fa .type = 'VERTEX';
   if (k == 'p') fa .type = 'FRAGMENT';
}
"
}
ROUTE K.keyPress TO R.on_key""", vp="Viewpoint { position 0 0 6 }")
scripts["parts-runtime"] = runtime_json(260, list("abcdefghijklmnop"), step=8)

# isValid / isSelected events as the world sees them
worlds["events-runtime.wrl"] = world(
    "Transform { children Shape { appearance DEF AP Appearance { material Material { diffuseColor 0.4 0.4 0.4 } shaders [ DEF BADC ComposedShader { language \"GLSL\" parts [ "
    + part("vertex", VS) + " DEF BADF ShaderPart { type \"FRAGMENT\" url \"data:x-shader/x-fragment,precision mediump float; void main() { gl_FragColor = vec4 (0.5) oops; }\" } ] } DEF GOODC ComposedShader { language \"GLSL\" parts [ "
    + part("vertex", VS) + " DEF GOODF " + part("fragment", FS_HEAD + "void main() { gl_FragColor = vec4 (0.2, 0.9, 0.5, 1.0); }\n") + " ] } ] } geometry Sphere { radius 1 } } }",
    KEYHEAD,
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOnly SFBool valid1
  inputOnly SFBool selected1
  inputOnly SFBool valid2
  inputOnly SFBool selected2
  initializeOnly SFNode ap USE AP
  initializeOnly SFNode badc USE BADC
  initializeOnly SFNode goodc USE GOODC
  initializeOnly SFNode badf USE BADF
  initializeOnly SFNode goodf USE GOODF
  url "javascript:
function valid1 (v, t) { Browser .println ('badc.isValid ' + v); }
function selected1 (v, t) { Browser .println ('badc.isSelected ' + v); }
function valid2 (v, t) { Browser .println ('goodc.isValid ' + v); }
function selected2 (v, t) { Browser .println ('goodc.isSelected ' + v); }
function on_key (k)
{
   Browser .println ('key ' + k + ' ' + badc .isValid + ' ' + badc .isSelected + ' ' + goodc .isValid + ' ' + goodc .isSelected);
   if (k == 'a') badf .url = new MFString ('data:x-shader/x-fragment,precision mediump float; void main() { gl_FragColor = vec4 (0.9, 0.2, 0.2, 1.0); }');
   if (k == 'b') ap .shaders = new MFNode (goodc, badc);
   if (k == 'c') ap .shaders = new MFNode ();
   if (k == 'd') ap .shaders = new MFNode (badc, goodc);
   if (k == 'e') goodf .url = new MFString ('data:x-shader/x-fragment,precision mediump float; void main() { gl_FragColor = vec4 (1.0) oops; }');
   if (k == 'f') goodc .activate = true;
}
"
}
ROUTE BADC.isValid TO R.valid1
ROUTE BADC.isSelected TO R.selected1
ROUTE GOODC.isValid TO R.valid2
ROUTE GOODC.isSelected TO R.selected2
ROUTE K.keyPress TO R.on_key""", vp="Viewpoint { position 0 0 4 }", nav="NavigationInfo { headlight TRUE }")
scripts["events-runtime"] = runtime_json(120, list("abcdef"), step=8)

# ShaderPart type on load, texture field changes, mf and sampler runtime, appearance texture
FS_TEXRT = FS_HEAD + """uniform sampler2D map;
uniform sampler2D x3d_Texture2D [x3d_MaxTextures];
uniform float mixer;
void main() { gl_FragColor = mix (texture2D (map, vT), texture2D (x3d_Texture2D [0], vT * 2.0), mixer); }
"""
worlds["textures-runtime.wrl"] = world(
    "Transform { children Shape { appearance DEF AP Appearance { texture DEF TX ImageTexture { url \"" + CHECKER + "\" } shaders [ DEF CS ComposedShader { language \"GLSL\" field SFNode map ImageTexture { url \"" + CHECKER + "\" } field SFFloat mixer 0 parts [ "
    + part("vertex", VS) + " " + part("fragment", FS_TEXRT) + " ] } ] } geometry Box { size 2 2 2 } } }",
    KEYHEAD,
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  initializeOnly SFNode cs USE CS
  initializeOnly SFNode ap USE AP
  url "javascript:
function on_key (k)
{
   if (k == 'a') cs .mixer = 0.5;
   if (k == 'b') cs .map = Browser .currentScene .createNode ('PixelTexture');
   if (k == 'c') cs .map .image = new SFImage (2, 2, 3, new MFInt32 (0xff0000, 0x00ff00, 0x0000ff, 0xffff00));
   if (k == 'd') ap .texture = Browser .currentScene .createNode ('PixelTexture');
   if (k == 'e') ap .texture .image = new SFImage (2, 1, 3, new MFInt32 (0xff00ff, 0x00ffff));
   if (k == 'f') cs .map = null;
   if (k == 'g') ap .texture = null;
   if (k == 'h') cs .mixer = 1;
}
"
}
ROUTE K.keyPress TO R.on_key""", vp="Viewpoint { position 0 0 4 }")
scripts["textures-runtime"] = runtime_json(160, list("abcdefgh"), step=8)

# lights / fog / clip planes changing under a shader
worlds["environment-runtime.wrl"] = world(
    "DEF DL DirectionalLight { direction -0.4 -0.6 -0.7 intensity 0.8 color 1 0.95 0.9 }",
    "DEF FG Fog { color 0.2 0.3 0.8 visibilityRange 0 fogType \"LINEAR\" }",
    "DEF CP ClipPlane { plane 1 0 0 5 enabled TRUE }",
    "Transform { children Shape { appearance Appearance { material DEF MT Material { diffuseColor 0.8 0.4 0.2 specularColor 0.7 0.7 0.7 shininess 0.4 ambientIntensity 0.3 } shaders [ ComposedShader { language \"GLSL\" parts [ "
    + part("vertex", VS) + " " + part("fragment", FS_HEAD + FS_LIGHT + FS_FOG + FS_CLIP + "void main()\n{\n" + BODY_LIGHT.replace("gl_FragColor = vec4 (c, 1.0 - x3d_Material.transparency);",
      "float d = length (vP);\n   if (x3d_Fog.visibilityRange > 0.0) c = mix (x3d_Fog.color, c, clamp ((x3d_Fog.visibilityRange - d) / x3d_Fog.visibilityRange, 0.0, 1.0));\n   for (int i = 0; i < x3d_MaxClipPlanes; ++ i) { if (x3d_ClipPlane [i] == vec4 (0.0)) break; if (dot (vP, x3d_ClipPlane [i] .xyz) < x3d_ClipPlane [i] .w) discard; }\n   gl_FragColor = vec4 (c, 1.0 - x3d_Material.transparency);") + "\n}\n") + " ] } ] } geometry Sphere { radius 1.3 } } }",
    KEYHEAD,
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  initializeOnly SFNode dl USE DL
  initializeOnly SFNode fg USE FG
  initializeOnly SFNode cp USE CP
  initializeOnly SFNode mt USE MT
  url "javascript:
function on_key (k)
{
   if (k == 'a') dl .color = new SFColor (0.2, 1, 0.2);
   if (k == 'b') dl .direction = new SFVec3f (0.5, -0.2, -0.8);
   if (k == 'c') fg .visibilityRange = 6;
   if (k == 'd') cp .plane = new SFVec4f (1, 0, 0, 0.2);
   if (k == 'e') mt .diffuseColor = new SFColor (0.2, 0.2, 0.9);
   if (k == 'f') cp .enabled = false;
   if (k == 'g') fg .color = new SFColor (0.8, 0.1, 0.1);
   if (k == 'h') mt .transparency = 0.5;
}
"
}
ROUTE K.keyPress TO R.on_key""", vp="Viewpoint { position 0 0 4 }", nav="NavigationInfo { headlight FALSE }")
scripts["environment-runtime"] = runtime_json(160, list("abcdefgh"), step=8)

# a Script creates a ComposedShader from nothing (createNode, fields via addUserDefinedField is not
# available to scripts, so the shader reads only built-in uniforms), and one from VRML text
worlds["created-runtime.wrl"] = world(
    "DEF GR Group { }",
    "DEF AP0 Appearance { }",
    "Transform { children Shape { appearance USE AP0 geometry Sphere { radius 1 } } }",
    KEYHEAD,
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  initializeOnly SFNode ap USE AP0
  initializeOnly SFNode gr USE GR
  url "javascript:
var cs = null, vp = null, fp = null;
function on_key (k)
{
   if (k == 'a')
   {
      vp = Browser .currentScene .createNode ('ShaderPart');
      vp .type = 'VERTEX';
      vp .url = new MFString ('data:x-shader/x-vertex,attribute vec4 x3d_Vertex; uniform mat4 x3d_ProjectionMatrix; uniform mat4 x3d_ModelViewMatrix; void main() { gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex; }');
      fp = Browser .currentScene .createNode ('ShaderPart');
      fp .type = 'FRAGMENT';
      fp .url = new MFString ('data:x-shader/x-fragment,precision mediump float; void main() { gl_FragColor = vec4 (0.9, 0.5, 0.1, 1.0); }');
      cs = Browser .currentScene .createNode ('ComposedShader');
      cs .language = 'GLSL';
      cs .parts = new MFNode (vp, fp);
   }
   if (k == 'b') ap .shaders = new MFNode (cs);
   if (k == 'c') fp .url = new MFString ('data:x-shader/x-fragment,precision mediump float; void main() { gl_FragColor = vec4 (0.1, 0.5, 0.9, 1.0); }');
   if (k == 'd') ap .shaders = new MFNode ();
   if (k == 'e') ap .shaders = new MFNode (cs);
   if (k == 'f') { cs = null; vp = null; fp = null; ap .shaders = new MFNode (); }
}
"
}
ROUTE K.keyPress TO R.on_key""", vp="Viewpoint { position 0 0 4 }")
scripts["created-runtime"] = runtime_json(120, list("abcdef"), step=8)

# static scripts for the XML worlds (the still-picture tool reads .wrl only)
for x in ("xml-basic", "xml-fields", "xml-invalid"):
    scripts[x] = {"settle": True, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 20, "capture": [12], "dump": [10, 19], "events": []}

# ---------------------------------------------------------------------------------------------

for name, text in worlds.items():
    with open(os.path.join(OUT, name), "w") as f:
        f.write(text)

for name, text in files.items():
    with open(os.path.join(OUT, name), "w") as f:
        f.write(text)

for name, data in scripts.items():
    with open(os.path.join(OUT, name + ".json"), "w") as f:
        json.dump(data, f, indent=1)

print(f"{len(worlds)} worlds, {len(files)} shader files, {len(scripts)} scripts -> {OUT}")
