#!/usr/bin/env python3
"""REA batch 04: NURBS geometry oracle cases (oracle/synthetic-rea-b04/geometry-cases.json).

Every case is synthetic. Cases whose name starts with "iso_" exercise a rule where ISO/IEC
19775-1 clause 27 and X_ITE 16.5.0 disagree (owner decision 2026-10-17: Rust follows ISO);
their X_ITE arrays are recorded for the record but are not expected to match.
"""
import json, math, sys

def grid(nu, nv, f):
    pts = []
    for j in range(nv):
        for i in range(nu):
            pts.append(f(i, j))
    return pts

def fmt(pts):
    return ", ".join(" ".join(repr(round(c, 6)) for c in p) for p in pts)

def wave(nu, nv):
    return grid(nu, nv, lambda i, j: (i - (nu - 1) / 2, 0.4 * math.sin(i * 1.3) * math.cos(j * 0.9), j - (nv - 1) / 2))

def patch(nu, nv, pts, extra="", node="NurbsPatchSurface", coord="Coordinate"):
    return f"{node} {{ uDimension {nu} vDimension {nv} {extra} controlPoint {coord} {{ point [ {fmt(pts)} ] }} }}"

def cyl(nu, nv):
    # u closed: the last column repeats the first.
    pts = []
    for j in range(nv):
        for i in range(nu):
            a = 2 * math.pi * (i % (nu - 1)) / (nu - 1)
            pts.append((math.cos(a), j * 0.7 - 1, -math.sin(a)))
    return pts

def vcyl(nu, nv):
    pts = []
    for j in range(nv):
        for i in range(nu):
            a = 2 * math.pi * (j % (nv - 1)) / (nv - 1)
            pts.append((i * 0.7 - 1, math.cos(a), math.sin(a)))
    return pts

W = wave(5, 4)
cases = {}
cases["p_basic"] = patch(5, 4, W)
cases["p_order2"] = patch(5, 4, W, "uOrder 2 vOrder 2")
cases["p_order4_tess"] = patch(5, 4, W, "uOrder 4 vOrder 4 uTessellation 8 vTessellation 5")
cases["p_neg_tess"] = patch(5, 4, W, "uTessellation -2 vTessellation -1")
cases["p_knots"] = patch(5, 4, W, "uKnot [0 0 0 0.2 0.7 1 1 1] vKnot [0 0 0 0.4 1 1 1]")
cases["p_knots_unpinned"] = patch(5, 4, W, "uKnot [0 1 2 3 4 5 6 7] vKnot [0 0.5 1 1.5 2 2.5 3]")
cases["p_knots_bad_count"] = patch(5, 4, W, "uKnot [0 0 1 1] vKnot [0 0 0 1 1 1 1]")
cases["p_knots_decreasing"] = patch(5, 4, W, "uKnot [0 0 0 0.6 0.4 1 1 1]")
cases["p_knots_multiplicity"] = patch(5, 4, W, "uKnot [0 0 0 0.5 0.5 0.5 1 1]")
cases["p_solid_false"] = patch(5, 4, W, "solid FALSE")
cases["p_coord_double"] = patch(5, 4, W, coord="CoordinateDouble")
cases["p_uclosed"] = patch(7, 3, cyl(7, 3), "uClosed TRUE uTessellation 12")
cases["p_uclosed_order4"] = patch(7, 3, cyl(7, 3), "uClosed TRUE uOrder 4")
cases["p_uclosed_open_points"] = patch(5, 4, W, "uClosed TRUE")
cases["p_vclosed"] = patch(3, 7, vcyl(3, 7), "vClosed TRUE")
cases["p_weights_ones"] = patch(5, 4, W, "weight [" + " ".join(["1"] * 20) + "]")
cases["p_weights_short"] = patch(5, 4, W, "weight [2 3]")
cases["p_texcoord"] = patch(5, 4, W, "texCoord TextureCoordinate { point [" + fmt(grid(5, 4, lambda i, j: (i / 4 * 2, j / 3))) + "] }")
cases["p_texcoord_short"] = patch(5, 4, W, "texCoord TextureCoordinate { point [0 0, 1 0, 1 1] }")
cases["p_texcoord4d"] = patch(5, 4, W, "texCoord TextureCoordinate4D { point [" + fmt(grid(5, 4, lambda i, j: (i / 4, j / 3, 0.5, 1 + i * 0.1))) + "] }")
cases["p_multitex"] = patch(5, 4, W, "texCoord MultiTextureCoordinate { texCoord [ TextureCoordinate { point [" + fmt(grid(5, 4, lambda i, j: (i / 4, j / 3))) + "] } TextureCoordinate3D { point [" + fmt(grid(5, 4, lambda i, j: (j / 3, i / 4, 0.25))) + "] } ] }")
cases["p_nurbstex"] = patch(5, 4, W, "texCoord NurbsTextureCoordinate { uDimension 2 vDimension 2 uOrder 2 vOrder 2 controlPoint [0 0, 2 0, 0 1, 2 1] }")
cases["p_nurbstex_order3"] = patch(5, 4, W, "texCoord NurbsTextureCoordinate { uDimension 3 vDimension 3 controlPoint [0 0, 0.5 0.1, 1 0, 0 0.5, 0.6 0.5, 1 0.5, 0 1, 0.5 0.9, 1 1] }")
cases["p_nurbstex_invalid"] = patch(5, 4, W, "texCoord NurbsTextureCoordinate { uDimension 1 vDimension 2 controlPoint [0 0, 1 1] }")
cases["p_invalid_order"] = patch(5, 4, W, "uOrder 1")
cases["p_invalid_dimension"] = patch(5, 4, W, "uDimension 2")
cases["p_too_few_points"] = patch(5, 4, W[:10])
cases["p_no_controlpoint"] = "NurbsPatchSurface { uDimension 3 vDimension 3 }"
cases["iso_p_weighted"] = patch(5, 4, W, "weight [" + " ".join(str(1 + (k % 3) * 0.5) for k in range(20)) + "]")
cases["iso_p_weights_long"] = patch(5, 4, W, "weight [" + " ".join(["2"] + ["1"] * 19) + " 5 5]")
cases["iso_p_nurbstex_weighted"] = patch(5, 4, W, "texCoord NurbsTextureCoordinate { uDimension 3 vDimension 3 weight [1 2 1 2 4 2 1 2 1] controlPoint [0 0, 0.5 0, 1 0, 0 0.5, 0.5 0.5, 1 0.5, 0 1, 0.5 1, 1 1] }")
cases["iso_p_uclosed_seam_weight"] = patch(7, 3, cyl(7, 3), "uClosed TRUE weight [" + " ".join(["2"] + ["1"] * 5 + ["2"] + ["1"] * 14) + "]")

