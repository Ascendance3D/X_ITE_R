//! NURBS component nodes with runtime behavior (X_ITE 16.5.0 Components/NURBS): NurbsSet,
//! the add/remove fields of Contour2D, NurbsSet and NurbsTrimmedSurface, and the
//! NurbsPositionInterpolator, NurbsOrientationInterpolator and NurbsSurfaceInterpolator.
//!
//! The interpolators follow ISO/IEC 19775-1 27.4.5, 27.4.7 and 27.4.9 where they differ from
//! X_ITE (owner decision 2026-10-17): `set_fraction` is the curve parameter (the knots play the
//! role of an interpolator's keys) and the values are those of the rational curve or surface.

use crate::nurbs::{self as m, Spline};
use crate::*;
use x3d_fields::math::{Rotation4, Vector3};

/// The (add, remove, target) fields of the nodes whose add/remove use X_ITE's `filter`.
pub const FILTER_FIELDS: [(&str, &str, &str, &str); 3] = [
    ("Contour2D", "addChildren", "removeChildren", "children"),
    ("NurbsSet", "addGeometry", "removeGeometry", "geometry"),
    (
        "NurbsTrimmedSurface",
        "addTrimmingContour",
        "removeTrimmingContour",
        "trimmingContour",
    ),
];

pub fn is_nurbs_interpolator(name: &str) -> bool {
    matches!(
        name,
        "NurbsPositionInterpolator" | "NurbsOrientationInterpolator" | "NurbsSurfaceInterpolator"
    )
}

impl Runtime {
    pub(crate) fn nurbs_initialize(&mut self, node: NodeId, name: &str) {
        if let Some(i) = FILTER_FIELDS.iter().position(|(t, ..)| *t == name) {
            self.interest(node, FILTER_FIELDS[i].1, Cb::NurbsFilterAdd(i as u8));
            self.interest(node, FILTER_FIELDS[i].2, Cb::NurbsFilterRemove(i as u8));
        }

        match name {
            "NurbsSet" => {
                self.interest(node, "tessellationScale", Cb::NurbsSetScale);
                self.interest(node, "geometry", Cb::NurbsSetGeometry);
                self.nurbs_set_geometry(node);
            }
            n if is_nurbs_interpolator(n) => {
                self.interest(node, "set_fraction", Cb::NurbsInterpSetFraction)
            }
            _ => {}
        }
    }

    fn nodes_of(&self, node: NodeId, name: &str) -> Vec<Option<NodeId>> {
        match self.pre_value(node, name) {
            Some(FieldValue::MFNode(v)) => v.clone(),
            _ => vec![],
        }
    }

    /// `set_addXxx__`: `add .assign (filter (add, target))`, every node pushed onto the target,
    /// `add .length = 0` (no Set: duplicates in `add` stay; no early return).
    pub(crate) fn nurbs_filter_add(&mut self, node: NodeId, i: usize) {
        let (_, add, _, target) = FILTER_FIELDS[i];
        let (Some(add_key), Some(target_key)) = (self.pre(node, add), self.pre(node, target))
        else {
            return;
        };

        self.set_field_tainted(add_key, true);

        let current = self.nodes_of(node, target);
        let filtered: Vec<Option<NodeId>> = self
            .nodes_of(node, add)
            .into_iter()
            .filter(|n| !current.contains(n))
            .collect();

        self.set_raw(add_key, FieldValue::MFNode(filtered.clone()));

        for child in filtered {
            let mut v = self.nodes_of(node, target);
            v.push(child);
            self.set_value(target_key, FieldValue::MFNode(v));
        }

        self.set_raw(add_key, FieldValue::MFNode(vec![]));
        self.set_field_tainted(add_key, false);
    }

    /// `set_removeXxx__`: `target .assign (filter (target, remove))`, `remove .length = 0`.
    pub(crate) fn nurbs_filter_remove(&mut self, node: NodeId, i: usize) {
        let (_, _, remove, target) = FILTER_FIELDS[i];
        let (Some(remove_key), Some(target_key)) = (self.pre(node, remove), self.pre(node, target))
        else {
            return;
        };

        self.set_field_tainted(remove_key, true);

        let remove_nodes = self.nodes_of(node, remove);
        let filtered: Vec<Option<NodeId>> = self
            .nodes_of(node, target)
            .into_iter()
            .filter(|n| !remove_nodes.contains(n))
            .collect();

        self.set_value(target_key, FieldValue::MFNode(filtered));
        self.set_raw(remove_key, FieldValue::MFNode(vec![]));
        self.set_field_tainted(remove_key, false);
    }

