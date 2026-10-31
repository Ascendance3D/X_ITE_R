//! Pointing device sensors: Browser/PointingDeviceSensor/X3DPointingDeviceSensorContext.js,
//! PointingDevice.js, Components/PointingDeviceSensor/{X3DPointingDeviceSensorNode,
//! X3DTouchSensorNode, X3DDragSensorNode, TouchSensor, PlaneSensor, CylinderSensor,
//! SphereSensor}.js and Components/Networking/Anchor.js (its hidden TouchSensor).
//!
//! X_ITE picks by rendering the pointer's pixel into a 1x1 float buffer (vertex position,
//! normal, texCoord in view space). Here the pointer ray is cast against the triangles of the
//! visible shapes instead; the hit values are the same (up to float precision).

use crate::geom::{Cylinder3, Line2, Line3, Plane3, Sphere3, Triangle3, ViewVolume, interval};
use crate::walk::{NodeRef, ParticleRun, SensorRef, ShapeInst, rt_at_mut};
use std::rc::Rc;
use x3d_fields::math::{Matrix3, Matrix4, Rotation4, Vector2, Vector3, Vector4};
use x3d_fields::{FieldValue, NodeId};
use x3d_render::geometry::Geometry;
use x3d_render::pointing_gpu::{PointingGpu, PointingTriangle};
use x3d_runtime::Runtime;

#[derive(Clone, Debug)]
pub struct Hit {
    pub id: bool,
    pub pointer: Vector2,
    /// The pointer ray in view space (X3DRenderObject.getHitRay).
    pub ray: Line3,
    pub sensors: Vec<SensorRef>,
    pub view_matrix: Matrix4,
    pub model_view: Matrix4,
    pub point: Vector3,
    pub normal: Vector3,
    pub tex_coord: Vector4,
    pub shape: Option<NodeRef>,
    /// The hit shape's geometry bounding box in its local space (`hit .shapeNode .getBBox ()`).
    pub bbox: Option<(Vector3, Vector3)>,
    pub anchors: Vec<NodeRef>,
    /// hit .layerNode: the layer the pick ran in.
    pub layer: x3d_render::layer::LayerKey,
}

impl Default for Hit {
    fn default() -> Hit {
        Hit {
            id: false,
            pointer: Vector2::ZERO,
            ray: Line3::new(Vector3::ZERO, Vector3::new(0.0, 0.0, -1.0)),
            sensors: vec![],
            view_matrix: Matrix4::IDENTITY,
            model_view: Matrix4::IDENTITY,
            point: Vector3::ZERO,
            normal: Vector3::ZERO,
            tex_coord: Vector4::ZERO,
            shape: None,
            bbox: None,
            layer: None,
            anchors: vec![],
        }
    }
}

/// What the viewer needs to know about an Anchor click (the UI reports it, nothing is opened).
#[derive(Clone, Debug)]
pub struct AnchorActivation {
    pub node: NodeRef,
    pub description: String,
    pub urls: Vec<String>,
    pub parameters: Vec<String>,
}

/// Camera for picking: projection matrix and viewport (x, y, w, h).
#[derive(Clone, Copy, Debug)]
pub struct PickCamera {
    pub projection: Matrix4,
    pub viewport: Vector4,
    pub view: Matrix4,
    pub near: f64,
    pub far: f64,
}

/// X3DRenderObject.setHitRay.
pub fn hit_ray(cam: &PickCamera, x: f64, y: f64) -> Line3 {
    let mut ray = Line3::new(Vector3::ZERO, Vector3::new(0.0, 0.0, -1.0));
    ViewVolume::unproject_ray(
        x,
        y,
        &Matrix4::IDENTITY,
        &cam.projection,
        &cam.viewport,
        &mut ray,
    );
    ray
}

struct TriHit {
    /// The ray is inside the triangle (within [`EDGE`]); the others are near enough that the
    /// GPU's snapped vertices might still cover the pixel centre.
    strict: bool,
    t: f64,
    u: f64,
    v: f64,
    tri: usize,
}

/// Möller-Trumbore in object space; both faces.
/// The nearest triangle hit along the ray whose point `keep` accepts (the pointing shader
/// discards clipped fragments, so a clipped hit lets the ray reach the triangles behind it).
fn ray_triangles(
    g: &Geometry,
    origin: &Vector3,
    dir: &Vector3,
    keep: &dyn Fn(f64) -> bool,
) -> Vec<TriHit> {
    if g.geometry_type < 2 {
        return vec![];
    }

    let mut hits: Vec<TriHit> = vec![];

    let vert = |k: usize| {
        let v = &g.vertices[k * 4..k * 4 + 3];
        Vector3::new(v[0] as f64, v[1] as f64, v[2] as f64)
    };

    for tri in 0..g.vertex_count() / 3 {
        let (a, b, c) = (vert(tri * 3), vert(tri * 3 + 1), vert(tri * 3 + 2));
        let mut e1 = b;
        e1.subtract(&a);
        let mut e2 = c;
        e2.subtract(&a);
        let mut p = *dir;
        p.cross(&e2);
        let det = e1.dot(&p);

        if det.abs() < 1e-14 {
            continue;
        }

        let inv = 1.0 / det;
        let mut s = *origin;
        s.subtract(&a);
        let u = s.dot(&p) * inv;

        // A ray through an edge belongs to both neighbours; the GPU's fill rule picks one.
        if !(-NEAR..=1.0 + NEAR).contains(&u) {
            continue;
        }

        let mut q = s;
        q.cross(&e1);
        let v = dir.dot(&q) * inv;

        if v < -NEAR || u + v > 1.0 + NEAR {
            continue;
        }

        let t = e2.dot(&q) * inv;

        if keep(t) {
            hits.push(TriHit {
                strict: (-EDGE..=1.0 + EDGE).contains(&u) && v >= -EDGE && u + v <= 1.0 + EDGE,
                t,
                u,
                v,
                tri,
            });
        }
    }

    hits
}

/// Barycentric slack of the ray cast: the pointing shader decides the triangles on an edge.
const EDGE: f64 = 1e-9;

/// Candidates for the GPU: triangles whose edge the ray passes within this barycentric distance
/// (the rasterizer snaps vertices to 1/256 pixel).
const NEAR: f64 = 0.05;

/// The primitives of one shape that can cover the pixel under the pointer.
enum Prims {
    /// Ray-cast triangles (strict hits and near misses).
    Triangles(Vec<TriHit>),
    /// Point indices.
    Points(Vec<usize>),
    /// Segment indices (vertices 2k and 2k + 1).
    Lines(Vec<usize>),
}

/// One primitive the CPU ray cast hits at the pointer's pixel: the values of the hit and what the depth buffer needs.
struct CpuHit {
    /// View-space distance.
    dist: f64,
    point: Vector3,
    normal: Vector3,
    tex_coord: Vector4,
    /// Normalized device z of the hit (before gl.depthRange).
    ndc_z: f64,
    /// Triangles only: the largest change of ndc z per pixel along x or y (the polygon offset's slope); points and lines
    /// have no polygon offset.
    slope: Option<f64>,
}

/// A shape the CPU ray cast found primitives near the pointer in; the GPU path keeps no such record.
struct Candidate {
    shape: usize,
    geometry: Rc<Geometry>,
    /// The CPU's own hits, in the pointing pass's drawing order (the primitive index).
    cpu: Vec<CpuHit>,
}

/// ndc z of a view-space point.
fn ndc_z(projection: &Matrix4, eye: &Vector3) -> f64 {
    let mut clip = Vector4::new(eye.x, eye.y, eye.z, 1.0);
    projection.mult_vec_matrix4(&mut clip);
    clip.z / clip.w
}

/// The pointing objects of a layer (X3DRenderObject.pointingShapes): the shapes in traversal order and the
/// ParticleSystems among them, each as one run with its particles' instance matrices.
///
/// Nothing here holds a record per particle. The pointing pass draws a system at once
/// ([`PointingTriangle::instances`]), so the normal GPU path never builds one; the CPU ray cast needs a shape of its
/// own per particle and expands the runs itself ([`PointingShapes::expanded`]) when it runs.
#[derive(Clone, Copy)]
pub struct PointingShapes<'a> {
    pub shapes: &'a [ShapeInst],
    pub particles: &'a [ParticleRun],
}

/// One pointing object of the pass: a shape, or a ParticleSystem with every drawn particle of one run.
struct Group<'a> {
    /// The shape, or the system's template (`instance` None, `model_view` the system's).
    shape: &'a ShapeInst,
    run: Option<&'a ParticleRun>,
}

impl<'a> PointingShapes<'a> {
    /// The pointing objects in traversal order, a ParticleSystem as one group: the order of
    /// [`crate::walk::Scan::pointing_shapes`] with each system's particles collapsed into their run.
    fn groups(&self) -> Vec<Group<'a>> {
        let mut out = Vec::with_capacity(self.shapes.len() + self.particles.len());
        let mut next = 0;

        for r in self.particles {
            out.extend(self.shapes[next..r.at].iter().map(|s| Group {
                shape: s,
                run: None,
            }));
            out.push(Group {
                shape: &r.template,
                run: Some(r),
            });
            next = r.at;
        }

        out.extend(self.shapes[next..].iter().map(|s| Group {
            shape: s,
            run: None,
        }));
        out
    }

    /// The pointing objects with a shape record per particle (X3DRenderObject.pointingShapes as the ray cast needs
    /// it): the expansion the GPU path avoids.
    fn expanded(&self) -> std::borrow::Cow<'a, [ShapeInst]> {
        if self.particles.iter().all(|r| r.len() == 0) {
            return std::borrow::Cow::Borrowed(self.shapes);
        }

        let len = self.shapes.len() + self.particles.iter().map(ParticleRun::len).sum::<usize>();
        let mut out = Vec::with_capacity(len);
        let mut next = 0;

        for r in self.particles {
            out.extend_from_slice(&self.shapes[next..r.at]);
            out.extend(r.entries());
            next = r.at;
        }

        out.extend_from_slice(&self.shapes[next..]);
        std::borrow::Cow::Owned(out)
    }
}

/// A run of x3d_Ids one draw of the pointing pass owns: `count` is 1 for a shape and the particle count for a
/// ParticleSystem, whose particle k carries `base + k`.
struct Block<'a> {
    group: Group<'a>,
    geometry: Rc<Geometry>,
    base: f32,
    count: usize,
}

