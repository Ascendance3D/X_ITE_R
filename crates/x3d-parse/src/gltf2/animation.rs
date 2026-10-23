//! Animations, interpolators, morph target weights and KHR_animation_pointer of GLTF2Parser.js.

use super::util::*;
use super::*;
use x3d_fields::math::{Vector2, Vector4};
use x3d_fields::types::{FieldType, access};

const SAMPLES_PER_SECOND: f64 = 30.0;

fn undefined_read(key: &str) -> String {
    format!("TypeError: Cannot read properties of undefined (reading '{key}')")
}

/// Key values: a typed array (has `subarray`) or a plain array (`filter` result).
struct Kv {
    v: Vec<f64>,
    typed: bool,
}

enum Interp {
    Step,
    Linear,
    Cubic,
}

fn interpolation(v: V) -> Interp {
    match v.str() {
        Some("STEP") => Interp::Step,
        Some("CUBICSPLINE") => Interp::Cubic,
        _ => Interp::Linear,
    }
}

/// `Algorithm.upperBound (times, 0, times.length, value)`
fn upper_bound(times: &[f64], value: f64) -> usize {
    let (mut first, mut count) = (0usize, times.len());

    while count > 0 {
        let step = count >> 1;
        let index = first + step;

        if value < times[index] {
            count = step;
        } else {
            first = index + 1;
            count -= step + 1;
        }
    }

    first
}

fn clamp_index(v: usize, min: usize, max: usize) -> usize {
    v.max(min).min(max)
}

/// `cubicSplineVector`: the spline between keys, componentwise on `n`-vectors.
fn cubic_spline_vector(time: f64, times: &[f64], values: &[Vec<f64>]) -> Vec<f64> {
    let n = times.len();
    let index1 = clamp_index(upper_bound(times, time), 1, n.saturating_sub(1));
    let index0 = index1 - 1;
    let td = times[index1] - times[index0];
    let t1 = (time - times[index0]) / td;
    let t2 = t1 * t1;
    let t3 = t2 * t1;

    let mut v0 = values[index0 * 3 + 1].clone();
    let mut b0 = values[index0 * 3 + 2].clone();
    let mut v1 = values[index1 * 3 + 1].clone();
    let mut a1 = values[index1 * 3].clone();

    let (f0, fb, f1, fa) = (
        2.0 * t3 - 3.0 * t2 + 1.0,
        td * (t3 - 2.0 * t2 + t1),
        -2.0 * t3 + 3.0 * t2,
        td * (t3 - t2),
    );

    for c in 0..v0.len() {
        v0[c] *= f0;
        b0[c] *= fb;
        v1[c] *= f1;
        a1[c] *= fa;
        v0[c] = v0[c] + b0[c] + v1[c] + a1[c];
    }

    v0
}

/// `cubicSplineScalarArray`
fn cubic_spline_scalar_array(
    time: f64,
    times: &[f64],
    values: &[f64],
    length: usize,
    i: usize,
) -> f64 {
    let n = times.len();
    let index1 = clamp_index(upper_bound(times, time), 1, n.saturating_sub(1));
    let index0 = index1 - 1;
    let td = times[index1] - times[index0];
    let t1 = (time - times[index0]) / td;
    let t2 = t1 * t1;
    let t3 = t2 * t1;
    let at = |k: usize| values.get(k).copied().unwrap_or(f64::NAN);

    let mut v0 = at((index0 + 1) * length + i);
    let mut b0 = at((index0 + 2) * length + i);
    let mut v1 = at((index1 + 1) * length + i);
    let mut a1 = at(index1 * length + i);

    v0 *= 2.0 * t3 - 3.0 * t2 + 1.0;
    b0 *= td * (t3 - 2.0 * t2 + t1);
    v1 *= -2.0 * t3 + 3.0 * t2;
    a1 *= td * (t3 - t2);

    v0 + b0 + v1 + a1
}

fn samples(times: &[f64]) -> Vec<f64> {
    let last = times.last().copied().unwrap_or(f64::NAN);
    let length = (last * SAMPLES_PER_SECOND).floor();
    let length = if length.is_nan() || length < 0.0 {
        0
    } else {
        length as usize
    };

    (0..length)
        .map(|i| i as f64 / (length as f64 - 1.0) * last)
        .collect()
}

