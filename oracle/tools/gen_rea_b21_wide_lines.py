#!/usr/bin/env python3
"""Wide LINE particles in the pointing, collision and shadow passes (lane XITE_R_PARTICLE_WIDE_LINE_PASSES_01): synthetic
worlds and scripts.

  python3 -I oracle/tools/gen_rea_b21_wide_lines.py

writes oracle/synthetic-rea-b21-wide-lines/NAME.x3dv and scripts/NAME.json for `x3d-oracle particles`.

What pinned X_ITE 16.5.0 does, read from its source (x_ite-pin, 34cdf9f97); these cases measure it:

* Widths up to ALIASED_LINE_WIDTH_RANGE [1] (10 on the oracle GPU) are gl.lineWidth lines in every pass: the pointing
  and depth (collision, shadow) passes call LineProperties.setShaderUniforms, which sets gl.lineWidth (and the stipple
  uniforms) unless getTransformLines, before X3DLineGeometryNode.displaySimpleInstanced draws gl.LINES.
* Wider lines (LineProperties.getTransformLines) are widened to triangles by the color pass (displayInstancedThick:
  the color pass's viewport and modelViewProjection, back to model space). displaySimpleInstancedThick draws those
  triangles, and draws nothing when the stored line count (lines x instances) is not the current one.
* Per frame, COLLISION traverses before DISPLAY, and the shadow maps are rendered at the start of DISPLAY, before the
  color pass; pick probes run after the frame. So collision and shadow see the previous frame's triangles, or none when
  the particle count changed, and a system the color pass culls has none.

pw-pick-*: pickProbes (the batch 14 grid) over a row or a slanted run of LINE particles, widths 1 (no LineProperties),
6, 10, 11, 24, and a steady particle count.
pw-col-*: collisionProbes (batch 15) every frame, LINE particles crossing the -Z collision camera, widths 1, 6, 11, 24,
and a steady count.
pw-shadow-*: images at the dump frames of a DirectionalLight shadow of LINE particles on a floor, widths 1, 6, 11, 24,
a steady count, and a system outside the view whose shadow falls inside it.
Every emitter is deterministic (no speed, lifetime or angle variation). Newly authored; no historical content.
"""
import json, os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b21-wide-lines"))

HEAD = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\nCOMPONENT Shape:5\n"


def appearance(width, color="1 1 1"):
    lines = f" lineProperties LineProperties {{ linewidthScaleFactor {width} }}" if width > 1 else ""
    return f"Appearance {{ material Material {{ emissiveColor {color} }}{lines} }}"


def ps(emitter, width, size="0.1 0.15", lifetime=2, max_particles=20, name="PS", color="1 1 1"):
    return (f'DEF {name} ParticleSystem {{\n  geometryType "LINE"\n  maxParticles {max_particles}\n'
            f'  particleLifetime {lifetime}\n  lifetimeVariation 0\n  particleSize {size}\n'
            f'  emitter {emitter}\n  appearance {appearance(width, color)}\n}}\n')


# pointing: the batch 14 view and probe grid
PICK_VIEW = "Viewpoint { position 0 0 3 }\nBackground { skyColor 0.1 0.1 0.2 }\n"
PICK_ROW = "PointEmitter { position -0.8 0 0 direction 1 0 0 speed 0.8 variation 0 }"
PICK_SLANT = "PointEmitter { position -0.8 -0.4 0 direction 1 0.6 0 speed 0.8 variation 0 }"


def grid(step=32):
    pts = [[x + step / 2, y + step / 2] for y in range(0, 512, step) for x in range(0, 512, step)]
    dense = [[x + 0.5, y + 0.5] for y in range(160, 352, 5) for x in range(96, 416, 5)]
    row = [[x + 0.5, 256.5] for x in range(80, 432)]
    column = [[256.5, y + 0.5] for y in range(200, 312)]
    return pts + dense + row + column


# collision: the batch 15 view (FLY avatar at the origin looking along -Z, an empty Collision keeps COLLISION running)
COL_VIEW = ('NavigationInfo { type "FLY" }\nViewpoint { position 0 0 0 orientation 0 0 1 0 }\n'
            "Background { skyColor 0.1 0.1 0.2 }\nDEF RUN Collision { }\n")
COL_ROW = "PointEmitter { position -0.4 0.011 -0.3 direction 1 0 0 speed 0.5 variation 0 }"
DIRECTIONS = [[1, 0, 0], [0, 1, 0], [0, 0, 1], [-1, 0, 0], [0, -1, 0], [0, 0, -1],
              [0.6, 0, -0.8], [-0.6, 0, -0.8], [0, -0.6, -0.8], [0.1, 0.05, -1], [-0.1, -0.05, -1]]