/// The pick the pointing buffer decides: the winning shape (a particle's record built here and only here), its
/// geometry and the sample. `Ok(None)` is a miss; `Err(())` means no answer came back, and the caller falls back to
/// the CPU ray cast.
#[allow(clippy::too_many_arguments)]
fn gpu_pick(
    gpu: &mut PointingGpu,
    shapes: PointingShapes,
    geometry: &mut dyn FnMut(&ShapeInst) -> Option<Rc<Geometry>>,
    cam: &PickCamera,
    x: f64,
    y: f64,
    px: f64,
    py: f64,
    pointing: bool,
) -> Result<
    Option<(
        ShapeInst,
        Rc<Geometry>,
        x3d_render::pointing_gpu::PointingSample,
    )>,
    (),
> {
    let groups = shapes.groups();
    let sray = hit_ray(cam, px, py);
    let mut draws: Vec<PointingTriangle> = vec![];
    let mut blocks: Vec<Block> = vec![];
    // x3d_Id 0 is "no hit" in the cleared buffer (PointingBuffer.getHit), so the ids start at 1.
    let mut next = 1.0f32;

    for group in groups {
        let s = group.shape;
        let Some(g) = pointing_geometry(s, geometry, cam, x, y, pointing) else {
            continue;
        };

        let (draw, count) = match group.run {
            // One instanced draw of the system: no ray cast, no record per particle.
            Some(run) => match particle_draw(s, &g, run, cam, x, y, next) {
                Some(draw) => (draw, run.len()),
                None => continue,
            },
            // A shape sends only the primitives that can cover the pixel; its CPU hit is not computed.
            None => match shape_candidate(s, &g, cam, px, py, &sray, false) {
                Some((prims, _)) => (pointing_draw(s, &g, &prims, cam, x, y, next), 1),
                None => continue,
            },
        };

        next += count as f32;
        draws.push(draw);
        blocks.push(Block {
            group,
            geometry: g,
            base: next - count as f32,
            count,
        });
    }

    let Some(sample) = gpu.sample_all(&draws).map_err(|_| ())? else {
        return Ok(None);
    };
    // The id names the draw and, inside a ParticleSystem's block, the particle.
    let Some(block) = blocks
        .iter()
        .find(|b| sample.id >= b.base && sample.id < b.base + b.count as f32)
    else {
        // An id outside every block cannot happen: treat it as a miss rather than guess a shape.
        debug_assert!(false, "pointing id {} is in no block", sample.id);
        return Ok(None);
    };
    let k = (sample.id - block.base) as usize;
    let shape = match block.group.run {
        Some(run) => run.entry(k),
        None => block.group.shape.clone(),
    };

    Ok(Some((shape, block.geometry.clone(), sample)))
}

/// gl_PointSize of the pointing shader in pixels: 2, or max (getPointSize (vertex), 2) with PointProperties (PointSize2.glsl).
fn point_size(
    style: Option<&x3d_render::traverse::StyleParams>,
    textures: (bool, bool),
    eye: &Vector3,
) -> f64 {
    let Some(x3d_render::traverse::StyleParams::Point(p)) = style else {
        return 2.0;
    };
    let dl = eye.norm();
    let size = (p.scale_factor
        / (p.attenuation[0] + p.attenuation[1] * dl + p.attenuation[2] * dl * dl))
        .clamp(p.min_value, p.max_value);
    let size = if textures.0 || size <= 1.0 {
        size
    } else {
        size + 1.0
    };

    size.max(2.0)
}

/// The width of an applied LineProperties (gl.lineWidth, or the transformed triangles above the aliased range); 1 without.
fn line_width(style: Option<&x3d_render::traverse::StyleParams>) -> f64 {
    match style {
        Some(x3d_render::traverse::StyleParams::Line(l)) => l.width,
        _ => 1.0,
    }
}

/// View-space point of an object-space vertex and its GL window coordinates in `viewport` (x, y up, depth unused);
/// `None` behind the eye.
fn window(
    model_view: &Matrix4,
    projection: &Matrix4,
    viewport: &[f64; 4],
    v: &[f32],
) -> (Vector3, Option<[f64; 2]>) {
    let mut eye = Vector3::new(v[0] as f64, v[1] as f64, v[2] as f64);
    model_view.mult_vec_matrix3(&mut eye);
    let mut clip = Vector4::new(eye.x, eye.y, eye.z, 1.0);
    projection.mult_vec_matrix4(&mut clip);

    if clip.w <= 0.0 {
        return (eye, None);
    }

    (
        eye,
        Some([
            viewport[0] + (clip.x / clip.w + 1.0) / 2.0 * viewport[2],
            viewport[1] + (clip.y / clip.w + 1.0) / 2.0 * viewport[3],
        ]),
    )
}

/// Pick: the nearest shape under the pointer (X3DPointingDeviceSensorContext.touch, X3DRenderObject.pointing).
/// A shape inside Viewport nodes is drawn into the pointing buffer with its own viewport: it is
/// skipped when the pointer is outside that rectangle (Viewport.traverse, unless `pointing`, a
/// layer holds the pointer), and its ray goes through that viewport.
///
/// With a GPU, the primitives of every shape that can cover the pointer's pixel go through the pointing pass in drawing
/// order (depth test LEQUAL, so the nearest fragment and, on a depth tie, the last primitive drawn wins), and the buffer decides
/// the shape and gives the point, normal and texCoord. Without one, the CPU ray cast picks the nearest triangle, point
/// (inside its gl_PointSize square) or line (within half a pixel) and its double values are kept.
pub fn pick(
    shapes: PointingShapes,
    geometry: &mut dyn FnMut(&ShapeInst) -> Option<Rc<Geometry>>,
    cam: &PickCamera,
    x: f64,
    y: f64,
    pointing: bool,
    mut gpu: Option<&mut PointingGpu>,
) -> Hit {
    let ray = hit_ray(cam, x, y);
    // The hit point, normal and texCoord come from the pointing buffer: a 1x1 float buffer drawn
    // with the viewport shifted so that it covers the pixel under the pointer
    // (X3DRenderObject.pointing), i.e. they are sampled at that pixel's center. The hit ray
    // itself (setHitRay) uses the exact pointer.
    let (px, py) = (x.floor() + 0.5, y.floor() + 0.5);
    let mut hit = Hit {
        pointer: Vector2::new(x, y),
        ray: ray.copy(),
        view_matrix: cam.view,
        ..Default::default()
    };

    // The pointing buffer decides the pick when there is one. A ParticleSystem goes through it as a single instanced
    // draw, so no particle gets a shape record or a ray cast, and no CPU hit is computed at all: the buffer's own
    // float32 values are the hit.
    if let Some(gpu) = gpu.as_deref_mut() {
        match gpu_pick(gpu, shapes, geometry, cam, x, y, px, py, pointing) {
            // A miss: the pass wrote no fragment, so nothing is under the pointer.
            Ok(None) => return hit,
            Ok(Some((shape, g, sample))) => {
                let (p, normal, tc) = gpu_hit(&shape, &g, &sample);

                fill_hit(&mut hit, &shape, &g, p, normal, tc);
                return hit;
            }
            // No answer came back (the device is lost, or that one readback failed). Not a miss: the ray cast below
            // decides this pick.
            Err(()) => {}
        }
    }

    // The CPU ray cast: it needs a shape record per particle, so the runs are expanded here and only here.
    let shapes = shapes.expanded();
    let candidates = cpu_candidates(&shapes, geometry, cam, x, y, px, py, pointing);
    let Some((c, h)) = cpu_winner(&shapes, &candidates) else {
        return hit;
    };
    fill_hit(
        &mut hit,
        &shapes[c.shape],
        &c.geometry,
        h.point,
        h.normal,
        h.tex_coord,
    );
    hit
}

/// The candidates of every shape, with the CPU ray cast's own hit for each: the fallback path, where a
/// ParticleSystem's particles are shapes of their own.
fn cpu_candidates(
    shapes: &[ShapeInst],
    geometry: &mut dyn FnMut(&ShapeInst) -> Option<Rc<Geometry>>,
    cam: &PickCamera,
    x: f64,
    y: f64,
    px: f64,
    py: f64,
    pointing: bool,
) -> Vec<Candidate> {
    let sray = hit_ray(cam, px, py);
    let mut candidates: Vec<Candidate> = vec![];

    for (i, s) in shapes.iter().enumerate() {
        let Some(g) = pointing_geometry(s, geometry, cam, x, y, pointing) else {
            continue;
        };
        let Some((_, cpu)) = shape_candidate(s, &g, cam, px, py, &sray, true) else {
            continue;
        };

        candidates.push(Candidate {
            shape: i,
            geometry: g,
            cpu,
        });
    }

    candidates
}

/// The geometry the pointing pass draws for `s`, skinned like the pointing shader does, or `None` when the shape is
/// not drawn into the buffer at all.
fn pointing_geometry(
    s: &ShapeInst,
    geometry: &mut dyn FnMut(&ShapeInst) -> Option<Rc<Geometry>>,
    cam: &PickCamera,
    x: f64,
    y: f64,
    pointing: bool,
) -> Option<Rc<Geometry>> {
    if !pointing && !x3d_render::layer::pointer_in_rectangle(&s.viewport, x, y) {
        return None;
    }

    // addPointingShape: a ParticleSystem whose bbox is outside the view volume is not drawn into the pointing buffer
    // (a Shape's bbox holds its geometry, so the ray tests below decide the same for it).
    if s.particle
        && !s.explicit_bbox.is_some_and(|b| {
            x3d_render::traverse::bbox_in_view_volume(&cam.projection, &s.shape_model_view, &b)
        })
    {
        return None;
    }

    let g = geometry(s)?;

    // The pointing shader skins the shapes of an HAnimHumanoid or HAnimSegment.
    Some(match &s.skin {
        Some(skin) => Rc::new(x3d_render::hanim::skin(&g, &skin.0, &skin.1)),
        None => g,
    })
}

