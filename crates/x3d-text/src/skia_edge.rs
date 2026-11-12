//! The polylines that Skia's analytic anti-aliasing scan converter fills for a path on a software 2D canvas (ScreenText draws on
//! a canvas made with `willReadFrequently`): SkAnalyticEdge.setLine, SkAnalyticQuadraticEdge and SkAnalyticCubicEdge
//! (SkAnalyticEdge.cpp), on top of SkQuadraticEdge and SkCubicEdge (SkEdge.cpp), after SkEdgeBuilder cut each curve at its Y extrema.
//!
//! The scan converter does not fill the curve. It walks each Y-monotonic part in `1 << shift` forward-difference steps in 16.16
//! fixed point and joins the steps with lines whose Y is snapped to a quarter of a pixel (`SnapY`, kDefaultAccuracy 2); a line's
//! ends are truncated to 1/256 pixel in X and snapped the same way in Y. On an edge that crosses many columns in one row, a Y
//! moved by an eighth of a pixel moves the coverage of the row's pixels by up to a quarter, so these vertices, not the exact
//! curve, give the canvas' alpha. The functions take points in the canvas' device space (pixels, Y in the direction of
//! the canvas' rows), because the truncation, the snapping and the walk from the first row are not symmetric.
//!
//! Checked against Chrome 156's canvas (triangles, circles of 8 quadratic arcs and of 4 cubic arcs at three sizes): the alpha of
//! every pixel within 26 of 255, and at most 7 pixels of a shape over 8, where the exact curve is up to 75 off. The rest is the
//! scan converter's own coverage arithmetic, which crate::raster replaces by the exact area.

type Fixed = i64;

/// One step of a canvas path, in the canvas' device space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum PathCmd {
    MoveTo([f64; 2]),
    LineTo([f64; 2]),
    /// `quadraticCurveTo`: the control point, then the end.
    QuadTo([f64; 2], [f64; 2]),
    /// `bezierCurveTo`: two control points, then the end.
    CubicTo([f64; 2], [f64; 2], [f64; 2]),
    ClosePath,
}

/// `fill ()` of one path (nonzero winding) on a `width` × `height` canvas: the coverage (0 to 1) of every pixel, in the canvas'
/// row order. A canvas keeps its path in single precision, so every point is rounded to f32 first.
pub fn fill_path(width: usize, height: usize, path: &[PathCmd]) -> Vec<f64> {
    // The Y of every line end: the rows where the scan converter stops. They do not depend on X, so a first pass finds them
    // and the second walks each cubic line over them (see `walk`).
    let first = contours_of(path, None);
    let mut events: Vec<Fixed> = first
        .iter()
        .flatten()
        .map(|v| (v[1] * 65536.0).round() as Fixed)
        .collect();

    events.sort_unstable();
    events.dedup();

    crate::raster::fill(width, height, &contours_of(path, Some(&events)))
}

/// The contours of a path as lines of the scan converter. With `events`, the lines of cubic curves follow the walk.
fn contours_of(path: &[PathCmd], events: Option<&[Fixed]>) -> Vec<Vec<[f64; 2]>> {
    let single = |p: [f64; 2]| [p[0] as f32 as f64, p[1] as f32 as f64];
    let mut contours: Vec<Vec<[f64; 2]>> = vec![];
    let mut contour: Vec<[f64; 2]> = vec![];
    // The current point of the path (a line's vertex is rounded, a curve starts from the path's own point) and the start of
    // the subpath, where closePath returns.
    let (mut current, mut start) = ([0.0, 0.0], [0.0, 0.0]);

    // A subpath of one point (a moveTo, or the point where closePath left the path) has no edge.
    let end_subpath = |contour: &mut Vec<[f64; 2]>, contours: &mut Vec<Vec<[f64; 2]>>| {
        let done = std::mem::take(contour);

        if done.len() > 1 {
            contours.push(done);
        }
    };

    for cmd in path {
        match *cmd {
            PathCmd::MoveTo(p) => {
                end_subpath(&mut contour, &mut contours);
                current = single(p);
                start = current;
                contour.push(point(current));
            }
            PathCmd::LineTo(p) => {
                current = single(p);
                contour.push(point(current));
            }
            PathCmd::QuadTo(c, p) => {
                let to = single(p);

                quad(current, single(c), to, &mut contour);
                current = to;
            }
            PathCmd::CubicTo(c1, c2, p) => {
                let to = single(p);

                cubic(current, single(c1), single(c2), to, &mut contour, events);
                current = to;
            }
            // The fill closes every subpath by itself. closePath also starts a new subpath at the start point, so a later
            // line or curve does not join the closed one.
            PathCmd::ClosePath if contour.is_empty() => {}
            PathCmd::ClosePath => {
                end_subpath(&mut contour, &mut contours);
                current = start;
                contour.push(point(current));
            }
        }
    }

    end_subpath(&mut contour, &mut contours);

    contours
}