# Trimmed surfaces: the contours are in the (u, v) knot space; default knots are [0, 1].
SQ_CCW = "ContourPolyline2D { controlPoint [0.2 0.2, 0.8 0.2, 0.8 0.8, 0.2 0.8, 0.2 0.2] }"
SQ_CW_HOLE = "ContourPolyline2D { controlPoint [0.4 0.4, 0.4 0.6, 0.6 0.6, 0.6 0.4, 0.4 0.4] }"
SQ_CW = "ContourPolyline2D { controlPoint [0.2 0.2, 0.2 0.8, 0.8 0.8, 0.8 0.2, 0.2 0.2] }"
CIRCLE = "NurbsCurve2D { order 3 tessellation 16 closed TRUE controlPoint [0.5 0.1, 0.9 0.1, 0.9 0.5, 0.9 0.9, 0.5 0.9, 0.1 0.9, 0.1 0.5, 0.1 0.1, 0.5 0.1] }"
def trimmed(extra, contours):
    return patch(5, 4, W, extra + " trimmingContour [ " + " ".join(f"Contour2D {{ children [ {c} ] }}" for c in contours) + " ]", node="NurbsTrimmedSurface")
cases["t_none"] = trimmed("", [])
cases["t_ccw_square"] = trimmed("", [SQ_CCW])
cases["t_ccw_square_cw_hole"] = trimmed("", [SQ_CCW, SQ_CW_HOLE])
cases["t_circle"] = trimmed("", [CIRCLE])
cases["t_circle_tess"] = trimmed("uTessellation 16 vTessellation 12", [CIRCLE])
cases["t_two_children"] = patch(5, 4, W, "trimmingContour [ Contour2D { children [ ContourPolyline2D { controlPoint [0.1 0.1, 0.9 0.1, 0.9 0.5] } NurbsCurve2D { controlPoint [0.9 0.5, 0.9 0.9, 0.5 0.9, 0.1 0.9, 0.1 0.5, 0.1 0.1] } ] } ]", node="NurbsTrimmedSurface")
cases["t_partly_outside"] = trimmed("", ["ContourPolyline2D { controlPoint [-0.3 0.3, 0.6 0.3, 0.6 1.4, -0.3 1.4, -0.3 0.3] }"])
cases["t_knots_scaled"] = trimmed("uKnot [0 0 0 1 2 3 3 3] vKnot [0 0 0 2 4 4 4]", ["ContourPolyline2D { controlPoint [0.5 1, 2.5 1, 2.5 3, 0.5 3, 0.5 1] }"])
cases["t_texcoord"] = trimmed("texCoord TextureCoordinate { point [" + fmt(grid(5, 4, lambda i, j: (i / 4, j / 3))) + "] }", [SQ_CCW, SQ_CW_HOLE])
cases["t_empty_contour"] = trimmed("", ["ContourPolyline2D { }"])
cases["iso_t_cw_square"] = trimmed("", [SQ_CW])
cases["iso_t_weighted_curve"] = trimmed("", ["NurbsCurve2D { order 3 tessellation 16 closed TRUE weight [1 0.7 1 0.7 1 0.7 1 0.7 1] controlPoint [0.5 0.1, 0.9 0.1, 0.9 0.5, 0.9 0.9, 0.5 0.9, 0.1 0.9, 0.1 0.5, 0.1 0.1, 0.5 0.1] }"])

