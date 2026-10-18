#!/usr/bin/env python3
"""REA batch 03 PhysicalMaterial extension worlds (ext-*.wrl) for oracle/synthetic-rea-b03.

  python3 -I oracle/tools/gen_rea_b03_materials.py oracle/synthetic-rea-b03

X_ITE 16.5.0's X3DMaterialExtensionNodes (component X_ITE, experimental): EmissiveStrength, IOR,
Specular, Clearcoat, Sheen, Iridescence, Anisotropy, Transmission, Volume, DiffuseTransmission;
field clamps, textures with mappings, punctual lights and an EnvironmentLight (IBL), the
transmission pass, combinations, back materials, shadows, texture projection, layers, a
RenderedTexture as an extension texture, PickableGroup; a stepped runtime case (field, texture
and extension list changes). Textures come from synthetic-rea-b02/pbr and synthetic-rea-b03.
"""
import json, os, sys

HEAD = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Shape:4
COMPONENT Lighting:3
COMPONENT CubeMapTexturing:3
COMPONENT Texturing:3
COMPONENT TextureProjection:2
COMPONENT Layering:1
COMPONENT Picking:3
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
COMPONENT X_ITE:1
"""
ENV = 'EnvironmentLight { intensity 1 specularTexture ImageCubeMapTexture { url "../synthetic-rea-b02/pbr/env.png" } }'
LIGHTS = ("DirectionalLight { direction -0.4 -0.6 -0.7 intensity 0.8 color 1 0.95 0.9 }\n"
          "PointLight { location 2.5 2 3 intensity 2 color 0.6 0.8 1 radius 20 attenuation 0 0 1 }")
BG = "Background { skyColor 0.15 0.15 0.18 }"
PBR = "../synthetic-rea-b02/pbr/"

def tex(name, extra=""):
    return f'ImageTexture {{ url "{name}" {extra}}}'

def grid(materials, cols=4, spacing=1.05, y0=1.6, r=0.45, geometry=None, z=0):
    out = []
    for i, m in enumerate(materials):
        x = (i % cols - (cols - 1) / 2) * spacing
        y = y0 - (i // cols) * spacing
        g = geometry or f"Sphere {{ radius {r} }}"
        out.append(f"Transform {{ translation {x:.3f} {y:.3f} {z} children Shape {{ appearance Appearance {{ material {m} }} geometry {g} }} }}")
    return "\n".join(out)

def pm(ext, base="baseColor 0.8 0.5 0.3", extra=""):
    return f"PhysicalMaterial {{ {base} {extra} extensions [ {ext} ] }}"

def world(*parts, vp="Viewpoint { position 0 0 6 }"):
    return HEAD + vp + "\n" + "\n".join(parts) + "\n"

worlds, scripts = {}, {}

# EmissiveStrength: multiplies emissiveColor (and an emissive texture); max (value, 0).
em = [pm(f"EmissiveStrengthMaterialExtension {{ emissiveStrength {s} }}", "baseColor 0.2 0.2 0.2 emissiveColor 0.2 0.1 0.05", "metallic 0 roughness 0.5") for s in (0, 1, 3, -2)]
em += [pm(f"EmissiveStrengthMaterialExtension {{ emissiveStrength {s} }}", "baseColor 0.2 0.2 0.2 emissiveColor 0.25 0.25 0.25", f"metallic 0 roughness 0.5 emissiveTexture {tex(PBR + 'emissive.png')}") for s in (0.5, 2, 4)]
em += [pm("EmissiveStrengthMaterialExtension { }", "baseColor 0.2 0.2 0.2 emissiveColor 0.4 0.1 0.6")]
worlds["ext-emissive.wrl"] = world(BG, LIGHTS, grid(em), grid(em[:4], y0=-0.6, geometry="Box { size 0.7 0.7 0.7 }"))
worlds["ext-emissive-ibl.wrl"] = world(BG, ENV, grid(em))

# IOR: f0 from indexOfRefraction (max 1); also the clearcoat F0.
ior = [pm(f"IORMaterialExtension {{ indexOfRefraction {v} }}", "baseColor 0.3 0.4 0.8", f"metallic {m} roughness {r}") for v in (1, 1.5, 2.5, 0.5) for (m, r) in ((0, 0.2), (0, 0.6))]
ior += [pm(f"IORMaterialExtension {{ indexOfRefraction {v} }} ClearcoatMaterialExtension {{ clearcoat 1 clearcoatRoughness 0.1 }}", "baseColor 0.3 0.4 0.8", "metallic 0 roughness 0.6") for v in (1.2, 3, 5, 1)]
worlds["ext-ior.wrl"] = world(BG, LIGHTS, grid(ior, y0=1.6))
worlds["ext-ior-ibl.wrl"] = world(BG, ENV, LIGHTS, grid(ior, y0=1.6))

# Specular: specular (weight), specularColor × specularStrength, alpha / rgb textures.
sp = []
for s, c, st in ((0, "1 1 1", 1), (0.5, "1 1 1", 1), (1, "1 0.3 0.3", 1), (1, "0.3 1 0.3", 3), (1, "1 1 1", 0), (2, "1 1 1", -1), (1, "2 2 2", 1), (0.8, "0.2 0.5 1", 2)):
    sp.append(pm(f"SpecularMaterialExtension {{ specular {s} specularColor {c} specularStrength {st} }}", "baseColor 0.6 0.6 0.6", "metallic 0 roughness 0.25"))
sp.append(pm(f"SpecularMaterialExtension {{ specularTexture {tex(PBR + 'base-alpha.png')} }}", "baseColor 0.6 0.6 0.6", "metallic 0 roughness 0.25"))
sp.append(pm(f"SpecularMaterialExtension {{ specularColorTexture {tex(PBR + 'base.png')} specularStrength 4 }}", "baseColor 0.6 0.6 0.6", "metallic 0 roughness 0.25"))
sp.append(pm(f"SpecularMaterialExtension {{ specular 0.7 specularTexture {tex(PBR + 'base-alpha.png')} specularColorTexture {tex('proj-checker.png')} specularColor 1 0.8 0.5 specularStrength 2 }}", "baseColor 0.6 0.6 0.6", "metallic 0 roughness 0.25"))
sp.append(pm("SpecularMaterialExtension { specular 1 } IORMaterialExtension { indexOfRefraction 2 }", "baseColor 0.6 0.6 0.6", "metallic 0.3 roughness 0.25"))
worlds["ext-specular.wrl"] = world(BG, LIGHTS, grid(sp, y0=1.6))
worlds["ext-specular-ibl.wrl"] = world(BG, ENV, LIGHTS, grid(sp, y0=1.6))

# Clearcoat: factor, roughness (clamped), textures (r, g), clearcoat normal texture.
cc = []
for f, r in ((0, 0), (0.5, 0), (1, 0), (1, 0.3), (1, 0.7), (2, -1), (0.8, 2), (1, 0.15)):
    cc.append(pm(f"ClearcoatMaterialExtension {{ clearcoat {f} clearcoatRoughness {r} }}", "baseColor 0.7 0.1 0.1", "metallic 0 roughness 0.8"))
cc.append(pm(f"ClearcoatMaterialExtension {{ clearcoat 1 clearcoatTexture {tex(PBR + 'occlusion.png')} }}", "baseColor 0.1 0.5 0.1", "metallic 0 roughness 0.8"))
cc.append(pm(f"ClearcoatMaterialExtension {{ clearcoat 1 clearcoatRoughness 1 clearcoatRoughnessTexture {tex(PBR + 'mr.png')} }}", "baseColor 0.1 0.5 0.1", "metallic 0 roughness 0.8"))
cc.append(pm(f"ClearcoatMaterialExtension {{ clearcoat 1 clearcoatNormalTexture {tex(PBR + 'normal.png')} }}", "baseColor 0.1 0.1 0.6", "metallic 0.5 roughness 0.5"))
cc.append(pm(f"ClearcoatMaterialExtension {{ clearcoat 1 clearcoatRoughness 0.2 clearcoatNormalTexture {tex(PBR + 'normal.png')} }}", "baseColor 0.1 0.1 0.6", f"metallic 0.5 roughness 0.5 normalTexture {tex(PBR + 'normal.png')} emissiveColor 0.1 0 0"))
worlds["ext-clearcoat.wrl"] = world(BG, LIGHTS, grid(cc, y0=1.6))
worlds["ext-clearcoat-ibl.wrl"] = world(BG, ENV, LIGHTS, grid(cc, y0=1.6))

# Sheen: sheenColor, sheenRoughness (clamped), textures (rgb sRGB, a); Charlie BRDF, LUTs.
sh = []
for c, r in (("0 0 0", 0), ("1 1 1", 0.3), ("1 0.2 0.2", 0.5), ("0.2 0.2 1", 1), ("1 1 1", 0), ("0.5 1 0.5", 2), ("1 0.8 0.2", 0.8), ("1 1 1", 0.05)):
    sh.append(pm(f"SheenMaterialExtension {{ sheenColor {c} sheenRoughness {r} }}", "baseColor 0.2 0.2 0.3", "metallic 0 roughness 0.7"))
sh.append(pm(f"SheenMaterialExtension {{ sheenColor 1 1 1 sheenRoughness 0.5 sheenColorTexture {tex(PBR + 'base.png')} }}", "baseColor 0.2 0.2 0.3", "metallic 0 roughness 0.7"))
sh.append(pm(f"SheenMaterialExtension {{ sheenColor 1 1 1 sheenRoughness 1 sheenRoughnessTexture {tex(PBR + 'base-alpha.png')} }}", "baseColor 0.2 0.2 0.3", "metallic 0 roughness 0.7"))
sh.append(pm("SheenMaterialExtension { sheenColor 1 0.5 0.5 sheenRoughness 0.4 } ClearcoatMaterialExtension { clearcoat 0.5 }", "baseColor 0.2 0.2 0.3", "metallic 0.2 roughness 0.7"))
sh.append(pm("SheenMaterialExtension { sheenColor 0.6 0.6 1 sheenRoughness 0.6 }", "baseColor 0.6 0.4 0.2", "metallic 1 roughness 0.4"))
worlds["ext-sheen.wrl"] = world(BG, LIGHTS, grid(sh, y0=1.6))
worlds["ext-sheen-ibl.wrl"] = world(BG, ENV, LIGHTS, grid(sh, y0=1.6))

# Iridescence: factor, ior (max 1), thickness min / max (max 0), textures (r, g); thin film.
ir = []
for f, i, mn, mx in ((1, 1.3, 100, 400), (1, 1.8, 100, 400), (1, 1.3, 100, 800), (0.5, 1.3, 100, 400), (1, 1.3, 100, 0), (1, 0.5, 100, 250), (1, 2.2, 0, 1200), (2, 1.3, -5, 400)):
    ir.append(pm(f"IridescenceMaterialExtension {{ iridescence {f} iridescenceIndexOfRefraction {i} iridescenceThicknessMinimum {mn} iridescenceThicknessMaximum {mx} }}", "baseColor 0.1 0.1 0.1", "metallic 0.2 roughness 0.2"))
ir.append(pm(f"IridescenceMaterialExtension {{ iridescence 1 iridescenceTexture {tex(PBR + 'occlusion.png')} }}", "baseColor 0.1 0.1 0.1", "metallic 0 roughness 0.2"))
ir.append(pm(f"IridescenceMaterialExtension {{ iridescence 1 iridescenceThicknessMinimum 200 iridescenceThicknessMaximum 900 iridescenceThicknessTexture {tex(PBR + 'mr.png')} }}", "baseColor 0.1 0.1 0.1", "metallic 0 roughness 0.2"))
ir.append(pm("IridescenceMaterialExtension { iridescence 1 } SpecularMaterialExtension { specularColor 1 0.5 0.2 }", "baseColor 0.8 0.8 0.8", "metallic 1 roughness 0.3"))
ir.append(pm("IridescenceMaterialExtension { iridescence 0.8 iridescenceThicknessMaximum 500 } IORMaterialExtension { indexOfRefraction 1.8 }", "baseColor 0.3 0.3 0.3", "metallic 0.5 roughness 0.1"))
worlds["ext-iridescence.wrl"] = world(BG, LIGHTS, grid(ir, y0=1.6))
worlds["ext-iridescence-ibl.wrl"] = world(BG, ENV, LIGHTS, grid(ir, y0=1.6))

# Anisotropy: strength (clamped), rotation, texture (rg direction, b strength); bent normal IBL.
an = []
for s, rot in ((0, 0), (0.5, 0), (1, 0), (1, 0.785), (1, 1.571), (0.8, 3), (2, -0.5), (0.3, 6)):
    an.append(pm(f"AnisotropyMaterialExtension {{ anisotropyStrength {s} anisotropyRotation {rot} }}", "baseColor 0.8 0.8 0.85", "metallic 1 roughness 0.35"))
an.append(pm(f"AnisotropyMaterialExtension {{ anisotropyStrength 1 anisotropyTexture {tex(PBR + 'normal.png')} }}", "baseColor 0.8 0.8 0.85", "metallic 1 roughness 0.35"))
an.append(pm(f"AnisotropyMaterialExtension {{ anisotropyStrength 0.7 anisotropyRotation 1 anisotropyTexture {tex(PBR + 'mr.png')} }}", "baseColor 0.8 0.6 0.4", "metallic 0.5 roughness 0.5"))
an.append(pm("AnisotropyMaterialExtension { anisotropyStrength 0.9 } ClearcoatMaterialExtension { clearcoat 0.6 }", "baseColor 0.2 0.3 0.8", "metallic 0 roughness 0.4"))
an.append(pm(f"AnisotropyMaterialExtension {{ anisotropyStrength 0.9 anisotropyRotation 0.5 }}", "baseColor 0.8 0.8 0.85", f"metallic 1 roughness 0.35 normalTexture {tex(PBR + 'normal.png')}"))
worlds["ext-anisotropy.wrl"] = world(BG, LIGHTS, grid(an, y0=1.6), grid(an[2:6], y0=-1.6, geometry="Cylinder { radius 0.35 height 0.8 }"))
worlds["ext-anisotropy-ibl.wrl"] = world(BG, ENV, LIGHTS, grid(an, y0=1.6))

# Transmission: the transmission pass (opaque and blended shapes, background, not the
# transmission shapes), mip level from roughness and ior; texture (r); without IBL only the
# punctual transmission.
BEHIND = "\n".join([
    "Transform { translation 0 0 -3 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture ImageTexture { url \"proj-checker.png\" } } geometry Box { size 8 6 0.1 } } }",
    "Transform { translation -1.5 0.8 -1.5 children Shape { appearance Appearance { material Material { diffuseColor 1 0.2 0.2 } } geometry Box { size 0.6 2.5 0.3 } } }",
    "Transform { translation 1.5 -0.8 -1.5 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.4 1 transparency 0.5 } } geometry Cone { bottomRadius 0.5 height 1.5 } } }",
])
tr = []
for t, r, i in ((0, 0, 1.5), (1, 0, 1.5), (1, 0.3, 1.5), (1, 0.7, 1.5), (0.5, 0.1, 1.5), (1, 0.2, 1.1), (1, 0.2, 2.4), (2, 0.1, 1.5)):
    ext = f"TransmissionMaterialExtension {{ transmission {t} }}" + ("" if i == 1.5 else f" IORMaterialExtension {{ indexOfRefraction {i} }}")
    tr.append(pm(ext, "baseColor 1 1 1", f"metallic 0 roughness {r}"))
tr.append(pm(f"TransmissionMaterialExtension {{ transmission 1 transmissionTexture {tex(PBR + 'occlusion.png')} }}", "baseColor 0.9 1 0.9", "metallic 0 roughness 0.1"))
tr.append(pm("TransmissionMaterialExtension { transmission 1 }", "baseColor 1 0.6 0.6", "metallic 0 roughness 0.05 transparency 0.3"))
tr.append(pm("TransmissionMaterialExtension { transmission 1 } SpecularMaterialExtension { specular 0.5 }", "baseColor 0.6 0.8 1", "metallic 0 roughness 0"))
tr.append(pm("TransmissionMaterialExtension { transmission 0.8 }", "baseColor 1 1 1", "metallic 0.5 roughness 0.2"))
worlds["ext-transmission.wrl"] = world(BG, LIGHTS, BEHIND, grid(tr, y0=1.6))
worlds["ext-transmission-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, grid(tr, y0=1.6))

# Volume: thickness (max 0), attenuationDistance (0 → 1e6, negative → 0), attenuationColor,
# thickness texture (g); with transmission (refraction ray, Beer's law) and scaled models.
vo = []
for th, d, c in ((0, 1e6, "1 1 1"), (0.5, 1e6, "1 1 1"), (1, 0.5, "1 0.3 0.2"), (2, 1, "0.2 0.6 1"), (1, 0, "0.2 1 0.2"), (1, -1, "1 0.2 1"), (-1, 1, "1 1 0.2"), (3, 2, "0.6 0.6 0.6")):
    vo.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} VolumeMaterialExtension {{ thickness {th} attenuationDistance {d} attenuationColor {c} }}", "baseColor 1 1 1", "metallic 0 roughness 0.05"))
vo.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} VolumeMaterialExtension {{ thickness 2 attenuationDistance 0.5 attenuationColor 0.8 0.4 0.1 thicknessTexture {tex(PBR + 'mr.png')} }}", "baseColor 1 1 1", "metallic 0 roughness 0.05"))
vo.append(pm("TransmissionMaterialExtension { transmission 1 } VolumeMaterialExtension { thickness 1 attenuationDistance 0.7 attenuationColor 0.3 0.9 0.5 } IORMaterialExtension { indexOfRefraction 1.9 }", "baseColor 1 1 1", "metallic 0 roughness 0.2"))
vo.append(pm("VolumeMaterialExtension { thickness 1 attenuationDistance 0.5 attenuationColor 1 0 0 }", "baseColor 0.8 0.8 0.8", "metallic 0 roughness 0.3"))
vo.append(pm("TransmissionMaterialExtension { transmission 0.7 } VolumeMaterialExtension { thickness 1.5 attenuationDistance 1 attenuationColor 0.5 0.5 1 }", "baseColor 1 0.9 0.8", "metallic 0 roughness 0.4"))
scaled = "Transform { translation 0 -2.05 0 scale 2 0.6 1 children Shape { appearance Appearance { material " + vo[3] + " } geometry Sphere { radius 0.3 } } }"
worlds["ext-volume.wrl"] = world(BG, LIGHTS, BEHIND, grid(vo, y0=1.6), scaled)
worlds["ext-volume-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, grid(vo, y0=1.6), scaled)

# DiffuseTransmission: factor, color, textures (a, rgb sRGB); back lighting (a light behind the
# shapes), the IBL of -n, with Volume (thickness from the model view scale).
BACKLIGHT = "PointLight { location 0 0 -2.5 intensity 3 color 1 0.9 0.6 radius 20 attenuation 0 0 1 }"
dt = []
for f, c in ((0, "1 1 1"), (0.5, "1 1 1"), (1, "1 1 1"), (1, "1 0.3 0.3"), (2, "0.3 1 0.3"), (0.8, "0.3 0.3 1"), (1, "2 2 2"), (0.3, "1 0.8 0.2")):
    dt.append(pm(f"DiffuseTransmissionMaterialExtension {{ diffuseTransmission {f} diffuseTransmissionColor {c} }}", "baseColor 0.8 0.8 0.8", "metallic 0 roughness 0.6"))
dt.append(pm(f"DiffuseTransmissionMaterialExtension {{ diffuseTransmission 1 diffuseTransmissionTexture {tex(PBR + 'base-alpha.png')} }}", "baseColor 0.8 0.8 0.8", "metallic 0 roughness 0.6"))
dt.append(pm(f"DiffuseTransmissionMaterialExtension {{ diffuseTransmission 1 diffuseTransmissionColorTexture {tex(PBR + 'base.png')} }}", "baseColor 0.8 0.8 0.8", "metallic 0 roughness 0.6"))
dt.append(pm("DiffuseTransmissionMaterialExtension { diffuseTransmission 1 diffuseTransmissionColor 1 0.5 0.2 } VolumeMaterialExtension { thickness 1 attenuationDistance 0.5 attenuationColor 1 0.2 0.1 }", "baseColor 0.8 0.8 0.8", "metallic 0 roughness 0.6"))
dt.append(pm("DiffuseTransmissionMaterialExtension { diffuseTransmission 0.7 } TransmissionMaterialExtension { transmission 0.5 }", "baseColor 0.9 0.9 0.9", "metallic 0 roughness 0.3"))
leaf = ("Transform { translation 0 -2.1 0 rotation 1 0 0 -0.3 scale 1.5 1 1 children Shape { appearance Appearance { material "
        + dt[11] + " } geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -1 -0.4 0, 1 -0.4 0, 1 0.4 0, -1 0.4 0 ] } coordIndex [ 0 1 2 3 -1 ] } } }")
worlds["ext-diffuse-transmission.wrl"] = world(BG, LIGHTS, BACKLIGHT, grid(dt, y0=1.6), leaf)
worlds["ext-diffuse-transmission-ibl.wrl"] = world(BG, ENV, BACKLIGHT, grid(dt, y0=1.6), leaf)

# Combinations, the extension list (duplicates: the later node wins; non extension nodes are
# ignored), back materials, texture mappings and coordinates, points and lines (no Clearcoat or
# Anisotropy there: X_ITE's shader reads normalInfo, which it declares only with lighting or IBL,
# and fails to compile).
co = [
    pm("EmissiveStrengthMaterialExtension { emissiveStrength 5 } EmissiveStrengthMaterialExtension { emissiveStrength 0.5 }", "baseColor 0.3 0.3 0.3 emissiveColor 0.4 0.2 0", "metallic 0 roughness 0.5"),
    pm("ClearcoatMaterialExtension { clearcoat 1 } SheenMaterialExtension { sheenColor 1 1 1 sheenRoughness 0.5 } IridescenceMaterialExtension { iridescence 0.6 } SpecularMaterialExtension { specular 0.8 specularColor 0.5 1 0.5 } IORMaterialExtension { indexOfRefraction 1.7 } EmissiveStrengthMaterialExtension { emissiveStrength 2 }", "baseColor 0.4 0.2 0.6 emissiveColor 0.05 0.05 0.05", "metallic 0.2 roughness 0.4"),
    pm("AnisotropyMaterialExtension { anisotropyStrength 0.8 anisotropyRotation 0.4 } IridescenceMaterialExtension { iridescence 1 iridescenceThicknessMaximum 300 }", "baseColor 0.9 0.9 0.9", "metallic 1 roughness 0.3"),
    pm("TransmissionMaterialExtension { transmission 1 } VolumeMaterialExtension { thickness 0.8 attenuationColor 0.9 0.5 0.5 attenuationDistance 1 } ClearcoatMaterialExtension { clearcoat 1 } DiffuseTransmissionMaterialExtension { diffuseTransmission 0.4 }", "baseColor 1 1 1", "metallic 0 roughness 0.1"),
    pm("Material { } MetadataString { value \"x\" } SheenMaterialExtension { sheenColor 1 0.6 0.2 sheenRoughness 0.3 }", "baseColor 0.3 0.5 0.3", "metallic 0 roughness 0.6"),
    pm("SpecularMaterialExtension { specular 0.2 } SpecularMaterialExtension { specular 1 specularColor 1 0.2 0.2 }", "baseColor 0.5 0.5 0.5", "metallic 0 roughness 0.2"),
    pm(f"ClearcoatMaterialExtension {{ clearcoat 1 clearcoatTexture {tex(PBR + 'occlusion.png')} clearcoatTextureMapping \"second\" }} SheenMaterialExtension {{ sheenColor 1 1 1 sheenColorTexture {tex('proj-checker.png')} sheenColorTextureMapping \"second\" }}", "baseColor 0.5 0.3 0.3", "metallic 0 roughness 0.6"),
    pm(f"SpecularMaterialExtension {{ specularColorTexture {tex(PBR + 'base.png')} specularColorTextureMapping \"none\" }} IridescenceMaterialExtension {{ iridescence 1 iridescenceTexture {tex(PBR + 'occlusion.png')} }}", "baseColor 0.5 0.5 0.5", f"metallic 0 roughness 0.3 baseTexture {tex(PBR + 'base.png')} metallicRoughnessTexture {tex(PBR + 'mr.png')} occlusionTexture {tex(PBR + 'occlusion.png')}"),
]
mapped = []
for i, m in enumerate(co):
    x = (i % 4 - 1.5) * 1.05
    y = 1.6 - (i // 4) * 1.05
    geom = ("IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.45 -0.45 0, 0.45 -0.45 0, 0.45 0.45 0, -0.45 0.45 0 ] } coordIndex [ 0 1 2 3 -1 ] "
            "texCoord MultiTextureCoordinate { texCoord [ TextureCoordinate { mapping \"first\" point [ 0 0, 1 0, 1 1, 0 1 ] } TextureCoordinate { mapping \"second\" point [ 0 0, 3 0, 3 3, 0 3 ] } ] } }") if i in (6, 7) else "Sphere { radius 0.45 }"
    mapped.append(f"Transform {{ translation {x:.3f} {y:.3f} 0 children Shape {{ appearance Appearance {{ material {m} }} geometry {geom} }} }}")
back = ("Transform { translation -1.6 -0.9 0 rotation 0 1 0 2.6 children Shape { appearance Appearance { material " + pm("ClearcoatMaterialExtension { clearcoat 1 }", "baseColor 0.2 0.8 0.2", "metallic 0 roughness 0.9")
        + " backMaterial " + pm("SheenMaterialExtension { sheenColor 1 0.2 0.2 sheenRoughness 0.4 } EmissiveStrengthMaterialExtension { emissiveStrength 4 }", "baseColor 0.2 0.2 0.8 emissiveColor 0.1 0.05 0", "metallic 0 roughness 0.6")
        + " } geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.5 -0.5 0, 0.5 -0.5 0, 0.5 0.5 0, -0.5 0.5 0 ] } coordIndex [ 0 1 2 3 -1 ] } } }")
lines = ("Transform { translation 0.6 -1.0 0 children [ Shape { appearance Appearance { material " + pm("EmissiveStrengthMaterialExtension { emissiveStrength 2 } SheenMaterialExtension { sheenColor 1 1 1 }", "baseColor 0.2 0.6 0.9 emissiveColor 0.2 0.1 0") + " } "
         "geometry LineSet { vertexCount [ 3 ] coord Coordinate { point [ 0 0 0, 0.8 0.3 0, 1.6 -0.2 0 ] } } } "
         "Shape { appearance Appearance { material " + pm("SpecularMaterialExtension { specular 0.5 } EmissiveStrengthMaterialExtension { emissiveStrength 3 }", "baseColor 0.9 0.6 0.2 emissiveColor 0.1 0 0") + " } geometry PointSet { coord Coordinate { point [ 0 -0.4 0, 0.5 -0.4 0, 1 -0.4 0 ] } } } ] }")
worlds["ext-combined.wrl"] = world(BG, ENV, LIGHTS, "\n".join(mapped), back, lines)

# Cross-system: shadows, a texture projector (base color), a RenderedTexture as an extension
# texture, PickableGroup, a LayerSet with a full-size Layer and a LayoutLayer, Inline-free.
cross = "\n".join([
    BG,
    ENV,
    "DirectionalLight { direction -0.3 -1 -0.4 intensity 1 shadows TRUE }",
    "TextureProjector { location 0 0 4 direction 0 0 -1 fieldOfView 0.4 texture ImageTexture { url \"proj-checker.png\" } }",
    "Transform { translation 0 -1.8 0 children Shape { appearance Appearance { material PhysicalMaterial { baseColor 0.8 0.8 0.8 metallic 0 roughness 0.8 } } geometry Box { size 6 0.1 4 } } }",
    "DEF RT RenderedTexture { width 64 height 64 children [ Viewpoint { position 0 0 4 } Background { skyColor 0.9 0.2 0.2 } Shape { appearance Appearance { material Material { diffuseColor 0.2 0.2 1 } } geometry Box { } } ] }",
    "DEF PG PickableGroup { children [",
    grid([
        pm("ClearcoatMaterialExtension { clearcoat 1 clearcoatTexture USE RT }", "baseColor 0.6 0.3 0.1", "metallic 0 roughness 0.8"),
        pm("SheenMaterialExtension { sheenColor 1 1 1 sheenRoughness 0.5 sheenColorTexture USE RT }", "baseColor 0.2 0.2 0.2", "metallic 0 roughness 0.7"),
        pm("TransmissionMaterialExtension { transmission 1 } VolumeMaterialExtension { thickness 1 attenuationColor 0.6 0.9 0.6 attenuationDistance 1 }", "baseColor 1 1 1", "metallic 0 roughness 0.1"),
        pm("IridescenceMaterialExtension { iridescence 1 } EmissiveStrengthMaterialExtension { emissiveStrength 3 }", "baseColor 0.3 0.3 0.3 emissiveColor 0.05 0.02 0", "metallic 0.5 roughness 0.2"),
    ], y0=0.3),
    "] }",
    "LayerSet { order [ 0 1 2 ] layers [",
    "  Layer { children [ Viewpoint { position 0 0 6 } " + LIGHTS.replace("\n", " ") + " " + grid([pm("AnisotropyMaterialExtension { anisotropyStrength 1 }", "baseColor 0.8 0.8 0.8", "metallic 1 roughness 0.3"), pm("TransmissionMaterialExtension { transmission 1 }", "baseColor 1 0.8 0.8", "metallic 0 roughness 0.2")], cols=2, y0=-1.0).replace("\n", " ") + " ] }",
    "  LayoutLayer { layout Layout { size [ 0.3 0.3 ] sizeUnits [ \"FRACTION\" \"FRACTION\" ] align [ \"RIGHT\" \"TOP\" ] } children [ Shape { appearance Appearance { material " + pm("EmissiveStrengthMaterialExtension { emissiveStrength 2 } ClearcoatMaterialExtension { clearcoat 1 }", "baseColor 0.2 0.5 0.2 emissiveColor 0.2 0.2 0.4") + " } geometry Rectangle2D { size 0.3 0.3 } } ] }",
    "] }",
])
worlds["ext-cross.wrl"] = world(cross)

# Runtime: field changes, texture changes, extension list changes, transmission on and off.
rt = "\n".join([
    BG, ENV, LIGHTS, BEHIND,
    "DEF CC ClearcoatMaterialExtension { clearcoat 0 }",
    "DEF SH SheenMaterialExtension { sheenColor 0 0 0 }",
    "DEF TR TransmissionMaterialExtension { transmission 0 }",
    "DEF EM EmissiveStrengthMaterialExtension { emissiveStrength 1 }",
    "DEF M1 PhysicalMaterial { baseColor 0.7 0.2 0.2 emissiveColor 0.1 0.05 0 metallic 0 roughness 0.6 extensions [ USE CC USE EM ] }",
    "DEF M2 PhysicalMaterial { baseColor 1 1 1 metallic 0 roughness 0.1 extensions [ USE TR ] }",
    "DEF M3 PhysicalMaterial { baseColor 0.2 0.3 0.6 metallic 0 roughness 0.7 extensions [ USE SH ] }",
    "Transform { translation -1.4 0.6 0 children Shape { appearance Appearance { material USE M1 } geometry Sphere { radius 0.6 } } }",
    "Transform { translation 0 0.6 0 children Shape { appearance Appearance { material USE M2 } geometry Sphere { radius 0.6 } } }",
    "Transform { translation 1.4 0.6 0 children Shape { appearance Appearance { material USE M3 } geometry Sphere { radius 0.6 } } }",
    "DEF K KeySensor { }",
    """DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOutput SFString log ""
  initializeOnly SFNode cc USE CC
  initializeOnly SFNode sh USE SH
  initializeOnly SFNode tr USE TR
  initializeOnly SFNode em USE EM
  initializeOnly SFNode m1 USE M1
  initializeOnly SFNode m2 USE M2
  initializeOnly SFNode m3 USE M3
  url "javascript:
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') { cc .clearcoat = 1; cc .clearcoatRoughness = 0.2; }
   if (k == 'b') { sh .sheenColor = new SFColor (1, 0.8, 0.6); sh .sheenRoughness = 0.4; }
   if (k == 'c') tr .transmission = 1;
   if (k == 'd') em .emissiveStrength = 6;
   if (k == 'e') cc .clearcoatTexture = Browser .currentScene .createNode ('PixelTexture');
   if (k == 'f') { var p = cc .clearcoatTexture; p .image = new SFImage (2, 2, 1, new MFInt32 (0x20, 0xff, 0xff, 0x20)); }
   if (k == 'g') m1 .extensions = new MFNode (em);
   if (k == 'h') m3 .extensions = new MFNode (sh, tr);
   if (k == 'i') m2 .extensions = new MFNode ();
   if (k == 'j') { var v = Browser .currentScene .createNode ('VolumeMaterialExtension'); v .thickness = 1; v .attenuationDistance = 0.5; v .attenuationColor = new SFColor (1, 0.3, 0.1); m3 .extensions = new MFNode (sh, tr, v); }
}
"
}""",
    "ROUTE K.keyPress TO R.on_key",
])
worlds["ext-runtime.wrl"] = world(rt)
FOCUS = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507}, {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
keys = [(20 + 6 * i, k) for i, k in enumerate("abcdefghij")]
scripts["ext-runtime.json"] = {"settle": True, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": 90, "capture": [12] + [f + 4 for f, _ in keys], "dump": [10] + [f + 4 for f, _ in keys] + [89], "events": FOCUS + [{"frame": f, "type": "key", "key": k} for f, k in keys]}

out = sys.argv[1]
os.makedirs(out, exist_ok=True)
for name, text in worlds.items():
    with open(os.path.join(out, name), "w") as f:
        f.write(text)
for name, script in scripts.items():
    with open(os.path.join(out, name), "w") as f:
        json.dump(script, f, indent=1)
print(len(worlds), "worlds,", len(scripts), "scripts")
