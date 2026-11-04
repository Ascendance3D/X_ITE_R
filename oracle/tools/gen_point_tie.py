#!/usr/bin/env python3
"""Point-sprite tie worlds (issue #51, sub:render.core): PointSet points whose sprite edges fall exactly on pixel
centers, on pixel corners and near both, seen by an OrthoViewpoint so the window positions are exact.

  python3 -I oracle/tools/gen_point_tie.py

writes oracle/synthetic-point-tie/*.wrl. Newly authored; no historical content.

Picture 256 x 256, OrthoViewpoint fieldOfView -1 -1 1 1: one unit is 128 pixels, window x = (X + 1) * 128 and
window y (from the bottom) = (Y + 1) * 128. X_ITE draws a point of PointProperties scale s (above 1) with
gl_PointSize s + 1 (PointSize2.glsl: size + 1 - step (size, 1)), so the scale factors 1, 1.0000001 (the float
sum rounds to exactly 2), 2, 3, 4, 5 and 8 give the sizes 1, 2, 3, 4, 5, 6 and 9, and 1.5 and 7.5 give 2.5 and 8.5.
A sprite edge is on a pixel center when (center - 0.5 + size / 2) is a whole number: for an odd size the center is
on a whole pixel coordinate, for an even size on a half.
"""
import os
from fractions import Fraction

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-point-tie"))

HEAD = "#X3D V4.0 utf8\nPROFILE Full\n# %s\nOrthoViewpoint { position 0 0 10 fieldOfView [ -1 -1 1 1 ] }\nNavigationInfo { headlight FALSE }\n"

E = Fraction(1, 512)  # a quarter of one hundredth of a pixel: far above float32 error, far below a pixel


def shape(scale, points):
    pts = ",\n    ".join("%s %s 0" % (x, y) for x, y in points)
    return """Shape {
  appearance Appearance { material Material { emissiveColor 1 0.8 0.2 } pointProperties PointProperties { pointSizeScaleFactor %s pointSizeMinValue 1 pointSizeMaxValue 64 } }
  geometry PointSet { coord Coordinate { point [
    %s
  ] } }
}
""" % (scale, pts)


def units(window):
    v = (Fraction(window) - 128) / 128
    return repr(float(v))


def grid(tie, offsets):
    """6 x 6 points, 40 pixels apart. `tie` is the fractional window coordinate of a tie for this size."""
    pts = []
    for j in range(6):
        for i in range(6):
            ox = (tie + offsets[i]) % 1
            oy = (tie + offsets[j]) % 1
            pts.append((units(24 + 40 * i + ox), units(24 + 40 * j + oy)))
    return pts


# (name, scale factor, gl_PointSize)
SIZES = [("1", "1", 1), ("2", "1.0000001", 2), ("3", "2", 3), ("4", "3", 4), ("5", "4", 5), ("6", "5", 6), ("9", "8", 9),
         ("2p5", "1.5", 2.5), ("8p5", "7.5", 8.5)]
# offsets from the tie: on it, a whole pixel away (edge on a pixel corner), a little either side, then two plain ones.
OFFS = [Fraction(0), Fraction(1, 2), E, -E, Fraction(1, 4), Fraction(1, 8)]

WORLDS = {}
for name, scale, size in SIZES:
    # the fractional window coordinate where an edge (center +- size / 2) is on a pixel center
    # centre on tie => centre - size/2 = k + 1/2, so centre = k + 1/2 + size/2
    tie = (Fraction(1, 2) + Fraction(size).limit_denominator(1000) / 2) % 1
    WORLDS["point-tie-%s" % name] = HEAD % ("size %s: edges on pixel centers at offset 0" % size) + shape(scale, grid(tie, OFFS))

# control: no edge on a pixel center, no center on a half pixel; sub-pixel positions at least 0.12 from a tie of any
# size (fractions .15 .27 .33 .66 .73 .85 along x and .12 .24 .36 .64 .76 .88 along y), size 4 and size 3.
XF = [Fraction(n, 100) for n in (15, 27, 33, 66, 73, 85)]
YF = [Fraction(n, 100) for n in (12, 24, 36, 64, 76, 88)]
for name, scale in (("control", "3"), ("control3", "2")):
    ctl = [(units(24 + 40 * i + XF[i]), units(24 + 40 * j + YF[j])) for j in range(6) for i in range(6)]
    WORLDS["point-tie-" + name] = HEAD % "control: plain sub-pixel positions, scale %s" % scale + shape(scale, ctl)

# the snap world: sprites around one tie in steps of 1/1024 pixel, along y (a row of points) and along x (a column),
# at size 1 (tie on a whole pixel coordinate) and size 2 (tie on a half), to measure how the rasterizer snaps a position
K = range(-6, 6)
snap = []
snap += [(units(16 + 20 * (k + 6)), units(Fraction(104) + Fraction(k, 1024))) for k in K]          # size 1, along y
snap1 = [(units(Fraction(104) + Fraction(k, 1024)), units(24 + 20 * (k + 6))) for k in K]           # size 1, along x
snap2 = [(units(16 + 20 * (k + 6)), units(Fraction(369, 2) + Fraction(k, 1024))) for k in K]         # size 2, along y
WORLDS["point-tie-snap"] = (HEAD % "snap: positions in steps of 1/1024 pixel around a tie, sizes 1 and 2"
                            + shape("1", snap + snap1) + shape("1.0000001", snap2))

if __name__ == "__main__":
    os.makedirs(ROOT, exist_ok=True)
    for name, text in WORLDS.items():
        with open(os.path.join(ROOT, name + ".wrl"), "w") as f:
            f.write(text)
        print(name)
