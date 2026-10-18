#!/usr/bin/env python3
"""REA batch 08 lane render2: worlds for oracle/synthetic-rea-b08-vs (VolumeScatterMaterialExtension).

  python3 -I oracle/tools/gen_rea_b08_vs.py oracle/synthetic-rea-b08-vs

VolumeScatterMaterialExtension (X_ITE 16.5.0, component X_ITE, experimental): the scatter pass draws the
shapes with the extension into an RGBA32F buffer, the main pass blurs it (55 Burley samples) with the
radius attenuationDistance * multiscatterColor. Every file is synthetic. The worlds use metallic 0
(the PhysicalMaterial default is 1, which turns the scatter term off). Stepped runtime cases are
vs-runtime.wrl with vs-runtime.json.
"""
import json, os, sys

OUT = sys.argv[1] if len(sys.argv) > 1 else "oracle/synthetic-rea-b08-vs"
os.makedirs(OUT, exist_ok=True)

HEAD = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Shape:4
COMPONENT Lighting:3
COMPONENT CubeMapTexturing:3
COMPONENT Texturing:3
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
COMPONENT EnvironmentalEffects:3
COMPONENT Grouping:3
COMPONENT X_ITE:1
"""
ENV = 'EnvironmentLight { intensity 1 specularTexture ImageCubeMapTexture { url "../synthetic-rea-b02/pbr/env.png" } }'
LIGHTS = ("DirectionalLight { direction -0.4 -0.6 -0.7 intensity 0.8 color 1 0.95 0.9 }\n"
          "PointLight { location 2.5 2 3 intensity 2 color 0.6 0.8 1 radius 20 attenuation 0 0 1 }")
BG = "Background { skyColor 0.15 0.15 0.18 }"
PBR = "../synthetic-rea-b02/pbr/"
CHECKER = "../synthetic-rea-b03/proj-checker.png"
BEHIND = "\n".join([
    f'Transform {{ translation 0 0 -3 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture ImageTexture {{ url "{CHECKER}" }} }} geometry Box {{ size 8 6 0.1 }} }} }}',
    "Transform { translation -1.5 0.8 -1.5 children Shape { appearance Appearance { material Material { diffuseColor 1 0.2 0.2 } } geometry Box { size 0.6 2.5 0.3 } } }",
])

def tex(name, extra=""):
    return f'ImageTexture {{ url "{name}" {extra}}}'

def grid(materials, cols=4, spacing=1.05, y0=1.6, r=0.45, geometry=None, z=0, app_extra=""):
    out = []
    for i, m in enumerate(materials):
        x = (i % cols - (cols - 1) / 2) * spacing
        y = y0 - (i // cols) * spacing
        g = geometry or f"Sphere {{ radius {r} }}"
        out.append(f"Transform {{ translation {x:.3f} {y:.3f} {z} children Shape {{ appearance Appearance {{ {app_extra} material {m} }} geometry {g} }} }}")
    return "\n".join(out)

def pm(ext, base="baseColor 0.9 0.8 0.7", extra="metallic 0 roughness 0.6"):
    return f"PhysicalMaterial {{ {base} {extra} extensions [ {ext} ] }}"

def dt(f=1, c="1 0.7 0.5"):
    return f"DiffuseTransmissionMaterialExtension {{ diffuseTransmission {f} diffuseTransmissionColor {c} }}"

def vol(dist=0.6, th=0.4, col="1 0.8 0.6"):
    return f"VolumeMaterialExtension {{ thickness {th} attenuationDistance {dist} attenuationColor {col} }}"

def vs(ms="0.9 0.5 0.3", extra=""):
    return f"VolumeScatterMaterialExtension {{ multiscatterColor {ms} {extra} }}"

def world(*parts, vp="Viewpoint { position 0 0 6 }", head=HEAD):
    return head + vp + "\n" + "\n".join(parts) + "\n"

def runtime_json(frames, keys, start=20, step=6):
    events = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507},
              {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
    caps = []
    for i, k in enumerate(keys):
        f = start + i * step
        events.append({"frame": f, "type": "key", "key": k})
        caps.append(f + 4)
    return {"settle": False, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames,
            "capture": [12] + caps, "dump": [10] + caps + [frames - 1], "events": events}

worlds, scripts = {}, {}

# ---------------------------------------------------------------------------------------------
# The scatter color and distance: the scatter radius is attenuationDistance * multiscatterColor.
base = []
for ms, d in (("0.9 0.5 0.3", 0.6), ("0.2 0.2 0.2", 0.6), ("1 1 1", 0.6), ("0.05 0.6 0.9", 0.6),
              ("0.9 0.5 0.3", 0.15), ("0.9 0.5 0.3", 1.5), ("0.9 0.5 0.3", 4), ("0 0 0", 0.6)):
    base.append(pm(f"{dt()} {vol(d)} {vs(ms)}"))
base.append(pm(f"{dt(0.5)} {vol()} {vs()}"))
base.append(pm(f"{dt(1, '0.3 0.9 0.4')} {vol()} {vs()}"))
base.append(pm(f"{dt()} {vol()} {vs()}", "baseColor 0.1 0.2 0.9"))
base.append(pm(f"{dt()} {vol()}"))
worlds["vs-ibl.wrl"] = world(BG, ENV, BEHIND, grid(base, y0=1.6, spacing=1.0))
worlds["vs-lights.wrl"] = world(BG, LIGHTS, BEHIND, grid(base, y0=1.6, spacing=1.0))
worlds["vs-ibl-lights.wrl"] = world(BG, ENV, LIGHTS, grid(base, y0=1.6, spacing=1.0))
# A backlight: light from behind the sphere shows the transmitted (scattered) light.
worlds["vs-backlight.wrl"] = world(BG, "DirectionalLight { direction 0.2 0.1 1 intensity 2 color 1 0.9 0.8 }", grid(base[:8], y0=1.0, spacing=1.0, r=0.5))

# Missing parts: no Volume (attenuationDistance is not set), no DiffuseTransmission.
miss = [
    pm(f"{dt()} {vs()}"),
    pm(f"{vol()} {vs()}"),
    pm(vs()),
    pm(f"{dt()} {vol(0)} {vs()}"),
    pm(f"{dt()} {vol(-1)} {vs()}"),
    pm(f"{dt()} {vol(100)} {vs()}"),
    pm(f"{dt()} {vol(0.6, 0)} {vs()}"),
    pm(f"{dt()} {vol(0.6, 2)} {vs()}"),
]
worlds["vs-missing-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, grid(miss, y0=1.0, spacing=1.0))

# Other extensions on the same material.
oth = []
for e in ("ClearcoatMaterialExtension { clearcoat 1 clearcoatRoughness 0.1 }", "IridescenceMaterialExtension { iridescence 1 iridescenceThicknessMaximum 500 }",
          "SheenMaterialExtension { sheenColor 1 0.5 0.5 sheenRoughness 0.4 }", "SpecularMaterialExtension { specular 0.5 specularColor 1 0.4 0.4 }",
          "IORMaterialExtension { indexOfRefraction 2 }", "TransmissionMaterialExtension { transmission 0.5 }", "TransmissionMaterialExtension { transmission 1 }",
          "AnisotropyMaterialExtension { anisotropyStrength 1 }", "RetroreflectionMaterialExtension { retroreflection 1 }",
          "EmissiveStrengthMaterialExtension { emissiveStrength 3 }", "DispersionMaterialExtension { dispersion 5 }"):
    oth.append(pm(f"{dt()} {vol(0.8)} {vs()} {e}"))
oth.append(pm(f"{dt()} {vol(0.8)} {vs()}", extra="metallic 0.5 roughness 0.3"))
oth.append(pm(f"{dt()} {vol(0.8)} {vs()}", extra="metallic 0 roughness 0.1"))
oth.append(pm(f"{dt()} {vol(0.8)} {vs()} {vs('0.1 0.9 0.2')}"))
oth.append(pm(f"{vs('0.2 0.4 0.9')} {dt()} {vol(0.8)}", "baseColor 0.9 0.8 0.7 emissiveColor 0.2 0 0"))
worlds["vs-combined-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, grid(oth, y0=1.6, spacing=0.98))
# (X_ITE compiles each material twice, main and scatter pass: 8 materials per world keeps a page load under the harness's 45 s.)
# (A TransmissionMaterialExtension with the scatter extension and no EnvironmentLight never finishes loading in X_ITE 16.5.0:
# the scatter pass shader uses getDiffuseLight without the IBL code. Those two materials are only in vs-combined-ibl.)
lights_only = [o for i, o in enumerate(oth) if i not in (5, 6)]
worlds["vs-combined-a.wrl"] = world(BG, LIGHTS, BEHIND, grid(lights_only[:7], y0=1.0, spacing=1.0, r=0.48))
worlds["vs-combined-b.wrl"] = world(BG, LIGHTS, BEHIND, grid(lights_only[7:], y0=1.0, spacing=1.0, r=0.48))

# Geometry: shapes of different sizes and depths, overlapping shapes, scaled models, a plane, a cylinder.
mat = pm(f"{dt()} {vol(0.6)} {vs()}")
mat2 = pm(f"{dt()} {vol(0.6)} {vs('0.2 0.9 0.4')}", "baseColor 0.8 0.9 0.7")
mat3 = pm(f"{dt(1, '0.4 0.6 1')} {vol(0.3)} {vs('0.1 0.4 0.9')}", "baseColor 0.7 0.8 0.9")
geo = f"""
Transform {{ translation -1.6 0.9 0 children Shape {{ appearance Appearance {{ material {mat} }} geometry Sphere {{ radius 0.8 }} }} }}
Transform {{ translation -0.9 0.5 0.6 children Shape {{ appearance Appearance {{ material {mat2} }} geometry Sphere {{ radius 0.5 }} }} }}
Transform {{ translation 0.4 1.0 0 scale 2 0.7 1 children Shape {{ appearance Appearance {{ material {mat3} }} geometry Sphere {{ radius 0.45 }} }} }}
Transform {{ translation 1.7 0.8 -0.5 rotation 1 1 0 0.7 children Shape {{ appearance Appearance {{ material {mat} }} geometry Box {{ size 0.9 0.9 0.9 }} }} }}
Transform {{ translation -1.4 -1.0 0 children Shape {{ appearance Appearance {{ material {mat3} }} geometry Cylinder {{ radius 0.5 height 1.1 }} }} }}
Transform {{ translation 0 -1.0 0 rotation 0 1 0 0.6 children Shape {{ appearance Appearance {{ material {mat2} }} geometry IndexedFaceSet {{ solid FALSE coord Coordinate {{ point [ -0.7 -0.6 0, 0.7 -0.6 0, 0.7 0.6 0, -0.7 0.6 0 ] }} coordIndex [ 0 1 2 3 -1 ] }} }} }}
Transform {{ translation 1.5 -1.0 0 scale 1.5 1.5 1.5 children Shape {{ appearance Appearance {{ material {mat} }} geometry Cone {{ bottomRadius 0.4 height 0.8 }} }} }}
"""
worlds["vs-geometry-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, geo)
worlds["vs-geometry.wrl"] = world(BG, LIGHTS, BEHIND, geo)

# Appearance: transparency, backMaterial, blend mode, textures, fog, clipping.
t_mat = lambda tr: f"PhysicalMaterial {{ baseColor 0.9 0.8 0.7 metallic 0 roughness 0.6 transparency {tr} extensions [ {dt()} {vol(0.6)} {vs()} ] }}"
trans = [t_mat(0.3), t_mat(0.6), t_mat(0.9)]
# (Appearance.alphaMode is not used here: the Rust scene does not read the field.)
# Transparent shapes are drawn last into the scatter buffer without writing depth, with blending; the
# alpha is the shape id (1, 2, 3 ... in draw order), so only the first one blends as expected.
ov = lambda tr, x, y: (f"Transform {{ translation {x} {y} 0 children Shape {{ appearance Appearance {{ material "
                       f"{t_mat(tr)} }} geometry Sphere {{ radius 0.6 }} }} }}")
worlds["vs-transparent-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, grid(trans, cols=3, y0=0.9, spacing=1.2, r=0.55),
                                         grid([mat, mat2, mat3], cols=3, y0=-0.9, spacing=1.2, r=0.55),
                                         ov(0.5, -0.4, -0.9), ov(0.5, 0.1, -0.9), ov(0.2, 0.6, -0.9))
tex_m = [
    pm(f"{dt()} {vol(0.8)} VolumeScatterMaterialExtension {{ multiscatterColor 1 1 1 multiscatterColorTexture {tex(PBR + 'base.png')} }}"),
    pm(f"{dt()} {vol(0.8)} VolumeScatterMaterialExtension {{ multiscatterColor 0.9 0.6 0.4 multiscatterColorTexture {tex(CHECKER)} }}"),
    pm(f"{dt()} {vol(0.8)} VolumeScatterMaterialExtension {{ multiscatterColor 0.9 0.6 0.4 multiscatterColorTexture {tex(PBR + 'emissive.png')} scatterStrength 0.5 scatterAnisotropy 0.3 }}"),
    pm(f"{dt()} {vol(0.8)} {vs()}", f"baseColor 1 1 1 baseTexture {tex(PBR + 'base.png')}", "metallic 0 roughness 0.6"),
    pm(f"{dt(1, '1 1 1')} {vol(0.8)} {vs()}", "baseColor 1 1 1", f"metallic 0 roughness 0.6 normalTexture {tex(PBR + 'normal.png')}"),
    pm(f"{dt()} {vol(0.8)} {vs()}", "baseColor 1 1 1", f"metallic 0 roughness 0.6 metallicRoughnessTexture {tex(PBR + 'mr.png')}"),
    pm(f"DiffuseTransmissionMaterialExtension {{ diffuseTransmission 1 diffuseTransmissionColorTexture {tex(PBR + 'base.png')} }} {vol(0.8)} {vs()}"),
    pm(f"DiffuseTransmissionMaterialExtension {{ diffuseTransmission 1 diffuseTransmissionTexture {tex(PBR + 'occlusion.png')} }} {vol(0.8)} {vs()}"),
]
# (scatterStrengthTexture is left out: with it X_ITE 16.5.0 never finishes loading the world; its shader does not build.)
worlds["vs-textures-a-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, grid(tex_m[:4], y0=1.0, spacing=1.0, r=0.48))
worlds["vs-textures-b-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, grid(tex_m[4:], y0=1.0, spacing=1.0, r=0.48))
worlds["vs-fog.wrl"] = world(BG, ENV, LIGHTS, 'Fog { color 0.6 0.7 0.9 fogType "LINEAR" visibilityRange 9 }', grid(base[:8], y0=1.0, spacing=1.0, r=0.48))
# The ClipPlane is local to its Group (a root ClipPlane also clips the Background in X_ITE, which the Rust
# renderer does not do; that is not part of this node).
worlds["vs-clip.wrl"] = world(BG, ENV, LIGHTS,
                              'Group { children [ ClipPlane { plane 1 0 0 0.2 } ' + grid(base[:8], y0=1.0, spacing=1.0, r=0.48) + ' ] }')
two = ("Transform { translation -0.9 0 0 children Shape { appearance Appearance { material " + pm(f"{dt()} {vol(0.6)} {vs()}")
       + " backMaterial " + pm(f"{dt(1,'0.2 0.6 1')} {vol(0.6)} {vs('0.2 0.5 0.9')}", "baseColor 0.2 0.4 0.9")
       + " } geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.7 -0.7 0, 0.7 -0.7 0, 0.7 0.7 0, -0.7 0.7 0 ] } coordIndex [ 0 1 2 3 -1 ] } } }\n"
       "Transform { translation 1.0 0 0 rotation 0 1 0 2.6 children Shape { appearance Appearance { material " + pm(f"{dt()} {vol(0.6)} {vs()}")
       + " backMaterial " + pm(f"{dt(1,'0.2 0.6 1')} {vol(0.6)} {vs('0.2 0.5 0.9')}", "baseColor 0.2 0.4 0.9")
       + " } geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.7 -0.7 0, 0.7 -0.7 0, 0.7 0.7 0, -0.7 0.7 0 ] } coordIndex [ 0 1 2 3 -1 ] } } }")
worlds["vs-back-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, two)
worlds["vs-point-lines.wrl"] = world(BG, ENV, LIGHTS, f"""
Transform {{ translation -1 0 0 children Shape {{ appearance Appearance {{ material {mat} }} geometry PointSet {{ coord Coordinate {{ point [ 0 0 0, 0.5 0.5 0, -0.5 0.4 0 ] }} }} }} }}
Transform {{ translation 1 0 0 children Shape {{ appearance Appearance {{ material {mat} }} geometry IndexedLineSet {{ coord Coordinate {{ point [ 0 0 0, 0.5 0.5 0, -0.5 0.4 0 ] }} coordIndex [ 0 1 2 -1 ] }} }} }}""")
worlds["vs-small-ibl.wrl"] = world(BG, ENV, LIGHTS, grid(base[:4], y0=0.0, spacing=0.4, r=0.15, cols=4),
                                   vp="Viewpoint { position 0 0 12 }")

# Runtime: field changes, extension assigned / cleared on a live material, Shape removed from a live Group.
rt = HEAD + """Viewpoint { position 0 0 6 }
""" + "\n".join([BG, ENV, LIGHTS, BEHIND]) + f"""
DEF DT1 DiffuseTransmissionMaterialExtension {{ diffuseTransmission 1 diffuseTransmissionColor 1 0.7 0.5 }}
DEF VO1 VolumeMaterialExtension {{ thickness 0.4 attenuationDistance 0.6 attenuationColor 1 0.8 0.6 }}
DEF VS1 VolumeScatterMaterialExtension {{ multiscatterColor 0.9 0.5 0.3 }}
DEF VS3 VolumeScatterMaterialExtension {{ multiscatterColor 0.2 0.4 0.9 multiscatterColorTexture ImageTexture {{ url "{PBR}base.png" }} }}
DEF M1 PhysicalMaterial {{ baseColor 0.9 0.8 0.7 metallic 0 roughness 0.6 extensions [ USE DT1 USE VO1 USE VS1 ] }}
DEF M2 PhysicalMaterial {{ baseColor 0.9 0.8 0.7 metallic 0 roughness 0.6 extensions [ USE DT1 USE VO1 ] }}
DEF M3 PhysicalMaterial {{ baseColor 0.9 0.8 0.7 metallic 0 roughness 0.6 extensions [ USE DT1 USE VO1 USE VS3 ] }}
DEF G1 Group {{ children [
  DEF S1 Transform {{ translation -1.5 0.8 0 children Shape {{ appearance Appearance {{ material USE M1 }} geometry Sphere {{ radius 0.6 }} }} }}
] }}
DEF S2 Transform {{ translation 0 0.8 0 children Shape {{ appearance Appearance {{ material USE M2 }} geometry Sphere {{ radius 0.6 }} }} }}
DEF S3 Transform {{ translation 1.5 0.8 0 children Shape {{ appearance Appearance {{ material USE M3 }} geometry Sphere {{ radius 0.6 }} }} }}
DEF K KeySensor {{ }}
DEF R Script {{
  directOutput TRUE
  inputOnly SFString on_key
  inputOutput SFString log ""
  initializeOnly SFNode vs1 USE VS1
  initializeOnly SFNode vs3 USE VS3
  initializeOnly SFNode vo1 USE VO1
  initializeOnly SFNode dt1 USE DT1
  initializeOnly SFNode m1 USE M1
  initializeOnly SFNode m2 USE M2
  initializeOnly SFNode m3 USE M3
  initializeOnly SFNode g1 USE G1
  initializeOnly SFNode s1 USE S1
  url "javascript:
var made = null;
function on_key (k)
{{
   log = log + 'K' + k + ' ';
   if (k == 'a') vs1 .multiscatterColor = new SFColor (0.1, 0.9, 0.2);
   if (k == 'b') vo1 .attenuationDistance = 2;
   if (k == 'c') dt1 .diffuseTransmissionColor = new SFColor (0.3, 0.5, 1);
   if (k == 'd') {{ made = Browser .currentScene .createNode ('VolumeScatterMaterialExtension'); made .multiscatterColor = new SFColor (0.8, 0.8, 0.2); m2 .extensions = new MFNode (dt1, vo1, made); }}
   if (k == 'e') made .multiscatterColor = new SFColor (0.1, 0.3, 0.9);
   if (k == 'f') m1 .extensions = new MFNode (dt1, vo1);
   if (k == 'g') vs3 .multiscatterColorTexture = Browser .currentScene .createNode ('PixelTexture');
   if (k == 'h') {{ var p = vs3 .multiscatterColorTexture; p .image = new SFImage (2, 2, 3, new MFInt32 (0xff0000, 0x00ff00, 0x0000ff, 0xffff00)); }}
   if (k == 'i') vs1 .scatterStrength = 0.2;
   if (k == 'j') m3 .extensions = new MFNode ();
   if (k == 'k') g1 .removeChildren = new MFNode (s1);
   if (k == 'l') g1 .addChildren = new MFNode (s1);
   if (k == 'm') m1 .extensions = new MFNode (dt1, vo1, vs1);
   if (k == 'n') vo1 .attenuationDistance = 0.1;
   if (k == 'o') m3 .metallic = 1;
}}
"
}}
ROUTE K.keyPress TO R.on_key
"""
worlds["vs-runtime.wrl"] = rt
scripts["vs-runtime.json"] = runtime_json(190, list("abcdefghijklmno"), step=10, start=16)

for name, text in worlds.items():
    with open(os.path.join(OUT, name), "w") as f:
        f.write(text)
for name, obj in scripts.items():
    with open(os.path.join(OUT, name), "w") as f:
        json.dump(obj, f, indent=1)
print(f"{len(worlds)} worlds, {len(scripts)} scripts in {OUT}")
