//! Instanced particle picking in the pointer (lane XITE_R_B25_2_GPU_INSTANCED_PICKING): `pointer::pick` sends a
//! ParticleSystem through the pointing pass as one instanced draw instead of one shape record per particle.
//!
//! Every case picks the same particles twice with the same pointing buffer:
//!
//! * as a [`ParticleRun`] — the B25-2 path, where no particle gets a `ShapeInst` or a ray cast;
//! * as the run's own [`ParticleRun::entries`] in [`PointingShapes::shapes`] — one shape per particle, which is what
//!   the pointer did before, and the form `rea_b14_particle_pick.rs` holds against pinned X_ITE 16.5.0.
//!
//! The whole [`Hit`] has to match: the hit shape, point, normal, texCoord, model-view, bounding box, sensors and
//! Anchors. A run that matches the per-particle form matches X_ITE.
//!
//! The pass needs a GPU; without an adapter the tests are skipped.

mod common;

use std::rc::Rc;
use x3d_fields::NodeId;
use x3d_fields::math::{Matrix4, Vector3, Vector4};
use x3d_render::geometry::Geometry;
use x3d_render::pointing_gpu::PointingGpu;
use x3d_render::traverse::{LineParams, PointParams, StyleParams};
use x3d_viewer::pointer::{Hit, PickCamera, PointingShapes, pick};
use x3d_viewer::walk::{NodeRef, ParticleRun, SensorRef, ShapeInst};

const SIZE: f64 = 64.0;
/// The pointer, at the center pixel's center.
const PX: f64 = 32.5;
const PY: f64 = 32.5;
/// The ParticleSystem node, and the TouchSensor and Anchor around it.
const SYSTEM: NodeId = 7;
const SENSOR: NodeId = 8;
const ANCHOR: NodeId = 9;

fn node(n: NodeId) -> NodeRef {
    NodeRef {
        path: Rc::new(vec![]),
        node: n,
    }
}

fn translation(x: f64, y: f64, z: f64) -> Matrix4 {
    let mut m = Matrix4::IDENTITY;
    m.0[12] = x;
    m.0[13] = y;
    m.0[14] = z;
    m
}

fn camera() -> PickCamera {
    PickCamera {
        projection: x3d_render::traverse::perspective(0.785_398, 0.1, 100.0, SIZE, SIZE),
        viewport: Vector4::new(0.0, 0.0, SIZE, SIZE),
        view: Matrix4::IDENTITY,
        near: 0.1,
        far: 100.0,
    }
}

/// The template of a ParticleSystem's run: `instance` None and `model_view` the system's, with a TouchSensor and an
/// Anchor in scope and the system's explicit bounding box (addPointingShape's view volume test).
fn template(geometry: Rc<Geometry>, style: Option<StyleParams>, clip: Vec<[f64; 4]>) -> ShapeInst {
    ShapeInst {
        shape: node(SYSTEM),
        geometry: SYSTEM,
        model_view: Matrix4::IDENTITY,
        shape_model_view: Matrix4::IDENTITY,
        instance: None,
        all_instances: None,
        explicit_bbox: Some((Vector3::new(-4.0, -4.0, -8.0), Vector3::new(4.0, 4.0, 8.0))),
        sensors: vec![SensorRef {
            node: node(SENSOR),
            model_view: Matrix4::IDENTITY,
        }],
        collidable: false,
        collisions: vec![],
        anchors: vec![node(ANCHOR)],
        clip_planes: clip,
        viewport: [0.0, 0.0, SIZE, SIZE],
        skin: None,
        style,
        textures: (false, false),
        particle: true,
        particle_geometry: Some(geometry),
    }
}

/// The hit as comparable bits: everything the pointer fills in, float values exactly.
fn facts(h: &Hit) -> String {
    format!(
        "id {} shape {:?} point {:?} normal {:?} tex {:?} mv {:?} bbox {:?} sensors {:?} anchors {:?}",
        h.id,
        h.shape.as_ref().map(|s| s.node),
        [
            h.point.x.to_bits(),
            h.point.y.to_bits(),
            h.point.z.to_bits()
        ],
        [
            h.normal.x.to_bits(),
            h.normal.y.to_bits(),
            h.normal.z.to_bits()
        ],
        [
            h.tex_coord.x.to_bits(),
            h.tex_coord.y.to_bits(),
            h.tex_coord.z.to_bits(),
            h.tex_coord.w.to_bits()
        ],
        h.model_view.0.map(f64::to_bits),
        h.bbox.map(|(a, b)| (
            [a.x.to_bits(), a.y.to_bits(), a.z.to_bits()],
            [b.x.to_bits(), b.y.to_bits(), b.z.to_bits()]
        )),
        h.sensors.iter().map(|s| s.node.node).collect::<Vec<_>>(),
        h.anchors.iter().map(|a| a.node).collect::<Vec<_>>(),
    )
}

