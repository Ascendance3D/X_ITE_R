//! The optional geometric property nodes every X3DComposedGeometryNode-like node feeds while it
//! builds: texture coordinates (Components/Texturing/{TextureCoordinate,TextureCoordinateGenerator,
//! MultiTextureCoordinate}.js, Components/Texturing3D/TextureCoordinate{3D,4D}.js), FogCoordinate,
//! Tangent and the vertex attributes (Components/Shaders/{Float,Matrix3,Matrix4}VertexAttribute.js).

use crate::geometry::Geometry;

/// One texture coordinate channel (an X3DSingleTextureCoordinateNode).
#[derive(Clone, Copy, Debug)]
pub enum TexChannel<'a> {
    /// TextureCoordinate (2), TextureCoordinate3D (3), TextureCoordinate4D (4) points.
    Point {
        values: &'a [f32],
        components: usize,
        mapping: &'a str,
    },
    /// TextureCoordinateGenerator: the channel holds 0 0 0 1 per vertex; the shader generates.
    Generator {
        mode: i32,
        parameter: [f32; 6],
        mapping: &'a str,
    },
}

/// One X3DVertexAttributeNode.
#[derive(Clone, Copy, Debug)]
pub enum Attrib<'a> {
    Float {
        name: &'a str,
        components: usize,
        values: &'a [f32],
    },
    Matrix3 {
        name: &'a str,
        values: &'a [f32],
    },
    Matrix4 {
        name: &'a str,
        values: &'a [f32],
    },
}

impl Attrib<'_> {
    pub fn name(&self) -> &str {
        match self {
            Attrib::Float { name, .. }
            | Attrib::Matrix3 { name, .. }
            | Attrib::Matrix4 { name, .. } => name,
        }
    }

    pub fn components(&self) -> usize {
        match self {
            Attrib::Float { components, .. } => *components,
            Attrib::Matrix3 { .. } => 9,
            Attrib::Matrix4 { .. } => 16,
        }
    }

    /// addValue (index, array).
    pub fn add_value(&self, index: i64, array: &mut Vec<f32>) {
        match self {
            Attrib::Float {
                components, values, ..
            } => {
                let n = *components as i64;
                let (first, last) = (index * n, index * n + n);

                if last <= values.len() as i64 && first >= 0 {
                    array.extend_from_slice(&values[first as usize..last as usize]);
                } else if *components <= values.len() {
                    // X_ITE reads the last elements of the field's typed array, whose length is its
                    // capacity: 16 at creation, else the next power of two it grew to; the spare
                    // elements of a freshly parsed field are zero. (A field that shrank at run time
                    // keeps stale elements there in X_ITE; that history is not modelled.)
                    let capacity = values.len().next_power_of_two().max(16);
                    for k in capacity - components..capacity {
                        array.push(values.get(k).copied().unwrap_or(0.0));
                    }
                } else {
                    for _ in first..last {
                        array.push(0.0);
                    }
                }
            }
            Attrib::Matrix3 { values, .. } | Attrib::Matrix4 { values, .. } => {
                let size = self.components();
                let length = values.len() / size;

                if index >= 0 && (index as usize) < length {
                    let i = index as usize * size;
                    array.extend_from_slice(&values[i..i + size]);
                } else if length > 0 {
                    let i = (length - 1) * size;
                    array.extend_from_slice(&values[i..i + size]);
                } else if size == 9 {
                    array.extend_from_slice(&[1.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 1.0]);
                } else {
                    array.extend_from_slice(&[
                        1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0,
                        1.0,
                    ]);
                }
            }
        }
    }
}

/// The property nodes of one geometry node. `tex` is `None` without a texCoord node.
#[derive(Clone, Copy, Debug, Default)]
pub struct Extras<'a> {
    pub tex: Option<&'a [TexChannel<'a>]>,
    pub tangent: Option<&'a [f32]>,
    pub fog: Option<&'a [f32]>,
    pub attribs: &'a [Attrib<'a>],
}