/// The primitives of `s` that can cover the pointer's pixel, and with `want_cpu` the CPU ray cast's own hits among
/// them in primitive order (view-space distance, point, normal, texCoord, depth). `None` when none of them can. `sray`
/// is the ray through the pixel's center in the layer's own viewport.
fn shape_candidate(
    s: &ShapeInst,
    g: &Geometry,
    cam: &PickCamera,
    px: f64,
    py: f64,
    sray: &Line3,
    want_cpu: bool,
) -> Option<(Prims, Vec<CpuHit>)> {
    let shape_ray = |s: &ShapeInst| {
        if s.viewport
            == [
                cam.viewport.x,
                cam.viewport.y,
                cam.viewport.z,
                cam.viewport.w,
            ]
        {
            sray.copy()
        } else {
            let v = s.viewport;
            hit_ray(
                &PickCamera {
                    viewport: Vector4::new(v[0], v[1], v[2], v[3]),
                    ..cam.clone()
                },
                px,
                py,
            )
        }
    };

    {
        // ClipPlanes2.glsl clip () in the pointing shader: view-space points.
        let clipped = |p: &Vector3| x3d_render::traverse::is_clipped(&s.clip_planes, p);
        let normal_at = |j: usize| -> Vector3 {
            if g.normals.len() < (j + 1) * 3 {
                return Vector3::Z_AXIS;
            }

            // normalMatrix * normal, then touch (): hit .modelViewMatrix .submatrix .inverse () .multMatrixVec (hit .normal)
            let mut n = Vector3::new(
                g.normals[j * 3] as f64,
                g.normals[j * 3 + 1] as f64,
                g.normals[j * 3 + 2] as f64,
            );
            // A particle's instance matrix does not turn normals (no X3D_INSTANCE_NORMAL).
            let normal_mv = if s.particle {
                s.shape_model_view
            } else {
                s.model_view
            };
            let mut nm = normal_mv.submatrix();
            nm.inverse();
            nm.transpose();
            nm.mult_matrix_vec3(&mut n);
            n.normalize();
            let mut inv3 = normal_mv.submatrix();
            inv3.inverse();
            inv3.mult_matrix_vec3(&mut n);
            n
        };

        if g.geometry_type == 0 {
            let size_of = |eye: &Vector3| point_size(s.style.as_ref(), s.textures, eye);
            let mut points = vec![];
            let mut cpu: Vec<CpuHit> = vec![];

            for j in 0..g.vertex_count() {
                let (eye, win) = window(
                    &s.model_view,
                    &cam.projection,
                    &s.viewport,
                    &g.vertices[j * 4..j * 4 + 3],
                );
                let Some([wx, wy]) = win else { continue };
                let half = size_of(&eye) / 2.0;

                // The square of gl_PointSize pixels around the point (one more pixel for the rasterizer's snapping).
                if (wx - px).abs() > half + 1.0
                    || (wy - py).abs() > half + 1.0
                    || -eye.z < cam.near
                    || -eye.z > cam.far
                {
                    continue;
                }

                points.push(j);

                if want_cpu && (wx - px).abs() < half && (wy - py).abs() < half && !clipped(&eye) {
                    cpu.push(CpuHit {
                        dist: -eye.z,
                        point: eye,
                        normal: normal_at(j),
                        tex_coord: Vector4::new(0.0, 0.0, 0.0, 1.0),
                        ndc_z: ndc_z(&cam.projection, &eye),
                        slope: None,
                    });
                }
            }

            return (!points.is_empty()).then_some((Prims::Points(points), cpu));
        }

        if g.geometry_type == 1 {
            let width = line_width(s.style.as_ref());
            let mut segments = vec![];
            let mut cpu: Vec<CpuHit> = vec![];

            for k in 0..g.vertex_count() / 2 {
                let (e0, w0) = window(
                    &s.model_view,
                    &cam.projection,
                    &s.viewport,
                    &g.vertices[k * 8..k * 8 + 3],
                );
                let (e1, w1) = window(
                    &s.model_view,
                    &cam.projection,
                    &s.viewport,
                    &g.vertices[k * 8 + 4..k * 8 + 7],
                );

                let (Some(a), Some(b)) = (w0, w1) else {
                    // An end behind the eye: the GPU clips it.
                    segments.push(k);
                    continue;
                };

                let (dx, dy) = (b[0] - a[0], b[1] - a[1]);
                let len2 = dx * dx + dy * dy;
                let t = if len2 > 0.0 {
                    (((px - a[0]) * dx + (py - a[1]) * dy) / len2).clamp(0.0, 1.0)
                } else {
                    0.0
                };
                let (cx, cy) = (a[0] + dx * t, a[1] + dy * t);
                let distance = ((px - cx).powi(2) + (py - cy).powi(2)).sqrt();

                if distance > width / 2.0 + 1.0 {
                    continue;
                }

                segments.push(k);

                let eye = Vector3::new(
                    e0.x + (e1.x - e0.x) * t,
                    e0.y + (e1.y - e0.y) * t,
                    e0.z + (e1.z - e0.z) * t,
                );

                if want_cpu
                    && distance <= width.max(1.0) / 2.0
                    && !clipped(&eye)
                    && -eye.z >= cam.near
                    && -eye.z <= cam.far
                {
                    cpu.push(CpuHit {
                        dist: -eye.z,
                        point: eye,
                        normal: normal_at(2 * k),
                        tex_coord: Vector4::new(0.0, 0.0, 0.0, 1.0),
                        ndc_z: ndc_z(&cam.projection, &eye),
                        slope: None,
                    });
                }
            }

            return (!segments.is_empty()).then_some((Prims::Lines(segments), cpu));
        }

        let sray = shape_ray(s);
        let mut inv = s.model_view;
        inv.inverse();
        let mut o = sray.point;
        inv.mult_vec_matrix3(&mut o);
        let mut d = sray.direction;
        inv.mult_dir_matrix(&mut d);

        // Quick reject with the object's bounding box, widened by the candidates' slack (NEAR of its size): a ray that
        // just misses the box in double can still be inside a triangle the GPU rasterizes in float32, and the
        // pointing pass, not this test, decides.
        let mut margin = g.max;
        margin.subtract(&g.min);
        margin.multiply(NEAR);
        let (mut lo, mut hi) = (g.min, g.max);
        lo.subtract(&margin);
        hi.add(&margin);

        if !ray_box(&lo, &hi, &o, &d) {
            return None;
        }

        let at = |t: f64| {
            let mut p = d;
            p.multiply(t);
            p.add(&o);
            s.model_view.mult_vec_matrix3(&mut p);
            p
        };

        let hits = ray_triangles(&g, &o, &d, &|t| !clipped(&at(t)));

        if hits.is_empty() {
            return None;
        }

        if !want_cpu {
            return Some((Prims::Triangles(hits), vec![]));
        }

        // Every strict hit between the near and the far plane, in triangle order (the order the pointing pass draws).
        let mut order: Vec<&TriHit> = hits.iter().filter(|h| h.strict).collect();
        order.sort_by_key(|h| h.tri);

        let cpu = order
            .into_iter()
            .filter_map(|th| {
                let p = at(th.t);

                if -p.z < cam.near || -p.z > cam.far {
                    return None;
                }

                let w0 = 1.0 - th.u - th.v;
                let k = th.tri * 3;
                let mut normal = Vector3::Z_AXIS;

                if !g.normals.is_empty() {
                    let (n0, n1, n2) = (normal_at(k), normal_at(k + 1), normal_at(k + 2));
                    normal = Vector3::new(
                        n0.x * w0 + n1.x * th.u + n2.x * th.v,
                        n0.y * w0 + n1.y * th.u + n2.y * th.v,
                        n0.z * w0 + n1.z * th.u + n2.z * th.v,
                    );
                }

                let mut tc = Vector4::new(0.0, 0.0, 0.0, 1.0);

                if g.tex_coords.len() >= (k + 3) * 4 {
                    let tv = |j: usize| {
                        [
                            g.tex_coords[j * 4] as f64,
                            g.tex_coords[j * 4 + 1] as f64,
                            g.tex_coords[j * 4 + 2] as f64,
                            g.tex_coords[j * 4 + 3] as f64,
                        ]
                    };
                    let (t0, t1, t2) = (tv(k), tv(k + 1), tv(k + 2));
                    let lerp = |c: usize| t0[c] * w0 + t1[c] * th.u + t2[c] * th.v;
                    tc = Vector4::new(lerp(0), lerp(1), lerp(2), lerp(3));
                }

                Some(CpuHit {
                    dist: -p.z,
                    point: p,
                    normal,
                    tex_coord: tc,
                    ndc_z: ndc_z(&cam.projection, &p),
                    slope: triangle_slope(s, g, cam, th.tri),
                })
            })
            .collect();

        Some((Prims::Triangles(hits), cpu))
    }
}

/// The largest change of ndc z per pixel, along x or y, over triangle `tri` of `g` (the slope that gl.polygonOffset's
/// factor scales). `None` when a vertex is behind the eye or the triangle is edge-on.
fn triangle_slope(s: &ShapeInst, g: &Geometry, cam: &PickCamera, tri: usize) -> Option<f64> {
    let mut w = [[0.0f64; 3]; 3];

    for (i, corner) in w.iter_mut().enumerate() {
        let v = &g.vertices[(tri * 3 + i) * 4..(tri * 3 + i) * 4 + 3];
        let mut eye = Vector3::new(v[0] as f64, v[1] as f64, v[2] as f64);
        s.model_view.mult_vec_matrix3(&mut eye);
        let mut clip = Vector4::new(eye.x, eye.y, eye.z, 1.0);
        cam.projection.mult_vec_matrix4(&mut clip);

        if clip.w <= 0.0 {
            return None;
        }

        *corner = [
            (clip.x / clip.w + 1.0) / 2.0 * s.viewport[2],
            (clip.y / clip.w + 1.0) / 2.0 * s.viewport[3],
            clip.z / clip.w,
        ];
    }

    // z = a x + b y + c over the three corners.
    let (e1, e2) = (
        [w[1][0] - w[0][0], w[1][1] - w[0][1], w[1][2] - w[0][2]],
        [w[2][0] - w[0][0], w[2][1] - w[0][1], w[2][2] - w[0][2]],
    );
    let det = e1[0] * e2[1] - e1[1] * e2[0];

    if det.abs() < 1e-12 {
        return None;
    }

    let a = (e1[2] * e2[1] - e2[2] * e1[1]) / det;
    let b = (e2[2] * e1[0] - e1[2] * e2[0]) / det;
    Some(a.abs().max(b.abs()))
}