/// Source-over of a white fill with `coverage` onto the canvas' alpha (8 bits, as the canvas keeps it).
pub fn composite(alpha: &mut [u8], coverage: &[f64]) {
    for (dst, src) in alpha.iter_mut().zip(coverage) {
        let src = (src * 255.0).round() as u32;

        if src != 0 {
            let d = *dst as u32;

            *dst = (src + (d * (255 - src) + 127) / 255) as u8;
        }
    }
}

/// `SnapY`: a 16.16 Y rounded to a quarter of a pixel.
fn snap_y(y: Fixed) -> Fixed {
    ((y + (0x10000 >> 3)) >> 14) << 14
}

/// A coordinate as SkFDot6 scaled up by kDefaultAccuracy (1/256 pixel), truncated toward zero.
fn dot8(v: f64) -> Fixed {
    (v * 256.0) as Fixed
}

fn pixels(x: Fixed, y: Fixed) -> [f64; 2] {
    [x as f64 / 65536.0, y as f64 / 65536.0]
}

/// The end of a line (SkAnalyticEdge.setLine): X in 1/256 pixel, Y snapped.
pub(crate) fn point(p: [f64; 2]) -> [f64; 2] {
    pixels(dot8(p[0]) << 8, snap_y(dot8(p[1]) << 8))
}

/// cheap_distance and diff_to_shift with the AA shift 2.
fn diff_to_shift(dx: Fixed, dy: Fixed) -> u32 {
    let dist = if dx > dy {
        dx + (dy >> 1)
    } else {
        dy + (dx >> 1)
    };
    let dist = ((dist + (1 << 4)) >> (3 + 2)) as u32;

    (32 - dist.leading_zeros()) >> 1
}

/// SkFDot6Div and QuickSkFDot6Div: a 16.16 quotient, truncated toward zero.
fn fdot6_div(a: Fixed, b: Fixed) -> Fixed {
    (a << 16) / b
}

/// The lines of a quadratic curve from `p0`: appends every vertex after the first.
pub(crate) fn quad(p0: [f64; 2], p1: [f64; 2], p2: [f64; 2], out: &mut Vec<[f64; 2]>) {
    // SkChopQuadAtYExtrema
    let (a, b, c) = (p0[1], p1[1], p2[1]);

    if (b - a) * (c - b) < 0.0 {
        let t = (a - b) / (a - 2.0 * b + c);

        if t > 0.0 && t < 1.0 {
            let lerp =
                |p: [f64; 2], q: [f64; 2]| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
            let (mut ab, mut bc) = (lerp(p0, p1), lerp(p1, p2));
            let mid = lerp(ab, bc);

            ab[1] = mid[1];
            bc[1] = mid[1];
            quad_edge([p0, ab, mid], out);
            quad_edge([mid, bc, p2], out);
            return;
        }
    }

    quad_edge([p0, p1, p2], out);
}

