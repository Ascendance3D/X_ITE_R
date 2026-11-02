#!/usr/bin/env python3
"""Synthetic GaussianSplats avatar-collision worlds (issue #69, pass 4) and their probe scripts.

  python3 -I -B oracle/tools/gen_b69_gs_collision.py

writes oracle/synthetic-b69-gs-collision/NAME.x3dv and scripts/NAME.json. A world is probed with the `collisionProbes` of
`x3d-oracle particles` (getClosestObject along nine directions, constrainTranslation, the named Collision nodes), as
oracle/synthetic-rea-b15-particle-collision is. The splats sit around the viewer at the origin on the probe directions, at
three distances each; they are small enough (the collision camera sees a 0.5 wide column) that a quad stays within the
display viewport the program sizes it from. All data are synthetic (seeded pseudo-random).
"""
import json, os, random

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out = os.path.join(root, "oracle", "synthetic-b69-gs-collision")
os.makedirs(os.path.join(out, "scripts"), exist_ok=True)

DIRECTIONS = [[1, 0, 0], [0, 1, 0], [0, 0, 1], [-1, 0, 0], [0, -1, 0], [0, 0, -1], [0.6, 0, -0.8], [-0.6, 0, -0.8], [0, -0.6, -0.8]]
TRANSLATIONS = [[0, 0, -0.01], [0, 0, -0.1], [0, 0, -0.3], [0.2, 0, -0.2], [-0.05, 0, -0.04], [0, 0, 0.1]]


def cloud(seed, extra="", distances=(0.7, 1.4, 2.1), dirs=DIRECTIONS, scale=(0.04, 0.09)):
    r = random.Random(seed)
    pos, ori, sc, op, sh = [], [], [], [], []
    for d in dirs:
        l = sum(v * v for v in d) ** 0.5
        for k in distances:
            c = [k * v / l + r.uniform(-0.06, 0.06) for v in d]
            pos.append(" ".join("%.4f" % v for v in c))
            q = [r.uniform(-1, 1) for _ in range(4)]
            n = sum(v * v for v in q) ** 0.5
            ori.append(" ".join("%.4f" % (v / n) for v in q))
            sc.append(" ".join("%.4f" % r.uniform(*scale) for _ in range(3)))
            op.append("%.3f" % r.uniform(0.7, 0.95))
            sh.append(" ".join("%.4f" % r.uniform(-0.8, 1.2) for _ in range(3)))
    return ("GaussianSplats {\n  %s\n  positions [ %s ]\n  orientations [ %s ]\n  scales [ %s ]\n  opacities [ %s ]\n"
            "  sphericalHarmonicsDegree0Coef0 [ %s ]\n}" % (extra, ", ".join(pos), ", ".join(ori), ", ".join(sc), ", ".join(op), ", ".join(sh)))


def box(at, size):
    return ("Transform { translation %s children Shape { appearance Appearance { material Material { diffuseColor 0.8 0.2 0.2 } } "
            "geometry Box { size %s %s %s } } }" % (at, size, size, size))


def floor(y, half=0.75, step=0.25, scale=(0.18, 0.02, 0.18)):
    """A grid of flat splats at height y under the viewer (WALK gravity looks straight down)."""
    n = int(round(half / step))
    pts = [(i * step, j * step) for i in range(-n, n + 1) for j in range(-n, n + 1)]
    pos = ", ".join("%.4f %.4f %.4f" % (x, y, z) for x, z in pts)
    one = lambda v: ", ".join(v for _ in pts)
    return ("GaussianSplats {\n  positions [ %s ]\n  orientations [ %s ]\n  scales [ %s ]\n  opacities [ %s ]\n"
            "  sphericalHarmonicsDegree0Coef0 [ %s ]\n}" % (pos, one("0 0 0 1"), one("%.3f %.3f %.3f" % scale), one("0.9"), one("0.2 0.6 0.4")))


