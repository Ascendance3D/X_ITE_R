//! The instanced draw of the pointing buffer (lane XITE_R_B25_2_GPU_INSTANCED_PICKING): a ParticleSystem goes through
//! the pass as one draw with its particles' x3d_InstanceMatrix, particle k carrying x3d_Id `id + k`
//! ([`PointingTriangle::instances`], ParticleSystem.displaySimple).
//!
//! Every case draws the same particles twice — once as one instanced draw, once as one draw per particle, which is how
//! the pointer sent them before — and requires the same x3d_Id and the same hit values. The per-particle form is the
//! one `crates/x3d-viewer/tests/rea_b14_particle_pick.rs` holds against pinned X_ITE 16.5.0, so an instanced draw that
//! matches it matches X_ITE.
//!
//! The pass needs a GPU; without an adapter the tests are skipped.

use std::rc::Rc;
use x3d_fields::math::Matrix4;
use x3d_render::pointing_gpu::{
    IDENTITY, IDENTITY3, PointingGpu, PointingPrimitive, PointingSample, PointingTriangle,
};

const SIZE: f64 = 64.0;
/// The pointer, at the center pixel's center.
const PX: f64 = 32.5;
const PY: f64 = 32.5;

fn translation(z: f64) -> Matrix4 {
    let mut m = Matrix4::IDENTITY;
    m.0[14] = z;
    m
}

/// A unit quad in the XY plane, two triangles, counterclockwise.
fn quad() -> Vec<[f32; 4]> {
    let c = |x: f32, y: f32| [x, y, 0.0, 1.0];
    vec![
        c(-0.5, -0.5),
        c(0.5, -0.5),
        c(0.5, 0.5),
        c(-0.5, -0.5),
        c(0.5, 0.5),
        c(-0.5, 0.5),
    ]
}

/// A draw of `vertices` with `id`, the viewport shifted so the 1x1 buffer covers the pointer's pixel
/// (X3DRenderObject.pointing), like `crate::pointer::pointing_draw_base`.
fn draw(vertices: Vec<[f32; 4]>, id: f32) -> PointingTriangle {
    let f32x16 = |m: &Matrix4| m.0.map(|v| v as f32);
    let n = vertices.len();

    PointingTriangle {
        primitive: PointingPrimitive::Triangles,
        normals: vec![[0.0, 0.0, 1.0]; n],
        tex_coords: vec![[0.0, 0.0, 0.0, 1.0]; n],
        offsets: vec![],
        vertices,
        projection: f32x16(&x3d_render::traverse::perspective(
            0.785_398, 0.1, 100.0, SIZE, SIZE,
        )),
        model_view: IDENTITY,
        instance: IDENTITY,
        instance_normal: IDENTITY3,
        viewport: [-(PX as i32), -(PY as i32), SIZE as i32, SIZE as i32],
        clip_planes: vec![],
        point_style: None,
        constant_tex_coord: false,
        id,
        particle: true,
        instances: None,
        per_instance_vertices: 0,
    }
}

/// The particles of one system as one instanced draw: ids `1 ..= matrices.len()`.
fn run(matrices: &Rc<Vec<Matrix4>>, clip_planes: Vec<[f32; 4]>) -> Vec<PointingTriangle> {
    vec![PointingTriangle {
        instances: Some(matrices.clone()),
        clip_planes,
        ..draw(quad(), 1.0)
    }]
}

/// The same particles as one draw each, the form the pointer used before B25-2 and the one held against X_ITE.
fn alone(matrices: &[Matrix4], clip_planes: Vec<[f32; 4]>) -> Vec<PointingTriangle> {
    let f32x16 = |m: &Matrix4| m.0.map(|v| v as f32);

    matrices
        .iter()
        .enumerate()
        .map(|(k, m)| PointingTriangle {
            instance: f32x16(m),
            clip_planes: clip_planes.clone(),
            ..draw(quad(), k as f32 + 1.0)
        })
        .collect()
}

/// Every particle with its own six vertices (`per_instance_vertices`), the form the transformed thick lines take.
fn own_vertices(matrices: &Rc<Vec<Matrix4>>) -> Vec<PointingTriangle> {
    let vertices: Vec<[f32; 4]> = matrices.iter().flat_map(|_| quad()).collect();
    let n = vertices.len();

    vec![PointingTriangle {
        normals: vec![[0.0, 0.0, 1.0]; n],
        tex_coords: vec![[0.0, 0.0, 0.0, 1.0]; n],
        instances: Some(matrices.clone()),
        per_instance_vertices: 6,
        ..draw(vertices, 1.0)
    }]
}