fn quaternion_rotation(q: [f64; 4]) -> [f64; 4] {
    let mut r = Rotation4::identity();

    r.set_quaternion(&Quaternion::new(q[0], q[1], q[2], q[3]));
    r.components()
}

impl<'j, 's> P<'j, 's> {
    pub(super) fn animations_array(&mut self, animations: V<'j>) -> R<()> {
        let Some(list) = animations.arr() else {
            return Ok(());
        };

        for (i, a) in list.iter().enumerate() {
            self.animation_object(i, V::new(a))?;
        }

        Ok(())
    }

    fn animation_object(&mut self, id: usize, animation: V<'j>) -> R<()> {
        if !animation.is_obj() {
            return Ok(());
        }

        let timer = self.create("TimeSensor")?;
        let channels = self.animation_channels_array(
            animation.get("channels"),
            animation.get("samplers"),
            timer,
        )?;

        if channels.is_empty() {
            return Ok(());
        }

        let group = self.create("Group")?;
        let name = self.sanitize(animation.get("name"));
        let group_name = if name.is_empty() {
            format!("Animation{}", id + 1)
        } else {
            name
        };
        let timer_name = format!("Timer{}", id + 1);

        self.named(&group_name, group)?;
        self.named(&timer_name, timer)?;
        self.exported(&group_name, group)?;
        self.exported(&timer_name, timer)?;

        let d = description(animation.get("name").str());
        let d = if d.is_empty() {
            format!("Animation {}", id + 1)
        } else {
            d
        };

        self.set(timer, "description", FieldValue::SFString(d));

        self.push_node(group, "children", Some(timer));

        for c in channels {
            self.push_node(group, "children", Some(c));
        }

        self.setup(timer);
        self.setup(group);
        self.animation_nodes.insert(id, group);
        Ok(())
    }

    fn animation_channels_array(
        &mut self,
        channels: V<'j>,
        samplers: V<'j>,
        timer: NodeId,
    ) -> R<Vec<NodeId>> {
        let (Some(channel_list), Some(sampler_list)) = (channels.arr(), samplers.arr()) else {
            return Ok(vec![]);
        };

        // Determine cycleInterval.
        let mut cycle_interval = 0.0f64;

        for sampler in sampler_list {
            let sampler = V::new(sampler);

            if matches!(sampler.0, Some(J::Null)) {
                return Err("TypeError: Cannot read properties of null (reading 'input')".into());
            }

            let Some(a) = self.accessor(sampler.get("input")) else {
                continue;
            };
            let array = self.accessor_array(a)?;

            if array.len() > 0 {
                cycle_interval =
                    x3d_fields::jsmath::js_max(cycle_interval, *array.v.last().unwrap());
            }
        }

        self.set(timer, "cycleInterval", FieldValue::SFTime(cycle_interval));

        // Get interpolators.
        let mut out: Vec<NodeId> = vec![];

        for channel in channel_list {
            out.extend(self.animation_channel_object(
                V::new(channel),
                samplers,
                timer,
                cycle_interval,
            )?);
        }

        // Insert Script nodes after last interpolator.
        let scripts = std::mem::take(&mut self.animation_scripts);

        for script in scripts {
            let index = out.iter().rposition(|n| {
                self.scene
                    .ctx(ROOT)
                    .routes
                    .iter()
                    .any(|r| r.source_node == *n && r.destination_node == script)
            });
            let at = index.map(|i| i + 1).unwrap_or(0);

            out.insert(at, script);
        }

        Ok(out)
    }