/// SkQuadraticEdge.setQuadraticWithoutUpdate and SkAnalyticQuadraticEdge.updateQuadratic for one Y-monotonic quad.
fn quad_edge(p: [[f64; 2]; 3], out: &mut Vec<[f64; 2]>) {
    let mut x = p.map(|q| dot8(q[0]));
    let mut y = p.map(|q| dot8(q[1]));
    let reverse = y[0] > y[2];

    if reverse {
        x.reverse();
        y.reverse();
    }

    // A quad of no height is no edge (SkFDot6Round of both ends).
    if (y[0] + 32) >> 6 == (y[2] + 32) >> 6 {
        out.push(point(p[2]));
        return;
    }

    let dx = (((x[1] << 1) - x[0] - x[2]) >> 2).abs();
    let dy = (((y[1] << 1) - y[0] - y[2]) >> 2).abs();
    // At least 1 subdivision, at most MAX_COEFF_SHIFT (6).
    let shift = diff_to_shift(dx, dy).clamp(1, 6);
    let curve_shift = shift - 1;
    // [Q, QD, QDD, QLast], each shifted down by kDefaultAccuracy.
    let coeff = |v: [Fixed; 3]| {
        let a = (v[0] - v[1] - v[1] + v[2]) << 9;
        let b = (v[1] - v[0]) << 10;

        [
            (v[0] << 10) >> 2,
            (b + (a >> shift)) >> 2,
            (a >> (shift - 1)) >> 2,
            (v[2] << 10) >> 2,
        ]
    };
    let (cx, mut cy) = (coeff(x), coeff(y));

    cy[0] = snap_y(cy[0]);
    cy[3] = snap_y(cy[3]);

    let (mut old_x, mut old_y) = (cx[0], cy[0]);
    let (mut dx, mut dy) = (cx[1], cy[1]);
    let (mut snapped_x, mut snapped_y) = (cx[0], cy[0]);
    let mut vertices = vec![(snapped_x, snapped_y)];
    let mut count = 1 << shift;

    while count > 0 {
        count -= 1;

        let (new_x, new_y, new_snapped_x, new_snapped_y, slope);

        if count > 0 {
            new_x = old_x + (dx >> curve_shift);
            new_y = old_y + (dy >> curve_shift);

            if (dy >> curve_shift).abs() >= 0x20000 {
                // A step of 2 pixels or more in Y ends on a whole pixel, and X follows the line.
                let diff_y = (new_y - snapped_y) >> 10;

                slope = (diff_y != 0).then(|| fdot6_div((new_x - snapped_x) >> 10, diff_y));
                new_snapped_y = cy[3].min(((new_y + 0x8000) >> 16) << 16);
                new_snapped_x = new_x - slope.map_or(0, |s| (s * (new_y - new_snapped_y)) >> 16);
            } else {
                new_snapped_y = cy[3].min(snap_y(new_y));
                new_snapped_x = new_x;

                let diff_y = (new_snapped_y - snapped_y) >> 10;

                slope = (diff_y != 0).then(|| fdot6_div((new_x - snapped_x) >> 10, diff_y));
            }

            dx += cx[2];
            dy += cy[2];
        } else {
            // The last line ends on the curve's end.
            new_x = cx[3];
            new_y = cy[3];
            new_snapped_x = new_x;
            new_snapped_y = new_y;

            let diff_y = (new_y - snapped_y) >> 10;

            slope = (diff_y != 0).then(|| fdot6_div((new_x - snapped_x) >> 10, diff_y));
        }

        // updateLine refuses a line of no height, and a slope of 0 is not passed to it: the next line starts where this one
        // started. The end of the curve is always a vertex.
        let line = slope.is_some_and(|s| s != 0) && (new_snapped_y - snapped_y) >> 10 != 0;

        if line || count == 0 {
            vertices.push((new_snapped_x, new_snapped_y));
            snapped_x = new_snapped_x;
            snapped_y = new_snapped_y;
        }

        old_x = new_x;
        old_y = new_y;
    }

    append(vertices, reverse, out);
}

