//! The drawing side of X_ITE's ParticleSystem (`Components/ParticleSystems/ParticleSystem.js`): the
//! geometry of each geometry type and the per-particle instance data the instanced draw reads from the
//! particle buffer. The simulation (the buffer itself) is x3d-runtime's [`x3d_runtime::particles`].
//!
//! X_ITE draws `numParticles` instances (`gl .drawArraysInstanced`) with the shape's shader and
//! `X3D_INSTANCING` (Instancing2.glsl): `x3d_InstanceMatrix` is the record's scale / rotation rows and
//! its position, `x3d_Color` the record's color when the ParticleSystem has a color ramp, and with a
//! texCoord ramp (`X3D_TEX_COORD_RAMP`) a quad vertex takes its texture coordinate from the ramp
//! texture at `x3d_Particle [3] + map [gl_VertexID]`. There is no `X3D_INSTANCE_NORMAL`: the instance
//! matrix does not transform normals. A hidden particle has a NaN position and draws nothing.

use crate::geometry::Geometry;
use x3d_fields::math::{Matrix4, Vector3};
use x3d_runtime::particles::{GeometryType, Particle, Ramps};

/// `QuadGeometry` texture coordinates (p1 p2 p3, p1 p3 p4).
const QUAD_TEX_COORDS: [[f32; 2]; 6] = [[0.0, 0.0], [1.0, 0.0], [1.0, 1.0], [0.0, 0.0], [1.0, 1.0], [0.0, 1.0]];

/// `QuadGeometry` vertices.
const QUAD: [[f64; 2]; 6] = [[-0.5, -0.5], [0.5, -0.5], [0.5, 0.5], [-0.5, -0.5], [0.5, 0.5], [-0.5, 0.5]];

/// The texCoord ramp's `map [gl_VertexID]`: the ramp entry of each quad vertex.
pub const QUAD_RAMP_MAP: [usize; 6] = [0, 1, 2, 0, 2, 3];

fn bounds(g: &mut Geometry) {
    let mut min = Vector3::new(f64::INFINITY, f64::INFINITY, f64::INFINITY);
    let mut max = Vector3::new(f64::NEG_INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY);

    for v in g.vertices.chunks_exact(4) {
        min = Vector3::new(min.x.min(v[0] as f64), min.y.min(v[1] as f64), min.z.min(v[2] as f64));
        max = Vector3::new(max.x.max(v[0] as f64), max.y.max(v[1] as f64), max.z.max(v[2] as f64));
    }

    g.min = min;
    g.max = max;
}

/// POINT: `PointGeometry`, one point at the origin (`geometryContext .geometryType` 0, no normals).
pub fn point_geometry(color_material: bool) -> Geometry {
    let mut g = Geometry { geometry_type: 0, vertices: vec![0.0, 0.0, 0.0, 1.0], solid: true, ccw: true, ..Default::default() };

    if color_material {
        g.colors = vec![1.0; 4];
    }

    bounds(&mut g);
    g
}

/// LINE: the private LineSet, `vertexCount [2]`, points `0 0 -size.y/2` and `0 0 size.y/2` (`set_particleSize__`).
pub fn line_geometry(particle_size_y: f64, color_material: bool) -> Geometry {
    let h = (particle_size_y / 2.0) as f32;
    let mut g = Geometry { geometry_type: 1, vertices: vec![0.0, 0.0, -h, 1.0, 0.0, 0.0, h, 1.0], solid: true, ccw: true, ..Default::default() };

    if color_material {
        g.colors = vec![1.0; 8];
    }

    bounds(&mut g);
    g
}

/// TRIANGLE and QUAD: `QuadGeometry` (two triangles of the unit square, normal 0 0 1). SPRITE: the same
/// quad turned by `rotation` (`getScreenAlignedRotation`, rows x y z) and scaled by `size`
/// (`updateSprite`: `rotation .multVecMatrix (quad [i] .multVec (spriteSize))`, normal = row z).
/// The draw culls back faces (`gl .enable (gl .CULL_FACE)`): solid.
pub fn quad_geometry(sprite: Option<(&[Vector3; 3], [f64; 2])>, color_material: bool) -> Geometry {
    let mut vertices = Vec::with_capacity(24);
    let mut normal = [0.0f32, 0.0, 1.0];

    match sprite {
        Some((rows, size)) => {
            for q in QUAD {
                let (x, y) = (q[0] * size[0], q[1] * size[1]);
                // multVecMatrix of a Matrix3 (rows x, y, z): v .x * x + v .y * y + v .z * z, z = 0 here.
                let v = [x * rows[0].x + y * rows[1].x, x * rows[0].y + y * rows[1].y, x * rows[0].z + y * rows[1].z];

                vertices.extend_from_slice(&[v[0] as f32, v[1] as f32, v[2] as f32, 1.0]);
            }

            normal = [rows[2].x as f32, rows[2].y as f32, rows[2].z as f32];
        }
        None => {
            for q in QUAD {
                vertices.extend_from_slice(&[q[0] as f32, q[1] as f32, 0.0, 1.0]);
            }
        }
    }

    let mut g = Geometry {
        geometry_type: 2,
        vertices,
        normals: normal.repeat(6),
        tex_coords: QUAD_TEX_COORDS.iter().flat_map(|t| [t[0], t[1], 0.0, 1.0]).collect(),
        solid: true,
        ccw: true,
        ..Default::default()
    };

    if color_material {
        g.colors = vec![1.0; 24];
    }

    bounds(&mut g);
    g
}

