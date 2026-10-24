//! Geometry generation, ported from X_ITE:
//! Components/Rendering/X3DGeometryNode.js (rebuild, generateTexCoords, refineNormals, updateBBox),
//! Components/Geometry3D/IndexedFaceSet.js (triangulate, build, createNormals),
//! Components/Rendering/{Coordinate,Color,Normal}.js, Components/Texturing/TextureCoordinate.js,
//! Components/Rendering/{IndexedLineSet,PointSet}.js,
//! Components/Geometry3D/{Box,Sphere,Cone,Cylinder,Extrusion,ElevationGrid}.js and
//! Browser/Geometry3D/{Box,QuadSphere,Cone,Cylinder}Options.js.
//!
//! X_ITE computes in doubles and stores the attribute arrays as Float32Array; this module does
//! the same (f64 math, f32 storage). Field values (MFVec3f …) are already f32.

use crate::tess;
use x3d_fields::jsmath;
use x3d_fields::math::{Matrix4, Rotation4, Vector2, Vector3};

/// The arrays X3DGeometryNode hands to the GPU.
#[derive(Clone, Debug, Default)]
pub struct Geometry {
    /// 0 points, 1 lines, 2 2D geometry, 3 3D geometry (X3DGeometryNode.geometryType).
    pub geometry_type: u8,
    /// x y z 1 per vertex.
    pub vertices: Vec<f32>,
    /// x y z per vertex; empty if the geometry has no normals (points, lines).
    pub normals: Vec<f32>,
    /// s t p q per vertex (multiTexCoords [0]).
    pub tex_coords: Vec<f32>,
    /// r g b a per vertex; empty unless a Color/ColorRGBA node is used (colorMaterial).
    pub colors: Vec<f32>,
    /// One depth per vertex; empty unless a FogCoordinate node is used.
    pub fog_depths: Vec<f32>,
    /// x y z w per vertex: a Tangent node's vectors, or generated (MikkTSpace).
    pub tangents: Vec<f32>,
    /// Texture coordinate channels 1.. (multiTexCoords [1..]); `tex_coords` is channel 0.
    pub tex_coords_extra: Vec<Vec<f32>>,
    /// Per channel of a texCoord node: generator mode (0 = none), parameter, mapping.
    pub tex_channels: Vec<TexChannelInfo>,
    /// X3DVertexAttributeNode arrays (attribArrays), in attrib order.
    pub attribs: Vec<AttribArray>,
    /// X3DGeometryNode.coordIndices: the coord index of each vertex (x3d_CoordIndex, read by the
    /// HAnim skinning); empty means 0 for every vertex ("fill with zero data if not provided").
    pub coord_indices: Vec<u32>,
    pub solid: bool,
    pub ccw: bool,
    pub min: Vector3,
    pub max: Vector3,
    /// Text.display's `renderContext .textureNode`: a ScreenText's canvas, drawn instead of the appearance's texture.
    pub texture: Option<std::rc::Rc<crate::texture::TextureImage>>,
    /// Built anew for (about) every frame, like the LINE particle triangles: once no frame holds it, the renderer
    /// rewrites its vertex buffer for the next geometry instead of keeping it for KEEP_FRAMES.
    pub per_frame: bool,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct TexChannelInfo {
    pub mode: i32,
    pub parameter: [f32; 6],
    pub mapping: String,
}

#[derive(Clone, Debug, Default)]
pub struct AttribArray {
    pub name: String,
    pub components: usize,
    pub values: Vec<f32>,
}

impl Geometry {
    pub fn vertex_count(&self) -> usize {
        self.vertices.len() / 4
    }

    pub fn bbox_center(&self) -> Vector3 {
        let mut c = self.min;
        c.add(&self.max).divide(2.0);
        c
    }

    pub fn bbox_size(&self) -> Vector3 {
        let mut s = self.max;
        s.subtract(&self.min);
        s
    }
}

fn push3(a: &mut Vec<f32>, v: &Vector3) {
    a.push(v.x as f32);
    a.push(v.y as f32);
    a.push(v.z as f32);
}

fn normalize(v: &mut Vector3) {
    // Vector3.normalize: divide by the norm if it is not zero.
    v.normalize();
}

/// Triangle3.normal (a, b, c).
pub fn triangle_normal(a: &Vector3, b: &Vector3, c: &Vector3) -> Vector3 {
    let (x1, y1, z1) = (c.x - b.x, c.y - b.y, c.z - b.z);
    let (x2, y2, z2) = (a.x - b.x, a.y - b.y, a.z - b.z);
    let mut n = Vector3::new(y1 * z2 - z1 * y2, z1 * x2 - x1 * z2, x1 * y2 - y1 * x2);
    normalize(&mut n);
    n
}

/// Triangle3.quadNormal (a, b, c, d).
pub fn quad_normal(a: &Vector3, b: &Vector3, c: &Vector3, d: &Vector3) -> Vector3 {
    let (x1, y1, z1) = (c.x - a.x, c.y - a.y, c.z - a.z);
    let (x2, y2, z2) = (d.x - b.x, d.y - b.y, d.z - b.z);
    let mut n = Vector3::new(y1 * z2 - z1 * y2, z1 * x2 - x1 * z2, x1 * y2 - y1 * x2);
    normalize(&mut n);
    n
}

/// X3DCoordinateNode over an MFVec3f.
pub struct Coords<'a>(pub &'a [f32]);

impl Coords<'_> {
    pub fn len(&self) -> usize {
        self.0.len() / 3
    }

    pub fn is_empty(&self) -> bool {
        self.0.len() < 3
    }

    /// get1Point: (0, 0, 0) for an index past the end.
    pub fn get(&self, index: i64) -> Vector3 {
        if index >= 0 && (index as usize) < self.len() {
            let i = index as usize * 3;
            Vector3::new(self.0[i] as f64, self.0[i + 1] as f64, self.0[i + 2] as f64)
        } else {
            Vector3::ZERO
        }
    }

    /// addPoint.
    pub fn add_point(&self, index: i64, array: &mut Vec<f32>) {
        if index >= 0 && (index as usize) < self.len() {
            let i = index as usize * 3;
            array.extend_from_slice(&[self.0[i], self.0[i + 1], self.0[i + 2], 1.0]);
        } else {
            array.extend_from_slice(&[0.0, 0.0, 0.0, 1.0]);
        }
    }

    /// getNormal (i1, i2, i3): zero vector if an index is out of range.
    pub fn normal(&self, i1: i64, i2: i64, i3: i64) -> Vector3 {
        let l = self.len() as i64;
        if i1 < l && i2 < l && i3 < l {
            triangle_normal(&self.get(i1), &self.get(i2), &self.get(i3))
        } else {
            Vector3::ZERO
        }
    }

    pub fn quad_normal(&self, i1: i64, i2: i64, i3: i64, i4: i64) -> Vector3 {
        let l = self.len() as i64;
        if i1 < l && i2 < l && i3 < l && i4 < l {
            quad_normal(&self.get(i1), &self.get(i2), &self.get(i3), &self.get(i4))
        } else {
            Vector3::ZERO
        }
    }
}

/// Color (3 components) or ColorRGBA (4 components) addColor / addColors.
#[derive(Clone, Copy)]
pub struct Colors<'a> {
    pub values: &'a [f32],
    pub components: usize,
}

impl Colors<'_> {
    fn len(&self) -> usize {
        self.values.len() / self.components
    }

    pub fn add_color(&self, index: i64, array: &mut Vec<f32>) {
        let length = self.len();

        if index >= 0 && length > 0 {
            let i = (index as usize % length) * self.components;
            let v = self.values;
            let a = if self.components == 4 { v[i + 3] } else { 1.0 };
            array.extend_from_slice(&[v[i], v[i + 1], v[i + 2], a]);
        } else {
            array.extend_from_slice(&[1.0, 1.0, 1.0, 1.0]);
        }
    }

    pub fn add_colors(&self, array: &mut Vec<f32>, min: usize) {
        for i in 0..min {
            self.add_color(if self.len() > 0 { i as i64 } else { -1 }, array);
        }
    }
}

