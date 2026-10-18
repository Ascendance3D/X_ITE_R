//! REA batch 07: the pieces of the PolylineEmitter / WindPhysicsModel port that do not need a scene
//! (the scene level comparison with X_ITE is crates/x3d-viewer/tests/particles_b07.rs).

use x3d_runtime::particles::{self, polyline_texture};

#[test]
fn polylines_texture_layout_is_x_ites() {
    // Two segments: (0,0,0)-(3,4,0) of length 5 and (3,4,0)-(3,4,12) of length 12.
    let v = [0.0, 0.0, 0.0, 1.0, 3.0, 4.0, 0.0, 1.0, 3.0, 4.0, 0.0, 1.0, 3.0, 4.0, 12.0, 1.0];
    let t = polyline_texture(&v);

    // numVertices 4, numLengthSoFar 3, polylineArraySize ceil (sqrt (3 + 4)) = 3.
    assert_eq!(t.vertices_index, 3);
    assert_eq!(t.array.len(), 9);
    assert_eq!([t.array[0][0], t.array[1][0], t.array[2][0]], [0.0, 5.0, 17.0]);
    assert_eq!(t.array[3], [0.0, 0.0, 0.0, 1.0]);
    assert_eq!(t.array[6], [3.0, 4.0, 12.0, 1.0]);
}

#[test]
fn an_empty_polyline_set_has_no_vertices_and_one_texel() {
    let t = polyline_texture(&[]);

    assert_eq!((t.vertices_index, t.array.len()), (-1, 1));
}

#[test]
fn the_lengths_are_float_rounded_double_sums() {
    // 0.1 + 0.1 + 0.1 summed in doubles then rounded to float, not summed in float.
    let seg = |x0: f64, x1: f64| [x0, 0.0, 0.0, 1.0, x1, 0.0, 0.0, 1.0];
    let v: Vec<f64> = [seg(0.0, 0.1), seg(0.1, 0.2), seg(0.2, 0.30000000000000004)].concat();
    let t = polyline_texture(&v);
    let sum = 0.1f64.hypot(0.0) + (0.2f64 - 0.1).abs() + (0.30000000000000004f64 - 0.2).abs();

    assert_eq!(t.array[3][0], sum as f32);
}

/// The force draw list is consumed in order, a dry list is reported once and falls back to a random source.
#[test]
fn force_draws_replay_in_order() {
    particles::set_force_draws(vec![0.25, 0.5]);
    assert_eq!(particles::force_draws_left(), 2);
    assert_eq!(particles::next_force_random(), 0.25);
    assert_eq!(particles::next_force_random(), 0.5);
    assert!(!particles::force_draws_exhausted());

    let v = particles::next_force_random();

    assert!((0.0..1.0).contains(&v));
    assert!(particles::force_draws_exhausted());
    particles::use_random_force_draws();
}