    /// NurbsSet.set_geometry__: the old members go back to scale 1, the members are the
    /// X3DNurbsSurfaceGeometryNode nodes of `geometry`, then set_tessellationScale__.
    pub(crate) fn nurbs_set_geometry(&mut self, node: NodeId) {
        for g in self.nurbs_set_members.remove(&node).unwrap_or_default() {
            self.nurbs_set_scale_of(g, 1.0);
        }

        let members: Vec<NodeId> = self
            .nodes_of(node, "geometry")
            .into_iter()
            .flatten()
            .filter_map(|n| self.inner_node(n))
            .filter(|n| {
                matches!(
                    self.builtin_name(*n),
                    Some("NurbsPatchSurface" | "NurbsTrimmedSurface")
                )
            })
            .collect();

        self.nurbs_set_members.insert(node, members);
        self.nurbs_set_scale(node);
    }

    /// NurbsSet.set_tessellationScale__: `Math .max (0, tessellationScale)` on every member.
    pub(crate) fn nurbs_set_scale(&mut self, node: NodeId) {
        let scale = self.get_f64(node, "tessellationScale").max(0.0);

        for g in self
            .nurbs_set_members
            .get(&node)
            .cloned()
            .unwrap_or_default()
        {
            self.nurbs_set_scale_of(g, scale);
        }
    }

    /// X3DNurbsSurfaceGeometryNode.setTessellationScale: store, requestRebuild.
    fn nurbs_set_scale_of(&mut self, geometry: NodeId, scale: f64) {
        self.nurbs_scales.insert(geometry, scale);
        self.touch(geometry);
    }

    /// The tessellationScale of a NURBS surface (1 unless a NurbsSet set it).
    pub fn nurbs_tessellation_scale(&self, geometry: NodeId) -> f64 {
        self.nurbs_scales.get(&geometry).copied().unwrap_or(1.0)
    }

    /// X3DCoordinateNode.get1Point of every point of `controlPoint` (Coordinate: its Float32
    /// values; CoordinateDouble; GeoCoordinate through its geo system).
    fn nurbs_control_points(&self, node: NodeId) -> Option<Vec<[f64; 3]>> {
        let c = match self.pre_value(node, "controlPoint") {
            Some(FieldValue::SFNode(Some(c))) => self.inner_node(*c)?,
            _ => return None,
        };

        match (self.builtin_name(c)?, self.pre_value(c, "point")) {
            ("Coordinate", Some(FieldValue::MFVec3f(p))) => Some(
                p.chunks_exact(3)
                    .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
                    .collect(),
            ),
            ("CoordinateDouble", Some(FieldValue::MFVec3d(p))) => {
                Some(p.chunks_exact(3).map(|p| [p[0], p[1], p[2]]).collect())
            }
            ("GeoCoordinate", Some(FieldValue::MFVec3d(p))) => {
                let geo = self.geo_object(c);
                Some(
                    p.chunks_exact(3)
                        .map(|p| geo.get_coord(&Vector3::new(p[0], p[1], p[2])))
                        .map(|v| [v.x, v.y, v.z])
                        .collect(),
                )
            }
            _ => None,
        }
    }

    fn doubles(&self, node: NodeId, name: &str) -> Vec<f64> {
        match self.pre_value(node, name) {
            Some(FieldValue::MFDouble(v)) => v.clone(),
            _ => vec![],
        }
    }

    fn int(&self, node: NodeId, name: &str, default: i32) -> i32 {
        match self.pre_value(node, name) {
            Some(FieldValue::SFInt32(v)) => *v,
            _ => default,
        }
    }

    /// The curve of a NurbsPosition/OrientationInterpolator (its private NurbsCurve: closed FALSE)
    /// and whether it is rational. `None` when NurbsCurve.build would return early.
    fn nurbs_interp_curve(&self, node: NodeId) -> Option<(Spline, bool)> {
        let order = self.int(node, "order", 3);
        let points = self.nurbs_control_points(node)?;

        if order < 2 || points.len() < order as usize {
            return None;
        }

        let order = order as usize;
        let weights = m::weights(points.len(), &self.doubles(node, "weight"));
        let control = m::curve_control_points(false, order, weights.as_deref(), &points);
        let knots = m::knots(false, order, points.len(), &self.doubles(node, "knot"));

        Some((
            Spline::curve(order - 1, knots, 3 + weights.is_some() as usize, control),
            weights.is_some(),
        ))
    }

