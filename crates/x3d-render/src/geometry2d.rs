//! Geometry2D component and LineSet, ported from X_ITE:
//! Components/Geometry2D/{Arc2D,ArcClose2D,Circle2D,Disk2D,Polyline2D,Polypoint2D,Rectangle2D,
//! TriangleSet2D}.js, Browser/Geometry2D/*Options.js (PrimitiveQuality MEDIUM: dimension 40) and
//! Components/Rendering/LineSet.js.
//!
//! The option nodes keep their arrays in Float32 storage (Coordinate points, X3DGeometryNode
//! arrays); scaled copies are computed in doubles from those Float32 values, as in JS.

use crate::geometry::{Colors, Coords, Geometry, IfsInput, add_vector, finish, indexed_face_set};
use std::f64::consts::PI;
use x3d_fields::jsmath;
use x3d_fields::math::Vector3;

/// Arc2DOptions / ArcClose2DOptions / Circle2DOptions / Disk2DOptions dimension
/// (X3DGeometry2DContext.setPrimitiveQuality2D: LOW 20, MEDIUM 40, HIGH 80).
pub fn dimension_2d() -> usize {
    match crate::traverse::primitive_quality() {
        crate::traverse::Quality::Low => 20,
        crate::traverse::Quality::Medium => 40,
        crate::traverse::Quality::High => 80,
    }
}

fn polar(m: f64, angle: f64) -> (f64, f64) {
    (m * jsmath::cos(angle), m * jsmath::sin(angle))
}

/// Arc2D / ArcClose2D getSweepAngle.
fn sweep_angle(start_angle: f64, end_angle: f64) -> f64 {
    let start = jsmath::interval(start_angle, 0.0, PI * 2.0);
    let end = jsmath::interval(end_angle, 0.0, PI * 2.0);

    if start == end {
        return PI * 2.0;
    }

    let sweep = (end - start).abs();

    if start > end {
        return (PI * 2.0) - sweep;
    }

    if !sweep.is_nan() {
        return sweep;
    }

    0.0
}

fn line_geometry() -> Geometry {
    Geometry {
        geometry_type: 1,
        solid: false,
        ccw: true,
        ..Default::default()
    }
}

fn set_extents(g: &mut Geometry, min: Vector3, max: Vector3) {
    g.min = min;
    g.max = max;
}

/// Arc2D.build.
pub fn arc2d(radius: f64, start_angle: f64, end_angle: f64) -> Geometry {
    let mut g = line_geometry();
    let radius = radius.abs();
    let sweep = sweep_angle(start_angle, end_angle);
    let steps = jsmath::js_max(3.0, (sweep * dimension_2d() as f64 / (PI * 2.0)).floor());
    let steps = if steps.is_nan() { 0 } else { steps as usize };

    for n in 0..steps {
        let t1 = n as f64 / steps as f64;
        let (x1, y1) = polar(radius, start_angle + (sweep * t1));
        let t2 = (n + 1) as f64 / steps as f64;
        let (x2, y2) = polar(radius, start_angle + (sweep * t2));
        g.vertices.extend_from_slice(&[
            x1 as f32, y1 as f32, 0.0, 1.0, x2 as f32, y2 as f32, 0.0, 1.0,
        ]);
    }

    finish(&mut g, true);
    set_extents(
        &mut g,
        Vector3::new(-radius, -radius, 0.0),
        Vector3::new(radius, radius, 0.0),
    );
    g
}

