#!/usr/bin/env python3
"""REA batch 02: authored cube map worlds and their PNG images (no archive content).

  python3 -I oracle/tools/gen_rea_b02_cube.py oracle/synthetic-rea-b02
"""
import math, os, struct, sys, zlib

out = sys.argv[1]
os.makedirs(os.path.join(out, "cube"), exist_ok=True)

def png(path, w, h, pixel):
    raw = b"".join(b"\0" + bytes(c for x in range(w) for c in pixel(x, y)) for y in range(h))
    def chunk(t, d):
        return struct.pack(">I", len(d)) + t + d + struct.pack(">I", zlib.crc32(t + d) & 0xffffffff)
    data = b"\x89PNG\r\n\x1a\n" + chunk(b"IHDR", struct.pack(">IIBBBBB", w, h, 8, 6, 0, 0, 0)) + chunk(b"IDAT", zlib.compress(raw, 9)) + chunk(b"IEND", b"")
    open(os.path.join(out, path), "wb").write(data)

BASE = [(230, 60, 40), (40, 200, 70), (50, 80, 230), (230, 220, 40), (200, 60, 210), (40, 210, 220)]
NAMES = ["front", "back", "left", "right", "top", "bottom"]
for (r, g, b), name in zip(BASE, NAMES):
    # A face: its color with a diagonal gradient, a dark marker in the upper-left quarter.
    def face(x, y, r=r, g=g, b=b):
        k = 0.55 + 0.45 * (x + y) / 62
        if x < 10 and y < 6:
            return (20, 20, 20, 255)
        return (int(r * k), int(g * k), int(b * k), 255)
    png(f"cube/{name}.png", 32, 32, face)
png("cube/odd.png", 32, 16, lambda x, y: (128, 128, 128, 255))
png("cube/alpha.png", 32, 32, lambda x, y: (255, 255, 255, 64 + 6 * x))

