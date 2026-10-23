//! Collision and gravity: X3DRenderObject.constrainTranslation, getClosestObject,
//! getCollisionShape, gravitate, X3DLayerNode.collision, TextureBuffer.readDepth.
//!
//! X_ITE renders the collision shapes into a 16x16 orthographic depth buffer and reads the
//! closest depth back. Here the same orthographic camera samples the 16x16 pixel centers on the
//! CPU against the triangles (both faces, like `gl .disable (gl .CULL_FACE)`), which gives the
//! same depth up to rasterization coverage.
//!
//! An active ParticleSystem is a collision shape too (X3DShapeNode.traverse COLLISION): each drawn particle is one
//! [`CShape`] under its instance matrix. With the GPU its POINT particles are drawn as 1 px points like Depth2.vs does,
//! and its LINE particles are rasterized here by GL's diamond-exit rule (a WebGPU line list covers other pixels) and
//! merged into the read-back buffer. The CPU stand-in samples triangles only, so it misses POINT and LINE particles (and
//! the points and lines of PointSet and LineSet shapes, which neither path draws).

use crate::geom::{Camera, Plane3, ViewVolume};
use crate::layer::{NavInfo, Vp, VpBase};
use std::cell::RefCell;
use std::rc::Rc;
use x3d_fields::math::{Matrix4, Rotation4, Vector3, Vector4};
use x3d_render::depth_gpu::{DepthGpu, DepthShape};
use x3d_render::geometry::Geometry;

const DEPTH_BUFFER_SIZE: usize = 16;
/// Sub-pixel positions per pixel of the rasterizer's vertex snapping (subPixelPrecisionBits 8).
const SUB_PIXELS: f64 = 256.0;

pub struct CShape {
    /// The Shape, or the ParticleSystem of a particle (`hit .shapeNode`).
    pub node: crate::walk::NodeRef,
    pub geometry: Rc<Geometry>,
    /// One particle of a ParticleSystem: its points and lines are drawn (ParticleSystem.displaySimple).
    pub particle: bool,
    /// Shape model-view matrix (view space of the active viewpoint); for an InstancedShape
    /// instance, the instance matrix folded in.
    pub model_view: Matrix4,
    /// The shape's own model-view matrix and the instance matrix an InstancedShape applies
    /// first (the depth shader applies them separately).
    pub base_model_view: Matrix4,
    pub instance: Option<Matrix4>,
    /// The ClipPlanes in scope, in the same view space (renderContext .clipPlanes).
    pub clip_planes: Rc<[[f64; 4]]>,
    /// The Collision nodes the COLLISION traversal was inside of when it reached the shape
    /// (renderContext .collisions), outermost first.
    pub collisions: Rc<[x3d_fields::NodeId]>,
}

pub struct Closest {
    /// The collision shape (index into [`World::shapes`]) the depth pass found; `None` without a hit
    /// and on the CPU stand-in.
    pub shape: Option<usize>,
    pub distance: f64,
    /// Normal in the frame the translation lives in (None when nothing was hit).
    pub normal: Vector3,
    pub hit: bool,
}

/// How the collision depth pass draws triangles. Shading WIREFRAME sets the GL polygon mode
/// globally (`gl .LINE`), so X_ITE's collision and gravity depth renders inherit it; POINT only
/// changes the primitive mode of the display shaders (measured: the avatar still stands).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum Fill {
    #[default]
    Triangles,
    Edges,
}

#[derive(Default)]
pub struct World {
    pub shapes: Vec<CShape>,
    pub fill: Fill,
    /// The depth buffer on the GPU (X_ITE's own rasterization); without it, or while the polygon
    /// mode is not FILL, the CPU sampling below stands in.
    pub gpu: Option<Rc<RefCell<DepthGpu>>>,
}