/// ArcClose2D.build.
pub fn arc_close2d(
    radius: f64,
    start_angle: f64,
    end_angle: f64,
    chord: bool,
    solid: bool,
) -> Geometry {
    let mut g = Geometry {
        geometry_type: 2,
        solid,
        ccw: true,
        ..Default::default()
    };
    let radius = radius.abs();
    let sweep = sweep_angle(start_angle, end_angle);
    let steps = jsmath::js_max(4.0, (sweep * dimension_2d() as f64 / (PI * 2.0)).floor());
    let steps = if steps.is_nan() { 0 } else { steps as usize };
    let steps_1 = steps.saturating_sub(1);
    let mut tex = vec![];
    let mut points = vec![];

    for n in 0..steps {
        let t = n as f64 / steps_1 as f64;
        let theta = start_angle + (sweep * t);
        let (tx, ty) = polar(0.5, theta);
        tex.push((tx + 0.5, ty + 0.5));
        points.push(polar(radius, theta));
    }

    let push = |t: [(f64, f64); 3], p: [(f64, f64); 3], g: &mut Geometry| {
        for k in 0..3 {
            g.tex_coords
                .extend_from_slice(&[t[k].0 as f32, t[k].1 as f32, 0.0, 1.0]);
            g.normals.extend_from_slice(&[0.0, 0.0, 1.0]);
        }
        for k in 0..3 {
            g.vertices
                .extend_from_slice(&[p[k].0 as f32, p[k].1 as f32, 0.0, 1.0]);
        }
    };

    if chord {
        for i in 1..steps_1 {
            push(
                [tex[0], tex[i], tex[i + 1]],
                [points[0], points[i], points[i + 1]],
                &mut g,
            );
        }
    } else {
        for i in 0..steps_1 {
            push(
                [(0.5, 0.5), tex[i], tex[i + 1]],
                [(0.0, 0.0), points[i], points[i + 1]],
                &mut g,
            );
        }
    }

    finish(&mut g, true);
    set_extents(
        &mut g,
        Vector3::new(-radius, -radius, 0.0),
        Vector3::new(radius, radius, 0.0),
    );
    g
}

/// Circle2DOptions geometry: an IndexedLineSet over Float32 Coordinate points.
fn circle_vertices() -> Vec<f32> {
    let angle = PI * 2.0 / dimension_2d() as f64;
    let points: Vec<f32> = (0..dimension_2d())
        .flat_map(|n| {
            let (x, y) = polar(1.0, angle * n as f64);
            [x as f32, y as f32, 0.0]
        })
        .collect();
    let mut coord_index: Vec<i32> = (0..dimension_2d() as i32).collect();
    coord_index.extend_from_slice(&[0, -1]);
    crate::geometry::indexed_line_set(
        Some(&points),
        &coord_index,
        None,
        &[],
        true,
        None,
        &Default::default(),
    )
    .vertices
}

fn scaled_circle(radius: f64) -> Vec<f32> {
    let v = circle_vertices();

    if radius == 1.0 {
        return v;
    }

    v.chunks_exact(4)
        .flat_map(|p| {
            [
                (p[0] as f64 * radius) as f32,
                (p[1] as f64 * radius) as f32,
                0.0,
                1.0,
            ]
        })
        .collect()
}

/// Circle2D.build.
pub fn circle2d(radius: f64) -> Geometry {
    let mut g = line_geometry();
    g.vertices = scaled_circle(radius);
    finish(&mut g, true);
    set_extents(
        &mut g,
        Vector3::new(-radius, -radius, 0.0),
        Vector3::new(radius, radius, 0.0),
    );
    g
}

/// Disk2DOptions arrays (Float32 storage): texCoords, normals, vertices.
fn disk_options() -> (Vec<f32>, Vec<f32>, Vec<f32>) {
    let angle = PI * 2.0 / dimension_2d() as f64;
    let (mut t, mut n, mut v) = (vec![], vec![], vec![]);

    for i in 0..dimension_2d() {
        let theta1 = angle * i as f64;
        let theta2 = angle * (i + 1) as f64;
        let (t1x, t1y) = polar(0.5, theta1);
        let (t2x, t2y) = polar(0.5, theta2);
        let (p1x, p1y) = polar(1.0, theta1);
        let (p2x, p2y) = polar(1.0, theta2);

        t.extend_from_slice(&[
            0.5,
            0.5,
            0.0,
            1.0,
            (t1x + 0.5) as f32,
            (t1y + 0.5) as f32,
            0.0,
            1.0,
            (t2x + 0.5) as f32,
            (t2y + 0.5) as f32,
            0.0,
            1.0,
        ]);
        n.extend_from_slice(&[0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0]);
        v.extend_from_slice(&[
            0.0, 0.0, 0.0, 1.0, p1x as f32, p1y as f32, 0.0, 1.0, p2x as f32, p2y as f32, 0.0, 1.0,
        ]);
    }

    (t, n, v)
}