/// The lines of a cubic curve from `p0`: appends every vertex after the first.
pub(crate) fn cubic(
    p0: [f64; 2],
    p1: [f64; 2],
    p2: [f64; 2],
    p3: [f64; 2],
    out: &mut Vec<[f64; 2]>,
    events: Option<&[Fixed]>,
) {
    // SkChopCubicAtYExtrema: SkFindCubicExtrema on Y, then SkChopCubicAt with each next t mapped into the part that is left.
    let (a, b, c, d) = (p0[1], p1[1], p2[1], p3[1]);
    let roots = unit_quad_roots(d - a + 3.0 * (b - c), 2.0 * (a - b - b + c), b - a);
    let mut pts = [p0, p1, p2, p3];
    let mut done = 0.0;

    for (i, &root) in roots.iter().enumerate() {
        let t = if i == 0 {
            root
        } else {
            (root - done) / (1.0 - done)
        };

        if !(t > 0.0 && t < 1.0) {
            break;
        }

        let lerp = |p: [f64; 2], q: [f64; 2]| [p[0] + (q[0] - p[0]) * t, p[1] + (q[1] - p[1]) * t];
        let (ab, bc, cd) = (
            lerp(pts[0], pts[1]),
            lerp(pts[1], pts[2]),
            lerp(pts[2], pts[3]),
        );
        let (mut abc, mut bcd) = (lerp(ab, bc), lerp(bc, cd));
        let mid = lerp(abc, bcd);

        // flatten_double_cubic_extrema: the tangent at the extremum is horizontal.
        abc[1] = mid[1];
        bcd[1] = mid[1];
        cubic_edge([pts[0], ab, abc, mid], out, events);
        pts = [mid, bcd, cd, pts[3]];
        done = root;
    }

    cubic_edge(pts, out, events);
}

/// SkFindUnitQuadRoots: the roots of A t² + B t + C in the open interval (0, 1), ascending, a double root once.
fn unit_quad_roots(a: f64, b: f64, c: f64) -> Vec<f64> {
    // valid_unit_divide
    let unit = |numer: f64, denom: f64| -> Option<f64> {
        let (numer, denom) = if numer < 0.0 {
            (-numer, -denom)
        } else {
            (numer, denom)
        };

        if denom == 0.0 || numer == 0.0 || numer >= denom {
            return None;
        }

        let r = numer / denom;

        (r > 0.0 && r < 1.0).then_some(r)
    };

    if a == 0.0 {
        return unit(-c, b).into_iter().collect();
    }

    let dr = b * b - 4.0 * a * c;

    if dr < 0.0 || !dr.is_finite() {
        return vec![];
    }

    let dr = dr.sqrt();
    let q = if b < 0.0 {
        -(b - dr) / 2.0
    } else {
        -(b + dr) / 2.0
    };
    let mut roots: Vec<f64> = [unit(q, a), unit(c, q)].into_iter().flatten().collect();

    if roots.len() == 2 {
        if roots[0] > roots[1] {
            roots.swap(0, 1);
        } else if roots[0] == roots[1] {
            roots.pop();
        }
    }

    roots
}

