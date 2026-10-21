#!/usr/bin/env python3
"""REA batch 11 lane b11-4-fog-screen: LocalFog / Fog on normal geometry and on volume draws as stepped worlds + key scripts
(oracle/synthetic-rea-b11-fog). 512x512, dt = 1/30, KeySensor driven like gen_rea_b11_ortho.py. Cases are added to
oracle/rea-batch10-baseline/cases.txt by `rea_b10.py stepped record`.

  python3 -I oracle/tools/gen_rea_b11_fog.py
"""
import json, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b11-fog"))
os.makedirs(root, exist_ok=True)
HEAD = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT VolumeRendering:2\n"
BG = "Viewpoint { position 0 0 8 }\nBackground { skyColor 0.12 0.14 0.2 }\nNavigationInfo { type \"EXAMINE\" transitionType [ \"TELEPORT\" ] }\n"
IMG = ("[ 4 4 4 4 0x3C3C3CFF 0x783C3C5A 0xB43C3CFF 0xF03C3C5A 0x3C783C5A 0x78783CFF 0xB4783C5A 0xF0783CFF 0x3CB43CFF 0x78B43C5A 0xB4B43CFF 0xF0B43C5A "
       "0x3CF03C5A 0x78F03CFF 0xB4F03C5A 0xF0F03CFF 0x3C3C785A 0x783C78FF 0xB43C785A 0xF03C78FF 0x3C7878FF 0x7878785A 0xB47878FF 0xF078785A "
       "0x3CB4785A 0x78B478FF 0xB4B4785A 0xF0B478FF 0x3CF078FF 0x78F0785A 0xB4F078FF 0xF0F0785A 0x3C3CB4FF 0x783CB45A 0xB43CB4FF 0xF03CB45A "
       "0x3C78B45A 0x7878B4FF 0xB478B45A 0xF078B4FF 0x3CB4B4FF 0x78B4B45A 0xB4B4B4FF 0xF0B4B45A 0x3CF0B45A 0x78F0B4FF 0xB4F0B45A 0xF0F0B4FF "
       "0x3C3CF05A 0x783CF0FF 0xB43CF05A 0xF03CF0FF 0x3C78F0FF 0x7878F05A 0xB478F0FF 0xF078F05A 0x3CB4F05A 0x78B4F0FF 0xB4B4F05A 0xF0B4F0FF "
       "0x3CF0F0FF 0x78F0F05A 0xB4F0F0FF 0xF0F0F05A ]")
TEX = f"PixelTexture3D {{ image {IMG} }}"
SEGTEX = "PixelTexture3D { image [ 2 2 2 1 0 1 1 2 0 1 2 2 ] }"
SOLID = lambda c: f"Appearance {{ material Material {{ diffuseColor {c} }} }}"
SPHERE = lambda x, y, z, c: f"Transform {{ translation {x} {y} {z} children Shape {{ appearance {SOLID(c)} geometry Sphere {{ radius 0.8 }} }} }}\n"
BOX = lambda x, y, z, c: f"Transform {{ translation {x} {y} {z} children Shape {{ appearance {SOLID(c)} geometry Box {{ size 1.2 1.2 1.2 }} }} }}\n"
VOL = lambda x, y, z, s="": f"Transform {{ translation {x} {y} {z} children VolumeData {{ dimensions 2 2 2 {s} voxels {TEX} }} }}\n"
ISO = lambda x, y, z, s: f"Transform {{ translation {x} {y} {z} children IsoSurfaceVolumeData {{ dimensions 2 2 2 contourStepSize 0 surfaceValues [ 0.4 ] renderStyle [ {s} ] voxels {TEX} }} }}\n"
SEG = lambda x, y, z: (f"Transform {{ translation {x} {y} {z} children SegmentedVolumeData {{ dimensions 2 2 2 segmentIdentifiers {SEGTEX} "
                       f"renderStyle [ ToneMappedVolumeStyle {{ }} OpacityMapVolumeStyle {{ }} ToneMappedVolumeStyle {{ coolColor 1 0 0 1 }} ] voxels {TEX} }} }}\n")

def write(name, text):
    open(os.path.join(root, name + ".wrl"), "w").write(text)