/// Disk2D.build: a point, a circle, a disk or a disk with a hole.
pub fn disk2d(inner_radius: f64, outer_radius: f64, solid: bool) -> Geometry {
    let inner = jsmath::js_min(inner_radius.abs(), outer_radius.abs());
    let outer = jsmath::js_max(inner_radius.abs(), outer_radius.abs());

    if inner == outer {
        if outer == 0.0 {
            let mut g = Geometry {
                geometry_type: 0,
                solid: false,
                ccw: true,
                ..Default::default()
            };
            g.vertices.extend_from_slice(&[0.0, 0.0, 0.0, 1.0]);
            finish(&mut g, true);
            set_extents(&mut g, Vector3::ZERO, Vector3::ZERO);
            return g;
        }

        let mut g = circle2d(outer);
        set_extents(
            &mut g,
            Vector3::new(-outer, -outer, 0.0),
            Vector3::new(outer, outer, 0.0),
        );
        return g;
    }

    let (dt, dn, dv) = disk_options();
    let mut g = Geometry {
        geometry_type: 2,
        solid,
        ccw: true,
        ..Default::default()
    };

    if inner == 0.0 {
        g.tex_coords = dt;
        g.normals = dn;
        g.vertices = if outer == 1.0 {
            dv
        } else {
            dv.chunks_exact(4)
                .flat_map(|p| {
                    [
                        (p[0] as f64 * outer) as f32,
                        (p[1] as f64 * outer) as f32,
                        0.0,
                        1.0,
                    ]
                })
                .collect()
        };
    } else {
        let scale = inner / outer;
        let offset = (1.0 - scale) / 2.0;
        let f = |x: f32| x as f64;

        for i in (0..dv.len()).step_by(12) {
            let s = |k: usize| (f(dt[i + k]) * scale + offset) as f32;
            g.tex_coords.extend_from_slice(&[
                s(4),
                s(5),
                0.0,
                1.0,
                dt[i + 4],
                dt[i + 5],
                0.0,
                1.0,
                dt[i + 8],
                dt[i + 9],
                0.0,
                1.0,
                s(4),
                s(5),
                0.0,
                1.0,
                dt[i + 8],
                dt[i + 9],
                0.0,
                1.0,
                s(8),
                s(9),
                0.0,
                1.0,
            ]);
            g.normals.extend_from_slice(&[
                0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0,
                0.0, 1.0,
            ]);
            let m = |k: usize, r: f64| (f(dv[i + k]) * r) as f32;
            g.vertices.extend_from_slice(&[
                m(4, inner),
                m(5, inner),
                0.0,
                1.0,
                m(4, outer),
                m(5, outer),
                0.0,
                1.0,
                m(8, outer),
                m(9, outer),
                0.0,
                1.0,
                m(4, inner),
                m(5, inner),
                0.0,
                1.0,
                m(8, outer),
                m(9, outer),
                0.0,
                1.0,
                m(8, inner),
                m(9, inner),
                0.0,
                1.0,
            ]);
        }
    }

    finish(&mut g, true);
    set_extents(
        &mut g,
        Vector3::new(-outer, -outer, 0.0),
        Vector3::new(outer, outer, 0.0),
    );
    g
}

/// Polyline2D.build.
pub fn polyline2d(line_segments: &[f32]) -> Geometry {
    let mut g = line_geometry();
    let n = line_segments.len() / 2;

    for i in (0..n.saturating_sub(1) * 2).step_by(2) {
        g.vertices.extend_from_slice(&[
            line_segments[i],
            line_segments[i + 1],
            0.0,
            1.0,
            line_segments[i + 2],
            line_segments[i + 3],
            0.0,
            1.0,
        ]);
    }

    finish(&mut g, true);
    g
}

/// Polypoint2D.build.
pub fn polypoint2d(point: &[f32]) -> Geometry {
    let mut g = Geometry {
        geometry_type: 0,
        solid: false,
        ccw: true,
        ..Default::default()
    };

    for p in point.chunks_exact(2) {
        g.vertices.extend_from_slice(&[p[0], p[1], 0.0, 1.0]);
    }

    finish(&mut g, true);
    g
}

