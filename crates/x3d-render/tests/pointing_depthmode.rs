//! DepthMode in the pointing pass (issue #79): X3DRenderObject.pointing calls `depthModeNode .enable (gl)` around each
//! shape's draw, so the appearance's depthTest, depthFunction, depthMask, depthRange and polygonOffset decide which
//! shape's x3d_Id the 1x1 pointing buffer ends with. Every case draws two quads under the pointer in a fixed order and
//! checks the winner against what GL gives for those states (DepthMode.enable, pinned X_ITE 16.5.0).
//!
//! The pass needs a GPU; without an adapter the tests are skipped.

use x3d_fields::math::Matrix4;
use x3d_render::pointing_gpu::{
    IDENTITY, IDENTITY3, PointingGpu, PointingPrimitive, PointingTriangle,
};
use x3d_render::traverse::{DepthFunc, DepthParams};

const SIZE: f64 = 64.0;
const PX: f64 = 32.5;
const PY: f64 = 32.5;

fn gpu() -> Option<PointingGpu> {
    match PointingGpu::new_headless() {
        Ok(g) => Some(g),
        Err(e) => {
            x3d_evidence::no_gpu(format_args!("no pointing GPU ({e})"));
            None
        }
    }
}

/// A quad in the XY plane at depth `z` (view space), covering the pointer's pixel.
fn quad(id: f32, z: f64, depth: Option<DepthParams>) -> PointingTriangle {
    let c = |x: f32, y: f32| [x, y, 0.0, 1.0];
    let vertices = vec![
        c(-0.5, -0.5),
        c(0.5, -0.5),
        c(0.5, 0.5),
        c(-0.5, -0.5),
        c(0.5, 0.5),
        c(-0.5, 0.5),
    ];
    let n = vertices.len();
    let mut model_view = Matrix4::IDENTITY;

    model_view.0[14] = z;

    PointingTriangle {
        primitive: PointingPrimitive::Triangles,
        normals: vec![[0.0, 0.0, 1.0]; n],
        tex_coords: vec![[0.0, 0.0, 0.0, 1.0]; n],
        offsets: vec![],
        vertices,
        projection: x3d_render::traverse::perspective(
            std::f64::consts::FRAC_PI_4,
            0.1,
            100.0,
            SIZE,
            SIZE,
        )
        .0
        .map(|v| v as f32),
        model_view: model_view.0.map(|v| v as f32),
        instance: IDENTITY,
        instance_normal: IDENTITY3,
        viewport: [-(PX as i32), -(PY as i32), SIZE as i32, SIZE as i32],
        clip_planes: vec![],
        point_style: None,
        constant_tex_coord: false,
        id,
        particle: false,
        instances: None,
        per_instance_vertices: 0,
        depth,
    }
}

/// DepthMode defaults (depthTest TRUE, LEQUAL, depthMask TRUE, depthRange 0 1, polygonOffset 0 0) with `f` applied.
fn mode(f: impl FnOnce(&mut DepthParams)) -> Option<DepthParams> {
    let mut d = DepthParams {
        test: true,
        func: DepthFunc::LessEqual,
        mask: true,
        polygon_offset: [0.0, 0.0],
        range: [0.0, 1.0],
    };

    f(&mut d);
    Some(d)
}

fn winner(g: &mut PointingGpu, draws: &[PointingTriangle]) -> f32 {
    g.sample_all(draws).expect("readback").map_or(0.0, |s| s.id)
}

/// Without a DepthMode, and with the default one, the nearest quad wins whatever the drawing order.
#[test]
fn defaults_pick_the_nearest() {
    let Some(mut g) = gpu() else { return };

    for depth in [None, mode(|_| {})] {
        assert_eq!(
            winner(&mut g, &[quad(1.0, -2.0, depth), quad(2.0, -4.0, depth)]),
            1.0
        );
        assert_eq!(
            winner(&mut g, &[quad(1.0, -4.0, depth), quad(2.0, -2.0, depth)]),
            2.0
        );
    }
}

/// depthTest FALSE: the shape always passes, so the later draw wins even behind a nearer shape.
#[test]
fn depth_test_false_draws_over_nearer_shapes() {
    let Some(mut g) = gpu() else { return };
    let off = mode(|d| d.test = false);

    assert_eq!(
        winner(&mut g, &[quad(1.0, -2.0, None), quad(2.0, -4.0, off)]),
        2.0
    );
    // The default draw behind it fails the test only if it is drawn after a nearer one; here the DepthMode shape comes
    // first and writes no depth (gl.disable (DEPTH_TEST) stops writes), so the farther default shape wins.
    assert_eq!(
        winner(&mut g, &[quad(1.0, -2.0, off), quad(2.0, -4.0, None)]),
        2.0
    );
}