def keyscript(name, letters, step=8, start=6, tail=6, after=3):
    ev = [{"frame": start + step * i, "type": "key", "key": c} for i, c in enumerate(letters)]
    frames = start + step * len(letters) + tail
    capture = [start - 2] + [start + step * i + after for i in range(len(letters))]
    d = {"settle": True, "size": [512, 512], "t0": 946684800, "dt": 1 / 30, "frames": frames, "capture": capture,
         "dump": sorted(set(capture + [frames - 1])), "events": ev, "buildEachFrame": True}
    json.dump(d, open(os.path.join(root, name + ".json"), "w"), indent=1)

def script_node(names, body):
    refs = "".join(f"  initializeOnly SFNode {n.lower()} USE {n}\n" for n in names)
    return ("DEF K KeySensor { }\nDEF R Script {\n  directOutput TRUE\n  inputOnly SFString on_key\n  inputOutput SFString log \"\"\n" + refs
            + "  url \"javascript:\nfunction on_key (k)\n{\n   log = log + 'K' + k + ' ';\n" + body + "}\n\"\n}\nROUTE K.keyPress TO R.on_key\n")

FOGKEYS = ("   if (k == 'a') lf .color = new SFColor (0.2, 0.5, 0.9);\n   if (k == 'b') lf .visibilityRange = 11;\n   if (k == 'c') lf .fogType = 'EXPONENTIAL';\n"
           "   if (k == 'd') lf .enabled = false;\n   if (k == 'e') lf .enabled = true;\n   if (k == 'f') lf .visibilityStart = 2;\n")

# Normal geometry in a LocalFog (and outside it).
write("lf-geom", HEAD + BG + "Group { children [ DEF LF LocalFog { color 0.8 0.4 0.3 visibilityRange 16 }\n" + SPHERE(-1.5, 0, -1, "0.3 0.5 0.9") + SPHERE(1.5, 0, -5, "0.9 0.6 0.2") + BOX(0, -2, -2, "0.4 0.9 0.4") + "] }\n"
      + SPHERE(0, 2.2, -3, "0.9 0.9 0.9") + script_node(["LF"], FOGKEYS))
keyscript("lf-geom", "abcdef")
# The other volume nodes in a LocalFog, with styles and an opaque sphere (draw order against the fogged volumes).
write("lf-vol-types", HEAD + BG + "Group { children [ DEF LF LocalFog { color 0.8 0.4 0.3 visibilityRange 16 }\n" + ISO(-2.2, 0, -1, "ShadedVolumeStyle { lighting TRUE }") + SEG(0.3, 0, -3)
      + VOL(2.4, 0, -5, "renderStyle ToneMappedVolumeStyle { }") + SPHERE(0, -2, -2, "0.9 0.6 0.2") + "] }\nDirectionalLight { direction 0 -1 -1 }\n" + script_node(["LF"], FOGKEYS))
keyscript("lf-vol-types", "abcdef")
# A bound Fog fogs a volume outside any LocalFog; a LocalFog replaces it (nested LocalFog replaces the outer one).
write("lf-vol-global", HEAD + BG + "DEF GF Fog { color 0.3 0.7 0.4 visibilityRange 16 }\n" + VOL(-2.4, 1.6, -2) + SPHERE(-2.4, -1.6, -2, "0.9 0.6 0.2")
      + "Group { children [ LocalFog { color 0.8 0.4 0.3 visibilityRange 14 }\n" + VOL(0, 1.6, -4) + SPHERE(0, -1.6, -4, "0.4 0.5 0.9") + " ] }\n"
      + "Group { children [ DEF LF LocalFog { color 0.9 0.9 0.2 visibilityRange 16 }\n" + VOL(2.4, 1.6, -2)
      + " Group { children [ LocalFog { color 0.2 0.3 0.9 visibilityRange 9 fogType \"EXPONENTIAL\" }\n" + VOL(2.4, -1.6, -2) + " ] } ] }\n"
      + script_node(["GF", "LF"], "   if (k == 'a') gf .color = new SFColor (0.9, 0.2, 0.2);\n   if (k == 'b') gf .visibilityRange = 11;\n   if (k == 'c') gf .fogType = 'EXPONENTIAL';\n   if (k == 'd') lf .enabled = false;\n   if (k == 'e') lf .enabled = true;\n"))
keyscript("lf-vol-global", "abcde")
