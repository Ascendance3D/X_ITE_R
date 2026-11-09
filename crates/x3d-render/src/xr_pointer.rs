//! The input rays and hit points of the XR input sources: X_ITE's `ScreenLine` and `ScreenPoint` (Browser/Rendering) as
//! X3DWebXRContext.xrUpdatePointers draws them into the framebuffer of each view before it is blitted into the layer.
//!
//! The line is the one of the rubber band (`crate::rubber_band`), its white strip in the color of the ray. The hit point is a
//! ring (`Circle2DOptions`, 40 `LINES` of the unit circle, black at transparency 0.8) and a disk (`Disk2DOptions`, 40 triangles
//! of a fan, the ray's color at transparency 0.3) in the plane of the hit, with the view's projection matrix. All of them:
//! unlit, blended with `SRC_ALPHA, ONE_MINUS_SRC_ALPHA` for color and `ONE, ONE_MINUS_SRC_ALPHA` for alpha, no depth test, no depth
//! write, no culling.

use crate::xr::{POINT_DISK_TRANSPARENCY, POINT_RING_TRANSPARENCY, XrPointerDraw};
use bytemuck::cast_slice;
use std::collections::HashMap;
use wgpu::util::DeviceExt;

/// The dimension of Circle2DOptions and Disk2DOptions.
const DIMENSION: usize = 40;

const WGSL: &str = "
struct U { projection: mat4x4<f32>, model_view: mat4x4<f32>, tint: vec4<f32> };
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
    return vec4<f32>(v.color.rgb * u.tint.rgb, v.color.a * u.tint.a);
}
";

/// Which of the three shapes a draw is.
#[derive(Clone, Copy, PartialEq, Eq, Hash, Debug)]
enum Shape {
    Line,
    Ring,
    Disk,
}

/// One vertex: x, y, then the color.
fn vertex(x: f64, y: f64) -> [f32; 6] {
    [x as f32, y as f32, 1.0, 1.0, 1.0, 1.0]
}

/// The 40 segments of the circle, `(n, n + 1)` and the last `(39, 0)` (the coord index `0 .. 39, 0, -1` of an IndexedLineSet).
fn ring_vertices() -> Vec<[f32; 6]> {
    let angle = std::f64::consts::PI * 2.0 / DIMENSION as f64;
    let point = |n: usize| {
        let a = angle * (n % DIMENSION) as f64;
        (a.cos(), a.sin())
    };

    (0..DIMENSION)
        .flat_map(|n| {
            let (a, b) = (point(n), point(n + 1));
            [vertex(a.0, a.1), vertex(b.0, b.1)]
        })
        .collect()
}

/// The 40 triangles of the disk, `(0, 0)`, `p(n)`, `p(n + 1)`.
fn disk_vertices() -> Vec<[f32; 6]> {
    let angle = std::f64::consts::PI * 2.0 / DIMENSION as f64;
    let point = |n: usize| {
        let a = angle * n as f64;
        (a.cos(), a.sin())
    };

    (0..DIMENSION)
        .flat_map(|n| {
            let (a, b) = (point(n), point(n + 1));
            [vertex(0.0, 0.0), vertex(a.0, a.1), vertex(b.0, b.1)]
        })
        .collect()
}

/// The GPU objects of the input rays: the shader, the vertices of the three shapes and one pipeline per shape and target.
pub struct XrPointerGpu {
    module: wgpu::ShaderModule,
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    line: wgpu::Buffer,
    line_indices: wgpu::Buffer,
    ring: wgpu::Buffer,
    disk: wgpu::Buffer,
    pipelines: HashMap<(Shape, wgpu::TextureFormat, u32), wgpu::RenderPipeline>,
}

