//! Buffers, buffer views, accessors, samplers, images and textures of GLTF2Parser.js.

use super::util::*;
use super::*;

/// The `MaxAnisotropicDegree` rendering property of the oracle's headless Chromium (WebGL2).
pub(super) const MAX_ANISOTROPIC_DEGREE: f64 = 16.0;

fn undefined_read(key: &str) -> String {
    format!("TypeError: Cannot read properties of undefined (reading '{key}')")
}

impl<'j, 's> P<'j, 's> {
    // --- buffers -----------------------------------------------------------------------------------

    pub(super) fn buffers_array(&mut self, buffers: V<'j>) -> R<()> {
        let Some(list) = buffers.arr() else {
            return Ok(());
        };
        let previous = std::mem::take(&mut self.buffers);
        let mut out = Vec::with_capacity(list.len());
        let mut first_error = None;

        // `Promise .all (buffers .map (...))` (GLTF2Parser.js:600): every buffer is loaded, also
        // after one has failed, and the array fails with an error.
        for (i, buffer) in list.iter().enumerate() {
            let buffer = V::new(buffer);

            match self.buffer_object(buffer, i, &previous) {
                Ok(data) => out.push(data),
                Err(e) => {
                    first_error.get_or_insert(e);
                    out.push(None);
                }
            }
        }

        if let Some(e) = first_error {
            return Err(e);
        }

        self.buffers = out;
        Ok(())
    }

    fn buffer_object(
        &mut self,
        buffer: V<'j>,
        i: usize,
        previous: &[Option<Rc<Vec<u8>>>],
    ) -> R<Option<Rc<Vec<u8>>>> {
        if !buffer.is_obj() {
            return Ok(None);
        }

        let uri = buffer.get("uri");

        if !uri.truthy() {
            return Ok(previous.get(i).cloned().flatten());
        }

        let data = (self.fetch)(&uri.to_js_string(), &self.base_url)?;

        Ok(Some(Rc::new(data)))
    }

    // --- buffer views ------------------------------------------------------------------------------

    pub(super) fn buffer_views_array(&mut self, views: V<'j>) {
        if let Some(list) = views.arr() {
            self.buffer_views = views;
            self.bv_cache = vec![None; list.len()];
        }
    }

    fn buffer_view(&self, index: V) -> Option<usize> {
        self.buffer_views.arr().and_then(|a| index.index(a.len()))
    }

    /// `bufferView .getBuffer (componentType)`; `Ok (None)` is `undefined`.
    pub(super) fn buffer_view_buffer(
        &mut self,
        index: usize,
        component_type: Option<Ct>,
    ) -> R<Option<Rc<Vec<u8>>>> {
        let view = self.buffer_views.at(index);

        if !view.is_obj() {
            return Ok(None);
        }

        if let Some(cached) = &self.bv_cache[index] {
            return Ok(Some(cached.clone()));
        }

        let ext = view.get("extensions");
        let meshopt = {
            let a = ext.get("KHR_meshopt_compression");
            if a.nn() {
                a
            } else {
                ext.get("EXT_meshopt_compression")
            }
        };

        if meshopt.is_obj() {
            let byte_offset = if meshopt.get("byteOffset").truthy() {
                meshopt.get("byteOffset").to_number()
            } else {
                0.0
            };
            let byte_length = meshopt.get("byteLength");
            let byte_stride = meshopt.get("byteStride");
            let count = meshopt.get("count");
            let mode = meshopt.get("mode");
            let filter = if meshopt.get("filter").truthy() {
                meshopt.get("filter").to_js_string()
            } else {
                "NONE".into()
            };
            let buffer = meshopt
                .get("buffer")
                .index(self.buffers.len())
                .and_then(|i| self.buffers[i].clone());

            if !self.meshopt {
                return Err(
                    "TypeError: Cannot read properties of undefined (reading 'decodeGltfBuffer')"
                        .into(),
                );
            }

            let stride = byte_stride.to_number();
            let n = count.to_number();
            let size = component_type.map(|c| c.size() as f64).unwrap_or(f64::NAN);
            let component_count = stride / size;
            let view_length = n * component_count * size;
            let encoded = typed_array(
                Ct::U8,
                buffer.as_deref().map(|b| b.as_slice()),
                byte_offset,
                Some(byte_length.to_number()),
            )?;
            let decoded_len = to_index(view_length)?;
            let encoded: Vec<u8> = encoded.v.iter().map(|v| *v as u8).collect();
            let decoded = super::meshopt::decode_gltf_buffer(
                decoded_len,
                n as usize,
                stride as usize,
                &encoded,
                mode.to_js_string().as_str(),
                filter.as_str(),
            )?;
            let decoded = Rc::new(decoded);

            self.bv_cache[index] = Some(decoded.clone());
            return Ok(Some(decoded));
        }

        let buffer = view
            .get("buffer")
            .index(self.buffers.len())
            .and_then(|i| self.buffers[i].clone());
        let Some(buffer) = buffer else {
            return Ok(None);
        };

        let byte_offset = if view.get("byteOffset").truthy() {
            view.get("byteOffset").to_number()
        } else {
            0.0
        };
        let byte_length = view.get("byteLength").num().unwrap_or(f64::NAN);
        let (s, e) = js_slice(buffer.len(), byte_offset, byte_offset + byte_length);
        let slice = Rc::new(buffer[s..e].to_vec());

        self.bv_cache[index] = Some(slice.clone());
        Ok(Some(slice))
    }