/// The fragments a 1 px gl.LINES segment from `a` to `b` (camera space of the collision camera, which looks along -z)
/// covers in the 16x16 buffer of the orthographic view (l, r, b, t, near, far): (column, row, window z, camera space
/// point). The segment is clipped to the near and far planes and its window x and y snapped to the sub-pixel grid, then
/// GL's diamond-exit rule decides: a pixel is produced when the segment meets its diamond |x - xc| + |y - yc| < 1/2 and
/// does not end inside it. z and the point are interpolated at the projection of the pixel center onto the segment.
fn line_fragments(
    a: Vector3,
    b: Vector3,
    (l, r, bottom, t, near, far): (f64, f64, f64, f64, f64, f64),
) -> Vec<(usize, usize, f64, Vector3)> {
    let lerp = |p: &Vector3, q: &Vector3, s: f64| {
        Vector3::new(
            p.x + s * (q.x - p.x),
            p.y + s * (q.y - p.y),
            p.z + s * (q.z - p.z),
        )
    };
    let (mut a, mut b) = (a, b);

    // GL clips the segment to the near and far planes before it snaps the ends: (axis, bound, keep the side below).
    // x and y are not clipped (the guard band): the pixels outside the buffer are not produced.
    for (axis, bound, below) in [(2, -near, true), (2, -far, false)] {
        let at = |p: &Vector3| [p.x, p.y, p.z][axis];
        let inside = |p: &Vector3| {
            if below {
                at(p) <= bound
            } else {
                at(p) >= bound
            }
        };

        match (inside(&a), inside(&b)) {
            (true, true) => {}
            (false, false) => return vec![],
            (true, false) => b = lerp(&a, &b, (bound - at(&a)) / (at(&b) - at(&a))),
            (false, true) => a = lerp(&a, &b, (bound - at(&a)) / (at(&b) - at(&a))),
        }
    }

    let n = DEPTH_BUFFER_SIZE as f64;
    let window = |p: &Vector3| {
        (
            (p.x - l) / (r - l) * n,
            (p.y - bottom) / (t - bottom) * n,
            (-p.z - near) / (far - near),
        )
    };
    // The rasterizer snaps window x and y to its sub-pixel grid (8 bits).
    let snap = |v: f64| (v * SUB_PIXELS).round() / SUB_PIXELS;
    let (ax, ay, az) = window(&a);
    let (bx, by, bz) = window(&b);
    let (ax, ay, bx, by) = (snap(ax), snap(ay), snap(bx), snap(by));
    let (dx, dy) = (bx - ax, by - ay);
    let range = |u: f64, v: f64| {
        let lo = (u.min(v).floor() - 1.0).max(0.0);
        let hi = (u.max(v).floor() + 1.0).min(n - 1.0);
        (lo as i64)..=(hi as i64)
    };
    let mut fragments = vec![];

    for row in range(ay, by) {
        for column in range(ax, bx) {
            let (cx, cy) = (column as f64 + 0.5, row as f64 + 0.5);
            let l1 = |s: f64| (ax + s * dx - cx).abs() + (ay + s * dy - cy).abs();

            // The segment ends inside the diamond: it does not exit it.
            if l1(1.0) < 0.5 {
                continue;
            }

            // |x| + |y| along the segment is convex and piecewise linear: its least value is at an end or where x
            // or y is 0.
            let mut least = l1(0.0).min(l1(1.0));

            if dx != 0.0 {
                least = least.min(l1(((cx - ax) / dx).clamp(0.0, 1.0)));
            }

            if dy != 0.0 {
                least = least.min(l1(((cy - ay) / dy).clamp(0.0, 1.0)));
            }

            if least >= 0.5 {
                continue;
            }

            let length2 = dx * dx + dy * dy;
            let s = if length2 > 0.0 {
                (((cx - ax) * dx + (cy - ay) * dy) / length2).clamp(0.0, 1.0)
            } else {
                0.0
            };

            fragments.push((
                column as usize,
                row as usize,
                az + s * (bz - az),
                lerp(&a, &b, s),
            ));
        }
    }

    fragments
}