/// The sample as comparable bits (float32 values, so `==` is exactness).
fn bits(s: &Option<PointingSample>) -> Option<(u32, [u32; 3], [u32; 3], [u32; 4])> {
    s.as_ref().map(|s| {
        (
            s.id.to_bits(),
            s.point.map(f32::to_bits),
            s.normal.map(f32::to_bits),
            s.tex_coord.map(f32::to_bits),
        )
    })
}

fn gpu() -> Option<PointingGpu> {
    match PointingGpu::new_headless() {
        Ok(g) => Some(g),
        Err(e) => {
            eprintln!("no pointing GPU ({e})");
            None
        }
    }
}

/// An instanced draw and the same particles drawn one by one give the same x3d_Id and the same hit values, at equal
/// depth and at mixed depths, with and without clip planes, and with each particle's own vertices.
#[test]
fn a_run_matches_its_particles_drawn_one_by_one() {
    let Some(mut g) = gpu() else { return };
    // A plane that keeps only what is further away than z = -1.8 (dot (vertex, (0, 0, -1)) >= 1.8).
    let near_cut = vec![[0.0, 0.0, -1.0, 1.8]];

    for (name, zs, planes) in [
        // Four coincident particles: every covered pixel is a depth tie, and LEQUAL keeps the last drawn.
        ("equal depth", vec![-2.0, -2.0, -2.0, -2.0], vec![]),
        // The nearest wins wherever it covers the pixel, whatever its place in the buffer.
        ("nearest in the middle", vec![-2.0, -1.5, -2.5], vec![]),
        ("nearest last", vec![-2.5, -2.0, -1.5], vec![]),
        ("nearest first", vec![-1.5, -2.0, -2.5], vec![]),
        // A tie between the two nearest of a mixed run.
        (
            "tie among the nearest",
            vec![-2.0, -1.5, -2.5, -1.5],
            vec![],
        ),
        // The clip plane discards the nearest two, so the tie moves back.
        ("clipped", vec![-2.0, -1.5, -2.0, -1.5], near_cut.clone()),
        ("all clipped", vec![-1.5, -1.5], near_cut.clone()),
        ("one particle", vec![-2.0], vec![]),
    ] {
        let matrices = Rc::new(zs.iter().map(|&z| translation(z)).collect::<Vec<_>>());
        let instanced = g.sample_all(&run(&matrices, planes.clone())).unwrap();
        let separate = g.sample_all(&alone(&matrices, planes.clone())).unwrap();
        let per_vertex = g.sample_all(&own_vertices(&matrices)).unwrap();

        assert_eq!(
            bits(&instanced),
            bits(&separate),
            "{name}: the instanced draw differs from the particles drawn one by one"
        );

        if planes.is_empty() {
            assert_eq!(
                bits(&per_vertex),
                bits(&separate),
                "{name}: each particle's own vertices differ from the particles drawn one by one"
            );
        }
    }
}

/// Coincident particles: the hit is the last one of the buffer, the primitive the LEQUAL depth test drew last
/// (X3DRenderObject.pointing draws the pointing shapes in traversal order).
#[test]
fn coincident_particles_keep_the_one_drawn_last() {
    let Some(mut g) = gpu() else { return };

    for count in [1usize, 2, 3, 8, 64] {
        let matrices = Rc::new(vec![translation(-2.0); count]);
        let instanced = g.sample_all(&run(&matrices, vec![])).unwrap();
        let separate = g.sample_all(&alone(&matrices, vec![])).unwrap();

        assert_eq!(
            instanced.map(|s| s.id),
            Some(count as f32),
            "{count} coincident particles: the instanced draw did not keep the last"
        );
        assert_eq!(separate.map(|s| s.id), Some(count as f32));
    }
}

