#!/usr/bin/env python3
"""Wide LINE particle shadows of one ParticleSystem in two layers (lane XITE_R_B22_TWO_LAYER_PARTICLE_SHADOWS):
synthetic worlds and scripts.

  python3 -I -B oracle/tools/gen_rea_b22_two_layer_shadows.py

writes oracle/synthetic-rea-b22-two-layer-shadows/NAME.x3dv and scripts/NAME.json for `x3d-oracle particles`.

X_ITE 16.5.0 keeps the triangles of transformed lines (a width above ALIASED_LINE_WIDTH_RANGE [1]) on the
ParticleSystem (X3DLineGeometryNode.displayInstancedThick, `shapeNode [_numLines0]`), and every renderObject.draw renders
its shadow maps before its shapes. With layers drawn in order, a layer's shadow maps therefore draw the triangles of the
last color pass before them: the previous frame's last layer for the first layer, the first layer's of this frame for the
second. They draw nothing while the stored line count is not the current one.

Every world is a LayerSet of two layers side by side (Viewport clipBoundary, left and right half of the canvas), each with
the batch 21 floor and shadow-casting DirectionalLight, and LINE particles whose count changes while particles are
emitted. The worlds carry an `emissionRate` field, which ParticleSystem does not have: X_ITE ignores it with "Unknown
field 'emissionRate' in class 'ParticleSystem'." (in every baseline's log), and so does the port (the particle counts
match). The particles are emitted at X_ITE's rate of maxParticles / particleLifetime per second (10 in every case), so
the count rises by one every three or four frames. The field stays so that the worlds remain those of the recorded
baselines.

tl-shared-w24: one 24 px system in both layers (DEF in the first, USE in the second), the same viewpoint.
tl-shared-w24-near: the same with the second layer's viewpoint nearer (its triangles are built with another camera).
tl-first-w24, tl-second-w24: the system in one layer only (controls).
tl-shared-w6: a shared 6 px system (gl.lineWidth, no stored triangles; control).
tl-shared-w24-steady: a shared 24 px system of short-lived particles (maxParticles 4, particleLifetime 0.4): the count
  rises to 4 by frame 14 and then stays 4 while dead particles are replaced (it changes in frames 3, 6, 10 and 14).
"""
import json, os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                                     "synthetic-rea-b22-two-layer-shadows"))

HEAD = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\nCOMPONENT Shape:5\nCOMPONENT Layering:1\n"
LIGHT = "DirectionalLight { direction 0.15 -1 -0.3 ambientIntensity 0.3 shadows TRUE shadowIntensity 1 }\n"
FLOOR = ("Transform { translation 0 -0.05 0 children Shape { appearance Appearance { material Material "
         "{ diffuseColor 0.8 0.8 0.8 } } geometry Box { size 4 0.1 4 } } }\n")
VP = "Viewpoint { position 0 1.6 3.2 orientation 1 0 0 -0.45 }\n"
VP_NEAR = "Viewpoint { position 0 1.2 2.2 orientation 1 0 0 -0.45 }\n"
ROW = "PointEmitter { position -0.7 0.6 0.4 direction 1 0 -0.2 speed 0.8 variation 0 }"


def ps(width, name="PS", max_particles=20, lifetime=2, rate=8):
    lines = f" lineProperties LineProperties {{ linewidthScaleFactor {width} }}" if width > 1 else ""
    return (f'DEF {name} ParticleSystem {{\n  geometryType "LINE"\n  maxParticles {max_particles}\n'
            f'  emissionRate {rate}\n  particleLifetime {lifetime}\n  lifetimeVariation 0\n  particleSize 0.1 0.4\n'
            f'  emitter {ROW}\n  appearance Appearance {{ material Material {{ emissiveColor 1 0.5 0.2 }}{lines} }}\n}}\n')


def layer(clip, body, vp=VP):
    return (f"Layer {{ viewport Viewport {{ clipBoundary [{clip}] }} children [\n"
            f'NavigationInfo {{ headlight FALSE }}\n{vp}{LIGHT}{FLOOR}{body}] }}\n')


def world(first, second, vp2=VP):
    return (HEAD + "Background { skyColor 0.1 0.1 0.2 }\n"
            "LayerSet { activeLayer 1 order [1 2] layers [\n"
            + layer("0 0.5 0 1", first) + layer("0.5 1 0 1", second, vp2) + "] }\n")


CASES = {
    "tl-shared-w24": world(ps(24), "USE PS\n"),
    "tl-shared-w24-near": world(ps(24), "USE PS\n", VP_NEAR),
    "tl-first-w24": world(ps(24), ""),
    "tl-second-w24": world("", ps(24)),
    "tl-shared-w6": world(ps(6), "USE PS\n"),
    "tl-shared-w24-steady": world(ps(24, max_particles=4, lifetime=0.4, rate=100), "USE PS\n"),
}


def main():
    os.makedirs(os.path.join(ROOT, "scripts"), exist_ok=True)
    base = {"dt": 1 / 30, "t0": 946684800, "frames": 45, "warmup": 8, "node": "PS",
            "dump": list(range(2, 21)) + [30, 45]}

    for i, (name, body) in enumerate(CASES.items()):
        with open(os.path.join(ROOT, name + ".x3dv"), "w") as f:
            f.write(body)

        with open(os.path.join(ROOT, "scripts", name + ".json"), "w") as f:
            json.dump(dict(base, seed=22000000 + i), f)

    print(f"{len(CASES)} cases in {ROOT}")


if __name__ == "__main__":
    main()
