//! NURBS geometry (X3D NURBS component), ported from X_ITE 16.5.0
//! Components/NURBS/{X3DNurbsSurfaceGeometryNode,NurbsPatchSurface,NurbsTrimmedSurface,NurbsCurve,
//! NurbsCurve2D,ContourPolyline2D,Contour2D,NurbsSweptSurface,NurbsSwungSurface,
//! NurbsTextureCoordinate}.js with the math in [`x3d_runtime::nurbs`].
//!
//! Owner decision 2026-10-17 (REA batch 04): ISO/IEC 19775-1 clause 27 wins where it and X_ITE
//! disagree (marked `ISO:`): rational weights, the weight count and seam-weight rules (see
//! x3d_runtime::nurbs), trimming loop orientation, and rational NurbsTextureCoordinate.

use crate::geometry::{self, ExtrusionInput, Geometry, TexChannelInfo};
use crate::sg::Sg;
use x3d_fields::NodeId;
use x3d_fields::math::Vector3;
use x3d_runtime::nurbs::{self as m, Spline};

/// X3DCast (X3DCoordinateNode, controlPoint) → get1Point of every point (doubles; a Coordinate's
/// values are its Float32 values).
fn coord_points(sg: &Sg, c: NodeId) -> Option<Vec<[f64; 3]>> {
    match sg.type_name(c)? {
        "Coordinate" => Some(
            sg.f32s(c, "point")
                .chunks_exact(3)
                .map(|p| [p[0] as f64, p[1] as f64, p[2] as f64])
                .collect(),
        ),
        "CoordinateDouble" => Some(
            sg.f64s(c, "point")
                .chunks_exact(3)
                .map(|p| [p[0], p[1], p[2]])
                .collect(),
        ),
        "GeoCoordinate" => Some(
            sg.geo_coord_points(c)?
                .iter()
                .map(|p| [p.x, p.y, p.z])
                .collect(),
        ),
        _ => None,
    }
}

fn control_point_node(sg: &Sg, n: NodeId) -> Option<Vec<[f64; 3]>> {
    coord_points(sg, sg.node(n, "controlPoint")?)
}

fn vec2s(sg: &Sg, n: NodeId, name: &str) -> Vec<[f64; 2]> {
    sg.f64s(n, name)
        .chunks_exact(2)
        .map(|p| [p[0], p[1]])
        .collect()
}

fn usize_of(v: i32) -> usize {
    v.max(0) as usize
}

/// X3DNurbsSurfaceGeometryNode build and NurbsTrimmedSurface.trimSurface. `scale` is the
/// tessellationScale a NurbsSet gave the node (1 otherwise).
pub fn surface(sg: &Sg, n: NodeId, scale: f64, trimmed: bool) -> Geometry {
    let mut g = Geometry {
        geometry_type: 3,
        solid: true,
        ccw: true,
        ..Default::default()
    };
    let mut a = Arrays::default();

    if let Some(state) = build_surface(sg, n, scale, &mut a) {
        if trimmed {
            trim_surface(sg, n, &state, &mut a);
        }

        g.solid = sg.bool(n, "solid", true);
        g.ccw = true;
    }

    // X3DGeometryNode.rebuild: shrinkToFit stores the arrays as Float32Array.
    let to32 = |v: &[f64]| v.iter().map(|x| *x as f32).collect::<Vec<f32>>();
    let mut tex = a.tex.iter().map(|t| to32(t));
    g.vertices = to32(&a.vertices);
    g.normals = to32(&a.normals);
    g.tex_coords = tex.next().unwrap_or_default();
    g.tex_coords_extra = tex.collect();
    g.tex_channels = a.infos;

    geometry::finish(&mut g, true);
    g
}

/// The X3DGeometryNode arrays while they are plain JS arrays (doubles), before shrinkToFit.
#[derive(Default)]
struct Arrays {
    vertices: Vec<f64>,
    normals: Vec<f64>,
    /// multiTexCoords
    tex: Vec<Vec<f64>>,
    infos: Vec<TexChannelInfo>,
}