/// The collision camera: `viewpoint model · translate (userPosition) · rotate (rotation)`,
/// inverted and multiplied on the left by the viewpoint's camera space matrix.
fn camera_matrix(vp: &Vp, base: &VpBase, rotation: &Rotation4) -> Matrix4 {
    let mut m = vp.model;
    m.translate(&vp.user_position(base));
    m.rotate(rotation);
    m.inverse();
    m.mult_left(&vp.camera_space);
    m
}

impl World {
    /// X3DRenderObject.getCollisionShape on the GPU: the collision shapes drawn into the 16x16
    /// buffer with `view · ortho` as projection, then TextureBuffer.readDepth: the pixel with the
    /// smallest window z, unprojected with the orthographic matrix. Returns the distance and, if a
    /// shape was hit, its normal in the frame of the translation (`closestObject .modelViewMatrix
    /// .multRight (viewMatrix) .submatrix .inverse () .multMatrixVec (normal)`).
    fn depth_gpu(
        &self,
        gpu: &Rc<RefCell<DepthGpu>>,
        view: &Matrix4,
        ortho: &Matrix4,
        bounds: (f64, f64, f64, f64, f64, f64),
    ) -> (f64, Option<(Vector3, usize)>) {
        let mut view_projection = *view;
        view_projection.mult_right(ortho);

        let f32x16 = |m: &Matrix4| m.0.map(|v| v as f32);
        let shapes: Vec<DepthShape> = self
            .shapes
            .iter()
            .map(|s| {
                let g = &s.geometry;

                DepthShape {
                    geometry: g,
                    empty: (g.geometry_type < 2 && !s.particle)
                        || g.geometry_type == 1
                        || g.vertices.is_empty(),
                    model_view: f32x16(&s.base_model_view),
                    instance: s
                        .instance
                        .as_ref()
                        .map_or(x3d_render::pointing_gpu::IDENTITY, f32x16),
                    // A ParticleSystem's depth shader has X3D_INSTANCING without X3D_INSTANCE_NORMAL.
                    instance_normal: s.instance.as_ref().filter(|_| !s.particle).map_or(
                        x3d_render::pointing_gpu::IDENTITY3,
                        |m| {
                            // `matrix .submatrix .transpose () .inverse ()`
                            let mut n = m.submatrix();
                            n.transpose();
                            n.inverse();
                            n.0.map(|v| v as f32)
                        },
                    ),
                    clip_planes: &s.clip_planes,
                }
            })
            .collect();
        let mut pixels = gpu.borrow_mut().draw(&f32x16(&view_projection), &shapes);

        // The LINE particles, in draw order after the GPU's shapes (a depth tie keeps the GPU's pixel).
        for (id, s) in self.shapes.iter().enumerate() {
            let g = &s.geometry;

            if !s.particle || g.geometry_type != 1 || g.vertices.len() < 8 {
                continue;
            }

            let mut m = s.model_view;
            m.mult_right(view);

            let point = |k: usize| {
                let v = &g.vertices[k * 4..k * 4 + 3];
                let mut p = Vector3::new(v[0] as f64, v[1] as f64, v[2] as f64);
                m.mult_vec_matrix3(&mut p);
                p
            };
            let planes: Vec<[f64; 4]> = s
                .clip_planes
                .iter()
                .map(|p| x3d_render::traverse::plane_mult_right(*p, view))
                .collect();

            for (wx, wy, wz, at) in line_fragments(point(0), point(1), bounds) {
                if !planes.is_empty() && x3d_render::traverse::is_clipped(&planes, &at) {
                    continue;
                }

                let i = wy * DEPTH_BUFFER_SIZE + wx;
                let wz = wz as f32;

                if wz < pixels.data0[i][0] {
                    pixels.data0[i] = [wz, id as f32, id as f32, id as f32];
                    // Depth2.fs without X3D_NORMALS: normal 0 0 1, gl_FrontFacing TRUE.
                    pixels.data1[i] = [0.0, 0.0, 1.0, 1.0];
                }
            }
        }

        // TextureBuffer.readDepth
        let (mut win_x, mut win_y, mut win_z, mut index) = (0usize, 0usize, f64::INFINITY, 0usize);

        for wy in 0..DEPTH_BUFFER_SIZE {
            for wx in 0..DEPTH_BUFFER_SIZE {
                let wz = pixels.data0[wy * DEPTH_BUFFER_SIZE + wx][0] as f64;

                if wz < win_z {
                    (win_x, win_y, win_z, index) = (wx, wy, wz, wy * DEPTH_BUFFER_SIZE + wx);
                }
            }
        }

        let mut inverse = *ortho;
        inverse.inverse();
        let mut point = Vector3::ZERO;
        let size = DEPTH_BUFFER_SIZE as f64;
        ViewVolume::unproject_point_matrix(
            win_x as f64,
            win_y as f64,
            win_z,
            &inverse,
            &Vector4::new(0.0, 0.0, size, size),
            &mut point,
        );
        let distance = -point.z;
        let id = pixels.data0[index][1];

        if id < 0.0 || id as usize >= self.shapes.len() {
            return (distance, None);
        }

        let n = pixels.data1[index];
        let mut normal = Vector3::new(n[0] as f64, n[1] as f64, n[2] as f64);
        // renderContext .modelViewMatrix is a Float32Array.
        let mut mv = self.shapes[id as usize].base_model_view;
        mv.0 = mv.0.map(|v| v as f32 as f64);
        mv.mult_right(view);
        let mut sub = mv.submatrix();
        sub.inverse();
        sub.mult_matrix_vec3(&mut normal);
        (distance, Some((normal, id as usize)))
    }