/// SkCubicEdge.setCubicWithoutUpdate and SkAnalyticCubicEdge.updateCubic for one Y-monotonic cubic.
///
/// With `events`, each line is walked as SkScan_AAAPath's aaa_walk_edges does (see `walk`), and the next line starts where the
/// walk ended: SkAnalyticCubicEdge::keepContinuous sets fCx to the edge's fX before every updateCubic. The X of the
/// truncated slope steps adds up along the curve. Quadratic edges take each new X from fQx and do not drift.
fn cubic_edge(p: [[f64; 2]; 4], out: &mut Vec<[f64; 2]>, events: Option<&[Fixed]>) {
    let mut x = p.map(|q| dot8(q[0]));
    let mut y = p.map(|q| dot8(q[1]));
    let reverse = y[0] > y[3];

    if reverse {
        x.reverse();
        y.reverse();
    }

    // A cubic of no height is no edge.
    if (y[0] + 32) >> 6 == (y[3] + 32) >> 6 {
        out.push(point(p[3]));
        return;
    }

    // cubic_delta_from_line: the distance of the curve from its chord at t = 1/3 and t = 2/3.
    let delta = |v: [Fixed; 4]| {
        let one_third = ((v[0] * 8 - v[1] * 15 + 6 * v[2] + v[3]) * 19) >> 9;
        let two_third = ((v[0] + 6 * v[1] - v[2] * 15 + v[3] * 8) * 19) >> 9;

        one_third.abs().max(two_third.abs())
    };
    // One more than the shift of the distance ("by observation" in Skia), at most MAX_COEFF_SHIFT (6).
    let shift = (diff_to_shift(delta(x), delta(y)) + 1).min(6);
    // The points are shifted down by 10 at most, and the coefficients carry a factor 3: 6 is the largest safe up shift.
    let (up, down) = if shift + 6 >= 10 {
        (6, shift + 6 - 10)
    } else {
        (10 - shift, 0)
    };
    // [C, CD, CDD, CDDD, CLast], each shifted down by kDefaultAccuracy.
    let coeff = |v: [Fixed; 4]| {
        let b = (3 * (v[1] - v[0])) << up;
        let c = (3 * (v[0] - v[1] - v[1] + v[2])) << up;
        let d = (v[3] + 3 * (v[1] - v[2]) - v[0]) << up;

        [
            (v[0] << 10) >> 2,
            (b + (c >> shift) + (d >> (2 * shift))) >> 2,
            (2 * c + ((3 * d) >> (shift - 1))) >> 2,
            ((3 * d) >> (shift - 1)) >> 2,
            (v[3] << 10) >> 2,
        ]
    };
    let (mut cx, mut cy) = (coeff(x), coeff(y));

    cy[0] = snap_y(cy[0]);
    cy[4] = snap_y(cy[4]);

    let (mut old_x, mut old_y) = (cx[0], cy[0]);
    let mut vertices = vec![(old_x, old_y)];
    let mut count: i32 = -(1 << shift);

    while count < 0 {
        count += 1;

        let (new_x, mut new_y);

        if count < 0 {
            new_x = old_x + (cx[1] >> down);
            cx[1] += cx[2] >> shift;
            cx[2] += cx[3];
            new_y = old_y + (cy[1] >> down);
            cy[1] += cy[2] >> shift;
            cy[2] += cy[3];
        } else {
            // The last line ends on the curve's end.
            new_x = cx[4];
            new_y = cy[4];
        }

        // The fixed point walk can go back in Y or past the end: both are pinned.
        if new_y < old_y {
            new_y = old_y;
        }

        let mut new_snapped_y = snap_y(new_y);

        if cy[4] < new_snapped_y {
            new_snapped_y = cy[4];
            count = 0;
        }

        // A step of no height is no line, but the next line starts at its X.
        let snapped_y = vertices.last().unwrap().1;
        let end_x = match events {
            Some(events) if new_snapped_y > snapped_y => {
                // SkAnalyticCubicEdge.updateCubic: the slope of the line from the walked start, then the walk to its end.
                let slope = fdot6_div((new_x - old_x) >> 10, (new_snapped_y - snapped_y) >> 10);

                walk(old_x, snapped_y, new_snapped_y, slope, events)
            }
            _ => new_x,
        };

        vertices.push((end_x, new_snapped_y));
        old_x = end_x;
        old_y = new_y;
    }

    // The walk ends near, not on, the curve's end: the next edge starts on the end itself (a step of no height).
    if events.is_some() {
        vertices.push((cx[4], cy[4]));
    }

    append(vertices, reverse, out);
}