/// The instance buffer is written again when the matrices are not the last pass's, and left alone when they are: a
/// pass that repeats the same `Rc` reads the same particles, and a new `Rc` with new values is uploaded.
#[test]
fn the_instance_cache_follows_the_matrices() {
    let Some(mut g) = gpu() else { return };
    let near = Rc::new(vec![translation(-1.5), translation(-2.5)]);
    let far = Rc::new(vec![translation(-2.5), translation(-3.5)]);

    let a = g.sample_all(&run(&near, vec![])).unwrap().unwrap();
    // The same Rc twice: the cache holds and the hit does not move.
    let b = g.sample_all(&run(&near, vec![])).unwrap().unwrap();

    assert_eq!(bits(&Some(a)), bits(&Some(b)));
    assert_eq!(g.instance_sources(), 1);

    // A different Rc with different values: the buffer is written again.
    let c = g.sample_all(&run(&far, vec![])).unwrap().unwrap();

    assert_eq!(c.id, 1.0);
    assert!(
        c.point[2] < a.point[2],
        "the far matrices were not uploaded: {} vs {}",
        c.point[2],
        a.point[2]
    );

    // And back, so a cache that only ever grew would fail here too.
    let d = g.sample_all(&run(&near, vec![])).unwrap().unwrap();

    assert_eq!(bits(&Some(d)), bits(&Some(a)));

    // `forget_instances` makes the next pass upload again without changing what it sees.
    g.forget_instances();
    assert_eq!(g.instance_sources(), 0);

    let e = g.sample_all(&run(&near, vec![])).unwrap().unwrap();

    assert_eq!(bits(&Some(e)), bits(&Some(a)));
}

/// The matrices behind a held address cannot change, so reading the cache by `Rc` identity cannot serve stale data.
///
/// The buffer keeps a clone of every source it uploaded, so the `Rc` is never unique while it is cached:
/// `Rc::get_mut` is `None`, and `Rc::make_mut` has to copy to a *new* allocation, which the identity test then sees
/// as a different source and uploads. `Rc<Vec<Matrix4>>` has no interior mutability, so there is no third way in.
#[test]
fn a_cached_source_cannot_change_behind_its_address() {
    let Some(mut g) = gpu() else { return };
    let mut matrices = Rc::new(vec![translation(-1.5), translation(-2.5)]);
    let before = Rc::as_ptr(&matrices);
    let a = g.sample_all(&run(&matrices, vec![])).unwrap().unwrap();

    // The buffer holds a clone: the only two ways to a `&mut Vec<Matrix4>` are shut.
    assert!(Rc::strong_count(&matrices) >= 2);
    assert!(
        Rc::get_mut(&mut matrices).is_none(),
        "the cached source was still unique"
    );

    // `make_mut` therefore copies, and the copy has its own address.
    Rc::make_mut(&mut matrices)[0] = translation(-9.0);
    assert_ne!(
        Rc::as_ptr(&matrices),
        before,
        "make_mut changed the held allocation"
    );

    // The new address is a new source: the pass uploads it and the hit moves with it.
    let b = g.sample_all(&run(&matrices, vec![])).unwrap().unwrap();

    assert!(
        b.point[2] < a.point[2],
        "the changed matrices were not uploaded: {} vs {}",
        b.point[2],
        a.point[2]
    );
}

/// One draw per ParticleSystem, whatever the particle count: the pass does not grow with the particles.
#[test]
fn a_system_is_one_draw_however_many_particles() {
    let Some(mut g) = gpu() else { return };

    for count in [1usize, 100, 1_000, 10_000] {
        let matrices = Rc::new(vec![translation(-2.0); count]);

        g.sample_all(&run(&matrices, vec![])).unwrap();
        assert_eq!(g.draws, 1, "{count} particles took more than one draw");

        // The per-particle form, for the contrast the instanced draw replaces.
        if count <= 1_000 {
            g.sample_all(&alone(&matrices, vec![])).unwrap();
            assert_eq!(g.draws, count);
        }
    }
}

/// A run with no particles draws nothing and is a miss, not a stale hit from the last pass.
#[test]
fn an_empty_run_draws_nothing() {
    let Some(mut g) = gpu() else { return };
    let some = Rc::new(vec![translation(-2.0)]);
    let none: Rc<Vec<Matrix4>> = Rc::new(vec![]);

    assert!(g.sample_all(&run(&some, vec![])).unwrap().is_some());
    assert!(g.sample_all(&run(&none, vec![])).unwrap().is_none());
    assert!(g.sample_all(&own_vertices(&none)).unwrap().is_none());
}
