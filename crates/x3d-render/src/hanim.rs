//! Components/HAnim skinning (REA batch 04): HAnimHumanoid and HAnimSegment build the joint,
//! displacement and joint-matrix textures and X_ITE's vertex shader (Skin2.glsl) moves each vertex
//! by them. This port builds the same tables from the node fields and applies the same per-vertex
//! sums on the CPU to a copy of the geometry (vertices, normals, tangents). The tables:
//!
//! * HAnimHumanoid.set_jointTextures__: per skinCoord point index, the joints (indices into the
//!   humanoid's HAnimJoint `joints`) and weights with a weight other than 0, padded to a multiple of
//!   4 (X3D_NUM_JOINT_SETS × 4);
//! * set_displacementsTexture__: per point index, the displacements of the joints' HAnimDisplacer
//!   nodes with their joint and displacer index (X3D_NUM_DISPLACEMENTS);
//! * set_displacementWeightsTexture__: the displacer weights;
//! * skinning: per joint, binding × joint model-view × the inverse humanoid model-view, and the
//!   transposed inverse of its 3 × 3 part for normals.
//!
//! HAnimSegment with a coord and displacers: displacements only, joint 0 is the segment's
//! model-view × the inverse humanoid model-view.
//!
//! The bounding box stays the one of the unskinned geometry (X3DGeometryNode.getBBox).

use crate::geometry::Geometry;
use crate::sg::Sg;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};
use x3d_fields::{FieldValue, NodeId};

/// The joint, displacement and weight arrays of one HAnimHumanoid or HAnimSegment.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct SkinTables {
    /// skinCoord (segment: coord) point count, at least 1.
    pub length: usize,
    /// Joints per point index, a multiple of 4 (X3D_NUM_JOINT_SETS × 4).
    pub num_joints: usize,
    /// `length × num_joints × 2`: per point, the joint indices, then the weights.
    pub joints: Vec<f32>,
    /// Displacements per point index (X3D_NUM_DISPLACEMENTS).
    pub num_displacements: usize,
    /// `length × num_displacements × 8`: x y z joint, displacer 0 0 0.
    pub displacements: Vec<f32>,
    /// The weight of each displacer.
    pub displacement_weights: Vec<f32>,
}

/// X3DCast (Coordinate, node) ?? X3DCast (CoordinateDouble, node): the point count.
fn coord_length(sg: &Sg, coord: Option<NodeId>) -> Option<usize> {
    let c = coord.filter(|c| matches!(sg.type_name(*c), Some("Coordinate" | "CoordinateDouble")))?;
    Some(sg.f32s(c, "point").len() / 3)
}

/// HAnimHumanoid.set_joints__: the HAnimJoint nodes of `joints` and their binding matrices
/// (Matrix4.fromTransform of jointBindingPositions, Rotations, Scales at the same index of `joints`).
pub fn humanoid_joints(sg: &Sg, humanoid: NodeId) -> (Vec<NodeId>, Vec<Matrix4>) {
    let positions = sg.f32s(humanoid, "jointBindingPositions");
    let rotations = match sg.value(humanoid, "jointBindingRotations") {
        Some(FieldValue::MFRotation(v)) => v.clone(),
        _ => vec![],
    };
    let scales = sg.f32s(humanoid, "jointBindingScales");
    let (mut nodes, mut bindings) = (vec![], vec![]);

    for (i, c) in sg.raw_nodes(humanoid, "joints").into_iter().enumerate() {
        let Some(joint) = c.and_then(|c| sg.inner(c)).filter(|j| sg.type_name(*j) == Some("HAnimJoint")) else { continue };
        let v3 = |a: &[f32]| a.get(i * 3..i * 3 + 3).map(|p| Vector3::new(p[0] as f64, p[1] as f64, p[2] as f64));
        let t = v3(positions);
        let r = rotations.get(i * 4..i * 4 + 4).map(|r| Rotation4::new(r[0], r[1], r[2], r[3]));
        let s = v3(scales);
        let mut m = Matrix4::IDENTITY;
        m.set_transform(t.as_ref(), r.as_ref(), s.as_ref(), None, None);
        nodes.push(joint);
        bindings.push(m);
    }

    (nodes, bindings)
}

/// The HAnimDisplacer nodes of a `displacers` field (X3DCast).
fn displacers(sg: &Sg, n: NodeId) -> Vec<NodeId> {
    sg.nodes(n, "displacers").into_iter().filter(|d| sg.type_name(*d) == Some("HAnimDisplacer")).collect()
}

/// Adds the displacements of `displacers` (all of joint `joint`) to `per_point`.
fn add_displacements(sg: &Sg, displacer_nodes: &[NodeId], joint: usize, per_point: &mut [Vec<f32>], weights: &mut Vec<f32>) {
    for &d in displacer_nodes {
        let coord_index = sg.i32s(d, "coordIndex");

        if coord_index.is_empty() {
            continue;
        }

        let displacer = weights.len() as f32;
        weights.push(sg.f64(d, "weight", 0.0) as f32);

        let values = sg.f32s(d, "displacements");

        for (i, &index) in coord_index.iter().enumerate() {
            // `... _displacements [i]` of a missing value throws in X_ITE; such a point gets none.
            let (Some(slot), Some(v)) = (usize::try_from(index).ok().and_then(|k| per_point.get_mut(k)), values.get(i * 3..i * 3 + 3)) else { continue };
            slot.extend_from_slice(&[v[0], v[1], v[2], joint as f32, displacer, 0.0, 0.0, 0.0]);
        }
    }
}

