//! Anti-aliased polygon fill for ScreenText's canvas (`CanvasRenderingContext2D .fill ()`, nonzero winding): the exact area of
//! each pixel the outlines cover, by accumulating the signed area every edge sweeps (the scan converter of Skia's analytic
//! anti-aliasing computes the same coverage).

/// The coverage (0 to 1) of every pixel of a `width` × `height` canvas, rows from y = 0 up, for closed polygons in pixel units.
pub fn fill(width: usize, height: usize, contours: &[Vec<[f64; 2]>]) -> Vec<f64> {
    // One column more than the canvas: an edge to the right of it accumulates there.
    let stride = width + 2;
    let mut acc = vec![0.0f64; stride * height];

    for contour in contours {
        let n = contour.len();

        for i in 0..n {
            edge(
                &mut acc,
                stride,
                width,
                height,
                contour[i],
                contour[(i + 1) % n],
            );
        }
    }

    let mut out = vec![0.0; width * height];

    for y in 0..height {
        let mut sum = 0.0;

        for x in 0..width {
            sum += acc[y * stride + x];
            out[y * width + x] = sum.abs().min(1.0);
        }
    }

    out
}

/// The signed area to the right of one edge, row by row (the part above the canvas and below it is dropped; one left of it
/// counts in the first column, one right of it in the column after the last).
fn edge(acc: &mut [f64], stride: usize, width: usize, height: usize, p0: [f64; 2], p1: [f64; 2]) {
    if p0[1] == p1[1] {
        return;
    }

    let (dir, a, b) = if p0[1] < p1[1] {
        (1.0, p0, p1)
    } else {
        (-1.0, p1, p0)
    };
    let dxdy = (b[0] - a[0]) / (b[1] - a[1]);
    let w = width as f64;
    let first = a[1].max(0.0).floor() as usize;
    let last = (b[1].min(height as f64).ceil().max(0.0)) as usize;

    for y in first..last {
        let top = a[1].max(y as f64);
        let bottom = b[1].min((y + 1) as f64);
        let dy = bottom - top;

        if dy <= 0.0 {
            continue;
        }

        let x_top = (a[0] + (top - a[1]) * dxdy).clamp(0.0, w);
        let x_bottom = (a[0] + (bottom - a[1]) * dxdy).clamp(0.0, w);
        let d = dy * dir;
        let (x0, x1) = if x_top < x_bottom {
            (x_top, x_bottom)
        } else {
            (x_bottom, x_top)
        };
        let row = y * stride;
        let x0_floor = x0.floor();
        let x0_i = x0_floor as usize;
        let x1_ceil = x1.ceil();
        let x1_i = x1_ceil as usize;

        if x1_i <= x0_i + 1 {
            // Inside one pixel column.
            let xmf = 0.5 * (x_top + x_bottom) - x0_floor;

            acc[row + x0_i] += d - d * xmf;
            acc[row + x0_i + 1] += d * xmf;
        } else {
            // Across several columns: the area of the triangle in the first, the trapezoids between, the triangle in the last.
            let s = 1.0 / (x1 - x0);
            let x0f = x0 - x0_floor;
            let a0 = 0.5 * s * (1.0 - x0f) * (1.0 - x0f);
            let x1f = x1 - x1_ceil + 1.0;
            let am = 0.5 * s * x1f * x1f;

            acc[row + x0_i] += d * a0;

            if x1_i == x0_i + 2 {
                acc[row + x0_i + 1] += d * (1.0 - a0 - am);
            } else {
                let a1 = s * (1.5 - x0f);

                acc[row + x0_i + 1] += d * (a1 - a0);

                for xi in x0_i + 2..x1_i - 1 {
                    acc[row + xi] += d * s;
                }

                let a2 = a1 + (x1_i - x0_i - 3) as f64 * s;

                acc[row + x1_i - 1] += d * (1.0 - a2 - am);
            }

            acc[row + x1_i] += d * am;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::fill;

    #[test]
    fn a_pixel_aligned_square_covers_exactly_its_pixels() {
        let c = fill(
            6,
            5,
            &[vec![[1.0, 1.0], [4.0, 1.0], [4.0, 3.0], [1.0, 3.0]]],
        );

        for y in 0..5 {
            for x in 0..6 {
                let inside = (1..4).contains(&x) && (1..3).contains(&y);

                assert_eq!(
                    c[y * 6 + x],
                    if inside { 1.0 } else { 0.0 },
                    "pixel {x} {y}"
                );
            }
        }
    }

    #[test]
    fn a_half_pixel_edge_gives_half_coverage_and_a_hole_cancels() {
        let c = fill(
            4,
            4,
            &[vec![[0.5, 0.0], [3.5, 0.0], [3.5, 4.0], [0.5, 4.0]]],
        );

        assert_eq!((c[0], c[1], c[3]), (0.5, 1.0, 0.5));

        // A counter-clockwise hole inside a clockwise outline leaves the hole empty.
        let c = fill(
            6,
            6,
            &[
                vec![[0.0, 0.0], [6.0, 0.0], [6.0, 6.0], [0.0, 6.0]],
                vec![[2.0, 2.0], [2.0, 4.0], [4.0, 4.0], [4.0, 2.0]],
            ],
        );

        assert_eq!((c[0], c[2 * 6 + 2], c[3 * 6 + 3]), (1.0, 0.0, 0.0));
    }

    #[test]
    fn a_diagonal_edge_covers_half_of_its_pixels() {
        // The triangle below the diagonal of a 4 x 4 square: area 8 of 16.
        let c = fill(4, 4, &[vec![[0.0, 0.0], [4.0, 0.0], [4.0, 4.0]]]);

        assert!((c.iter().sum::<f64>() - 8.0).abs() < 1e-12);
    }
}