/// Normal.addVector: (0, 0, 0) for a negative index or an empty node.
pub fn add_vector(vectors: &[f32], index: i64, array: &mut Vec<f32>) {
    let length = vectors.len() / 3;

    if index >= 0 && length > 0 {
        let i = (index as usize % length) * 3;
        array.extend_from_slice(&vectors[i..i + 3]);
    } else {
        array.extend_from_slice(&[0.0, 0.0, 0.0]);
    }
}

/// TextureCoordinate.addPointToChannel.
pub fn add_tex_point(points: &[f32], index: i64, array: &mut Vec<f32>) {
    let length = points.len() / 2;

    if index >= 0 && length > 0 {
        let i = (index as usize % length) * 2;
        array.extend_from_slice(&[points[i], points[i + 1], 0.0, 1.0]);
    } else {
        array.extend_from_slice(&[0.0, 0.0, 0.0, 1.0]);
    }
}

/// `array [i] ?? fallback`: JS undefined past the end.
fn at_or(array: &[i32], i: usize, fallback: i64) -> i64 {
    array.get(i).map(|v| *v as i64).unwrap_or(fallback)
}

/// X3DGeometryNode.refineNormals.
pub fn refine_normals(
    normal_index: &[(i64, Vec<usize>)],
    normals: &[Vector3],
    crease_angle: f64,
) -> Vec<Vector3> {
    if crease_angle <= 0.0 {
        return normals.to_vec();
    }

    let cos_crease_angle = jsmath::cos(jsmath::clamp(crease_angle, 0.0, std::f64::consts::PI));
    let mut refined = normals.to_vec();

    for (_, vertex) in normal_index {
        for &p in vertex {
            let pn = normals[p];
            let mut n = Vector3::ZERO;

            for &q in vertex {
                let qn = normals[q];

                if qn.dot(&pn) > cos_crease_angle {
                    n.add(&qn);
                }
            }

            normalize(&mut n);
            refined[p] = n;
        }
    }

    refined
}

/// Insertion-ordered multimap point → positions, like the JS Map in createNormals.
#[derive(Default)]
struct NormalIndex {
    order: Vec<(i64, Vec<usize>)>,
    map: std::collections::HashMap<i64, usize>,
}

impl NormalIndex {
    fn push(&mut self, point: i64, index: usize) {
        match self.map.get(&point) {
            Some(&i) => self.order[i].1.push(index),
            None => {
                self.map.insert(point, self.order.len());
                self.order.push((point, vec![index]));
            }
        }
    }
}

/// Inputs of an IndexedFaceSet (fields plus the property nodes' arrays).
#[derive(Clone, Copy)]
pub struct IfsInput<'a> {
    pub coord: Option<&'a [f32]>,
    pub coord_index: &'a [i32],
    pub tex_coord: Option<&'a [f32]>,
    pub tex_coord_index: &'a [i32],
    pub normal: Option<&'a [f32]>,
    pub normal_index: &'a [i32],
    pub color: Option<Colors<'a>>,
    pub color_index: &'a [i32],
    pub ccw: bool,
    pub solid: bool,
    pub convex: bool,
    pub crease_angle: f64,
    pub color_per_vertex: bool,
    pub normal_per_vertex: bool,
    pub extras: crate::extras::Extras<'a>,
}

impl<'a> IfsInput<'a> {
    pub fn new(coord: &'a [f32], coord_index: &'a [i32]) -> Self {
        IfsInput {
            coord: Some(coord),
            coord_index,
            tex_coord: None,
            tex_coord_index: &[],
            normal: None,
            normal_index: &[],
            color: None,
            color_index: &[],
            ccw: true,
            solid: true,
            convex: true,
            crease_angle: 0.0,
            color_per_vertex: true,
            normal_per_vertex: true,
            extras: Default::default(),
        }
    }
}

struct Polygon {
    /// Positions in coordIndex.
    vertices: Vec<usize>,
    triangles: Vec<usize>,
    face: usize,
}

/// IndexedFaceSet.triangulate.
fn triangulate(input: &IfsInput, coord_index: &[i32]) -> Vec<Polygon> {
    let mut polygons = vec![];
    let coord = Coords(input.coord.unwrap_or(&[]));
    let mut vertices: Vec<usize> = vec![];
    let mut face = 0;

    for (i, &index) in coord_index.iter().enumerate() {
        if index > -1 {
            vertices.push(i);
            continue;
        }

        match vertices.len() {
            0..=2 => vertices.clear(),
            3 => {
                let v = std::mem::take(&mut vertices);
                polygons.push(Polygon {
                    triangles: v.clone(),
                    vertices: v,
                    face,
                });
            }
            _ => {
                let triangles = if input.convex {
                    tess::triangulate_convex(&vertices)
                } else {
                    // IndexedFaceSet.triangulatePolygon → Triangle3.triangulatePolygon (libtess, ODD).
                    let points: Vec<([f64; 3], usize)> = vertices
                        .iter()
                        .map(|&v| (coord.get(coord_index[v] as i64).to_array(), v))
                        .collect();
                    x3d_text::libtess::triangulate_indexed(&points)
                };

                if triangles.len() < 3 {
                    vertices.clear();
                } else {
                    let v = std::mem::take(&mut vertices);
                    polygons.push(Polygon {
                        vertices: v,
                        triangles,
                        face,
                    });
                }
            }
        }

        face += 1;
    }

    polygons
}

/// IndexedFaceSet.getPolygonNormal (Newell's method).
fn polygon_normal(vertices: &[usize], coord_index: &[i32], coord: &Coords) -> Vector3 {
    let mut normal = Vector3::ZERO;
    let length = vertices.len();
    let mut next = coord.get(coord_index[vertices[0]] as i64);

    for i in 0..length {
        let current = next;
        next = coord.get(coord_index[vertices[(i + 1) % length]] as i64);

        normal.x += (current.y - next.y) * (current.z + next.z);
        normal.y += (current.z - next.z) * (current.x + next.x);
        normal.z += (current.x - next.x) * (current.y + next.y);
    }

    normalize(&mut normal);
    normal
}

/// IndexedFaceSet.build + X3DGeometryNode.rebuild. Returns no geometry if nothing is drawn.
pub fn indexed_face_set(input: &IfsInput) -> Geometry {
    let mut g = Geometry {
        geometry_type: 3,
        solid: input.solid,
        ccw: input.ccw,
        ..Default::default()
    };

    // "Add -1 (polygon end marker) to coordIndex if not present."
    let mut coord_index = input.coord_index.to_vec();
    if input.coord.is_some() && coord_index.last().is_some_and(|v| *v > -1) {
        coord_index.push(-1);
    }

    let polygons = if input.coord.is_some() {
        triangulate(input, &coord_index)
    } else {
        vec![]
    };

    if !polygons.is_empty() {
        let coord = Coords(input.coord.unwrap());
        let extras = &input.extras;

        extras.init(&mut g);

        for p in &polygons {
            for &i in &p.triangles {
                let index = coord_index[i] as i64;

                g.coord_indices.push(index as u32);
                extras.add_attribs_and_fog(index, &mut g);

                if let Some(c) = &input.color {
                    let ci = if input.color_per_vertex {
                        at_or(input.color_index, i, index)
                    } else {
                        at_or(input.color_index, p.face, p.face as i64)
                    };
                    c.add_color(ci, &mut g.colors);
                }

                if extras.tex.is_some() {
                    extras.add_tex(at_or(input.tex_coord_index, i, index), &mut g);
                } else if let Some(t) = input.tex_coord {
                    add_tex_point(t, at_or(input.tex_coord_index, i, index), &mut g.tex_coords);
                }

                let ni = if input.normal_per_vertex {
                    at_or(input.normal_index, i, index)
                } else {
                    at_or(input.normal_index, p.face, p.face as i64)
                };

                extras.add_tangent(ni, &mut g);

                if let Some(n) = input.normal {
                    add_vector(n, ni, &mut g.normals);
                }

                coord.add_point(index, &mut g.vertices);
            }
        }

        if input.normal.is_none() {
            // createNormals
            let cw = !input.ccw;
            let mut normal_index = NormalIndex::default();
            let mut normals = vec![Vector3::ZERO; coord_index.len()];

            for p in &polygons {
                let v = &p.vertices;
                let mut normal = match v.len() {
                    3 => coord.normal(
                        coord_index[v[0]] as i64,
                        coord_index[v[1]] as i64,
                        coord_index[v[2]] as i64,
                    ),
                    4 => coord.quad_normal(
                        coord_index[v[0]] as i64,
                        coord_index[v[1]] as i64,
                        coord_index[v[2]] as i64,
                        coord_index[v[3]] as i64,
                    ),
                    _ => polygon_normal(v, &coord_index, &coord),
                };

                if cw {
                    normal.negate();
                }

                for &index in v {
                    normal_index.push(coord_index[index] as i64, index);
                    normals[index] = normal;
                }
            }

            let normals = if input.normal_per_vertex {
                refine_normals(&normal_index.order, &normals, input.crease_angle)
            } else {
                normals
            };

            for p in &polygons {
                for &i in &p.triangles {
                    push3(&mut g.normals, &normals[i]);
                }
            }
        }
    }

    // rebuild: generateTexCoords when no channel was made (no texCoord node, or one with none).
    let has_tex = if input.extras.tex.is_some() {
        input.extras.channels() > 0
    } else {
        input.tex_coord.is_some()
    };
    finish(&mut g, has_tex);
    g
}

