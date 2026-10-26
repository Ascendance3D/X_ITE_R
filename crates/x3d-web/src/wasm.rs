//! The wasm32 module behind `web/x_ite_r.mjs`: a [`WebBrowser`] owns the WebGPU device of one `<x3d-canvas>`, the
//! renderer, and the loaded world ([`Session`]). The page's adapter fetches the world, runs the
//! `requestAnimationFrame` loop and calls [`WebBrowser::frame`]; everything X3D happens here.

use crate::session::Session;
use wasm_bindgen::prelude::*;
use x3d_render::gpu::Renderer;

/// Serves the world's Inline, EXTERNPROTO and texture URLs with a synchronous XMLHttpRequest: the runtime's
/// [`x3d_runtime::Fetcher`] is synchronous. The bytes come through `x-user-defined`, the one binary-safe text
/// decoding a synchronous request may use in a window.
struct XhrFetcher;

impl x3d_runtime::Fetcher for XhrFetcher {
    fn fetch(&self, url: &url::Url) -> Result<Vec<u8>, String> {
        let error = |e: JsValue| format!("{e:?}");
        let xhr = web_sys::XmlHttpRequest::new().map_err(error)?;

        xhr.open_with_async("GET", url.as_str(), false)
            .map_err(error)?;
        xhr.override_mime_type("text/plain; charset=x-user-defined")
            .map_err(error)?;
        xhr.send().map_err(error)?;

        let status = xhr.status().map_err(error)?;

        if !(200..300).contains(&status) && status != 0 {
            return Err(format!(
                "{status} {}",
                xhr.status_text().unwrap_or_default()
            ));
        }

        let text = xhr.response_text().map_err(error)?.unwrap_or_default();

        // x-user-defined maps byte 0x80 + n to U+F780 + n.
        Ok(text.chars().map(|c| c as u32 as u8).collect())
    }
}

#[wasm_bindgen]
pub struct WebBrowser {
    surface: wgpu::Surface<'static>,
    device: wgpu::Device,
    config: wgpu::SurfaceConfiguration,
    view_format: wgpu::TextureFormat,
    renderer: Renderer,
    adapter_info: String,
    session: Option<Session>,
}

#[wasm_bindgen]
impl WebBrowser {
    /// The device and surface of `canvas` (WebGPU). Fails when the page has no WebGPU adapter.
    pub async fn create(canvas: web_sys::HtmlCanvasElement) -> Result<WebBrowser, JsError> {
        let (width, height) = (canvas.width().max(1), canvas.height().max(1));
        let instance = wgpu::Instance::new(wgpu::InstanceDescriptor {
            backends: wgpu::Backends::BROWSER_WEBGPU,
            ..wgpu::InstanceDescriptor::new_without_display_handle()
        });
        let surface = instance
            .create_surface(wgpu::SurfaceTarget::Canvas(canvas))
            .map_err(|e| JsError::new(&format!("create_surface: {e}")))?;
        let adapter = instance
            .request_adapter(&wgpu::RequestAdapterOptions {
                power_preference: wgpu::PowerPreference::HighPerformance,
                force_fallback_adapter: false,
                compatible_surface: Some(&surface),
                ..Default::default()
            })
            .await
            .map_err(|e| JsError::new(&format!("no WebGPU adapter: {e}")))?;
        let (device, queue) = adapter
            .request_device(&wgpu::DeviceDescriptor {
                label: Some("x3d-web"),
                required_features: Renderer::wanted_features(&adapter),
                required_limits: adapter.limits(),
                ..Default::default()
            })
            .await
            .map_err(|e| JsError::new(&format!("request_device: {e}")))?;

        device.on_uncaptured_error(std::sync::Arc::new(|e| {
            web_sys::console::error_1(&format!("WebGPU: {e}").into());
        }));

        // The shaders write display-encoded values: draw through a non-sRGB view (see x3d-view's window code).
        let caps = surface.get_capabilities(&adapter);
        let format = caps
            .formats
            .iter()
            .copied()
            .find(|f| x3d_render::gpu::target_format_ok(f.remove_srgb_suffix()))
            .ok_or_else(|| {
                JsError::new(&format!("no usable canvas format in {:?}", caps.formats))
            })?;
        let view_format = format.remove_srgb_suffix();
        let config = wgpu::SurfaceConfiguration {
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT,
            format,
            width,
            height,
            present_mode: wgpu::PresentMode::Fifo,
            alpha_mode: if caps.alpha_modes.contains(&wgpu::CompositeAlphaMode::Opaque) {
                wgpu::CompositeAlphaMode::Opaque
            } else {
                caps.alpha_modes[0]
            },
            view_formats: if view_format != format {
                vec![view_format]
            } else {
                vec![]
            },
            desired_maximum_frame_latency: 2,
            color_space: wgpu::SurfaceColorSpace::Auto,
        };

        surface.configure(&device, &config);

        let mut renderer = Renderer::new(device.clone(), queue);
        let info = adapter.get_info();

        renderer.set_supported_samples(Renderer::supported_sample_counts(
            &adapter,
            &device,
            view_format,
        ));
        renderer.set_sample_count(crate::DEFAULT_MULTISAMPLING);

        Ok(WebBrowser {
            surface,
            device,
            config,
            view_format,
            renderer,
            adapter_info: format!("{} ({:?}, {:?})", info.name, info.backend, info.device_type),
            session: None,
        })
    }