/// Pick with the particles as one run, and with the run's entries as shapes of their own.
fn both(gpu: &mut PointingGpu, run: &ParticleRun) -> (Hit, Hit) {
    let cam = camera();
    let entries: Vec<ShapeInst> = run.entries().collect();
    let geometry = |s: &ShapeInst| s.particle_geometry.clone();
    let grouped = pick(
        PointingShapes {
            shapes: &[],
            particles: std::slice::from_ref(run),
        },
        &mut |s| geometry(s),
        &cam,
        PX,
        PY,
        false,
        Some(gpu),
    );
    let separate = pick(
        PointingShapes {
            shapes: &entries,
            particles: &[],
        },
        &mut |s| geometry(s),
        &cam,
        PX,
        PY,
        false,
        Some(gpu),
    );

    (grouped, separate)
}

fn gpu() -> Option<PointingGpu> {
    let (device, queue) = common::gpu::device()?;
    Some(PointingGpu::new(device, queue))
}

/// The geometry of each particle type, as `x3d_render::traverse::particle_simple` builds it.
fn geometries() -> Vec<(&'static str, Rc<Geometry>, Option<StyleParams>)> {
    let point_props = Some(StyleParams::Point(PointParams {
        scale_factor: 8.0,
        min_value: 0.0,
        max_value: 10.0,
        attenuation: [1.0, 0.0, 0.0],
    }));
    let mut quad = x3d_render::particles::quad_geometry(None, false);
    quad.normals = vec![0.0; quad.normals.len()];
    quad.tex_coords = [0.0, 0.0, 0.0, 1.0].repeat(quad.vertex_count());

    vec![
        (
            "POINT",
            Rc::new(x3d_render::particles::point_geometry(false)),
            None,
        ),
        (
            "POINT with PointProperties",
            Rc::new(x3d_render::particles::point_geometry(false)),
            point_props,
        ),
        (
            "LINE aliased",
            Rc::new(x3d_render::particles::line_geometry(0.3, false)),
            Some(StyleParams::Line(LineParams {
                linetype: 1,
                width: 3.0,
            })),
        ),
        (
            "LINE thick",
            Rc::new(x3d_render::particles::line_geometry(0.3, false)),
            Some(StyleParams::Line(LineParams {
                linetype: 1,
                width: x3d_render::gpu::MAX_LINE_WIDTH + 4.0,
            })),
        ),
        ("QUAD", Rc::new(quad), None),
        // A GEOMETRY particle draws the `geometry` node's own geometry, with its normals and texture coordinates
        // (unlike QUAD, whose attributes displaySimple leaves disabled): the hit normal and texCoord come from the
        // interpolated varyings, so this is the case that checks them.
        ("GEOMETRY", Rc::new(two_triangles()), None),
    ]
}

/// Two triangles in the XY plane with normals and texture coordinates, as a `geometry` node would give them.
fn two_triangles() -> Geometry {
    let xy: [[f32; 2]; 6] = [
        [-0.4, -0.4],
        [0.4, -0.4],
        [0.4, 0.4],
        [-0.4, -0.4],
        [0.4, 0.4],
        [-0.4, 0.4],
    ];

    Geometry {
        geometry_type: 3,
        vertices: xy.iter().flat_map(|p| [p[0], p[1], 0.0, 1.0]).collect(),
        // Normals that differ per vertex, so an interpolated hit normal is visible.
        normals: xy
            .iter()
            .flat_map(|p| {
                let n = (p[0] * p[0] + p[1] * p[1] + 1.0).sqrt();
                [p[0] / n, p[1] / n, 1.0 / n]
            })
            .collect(),
        tex_coords: xy
            .iter()
            .flat_map(|p| [p[0] + 0.5, p[1] + 0.5, 0.0, 1.0])
            .collect(),
        min: Vector3::new(-0.4, -0.4, 0.0),
        max: Vector3::new(0.4, 0.4, 0.0),
        solid: false,
        ccw: true,
        ..Default::default()
    }
}