/// depthMask FALSE: the shape is tested but leaves the depth buffer alone, so a farther shape drawn later shows through.
#[test]
fn depth_mask_false_writes_no_depth() {
    let Some(mut g) = gpu() else { return };
    let no_write = mode(|d| d.mask = false);

    assert_eq!(
        winner(&mut g, &[quad(1.0, -2.0, None), quad(2.0, -4.0, None)]),
        1.0
    );
    assert_eq!(
        winner(&mut g, &[quad(1.0, -2.0, no_write), quad(2.0, -4.0, None)]),
        2.0
    );
}

/// depthFunction: a coplanar pair (a depth tie) and a far-behind-near pair, for every function.
#[test]
fn depth_function_decides_ties_and_order() {
    let Some(mut g) = gpu() else { return };

    // (function, winner on a tie, winner when the second shape is farther, winner when it is nearer); the first shape
    // is the default draw with id 1, the second has id 2 and the function.
    for (func, tie, farther, nearer) in [
        (DepthFunc::Never, 1.0, 1.0, 1.0),
        (DepthFunc::Less, 1.0, 1.0, 2.0),
        (DepthFunc::Equal, 2.0, 1.0, 1.0),
        (DepthFunc::LessEqual, 2.0, 1.0, 2.0),
        (DepthFunc::Greater, 1.0, 2.0, 1.0),
        (DepthFunc::NotEqual, 1.0, 2.0, 2.0),
        (DepthFunc::GreaterEqual, 2.0, 2.0, 1.0),
        (DepthFunc::Always, 2.0, 2.0, 2.0),
    ] {
        let d = mode(|d| d.func = func);

        for (name, z, expect) in [
            ("tie", -3.0, tie),
            ("farther", -4.0, farther),
            ("nearer", -2.0, nearer),
        ] {
            assert_eq!(
                winner(&mut g, &[quad(1.0, -3.0, None), quad(2.0, z, d)]),
                expect,
                "{func:?} {name}"
            );
        }
    }
}

/// depthRange: gl.depthRange (1, 1) puts the shape at the far end of the buffer, behind everything drawn with the
/// default range.
#[test]
fn depth_range_moves_the_shape_in_depth() {
    let Some(mut g) = gpu() else { return };

    assert_eq!(
        winner(&mut g, &[quad(1.0, -4.0, None), quad(2.0, -2.0, None)]),
        2.0
    );
    assert_eq!(
        winner(
            &mut g,
            &[
                quad(1.0, -4.0, None),
                quad(2.0, -2.0, mode(|d| d.range = [1.0, 1.0]))
            ]
        ),
        1.0
    );
    // A range at the near end pulls a far shape in front of a nearer one.
    assert_eq!(
        winner(
            &mut g,
            &[
                quad(1.0, -2.0, None),
                quad(2.0, -4.0, mode(|d| d.range = [0.0, 0.0]))
            ]
        ),
        2.0
    );
}

/// polygonOffset (factor, units): a negative offset pulls a coplanar triangle in front of the one drawn first, where
/// LESS alone lets the first keep the pixel.
#[test]
fn polygon_offset_breaks_a_coplanar_tie() {
    let Some(mut g) = gpu() else { return };
    let less = mode(|d| d.func = DepthFunc::Less);
    let pulled = mode(|d| {
        d.func = DepthFunc::Less;
        d.polygon_offset = [0.0, -100_000.0];
    });
    let pushed = mode(|d| {
        d.func = DepthFunc::LessEqual;
        d.polygon_offset = [0.0, 100_000.0];
    });

    assert_eq!(
        winner(&mut g, &[quad(1.0, -3.0, None), quad(2.0, -3.0, less)]),
        1.0
    );
    assert_eq!(
        winner(&mut g, &[quad(1.0, -3.0, None), quad(2.0, -3.0, pulled)]),
        2.0
    );
    assert_eq!(
        winner(&mut g, &[quad(1.0, -3.0, None), quad(2.0, -3.0, pushed)]),
        1.0
    );
}

/// The state of one shape does not leak into the next (DepthMode.disable restores the test, the function and the mask).
#[test]
fn depth_mode_state_is_per_shape() {
    let Some(mut g) = gpu() else { return };
    let odd = mode(|d| {
        d.test = false;
        d.mask = false;
        d.func = DepthFunc::Never;
        d.range = [1.0, 1.0];
    });

    assert_eq!(
        winner(
            &mut g,
            &[
                quad(1.0, -4.0, odd),
                quad(2.0, -2.0, None),
                quad(3.0, -3.0, None)
            ]
        ),
        2.0
    );
}