    // --- accessors ----------------------------------------------------------------------------------

    pub(super) fn accessors_array(&mut self, accessors: V<'j>) {
        if let Some(list) = accessors.arr() {
            self.accessors = accessors;
            self.acc = (0..list.len()).map(|_| Acc::default()).collect();
        }
    }

    /// `this .accessors [index]` when it is an Object (an accessor with an `array` getter).
    pub(super) fn accessor(&self, index: V) -> Option<usize> {
        let a = self.accessors.arr()?;
        let i = index.index(a.len())?;

        V::new(&a[i]).is_obj().then_some(i)
    }

    pub(super) fn acc_json(&self, i: usize) -> V<'j> {
        self.accessors.at(i)
    }

    /// `accessor .array`.
    pub(super) fn accessor_array(&mut self, i: usize) -> R<Rc<TArr>> {
        if let Some(a) = &self.acc[i].array {
            return Ok(a.clone());
        }

        let accessor = self.acc_json(i);
        let ct = Ct::from_component_type(accessor.get("componentType"));
        let bv_index = {
            let b = accessor.get("bufferView");
            if b.truthy() {
                b
            } else {
                V::new(&J::Number(0.0))
            }
        };
        let bv = self.buffer_view(bv_index);
        let byte_offset = if accessor.get("byteOffset").truthy() {
            accessor.get("byteOffset").to_number()
        } else {
            0.0
        };

        let Some(bv) = bv else {
            return Err(undefined_read("byteStride"));
        };

        let view = self.buffer_views.at(bv);

        if !view.is_obj() {
            return Err(undefined_read("byteStride"));
        }

        let byte_stride = if view.get("byteStride").truthy() {
            view.get("byteStride").to_number()
        } else {
            0.0
        };
        let components = match accessor.get("type").str() {
            Some("SCALAR") => Some(1.0),
            Some("VEC2") => Some(2.0),
            Some("VEC3") => Some(3.0),
            Some("VEC4") => Some(4.0),
            Some("MAT2") => Some(4.0),
            Some("MAT3") => Some(9.0),
            Some("MAT4") => Some(16.0),
            _ => None,
        };
        let count = if accessor.get("count").truthy() {
            accessor.get("count").to_number()
        } else {
            0.0
        };

        let stride = if byte_stride != 0.0 && !byte_stride.is_nan() {
            let Some(ct) = ct else {
                return Err(undefined_read("BYTES_PER_ELEMENT"));
            };
            byte_stride / ct.size() as f64
        } else {
            components.unwrap_or(f64::NAN)
        };

        let Some(ct) = ct else {
            return Err(undefined_read("BYTES_PER_ELEMENT"));
        };

        let bv_len = view.get("byteLength").num().unwrap_or(f64::NAN);
        let length = js_min(stride * count, (bv_len - byte_offset) / ct.size() as f64);
        let buffer = self.buffer_view_buffer(bv, Some(ct))?;
        let array = typed_array(
            ct,
            buffer.as_deref().map(|b| b.as_slice()),
            byte_offset,
            Some(length),
        )?;

        let components_n = components.unwrap_or(f64::NAN);
        let value = self.dense_array(ct, components_n, count, stride, array);
        let value = self.sparse_object(accessor, components_n, value)?;
        let value = normalized_array(accessor, value);
        let value = Rc::new(value);

        self.acc[i].array = Some(value.clone());
        Ok(value)
    }

    fn dense_array(&self, ct: Ct, components: f64, count: f64, stride: f64, array: TArr) -> TArr {
        if stride == components {
            return array;
        }

        let length = count * components;
        let length = if length.is_nan() {
            0
        } else {
            length.max(0.0) as usize
        };
        let mut dense = vec![ct.store(0.0); length];
        let (stride_n, comp_n) = (stride, components);
        let mut i = 0usize;
        let mut j = 0.0f64;

        while i < length {
            for c in 0..comp_n as usize {
                let src = j + c as f64;
                // `array [j + c]` beyond the end is undefined: NaN for floats, 0 for integers.
                let v = if src >= 0.0 && src.fract() == 0.0 && (src as usize) < array.v.len() {
                    array.v[src as usize]
                } else if ct.is_float() {
                    f64::NAN
                } else {
                    0.0
                };

                if i < length {
                    dense[i] = v;
                }
                i += 1;
            }

            j += stride_n;

            if stride_n.is_nan() || stride_n <= 0.0 && comp_n == 0.0 {
                break;
            }
        }

        TArr { ct, v: dense }
    }

    fn sparse_object(&mut self, accessor: V<'j>, components: f64, array: TArr) -> R<TArr> {
        let sparse = accessor.get("sparse");

        if !sparse.is_obj() || !sparse.get("indices").is_obj() || !sparse.get("values").is_obj() {
            return Ok(array);
        }

        let component_type = sparse.get("indices").get("componentType");
        let it = match component_type.num().map(|n| n as i64) {
            Some(5121) => Ct::U8,
            Some(5123) => Ct::U16,
            Some(5125) => Ct::U32,
            _ => {
                // `new IndicesTypedArray` is evaluated after the buffer view lookup.
                let ibv = self.buffer_view(sparse.get("indices").get("bufferView"));

                if ibv.is_none() {
                    return Err(undefined_read("getBuffer"));
                }

                return Err("TypeError: IndicesTypedArray is not a constructor".into());
            }
        };

        let Some(ibv) = self.buffer_view(sparse.get("indices").get("bufferView")) else {
            return Err(undefined_read("getBuffer"));
        };
        let ibuf = self.buffer_view_buffer(ibv, Some(it))?;
        let count = sparse.get("count").to_number();
        let indices = typed_array(
            it,
            ibuf.as_deref().map(|b| b.as_slice()),
            sparse.get("indices").get("byteOffset").to_number_or_zero(),
            Some(count),
        )?;

        let Some(vbv) = self.buffer_view(sparse.get("values").get("bufferView")) else {
            return Err(undefined_read("getBuffer"));
        };
        let vbuf = self.buffer_view_buffer(vbv, None)?;
        let values = typed_array(
            array.ct,
            vbuf.as_deref().map(|b| b.as_slice()),
            sparse.get("values").get("byteOffset").to_number_or_zero(),
            Some(count * components),
        )?;

        let mut out = array;
        let comps = components as usize;
        let mut v = 0usize;

        for idx in &indices.v {
            for c in 0..comps {
                let target = *idx as usize * comps + c;
                let value = values.v.get(v).copied().unwrap_or(if out.ct.is_float() {
                    f64::NAN
                } else {
                    0.0
                });

                if target < out.v.len() {
                    out.v[target] = out.ct.store(value);
                }

                v += 1;
            }
        }

        Ok(out)
    }

    // --- samplers --------------------------------------------------------------------------------------

    pub(super) fn samplers_array(&mut self, samplers: V<'j>) {
        if let Some(list) = samplers.arr() {
            self.samplers = samplers;
            self.sampler_nodes = vec![None; list.len()];
        }
    }

    /// `sampler .texturePropertiesNode`.
    pub(super) fn sampler_node(&mut self, i: usize) -> R<NodeId> {
        if let Some(n) = self.sampler_nodes[i] {
            return Ok(n);
        }

        let sampler = self.samplers.at(i);
        let node = self.create("TextureProperties")?;
        let name = self.sanitize(sampler.get("name"));

        if !name.is_empty() {
            self.named(&name, node)?;
        }

        let (min, mips) = match sampler.get("minFilter").num().map(|n| n as i64) {
            Some(9728) => ("NEAREST_PIXEL", false),
            Some(9729) => ("AVG_PIXEL", false),
            Some(9984) => ("NEAREST_PIXEL_NEAREST_MIPMAP", true),
            Some(9985) => ("AVG_PIXEL_NEAREST_MIPMAP", true),
            Some(9986) => ("NEAREST_PIXEL_AVG_MIPMAP", true),
            Some(9987) => ("AVG_PIXEL_AVG_MIPMAP", true),
            _ => ("AVG_PIXEL_AVG_MIPMAP", true),
        };

        self.set(node, "minificationFilter", FieldValue::SFString(min.into()));
        self.set(node, "generateMipMaps", FieldValue::SFBool(mips));

        let mag = match sampler.get("magFilter").num().map(|n| n as i64) {
            Some(9728) => "NEAREST_PIXEL",
            _ => "AVG_PIXEL",
        };

        self.set(
            node,
            "magnificationFilter",
            FieldValue::SFString(mag.into()),
        );

        let boundary = |v: V| match v.num().map(|n| n as i64) {
            Some(33071) => "CLAMP_TO_EDGE",
            Some(33648) => "MIRRORED_REPEAT",
            _ => "REPEAT",
        };

        self.set(
            node,
            "boundaryModeS",
            FieldValue::SFString(boundary(sampler.get("wrapS")).into()),
        );
        self.set(
            node,
            "boundaryModeT",
            FieldValue::SFString(boundary(sampler.get("wrapT")).into()),
        );
        self.set(
            node,
            "anisotropicDegree",
            FieldValue::SFFloat(MAX_ANISOTROPIC_DEGREE),
        );

        self.setup(node);
        self.sampler_nodes[i] = Some(node);
        Ok(node)
    }

    // --- images -------------------------------------------------------------------------------------------

    pub(super) fn images_array(&mut self, images: V<'j>) -> R<()> {
        let Some(list) = images.arr() else {
            return Ok(());
        };
        let mut out = Vec::with_capacity(list.len());

        for (i, image) in list.iter().enumerate() {
            out.push(self.image_object(V::new(image), i)?);
        }

        self.images = Some(out);
        Ok(())
    }

    pub(super) fn videos_array(&mut self, videos: V<'j>) -> R<()> {
        let Some(list) = videos.arr() else {
            return Ok(());
        };
        let mut out = Vec::with_capacity(list.len());

        for (i, image) in list.iter().enumerate() {
            out.push(self.image_object(V::new(image), i)?);
        }

        self.videos = Some(out);
        Ok(())
    }

    fn image_object(&mut self, image: V<'j>, index: usize) -> R<Option<Img>> {
        if !image.is_obj() {
            return Ok(None);
        }

        let name = image.get("name").str().map(str::to_string);
        let uri = image.get("uri");

        if uri.truthy() {
            return Ok(Some(Img {
                index,
                uri: Some(uri.to_js_string()),
                name,
            }));
        }

        let Some(bv) = self.buffer_view(image.get("bufferView")) else {
            return Ok(Some(Img {
                index,
                uri: None,
                name,
            }));
        };

        let buffer = self.buffer_view_buffer(bv, None)?;
        let bytes: &[u8] = buffer.as_deref().map(|b| b.as_slice()).unwrap_or(&[]);

        // new Blob ([...], { type: image .mimeType }) → FileReader.readAsDataURL
        let mime = match image.get("mimeType").0 {
            Some(J::String(s)) if s.bytes().all(|b| (0x20..=0x7e).contains(&b)) => {
                s.to_ascii_lowercase()
            }
            Some(J::String(_)) | None => String::new(),
            Some(_) => {
                let s = image.get("mimeType").to_js_string();
                if s.bytes().all(|b| (0x20..=0x7e).contains(&b)) {
                    s.to_ascii_lowercase()
                } else {
                    String::new()
                }
            }
        };
        let mime = if mime.is_empty() {
            "application/octet-stream".to_string()
        } else {
            mime
        };

        Ok(Some(Img {
            index,
            uri: Some(format!("data:{mime};base64,{}", base64_encode(bytes))),
            name,
        }))
    }

    // --- textures ------------------------------------------------------------------------------------------------

    pub(super) fn textures_array(&mut self, textures: V<'j>) {
        if let Some(list) = textures.arr() {
            self.textures = Some(textures);
            self.texture_nodes = vec![None; list.len()];
        }
    }

    pub(super) fn texture_object(&mut self, index: V<'j>) -> R<Option<NodeId>> {
        // `this .textures [texture .index]`
        let Some(textures) = self.textures else {
            return Err(undefined_read(&index.to_js_string()));
        };

        let Some(ti) = index.index(textures.arr().map(|a| a.len()).unwrap_or(0)) else {
            return Ok(None);
        };
        let texture = textures.at(ti);

        if !texture.is_obj() {
            return Ok(None);
        }

        if let Some(n) = self.texture_nodes[ti] {
            return Ok(Some(n));
        }

        // textureVideoObject
        let mut videos: Vec<(usize, Option<String>, Option<String>)> = vec![];

        if self.has_extension("EXT_texture_video") {
            let source = texture
                .get("extensions")
                .get("EXT_texture_video")
                .get("source");

            let Some(list) = &self.videos else {
                return Err(undefined_read(&source.to_js_string()));
            };

            if let Some(i) = source.index(list.len()) {
                if let Some(v) = &list[i] {
                    videos.push((v.index, v.uri.clone(), v.name.clone()));
                }
            }
        }

        // textureImageObject
        let mut images: Vec<(usize, Option<String>, Option<String>)> = vec![];

        let ext = texture.get("extensions");

        if ext.is_obj() {
            for (key, extension) in ext.entries() {
                if !self.has_extension(&key) {
                    continue;
                }

                if matches!(
                    key.as_str(),
                    "KHR_texture_basisu" | "EXT_texture_avif" | "EXT_texture_webp"
                ) {
                    let source = extension.get("source");
                    let Some(list) = &self.images else {
                        return Err(undefined_read(&source.to_js_string()));
                    };

                    if let Some(i) = source.index(list.len()) {
                        if let Some(v) = &list[i] {
                            images.push((v.index, v.uri.clone(), v.name.clone()));
                        }
                    }
                }
            }
        }

        {
            let source = texture.get("source");
            let Some(list) = &self.images else {
                return Err(undefined_read(&source.to_js_string()));
            };

            if let Some(i) = source.index(list.len()) {
                if let Some(v) = &list[i] {
                    images.push((v.index, v.uri.clone(), v.name.clone()));
                }
            }
        }

        let is_video = !videos.is_empty();
        let medias = if is_video { videos } else { images };

        if medias.is_empty() {
            return Ok(None);
        }

        let key = format!(
            "{}:{}",
            medias
                .iter()
                .map(|m| m.0.to_string())
                .collect::<Vec<_>>()
                .join(","),
            texture.get("sampler").to_js_string()
        );

        if let Some(cached) = self.texture_cache.get(&key) {
            let cached = *cached;

            self.texture_nodes[ti] = Some(cached);
            return Ok(Some(cached));
        }

        let node = self.create(if is_video {
            "MovieTexture"
        } else {
            "ImageTexture"
        })?;
        let tname = texture.get("name");
        let name = if tname.truthy() {
            self.sanitize(tname)
        } else {
            self.sanitize_str(medias[0].2.as_deref().unwrap_or(""))
        };

        if !name.is_empty() {
            self.named(&name, node)?;
        }

        self.set(
            node,
            "url",
            FieldValue::MFString(
                medias
                    .iter()
                    .map(|m| m.1.clone().unwrap_or_default())
                    .collect(),
            ),
        );

        // MovieTexture has no such field; the JS assignment is a plain property write.
        if !is_video {
            self.set(node, "colorSpaceConversion", FieldValue::SFBool(false));
        }

        if is_video {
            if !name.is_empty() {
                self.exported(&name, node)?;
            }

            self.set(node, "loop", FieldValue::SFBool(true));
        }

        let sampler = texture.get("sampler");

        if let Some(si) = sampler.index(self.samplers.arr().map(|a| a.len()).unwrap_or(0)) {
            if self.samplers.at(si).is_obj() {
                let tp = self.sampler_node(si)?;

                self.set_node(node, "textureProperties", Some(tp));
            }
        }

        self.setup(node);
        self.texture_cache.insert(key, node);
        self.texture_nodes[ti] = Some(node);
        Ok(Some(node))
    }
}

fn js_min(a: f64, b: f64) -> f64 {
    x3d_fields::jsmath::js_min(a, b)
}

/// `normalizedArray`.
fn normalized_array(accessor: V, array: TArr) -> TArr {
    if !accessor.get("normalized").truthy() {
        return array;
    }

    let f = |v: f64| v as f32 as f64;
    let (ct, v) = (array.ct, &array.v);

    let out: Option<Vec<f64>> = match ct {
        Ct::I8 => Some(v.iter().map(|x| f((x / 127.0).max(-1.0))).collect()),
        Ct::U8 => Some(v.iter().map(|x| f(x / 255.0)).collect()),
        Ct::I16 => Some(v.iter().map(|x| f((x / 32767.0).max(-1.0))).collect()),
        Ct::U16 => Some(v.iter().map(|x| f(x / 65535.0)).collect()),
        Ct::I32 => Some(v.iter().map(|x| f((x / 2147483647.0).max(-1.0))).collect()),
        Ct::U32 => Some(v.iter().map(|x| f(x / 4294967295.0)).collect()),
        Ct::F16 | Ct::F32 | Ct::F64 => None,
    };

    match out {
        Some(v) => TArr { ct: Ct::F32, v },
        None => array,
    }
}