/// X3DGeometryNode.rebuild after build: bbox, generated texCoords, generated tangents.
pub fn finish(g: &mut Geometry, has_tex_coords: bool) {
    finish_bbox(g, None);

    if !has_tex_coords && g.tex_coords.is_empty() {
        generate_tex_coords(g);
    }

    if g.tangents.is_empty() {
        generate_tangents(g);
    }
}

struct MikkMesh<'a> {
    positions: &'a [f32],
    normals: &'a [f32],
    tex_coords: &'a [f32],
    tangents: Vec<f32>,
}

impl mikktspace::Geometry for MikkMesh<'_> {
    fn num_faces(&self) -> usize {
        self.positions.len() / 9
    }

    fn num_vertices_of_face(&self, _face: usize) -> usize {
        3
    }

    fn position(&self, face: usize, vert: usize) -> [f32; 3] {
        let i = (face * 3 + vert) * 3;
        [
            self.positions[i],
            self.positions[i + 1],
            self.positions[i + 2],
        ]
    }

    fn normal(&self, face: usize, vert: usize) -> [f32; 3] {
        let i = (face * 3 + vert) * 3;
        [self.normals[i], self.normals[i + 1], self.normals[i + 2]]
    }

    fn tex_coord(&self, face: usize, vert: usize) -> [f32; 2] {
        let i = (face * 3 + vert) * 2;
        [self.tex_coords[i], self.tex_coords[i + 1]]
    }

    fn set_tangent_encoded(&mut self, tangent: [f32; 4], face: usize, vert: usize) {
        let i = (face * 3 + vert) * 4;
        self.tangents[i..i + 4].copy_from_slice(&tangent);
    }
}

/// X3DGeometryNode.generateTangents: MikkTSpace over the unindexed triangles (2D and 3D
/// geometry only), w flipped. X_ITE runs mikktspace 0.3.0 as wasm; a failure there (a panic
/// on short arrays, or "Failed to generate tangents.") is caught and leaves no tangents.
pub fn generate_tangents(g: &mut Geometry) {
    if g.geometry_type < 2 || g.vertices.is_empty() {
        return;
    }

    let positions: Vec<f32> = g
        .vertices
        .chunks_exact(4)
        .flat_map(|v| [v[0], v[1], v[2]])
        .collect();
    let tex_coords: Vec<f32> = g
        .tex_coords
        .chunks_exact(4)
        .flat_map(|v| [v[0], v[1]])
        .collect();
    let n = positions.len() / 3;

    // The wasm indexes the arrays per face vertex: anything shorter traps there.
    if g.normals.len() < (n / 3) * 9 || tex_coords.len() < (n / 3) * 6 {
        return;
    }

    // The wasm returns one tangent per vertex of a whole triangle (a trailing partial face has none).
    let mut mesh = MikkMesh {
        positions: &positions,
        normals: &g.normals,
        tex_coords: &tex_coords,
        tangents: vec![0.0; (n / 3) * 12],
    };

    let ok = std::panic::catch_unwind(std::panic::AssertUnwindSafe(|| {
        mikktspace::generate_tangents(&mut mesh)
    }))
    .unwrap_or(false);

    if !ok {
        return;
    }

    for i in (3..mesh.tangents.len()).step_by(4) {
        mesh.tangents[i] = -mesh.tangents[i];
    }

    g.tangents = mesh.tangents;
}

/// updateBBox: explicit extents, or from the vertices, or zero.
fn finish_bbox(g: &mut Geometry, extents: Option<(Vector3, Vector3)>) {
    if let Some((min, max)) = extents {
        g.min = min;
        g.max = max;
        return;
    }

    if g.vertices.is_empty() {
        g.min = Vector3::ZERO;
        g.max = Vector3::ZERO;
        return;
    }

    let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

    // Box3.setArray: `if (x < minX) minX = x` … (a NaN coordinate is skipped).
    for v in g.vertices.chunks_exact(4) {
        let (x, y, z) = (v[0] as f64, v[1] as f64, v[2] as f64);
        if x < min.x {
            min.x = x;
        }
        if y < min.y {
            min.y = y;
        }
        if z < min.z {
            min.z = z;
        }
        if x > max.x {
            max.x = x;
        }
        if y > max.y {
            max.y = y;
        }
        if z > max.z {
            max.z = z;
        }
    }

    // updateBBox: bbox .setArray (vertices, 4) .getExtents (min, max) goes through Box3's half
    // size and center (setExtents), so min = -half + center and max = half + center.
    let half = |a: f64, b: f64| (b - a) / 2.0;
    let center = |a: f64, b: f64| (b + a) / 2.0;
    let (sx, sy, sz) = (half(min.x, max.x), half(min.y, max.y), half(min.z, max.z));
    let (cx, cy, cz) = (
        center(min.x, max.x),
        center(min.y, max.y),
        center(min.z, max.z),
    );

    g.min = Vector3::new(-sx + cx, -sy + cy, -sz + cz);
    g.max = Vector3::new(sx + cx, sy + cy, sz + cz);
}

/// X3DGeometryNode.generateTexCoords with getTexCoordParams.
fn generate_tex_coords(g: &mut Geometry) {
    let size = g.bbox_size();
    let (xs, ys, zs) = (size.x, size.y, size.z);
    let mut min = g.bbox_center();
    let mut half = size;
    half.divide(2.0);
    min.subtract(&half);

    let (ssize, sindex, tindex) = if xs >= ys && xs >= zs {
        (xs, 0, if ys >= zs { 1 } else { 2 })
    } else if ys >= xs && ys >= zs {
        (ys, 1, if xs >= zs { 0 } else { 2 })
    } else {
        (zs, 2, if xs >= ys { 0 } else { 1 })
    };

    let s = min.get(sindex);
    let t = min.get(tindex);

    let mut tc = Vec::with_capacity(g.vertices.len());

    for v in g.vertices.chunks_exact(4) {
        tc.push(((v[sindex] as f64 - s) / ssize) as f32);
        tc.push(((v[tindex] as f64 - t) / ssize) as f32);
        tc.push(0.0);
        tc.push(1.0);
    }

    g.tex_coords = tc;
}

// ---------------------------------------------------------------------------------------------
// Primitives (Browser/Geometry3D/*Options.js, Components/Geometry3D/*.js).

/// X3DGeometry3DContext.setPrimitiveQuality3D: (cone and cylinder dimension, sphere xDimension,
/// sphere yDimension) for the PrimitiveQuality option.
fn dimensions_3d() -> (usize, usize, usize) {
    match crate::traverse::primitive_quality() {
        crate::traverse::Quality::Low => (16, 20, 9),
        crate::traverse::Quality::Medium => (20, 32, 15),
        crate::traverse::Quality::High => (32, 64, 31),
    }
}

/// Complex.fromPolar (magnitude, angle) → (real, imag).
fn polar(m: f64, angle: f64) -> (f64, f64) {
    (m * jsmath::cos(angle), m * jsmath::sin(angle))
}

fn f32v(v: &[f64]) -> Vec<f32> {
    v.iter().map(|x| *x as f32).collect()
}

