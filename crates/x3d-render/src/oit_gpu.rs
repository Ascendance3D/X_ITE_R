//! `OrderIndependentTransparency`: X_ITE's weighted blended order independent transparency, on the GPU.
//!
//! The pinned X_ITE 16.5.0 draws the transparent shapes of a frame, in traversal order and without the depth sort
//! (X3DRenderObject.js `draw`: `if (!this .orderIndependentTransparency) transparencySorter .sort`), into two RGBA16F
//! accumulation targets next to the main framebuffer's depth buffer, then composes them over the main color buffer:
//!
//! * `MultiSampleFramebuffer.js` constructor: the OIT framebuffer holds COLOR_ATTACHMENT0 (accum / revealage) and
//!   COLOR_ATTACHMENT1 (alpha), both RGBA16F, multisampled when the main buffer is, with the main depth buffer attached.
//! * `MultiSampleFramebuffer.bindTransparency`: both targets cleared to (0, 0, 0, 1); gl.blendFuncSeparate (ONE, ONE, ZERO,
//!   ONE_MINUS_SRC_ALPHA). X3DRenderObject.drawShapes: depthMask false, BLEND enabled, the transparent shapes drawn.
//! * `OIT2.js` `oit`: the fragment shader of an `X3D_ALPHA_MODE_BLEND` shape of the RENDER_KEY pass
//!   (X3DMaterialNode.addRenderOptions) writes (rgb * a * w, a) and (a * w), the weight `w` of `weight`.
//! * `MultiSampleFramebuffer.compose`: a multisampled target is resolved by `blitFramebuffer`; `OITCompose2.fs.js`
//!   (`oit.wgsl`) composes with gl.blendFunc (ONE, ONE_MINUS_SRC_ALPHA), without the depth test, over the whole target.
//!
//! The shader of a shape is the main shader (`x3d.wgsl`, any depth variant) with its `fs_main` turned into a function and
//! a second entry point, `fs_oit`, added ([`patch_shader`]).

use std::collections::HashMap;

/// RGBA16F (MultiSampleFramebuffer.js: "Must use 16bit color buffer textures instead of 32bit").
pub const FORMAT: wgpu::TextureFormat = wgpu::TextureFormat::Rgba16Float;

/// The color of both accumulation targets after `bindTransparency` (gl.clearColor (0, 0, 0, 1)).
const CLEAR: wgpu::Color = wgpu::Color {
    r: 0.0,
    g: 0.0,
    b: 0.0,
    a: 1.0,
};

/// gl.blendFuncSeparate (ONE, ONE, ZERO, ONE_MINUS_SRC_ALPHA) of `bindTransparency`.
pub fn blend_state() -> wgpu::BlendState {
    wgpu::BlendState {
        color: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::One,
            dst_factor: wgpu::BlendFactor::One,
            operation: wgpu::BlendOperation::Add,
        },
        alpha: wgpu::BlendComponent {
            src_factor: wgpu::BlendFactor::Zero,
            dst_factor: wgpu::BlendFactor::OneMinusSrcAlpha,
            operation: wgpu::BlendOperation::Add,
        },
    }
}

/// OIT2.js `weight` and `oit` as the second fragment entry of the main shader.
const ENTRY: &str = r#"
// OIT2.js
// https://learnopengl.com/Guest-Articles/2020/OIT/Weighted-Blended
fn oit_weight(z: f32, a: f32) -> f32 {
    return clamp(pow(min(1.0, a * 10.0) + 0.01, 3.0) * 1e8 * pow(1.0 - z * 0.9, 3.0), 1e-2, 3e3);
}

fn oit_colors(color: vec4<f32>, z: f32) -> array<vec4<f32>, 2> {
    let a = color.a;
    let w = oit_weight(z, a);
    // finalColor .rgb *= a; finalColor *= w;
    let weighted = vec4<f32>(color.rgb * a, a) * w;

    // x3d_FragData0 = vec4 (finalColor .rgb, a); x3d_FragData1 = vec4 (finalColor .a);
    return array<vec4<f32>, 2>(vec4<f32>(weighted.rgb, a), vec4<f32>(weighted.a));
}
"#;

