//! X3DComposedGeometryNode.build and the composed geometry nodes that are not IndexedFaceSet:
//! Components/Rendering/{TriangleSet,TriangleFanSet,TriangleStripSet,IndexedTriangleSet,
//! IndexedTriangleFanSet,IndexedTriangleStripSet}.js and Components/CADGeometry/{QuadSet,
//! IndexedQuadSet}.js.

use crate::geometry::{Colors, Coords, Geometry, finish, refine_normals};
use x3d_fields::math::Vector3;

/// Which composed node, with the field that defines its polygons.
#[derive(Clone, Copy)]
pub enum Kind<'a> {
    TriangleSet,
    TriangleFanSet { fan_count: &'a [i32] },
    TriangleStripSet { strip_count: &'a [i32] },
    QuadSet,
    IndexedTriangleSet { index: &'a [i32] },
    IndexedTriangleFanSet { index: &'a [i32] },
    IndexedTriangleStripSet { index: &'a [i32] },
    IndexedQuadSet { index: &'a [i32] },
}

#[derive(Clone, Copy)]
pub struct ComposedInput<'a> {
    pub kind: Kind<'a>,
    pub coord: Option<&'a [f32]>,
    pub color: Option<Colors<'a>>,
    pub tex_coord: Option<&'a [f32]>,
    pub normal: Option<&'a [f32]>,
    pub solid: bool,
    pub ccw: bool,
    pub color_per_vertex: bool,
    pub normal_per_vertex: bool,
    pub extras: crate::extras::Extras<'a>,
}

/// TriangleFanSet.set_fanCount__.
fn fan_count_index(fan_count: &[i32]) -> Vec<i64> {
    let mut t = vec![];
    let mut index: i64 = 0;

    for &vertex_count in fan_count {
        let count = vertex_count as i64 - 1;

        for i in 1..count.max(1) {
            t.extend_from_slice(&[index, index + i, index + i + 1]);
        }

        index += vertex_count as i64;
    }

    t
}

/// TriangleStripSet.set_stripCount__.
fn strip_count_index(strip_count: &[i32]) -> Vec<i64> {
    let mut t = vec![];
    let mut index: i64 = 0;

    for &vertex_count in strip_count {
        let count = vertex_count as i64 - 2;

        for i in 0..count.max(0) {
            let odd = i & 1 == 1;
            t.extend_from_slice(&[
                index + if odd { i + 1 } else { i },
                index + if odd { i } else { i + 1 },
                index + (i + 2),
            ]);
        }

        index += vertex_count as i64;
    }

    t
}

/// IndexedTriangleFanSet.set_index__.
fn indexed_fan_index(index: &[i32]) -> Vec<i64> {
    let mut t = vec![];
    let length = index.len();
    let mut i = 0;

    while i < length {
        let first = index[i] as i64;

        if first >= 0 {
            i += 1;

            if i < length {
                let mut second = index[i] as i64;

                if second >= 0 {
                    i += 1;

                    while i < length {
                        let third = index[i] as i64;

                        if third < 0 {
                            break;
                        }

                        t.extend_from_slice(&[first, second, third]);
                        second = third;
                        i += 1;
                    }
                }
            }
        }

        i += 1;
    }

    t
}

/// IndexedTriangleStripSet.set_index__.
fn indexed_strip_index(index: &[i32]) -> Vec<i64> {
    let mut t = vec![];
    let length = index.len();
    let mut i = 0;

    while i < length {
        let mut first = index[i] as i64;

        if first >= 0 {
            i += 1;

            if i < length {
                let mut second = index[i] as i64;

                if second >= 0 {
                    i += 1;
                    let mut face = 0;

                    while i < length {
                        let third = index[i] as i64;

                        if third < 0 {
                            break;
                        }

                        t.extend_from_slice(&[first, second, third]);

                        if face & 1 == 1 {
                            second = third;
                        } else {
                            first = third;
                        }

                        i += 1;
                        face += 1;
                    }
                }
            }
        }

        i += 1;
    }

    t
}

/// QuadSet / IndexedQuadSet getTriangleIndex.
fn quad_triangle_index(i: usize) -> usize {
    const TRIANGLES: [usize; 6] = [0, 1, 2, 0, 2, 3];
    let m = i % 6;
    (i - m) / 6 * 4 + TRIANGLES[m]
}

/// checkVertexCount.
fn check(n: usize, multiplier: usize) -> usize {
    n - n % multiplier
}

/// X3DComposedGeometryNode.getPolygonNormal (Newell's method).
fn polygon_normal(
    index: usize,
    vertices_per_polygon: usize,
    polygons: &[i64],
    coord: &Coords,
) -> Vector3 {
    let mut normal = Vector3::ZERO;
    let mut next = coord.get(polygons[index]);

    for i in 0..vertices_per_polygon {
        let current = next;
        next = coord.get(polygons[index + (i + 1) % vertices_per_polygon]);
        normal.x += (current.y - next.y) * (current.z + next.z);
        normal.y += (current.z - next.z) * (current.x + next.x);
        normal.z += (current.x - next.x) * (current.y + next.y);
    }

    normal.normalize();
    normal
}

/// X3DComposedGeometryNode.createFaceNormals.
fn face_normals(
    vertices_per_polygon: usize,
    polygons: &[i64],
    coord: &Coords,
    ccw: bool,
) -> Vec<Vector3> {
    let mut normals = vec![];
    let mut index = 0;

    while index < polygons.len() {
        let p = |k: usize| polygons.get(index + k).copied().unwrap_or(i64::MAX);
        let mut normal = match vertices_per_polygon {
            3 => coord.normal(p(0), p(1), p(2)),
            4 => coord.quad_normal(p(0), p(1), p(2), p(3)),
            _ => polygon_normal(index, vertices_per_polygon, polygons, coord),
        };

        if !ccw {
            normal.negate();
        }

        for _ in 0..vertices_per_polygon {
            normals.push(normal);
        }

        index += vertices_per_polygon;
    }

    normals
}

/// The build of every composed node except IndexedFaceSet.
pub fn composed_geometry(input: &ComposedInput) -> Geometry {
    let mut g = Geometry {
        geometry_type: 3,
        solid: true,
        ccw: true,
        ..Default::default()
    };
    let coord_len = input.coord.map(|c| c.len() / 3).unwrap_or(0);

    // Polygon index of each polygon vertex (getPolygonIndex), the per-polygon and per-face vertex
    // counts, and the triangle index into the polygon list (getTriangleIndex).
    let (polygons, vertices_per_polygon, vertices_per_face, quads, face_only_normals): (
        Vec<i64>,
        usize,
        usize,
        bool,
        bool,
    ) = match input.kind {
        Kind::TriangleSet => ((0..check(coord_len, 3) as i64).collect(), 3, 3, false, true),
        Kind::QuadSet => ((0..check(coord_len, 4) as i64).collect(), 4, 6, true, true),
        Kind::TriangleFanSet { fan_count } => (fan_count_index(fan_count), 3, 3, false, false),
        Kind::TriangleStripSet { strip_count } => {
            (strip_count_index(strip_count), 3, 3, false, false)
        }
        Kind::IndexedTriangleSet { index } => (
            index.iter().map(|i| *i as i64).collect(),
            3,
            3,
            false,
            false,
        ),
        Kind::IndexedTriangleFanSet { index } => (indexed_fan_index(index), 3, 3, false, false),
        Kind::IndexedTriangleStripSet { index } => (indexed_strip_index(index), 3, 3, false, false),
        Kind::IndexedQuadSet { index } => {
            (index.iter().map(|i| *i as i64).collect(), 4, 6, true, false)
        }
    };

    let Some(points) = input.coord.filter(|_| coord_len > 0) else {
        finish(&mut g, true);
        return g;
    };

    let coord = Coords(points);
    let polygons_size = check(polygons.len(), vertices_per_polygon);
    let triangles_size = if quads {
        check(polygons_size / 4 * 6, vertices_per_face)
    } else {
        check(polygons_size, vertices_per_face)
    };
    let polygons = &polygons[..polygons_size];
    let triangles: Vec<usize> = (0..triangles_size)
        .map(|i| if quads { quad_triangle_index(i) } else { i })
        .collect();
    let extras = &input.extras;

    extras.init(&mut g);

    for (i, &t) in triangles.iter().enumerate() {
        let face = (i / vertices_per_face) as i64;
        // polygons [triangles [i]]: undefined (NaN index) past the end.
        let index = polygons.get(t).copied().unwrap_or(-1);

        // Uint32Array: undefined becomes 0.
        g.coord_indices
            .push(polygons.get(t).map_or(0, |v| *v as u32));
        extras.add_attribs_and_fog(index, &mut g);

        if let Some(c) = &input.color {
            c.add_color(
                if input.color_per_vertex { index } else { face },
                &mut g.colors,
            );
        }

        if extras.tex.is_some() {
            extras.add_tex(index, &mut g);
        } else if let Some(tc) = input.tex_coord {
            crate::geometry::add_tex_point(tc, index, &mut g.tex_coords);
        }

        extras.add_tangent(if input.normal_per_vertex { index } else { face }, &mut g);

        if let Some(n) = input.normal {
            crate::geometry::add_vector(
                n,
                if input.normal_per_vertex { index } else { face },
                &mut g.normals,
            );
        }

        coord.add_point(index, &mut g.vertices);
    }

    if input.normal.is_none() {
        let normals = face_normals(vertices_per_polygon, polygons, &coord, input.ccw);
        let normals = if !face_only_normals && input.normal_per_vertex {
            // createNormals: smooth over every polygon vertex sharing a point index (Math.PI).
            let mut order: Vec<(i64, Vec<usize>)> = vec![];
            let mut map: std::collections::HashMap<i64, usize> = std::collections::HashMap::new();

            for (i, &index) in polygons.iter().enumerate() {
                match map.get(&index) {
                    Some(&k) => order[k].1.push(i),
                    None => {
                        map.insert(index, order.len());
                        order.push((index, vec![i]));
                    }
                }
            }

            refine_normals(&order, &normals, std::f64::consts::PI)
        } else {
            normals
        };

        for &t in &triangles {
            let n = normals
                .get(t)
                .copied()
                .unwrap_or(Vector3::new(f64::NAN, f64::NAN, f64::NAN));
            g.normals
                .extend_from_slice(&[n.x as f32, n.y as f32, n.z as f32]);
        }
    }

    g.solid = input.solid;
    g.ccw = input.ccw;
    let has_tex = if extras.tex.is_some() {
        extras.channels() > 0
    } else {
        input.tex_coord.is_some()
    };
    finish(&mut g, has_tex);
    g
}