/// BoxOptions.getGeometry.
pub fn box_options() -> Geometry {
    let tex_coord_index = [
        0, 1, 2, 3, -1, 0, 1, 2, 3, -1, 0, 1, 2, 3, -1, 0, 1, 2, 3, -1, 0, 1, 2, 3, -1, 0, 1, 2, 3,
        -1,
    ];
    let coord_index = [
        0, 1, 2, 3, -1, 5, 4, 7, 6, -1, 1, 5, 6, 2, -1, 4, 0, 3, 7, -1, 4, 5, 1, 0, -1, 3, 2, 6, 7,
        -1,
    ];
    let tex_coord = [1.0f32, 1.0, 0.0, 1.0, 0.0, 0.0, 1.0, 0.0];
    let coord = [
        1.0f32, 1.0, 1.0, -1.0, 1.0, 1.0, -1.0, -1.0, 1.0, 1.0, -1.0, 1.0, 1.0, 1.0, -1.0, -1.0,
        1.0, -1.0, -1.0, -1.0, -1.0, 1.0, -1.0, -1.0,
    ];
    let mut input = IfsInput::new(&coord, &coord_index);
    input.tex_coord = Some(&tex_coord);
    input.tex_coord_index = &tex_coord_index;
    indexed_face_set(&input)
}

/// Box.build.
pub fn make_box(size: Vector3, solid: bool) -> Geometry {
    let options = box_options();
    let mut g = Geometry {
        geometry_type: 3,
        solid,
        ccw: true,
        ..Default::default()
    };

    g.tex_coords = options.tex_coords.clone();
    g.tangents = options.tangents.clone();
    g.normals = options.normals.clone();

    let (min, max);

    if size.equals(&Vector3::new(2.0, 2.0, 2.0)) {
        g.vertices = options.vertices.clone();
        min = options.min;
        max = options.max;
    } else {
        let (x, y, z) = (
            (size.x / 2.0).abs(),
            (size.y / 2.0).abs(),
            (size.z / 2.0).abs(),
        );

        for v in options.vertices.chunks_exact(4) {
            g.vertices.extend_from_slice(&[
                (x * v[0] as f64) as f32,
                (y * v[1] as f64) as f32,
                (z * v[2] as f64) as f32,
                1.0,
            ]);
        }

        min = Vector3::new(-x, -y, -z);
        max = Vector3::new(x, y, z);
    }

    finish_bbox(&mut g, Some((min, max)));
    g
}

/// QuadSphereOptions.build.
pub fn sphere_options() -> Geometry {
    let (_, x, y) = dimensions_3d();
    let x_dimension = x + 1;
    let y_dimension = y;
    let pi = std::f64::consts::PI;

    // createTexCoordIndex
    let mut tci: Vec<i32> = vec![];
    for u in 0..x_dimension - 1 {
        tci.extend_from_slice(&[
            u as i32,
            (u + x_dimension - 1) as i32,
            (u + x_dimension) as i32,
            -1,
        ]);
    }
    let mut p = x_dimension - 1;
    for _v in 0..y_dimension - 3 {
        for _u in 0..x_dimension - 1 {
            tci.extend_from_slice(&[
                p as i32,
                (p + x_dimension) as i32,
                (p + x_dimension + 1) as i32,
                (p + 1) as i32,
                -1,
            ]);
            p += 1;
        }
        p += 1;
    }
    let mut p = (y_dimension - 2) * x_dimension - 1;
    for _u in 0..x_dimension - 1 {
        tci.extend_from_slice(&[(p + x_dimension) as i32, (p + 1) as i32, p as i32, -1]);
        p += 1;
    }

    // createTexCoord
    let mut tc: Vec<f64> = vec![];
    let pole_offset = -0.5 / (x_dimension - 1) as f64;
    for u in 1..x_dimension {
        tc.extend_from_slice(&[u as f64 / (x_dimension - 1) as f64 + pole_offset, 1.0]);
    }
    for v in 1..y_dimension - 1 {
        let y = 1.0 - v as f64 / (y_dimension - 1) as f64;
        for u in 0..x_dimension {
            tc.extend_from_slice(&[u as f64 / (x_dimension - 1) as f64, y]);
        }
    }
    for u in 1..x_dimension {
        tc.extend_from_slice(&[u as f64 / (x_dimension - 1) as f64 + pole_offset, 0.0]);
    }

    // createCoordIndex
    let mut ci: Vec<i32> = vec![];
    let mut u = 1;
    while u < x_dimension - 1 {
        ci.extend_from_slice(&[0, u as i32, (u + 1) as i32, -1]);
        u += 1;
    }
    ci.extend_from_slice(&[0, u as i32, 1, -1]);
    let mut p = 1usize;
    for _v in 0..y_dimension - 3 {
        for _u in 0..x_dimension - 2 {
            ci.extend_from_slice(&[
                p as i32,
                (p + x_dimension - 1) as i32,
                (p + x_dimension) as i32,
                (p + 1) as i32,
                -1,
            ]);
            p += 1;
        }
        ci.extend_from_slice(&[
            p as i32,
            (p + x_dimension - 1) as i32,
            (p + 1) as i32,
            (p + 2 - x_dimension) as i32,
            -1,
        ]);
        p += 1;
    }
    let last = p + x_dimension - 1;
    for _u in 0..x_dimension - 2 {
        ci.extend_from_slice(&[last as i32, (p + 1) as i32, p as i32, -1]);
        p += 1;
    }
    ci.extend_from_slice(&[last as i32, (last - x_dimension + 1) as i32, p as i32, -1]);

    // createPoints
    let mut pts: Vec<f64> = vec![0.0, 1.0, 0.0];
    let v_length = y_dimension - 1;
    for v in 1..v_length {
        let z_plane = polar(1.0, -pi * v as f64 / v_length as f64);
        let u_length = x_dimension - 1;
        for u in 0..u_length {
            let y_plane = polar(z_plane.1, 2.0 * pi * u as f64 / u_length as f64);
            pts.extend_from_slice(&[y_plane.1, z_plane.0, y_plane.0]);
        }
    }
    pts.extend_from_slice(&[0.0, -1.0, 0.0]);

    let (tc, pts) = (f32v(&tc), f32v(&pts));
    let mut input = IfsInput::new(&pts, &ci);
    input.tex_coord = Some(&tc);
    input.tex_coord_index = &tci;
    input.crease_angle = pi;
    indexed_face_set(&input)
}

/// Sphere.build.
pub fn make_sphere(radius: f64, solid: bool) -> Geometry {
    let options = sphere_options();
    let radius = radius.abs();
    let mut g = Geometry {
        geometry_type: 3,
        solid,
        ccw: true,
        ..Default::default()
    };

    g.tex_coords = options.tex_coords.clone();
    g.normals = options.normals.clone();
    g.tangents = options.tangents.clone();

    if radius == 1.0 {
        g.vertices = options.vertices.clone();
    } else {
        for v in options.vertices.chunks_exact(4) {
            g.vertices.extend_from_slice(&[
                (radius * v[0] as f64) as f32,
                (radius * v[1] as f64) as f32,
                (radius * v[2] as f64) as f32,
                1.0,
            ]);
        }
    }

    finish_bbox(
        &mut g,
        Some((
            Vector3::new(-radius, -radius, -radius),
            Vector3::new(radius, radius, radius),
        )),
    );
    g
}

