//! btBoxBoxDetector (the ODE box-box test of Russell L. Smith, redistributed under the zlib
//! license with Bullet 2.82), reporting its contacts through a callback exactly as
//! `btDiscreteCollisionDetectorInterface::Result::addContactPoint` receives them.
//!
//! Rounding: values stored in the C++ stack arrays (`A`, `B`, `quad`, `ret`, `point`, `dep`,
//! `center`, ...) are rounded to float; scalars kept in locals are not (asm.js doubles).

use super::math::*;

/// fudge_factor as a float constant.
const FUDGE_FACTOR: f64 = 1.05_f32 as f64;
const FLT_MAX: f64 = f32::MAX as f64;
const M_PI32: f64 = 3.14159265_f32 as f64;

type R = [[f64; 3]; 3];

fn dot41(r: &R, col: usize, v: V3) -> f64 {
    r[0][col] * v[0] + r[1][col] * v[1] + r[2][col] * v[2]
}

fn dot44(a: &R, ca: usize, b: &R, cb: usize) -> f64 {
    a[0][ca] * b[0][cb] + a[1][ca] * b[1][cb] + a[2][ca] * b[2][cb]
}

fn dot14(v: V3, r: &R, col: usize) -> f64 {
    v[0] * r[0][col] + v[1] * r[1][col] + v[2] * r[2][col]
}

fn d_dot(a: V3, b: V3) -> f64 {
    a[0] * b[0] + a[1] * b[1] + a[2] * b[2]
}

/// intersectRectQuad2: the points of the quad clipped by the rectangle; the result has 2 * n values.
fn intersect_rect_quad2(h: [f64; 2], p: [f64; 8]) -> Vec<f64> {
    let mut nq = 4usize;
    let mut cur: Vec<f64> = p.to_vec();
    let mut nr = 0usize;

    'outer: for dir in 0..2usize {
        for sign in [-1.0_f64, 1.0] {
            // chop q along the line xy[dir] = sign*h[dir]
            let mut out: Vec<f64> = Vec::with_capacity(16);
            nr = 0;
            let first_stage = dir == 0 && sign < 0.0;

            for i in (1..=nq).rev() {
                let idx = (nq - i) * 2;
                let mut pq = [cur[idx], cur[idx + 1]];

                // the first stage runs on unrounded locals. The second corner is stored when it is
                // inside on the unrounded value; the x read back from memory (a float) after that
                // store is what the crossing test against the third corner then sees, so a corner
                // that sits inside by less than a float ulp is stored and also gets a crossing point
                let inside_store = sign * pq[dir] < h[dir];

                if first_stage && idx == 2 && inside_store {
                    pq[0] = f(pq[0]);
                }

                // go through all points in q and all lines between adjacent points
                if inside_store {
                    // this point is inside the chopping line
                    out.push(f(pq[0]));
                    out.push(f(pq[1]));
                    nr += 1;

                    if nr & 8 != 0 {
                        cur = out;
                        break 'outer;
                    }
                }

                let nextq = if i > 1 {
                    [cur[idx + 2], cur[idx + 3]]
                } else {
                    [cur[0], cur[1]]
                };

                if (sign * pq[dir] < h[dir]) ^ (sign * nextq[dir] < h[dir]) {
                    // this line crosses the chopping line
                    let mut np = [0.0; 2];
                    np[1 - dir] = f(pq[1 - dir]
                        + (nextq[1 - dir] - pq[1 - dir]) / (nextq[dir] - pq[dir])
                            * (sign * h[dir] - pq[dir]));
                    np[dir] = f(sign * h[dir]);
                    out.push(np[0]);
                    out.push(np[1]);
                    nr += 1;

                    if nr & 8 != 0 {
                        cur = out;
                        break 'outer;
                    }
                }
            }

            cur = out;
            nq = nr;
        }
    }

    cur.truncate(nr * 2);
    cur
}

