#!/usr/bin/env python3
"""ParticleSystem rendering (lane XITE_R_PARTICLE_RENDER_01): synthetic worlds and scripts.

  python3 -I oracle/tools/gen_rea_b13_particles.py

writes oracle/synthetic-rea-b13-particles/NAME.x3dv and scripts/NAME.json. A script is the one
oracle/tools/x_ite_particles*.mjs and crates/x3d-viewer/src/bin/x3d-particles-drive.rs read (dt, t0, frames, warmup,
seed, node, dump); the render oracle captures the 256 x 256 page after each dump frame. Every world has one
ParticleSystem named PS (the per-frame emitter seed is recorded for one system) and a Viewpoint close enough that
the particles cover many pixels. Newly authored; no historical content.
"""
import json, os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "synthetic-rea-b13-particles"))

HEAD = """#X3D V3.3 utf8
PROFILE Full
COMPONENT ParticleSystems:3
"""

VIEW = 'Viewpoint { position 0 1.2 4 orientation 1 0 0 -0.15 }\nBackground { skyColor 0.1 0.1 0.2 }\n'

CONE = "ConeEmitter { direction 0 1 0 angle 0.5 speed 1.2 variation 0.2 }"

# A 4 x 4 RGBA checker for the textured cases (no image files).
PIXELS = "PixelTexture { image 4 4 4 " + " ".join(
    ("0xff4040ff" if (x + y) % 2 else "0x40ff40c0") for y in range(4) for x in range(4)) + " }"

RAMP_RGB = "colorKey [0 0.5 1] color Color { color [1 0 0, 1 1 0, 0 0.4 1] }"
RAMP_RGBA = "colorKey [0 0.5 1] color ColorRGBA { color [1 0 0 1, 1 1 0 0.7, 0 0.4 1 0.2] }"
SCALE = "scaleKey [0 1] scale Coordinate { point [0.5 0.5 0.5, 2 1.5 1] }"
TEXRAMP = ("texCoordKey [0 0.5] texCoord TextureCoordinate { point [0 0, 0.5 0, 0.5 0.5, 0 0.5, "
           "0.5 0.5, 1 0.5, 1 1, 0.5 1] }")


def ps(geometry_type, extra="", appearance="Appearance { material Material { diffuseColor 1 1 1 } }",
       emitter=CONE, size="0.15 0.15", lifetime=3, max_particles=150):
    return (f'DEF PS ParticleSystem {{\n  geometryType "{geometry_type}"\n  maxParticles {max_particles}\n'
            f'  particleLifetime {lifetime}\n  particleSize {size}\n  {extra}\n  emitter {emitter}\n'
            f'  appearance {appearance}\n}}\n')


