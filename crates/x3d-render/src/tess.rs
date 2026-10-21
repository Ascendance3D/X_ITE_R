//! Polygon triangulation (standard/Math/Geometry/Triangle3.js).
//!
//! `triangulate_convex` is an exact port (triangle fan). Non-convex polygons (`convex FALSE`) go through
//! Triangle3.triangulatePolygon, the libtess port `x3d_text::libtess::triangulate_indexed` (GLU tessellator, odd winding rule).

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
