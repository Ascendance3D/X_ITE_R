//! The rubber band of the FLY and WALK viewers: X_ITE's `ScreenLine` (Browser/Rendering/ScreenLine.js) as
//! `X3DFlyViewer.display` draws it on `displayFinishEvents` while a button is held and the `Rubberband` browser option is on.
//!
//! The line is a black and a white strip, each of two quads (`fromWidth` 1, `toWidth` 1, `tipStart` 0.4), drawn in
//! drawing-buffer pixels with an orthographic projection of the viewport and the default (unlit) material with vertex colors,
//! blended with `SRC_ALPHA, ONE_MINUS_SRC_ALPHA` for color and `ONE, ONE_MINUS_SRC_ALPHA` for alpha, no depth test, no depth write.
//! The black strip is one pixel wider on each side than the white one; the vertex alpha of the last two vertices of each strip is 0,
//! so the line fades out over the last 60% of its length. The model view matrix has the line vector as its x axis and the
//! perpendicular scaled by the ContentScale rendering property as its y axis.

use bytemuck::cast_slice;
use std::collections::HashMap;

/// One rubber band: the two ends in viewport pixels (y up), the ContentScale rendering property and the viewport rectangle
/// (x, y, width, height of `browser .getViewport ()`, y up).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct RubberBand {
    pub from: [f64; 2],
    pub to: [f64; 2],
    pub content_scale: f64,
    pub viewport: [f64; 4],
}

/// The local vertices of the two strips: x along the line (0 at the start, 1 at the end), y across it, then the color. Black
/// strip first, white strip second; `(2 * (width + 1) / 2, ...)` as in the ScreenLine constructor: the normal of the unit line is
/// (0, 0.5), the black half width is `(1 + 1) * 0.5`, the white one `1 * 0.5`.
const TIP_START: f32 = 0.4;
const BLACK: f32 = 1.0;
const WHITE: f32 = 0.5;

fn local_vertices() -> [[f32; 6]; 12] {
    let strip = |half: f32, color: f32| {
        [
            [0.0, half, color, color, color, 1.0],
            [0.0, -half, color, color, color, 1.0],
            [TIP_START, half, color, color, color, 1.0],
            [TIP_START, -half, color, color, color, 1.0],
            [1.0, half, color, color, color, 0.0],
            [1.0, -half, color, color, color, 0.0],
        ]
    };
    let (black, white) = (strip(BLACK, 0.0), strip(WHITE, 1.0));
    let mut out = [[0.0; 6]; 12];
    out[..6].copy_from_slice(&black);
    out[6..].copy_from_slice(&white);
    out
}

/// ScreenLine's index buffer: for each strip the triangles (0 1 3) (0 3 2) (2 3 5) (2 5 4) of its six vertices.
const INDICES: [u16; 24] = [
    0, 1, 3, 0, 3, 2, 2, 3, 5, 2, 5, 4, 6, 7, 9, 6, 9, 8, 8, 9, 11, 8, 11, 10,
];

impl RubberBand {
    /// ScreenLine.display: the projection `Camera .ortho (0, width, 0, height, -1, 1)` and the model view matrix (x axis the line,
    /// y axis its normal times ContentScale, translation the start), column major. `None` when the line has no length (the y axis is
    /// then not defined and X_ITE draws nothing visible).
    pub fn matrices(&self, flip: bool) -> Option<([f32; 16], [f32; 16])> {
        let (width, height) = (self.viewport[2], self.viewport[3]);
        let x_axis = [self.to[0] - self.from[0], self.to[1] - self.from[1]];
        let length = x_axis[0].hypot(x_axis[1]);

        if length.is_nan() || length <= 0.0 {
            return None;
        }

        let y_axis = [
            -x_axis[1] / length * self.content_scale,
            x_axis[0] / length * self.content_scale,
        ];
        // Camera.ortho: m00 = 2 / (r - l), m11 = 2 / (t - b), m22 = -2 / (f - n), the translation -(r + l) / (r - l) and so on.
        let sign = if flip { -1.0 } else { 1.0 };
        let projection = [
            2.0 / width,
            0.0,
            0.0,
            0.0,
            0.0,
            2.0 / height * sign,
            0.0,
            0.0,
            0.0,
            0.0,
            -1.0,
            0.0,
            -1.0,
            -sign,
            0.0,
            1.0,
        ];
        // A pixel center exactly on a horizontal edge: the white strip of a horizontal band at ContentScale 1 has its two long edges
        // (0.5 px from the line, which is on a pixel boundary) on pixel centers. ANGLE flips y for the default framebuffer, so
        // X_ITE's GL includes the center on the lower edge (y up) and not the one on the upper edge, the reverse of the top-left
        // rule the Vulkan, Metal and Direct3D rasterizers apply to the same triangles. One sub-pixel step (1/256 px, the snap grid)
        // down makes the same pixels belong to the strip. Other edges are not on a pixel center in a way the two rules treat
        // differently (left edges are left edges in both), so only the exactly horizontal band needs it.
        let nudge = if x_axis[1] == 0.0 { -1.0 / 256.0 } else { 0.0 };
        let model_view = [
            x_axis[0],
            x_axis[1],
            0.0,
            0.0,
            y_axis[0],
            y_axis[1],
            0.0,
            0.0,
            0.0,
            0.0,
            1.0,
            0.0,
            self.from[0],
            self.from[1] + nudge,
            0.0,
            1.0,
        ];

        Some((projection.map(|v| v as f32), model_view.map(|v| v as f32)))
    }
}