/// ConeOptions.build → (side, bottom).
pub fn cone_options() -> (Geometry, Geometry) {
    let d = dimensions_3d().0;
    let pi = std::f64::consts::PI;

    let mut side_tci: Vec<i32> = vec![];
    for i in 0..d {
        side_tci.extend_from_slice(&[i as i32, (i + 1) as i32, (i + d + 1) as i32, -1]);
    }
    let mut bottom_tci: Vec<i32> = (0..d).rev().map(|i| (2 * (d + 1) + i) as i32).collect();
    bottom_tci.push(-1);

    let mut tc: Vec<f64> = vec![];
    for i in 0..d + 1 {
        tc.extend_from_slice(&[i as f64 / d as f64, 0.0]);
    }
    for i in 0..d + 1 {
        tc.extend_from_slice(&[(i as f64 + 0.5) / d as f64, 1.0]);
    }
    for i in 0..d {
        let t = polar(-1.0, 2.0 * pi * (i as f64 / d as f64));
        tc.extend_from_slice(&[(t.1 + 1.0) / 2.0, (t.0 + 1.0) / 2.0]);
    }

    let mut side_ni: Vec<i32> = vec![];
    for i in 0..d {
        side_ni.extend_from_slice(&[i as i32, ((i + 1) % d) as i32, (i + d) as i32, -1]);
    }
    let mut bottom_ni: Vec<i32> = vec![(2 * d) as i32; d];
    bottom_ni.push(-1);

    let mut nv: Vec<f64> = vec![];
    let nz = polar(1.0, -pi / 4.0);
    for i in 0..d {
        let n = polar(nz.1, 2.0 * pi * (i as f64 / d as f64));
        nv.extend_from_slice(&[n.1, nz.0, n.0]);
    }
    for i in 0..d {
        let n = polar(nz.1, 2.0 * pi * ((i as f64 + 0.5) / d as f64));
        nv.extend_from_slice(&[n.1, nz.0, n.0]);
    }
    nv.extend_from_slice(&[0.0, -1.0, 0.0]);

    let mut side_ci: Vec<i32> = vec![];
    for i in 0..d {
        side_ci.extend_from_slice(&[i as i32, ((i + 1) % d) as i32, d as i32, -1]);
    }
    let mut bottom_ci: Vec<i32> = (0..d).rev().map(|i| i as i32).collect();
    bottom_ci.push(-1);

    let mut pts: Vec<f64> = vec![];
    for i in 0..d {
        let p = polar(-1.0, 2.0 * pi * (i as f64 / d as f64));
        pts.extend_from_slice(&[p.1, -1.0, p.0]);
    }
    pts.extend_from_slice(&[0.0, 1.0, 0.0]);

    let (tc, nv, pts) = (f32v(&tc), f32v(&nv), f32v(&pts));

    let mut side = IfsInput::new(&pts, &side_ci);
    side.tex_coord = Some(&tc);
    side.tex_coord_index = &side_tci;
    side.normal = Some(&nv);
    side.normal_index = &side_ni;

    let mut bottom = side;
    bottom.coord_index = &bottom_ci;
    bottom.tex_coord_index = &bottom_tci;
    bottom.normal_index = &bottom_ni;

    (indexed_face_set(&side), indexed_face_set(&bottom))
}

/// Cone.build.
pub fn make_cone(
    height: f64,
    bottom_radius: f64,
    side: bool,
    bottom: bool,
    solid: bool,
) -> Geometry {
    let (side_g, bottom_g) = cone_options();
    let h = height.abs();
    let h1_2 = h / 2.0;
    let r = bottom_radius.abs();
    let mut g = Geometry {
        geometry_type: 3,
        solid,
        ccw: true,
        ..Default::default()
    };

    if side {
        g.tex_coords.extend_from_slice(&side_g.tex_coords);

        let mut rz = Rotation4::new(1.0, 0.0, 0.0, -(r / h).atan());

        for n in side_g.normals.chunks_exact(3) {
            let v1 = Vector3::new(n[0] as f64, 0.0, n[2] as f64);
            let mut rx = Rotation4::identity();
            rx.set_vectors(&Vector3::Z_AXIS, &v1).mult_left(&rz);
            let mut out = Vector3::new(0.0, 0.0, 1.0);
            rx.mult_vec_rot(&mut out);
            push3(&mut g.normals, &out);
        }

        let _ = &mut rz;

        for v in side_g.vertices.chunks_exact(4) {
            g.vertices.extend_from_slice(&[
                (r * v[0] as f64) as f32,
                (h1_2 * v[1] as f64) as f32,
                (r * v[2] as f64) as f32,
                1.0,
            ]);
        }
    }

    if bottom {
        g.tex_coords.extend_from_slice(&bottom_g.tex_coords);
        g.normals.extend_from_slice(&bottom_g.normals);

        for v in bottom_g.vertices.chunks_exact(4) {
            g.vertices.extend_from_slice(&[
                (r * v[0] as f64) as f32,
                (h1_2 * v[1] as f64) as f32,
                (r * v[2] as f64) as f32,
                1.0,
            ]);
        }
    }

    // setExtents uses the raw (not abs) field values.
    let y1 = height / 2.0;
    let y2 = -y1;
    let br = bottom_radius;
    let ext = if !side && !bottom {
        (Vector3::ZERO, Vector3::ZERO)
    } else if !side {
        (Vector3::new(-br, y2, -br), Vector3::new(br, y2, br))
    } else {
        (Vector3::new(-br, y2, -br), Vector3::new(br, y1, br))
    };

    finish_bbox(&mut g, Some(ext));
    // Cone.build copies no tangents: rebuild generates them for the cone itself.
    generate_tangents(&mut g);
    g
}

/// CylinderOptions.build → (side, top, bottom).
pub fn cylinder_options() -> (Geometry, Geometry, Geometry) {
    let d = dimensions_3d().0;
    let pi = std::f64::consts::PI;

    let mut side_tci: Vec<i32> = vec![];
    for i in 0..d {
        side_tci.extend_from_slice(&[
            i as i32,
            (i + 1) as i32,
            (i + 2 + d) as i32,
            (i + 1 + d) as i32,
            -1,
        ]);
    }
    let mut bottom_tci: Vec<i32> = (0..d).rev().map(|i| (2 * (d + 1) + i) as i32).collect();
    bottom_tci.push(-1);
    let mut top_tci: Vec<i32> = (0..d).map(|i| (2 * (d + 1) + d + i) as i32).collect();
    top_tci.push(-1);

    let mut tc: Vec<f64> = vec![];
    for i in 0..d + 1 {
        tc.extend_from_slice(&[i as f64 / d as f64, 0.0]);
    }
    for i in 0..d + 1 {
        tc.extend_from_slice(&[i as f64 / d as f64, 1.0]);
    }
    for i in 0..d {
        let t = polar(-1.0, 2.0 * pi * (i as f64 / d as f64));
        tc.extend_from_slice(&[(t.1 + 1.0) / 2.0, (t.0 + 1.0) / 2.0]);
    }
    for i in 0..d {
        let t = polar(1.0, 2.0 * pi * (i as f64 / d as f64));
        tc.extend_from_slice(&[1.0 - (t.1 + 1.0) / 2.0, (t.0 + 1.0) / 2.0]);
    }

    let mut side_ni: Vec<i32> = vec![];
    for i in 0..d {
        side_ni.extend_from_slice(&[
            i as i32,
            ((i + 1) % d) as i32,
            ((i + 1) % d) as i32,
            i as i32,
            -1,
        ]);
    }
    let mut bottom_ni: Vec<i32> = vec![d as i32; d];
    bottom_ni.push(-1);
    let mut top_ni: Vec<i32> = vec![(d + 1) as i32; d];
    top_ni.push(-1);

    let mut nv: Vec<f64> = vec![];
    for i in 0..d {
        let n = polar(-1.0, 2.0 * pi * (i as f64 / d as f64));
        nv.extend_from_slice(&[n.1, 0.0, n.0]);
    }
    nv.extend_from_slice(&[0.0, -1.0, 0.0, 0.0, 1.0, 0.0]);

    let mut side_ci: Vec<i32> = vec![];
    for i in 0..d {
        side_ci.extend_from_slice(&[
            i as i32,
            ((i + 1) % d) as i32,
            ((i + 1) % d + d) as i32,
            (i + d) as i32,
            -1,
        ]);
    }
    let mut bottom_ci: Vec<i32> = (0..d).rev().map(|i| i as i32).collect();
    bottom_ci.push(-1);
    let mut top_ci: Vec<i32> = (0..d).map(|i| (i + d) as i32).collect();
    top_ci.push(-1);

    let mut pts: Vec<f64> = vec![];
    for y in [-1.0, 1.0] {
        for i in 0..d {
            let p = polar(-1.0, 2.0 * pi * (i as f64 / d as f64));
            pts.extend_from_slice(&[p.1, y, p.0]);
        }
    }

    let (tc, nv, pts) = (f32v(&tc), f32v(&nv), f32v(&pts));

    let mut side = IfsInput::new(&pts, &side_ci);
    side.tex_coord = Some(&tc);
    side.tex_coord_index = &side_tci;
    side.normal = Some(&nv);
    side.normal_index = &side_ni;

    let mut top = side;
    top.coord_index = &top_ci;
    top.tex_coord_index = &top_tci;
    top.normal_index = &top_ni;

    let mut bottom = side;
    bottom.coord_index = &bottom_ci;
    bottom.tex_coord_index = &bottom_tci;
    bottom.normal_index = &bottom_ni;

    (
        indexed_face_set(&side),
        indexed_face_set(&top),
        indexed_face_set(&bottom),
    )
}