fn pack(per_point: &[Vec<f32>], stride: usize) -> Vec<f32> {
    let mut out = vec![0.0; per_point.len() * stride];

    for (i, p) in per_point.iter().enumerate() {
        out[i * stride..i * stride + p.len()].copy_from_slice(p);
    }

    out
}

fn displacement_tables(per_point: &[Vec<f32>], tables: &mut SkinTables) {
    tables.num_displacements = per_point.iter().map(Vec::len).max().unwrap_or(0) / 8;
    tables.displacements = pack(per_point, tables.num_displacements * 8);
}

/// HAnimHumanoid set_jointTextures__, set_displacementsTexture__ and
/// set_displacementWeightsTexture__.
pub fn humanoid_tables(sg: &Sg, humanoid: NodeId, joint_nodes: &[NodeId]) -> SkinTables {
    let length = coord_length(sg, sg.node(humanoid, "skinCoord")).filter(|l| *l > 0).unwrap_or(1);
    let mut joints: Vec<Vec<f32>> = vec![vec![]; length];
    let mut weights: Vec<Vec<f32>> = vec![vec![]; length];

    for (j, &joint) in joint_nodes.iter().enumerate() {
        let skin_weight = sg.f32s(joint, "skinCoordWeight");

        for (i, &index) in sg.i32s(joint, "skinCoordIndex").iter().enumerate() {
            // An index past skinCoordWeight reads undefined: not 0, so NaN in the Float32Array.
            let weight = skin_weight.get(i).copied().unwrap_or(f32::NAN);

            if weight == 0.0 {
                continue;
            }

            if let Some(k) = usize::try_from(index).ok().filter(|k| *k < length) {
                joints[k].push(j as f32);
                weights[k].push(weight);
            }
        }
    }

    let num_joints = joints.iter().map(Vec::len).max().unwrap_or(0).div_ceil(4) * 4;
    let mut packed = vec![0.0; length * num_joints * 2];

    for i in 0..length {
        let base = i * num_joints * 2;
        packed[base..base + joints[i].len()].copy_from_slice(&joints[i]);
        packed[base + num_joints..base + num_joints + weights[i].len()].copy_from_slice(&weights[i]);
    }

    let mut tables = SkinTables { length, num_joints, joints: packed, ..Default::default() };
    let mut per_point: Vec<Vec<f32>> = vec![vec![]; length];

    for (j, &joint) in joint_nodes.iter().enumerate() {
        add_displacements(sg, &displacers(sg, joint), j, &mut per_point, &mut tables.displacement_weights);
    }

    displacement_tables(&per_point, &mut tables);
    tables
}

/// HAnimSegment set_displacementsTexture__ and set_displacementWeightsTexture__; `None` when the
/// segment does not skin (no Coordinate coord, or no displacement).
pub fn segment_tables(sg: &Sg, segment: NodeId) -> Option<SkinTables> {
    let length = coord_length(sg, sg.node(segment, "coord"))?.max(1);
    let mut tables = SkinTables { length, ..Default::default() };
    let mut per_point: Vec<Vec<f32>> = vec![vec![]; length];

    add_displacements(sg, &displacers(sg, segment), 0, &mut per_point, &mut tables.displacement_weights);
    displacement_tables(&per_point, &mut tables);
    (tables.num_displacements > 0).then_some(tables)
}

/// HAnimHumanoid.skinning: per joint the joint matrix (16) and the joint normal matrix (9, in a
/// 16 slot), as Float32 like the texture.
pub fn joint_matrices(bindings: &[Matrix4], joint_model_views: &[Matrix4], inv_humanoid: &Matrix4) -> Vec<f32> {
    let mut out = vec![0.0f32; bindings.len() * 32];

    for (i, (binding, mv)) in bindings.iter().zip(joint_model_views).enumerate() {
        let mut m = *mv;
        m.mult_right(inv_humanoid);
        m.mult_left(binding);

        let mut normal = m.submatrix();
        normal.transpose();
        normal.inverse();

        for (k, v) in m.0.iter().enumerate() {
            out[i * 32 + k] = *v as f32;
        }

        for (k, v) in normal.0.iter().enumerate() {
            out[i * 32 + 16 + k] = *v as f32;
        }
    }

    out
}

/// HAnimSegment.skinning: joint 0 is the segment's model-view × the inverse humanoid model-view.
pub fn segment_matrices(model_view: &Matrix4, inv_humanoid: &Matrix4) -> Vec<f32> {
    let mut m = *model_view;
    m.mult_right(inv_humanoid);

    let mut out = vec![0.0f32; 32];
    for (k, v) in m.0.iter().enumerate() {
        out[k] = *v as f32;
    }
    out
}

