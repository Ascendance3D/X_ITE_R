#!/usr/bin/env python3
"""Line-edge worlds (issue #83, part of #51): the points, a wide polyline and a thin rectangle of the XR case
stereo-points-lines, seen by one ordinary perspective Viewpoint (no XR).

  python3 -I oracle/tools/gen_lines_edge.py

writes oracle/synthetic-lines-edge/*.wrl. Newly authored; no historical content. The XR case's PointProperties field
pointSizeAttenuation is left out: pinned X_ITE 16.5.0 has no such field and logs a parser warning for it.
"""
import os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-lines-edge"))

HEAD = "#X3D V4.0 utf8\nPROFILE Full\n"

POINTS = """Shape {
  appearance Appearance { material Material { emissiveColor 1 0.8 0.2 } pointProperties PointProperties { pointSizeScaleFactor 8 pointSizeMinValue 2 pointSizeMaxValue 30 } }
  geometry PointSet { coord Coordinate { point [ -1.5 0.5 -2, -0.8 -0.2 -3, 0 0.6 -5, 0.8 0.1 -3.5, 1.6 0.7 -2, 1.0 -0.5 -6, -1.2 0.9 -7 ] } }
}
"""
WIDE = """Shape {
  appearance Appearance { material Material { emissiveColor 0.3 0.8 1 } lineProperties LineProperties { linewidthScaleFactor 6 } }
  geometry IndexedLineSet { coord Coordinate { point [ -1.5 -0.6 -2.5, -0.5 0.3 -3, 0.5 -0.6 -4, 1.5 0.3 -4.5 ] } coordIndex [ 0 1 2 3 -1 ] }
}
"""
THIN = """Shape {
  appearance Appearance { material Material { emissiveColor 1 0.3 0.5 } }
  geometry IndexedLineSet { coord Coordinate { point [ -2 -0.9 -2, 2 -0.9 -2, 2 -0.9 -8, -2 -0.9 -8 ] } coordIndex [ 0 1 2 3 0 -1 ] }
}
"""

# fieldOfView 1.3396 = 2 * atan(1 / 1.25): the projection scale of one eye of the XR case.
CAM1 = "Viewpoint { position 0.27 0.1 0.2 orientation 0 1 0 0.15 fieldOfView 1.3396 }"
CAM2 = "Viewpoint { position 0 0.2 1 fieldOfView 1.3396 }"
CAM3 = "Viewpoint { position -0.3 0 0.5 orientation 0 1 0 -0.2 fieldOfView 1.0 }"
CAM4 = "Viewpoint { position 0.1 0.3 0.6 orientation 0 0 1 0.1 fieldOfView 1.3396 }"

WORLDS = {
    "lines-edge": (CAM1, POINTS + WIDE + THIN),
    "lines-edge-points": (CAM1, POINTS),
    "lines-edge-wide": (CAM1, WIDE),
    "lines-edge-thin": (CAM1, THIN),
    "lines-edge-cam2": (CAM2, POINTS + WIDE + THIN),
    "lines-edge-cam3": (CAM3, POINTS + WIDE + THIN),
    "lines-edge-cam4": (CAM4, POINTS + WIDE + THIN),
}

os.makedirs(ROOT, exist_ok=True)

for name, (cam, body) in WORLDS.items():
    with open(os.path.join(ROOT, name + ".wrl"), "w") as f:
        f.write(HEAD + "# Points, a wide polyline and a thin rectangle seen by one plain perspective Viewpoint (no XR).\n"
                + cam + "\nNavigationInfo { headlight TRUE }\n" + body)