/// Cylinder.build.
pub fn make_cylinder(
    height: f64,
    radius: f64,
    side: bool,
    top: bool,
    bottom: bool,
    solid: bool,
) -> Geometry {
    let (side_g, top_g, bottom_g) = cylinder_options();
    let h1_2 = height.abs() / 2.0;
    let r = radius.abs();
    let mut g = Geometry {
        geometry_type: 3,
        solid,
        ccw: true,
        ..Default::default()
    };

    for (on, part) in [(side, &side_g), (top, &top_g), (bottom, &bottom_g)] {
        if !on {
            continue;
        }

        g.tex_coords.extend_from_slice(&part.tex_coords);
        g.tangents.extend_from_slice(&part.tangents);
        g.normals.extend_from_slice(&part.normals);

        for v in part.vertices.chunks_exact(4) {
            g.vertices.extend_from_slice(&[
                (r * v[0] as f64) as f32,
                (h1_2 * v[1] as f64) as f32,
                (r * v[2] as f64) as f32,
                1.0,
            ]);
        }
    }

    let y1 = height / 2.0;
    let y2 = -y1;
    let ext = if !top && !side && !bottom {
        (Vector3::ZERO, Vector3::ZERO)
    } else if !top && !side {
        (
            Vector3::new(-radius, y2, -radius),
            Vector3::new(radius, y2, radius),
        )
    } else if !bottom && !side {
        (
            Vector3::new(-radius, y1, -radius),
            Vector3::new(radius, y1, radius),
        )
    } else {
        (
            Vector3::new(-radius, y2, -radius),
            Vector3::new(radius, y1, radius),
        )
    };

    finish_bbox(&mut g, Some(ext));
    g
}

// ---------------------------------------------------------------------------------------------
// Lines and points.

/// IndexedLineSet.build.
/// `normal`: the Normal node's vectors, indexed by the coord index (IndexedLineSet.build: `normalNode ?.addVector (index, normalArray)`;
/// normalIndex and normalPerVertex are not read).
pub fn indexed_line_set(
    coord: Option<&[f32]>,
    coord_index: &[i32],
    color: Option<Colors>,
    color_index: &[i32],
    color_per_vertex: bool,
    normal: Option<&[f32]>,
    extras: &crate::extras::Extras,
) -> Geometry {
    let mut g = Geometry {
        geometry_type: 1,
        solid: false,
        ccw: true,
        ..Default::default()
    };
    extras.init(&mut g);

    if let Some(points) = coord.filter(|c| c.len() >= 3) {
        let coord = Coords(points);

        // getPolylineIndices
        let mut polylines: Vec<Vec<usize>> = vec![];
        let mut polyline = vec![];

        for (i, &index) in coord_index.iter().enumerate() {
            if index >= 0 {
                polyline.push(i);
            } else {
                polylines.push(std::mem::take(&mut polyline));
            }
        }

        if coord_index.last().is_some_and(|v| *v >= 0) {
            polylines.push(polyline);
        }

        for (face, polyline) in polylines.iter().enumerate() {
            if polyline.len() > 1 {
                for line in 0..polyline.len() - 1 {
                    for &i in &polyline[line..line + 2] {
                        let index = coord_index[i] as i64;

                        g.coord_indices.push(index as u32);
                        extras.add_attribs_and_fog(index, &mut g);

                        if let Some(c) = &color {
                            let ci = if color_per_vertex {
                                at_or(color_index, i, index)
                            } else {
                                at_or(color_index, face, face as i64)
                            };
                            c.add_color(ci, &mut g.colors);
                        }

                        if let Some(nv) = normal {
                            add_vector(nv, index, &mut g.normals);
                        }

                        coord.add_point(index, &mut g.vertices);
                    }
                }
            }
        }
    }

    // X3DLineGeometryNode has no multiTexCoords.
    finish(&mut g, true);
    g
}

/// PointSet.build.
/// `normal`: the Normal node's vectors (PointSet.build: `normalNode ?.addVectors (normalArray, numPoints)`).
pub fn point_set(coord: Option<&[f32]>, color: Option<Colors>, normal: Option<&[f32]>) -> Geometry {
    let mut g = Geometry {
        geometry_type: 0,
        solid: false,
        ccw: true,
        ..Default::default()
    };

    if let Some(points) = coord.filter(|c| c.len() >= 3) {
        let n = points.len() / 3;

        if let Some(c) = &color {
            c.add_colors(&mut g.colors, n);
        }

        // Normal.addVectors: every point's vector, or zeros for an empty Normal node.
        if let Some(nv) = normal {
            for i in 0..n {
                add_vector(
                    nv,
                    if nv.len() >= 3 { i as i64 } else { -1 },
                    &mut g.normals,
                );
            }
        }

        for (i, p) in points.chunks_exact(3).enumerate() {
            g.coord_indices.push(i as u32);
            g.vertices.extend_from_slice(&[p[0], p[1], p[2], 1.0]);
        }
    }

    // X3DPointGeometryNode has no multiTexCoords.
    finish(&mut g, true);
    g
}

// ---------------------------------------------------------------------------------------------
// ElevationGrid (Components/Geometry3D/ElevationGrid.js).

pub struct ElevationInput<'a> {
    pub x_dimension: i32,
    pub z_dimension: i32,
    pub x_spacing: f64,
    pub z_spacing: f64,
    pub height: &'a [f32],
    pub color: Option<Colors<'a>>,
    pub tex_coord: Option<&'a [f32]>,
    pub normal: Option<&'a [f32]>,
    pub ccw: bool,
    pub solid: bool,
    pub crease_angle: f64,
    pub color_per_vertex: bool,
    pub normal_per_vertex: bool,
    pub extras: crate::extras::Extras<'a>,
    /// GeoElevationGrid: the grid points (createPoints through getCoord); its triangles wind the
    /// other way (p1 p3 p2, p1 p4 p3).
    pub geo_points: Option<Vec<Vector3>>,
}