worlds = {
    # pointerEvents FALSE (the default): a collision object all the same.
    "gc-basic": ([cloud(1)], []),
    "gc-pointer": ([cloud(2, "pointerEvents TRUE")], []),
    "gc-collision-node": (["DEF COL Collision { children [ %s ] }" % cloud(3, distances=(0.3, 1.0)), "DEF COL_OFF Collision { enabled FALSE children [ %s ] }" % cloud(4, distances=(0.5,), dirs=[[0, 0, -1]], scale=(0.06, 0.08))], ["COL", "COL_OFF"]),
    "gc-clip": (["Group { children [ ClipPlane { plane 1 0 0 0 } %s ] }" % cloud(5)], []),
    "gc-xform": (["Transform { translation 0.1 0 0 rotation 0 1 0 0.6 scale 1.3 1 1.2 children %s }" % cloud(6)], []),
    # Shapes and splats in one collision pass: the ids follow the traversal order.
    # addCollisionShape keeps a shape whose bbox sphere meets the collision camera's volume (ortho +-3.2 around the viewer for
    # the default avatar): an explicit bbox centered outside it (but in the camera's view: a shape never displayed keeps the pinned page loading) removes the splats from the pass, near the edge it keeps them.
    "gc-bbox-in": ([cloud(8, "bboxCenter 0 0 0 bboxSize 0.2 0.2 0.2")], []),
    "gc-bbox-edge-out": ([cloud(8, "bboxCenter 0 0 -4.5 bboxSize 1 1 1")], []),
    "gc-bbox-edge-in": ([cloud(8, "bboxCenter 0 0 -3.8 bboxSize 1.5 1.5 1.5")], []),
    # WALK gravity onto a splat floor: the viewpoint's positionOffset follows the frames.
    "gc-walk-floor": ([floor(-1.0)], []),
    # The same fall with no splats and with 441 and 3969 (21 and 63 per side) of them: the avatar's start is two frames later
    # whenever the scene holds splats, whatever their count (docs/gaussian-splats-walk-start-lag.md).
    "gc-walk-lag-control": ([], []),
    "gc-walk-lag-n441": ([floor(-1.0, half=2.5, step=0.25)], []),
    "gc-walk-lag-n3969": ([floor(-1.0, half=7.5, step=0.25)], []),
    "gc-walk-far": ([floor(-1.0, scale=(0.18, 0.02, 0.18)).replace("GaussianSplats {", "GaussianSplats {\n  bboxCenter 0 -1 -4.5 bboxSize 1 1 1")], []),
    "gc-mixed": ([box("0 0 -1.0", 0.6), cloud(7), box("1.0 0 0", 0.5)], []),
}

for name, (kids, nodes) in worlds.items():
    frames = [1, 2, 3, 5, 8, 12] if nodes else [3, 8]
    walk = name.startswith("gc-walk")
    if walk:
        frames = list(range(1, 15)) + [20, 21, 22, 40, 41, 42]
    script = {"dt": 1 / 30, "t0": 946684800, "frames": max(frames), "warmup": 8, "node": "PS", "dump": [max(frames)], "seed": 69000002,
              "collisionProbes": {"frames": frames, "directions": DIRECTIONS, "translations": TRANSLATIONS, "nodes": nodes}, "seedFree": True}
    with open(os.path.join(out, name + ".x3dv"), "w") as f:
        f.write("#X3D V4.1 utf8\nPROFILE Full\nCOMPONENT X_ITE:1\nCOMPONENT GaussianSplats:1\n")
        f.write("# Synthetic collision world for GaussianSplats (generated by oracle/tools/gen_b69_gs_collision.py).\n")
        f.write("NavigationInfo { type \"" + ("WALK" if walk else "FLY") + "\" }\nViewpoint { position 0 0 0 orientation 0 0 1 0 }\nDEF RUN Collision { }\n")
        f.write("DEF PS ParticleSystem { enabled FALSE maxParticles 1 geometryType \"POINT\" }\n")
        f.write("\n".join(kids) + "\n")
    with open(os.path.join(out, "scripts", name + ".json"), "w") as f:
        json.dump(script, f)
print(len(worlds), "worlds")