    fn animation_channel_object(
        &mut self,
        channel: V<'j>,
        samplers: V<'j>,
        timer: NodeId,
        cycle_interval: f64,
    ) -> R<Vec<NodeId>> {
        if !channel.is_obj() {
            return Ok(vec![]);
        }

        let target = channel.get("target");

        if !target.is_obj() {
            return Ok(vec![]);
        }

        let node = target
            .get("node")
            .index(self.nodes.len())
            .and_then(|i| self.nodes[i].transform);

        if node.is_none() && target.get("path").str() != Some("pointer") {
            return Ok(vec![]);
        }

        let sampler = samplers
            .arr()
            .and_then(|a| channel.get("sampler").index(a.len()).map(|i| V::new(&a[i])));

        let Some(sampler) = sampler.filter(|s| s.truthy()) else {
            return Ok(vec![]);
        };

        let Some(input) = self.accessor(sampler.get("input")) else {
            return Ok(vec![]);
        };
        let times = self.accessor_array(input)?;

        if times.len() == 0 {
            return Ok(vec![]);
        }

        let Some(output) = self.accessor(sampler.get("output")) else {
            return Ok(vec![]);
        };
        let values = self.accessor_array(output)?;

        if values.len() == 0 {
            return Ok(vec![]);
        }

        self.create_interpolator(
            timer,
            node,
            target,
            sampler.get("interpolation"),
            &times,
            output,
            &values,
            cycle_interval,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn create_interpolator(
        &mut self,
        timer: NodeId,
        node: Option<NodeId>,
        target: V<'j>,
        interpolation_v: V<'j>,
        times: &TArr,
        output: usize,
        values: &TArr,
        cycle_interval: f64,
    ) -> R<Vec<NodeId>> {
        self.times_ct = times.ct;
        let path = target.get("path").str().unwrap_or("").to_string();

        match path.as_str() {
            "pointer" => {
                let pointer = target
                    .get("extensions")
                    .get("KHR_animation_pointer")
                    .get("pointer");
                let (n, f) = self.get_animation_pointer(pointer)?;

                self.pointer_interpolator(
                    timer,
                    n,
                    f,
                    interpolation_v,
                    times,
                    values,
                    cycle_interval,
                )
            }
            "translation" | "rotation" | "scale" => {
                let Some(n) = node else { return Ok(vec![]) };
                let field = self
                    .scene
                    .get_field(n, &path)
                    .map(|loc| self.scene.field(n, loc).name.clone());

                match field {
                    Some(f) => self.pointer_interpolator(
                        timer,
                        Some(n),
                        Some(f),
                        interpolation_v,
                        times,
                        values,
                        cycle_interval,
                    ),
                    None => Err(format!(
                        "Error: Unknown field '{path}' in node class {}.",
                        self.type_name(n)
                    )),
                }
            }
            "weights" => self.weights_interpolators(
                timer,
                target,
                interpolation_v,
                times,
                values,
                cycle_interval,
                output,
            ),
            _ => Ok(vec![]),
        }
    }

    #[allow(clippy::too_many_arguments)]
    fn weights_interpolators(
        &mut self,
        timer: NodeId,
        target: V<'j>,
        interpolation_v: V<'j>,
        times: &TArr,
        values: &TArr,
        cycle_interval: f64,
        _output: usize,
    ) -> R<Vec<NodeId>> {
        let node_index = target.get("node").index(self.nodes.len());
        let Some(ni) = node_index else {
            return Err(undefined_read("mesh"));
        };
        let mesh_index = self.nodes[ni].j.get("mesh");
        let Some(mi) = self.meshes.arr().and_then(|a| mesh_index.index(a.len())) else {
            return Ok(vec![]);
        };
        let mesh = self.meshes.at(mi);
        let Some(prims) = mesh.get("primitives").arr() else {
            return Ok(vec![]);
        };
        let mut nodes = vec![];

        for (pi, primitive) in prims.iter().enumerate() {
            let primitive = V::new(primitive);
            let p = self.prims.get(&(mi, pi));
            let shape = p.and_then(|p| p.shape);
            let Some(shape) = shape else {
                return Err(undefined_read("_geometry"));
            };

            if !self.has_field(shape, "geometry") {
                return Err(undefined_read("getValue"));
            }

            let geometry = self.get_node(shape, "geometry");
            let Some(geometry) = geometry else { continue };

            if !self.has_field(geometry, "coord") {
                continue;
            }

            let Some(coord) = self.get_node(geometry, "coord") else {
                continue;
            };
            let attr = self.prims.get(&(mi, pi)).and_then(|p| {
                p.attributes
                    .iter()
                    .find(|(k, _)| k == "POSITION")
                    .and_then(|(_, v)| *v)
            });
            let field_len = attr.and_then(|a| self.acc[a].field.as_ref().map(|f| f.len() / 3));
            let point_len = self.get(coord, "point").len();

            let Some(attr) = attr.filter(|_| field_len == Some(point_len)) else {
                continue;
            };
            let targets = primitive.get("targets");

            let Some(interp) = self.create_array_interpolator(
                "CoordinateInterpolator",
                &interpolation_v,
                times,
                values,
                cycle_interval,
                targets,
                attr,
                "POSITION",
            )?
            else {
                continue;
            };

            let key_len = self.get(interp, "key").len();
            let kv_len = self.get(interp, "keyValue").len();

            if key_len < 2 || kv_len / key_len != point_len || kv_len % key_len != 0 {
                continue;
            }

            nodes.push(interp);
            self.route(timer, "fraction_changed", interp, "set_fraction")?;
            self.route(interp, "value_changed", coord, "point")?;
        }

        for (pi, primitive) in prims.iter().enumerate() {
            let primitive = V::new(primitive);
            let p = self.prims.get(&(mi, pi));
            let shape = p.and_then(|p| p.shape);
            let Some(shape) = shape else {
                return Err(undefined_read("_geometry"));
            };
            let geometry = self.get_node(shape, "geometry");
            let Some(geometry) = geometry else { continue };

            if !self.has_field(geometry, "normal") {
                continue;
            }

            let Some(normal) = self.get_node(geometry, "normal") else {
                continue;
            };
            let attr = self.prims.get(&(mi, pi)).and_then(|p| {
                p.attributes
                    .iter()
                    .find(|(k, _)| k == "NORMAL")
                    .and_then(|(_, v)| *v)
            });
            let field_len = attr.and_then(|a| self.acc[a].field.as_ref().map(|f| f.len() / 3));
            let vec_len = self.get(normal, "vector").len();

            let Some(attr) = attr.filter(|_| field_len == Some(vec_len)) else {
                continue;
            };
            let targets = primitive.get("targets");

            let Some(interp) = self.create_array_interpolator(
                "NormalInterpolator",
                &interpolation_v,
                times,
                values,
                cycle_interval,
                targets,
                attr,
                "NORMAL",
            )?
            else {
                continue;
            };

            let key_len = self.get(interp, "key").len();
            let kv_len = self.get(interp, "keyValue").len();

            if key_len < 2 || kv_len / key_len != vec_len || kv_len % key_len != 0 {
                continue;
            }

            nodes.push(interp);
            self.route(timer, "fraction_changed", interp, "set_fraction")?;
            self.route(interp, "value_changed", normal, "vector")?;
        }

        Ok(nodes)
    }

    // --- KHR_animation_pointer ------------------------------------------------------------------

    fn alias(&self, node: NodeId, field: &str) -> Option<String> {
        self.pointer_aliases
            .get(&format!("{}.{}", self.type_name(node), field))
            .cloned()
    }

    /// `getAnimationPointer (pointer)`: the first pointer node that has the field.
    fn get_animation_pointer(&mut self, pointer: V<'j>) -> R<(Option<NodeId>, Option<String>)> {
        let text = if pointer.is_undef() {
            String::new()
        } else {
            pointer.to_js_string()
        };
        let mut path: Vec<&str> = text.split('/').filter(|p| !p.is_empty()).collect();

        let Some(last) = path.pop() else {
            return Err(undefined_read("replace"));
        };
        let field = last.strip_suffix("Factor").unwrap_or(last).to_string();

        let mut target = self.root;

        for property in path {
            target = target.get(property);
        }

        if !target.nn() {
            return Ok((None, None));
        }

        let key = addr(target);
        let nodes = match self.pointers.get(&key) {
            None => return Ok((None, None)),
            Some(Ptr::Nodes(n)) => n.clone(),
            Some(Ptr::Existing) => {
                return Err("TypeError: glTF?.pointers?.map is not a function".into());
            }
            Some(Ptr::TextureTransform {
                node,
                translation,
                rotation,
                scale,
            }) => {
                let (node, translation, rotation, scale) = (*node, *translation, *rotation, *scale);
                let script = self.texture_transform_script(node, translation, rotation, scale)?;

                self.pointers.insert(key, Ptr::Nodes(vec![script]));
                vec![script]
            }
            Some(Ptr::Ortho {
                viewpoint,
                xmag,
                ymag,
            }) => {
                let (viewpoint, xmag, ymag) = (*viewpoint, *xmag, *ymag);
                let script = self.ortho_script(viewpoint, xmag, ymag)?;

                self.pointers
                    .insert(key, Ptr::Nodes(vec![viewpoint, script]));
                vec![viewpoint, script]
            }
        };

        for node in nodes {
            let name = self.alias(node, &field).unwrap_or_else(|| field.clone());

            if let Some(loc) = self.scene.get_field(node, &name) {
                let canonical = self.scene.field(node, loc).name.clone();

                return Ok((Some(node), Some(canonical)));
            }
        }

        Ok((None, None))
    }

    fn texture_transform_script(
        &mut self,
        texture_transform: NodeId,
        translation: [f64; 2],
        rotation: f64,
        scale: [f64; 2],
    ) -> R<NodeId> {
        let script = self.create("Script")?;

        self.scene.add_user_defined_field(
            script,
            Field::new(
                "translation",
                access::INPUT_OUTPUT,
                FieldValue::SFVec2f(Vector2::new(translation[0], translation[1])),
            ),
        );
        self.scene.add_user_defined_field(
            script,
            Field::new(
                "rotation",
                access::INPUT_OUTPUT,
                FieldValue::SFFloat(rotation),
            ),
        );
        self.scene.add_user_defined_field(
            script,
            Field::new(
                "scale",
                access::INPUT_OUTPUT,
                FieldValue::SFVec2f(Vector2::new(scale[0], scale[1])),
            ),
        );
        self.scene.add_user_defined_field(
            script,
            Field::new(
                "value_changed",
                access::OUTPUT_ONLY,
                FieldValue::SFMatrix4f(Matrix4::IDENTITY),
            ),
        );
        self.set(
            script,
            "url",
            FieldValue::MFString(vec![super::scripts::TEXTURE_TRANSFORM.into()]),
        );

        self.setup(script);
        self.named("CombineTextureMatrixScript", script)?;
        self.route(script, "value_changed", texture_transform, "set_matrix")?;

        self.add_pointer_alias(script, "offset", "translation");
        self.animation_scripts.push(script);
        Ok(script)
    }

    fn ortho_script(&mut self, viewpoint: NodeId, xmag: f64, ymag: f64) -> R<NodeId> {
        let script = self.create("Script")?;

        self.scene.add_user_defined_field(
            script,
            Field::new("xmag", access::INPUT_OUTPUT, FieldValue::SFFloat(xmag)),
        );
        self.scene.add_user_defined_field(
            script,
            Field::new("ymag", access::INPUT_OUTPUT, FieldValue::SFFloat(ymag)),
        );
        self.scene.add_user_defined_field(
            script,
            Field::new(
                "value_changed",
                access::OUTPUT_ONLY,
                FieldValue::MFFloat(vec![]),
            ),
        );
        self.set(
            script,
            "url",
            FieldValue::MFString(vec![super::scripts::ORTHO.into()]),
        );

        self.setup(script);
        self.named("CombineFieldOfViewScript", script)?;
        self.route(script, "value_changed", viewpoint, "set_fieldOfView")?;

        self.animation_scripts.push(script);
        Ok(script)
    }

    // --- interpolator creation ---------------------------------------------------------------------

    #[allow(clippy::too_many_arguments)]
    fn pointer_interpolator(
        &mut self,
        timer: NodeId,
        node: Option<NodeId>,
        field: Option<String>,
        interpolation_v: V<'j>,
        times: &TArr,
        values: &TArr,
        cycle_interval: f64,
    ) -> R<Vec<NodeId>> {
        self.times_ct = times.ct;
        let (Some(node), Some(field)) = (node, field) else {
            return Ok(vec![]);
        };
        let loc = self.scene.get_field(node, &field).unwrap();
        let ty = self.scene.field(node, loc).ty();
        let interp = interpolation(interpolation_v);
        let times_v = &times.v;
        let upper = upper_first(&field);

        match ty {
            FieldType::SFColor => {
                let mut nodes = vec![];
                let ratio = (values.len() as f64 / times_v.len() as f64) % 3.0;
                let (colors, transparencies): (Kv, Option<Vec<f64>>) = if ratio == 0.0 {
                    (
                        Kv {
                            v: values.v.clone(),
                            typed: true,
                        },
                        None,
                    )
                } else {
                    let colors: Vec<f64> = values
                        .v
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| i % 4 < 3)
                        .map(|(_, v)| *v)
                        .collect();
                    let tr: Vec<f64> = values
                        .v
                        .iter()
                        .enumerate()
                        .filter(|(i, _)| i % 4 == 3)
                        .map(|(_, v)| *v)
                        .collect();
                    let tr = if tr.iter().all(|v| *v >= 1.0) {
                        None
                    } else {
                        Some(tr.iter().map(|v| 1.0 - v).collect())
                    };

                    (
                        Kv {
                            v: colors,
                            typed: false,
                        },
                        tr,
                    )
                };

                let i = self.named_interpolator(
                    "ColorInterpolator",
                    Some(3),
                    &interp,
                    times_v,
                    &colors,
                    cycle_interval,
                )?;

                self.named(&format!("{upper}Interpolator"), i)?;
                self.route(timer, "fraction_changed", i, "set_fraction")?;
                self.route(i, "value_changed", node, &field)?;
                nodes.push(i);

                if (field == "baseColor" || field == "emissiveColor") && transparencies.is_some() {
                    let tr = Kv {
                        v: transparencies.unwrap(),
                        typed: false,
                    };
                    let i = self.named_interpolator(
                        "ScalarInterpolator",
                        Some(1),
                        &interp,
                        times_v,
                        &tr,
                        cycle_interval,
                    )?;

                    self.named("TransparencyInterpolator", i)?;
                    self.route(timer, "fraction_changed", i, "set_fraction")?;
                    self.route(i, "value_changed", node, "transparency")?;
                    nodes.push(i);
                }

                Ok(nodes)
            }
            FieldType::SFColorRGBA => {
                let mut nodes = vec![];
                let colors: Vec<f64> = values
                    .v
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| i % 4 < 3)
                    .map(|(_, v)| *v)
                    .collect();
                let tr: Vec<f64> = values
                    .v
                    .iter()
                    .enumerate()
                    .filter(|(i, _)| i % 4 == 3)
                    .map(|(_, v)| *v)
                    .collect();
                let tr = if tr.iter().all(|v| *v >= 1.0) {
                    None
                } else {
                    Some(tr.iter().map(|v| 1.0 - v).collect::<Vec<f64>>())
                };

                // Script
                let script = self.create("Script")?;

                self.scene.add_user_defined_field(
                    script,
                    Field::new(
                        "color",
                        access::INPUT_OUTPUT,
                        FieldValue::SFColor(Color3::new(1.0, 1.0, 1.0)),
                    ),
                );
                self.scene.add_user_defined_field(
                    script,
                    Field::new("alpha", access::INPUT_OUTPUT, FieldValue::SFFloat(1.0)),
                );
                self.scene.add_user_defined_field(
                    script,
                    Field::new(
                        "value_changed",
                        access::OUTPUT_ONLY,
                        FieldValue::SFColorRGBA(Color4::new(0.0, 0.0, 0.0, 0.0)),
                    ),
                );
                self.set(
                    script,
                    "url",
                    FieldValue::MFString(vec![super::scripts::COLOR_RGBA.into()]),
                );

                self.setup(script);
                self.named("CombineColorRGBAScript", script)?;
                self.route(script, "value_changed", node, &field)?;

                // ColorInterpolator
                let i = self.named_interpolator(
                    "ColorInterpolator",
                    Some(3),
                    &interp,
                    times_v,
                    &Kv {
                        v: colors,
                        typed: false,
                    },
                    cycle_interval,
                )?;

                self.named(&format!("{upper}Interpolator"), i)?;
                self.route(timer, "fraction_changed", i, "set_fraction")?;
                self.route(i, "value_changed", script, "color")?;
                nodes.push(i);

                // AlphaInterpolator
                if let Some(tr) = tr {
                    let i = self.named_interpolator(
                        "ScalarInterpolator",
                        Some(1),
                        &interp,
                        times_v,
                        &Kv {
                            v: tr,
                            typed: false,
                        },
                        cycle_interval,
                    )?;

                    self.named("AlphaInterpolator", i)?;
                    self.route(timer, "fraction_changed", i, "set_fraction")?;
                    self.route(i, "value_changed", script, "alpha")?;
                    nodes.push(i);
                }

                nodes.push(script);
                Ok(nodes)
            }
            FieldType::SFRotation => {
                let i =
                    self.orientation_interpolator(&interp, times_v, &values.v, cycle_interval)?;

                self.named(&format!("{upper}Interpolator"), i)?;
                self.route(timer, "fraction_changed", i, "set_fraction")?;
                self.route(i, "value_changed", node, &field)?;
                Ok(vec![i])
            }
            FieldType::SFBool
            | FieldType::SFInt32
            | FieldType::SFFloat
            | FieldType::SFVec2f
            | FieldType::SFVec3f => {
                let (type_name, components, suffix) = match ty {
                    FieldType::SFBool => ("BooleanSequencer", None, "Sequencer"),
                    FieldType::SFInt32 => ("IntegerSequencer", None, "Sequencer"),
                    FieldType::SFFloat => ("ScalarInterpolator", Some(1), "Interpolator"),
                    FieldType::SFVec2f => ("PositionInterpolator2D", Some(2), "Interpolator"),
                    _ => ("PositionInterpolator", Some(3), "Interpolator"),
                };

                let mode = if components.is_some() {
                    interp
                } else {
                    Interp::Linear
                };
                let i = self.named_interpolator(
                    type_name,
                    components,
                    &mode,
                    times_v,
                    &Kv {
                        v: values.v.clone(),
                        typed: true,
                    },
                    cycle_interval,
                )?;

                self.named(&format!("{upper}{suffix}"), i)?;
                self.route(timer, "fraction_changed", i, "set_fraction")?;
                self.route(i, "value_changed", node, &field)?;
                Ok(vec![i])
            }
            _ => Ok(vec![]),
        }
    }

