#!/usr/bin/env python3
"""ParticleSystem shadow-pass culling (lane XITE_R_AGENT_X_PARTICLES): synthetic worlds and scripts.

  python3 -I oracle/tools/gen_rea_b25_particle_shadow_cull.py

writes oracle/synthetic-rea-b25-particle-shadow-cull/NAME.x3dv and scripts/NAME.json. A script is the one
oracle/tools/x_ite_particles*.mjs and crates/x3d-viewer/src/bin/x3d-particles-drive.rs read (dt, t0, frames, warmup,
seed, node, dump).

X3DRenderObject.addDepthShape (the SHADOW and DEPTH traversals) keeps a shape only when `viewVolume .intersectsSphere
(bboxSize .norm () / 2, bboxCenter)` of the light's view volume holds, with the shape's bbox in the light's view space.
A ParticleSystem's default bbox is its emitter's: for a PointEmitter a cube of side 2 * particleLifetime * speed (with the
variations) at its position, which a ForcePhysicsModel does not enlarge. So a system with a speed of 0 that a force
accelerates has a bbox of size 0 at its emitter; when that point is outside a SpotLight's cone (or outside five of a
PointLight's six faces) the system draws nothing into the shadow map there although its particles fly through the cone.
An explicit bboxSize does the same. A DirectionalLight's ortho volume is the box of the group, which includes the
system's bbox, so a system is never culled by it (control).

Every world: a 6 x 6 floor, a light with shadows 3 m above it, one ParticleSystem named PS of GEOMETRY spheres (or SPRITE /
LINE) that moves through the light's volume, a Viewpoint above looking at the floor. Newly authored; no historical
content.

  sp-default-out    SpotLight; speed 0, force along -x; the default bbox is a point outside the cone: culled in X_ITE.
  sp-default-in     the same with the emitter inside the cone (control: drawn).
  sp-bbox-out       explicit bboxSize 0.4 at a center outside the cone, particles moving at 1.5 m/s: culled.
  sp-bbox-sphere    explicit bboxSize 1 at a center whose box is outside the cone but whose bounding sphere reaches into
                    it: drawn (the test is a sphere, not a box).
  sp-bbox-wide      explicit bboxSize 6 6 6 (control: drawn).
  sp-transform      the force system in a Transform (translation, rotation), outside the cone in the light's space:
                    culled.
  sp-sprite-out     SPRITE particles, default bbox outside the cone: culled.
  sp-line-out       LINE particles, 24 px, default bbox outside the cone: culled.
  pt-default        PointLight; the emitter is in the +x face and the particles fall into the -y face, the one that
                    shadows the floor: only the +x face draws them in X_ITE.
  pt-bbox-wide      the same with bboxSize 8 8 8 (control: every face draws them).
  dl-default        DirectionalLight with the default bbox (control: never culled).
"""
import json, os

ROOT = os.path.normpath(os.path.join(os.path.dirname(os.path.abspath(__file__)), "..",
                                     "synthetic-rea-b25-particle-shadow-cull"))

HEAD = "#X3D V4.0 utf8\nPROFILE Full\nCOMPONENT ParticleSystems:3\nCOMPONENT Shape:5\nCOMPONENT Lighting:3\n"
VIEW = ("NavigationInfo { headlight FALSE }\nViewpoint { position 0 4.5 4.5 orientation 1 0 0 -0.785 }\n"
        "Background { skyColor 0.1 0.1 0.2 }\n")
FLOOR = ("Shape { appearance Appearance { material Material { diffuseColor 0.8 0.8 0.8 } } "
         "geometry Box { size 6 0.1 6 } }\n")
SPOT = ("SpotLight { location 0 3 0 direction 0 -1 0 cutOffAngle 0.5 beamWidth 0.45 radius 30 ambientIntensity 0.3 "
        "shadows TRUE shadowIntensity 1 shadowMapSize 1024 }\n")