/// `x3d.wgsl` (with the patches of a depth variant) with its fragment entry `fs_main` turned into `shade_main` and the
/// entry `fs_oit` added: the two color outputs of OIT2.js `oit`. The logarithmic depth variant keeps its `frag_depth`
/// (Fragment2.glsl.js calls `logarithmic` after `oit`).
///
/// `Err` when the source has no `fs_main` entry of either known form (the shader changed without this patch): the caller
/// draws without OIT instead of failing a frame.
pub fn patch_shader(src: &str) -> Result<String, String> {
    let plain = "@fragment\nfn fs_main(in: VOut, @builtin(front_facing) front_facing: bool) -> @location(0) vec4<f32> {";
    let log =
        "@fragment\nfn fs_main(in: VOut, @builtin(front_facing) front_facing: bool) -> FOut {";
    let (head, logarithmic) = if src.matches(plain).count() == 1 {
        (plain, false)
    } else if src.matches(log).count() == 1 {
        (log, true)
    } else {
        return Err(
            "OrderIndependentTransparency: the fragment entry fs_main of x3d.wgsl was not found"
                .into(),
        );
    };
    let mut out = src.replacen(
        head,
        &format!(
            "fn shade_main(in: VOut, front_facing: bool) -> {} {{",
            if logarithmic { "FOut" } else { "vec4<f32>" }
        ),
        1,
    );

    out.push_str(ENTRY);

    if logarithmic {
        out.push_str(
            r#"
struct OitOut {
    @location(0) accum: vec4<f32>,
    @location(1) alpha: vec4<f32>,
    @builtin(frag_depth) depth: f32,
};

@fragment
fn fs_oit(in: VOut, @builtin(front_facing) front_facing: bool) -> OitOut {
    let shaded = shade_main(in, front_facing);
    let c = oit_colors(shaded.color, in.position.z);
    return OitOut(c[0], c[1], shaded.depth);
}
"#,
        );
    } else {
        out.push_str(
            r#"
struct OitOut {
    @location(0) accum: vec4<f32>,
    @location(1) alpha: vec4<f32>,
};

@fragment
fn fs_oit(in: VOut, @builtin(front_facing) front_facing: bool) -> OitOut {
    let c = oit_colors(shade_main(in, front_facing), in.position.z);
    return OitOut(c[0], c[1]);
}
"#,
        );
    }

    Ok(out)
}

/// The accumulation targets of one target size and sample count.
struct Targets {
    width: u32,
    height: u32,
    samples: u32,
    /// The multisampled targets (`samples > 1`): drawn into, resolved into `accum` and `alpha` (blitFramebuffer).
    multisampled: Option<[wgpu::TextureView; 2]>,
    accum: wgpu::TextureView,
    alpha: wgpu::TextureView,
    group: wgpu::BindGroup,
}

/// The OIT targets, the compose shader and its pipelines.
pub struct Oit {
    layout: wgpu::BindGroupLayout,
    pipeline_layout: wgpu::PipelineLayout,
    shader: wgpu::ShaderModule,
    pipelines: HashMap<(wgpu::TextureFormat, u32), wgpu::RenderPipeline>,
    targets: Option<Targets>,
}