struct SurfaceState {
    u_knots: Vec<f64>,
    v_knots: Vec<f64>,
    resolution: (usize, usize),
}

fn build_surface(sg: &Sg, n: NodeId, scale: f64, g: &mut Arrays) -> Option<SurfaceState> {
    let (u_order, v_order) = (sg.i32(n, "uOrder", 3), sg.i32(n, "vOrder", 3));
    let (u_dim, v_dim) = (sg.i32(n, "uDimension", 0), sg.i32(n, "vDimension", 0));

    if u_order < 2 || v_order < 2 || u_dim < u_order || v_dim < v_order {
        return None;
    }

    let points = control_point_node(sg, n)?;
    let (uo, vo, ud, vd) = (
        u_order as usize,
        v_order as usize,
        u_dim as usize,
        v_dim as usize,
    );

    if points.len() < ud * vd {
        return None;
    }

    let point = |i: usize| points[i];
    let weights = m::weights(ud * vd, &sg.f64s(n, "weight"));
    let u_closed =
        sg.bool(n, "uClosed", false) && m::u_closed(uo, ud, vd, weights.as_deref(), point);
    let v_closed =
        sg.bool(n, "vClosed", false) && m::v_closed(vo, ud, vd, weights.as_deref(), point);
    let (control, size) = m::surface_control_points(
        u_closed,
        v_closed,
        uo,
        vo,
        ud,
        vd,
        weights.as_deref(),
        point,
    );
    let u_knots = m::knots(u_closed, uo, ud, &sg.f64s(n, "uKnot"));
    let v_knots = m::knots(v_closed, vo, vd, &sg.f64s(n, "vKnot"));
    let components = 3 + weights.is_some() as usize;
    let spline = Spline::surface(
        [uo - 1, vo - 1],
        [u_knots.clone(), v_knots.clone()],
        size,
        components,
        control,
    );

    // getUTessellation: Math.floor (NURBS.getTessellation (...) * tessellationScale).
    let resolution_of =
        |t: i32, d: i32| ((m::tessellation(t, d) as f64) * scale).floor().max(0.0) as usize;
    let nu = resolution_of(sg.i32(n, "uTessellation", 0), u_dim);
    let nv = resolution_of(sg.i32(n, "vTessellation", 0), v_dim);
    let mesh = m::sample_surface(&spline, nu, nv, u_closed, v_closed, weights.is_some());

    for &face in &mesh.faces {
        let p = &mesh.points[face * 3..face * 3 + 3];
        g.vertices.extend_from_slice(&[p[0], p[1], p[2], 1.0]);
    }

    build_tex_coords(
        sg,
        n,
        g,
        (u_closed, v_closed),
        (uo, vo),
        (&u_knots, &v_knots),
        (ud, vd),
        (nu, nv),
    );
    generate_normals(g, &mesh);

    Some(SurfaceState {
        u_knots,
        v_knots,
        resolution: (nu, nv),
    })
}

/// X3DNurbsSurfaceGeometryNode.createNurbsTexCoords: a (u, v) surface of 4 components sampled
/// like the geometry (never closed, never divided by the weight).
fn nurbs_tex_coords(
    degree: [usize; 2],
    knots: [&[f64]; 2],
    size: [usize; 2],
    control: Vec<f64>,
    resolution: (usize, usize),
) -> Vec<f64> {
    let spline = Spline::surface(
        degree,
        [knots[0].to_vec(), knots[1].to_vec()],
        size,
        4,
        control,
    );
    let mesh = m::sample_surface(&spline, resolution.0, resolution.1, false, false, false);
    let mut out = Vec::with_capacity(mesh.faces.len() * 4);

    for &face in &mesh.faces {
        out.extend_from_slice(&mesh.points[face * 4..face * 4 + 4]);
    }

    out
}