/// ElevationGrid.build (also GeoElevationGrid.build).
pub fn elevation_grid(e: &ElevationInput) -> Geometry {
    let mut g = Geometry {
        geometry_type: 3,
        solid: e.solid,
        ccw: e.ccw,
        ..Default::default()
    };

    if e.x_dimension < 2 || e.z_dimension < 2 {
        finish(&mut g, true);
        return g;
    }

    let (xd, zd) = (e.x_dimension as usize, e.z_dimension as usize);
    let height = |i: usize| -> f64 { e.height.get(i).map(|h| *h as f64).unwrap_or(0.0) };

    // createPoints
    let points = match &e.geo_points {
        Some(p) => p.clone(),
        None => {
            let mut points = Vec::with_capacity(xd * zd);
            for z in 0..zd {
                for x in 0..xd {
                    points.push(Vector3::new(
                        e.x_spacing * x as f64,
                        height(x + z * xd),
                        e.z_spacing * z as f64,
                    ));
                }
            }
            points
        }
    };

    // createCoordIndex: two triangles per quad.
    let mut coord_index: Vec<usize> = vec![];
    for z in 0..zd - 1 {
        for x in 0..xd - 1 {
            let i1 = z * xd + x;
            let i2 = (z + 1) * xd + x;
            let i3 = (z + 1) * xd + (x + 1);
            let i4 = z * xd + (x + 1);
            if e.geo_points.is_some() {
                coord_index.extend_from_slice(&[i1, i3, i2, i1, i4, i3]);
            } else {
                coord_index.extend_from_slice(&[i1, i2, i3, i1, i3, i4]);
            }
        }
    }

    // createTexCoord (default)
    let x_size = (xd - 1) as f64;
    let z_size = (zd - 1) as f64;

    // createNormals
    let cw = !e.ccw;
    let mut normal_index = NormalIndex::default();
    let mut normals: Vec<Vector3> = vec![];

    for (i, tri) in coord_index.chunks_exact(3).enumerate() {
        let _ = i;
        let mut n = triangle_normal(&points[tri[0]], &points[tri[1]], &points[tri[2]]);
        if cw {
            n.negate();
        }
        for &p in tri {
            normal_index.push(p as i64, normals.len());
            normals.push(n);
        }
    }

    let normals = if e.normal_per_vertex {
        refine_normals(&normal_index.order, &normals, e.crease_angle)
    } else {
        normals
    };

    let extras = &e.extras;

    extras.init(&mut g);

    for (k, &index) in coord_index.iter().enumerate() {
        let face = k / 6;

        extras.add_attribs_and_fog(index as i64, &mut g);

        if let Some(c) = &e.color {
            c.add_color(
                if e.color_per_vertex {
                    index as i64
                } else {
                    face as i64
                },
                &mut g.colors,
            );
        }

        extras.add_tangent(
            if e.normal_per_vertex {
                index as i64
            } else {
                face as i64
            },
            &mut g,
        );

        match e.tex_coord {
            _ if extras.tex.is_some() => extras.add_tex(index as i64, &mut g),
            Some(t) => add_tex_point(t, index as i64, &mut g.tex_coords),
            None => {
                let x = (index % xd) as f64;
                let z = (index / xd) as f64;
                g.tex_coords.extend_from_slice(&[
                    (x / x_size) as f32,
                    (z / z_size) as f32,
                    0.0,
                    1.0,
                ]);
            }
        }

        match e.normal {
            Some(n) => add_vector(
                n,
                if e.normal_per_vertex {
                    index as i64
                } else {
                    face as i64
                },
                &mut g.normals,
            ),
            None => push3(&mut g.normals, &normals[k]),
        }

        let p = points[index];
        g.vertices
            .extend_from_slice(&[p.x as f32, p.y as f32, p.z as f32, 1.0]);
    }

    finish(&mut g, extras.tex.is_none() || extras.channels() > 0);
    g
}

// ---------------------------------------------------------------------------------------------
// Extrusion (Components/Geometry3D/Extrusion.js).

pub struct ExtrusionInput<'a> {
    pub cross_section: &'a [f32],
    pub orientation: &'a [f64],
    pub scale: &'a [f32],
    pub spine: &'a [f32],
    pub begin_cap: bool,
    pub end_cap: bool,
    pub solid: bool,
    pub ccw: bool,
    pub convex: bool,
    pub crease_angle: f64,
}

fn v2(a: &[f32], i: usize) -> Vector2 {
    Vector2::new(a[2 * i] as f64, a[2 * i + 1] as f64)
}

fn v3(a: &[f32], i: usize) -> Vector3 {
    Vector3::new(a[3 * i] as f64, a[3 * i + 1] as f64, a[3 * i + 2] as f64)
}

fn rot(a: &[f64], i: usize) -> Rotation4 {
    Rotation4::new(a[4 * i], a[4 * i + 1], a[4 * i + 2], a[4 * i + 3])
}

fn closed2(a: &[f32]) -> bool {
    let n = a.len() / 2;
    n == 0 || v2(a, 0).equals(&v2(a, n - 1))
}

fn closed3(a: &[f32]) -> bool {
    let n = a.len() / 3;
    n == 0 || v3(a, 0).equals(&v3(a, n - 1))
}

fn closed_rot(a: &[f64]) -> bool {
    let n = a.len() / 4;
    n == 0 || rot(a, 0).equals(&rot(a, n - 1))
}

fn set_rows(m: &mut Matrix4, x: &Vector3, y: &Vector3, z: &Vector3, s: &Vector3) {
    m.0 = [
        x.x, x.y, x.z, 0.0, y.x, y.y, y.z, 0.0, z.x, z.y, z.z, 0.0, s.x, s.y, s.z, 1.0,
    ];
}

/// Extrusion.createRotations.
fn extrusion_rotations(spine: &[f32]) -> Vec<Matrix4> {
    let num_spines = spine.len() / 3;
    let sp = |i: usize| v3(spine, i);
    let closed_spine = closed3(spine);
    let mut rotations = vec![Matrix4::IDENTITY; num_spines];
    let mut x_axis;
    let mut y_axis = Vector3::ZERO;
    let mut z_axis = Vector3::ZERO;

    if closed_spine {
        let s = sp(0);
        let length = num_spines.saturating_sub(2);

        for i in 1..length {
            let mut a = sp(i);
            a.subtract(&s).normalize();
            let mut b = sp(length);
            b.subtract(&s).normalize();
            a.subtract(&b).normalize();
            y_axis = a;
            if !y_axis.equals(&Vector3::ZERO) {
                break;
            }
        }

        for i in 0..length {
            let mut a = sp(i + 1);
            a.subtract(&sp(i));
            let mut b = sp(length);
            b.subtract(&sp(i));
            a.cross(&b).normalize();
            z_axis = a;
            if !z_axis.equals(&Vector3::ZERO) {
                break;
            }
        }
    } else {
        for i in 0..num_spines - 1 {
            let mut a = sp(i + 1);
            a.subtract(&sp(i)).normalize();
            y_axis = a;
            if !y_axis.equals(&Vector3::ZERO) {
                break;
            }
        }

        for i in 1..num_spines.saturating_sub(1) {
            let mut a = sp(i + 1);
            a.subtract(&sp(i));
            let mut b = sp(i - 1);
            b.subtract(&sp(i));
            a.cross(&b).normalize();
            z_axis = a;
            if !z_axis.equals(&Vector3::ZERO) {
                break;
            }
        }
    }

    if y_axis.equals(&Vector3::ZERO) {
        y_axis = Vector3::new(0.0, 1.0, 0.0);
    }

    if z_axis.equals(&Vector3::ZERO) {
        let mut r = Rotation4::identity();
        r.set_vectors(&Vector3::Y_AXIS, &y_axis);
        z_axis = Vector3::Z_AXIS;
        r.mult_vec_rot(&mut z_axis);
    }

    x_axis = y_axis;
    x_axis.cross(&z_axis);
    set_rows(&mut rotations[0], &x_axis, &y_axis, &z_axis, &sp(0));

    let mut y_prev = y_axis;
    let mut z_prev = z_axis;

    for i in 1..num_spines.saturating_sub(1) {
        let s = sp(i);
        let mut a = sp(i + 1);
        a.subtract(&s).normalize();
        let mut b = sp(i - 1);
        b.subtract(&s).normalize();
        a.subtract(&b).normalize();
        y_axis = a;

        let mut a = sp(i + 1);
        a.subtract(&s);
        let mut b = sp(i - 1);
        b.subtract(&s);
        a.cross(&b).normalize();
        z_axis = a;

        if y_prev.dot(&y_axis) < 0.0 {
            y_axis.negate();
        }
        if z_prev.dot(&z_axis) < 0.0 {
            z_axis.negate();
        }

        if y_axis.equals(&Vector3::ZERO) {
            y_axis = y_prev;
        } else {
            y_prev = y_axis;
        }

        if z_axis.equals(&Vector3::ZERO) {
            z_axis = z_prev;
        } else {
            z_prev = z_axis;
        }

        x_axis = y_axis;
        x_axis.cross(&z_axis);
        set_rows(&mut rotations[i], &x_axis, &y_axis, &z_axis, &s);
    }

    if closed_spine {
        rotations[num_spines - 1] = rotations[0];
    } else {
        let s = sp(num_spines - 1);
        let mut a = s;
        a.subtract(&sp(num_spines - 2)).normalize();
        y_axis = a;

        if num_spines > 2 {
            let mut a = s;
            a.subtract(&sp(num_spines - 2));
            let mut b = sp(num_spines - 3);
            b.subtract(&sp(num_spines - 2));
            a.cross(&b).normalize();
            z_axis = a;
        }

        if y_prev.dot(&y_axis) < 0.0 {
            y_axis.negate();
        }
        if z_prev.dot(&z_axis) < 0.0 {
            z_axis.negate();
        }
        if y_axis.equals(&Vector3::ZERO) {
            y_axis = y_prev;
        }
        if z_axis.equals(&Vector3::ZERO) {
            z_axis = z_prev;
        }

        x_axis = y_axis;
        x_axis.cross(&z_axis);
        set_rows(
            &mut rotations[num_spines - 1],
            &x_axis,
            &y_axis,
            &z_axis,
            &s,
        );
    }

    rotations
}

