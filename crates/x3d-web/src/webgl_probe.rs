//! The WebGL2 proof (release lane, Browser Preview 0.1.0): draw real geometry through wgpu's WebGL2 browser backend
//! (`Backends::GL` on wasm32), with no X3D renderer involved, and report what the backend has.
//!
//! The report is the evidence for the WebGL2 fallback: the adapter, its limits, its downlevel flags, the canvas formats,
//! and the validation errors of a small pipeline. A page test reads the drawn pixels back with a screenshot: an adapter
//! that starts but draws nothing is not a pass.

use wasm_bindgen::prelude::*;

const SHADER: &str = r#"
struct Params { color: vec4<f32>, offset: vec4<f32> };
@group(0) @binding(0) var<uniform> params: Params;

struct Out { @builtin(position) pos: vec4<f32>, @location(0) color: vec4<f32> };

@vertex
fn vs(@location(0) pos: vec2<f32>, @location(1) color: vec3<f32>) -> Out {
    var o: Out;
    o.pos = vec4<f32>(pos + params.offset.xy, 0.5, 1.0);
    o.color = vec4<f32>(color, 1.0) * params.color;
    return o;
}

@fragment
fn fs(i: Out) -> @location(0) vec4<f32> {
    return i.color;
}
"#;

/// Draws, on `canvas`, a dark blue clear color, a red triangle on the left and a green quad (two triangles, indexed, with
/// a depth buffer) on the right. Returns a JSON report. Fails only when no WebGL2 adapter or device exists.
#[wasm_bindgen(js_name = webgl2Probe)]
pub async fn webgl2_probe(canvas: web_sys::HtmlCanvasElement) -> Result<String, JsError> {
    let (width, height) = (canvas.width().max(1), canvas.height().max(1));
    let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
        backends: wgpu::Backends::GL,
        ..wgpu::InstanceDescriptor::new_without_display_handle()
    });
    let surface = instance
        .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
        .map_err(|e| JsError::new(&format!("create_surface (WebGL2): {e}")))?;
    let adapter = instance
        .request_adapter(&wgpu::RequestAdapterOptions {
            power_preference: wgpu::PowerPreference::HighPerformance,
            force_fallback_adapter: false,
            compatible_surface: Some(&surface),
            ..Default::default()
        })
        .await
        .map_err(|e| JsError::new(&format!("no WebGL2 adapter: {e}")))?;
    let info = adapter.get_info();
    let limits = adapter.limits();
    let downlevel = adapter.get_downlevel_capabilities();
    let features = adapter.features();
    let (device, queue) = adapter
        .request_device(&wgpu::DeviceDescriptor {
            label: Some("webgl2-probe"),
            required_features: wgpu::Features::empty(),
            required_limits: limits.clone(),
            ..Default::default()
        })
        .await
        .map_err(|e| JsError::new(&format!("request_device (WebGL2): {e}")))?;

    let caps = surface.get_capabilities(&adapter);
    let format = caps
        .formats
        .iter()
        .copied()
        .find(|f| !f.is_srgb())
        .unwrap_or(caps.formats[0]);

    surface.configure(
        &device,
        &wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: caps.alpha_modes[0],
            view_formats: vec![],
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        },
    );

    let scope = device.push_error_scope(wgpu::ErrorFilter::Validation);

    let module = device.create_shader_module(wgpu::ShaderModuleDescriptor {
        label: Some("webgl2-probe"),
        source: wgpu::ShaderSource::Wgsl(SHADER.into()),
    });
    let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
        label: None,
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
        label: None,
        bind_group_layouts: &[Some(&layout)],
        immediate_size: 0,
    });
    let depth_format = wgpu::TextureFormat::Depth24Plus;
    let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
        label: Some("webgl2-probe"),
        layout: Some(&pipeline_layout),
        vertex: wgpu::VertexState {
            module: &module,
            entry_point: Some("vs"),
            compilation_options: Default::default(),
            buffers: &[Some(wgpu::VertexBufferLayout {
                array_stride: 20,
                step_mode: wgpu::VertexStepMode::Vertex,
                attributes: &wgpu::vertex_attr_array![0 => Float32x2, 1 => Float32x3],
            })],
        },
        fragment: Some(wgpu::FragmentState {
            module: &module,
            entry_point: Some("fs"),
            compilation_options: Default::default(),
            targets: &[Some(format.into())],
        }),
        primitive: wgpu::PrimitiveState::default(),
        depth_stencil: Some(wgpu::DepthStencilState {
            format: depth_format,
            depth_write_enabled: Some(true),
            depth_compare: Some(wgpu::CompareFunction::Less),
            stencil: Default::default(),
            bias: Default::default(),
        }),
        multisample: Default::default(),
        multiview_mask: None,
        cache: None,
    });

    #[rustfmt::skip]
    let vertices: [f32; 35] = [
        // red triangle, left
        -0.9, -0.6, 1.0, 0.0, 0.0,
        -0.1, -0.6, 1.0, 0.0, 0.0,
        -0.5,  0.6, 1.0, 0.0, 0.0,
        // green quad, right
         0.1, -0.6, 0.0, 1.0, 0.0,
         0.9, -0.6, 0.0, 1.0, 0.0,
         0.9,  0.6, 0.0, 1.0, 0.0,
         0.1,  0.6, 0.0, 1.0, 0.0,
    ];
    let indices: [u16; 10] = [0, 1, 2, 3, 4, 5, 3, 5, 6, 0];
    let vertex_buffer = buffer(&device, &vertices, wgpu::BufferUsages::VERTEX);
    let index_buffer = buffer(&device, &indices, wgpu::BufferUsages::INDEX);
    let params: [f32; 8] = [1.0, 1.0, 1.0, 1.0, 0.0, 0.0, 0.0, 0.0];
    let uniform_buffer = buffer(&device, &params, wgpu::BufferUsages::UNIFORM);
    let bind_group = device.create_bind_group(&wgpu::BindGroupDescriptor {
        label: None,
        layout: &layout,
        entries: &[wgpu::BindGroupEntry {
            binding: 0,
            resource: uniform_buffer.as_entire_binding(),
        }],
    });
    let depth = device.create_texture(&wgpu::TextureDescriptor {
        label: None,
        size: wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
        mip_level_count: 1,
        sample_count: 1,
        dimension: wgpu::TextureDimension::D2,
        format: depth_format,
        usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
        view_formats: &[],
    });
    let depth_view = depth.create_view(&Default::default());

    let frame = match surface.get_current_texture() {
        wgpu::CurrentSurfaceTexture::Success(t) | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
        other => return Err(JsError::new(&format!("canvas texture (WebGL2): {other:?}"))),
    };
    let view = frame.texture.create_view(&Default::default());
    let mut encoder = device.create_command_encoder(&Default::default());
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: None,
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color {
                        r: 0.0,
                        g: 0.0,
                        b: 0.4,
                        a: 1.0,
                    }),
                    store: wgpu::StoreOp::Store,
                },
            })],
            depth_stencil_attachment: Some(wgpu::RenderPassDepthStencilAttachment {
                view: &depth_view,
                depth_ops: Some(wgpu::Operations {
                    load: wgpu::LoadOp::Clear(1.0),
                    store: wgpu::StoreOp::Discard,
                }),
                stencil_ops: None,
            }),
            timestamp_writes: None,
            occlusion_query_set: None,
            multiview_mask: None,
        });
        pass.set_pipeline(&pipeline);
        pass.set_bind_group(0, &bind_group, &[]);
        pass.set_vertex_buffer(0, vertex_buffer.slice(..));
        pass.set_index_buffer(index_buffer.slice(..), wgpu::IndexFormat::Uint16);
        pass.draw_indexed(0..9, 0, 0..1);
    }
    queue.submit([encoder.finish()]);
    queue.present(frame);

    let validation = scope.pop().await.map(|e| e.to_string());

    let report = serde_json::json!({
        "backend": format!("{:?}", info.backend),
        "adapter": { "name": info.name, "driver": info.driver, "driver_info": info.driver_info, "device_type": format!("{:?}", info.device_type) },
        "format": format!("{format:?}"),
        "formats": caps.formats.iter().map(|f| format!("{f:?}")).collect::<Vec<_>>(),
        "alpha_modes": caps.alpha_modes.iter().map(|m| format!("{m:?}")).collect::<Vec<_>>(),
        "limits": format!("{limits:?}"),
        "downlevel_flags": format!("{:?}", downlevel.flags),
        "shader_model": format!("{:?}", downlevel.shader_model),
        "features": format!("{features:?}"),
        "below_webgpu_defaults": below_defaults(&limits),
        "validation_error": validation,
        "size": [width, height],
    });

    Ok(report.to_string())
}

fn buffer<T: bytemuck::Pod>(
    device: &wgpu::Device,
    data: &[T],
    usage: wgpu::BufferUsages,
) -> wgpu::Buffer {
    use wgpu::util::DeviceExt;

    device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: None,
        contents: bytemuck::cast_slice(data),
        usage,
    })
}

/// The WebGPU default limits this adapter does not reach, `name have < want`.
pub fn below_defaults(limits: &wgpu::Limits) -> Vec<String> {
    let mut missing = vec![];

    wgpu::Limits::default().check_limits_with_fail_fn(limits, false, |name, want, have| {
        missing.push(format!("{name} {have} < {want}"))
    });

    missing
}