TRANSLATIONS = [[0, 0, -0.01], [0, 0, -0.1], [0, 0, -0.3], [0.2, 0, -0.2], [-0.05, 0, -0.04], [0, 0, 0.1]]

# shadow: a lit floor seen from above and in front, the light from above and slightly behind
SHADOW_VIEW = ("NavigationInfo { headlight FALSE }\nViewpoint { position 0 1.6 3.2 orientation 1 0 0 -0.45 }\n"
               "Background { skyColor 0.1 0.1 0.2 }\n"
               "DirectionalLight { direction 0.15 -1 -0.3 ambientIntensity 0.3 shadows TRUE shadowIntensity 1 }\n"
               "Transform { translation 0 -0.05 0 children Shape { appearance Appearance { material Material "
               "{ diffuseColor 0.8 0.8 0.8 } } geometry Box { size 4 0.1 4 } } }\n")
SHADOW_ROW = "PointEmitter { position -1 0.6 0.2 direction 1 0 -0.2 speed 0.8 variation 0 }"
# above the viewpoint, outside its view volume; the light throws the shadow forward onto the visible floor
SHADOW_HIDDEN = "PointEmitter { position -1 3.4 3.6 direction 1 0 0 speed 0.8 variation 0 }"

STEADY = {"lifetime": 0.4, "max_particles": 4}

PICK = {
    "pw-pick-row-w1": ps(PICK_ROW, 1),
    "pw-pick-row-w6": ps(PICK_ROW, 6),
    "pw-pick-row-w10": ps(PICK_ROW, 10),
    "pw-pick-row-w11": ps(PICK_ROW, 11),
    "pw-pick-row-w24": ps(PICK_ROW, 24),
    "pw-pick-slant-w24": ps(PICK_SLANT, 24, size="0.1 0.3"),
    "pw-pick-row-w24-steady": ps(PICK_ROW, 24, **STEADY),
}
COL = {
    "pw-col-w1": ps(COL_ROW, 1, lifetime=1.6),
    "pw-col-w6": ps(COL_ROW, 6, lifetime=1.6),
    "pw-col-w11": ps(COL_ROW, 11, lifetime=1.6),
    "pw-col-w24": ps(COL_ROW, 24, lifetime=1.6),
    "pw-col-w24-steady": ps(COL_ROW, 24, **STEADY),
}
SHADOW = {
    "pw-shadow-w1": ps(SHADOW_ROW, 1, size="0.1 0.4", color="1 0.5 0.2"),
    "pw-shadow-w6": ps(SHADOW_ROW, 6, size="0.1 0.4", color="1 0.5 0.2"),
    "pw-shadow-w11": ps(SHADOW_ROW, 11, size="0.1 0.4", color="1 0.5 0.2"),
    "pw-shadow-w24": ps(SHADOW_ROW, 24, size="0.1 0.4", color="1 0.5 0.2"),
    "pw-shadow-w24-steady": ps(SHADOW_ROW, 24, size="0.1 0.4", color="1 0.5 0.2", **STEADY),
    "pw-shadow-w24-hidden": ps(SHADOW_HIDDEN, 24, size="0.1 0.4", color="1 0.5 0.2"),
}


def main():
    os.makedirs(os.path.join(ROOT, "scripts"), exist_ok=True)
    base = {"dt": 1 / 30, "t0": 946684800, "frames": 45, "warmup": 8, "node": "PS", "dump": [15, 30, 45]}
    cases = ([(n, PICK_VIEW + b, {"pickProbes": {"frames": [15, 30, 45], "points": grid()}}) for n, b in PICK.items()]
             + [(n, COL_VIEW + b, {"collisionProbes": {"frames": list(range(1, 46)), "directions": DIRECTIONS,
                                                       "translations": TRANSLATIONS, "nodes": []}}) for n, b in COL.items()]
             + [(n, SHADOW_VIEW + b, {}) for n, b in SHADOW.items()])

    for i, (name, body, extra) in enumerate(cases):
        with open(os.path.join(ROOT, name + ".x3dv"), "w") as f:
            f.write(HEAD + body)

        with open(os.path.join(ROOT, "scripts", name + ".json"), "w") as f:
            json.dump(dict(base, seed=21000000 + i, **extra), f)

    print(f"{len(cases)} cases in {ROOT}")


if __name__ == "__main__":
    main()
