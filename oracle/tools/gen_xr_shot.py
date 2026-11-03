#!/usr/bin/env python3
"""XR oracle scripts (issue #83): writes oracle/synthetic-xr/NAME.json, the viewer pose and views that
oracle/x_ite-xr-shot.mjs gives the pinned X_ITE through its mock session and that crates/x3d-render/tests/xr_views.rs
gives the Rust renderer. Matrices are column-major arrays, as WebXR hands them out. The worlds (synthetic, written by
hand) are oracle/synthetic-xr/NAME.x3dv.

  python3 -I oracle/tools/gen_xr_shot.py
"""
import json, math, os

root = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", ".."))
out = os.path.join(root, "oracle", "synthetic-xr")

I = [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1]


def mul(a, b):
    """Column-major a * b."""
    return [sum(a[k * 4 + r] * b[c * 4 + k] for k in range(4)) for c in range(4) for r in range(4)]


def trans(x, y, z):
    return [1, 0, 0, 0, 0, 1, 0, 0, 0, 0, 1, 0, x, y, z, 1]


def roty(a):
    c, s = math.cos(a), math.sin(a)
    return [c, 0, -s, 0, 0, 1, 0, 0, s, 0, c, 0, 0, 0, 0, 1]


def rotx(a):
    c, s = math.cos(a), math.sin(a)
    return [1, 0, 0, 0, 0, c, s, 0, 0, -s, c, 0, 0, 0, 0, 1]


def invert_rigid(m):
    """Inverse of a rotation plus translation."""
    r = [[m[c * 4 + rr] for c in range(3)] for rr in range(3)]
    t = m[12:15]
    rt = [[r[c][rr] for c in range(3)] for rr in range(3)]
    ti = [-sum(rt[rr][c] * t[c] for c in range(3)) for rr in range(3)]
    return [rt[0][0], rt[1][0], rt[2][0], 0, rt[0][1], rt[1][1], rt[2][1], 0, rt[0][2], rt[1][2], rt[2][2], 0, ti[0], ti[1], ti[2], 1]


def persp(fovy, aspect, n, f):
    t = 1 / math.tan(fovy / 2)
    return [t / aspect, 0, 0, 0, 0, t, 0, 0, 0, 0, (f + n) / (n - f), -1, 0, 0, 2 * f * n / (n - f), 0]


def offaxis(left, right, bottom, top, n, f):
    """A WebXR-style asymmetric frustum (tangents at distance 1)."""
    l, r, b, t = left * n, right * n, bottom * n, top * n
    return [2 * n / (r - l), 0, 0, 0, 0, 2 * n / (t - b), 0, 0, (r + l) / (r - l), (t + b) / (t - b), -(f + n) / (f - n), -1, 0, 0, -2 * f * n / (f - n), 0]


def views(pose, eyes, size, projections):
    w, h = size
    half = w // 2
    result = []
    for i, (offset, projection) in enumerate(zip(eyes, projections)):
        t = mul(pose, offset)
        result.append({"projectionMatrix": projection, "matrix": t, "inverse": invert_rigid(t), "viewport": [i * half, 0, half, h]})
    return result


def script(size, pose, eyes, projections, world=None, headlight=True, frames=4):
    return {"world": world, "headlight": headlight, "size": size, "frames": frames, "pose": {"matrix": pose, "inverse": invert_rigid(pose)}, "views": views(pose, eyes, size, projections)}


IPD = 0.064
sym = persp(math.radians(60), 1.0, 0.1, 100.0)
SIZE = [512, 256]
STEREO = [trans(-IPD / 2, 0, 0), trans(IPD / 2, 0, 0)]
POSE = mul(trans(0.3, 0.1, 0.2), roty(0.2))
TOE_IN = [mul(trans(-IPD / 2, 0, 0), roty(-0.05)), mul(trans(IPD / 2, 0, 0), roty(0.05))]
OFF_AXIS = [offaxis(-0.9, 0.7, -0.8, 0.8, 0.1, 100.0), offaxis(-0.7, 0.9, -0.8, 0.8, 0.1, 100.0)]
# The world of each script is oracle/synthetic-xr/WORLD.x3dv ("world"). The viewer pose, the eyes and the frusta vary by case.
SCRIPTS = {
    # The viewer at the origin, eyes 64 mm apart, the same symmetric frustum: the simplest stereo pair.
    "stereo-shapes": script(SIZE, I, STEREO, [sym, sym], "stereo-shapes"),
    # Eyes turned in (toe-in): the eye matrix has a rotation.
    "stereo-toein": script(SIZE, I, TOE_IN, [sym, sym], "stereo-shapes"),
    # Asymmetric frusta, as a headset reports them.
    "stereo-offaxis": script(SIZE, I, STEREO, OFF_AXIS, "stereo-shapes"),
    # A viewer pose that is not the identity (translated and turned), the eyes in front of it.
    "stereo-pose-only": script(SIZE, POSE, STEREO, [sym, sym], "stereo-shapes"),
    # All together.
    "stereo-pose": script(SIZE, POSE, TOE_IN, OFF_AXIS, "stereo-shapes"),
    # Lights with positions, a physical material; fog and a background: the same pose.
    "stereo-lights": script(SIZE, POSE, TOE_IN, OFF_AXIS, "stereo-lights", headlight=False),
    "stereo-fog-background": script(SIZE, POSE, TOE_IN, OFF_AXIS, "stereo-fog-background"),
}

for name, s in SCRIPTS.items():
    json.dump(s, open(os.path.join(out, name + ".json"), "w"), indent=1)
    open(os.path.join(out, name + ".json"), "a").write("\n")

print(f"{len(SCRIPTS)} scripts written to {os.path.relpath(out, root)}")