impl XrPointerGpu {
    pub fn new(device: &wgpu::Device) -> XrPointerGpu {
        let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("xr pointers"),
            source: wgpu::ShaderSource::Wgsl(WGSL.into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("xr pointers"),
            entries: &[wgpu::BindGroupLayoutEntry {
                binding: 0,
                visibility: wgpu::ShaderStages::VERTEX | wgpu::ShaderStages::FRAGMENT,
                ty: wgpu::BindingType::Buffer {
                    ty: wgpu::BufferBindingType::Uniform,
                    has_dynamic_offset: false,
                    min_binding_size: None,
                },
                count: None,
            }],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("xr pointers"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let buffer = |data: &[f32], usage| {
            device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("xr pointers"),
                contents: cast_slice(data),
                usage,
            })
        };
        let flat = |v: Vec<[f32; 6]>| v.into_iter().flatten().collect::<Vec<f32>>();
        // `new ScreenLine (this, 4, 2, 0.9)` of X3DWebXRContext.
        let line: Vec<f32> = crate::rubber_band::local_vertices_of(4.0, 2.0, 0.9)
            .iter()
            .flatten()
            .copied()
            .collect();

        XrPointerGpu {
            line: buffer(&line, wgpu::BufferUsages::VERTEX),
            line_indices: device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("xr pointers"),
                contents: cast_slice(&crate::rubber_band::INDICES),
                usage: wgpu::BufferUsages::INDEX,
            }),
            ring: buffer(&flat(ring_vertices()), wgpu::BufferUsages::VERTEX),
            disk: buffer(&flat(disk_vertices()), wgpu::BufferUsages::VERTEX),
            module,
            layout,
            pipeline_layout,
            pipelines: HashMap::new(),
        }
    }

    fn pipeline(
        &mut self,
        device: &wgpu::Device,
        shape: Shape,
        format: wgpu::TextureFormat,
        samples: u32,
    ) {
        let (module, layout) = (&self.module, &self.pipeline_layout);

        self.pipelines
            .entry((shape, format, samples))
            .or_insert_with(|| {
                device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                    label: Some("xr pointers"),
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
                    // `gl .disable (gl .DEPTH_TEST)`, culling off: the strips are counter clockwise triangles of a rotation, and
                    // the disk is seen from both sides.
                    primitive: wgpu::PrimitiveState {
                        topology: if shape == Shape::Ring {
                            wgpu::PrimitiveTopology::LineList
                        } else {
                            wgpu::PrimitiveTopology::TriangleList
                        },
                        ..Default::default()
                    },
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
            });
    }

    /// Draw the rays of one view over `target`, which keeps what it has (see `RubberBandGpu::encode` for `msaa`, `flip` and
    /// the rectangle `viewport` = the view's, x, y, width, height, y up). Nothing is drawn for a ray whose line has no length.
    #[allow(clippy::too_many_arguments)]
    pub fn encode(
        &mut self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        draws: &[XrPointerDraw],
        target: &wgpu::TextureView,
        msaa: Option<&wgpu::TextureView>,
        format: wgpu::TextureFormat,
        width: u32,
        height: u32,
        flip: bool,
    ) {
        // One uniform per draw (a queue write would leave the last value for all of them), in drawing order.
        let mut items: Vec<(Shape, wgpu::BindGroup)> = vec![];
        let mut rect = None;
        let uniform = |projection: [f32; 16], model_view: [f32; 16], tint: [f32; 4]| {
            let mut data = [0.0f32; 36];

            data[..16].copy_from_slice(&projection);
            data[16..32].copy_from_slice(&model_view);
            data[32..].copy_from_slice(&tint);

            let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
                label: Some("xr pointers"),
                contents: cast_slice(&data),
                usage: wgpu::BufferUsages::UNIFORM,
            });

            device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("xr pointers"),
                layout: &self.layout,
                entries: &[wgpu::BindGroupEntry {
                    binding: 0,
                    resource: buffer.as_entire_binding(),
                }],
            })
        };

        for draw in draws {
            let Some((projection, model_view)) = draw.line.matrices(flip) else {
                continue;
            };

            rect = Some(draw.line.viewport);
            let [r, g, b] = draw.color;

            items.push((Shape::Line, uniform(projection, model_view, [r, g, b, 1.0])));

            if let Some(point) = &draw.point {
                // gl_Position = x3d_ProjectionMatrix * x3d_ModelViewMatrix * vertex; the clip y of a flipped target is negated.
                let mut projection = point.projection.0.map(|v| v as f32);

                if flip {
                    for i in [1, 5, 9, 13] {
                        projection[i] = -projection[i];
                    }
                }

                let model_view = point.model_view.0.map(|v| v as f32);
                let [r, g, b] = point.color;

                items.push((
                    Shape::Ring,
                    uniform(
                        projection,
                        model_view,
                        [0.0, 0.0, 0.0, (1.0 - POINT_RING_TRANSPARENCY) as f32],
                    ),
                ));
                items.push((
                    Shape::Disk,
                    uniform(
                        projection,
                        model_view,
                        [r, g, b, (1.0 - POINT_DISK_TRANSPARENCY) as f32],
                    ),
                ));
            }
        }

        let Some([x, y, w, h]) = rect else {
            return;
        };

        // gl.viewport (... viewport) and gl.scissor: the view's rectangle, y up, in the target.
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
        let samples = msaa.map_or(1, |m| m.texture().sample_count());

        for (shape, _) in &items {
            self.pipeline(device, *shape, format, samples);
        }

        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("xr pointers"),
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

        for (shape, bind_group) in &items {
            pass.set_pipeline(&self.pipelines[&(*shape, format, samples)]);
            pass.set_bind_group(0, bind_group, &[]);

            match shape {
                Shape::Line => {
                    pass.set_vertex_buffer(0, self.line.slice(..));
                    pass.set_index_buffer(self.line_indices.slice(..), wgpu::IndexFormat::Uint16);
                    pass.draw_indexed(0..crate::rubber_band::INDICES.len() as u32, 0, 0..1);
                }
                Shape::Ring => {
                    pass.set_vertex_buffer(0, self.ring.slice(..));
                    pass.draw(0..(DIMENSION * 2) as u32, 0..1);
                }
                Shape::Disk => {
                    pass.set_vertex_buffer(0, self.disk.slice(..));
                    pass.draw(0..(DIMENSION * 3) as u32, 0..1);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_ring_is_forty_closed_segments_and_the_disk_forty_triangles() {
        let ring = ring_vertices();
        let disk = disk_vertices();

        assert_eq!(ring.len(), 80);
        assert_eq!(disk.len(), 120);
        // The last segment ends where the first starts.
        assert_eq!(ring[79][..2], ring[0][..2]);
        assert_eq!(ring[1][..2], ring[2][..2]);
        // Every triangle of the disk starts at the center.
        assert!(disk.chunks(3).all(|t| t[0][..2] == [0.0, 0.0]));
    }
}
