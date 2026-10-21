//! The GPU side of GaussianSplats (see [`crate::splats`]): storage buffers per splat data set, a
//! sorted index buffer per draw, one pipeline per target, a dynamic-offset uniform buffer, one bind
//! group per draw. Everything is created when a frame first needs it and dropped when no frame has
//! used it for [`KEEP_FRAMES`] frames, so after an unload every counter of
//! [`SplatGpu::resources`] returns to 0.

use crate::splats::{sort_indices, SortScratch, SplatDraw};
use std::collections::HashMap;
use std::rc::Rc;
use x3d_fields::math::Matrix4;

/// Frames a buffer, pipeline or bind group stays after its last use.
pub const KEEP_FRAMES: u64 = 120;

/// The splats WGSL.
pub const SPLATS_WGSL: &str = include_str!("splats.wgsl");

/// A splat draw of the frame.
pub struct SplatInput<'a> {
    pub draw: &'a SplatDraw,
    pub clip_planes: Vec<[f64; 4]>,
    /// gl.viewport of the draw: x, y, width, height.
    pub viewport: [f64; 4],
    /// The fog of the draw: the innermost LocalFog, else the layer's Fog.
    pub fog: Option<&'a crate::fog::FogParams>,
}

struct DataGpu {
    positions: wgpu::Buffer,
    orientations: wgpu::Buffer,
    scales: wgpu::Buffer,
    opacities: wgpu::Buffer,
    sh: wgpu::Buffer,
    used: u64,
    /// Keeps the data alive for the pointer key.
    _data: Rc<crate::splats::SplatData>,
}

struct IndexGpu {
    buffer: wgpu::Buffer,
    /// The model-view matrix of the last sort (sortModelViewMatrix), `None` before the first.
    sorted_for: Option<[f32; 16]>,
    count: usize,
    used: u64,
}

#[derive(Clone, Copy, PartialEq, Eq, Hash)]
struct PipelineKey {
    format: wgpu::TextureFormat,
    samples: u32,
    reversed: bool,
}

struct Prepared {
    pipeline: PipelineKey,
    group: (usize, usize),
    offset: u32,
    instances: u32,
}

const UNIFORM_BYTES: usize = 64 + 64 + 16 + 16 + 16 + 16 + 6 * 16 + 5 * 16;

#[derive(Default)]
pub struct SplatGpu {
    module: Option<(wgpu::ShaderModule, wgpu::BindGroupLayout)>,
    data: HashMap<usize, DataGpu>,
    indices: HashMap<(usize, usize), IndexGpu>,
    pipelines: HashMap<PipelineKey, (wgpu::RenderPipeline, u64)>,
    groups: HashMap<(usize, usize), (wgpu::BindGroup, u64)>,
    buffer: Option<(wgpu::Buffer, u64)>,
    prepared: Vec<Prepared>,
    scratch: SortScratch,
    /// Sorts run so far (for tests).
    pub sorts: usize,
}

fn between<'a>(source: &'a str, from: &str, to: &str) -> &'a str {
    let a = source.find(from).unwrap_or_else(|| panic!("x3d.wgsl: marker {from:?} not found (splats.wgsl copies from it)"));
    let b = a + source[a..].find(to).unwrap_or_else(|| panic!("x3d.wgsl: marker {to:?} not found (splats.wgsl copies from it)"));
    &source[a..b]
}

/// splats.wgsl with the frame structs, the tone map and the fog functions of x3d.wgsl filled in.
pub fn assemble() -> String {
    let x3d = include_str!("x3d.wgsl");
    let structs = between(x3d, "struct Light {", "struct DrawUniforms {");
    let tone = between(x3d, "// ToneMapping2.glsl linearToSRGB", "fn f_schlick(");
    let fog = between(x3d, "// Fog2.glsl getFogInterpolant", "@fragment\nfn fs_main(");

    SPLATS_WGSL.replace("//@FRAME_STRUCTS@", structs).replace("//@TONE_AND_FOG@", &format!("{tone}\n{fog}"))
}

fn matrix_f32(m: &Matrix4) -> [f32; 16] {
    m.0.map(|v| v as f32)
}

pub(crate) fn inverse_f32(m: &[f32; 16]) -> [f32; 16] {
    let mut inv = Matrix4(m.map(|v| v as f64));
    inv.inverse();
    matrix_f32(&inv)
}