# Curves.
C = [(0, 0, 0), (1, 1, 0), (2, -1, 0.5), (3, 0.5, 1), (4, 0, 0)]
def curve(pts, extra="", coord="Coordinate"):
    return f"NurbsCurve {{ {extra} controlPoint {coord} {{ point [ {fmt(pts)} ] }} }}"
cases["c_basic"] = curve(C)
cases["c_order2"] = curve(C, "order 2")
cases["c_order4_tess"] = curve(C, "order 4 tessellation 7")
cases["c_neg_tess"] = curve(C, "tessellation -3")
cases["c_knots"] = curve(C, "knot [0 0 0 0.1 0.8 1 1 1]")
cases["c_knots_unpinned"] = curve(C, "knot [0 1 2 3 4 5 6 7]")
CL = [(math.cos(a), math.sin(a), 0) for a in [2 * math.pi * k / 6 for k in range(6)]] + [(1, 0, 0)]
cases["c_closed"] = curve(CL, "closed TRUE")
cases["c_closed_order4"] = curve(CL, "closed TRUE order 4 tessellation 20")
cases["c_closed_open_points"] = curve(C, "closed TRUE")
cases["c_weights_ones"] = curve(C, "weight [1 1 1 1 1]")
cases["c_too_few"] = curve(C[:2])
cases["c_order1"] = curve(C, "order 1")
cases["c_coord_double"] = curve(C, coord="CoordinateDouble")
cases["iso_c_weighted"] = curve(C, "weight [1 2 0.5 3 1]")
cases["iso_c_closed_seam_weight"] = curve(CL, "closed TRUE weight [1 2 1 1 1 1 1]")

# Swept and swung surfaces.
SECTION_POLY = "ContourPolyline2D { controlPoint [0.3 0, 0 0.3, -0.3 0, 0 -0.3, 0.3 0] }"
SECTION_CURVE = "NurbsCurve2D { tessellation 8 controlPoint [0.3 0, 0.3 0.3, -0.3 0.3, -0.3 -0.3, 0.3 -0.3] }"
TRAJ = "NurbsCurve { tessellation 10 controlPoint Coordinate { point [0 0 0, 1 1 0, 2 1 1, 3 0 1] } }"
cases["s_poly_section"] = f"NurbsSweptSurface {{ crossSectionCurve {SECTION_POLY} trajectoryCurve {TRAJ} }}"
cases["s_curve_section"] = f"NurbsSweptSurface {{ crossSectionCurve {SECTION_CURVE} trajectoryCurve {TRAJ} }}"
cases["s_ccw_false"] = f"NurbsSweptSurface {{ ccw FALSE solid FALSE crossSectionCurve {SECTION_POLY} trajectoryCurve {TRAJ} }}"
cases["s_no_trajectory"] = f"NurbsSweptSurface {{ crossSectionCurve {SECTION_POLY} }}"
cases["iso_s_weighted_trajectory"] = f"NurbsSweptSurface {{ crossSectionCurve {SECTION_POLY} trajectoryCurve NurbsCurve {{ tessellation 10 weight [1 3 1 1] controlPoint Coordinate {{ point [0 0 0, 1 1 0, 2 1 1, 3 0 1] }} }} }}"
PROFILE = "NurbsCurve2D { tessellation 6 controlPoint [1 0, 1.2 0.5, 0.8 1, 1 1.5] }"
TRAJ2 = "NurbsCurve2D { tessellation 12 controlPoint [2 0, 2 2, -2 2, -2 -2, 2 -2, 2 0] }"
cases["w_basic"] = f"NurbsSwungSurface {{ profileCurve {PROFILE} trajectoryCurve {TRAJ2} }}"
cases["w_poly"] = f"NurbsSwungSurface {{ profileCurve ContourPolyline2D {{ controlPoint [1 0, 1 1] }} trajectoryCurve ContourPolyline2D {{ controlPoint [2 0, 0 2, -2 0, 0 -2, 2 0] }} }}"
cases["w_ccw_false"] = f"NurbsSwungSurface {{ ccw FALSE profileCurve {PROFILE} trajectoryCurve {TRAJ2} }}"
cases["iso_w_weighted_profile"] = f"NurbsSwungSurface {{ profileCurve NurbsCurve2D {{ tessellation 6 weight [1 2 1 1] controlPoint [1 0, 1.2 0.5, 0.8 1, 1 1.5] }} trajectoryCurve {TRAJ2} }}"

json.dump(cases, open(sys.argv[1], "w"), indent=1)
print(len(cases), "cases")