    /// The WebGPU adapter: name, backend, device type.
    #[wasm_bindgen(getter, js_name = adapterInfo)]
    pub fn adapter_info(&self) -> String {
        self.adapter_info.clone()
    }

    /// Replace the world with `data` from `url` at the browser time `now`. When the new world does not parse, the
    /// error is returned and the current world stays (X3DBrowser.loadURL: no replaceWorld without a scene).
    pub fn load(&mut self, data: &[u8], url: &str, now: f64) -> Result<(), JsError> {
        let next = Session::load(
            data,
            url,
            Box::new(XhrFetcher),
            now,
            self.config.width,
            self.config.height,
        )
        .map_err(|e| JsError::new(&e))?;

        // The old world goes first (its shutdown), then the new one is drawn with its own rendering properties.
        self.session = None;
        self.session = Some(next);

        if let Some(s) = self.session.as_mut() {
            s.viewer().rendering_properties().apply(&mut self.renderer);
        }

        Ok(())
    }

    /// Unload the world, if any.
    pub fn unload(&mut self) {
        self.session = None;
    }

    #[wasm_bindgen(getter, js_name = hasWorld)]
    pub fn has_world(&self) -> bool {
        self.session.is_some()
    }

    #[wasm_bindgen(getter, js_name = worldURL)]
    pub fn world_url(&self) -> Option<String> {
        self.session.as_ref().map(|s| s.world_url().to_string())
    }

    /// The node id of the world's DEF name, or `undefined`.
    #[wasm_bindgen(js_name = namedNode)]
    pub fn named_node(&self, name: &str) -> Option<u32> {
        self.session.as_ref()?.named_node(name)
    }

    /// The world's root nodes as node ids; -1 is NULL.
    #[wasm_bindgen(js_name = rootNodes)]
    pub fn root_nodes(&self) -> Vec<i64> {
        self.session.as_ref().map_or(vec![], |s| {
            s.root_nodes()
                .into_iter()
                .map(|n| n.map_or(-1, i64::from))
                .collect()
        })
    }

    #[wasm_bindgen(js_name = typeName)]
    pub fn type_name(&self, node: u32) -> Option<String> {
        self.session.as_ref()?.type_name(node)
    }

    #[wasm_bindgen(js_name = nodeName)]
    pub fn node_name(&self, node: u32) -> Option<String> {
        self.session.as_ref()?.node_name(node)
    }

    #[wasm_bindgen(js_name = fieldNames)]
    pub fn field_names(&self, node: u32) -> Vec<String> {
        self.session
            .as_ref()
            .map_or(vec![], |s| s.field_names(node))
    }

    /// The node ids of an SFNode or MFNode field; -1 is NULL.
    #[wasm_bindgen(js_name = nodeValue)]
    pub fn node_value(&self, node: u32, name: &str) -> Result<Vec<i64>, JsError> {
        let s = self
            .session
            .as_ref()
            .ok_or_else(|| JsError::new("no world"))?;
        let nodes = s.node_value(node, name).map_err(|e| JsError::new(&e))?;

        Ok(nodes.into_iter().map(|n| n.map_or(-1, i64::from)).collect())
    }

    /// `[typeName, accessType, toString ()]` of the field.
    pub fn field(&self, node: u32, name: &str) -> Result<Vec<String>, JsError> {
        let s = self
            .session
            .as_ref()
            .ok_or_else(|| JsError::new("no world"))?;
        let f = s.field(node, name).map_err(|e| JsError::new(&e))?;

        Ok(vec![f.type_name.to_string(), f.access.to_string(), f.value])
    }

    /// Set the field from its VRML encoding (see [`Session::set_field`]).
    #[wasm_bindgen(js_name = setField)]
    pub fn set_field(&mut self, node: u32, name: &str, vrml: &str) -> Result<(), JsError> {
        let s = self
            .session
            .as_mut()
            .ok_or_else(|| JsError::new("no world"))?;

        s.set_field(node, name, vrml).map_err(|e| JsError::new(&e))
    }