const WGSL: &str = "
struct U { projection: mat4x4<f32>, model_view: mat4x4<f32> };
@group(0) @binding(0) var<uniform> u: U;

struct V { @builtin(position) position: vec4<f32>, @location(0) color: vec4<f32> };

@vertex
fn vs_main(@location(0) position: vec2<f32>, @location(1) color: vec4<f32>) -> V {
    var o: V;
    o.position = u.projection * u.model_view * vec4<f32>(position, 0.0, 1.0);
    o.color = color;
    return o;
}

@fragment
fn fs_main(v: V) -> @location(0) vec4<f32> {
    return v.color;
}
";

/// The GPU objects of the rubber band: shader, buffers and one pipeline per target format.
pub struct RubberBandGpu {
    module: wgpu::ShaderModule,
    pipeline_layout: wgpu::PipelineLayout,
    uniform: wgpu::Buffer,
    bind_group: wgpu::BindGroup,
    vertices: wgpu::Buffer,
    indices: wgpu::Buffer,
    pipelines: HashMap<(wgpu::TextureFormat, u32), wgpu::RenderPipeline>,
}

impl RubberBandGpu {
    pub fn new(device: &wgpu::Device) -> RubberBandGpu {
        use wgpu::util::DeviceExt;

        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("rubber band"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("rubber band"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("rubber band"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let uniform = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("rubber band"),
            size: 128,
            usage: wgpu::BufferUsages::UNIFORM | wgpu::BufferUsages::COPY_DST,
            mapped_at_creation: false,
        });
        let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("rubber band"),
            layout: &layout,
            entries: &[wgpu::BindGroupEntry {
                binding: 0,
                resource: uniform.as_entire_binding(),
            }],
        });
        let flat: Vec<f32> = local_vertices().iter().flatten().copied().collect();
        let vertices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rubber band"),
            contents: cast_slice(&flat),
            usage: wgpu::BufferUsages::VERTEX,
        });
        let indices = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
            label: Some("rubber band"),
            contents: cast_slice(&INDICES),
            usage: wgpu::BufferUsages::INDEX,
        });

        RubberBandGpu {
            module,
            pipeline_layout,
            uniform,
            bind_group,
            vertices,
            indices,
            pipelines: HashMap::new(),
        }
    }

    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        samples: u32,
    ) -> &wgpu::RenderPipeline {
        let (module, layout) = (&self.module, &self.pipeline_layout);

        self.pipelines.entry((format, samples)).or_insert_with(|| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("rubber band"),
                layout: Some(layout),
                vertex: wgpu::VertexState {
                    module,
                    entry_point: Some("vs_main"),
                    compilation_options: Default::default(),
                    buffers: &[Some(wgpu::VertexBufferLayout {
                        array_stride: 24,
                        step_mode: wgpu::VertexStepMode::Vertex,
                        attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x4],
                    })],
                },
                // Both strips are made of counter clockwise triangles of a rotation: nothing is culled, so the flipped target
                // of a multisampled oracle comparison draws the same.
                primitive: Default::default(),
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: samples,
                    ..Default::default()
                },
                fragment: Some(wgpu::FragmentState {
                    module,
                    entry_point: Some("fs_main"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        // gl.blendFuncSeparate (SRC_ALPHA, ONE_MINUS_SRC_ALPHA, ONE, ONE_MINUS_SRC_ALPHA)
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::SrcAlpha,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                            alpha: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
                                dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
                                operation: wgpu::BlendOperation::Add,
                            },
                        }),
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                }),
                multiview_mask: None,
                cache: None,
            })
        })
    }

    /// Draw `band` over `target` (a `width` x `height` view of `format`, which keeps what it has). With `msaa`, the multisampled
    /// color buffer of the frame (its samples kept), the band is drawn into it and resolved into `target`. `flip` is the upside
    /// down target of the GL sample orientation.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        queue: &wgpu::Queue,
        encoder: &mut wgpu::CommandEncoder,
        band: &RubberBand,
        target: &wgpu::TextureView,
        msaa: Option<&wgpu::TextureView>,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        flip: bool,
    ) {
        let Some((projection, model_view)) = band.matrices(flip) else {
            return;
        };
        // gl.viewport (... viewport) and gl.scissor: the viewport rectangle, y up, in the target.
        let [x, y, w, h] = band.viewport;
        let (x0, x1) = (x.max(0.0), (x + w).min(width as f64));
        let (top, bottom) = (
            (height as f64 - (y + h)).max(0.0),
            (height as f64 - y).min(height as f64),
        );

        if !(x1 > x0 && bottom > top) {
            return;
        }

        let (x0, x1, top, bottom) = (x0 as u32, x1 as u32, top as u32, bottom as u32);
        let rect_height = bottom - top;
        let top = if flip {
            height - top - rect_height
        } else {
            top
        };
        let mut data = [0.0f32; 32];
        data[..16].copy_from_slice(&projection);
        data[16..].copy_from_slice(&model_view);
        queue.write_buffer(&self.uniform, 0, cast_slice(&data));
        let samples = msaa.map_or(1, |m| m.texture().sample_count());
        self.pipeline(device, format, samples);
        let pipeline = &self.pipelines[&(format, samples)];
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("rubber band"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: msaa.unwrap_or(target),
                depth_slice: None,
                resolve_target: msaa.map(|_| target),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Load,
                    // MultiSampleFramebuffer.blit: the samples are not kept after the resolve.
                    store: if msaa.is_some() {
                        wgpu::StoreOp::Discard
                    } else {
                        wgpu::StoreOp::Store
                    },
                },
            })],
            ..Default::default()
        });
        pass.set_viewport(
            x0 as f32,
            top as f32,
            (x1 - x0) as f32,
            rect_height as f32,
            0.0,
            1.0,
        );
        pass.set_scissor_rect(x0, top, x1 - x0, rect_height);
        pass.set_pipeline(pipeline);
        pass.set_bind_group(0, &self.bind_group, &[]);
        pass.set_vertex_buffer(0, self.vertices.slice(..));
        pass.set_index_buffer(self.indices.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..INDICES.len() as u32, 0, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_zero_length_band_draws_nothing() {
        let band = RubberBand {
            from: [10.0, 10.0],
            to: [10.0, 10.0],
            content_scale: 1.0,
            viewport: [0.0, 0.0, 512.0, 512.0],
        };

        assert!(band.matrices(false).is_none());
    }

    #[test]
    fn the_model_view_has_the_line_as_x_axis_and_a_normal_scaled_by_content_scale() {
        let band = RubberBand {
            from: [100.0, 50.0],
            to: [100.0, 150.0],
            content_scale: 2.0,
            viewport: [0.0, 0.0, 512.0, 256.0],
        };
        let (projection, model_view) = band.matrices(false).unwrap();

        assert_eq!(&model_view[..2], &[0.0, 100.0]);
        assert_eq!(&model_view[4..6], &[-2.0, 0.0]);
        assert_eq!(&model_view[12..14], &[100.0, 50.0]);
        assert_eq!((projection[0], projection[5]), (2.0 / 512.0, 2.0 / 256.0));
    }

    #[test]
    fn the_strips_are_black_then_white_and_the_tip_fades() {
        let v = local_vertices();

        assert_eq!(v[0][2..], [0.0, 0.0, 0.0, 1.0]);
        assert_eq!(v[4][2..], [0.0, 0.0, 0.0, 0.0]);
        assert_eq!(v[6][2..], [1.0, 1.0, 1.0, 1.0]);
        assert_eq!(v[11][2..], [1.0, 1.0, 1.0, 0.0]);
        assert_eq!(v[2][0], 0.4);
        // The black strip is one pixel wider on each side than the white one.
        assert_eq!(v[0][1] - v[6][1], 0.5);
    }
}