    /// `createNamedInterpolator`
    fn named_interpolator(
        &mut self,
        type_name: &str,
        components: Option<usize>,
        interp: &Interp,
        times: &[f64],
        values: &Kv,
        cycle: f64,
    ) -> R<NodeId> {
        let node = self.create(type_name)?;

        match interp {
            Interp::Step => {
                let c = components.unwrap_or(0);
                let mut key = vec![times[0] / cycle];

                for t in &times[1..] {
                    key.push(t / cycle);
                    key.push(t / cycle);
                }

                let mut kv: Vec<f64> = vec![];

                for k in 0..c {
                    kv.push(values.v.get(k).copied().unwrap_or(f64::NAN));
                }

                let c2 = c * 2;
                let length = values.v.len().saturating_sub(c);
                let mut i = 0;

                while i < length {
                    for k in 0..c2 {
                        kv.push(values.v.get(i + k).copied().unwrap_or(f64::NAN));
                    }

                    i += c;

                    if c == 0 {
                        break;
                    }
                }

                self.set_nums(node, "key", &key);
                self.set_nums(node, "keyValue", &kv);
            }
            Interp::Linear => {
                let tct = self.times_ct;
                let key: Vec<f64> = times
                    .iter()
                    .map(|t| tct.store(t / cycle) as f32 as f64)
                    .collect();

                self.set_nums(node, "key", &key);
                self.set_nums(node, "keyValue", &values.v);
            }
            Interp::Cubic => {
                let c = components.unwrap_or(1).max(1);

                if !values.typed {
                    return Err("TypeError: keyValues.subarray is not a function".into());
                }

                let width = if c == 1 { 2 } else { c };
                let mut vectors: Vec<Vec<f64>> = vec![];
                let mut i = 0;

                while i < values.v.len() {
                    let mut v: Vec<f64> = values.v[i..(i + c).min(values.v.len())].to_vec();

                    v.resize(width, 0.0);
                    vectors.push(v);
                    i += c;
                }

                let mut key = vec![];
                let mut kv = vec![];

                for t in samples(times) {
                    key.push(t / cycle);
                    kv.extend(cubic_spline_vector(t, times, &vectors));
                }

                let kv: Vec<f64> = if c == 1 {
                    kv.iter()
                        .enumerate()
                        .filter(|(i, _)| i % 2 < 1)
                        .map(|(_, v)| *v)
                        .collect()
                } else {
                    kv
                };

                self.set_nums(node, "key", &key);
                self.set_nums(node, "keyValue", &kv);
            }
        }

        self.setup(node);
        Ok(node)
    }