/// createDefaultNurbsTexCoords: degree 1, knots [0 0 5 5], the unit square.
fn default_tex_points(resolution: (usize, usize)) -> Vec<f64> {
    let knots = [0.0, 0.0, 5.0, 5.0];
    let control = vec![
        0.0, 0.0, 0.0, 1.0, 0.0, 1.0, 0.0, 1.0, 1.0, 0.0, 0.0, 1.0, 1.0, 1.0, 0.0, 1.0,
    ];
    let spline = Spline::surface([1, 1], [knots.to_vec(), knots.to_vec()], [2, 2], 4, control);
    let mesh = m::sample_surface(&spline, resolution.0, resolution.1, false, false, false);
    let mut out = Vec::with_capacity(mesh.faces.len() * 4);

    for &face in &mesh.faces {
        out.extend_from_slice(&mesh.points[face * 4..face * 4 + 4]);
    }

    out
}

/// get1Point of an X3DSingleTextureCoordinateNode as x y z w.
fn tex_point(sg: &Sg, c: NodeId, index: usize) -> [f64; 4] {
    let (name, n) = match sg.type_name(c) {
        Some("TextureCoordinate") => ("point", 2),
        Some("TextureCoordinate3D") => ("point", 3),
        Some("TextureCoordinate4D") => ("point", 4),
        _ => return [0.0, 0.0, 0.0, 1.0],
    };
    let points = sg.f32s(c, name);

    if index < points.len() / n {
        let p = &points[index * n..index * n + n];
        match n {
            2 => [p[0] as f64, p[1] as f64, 0.0, 1.0],
            3 => [p[0] as f64, p[1] as f64, p[2] as f64, 1.0],
            _ => [p[0] as f64, p[1] as f64, p[2] as f64, p[3] as f64],
        }
    } else {
        [0.0, 0.0, 0.0, 1.0]
    }
}

/// X3DNurbsSurfaceGeometryNode.buildNurbsTexCoords.
#[allow(clippy::too_many_arguments)]
fn build_tex_coords(
    sg: &Sg,
    n: NodeId,
    g: &mut Arrays,
    closed: (bool, bool),
    order: (usize, usize),
    knots: (&[f64], &[f64]),
    dim: (usize, usize),
    resolution: (usize, usize),
) {
    let tex = sg.node(n, "texCoord");
    let ty = tex.and_then(|t| sg.type_name(t));
    let mut channels: Vec<Vec<f64>> = vec![];
    let mut infos: Vec<TexChannelInfo> = vec![];

    match ty {
        Some(
            "TextureCoordinate"
            | "TextureCoordinate3D"
            | "TextureCoordinate4D"
            | "MultiTextureCoordinate",
        ) => {
            let t = tex.unwrap();
            let nodes: Vec<NodeId> = if ty == Some("MultiTextureCoordinate") {
                sg.nodes(t, "texCoord")
                    .into_iter()
                    .filter(|c| {
                        matches!(
                            sg.type_name(*c),
                            Some(
                                "TextureCoordinate"
                                    | "TextureCoordinate3D"
                                    | "TextureCoordinate4D"
                                    | "TextureCoordinateGenerator"
                            )
                        )
                    })
                    .take(crate::extras::MAX_TEX_COORDS)
                    .collect()
            } else {
                vec![t]
            };

            for c in nodes {
                // getTexControlPoints: [u][v], closed directions extended.
                let column = |u: usize| -> Vec<f64> {
                    let mut out = vec![];
                    let rows: Vec<usize> = (0..dim.1)
                        .chain(if closed.1 {
                            1..order.1.saturating_sub(1)
                        } else {
                            0..0
                        })
                        .collect();
                    for v in rows {
                        out.extend_from_slice(&tex_point(sg, c, v * dim.0 + u));
                    }
                    out
                };
                let mut columns: Vec<Vec<f64>> = (0..dim.0).map(column).collect();
                if closed.0 {
                    for u in 1..order.0.saturating_sub(1) {
                        columns.push(columns[u].clone());
                    }
                }
                let size = [
                    columns.len(),
                    columns.first().map(|c| c.len() / 4).unwrap_or(0),
                ];
                channels.push(nurbs_tex_coords(
                    [order.0 - 1, order.1 - 1],
                    [knots.0, knots.1],
                    size,
                    columns.concat(),
                    resolution,
                ));
                infos.push(TexChannelInfo {
                    mapping: sg.string(c, "mapping", ""),
                    ..Default::default()
                });
            }
        }
        Some("NurbsTextureCoordinate") if nurbs_tex_valid(sg, tex.unwrap()) => {
            let t = tex.unwrap();
            let (uo, vo) = (
                sg.i32(t, "uOrder", 3) as usize,
                sg.i32(t, "vOrder", 3) as usize,
            );
            let (ud, vd) = (
                sg.i32(t, "uDimension", 0) as usize,
                sg.i32(t, "vDimension", 0) as usize,
            );
            let u_knots = m::knots(false, uo, ud, &sg.f64s(t, "uKnot"));
            let v_knots = m::knots(false, vo, vd, &sg.f64s(t, "vKnot"));
            let weights = m::weights(ud * vd, &sg.f64s(t, "weight"));
            let points = vec2s(sg, t, "controlPoint");
            let mut control = Vec::with_capacity(ud * vd * 4);

            for u in 0..ud {
                for v in 0..vd {
                    let index = v * ud + u;
                    let p = points[index];
                    match &weights {
                        // ISO: rational, homogeneous (u w, v w, 0, w).
                        Some(w) => control.extend_from_slice(&[
                            p[0] * w[index],
                            p[1] * w[index],
                            0.0,
                            w[index],
                        ]),
                        None => control.extend_from_slice(&[p[0], p[1], 0.0, 1.0]),
                    }
                }
            }

            let mut c = nurbs_tex_coords(
                [uo - 1, vo - 1],
                [&u_knots, &v_knots],
                [ud, vd],
                control,
                resolution,
            );

            if weights.is_some() {
                // ISO: 2D texture coordinates of the rational surface.
                for t in c.chunks_exact_mut(4) {
                    let w = t[3];
                    t[0] /= w;
                    t[1] /= w;
                    t[2] = 0.0;
                    t[3] = 1.0;
                }
            }

            channels.push(c);
        }
        _ => channels.push(default_tex_points(resolution)),
    }

    g.tex = channels;
    g.infos = infos;
}