/// The depth buffer of the pointing pass is DEPTH_COMPONENT24: depths are compared as 24-bit integers.
const D24: f64 = 16_777_215.0;

/// The 24-bit depth the pointing pass stores for `hit` under `depth` (X3DRenderObject.pointing enables the appearance's
/// DepthMode around the draw): ndc z through gl.depthRange, then the polygon offset of a triangle (factor times the
/// slope, plus units of the smallest depth step), clamped to the buffer. A reversed range keeps the default (0, 1), as
/// WebGL rejects it (see `PointingGpu`).
fn window_depth(depth: Option<&x3d_render::traverse::DepthParams>, hit: &CpuHit) -> i64 {
    let (range, offset) = match depth {
        Some(d) => (
            if d.range[0] > d.range[1] {
                [0.0, 1.0]
            } else {
                d.range
            },
            d.polygon_offset,
        ),
        None => ([0.0, 1.0], [0.0, 0.0]),
    };
    let z = range[0] + (range[1] - range[0]) * (hit.ndc_z + 1.0) / 2.0;
    let mut q = (z.clamp(0.0, 1.0) * D24).round() as i64;

    if let Some(slope) = hit.slope {
        let slope = slope * (range[1] - range[0]) / 2.0;
        q += (offset[0] * slope * D24).round() as i64 + offset[1].round() as i64;
    }

    q.clamp(0, D24 as i64)
}

/// Whether a fragment of depth `q` passes the depth test against the stored depth `stored`.
fn depth_passes(func: x3d_render::traverse::DepthFunc, q: i64, stored: i64) -> bool {
    use x3d_render::traverse::DepthFunc;

    match func {
        DepthFunc::Never => false,
        DepthFunc::Less => q < stored,
        DepthFunc::Equal => q == stored,
        DepthFunc::LessEqual => q <= stored,
        DepthFunc::Greater => q > stored,
        DepthFunc::NotEqual => q != stored,
        DepthFunc::GreaterEqual => q >= stored,
        DepthFunc::Always => true,
    }
}

/// The hit the pointing pass would leave: the candidates' hits through the depth buffer in drawing order. A fragment
/// that passes the depth test writes its id (the last one stays), and the depth only when the mask is on and the test is
/// enabled (gl.disable (DEPTH_TEST) also stops depth writes). The buffer is cleared to the far depth.
///
/// Without any DepthMode in the candidates the depth test is the default LEQUAL on every fragment, which is the nearest
/// hit (the last on a tie): that is decided in the ray cast's own double values, as before, not in 24 bits.
fn cpu_winner<'a>(
    shapes: &[ShapeInst],
    candidates: &'a [Candidate],
) -> Option<(&'a Candidate, &'a CpuHit)> {
    if candidates.iter().all(|c| shapes[c.shape].depth.is_none()) {
        return candidates
            .iter()
            .filter_map(|c| {
                c.cpu
                    .iter()
                    .min_by(|a, b| a.dist.total_cmp(&b.dist))
                    .map(|h| (c, h))
            })
            .fold(None, |best, h| match best {
                Some(b) if h.1.dist > b.1.dist => Some(b),
                _ => Some(h),
            });
    }

    let mut stored = D24 as i64;
    let mut winner = None;

    for c in candidates {
        let depth = shapes[c.shape].depth.as_ref();

        for h in &c.cpu {
            let q = window_depth(depth, h);
            let (test, func, mask) = match depth {
                Some(d) => (d.test, d.func, d.mask),
                None => (true, x3d_render::traverse::DepthFunc::LessEqual, true),
            };

            if test && !depth_passes(func, q, stored) {
                continue;
            }

            winner = Some((c, h));

            if test && mask {
                stored = q;
            }
        }
    }

    winner
}

/// The hit values of a pointing-buffer sample: the point, the normal turned back into the shape's frame (touch ():
/// `hit .modelViewMatrix .submatrix .inverse () .multMatrixVec (hit .normal)` when the geometry has normals, else the
/// Z axis) and the texCoord.
fn gpu_hit(
    s: &ShapeInst,
    g: &Geometry,
    sample: &x3d_render::pointing_gpu::PointingSample,
) -> (Vector3, Vector3, Vector4) {
    let p = Vector3::new(
        sample.point[0] as f64,
        sample.point[1] as f64,
        sample.point[2] as f64,
    );
    let tc = Vector4::new(
        sample.tex_coord[0] as f64,
        sample.tex_coord[1] as f64,
        sample.tex_coord[2] as f64,
        sample.tex_coord[3] as f64,
    );
    let mut normal = Vector3::Z_AXIS;

    if !g.normals.is_empty() {
        let mut n = Vector3::new(
            sample.normal[0] as f64,
            sample.normal[1] as f64,
            sample.normal[2] as f64,
        );
        let mut mv = s.shape_model_view;
        mv.0 = mv.0.map(|v| v as f32 as f64);
        let mut inv3 = mv.submatrix();
        inv3.inverse();
        inv3.mult_matrix_vec3(&mut n);
        normal = n;
    }

    (p, normal, tc)
}

/// The hit of the shape the pick found, with its sensors, Anchors and bounding box.
fn fill_hit(hit: &mut Hit, s: &ShapeInst, g: &Geometry, p: Vector3, normal: Vector3, tc: Vector4) {
    hit.id = true;
    hit.point = p;
    hit.normal = normal;
    hit.tex_coord = tc;
    // hit .modelViewMatrix is the shape's own (an InstancedShape's instance matrices are in its
    // vertex shader) and hit .shapeNode .getBBox () the union of the instances' boxes
    // (InstancedShape.set_bbox__: `bbox .add (subBBox .assign (geometryBBox) .multRight (matrix))`).
    hit.model_view = s.shape_model_view;
    hit.sensors = s.sensors.clone();
    hit.shape = Some(s.shape.clone());
    hit.bbox = Some(match (&s.explicit_bbox, &s.all_instances) {
        (Some(explicit), _) => *explicit,
        (None, Some(all)) => {
            let mut union: Option<(Vector3, Vector3)> = None;

            for m in all.iter() {
                let mut b = crate::lookat::Box3::from_extents(&g.min, &g.max);
                b.mult_right(m);
                let (lo, hi) = b.extents();

                union = Some(match union {
                    None => (lo, hi),
                    Some((mut a, mut c)) => (*a.min(&lo), *c.max(&hi)),
                });
            }

            union.unwrap_or((g.min, g.max))
        }
        (None, None) => (g.min, g.max),
    });
    hit.anchors = s.anchors.clone();
}

/// A ParticleSystem's draw in the pointing pass: every drawn particle at once under its own x3d_InstanceMatrix
/// (ParticleSystem.displaySimple, an instanced draw), particle k with x3d_Id `id + k`. The whole geometry goes
/// through — the rasterizer and the depth test decide which particle covers the pixel, so no particle is tested on
/// the CPU. `None` when the system draws nothing.
///
/// POINT, QUAD, SPRITE and GEOMETRY particles share the one vertex range. LINE particles do not: a line is widened
/// along its *projected* minor axis, which the instance matrix turns, so each particle brings its own six vertices
/// ([`PointingTriangle::per_instance_vertices`]) — still one draw, but the vertex data grows with the particle count.
fn particle_draw(
    s: &ShapeInst,
    g: &Geometry,
    run: &ParticleRun,
    cam: &PickCamera,
    x: f64,
    y: f64,
    id: f32,
) -> Option<PointingTriangle> {
    if run.len() == 0 || g.vertex_count() == 0 {
        return None;
    }

    let base = PointingTriangle {
        instances: Some(run.matrices.clone()),
        ..pointing_draw_base(s, cam, x, y, id)
    };

    // The lines of every particle in turn: their offsets (and, above MAX_LINE_WIDTH, their vertices) are the
    // particle's own.
    if g.geometry_type == 1 {
        let segments = g.vertex_count() / 2;
        let per = 6 * segments;
        let mut draw = PointingTriangle {
            vertices: Vec::with_capacity(per * run.len()),
            normals: Vec::with_capacity(per * run.len()),
            tex_coords: vec![[0.0, 0.0, 0.0, 1.0]; per * run.len()],
            offsets: Vec::with_capacity(per * run.len()),
            constant_tex_coord: true,
            per_instance_vertices: per as u32,
            ..base
        };
        let prims = Prims::Lines((0..segments).collect());
        // One record, moved from particle to particle: the draw of a line reads only the two matrices, and a
        // ShapeInst holds Vecs of sensors, clip planes and Anchors that there is no reason to clone per particle.
        let mut one = run.entry(0);

        for k in 0..run.len() {
            let m = run.matrices[k];
            let mut model_view = m;

            model_view.mult_right(&run.template.shape_model_view);
            one.instance = Some(m);
            one.model_view = model_view;

            let d = pointing_draw(&one, g, &prims, cam, x, y, id);

            draw.primitive = d.primitive;
            draw.vertices.extend(d.vertices);
            draw.normals.extend(d.normals);
            draw.offsets.extend(d.offsets);
        }

        // Above MAX_LINE_WIDTH the vertices are the transformed triangles of displayInstancedThick, which the
        // instance matrix still turns, exactly as it does for a particle drawn on its own.
        return Some(draw);
    }

    let prims = match g.geometry_type {
        0 => Prims::Points((0..g.vertex_count()).collect()),
        _ => Prims::Triangles(
            (0..g.vertex_count() / 3)
                .map(|tri| TriHit {
                    strict: false,
                    t: 0.0,
                    u: 0.0,
                    v: 0.0,
                    tri,
                })
                .collect(),
        ),
    };

    Some(PointingTriangle {
        instances: base.instances.clone(),
        per_instance_vertices: 0,
        ..pointing_draw(s, g, &prims, cam, x, y, id)
    })
}