    fn orientation_interpolator(
        &mut self,
        interp: &Interp,
        times: &[f64],
        values: &[f64],
        cycle: f64,
    ) -> R<NodeId> {
        let node = self.create("OrientationInterpolator")?;
        let q = |i: usize| -> [f64; 4] {
            [0, 1, 2, 3].map(|k| values.get(i + k).copied().unwrap_or(f64::NAN))
        };

        match interp {
            Interp::Step => {
                let mut key = vec![times[0] / cycle];

                for t in &times[1..] {
                    key.push(t / cycle);
                    key.push(t / cycle);
                }

                let mut kv: Vec<f64> = quaternion_rotation(q(0)).to_vec();
                let length = values.len().saturating_sub(4);
                let mut i = 0;

                while i < length {
                    kv.extend(quaternion_rotation(q(i)));
                    kv.extend(quaternion_rotation(q(i + 4)));
                    i += 4;
                }

                self.set_nums(node, "key", &key);
                self.set_nums(node, "keyValue", &kv);
            }
            Interp::Linear => {
                let tct = self.times_ct;
                let key: Vec<f64> = times
                    .iter()
                    .map(|t| tct.store(t / cycle) as f32 as f64)
                    .collect();
                let mut kv = vec![];
                let mut i = 0;

                while i < values.len() {
                    kv.extend(quaternion_rotation(q(i)));
                    i += 4;
                }

                self.set_nums(node, "key", &key);
                self.set_nums(node, "keyValue", &kv);
            }
            Interp::Cubic => {
                let mut quaternions: Vec<Vec<f64>> = vec![];
                let mut i = 0;

                while i < values.len() {
                    quaternions.push(q(i).to_vec());
                    i += 4;
                }

                let mut key = vec![];
                let mut kv = vec![];

                for t in samples(times) {
                    let mut v = cubic_spline_vector(t, times, &quaternions);
                    let mut quat = Quaternion::new(v[0], v[1], v[2], v[3]);

                    quat.normalize();
                    v = vec![quat.x, quat.y, quat.z, quat.w];
                    key.push(t / cycle);
                    kv.extend(quaternion_rotation([v[0], v[1], v[2], v[3]]));
                }

                self.set_nums(node, "key", &key);
                self.set_nums(node, "keyValue", &kv);
            }
        }

        self.setup(node);
        Ok(node)
    }