impl Extras<'_> {
    /// Number of texture coordinate channels `init` makes (MultiTextureCoordinate: at most 4).
    pub fn channels(&self) -> usize {
        self.tex
            .map(|t| t.len().min(crate::extras::MAX_TEX_COORDS))
            .unwrap_or(0)
    }

    /// texCoordNode ?.init (multiTexCoordArray) and the attribute arrays.
    pub fn init(&self, g: &mut Geometry) {
        let n = self.channels();

        g.tex_coords_extra = vec![vec![]; n.saturating_sub(1)];
        g.tex_channels = self
            .tex
            .map(|t| t[..n].iter().map(channel_info).collect())
            .unwrap_or_default();
        g.attribs = self
            .attribs
            .iter()
            .map(|a| crate::geometry::AttribArray {
                name: a.name().to_string(),
                components: a.components(),
                values: vec![],
            })
            .collect();
    }

    /// texCoordNode ?.addPoint (index, multiTexCoordArray).
    pub fn add_tex(&self, index: i64, g: &mut Geometry) {
        let Some(tex) = self.tex else { return };

        for (i, c) in tex.iter().take(MAX_TEX_COORDS).enumerate() {
            let array = if i == 0 {
                &mut g.tex_coords
            } else {
                &mut g.tex_coords_extra[i - 1]
            };

            match c {
                TexChannel::Point {
                    values, components, ..
                } => {
                    let n = *components;
                    let length = values.len() / n;

                    if index >= 0 && length > 0 {
                        let k = (index as usize % length) * n;
                        match n {
                            2 => array.extend_from_slice(&[values[k], values[k + 1], 0.0, 1.0]),
                            3 => array.extend_from_slice(&[
                                values[k],
                                values[k + 1],
                                values[k + 2],
                                1.0,
                            ]),
                            _ => array.extend_from_slice(&values[k..k + 4]),
                        }
                    } else {
                        array.extend_from_slice(&[0.0, 0.0, 0.0, 1.0]);
                    }
                }
                TexChannel::Generator { .. } => array.extend_from_slice(&[0.0, 0.0, 0.0, 1.0]),
            }
        }
    }

    /// The attribute values and the fog depth of one vertex.
    pub fn add_attribs_and_fog(&self, index: i64, g: &mut Geometry) {
        for (a, out) in self.attribs.iter().zip(g.attribs.iter_mut()) {
            a.add_value(index, &mut out.values);
        }

        if let Some(depth) = self.fog {
            // FogCoordinate.addDepth.
            if index >= 0 && !depth.is_empty() {
                g.fog_depths.push(depth[index as usize % depth.len()]);
            } else {
                g.fog_depths.push(0.0);
            }
        }
    }

    /// Tangent.addVector.
    pub fn add_tangent(&self, index: i64, g: &mut Geometry) {
        if let Some(v) = self.tangent {
            let length = v.len() / 4;

            if index >= 0 && length > 0 {
                let i = (index as usize % length) * 4;
                g.tangents.extend_from_slice(&v[i..i + 4]);
            } else {
                g.tangents.extend_from_slice(&[1.0, 0.0, 0.0, 1.0]);
            }
        }
    }
}

/// X3DTexturingContext maxTexCoords.
pub const MAX_TEX_COORDS: usize = 4;

fn channel_info(c: &TexChannel) -> crate::geometry::TexChannelInfo {
    match c {
        TexChannel::Point { mapping, .. } => crate::geometry::TexChannelInfo {
            mode: 0,
            parameter: [0.0; 6],
            mapping: mapping.to_string(),
        },
        TexChannel::Generator {
            mode,
            parameter,
            mapping,
        } => crate::geometry::TexChannelInfo {
            mode: *mode,
            parameter: *parameter,
            mapping: mapping.to_string(),
        },
    }
}

/// TextureCoordinateGenerator.set_mode__ (legacy names included; unknown → SPHERE).
pub fn generator_mode(mode: &str) -> i32 {
    match mode {
        "SPHERE" => 1,
        "CAMERA_SPACE_NORMAL" | "CAMERASPACENORMAL" => 2,
        "CAMERA_SPACE_POSITION" | "CAMERASPACEPOSITION" => 3,
        "CAMERA_SPACE_REFLECTION_VECTOR" | "CAMERASPACEREFLECTIONVECTOR" => 4,
        "SPHERE_LOCAL" | "SPHERE-LOCAL" => 5,
        "COORD" => 6,
        "COORD_EYE" | "COORD-EYE" => 7,
        "NOISE" => 8,
        "NOISE_EYE" | "NOISE-EYE" => 9,
        "SPHERE_REFLECT" | "SPHERE-REFLECT" => 10,
        "SPHERE_REFLECT_LOCAL" | "SPHERE-REFLECT-LOCAL" => 11,
        _ => 1,
    }
}