/// One shape's draw in the pointing pass: its candidate primitives as the pointing shader receives them.
fn pointing_draw(
    s: &ShapeInst,
    g: &Geometry,
    prims: &Prims,
    cam: &PickCamera,
    x: f64,
    y: f64,
    id: f32,
) -> PointingTriangle {
    use x3d_render::pointing_gpu::{PointingPointStyle, PointingPrimitive};

    let has_normals = !g.normals.is_empty();
    let vert = |j: usize| {
        [
            g.vertices[j * 4],
            g.vertices[j * 4 + 1],
            g.vertices[j * 4 + 2],
            g.vertices[j * 4 + 3],
        ]
    };
    let norm = |j: usize| {
        if has_normals {
            [g.normals[j * 3], g.normals[j * 3 + 1], g.normals[j * 3 + 2]]
        } else {
            [0.0, 0.0, 1.0]
        }
    };
    let has_tex = g.geometry_type >= 2 && g.tex_coords.len() >= g.vertex_count() * 4;
    let tex = |j: usize| {
        if has_tex {
            [
                g.tex_coords[j * 4],
                g.tex_coords[j * 4 + 1],
                g.tex_coords[j * 4 + 2],
                g.tex_coords[j * 4 + 3],
            ]
        } else {
            [0.0, 0.0, 0.0, 1.0]
        }
    };
    let v = s.viewport;
    let mut point_style = None;
    let mut offsets = vec![];

    let (primitive, corners): (PointingPrimitive, Vec<usize>) = match prims {
        Prims::Triangles(hits) => {
            let mut tris: Vec<usize> = hits.iter().map(|h| h.tri).collect();
            tris.sort_unstable();
            (
                PointingPrimitive::Triangles,
                tris.iter()
                    .flat_map(|t| [t * 3, t * 3 + 1, t * 3 + 2])
                    .collect(),
            )
        }
        Prims::Points(points) => {
            if let Some(x3d_render::traverse::StyleParams::Point(p)) = s.style {
                point_style = Some(PointingPointStyle {
                    scale_factor: p.scale_factor as f32,
                    min_value: p.min_value as f32,
                    max_value: p.max_value as f32,
                    attenuation: p.attenuation.map(|a| a as f32),
                    texture: s.textures.0,
                    any_texture: s.textures.0 || s.textures.1,
                });
            }

            const SQUARE: [[f32; 2]; 6] = [
                [-1.0, -1.0],
                [1.0, -1.0],
                [-1.0, 1.0],
                [1.0, -1.0],
                [1.0, 1.0],
                [-1.0, 1.0],
            ];
            offsets = points.iter().flat_map(|_| SQUARE).collect();
            (
                PointingPrimitive::Points,
                points.iter().flat_map(|&j| [j; 6]).collect(),
            )
        }
        Prims::Lines(segments) => {
            let width = line_width(s.style.as_ref());

            if width > x3d_render::gpu::MAX_LINE_WIDTH {
                // displaySimpleThick: the transformed triangles of the DISPLAY pass, six vertices per segment.
                let Some(x3d_render::traverse::StyleParams::Line(l)) = s.style else {
                    unreachable!()
                };
                // A particle's are the float32 triangles displayInstancedThick made with this view after the frame's
                // DISPLAY pass (x3d_runtime::particles::Sim::thick_lines), under its instance matrix.
                let positions = match (s.particle, &s.instance) {
                    (true, Some(m)) => x3d_render::gpu::instanced_thick_line_positions(
                        g,
                        &s.shape_model_view,
                        &cam.projection,
                        s.viewport,
                        l.width,
                        &m.0.map(|v| v as f32),
                    ),
                    _ => x3d_render::gpu::transformed_line_positions(
                        g,
                        &s.model_view,
                        &cam.projection,
                        s.viewport,
                        &l,
                    ),
                };
                let ends = [0, 0, 1, 0, 1, 1];
                let corners: Vec<usize> = segments
                    .iter()
                    .flat_map(|&k| ends.map(|e| 2 * k + e))
                    .collect();
                let vertices: Vec<[f32; 4]> = segments
                    .iter()
                    .flat_map(|&k| (0..6).map(move |c| k * 6 + c))
                    .map(|i| positions[i])
                    .collect();

                return PointingTriangle {
                    primitive: PointingPrimitive::Triangles,
                    vertices,
                    normals: corners.iter().map(|&j| norm(j)).collect(),
                    tex_coords: corners.iter().map(|_| [0.0, 0.0, 0.0, 1.0]).collect(),
                    offsets: vec![],
                    constant_tex_coord: true,
                    ..pointing_draw_base(s, cam, x, y, id)
                };
            }

            // Aliased lines (1 px included): the minor axis column of the rounded width.
            let w = width.round().max(1.0) as f32 / 2.0;

            for &k in segments {
                let (_, a) = window(
                    &s.model_view,
                    &cam.projection,
                    &v,
                    &g.vertices[k * 8..k * 8 + 3],
                );
                let (_, b) = window(
                    &s.model_view,
                    &cam.projection,
                    &v,
                    &g.vertices[k * 8 + 4..k * 8 + 7],
                );
                let x_major = match (a, b) {
                    (Some(a), Some(b)) => (b[0] - a[0]).abs() >= (b[1] - a[1]).abs(),
                    _ => true,
                };
                let o = if x_major { [0.0, w] } else { [w, 0.0] };
                let n = [-o[0], -o[1]];
                offsets.extend_from_slice(&[o, n, n, o, n, o]);
            }

            (
                PointingPrimitive::WideLines,
                segments
                    .iter()
                    .flat_map(|&k| [2 * k, 2 * k, 2 * k + 1, 2 * k, 2 * k + 1, 2 * k + 1])
                    .collect(),
            )
        }
    };

    PointingTriangle {
        primitive,
        vertices: corners.iter().map(|&j| vert(j)).collect(),
        normals: corners.iter().map(|&j| norm(j)).collect(),
        tex_coords: corners.iter().map(|&j| tex(j)).collect(),
        offsets,
        point_style,
        constant_tex_coord: g.geometry_type < 2,
        ..pointing_draw_base(s, cam, x, y, id)
    }
}

/// The uniforms of a shape's pointing draw: matrices, the shifted viewport, clip planes and x3d_Id.
fn pointing_draw_base(
    s: &ShapeInst,
    cam: &PickCamera,
    x: f64,
    y: f64,
    id: f32,
) -> PointingTriangle {
    let f32x16 = |m: &Matrix4| m.0.map(|v| v as f32);
    let v = s.viewport;

    PointingTriangle {
        primitive: x3d_render::pointing_gpu::PointingPrimitive::Triangles,
        vertices: vec![],
        normals: vec![],
        tex_coords: vec![],
        offsets: vec![],
        projection: f32x16(&cam.projection),
        model_view: f32x16(&s.shape_model_view),
        instance: s
            .instance
            .as_ref()
            .map_or(x3d_render::pointing_gpu::IDENTITY, f32x16),
        // Particles: X3D_INSTANCING without X3D_INSTANCE_NORMAL (ParticleSystem.getShapeKey 1 or 2).
        instance_normal: s.instance.as_ref().filter(|_| !s.particle).map_or(
            x3d_render::pointing_gpu::IDENTITY3,
            |m| {
                let mut n = m.submatrix();
                n.transpose();
                n.inverse();
                n.0.map(|v| v as f32)
            },
        ),
        // gl .viewport (viewport .x - x, viewport .y - y, viewport .z, viewport .w): GLint arguments.
        viewport: [
            (v[0] - x) as i32,
            (v[1] - y) as i32,
            v[2] as i32,
            v[3] as i32,
        ],
        clip_planes: s.clip_planes.iter().map(|p| p.map(|c| c as f32)).collect(),
        particle: s.particle,
        point_style: None,
        constant_tex_coord: false,
        id,
        // A ParticleSystem's draw puts its particles' matrices here ([`particle_draw`]).
        instances: None,
        per_instance_vertices: 0,
        depth: s.depth,
    }
}

fn ray_box(min: &Vector3, max: &Vector3, o: &Vector3, d: &Vector3) -> bool {
    let mut t0 = f64::NEG_INFINITY;
    let mut t1 = f64::INFINITY;

    for (mn, mx, oo, dd) in [
        (min.x, max.x, o.x, d.x),
        (min.y, max.y, o.y, d.y),
        (min.z, max.z, o.z, d.z),
    ] {
        if dd.abs() < 1e-300 {
            if oo < mn || oo > mx {
                return false;
            }
        } else {
            let (a, b) = ((mn - oo) / dd, (mx - oo) / dd);
            t0 = t0.max(a.min(b));
            t1 = t1.min(a.max(b));
        }
    }

    t1 >= t0.max(f64::NEG_INFINITY) && t1 >= -1e9
}

// --- sensor behavior -------------------------------------------------------------------------

#[derive(Clone)]
struct SensorState {
    mv: Matrix4,
    inv_mv: Matrix4,
    projection: Matrix4,
    viewport: Vector4,
    activate_hit: Option<Hit>,
    motion_hit: Option<Hit>,
    // PlaneSensor
    plane_sensor: bool,
    plane: Plane3,
    line: Line3,
    start_offset_v: Vector3,
    start_point: Vector3,
    // CylinderSensor
    cylinder: Cylinder3,
    disk: bool,
    y_plane: Plane3,
    z_plane: Plane3,
    sx_plane: Plane3,
    sz_normal: Vector3,
    behind: bool,
    from_vector: Vector3,
    start_offset_r: Rotation4,
    angle: f64,
    start_vector: Vector3,
    // SphereSensor
    sphere: Sphere3,
}

impl SensorState {
    fn new() -> SensorState {
        SensorState {
            mv: Matrix4::IDENTITY,
            inv_mv: Matrix4::IDENTITY,
            projection: Matrix4::IDENTITY,
            viewport: Vector4::ZERO,
            activate_hit: None,
            motion_hit: None,
            plane_sensor: true,
            plane: Plane3::new(Vector3::ZERO, Vector3::Z_AXIS),
            line: Line3::new(Vector3::ZERO, Vector3::Y_AXIS),
            start_offset_v: Vector3::ZERO,
            start_point: Vector3::ZERO,
            cylinder: Cylinder3::new(Line3::new(Vector3::ZERO, Vector3::Y_AXIS), 0.0),
            disk: false,
            y_plane: Plane3::new(Vector3::ZERO, Vector3::Y_AXIS),
            z_plane: Plane3::new(Vector3::ZERO, Vector3::Z_AXIS),
            sx_plane: Plane3::new(Vector3::ZERO, Vector3::X_AXIS),
            sz_normal: Vector3::Z_AXIS,
            behind: false,
            from_vector: Vector3::ZERO,
            start_offset_r: Rotation4::identity(),
            angle: 0.0,
            start_vector: Vector3::ZERO,
            sphere: Sphere3::new(0.0, Vector3::ZERO),
        }
    }
}