impl Oit {
    pub fn new(device: &wgpu::Device) -> Oit {
        let texture = |binding| wgpu::BindGroupLayoutEntry {
            binding,
            visibility: wgpu::ShaderStages::FRAGMENT,
            ty: wgpu::BindingType::Texture {
                sample_type: wgpu::TextureSampleType::Float { filterable: false },
                view_dimension: wgpu::TextureViewDimension::D2,
                multisampled: false,
            },
            count: None,
        };
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("x3d oit compose"),
            entries: &[texture(0), texture(1)],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("x3d oit compose"),
            bind_group_layouts: &[Some(&layout)],
            ..Default::default()
        });
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("oit.wgsl"),
            source: wgpu::ShaderSource::Wgsl(include_str!("oit.wgsl").into()),
        });

        Oit {
            layout,
            pipeline_layout,
            shader,
            pipelines: HashMap::new(),
            targets: None,
        }
    }

    /// The targets for a `width` x `height` color target drawn with `samples`, and the compose pipeline for its
    /// `format` (MultiSampleFramebuffer: created again when the size or the sample count changes).
    pub fn prepare(
        &mut self,
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        samples: u32,
        width: u32,
        height: u32,
    ) {
        if self
            .targets
            .as_ref()
            .is_none_or(|t| t.width != width || t.height != height || t.samples != samples)
        {
            let make = |label: &str, samples: u32, binding: bool| {
                device
                    .create_texture(&wgpu::TextureDescriptor {
                        label: Some(label),
                        size: wgpu::Extent3d {
                            width,
                            height,
                            depth_or_array_layers: 1,
                        },
                        mip_level_count: 1,
                        sample_count: samples,
                        dimension: wgpu::TextureDimension::D2,
                        format: FORMAT,
                        usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                            | if binding {
                                wgpu::TextureUsages::TEXTURE_BINDING
                            } else {
                                wgpu::TextureUsages::empty()
                            },
                        view_formats: &[],
                    })
                    .create_view(&Default::default())
            };
            // gl.texImage2D (RGBA16F) textures, or the resolve of the multisampled renderbuffers.
            let accum = make("oit accum", 1, true);
            let alpha = make("oit alpha", 1, true);
            let multisampled = (samples > 1).then(|| {
                [
                    make("oit accum (multisampled)", samples, false),
                    make("oit alpha (multisampled)", samples, false),
                ]
            });
            let group = device.create_bind_group(&wgpu::BindGroupDescriptor {
                label: Some("x3d oit compose"),
                layout: &self.layout,
                entries: &[
                    wgpu::BindGroupEntry {
                        binding: 0,
                        resource: wgpu::BindingResource::TextureView(&accum),
                    },
                    wgpu::BindGroupEntry {
                        binding: 1,
                        resource: wgpu::BindingResource::TextureView(&alpha),
                    },
                ],
            });

            self.targets = Some(Targets {
                width,
                height,
                samples,
                multisampled,
                accum,
                alpha,
                group,
            });
        }

        let key = (format, samples);

        if !self.pipelines.contains_key(&key) {
            let pipeline = device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some("x3d oit compose"),
                layout: Some(&self.pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &self.shader,
                    entry_point: Some("vs_compose"),
                    compilation_options: Default::default(),
                    buffers: &[],
                },
                primitive: wgpu::PrimitiveState::default(),
                // The composite is drawn without the depth test (gl.disable (DEPTH_TEST)).
                depth_stencil: None,
                multisample: wgpu::MultisampleState {
                    count: samples,
                    mask: !0,
                    alpha_to_coverage_enabled: false,
                },
                fragment: Some(wgpu::FragmentState {
                    module: &self.shader,
                    entry_point: Some("fs_compose"),
                    compilation_options: Default::default(),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        // gl.blendFunc (ONE, ONE_MINUS_SRC_ALPHA)
                        blend: Some(wgpu::BlendState {
                            color: wgpu::BlendComponent {
                                src_factor: wgpu::BlendFactor::One,
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
            });
            self.pipelines.insert(key, pipeline);
        }
    }

    /// The color attachments of the transparent pass (`bindTransparency`: both targets cleared to (0, 0, 0, 1); a
    /// multisampled target is resolved into its texture, the samples are not kept).
    pub fn color_attachments(&self) -> [Option<wgpu::RenderPassColorAttachment<'_>>; 2] {
        let t = self.targets.as_ref().expect("OIT targets are prepared");
        fn one<'a>(
            multisampled: Option<&'a wgpu::TextureView>,
            resolved: &'a wgpu::TextureView,
        ) -> Option<wgpu::RenderPassColorAttachment<'a>> {
            Some(wgpu::RenderPassColorAttachment {
                view: multisampled.unwrap_or(resolved),
                depth_slice: None,
                resolve_target: multisampled.map(|_| resolved),
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(CLEAR),
                    store: if multisampled.is_some() {
                        wgpu::StoreOp::Discard
                    } else {
                        wgpu::StoreOp::Store
                    },
                },
            })
        }
        let ms = t.multisampled.as_ref();

        [
            one(ms.map(|m| &m[0]), &t.accum),
            one(ms.map(|m| &m[1]), &t.alpha),
        ]
    }

    /// `MultiSampleFramebuffer.compose`: the full-screen composite over the color target of `pass`.
    pub fn compose(
        &self,
        pass: &mut wgpu::RenderPass<'_>,
        format: wgpu::TextureFormat,
        samples: u32,
    ) {
        let t = self.targets.as_ref().expect("OIT targets are prepared");

        pass.set_pipeline(&self.pipelines[&(format, samples)]);
        pass.set_bind_group(0, &t.group, &[]);
        pass.draw(0..6, 0..1);
    }
}

#[cfg(test)]
mod tests {
    use super::patch_shader;

    #[test]
    fn a_source_without_the_fragment_entry_is_an_error_not_a_panic() {
        assert!(patch_shader("fn other() {}").is_err());
    }

    #[test]
    fn the_shipped_shader_patches() {
        let out = patch_shader(include_str!("x3d.wgsl")).unwrap();

        assert!(out.contains("fn fs_oit("));
        assert!(!out.contains("fn fs_main("));
    }
}