# 4:3 sky box cross, 128 x 96: faces 32 x 32.
CROSS = {(1, 1): 0, (3, 1): 1, (0, 1): 2, (2, 1): 3, (1, 0): 4, (1, 2): 5}
def skybox(x, y):
    f = CROSS.get((x // 32, y // 32))
    if f is None:
        return (0, 0, 0, 0)
    r, g, b = BASE[f]
    k = 0.5 + 0.5 * ((x % 32) + 2 * (y % 32)) / 93
    return (int(r * k), int(g * k), int(b * k), 255)
png("cube/skybox.png", 128, 96, skybox)

# 2:1 panorama, 128 x 64: longitude bands and latitude shading.
def panorama(x, y):
    r, g, b = BASE[(x // 22) % 6]
    k = 0.3 + 0.7 * y / 63
    return (int(r * k), int(g * k), int(b * k), 255)
png("cube/panorama.png", 128, 64, panorama)

PIX = ["0xff0000", "0x00ff00", "0x0000ff", "0xffff00", "0xff00ff", "0x00ffff"]
def pixel_face(i):
    c = PIX[i]
    return f"PixelTexture {{ image 2 2 3 {c} 0x202020 0x808080 {c} }}"

def composed(faces, legacy=False):
    names = ["front", "back", "left", "right", "top", "bottom"] if legacy else [n + "Texture" for n in NAMES]
    return "ComposedCubeMapTexture { " + " ".join(f"{n} {f}" for n, f in zip(names, faces)) + " }"

IMG = [f'ImageTexture {{ url "cube/{n}.png" }}' for n in NAMES]
REFL = 'TextureCoordinateGenerator { mode "CAMERASPACEREFLECTIONVECTOR" }'

def cell(x, y, app, geometry):
    return f"Transform {{ translation {x} {y} 0 children Shape {{ appearance {app} geometry {geometry} }} }}"

def sphere_refl():
    return f"IndexedFaceSet {{ }}"

HEAD = "#X3D V{v} utf8\nPROFILE Interchange\nCOMPONENT CubeMapTexturing:3\nCOMPONENT Texturing:3\nCOMPONENT Geometry3D:4\nViewpoint {{ position 0 0 7 }}\nBackground {{ skyColor 0.2 0.25 0.3 }}\n"

def sphere_ifs(tc=True):
    # A UV sphere as an IndexedFaceSet so a TextureCoordinateGenerator can be set.
    pts, idx = [], []
    n, m = 24, 12
    for j in range(m + 1):
        phi = math.pi * j / m
        for i in range(n + 1):
            th = 2 * math.pi * i / n
            pts.append((0.7 * math.sin(phi) * math.sin(th), 0.7 * math.cos(phi), 0.7 * math.sin(phi) * math.cos(th)))
    for j in range(m):
        for i in range(n):
            a = j * (n + 1) + i
            idx += [a, a + n + 1, a + n + 2, a + 1, -1]
    coord = "Coordinate { point [ " + ", ".join("%.5f %.5f %.5f" % p for p in pts) + " ] }"
    return coord, " ".join(map(str, idx))

COORD, INDEX = sphere_ifs()
def sphere(texcoord):
    return f"IndexedFaceSet {{ creaseAngle 3.2 coord DEF SC {COORD} coordIndex [ {INDEX} ]" + (f" texCoord {texcoord}" if texcoord else "") + " }"

MAT = "Material { diffuseColor 1 1 1 }"
UNLIT = "UnlitMaterial { }"

# 1: ComposedCubeMapTexture: PixelTexture and ImageTexture faces, reflection and default coordinates,
# Box, lit and unlit, an incomplete set, mixed sizes, a transparent face.
cells = [
    (f"Appearance {{ material {UNLIT} texture {composed([pixel_face(i) for i in range(6)])} }}", sphere(REFL)),
    (f"Appearance {{ material {UNLIT} texture {composed(IMG)} }}", sphere(REFL)),
    (f"Appearance {{ material {MAT} texture {composed(IMG)} }}", sphere(REFL)),
    (f"Appearance {{ material {UNLIT} texture {composed(IMG)} }}", sphere(None)),
    (f"Appearance {{ material {UNLIT} texture {composed(IMG)} }}", "Box { size 1.2 1.2 1.2 }"),
    (f"Appearance {{ material {UNLIT} texture {composed(IMG[:5] + ['NULL'])} }}", sphere(REFL)),
    (f"Appearance {{ material {UNLIT} texture {composed(IMG[:5] + [chr(73) + 'mageTexture { url \"cube/odd.png\" }'])} }}", sphere(REFL)),
    (f"Appearance {{ material {UNLIT} texture {composed(IMG[:5] + ['ImageTexture { url \"cube/alpha.png\" }'])} }}", sphere(REFL)),
    (f"Appearance {{ material {UNLIT} texture {composed(IMG[:5] + ['ImageTexture { url \"cube/missing.png\" }'])} }}", sphere(REFL)),
    (f"Appearance {{ material {UNLIT} texture {composed(IMG[:5] + [pixel_face(5)])} }}", sphere(REFL)),
    (f"Appearance {{ material {UNLIT} texture {composed([pixel_face(i) for i in range(6)])} textureTransform TextureTransform3D {{ rotation 0 1 0 0.7 }} }}", sphere(REFL)),
    (f"Appearance {{ material {MAT} texture MultiTexture {{ mode [ \"MODULATE\" \"ADD\" ] texture [ ImageTexture {{ url \"cube/front.png\" }} {composed(IMG)} ] }} }}",
     f"IndexedFaceSet {{ creaseAngle 3.2 coord USE SC coordIndex [ {INDEX} ] texCoord MultiTextureCoordinate {{ texCoord [ TextureCoordinateGenerator {{ mode \"SPHERE\" }} {REFL} ] }} }}"),
]
shapes = [cell(-2.4 + 1.6 * (k % 4), 1.6 - 1.6 * (k // 4), a, g) for k, (a, g) in enumerate(cells)]
open(os.path.join(out, "cube-composed.wrl"), "w").write(HEAD.format(v="4.0") + "\n".join(shapes) + "\n")

# 2: the same faces with the legacy (3.3) field names.
cells = [(f"Appearance {{ material {UNLIT} texture {composed(IMG, legacy=True)} }}", sphere(REFL)),
         (f"Appearance {{ material {MAT} texture {composed(IMG, legacy=True)} }}", sphere(REFL))]
shapes = [cell(-0.8 + 1.6 * k, 0, a, g) for k, (a, g) in enumerate(cells)]
open(os.path.join(out, "cube-composed-v33.wrl"), "w").write(HEAD.format(v="3.3") + "\n".join(shapes) + "\n")

# 3: ImageCubeMapTexture: sky box cross, panorama, an image of another aspect, a missing url,
# url fallback, TextureProperties.
cells = [
    (f'Appearance {{ material {UNLIT} texture ImageCubeMapTexture {{ url "cube/skybox.png" }} }}', sphere(REFL)),
    (f'Appearance {{ material {UNLIT} texture ImageCubeMapTexture {{ url "cube/panorama.png" }} }}', sphere(REFL)),
    (f'Appearance {{ material {UNLIT} texture ImageCubeMapTexture {{ url "cube/front.png" }} }}', sphere(REFL)),
    (f'Appearance {{ material {UNLIT} texture ImageCubeMapTexture {{ url "cube/missing.png" }} }}', sphere(REFL)),
    (f'Appearance {{ material {UNLIT} texture ImageCubeMapTexture {{ url [ "cube/missing.png" "cube/skybox.png" ] }} }}', "Box { size 1.2 1.2 1.2 }"),
    (f'Appearance {{ material {MAT} texture ImageCubeMapTexture {{ url "cube/panorama.png" textureProperties TextureProperties {{ minificationFilter "NEAREST_PIXEL" magnificationFilter "NEAREST_PIXEL" }} }} }}', sphere(REFL)),
    (f'Appearance {{ material {UNLIT} texture ImageCubeMapTexture {{ url "cube/skybox.png" load FALSE }} }}', sphere(REFL)),
    (f'Appearance {{ material {UNLIT} texture ImageCubeMapTexture {{ url "cube/panorama.png" }} }}', sphere(None)),
]
shapes = [cell(-2.4 + 1.6 * (k % 4), 0.8 - 1.6 * (k // 4), a, g) for k, (a, g) in enumerate(cells)]
open(os.path.join(out, "cube-image.wrl"), "w").write(HEAD.format(v="4.0") + "\n".join(shapes) + "\n")