/// NurbsTextureCoordinate.isValid.
fn nurbs_tex_valid(sg: &Sg, t: NodeId) -> bool {
    let (uo, vo) = (sg.i32(t, "uOrder", 3), sg.i32(t, "vOrder", 3));
    let (ud, vd) = (sg.i32(t, "uDimension", 0), sg.i32(t, "vDimension", 0));

    uo >= 2
        && vo >= 2
        && ud >= uo
        && vd >= vo
        && sg.f64s(t, "controlPoint").len() / 2 >= (ud * vd) as usize
}

/// generateNormals: face normals, refined per sampled point with 85°.
fn generate_normals(g: &mut Arrays, mesh: &m::Mesh) {
    let p = |i: usize| {
        Vector3::new(
            mesh.points[i * 3],
            mesh.points[i * 3 + 1],
            mesh.points[i * 3 + 2],
        )
    };
    let mut normals = Vec::with_capacity(mesh.faces.len());

    for f in mesh.faces.chunks_exact(3) {
        let normal = geometry::triangle_normal(&p(f[0]), &p(f[1]), &p(f[2]));
        normals.extend_from_slice(&[normal, normal, normal]);
    }

    let mut order: Vec<(i64, Vec<usize>)> = vec![];
    let mut map = std::collections::HashMap::new();

    for (i, &index) in mesh.faces.iter().enumerate() {
        match map.get(&index) {
            Some(&k) => {
                let entry: &mut (i64, Vec<usize>) = &mut order[k];
                entry.1.push(i);
            }
            None => {
                map.insert(index, order.len());
                order.push((index as i64, vec![i]));
            }
        }
    }

    // Algorithm.radians (85) = 85 * (Math.PI / 180).
    let refined = geometry::refine_normals(&order, &normals, 85.0 * (std::f64::consts::PI / 180.0));

    for n in refined {
        g.normals.extend_from_slice(&[n.x, n.y, n.z]);
    }
}