    /// Closest depth under an orthographic view (l, r, b, t, near, far) of the collision camera.
    fn depth(
        &self,
        view: &Matrix4,
        (l, r, b, t, near, far): (f64, f64, f64, f64, f64, f64),
    ) -> Option<(f64, Vector3, Option<usize>)> {
        if let (Some(gpu), Fill::Triangles) = (&self.gpu, self.fill) {
            let mut ortho = Matrix4::IDENTITY;
            Camera::ortho(l, r, b, t, near, far, &mut ortho);
            let (distance, normal) = self.depth_gpu(gpu, view, &ortho, (l, r, b, t, near, far));
            return normal.map(|(n, id)| (distance, n, Some(id)));
        }

        let mut best: Option<(f64, Vector3)> = None;
        let n = DEPTH_BUFFER_SIZE;

        for shape in &self.shapes {
            let g = &shape.geometry;

            if g.geometry_type < 2 || g.vertices.is_empty() {
                continue;
            }

            let mut m = shape.model_view;
            m.mult_right(view);

            // The depth shader's clip planes, moved into the collision camera's space.
            let planes: Vec<[f64; 4]> = shape
                .clip_planes
                .iter()
                .map(|p| x3d_render::traverse::plane_mult_right(*p, view))
                .collect();

            // Bounding box reject.
            let (mut lo, mut hi) = (
                Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY),
                Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY),
            );
            for i in 0..8 {
                let mut p = Vector3::new(
                    if i & 1 == 0 { g.min.x } else { g.max.x },
                    if i & 2 == 0 { g.min.y } else { g.max.y },
                    if i & 4 == 0 { g.min.z } else { g.max.z },
                );
                m.mult_vec_matrix3(&mut p);
                lo.min(&p);
                hi.max(&p);
            }

            if hi.x < l || lo.x > r || hi.y < b || lo.y > t || hi.z < -far || lo.z > -near {
                continue;
            }

            let tri = |k: usize| -> Vector3 {
                let v = &g.vertices[k * 4..k * 4 + 3];
                let mut p = Vector3::new(v[0] as f64, v[1] as f64, v[2] as f64);
                m.mult_vec_matrix3(&mut p);
                p
            };