/// The X where an edge from (`x`, `y0`) with `slope` (16.16) is at `y1`, walked as aaa_walk_edges walks it: from each
/// row stop to the next (the line ends in `events`, and the integer rows), with `goY (y, yShift)`, which adds
/// `fDX >> yShift` per step (a quarter row when the gap has the quarter bit, half a row for a gap of a half, else a row).
fn walk(mut x: Fixed, y0: Fixed, y1: Fixed, slope: Fixed, events: &[Fixed]) -> Fixed {
    let mut y = y0;
    let mut i = events.partition_point(|&e| e <= y);

    while y < y1 {
        let row = ((y >> 16) + 1) << 16;
        let mut next = events.get(i).copied().unwrap_or(Fixed::MAX).min(row);
        let gap = next - y;
        let shift = if gap & 0x4000 != 0 {
            next = y + 0x4000;
            2
        } else if gap & 0x8000 != 0 {
            1
        } else {
            0
        };

        x += slope >> shift;
        y = next;

        while i < events.len() && events[i] <= y {
            i += 1;
        }
    }

    x
}

/// The vertices after the first, in the direction of the path.
fn append(mut vertices: Vec<(Fixed, Fixed)>, reverse: bool, out: &mut Vec<[f64; 2]>) {
    if reverse {
        vertices.reverse();
    }

    out.extend(vertices[1..].iter().map(|&(x, y)| pixels(x, y)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_line_end_is_truncated_in_x_and_snapped_to_a_quarter_in_y() {
        assert_eq!(point([3.3, 5.2]), [844.0 / 256.0, 5.25]);
        assert_eq!(point([8.1, 33.4]), [2073.0 / 256.0, 33.5]);
        // Toward zero, not down.
        assert_eq!(point([-0.999, 0.1]), [-255.0 / 256.0, 0.0]);
    }

    #[test]
    fn a_curve_ends_on_its_snapped_end_point_in_both_directions() {
        for (a, b) in [([10.3, 4.2], [40.6, 30.9]), ([40.6, 30.9], [10.3, 4.2])] {
            let mut q = vec![];

            quad(a, [a[0], b[1]], b, &mut q);
            assert_eq!(*q.last().unwrap(), point(b));

            let mut c = vec![];

            cubic(a, [a[0], b[1]], [a[0], b[1]], b, &mut c, None);
            assert_eq!(*c.last().unwrap(), point(b));
        }
    }

    #[test]
    fn a_quarter_circle_of_70_pixels_has_16_lines_with_y_on_quarter_pixels() {
        let (r, k) = (70.3, 0.5523 * 70.3);
        let mut v = vec![];

        cubic([r, 0.0], [r, k], [k, r], [0.0, r], &mut v, None);
        assert_eq!(v.len(), 16);
        assert!(v.iter().all(|p| (p[1] * 4.0).fract() == 0.0));
        // The vertices are near the curve: within a quarter of a pixel of the circle.
        assert!(
            v.iter()
                .all(|p| ((p[0] * p[0] + p[1] * p[1]).sqrt() - r).abs() < 0.25)
        );
    }

    #[test]
    fn the_walk_adds_the_truncated_steps_of_each_row_stop() {
        // A slope just past -1 pixel per row (16.16). A row with no stop is one step of the whole slope. A stop at a quarter
        // cuts the row: a quarter step, then the three quarters left as a quarter and a half. Each step adds `slope >> shift`,
        // which rounds down, so the cut row ends 2/65536 pixel further left.
        let slope = -0x10001;

        assert_eq!(walk(0, 0, 0x10000, slope, &[0, 0x10000]), -0x10001);
        assert_eq!(
            walk(0, 0, 0x10000, slope, &[0, 0x4000, 0x10000]),
            2 * (slope >> 2) + (slope >> 1)
        );
        assert_eq!(2 * (slope >> 2) + (slope >> 1), -0x10003);
        // A line that starts on a quarter: the same three-quarter gap.
        assert_eq!(
            walk(0, 0x4000, 0x10000, slope, &[0x4000, 0x10000]),
            (slope >> 2) + (slope >> 1)
        );
    }

    #[test]
    fn a_cubic_line_starts_where_the_walk_of_the_previous_line_ended() {
        // Without events the lines end on the forward-difference points; with them each next line starts at the walked X,
        // so the lines drift, and the last vertex is the curve's end itself.
        let (p0, p1, p2, p3) = ([10.0, 0.0], [2.0, 6.0], [2.0, 14.0], [10.0, 20.0]);
        let (mut exact, mut walked) = (vec![], vec![]);

        cubic(p0, p1, p2, p3, &mut exact, None);

        let mut events: Vec<Fixed> = std::iter::once(point(p0))
            .chain(exact.iter().copied())
            .map(|v| (v[1] * 65536.0).round() as Fixed)
            .collect();

        events.sort_unstable();
        events.dedup();
        cubic(p0, p1, p2, p3, &mut walked, Some(&events));

        assert_eq!(walked.last(), exact.last());
        assert_eq!(walked.len(), exact.len() + 1);

        let ys = |v: &[[f64; 2]]| v.iter().map(|p| p[1]).collect::<Vec<_>>();

        assert_eq!(ys(&walked[..exact.len()]), ys(&exact));
        assert!(
            walked[..exact.len() - 1]
                .iter()
                .zip(&exact)
                .any(|(w, e)| w[0] != e[0]),
            "the walk changed no X"
        );
    }

    #[test]
    fn a_curve_is_cut_at_its_y_extremum() {
        // Up and down again: two Y-monotonic parts, and the extremum (y = 10 at t = 0.5) is a vertex.
        let mut v = vec![];

        quad([0.0, 30.0], [20.0, -10.0], [40.0, 30.0], &mut v);
        assert!(v.contains(&[20.0, 10.0]), "{v:?}");

        let mut v = vec![];

        cubic(
            [0.0, 30.0],
            [10.0, 0.0],
            [30.0, 0.0],
            [40.0, 30.0],
            &mut v,
            None,
        );
        assert!(v.contains(&[20.0, 7.5]), "{v:?}");
    }

    const A: [f64; 2] = [2.0, 2.0];
    const TRIANGLE: [PathCmd; 3] = [
        PathCmd::MoveTo(A),
        PathCmd::LineTo([12.0, 2.0]),
        PathCmd::LineTo([12.0, 12.0]),
    ];

    /// TRIANGLE, then `between`, then `rest`.
    fn after_triangle(between: &[PathCmd], rest: &[PathCmd]) -> Vec<f64> {
        fill_path(24, 24, &[&TRIANGLE[..], between, rest].concat())
    }

    #[test]
    fn close_path_starts_a_new_subpath_at_the_start_point() {
        // `M a, L b, L c, Z, L d, L e` is two triangles, a b c and a d e, not the polygon a b c d e.
        let lines = [PathCmd::LineTo([2.0, 14.0]), PathCmd::LineTo([8.0, 14.0])];
        let got = after_triangle(&[PathCmd::ClosePath], &lines);

        assert_eq!(got, after_triangle(&[PathCmd::MoveTo(A)], &lines));
        // Pixels inside each triangle, and one between them that only the polygon a b c d e covers.
        assert!(got[5 * 24 + 10] > 0.999);
        assert!(got[10 * 24 + 3] > 0.999);
        assert!(got[12 * 24 + 8] < 0.001);
    }

    #[test]
    fn a_curve_after_close_path_starts_at_the_start_point() {
        for curve in [
            PathCmd::QuadTo([2.0, 20.0], [10.0, 20.0]),
            PathCmd::CubicTo([2.0, 14.0], [4.0, 20.0], [10.0, 20.0]),
        ] {
            assert_eq!(
                after_triangle(&[PathCmd::ClosePath], &[curve]),
                after_triangle(&[PathCmd::MoveTo(A)], &[curve])
            );
        }
    }

    #[test]
    fn close_path_before_move_to_or_the_end_changes_nothing() {
        let next = [
            PathCmd::MoveTo([14.0, 14.0]),
            PathCmd::LineTo([20.0, 14.0]),
            PathCmd::LineTo([20.0, 20.0]),
        ];

        assert_eq!(
            after_triangle(&[PathCmd::ClosePath], &next),
            after_triangle(&[], &next)
        );
        assert_eq!(
            after_triangle(&[PathCmd::ClosePath], &[]),
            after_triangle(&[], &[])
        );
    }
}
