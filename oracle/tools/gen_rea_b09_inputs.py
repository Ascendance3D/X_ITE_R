#!/usr/bin/env python3
"""REA batch 09 lane render-inputs: authored worlds for FogCoordinate / LocalFog, Tangent (normal maps),
TextureCoordinateGenerator, MultiTextureCoordinate, TextureCoordinate mapping and vertex attributes
(no archive content).

  python3 -I oracle/tools/gen_rea_b09_inputs.py oracle/synthetic-rea-b09-inputs
"""
import os, sys

out = sys.argv[1]
os.makedirs(out, exist_ok=True)

def write(name, text):
    with open(os.path.join(out, name), "w") as f:
        f.write(text)

HEAD = "#X3D V4.0 utf8\nPROFILE Full\n"
VIEW = "Viewpoint { position 0 0 7 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.15 0.18 0.22 }\n"
LIGHT = "DirectionalLight { direction -0.3 -0.4 -1 intensity 0.9 ambientIntensity 0.3 }\n"
T1 = "PixelTexture { image 4 4 4 " + " ".join("0x%02x%02x%02x%02x" % ((40 + 60 * (i % 4)) % 256, (200 - 40 * (i // 4)) % 256, (30 * i) % 256, 255) for i in range(16)) + " }"
T2 = "PixelTexture { image 4 4 3 " + " ".join("0x%02x%02x%02x" % (255 - 16 * i, (90 + 25 * i) % 256, 128) for i in range(16)) + " }"
# A 4x4 tilted normal map (tangent space, encoded rgb).
NORMALS = [(0.5 + 0.45 * ((i % 4) - 1.5) / 1.5, 0.5 + 0.45 * ((i // 4) - 1.5) / 1.5, 1.0) for i in range(16)]
NM = "PixelTexture { image 4 4 3 " + " ".join("0x%02x%02x%02x" % (int(r * 255), int(g * 255), int(b * 255)) for r, g, b in NORMALS) + " }"

def quad(x, y, appearance, s=0.45, extra="", z=0):
    return f"""Transform {{ translation {x} {y} {z} children Shape {{
  appearance {appearance}
  geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -{s} -{s} 0, {s} -{s} 0, {s} {s} 0, -{s} {s} 0 ] }}
    texCoord TextureCoordinate {{ point [ 0 0, 1 0, 1 1, 0 1 ] }} coordIndex [ 0 1 2 3 -1 ] {extra} }}
}} }}"""

MAT = "Material { diffuseColor 0.8 0.6 0.4 specularColor 0.4 0.3 0.2 shininess 0.3 }"

# ---- fog coordinates (global Fog) ----
def fogworld(fog, shapes):
    return HEAD + VIEW + LIGHT + fog + "\n" + shapes + "\n"

def strip(x, depths, geom="IndexedFaceSet"):
    return f"""Transform {{ translation {x} 0 0 children Shape {{ appearance Appearance {{ material {MAT} }}
  geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -0.45 -1.5 0, 0.45 -1.5 0, 0.45 -0.5 0, -0.45 -0.5 0, 0.45 0.5 0, -0.45 0.5 0, 0.45 1.5 0, -0.45 1.5 0 ] }}
    coordIndex [ 0 1 2 3 -1 3 2 4 5 -1 5 4 6 7 -1 ]
    fogCoord FogCoordinate {{ depth [ {depths} ] }} }} }} }}"""

shapes = strip(-3, "0 0 0.3 0.3 0.6 0.6 1 1") + strip(-1.5, "1 1 0.5 0.5 0 0 0.5 0.5") + \
    strip(0, "0 0 0 0 0 0 0 0") + strip(1.5, "1 1 1 1 1 1 1 1") + strip(3, "-0.5 1.5 0.25 0.75 2 -1 0.1 0.9")
write("fogcoord-linear.wrl", fogworld("Fog { color 0.9 0.3 0.2 fogType \"LINEAR\" visibilityRange 12 }", shapes))
write("fogcoord-exponential.wrl", fogworld("Fog { color 0.2 0.8 0.4 fogType \"EXPONENTIAL\" visibilityRange 6 }", shapes))
write("fogcoord-nofog.wrl", fogworld("", shapes))
write("fogcoord-disabled-fog.wrl", fogworld("Fog { color 0.9 0.3 0.2 visibilityRange 0 }", shapes))
# sphere without coordinates, Fog without FogCoordinate next to the FogCoordinate strips
mixed = strip(-2, "0 0 0.5 0.5 1 1 0.2 0.2") + f"Transform {{ translation 1.5 0 0 children Shape {{ appearance Appearance {{ material {MAT} }} geometry Sphere {{ radius 1.2 }} }} }}"
write("fogcoord-mixed.wrl", fogworld("Fog { color 0.5 0.6 1 visibilityRange 9 }", mixed))
# other geometry carrying fogCoord: ElevationGrid, IndexedLineSet, LineSet
eg = f"""Transform {{ translation -2.5 -1 0 children Shape {{ appearance Appearance {{ material {MAT} }} geometry ElevationGrid {{ xDimension 3 zDimension 3 xSpacing 1 zSpacing 1 solid FALSE height [ 0 0.4 0 0.4 0.8 0.4 0 0.4 0 ]
  fogCoord FogCoordinate {{ depth [ 0 0.2 0.4 0.2 0.5 0.7 0.5 0.8 1 ] }} }} }} }}"""
ils = """Transform { translation 1 1 0 children Shape { appearance Appearance { material Material { emissiveColor 1 1 0.5 diffuseColor 0 0 0 } }
  geometry IndexedLineSet { coord Coordinate { point [ -1 -1 0, 0 1 0, 1 -1 0, 2 1 0 ] } coordIndex [ 0 1 2 3 -1 ] fogCoord FogCoordinate { depth [ 0 0.3 0.7 1 ] } } } }"""
ls = """Transform { translation 1 -1.5 0 children Shape { appearance Appearance { material Material { emissiveColor 0.5 1 1 diffuseColor 0 0 0 } }
  geometry LineSet { vertexCount [ 3 ] coord Coordinate { point [ -1 0 0, 0 1 0, 1 0 0 ] } fogCoord FogCoordinate { depth [ 0 1 0.5 ] } } } }"""
write("fogcoord-geometries.wrl", fogworld("Fog { color 0.9 0.5 0.1 visibilityRange 10 }", eg + ils + ls))
# PBR material with fog coordinates
pbr = strip(-1, "0 0 0.5 0.5 1 1 0.3 0.3").replace(MAT, "PhysicalMaterial { baseColor 0.8 0.7 0.4 metallic 0.2 roughness 0.5 }")
write("fogcoord-pbr.wrl", fogworld("Fog { color 0.3 0.4 0.9 visibilityRange 9 }", pbr))
# unlit material and texture
unlit = strip(-1, "0 0 0.5 0.5 1 1 0.3 0.3").replace(MAT, "UnlitMaterial { emissiveColor 0.2 0.9 0.5 }")
write("fogcoord-unlit.wrl", fogworld("Fog { color 0.8 0.2 0.9 visibilityRange 9 }", unlit))
# FogCoordinate with fewer depths than vertices, runtime-ish short array
write("fogcoord-short.wrl", fogworld("Fog { color 0.9 0.3 0.2 visibilityRange 12 }", strip(-2, "0.2 0.9 0.4") + strip(1, "0.5")))

# ---- LocalFog ----
def sph(x, y=0, z=0, r=0.8):
    return f"Transform {{ translation {x} {y} {z} children Shape {{ appearance Appearance {{ material {MAT} }} geometry Sphere {{ radius {r} }} }} }}\n"

row = lambda xs, z=0: "".join(sph(x, 0, z) for x in xs)
write("localfog-basic.wrl", HEAD + VIEW + LIGHT +
      sph(-3.2, 0, -4) + "Group { children [ LocalFog { color 1 0.2 0.2 visibilityRange 8 }\n" + sph(-1, 0, -3) + sph(1, 0, -6) + "] }\n" + sph(3.2, 0, -4))
write("localfog-global.wrl", HEAD + VIEW + LIGHT + "Fog { color 0.2 0.3 0.8 visibilityRange 14 }\n" +
      sph(-3, 0, -2) + "Group { children [ LocalFog { color 1 0.8 0.1 fogType \"EXPONENTIAL\" visibilityRange 9 }\n" + sph(0, 0, -2) + sph(2, 0, -8) + "] }\n" + sph(3, 1.5, -6))
write("localfog-nested.wrl", HEAD + VIEW + LIGHT +
      "Group { children [ LocalFog { color 0.9 0.1 0.1 visibilityRange 10 }\n" + sph(-3, 0, -3) +
      "Transform { translation 0 0 -3 children [ LocalFog { color 0.1 0.9 0.1 visibilityRange 6 }\n" + sph(0, 0, 0) + sph(2.5, 0, -4) + "] }\n" + sph(3, 1.5, -5) + "] }\n")
write("localfog-twin.wrl", HEAD + VIEW + LIGHT +
      "Group { children [ LocalFog { color 0.9 0.1 0.1 visibilityRange 10 } LocalFog { color 0.1 0.2 0.9 visibilityRange 7 }\n" + sph(-2, 0, -4) + sph(2, 0, -8) + "] }\n")
write("localfog-disabled.wrl", HEAD + VIEW + LIGHT +
      "Fog { color 0.2 0.3 0.8 visibilityRange 14 }\nGroup { children [ LocalFog { enabled FALSE color 1 0.2 0.2 visibilityRange 8 }\n" + sph(-2, 0, -4) + sph(2, 0, -8) + "] }\n")
write("localfog-nofog-range0.wrl", HEAD + VIEW + LIGHT +
      "Fog { color 0.2 0.3 0.8 visibilityRange 14 }\nGroup { children [ LocalFog { color 1 0.2 0.2 visibilityRange 0 }\n" + sph(-2, 0, -4) + sph(2, 0, -8) + "] }\n")
write("localfog-start.wrl", HEAD + VIEW + LIGHT +
      "Group { children [ LocalFog { color 1 0.5 0.1 visibilityStart 4 visibilityRange 12 }\n" + sph(-3, 0, 0) + sph(0, 0, -4) + sph(3, 0, -9) + "] }\n")
# fog space is the LocalFog's own coordinate system: a scaled Transform changes the distances
write("localfog-scaled.wrl", HEAD + VIEW + LIGHT +
      "Transform { scale 0.5 0.5 0.5 translation 0 0 -2 rotation 0 1 0 0.6 children [ LocalFog { color 0.2 0.9 0.9 visibilityRange 8 }\n" +
      sph(-4, 0, -4, 1.6) + sph(4, 0, 0, 1.6) + sph(0, 0, -10, 1.6) + "] }\n")
write("localfog-fogcoord.wrl", fogworld("Fog { color 0.2 0.3 0.8 visibilityRange 14 }\n", "") .replace("\n\n", "\n") +
      "Group { children [ LocalFog { color 1 0.2 0.5 visibilityRange 12 }\n" + strip(-2, "0 0 0.3 0.3 0.6 0.6 1 1") + strip(1.5, "1 1 0.5 0.5 0 0 0.5 0.5") + "] }\n" + strip(3.5, "0 0 0.3 0.3 0.6 0.6 1 1"))
write("localfog-transparent.wrl", HEAD + VIEW + LIGHT +
      "Group { children [ LocalFog { color 1 0.2 0.2 visibilityRange 8 }\n" +
      f"Transform {{ translation -1 0 -3 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.4 0.8 0.4 transparency 0.5 }} }} geometry Sphere {{ radius 1 }} }} }}\n" + "] }\n" +
      f"Transform {{ translation 1 0 -3 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.4 0.8 0.4 transparency 0.5 }} }} geometry Sphere {{ radius 1 }} }} }}\n")
write("localfog-pbr-tex.wrl", HEAD + VIEW + LIGHT +
      "Group { children [ LocalFog { color 0.7 0.7 1 visibilityRange 9 }\n" +
      f"Transform {{ translation -1.5 0 -2 children Shape {{ appearance Appearance {{ material PhysicalMaterial {{ baseColor 1 1 1 baseTexture {T1} }} }} geometry Box {{ size 1.5 1.5 1.5 }} }} }}\n" +
      f"Transform {{ translation 1.5 0 -5 children Shape {{ appearance Appearance {{ material UnlitMaterial {{ emissiveTexture {T2} }} }} geometry Box {{ size 1.5 1.5 1.5 }} }} }}\n" + "] }\n")

# ---- tangents / normal maps ----
def nm_sphere_world(material_kind, tangent):
    mat = {"phong": f"Material {{ diffuseColor 0.8 0.8 0.8 specularColor 0.6 0.6 0.6 shininess 0.4 normalTexture {NM} }}",
           "pbr": f"PhysicalMaterial {{ baseColor 0.8 0.8 0.8 metallic 0.1 roughness 0.4 normalTexture {NM} }}"}[material_kind]
    return mat

PTS = "-0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0"
def nm_quad(x, material, tangent, ci="0 1 2 3 -1", tex="0 0, 1 0, 1 1, 0 1", pts=PTS):
    return f"""Transform {{ translation {x} 0 0 children Shape {{ appearance Appearance {{ material {material} }}
 geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ {pts} ] }} coordIndex [ {ci} ]
   normal Normal {{ vector [ 0 0 1, 0 0 1, 0 0 1, 0 0 1 ] }} texCoord TextureCoordinate {{ point [ {tex} ] }} {tangent} }} }} }}"""

lights = "DirectionalLight { direction 0.5 -0.4 -1 intensity 1.2 }\nPointLight { location 1 1 3 intensity 1 radius 30 }\n"
for kind in ["phong", "pbr"]:
    m = nm_sphere_world(kind, "")
    body = nm_quad(-3, m, "") + \
        nm_quad(0, m, "tangent Tangent { vector [ 1 0 0 1, 1 0 0 1, 1 0 0 1, 1 0 0 1 ] }") + \
        nm_quad(3, m, "tangent Tangent { vector [ 0 1 0 -1, 0 1 0 -1, 0 1 0 -1, 0 1 0 -1 ] }")
    write(f"tangent-{kind}.wrl", HEAD + "Viewpoint { position 0 0 8 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.1 0.1 0.12 }\n" + lights + body)
    # tangents of a rotated texture mapping, mirrored uv (generated tangents)
    body = nm_quad(-3, m, "", tex="1 0, 0 0, 0 1, 1 1") + nm_quad(0, m, "", tex="0 0, 0 1, 1 1, 1 0") + \
        nm_quad(3, m, "tangent Tangent { vector [ 0 -1 0 1, 0 -1 0 1, 0 -1 0 1, 0 -1 0 1 ] }", tex="0 0, 2 0, 2 2, 0 2")
    write(f"tangent-gen-{kind}.wrl", HEAD + "Viewpoint { position 0 0 8 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.1 0.1 0.12 }\n" + lights + body)
    # Tangent with fewer vectors than vertices, per-face index, sphere (generated), box (generated)
    body = nm_quad(-3, m, "tangent Tangent { vector [ 1 0 0 1, 0 1 0 1 ] }") + \
        f"Transform {{ translation 0 0 0 children Shape {{ appearance Appearance {{ material {m} }} geometry Sphere {{ radius 1.1 }} }} }}\n" + \
        f"Transform {{ translation 3 0 0 children Shape {{ appearance Appearance {{ material {m} }} geometry Box {{ size 1.6 1.6 1.6 }} }} }}\n"
    write(f"tangent-prims-{kind}.wrl", HEAD + "Viewpoint { position 0 0 8 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.1 0.1 0.12 }\n" + lights + body)
    # ElevationGrid / IndexedTriangleSet with tangents, two-sided
    eg = f"""Transform {{ translation -2 -1 0 children Shape {{ appearance Appearance {{ material {m} }} geometry ElevationGrid {{ xDimension 3 zDimension 3 xSpacing 1 zSpacing 1 solid FALSE height [ 0 0.4 0 0.4 0.8 0.4 0 0.4 0 ]
     texCoord TextureCoordinate {{ point [ 0 0, 0.5 0, 1 0, 0 0.5, 0.5 0.5, 1 0.5, 0 1, 0.5 1, 1 1 ] }} }} }} }}"""
    its = f"""Transform {{ translation 2 0 0 children Shape {{ appearance Appearance {{ material {m} }} geometry IndexedTriangleSet {{ solid FALSE coord Coordinate {{ point [ -1 -1 0, 1 -1 0, 0 1 0, 1 1 0 ] }} index [ 0 1 2 1 3 2 ]
     texCoord TextureCoordinate {{ point [ 0 0, 1 0, 0.5 1, 1 1 ] }} tangent Tangent {{ vector [ 1 0 0 1, 1 0 0 1, 0 1 0 1, 0 1 0 1 ] }} }} }} }}"""
    write(f"tangent-grids-{kind}.wrl", HEAD + "Viewpoint { position 0 0 8 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.1 0.1 0.12 }\n" + lights + eg + its)

# ---- TextureCoordinateGenerator modes ----
MODES = ["SPHERE", "CAMERASPACENORMAL", "CAMERASPACEPOSITION", "CAMERASPACEREFLECTIONVECTOR", "SPHERE-LOCAL", "COORD",
         "COORD-EYE", "NOISE", "NOISE-EYE", "SPHERE-REFLECT", "SPHERE-REFLECT-LOCAL", "BOGUS",
         "CAMERA_SPACE_NORMAL", "CAMERA_SPACE_POSITION", "CAMERA_SPACE_REFLECTION_VECTOR", "SPHERE_LOCAL", "COORD_EYE", "NOISE_EYE", "SPHERE_REFLECT", "SPHERE_REFLECT_LOCAL"]
GENT = "PixelTexture { image 4 4 3 " + " ".join("0x%02x%02x%02x" % ((i % 4) * 80 + 10, (i // 4) * 80 + 10, 255 - 15 * i) for i in range(16)) + " }"
def gen_cell(k, mode, param="", geom="Sphere { radius 0.42 }"):
    col, row = k % 5, k // 5
    return f"""Transform {{ translation {-2.4 + 1.2 * col} {1.8 - 1.2 * row} 0 children Transform {{ rotation 0.3 1 0.2 0.9 children Shape {{
  appearance Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {GENT} }}
  geometry IndexedFaceSet {{ coord Coordinate {{ point [ -0.4 -0.4 0.4, 0.4 -0.4 0.4, 0.4 0.4 0.4, -0.4 0.4 0.4, -0.4 -0.4 -0.4, 0.4 -0.4 -0.4, 0.4 0.4 -0.4, -0.4 0.4 -0.4 ] }}
    coordIndex [ 0 1 2 3 -1 5 4 7 6 -1 1 5 6 2 -1 4 0 3 7 -1 3 2 6 7 -1 4 5 1 0 -1 ]
    texCoord TextureCoordinateGenerator {{ mode "{mode}" {param} }} }} }} }} }}"""
gen_head = HEAD + "Viewpoint { position 0 0 7.5 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.2 0.2 0.25 }\nDirectionalLight { direction -0.3 -0.4 -1 intensity 0.9 ambientIntensity 0.3 }\n"
write("texgen-modes.wrl", gen_head + "\n".join(gen_cell(k, m) for k, m in enumerate(MODES)))
write("texgen-params.wrl", gen_head + "\n".join(gen_cell(k, m, "parameter [ 2.0 1.5 0.5 1 0.3 0.7 ]") for k, m in enumerate(MODES)))
sph = lambda k, mode: f"""Transform {{ translation {-2.4 + 1.2 * (k % 5)} {1.8 - 1.2 * (k // 5)} 0 children Shape {{
  appearance Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {GENT} }}
  geometry Sphere {{ radius 0.5 }} }} }}"""
# Environment-style generator on a primitive: Box/Sphere take texCoord? no, only IFS family -> use ElevationGrid and IndexedTriangleSet too
write("texgen-prims.wrl", gen_head + "\n".join(
    f"""Transform {{ translation {-2.4 + 1.2 * (k % 5)} {1.8 - 1.2 * (k // 5)} 0 children Shape {{
  appearance Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture {GENT} }}
  geometry ElevationGrid {{ xDimension 4 zDimension 4 xSpacing 0.3 zSpacing 0.3 height [ 0 0.2 0 0.2 0.2 0 0.2 0 0 0.2 0 0.2 0.2 0 0.2 0 ] solid FALSE
    texCoord TextureCoordinateGenerator {{ mode "{m}" }} }} }} }}""" for k, m in enumerate(MODES[:11])))

# ---- MultiTexture channels 1-3, MultiTextureCoordinate, mapping ----
TA, TB, TC, TD = (
    "PixelTexture { image 2 2 3 0xff0000 0x00ff00 0x0000ff 0xffff00 }",
    "PixelTexture { image 2 2 3 0x00ffff 0xff00ff 0x808080 0xffffff }",
    "PixelTexture { image 4 1 3 0xff8000 0x0080ff 0x80ff00 0x8000ff }",
    "PixelTexture { image 1 4 3 0x202020 0x606060 0xa0a0a0 0xe0e0e0 }")
GRID = "coord Coordinate { point [ -0.9 -0.9 0, 0.9 -0.9 0, 0.9 0.9 0, -0.9 0.9 0 ] } coordIndex [ 0 1 2 3 -1 ] solid FALSE"
def mt_shape(x, y, modes, texes, coords, mat=MAT, extra=""):
    ms = " ".join(f'"{m}"' for m in modes)
    return f"""Transform {{ translation {x} {y} 0 children Shape {{
  appearance Appearance {{ material {mat} texture MultiTexture {{ mode [ {ms} ] texture [ {' '.join(texes)} ] {extra} }} }}
  geometry IndexedFaceSet {{ {GRID} texCoord MultiTextureCoordinate {{ texCoord [ {' '.join(coords)} ] }} }} }} }}"""
C = lambda s: f"TextureCoordinate {{ point [ {s} ] }}"
mt = HEAD + "Viewpoint { position 0 0 8 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.2 0.2 0.25 }\n" + LIGHT
mt += mt_shape(-3, 1.8, ["REPLACE", "MODULATE", "ADD", "MODULATE"], [TA, TB, TC, TD],
               [C("0 0 1 0 1 1 0 1"), C("1 1 0 1 0 0 1 0"), C("0 0 2 0 2 1 0 1"), C("0 0 0 1 1 1 1 0")])
mt += mt_shape(0, 1.8, ["REPLACE", "ADD", "SUBTRACT", "MODULATE"], [TA, TB, TC, TD],
               [C("0 0 1 0 1 1 0 1"), 'TextureCoordinateGenerator { mode "COORD" }', 'TextureCoordinateGenerator { mode "CAMERASPACENORMAL" }', 'TextureCoordinateGenerator { mode "SPHERE-REFLECT" }'])
mt += mt_shape(3, 1.8, ["REPLACE", "MODULATE", "MODULATE", "MODULATE"], [TD, TC, TB, TA],
               [C("0 0 1 0 1 1 0 1")] * 4)
# fewer coordinate channels than textures: channel reuse / fallback
mt += mt_shape(-3, -1.8, ["REPLACE", "MODULATE", "MODULATE", "MODULATE"], [TA, TB, TC, TD], [C("0 0 1 0 1 1 0 1"), C("0 0 3 0 3 3 0 3")])
# single TextureCoordinate for a MultiTexture
mt += f"""Transform {{ translation 0 -1.8 0 children Shape {{ appearance Appearance {{ material {MAT} texture MultiTexture {{ mode [ "REPLACE" "MODULATE" ] texture [ {TA} {TB} ] }} }}
  geometry IndexedFaceSet {{ {GRID} texCoord TextureCoordinate {{ point [ 0 0 1 0 1 1 0 1 ] }} }} }} }}"""
# TextureCoordinate3D/4D channels
mt += f"""Transform {{ translation 3 -1.8 0 children Shape {{ appearance Appearance {{ material {MAT} texture MultiTexture {{ mode [ "REPLACE" "MODULATE" ] texture [ {TA} {TB} ] }} }}
  geometry IndexedFaceSet {{ {GRID} texCoord MultiTextureCoordinate {{ texCoord [ TextureCoordinate4D {{ point [ 0 0 0 1, 1 0 0 1, 1 1 0 2, 0 1 0 2 ] }} TextureCoordinate3D {{ point [ 0 1 0, 1 1 0, 1 0 0, 0 0 0 ] }} ] }} }} }} }}"""
write("multitex-channels.wrl", mt)

# mapping: named channels for material textures
mp = HEAD + "Viewpoint { position 0 0 8 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.2 0.2 0.25 }\n" + LIGHT
def mapped(x, y, mat, coords, extra=""):
    return f"""Transform {{ translation {x} {y} 0 children Shape {{ appearance Appearance {{ material {mat} {extra} }}
  geometry IndexedFaceSet {{ {GRID} texCoord MultiTextureCoordinate {{ texCoord [ {' '.join(coords)} ] }} }} }} }}"""
Cm = lambda name, s: f'TextureCoordinate {{ mapping "{name}" point [ {s} ] }}'
mp += mapped(-3, 1.8, f'PhysicalMaterial {{ baseColor 1 1 1 baseTexture {TA} baseTextureMapping "B" emissiveTexture {TB} emissiveTextureMapping "A" emissiveColor 1 1 1 }}',
             [Cm("A", "0 0 1 0 1 1 0 1"), Cm("B", "1 0 0 0 0 1 1 1")])
mp += mapped(0, 1.8, f'PhysicalMaterial {{ baseColor 1 1 1 baseTexture {TA} baseTextureMapping "Z" }}', [Cm("A", "0 0 1 0 1 1 0 1")])
mp += mapped(3, 1.8, f'Material {{ diffuseColor 1 1 1 diffuseTexture {TC} diffuseTextureMapping "B" emissiveColor 1 1 1 emissiveTexture {TD} emissiveTextureMapping "A" }}',
             [Cm("A", "0 0 1 0 1 1 0 1"), Cm("B", "0 0 2 0 2 2 0 2")])
mp += mapped(-3, -1.8, f'PhysicalMaterial {{ baseColor 1 1 1 baseTexture {TA} baseTextureMapping "B" }}',
             ['TextureCoordinateGenerator { mapping "A" mode "COORD" }', 'TextureCoordinateGenerator { mapping "B" mode "CAMERASPACENORMAL" }'])
mp += mapped(0, -1.8, f'PhysicalMaterial {{ baseColor 1 1 1 baseTexture {TA} baseTextureMapping "B" }}',
             [Cm("B", "0 0 1 0 1 1 0 1"), Cm("B", "0 0 2 0 2 2 0 2")])
mp += mapped(3, -1.8, f'PhysicalMaterial {{ baseColor 1 1 1 baseTexture {TA} baseTextureMapping "B" }}',
             [Cm("A", "0 0 1 0 1 1 0 1"), Cm("B", "0 0 2 0 2 2 0 2")],
             f'textureTransform MultiTextureTransform {{ textureTransform [ TextureTransform {{ mapping "A" scale 3 3 }} TextureTransform {{ mapping "B" translation 0.5 0 rotation 0.5 }} ] }}')
write("texcoord-mapping.wrl", mp)

# ---- vertex attributes into ComposedShader ----
VS_HEAD = "attribute vec4 x3d_Vertex;\nuniform mat4 x3d_ProjectionMatrix;\nuniform mat4 x3d_ModelViewMatrix;\n"
def shader_world(decls, body, varyings, fragment, attribs, extra_geom=""):
    return HEAD + "Viewpoint { position 0 0 4 }\nNavigationInfo { headlight FALSE }\n" + f"""Shape {{ appearance Appearance {{ material Material {{ diffuseColor 1 0 0 }} shaders [ ComposedShader {{ language "GLSL" parts [
 ShaderPart {{ type "VERTEX" url "data:x-shader/x-vertex,
{VS_HEAD}{decls}
{varyings}
void main()
{{
{body}
   gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;
}}
" }} ShaderPart {{ type "FRAGMENT" url "data:x-shader/x-fragment,
precision mediump float;
{varyings}
void main() {{ {fragment} }}
" }} ] }} ] }} geometry IndexedFaceSet {{ coord Coordinate {{ point [ -1 -1 0 1 -1 0 1 1 0 -1 1 0 ] }} coordIndex [ 0 1 2 3 -1 ] {attribs} {extra_geom} }} }}
"""
write("attrib-matrix4.wrl", shader_world("attribute mat4 m;", "   vec4 c4 = m * vec4(1.0, 0.5, 0.25, 1.0);\n   c = c4.xyz;", "varying vec3 c;", "gl_FragColor = vec4 (abs(c) * 0.25, 1.0);",
    "attrib [ Matrix4VertexAttribute { name \"m\" value [ 1 0 0 0, 0 1 0 0, 0 0 1 0, 0 0 0 1,  0 1 0 0, 1 0 0 0, 0 0 1 0, 0 0 0 1,  2 0 0 0, 0 2 0 0, 0 0 2 0, 0 0 0 1,  1 0 0 0.5, 0 0 1 0.5, 0 1 0 0.5, 0 0 0 1 ] } ]"))
write("attrib-float2-float4.wrl", shader_world("attribute vec2 uv2;\nattribute vec4 col4;", "   c = vec3(uv2, 0.0) + col4.rgb * col4.a;", "varying vec3 c;", "gl_FragColor = vec4 (c * 0.5, 1.0);",
    "attrib [ FloatVertexAttribute { name \"uv2\" numComponents 2 value [ 0 0 1 0 1 1 0 1 ] } FloatVertexAttribute { name \"col4\" numComponents 4 value [ 1 0 0 1, 0 1 0 0.5, 0 0 1 0.25, 1 1 1 0.75 ] } ]"))
write("attrib-short-array.wrl", shader_world("attribute float w;", "   c = vec3(w, w * 0.5, 1.0 - w);", "varying vec3 c;", "gl_FragColor = vec4 (c, 1.0);",
    "attrib [ FloatVertexAttribute { name \"w\" numComponents 1 value [ 0.2 0.9 ] } ]"))
write("attrib-dup-name.wrl", shader_world("attribute float w;", "   c = vec3(w);", "varying vec3 c;", "gl_FragColor = vec4 (c, 1.0);",
    "attrib [ FloatVertexAttribute { name \"w\" numComponents 1 value [ 0.2 0.4 0.6 0.8 ] } FloatVertexAttribute { name \"w\" numComponents 1 value [ 0.8 0.6 0.4 0.2 ] } ]"))
write("attrib-tangent-fogdepth.wrl", HEAD + "Viewpoint { position 0 0 4 }\nNavigationInfo { headlight FALSE }\nFog { color 1 0 0 visibilityRange 5 }\n" + f"""Shape {{ appearance Appearance {{ material Material {{ diffuseColor 1 0 0 }} shaders [ ComposedShader {{ language "GLSL" parts [
 ShaderPart {{ type "VERTEX" url "data:x-shader/x-vertex,
attribute vec4 x3d_Vertex;
attribute float x3d_FogDepth;
attribute vec4 x3d_Tangent;
uniform mat4 x3d_ProjectionMatrix;
uniform mat4 x3d_ModelViewMatrix;
varying vec3 c;
void main()
{{
   c = vec3(x3d_FogDepth, x3d_Tangent.xy * 0.5 + 0.5);
   gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * x3d_Vertex;
}}
" }} ShaderPart {{ type "FRAGMENT" url "data:x-shader/x-fragment,
precision mediump float;
varying vec3 c;
void main() {{ gl_FragColor = vec4 (c, 1.0); }}
" }} ] }} ] }} geometry IndexedFaceSet {{ coord Coordinate {{ point [ -1 -1 0 1 -1 0 1 1 0 -1 1 0 ] }} coordIndex [ 0 1 2 3 -1 ]
 fogCoord FogCoordinate {{ depth [ 0.1 0.5 0.9 0.3 ] }} tangent Tangent {{ vector [ 1 0 0 1, 0 1 0 1, -1 0 0 1, 0 -1 0 1 ] }} }} }}
""")

# ---- Material (Phong) textures: ambient, diffuse, specular, emissive, shininess, occlusion, normal ----
def phong_grid(version):
    cells = []
    base = "diffuseColor 0.8 0.7 0.6 specularColor 0.8 0.8 0.8 shininess 0.4 ambientIntensity 0.6"
    ST = "PixelTexture { image 2 2 4 0xff8040ff 0x40ff80c0 0x8040ffa0 0xffffff60 }"
    G4 = "PixelTexture { image 4 4 1 " + " ".join("0x%02x" % (40 + 14 * i) for i in range(16)) + " }"
    A4 = "PixelTexture { image 4 4 2 " + " ".join("0x%02x%02x" % (255 - 12 * i, 40 + 14 * i) for i in range(16)) + " }"
    tex = {
        "diffuse": f"diffuseTexture {T1}",
        "diffuse-alpha": f"diffuseTexture {ST}",
        "ambient": f"ambientTexture {T2}",
        "specular": f"specularTexture {T2}",
        "emissive": f"emissiveTexture {T1} emissiveColor 0.5 0.5 0.5",
        "shininess": f"shininessTexture {A4}",
        "occlusion": f"occlusionTexture {G4} occlusionStrength 0.8",
        "normal": f"normalTexture {NM}",
        "normal-scale": f"normalTexture {NM} normalScale 3",
        "all": f"diffuseTexture {T1} ambientTexture {T2} specularTexture {T2} emissiveTexture {T2} emissiveColor 0.3 0.3 0.3 shininessTexture {A4} occlusionTexture {G4} normalTexture {NM}",
    }
    for k, (name, fields) in enumerate(tex.items()):
        col, row = k % 5, k // 5
        cells.append(f"""Transform {{ translation {-2.4 + 1.2 * col} {0.9 - 1.8 * row} 0 children Shape {{
  appearance Appearance {{ material Material {{ {base} {fields} }} }}
  geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -0.5 -0.5 0, 0.5 -0.5 0, 0.5 0.5 0, -0.5 0.5 0 ] }} coordIndex [ 0 1 2 3 -1 ]
    normal Normal {{ vector [ 0 0 1, 0 0 1, 0 0 1, 0 0 1 ] }} texCoord TextureCoordinate {{ point [ 0 0, 1 0, 1 1, 0 1 ] }} }} }} }}""")
    return f"#X3D V{version} utf8\nPROFILE Full\n" + "Viewpoint { position 0 0 6 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.2 0.2 0.25 }\n" + \
        "DirectionalLight { direction 0.4 -0.3 -1 intensity 1 ambientIntensity 0.5 }\nPointLight { location 1 1 2 intensity 0.8 ambientIntensity 0.2 radius 30 }\n" + "\n".join(cells) + "\n"
write("phong-textures.wrl", phong_grid("4.0"))
write("phong-textures-v33.wrl", phong_grid("3.3"))
# Phong textures on a lit sphere / box (generated normals and tangents) and with a MultiTextureCoordinate mapping
write("phong-textures-prims.wrl", HEAD + "Viewpoint { position 0 0 7 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.2 0.2 0.25 }\n" + lights +
      f"Transform {{ translation -2 0 0 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.9 0.9 0.9 specularColor 0.6 0.6 0.6 shininess 0.5 diffuseTexture {T1} normalTexture {NM} }} }} geometry Sphere {{ radius 1.2 }} }} }}\n" +
      f"Transform {{ translation 2 0 0 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 0.9 0.9 0.9 specularColor 0.6 0.6 0.6 shininess 0.5 emissiveTexture {T2} emissiveColor 0.4 0.4 0.4 normalTexture {NM} }} }} geometry Box {{ size 1.8 1.8 1.8 }} }} }}\n")
write("phong-textures-mapping.wrl", HEAD + "Viewpoint { position 0 0 6 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.2 0.2 0.25 }\n" + lights +
      mapped(-1.5, 0, f'Material {{ diffuseColor 1 1 1 diffuseTexture {TA} diffuseTextureMapping "B" ambientTexture {TB} ambientTextureMapping "A" ambientIntensity 1 }}',
             [Cm("A", "0 0 1 0 1 1 0 1"), Cm("B", "1 1 0 1 0 0 1 0")]) +
      mapped(1.5, 0, f'Material {{ diffuseColor 1 1 1 diffuseTexture {TC} diffuseTextureMapping "A" }}', ['TextureCoordinateGenerator { mapping "A" mode "CAMERASPACENORMAL" }']))
# UnlitMaterial emissive texture and FogCoordinate / LocalFog together
write("unlit-emissive-texture.wrl", HEAD + "Viewpoint { position 0 0 6 }\nNavigationInfo { headlight FALSE }\nBackground { skyColor 0.2 0.2 0.25 }\n" +
      quad(-1.5, 0, f"Appearance {{ material UnlitMaterial {{ emissiveTexture {T1} }} }}", 0.9) + quad(1.5, 0, f"Appearance {{ material UnlitMaterial {{ emissiveColor 0.5 1 0.5 emissiveTexture {T2} transparency 0.3 }} }}", 0.9))