/// A run and its particles drawn one by one give the same hit, for every particle geometry, at equal depth and at
/// mixed depths, with and without clip planes.
#[test]
fn a_run_hits_like_its_particles_on_their_own() {
    let Some(mut g) = gpu() else {
        return eprintln!("no adapter");
    };
    // ClipPlanes2.glsl: a fragment with dot (vertex, normal) < distance is discarded, so this keeps z <= -1.8.
    let near_cut = vec![[0.0, 0.0, -1.0, 1.8]];

    for (name, geometry, style) in geometries() {
        for (skinned, skin) in [("", None), (" skinned", Some(one_joint_skin()))] {
            for (depths, zs) in cases() {
                for (clipped, clip) in [("", vec![]), (" clipped", near_cut.clone())] {
                    let off = depths == "far off the pointer";
                    let matrices = Rc::new(
                        zs.iter()
                            .map(|&z| {
                                if off {
                                    translation(20.0, 0.0, z)
                                } else {
                                    translation(0.0, 0.0, z)
                                }
                            })
                            .collect::<Vec<_>>(),
                    );
                    let run = ParticleRun {
                        at: 0,
                        template: ShapeInst {
                            skin: skin.clone(),
                            ..template(geometry.clone(), style, clip.clone())
                        },
                        matrices,
                    };
                    let (grouped, separate) = both(&mut g, &run);

                    assert_eq!(
                        facts(&grouped),
                        facts(&separate),
                        "{name}{skinned}, {depths}{clipped}"
                    );

                    if off {
                        assert!(
                            !grouped.id,
                            "{name}{skinned}, {depths}{clipped}: a hit off the pointer"
                        );
                    }
                }
            }
        }
    }
}

/// The depth arrangements every case runs through.
fn cases() -> Vec<(&'static str, Vec<f64>)> {
    vec![
        // Coincident particles: every covered pixel is a depth tie, and LEQUAL keeps the last drawn.
        ("equal depth", vec![-2.0, -2.0, -2.0, -2.0]),
        ("nearest in the middle", vec![-2.0, -1.5, -2.5]),
        ("nearest last", vec![-2.5, -2.0, -1.5]),
        ("nearest first", vec![-1.5, -2.0, -2.5]),
        ("a tie among the nearest", vec![-2.0, -1.5, -2.5, -1.5]),
        ("one particle", vec![-2.0]),
        // Nothing near the pointer's pixel: a miss for both forms.
        ("far off the pointer", vec![-2.0, -2.0]),
    ]
}

/// An HAnimHumanoid's skin tables around the system: one joint that shifts every point, all weight on it. The
/// pointing shader skins the geometry of the draw, so the run and its particles have to be skinned alike.
fn one_joint_skin() -> Rc<(x3d_render::hanim::SkinTables, Vec<f32>)> {
    let tables = x3d_render::hanim::SkinTables {
        length: 1,
        num_joints: 4,
        // Point 0: joint indices 0 0 0 0, then the weights 1 0 0 0.
        joints: vec![0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0],
        num_displacements: 0,
        displacements: vec![],
        displacement_weights: vec![],
    };
    let mut joint = Matrix4::IDENTITY;
    joint.0[13] = 0.05;

    Rc::new((tables, joint.0.map(|v| v as f32).to_vec()))
}

/// The normal GPU path never looks at a particle on its own: it asks for one geometry per ParticleSystem and computes
/// no CPU hit. Only the ray cast expands a run, and it then needs every particle.
#[test]
fn the_gpu_path_does_not_touch_the_particles_one_by_one() {
    let Some(mut g) = gpu() else {
        return eprintln!("no adapter");
    };
    let quad = Rc::new(x3d_render::particles::quad_geometry(None, false));
    let run = ParticleRun {
        at: 0,
        template: template(quad, None, vec![]),
        matrices: Rc::new(vec![translation(0.0, 0.0, -2.0); 2_000]),
    };
    let cam = camera();
    let shapes = PointingShapes {
        shapes: &[],
        particles: std::slice::from_ref(&run),
    };
    let mut asked = 0;
    let hit = pick(
        shapes,
        &mut |s: &ShapeInst| {
            asked += 1;
            s.particle_geometry.clone()
        },
        &cam,
        PX,
        PY,
        false,
        Some(&mut g),
    );

    assert!(hit.id);
    assert_eq!(asked, 1, "the GPU path resolved {asked} geometries");
    assert_eq!(g.draws, 1, "the system took {} draws", g.draws);

    // Without a pointing buffer the ray cast decides, and it has to test every particle.
    let mut asked_cpu = 0;
    let fallback = pick(
        shapes,
        &mut |s: &ShapeInst| {
            asked_cpu += 1;
            s.particle_geometry.clone()
        },
        &cam,
        PX,
        PY,
        false,
        None,
    );

    assert!(fallback.id);
    assert_eq!(asked_cpu, run.len());
}