    /// Step the world to `now` and draw it into the canvas, sized `width` x `height` device pixels.
    pub fn frame(&mut self, now: f64, width: u32, height: u32) -> Result<(), JsError> {
        let (width, height) = (width.max(1), height.max(1));

        if (width, height) != (self.config.width, self.config.height) {
            self.config.width = width;
            self.config.height = height;
            self.surface.configure(&self.device, &self.config);

            if let Some(s) = self.session.as_mut() {
                s.resize(width, height);
            }
        }

        let texture = match self.surface.get_current_texture() {
            wgpu::CurrentSurfaceTexture::Success(t)
            | wgpu::CurrentSurfaceTexture::Suboptimal(t) => t,
            other => return Err(JsError::new(&format!("canvas texture: {other:?}"))),
        };
        let view = texture.texture.create_view(&wgpu::TextureViewDescriptor {
            format: Some(self.view_format),
            ..Default::default()
        });

        match self.session.as_mut() {
            Some(s) => {
                s.step(now);

                let changes = s.viewer().take_rendering_changes();

                changes.apply(&mut self.renderer);

                let frame = s.build_frame();

                self.renderer
                    .render(&frame, &view, self.view_format, width, height)
                    .map_err(|e| JsError::new(&e))?;
            }
            None => self
                .renderer
                .render(
                    &crate::empty_frame(),
                    &view,
                    self.view_format,
                    width,
                    height,
                )
                .map_err(|e| JsError::new(&e))?,
        }

        self.renderer.queue().present(texture);
        Ok(())
    }

    /// Draw the current state (no stepping) once more into an RGBA8 texture of the canvas size and read it back: a
    /// Promise of the pixels as the canvas shows them, top row first. For tests and diagnostics: a page that cannot read the WebGPU canvas
    /// (headless Chromium without a display compositor) still gets the picture the renderer made.
    #[wasm_bindgen(js_name = readPixels)]
    pub fn read_pixels(&mut self) -> Result<js_sys::Promise, JsError> {
        let (width, height) = (self.config.width, self.config.height);
        let format = wgpu::TextureFormat::Rgba8Unorm;
        let frame = match self.session.as_mut() {
            Some(s) => s.build_frame(),
            None => crate::empty_frame(),
        };
        let texture = self.device.create_texture(&wgpu::TextureDescriptor {
            label: Some("x3d-web readPixels"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });

        self.renderer
            .render(
                &frame,
                &texture.create_view(&Default::default()),
                format,
                width,
                height,
            )
            .map_err(|e| JsError::new(&e))?;

        let row = width * 4;
        let padded =
            row.div_ceil(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT) * wgpu::COPY_BYTES_PER_ROW_ALIGNMENT;
        let buffer = self.device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("x3d-web readPixels"),
            size: u64::from(padded) * u64::from(height),
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let mut encoder = self.device.create_command_encoder(&Default::default());

        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(padded),
                    rows_per_image: Some(height),
                },
            },
            texture.size(),
        );
        self.renderer.queue().submit([encoder.finish()]);

        // An opaque canvas shows every pixel with alpha 1, whatever the framebuffer alpha is (the render oracle reads
        // X_ITE's opaque WebGL canvas the same way).
        let opaque = self.config.alpha_mode == wgpu::CompositeAlphaMode::Opaque;

        Ok(js_sys::Promise::new(&mut |resolve, reject| {
            let mapped = buffer.clone();

            buffer
                .slice(..)
                .map_async(wgpu::MapMode::Read, move |result| {
                    if let Err(e) = result {
                        let _ = reject.call1(&JsValue::NULL, &JsError::new(&e.to_string()).into());
                        return;
                    }

                    let data = match mapped.slice(..).get_mapped_range() {
                        Ok(data) => data,
                        Err(e) => {
                            let _ =
                                reject.call1(&JsValue::NULL, &JsError::new(&e.to_string()).into());
                            return;
                        }
                    };
                    let mut rgba = Vec::with_capacity((row * height) as usize);

                    for y in 0..height as usize {
                        let start = y * padded as usize;

                        rgba.extend_from_slice(&data[start..start + row as usize]);
                    }

                    drop(data);
                    mapped.unmap();

                    if opaque {
                        rgba.iter_mut().skip(3).step_by(4).for_each(|a| *a = 255);
                    }

                    let _ =
                        resolve.call1(&JsValue::NULL, &js_sys::Uint8Array::from(&rgba[..]).into());
                });
        }))
    }

    /// The runtime's log and Browser.print output since the last call.
    #[wasm_bindgen(js_name = takeLog)]
    pub fn take_log(&mut self) -> Vec<String> {
        self.session.as_mut().map_or(vec![], |s| s.take_log())
    }

    /// Unload the world and release the canvas configuration. The object is unusable afterwards.
    pub fn dispose(&mut self) {
        self.session = None;
    }
}