/// Output of the browser for the UI.
#[derive(Default)]
pub struct PointerOutput {
    pub anchors: Vec<AnchorActivation>,
    pub description: Option<String>,
}

pub struct Pointer {
    over: Vec<(NodeRef, SensorRef)>,
    active: Vec<(NodeRef, SensorRef)>,
    states: Vec<(NodeRef, SensorState)>,
    anchor_over: std::collections::HashMap<(Vec<NodeId>, NodeId), bool>,
    anchor_active: std::collections::HashMap<(Vec<NodeId>, NodeId), bool>,
    pub hit: Hit,
    pub out: PointerOutput,
}

type FKey = (NodeId, x3d_scene::FieldLoc);

fn field(rt: &Runtime, n: NodeId, name: &str) -> Option<FKey> {
    rt.pre(n, name)
}

fn set(rt: &mut Runtime, n: NodeId, name: &str, v: FieldValue) {
    if let Some(k) = field(rt, n, name) {
        rt.set_value(k, v);
    }
}

fn get_v3(rt: &Runtime, n: NodeId, name: &str) -> Vector3 {
    match rt.pre_value(n, name) {
        Some(FieldValue::SFVec3f(v)) => *v,
        _ => Vector3::ZERO,
    }
}

fn get_v2(rt: &Runtime, n: NodeId, name: &str) -> Vector2 {
    match rt.pre_value(n, name) {
        Some(FieldValue::SFVec2f(v)) => *v,
        _ => Vector2::ZERO,
    }
}

fn get_rot(rt: &Runtime, n: NodeId, name: &str) -> Rotation4 {
    match rt.pre_value(n, name) {
        Some(FieldValue::SFRotation(v)) => *v,
        _ => Rotation4::identity(),
    }
}

fn get_f(rt: &Runtime, n: NodeId, name: &str) -> f64 {
    rt.get_f64(n, name)
}

fn rot_vec(r: &Rotation4, v: Vector3) -> Vector3 {
    let mut v = v;
    r.mult_vec_rot(&mut v);
    v
}

fn mult_vec_matrix(m: &Matrix4, v: Vector3) -> Vector3 {
    let mut v = v;
    m.mult_vec_matrix3(&mut v);
    v
}

fn mult_dir_matrix(m: &Matrix4, v: Vector3) -> Vector3 {
    let mut v = v;
    m.mult_dir_matrix(&mut v);
    v
}

impl Pointer {
    pub fn new() -> Pointer {
        Pointer {
            over: vec![],
            active: vec![],
            states: vec![],
            anchor_over: Default::default(),
            anchor_active: Default::default(),
            hit: Hit::default(),
            out: PointerOutput::default(),
        }
    }

    fn state(&mut self, n: &NodeRef) -> &mut SensorState {
        if let Some(i) = self.states.iter().position(|(k, _)| k == n) {
            return &mut self.states[i].1;
        }

        self.states.push((n.clone(), SensorState::new()));
        &mut self.states.last_mut().unwrap().1
    }

    pub fn is_active(&self) -> bool {
        !self.active.is_empty()
    }

    /// The runtime released `released` ([`Runtime::take_released`]): their ids may name nodes of
    /// another scene from now on. Every sensor, Anchor and hit this pointer remembers of them is
    /// dropped, without events: a drag or a press on a node that is gone ends with it, and the
    /// node that takes the slot starts inactive and not under the pointer. The other sensors keep
    /// their state.
    pub fn forget(&mut self, released: &[NodeId]) {
        if released.is_empty() {
            return;
        }

        let gone = |path: &[NodeId], n: NodeId| {
            released.contains(&n) || path.iter().any(|p| released.contains(p))
        };
        let dead = |r: &NodeRef| gone(&r.path, r.node);

        self.over.retain(|(n, _)| !dead(n));
        self.active.retain(|(n, _)| !dead(n));
        self.states.retain(|(n, _)| !dead(n));
        self.anchor_over.retain(|(path, n), _| !gone(path, *n));
        self.anchor_active.retain(|(path, n), _| !gone(path, *n));

        // The last pick is reused while navigating: it must not name a released node either.
        if self.hit.shape.as_ref().is_some_and(dead) {
            self.hit = Hit::default();
        } else {
            self.hit.sensors.retain(|s| !dead(&s.node));
            self.hit.anchors.retain(|a| !dead(a));
        }
    }

    /// X3DPointingDeviceSensorContext.buttonPressEvent. `hit` is the fresh pick.
    pub fn button_press(
        &mut self,
        root: &mut Runtime,
        hit: Hit,
        cam: &PickCamera,
        now: f64,
    ) -> bool {
        self.hit = hit;

        if !self.hit.id {
            return false;
        }

        let sensors = self.hit.sensors.clone();

        for s in sensors {
            if self.active.iter().any(|(k, _)| *k == s.node) {
                continue;
            }

            if !self.over.iter().any(|(k, _)| *k == s.node) {
                self.over.push((s.node.clone(), s.clone()));
            }

            self.active.push((s.node.clone(), s.clone()));
            self.set_active(root, &s, true, cam, now);
        }

        root.process_events();
        !self.hit.sensors.is_empty()
    }

    /// PointingDevice.mousedown: `grabbing`, a sensor of the hit is an X3DDragSensorNode.
    pub fn hit_has_drag_sensor(&self, root: &Runtime) -> bool {
        self.hit.sensors.iter().any(|s| {
            matches!(
                Self::ty(root, s),
                Some("PlaneSensor" | "CylinderSensor" | "SphereSensor")
            )
        })
    }

    /// buttonReleaseEvent.
    pub fn button_release(&mut self, root: &mut Runtime, cam: &PickCamera, now: f64) {
        let active = std::mem::take(&mut self.active);

        for (_, s) in active {
            self.set_active(root, &s, false, cam, now);
        }

        root.process_events();
    }

    /// motionNotifyEvent: `hit` is the fresh pick (or, while a button is held, a pick whose ray
    /// still drives the active drag sensors).
    pub fn motion(&mut self, root: &mut Runtime, hit: Hit, cam: &PickCamera, now: f64) -> bool {
        self.hit = hit;
        self.motion_hit(root, cam, now);
        root.process_events();
        !self.hit.sensors.is_empty()
    }

    fn motion_hit(&mut self, root: &mut Runtime, cam: &PickCamera, now: f64) {
        let hit = self.hit.clone();

        // isOver FALSE for sensors no longer under the pointer.
        let over = std::mem::take(&mut self.over);
        let mut keep = vec![];

        for (n, s) in over {
            if hit.id && hit.sensors.iter().any(|h| h.node == n) {
                keep.push((n, s));
                continue;
            }

            self.set_over(root, &s, false, &hit, now);
        }

        self.over = keep;

        // isOver TRUE for the sensors under the pointer: `overSensors .set (node, sensor);
        // sensor .set_over__ (true, hit)` for every one, also those already over (a TouchSensor
        // sends its hit outputs again for the new hit).
        for s in &hit.sensors {
            match self.over.iter_mut().find(|(k, _)| *k == s.node) {
                Some(entry) => entry.1 = s.clone(),
                None => self.over.push((s.node.clone(), s.clone())),
            }

            self.set_over(root, s, true, &hit, now);
        }

        // Motion for the active drag sensors.
        for (_, s) in self.active.clone() {
            self.set_motion(root, &s, &hit, cam);
        }
    }