POINT = "PointLight { location 0 3 0 radius 30 ambientIntensity 0.3 shadows TRUE shadowIntensity 1 shadowMapSize 1024 }\n"
DIRECTIONAL = ("DirectionalLight { direction 0.2 -1 -0.1 ambientIntensity 0.3 shadows TRUE shadowIntensity 1 "
               "shadowMapSize 1024 }\n")
SPHERE = "geometry Sphere { radius 0.14 }"
MAT = "appearance Appearance { material Material { diffuseColor 1 0.6 0.2 } }"
FLY = "PointEmitter { position %s direction %s speed %s variation 0 mass 1 }"
def fall(position, force):
    """A speed-0 emitter and the force that accelerates the particles (default bbox: a point)."""
    return dict(emitter=FLY % (position, "0 0 1", 0), physics=f"physics ForcePhysicsModel {{ force {force} }}")


def ps(geometry="GEOMETRY", emitter=FLY % ("1.6 1.5 0", "-1 0 0", 1.5), extra="", appearance=MAT, physics=""):
    gt = {"GEOMETRY": SPHERE + "\n  ", "SPRITE": "", "LINE": ""}[geometry]
    return (f'DEF PS ParticleSystem {{\n  geometryType "{geometry}"\n  maxParticles 80\n  emissionRate 20\n'
            f'  particleLifetime 4\n  lifetimeVariation 0\n  particleSize 0.3 0.3\n  {gt}{extra}\n  {physics}\n  emitter {emitter}\n'
            f'  {appearance}\n}}\n')


def world(light, body):
    return HEAD + VIEW + light + FLOOR + body


LINE24 = ("appearance Appearance { material Material { emissiveColor 1 0.6 0.2 } "
          "lineProperties LineProperties { linewidthScaleFactor 24 } }")
CASES = {
    "sp-default-out": world(SPOT, ps(**fall("1.6 1.5 0", "-4 0 0"))),
    "sp-default-in": world(SPOT, ps(**fall("0.3 1.5 0", "-4 0 0"))),
    "sp-bbox-out": world(SPOT, ps(extra="bboxSize 0.4 0.4 0.4 bboxCenter 1.7 1.5 0")),
    "sp-bbox-sphere": world(SPOT, ps(extra="bboxSize 1 1 1 bboxCenter 1.7 1.5 0")),
    "sp-bbox-wide": world(SPOT, ps(extra="bboxSize 6 6 6 bboxCenter 0 1.5 0")),
    "sp-transform": world(SPOT, "Transform { translation 0 1.5 0 rotation 0 1 0 3.14159265 children [\n"
                          + ps(**fall("-1.6 0 0", "4 0 0")) + "] }\n"),
    "sp-sprite-out": world(SPOT, ps("SPRITE", **fall("1.6 1.5 0", "-4 0 0"))),
    "sp-line-out": world(SPOT, ps("LINE", appearance=LINE24, **fall("1.6 1.5 0", "-4 0 0"))),
    "pt-default": world(POINT, ps(**fall("1.6 2.7 0", "0 -4 0"))),
    "pt-bbox-wide": world(POINT, ps(extra="bboxSize 8 8 8", **fall("1.6 2.7 0", "0 -4 0"))),
    "dl-default": world(DIRECTIONAL, ps(**fall("1.6 1.5 0", "-4 0 0"))),
}


def main():
    os.makedirs(os.path.join(ROOT, "scripts"), exist_ok=True)
    base = {"dt": 1 / 30, "t0": 946684800, "frames": 45, "warmup": 8, "node": "PS", "dump": [15, 25, 35, 45]}

    for i, (name, body) in enumerate(CASES.items()):
        with open(os.path.join(ROOT, name + ".x3dv"), "w") as f:
            f.write(body)

        with open(os.path.join(ROOT, "scripts", name + ".json"), "w") as f:
            json.dump(dict(base, seed=25000000 + i), f)

    print(f"{len(CASES)} cases in {ROOT}")


if __name__ == "__main__":
    main()
