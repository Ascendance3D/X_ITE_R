//! Polygon triangulation (standard/Math/Geometry/Triangle3.js).
//!
//! `triangulate_convex` is an exact port (triangle fan). X_ITE triangulates non-convex polygons
//! (`convex FALSE`) with libtess (GLU tessellator, odd winding rule). That library is not
//! ported; `triangulate_polygon` is an ear-clipping substitute that covers the same area for
//! simple polygons but may choose different diagonals (visible only on non-planar polygons or
//! through texture/normal interpolation along the diagonals). Self-intersecting polygons are
//! not split like libtess does.

use x3d_fields::math::Vector3;

/// Triangle3.triangulateConvexPolygon: fan around the first vertex.
pub fn triangulate_convex<T: Copy>(vertices: &[T]) -> Vec<T> {
    let mut triangles = vec![];

    for i in 1..vertices.len().saturating_sub(1) {
        triangles.push(vertices[0]);
        triangles.push(vertices[i]);
        triangles.push(vertices[i + 1]);
    }

    triangles
}

/// Ear clipping in the polygon's dominant plane. Returns indices into `points`, three per
/// triangle, with the polygon's own orientation.
pub fn triangulate_polygon(points: &[Vector3]) -> Vec<usize> {
    let n = points.len();

    if n < 3 {
        return vec![];
    }

    // Newell normal → drop the dominant axis.
    let mut normal = Vector3::ZERO;
    for i in 0..n {
        let (c, d) = (points[i], points[(i + 1) % n]);
        normal.x += (c.y - d.y) * (c.z + d.z);
        normal.y += (c.z - d.z) * (c.x + d.x);
        normal.z += (c.x - d.x) * (c.y + d.y);
    }

    let (ax, ay, az) = (normal.x.abs(), normal.y.abs(), normal.z.abs());
    let p2: Vec<(f64, f64)> = points
        .iter()
        .map(|p| if az >= ax && az >= ay { (p.x, p.y) } else if ax >= ay { (p.y, p.z) } else { (p.z, p.x) })
        .collect();

    // Signed area in the projection; ears must have the same orientation.
    let mut area = 0.0;
    for i in 0..n {
        let (a, b) = (p2[i], p2[(i + 1) % n]);
        area += a.0 * b.1 - b.0 * a.1;
    }
    let sign = if area >= 0.0 { 1.0 } else { -1.0 };

    let cross = |a: (f64, f64), b: (f64, f64), c: (f64, f64)| (b.0 - a.0) * (c.1 - a.1) - (b.1 - a.1) * (c.0 - a.0);
    let inside = |p: (f64, f64), a: (f64, f64), b: (f64, f64), c: (f64, f64)| {
        let d1 = cross(a, b, p) * sign;
        let d2 = cross(b, c, p) * sign;
        let d3 = cross(c, a, p) * sign;
        d1 >= 0.0 && d2 >= 0.0 && d3 >= 0.0
    };

    let mut remaining: Vec<usize> = (0..n).collect();
    let mut triangles = vec![];
    let mut guard = 0;

    while remaining.len() > 3 && guard < n * n {
        guard += 1;
        let m = remaining.len();
        let mut clipped = false;

        for i in 0..m {
            let (ia, ib, ic) = (remaining[(i + m - 1) % m], remaining[i], remaining[(i + 1) % m]);
            let (a, b, c) = (p2[ia], p2[ib], p2[ic]);

            if cross(a, b, c) * sign <= 0.0 {
                continue; // reflex or degenerate
            }

            let blocked = remaining.iter().any(|&j| j != ia && j != ib && j != ic && p2[j] != a && p2[j] != b && p2[j] != c && inside(p2[j], a, b, c));

            if blocked {
                continue;
            }

            triangles.extend_from_slice(&[ia, ib, ic]);
            remaining.remove(i);
            clipped = true;
            break;
        }

        if !clipped {
            // Degenerate input: fall back to a fan of what is left.
            break;
        }
    }

    triangles.extend(triangulate_convex(&remaining));
    triangles
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn concave_l_shape() {
        let p = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (1.0, 1.0), (1.0, 2.0), (0.0, 2.0)].map(|(x, y)| Vector3::new(x, y, 0.0));
        let t = triangulate_polygon(&p);
        assert_eq!(t.len(), 12);
        // total area 3
        let area: f64 = t
            .chunks(3)
            .map(|c| {
                let (a, b, c) = (p[c[0]], p[c[1]], p[c[2]]);
                ((b.x - a.x) * (c.y - a.y) - (b.y - a.y) * (c.x - a.x)) / 2.0
            })
            .sum();
        assert!((area - 3.0).abs() < 1e-12);
    }
}
