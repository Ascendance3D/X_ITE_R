#!/usr/bin/env python3
"""REA batch 07 part C worlds (disp-*, retro-*, tex3d-*, inst-*) for oracle/synthetic-rea-b07-mat.

  python3 -I oracle/tools/gen_rea_b07_mat.py oracle/synthetic-rea-b07-mat

DispersionMaterialExtension and RetroreflectionMaterialExtension (X_ITE 16.5.0, component X_ITE,
experimental), ImageTexture3D (NRRD files written here; DICOM files authored here), InstancedShape.
Every file is synthetic: the images and volumes are generated here, nothing is copied from a
historical world. Stepped runtime cases are the *-runtime.wrl worlds with their *.json scripts.
"""
import json, os, struct, sys, zlib, gzip, math

OUT = sys.argv[1] if len(sys.argv) > 1 else "oracle/synthetic-rea-b07-mat"
os.makedirs(OUT, exist_ok=True)

HEAD = """#X3D V4.0 utf8
PROFILE Interchange
COMPONENT Shape:4
COMPONENT Lighting:3
COMPONENT CubeMapTexturing:3
COMPONENT Texturing:3
COMPONENT Layering:1
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
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
    "Transform { translation 1.5 -0.8 -1.5 children Shape { appearance Appearance { material Material { diffuseColor 0.2 0.4 1 transparency 0.5 } } geometry Cone { bottomRadius 0.5 height 1.5 } } }",
])

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

def world(*parts, vp="Viewpoint { position 0 0 6 }", head=HEAD):
    return head + vp + "\n" + "\n".join(parts) + "\n"

def runtime_json(frames, keys, extra_events=(), start=20, step=6, capture=None):
    """A stepped script: the first click focuses, then one key per `step` frames."""
    events = [{"frame": 4, "type": "mousemove", "x": 5, "y": 507}, {"frame": 4, "type": "mousedown", "button": 0, "x": 5, "y": 507},
              {"frame": 5, "type": "mouseup", "button": 0, "x": 5, "y": 507}]
    caps = []
    for i, k in enumerate(keys):
        f = start + i * step
        events.append({"frame": f, "type": "key", "key": k})
        caps.append(f + 4)
    events += list(extra_events)
    caps = capture or caps
    return {"settle": True, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames,
            "capture": [12] + caps, "dump": [10] + caps + [frames - 1], "events": events}

worlds, scripts = {}, {}

# ---------------------------------------------------------------------------------------------
# DispersionMaterialExtension: max (dispersion, 0); in the IBL transmission only (IBL2.glsl
# getIBLVolumeRefraction splits the ior of the red / green / blue channel by
# (ior - 1) * 0.025 * dispersion); needs transmission and a volume thickness to refract.
def vol(th=0.6):
    return f"VolumeMaterialExtension {{ thickness {th} }}"

di = []
for d in (0, 0.5, 1, 3, 10, 20, 50, -3):
    di.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol()} DispersionMaterialExtension {{ dispersion {d} }}", "baseColor 1 1 1", "metallic 0 roughness 0"))
for d, r, i in ((10, 0.2, 1.5), (10, 0.5, 1.5), (5, 0, 2.4), (5, 0, 1.1)):
    di.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol(0.8)} DispersionMaterialExtension {{ dispersion {d} }} IORMaterialExtension {{ indexOfRefraction {i} }}", "baseColor 1 1 1", f"metallic 0 roughness {r}"))
di.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol(1.5)} DispersionMaterialExtension {{ dispersion 4 }}", "baseColor 0.8 1 0.8", "metallic 0 roughness 0.05"))
di.append(pm(f"TransmissionMaterialExtension {{ transmission 0.5 }} {vol()} DispersionMaterialExtension {{ dispersion 10 }}", "baseColor 1 1 1", "metallic 0 roughness 0"))
di.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} DispersionMaterialExtension {{ dispersion 10 }}", "baseColor 1 1 1", "metallic 0 roughness 0"))
di.append(pm(f"{vol()} DispersionMaterialExtension {{ dispersion 10 }}", "baseColor 1 1 1", "metallic 0 roughness 0"))
worlds["disp-transmission-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, grid(di, y0=1.6, spacing=0.98))
worlds["disp-transmission.wrl"] = world(BG, LIGHTS, BEHIND, grid(di, y0=1.6, spacing=0.98))

dc = []
dc.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} VolumeMaterialExtension {{ thickness 0.8 attenuationDistance 0.5 attenuationColor 1 0.4 0.3 }} DispersionMaterialExtension {{ dispersion 8 }}", "baseColor 1 1 1", "metallic 0 roughness 0.05"))
dc.append(pm(f"TransmissionMaterialExtension {{ transmission 1 transmissionTexture {tex(PBR + 'occlusion.png')} }} {vol(0.8)} DispersionMaterialExtension {{ dispersion 8 }}", "baseColor 1 1 1", "metallic 0 roughness 0.05"))
dc.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol(0.8)} DispersionMaterialExtension {{ dispersion 1 }} DispersionMaterialExtension {{ dispersion 12 }}", "baseColor 1 1 1", "metallic 0 roughness 0.05"))
dc.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol(0.8)} DispersionMaterialExtension {{ dispersion 8 }} SpecularMaterialExtension {{ specular 0.5 specularColor 1 0.6 0.6 }}", "baseColor 1 1 1", "metallic 0 roughness 0.05"))
dc.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol(0.8)} DispersionMaterialExtension {{ dispersion 8 }} ClearcoatMaterialExtension {{ clearcoat 1 clearcoatRoughness 0.1 }}", "baseColor 1 1 1", "metallic 0 roughness 0.05"))
dc.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol(0.8)} DispersionMaterialExtension {{ dispersion 8 }} IridescenceMaterialExtension {{ iridescence 1 }}", "baseColor 1 1 1", "metallic 0 roughness 0.05"))
dc.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol(0.8)} DispersionMaterialExtension {{ dispersion 8 }} DiffuseTransmissionMaterialExtension {{ diffuseTransmission 0.5 }}", "baseColor 1 1 1", "metallic 0 roughness 0.05"))
dc.append(pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol(0.8)} DispersionMaterialExtension {{ dispersion 8 }} RetroreflectionMaterialExtension {{ retroreflection 1 }}", "baseColor 1 1 1", "metallic 0 roughness 0.05"))
mapped_back = ("Transform { translation -1.6 -1.1 0 rotation 0 1 0 2.6 children Shape { appearance Appearance { material "
               + pm("DispersionMaterialExtension { dispersion 3 }", "baseColor 0.2 0.8 0.2", "metallic 0 roughness 0.9")
               + " backMaterial " + pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol(0.8)} DispersionMaterialExtension {{ dispersion 10 }}", "baseColor 1 1 1", "metallic 0 roughness 0")
               + " } geometry IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.5 -0.5 0, 0.5 -0.5 0, 0.5 0.5 0, -0.5 0.5 0 ] } coordIndex [ 0 1 2 3 -1 ] } } }")
cyl = ("Transform { translation 0.5 -1.1 0 children Shape { appearance Appearance { material "
       + pm(f"TransmissionMaterialExtension {{ transmission 1 }} {vol(0.5)} DispersionMaterialExtension {{ dispersion 15 }}", "baseColor 1 1 1", "metallic 0 roughness 0")
       + " } geometry Cylinder { radius 0.4 height 0.9 } } }")
worlds["disp-combined-ibl.wrl"] = world(BG, ENV, LIGHTS, BEHIND, grid(dc, y0=1.6), mapped_back, cyl)

# Cross-system: layers, shadows are not part of the extension; a LayerSet with a second layer.
worlds["disp-layers-ibl.wrl"] = world(
    BG, ENV, LIGHTS, BEHIND, grid(di[:4], y0=1.6, spacing=1.05),
    'LayerSet { order [ 0 1 ] layers [ Layer { children [ Viewpoint { position 0 0 6 } '
    + "EnvironmentLight { intensity 1 specularTexture ImageCubeMapTexture { url \"../synthetic-rea-b02/pbr/env.png\" } } "
    + f'Transform {{ translation 0 -1 -1 children Shape {{ appearance Appearance {{ material Material {{ diffuseColor 1 1 1 }} texture ImageTexture {{ url "{CHECKER}" }} }} geometry Box {{ size 5 2 0.1 }} }} }} '
    + "Transform { translation 0 -1 0 children Shape { appearance Appearance { material " + di[5] + " } geometry Sphere { radius 1 } } } ] } ] }")

disp_rt = HEAD + """Viewpoint { position 0 0 6 }
""" + "\n".join([BG, ENV, LIGHTS, BEHIND]) + """
DEF D1 DispersionMaterialExtension { dispersion 0 }
DEF TR TransmissionMaterialExtension { transmission 1 }
DEF VO VolumeMaterialExtension { thickness 0.8 }
DEF M1 PhysicalMaterial { baseColor 1 1 1 metallic 0 roughness 0 extensions [ USE TR USE VO USE D1 ] }
DEF M2 PhysicalMaterial { baseColor 1 1 1 metallic 0 roughness 0 extensions [ USE TR USE VO ] }
DEF M3 PhysicalMaterial { baseColor 1 1 1 metallic 0 roughness 0 extensions [ USE TR USE VO DEF D3 DispersionMaterialExtension { dispersion 6 } ] }
DEF G1 Group { children [
  DEF S1 Transform { translation -1.4 0.6 0 children Shape { appearance Appearance { material USE M1 } geometry Sphere { radius 0.6 } } }
] }
DEF S2 Transform { translation 0 0.6 0 children Shape { appearance Appearance { material USE M2 } geometry Sphere { radius 0.6 } } }
DEF S3 Transform { translation 1.4 0.6 0 children Shape { appearance Appearance { material USE M3 } geometry Sphere { radius 0.6 } } }
DEF K KeySensor { }
DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOutput SFString log ""
  initializeOnly SFNode d1 USE D1
  initializeOnly SFNode d3 USE D3
  initializeOnly SFNode m2 USE M2
  initializeOnly SFNode m3 USE M3
  initializeOnly SFNode tr USE TR
  initializeOnly SFNode vo USE VO
  initializeOnly SFNode g1 USE G1
  initializeOnly SFNode s1 USE S1
  initializeOnly SFNode s2 USE S2
  url "javascript:
var made = null;
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') d1 .dispersion = 10;
   if (k == 'b') d1 .dispersion = -4;
   if (k == 'c') d3 .dispersion = 30;
   if (k == 'd') { made = Browser .currentScene .createNode ('DispersionMaterialExtension'); made .dispersion = 12; m2 .extensions = new MFNode (tr, vo, made); }
   if (k == 'e') made .dispersion = 2;
   if (k == 'f') m3 .extensions = new MFNode (tr, vo);
   if (k == 'g') vo .thickness = 1.6;
   if (k == 'h') tr .transmission = 0.4;
   if (k == 'i') g1 .removeChildren = new MFNode (s1);
   if (k == 'j') g1 .addChildren = new MFNode (s1);
   if (k == 'k') m2 .extensions = new MFNode ();
}
"
}
ROUTE K.keyPress TO R.on_key
"""
worlds["disp-runtime.wrl"] = disp_rt
scripts["disp-runtime.json"] = runtime_json(150, list("abcdefghijk"), step=10, start=16)

# ---------------------------------------------------------------------------------------------
# RetroreflectionMaterialExtension: clamp (retroreflection, 0, 1) × retroreflectionTexture (r);
# the specular lobe uses v_retro = reflect (-v, n) (IBL and punctual lights).
rt = []
for f, m, r in ((0, 1, 0.2), (0.5, 1, 0.2), (1, 1, 0.2), (1, 1, 0.5), (1, 0, 0.2), (1, 0, 0.5), (2, 1, 0.3), (-1, 1, 0.3)):
    rt.append(pm(f"RetroreflectionMaterialExtension {{ retroreflection {f} }}", "baseColor 0.9 0.6 0.3", f"metallic {m} roughness {r}"))
rt.append(pm(f"RetroreflectionMaterialExtension {{ retroreflection 1 retroreflectionTexture {tex(PBR + 'occlusion.png')} }}", "baseColor 0.9 0.6 0.3", "metallic 1 roughness 0.25"))
rt.append(pm(f"RetroreflectionMaterialExtension {{ retroreflection 0.8 retroreflectionTexture {tex(PBR + 'mr.png')} }}", "baseColor 0.9 0.6 0.3", "metallic 0.5 roughness 0.25"))
rt.append(pm(f"RetroreflectionMaterialExtension {{ retroreflection 1 retroreflectionTexture {tex(PBR + 'base-alpha.png')} }}", "baseColor 0.3 0.6 0.9", "metallic 1 roughness 0.15"))
rt.append(pm("RetroreflectionMaterialExtension { retroreflection 1 } RetroreflectionMaterialExtension { retroreflection 0.25 }", "baseColor 0.9 0.9 0.9", "metallic 1 roughness 0.1"))
rt.append(pm("RetroreflectionMaterialExtension { retroreflection 1 } IORMaterialExtension { indexOfRefraction 2.2 }", "baseColor 0.5 0.5 0.5", "metallic 0 roughness 0.1"))
rt.append(pm("RetroreflectionMaterialExtension { retroreflection 1 } SpecularMaterialExtension { specular 0.5 specularColor 1 0.4 0.4 }", "baseColor 0.5 0.5 0.5", "metallic 0 roughness 0.1"))
rt.append(pm("RetroreflectionMaterialExtension { retroreflection 1 }", "baseColor 0.9 0.9 0.9", f"metallic 1 roughness 0.3 normalTexture {tex(PBR + 'normal.png')}"))
rt.append(pm("RetroreflectionMaterialExtension { retroreflection 1 } EmissiveStrengthMaterialExtension { emissiveStrength 2 }", "baseColor 0.9 0.9 0.9 emissiveColor 0.1 0.05 0", "metallic 1 roughness 0.3"))
worlds["retro-ibl.wrl"] = world(BG, ENV, grid(rt, y0=1.6, spacing=0.98), grid(rt[:4], y0=-1.6, geometry="Box { size 0.7 0.7 0.7 }", spacing=0.98))
worlds["retro-lights.wrl"] = world(BG, LIGHTS, grid(rt, y0=1.6, spacing=0.98), grid(rt[:4], y0=-1.6, geometry="Box { size 0.7 0.7 0.7 }", spacing=0.98))
worlds["retro-ibl-lights.wrl"] = world(BG, ENV, LIGHTS, grid(rt, y0=1.6, spacing=0.98))

rc = []
rc.append(pm("RetroreflectionMaterialExtension { retroreflection 1 } AnisotropyMaterialExtension { anisotropyStrength 0.8 anisotropyRotation 0.4 }", "baseColor 0.8 0.8 0.85", "metallic 1 roughness 0.35"))
rc.append(pm("RetroreflectionMaterialExtension { retroreflection 1 } IridescenceMaterialExtension { iridescence 1 iridescenceThicknessMaximum 500 }", "baseColor 0.1 0.1 0.1", "metallic 0.5 roughness 0.2"))
rc.append(pm("RetroreflectionMaterialExtension { retroreflection 1 } ClearcoatMaterialExtension { clearcoat 1 clearcoatRoughness 0.1 }", "baseColor 0.7 0.1 0.1", "metallic 0 roughness 0.6"))
rc.append(pm("RetroreflectionMaterialExtension { retroreflection 1 } SheenMaterialExtension { sheenColor 1 0.5 0.5 sheenRoughness 0.4 }", "baseColor 0.2 0.2 0.3", "metallic 0.3 roughness 0.6"))
rc.append(pm("RetroreflectionMaterialExtension { retroreflection 0.7 } TransmissionMaterialExtension { transmission 1 } VolumeMaterialExtension { thickness 0.6 }", "baseColor 1 1 1", "metallic 0 roughness 0.1"))
rc.append(pm("RetroreflectionMaterialExtension { retroreflection 1 } DiffuseTransmissionMaterialExtension { diffuseTransmission 0.6 diffuseTransmissionColor 1 0.6 0.3 }", "baseColor 0.8 0.8 0.8", "metallic 0 roughness 0.3"))
rc.append(pm("RetroreflectionMaterialExtension { retroreflection 1 } AnisotropyMaterialExtension { anisotropyStrength 1 } IridescenceMaterialExtension { iridescence 1 } ClearcoatMaterialExtension { clearcoat 0.5 }", "baseColor 0.9 0.9 0.9", "metallic 1 roughness 0.3"))
rc.append(pm("DispersionMaterialExtension { dispersion 5 } RetroreflectionMaterialExtension { retroreflection 1 } TransmissionMaterialExtension { transmission 1 } VolumeMaterialExtension { thickness 0.6 }", "baseColor 1 1 1", "metallic 0 roughness 0"))
rmap = []
for i, m in enumerate([
    pm(f"RetroreflectionMaterialExtension {{ retroreflection 1 retroreflectionTexture {tex(PBR + 'occlusion.png')} retroreflectionTextureMapping \"second\" }}", "baseColor 0.9 0.6 0.3", "metallic 1 roughness 0.25"),
    pm(f"RetroreflectionMaterialExtension {{ retroreflection 1 retroreflectionTexture {tex(CHECKER)} retroreflectionTextureMapping \"first\" }}", "baseColor 0.9 0.6 0.3", "metallic 1 roughness 0.25"),
]):
    x = (i - 0.5) * 1.05 + 1.6
    geom = ("IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.45 -0.45 0, 0.45 -0.45 0, 0.45 0.45 0, -0.45 0.45 0 ] } coordIndex [ 0 1 2 3 -1 ] "
            "texCoord MultiTextureCoordinate { texCoord [ TextureCoordinate { mapping \"first\" point [ 0 0, 1 0, 1 1, 0 1 ] } TextureCoordinate { mapping \"second\" point [ 0 0, 3 0, 3 3, 0 3 ] } ] } }")
    rmap.append(f"Transform {{ translation {x:.3f} -1.6 0 rotation 0 1 0 0.5 children Shape {{ appearance Appearance {{ material {m} }} geometry {geom} }} }}")
worlds["retro-combined-ibl.wrl"] = world(BG, ENV, LIGHTS, grid(rc, y0=1.6), "\n".join(rmap))
worlds["retro-combined.wrl"] = world(BG, LIGHTS, grid(rc, y0=1.6), "\n".join(rmap))

retro_rt = HEAD + """Viewpoint { position 0 0 6 }
""" + "\n".join([BG, ENV, LIGHTS]) + """
DEF R1 RetroreflectionMaterialExtension { retroreflection 0 }
DEF R3 RetroreflectionMaterialExtension { retroreflection 1 retroreflectionTexture ImageTexture { url \"""" + PBR + """occlusion.png" } }
DEF M1 PhysicalMaterial { baseColor 0.9 0.6 0.3 metallic 1 roughness 0.25 extensions [ USE R1 ] }
DEF M2 PhysicalMaterial { baseColor 0.9 0.6 0.3 metallic 1 roughness 0.25 }
DEF M3 PhysicalMaterial { baseColor 0.9 0.6 0.3 metallic 1 roughness 0.25 extensions [ USE R3 ] }
DEF G1 Group { children [
  DEF S1 Transform { translation -1.4 0.6 0 children Shape { appearance Appearance { material USE M1 } geometry Sphere { radius 0.6 } } }
] }
DEF S2 Transform { translation 0 0.6 0 children Shape { appearance Appearance { material USE M2 } geometry Sphere { radius 0.6 } } }
DEF S3 Transform { translation 1.4 0.6 0 children Shape { appearance Appearance { material USE M3 } geometry Sphere { radius 0.6 } } }
DEF K KeySensor { }
DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOutput SFString log ""
  initializeOnly SFNode r1 USE R1
  initializeOnly SFNode r3 USE R3
  initializeOnly SFNode m2 USE M2
  initializeOnly SFNode m3 USE M3
  initializeOnly SFNode g1 USE G1
  initializeOnly SFNode s1 USE S1
  url "javascript:
var made = null;
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') r1 .retroreflection = 1;
   if (k == 'b') r1 .retroreflection = 7;
   if (k == 'c') r1 .retroreflection = -2;
   if (k == 'd') r3 .retroreflection = 0.5;
   if (k == 'e') r3 .retroreflectionTexture = Browser .currentScene .createNode ('PixelTexture');
   if (k == 'f') { var p = r3 .retroreflectionTexture; p .image = new SFImage (2, 2, 1, new MFInt32 (0x20, 0xff, 0xff, 0x20)); }
   if (k == 'g') r3 .retroreflectionTextureMapping = 'nothing';
   if (k == 'h') { made = Browser .currentScene .createNode ('RetroreflectionMaterialExtension'); made .retroreflection = 1; m2 .extensions = new MFNode (made); }
   if (k == 'i') made .retroreflection = 0.3;
   if (k == 'j') m3 .extensions = new MFNode ();
   if (k == 'k') g1 .removeChildren = new MFNode (s1);
   if (k == 'l') g1 .addChildren = new MFNode (s1);
}
"
}
ROUTE K.keyPress TO R.on_key
"""
worlds["retro-runtime.wrl"] = retro_rt
scripts["retro-runtime.json"] = runtime_json(170, list("abcdefghijkl"), step=10, start=16)

# ---------------------------------------------------------------------------------------------
# ---------------------------------------------------------------------------------------------
# ImageTexture3D: NRRD (NRRDParser.js) and DICOM (DICOMParser.js over dicom-parser) volumes,
# written here from formulas (no historical data). The files live in vol/.
VOL = os.path.join(OUT, "vol")
os.makedirs(VOL, exist_ok=True)

def voxel(x, y, z, w, h, d, comps):
    """One synthetic voxel: a gradient with a coarse checker, so slices and axes are told apart."""
    sc = lambda v, n: v * 255 // max(n - 1, 1)
    r, g, b = sc(x, w), sc(y, h), sc(z, d)
    chk = 70 if ((x // 4) + (y // 4) + (z // 2)) % 2 else 0
    if comps == 1:
        return [(r // 2 + g // 4 + b // 4 + chk) % 256]
    if comps == 2:
        return [(r + chk) % 256, 255 - sc(z, d) // 2]
    if comps == 3:
        return [(r + chk) % 256, g, b]
    return [(r + chk) % 256, g, b, 90 + sc(x + y, w + h) // 2]

def vol_bytes(w, h, d, comps):
    return [v for z in range(d) for y in range(h) for x in range(w) for v in voxel(x, y, z, w, h, d, comps)]

def nrrd(name, w, h, d, comps=1, typ="uchar", enc="raw", endian=None, extra="", payload=None, header=None, sizes=None, dim=None, trailing=b"", comment=True):
    vals = vol_bytes(w, h, d, comps)
    size_list = sizes if sizes is not None else ([comps] if comps > 1 else []) + [w, h, d]
    lines = ["NRRD0004"]
    if comment:
        lines += ["# Complete NRRD file format specification at:", "# synthetic volume of oracle/synthetic-rea-b07-mat (generated)"]
    if typ is not None:
        lines.append(f"type: {typ}")
    lines.append(f"dimension: {dim if dim is not None else len(size_list)}")
    if size_list:
        lines.append("sizes: " + " ".join(str(v) for v in size_list))
    if endian:
        lines.append(f"endian: {endian}")
    if enc:
        lines.append(f"encoding: {enc}")
    if extra:
        lines.append(extra)
    head = ("\n".join(lines) + "\n\n").encode()
    if header is not None:
        head = header
    if payload is None:
        t = typ or "uchar"
        big = endian == "big"
        def conv(v):
            if t in ("uchar", "unsigned char", "uint8", "uint8_t", "signed char", "int8", "int8_t"):
                return bytes([v])
            if t in ("ushort", "short", "uint16", "int16", "unsigned short", "signed short"):
                return struct.pack(">H" if big else "<H", v * 256 + 17)
            if t in ("uint", "int", "uint32", "int32", "unsigned int", "signed int"):
                return struct.pack(">I" if big else "<I", v * 16777216 + 12345)
            if t == "float":
                return struct.pack(">f" if big else "<f", v * 256.0 + 0.5)
            if t == "double":
                return struct.pack(">d" if big else "<d", v * 16777216.0 + 100.5)
            return bytes([v])  # an unsupported type name: the parser rejects it before the data
        if enc in ("raw", "gzip", "gz", "hex"):
            raw = b"".join(conv(v) for v in vals)
            if enc == "hex":
                body = "\n".join(raw[i:i + 32].hex() for i in range(0, len(raw), 32)).encode() + b"\n"
            elif enc in ("gzip", "gz"):
                body = gzip.compress(raw, 9, mtime=0)
            else:
                body = raw
        else:  # ascii
            def text(v):
                if t.startswith("u") or t in ("short", "int", "signed char", "int8"):
                    pass
                if t in ("float",):
                    return f"{v * 256.0 + 0.5}"
                if t in ("double",):
                    return f"{v * 16777216.0 + 100.5}"
                if t in ("ushort", "short", "uint16", "int16"):
                    return str(v * 256 + 17)
                if t in ("uint", "int", "uint32", "int32"):
                    return str(v * 16777216 + 12345)
                return str(v)
            body = (" ".join(text(v) for v in vals)).encode() + b"\n"
        payload = body
    open(os.path.join(VOL, name), "wb").write(head + payload + trailing)

TT3 = "textureTransform TextureTransform3D { translation 0.5 0.5 0.5 scale 0.5 0.5 0.5 }"
GEN = 'texCoord TextureCoordinateGenerator { mode "COORD" }'
HEAD3D = """#X3D V4.1 utf8
PROFILE Interchange
COMPONENT Texturing3D:2
COMPONENT Layering:1
COMPONENT Layout:2
COMPONENT Geometry2D:2
COMPONENT Scripting:1
COMPONENT KeyDeviceSensor:1
COMPONENT Networking:4
COMPONENT Lighting:3
"""

def tex3d_item(tex, x, y, shape="sphere", extra=TT3, mat="Material { diffuseColor 1 1 1 }"):
    if shape == "sphere":
        geom, rot = "Sphere { radius 0.9 }", ""
    else:
        geom, rot = "Box { size 1.4 1.4 1.4 }", "rotation 1 1 0 0.6"
        extra = "textureTransform TextureTransform3D { translation 0.5 0.5 0.5 scale 0.7 0.7 0.7 }"
    return f"Transform {{ translation {x} {y} 0 {rot} children Shape {{ appearance Appearance {{ material {mat} texture {tex} {extra} }} geometry {geom} }} }}"

def tex3d_grid(textures, cols=4, shape_cycle=("sphere", "box"), x0=-4.5, y0=2.5, dx=3, dy=2.8, mat=None):
    out = []
    for i, t in enumerate(textures):
        x, y = x0 + (i % cols) * dx, y0 - (i // cols) * dy
        out.append(tex3d_item(t, x, y, shape_cycle[(i // cols) % len(shape_cycle)], mat=mat or "Material { diffuseColor 1 1 1 }"))
    return "\n".join(out)

def world3d(*parts, vp="Viewpoint { position 0 0 15 }"):
    return HEAD3D + vp + "\nNavigationInfo { headlight TRUE }\n" + "\n".join(parts) + "\n"

def it3(url, extra=""):
    return f'ImageTexture3D {{ url {url} {extra}}}'

# Encodings, components and sizes.
nrrd("u8-raw.nrrd", 16, 16, 16)
nrrd("u8-ascii.nrrd", 16, 16, 16, enc="ascii")
nrrd("u8-hex.nrrd", 16, 16, 16, enc="hex")
nrrd("u8-gzip.nrrd", 16, 16, 16, enc="gzip")
nrrd("u8-gz-alias.nrrd", 16, 16, 16, enc="gz", typ="uint8_t")
nrrd("la-raw.nrrd", 16, 16, 16, comps=2)
nrrd("rgb-raw.nrrd", 16, 16, 16, comps=3)
nrrd("rgba-raw.nrrd", 16, 16, 16, comps=4)
nrrd("rgb-gzip.nrrd", 8, 8, 8, comps=3, enc="gzip")
nrrd("tiny-u8.nrrd", 4, 4, 4)
nrrd("wide-u8.nrrd", 32, 16, 8)
nrrd("flat-2d.nrrd", 16, 16, 1, sizes=[16, 16], dim=2)
nrrd("line-1d.nrrd", 16, 1, 1, sizes=[16], dim=1)
nrrd("u8-trailing.nrrd", 16, 16, 16, trailing=bytes(range(5)))
nrrd("u8-upper.nrrd", 16, 16, 16, extra="Content: Synthetic Volume\nspace: left-posterior-superior\nspace directions: (1,0,0) (0,1,0) (0,0,1)\nkinds: domain domain domain")
nrrd("u8-crlf-field.nrrd", 8, 8, 8, header=b"NRRD0004\ntype: uchar\ndimension: 3\nsizes: 8 8 8\r\nencoding: raw\n\n", payload=bytes(vol_bytes(8, 8, 8, 1)))
# Numeric types (the sample is divided down to 8 bit).
for name, typ, endian in (("short-le", "short", "little"), ("ushort-be", "ushort", "big"), ("int-le", "int", "little"), ("uint-be", "uint32", "big"), ("float-le", "float", "little"), ("double-be", "double", "big"), ("schar", "signed char", None), ("int8t", "int8_t", None)):
    nrrd(f"type-{name}.nrrd", 8, 8, 8, typ=typ, endian=endian)
nrrd("type-short-ascii.nrrd", 8, 8, 8, typ="short", enc="ascii")
nrrd("type-float-ascii.nrrd", 8, 8, 8, typ="float", enc="ascii")
nrrd("type-int-ascii.nrrd", 8, 8, 8, typ="int", enc="ascii")
nrrd("type-double-ascii.nrrd", 8, 8, 8, typ="double", enc="ascii")
nrrd("type-float-unit.nrrd", 8, 8, 8, typ="float", payload=b"".join(struct.pack("<f", v / 255.0) for v in vol_bytes(8, 8, 8, 1)))
# Invalid files.
nrrd("bad-type.nrrd", 8, 8, 8, typ="complex")
nrrd("bad-encoding.nrrd", 8, 8, 8, enc="bzip2", payload=b"x")
nrrd("bad-dimension.nrrd", 8, 8, 8, dim=5)
nrrd("bad-dimension-none.nrrd", 8, 8, 8, extra="dimension: none")
nrrd("bad-sizes.nrrd", 8, 8, 8, sizes=[2, 2, 2, 2, 2], dim=4)
nrrd("bad-endian.nrrd", 8, 8, 8, typ="short", endian="middle")
nrrd("short-raw.nrrd", 8, 8, 8, payload=bytes(100))
nrrd("short-gzip.nrrd", 8, 8, 8, enc="gzip", payload=gzip.compress(bytes(100), 9, mtime=0))
nrrd("bad-gzip.nrrd", 8, 8, 8, enc="gzip", payload=b"not gzip data at all")
nrrd("bad-hex.nrrd", 8, 8, 8, enc="hex", payload=b"zz yy xx\n")
nrrd("empty-hex.nrrd", 8, 8, 8, enc="hex", payload=b"")
nrrd("too-big.nrrd", 16385, 1, 1, payload=bytes(16385))
nrrd("zero-size.nrrd", 0, 4, 4, payload=b"")
nrrd("comp5.nrrd", 8, 8, 8, comps=5, sizes=[5, 4, 4, 4], dim=4, payload=bytes(5 * 64))
nrrd("no-sizes.nrrd", 8, 8, 8, sizes=[], dim=3, payload=bytes(10))
nrrd("no-encoding.nrrd", 4, 4, 4, enc=None, payload=b"")
nrrd("no-type.nrrd", 4, 4, 4, typ=None, payload=bytes(64))
open(os.path.join(VOL, "not-a-volume.nrrd"), "wb").write(b"This is a plain text file and no volume at all.\n" * 5)
open(os.path.join(VOL, "tiny-file.nrrd"), "wb").write(b"NRRD")
open(os.path.join(VOL, "nrrd-version-only.nrrd"), "wb").write(b"NRRD0004\n")
open(os.path.join(VOL, "nrrd-crlf.nrrd"), "wb").write(b"NRRD0004\r\ntype: uchar\r\nsizes: 4 4 4\r\nencoding: raw\r\n\r\n" + bytes(64))

# DICOM Part 10 files, written here.
def dicom_element(tag, vr, value, implicit=False, big=False):
    g, e = tag
    pack16 = ">H" if big else "<H"
    pack32 = ">I" if big else "<I"
    if implicit:
        return struct.pack("<HHI", g, e, len(value)) + value
    head = struct.pack(pack16, g) + struct.pack(pack16, e) + vr.encode()
    if vr in ("OB", "OW", "OF", "SQ", "UT", "UN", "OD", "OL", "UC", "UR"):
        return head + b"\x00\x00" + struct.pack(pack32, len(value)) + value
    return head + struct.pack(pack16, len(value)) + value

def pad(s, ch=b" "):
    s = s if isinstance(s, bytes) else s.encode()
    return s + (ch if len(s) % 2 else b"")

def dicom(name, w, h, d, comps=1, photometric="MONOCHROME2", bits=8, signed=0, syntax="1.2.840.10008.1.2.1", planar=0, stored=None, frames=None, pixels=None, big=False, implicit=False, encapsulated=None, element7fe0="OW", extra=b"", no_prefix=False, omit=()):
    ts = pad(syntax, b"\x00")
    meta = b"".join([
        dicom_element((2, 1), "OB", b"\x00\x01"),
        dicom_element((2, 2), "UI", pad("1.2.840.10008.5.1.4.1.1.7", b"\x00")),
        dicom_element((2, 3), "UI", pad("1.2.3.4.5.6.7.8.9", b"\x00")),
        dicom_element((2, 0x10), "UI", ts),
        dicom_element((2, 0x12), "UI", pad("1.2.3.4.5", b"\x00")),
    ])
    meta = dicom_element((2, 0), "UL", struct.pack("<I", len(meta))) + meta
    us = lambda v: struct.pack(">H" if big else "<H", v)
    imp = implicit
    els = []
    if ("spp" not in omit):
        els.append(dicom_element((0x28, 2), "US", us(comps), imp, big))
    if "pi" not in omit:
        els.append(dicom_element((0x28, 4), "CS", pad(photometric), imp, big))
    if comps > 1 or planar:
        els.append(dicom_element((0x28, 6), "US", us(planar), imp, big))
    if d != 1 or "frames1" in omit:
        els.append(dicom_element((0x28, 8), "IS", pad(str(d)), imp, big))
    els.append(dicom_element((0x28, 0x10), "US", us(h), imp, big))
    els.append(dicom_element((0x28, 0x11), "US", us(w), imp, big))
    els.append(dicom_element((0x28, 0x100), "US", us(bits), imp, big))
    els.append(dicom_element((0x28, 0x101), "US", us(stored or bits), imp, big))
    els.append(dicom_element((0x28, 0x102), "US", us((stored or bits) - 1), imp, big))
    els.append(dicom_element((0x28, 0x103), "US", us(signed), imp, big))
    els.append(extra)
    if pixels is None:
        pixels = b""
        for z in range(d):
            for y in range(h):
                for x in range(w):
                    for c in voxel(x, y, z, w, h, d, comps if comps in (1, 3) else 1)[:comps]:
                        v = c if bits == 8 else c * 257
                        if signed and bits == 16:
                            v = c * 100 - 12000
                        pixels += (bytes([v]) if bits == 8 else struct.pack(">h" if big and signed else (">H" if big else ("<h" if signed else "<H")), v))
    if encapsulated is not None:
        bot, frag_list = encapsulated
        pix = b"\xe0\x7f\x10\x00" + b"OB\x00\x00" + b"\xff\xff\xff\xff"
        pix += struct.pack("<HHI", 0xfffe, 0xe000, len(bot) * 4) + b"".join(struct.pack("<I", o) for o in bot)
        for fr in frag_list:
            pix += struct.pack("<HHI", 0xfffe, 0xe000, len(fr)) + fr
        pix += struct.pack("<HHI", 0xfffe, 0xe0dd, 0)
        els.append(pix)
    elif "nopixels" not in omit:
        els.append(dicom_element((0x7fe0, 0x10), element7fe0 if not imp else "OW", pixels, imp, big))
    data = b"".join(els)
    out = (b"" if no_prefix else b"\x00" * 128 + b"DICM") + meta + data
    open(os.path.join(VOL, name), "wb").write(out)

def rle_segment(raw):
    """PackBits of one segment (DICOM RLE: header byte n 0..127 literal n + 1, -1..-127 run)."""
    out, i = bytearray(), 0
    while i < len(raw):
        j = i
        while j + 1 < len(raw) and raw[j + 1] == raw[i] and j - i < 127:
            j += 1
        if j > i:
            out += bytes([(257 - (j - i + 1)) & 0xff, raw[i]])
            i = j + 1
        else:
            k = i
            while k + 1 < len(raw) and (k + 2 >= len(raw) or raw[k + 1] != raw[k + 2]) and k - i < 127:
                k += 1
            out += bytes([k - i]) + raw[i:k + 1]
            i = k + 1
    return bytes(out)

def rle_frame(raw_planes):
    segs = [rle_segment(p) for p in raw_planes]
    head = struct.pack("<I", len(segs)) + b"".join(struct.pack("<I", 64 + sum(len(s) for s in segs[:i])) for i in range(len(segs)))
    head += b"\x00" * (64 - len(head))
    return head + b"".join(segs) + (b"\x00" if sum(len(s) for s in segs) % 2 else b"")

dicom("mono2-explicit.dcm", 16, 16, 8)
dicom("mono2-implicit.dcm", 16, 16, 8, syntax="1.2.840.10008.1.2", implicit=True)
dicom("mono2-big-endian.dcm", 16, 16, 8, syntax="1.2.840.10008.1.2.2", big=True)
dicom("mono1.dcm", 16, 16, 8, photometric="MONOCHROME1")
dicom("mono2-16bit.dcm", 16, 16, 8, bits=16)
dicom("mono2-16bit-signed.dcm", 16, 16, 8, bits=16, signed=1, stored=12)
dicom("mono2-16bit-big-endian.dcm", 16, 16, 8, bits=16, syntax="1.2.840.10008.1.2.2", big=True)
dicom("mono2-single-frame.dcm", 16, 16, 1)
dicom("rgb-interleaved.dcm", 16, 16, 8, comps=3, photometric="RGB")
dicom("rgb-planar.dcm", 16, 16, 4, comps=3, photometric="RGB", planar=1, pixels=b"".join(bytes(voxel(x, y, z, 16, 16, 4, 3)[c] for y in range(16) for x in range(16)) for z in range(4) for c in range(3)))
planes = lambda z: [bytes(voxel(x, y, z, 16, 16, 4, 1)[0] for y in range(16) for x in range(16)) for _ in (0,)]
dicom("rle-8bit.dcm", 16, 16, 4, syntax="1.2.840.10008.1.2.5", encapsulated=([], [rle_frame(planes(z)) for z in range(4)]))
dicom("rle-8bit-bot.dcm", 16, 16, 4, syntax="1.2.840.10008.1.2.5", encapsulated=([0, 0, 0, 0], [rle_frame(planes(z)) for z in range(4)]))
dicom("rle-rgb.dcm", 16, 16, 2, comps=3, photometric="RGB", syntax="1.2.840.10008.1.2.5", encapsulated=([], [rle_frame([bytes(voxel(x, y, z, 16, 16, 2, 3)[c] for y in range(16) for x in range(16)) for c in range(3)]) for z in range(2)]))
dicom("rle-16bit.dcm", 16, 16, 2, bits=16, syntax="1.2.840.10008.1.2.5", encapsulated=([], [rle_frame([bytes((voxel(x, y, z, 16, 16, 2, 1)[0] * 257) >> 8 for y in range(16) for x in range(16)), bytes((voxel(x, y, z, 16, 16, 2, 1)[0] * 257) & 255 for y in range(16) for x in range(16))]) for z in range(2)]))
dicom("jpeg-unsupported.dcm", 16, 16, 1, syntax="1.2.840.10008.1.2.4.52", encapsulated=([], [b"\xff\xd8\xff\xd9"]))
dicom("unknown-syntax.dcm", 16, 16, 1, syntax="1.2.3.4.5")
dicom("unknown-photometric.dcm", 16, 16, 2, photometric="HSV")
dicom("deflated.dcm", 16, 16, 1, syntax="1.2.840.10008.1.2.1.99")
dicom("no-prefix.dcm", 16, 16, 1, no_prefix=True)
dicom("no-pixels.dcm", 16, 16, 1, omit=("nopixels",))
dicom("bits-12-unsigned.dcm", 16, 16, 4, bits=16, stored=12)
dicom("bits-32-float.dcm", 8, 8, 2, bits=32, pixels=b"".join(struct.pack("<f", float(v)) for v in vol_bytes(8, 8, 2, 1)), element7fe0="OF", syntax="1.2.840.10008.1.2.1")

# Worlds.
u = lambda *names: " ".join(f'"vol/{n}"' for n in names) if len(names) == 1 else "[ " + " ".join(f'"vol/{n}"' for n in names) + " ]"
worlds["tex3d-nrrd-encodings.wrl"] = world3d(tex3d_grid([it3(u(n)) for n in ("u8-raw.nrrd", "u8-ascii.nrrd", "u8-hex.nrrd", "u8-gzip.nrrd", "la-raw.nrrd", "rgb-raw.nrrd", "rgba-raw.nrrd", "rgb-gzip.nrrd", "tiny-u8.nrrd", "wide-u8.nrrd", "u8-gz-alias.nrrd", "u8-upper.nrrd")], cols=4))
worlds["tex3d-nrrd-shapes.wrl"] = world3d(tex3d_grid([it3(u(n)) for n in ("flat-2d.nrrd", "line-1d.nrrd", "u8-trailing.nrrd", "u8-crlf-field.nrrd", "zero-size.nrrd", "comp5.nrrd", "no-encoding.nrrd", "no-type.nrrd")], cols=4))
worlds["tex3d-nrrd-types.wrl"] = world3d(tex3d_grid([it3(u(f"type-{n}.nrrd")) for n in ("short-le", "ushort-be", "int-le", "uint-be", "float-le", "double-be", "schar", "int8t", "short-ascii", "float-ascii", "int-ascii", "double-ascii")], cols=4), tex3d_item(it3(u("type-float-unit.nrrd")), 5.5, -3.2))
bad = [it3(u(n)) for n in ("bad-type.nrrd", "bad-encoding.nrrd", "bad-dimension.nrrd", "bad-dimension-none.nrrd", "bad-sizes.nrrd", "bad-endian.nrrd", "short-raw.nrrd", "short-gzip.nrrd", "bad-gzip.nrrd", "bad-hex.nrrd", "empty-hex.nrrd", "too-big.nrrd", "not-a-volume.nrrd", "tiny-file.nrrd", "nrrd-version-only.nrrd", "nrrd-crlf.nrrd", "no-sizes.nrrd")]
bad += [it3(u("bad-type.nrrd", "missing.nrrd", "tiny-u8.nrrd")), it3(u("not-a-volume.nrrd", "u8-raw.nrrd")), it3('[ "" "vol/u8-gzip.nrrd" ]')]
worlds["tex3d-nrrd-errors.wrl"] = world3d(tex3d_grid(bad[:20], cols=5, dx=2.5, dy=2.2, y0=4.4, x0=-5))
dc = [it3(u(n)) for n in ("mono2-explicit.dcm", "mono2-implicit.dcm", "mono2-big-endian.dcm", "mono1.dcm", "mono2-16bit.dcm", "mono2-16bit-signed.dcm", "mono2-16bit-big-endian.dcm", "mono2-single-frame.dcm", "rgb-interleaved.dcm", "rgb-planar.dcm", "rle-8bit.dcm", "rle-8bit-bot.dcm", "rle-rgb.dcm", "rle-16bit.dcm", "bits-12-unsigned.dcm", "bits-32-float.dcm")]
worlds["tex3d-dicom.wrl"] = world3d(tex3d_grid(dc, cols=4, dy=2.4, y0=3.6))
de = [it3(u(n)) for n in ("jpeg-unsupported.dcm", "unknown-syntax.dcm", "unknown-photometric.dcm", "deflated.dcm", "no-prefix.dcm", "no-pixels.dcm")]
de += [it3(u("jpeg-unsupported.dcm", "mono2-explicit.dcm")), it3(u("not-a-volume.nrrd", "mono2-implicit.dcm"))]
worlds["tex3d-dicom-errors.wrl"] = world3d(tex3d_grid(de, cols=4, dy=2.8))

# Texture parameters: repeat, TextureProperties, mipmaps (16 texels and more), nearest below 16, 
# transparency, outputs, several users of one node, MultiTexture, a PhysicalMaterial, layers.
par = [
    it3(u("u8-raw.nrrd"), "repeatS TRUE repeatT TRUE repeatR TRUE "),
    it3(u("tiny-u8.nrrd"), "repeatS TRUE repeatT TRUE repeatR TRUE "),
    it3(u("tiny-u8.nrrd"), 'textureProperties TextureProperties { boundaryModeS "MIRRORED_REPEAT" boundaryModeT "REPEAT" boundaryModeR "CLAMP_TO_EDGE" magnificationFilter "AVG_PIXEL" minificationFilter "AVG_PIXEL" } '),
    it3(u("u8-raw.nrrd"), 'textureProperties TextureProperties { generateMipMaps TRUE minificationFilter "NEAREST_PIXEL_NEAREST_MIPMAP" magnificationFilter "NEAREST_PIXEL" } '),
    it3(u("rgba-raw.nrrd")),
    it3(u("la-raw.nrrd")),
    it3(u("rgb-raw.nrrd"), "repeatS TRUE "),
    it3(u("wide-u8.nrrd"), "repeatR TRUE "),
]
rep = tex3d_grid(par, cols=4)
# A PhysicalMaterial and a MultiTexture, an UnlitMaterial, USE of one node by two shapes.
extra = [
    tex3d_item(it3(u("rgb-raw.nrrd")), -4.5, -3.2, mat="PhysicalMaterial { baseColor 1 1 1 metallic 0.2 roughness 0.4 }"),
    tex3d_item(it3(u("rgb-raw.nrrd")), -1.5, -3.2, mat="UnlitMaterial { emissiveColor 1 1 1 }"),
    "Transform { translation 1.5 -3.2 0 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture MultiTexture { mode [ \"MODULATE\" \"ADD\" ] texture [ ImageTexture3D { url \"vol/rgb-raw.nrrd\" } ImageTexture3D { url \"vol/u8-raw.nrrd\" } ] } textureTransform MultiTextureTransform { textureTransform [ TextureTransform3D { translation 0.5 0.5 0.5 scale 0.5 0.5 0.5 } TextureTransform3D { translation 0.5 0.5 0.5 scale 0.5 0.5 0.5 } ] } } geometry Sphere { radius 0.9 } } }",
    "DEF SH ImageTexture3D { url \"vol/rgb-raw.nrrd\" }",
    "Transform { translation 4.5 -3.2 0 children [ Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture USE SH " + TT3 + " } geometry Sphere { radius 0.9 } } Transform { translation 0 0.5 0 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture USE SH textureTransform TextureTransform3D { translation 0.5 0.5 0.5 scale 0.3 0.3 0.3 } } geometry Box { size 0.5 0.5 0.5 } } } ] }",
]
worlds["tex3d-params.wrl"] = world3d(rep, *extra)
worlds["tex3d-layers.wrl"] = world3d(
    'LayerSet { order [ 0 1 2 ] layers [ Layer { children [ Viewpoint { position 0 0 6 } ' + tex3d_item(it3(u("rgb-raw.nrrd")), -1.2, 0) + ' ] } '
    'Layer { viewport Viewport { clipBoundary [ 0.5 1 0.5 1 ] } children [ Viewpoint { position 0 0 6 } ' + tex3d_item(it3(u("u8-gzip.nrrd")), 1.2, 0.8) + ' ] } '
    'LayoutLayer { layout Layout { size [ 0.4 0.4 ] sizeUnits [ "FRACTION" "FRACTION" ] align [ "LEFT" "BOTTOM" ] } children [ Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture ImageTexture3D { url "vol/rgba-raw.nrrd" } } geometry Rectangle2D { size 0.4 0.4 } } ] } ] }', vp="Viewpoint { position 0 0 15 }")

# Relative urls: an Inline in a subdirectory loads vol urls relative to the inlined world.
os.makedirs(os.path.join(OUT, "tex3d-inline"), exist_ok=True)
nrrd_inner = os.path.join(OUT, "tex3d-inline")
worlds["tex3d-inline/inner.wrl"] = world3d(tex3d_item('ImageTexture3D { url [ "missing.nrrd" "../vol/rgb-raw.nrrd" ] }', 0, 0), tex3d_item('ImageTexture3D { url "../vol/u8-raw.nrrd" }', 2.5, 0))
worlds["tex3d-inline.wrl"] = world3d("Inline { url \"tex3d-inline/inner.wrl\" }", tex3d_item(it3(u("u8-hex.nrrd")), -3, -3))

rt3 = HEAD3D + """Viewpoint { position 0 0 15 }
NavigationInfo { headlight TRUE }
""" + "\n".join([
    tex3d_item("DEF A ImageTexture3D { url \"vol/u8-raw.nrrd\" }", -3, 2),
    tex3d_item("DEF B ImageTexture3D { url [ \"vol/not-a-volume.nrrd\" \"vol/rgb-raw.nrrd\" ] }", 0, 2),
    tex3d_item("DEF C ImageTexture3D { url \"vol/u8-gzip.nrrd\" load FALSE }", 3, 2),
    tex3d_item("DEF D ImageTexture3D { url \"vol/mono2-explicit.dcm\" }", -3, -1.5),
    tex3d_item("DEF E ImageTexture3D { }", 0, -1.5),
    "DEF LS LoadSensor { timeOut 0 children [ USE A USE B USE C USE D USE E ] }",
]) + """
DEF G Group { children [ DEF GS Transform { translation 3 -1.5 0 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } texture DEF F ImageTexture3D { url "vol/rgba-raw.nrrd" } textureTransform TextureTransform3D { translation 0.5 0.5 0.5 scale 0.5 0.5 0.5 } } geometry Sphere { radius 0.9 } } } ] }
DEF K KeySensor { }
DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOnly SFInt32 on_wa
  inputOnly SFInt32 on_ha
  inputOnly SFInt32 on_da
  inputOnly SFInt32 on_ca
  inputOnly SFInt32 on_cb
  inputOnly SFInt32 on_ce
  inputOnly SFBool on_loaded
  inputOnly SFTime on_progress
  inputOutput SFString log ""
  initializeOnly SFNode a USE A
  initializeOnly SFNode b USE B
  initializeOnly SFNode c USE C
  initializeOnly SFNode d USE D
  initializeOnly SFNode e USE E
  initializeOnly SFNode f USE F
  initializeOnly SFNode g USE G
  initializeOnly SFNode gs USE GS
  url "javascript:
var made = null;
function on_wa (v) { log = log + 'Wa' + v + ' '; }
function on_ha (v) { log = log + 'Ha' + v + ' '; }
function on_da (v) { log = log + 'Da' + v + ' '; }
function on_ca (v) { log = log + 'Ca' + v + ' '; }
function on_cb (v) { log = log + 'Cb' + v + ' '; }
function on_ce (v) { log = log + 'Ce' + v + ' '; }
function on_loaded (v) { log = log + 'L' + v + ' '; }
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') a .url = new MFString ('vol/rgb-raw.nrrd');
   if (k == 'b') a .url = new MFString ('vol/missing.nrrd');
   if (k == 'c') a .url = new MFString ('vol/missing.nrrd', 'vol/tiny-u8.nrrd');
   if (k == 'd') c .load = true;
   if (k == 'e') c .load = false;
   if (k == 'f') e .url = new MFString ('vol/rle-8bit.dcm');
   if (k == 'g') d .url = new MFString ('vol/bad-type.nrrd');
   if (k == 'h') b .url = new MFString ();
   if (k == 'i') b .url = new MFString ('vol/u8-ascii.nrrd');
   if (k == 'j') { made = Browser .currentScene .createNode ('ImageTexture3D'); made .url = new MFString ('vol/la-raw.nrrd'); gs .children[0] .appearance .texture = made; }
   if (k == 'k') g .removeChildren = new MFNode (gs);
   if (k == 'l') g .addChildren = new MFNode (gs);
   if (k == 'm') f .url = new MFString ('vol/wide-u8.nrrd');
}
"
}
ROUTE K.keyPress TO R.on_key
ROUTE A.width TO R.on_wa
ROUTE A.height TO R.on_ha
ROUTE A.depth TO R.on_da
ROUTE A.colorDepth TO R.on_ca
ROUTE B.colorDepth TO R.on_cb
ROUTE E.colorDepth TO R.on_ce
ROUTE LS.isLoaded TO R.on_loaded
"""
worlds["tex3d-runtime.wrl"] = rt3
scripts["tex3d-runtime.json"] = runtime_json(190, list("abcdefghijklm"), step=10, start=16)

# ---------------------------------------------------------------------------------------------
# InstancedShape (component X_ITE): one geometry drawn with a matrix per instance; the longest of
# translations / rotations / scales / scaleOrientations / centers sets the count and a shorter field
# repeats its last value; isEnabled () needs instances and a geometry.
def ins(fields, geom="Box { size 0.5 0.5 0.5 }", mat="Material { diffuseColor 0.9 0.5 0.2 }", extra="", tx=""):
    return f"InstancedShape {{ {fields} {extra} appearance Appearance {{ material {mat} {tx} }} geometry {geom} }}"

LIT = "DirectionalLight { direction -0.4 -0.6 -0.7 intensity 0.9 }\nPointLight { location 2.5 2 3 intensity 1.5 radius 20 attenuation 0 0 1 }"
VP = "Viewpoint { position 0 0 7 }"
T9 = "translations [ -2 1.5 0, 0 1.5 0, 2 1.5 0, -2 0 0, 0 0 0, 2 0 0, -2 -1.5 0, 0 -1.5 0, 2 -1.5 0 ]"
worlds["inst-transforms.wrl"] = world(BG, LIT, ins(T9 + " rotations [ 0 0 1 0.3, 0 1 0 0.8, 1 0 0 1.2 ] scales [ 1 1 1, 1.4 0.6 1, 0.5 1.5 0.7 ]"), vp=VP)
worlds["inst-scale-orientation.wrl"] = world(BG, LIT, ins("translations [ -1.5 0.8 0, 0 0.8 0, 1.5 0.8 0 ] scales [ 1.6 0.5 1 ] scaleOrientations [ 0 0 1 0.6, 0 0 1 -0.6, 0 1 0 0.5 ] rotations [ 0 0 1 0 ]", "Box { size 0.6 0.6 0.6 }")
                                  + ins("translations [ -1.5 -1.0 0, 0 -1.0 0, 1.5 -1.0 0 ] rotations [ 0 0 1 0.7 ] centers [ 0.3 0 0, 0 0.3 0, -0.3 -0.3 0 ] scales [ 1 1 1, 1.5 1.5 1.5 ]", "Cone { bottomRadius 0.35 height 0.8 }"), vp=VP)
worlds["inst-lengths.wrl"] = world(BG, LIT,
    ins("translations [ -2 1.4 0 ] rotations [ 0 0 1 0, 0 0 1 0.5, 0 0 1 1, 0 0 1 1.5 ]", "Box { size 0.5 0.5 0.5 }"),
    ins("translations [ -2 0 0, 0 0 0, 2 0 0 ] scales [ 0.7 0.7 0.7, 1.2 1.2 1.2 ]", "Sphere { radius 0.4 }", "Material { diffuseColor 0.3 0.8 0.4 }"),
    ins("scales [ 1 1 1, 1.3 1.3 1.3, 1.6 1.6 1.6 ] translations [ -2 -1.4 0 ]", "Cylinder { radius 0.25 height 0.8 }", "Material { diffuseColor 0.4 0.4 1 }"),
    ins("", "Box { size 1 1 1 }", "Material { diffuseColor 1 0 0 }"),
    vp=VP)
geoms = [
    "Box { size 0.6 0.6 0.6 }", "Sphere { radius 0.35 }", "Cone { bottomRadius 0.3 height 0.7 }", "Cylinder { radius 0.25 height 0.7 }",
    "Text { string [ \"Ab\" ] fontStyle FontStyle { size 0.5 } }",
    "IndexedFaceSet { solid FALSE creaseAngle 1 coord Coordinate { point [ -0.4 -0.4 0, 0.4 -0.4 0, 0 0.4 0.3 ] } coordIndex [ 0 1 2 -1 ] color Color { color [ 1 0 0, 0 1 0, 0 0 1 ] } }",
    "IndexedLineSet { coord Coordinate { point [ -0.4 -0.4 0, 0.4 -0.4 0, 0 0.4 0 ] } coordIndex [ 0 1 2 0 -1 ] }",
    "PointSet { coord Coordinate { point [ -0.3 -0.3 0, 0.3 -0.3 0, 0 0.3 0, 0 0 0.3 ] } }",
]
parts = []
for i, g in enumerate(geoms):
    x, y = (i % 4 - 1.5) * 1.7, 1.0 - (i // 4) * 2.0
    parts.append(f"Transform {{ translation {x} {y} 0 children [ " + ins("translations [ -0.35 0.35 0, 0.35 -0.35 0 ] rotations [ 0 1 0 0.6, 1 0 0 -0.4 ] scales [ 0.8 0.8 0.8, 1 1 1 ]", g) + " ] }")
worlds["inst-geometries.wrl"] = world(BG, LIT, *parts, vp=VP)
worlds["inst-pbr-ibl.wrl"] = world(BG, ENV, LIGHTS,
    ins(T9 + " scales [ 1 1 1, 0.8 0.8 0.8, 1.2 1.2 1.2 ]", "Sphere { radius 0.4 }", pm("ClearcoatMaterialExtension { clearcoat 1 }", "baseColor 0.8 0.3 0.2", "metallic 1 roughness 0.3")),
    vp=VP)
worlds["inst-transparent-textured.wrl"] = world(BG, LIT, BEHIND,
    ins("translations [ -1.6 0.8 0, 0 0.8 0, 1.6 0.8 0 ] rotations [ 0 1 0 0.4, 0 1 0 0.9, 0 1 0 1.4 ]", "Box { size 0.9 0.9 0.9 }", "Material { diffuseColor 1 0.4 0.4 transparency 0.5 }"),
    ins("translations [ -1.6 -1.0 0, 0 -1.0 0, 1.6 -1.0 0 ] rotations [ 0 0 1 0.3, 0 0 1 0.6, 0 0 1 0.9 ] scales [ 1 1 1, 1.2 1.2 1.2 ]", "Box { size 0.9 0.9 0.9 }", "Material { diffuseColor 1 1 1 }", tx=f'texture ImageTexture {{ url "{CHECKER}" }} textureTransform TextureTransform {{ scale 2 2 }}'),
    vp=VP)
SH = "DirectionalLight { direction 0 -1 -0.3 intensity 1 shadows TRUE shadowIntensity 0.7 shadowMapSize 1024 }"
worlds["inst-shadows.wrl"] = world(BG, SH,
    "Transform { translation 0 -1.5 0 children Shape { appearance Appearance { material Material { diffuseColor 0.8 0.8 0.8 } } geometry Box { size 8 0.1 6 } } }",
    ins("translations [ -2 0 0, 0 0.2 0.5, 2 0 -0.5 ] rotations [ 0 1 0 0.5 ] scales [ 1 1.4 1, 1 1 1 ]", "Box { size 0.9 0.9 0.9 }", extra="castShadow TRUE"),
    ins("translations [ -1 1.0 -1.5, 1 1.2 -1.5 ]", "Sphere { radius 0.4 }", "Material { diffuseColor 0.3 0.7 0.9 }", extra="castShadow FALSE"),
    vp="Viewpoint { position 0 3 7 orientation 1 0 0 -0.35 }")
worlds["inst-twosided-layers.wrl"] = world(BG, LIT,
    ins("translations [ -1.5 0.8 0, 1.5 0.8 0 ] rotations [ 0 1 0 2.4 ]", "IndexedFaceSet { solid FALSE coord Coordinate { point [ -0.6 -0.6 0, 0.6 -0.6 0, 0.6 0.6 0, -0.6 0.6 0 ] } coordIndex [ 0 1 2 3 -1 ] }",
        "Material { diffuseColor 0.2 0.8 0.2 }"),
    'LayerSet { order [ 0 1 ] layers [ Layer { children [ Viewpoint { position 0 0 7 } ' + LIT.replace("\n", " ") + " "
    + ins("translations [ -1.5 -1 0, 0 -1 0, 1.5 -1 0 ] scales [ 1 0.6 1, 1 1 1, 1.3 1.3 1.3 ]", "Cone { bottomRadius 0.4 height 0.8 }", "Material { diffuseColor 0.9 0.8 0.2 }") + " ] } ] }",
    vp=VP)

inst_rt = HEAD + VP + "\n" + "\n".join([BG, LIT]) + """
DEF G1 Group { children [
  DEF I1 InstancedShape {
    translations [ -2 1 0, 0 1 0, 2 1 0 ]
    appearance DEF AP Appearance { material DEF MA Material { diffuseColor 0.9 0.5 0.2 } }
    geometry DEF BX Box { size 0.6 0.6 0.6 }
  }
] }
DEF I2 InstancedShape { translations [ -2 -1 0 ] appearance Appearance { material Material { diffuseColor 0.3 0.8 0.4 } } }
DEF I3 InstancedShape { appearance Appearance { material Material { diffuseColor 0.4 0.4 1 } } geometry Sphere { radius 0.4 } }
DEF K KeySensor { }
DEF R Script {
  directOutput TRUE
  inputOnly SFString on_key
  inputOutput SFString log ""
  initializeOnly SFNode i1 USE I1
  initializeOnly SFNode i2 USE I2
  initializeOnly SFNode i3 USE I3
  initializeOnly SFNode g1 USE G1
  initializeOnly SFNode bx USE BX
  initializeOnly SFNode ma USE MA
  url "javascript:
var made = null;
function on_key (k)
{
   log = log + 'K' + k + ' ';
   if (k == 'a') i1 .translations = new MFVec3f (new SFVec3f (-2, 1, 0), new SFVec3f (0, 1, 0), new SFVec3f (2, 1, 0), new SFVec3f (0, 2, 0));
   if (k == 'b') i1 .rotations = new MFRotation (new SFRotation (0, 0, 1, 0.5), new SFRotation (0, 0, 1, 1));
   if (k == 'c') i1 .scales = new MFVec3f (new SFVec3f (1, 1, 1), new SFVec3f (1.5, 0.5, 1));
   if (k == 'd') i1 .scaleOrientations = new MFRotation (new SFRotation (0, 0, 1, 0.8));
   if (k == 'e') i1 .centers = new MFVec3f (new SFVec3f (0.4, 0, 0));
   if (k == 'f') i2 .geometry = Browser .currentScene .createNode ('Cone');
   if (k == 'g') i3 .translations = new MFVec3f (new SFVec3f (0, -1, 0), new SFVec3f (2, -1, 0));
   if (k == 'h') i1 .translations = new MFVec3f ();
   if (k == 'i') i1 .translations = new MFVec3f (new SFVec3f (-1, 0, 0));
   if (k == 'j') ma .diffuseColor = new SFColor (0.2, 0.9, 0.9);
   if (k == 'k') bx .size = new SFVec3f (1, 0.3, 0.3);
   if (k == 'l') i1 .appearance = null;
   if (k == 'm') g1 .removeChildren = new MFNode (i1);
   if (k == 'n') g1 .addChildren = new MFNode (i1);
   if (k == 'o') { made = Browser .currentScene .createNode ('InstancedShape'); made .translations = new MFVec3f (new SFVec3f (1, 0, 0), new SFVec3f (-1, 0, 0)); made .geometry = Browser .currentScene .createNode ('Sphere'); g1 .addChildren = new MFNode (made); }
   if (k == 'p') made .scales = new MFVec3f (new SFVec3f (0.5, 0.5, 0.5), new SFVec3f (1.5, 1.5, 1.5));
   if (k == 'q') g1 .removeChildren = new MFNode (made);
   if (k == 'r') i1 .visible = false;
   if (k == 's') i1 .visible = true;
}
"
}
ROUTE K.keyPress TO R.on_key
"""
worlds["inst-runtime.wrl"] = inst_rt
scripts["inst-runtime.json"] = runtime_json(230, list("abcdefghijklmnopqrs"), step=10, start=16)

for name, text in worlds.items():
    open(os.path.join(OUT, name), "w").write(text)
for name, js in scripts.items():
    open(os.path.join(OUT, name), "w").write(json.dumps(js, indent=1))
print(f"{len(worlds)} worlds, {len(scripts)} scripts -> {OUT}")