/// cullPoints2: choose m of the n points of the polygon p, the first being i0.
fn cull_points2(n: usize, p: &[f64], m: usize, i0: usize) -> Vec<usize> {
    let (cx, cy);

    if n == 1 {
        cx = p[0];
        cy = p[1];
    } else if n == 2 {
        cx = 0.5 * (p[0] + p[2]);
        cy = 0.5 * (p[1] + p[3]);
    } else {
        let mut a = 0.0;
        let mut x = 0.0;
        let mut y = 0.0;

        for i in 0..(n - 1) {
            let q = p[i * 2] * p[i * 2 + 3] - p[i * 2 + 2] * p[i * 2 + 1];
            a += q;
            x += q * (p[i * 2] + p[i * 2 + 2]);
            y += q * (p[i * 2 + 1] + p[i * 2 + 3]);
        }

        let q = p[n * 2 - 2] * p[1] - p[0] * p[n * 2 - 1];

        if (a + q).abs() > SIMD_EPSILON {
            a = f(1.0 / (3.0 * (a + q)));
        } else {
            a = BT_LARGE_FLOAT;
        }

        cx = a * (x + q * (p[n * 2 - 2] + p[0]));
        cy = a * (y + q * (p[n * 2 - 1] + p[1]));
    }

    // compute the angle of each point w.r.t. the centroid
    let mut ang = [0.0; 8];
    for i in 0..n {
        ang[i] = f((p[i * 2 + 1] - cy).atan2(p[i * 2] - cx));
    }

    // search for points that have angles closest to A[i0] + i*(2*pi/m).
    let mut avail = [true; 8];
    avail[i0] = false;
    let mut iret = vec![i0];

    for j in 1..m {
        let mut a = (j as f64) * (2.0 * M_PI32 / m as f64) + ang[i0];

        if a > M_PI32 {
            a -= 2.0 * M_PI32;
        }

        let mut maxdiff = 1e9;
        let mut sel = i0;

        for i in 0..n {
            if avail[i] {
                let mut diff = (ang[i] - a).abs();

                if diff > M_PI32 {
                    diff = 2.0 * M_PI32 - diff;
                }

                if diff < maxdiff {
                    maxdiff = diff;
                    sel = i;
                }
            }
        }

        avail[sel] = false;
        iret.push(sel);
    }

    iret
}

/// One reported contact: (normalOnB, pointInWorld, depth).
pub type Contact = (V3, V3, f64);