    fn ty(root: &Runtime, s: &SensorRef) -> Option<&'static str> {
        let rt = crate::walk::rt_at(root, &s.node.path)?;
        rt.builtin_name(s.node.node)
    }

    // X3DPointingDeviceSensorNode.set_over__ and the X3DTouchSensorNode override.
    fn set_over(&mut self, root: &mut Runtime, s: &SensorRef, over: bool, hit: &Hit, _now: f64) {
        let ty = Self::ty(root, s);
        let Some(rt) = rt_at_mut(root, &s.node.path) else {
            return;
        };
        let n = s.node.node;
        let is_anchor = ty == Some("Anchor");

        // Anchor: its hidden TouchSensor has the Anchor's description as description.
        if is_anchor {
            let cur = self
                .anchor_over
                .get(&(s.node.path.to_vec(), n))
                .copied()
                .unwrap_or(false);
            if cur == over {
                return;
            }
            self.anchor_over.insert((s.node.path.to_vec(), n), over);

            if over {
                if let Some(FieldValue::SFString(d)) = rt.pre_value(n, "description") {
                    self.out.description = Some(d.clone());
                }
            }
            return;
        }

        if rt.get_bool(n, "isOver") != over {
            set(rt, n, "isOver", FieldValue::SFBool(over));

            if over {
                if let Some(FieldValue::SFString(d)) = rt.pre_value(n, "description") {
                    self.out.description = Some(d.clone());
                }
            }
        }

        if matches!(ty, Some("TouchSensor" | "GeoTouchSensor")) && over && rt.get_bool(n, "isOver")
        {
            let mut inv = s.model_view;
            inv.inverse();
            let w = hit.tex_coord.w;
            let tc = Vector2::new(hit.tex_coord.x / w, hit.tex_coord.y / w);
            let mut normal = hit.normal;
            s.model_view.mult_matrix_dir(&mut normal);
            normal.normalize();
            let point = mult_vec_matrix(&inv, hit.point);

            set(rt, n, "hitTexCoord_changed", FieldValue::SFVec2f(tc));
            set(rt, n, "hitNormal_changed", FieldValue::SFVec3f(normal));
            set(rt, n, "hitPoint_changed", FieldValue::SFVec3f(point));
        }

        // GeoTouchSensor.set_over__: `if (this ._isOver .getValue ()) hitGeoCoord_changed = getGeoCoord (hitPoint_changed)`.
        if ty == Some("GeoTouchSensor") && rt.get_bool(n, "isOver") {
            let hit_point = match rt.pre_value(n, "hitPoint_changed") {
                Some(FieldValue::SFVec3f(v)) => *v,
                _ => Vector3::ZERO,
            };
            let geo = rt.geo_object(n).get_geo_coord(&hit_point);
            set(rt, n, "hitGeoCoord_changed", FieldValue::SFVec3d(geo));
        }
    }

    // set_active__ for all sensor types.
    fn set_active(
        &mut self,
        root: &mut Runtime,
        s: &SensorRef,
        active: bool,
        cam: &PickCamera,
        now: f64,
    ) {
        let ty = Self::ty(root, s);
        let hit = self.hit.clone();
        let key = (s.node.path.to_vec(), s.node.node);

        if ty == Some("Anchor") {
            let over = self.anchor_over.get(&key).copied().unwrap_or(false);
            let was_active = self.anchor_active.get(&key).copied().unwrap_or(false);

            if was_active == active {
                return;
            }

            self.anchor_active.insert(key, active);

            // The hidden TouchSensor: on release over the anchor, request the load.
            if over && !active {
                if let Some(rt) = crate::walk::rt_at(root, &s.node.path) {
                    let n = s.node.node;
                    let strings = |name: &str| match rt.pre_value(n, name) {
                        Some(FieldValue::MFString(v)) => v.clone(),
                        _ => vec![],
                    };
                    let description = match rt.pre_value(n, "description") {
                        Some(FieldValue::SFString(d)) => d.clone(),
                        _ => String::new(),
                    };
                    self.out.anchors.push(AnchorActivation {
                        node: s.node.clone(),
                        description,
                        urls: strings("url"),
                        parameters: strings("parameter"),
                    });
                }
            }
            return;
        }

        let Some(rt) = rt_at_mut(root, &s.node.path) else {
            return;
        };
        let n = s.node.node;

        if rt.get_bool(n, "isActive") == active {
            return;
        }

        set(rt, n, "isActive", FieldValue::SFBool(active));

        match ty {
            Some("TouchSensor" | "GeoTouchSensor") => {
                if rt.get_bool(n, "enabled") && rt.get_bool(n, "isOver") && !active {
                    set(rt, n, "touchTime", FieldValue::SFTime(now));
                }
            }
            Some("PlaneSensor") | Some("CylinderSensor") | Some("SphereSensor") => {
                if active {
                    let st = self.state(&s.node);
                    st.activate_hit = Some(hit.clone());
                    st.motion_hit = None;
                    st.mv = s.model_view;
                    st.projection = cam.projection;
                    st.viewport = cam.viewport;
                    let mut inv = s.model_view;
                    inv.inverse();
                    st.inv_mv = inv;

                    let mut st = self.state(&s.node).clone();
                    match ty {
                        Some("PlaneSensor") => Self::plane_activate(rt, n, &mut st, &hit),
                        Some("CylinderSensor") => Self::cylinder_activate(rt, n, &mut st, &hit),
                        _ => Self::sphere_activate(rt, n, &mut st, &hit),
                    }
                    *self.state(&s.node) = st;
                } else if rt.get_bool(n, "autoOffset") {
                    match ty {
                        Some("PlaneSensor") => {
                            let v = get_v3(rt, n, "translation_changed");
                            set(rt, n, "offset", FieldValue::SFVec3f(v));
                        }
                        Some("CylinderSensor") => {
                            let mut r = get_rot(rt, n, "rotation_changed");
                            let st = self.state(&s.node).clone();
                            let a = Self::cylinder_angle(&st, &mut r);
                            set(rt, n, "offset", FieldValue::SFFloat(a));
                        }
                        _ => {
                            let r = get_rot(rt, n, "rotation_changed");
                            set(rt, n, "offset", FieldValue::SFRotation(r));
                        }
                    }
                }
            }
            _ => {}
        }
    }

    fn set_motion(&mut self, root: &mut Runtime, s: &SensorRef, hit: &Hit, _cam: &PickCamera) {
        let ty = Self::ty(root, s);
        let Some(rt) = rt_at_mut(root, &s.node.path) else {
            return;
        };
        let n = s.node.node;
        let mut st = self.state(&s.node).clone();

        match ty {
            Some("PlaneSensor") => Self::plane_motion(rt, n, &mut st, hit),
            Some("CylinderSensor") => Self::cylinder_motion(rt, n, &mut st, hit),
            Some("SphereSensor") => Self::sphere_motion(rt, n, &mut st, hit),
            _ => return,
        }

        *self.state(&s.node) = st;
    }

    // --- PlaneSensor ---------------------------------------------------------------------------

    fn line_track_point(
        st: &SensorState,
        pointer: &Vector2,
        line: &Line3,
        track_point: &mut Vector3,
    ) -> bool {
        let mut screen_line = Line2::new(Vector2::ZERO, Vector2::new(1.0, 0.0));
        ViewVolume::project_line(line, &st.mv, &st.projection, &st.viewport, &mut screen_line);
        let tp1 = screen_line.get_closest_point_to_point(pointer);
        let mut track_line = Line3::new(Vector3::ZERO, Vector3::new(0.0, 0.0, -1.0));
        ViewVolume::unproject_ray(
            tp1.x,
            tp1.y,
            &st.mv,
            &st.projection,
            &st.viewport,
            &mut track_line,
        );
        line.get_closest_point_to_line(&track_line, track_point)
    }

    fn plane_activate(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        let mut hit_ray = hit.ray.copy();
        hit_ray.mult_line_matrix(&st.inv_mv);
        let hit_point = mult_vec_matrix(&st.inv_mv, hit.point);
        let axis_rotation = get_rot(rt, n, "axisRotation");
        let (min, max) = (get_v2(rt, n, "minPosition"), get_v2(rt, n, "maxPosition"));

        if min.x == max.x {
            st.plane_sensor = false;
            let direction = rot_vec(
                &axis_rotation,
                Vector3::new(0.0, (max.y - min.y).abs(), 0.0),
            );
            let mut d = direction;
            d.normalize();
            st.line = Line3::new(hit_point, d);
        } else if min.y == max.y {
            st.plane_sensor = false;
            let direction = rot_vec(
                &axis_rotation,
                Vector3::new((max.x - min.x).abs(), 0.0, 0.0),
            );
            let mut d = direction;
            d.normalize();
            st.line = Line3::new(hit_point, d);
        } else {
            st.plane_sensor = true;
            st.plane = Plane3::new(
                hit_point,
                rot_vec(&axis_rotation, Vector3::new(0.0, 0.0, 1.0)),
            );
        }

        if st.plane_sensor {
            let mut sp = Vector3::ZERO;

            if st.plane.intersects_line(&hit_ray, &mut sp) {
                st.start_point = sp;
                Self::plane_track_start(rt, n, st, sp);
            }
        } else {
            let mut sp = Vector3::ZERO;

            if Self::line_track_point(st, &hit.pointer, &st.line.copy(), &mut sp) {
                st.start_point = sp;
                let mut track_point = Vector3::ZERO;
                let l2 = Line3::new(st.line.direction, st.line.direction);

                if !Self::line_track_point(st, &hit.pointer, &l2, &mut track_point) {
                    track_point = sp;
                }

                Self::plane_track_start(rt, n, st, track_point);
            }
        }
    }

    fn plane_track_start(rt: &mut Runtime, n: NodeId, st: &mut SensorState, track_point: Vector3) {
        st.start_offset_v = get_v3(rt, n, "offset");
        set(
            rt,
            n,
            "trackPoint_changed",
            FieldValue::SFVec3f(track_point),
        );
        let off = get_v3(rt, n, "offset");
        set(rt, n, "translation_changed", FieldValue::SFVec3f(off));
    }

    fn plane_motion(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        st.motion_hit = Some(hit.clone());

        let result: Option<(Vector3, Vector3)> = if st.plane_sensor {
            let mut hit_ray = hit.ray.copy();
            hit_ray.mult_line_matrix(&st.inv_mv);
            let mut end = Vector3::ZERO;

            if st.plane.intersects_line(&hit_ray, &mut end) {
                Some((end, end))
            } else {
                None
            }
        } else {
            let mut end = Vector3::ZERO;

            if Self::line_track_point(st, &hit.pointer, &st.line.copy(), &mut end) {
                let mut tp = Vector3::ZERO;
                let l2 = Line3::new(Vector3::ZERO, st.line.direction);

                if !Self::line_track_point(st, &hit.pointer, &l2, &mut tp) {
                    tp = end;
                }

                Some((end, tp))
            } else {
                None
            }
        };

        match result {
            Some((end, track_point)) => Self::plane_track(rt, n, st, end, track_point),
            None => {
                // this ._trackPoint_changed .addEvent (); this ._translation_changed .addEvent ();
                if let Some(k) = field(rt, n, "trackPoint_changed") {
                    rt.add_event(k);
                }
                if let Some(k) = field(rt, n, "translation_changed") {
                    rt.add_event(k);
                }
            }
        }
    }

    fn plane_track(
        rt: &mut Runtime,
        n: NodeId,
        st: &mut SensorState,
        end_point: Vector3,
        track_point: Vector3,
    ) {
        let axis_rotation = get_rot(rt, n, "axisRotation");
        let mut inv_ar = axis_rotation;
        inv_ar.inverse();
        let mut e = end_point;
        e.add(&st.start_offset_v).subtract(&st.start_point);
        let mut translation = rot_vec(&inv_ar, e);
        let (min, max) = (get_v2(rt, n, "minPosition"), get_v2(rt, n, "maxPosition"));

        if !(min.x > max.x) {
            translation.x = translation.x.max(min.x).min(max.x);
        }

        if !(min.y > max.y) {
            translation.y = translation.y.max(min.y).min(max.y);
        }

        let translation = rot_vec(&axis_rotation, translation);

        if !get_v3(rt, n, "trackPoint_changed").equals(&track_point) {
            set(
                rt,
                n,
                "trackPoint_changed",
                FieldValue::SFVec3f(track_point),
            );
        }

        if !get_v3(rt, n, "translation_changed").equals(&translation) {
            set(
                rt,
                n,
                "translation_changed",
                FieldValue::SFVec3f(translation),
            );
        }
    }

    // --- CylinderSensor ------------------------------------------------------------------------

    fn cylinder_is_behind(st: &SensorState, hit_ray: &Line3, hit_point: &Vector3) -> bool {
        let (mut enter, mut exit) = (Vector3::ZERO, Vector3::ZERO);
        st.cylinder.intersects_line(hit_ray, &mut enter, &mut exit);
        hit_point.distance(&enter) > hit_point.distance(&exit)
    }

    fn cylinder_track_point(st: &SensorState, hit_ray: &Line3) -> Vector3 {
        let mut z_point = Vector3::ZERO;
        st.z_plane.intersects_line(hit_ray, &mut z_point);
        let mut axis_point = z_point;
        axis_point.add(&st.cylinder.axis.get_perpendicular_vector_to_point(&z_point));
        let distance = st.sx_plane.get_distance_to_point(&z_point) / st.cylinder.radius;
        let section = ((distance + 1.0) / 2.0).floor();
        let sinp = interval(distance, -1.0, 1.0);
        let phi = if section == 0.0 {
            sinp.asin()
        } else {
            sinp * std::f64::consts::PI / 2.0
        };
        let angle = phi + section * std::f64::consts::PI;
        let d = st.cylinder.axis.direction;
        let rotation = Rotation4::new(d.x, d.y, d.z, angle);
        let mut tp = st.sz_normal;
        tp.multiply(st.cylinder.radius);
        rotation.mult_vec_rot(&mut tp);
        tp.add(&axis_point);
        tp
    }

    fn cylinder_angle(st: &SensorState, rotation: &mut Rotation4) -> f64 {
        if rotation.get_axis().dot(&st.cylinder.axis.direction) > 0.0 {
            rotation.angle()
        } else {
            -rotation.angle()
        }
    }

    fn cylinder_activate(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        let mut hit_ray = hit.ray.copy();
        hit_ray.mult_line_matrix(&st.inv_mv);
        let hit_point = mult_vec_matrix(&st.inv_mv, hit.point);
        let axis_rotation = get_rot(rt, n, "axisRotation");
        let y_axis = rot_vec(&axis_rotation, Vector3::new(0.0, 1.0, 0.0));
        let mut camera_back = mult_dir_matrix(&st.inv_mv, Vector3::new(0.0, 0.0, 1.0));
        camera_back.normalize();

        let mut ya = y_axis;
        ya.normalize();
        let axis = Line3::new(Vector3::ZERO, ya);
        let radius = axis.get_perpendicular_vector_to_point(&hit_point).norm();

        st.cylinder = Cylinder3::new(axis, radius);
        st.disk = camera_back.dot(&y_axis).abs() > get_f(rt, n, "diskAngle").cos();
        st.behind = Self::cylinder_is_behind(st, &hit_ray, &hit_point);
        st.y_plane = Plane3::new(hit_point, y_axis);
        st.z_plane = Plane3::new(hit_point, camera_back);

        let billboard_to_viewer = Vector3::new(st.inv_mv.0[12], st.inv_mv.0[13], st.inv_mv.0[14]);
        let mut sx_normal = y_axis;
        sx_normal.cross(&billboard_to_viewer).normalize();
        st.sx_plane = Plane3::new(Vector3::ZERO, sx_normal);
        let mut sz = sx_normal;
        sz.cross(&y_axis).normalize();
        st.sz_normal = sz;

        let track_point = if st.disk {
            let mut tp = Vector3::ZERO;
            st.y_plane.intersects_line(&hit_ray, &mut tp);
            tp
        } else {
            Self::cylinder_track_point(st, &hit_ray)
        };

        let mut fv = st
            .cylinder
            .axis
            .get_perpendicular_vector_to_point(&track_point);
        fv.negate();
        st.from_vector = fv;
        let offset = get_f(rt, n, "offset");
        st.start_offset_r = Rotation4::new(y_axis.x, y_axis.y, y_axis.z, offset);

        set(
            rt,
            n,
            "trackPoint_changed",
            FieldValue::SFVec3f(track_point),
        );
        set(
            rt,
            n,
            "rotation_changed",
            FieldValue::SFRotation(st.start_offset_r),
        );

        st.angle = offset;
        let rc = get_rot(rt, n, "rotation_changed");
        st.start_vector = rot_vec(&rc, rot_vec(&axis_rotation, Vector3::new(0.0, 0.0, 1.0)));
    }

    fn cylinder_motion(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        st.motion_hit = Some(hit.clone());

        let mut hit_ray = hit.ray.copy();
        hit_ray.mult_line_matrix(&st.inv_mv);

        let track_point = if st.disk {
            let mut tp = Vector3::ZERO;
            st.y_plane.intersects_line(&hit_ray, &mut tp);
            tp
        } else {
            Self::cylinder_track_point(st, &hit_ray)
        };

        set(
            rt,
            n,
            "trackPoint_changed",
            FieldValue::SFVec3f(track_point),
        );

        let mut to_vector = st
            .cylinder
            .axis
            .get_perpendicular_vector_to_point(&track_point);
        to_vector.negate();
        let mut rotation = Rotation4::identity();
        rotation.set_vectors(&st.from_vector, &to_vector);

        if st.disk {
            let tp = mult_vec_matrix(&st.mv, track_point);

            if tp.z > 0.0 {
                let pn = st.y_plane.normal;
                rotation.mult_right(&Rotation4::new(pn.x, pn.y, pn.z, std::f64::consts::PI));
            }
        } else if st.behind {
            rotation.inverse();
        }

        rotation.mult_left(&st.start_offset_r);

        let (min_a, max_a) = (get_f(rt, n, "minAngle"), get_f(rt, n, "maxAngle"));

        if min_a > max_a {
            set(rt, n, "rotation_changed", FieldValue::SFRotation(rotation));
        } else {
            let axis_rotation = get_rot(rt, n, "axisRotation");
            let end_vector = rot_vec(
                &rotation,
                rot_vec(&axis_rotation, Vector3::new(0.0, 0.0, 1.0)),
            );
            let mut delta = Rotation4::identity();
            delta.set_vectors(&st.start_vector, &end_vector);
            let axis = rot_vec(&axis_rotation, Vector3::new(0.0, 1.0, 0.0));
            let sign = if axis.dot(&delta.get_axis()) > 0.0 {
                1.0
            } else {
                -1.0
            };

            st.angle += sign * delta.angle();
            st.start_vector = end_vector;

            let d = st.cylinder.axis.direction;
            let a = if st.angle < min_a {
                min_a
            } else if st.angle > max_a {
                max_a
            } else {
                st.angle
            };
            rotation = Rotation4::new(d.x, d.y, d.z, a);

            if !get_rot(rt, n, "rotation_changed").equals(&rotation) {
                set(rt, n, "rotation_changed", FieldValue::SFRotation(rotation));
            }
        }
    }

    // --- SphereSensor --------------------------------------------------------------------------

    fn sphere_track_point(
        st: &SensorState,
        hit_ray: &Line3,
        track_point: &mut Vector3,
        behind: bool,
    ) -> bool {
        let mut exit = Vector3::ZERO;

        if st.sphere.intersects_line(hit_ray, track_point, &mut exit) {
            let closer_exit = hit_ray.point.distance(&exit) < hit_ray.point.distance(track_point);

            if (closer_exit as i32 - behind as i32) != 0 {
                *track_point = exit;
            }

            return true;
        }

        false
    }

    fn sphere_activate(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        let hit_point = mult_vec_matrix(&st.inv_mv, hit.point);
        let center = Vector3::ZERO;
        let mut zn = mult_dir_matrix(&st.inv_mv, Vector3::new(0.0, 0.0, 1.0));
        zn.normalize();

        st.z_plane = Plane3::new(center, zn);
        st.sphere = Sphere3::new(hit_point.norm(), center);
        st.behind = st.z_plane.get_distance_to_point(&hit_point) < 0.0;
        st.from_vector = hit_point;
        st.start_point = hit_point;
        st.start_offset_r = get_rot(rt, n, "offset");

        set(rt, n, "trackPoint_changed", FieldValue::SFVec3f(hit_point));
        let off = get_rot(rt, n, "offset");
        set(rt, n, "rotation_changed", FieldValue::SFRotation(off));
    }

    fn sphere_motion(rt: &mut Runtime, n: NodeId, st: &mut SensorState, hit: &Hit) {
        let mut hit_ray = hit.ray.copy();
        hit_ray.mult_line_matrix(&st.inv_mv);
        let mut track_point = Vector3::ZERO;

        if Self::sphere_track_point(st, &hit_ray, &mut track_point, st.behind) {
            let mut z_axis = mult_dir_matrix(&st.inv_mv, Vector3::new(0.0, 0.0, 1.0));
            z_axis.normalize();
            st.z_plane = Plane3::new(track_point, z_axis);
        } else {
            let mut tangent = Vector3::ZERO;
            st.z_plane.intersects_line(&hit_ray, &mut tangent);

            let mut dir = st.sphere.center;
            dir.subtract(&tangent).normalize();
            hit_ray.set(&tangent, &dir);

            Self::sphere_track_point(st, &hit_ray, &mut track_point, false);

            let mut tri_normal = Vector3::ZERO;
            Triangle3::normal(
                &st.sphere.center,
                &track_point,
                &st.start_point,
                &mut tri_normal,
            );
            let mut dir_from_center = track_point;
            dir_from_center.subtract(&st.sphere.center).normalize();
            let mut normal = tri_normal;
            normal.cross(&dir_from_center).normalize();

            let mut tmp = tangent;
            tmp.subtract(&track_point);
            let mut off = normal;
            off.multiply(tmp.norm());
            let mut point1 = track_point;
            point1.subtract(&off);

            let mut dir = st.sphere.center;
            dir.subtract(&point1).normalize();
            hit_ray.set(&point1, &dir);

            Self::sphere_track_point(st, &hit_ray, &mut track_point, false);
        }

        set(
            rt,
            n,
            "trackPoint_changed",
            FieldValue::SFVec3f(track_point),
        );

        let mut to_vector = track_point;
        to_vector.subtract(&st.sphere.center);
        let mut rotation = Rotation4::identity();
        rotation.set_vectors(&st.from_vector, &to_vector);

        if st.behind {
            rotation.inverse();
        }

        let mut r = st.start_offset_r;
        r.mult_right(&rotation);
        set(rt, n, "rotation_changed", FieldValue::SFRotation(r));
    }
}

#[allow(dead_code)]
fn _m3(_: &Matrix3) {}
