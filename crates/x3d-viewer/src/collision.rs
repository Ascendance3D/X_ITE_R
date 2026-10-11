//! Collision and gravity: X3DRenderObject.constrainTranslation, getClosestObject,
//! getCollisionShape, gravitate, X3DLayerNode.collision, TextureBuffer.readDepth.
//!
//! X_ITE renders the collision shapes into a 16x16 orthographic depth buffer and reads the
//! closest depth back. Here the same orthographic camera samples the 16x16 pixel centers on the
//! CPU against the triangles (both faces, like `gl .disable (gl .CULL_FACE)`), which gives the
//! same depth up to rasterization coverage.

use crate::geom::{Camera, Plane3};
use crate::layer::{NavInfo, Vp, VpBase};
use std::rc::Rc;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};
use x3d_render::geometry::Geometry;

const DEPTH_BUFFER_SIZE: usize = 16;

pub struct CShape {
    pub geometry: Rc<Geometry>,
    /// Shape model-view matrix (view space of the active viewpoint).
    pub model_view: Matrix4,
}

pub struct Closest {
    pub distance: f64,
    /// Normal in the frame the translation lives in (None when nothing was hit).
    pub normal: Vector3,
    pub hit: bool,
}

#[derive(Default)]
pub struct World {
    pub shapes: Vec<CShape>,
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
    /// Closest depth under an orthographic view (l, r, b, t, near, far) of the collision camera.
    fn depth(&self, view: &Matrix4, (l, r, b, t, near, far): (f64, f64, f64, f64, f64, f64)) -> Option<(f64, Vector3)> {
        let mut best: Option<(f64, Vector3)> = None;
        let n = DEPTH_BUFFER_SIZE;

        for shape in &self.shapes {
            let g = &shape.geometry;

            if g.geometry_type < 2 || g.vertices.is_empty() {
                continue;
            }

            let mut m = shape.model_view;
            m.mult_right(view);

            // Bounding box reject.
            let (mut lo, mut hi) = (Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY), Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY));
            for i in 0..8 {
                let mut p = Vector3::new(if i & 1 == 0 { g.min.x } else { g.max.x }, if i & 2 == 0 { g.min.y } else { g.max.y }, if i & 4 == 0 { g.min.z } else { g.max.z });
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
            (near + z * (far - near), n)
        })
    }

    /// X3DRenderObject.getClosestObject: the distance to the closest object in `direction`,
    /// at most avatarHeight * 2.
    pub fn closest_object(&self, vp: &Vp, base: &VpBase, nav: &NavInfo, direction: &Vector3) -> Closest {
        let radius = nav.collision_radius();
        let bottom = nav.step_height() - nav.avatar_height();
        let near = vp.near_distance(base, nav);
        let avatar_height = nav.avatar_height();

        let mut rotation = Rotation4::identity();
        rotation.set_vectors(&Vector3::new(0.0, 0.0, -1.0), direction);
        // localOrientation = orientation field ^-1 * getOrientation () = identity.
        let rotation = x3d_render::traverse::straighten(&rotation, &Vector3::Y_AXIS);

        let far = (radius * 2.0).max(avatar_height * 2.0);
        let view = camera_matrix(vp, base, &rotation);
        let mut proj = Matrix4::IDENTITY;
        Camera::ortho(-radius, radius, bottom.min(-radius), radius, near, far, &mut proj);

        match self.depth(&view, (-radius, radius, bottom.min(-radius), radius, near, far)) {
            Some((d, n)) => {
                let mut normal = n;
                rotation.mult_vec_rot(&mut normal);
                normal.normalize();
                Closest { distance: d, normal, hit: true }
            }
            None => Closest { distance: far, normal: Vector3::new(0.0, 0.0, 1.0), hit: false },
        }
    }

    /// X3DRenderObject.constrainTranslation.
    pub fn constrain_translation(&self, vp: &Vp, base: &VpBase, nav: &NavInfo, translation: Vector3, step_back: bool, slide: bool, wall_friction: f64) -> Vector3 {
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

                let plane = Plane3::new(Vector3::ZERO, Vector3::Y_AXIS);
                let mut constrained = plane.get_closest_point_to_point(&closest_pt);

                constrained.subtract(&point).normalize().multiply(length * (1.0 - friction));

                return self.constrain_translation(vp, base, nav, constrained, false, false, wall_friction);
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
    pub fn gravitate(&self, vp: &Vp, base: &VpBase, nav: &NavInfo, speed: &mut f64, frame_rate: f64, gravity: f64) -> Option<Vector3> {
        let radius = nav.collision_radius();
        let avatar_height = nav.avatar_height();
        let step_height = nav.step_height();
        let near = vp.near_distance(base, nav);
        let far = radius.max(avatar_height) * 2.0;

        let up = Vector3::Y_AXIS;
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
        // CPU depth here is exact, so a standing avatar is treated as grounded within 1e-6.
        if distance > 1e-6 {
            // Gravitate and fall down to the floor.
            let rate = if *speed != 0.0 { frame_rate } else { 1_000_000.0 };
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
