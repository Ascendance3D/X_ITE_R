#!/usr/bin/env python3
"""Wide and stippled instanced lines (lane XITE_R_PARTICLE_THICK_LINES_01): synthetic worlds and scripts.

  python3 -I oracle/tools/gen_rea_b18_particle_lines.py

writes oracle/synthetic-rea-b18-particle-lines/NAME.x3dv and scripts/NAME.json in the format of
gen_rea_b13_particles.py (dt, t0, frames, warmup, seed, node, dump).

pl-*: ParticleSystem LINE particles with LineProperties. The ConeEmitter direction is off every axis and the cases with a
scale ramp stretch the instance matrix, so a width computed before the particle's instance matrix is visibly wrong.
Widths 2 to 10 are gl.lineWidth lines on the oracle GPU (ALIASED_LINE_WIDTH_RANGE [1] = 10), 11 and 24 are
LineProperties.getTransformLines triangles (X3DLineGeometryNode.displayInstancedThick).

inst-line-*: the same LineProperties on an InstancedShape IndexedLineSet (X3DLineGeometryNode.displayInstanced). These
worlds also hold a disabled ParticleSystem named PS so that the particle oracle and x3d-particles-drive can step them; it
draws nothing and records no particles. Newly authored; no historical content.
"""
import json, os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b18-particle-lines"))

HEAD = """#X3D V4.0 utf8
PROFILE Full
COMPONENT ParticleSystems:3
COMPONENT Shape:4
COMPONENT X_ITE:1
"""

VIEW = 'Viewpoint { position 0 1.2 4 orientation 1 0 0 -0.15 }\nBackground { skyColor 0.1 0.1 0.2 }\n'

CONE = "ConeEmitter { direction 0.5 1 0.3 angle 0.4 speed 1.1 variation 0.2 }"

RAMP_RGB = "colorKey [0 0.5 1] color Color { color [1 0 0, 1 1 0, 0 0.4 1] }"
SCALE = "scaleKey [0 1] scale Coordinate { point [0.5 0.5 0.5, 2 2.5 1] }"


def lines(width=None, linetype=None):
    fields = ([f"linewidthScaleFactor {width}"] if width is not None else []) + \
             ([f"linetype {linetype}"] if linetype is not None else [])

    return "lineProperties LineProperties { " + " ".join(fields) + " }"


def appearance(width=None, linetype=None, color="0.3 1 0.5"):
    return f"Appearance {{ material Material {{ emissiveColor {color} }} {lines(width, linetype)} }}"


def ps(width=None, linetype=None, extra="", size="0.15 0.3", max_particles=40):
    return (f'DEF PS ParticleSystem {{\n  geometryType "LINE"\n  maxParticles {max_particles}\n'
            f'  particleLifetime 3\n  particleSize {size}\n  {extra}\n  emitter {CONE}\n'
            f'  appearance {appearance(width, linetype)}\n}}\n')


DISABLED_PS = 'DEF PS ParticleSystem { enabled FALSE geometryType "LINE" }\n'

INSTANCES = ("translations [ -0.6 0.2 0, 0.6 0.9 -0.5, 0 1.5 0.3 ] "
             "rotations [ 0 1 0 0.6, 1 0 0 -0.9, 0.3 0.5 1 1.2 ] "
             "scales [ 0.6 0.6 0.6, 1.5 0.5 1, 1 2 1 ]")


def instanced(width=None, linetype=None):
    return (DISABLED_PS +
            f"InstancedShape {{ {INSTANCES}\n  appearance {appearance(width, linetype, '1 0.6 0.2')}\n"
            "  geometry IndexedLineSet { coord Coordinate { point [ -0.4 -0.3 0, 0.4 -0.3 0, 0 0.4 0.2 ] } "
            "coordIndex [ 0 1 2 0 -1 ] } }\n")


CASES = {
    "pl-width-2": ps(2),
    "pl-width-6": ps(6),
    "pl-width-10": ps(10),
    "pl-width-11": ps(11),
    "pl-width-24": ps(24, max_particles=25),
    "pl-dashed": ps(linetype=2),
    "pl-dashed-width-24": ps(24, 2, max_particles=25),
    "pl-width-6-scale-ramp": ps(6, extra=RAMP_RGB + " " + SCALE),
    "pl-width-24-mirror": "Transform { scale -1 1 1 rotation 0 1 0 0.6 children [\n" + ps(24, max_particles=25) + "] }\n",
    "inst-line-width-6": instanced(6),
    "inst-line-width-24": instanced(24),
    "inst-line-dashed": instanced(linetype=2),
}


def main():
    os.makedirs(os.path.join(ROOT, "scripts"), exist_ok=True)

    script = {"dt": 1 / 30, "t0": 946684800, "frames": 45, "warmup": 8, "node": "PS", "dump": [15, 25, 45]}

    for i, (name, body) in enumerate(sorted(CASES.items())):
        with open(os.path.join(ROOT, name + ".x3dv"), "w") as f:
            f.write(HEAD + VIEW + body)

        with open(os.path.join(ROOT, "scripts", name + ".json"), "w") as f:
            json.dump(dict(script, seed=1800017 * (i + 1) % 2147483647), f)

    print(f"{len(CASES)} cases in {ROOT}")


if __name__ == "__main__":
    main()