// --- Trimming ----------------------------------------------------------------------------------

/// Contour2D.addTrimmingContour of every Contour2D in trimmingContour: the tessellated children
/// (NurbsCurve2D, ContourPolyline2D), mapped by (point − offset) / scale.
fn trimming_contours(
    sg: &Sg,
    n: NodeId,
    offset: (f64, f64),
    scale: (f64, f64),
) -> Vec<Vec<[f64; 2]>> {
    let mut contours = vec![];

    for c in sg
        .nodes(n, "trimmingContour")
        .into_iter()
        .filter(|c| sg.type_name(*c) == Some("Contour2D"))
    {
        let mut contour = vec![];

        for child in sg.nodes(c, "children") {
            match sg.type_name(child) {
                Some("NurbsCurve2D") => contour.extend(curve2d_points(sg, child)),
                Some("ContourPolyline2D") => contour.extend(vec2s(sg, child, "controlPoint")),
                _ => {}
            }
        }

        if contour.is_empty() {
            continue;
        }

        for p in contour.iter_mut() {
            // point .subtract (offset) .divVec (scale)
            p[0] = (p[0] - offset.0) / scale.0;
            p[1] = (p[1] - offset.1) / scale.1;
        }

        contours.push(contour);
    }

    contours
}

fn signed_area(c: &[[f64; 2]]) -> f64 {
    let mut a = 0.0;

    for i in 0..c.len() {
        let (p, q) = (c[i], c[(i + 1) % c.len()]);
        a += p[0] * q[1] - q[0] * p[1];
    }

    a / 2.0
}

fn inside(p: [f64; 2], c: &[[f64; 2]]) -> bool {
    let mut odd = false;
    let mut j = c.len().wrapping_sub(1);

    for i in 0..c.len() {
        let (a, b) = (c[i], c[j]);

        if (a[1] > p[1]) != (b[1] > p[1])
            && p[0] < (b[0] - a[0]) * (p[1] - a[1]) / (b[1] - a[1]) + a[0]
        {
            odd = !odd;
        }

        j = i;
    }

    odd
}

/// ISO 27.2.5: a CW loop discards its inside and keeps the rest. X_ITE's odd rule keeps the
/// inside of every outermost loop; adding the domain once more as a contour inverts that for
/// the case where an outermost loop is CW. (Valid loops do not intersect and alternate their
/// orientation when nested, so the winding is 0 or 1 and the odd rule equals the ISO rule.)
fn iso_needs_domain(contours: &[Vec<[f64; 2]>]) -> bool {
    contours.iter().enumerate().any(|(i, c)| {
        let outermost = !contours
            .iter()
            .enumerate()
            .any(|(j, o)| j != i && !c.is_empty() && inside(c[0], o));
        outermost && signed_area(c) < 0.0
    })
}