/// Rectangle2D.build over the Rectangle2DOptions IndexedFaceSet.
pub fn rectangle2d(size: (f64, f64), solid: bool) -> Geometry {
    let tex = [1.0f32, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0];
    let coord = [
        1.0f32, 1.0, 0.0, -1.0, 1.0, 0.0, -1.0, -1.0, 0.0, 1.0, -1.0, 0.0,
    ];
    let base = indexed_face_set(&IfsInput {
        coord: Some(&coord),
        coord_index: &[0, 1, 2, 3, -1],
        tex_coord: Some(&tex),
        tex_coord_index: &[],
        normal: None,
        normal_index: &[],
        color: None,
        color_index: &[],
        ccw: true,
        solid: true,
        convex: true,
        crease_angle: 0.0,
        color_per_vertex: true,
        normal_per_vertex: true,
        extras: Default::default(),
    });

    let mut g = Geometry {
        geometry_type: 2,
        solid,
        ccw: true,
        tex_coords: base.tex_coords.clone(),
        normals: base.normals.clone(),
        tangents: base.tangents.clone(),
        ..Default::default()
    };

    if size == (2.0, 2.0) {
        g.vertices = base.vertices.clone();
        g.min = base.min;
        g.max = base.max;
    } else {
        let x = (size.0 / 2.0).abs();
        let y = (size.1 / 2.0).abs();
        g.vertices = base
            .vertices
            .chunks_exact(4)
            .flat_map(|p| [(x * p[0] as f64) as f32, (y * p[1] as f64) as f32, 0.0, 1.0])
            .collect();
        g.min = Vector3::new(-x, -y, 0.0);
        g.max = Vector3::new(x, y, 0.0);
    }

    g
}

/// TriangleSet2D.build with its own generateTexCoords (s from x, t from y).
pub fn triangle_set2d(vertices: &[f32], solid: bool) -> Geometry {
    let mut g = Geometry {
        geometry_type: 2,
        solid,
        ccw: true,
        ..Default::default()
    };

    for p in vertices.chunks_exact(2) {
        g.normals.extend_from_slice(&[0.0, 0.0, 1.0]);
        g.vertices.extend_from_slice(&[p[0], p[1], 0.0, 1.0]);
    }

    finish(&mut g, true);

    // getTexCoordParams: min is the bbox min, Ssize the largest bbox dimension.
    let size = g.bbox_size();
    let ssize = if size.x >= size.y && size.x >= size.z {
        size.x
    } else if size.y >= size.x && size.y >= size.z {
        size.y
    } else {
        size.z
    };
    let mut min = g.bbox_center();
    let mut half = size;
    half.divide(2.0);
    min.subtract(&half);

    for v in g.vertices.chunks_exact(4) {
        g.tex_coords.extend_from_slice(&[
            ((v[0] as f64 - min.x) / ssize) as f32,
            ((v[1] as f64 - min.y) / ssize) as f32,
            0.0,
            1.0,
        ]);
    }

    // rebuild: generateTexCoords (above), then generateTangents.
    crate::geometry::generate_tangents(&mut g);
    g
}

/// LineSet.build (color per vertex only; normals when a Normal node is given).
pub fn line_set(
    vertex_count: &[i32],
    coord: Option<&[f32]>,
    color: Option<Colors>,
    normal: Option<&[f32]>,
    extras: &crate::extras::Extras,
) -> Geometry {
    let mut g = line_geometry();

    if let Some(points) = coord.filter(|c| c.len() >= 3) {
        let coord = Coords(points);
        let size = coord.len() as i64;
        let mut index: i64 = 0;

        // X3DLineGeometryNode: no texture coordinates.
        crate::extras::Extras {
            tex: None,
            ..*extras
        }
        .init(&mut g);

        for &count in vertex_count {
            let count = count as i64;

            if index + count > size {
                break;
            }

            if count > 1 {
                let n = 2 * count - 2;
                let mut i = 0;

                while i < n {
                    g.coord_indices.push(index as u32);
                    extras.add_attribs_and_fog(index, &mut g);
                    extras.add_tangent(index, &mut g);

                    if let Some(c) = &color {
                        c.add_color(index, &mut g.colors);
                    }

                    if let Some(nv) = normal {
                        add_vector(nv, index, &mut g.normals);
                    }

                    coord.add_point(index, &mut g.vertices);
                    i += 1;
                    index += i & 1;
                }

                index += 1;
            } else {
                index += count;
            }
        }
    }

    finish(&mut g, true);
    g
}