/// `getScreenAlignedRotation (modelViewMatrix)`: the rows x, y, z of the rotation that turns a sprite to
/// face the viewer.
pub fn screen_aligned_rotation(model_view: &Matrix4) -> [Vector3; 3] {
    let mut inv = *model_view;
    inv.inverse();

    let mut x = Vector3::new(0.0, 1.0, 0.0);
    let mut z = Vector3::new(0.0, 0.0, 1.0);

    inv.mult_dir_matrix(&mut x);
    inv.mult_dir_matrix(&mut z);

    x.cross(&z);

    let mut y = z;
    y.cross(&x);

    x.normalize();
    y.normalize();
    z.normalize();

    [x, y, z]
}

/// The instances of the particles `0 .. num`: `x3d_InstanceMatrix` (columns: the three scale / rotation
/// rows of the record and its position), and with `color_material` the record's color, with a texCoord
/// ramp of a quad type the four ramp texture coordinates of the particle. Hidden particles (NaN position)
/// are left out: X_ITE draws them, and they cover no pixel.
pub struct ParticleInstances {
    pub matrices: Vec<Matrix4>,
    pub colors: Vec<[f32; 4]>,
    pub tex_coords: Vec<[[f32; 4]; 4]>,
}

pub fn instances(particles: &[Particle], ramps: &Ramps, geometry_type: GeometryType, color_material: bool) -> ParticleInstances {
    let tex_ramp = ramps.tex_coords.as_ref().filter(|_| geometry_type.tex_coord_count() == 4);
    let mut out = ParticleInstances { matrices: vec![], colors: vec![], tex_coords: vec![] };

    for p in particles {
        if p[24..28].iter().any(|v| v.is_nan()) {
            continue;
        }

        let mut m = [0.0f64; 16];

        for (k, v) in p[12..28].iter().enumerate() {
            m[k] = *v as f64;
        }

        out.matrices.push(Matrix4(m));

        if color_material {
            out.colors.push([p[4], p[5], p[6], p[7]]);
        }

        if let Some(r) = tex_ramp {
            // texelFetch (x3d_TexCoordRamp, int (x3d_Particle [3]) + map [gl_VertexID], 0).
            let index0 = p[3] as i64;

            out.tex_coords.push(std::array::from_fn(|k| r.fetch(index0 + k as i64)));
        }
    }

    out
}

#[cfg(test)]
mod tests {
    use super::*;
    use x3d_runtime::particles::{Ramp, STRIDE};

    fn particle(position: [f32; 4], color: [f32; 4], index0: f32) -> Particle {
        let mut p: Particle = [0.0; STRIDE];
        p[3] = index0;
        p[4..8].copy_from_slice(&color);
        p[12] = 2.0;
        p[17] = 3.0;
        p[22] = 4.0;
        p[24..28].copy_from_slice(&position);
        p
    }

    #[test]
    fn instances_skip_hidden_particles_and_read_the_record() {
        let ramp = Ramp { count: 1, texels: vec![[0.0; 4], [0.1, 0.2, 0.0, 1.0], [0.3, 0.4, 0.0, 1.0], [0.5, 0.6, 0.0, 1.0], [0.7, 0.8, 0.0, 1.0]] };
        let ramps = Ramps { tex_coords: Some(ramp), tex_coords_count: 4, ..Default::default() };
        let records = [particle([1.0, 2.0, 3.0, 1.0], [1.0, 0.5, 0.25, 0.5], 1.0), particle([f32::NAN; 4], [0.0; 4], 1.0)];
        let i = instances(&records, &ramps, GeometryType::Quad, true);

        assert_eq!(i.matrices.len(), 1);
        assert_eq!(i.matrices[0].0, [2.0, 0.0, 0.0, 0.0, 0.0, 3.0, 0.0, 0.0, 0.0, 0.0, 4.0, 0.0, 1.0, 2.0, 3.0, 1.0]);
        assert_eq!(i.colors, vec![[1.0, 0.5, 0.25, 0.5]]);
        assert_eq!(i.tex_coords[0][0], [0.1, 0.2, 0.0, 1.0]);
        assert_eq!(i.tex_coords[0][3], [0.7, 0.8, 0.0, 1.0]);

        // POINT has no texture coordinate count: no ramp coordinates; no color material: no colors.
        let i = instances(&records, &ramps, GeometryType::Point, false);
        assert!(i.tex_coords.is_empty() && i.colors.is_empty());
    }

    #[test]
    fn quad_and_sprite_geometry() {
        let q = quad_geometry(None, false);
        assert_eq!(q.vertex_count(), 6);
        assert_eq!(&q.vertices[0..4], &[-0.5, -0.5, 0.0, 1.0]);
        assert_eq!(&q.normals[0..3], &[0.0, 0.0, 1.0]);
        assert!(q.colors.is_empty() && q.solid);

        // Looking down -z with no rotation: the sprite is the quad scaled by the size, facing +z.
        let rows = screen_aligned_rotation(&Matrix4::IDENTITY);
        let s = quad_geometry(Some((&rows, [2.0, 4.0])), true);
        assert_eq!(&s.vertices[0..4], &[-1.0, -2.0, 0.0, 1.0]);
        assert_eq!(&s.normals[0..3], &[0.0, 0.0, 1.0]);
        assert_eq!(s.colors.len(), 24);

        let l = line_geometry(0.3, false);
        assert_eq!(l.vertices, vec![0.0, 0.0, -0.15, 1.0, 0.0, 0.0, 0.15, 1.0]);
    }
}