    #[allow(clippy::too_many_arguments)]
    fn create_array_interpolator(
        &mut self,
        type_name: &str,
        interp_v: &V<'j>,
        times: &TArr,
        weights: &TArr,
        cycle: f64,
        targets: V<'j>,
        accessor: usize,
        key: &str,
    ) -> R<Option<NodeId>> {
        let Some(base) = self.acc[accessor].field.clone() else {
            return Ok(None);
        };
        let Some(target_list) = targets.arr() else {
            return Err("TypeError: targets.keys is not a function".into());
        };
        let n_targets = target_list.len();
        let node = self.create(type_name)?;
        let interp = interpolation(*interp_v);
        let times_v = &times.v;
        let w = |t: usize, i: usize| {
            weights
                .v
                .get(t * n_targets + i)
                .copied()
                .unwrap_or(f64::NAN)
        };

        match interp {
            Interp::Step => {
                let mut k = vec![times_v[0] / cycle];

                for t in &times_v[1..] {
                    k.push(t / cycle);
                    k.push(t / cycle);
                }

                let mut kv: Vec<f64> = vec![];
                let first: Vec<f64> = (0..n_targets)
                    .map(|i| weights.v.get(i).copied().unwrap_or(f64::NAN))
                    .collect();

                kv.extend(self.apply_morph_targets(&base, targets, key, &first)?);

                for t in 1..times_v.len() {
                    let ws: Vec<f64> = (0..n_targets).map(|i| w(t, i)).collect();
                    let values = self.apply_morph_targets(&base, targets, key, &ws)?;

                    kv.extend(values.iter());
                    kv.extend(values.iter());
                }

                self.set_nums(node, "key", &k);
                self.set_nums(node, "keyValue", &kv);
            }
            Interp::Linear => {
                let k: Vec<f64> = times_v.iter().map(|t| (t / cycle) as f32 as f64).collect();
                let mut kv = vec![];

                for t in 0..times_v.len() {
                    let ws: Vec<f64> = (0..n_targets).map(|i| w(t, i)).collect();

                    kv.extend(self.apply_morph_targets(&base, targets, key, &ws)?);
                }

                self.set_nums(node, "key", &k);
                self.set_nums(node, "keyValue", &kv);
            }
            Interp::Cubic => {
                let mut k = vec![];
                let mut kv = vec![];

                for t in samples(times_v) {
                    k.push(t / cycle);

                    let ws: Vec<f64> = (0..n_targets)
                        .map(|i| cubic_spline_scalar_array(t, times_v, &weights.v, n_targets, i))
                        .collect();

                    kv.extend(self.apply_morph_targets(&base, targets, key, &ws)?);
                }

                self.set_nums(node, "key", &k);
                self.set_nums(node, "keyValue", &kv);
            }
        }

        self.setup(node);
        Ok(Some(node))
    }
}

#[allow(dead_code)]
fn _unused(_: Vector4) {}