/// Extrusion.build.
pub fn extrusion(e: &ExtrusionInput) -> Geometry {
    let mut g = Geometry {
        geometry_type: 3,
        solid: e.solid,
        ccw: e.ccw,
        ..Default::default()
    };
    let cw = !e.ccw;
    let num_cross = e.cross_section.len() / 2;
    let num_spines = e.spine.len() / 3;

    if num_spines < 2 || num_cross < 2 {
        finish(&mut g, true);
        return g;
    }

    let index = |n: usize, k: usize| n * num_cross + k;
    let closed = closed3(e.spine) && closed_rot(e.orientation) && closed2(e.scale);
    let closed_cross = closed2(e.cross_section);

    let mut min = v2(e.cross_section, 0);
    let mut max = min;
    for k in 1..num_cross {
        min.min(&v2(e.cross_section, k));
        max.max(&v2(e.cross_section, k));
    }
    let mut cap_size = max;
    cap_size.subtract(&min);
    let cap_max = jsmath::js_max(cap_size.x, cap_size.y);
    let num_cap_points = if closed_cross {
        num_cross - 1
    } else {
        num_cross
    };

    // createPoints
    let mut rotations = extrusion_rotations(e.spine);
    let num_orient = e.orientation.len() / 4;
    let num_scales = e.scale.len() / 2;
    let mut points: Vec<Vector3> = Vec::with_capacity(num_spines * num_cross);

    for (i, matrix) in rotations.iter_mut().enumerate() {
        if num_orient > 0 {
            matrix.rotate(&rot(e.orientation, i.min(num_orient - 1)));
        }
        if num_scales > 0 {
            let s = v2(e.scale, i.min(num_scales - 1));
            matrix.scale(&Vector3::new(s.x, 1.0, s.y));
        }
        for cs in 0..num_cross {
            let v = v2(e.cross_section, cs);
            let mut p = Vector3::new(v.x, 0.0, v.y);
            matrix.mult_vec_matrix3(&mut p);
            points.push(p);
        }
    }

    let mut normal_index = NormalIndex::default();
    for p in 0..points.len() {
        normal_index.push(p as i64, usize::MAX);
        normal_index.order[p].1.clear();
    }
    let mut normals: Vec<Vector3> = vec![];

    let nc1 = num_cross - 1;
    let ns1 = num_spines - 1;
    let mut index_left = index(0, 0);
    let mut index_right = index(0, if closed_cross { 0 } else { nc1 });
    let tex =
        |a: f64, b: f64, t: &mut Vec<f32>| t.extend_from_slice(&[a as f32, b as f32, 0.0, 1.0]);

    for n in 0..ns1 {
        for k in 0..nc1 {
            let n1 = if closed && n == num_spines - 2 {
                0
            } else {
                n + 1
            };
            let k1 = if closed_cross && k == num_cross - 2 {
                0
            } else {
                k + 1
            };

            let mut i1 = index(n, k);
            let i2 = index(n, k1);
            let mut i3 = index(n1, k1);
            let i4 = index(n1, k);
            let mut p1 = points[i1];
            let p2 = points[i2];
            let mut p3 = points[i3];
            let p4 = points[i4];
            let l1 = p2.distance(&p3) >= 1e-7;
            let l2 = p4.distance(&p1) >= 1e-7;

            let mut normal1 = triangle_normal(&p1, &p2, &p3);
            let mut normal2 = triangle_normal(&p1, &p3, &p4);

            if cw {
                normal1.negate();
                normal2.negate();
            }

            if k == 0 {
                if l2 {
                    index_left = i1;
                } else {
                    i1 = index_left;
                    p1 = points[i1];
                }
            }

            if k == num_cross - 2 {
                if l1 {
                    index_right = i2;
                } else {
                    i3 = index_right;
                    p3 = points[i3];
                }
            }

            let (nf, kf) = (n as f64, k as f64);
            let (ns1f, nc1f) = (ns1 as f64, nc1 as f64);

            if l1 {
                if l2 {
                    tex(kf / nc1f, nf / ns1f, &mut g.tex_coords);
                } else {
                    let y = (nf / ns1f + (nf + 1.0) / ns1f) / 2.0;
                    tex(kf / nc1f, y, &mut g.tex_coords);
                }
                normal_index.order[i1].1.push(normals.len());
                normals.push(normal1);
                g.vertices
                    .extend_from_slice(&[p1.x as f32, p1.y as f32, p1.z as f32, 1.0]);

                tex((kf + 1.0) / nc1f, nf / ns1f, &mut g.tex_coords);
                normal_index.order[i2].1.push(normals.len());
                normals.push(normal1);
                g.vertices
                    .extend_from_slice(&[p2.x as f32, p2.y as f32, p2.z as f32, 1.0]);

                tex((kf + 1.0) / nc1f, (nf + 1.0) / ns1f, &mut g.tex_coords);
                normal_index.order[i3].1.push(normals.len());
                normals.push(normal1);
                g.vertices
                    .extend_from_slice(&[p3.x as f32, p3.y as f32, p3.z as f32, 1.0]);
            }

            if l2 {
                tex(kf / nc1f, nf / ns1f, &mut g.tex_coords);
                normal_index.order[i1].1.push(normals.len());
                normals.push(normal2);
                g.vertices
                    .extend_from_slice(&[p1.x as f32, p1.y as f32, p1.z as f32, 1.0]);

                if l1 {
                    tex((kf + 1.0) / nc1f, (nf + 1.0) / ns1f, &mut g.tex_coords);
                } else {
                    let y = ((nf + 1.0) / ns1f + nf / ns1f) / 2.0;
                    tex((kf + 1.0) / nc1f, y, &mut g.tex_coords);
                }
                normal_index.order[i3].1.push(normals.len());
                normals.push(normal2);
                g.vertices
                    .extend_from_slice(&[p3.x as f32, p3.y as f32, p3.z as f32, 1.0]);

                tex(kf / nc1f, (nf + 1.0) / ns1f, &mut g.tex_coords);
                normal_index.order[i4].1.push(normals.len());
                normals.push(normal2);
                g.vertices
                    .extend_from_slice(&[p4.x as f32, p4.y as f32, p4.z as f32, 1.0]);
            }
        }
    }

    for n in refine_normals(&normal_index.order, &normals, e.crease_angle) {
        push3(&mut g.normals, &n);
    }

    if cap_max != 0.0 && num_cap_points > 2 {
        for (on, j, reverse) in [
            (e.begin_cap, 0usize, true),
            (e.end_cap, num_spines - 1, false),
        ] {
            if !on {
                continue;
            }

            let mut idx = vec![];
            let mut tcs = std::collections::HashMap::new();

            for k in 0..num_cap_points {
                let kk = if reverse { num_cap_points - 1 - k } else { k };
                let i = index(j, kk);
                let mut t = v2(e.cross_section, kk);
                t.subtract(&min).divide(cap_max);
                tcs.insert(i, t);
                idx.push(i);
            }

            let triangles: Vec<usize> = if e.convex {
                tess::triangulate_convex(&idx)
            } else {
                // Triangle3.triangulatePolygon (libtess, ODD) of the cap points with their indices, as IndexedFaceSet.
                let pts: Vec<([f64; 3], usize)> =
                    idx.iter().map(|&i| (points[i].to_array(), i)).collect();
                x3d_text::libtess::triangulate_indexed(&pts)
            };

            if triangles.len() >= 3 {
                let mut normal = triangle_normal(
                    &points[triangles[0]],
                    &points[triangles[1]],
                    &points[triangles[2]],
                );
                if cw {
                    normal.negate();
                }

                for tri in triangles.chunks_exact(3) {
                    for &i in tri {
                        let t = tcs[&i];
                        g.tex_coords
                            .extend_from_slice(&[t.x as f32, t.y as f32, 0.0, 1.0]);
                        push3(&mut g.normals, &normal);
                        let p = points[i];
                        g.vertices
                            .extend_from_slice(&[p.x as f32, p.y as f32, p.z as f32, 1.0]);
                    }
                }
            }
        }
    }

    finish(&mut g, true);
    g
}
