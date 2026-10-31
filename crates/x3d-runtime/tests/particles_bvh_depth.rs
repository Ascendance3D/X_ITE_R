//! Issue #93, "BVH depth > 32": the GLSL traversal stack holds `BVH_STACK_SIZE` = 32 nodes, but BVH.js splits every
//! node at the median (`leftSize = size >>> 1`), so a hierarchy of N triangles is `ceil (log2 N)` deep whatever the
//! triangles are. A hierarchy deeper than the stack would need more than 2 ** 31 triangles, so no world can reach it
//! and there is nothing to record from X_ITE. This test pins the depth the Rust builder produces.

use x3d_runtime::particles::build_hierarchy;

/// Triangles `0 .. n` as 12 floats each (three vec4 vertices), spread over a line and a plane so the
/// centroids differ and tie in turns.
fn soup(n: usize) -> Vec<f32> {
    let mut v = Vec::with_capacity(n * 12);

    for t in 0..n {
        let (x, y, z) = ((t % 97) as f32, ((t * 31) % 53) as f32, (t / 97) as f32);

        for k in 0..3 {
            v.extend_from_slice(&[x + k as f32 * 0.25, y, z + (k % 2) as f32 * 0.25, 1.0]);
        }
    }

    v
}

/// The depth of the hierarchy texels `h` (`[0, left, right]` + two box rows per node, `[1, ...]` per triangle, the
/// root pointer last), a triangle being depth 0.
fn depth(h: &[[f32; 4]], at: usize) -> usize {
    if h[at][0] == 1.0 {
        0
    } else {
        1 + depth(h, h[at][1] as usize).max(depth(h, h[at][2] as usize))
    }
}

#[test]
fn bvh_depth_is_log2_of_the_triangle_count() {
    for n in [2usize, 3, 4, 5, 31, 32, 33, 100, 1000, 4097, 70_000] {
        let h = build_hierarchy(&soup(n));
        let root = h.last().expect("root pointer")[0] as usize;
        let expected = n.next_power_of_two().trailing_zeros() as usize;

        assert_eq!(depth(&h, root), expected, "{n} triangles");
        assert!(expected < 32);
    }
}