CASES = {
    # geometry types
    "pr-point": ps("POINT", appearance="Appearance { material Material { emissiveColor 1 1 1 } }"),
    "pr-point-props": ps("POINT", appearance="Appearance { material Material { emissiveColor 1 0.8 0.2 } "
                                             "pointProperties PointProperties { pointSizeScaleFactor 6 } }"),
    "pr-line": ps("LINE", RAMP_RGB, size="0.15 0.3"),
    "pr-line-scale": ps("LINE", SCALE, size="0.15 0.3",
                        appearance="Appearance { material Material { emissiveColor 0.2 1 0.4 } }"),
    "pr-triangle": ps("TRIANGLE"),
    "pr-quad": ps("QUAD"),
    "pr-quad-default-appearance": ps("QUAD", appearance="NULL"),
    "pr-quad-unlit": ps("QUAD", appearance="Appearance { material UnlitMaterial { emissiveColor 0.9 0.5 0.1 } }"),
    "pr-sprite": ps("SPRITE"),
    "pr-geometry-box": ps("GEOMETRY", "geometry Box { size 0.12 0.12 0.12 }", max_particles=60),
    "pr-geometry-sphere-rgb": ps("GEOMETRY", RAMP_RGB + " geometry Sphere { radius 0.08 }", max_particles=60),
    "pr-geometry-none": ps("GEOMETRY"),
    # ramps
    "pr-quad-color": ps("QUAD", RAMP_RGB),
    "pr-quad-rgba": ps("QUAD", RAMP_RGBA),
    "pr-quad-scale": ps("QUAD", SCALE),
    "pr-quad-rgba-scale": ps("QUAD", RAMP_RGBA + " " + SCALE),
    "pr-color-no-node": ps("QUAD", "colorKey [0 1]"),
    "pr-sprite-texture": ps("SPRITE", size="0.3 0.3",
                            appearance=f"Appearance {{ material Material {{ }} texture {PIXELS} }}"),
    "pr-sprite-texramp": ps("SPRITE", TEXRAMP, size="0.3 0.3",
                            appearance=f"Appearance {{ material Material {{ }} texture {PIXELS} }}"),
    "pr-sprite-texramp3": ps("SPRITE", "texCoordKey [0 0.3 0.6] texCoord TextureCoordinate { point [0 0, 0.5 0, 0.5 0.5, 0 0.5, "
                                       "0.5 0, 1 0, 1 0.5, 0.5 0.5, 0.5 0.5, 1 0.5, 1 1, 0.5 1] }", size="0.3 0.3",
                             appearance=f"Appearance {{ material Material {{ }} texture {PIXELS} }}"),
    "pr-quad-texramp-rgba": ps("QUAD", TEXRAMP + " " + RAMP_RGBA, size="0.3 0.3",
                               appearance=f"Appearance {{ material Material {{ }} texture {PIXELS} }}"),
    # emitters and physics
    "pr-explosion": ps("QUAD", RAMP_RGB, emitter="ExplosionEmitter { speed 1 variation 0.3 }", lifetime=1.5),
    "pr-point-emitter-gravity": ps("QUAD", "physics ForcePhysicsModel { force 0 -2 0 }",
                                   emitter="PointEmitter { position 0 0.5 0 direction 0 1 0 speed 1 variation 0.5 mass 1 }"),
    # bbox: culling with the emitter's box, bboxSize, bboxDisplay
    "pr-bbox-culled": ps("QUAD", "bboxSize 0.5 0.5 0.5 bboxCenter 20 0 0"),
    "pr-bbox-display": ps("QUAD", "bboxDisplay TRUE", emitter="ConeEmitter { direction 0 1 0 angle 0.5 speed 0.3 variation 0.2 }",
                          lifetime=1.5),
    "pr-bbox-display-size": ps("QUAD", "bboxDisplay TRUE bboxSize 1 2 1 bboxCenter 0 0.8 0"),
    "pr-bbox-zero-speed": ps("QUAD", "physics ForcePhysicsModel { force 0 3 0 }",
                             emitter="PointEmitter { position 0 -3 0 speed 0 mass 1 }"),
    # transforms
    "pr-transform-mirror": "Transform { scale -1 1 1 rotation 0 1 0 0.6 children [\n" + ps("QUAD", RAMP_RGB) + "] }\n",
    "pr-sprite-rotated": "Transform { rotation 1 0 0 0.8 children [\n" + ps("SPRITE", RAMP_RGB) + "] }\n",
    # shadows
    "pr-shadow": (
        "DirectionalLight { direction 0.3 -1 -0.2 shadowIntensity 0.8 shadowMapSize 1024 }\n"
        "NavigationInfo { headlight FALSE }\n"
        "Transform { translation 0 -0.2 0 children Shape { appearance Appearance { material Material { } } "
        "geometry Box { size 4 0.05 4 } } }\n" + ps("QUAD", RAMP_RGB, size="0.25 0.25")),
    "pr-shadow-off": (
        "DirectionalLight { direction 0.3 -1 -0.2 shadowIntensity 0.8 shadowMapSize 1024 }\n"
        "NavigationInfo { headlight FALSE }\n"
        "Transform { translation 0 -0.2 0 children Shape { appearance Appearance { material Material { } } "
        "geometry Box { size 4 0.05 4 } } }\n" + ps("QUAD", RAMP_RGB + " castShadow FALSE", size="0.25 0.25")),
}

# Runtime field changes (applied before the frame's step, see x_ite_particles_events.mjs).
EVENTS = {
    "pr-events-size": ("pr-quad", [{"frame": 20, "node": "PS", "field": "particleSize", "value": [0.3, 0.05]}]),
    "pr-events-disable": ("pr-quad-color", [{"frame": 20, "node": "PS", "field": "enabled", "value": False},
                                            {"frame": 30, "node": "PS", "field": "enabled", "value": True}]),
    "pr-events-visible": ("pr-quad-color", [{"frame": 20, "node": "PS", "field": "visible", "value": False}]),
    "pr-events-maxparticles": ("pr-quad-color", [{"frame": 20, "node": "PS", "field": "maxParticles", "value": 20}]),
}


def main():
    os.makedirs(os.path.join(ROOT, "scripts"), exist_ok=True)

    script = {"dt": 1 / 30, "t0": 946684800, "frames": 45, "warmup": 8, "node": "PS", "dump": [15, 25, 45]}

    for i, (name, body) in enumerate(sorted(CASES.items())):
        with open(os.path.join(ROOT, name + ".x3dv"), "w") as f:
            f.write(HEAD + VIEW + body)

        with open(os.path.join(ROOT, "scripts", name + ".json"), "w") as f:
            json.dump(dict(script, seed=1000003 * (i + 1) % 2147483647), f)

    for i, (name, (world, events)) in enumerate(sorted(EVENTS.items())):
        with open(os.path.join(ROOT, name + ".x3dv"), "w") as f:
            f.write(HEAD + VIEW + CASES[world])

        with open(os.path.join(ROOT, "scripts", name + ".json"), "w") as f:
            json.dump(dict(script, seed=7000003 * (i + 1) % 2147483647, events=events), f)

    print(f"{len(CASES) + len(EVENTS)} cases in {ROOT}")


if __name__ == "__main__":
    main()