            for t3 in 0..g.vertex_count() / 3 {
                let (a, bb, c) = (tri(t3 * 3), tri(t3 * 3 + 1), tri(t3 * 3 + 2));

                if self.fill != Fill::Triangles {
                    let mut e1 = bb;
                    e1.subtract(&a);
                    let mut e2 = c;
                    e2.subtract(&a);
                    let mut nrm = e1;
                    nrm.cross(&e2).normalize();

                    let mut sample = |px: f64, py: f64, z: f64| {
                        let depth = -z;

                        if depth < near || depth > far {
                            return;
                        }

                        if !planes.is_empty()
                            && x3d_render::traverse::is_clipped(&planes, &Vector3::new(px, py, z))
                        {
                            return;
                        }

                        if best.as_ref().is_none_or(|(d, _)| depth < *d) {
                            best = Some((depth, nrm));
                        }
                    };

                    let to_pixel = |p: &Vector3| {
                        (
                            (p.x - l) / (r - l) * n as f64,
                            (p.y - b) / (t - b) * n as f64,
                        )
                    };

                    {
                        // GL clips the polygon before it draws its edges, so a triangle cut by
                        // the near or far plane gets an edge along that plane (Sutherland-Hodgman
                        // against z = -near and z = -far; x and y fall in the guard band).
                        let mut polygon = vec![a, bb, c];

                        for (plane_z, keep_behind) in [(-near, true), (-far, false)] {
                            let inside = |p: &Vector3| {
                                if keep_behind {
                                    p.z <= plane_z
                                } else {
                                    p.z >= plane_z
                                }
                            };
                            let mut out = Vec::with_capacity(polygon.len() + 1);

                            for i in 0..polygon.len() {
                                let (p, q) = (polygon[i], polygon[(i + 1) % polygon.len()]);

                                if inside(&p) {
                                    out.push(p);
                                }

                                if inside(&p) != inside(&q) {
                                    let s = (plane_z - p.z) / (q.z - p.z);
                                    out.push(Vector3::new(
                                        p.x + s * (q.x - p.x),
                                        p.y + s * (q.y - p.y),
                                        plane_z,
                                    ));
                                }
                            }

                            polygon = out;
                        }

                        // A 1 pixel line: along its major axis, the pixel row (column) of the
                        // line at each pixel center it passes.
                        let edges: Vec<(Vector3, Vector3)> = (0..polygon.len())
                            .map(|i| (polygon[i], polygon[(i + 1) % polygon.len()]))
                            .collect();

                        for (p, q) in edges {
                            let ((x0, y0), (x1, y1)) = (to_pixel(&p), to_pixel(&q));
                            let x_major = (x1 - x0).abs() >= (y1 - y0).abs();
                            let (u0, u1, v0, v1) = if x_major {
                                (x0, x1, y0, y1)
                            } else {
                                (y0, y1, x0, x1)
                            };

                            if u0 == u1 {
                                continue;
                            }

                            let (lo, hi) = (u0.min(u1), u0.max(u1));
                            let first = ((lo - 0.5).ceil()).max(0.0) as i64;
                            let last = ((hi - 0.5).ceil()).min(n as f64) as i64;

                            for i in first..last {
                                let center = i as f64 + 0.5;
                                let s = (center - u0) / (u1 - u0);
                                let v = v0 + s * (v1 - v0);

                                if v < 0.0 || v >= n as f64 {
                                    continue;
                                }

                                let (px, py) = if x_major {
                                    (center, v.floor() + 0.5)
                                } else {
                                    (v.floor() + 0.5, center)
                                };
                                let z = p.z + s * (q.z - p.z);
                                sample(l + px * (r - l) / n as f64, b + py * (t - b) / n as f64, z);
                            }
                        }
                    }

                    continue;
                }

                let (min_x, max_x) = (a.x.min(bb.x).min(c.x), a.x.max(bb.x).max(c.x));
                let (min_y, max_y) = (a.y.min(bb.y).min(c.y), a.y.max(bb.y).max(c.y));

                if max_x < l || min_x > r || max_y < b || min_y > t {
                    continue;
                }

                let det = (bb.y - c.y) * (a.x - c.x) + (c.x - bb.x) * (a.y - c.y);

                if det == 0.0 {
                    continue;
                }

                let i0 = (((min_x - l) / (r - l) * n as f64).floor().max(0.0)) as usize;
                let i1 = (((max_x - l) / (r - l) * n as f64).ceil().min(n as f64)) as usize;
                let j0 = (((min_y - b) / (t - b) * n as f64).floor().max(0.0)) as usize;
                let j1 = (((max_y - b) / (t - b) * n as f64).ceil().min(n as f64)) as usize;

                for j in j0..j1 {
                    for i in i0..i1 {
                        let px = l + (i as f64 + 0.5) * (r - l) / n as f64;
                        let py = b + (j as f64 + 0.5) * (t - b) / n as f64;
                        let w0 = ((bb.y - c.y) * (px - c.x) + (c.x - bb.x) * (py - c.y)) / det;
                        let w1 = ((c.y - a.y) * (px - c.x) + (a.x - c.x) * (py - c.y)) / det;
                        let w2 = 1.0 - w0 - w1;

                        if w0 < 0.0 || w1 < 0.0 || w2 < 0.0 {
                            continue;
                        }

                        let z = w0 * a.z + w1 * bb.z + w2 * c.z;
                        let depth = -z;

                        if depth < near || depth > far {
                            continue;
                        }

                        if !planes.is_empty()
                            && x3d_render::traverse::is_clipped(&planes, &Vector3::new(px, py, z))
                        {
                            continue;
                        }

                        if best.as_ref().is_none_or(|(d, _)| depth < *d) {
                            let mut e1 = bb;
                            e1.subtract(&a);
                            let mut e2 = c;
                            e2.subtract(&a);
                            let mut nrm = e1;
                            nrm.cross(&e2).normalize();
                            best = Some((depth, nrm));
                        }
                    }
                }
            }
        }

        // The depth buffer is a float color attachment: the window z is stored as an f32 and
        // TextureBuffer.readDepth unprojects it again.
        best.map(|(d, n)| {
            let z = ((d - near) / (far - near)) as f32 as f64;
            (near + z * (far - near), n, None)
        })
    }

    /// X3DRenderObject.getClosestObject: the distance to the closest object in `direction`,
    /// at most avatarHeight * 2.
    pub fn closest_object(
        &self,
        vp: &Vp,
        base: &VpBase,
        nav: &NavInfo,
        direction: &Vector3,
    ) -> Closest {
        let radius = nav.collision_radius();
        let bottom = nav.step_height() - nav.avatar_height();
        let near = vp.near_distance(base, nav);
        let avatar_height = nav.avatar_height();

        let mut rotation = Rotation4::identity();
        rotation.set_vectors(&Vector3::new(0.0, 0.0, -1.0), direction);
        // localOrientation = orientation field ^-1 * getOrientation ().
        rotation.mult_right(&base.local_orientation());
        let rotation = base.straighten_horizon(vp, &rotation);

        let far = (radius * 2.0).max(avatar_height * 2.0);
        let view = camera_matrix(vp, base, &rotation);
        let mut proj = Matrix4::IDENTITY;
        Camera::ortho(
            -radius,
            radius,
            bottom.min(-radius),
            radius,
            near,
            far,
            &mut proj,
        );

        match self.depth(
            &view,
            (-radius, radius, bottom.min(-radius), radius, near, far),
        ) {
            Some((d, n, shape)) => {
                let mut normal = n;
                rotation.mult_vec_rot(&mut normal);
                normal.normalize();
                Closest {
                    shape,
                    distance: d,
                    normal,
                    hit: true,
                }
            }
            None => Closest {
                shape: None,
                distance: far,
                normal: Vector3::new(0.0, 0.0, 1.0),
                hit: false,
            },
        }
    }

    /// X3DRenderObject.constrainTranslation.
    pub fn constrain_translation(
        &self,
        vp: &Vp,
        base: &VpBase,
        nav: &NavInfo,
        translation: Vector3,
        step_back: bool,
        slide: bool,
        wall_friction: f64,
    ) -> Vector3 {
        let closest = self.closest_object(vp, base, nav, &translation);
        let radius = nav.collision_radius();
        let distance = closest.distance - radius;

        if distance > 0.0 {
            let length = translation.norm();

            if length > distance {
                let mut point = translation;
                point.normalize().multiply(distance);

                if !slide {
                    return point;
                }

                let friction = wall_friction.clamp(0.0, 1.0);

                let plane = Plane3::new(point, closest.normal);
                let closest_pt = plane.get_closest_point_to_point(&translation);

                let plane = Plane3::new(Vector3::ZERO, base.up_vector(vp, false));
                let mut constrained = plane.get_closest_point_to_point(&closest_pt);

                constrained
                    .subtract(&point)
                    .normalize()
                    .multiply(length * (1.0 - friction));

                return self.constrain_translation(
                    vp,
                    base,
                    nav,
                    constrained,
                    false,
                    false,
                    wall_friction,
                );
            }

            return translation;
        }

        if step_back {
            let mut point = translation;
            point.normalize().multiply(distance);
            return self.constrain_translation(vp, base, nav, point, false, false, wall_friction);
        }

        Vector3::ZERO
    }

    /// X3DRenderObject.gravitate: returns the new position offset delta and the new speed.
    pub fn gravitate(
        &self,
        vp: &Vp,
        base: &VpBase,
        nav: &NavInfo,
        speed: &mut f64,
        frame_rate: f64,
        gravity: f64,
    ) -> Option<Vector3> {
        let radius = nav.collision_radius();
        let avatar_height = nav.avatar_height();
        let step_height = nav.step_height();
        let near = vp.near_distance(base, nav);
        let far = radius.max(avatar_height) * 2.0;

        let up = base.up_vector(vp, false);
        let mut down = Rotation4::identity();
        down.set_vectors(&Vector3::Z_AXIS, &up);

        let view = camera_matrix(vp, base, &down);
        let depth = self.depth(&view, (-radius, radius, -radius, radius, near, far));
        let mut distance = depth.map(|d| d.0).unwrap_or(far);

        distance -= avatar_height;

        let mut up_rot = Rotation4::identity();
        up_rot.set_vectors(&Vector3::Y_AXIS, &up);

        // X_ITE tests `distance > 0`. Its depth is an f32 read back from the GPU (about 2e-7
        // of noise at this range), which settles a standing avatar just on the ground side; the
        // GPU depth here is the same f32, the CPU depth is exact, so a standing avatar is treated
        // as grounded within 1e-6 there.
        let gpu = self.gpu.is_some() && self.fill == Fill::Triangles;

        if distance > if gpu { 0.0 } else { 1e-6 } {
            // Gravitate and fall down to the floor.
            let rate = if *speed != 0.0 {
                frame_rate
            } else {
                1_000_000.0
            };
            *speed -= gravity / rate;

            let mut dy = *speed / rate;

            if dy < -distance {
                dy = -distance;
                *speed = 0.0;
            }

            let mut falling = Vector3::new(0.0, dy, 0.0);
            up_rot.mult_vec_rot(&mut falling);
            Some(falling)
        } else {
            *speed = 0.0;
            distance = -distance;

            if distance > 0.01 && distance < step_height {
                let mut step = Vector3::new(0.0, distance, 0.0);
                up_rot.mult_vec_rot(&mut step);
                Some(step)
            } else {
                None
            }
        }
    }
}