fn storage(device: &wgpu::Device, label: &str, bytes: &[u8]) -> wgpu::Buffer {
    let size = (bytes.len() as u64).max(16).next_multiple_of(4);
    let buffer = device.create_buffer(&wgpu::BufferDescriptor { label: Some(label), size, usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST, mapped_at_creation: true });
    {
        let mut view = buffer.slice(..).get_mapped_range_mut().expect("mapped at creation");
        let mut padded = bytes.to_vec();
        padded.resize(size as usize, 0);
        view.copy_from_slice(&padded);
    }
    buffer.unmap();
    buffer
}

/// `values` with `per` floats per element padded to four.
fn pad4(values: &[f32], per: usize) -> Vec<f32> {
    let mut out = Vec::with_capacity(values.len() / per * 4);

    for c in values.chunks_exact(per) {
        out.extend_from_slice(c);
        out.extend(std::iter::repeat_n(0.0, 4 - per));
    }

    out
}

impl SplatGpu {
    /// GPU objects held: data sets (five buffers each), sorted index buffers, pipelines, bind
    /// groups; 0 each after an unload.
    pub fn resources(&self) -> [usize; 4] {
        [self.data.len(), self.indices.len(), self.pipelines.len(), self.groups.len()]
    }

    fn ensure_module(&mut self, device: &wgpu::Device) {
        if self.module.is_some() {
            return;
        }

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor { label: Some("splats"), source: wgpu::ShaderSource::Wgsl(assemble().into()) });
        let storage_entry = |binding: u32| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::VERTEX,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Storage { read_only: true }, has_dynamic_offset: false, min_binding_size: None },
            count: None,
        };
        let mut entries = vec![wgpu::BindGroupLayoutEntry {
            binding: 0,
            visibility: wgpu::ShaderStages::VERTEX_FRAGMENT,
            ty: wgpu::BindingType::Buffer { ty: wgpu::BufferBindingType::Uniform, has_dynamic_offset: true, min_binding_size: wgpu::BufferSize::new(UNIFORM_BYTES as u64) },
            count: None,
        }];

        for b in 1..=6 {
            entries.push(storage_entry(b));
        }

        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor { label: Some("splats"), entries: &entries });
        self.module = Some((module, layout));
    }

    /// Record the splats of a frame: data buffers, sorts, uniforms, pipelines and bind groups.
    /// `projection` is the projection matrix uploaded to the frame uniforms.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        frame_layout: &wgpu::BindGroupLayout,
        format: wgpu::TextureFormat,
        samples: u32,
        depth_format: wgpu::TextureFormat,
        reversed: bool,
        projection: &[f32; 16],
        frame_no: u64,
        inputs: &[SplatInput],
    ) {
        self.prepared.clear();

        if inputs.is_empty() {
            return;
        }

        self.ensure_module(device);

        let inv_projection = inverse_f32(projection);
        let mut blobs: Vec<Vec<u8>> = vec![];
        let mut ordinals: HashMap<usize, usize> = HashMap::new();
        let stride = (UNIFORM_BYTES as u64).next_multiple_of(256);

        for input in inputs {
            let d = input.draw;
            let data_key = Rc::as_ptr(&d.data) as usize;
            let ordinal = {
                let o = ordinals.entry(data_key).or_insert(0);
                *o += 1;
                *o - 1
            };
            let data = &d.data;

            // The data set.
            if !self.data.contains_key(&data_key) {
                let g = DataGpu {
                    positions: storage(device, "splat positions", bytemuck::cast_slice(&pad4(&data.positions, 3))),
                    orientations: storage(device, "splat orientations", bytemuck::cast_slice(&data.orientations)),
                    scales: storage(device, "splat scales", bytemuck::cast_slice(&pad4(&data.scales, 3))),
                    opacities: storage(device, "splat opacities", bytemuck::cast_slice(&data.opacities)),
                    sh: storage(device, "splat spherical harmonics", bytemuck::cast_slice(&data.sh.texels)),
                    used: frame_no,
                    _data: d.data.clone(),
                };
                self.data.insert(data_key, g);
            }

            self.data.get_mut(&data_key).unwrap().used = frame_no;

            // The sorted indices (GaussianSplatsShape.sortIndices).
            let mv = matrix_f32(&d.model_view);
            let index_key = (data_key, ordinal);
            let count = data.count;
            let fresh = !self.indices.get(&index_key).is_some_and(|e| e.count == count);

            if fresh {
                let buffer = device.create_buffer(&wgpu::BufferDescriptor {
                    label: Some("splat indices"),
                    size: ((count.max(1) * 4) as u64).next_multiple_of(4),
                    usage: wgpu::BufferUsages::STORAGE | wgpu::BufferUsages::COPY_DST,
                    mapped_at_creation: false,
                });
                self.indices.insert(index_key, IndexGpu { buffer, sorted_for: None, count, used: frame_no });
                self.groups.remove(&index_key);
            }

            let entry = self.indices.get_mut(&index_key).unwrap();
            entry.used = frame_no;

            if entry.sorted_for != Some(mv) {
                let sorted = sort_indices(&data.positions, count, &mv, &mut self.scratch);
                queue.write_buffer(&entry.buffer, 0, bytemuck::cast_slice(&sorted));
                entry.sorted_for = Some(mv);
                self.sorts += 1;
            }

            // The uniforms.
            let model_view_inverse = inverse_f32(&mv);
            let (viewport_w, viewport_h) = (input.viewport[2] as f32, input.viewport[3] as f32);
            let mut bytes: Vec<u8> = Vec::with_capacity(stride as usize);
            let f = |v: &[f32], bytes: &mut Vec<u8>| bytes.extend_from_slice(bytemuck::cast_slice(v));

            f(&mv, &mut bytes);
            f(&inv_projection, &mut bytes);
            f(&[model_view_inverse[12], model_view_inverse[13], model_view_inverse[14], 0.0], &mut bytes);
            // The projection matrix stores the focal length in the first and second element of the diagonal.
            f(&[projection[0] * viewport_w * 0.5, projection[5] * viewport_h * 0.5, viewport_w, viewport_h], &mut bytes);
            let viewpoint_type = projection[15] as i32 as u32;
            bytes.extend_from_slice(bytemuck::cast_slice(&[viewpoint_type, (data.degrees + 1) as u32, d.linear as u32, reversed as u32]));
            bytes.extend_from_slice(bytemuck::cast_slice(&[input.clip_planes.len().min(6) as u32, data.width as u32, data.sh.width as u32, data.sh.layers as u32]));
            let mut planes = [[0.0f32; 4]; 6];

            for (i, p) in input.clip_planes.iter().take(6).enumerate() {
                planes[i] = p.map(|v| v as f32);
            }

            bytes.extend_from_slice(bytemuck::cast_slice(&planes));

            match input.fog {
                Some(fog) => {
                    f(&[fog.color[0], fog.color[1], fog.color[2], fog.fog_type as f32], &mut bytes);
                    f(&[fog.visibility_start, fog.visibility_range, 0.0, 0.0], &mut bytes);
                    f(&[fog.matrix[0], fog.matrix[1], fog.matrix[2], 0.0], &mut bytes);
                    f(&[fog.matrix[3], fog.matrix[4], fog.matrix[5], 0.0], &mut bytes);
                    f(&[fog.matrix[6], fog.matrix[7], fog.matrix[8], 0.0], &mut bytes);
                }
                None => bytes.extend_from_slice(&[0u8; 5 * 16]),
            }

            debug_assert_eq!(bytes.len(), UNIFORM_BYTES);
            bytes.resize(stride as usize, 0);
            blobs.push(bytes);
        }

        // The uniform buffer, grown when needed (the bind groups refer to it).
        let total = stride * blobs.len() as u64;

        if self.buffer.as_ref().is_none_or(|b| b.1 < total) {
            let capacity = total.next_power_of_two().max(4096);
            self.buffer = Some((device.create_buffer(&wgpu::BufferDescriptor { label: Some("splat uniforms"), size: capacity, usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST, mapped_at_creation: false }), capacity));
            self.groups.clear();
        }

        queue.write_buffer(&self.buffer.as_ref().unwrap().0, 0, &blobs.concat());

        let (module, layout) = self.module.as_ref().unwrap();
        let pipeline_key = PipelineKey { format, samples, reversed };

        if !self.pipelines.contains_key(&pipeline_key) {
            let blend = wgpu::BlendState {
                // gl.blendFuncSeparate (SRC_ALPHA, ONE_MINUS_SRC_ALPHA, ONE, ONE_MINUS_SRC_ALPHA)
                color: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::SrcAlpha, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
                alpha: wgpu::BlendComponent { src_factor: wgpu::BlendFactor::One, dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha, operation: wgpu::BlendOperation::Add },
            };
            let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor { label: Some("splats"), bind_group_layouts: &[Some(frame_layout), Some(layout)], immediate_size: 0 });
            let p = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("splats"),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState { module, entry_point: Some("vs_main"), compilation_options: Default::default(), buffers: &[] },
                // The quad is counter clockwise in normalized device coordinates, whatever the projection.
                primitive: wgpu::PrimitiveState { topology: wgpu::PrimitiveTopology::TriangleList, front_face: wgpu::FrontFace::Ccw, cull_mode: None, ..Default::default() },
                depth_stencil: Some(wgpu::DepthStencilState {
                    format: depth_format,
                    // drawShapes: gl.depthMask (false) for the transparent shapes.
                    depth_write_enabled: Some(false),
                    depth_compare: Some(if reversed { wgpu::CompareFunction::GreaterEqual } else { wgpu::CompareFunction::LessEqual }),
                    stencil: Default::default(),
                    bias: Default::default(),
                }),
                multisample: wgpu::MultisampleState { count: samples, mask: !0, alpha_to_coverage_enabled: false },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState { format, blend: Some(blend), write_mask: wgpu::ColorWrites::ALL })],
                }),
                multiview_mask: None,
                cache: None,
            });
            self.pipelines.insert(pipeline_key, (p, frame_no));
        }

        self.pipelines.get_mut(&pipeline_key).unwrap().1 = frame_no;

        let mut ordinals: HashMap<usize, usize> = HashMap::new();

        for (i, input) in inputs.iter().enumerate() {
            let data_key = Rc::as_ptr(&input.draw.data) as usize;
            let ordinal = {
                let o = ordinals.entry(data_key).or_insert(0);
                *o += 1;
                *o - 1
            };
            let group_key = (data_key, ordinal);

            match self.groups.get_mut(&group_key) {
                Some(g) => g.1 = frame_no,
                None => {
                    let data = &self.data[&data_key];
                    let index = &self.indices[&group_key];
                    let buffer = &self.buffer.as_ref().unwrap().0;
                    let entries = [
                        wgpu::BindGroupEntry { binding: 0, resource: wgpu::BindingResource::Buffer(wgpu::BufferBinding { buffer, offset: 0, size: wgpu::BufferSize::new(UNIFORM_BYTES as u64) }) },
                        wgpu::BindGroupEntry { binding: 1, resource: data.positions.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 2, resource: data.orientations.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 3, resource: data.scales.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 4, resource: data.opacities.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 5, resource: data.sh.as_entire_binding() },
                        wgpu::BindGroupEntry { binding: 6, resource: index.buffer.as_entire_binding() },
                    ];
                    let group = device.create_bind_group(&wgpu::BindGroupDescriptor { label: Some("splats"), layout, entries: &entries });
                    self.groups.insert(group_key, (group, frame_no));
                }
            }

            self.prepared.push(Prepared { pipeline: pipeline_key, group: group_key, offset: (stride * i as u64) as u32, instances: input.draw.data.count as u32 });
        }
    }

    /// Draw prepared splats `i`.
    pub fn draw(&self, pass: &mut wgpu::RenderPass<'_>, i: usize) {
        let p = &self.prepared[i];

        if p.instances == 0 {
            return;
        }

        pass.set_pipeline(&self.pipelines[&p.pipeline].0);
        pass.set_bind_group(1, &self.groups[&p.group].0, &[p.offset]);
        pass.draw(0..6, 0..p.instances);
    }

    /// Drop what no frame has used for [`KEEP_FRAMES`] frames.
    pub fn prune(&mut self, frame_no: u64) {
        self.pipelines.retain(|_, e| e.1 + KEEP_FRAMES >= frame_no);
        self.groups.retain(|_, e| e.1 + KEEP_FRAMES >= frame_no);
        self.indices.retain(|_, e| e.used + KEEP_FRAMES >= frame_no);
        self.data.retain(|_, e| e.used + KEEP_FRAMES >= frame_no);

        // A bind group refers to its buffers.
        let (data, indices) = (&self.data, &self.indices);
        self.groups.retain(|k, _| data.contains_key(&k.0) && indices.contains_key(k));

        if self.data.is_empty() && self.indices.is_empty() && self.pipelines.is_empty() {
            self.buffer = None;
            self.groups.clear();
            self.module = None;
        }
    }
}