/// dBoxBox2 with maxc = 4. `side1`/`side2` are twice the half extents with margin. Returns the
/// contacts in call order.
pub fn box_box(p1: V3, r1: &R, side1: V3, p2: V3, r2: &R, side2: V3) -> Vec<Contact> {
    let mut out: Vec<Contact> = vec![];
    // the tests run on unrounded values; the stack arrays A and B hold floats
    let au = [side1[0] * 0.5, side1[1] * 0.5, side1[2] * 0.5];
    let bu = [side2[0] * 0.5, side2[1] * 0.5, side2[2] * 0.5];
    let a = r3(au);
    let b = r3(bu);

    // get vector from centers of box 1 to box 2, relative to box 1
    let p = sub(p2, p1);
    let pp = [dot41(r1, 0, p), dot41(r1, 1, p), dot41(r1, 2, p)];

    // Rij is R1'*R2, i.e. the relative rotation between R1 and R2
    let r11 = dot44(r1, 0, r2, 0);
    let r12 = dot44(r1, 0, r2, 1);
    let r13 = dot44(r1, 0, r2, 2);
    let r21 = dot44(r1, 1, r2, 0);
    let r22 = dot44(r1, 1, r2, 1);
    let r23 = dot44(r1, 1, r2, 2);
    let r31 = dot44(r1, 2, r2, 0);
    let r32 = dot44(r1, 2, r2, 1);
    let r33 = dot44(r1, 2, r2, 2);

    let (mut q11, mut q12, mut q13) = (r11.abs(), r12.abs(), r13.abs());
    let (mut q21, mut q22, mut q23) = (r21.abs(), r22.abs(), r23.abs());
    let (mut q31, mut q32, mut q33) = (r31.abs(), r32.abs(), r33.abs());

    let mut s = -FLT_MAX;
    let mut invert_normal = false;
    let mut code = 0;
    // normalR: (box, column)
    let mut normal_r: Option<(usize, usize)> = None;
    let mut normal_c = ZERO;

    macro_rules! tst1 {
        ($e1:expr, $e2:expr, $norm:expr, $cc:expr) => {{
            let e1 = $e1;
            let s2 = e1.abs() - ($e2);
            if s2 > 0.0 {
                return out;
            }
            if s2 > s {
                s = s2;
                normal_r = Some($norm);
                invert_normal = e1 < 0.0;
                code = $cc;
            }
        }};
    }

    // separating axis = u1,u2,u3
    tst1!(
        pp[0],
        au[0] + bu[0] * q11 + bu[1] * q12 + bu[2] * q13,
        (1, 0),
        1
    );
    tst1!(
        pp[1],
        au[1] + bu[0] * q21 + bu[1] * q22 + bu[2] * q23,
        (1, 1),
        2
    );
    tst1!(
        pp[2],
        au[2] + bu[0] * q31 + bu[1] * q32 + bu[2] * q33,
        (1, 2),
        3
    );

    // separating axis = v1,v2,v3 (the association is the one the compiled code uses)
    tst1!(
        dot41(r2, 0, p),
        bu[0] + (au[2] * q31 + (au[0] * q11 + au[1] * q21)),
        (2, 0),
        4
    );
    tst1!(
        dot41(r2, 1, p),
        bu[1] + (au[2] * q32 + (au[0] * q12 + au[1] * q22)),
        (2, 1),
        5
    );
    tst1!(
        dot41(r2, 2, p),
        bu[2] + (au[2] * q33 + (au[0] * q13 + au[1] * q23)),
        (2, 2),
        6
    );

    // note: cross product axes need to be scaled when s is computed.
    macro_rules! tst2 {
        ($e1:expr, $e2:expr, $n1:expr, $n2:expr, $n3:expr, $cc:expr) => {{
            let e1 = $e1;
            let mut s2 = e1.abs() - ($e2);
            if s2 > SIMD_EPSILON {
                return out;
            }
            let (n1, n2, n3) = ($n1, $n2, $n3);
            let l = (n1 * n1 + n2 * n2 + n3 * n3).sqrt();
            if l > SIMD_EPSILON {
                s2 /= l;
                if s2 * FUDGE_FACTOR > s {
                    s = s2;
                    normal_r = None;
                    normal_c = [n1 / l, n2 / l, n3 / l];
                    invert_normal = e1 < 0.0;
                    code = $cc;
                }
            }
        }};
    }

    let fudge2 = 1.0e-5_f32 as f64;
    q11 += fudge2;
    q12 += fudge2;
    q13 += fudge2;
    q21 += fudge2;
    q22 += fudge2;
    q23 += fudge2;
    q31 += fudge2;
    q32 += fudge2;
    q33 += fudge2;

    // separating axis = u1 x (v1,v2,v3)
    tst2!(
        pp[2] * r21 - pp[1] * r31,
        bu[2] * q12 + ((au[1] * q31 + au[2] * q21) + bu[1] * q13),
        0.0,
        -r31,
        r21,
        7
    );
    tst2!(
        pp[2] * r22 - pp[1] * r32,
        bu[2] * q11 + ((au[1] * q32 + au[2] * q22) + bu[0] * q13),
        0.0,
        -r32,
        r22,
        8
    );
    tst2!(
        pp[2] * r23 - pp[1] * r33,
        bu[1] * q11 + (bu[0] * q12 + (au[1] * q33 + au[2] * q23)),
        0.0,
        -r33,
        r23,
        9
    );

    // separating axis = u2 x (v1,v2,v3)
    tst2!(
        pp[0] * r31 - pp[2] * r11,
        bu[2] * q22 + ((au[0] * q31 + au[2] * q11) + bu[1] * q23),
        r31,
        0.0,
        -r11,
        10
    );
    tst2!(
        pp[0] * r32 - pp[2] * r12,
        bu[2] * q21 + ((au[0] * q32 + au[2] * q12) + bu[0] * q23),
        r32,
        0.0,
        -r12,
        11
    );
    tst2!(
        pp[0] * r33 - pp[2] * r13,
        bu[1] * q21 + (bu[0] * q22 + (au[0] * q33 + au[2] * q13)),
        r33,
        0.0,
        -r13,
        12
    );

    // separating axis = u3 x (v1,v2,v3)
    tst2!(
        pp[1] * r11 - pp[0] * r21,
        bu[2] * q32 + ((au[0] * q21 + au[1] * q11) + bu[1] * q33),
        -r21,
        r11,
        0.0,
        13
    );
    tst2!(
        pp[1] * r12 - pp[0] * r22,
        bu[2] * q31 + ((au[0] * q22 + au[1] * q12) + bu[0] * q33),
        -r22,
        r12,
        0.0,
        14
    );
    tst2!(
        pp[1] * r13 - pp[0] * r23,
        bu[1] * q31 + (bu[0] * q32 + (au[0] * q23 + au[1] * q13)),
        -r23,
        r13,
        0.0,
        15
    );

    if code == 0 {
        return out;
    }

    // if we get to this point, the boxes interpenetrate. compute the normal in global coordinates.
    let mut normal: V3 = match normal_r {
        Some((bx, c)) => {
            let rr = if bx == 1 { r1 } else { r2 };
            [rr[0][c], rr[1][c], rr[2][c]]
        }
        None => [
            d_dot(r1[0], normal_c),
            d_dot(r1[1], normal_c),
            d_dot(r1[2], normal_c),
        ],
    };

    if invert_normal {
        normal = [-normal[0], -normal[1], -normal[2]];
    }
    let normal = r3(normal);

    // compute contact point(s)
    if code > 6 {
        // an edge from box 1 touches an edge from box 2. find a point pa on the intersecting edge of box 1
        let col = |r: &R, j: usize| [r[0][j], r[1][j], r[2][j]];
        let sg1 = |j: usize| {
            if dot14(normal, r1, j) > 0.0 {
                1.0
            } else {
                -1.0
            }
        };
        let (s0, s1, s2) = (sg1(0) * au[0], sg1(1) * au[1], sg1(2) * au[2]);
        let pa: V3 = std::array::from_fn(|i| p1[i] + s0 * r1[i][0] + s1 * r1[i][1] + s2 * r1[i][2]);
        // the incident box: the third half extent is read back from the float array B
        let sg2 = |j: usize| {
            if dot14(normal, r2, j) > 0.0 {
                -1.0
            } else {
                1.0
            }
        };
        let (t0, t1, t2) = (sg2(0) * bu[0], sg2(1) * bu[1], sg2(2) * b[2]);
        let pb: V3 = std::array::from_fn(|i| p2[i] + t0 * r2[i][0] + t1 * r2[i][1] + t2 * r2[i][2]);
        let ua = col(r1, (code - 7) / 3);
        let ub = col(r2, (code - 7) % 3);

        // dLineClosestApproach, beta only
        let uaub = ua[0] * ub[0] + ua[1] * ub[1] + ua[2] * ub[2];
        let o = 1.0 - uaub * uaub;
        let beta = if !(o <= 0.0001_f32 as f64) {
            let d = [pb[0] - pa[0], pb[1] - pa[1], pb[2] - pa[2]];
            ((d[0] * ua[0] + d[1] * ua[1] + d[2] * ua[2]) * uaub
                - (d[0] * ub[0] + d[1] * ub[1] + d[2] * ub[2]))
                * (1.0 / o)
        } else {
            0.0
        };

        let pbv = [
            pb[0] + beta * ub[0],
            pb[1] + beta * ub[1],
            pb[2] + beta * ub[2],
        ];
        // the depth is passed on as the unrounded local
        out.push((r3([-normal[0], -normal[1], -normal[2]]), r3(pbv), s));
        return out;
    }

    // okay, we have a face-something intersection (because the separating axis is perpendicular
    // to a face). define face 'a' to be the reference face and face 'b' the incident face.
    let (ra, rb, pa, pb, sa, sb) = if code <= 3 {
        (r1, r2, p1, p2, a, b)
    } else {
        (r2, r1, p2, p1, b, a)
    };

    // nr = normal vector of reference face dotted with axes of incident box.
    let normal2: V3 = if code <= 3 {
        normal
    } else {
        [-normal[0], -normal[1], -normal[2]]
    };
    // nr is stored as floats, but the largest component is chosen on the unrounded values
    let nru = [
        dot41(rb, 0, normal2),
        dot41(rb, 1, normal2),
        dot41(rb, 2, normal2),
    ];
    let nr = r3(nru);
    let anr = [nru[0].abs(), nru[1].abs(), nru[2].abs()];

    // find the largest component of anr: this corresponds to the normal for the incident face.
    let (lanr, a1, a2);

    if anr[1] > anr[0] {
        if anr[1] > anr[2] {
            a1 = 0;
            lanr = 1;
            a2 = 2;
        } else {
            a1 = 0;
            a2 = 1;
            lanr = 2;
        }
    } else if anr[0] > anr[2] {
        lanr = 0;
        a1 = 1;
        a2 = 2;
    } else {
        a1 = 0;
        a2 = 1;
        lanr = 2;
    }

    // compute center point of incident face, in reference-face coordinates
    let mut center = ZERO;

    for i in 0..3 {
        center[i] = if nr[lanr] < 0.0 {
            pb[i] - pa[i] + sb[lanr] * rb[i][lanr]
        } else {
            pb[i] - pa[i] - sb[lanr] * rb[i][lanr]
        };
    }

    // find the normal and non-normal axis numbers of the reference box
    let code_n = if code <= 3 { code - 1 } else { code - 4 };
    let (code1, code2) = match code_n {
        0 => (1, 2),
        1 => (0, 2),
        _ => (0, 1),
    };

    // find the four corners of the incident face, in reference-face coordinates
    let c1 = dot14(center, ra, code1);
    let c2 = dot14(center, ra, code2);
    let mut m11 = dot44(ra, code1, rb, a1);
    let mut m12 = dot44(ra, code1, rb, a2);
    let mut m21 = dot44(ra, code2, rb, a1);
    let mut m22 = dot44(ra, code2, rb, a2);

    let k1 = m11 * sb[a1];
    let k2 = m21 * sb[a1];
    let k3 = m12 * sb[a2];
    let k4 = m22 * sb[a2];
    let quad = [
        c1 - k1 - k3,
        c2 - k2 - k4,
        c1 - k1 + k3,
        c2 - k2 + k4,
        c1 + k1 + k3,
        c2 + k2 + k4,
        c1 + k1 - k3,
        c2 + k2 - k4,
    ];

    // find the size of the reference face
    let rect = [f(sa[code1]), f(sa[code2])];

    // intersect the incident and reference faces
    let mut ret = intersect_rect_quad2(rect, quad);
    let n = ret.len() / 2;

    if n < 1 {
        return out;
    }

    // convert the intersection points into reference-face coordinates, and compute the contact
    // position and depth for each point.
    let mut point: Vec<V3> = vec![];
    let mut dep: Vec<f64> = vec![];
    let det1 = 1.0 / (m11 * m22 - m12 * m21);
    m11 *= det1;
    m12 *= det1;
    m21 *= det1;
    m22 *= det1;
    let mut cnum = 0;

    for j in 0..n {
        let k1 = m22 * (ret[j * 2] - c1) - m12 * (ret[j * 2 + 1] - c2);
        let k2 = -m21 * (ret[j * 2] - c1) + m11 * (ret[j * 2 + 1] - c2);
        let mut pt = ZERO;

        for i in 0..3 {
            pt[i] = center[i] + k1 * rb[i][a1] + k2 * rb[i][a2];
        }
        // the depth uses the unrounded point, the arrays hold floats
        let d = f(sa[code_n] - d_dot(normal2, pt));
        let pt = r3(pt);

        if cnum < point.len() {
            point[cnum] = pt;
            dep[cnum] = d;
        } else {
            point.push(pt);
            dep.push(d);
        }

        if d >= 0.0 {
            ret[cnum * 2] = ret[j * 2];
            ret[cnum * 2 + 1] = ret[j * 2 + 1];
            cnum += 1;
        }
    }

    if cnum < 1 {
        return out;
    }

    let maxc = 4usize.min(cnum).max(1);
    let neg_normal = r3([-normal[0], -normal[1], -normal[2]]);

    if cnum <= maxc {
        for j in 0..cnum {
            let mut pw = ZERO;

            for i in 0..3 {
                pw[i] = if code < 4 {
                    point[j][i] + pa[i]
                } else {
                    point[j][i] + pa[i] - normal[i] * dep[j]
                };
            }

            out.push((neg_normal, r3(pw), f(-dep[j])));
        }
    } else {
        // we have more contacts than are wanted, some of them must be culled.
        let mut i1 = 0;
        let mut maxdepth = dep[0];

        for i in 1..cnum {
            if dep[i] > maxdepth {
                maxdepth = dep[i];
                i1 = i;
            }
        }

        let iret = cull_points2(cnum, &ret, maxc, i1);

        for j in 0..maxc {
            let k = iret[j];
            let mut pw = ZERO;

            for i in 0..3 {
                pw[i] = point[k][i] + pa[i];
            }

            if code < 4 {
                out.push((neg_normal, r3(pw), f(-dep[k])));
            } else {
                out.push((neg_normal, r3(sub(pw, scale(normal, dep[k]))), f(-dep[k])));
            }
        }
    }

    out
}