    /// The surface of a NurbsSurfaceInterpolator (its private NurbsPatchSurface, not closed).
    fn nurbs_interp_surface(&self, node: NodeId) -> Option<(Spline, bool)> {
        let (uo, vo) = (self.int(node, "uOrder", 3), self.int(node, "vOrder", 3));
        let (ud, vd) = (
            self.int(node, "uDimension", 0),
            self.int(node, "vDimension", 0),
        );

        if uo < 2 || vo < 2 || ud < uo || vd < vo {
            return None;
        }

        let points = self.nurbs_control_points(node)?;
        let (uo, vo, ud, vd) = (uo as usize, vo as usize, ud as usize, vd as usize);

        if points.len() < ud * vd {
            return None;
        }

        let weights = m::weights(ud * vd, &self.doubles(node, "weight"));
        let (control, size) =
            m::surface_control_points(false, false, uo, vo, ud, vd, weights.as_deref(), |i| {
                points[i]
            });
        let u_knots = m::knots(false, uo, ud, &self.doubles(node, "uKnot"));
        let v_knots = m::knots(false, vo, vd, &self.doubles(node, "vKnot"));

        Some((
            Spline::surface(
                [uo - 1, vo - 1],
                [u_knots, v_knots],
                size,
                3 + weights.is_some() as usize,
                control,
            ),
            weights.is_some(),
        ))
    }

    /// set_fraction__ of the three NURBS interpolators.
    pub(crate) fn nurbs_interp_set_fraction(&mut self, node: NodeId) {
        let Some(name) = self.builtin_name(node) else {
            return;
        };

        match name {
            "NurbsSurfaceInterpolator" => {
                let Some((surface, rational)) = self.nurbs_interp_surface(node) else {
                    return;
                };
                let fraction = match self.pre_value(node, "set_fraction") {
                    Some(FieldValue::SFVec2f(v) | FieldValue::SFVec2d(v)) => *v,
                    _ => return,
                };

                // ISO 27.4.9: the position and normal "for the given u and v coordinates".
                let (u0, u1) = surface.domain(0);
                let (v0, v1) = surface.domain(1);
                let u = fraction.x.max(u0).min(u1);
                let v = fraction.y.max(v0).min(v1);
                let c = surface.components;
                let (mut p, mut du, mut dv) = (vec![0.0; c], vec![0.0; c], vec![0.0; c]);

                surface.evaluate(&[u, v], Some(&[1, 0]), &mut du);
                surface.evaluate(&[u, v], Some(&[0, 1]), &mut dv);
                surface.evaluate(&[u, v], None, &mut p);

                let (position, su, sv) = if rational {
                    rational_derivatives(&p, &du, &dv)
                } else {
                    (v3(&p), v3(&du), v3(&dv))
                };
                let mut normal = su;
                normal.cross(&sv);

                if let Some(k) = self.pre(node, "normal_changed") {
                    self.set_value(k, FieldValue::SFVec3f(normal));
                }

                if let Some(k) = self.pre(node, "position_changed") {
                    self.set_value(k, FieldValue::SFVec3f(position));
                }
            }
            _ => {
                let Some((curve, rational)) = self.nurbs_interp_curve(node) else {
                    return;
                };
                let fraction = self.get_f64(node, "set_fraction");

                // ISO 27.4.7: "the knot corresponds to the key field of a conventional
                // interpolator node": the fraction is the curve parameter.
                let (u0, u1) = curve.domain(0);
                let u = fraction.max(u0).min(u1);
                let c = curve.components;
                let mut p = vec![0.0; c];

                curve.evaluate(&[u], None, &mut p);

                if name == "NurbsPositionInterpolator" {
                    let value = if rational {
                        Vector3::new(p[0] / p[3], p[1] / p[3], p[2] / p[3])
                    } else {
                        v3(&p)
                    };

                    if let Some(k) = self.pre(node, "value_changed") {
                        self.set_value(k, FieldValue::SFVec3f(value));
                    }
                } else {
                    let mut d = vec![0.0; c];
                    curve.evaluate(&[u], Some(&[1]), &mut d);

                    // ISO 27.4.5: the tangent of the (rational) curve.
                    let direction = if rational {
                        rational_derivatives(&p, &d, &d).1
                    } else {
                        v3(&d)
                    };
                    let mut rotation = Rotation4::identity();
                    rotation.set_vectors(&Vector3::Z_AXIS, &direction);

                    if let Some(k) = self.pre(node, "value_changed") {
                        self.set_value(k, FieldValue::SFRotation(rotation));
                    }
                }
            }
        }
    }
}

fn v3(p: &[f64]) -> Vector3 {
    Vector3::new(p[0], p[1], p[2])
}

/// From homogeneous A = (w x, w y, w z, w) and its derivatives: S = A / w and
/// S' = (A' − w' S) / w for both derivatives.
fn rational_derivatives(a: &[f64], du: &[f64], dv: &[f64]) -> (Vector3, Vector3, Vector3) {
    let w = a[3];
    let s = Vector3::new(a[0] / w, a[1] / w, a[2] / w);
    let d = |q: &[f64]| {
        Vector3::new(
            (q[0] - q[3] * s.x) / w,
            (q[1] - q[3] * s.y) / w,
            (q[2] - q[3] * s.z) / w,
        )
    };

    (s, d(du), d(dv))
}
