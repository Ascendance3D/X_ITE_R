#!/usr/bin/env python3
"""Author the X8 synthetic scenes (Text/FontStyle, Fog, Background textures) into
oracle/synthetic-x8/. All content is original: generated scenes plus generated PNG/JPEG faces.
Run: python3 -I oracle/tools/gen_synthetic_x8.py oracle/synthetic-x8
"""
import sys, os
from PIL import Image, ImageDraw, ImageFont

out = sys.argv[1]
os.makedirs(out, exist_ok=True)

def write(name, body):
    with open(os.path.join(out, name), "w") as f:
        f.write("#VRML V2.0 utf8\n" + body)

# ---------------------------------------------------------------- images
def face(name, color, label, size=(128, 128), fmt="PNG", alpha=False):
    mode = "RGBA" if alpha else "RGB"
    img = Image.new(mode, size, color + ((255,) if alpha else ()))
    d = ImageDraw.Draw(img)
    w, h = size
    d.rectangle([0, 0, w - 1, h - 1], outline=(255, 255, 255), width=4)
    d.line([0, 0, w, h], fill=(0, 0, 0), width=3)
    d.polygon([(w // 2, 8), (w // 2 - 14, 30), (w // 2 + 14, 30)], fill=(255, 255, 0))  # up arrow marks the top edge
    font = ImageFont.load_default(size=max(16, h // 3))
    d.text((w // 3, h // 2 - 10), label, fill=(255, 255, 255), font=font)
    if alpha:
        # carve a transparent hole so the sky shows through
        px = img.load()
        for y in range(h // 4, h // 2):
            for x in range(w // 4, 3 * w // 4):
                px[x, y] = (0, 0, 0, 0)
    img.save(os.path.join(out, name), fmt)

face("bg-front.png", (200, 40, 40), "F")
face("bg-back.png", (40, 160, 60), "B")
face("bg-left.png", (50, 80, 210), "L")
face("bg-right.png", (210, 180, 40), "R")
face("bg-top.png", (160, 60, 190), "T")
face("bg-bottom.png", (40, 170, 180), "D")
face("bg-npot.png", (230, 120, 30), "N", size=(100, 60))
face("bg-alpha.png", (220, 60, 140), "A", alpha=True)
face("bg-front.jpg", (60, 60, 60), "J", fmt="JPEG")
face("tex-text.png", (30, 120, 200), "X", size=(64, 64))

BASE = "Viewpoint { position 0 0 12 }\n"
MARK = """Shape { appearance Appearance { material Material { diffuseColor 1 0 0 } } geometry Box { size 0.12 0.12 0.12 } }
Shape { appearance Appearance { material Material { emissiveColor 0.35 0.35 0.35 } } geometry Box { size 18 0.012 0.012 } }
Shape { appearance Appearance { material Material { emissiveColor 0.35 0.35 0.35 } } geometry Box { size 0.012 10 0.012 } }
"""

def text(string, fs="", extra="", mat="diffuseColor 1 0.9 0.3", view=BASE, wrap=None):
    fsnode = f"fontStyle FontStyle {{ {fs} }}" if fs is not None else ""
    t = f"Shape {{ appearance Appearance {{ material Material {{ {mat} }} }} geometry Text {{ string {string} {extra} {fsnode} }} }}\n"
    return view + MARK + (wrap(t) if wrap else t)

LINES = '[ "Hello" "Wide World" "ij" ]'
VLINES = '[ "ABC" "defg" "H" ]'

# ---------------------------------------------------------------- Text
write("t-default.wrl", text('"Hello World"', fs=None))
write("t-default-multi.wrl", text(LINES, fs=None))
JUST = ["BEGIN", "FIRST", "MIDDLE", "END"]
for a in JUST:
    for b in JUST:
        write(f"t-justify-{a}-{b}.wrl", text(LINES, fs=f'size 1.1 justify [ "{a}" "{b}" ]'))
write("t-justify-single-MIDDLE.wrl", text(LINES, fs='justify "MIDDLE"'))
write("t-justify-empty.wrl", text(LINES, fs='justify []'))
write("t-justify-bad.wrl", text(LINES, fs='justify [ "BAD" "WORSE" ]'))
for fam in ["SERIF", "SANS", "TYPEWRITER"]:
    for st in ["PLAIN", "BOLD", "ITALIC", "BOLDITALIC"]:
        write(f"t-font-{fam}-{st}.wrl", text('[ "The quick fox" "AVAV To Wa 0123" ]', fs=f'family "{fam}" style "{st}" size 0.7'))
write("t-family-fallback.wrl", text('"Fallback to SANS"', fs='family [ "NoSuchFont" "SANS" ] style "BOLD"'))
write("t-family-unknown.wrl", text('"Unknown family"', fs='family "NoSuchFont"'))
write("t-family-lower.wrl", text('"lower case family"', fs='family "sans"'))
write("t-style-bad.wrl", text('"Bad style"', fs='style "BLINK"'))
write("t-size-spacing-a.wrl", text(LINES, fs='size 0.5 spacing 2'))
write("t-size-spacing-b.wrl", text(LINES, fs='size 1.5 spacing 0.7'))
write("t-size-neg.wrl", text('"Neg"', fs='size -1'))
write("t-ltr-false.wrl", text(LINES, fs='leftToRight FALSE'))
write("t-ltr-false-end.wrl", text(LINES, fs='leftToRight FALSE justify [ "END" "BEGIN" ]'))
write("t-ttb-false.wrl", text(LINES, fs='topToBottom FALSE'))
write("t-ttb-false-middle.wrl", text(LINES, fs='topToBottom FALSE justify [ "MIDDLE" "MIDDLE" ]'))
for ltr in ["TRUE", "FALSE"]:
    for ttb in ["TRUE", "FALSE"]:
        write(f"t-vertical-{ltr}-{ttb}.wrl", text(VLINES, fs=f'horizontal FALSE leftToRight {ltr} topToBottom {ttb} size 0.9'))
for j in ["MIDDLE", "END"]:
    write(f"t-vertical-{j}.wrl", text(VLINES, fs=f'horizontal FALSE justify [ "{j}" "{j}" ] size 0.9'))
write("t-vertical-length.wrl", text(VLINES, fs='horizontal FALSE size 0.9', extra='length [ 5 3 ]'))
write("t-length.wrl", text(LINES, fs='', extra='length [ 3 6 ]'))
write("t-length-middle.wrl", text(LINES, fs='justify "MIDDLE"', extra='length [ 3 12 0 ]'))
write("t-maxextent.wrl", text(LINES, fs='', extra='maxExtent 4'))
write("t-maxextent-big.wrl", text(LINES, fs='', extra='maxExtent 40'))
write("t-length-maxextent.wrl", text(LINES, fs='justify "END"', extra='length [ 3 6 ] maxExtent 4'))
write("t-empty-string.wrl", text('""', fs=''))
write("t-empty-mf.wrl", text('[ ]', fs=''))
write("t-empty-lines.wrl", text('[ "" "a" "" "b" ]', fs=''))
write("t-latin1.wrl", text('[ "Café Ünï ñ ß" "© ½ æøå" ]', fs=''))
write("t-notdef.wrl", text('[ "AЖ中B" "Ωmega" ]', fs=''))
write("t-punct.wrl", text('[ "{}[]()#$%&*@!?" "<>=+-_~^|/\\\\ ;:,.\'" ]', fs=''))
write("t-spaces.wrl", text('[ "  lead" "trail  " "a   b" ]', fs='justify "MIDDLE"'))
write("t-long.wrl", text('"The quick brown fox jumps over the lazy dog 0123456789"', fs='size 0.45 family "SANS"'))
write("t-texture.wrl", text('[ "TEXTURED" "text" ]', fs='size 1.4', mat='diffuseColor 1 1 1')
      .replace("material Material { diffuseColor 1 1 1 }", 'material Material { diffuseColor 1 1 1 } texture ImageTexture { url "tex-text.png" }'))
write("t-rotated.wrl", text(LINES, fs='justify "MIDDLE"',
      wrap=lambda t: "Transform { rotation 0.3 1 0.2 0.8 scale 1.2 0.8 1 children [\n" + t + "] }\n"))
write("t-behind.wrl", BASE + MARK + "Transform { rotation 0 1 0 3.14159 children [\n" +
      "Shape { appearance Appearance { material Material { diffuseColor 0.3 0.9 0.4 } } geometry Text { string \"Seen from behind\" fontStyle FontStyle { justify \"MIDDLE\" } } }\n] }\n")
write("t-multi-nodes.wrl", BASE + MARK +
      "Transform { translation -4 2 0 children Shape { appearance Appearance { material Material { diffuseColor 1 0.5 0.5 } } geometry Text { string \"One\" fontStyle DEF F FontStyle { family \"SANS\" style \"BOLD\" } } } }\n"
      "Transform { translation -4 -2 0 children Shape { appearance Appearance { material Material { diffuseColor 0.5 1 0.5 } } geometry Text { string \"Two\" fontStyle USE F } } }\n"
      "Transform { translation 2 0 0 children Shape { appearance Appearance { material Material { diffuseColor 0.5 0.5 1 } } geometry Text { string \"Three\" } } }\n")

# ---------------------------------------------------------------- Fog
def box(x, z, color, size="1.4 1.4 1.4", extra=""):
    return (f"Transform {{ translation {x} 0 {z} children Shape {{ appearance Appearance {{ material Material {{ diffuseColor {color} {extra} }} }} "
            f"geometry Box {{ size {size} }} }} }}\n")

def row(extra_fn=None):
    s = ""
    cols = ["1 0.3 0.3", "0.3 1 0.3", "0.3 0.4 1", "1 1 0.3", "1 0.4 1", "0.3 1 1"]
    for i in range(6):
        z = -4 * i
        s += box(-3 + (i % 2) * 6, z, cols[i], extra=extra_fn(i) if extra_fn else "")
    return s

FOGVIEW = "Viewpoint { position 0 1.5 6 orientation 1 0 0 -0.08 }\nBackground { skyColor [ 0.1 0.2 0.5 ] }\n"
def ground():
    return "Shape { appearance Appearance { material Material { diffuseColor 0.6 0.6 0.6 } } geometry ElevationGrid { xDimension 2 zDimension 2 xSpacing 40 zSpacing 60 height [ 0 0 0 0 ] } }\n".replace("Shape {", "Transform { translation -20 -1 -50 children Shape {", 1).rstrip() + " }\n"

def fogscene(name, fog, extra="", rowfn=None, view=FOGVIEW):
    write(name, view + fog + "\n" + ground() + row(rowfn) + extra)

fogscene("f-linear-20.wrl", "Fog { color 0.8 0.8 0.85 fogType \"LINEAR\" visibilityRange 20 }")
fogscene("f-linear-10.wrl", "Fog { color 0.8 0.8 0.85 visibilityRange 10 }")
fogscene("f-linear-40.wrl", "Fog { color 1 0.8 0.6 visibilityRange 40 }")
fogscene("f-linear-red.wrl", "Fog { color 1 0 0 visibilityRange 25 }")
fogscene("f-exp-10.wrl", "Fog { color 0.7 0.7 0.75 fogType \"EXPONENTIAL\" visibilityRange 10 }")
fogscene("f-exp-20.wrl", "Fog { color 0.7 0.7 0.75 fogType \"EXPONENTIAL\" visibilityRange 20 }")
fogscene("f-exp-40.wrl", "Fog { color 0.2 0.2 0.2 fogType \"EXPONENTIAL\" visibilityRange 40 }")
fogscene("f-range-0.wrl", "Fog { color 1 1 1 visibilityRange 0 }")
fogscene("f-range-neg.wrl", "Fog { color 1 1 1 visibilityRange -5 }")
fogscene("f-type-bad.wrl", "Fog { color 0.8 0.8 0.85 fogType \"FOO\" visibilityRange 20 }")
fogscene("f-type-lower.wrl", "Fog { color 0.8 0.8 0.85 fogType \"exponential\" visibilityRange 20 }")
fogscene("f-scaled.wrl", "Transform { scale 2 2 2 children Fog { color 0.8 0.8 0.85 visibilityRange 10 } }")
fogscene("f-translated.wrl", "Transform { translation 0 0 -20 children Fog { color 0.8 0.8 0.85 visibilityRange 10 } }")
fogscene("f-two-fogs.wrl", "Fog { color 0.9 0.5 0.5 visibilityRange 15 }\nFog { color 0.5 0.9 0.5 visibilityRange 30 }")
fogscene("f-unbound.wrl", "DEF F Fog { color 0.9 0.5 0.5 visibilityRange 15 }\nTimeSensor { }", extra="")
fogscene("f-lines-points.wrl", "Fog { color 0.8 0.8 0.85 visibilityRange 20 }",
         extra="Shape { geometry IndexedLineSet { coord Coordinate { point [ -4 -0.5 2, -4 -0.5 -20, 4 -0.5 2, 4 -0.5 -20 ] } coordIndex [ 0 1 -1 2 3 -1 ] } appearance Appearance { material Material { emissiveColor 1 1 0 } } }\n"
               "Shape { geometry PointSet { coord Coordinate { point [ 0 0.5 0, 0 0.5 -5, 0 0.5 -10, 0 0.5 -15 ] } } appearance Appearance { material Material { emissiveColor 0 1 0 } } }\n")
fogscene("f-transparent.wrl", "Fog { color 0.8 0.8 0.85 visibilityRange 20 }", rowfn=lambda i: "transparency 0.5")
fogscene("f-emissive.wrl", "Fog { color 0.8 0.8 0.85 visibilityRange 20 }",
         extra="Transform { translation 0 3 -8 children Shape { appearance Appearance { material Material { emissiveColor 1 1 0 } } geometry Sphere { radius 1 } } }\n"
               "Transform { translation 3 3 -16 children Shape { geometry Sphere { radius 1 } } }\n")
fogscene("f-textured.wrl", "Fog { color 0.8 0.8 0.85 visibilityRange 20 }",
         extra="Transform { translation 0 3 -10 children Shape { appearance Appearance { texture ImageTexture { url \"bg-front.png\" } } geometry Box { size 3 3 3 } } }\n")
fogscene("f-nobackground.wrl", "Fog { color 0.8 0.2 0.2 visibilityRange 20 }", view="Viewpoint { position 0 1.5 6 orientation 1 0 0 -0.08 }\n")
fogscene("f-bg-textures.wrl", "Fog { color 0.8 0.8 0.85 visibilityRange 30 }",
         view="Viewpoint { position 0 1.5 6 orientation 1 0 0 -0.08 }\nBackground { frontUrl \"bg-front.png\" backUrl \"bg-back.png\" leftUrl \"bg-left.png\" rightUrl \"bg-right.png\" topUrl \"bg-top.png\" bottomUrl \"bg-bottom.png\" }\n")
fogscene("f-inline.wrl", "Fog { color 0.8 0.8 0.85 visibilityRange 20 }", extra="Inline { url \"f-inline-part.wrl\" }\n")
write("f-inline-part.wrl", "Fog { color 1 0 0 visibilityRange 2 }\nTransform { translation 0 3 -6 children Shape { appearance Appearance { material Material { diffuseColor 1 1 1 } } geometry Cone { } } }\n")

# ---------------------------------------------------------------- Background
SKY = "skyColor [ 0.1 0.2 0.6, 0.6 0.7 0.9 ] skyAngle [ 1.5708 ] groundColor [ 0.3 0.25 0.2, 0.15 0.1 0.1 ] groundAngle [ 1.5708 ]"
ALL = 'frontUrl "bg-front.png" backUrl "bg-back.png" leftUrl "bg-left.png" rightUrl "bg-right.png" topUrl "bg-top.png" bottomUrl "bg-bottom.png"'
VIEWS = {"front": "0 1 0 0", "back": "0 1 0 3.14159", "left": "0 1 0 1.5708", "right": "0 1 0 -1.5708", "top": "1 0 0 1.5708", "bottom": "1 0 0 -1.5708"}
def bgview(orient="0 1 0 0", fov="0.785398"):
    return f"Viewpoint {{ position 0 0 0 orientation {orient} fieldOfView {fov} }}\nNavigationInfo {{ type \"NONE\" }}\n"
DEMO = "Transform { translation 0 -0.3 -4 children Shape { appearance Appearance { material Material { diffuseColor 0.9 0.9 0.9 } } geometry Sphere { radius 0.4 } } }\n"
for face_name, orient in VIEWS.items():
    write(f"b-{face_name}-only.wrl", bgview(orient) + f"Background {{ {face_name}Url \"bg-{face_name}.png\" skyColor [ 0.2 0.3 0.5 ] }}\n")
    write(f"b-{face_name}-sky.wrl", bgview(orient) + f"Background {{ {face_name}Url \"bg-{face_name}.png\" {SKY} }}\n")
write("b-all-front.wrl", bgview() + f"Background {{ {ALL} }}\n" + DEMO)
write("b-all-wide.wrl", bgview("0 1 0 0.6", "2.0") + f"Background {{ {ALL} }}\n")
write("b-all-wide-up.wrl", bgview("1 0 0 0.9", "2.2") + f"Background {{ {ALL} {SKY} }}\n")
write("b-all-sky.wrl", bgview("0 1 0 0.3", "1.5") + f"Background {{ {ALL} {SKY} }}\n" + DEMO)
write("b-all-transparency.wrl", bgview("0 1 0 0.6", "2.0") + f"Background {{ {ALL} {SKY} transparency 0.5 }}\n")
write("b-all-transparency-1.wrl", bgview("0 1 0 0.6", "2.0") + f"Background {{ {ALL} {SKY} transparency 1 }}\n")
write("b-sky-transparency.wrl", bgview("0 1 0 0.6", "2.0") + f"Background {{ {SKY} transparency 0.5 }}\n")
write("b-alpha.wrl", bgview() + f"Background {{ frontUrl \"bg-alpha.png\" {SKY} }}\n")
write("b-alpha-nosky.wrl", bgview() + "Background { frontUrl \"bg-alpha.png\" }\n")
write("b-npot.wrl", bgview("0 1 0 0.4", "1.6") + f"Background {{ frontUrl \"bg-npot.png\" backUrl \"bg-npot.png\" leftUrl \"bg-npot.png\" rightUrl \"bg-npot.png\" {SKY} }}\n")
write("b-jpeg.wrl", bgview() + "Background { frontUrl \"bg-front.jpg\" skyColor [ 0.8 0.2 0.2 ] }\n")
write("b-missing.wrl", bgview() + f"Background {{ frontUrl \"no-such-file.png\" {SKY} }}\n")
write("b-fallback.wrl", bgview() + f"Background {{ frontUrl [ \"no-such-file.png\" \"bg-front.png\" ] {SKY} }}\n")
write("b-corrupt.wrl", bgview() + "Background { frontUrl \"b-corrupt.png\" skyColor [ 0.2 0.5 0.2 ] }\n")
with open(os.path.join(out, "b-corrupt.png"), "wb") as f:
    f.write(b"\x89PNG\r\n\x1a\nthis is not a png")
write("b-sky-ground.wrl", bgview("1 0 0 0.2", "1.5") + f"Background {{ {SKY} }}\n")
write("b-nosky-nourl.wrl", bgview() + "Background { }\n")
write("b-cube-color.wrl", bgview("0 1 0 0.5", "1.8") + "Background { skyColor [ 0.3 0.6 0.3 ] }\n")
write("b-cube-textured.wrl", bgview("0 1 0 0.5", "1.8") + f"Background {{ {ALL} skyColor [ 0.3 0.6 0.3 ] }}\n")
write("b-viewpoint-rot.wrl", "Viewpoint { position 0 0 5 orientation 1 1 0 0.4 fieldOfView 1.2 }\n" + f"Background {{ {ALL} {SKY} }}\n" + DEMO)
write("b-two.wrl", bgview() + "Background { skyColor [ 1 0 0 ] }\nBackground { frontUrl \"bg-front.png\" }\n")
write("b-transformed.wrl", bgview() + f"Transform {{ rotation 0 1 0 1.5708 children Background {{ {ALL} }} }}\n")
open(os.path.join(out, "b-texture-bg.wrl"), "w").write("#X3D V3.0 utf8\nPROFILE Immersive\nViewpoint { position 0 0 0 orientation 0 1 0 0.6 fieldOfView 2.0 }\nNavigationInfo { type \"NONE\" }\n"
      "TextureBackground { frontTexture ImageTexture { url \"bg-front.png\" } backTexture ImageTexture { url \"bg-back.png\" } leftTexture ImageTexture { url \"bg-left.png\" } "
      "rightTexture ImageTexture { url \"bg-right.png\" } topTexture ImageTexture { url \"bg-top.png\" } bottomTexture ImageTexture { url \"bg-bottom.png\" } }\n")