/// Triangle2.area.
fn area(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> f64 {
    (a[0] * (b[1] - c[1]) + b[0] * (c[1] - a[1]) + c[0] * (a[1] - b[1])).abs() / 2.0
}

/// Triangle2.toBarycentric → (u, v, t).
fn barycentric(p: [f64; 2], a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> (f64, f64, f64) {
    let v0 = [b[0] - a[0], b[1] - a[1]];
    let v1 = [c[0] - a[0], c[1] - a[1]];
    let v2 = [p[0] - a[0], p[1] - a[1]];
    let dot = |x: [f64; 2], y: [f64; 2]| x[0] * y[0] + x[1] * y[1];
    let (d00, d01, d11, d20, d21) = (
        dot(v0, v0),
        dot(v0, v1),
        dot(v1, v1),
        dot(v2, v0),
        dot(v2, v1),
    );
    let denom = d00 * d11 - d01 * d01;
    let v = (d11 * d20 - d01 * d21) / denom;
    let t = (d00 * d21 - d01 * d20) / denom;
    let u = 1.0 - v - t;
    (u, v, t)
}

/// NurbsTrimmedSurface.trimSurface.
fn trim_surface(sg: &Sg, n: NodeId, s: &SurfaceState, g: &mut Arrays) {
    let at = |k: &[f64], i: i64| {
        if i < 0 {
            k.get(k.len().wrapping_sub(i.unsigned_abs() as usize))
        } else {
            k.get(i as usize)
        }
        .copied()
        .unwrap_or(f64::NAN)
    };
    let (u_min, v_min) = (at(&s.u_knots, 0), at(&s.v_knots, 0));
    let (u_max, v_max) = (at(&s.u_knots, -1), at(&s.v_knots, -1));
    let unit_square = vec![[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 1.0]];
    let loops = trimming_contours(sg, n, (u_min, v_min), (u_max - u_min, v_max - v_min));

    // Do nothing if there are no trimming contours.
    if loops.is_empty() {
        return;
    }

    let default_points = default_tex_points(s.resolution);
    let default_triangles: Vec<[f64; 2]> = default_points
        .chunks_exact(4)
        .map(|t| [t[0], t[1]])
        .collect();
    let num_default = default_triangles.len();

    let mut pass1: Vec<Vec<([f64; 2], Option<usize>)>> =
        vec![unit_square.iter().map(|p| (*p, None)).collect()];

    if iso_needs_domain(&loops) {
        pass1.push(unit_square.iter().map(|p| (*p, None)).collect());
    }

    pass1.extend(loops.iter().map(|c| c.iter().map(|p| (*p, None)).collect()));

    let trimming_triangles = x3d_text::libtess::triangulate_points(&pass1);

    let mut contours: Vec<Vec<([f64; 2], Option<usize>)>> = vec![];

    for (i, t) in default_triangles.chunks(3).enumerate() {
        contours.push(
            t.iter()
                .enumerate()
                .map(|(k, p)| (*p, Some(i * 3 + k)))
                .collect(),
        );
    }

    for t in trimming_triangles.chunks(3) {
        contours.push(t.to_vec());
    }

    let mut trimmed = x3d_text::libtess::triangulate_points(&contours);

    // Filter triangles with very small area.
    const MIN_AREA: f64 = 1e-6;
    let mut f = 0;

    for t in (0..trimmed.len().saturating_sub(2)).step_by(3) {
        let (a, b, c) = (trimmed[t], trimmed[t + 1], trimmed[t + 2]);

        if area(a.0, b.0, c.0) < MIN_AREA {
            continue;
        }

        trimmed[f] = a;
        trimmed[f + 1] = b;
        trimmed[f + 2] = c;
        f += 3;
    }

    trimmed.truncate(f);

    // Find points in defaultTriangles and interpolate new points.
    const MIN_BARYCENTRIC_DISTANCE: f64 = 1e-5;

    let channels: Vec<&Vec<f64>> = g.tex.iter().collect();
    let mut out_tex: Vec<Vec<f64>> = vec![vec![]; channels.len()];
    let mut out_normals: Vec<f64> = vec![];
    let mut out_vertices: Vec<f64> = vec![];
    let fv = |a: &[f64], i: usize| a.get(i).copied().unwrap_or(f64::NAN);

    let mut t = 0;

    'find: while t < f {
        let (p, index) = trimmed[t];

        if let Some(d) = index {
            let (d3, d4) = (d * 3, d * 4);

            for (tc, ch) in channels.iter().enumerate() {
                out_tex[tc].extend((0..4).map(|k| fv(ch, d4 + k)));
            }

            out_normals.extend((0..3).map(|k| fv(&g.normals, d3 + k)));
            out_vertices.extend((0..3).map(|k| fv(&g.vertices, d4 + k)));
            out_vertices.push(1.0);
            t += 1;
            continue;
        }

        for d in (0..num_default.saturating_sub(2)).step_by(3) {
            let (u, v, w) = barycentric(
                p,
                default_triangles[d],
                default_triangles[d + 1],
                default_triangles[d + 2],
            );

            if (u - 0.5).abs() > 0.5 + MIN_BARYCENTRIC_DISTANCE
                || (v - 0.5).abs() > 0.5 + MIN_BARYCENTRIC_DISTANCE
                || (w - 0.5).abs() > 0.5 + MIN_BARYCENTRIC_DISTANCE
            {
                continue;
            }

            let (d3, d4) = (d * 3, d * 4);

            for (tc, ch) in channels.iter().enumerate() {
                for k in 0..4 {
                    out_tex[tc]
                        .push(u * fv(ch, d4 + k) + v * fv(ch, d4 + 4 + k) + w * fv(ch, d4 + 8 + k));
                }
            }

            for k in 0..3 {
                out_normals.push(
                    u * fv(&g.normals, d3 + k)
                        + v * fv(&g.normals, d3 + 3 + k)
                        + w * fv(&g.normals, d3 + 6 + k),
                );
            }

            for k in 0..3 {
                out_vertices.push(
                    u * fv(&g.vertices, d4 + k)
                        + v * fv(&g.vertices, d4 + 4 + k)
                        + w * fv(&g.vertices, d4 + 8 + k),
                );
            }

            out_vertices.push(1.0);
            t += 1;
            continue 'find;
        }

        // Point not found, discard triangle.
        let k = t % 3;

        for tex in out_tex.iter_mut() {
            tex.truncate(tex.len().saturating_sub(k * 4));
        }

        out_normals.truncate(out_normals.len().saturating_sub(k * 3));
        out_vertices.truncate(out_vertices.len().saturating_sub(k * 4));
        t += 2 - k + 1;
    }

    g.tex = out_tex;
    g.normals = out_normals;
    g.vertices = out_vertices;
}

// --- Curves ------------------------------------------------------------------------------------

/// NurbsCurve.build: line segments between the sampled points. Returns the line geometry.
pub fn curve(sg: &Sg, n: NodeId) -> Geometry {
    let mut g = Geometry {
        geometry_type: 1,
        solid: false,
        ccw: true,
        ..Default::default()
    };

    if let Some(points) = curve_points(sg, n) {
        let num = points.len().saturating_sub(3);

        for i in (0..num).step_by(3) {
            g.vertices.extend_from_slice(&[
                points[i] as f32,
                points[i + 1] as f32,
                points[i + 2] as f32,
                1.0,
            ]);
            g.vertices.extend_from_slice(&[
                points[i + 3] as f32,
                points[i + 4] as f32,
                points[i + 5] as f32,
                1.0,
            ]);
        }
    }

    geometry::finish(&mut g, true);
    g
}

/// The sampled points (x y z) of a NurbsCurve node (`None` when build returns early).
pub fn curve_points(sg: &Sg, n: NodeId) -> Option<Vec<f64>> {
    let order = sg.i32(n, "order", 3);

    if order < 2 {
        return None;
    }

    let points = control_point_node(sg, n)?;
    let order = order as usize;

    if points.len() < order {
        return None;
    }

    let weights = m::weights(points.len(), &sg.f64s(n, "weight"));
    let closed = sg.bool(n, "closed", false) && m::curve_closed(order, weights.as_deref(), &points);
    let control = m::curve_control_points(closed, order, weights.as_deref(), &points);
    let knots = m::knots(closed, order, points.len(), &sg.f64s(n, "knot"));
    let spline = Spline::curve(order - 1, knots, 3 + weights.is_some() as usize, control);
    let resolution = usize_of(m::tessellation(
        sg.i32(n, "tessellation", 0),
        points.len() as i32,
    ));

    Some(m::sample_curve(&spline, resolution, closed, weights.is_some()).points)
}

/// NurbsCurve.tessellate: the points of the built line geometry (Float32 values).
fn curve_tessellate(sg: &Sg, n: NodeId) -> Vec<f32> {
    let g = curve(sg, n);
    let v = &g.vertices;
    let mut out = vec![];

    if !v.is_empty() {
        for i in (0..v.len()).step_by(8) {
            out.extend_from_slice(&v[i..i + 3]);
        }

        out.extend_from_slice(&v[v.len() - 4..v.len() - 1]);
    }

    out
}

/// NurbsCurve2D.tessellate (type 0 and 2): the sampled points.
pub fn curve2d_points(sg: &Sg, n: NodeId) -> Vec<[f64; 2]> {
    let order = sg.i32(n, "order", 3);
    let points = vec2s(sg, n, "controlPoint");

    if order < 2 || points.len() < order as usize {
        return vec![];
    }

    let order = order as usize;
    let weights = m::weights(points.len(), &sg.f64s(n, "weight"));
    let closed =
        sg.bool(n, "closed", false) && m::curve2d_closed(order, weights.as_deref(), &points);
    let control = m::curve_control_points(closed, order, weights.as_deref(), &points);
    let knots = m::knots(closed, order, points.len(), &sg.f64s(n, "knot"));
    let spline = Spline::curve(order - 1, knots, 2 + weights.is_some() as usize, control);
    let resolution = usize_of(m::tessellation(
        sg.i32(n, "tessellation", 0),
        points.len() as i32,
    ));

    m::sample_curve(&spline, resolution, closed, weights.is_some())
        .points
        .chunks_exact(2)
        .map(|p| [p[0], p[1]])
        .collect()
}

/// X3DNurbsControlCurveNode.tessellate (0): x y pairs.
fn control_curve_2d(sg: &Sg, n: NodeId) -> Option<Vec<[f64; 2]>> {
    match sg.type_name(n)? {
        "NurbsCurve2D" => Some(curve2d_points(sg, n)),
        "ContourPolyline2D" => Some(vec2s(sg, n, "controlPoint")),
        _ => None,
    }
}

/// NurbsSweptSurface.build / NurbsSwungSurface.build: an Extrusion (no caps, solid, ccw, convex,
/// creaseAngle π) of the cross section along the trajectory, normals flipped when ccw is FALSE.
pub fn swept(sg: &Sg, n: NodeId, swung: bool) -> Geometry {
    let (cross_field, trajectory_field) = if swung {
        ("profileCurve", "trajectoryCurve")
    } else {
        ("crossSectionCurve", "trajectoryCurve")
    };
    let cross = sg
        .node(n, cross_field)
        .and_then(|c| control_curve_2d(sg, c));
    let trajectory: Option<Vec<f32>> = sg.node(n, trajectory_field).and_then(|t| {
        if swung {
            // tessellate (1): x 0 y.
            control_curve_2d(sg, t).map(|p| {
                p.iter()
                    .flat_map(|p| [p[0] as f32, 0.0, p[1] as f32])
                    .collect()
            })
        } else {
            (sg.type_name(t) == Some("NurbsCurve")).then(|| curve_tessellate(sg, t))
        }
    });

    let (Some(cross), Some(spine)) = (cross, trajectory) else {
        let mut g = Geometry {
            geometry_type: 3,
            solid: true,
            ccw: true,
            ..Default::default()
        };
        geometry::finish(&mut g, false);
        return g;
    };

    // extrusion ._crossSection = array (MFVec2f: Float32).
    let cross: Vec<f32> = cross
        .iter()
        .flat_map(|p| [p[0] as f32, p[1] as f32])
        .collect();
    let mut g = geometry::extrusion(&ExtrusionInput {
        cross_section: &cross,
        orientation: &[0.0, 0.0, 1.0, 0.0],
        scale: &[1.0, 1.0],
        spine: &spine,
        begin_cap: false,
        end_cap: false,
        solid: true,
        ccw: true,
        convex: true,
        crease_angle: std::f64::consts::PI,
    });

    let ccw = sg.bool(n, "ccw", true);

    if !ccw {
        for v in g.normals.iter_mut() {
            *v *= -1.0;
        }
    }

    g.solid = sg.bool(n, "solid", true);
    g.ccw = ccw;
    g
}