/// A run with no particles is a miss and leaves no stale hit, and a run whose system's bounding box is outside the
/// view volume is not drawn at all (addPointingShape).
#[test]
fn an_empty_or_culled_run_is_a_miss() {
    let Some(mut g) = gpu() else {
        return eprintln!("no adapter");
    };
    let quad = Rc::new(x3d_render::particles::quad_geometry(None, false));
    let hit = |run: &ParticleRun, g: &mut PointingGpu| both(g, run).0.id;

    let some = ParticleRun {
        at: 0,
        template: template(quad.clone(), None, vec![]),
        matrices: Rc::new(vec![translation(0.0, 0.0, -2.0)]),
    };
    assert!(hit(&some, &mut g));

    let empty = ParticleRun {
        matrices: Rc::new(vec![]),
        ..some.clone()
    };
    assert!(!hit(&empty, &mut g));

    let culled = ParticleRun {
        template: ShapeInst {
            explicit_bbox: Some((Vector3::new(-0.1, -0.1, 9.9), Vector3::new(0.1, 0.1, 10.1))),
            ..some.template.clone()
        },
        ..some.clone()
    };
    assert!(!hit(&culled, &mut g));

    // And the run still hits afterwards: the misses left nothing behind in the instance buffer.
    assert!(hit(&some, &mut g));
}

/// A ParticleSystem among ordinary shapes keeps its place in the drawing order: a shape in front of it wins, one
/// behind it loses, and a shape at the same depth drawn after it wins the tie.
#[test]
fn a_run_keeps_its_place_among_the_shapes() {
    let Some(mut g) = gpu() else {
        return eprintln!("no adapter");
    };
    let quad = Rc::new(x3d_render::particles::quad_geometry(None, false));
    let wall = |z: f64, n: NodeId| ShapeInst {
        shape: node(n),
        geometry: n,
        model_view: translation(0.0, 0.0, z),
        shape_model_view: translation(0.0, 0.0, z),
        explicit_bbox: None,
        sensors: vec![],
        anchors: vec![],
        particle: false,
        particle_geometry: Some(quad.clone()),
        ..template(quad.clone(), None, vec![])
    };
    let run = ParticleRun {
        at: 1,
        template: template(quad.clone(), None, vec![]),
        matrices: Rc::new(vec![translation(0.0, 0.0, -2.0)]),
    };
    let cam = camera();
    let hit = |shapes: &[ShapeInst], runs: &[ParticleRun], g: &mut PointingGpu| {
        pick(
            PointingShapes {
                shapes,
                particles: runs,
            },
            &mut |s: &ShapeInst| s.particle_geometry.clone(),
            &cam,
            PX,
            PY,
            false,
            Some(g),
        )
        .shape
        .map(|s| s.node)
    };

    // In front of the particles.
    assert_eq!(
        hit(&[wall(-1.0, 1)], std::slice::from_ref(&run), &mut g),
        Some(1)
    );
    // Behind them.
    assert_eq!(
        hit(&[wall(-3.0, 1)], std::slice::from_ref(&run), &mut g),
        Some(SYSTEM)
    );
    // At the same depth: `at` 1 puts the run after `shapes [0 .. 1]`, so the particles are drawn last and the LEQUAL
    // depth test gives them the tie.
    assert_eq!(
        hit(&[wall(-2.0, 1)], std::slice::from_ref(&run), &mut g),
        Some(SYSTEM)
    );

    // `at` 0 puts the run first, so the shape is drawn last and wins the same tie.
    let first = ParticleRun {
        at: 0,
        ..run.clone()
    };
    assert_eq!(
        hit(&[wall(-2.0, 1)], std::slice::from_ref(&first), &mut g),
        Some(1)
    );
}
