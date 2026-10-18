#!/usr/bin/env python3
"""REA batch 04 still worlds (oracle/synthetic-rea-b04/nurbs-*.wrl) built from the geometry oracle
cases (geometry-cases.json): only cases where ISO and X_ITE agree (no "iso_" prefix)."""
import json, os
d = os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b04")
cases = json.load(open(os.path.join(d, "geometry-cases.json")))
HEAD = "#X3D V4.0 utf8\nPROFILE Interchange\nCOMPONENT NURBS:4\nCOMPONENT Texturing:3\n"
def world(name, picks, view, textured=False, lines=False, extra=""):
    out = [HEAD, f"Viewpoint {{ {view} }}\nDirectionalLight {{ direction -0.4 -1 -0.6 }}\n"]
    for i, c in enumerate(picks):
        x, z = (i % 3) * 6 - 6, (i // 3) * 6 - 6
        tex = ' texture ImageTexture { url "checker.png" }' if textured else ""
        mat = "emissiveColor 1 1 1" if lines else f"diffuseColor {0.5 + 0.15 * (i % 3)} 0.7 {0.9 - 0.1 * (i % 4)}"
        rot = "" if lines else " rotation 1 0 0 3.14159"
        out.append(f"Transform {{ translation {x} 0 {z}{rot} children Shape {{ appearance Appearance {{ material Material {{ {mat} }}{tex} }} geometry {cases[c]} }} }}\n")
    out.append(extra)
    open(os.path.join(d, name), "w").write("".join(out))
V = "position 0 18 20 orientation 1 0 0 -0.75"
world("nurbs-patch.wrl", ["p_basic", "p_order4_tess", "p_uclosed", "p_vclosed", "p_knots", "p_solid_false", "p_weights_ones", "p_neg_tess", "p_order2"], V)
world("nurbs-patch-tex.wrl", ["p_basic", "p_texcoord", "p_texcoord_short", "p_nurbstex", "p_nurbstex_order3", "p_knots_unpinned"], V, textured=True)
world("nurbs-trimmed.wrl", ["t_ccw_square", "t_ccw_square_cw_hole", "t_circle", "t_circle_tess", "t_two_children", "t_partly_outside", "t_knots_scaled", "t_texcoord"], V, textured=True)
world("nurbs-curve.wrl", ["c_basic", "c_order2", "c_order4_tess", "c_neg_tess", "c_knots", "c_closed", "c_closed_order4", "c_weights_ones", "c_knots_unpinned"], "position 0 1 24", lines=True)
world("nurbs-swept.wrl", ["s_poly_section", "s_curve_section", "s_ccw_false", "w_basic", "w_poly", "w_ccw_false"], "position 0 14 22 orientation 1 0 0 -0.6")
world("nurbs-set.wrl", [], V, extra=f"DEF A Transform {{ translation -4 0 0 rotation 1 0 0 3.14159 children Shape {{ appearance Appearance {{ material Material {{ }} }} geometry DEF G1 {cases['p_basic']} }} }}\nDEF B Transform {{ translation 4 0 0 rotation 1 0 0 3.14159 children Shape {{ appearance Appearance {{ material Material {{ }} }} geometry DEF G2 {cases['t_circle']} }} }}\nNurbsSet {{ tessellationScale 0.4 geometry [ USE G1 USE G2 ] }}\nNurbsSet {{ geometry [ {cases['p_order4_tess']} ] }}\n")
print("ok")