/// Skin2.glsl getSkinVertex (and skinNormal, skinTangent) for every vertex of `g`, with the
/// vertex's coord index (x3d_CoordIndex) into the tables. A texel past an array reads 0.
pub fn skin(g: &Geometry, tables: &SkinTables, matrices: &[f32]) -> Geometry {
    let mut out = g.clone();
    let texel = |a: &[f32], index: usize| -> [f64; 4] {
        let mut t = [0.0; 4];
        for (k, v) in t.iter_mut().enumerate() {
            *v = a.get(index * 4 + k).copied().unwrap_or(0.0) as f64;
        }
        t
    };
    let joint_matrix = |joint: usize| -> [f64; 16] {
        let mut m = [0.0; 16];
        for (k, v) in m.iter_mut().enumerate() {
            *v = matrices.get(joint * 32 + k).copied().unwrap_or(0.0) as f64;
        }
        m
    };
    let normal_matrix = |joint: usize| -> [f64; 9] {
        let mut m = [0.0; 9];
        for (k, v) in m.iter_mut().enumerate() {
            *v = matrices.get(joint * 32 + 16 + k).copied().unwrap_or(0.0) as f64;
        }
        m
    };
    // GLSL `mat * v` with the column-major array: the row vector v × M.
    let mul4 = |m: &[f64; 16], v: &[f64; 4]| -> [f64; 4] { std::array::from_fn(|i| (0..4).map(|c| m[c * 4 + i] * v[c]).sum()) };
    let mul3 = |m: &[f64; 9], v: &[f64; 3]| -> [f64; 3] { std::array::from_fn(|i| (0..3).map(|c| m[c * 3 + i] * v[c]).sum()) };
    // getDisplacementJointMatrix: mat3 (a .xyz, b .xyz, c .xyz) of the joint matrix.
    let mul_displacement = |m: &[f64; 16], v: &[f64; 3]| -> [f64; 3] { std::array::from_fn(|i| (0..3).map(|c| m[c * 4 + i] * v[c]).sum()) };
    let sets = tables.num_joints / 4;
    let has_normals = !g.normals.is_empty();
    let has_tangents = !g.tangents.is_empty();

    for k in 0..g.vertex_count() {
        let coord_index = g.coord_indices.get(k).copied().unwrap_or(0) as usize;
        let vertex = [g.vertices[k * 4] as f64, g.vertices[k * 4 + 1] as f64, g.vertices[k * 4 + 2] as f64, g.vertices[k * 4 + 3] as f64];
        let normal = if has_normals { [g.normals[k * 3] as f64, g.normals[k * 3 + 1] as f64, g.normals[k * 3 + 2] as f64] } else { [0.0; 3] };
        let tangent = if has_tangents { [g.tangents[k * 4] as f64, g.tangents[k * 4 + 1] as f64, g.tangents[k * 4 + 2] as f64] } else { [0.0; 3] };
        let mut skin = vertex;
        let mut skin_normal = normal;
        let mut skin_tangent = tangent;

        if tables.num_displacements > 0 {
            let base = coord_index * tables.num_displacements * 2;

            for i in 0..tables.num_displacements {
                let index = base + i * 2;
                let displacement = texel(&tables.displacements, index);
                let weight_index = texel(&tables.displacements, index + 1)[0] as usize;
                let weight = tables.displacement_weights.get(weight_index).copied().unwrap_or(0.0) as f64;
                let d = mul_displacement(&joint_matrix(displacement[3] as usize), &[displacement[0] * weight, displacement[1] * weight, displacement[2] * weight]);

                for c in 0..3 {
                    skin[c] += d[c];
                }
            }
        }

        if sets > 0 {
            let base = coord_index * sets * 2;

            for i in 0..sets {
                let joints = texel(&tables.joints, base + i);
                let weights = texel(&tables.joints, base + i + sets);

                for (joint, weight) in joints.iter().zip(weights) {
                    let joint = *joint as i32 as usize;
                    let m = joint_matrix(joint);
                    let v = mul4(&m, &vertex);

                    for c in 0..4 {
                        skin[c] += (v[c] - vertex[c]) * weight;
                    }

                    if has_normals {
                        let nm = normal_matrix(joint);
                        let n = mul3(&nm, &normal);

                        for c in 0..3 {
                            skin_normal[c] += (n[c] - normal[c]) * weight;
                        }

                        if has_tangents {
                            let t = mul3(&nm, &tangent);

                            for c in 0..3 {
                                skin_tangent[c] += (t[c] - tangent[c]) * weight;
                            }
                        }
                    }
                }
            }
        }

        for c in 0..4 {
            out.vertices[k * 4 + c] = skin[c] as f32;
        }

        if has_normals {
            for c in 0..3 {
                out.normals[k * 3 + c] = skin_normal[c] as f32;
            }
        }

        if has_tangents {
            for c in 0..3 {
                out.tangents[k * 4 + c] = skin_tangent[c] as f32;
            }
        }
    }

    out
}
