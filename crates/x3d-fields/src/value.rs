//! Field values with X_ITE storage semantics (src/x_ite/Fields/*.js, Base/X3DTypedArrayField.js).
//!
//! * SF numeric fields hold a JS number (`f64`); `SFFloat` is *not* rounded to single precision.
//! * MF single-precision fields are `Float32Array`s, so they hold `f32`.
//! * `SFColor`/`SFColorRGBA` clamp to [0, 1] on every set; MF colors store raw values and
//!   only clamp when an element is printed (the printer goes through an SFColor).
//! * `SFRotation` keeps the raw axis/angle and compares by quaternion.

use crate::generator::Generator;
use crate::jsmath::{self, to_int32};
use crate::math::*;
use crate::types::FieldType;

pub type NodeId = u32;

#[derive(Clone, Debug, PartialEq, Default)]
pub struct Image {
    pub width: i32,
    pub height: i32,
    pub comp: i32,
    pub array: Vec<i32>,
}

impl Image {
    /// SFImage setters: width/height = max (v|0, 0), comp = clamp (v|0, 0, 4); the pixel array
    /// is resized to width*height (SFImage.addEvent).
    pub fn new(width: f64, height: f64, comp: f64, array: Vec<i32>) -> Self {
        let width = to_int32(width).max(0);
        let height = to_int32(height).max(0);
        let comp = to_int32(comp).clamp(0, 4);
        let mut array = array;

        array.resize((width as i64 * height as i64).max(0) as usize, 0);
        Self { width, height, comp, array }
    }

    pub fn equals(&self, o: &Self) -> bool {
        self.width == o.width && self.height == o.height && self.comp == o.comp && self.array == o.array
    }
}

#[derive(Clone, Debug)]
pub enum FieldValue {
    SFBool(bool),
    SFColor(Color3),
    SFColorRGBA(Color4),
    SFDouble(f64),
    SFFloat(f64),
    SFImage(Image),
    SFInt32(i32),
    SFMatrix3d(Matrix3),
    SFMatrix3f(Matrix3),
    SFMatrix4d(Matrix4),
    SFMatrix4f(Matrix4),
    SFNode(Option<NodeId>),
    SFQuaternion(Quaternion),
    SFRotation(Rotation4),
    SFString(String),
    SFTime(f64),
    SFVec2d(Vector2),
    SFVec2f(Vector2),
    SFVec3d(Vector3),
    SFVec3f(Vector3),
    SFVec4d(Vector4),
    SFVec4f(Vector4),
    MFBool(Vec<bool>),
    MFColor(Vec<f32>),
    MFColorRGBA(Vec<f32>),
    MFDouble(Vec<f64>),
    MFFloat(Vec<f32>),
    MFImage(Vec<Image>),
    MFInt32(Vec<i32>),
    MFMatrix3d(Vec<f64>),
    MFMatrix3f(Vec<f32>),
    MFMatrix4d(Vec<f64>),
    MFMatrix4f(Vec<f32>),
    MFNode(Vec<Option<NodeId>>),
    MFQuaternion(Vec<f64>),
    MFRotation(Vec<f64>),
    MFString(Vec<String>),
    MFTime(Vec<f64>),
    MFVec2d(Vec<f64>),
    MFVec2f(Vec<f32>),
    MFVec3d(Vec<f64>),
    MFVec3f(Vec<f32>),
    MFVec4d(Vec<f64>),
    MFVec4f(Vec<f32>),
}

/// Store a flat list of JS numbers into a typed array of `components` per element,
/// dropping an incomplete tail like X3DTypedArrayField.set does (it warns and truncates).
fn truncate<T: Copy>(values: Vec<T>, components: usize) -> Vec<T> {
    let mut values = values;
    let rest = values.len() % components;

    if rest != 0 {
        let n = values.len() - rest;
        values.truncate(n);
    }

    values
}

fn f32s(values: &[f64], components: usize) -> Vec<f32> {
    truncate(values.iter().map(|v| *v as f32).collect(), components)
}

fn f64s(values: &[f64], components: usize) -> Vec<f64> {
    truncate(values.to_vec(), components)
}

impl FieldValue {
    pub fn default_for(ty: FieldType) -> FieldValue {
        use FieldType as T;
        use FieldValue as V;

        match ty {
            T::SFBool => V::SFBool(false),
            T::SFColor => V::SFColor(Color3::BLACK),
            T::SFColorRGBA => V::SFColorRGBA(Color4::new(0.0, 0.0, 0.0, 0.0)),
            T::SFDouble => V::SFDouble(0.0),
            T::SFFloat => V::SFFloat(0.0),
            T::SFImage => V::SFImage(Image::default()),
            T::SFInt32 => V::SFInt32(0),
            T::SFMatrix3d => V::SFMatrix3d(Matrix3::IDENTITY),
            T::SFMatrix3f => V::SFMatrix3f(Matrix3::IDENTITY),
            T::SFMatrix4d => V::SFMatrix4d(Matrix4::IDENTITY),
            T::SFMatrix4f => V::SFMatrix4f(Matrix4::IDENTITY),
            T::SFNode => V::SFNode(None),
            // Fields/SFQuaternion.js: constructor defaults are (0, 0, 0, 0), not identity.
            T::SFQuaternion => V::SFQuaternion(Quaternion::ZERO),
            T::SFRotation => V::SFRotation(Rotation4::identity()),
            T::SFString => V::SFString(String::new()),
            T::SFTime => V::SFTime(-1.0),
            T::SFVec2d => V::SFVec2d(Vector2::ZERO),
            T::SFVec2f => V::SFVec2f(Vector2::ZERO),
            T::SFVec3d => V::SFVec3d(Vector3::ZERO),
            T::SFVec3f => V::SFVec3f(Vector3::ZERO),
            T::SFVec4d => V::SFVec4d(Vector4::ZERO),
            T::SFVec4f => V::SFVec4f(Vector4::ZERO),
            T::MFBool => V::MFBool(vec![]),
            T::MFColor => V::MFColor(vec![]),
            T::MFColorRGBA => V::MFColorRGBA(vec![]),
            T::MFDouble => V::MFDouble(vec![]),
            T::MFFloat => V::MFFloat(vec![]),
            T::MFImage => V::MFImage(vec![]),
            T::MFInt32 => V::MFInt32(vec![]),
            T::MFMatrix3d => V::MFMatrix3d(vec![]),
            T::MFMatrix3f => V::MFMatrix3f(vec![]),
            T::MFMatrix4d => V::MFMatrix4d(vec![]),
            T::MFMatrix4f => V::MFMatrix4f(vec![]),
            T::MFNode => V::MFNode(vec![]),
            T::MFQuaternion => V::MFQuaternion(vec![]),
            T::MFRotation => V::MFRotation(vec![]),
            T::MFString => V::MFString(vec![]),
            T::MFTime => V::MFTime(vec![]),
            T::MFVec2d => V::MFVec2d(vec![]),
            T::MFVec2f => V::MFVec2f(vec![]),
            T::MFVec3d => V::MFVec3d(vec![]),
            T::MFVec3f => V::MFVec3f(vec![]),
            T::MFVec4d => V::MFVec4d(vec![]),
            T::MFVec4f => V::MFVec4f(vec![]),
        }
    }

    pub fn field_type(&self) -> FieldType {
        use FieldType as T;
        use FieldValue as V;

        match self {
            V::SFBool(_) => T::SFBool,
            V::SFColor(_) => T::SFColor,
            V::SFColorRGBA(_) => T::SFColorRGBA,
            V::SFDouble(_) => T::SFDouble,
            V::SFFloat(_) => T::SFFloat,
            V::SFImage(_) => T::SFImage,
            V::SFInt32(_) => T::SFInt32,
            V::SFMatrix3d(_) => T::SFMatrix3d,
            V::SFMatrix3f(_) => T::SFMatrix3f,
            V::SFMatrix4d(_) => T::SFMatrix4d,
            V::SFMatrix4f(_) => T::SFMatrix4f,
            V::SFNode(_) => T::SFNode,
            V::SFQuaternion(_) => T::SFQuaternion,
            V::SFRotation(_) => T::SFRotation,
            V::SFString(_) => T::SFString,
            V::SFTime(_) => T::SFTime,
            V::SFVec2d(_) => T::SFVec2d,
            V::SFVec2f(_) => T::SFVec2f,
            V::SFVec3d(_) => T::SFVec3d,
            V::SFVec3f(_) => T::SFVec3f,
            V::SFVec4d(_) => T::SFVec4d,
            V::SFVec4f(_) => T::SFVec4f,
            V::MFBool(_) => T::MFBool,
            V::MFColor(_) => T::MFColor,
            V::MFColorRGBA(_) => T::MFColorRGBA,
            V::MFDouble(_) => T::MFDouble,
            V::MFFloat(_) => T::MFFloat,
            V::MFImage(_) => T::MFImage,
            V::MFInt32(_) => T::MFInt32,
            V::MFMatrix3d(_) => T::MFMatrix3d,
            V::MFMatrix3f(_) => T::MFMatrix3f,
            V::MFMatrix4d(_) => T::MFMatrix4d,
            V::MFMatrix4f(_) => T::MFMatrix4f,
            V::MFNode(_) => T::MFNode,
            V::MFQuaternion(_) => T::MFQuaternion,
            V::MFRotation(_) => T::MFRotation,
            V::MFString(_) => T::MFString,
            V::MFTime(_) => T::MFTime,
            V::MFVec2d(_) => T::MFVec2d,
            V::MFVec2f(_) => T::MFVec2f,
            V::MFVec3d(_) => T::MFVec3d,
            V::MFVec3f(_) => T::MFVec3f,
            V::MFVec4d(_) => T::MFVec4d,
            V::MFVec4f(_) => T::MFVec4f,
        }
    }

    /// Number of elements of an MF field (1 for SF fields).
    pub fn len(&self) -> usize {
        use FieldValue as V;

        let c = self.field_type().components();

        match self {
            V::MFBool(v) => v.len(),
            V::MFColor(v) | V::MFColorRGBA(v) | V::MFFloat(v) | V::MFMatrix3f(v) | V::MFMatrix4f(v) | V::MFVec2f(v)
            | V::MFVec3f(v) | V::MFVec4f(v) => v.len() / c,
            V::MFDouble(v) | V::MFMatrix3d(v) | V::MFMatrix4d(v) | V::MFQuaternion(v) | V::MFRotation(v) | V::MFTime(v)
            | V::MFVec2d(v) | V::MFVec3d(v) | V::MFVec4d(v) => v.len() / c,
            V::MFImage(v) => v.len(),
            V::MFInt32(v) => v.len(),
            V::MFNode(v) => v.len(),
            V::MFString(v) => v.len(),
            _ => 1,
        }
    }

    pub fn is_empty(&self) -> bool {
        self.len() == 0
    }

    /// Replace a typed MF field's content by a flat list of JS numbers (`field .setValue (array)`).
    pub fn set_flat_numbers(&mut self, values: &[f64]) {
        use FieldValue as V;

        let c = self.field_type().components();

        match self {
            V::MFBool(v) => *v = values.iter().map(|x| jsmath::to_uint8(*x) != 0).collect(),
            V::MFColor(v) | V::MFColorRGBA(v) | V::MFFloat(v) | V::MFMatrix3f(v) | V::MFMatrix4f(v) | V::MFVec2f(v)
            | V::MFVec3f(v) | V::MFVec4f(v) => *v = f32s(values, c),
            V::MFDouble(v) | V::MFMatrix3d(v) | V::MFMatrix4d(v) | V::MFQuaternion(v) | V::MFRotation(v) | V::MFTime(v)
            | V::MFVec2d(v) | V::MFVec3d(v) | V::MFVec4d(v) => *v = f64s(values, c),
            V::MFInt32(v) => *v = values.iter().map(|x| to_int32(*x)).collect(),
            _ => panic!("set_flat_numbers on {:?}", self.field_type()),
        }
    }

    /// `field .push (sfValue)` on a typed MF field: append one element given as JS numbers.
    pub fn push_numbers(&mut self, element: &[f64]) {
        use FieldValue as V;

        match self {
            V::MFBool(v) => v.push(jsmath::to_uint8(element[0]) != 0),
            V::MFColor(v) | V::MFColorRGBA(v) | V::MFFloat(v) | V::MFMatrix3f(v) | V::MFMatrix4f(v) | V::MFVec2f(v)
            | V::MFVec3f(v) | V::MFVec4f(v) => v.extend(element.iter().map(|x| *x as f32)),
            V::MFDouble(v) | V::MFMatrix3d(v) | V::MFMatrix4d(v) | V::MFQuaternion(v) | V::MFRotation(v) | V::MFTime(v)
            | V::MFVec2d(v) | V::MFVec3d(v) | V::MFVec4d(v) => v.extend_from_slice(element),
            V::MFInt32(v) => v.push(to_int32(element[0])),
            _ => panic!("push_numbers on {:?}", self.field_type()),
        }
    }

    /// `field .length = 0`.
    pub fn clear(&mut self) {
        let ty = self.field_type();

        if ty.is_multi() {
            *self = FieldValue::default_for(ty);
        }
    }

    /// JS `equals` of two fields of the same type.
    pub fn equals(&self, other: &FieldValue) -> bool {
        use FieldValue as V;

        match (self, other) {
            (V::SFBool(a), V::SFBool(b)) => a == b,
            (V::SFColor(a), V::SFColor(b)) => a.equals(b),
            (V::SFColorRGBA(a), V::SFColorRGBA(b)) => a.equals(b),
            (V::SFDouble(a), V::SFDouble(b)) | (V::SFFloat(a), V::SFFloat(b)) | (V::SFTime(a), V::SFTime(b)) => a == b,
            (V::SFImage(a), V::SFImage(b)) => a.equals(b),
            (V::SFInt32(a), V::SFInt32(b)) => a == b,
            (V::SFMatrix3d(a), V::SFMatrix3d(b)) | (V::SFMatrix3f(a), V::SFMatrix3f(b)) => a.equals(b),
            (V::SFMatrix4d(a), V::SFMatrix4d(b)) | (V::SFMatrix4f(a), V::SFMatrix4f(b)) => a.equals(b),
            (V::SFNode(a), V::SFNode(b)) => a == b,
            (V::SFQuaternion(a), V::SFQuaternion(b)) => a.equals(b),
            (V::SFRotation(a), V::SFRotation(b)) => a.equals(b),
            (V::SFString(a), V::SFString(b)) => a == b,
            (V::SFVec2d(a), V::SFVec2d(b)) | (V::SFVec2f(a), V::SFVec2f(b)) => a.equals(b),
            (V::SFVec3d(a), V::SFVec3d(b)) | (V::SFVec3f(a), V::SFVec3f(b)) => a.equals(b),
            (V::SFVec4d(a), V::SFVec4d(b)) | (V::SFVec4f(a), V::SFVec4f(b)) => a.equals(b),
            (V::MFBool(a), V::MFBool(b)) => a == b,
            (V::MFColor(a), V::MFColor(b))
            | (V::MFColorRGBA(a), V::MFColorRGBA(b))
            | (V::MFFloat(a), V::MFFloat(b))
            | (V::MFMatrix3f(a), V::MFMatrix3f(b))
            | (V::MFMatrix4f(a), V::MFMatrix4f(b))
            | (V::MFVec2f(a), V::MFVec2f(b))
            | (V::MFVec3f(a), V::MFVec3f(b))
            | (V::MFVec4f(a), V::MFVec4f(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y),
            (V::MFDouble(a), V::MFDouble(b))
            | (V::MFMatrix3d(a), V::MFMatrix3d(b))
            | (V::MFMatrix4d(a), V::MFMatrix4d(b))
            | (V::MFQuaternion(a), V::MFQuaternion(b))
            | (V::MFRotation(a), V::MFRotation(b))
            | (V::MFTime(a), V::MFTime(b))
            | (V::MFVec2d(a), V::MFVec2d(b))
            | (V::MFVec3d(a), V::MFVec3d(b))
            | (V::MFVec4d(a), V::MFVec4d(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x == y),
            (V::MFImage(a), V::MFImage(b)) => a.len() == b.len() && a.iter().zip(b).all(|(x, y)| x.equals(y)),
            (V::MFInt32(a), V::MFInt32(b)) => a == b,
            (V::MFNode(a), V::MFNode(b)) => a == b,
            (V::MFString(a), V::MFString(b)) => a == b,
            _ => false,
        }
    }

    /// The field's own `isDefaultValue ()` (not the node's, which compares to the definition).
    pub fn is_default_value(&self) -> bool {
        use FieldValue as V;

        match self {
            V::SFBool(v) => !*v,
            V::SFColor(c) => c.equals(&Color3::BLACK),
            V::SFColorRGBA(c) => c.equals(&Color4::new(0.0, 0.0, 0.0, 0.0)),
            V::SFDouble(v) | V::SFFloat(v) => *v == 0.0,
            V::SFTime(v) => *v == -1.0,
            V::SFImage(i) => i.width == 0 && i.height == 0 && i.comp == 0,
            V::SFInt32(v) => *v == 0,
            V::SFMatrix3d(m) | V::SFMatrix3f(m) => m.equals(&Matrix3::IDENTITY),
            V::SFMatrix4d(m) | V::SFMatrix4f(m) => m.equals(&Matrix4::IDENTITY),
            V::SFNode(n) => n.is_none(),
            V::SFQuaternion(q) => q.equals(&Quaternion::ZERO),
            V::SFRotation(r) => r.equals(&Rotation4::identity()),
            V::SFString(s) => s.is_empty(),
            V::SFVec2d(v) | V::SFVec2f(v) => v.equals(&Vector2::ZERO),
            V::SFVec3d(v) | V::SFVec3f(v) => v.equals(&Vector3::ZERO),
            V::SFVec4d(v) | V::SFVec4f(v) => v.equals(&Vector4::ZERO),
            _ => self.is_empty(),
        }
    }

    /// Build a value from the JSON written by oracle/x_ite-nodes.mjs (raw JS values; non-finite
    /// numbers as strings; SFNode as null; MFNode as its length, which is 0 for every default).
    pub fn from_json(ty: FieldType, json: &serde_json::Value) -> Option<FieldValue> {
        use FieldType as T;
        use FieldValue as V;
        use serde_json::Value as J;

        fn num(j: &J) -> Option<f64> {
            match j {
                J::Number(n) => n.as_f64(),
                J::String(s) => match s.as_str() {
                    "NaN" => Some(f64::NAN),
                    "Infinity" => Some(f64::INFINITY),
                    "-Infinity" => Some(f64::NEG_INFINITY),
                    _ => None,
                },
                _ => None,
            }
        }

        fn nums(j: &J) -> Option<Vec<f64>> {
            j.as_array()?.iter().map(num).collect()
        }

        fn flat(j: &J) -> Option<Vec<f64>> {
            let mut out = vec![];
            for e in j.as_array()? {
                match e {
                    J::Array(_) => out.extend(nums(e)?),
                    _ => out.push(num(e)?),
                }
            }
            Some(out)
        }

        fn image(j: &J) -> Option<Image> {
            let v = nums(j)?;
            Some(Image::new(v[0], v[1], v[2], v[3..].iter().map(|x| to_int32(*x)).collect()))
        }

        let mut value = FieldValue::default_for(ty);

        Some(match ty {
            T::SFBool => V::SFBool(json.as_bool()?),
            T::SFColor => {
                let v = nums(json)?;
                V::SFColor(Color3::new(v[0], v[1], v[2]))
            }
            T::SFColorRGBA => {
                let v = nums(json)?;
                V::SFColorRGBA(Color4::new(v[0], v[1], v[2], v[3]))
            }
            T::SFDouble => V::SFDouble(num(json)?),
            T::SFFloat => V::SFFloat(num(json)?),
            T::SFTime => V::SFTime(num(json)?),
            T::SFInt32 => V::SFInt32(to_int32(num(json)?)),
            T::SFImage => V::SFImage(image(json)?),
            T::SFMatrix3d | T::SFMatrix3f => {
                let v = nums(json)?;
                let m = Matrix3(v.try_into().ok()?);
                if ty == T::SFMatrix3d { V::SFMatrix3d(m) } else { V::SFMatrix3f(m) }
            }
            T::SFMatrix4d | T::SFMatrix4f => {
                let v = nums(json)?;
                let m = Matrix4(v.try_into().ok()?);
                if ty == T::SFMatrix4d { V::SFMatrix4d(m) } else { V::SFMatrix4f(m) }
            }
            T::SFNode => V::SFNode(None),
            T::SFQuaternion => {
                let v = nums(json)?;
                V::SFQuaternion(Quaternion::new(v[0], v[1], v[2], v[3]))
            }
            T::SFRotation => {
                let v = nums(json)?;
                V::SFRotation(Rotation4::new(v[0], v[1], v[2], v[3]))
            }
            T::SFString => V::SFString(json.as_str()?.to_string()),
            T::SFVec2d | T::SFVec2f => {
                let v = Vector2::from_slice(&nums(json)?);
                if ty == T::SFVec2d { V::SFVec2d(v) } else { V::SFVec2f(v) }
            }
            T::SFVec3d | T::SFVec3f => {
                let v = Vector3::from_slice(&nums(json)?);
                if ty == T::SFVec3d { V::SFVec3d(v) } else { V::SFVec3f(v) }
            }
            T::SFVec4d | T::SFVec4f => {
                let v = Vector4::from_slice(&nums(json)?);
                if ty == T::SFVec4d { V::SFVec4d(v) } else { V::SFVec4f(v) }
            }
            T::MFNode => V::MFNode(vec![]),
            T::MFString => V::MFString(json.as_array()?.iter().map(|s| s.as_str().map(String::from)).collect::<Option<_>>()?),
            T::MFBool => V::MFBool(json.as_array()?.iter().map(|b| b.as_bool()).collect::<Option<_>>()?),
            T::MFImage => V::MFImage(json.as_array()?.iter().map(image).collect::<Option<_>>()?),
            _ => {
                value.set_flat_numbers(&flat(json)?);
                value
            }
        })
    }

    /// Print a non-node value exactly like the corresponding X_ITE `toVRMLStream`.
    /// SFNode/MFNode must be printed by the scene layer.
    pub fn to_vrml_stream(&self, g: &mut Generator) {
        use FieldValue as V;

        match self {
            V::SFNode(_) | V::MFNode(_) => panic!("node fields are printed by the scene"),
            V::MFBool(v) => g.typed_list(v.len(), |g, i| print_sf(g, &V::SFBool(v[i]))),
            V::MFInt32(v) => g.typed_list(v.len(), |g, i| print_sf(g, &V::SFInt32(v[i]))),
            V::MFFloat(v) => g.typed_list(v.len(), |g, i| print_sf(g, &V::SFFloat(v[i] as f64))),
            V::MFDouble(v) => g.typed_list(v.len(), |g, i| print_sf(g, &V::SFDouble(v[i]))),
            V::MFTime(v) => g.typed_list(v.len(), |g, i| print_sf(g, &V::SFTime(v[i]))),
            V::MFColor(v) => g.typed_list(v.len() / 3, |g, i| {
                let mut c = Color3::BLACK;
                for k in 0..3 {
                    c.set_index(k, v[i * 3 + k] as f64);
                }
                print_sf(g, &V::SFColor(c))
            }),
            V::MFColorRGBA(v) => g.typed_list(v.len() / 4, |g, i| {
                let mut c = Color4::default();
                for k in 0..4 {
                    c.set_index(k, v[i * 4 + k] as f64);
                }
                print_sf(g, &V::SFColorRGBA(c))
            }),
            V::MFRotation(v) => g.typed_list(v.len() / 4, |g, i| {
                let r = Rotation4::new(v[i * 4], v[i * 4 + 1], v[i * 4 + 2], v[i * 4 + 3]);
                print_sf(g, &V::SFRotation(r))
            }),
            V::MFQuaternion(v) => g.typed_list(v.len() / 4, |g, i| {
                print_sf(g, &V::SFQuaternion(Quaternion::new(v[i * 4], v[i * 4 + 1], v[i * 4 + 2], v[i * 4 + 3])))
            }),
            V::MFVec2f(v) => g.typed_list(v.len() / 2, |g, i| {
                print_sf(g, &V::SFVec2f(Vector2::new(v[i * 2] as f64, v[i * 2 + 1] as f64)))
            }),
            V::MFVec3f(v) => g.typed_list(v.len() / 3, |g, i| {
                print_sf(g, &V::SFVec3f(Vector3::new(v[i * 3] as f64, v[i * 3 + 1] as f64, v[i * 3 + 2] as f64)))
            }),
            V::MFVec4f(v) => g.typed_list(v.len() / 4, |g, i| {
                let e: Vec<f64> = v[i * 4..i * 4 + 4].iter().map(|x| *x as f64).collect();
                print_sf(g, &V::SFVec4f(Vector4::from_slice(&e)))
            }),
            V::MFVec2d(v) => g.typed_list(v.len() / 2, |g, i| print_sf(g, &V::SFVec2d(Vector2::from_slice(&v[i * 2..])))),
            V::MFVec3d(v) => g.typed_list(v.len() / 3, |g, i| print_sf(g, &V::SFVec3d(Vector3::from_slice(&v[i * 3..])))),
            V::MFVec4d(v) => g.typed_list(v.len() / 4, |g, i| print_sf(g, &V::SFVec4d(Vector4::from_slice(&v[i * 4..])))),
            V::MFMatrix3f(v) => g.typed_list(v.len() / 9, |g, i| {
                let e: Vec<f64> = v[i * 9..i * 9 + 9].iter().map(|x| *x as f64).collect();
                print_sf(g, &V::SFMatrix3f(Matrix3(e.try_into().unwrap())))
            }),
            V::MFMatrix3d(v) => g.typed_list(v.len() / 9, |g, i| {
                print_sf(g, &V::SFMatrix3d(Matrix3(v[i * 9..i * 9 + 9].try_into().unwrap())))
            }),
            V::MFMatrix4f(v) => g.typed_list(v.len() / 16, |g, i| {
                let e: Vec<f64> = v[i * 16..i * 16 + 16].iter().map(|x| *x as f64).collect();
                print_sf(g, &V::SFMatrix4f(Matrix4(e.try_into().unwrap())))
            }),
            V::MFMatrix4d(v) => g.typed_list(v.len() / 16, |g, i| {
                print_sf(g, &V::SFMatrix4d(Matrix4(v[i * 16..i * 16 + 16].try_into().unwrap())))
            }),
            V::MFString(v) => g.typed_list(v.len(), |g, i| print_sf(g, &V::SFString(v[i].clone()))),
            V::MFImage(v) => g.typed_list(v.len(), |g, i| print_sf(g, &V::SFImage(v[i].clone()))),
            _ => print_sf(g, self),
        }
    }
}

/// SF `toStream` implementations.
fn print_sf(g: &mut Generator, value: &FieldValue) {
    use FieldValue as V;

    match value {
        V::SFBool(b) => g.string.push_str(if *b { "TRUE" } else { "FALSE" }),
        V::SFInt32(i) => g.string.push_str(&i.to_string()),
        V::SFFloat(f) => {
            let s = g.float_format(g.to_unit(*f));
            g.string.push_str(&s)
        }
        V::SFDouble(f) | V::SFTime(f) => {
            let s = g.double_format(g.to_unit(*f));
            g.string.push_str(&s)
        }
        V::SFString(s) => {
            g.string.push('"');
            g.string.push_str(&escape(s));
            g.string.push('"');
        }
        V::SFColor(c) => g.numbers(&c.components(), false),
        V::SFColorRGBA(c) => g.numbers(&c.components(), false),
        V::SFVec2f(v) => g.numbers(&v.to_array().map(|x| g.to_unit(x)), false),
        V::SFVec3f(v) => g.numbers(&v.to_array().map(|x| g.to_unit(x)), false),
        V::SFVec4f(v) => g.numbers(&v.to_array().map(|x| g.to_unit(x)), false),
        V::SFVec2d(v) => g.numbers(&v.to_array().map(|x| g.to_unit(x)), true),
        V::SFVec3d(v) => g.numbers(&v.to_array().map(|x| g.to_unit(x)), true),
        V::SFVec4d(v) => g.numbers(&v.to_array().map(|x| g.to_unit(x)), true),
        V::SFMatrix3f(m) => g.numbers(&m.0, false),
        V::SFMatrix3d(m) => g.numbers(&m.0, true),
        V::SFMatrix4f(m) => g.numbers(&m.0, false),
        V::SFMatrix4d(m) => g.numbers(&m.0, true),
        V::SFQuaternion(q) => g.numbers(&[q.x, q.y, q.z, q.w], true),
        V::SFRotation(r) => {
            let mut raw = r.raw();
            raw[3] = g.to_unit_angle(raw[3]);
            g.numbers(&raw, true)
        }
        V::SFImage(image) => {
            // SFImage.toStream: hexadecimal pixels via a Uint32Array view.
            g.string.push_str(&image.width.to_string());
            g.space();
            g.string.push_str(&image.height.to_string());
            g.space();
            g.string.push_str(&image.comp.to_string());
            g.attrib_break();
            g.inc_indent();

            for y in 0..image.height {
                g.list_indent();

                let s = (y * image.width) as usize;

                for x in 0..image.width as usize {
                    g.string.push_str("0x");
                    g.string.push_str(&format!("{:x}", image.array[x + s] as u32));

                    if x != image.width as usize - 1 {
                        g.space();
                    }
                }

                if y != image.height - 1 {
                    g.attrib_break();
                }
            }

            g.dec_indent();
        }
        _ => unreachable!(),
    }
}

/// SFString.escape: backslash-escape `\` and `"` only.
pub fn escape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());

    for c in s.chars() {
        if c == '\\' || c == '"' {
            out.push('\\');
        }
        out.push(c);
    }

    out
}

/// SFString.unescape: `\\` → `\`, `\"` → `"`; any other backslash is kept.
pub fn unescape(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut chars = s.chars().peekable();

    while let Some(c) = chars.next() {
        if c == '\\' {
            if let Some(&n) = chars.peek() {
                if n == '\\' || n == '"' {
                    out.push(n);
                    chars.next();
                    continue;
                }
            }
        }
        out.push(c);
    }

    out
}

impl FieldValue {
    /// The single values a multi-valued field hands to its elements' printers (X3DTypedArrayField
    /// and X3DObjectArrayField `toXMLStream` / `toJSONStreamValue`), in order. `None` for single
    /// values and for MFNode, whose elements are nodes printed by the scene layer.
    pub fn elements(&self) -> Option<Vec<FieldValue>> {
        use FieldValue as V;

        let out: Vec<FieldValue> = match self {
            V::MFBool(v) => v.iter().map(|x| V::SFBool(*x)).collect(),
            V::MFInt32(v) => v.iter().map(|x| V::SFInt32(*x)).collect(),
            V::MFFloat(v) => v.iter().map(|x| V::SFFloat(*x as f64)).collect(),
            V::MFDouble(v) => v.iter().map(|x| V::SFDouble(*x)).collect(),
            V::MFTime(v) => v.iter().map(|x| V::SFTime(*x)).collect(),
            V::MFString(v) => v.iter().map(|x| V::SFString(x.clone())).collect(),
            V::MFImage(v) => v.iter().map(|x| V::SFImage(x.clone())).collect(),
            V::MFColor(v) => (0..v.len() / 3)
                .map(|i| {
                    let mut c = Color3::BLACK;
                    for k in 0..3 {
                        c.set_index(k, v[i * 3 + k] as f64);
                    }
                    V::SFColor(c)
                })
                .collect(),
            V::MFColorRGBA(v) => (0..v.len() / 4)
                .map(|i| {
                    let mut c = Color4::default();
                    for k in 0..4 {
                        c.set_index(k, v[i * 4 + k] as f64);
                    }
                    V::SFColorRGBA(c)
                })
                .collect(),
            V::MFRotation(v) => (0..v.len() / 4).map(|i| V::SFRotation(Rotation4::new(v[i * 4], v[i * 4 + 1], v[i * 4 + 2], v[i * 4 + 3]))).collect(),
            V::MFQuaternion(v) => (0..v.len() / 4)
                .map(|i| V::SFQuaternion(Quaternion::new(v[i * 4], v[i * 4 + 1], v[i * 4 + 2], v[i * 4 + 3])))
                .collect(),
            V::MFVec2f(v) => (0..v.len() / 2).map(|i| V::SFVec2f(Vector2::new(v[i * 2] as f64, v[i * 2 + 1] as f64))).collect(),
            V::MFVec3f(v) => (0..v.len() / 3)
                .map(|i| V::SFVec3f(Vector3::new(v[i * 3] as f64, v[i * 3 + 1] as f64, v[i * 3 + 2] as f64)))
                .collect(),
            V::MFVec4f(v) => (0..v.len() / 4)
                .map(|i| {
                    let e: Vec<f64> = v[i * 4..i * 4 + 4].iter().map(|x| *x as f64).collect();
                    V::SFVec4f(Vector4::from_slice(&e))
                })
                .collect(),
            V::MFVec2d(v) => (0..v.len() / 2).map(|i| V::SFVec2d(Vector2::from_slice(&v[i * 2..i * 2 + 2]))).collect(),
            V::MFVec3d(v) => (0..v.len() / 3).map(|i| V::SFVec3d(Vector3::from_slice(&v[i * 3..i * 3 + 3]))).collect(),
            V::MFVec4d(v) => (0..v.len() / 4).map(|i| V::SFVec4d(Vector4::from_slice(&v[i * 4..i * 4 + 4]))).collect(),
            V::MFMatrix3f(v) => (0..v.len() / 9)
                .map(|i| {
                    let e: Vec<f64> = v[i * 9..i * 9 + 9].iter().map(|x| *x as f64).collect();
                    V::SFMatrix3f(Matrix3(e.try_into().unwrap()))
                })
                .collect(),
            V::MFMatrix3d(v) => (0..v.len() / 9).map(|i| V::SFMatrix3d(Matrix3(v[i * 9..i * 9 + 9].try_into().unwrap()))).collect(),
            V::MFMatrix4f(v) => (0..v.len() / 16)
                .map(|i| {
                    let e: Vec<f64> = v[i * 16..i * 16 + 16].iter().map(|x| *x as f64).collect();
                    V::SFMatrix4f(Matrix4(e.try_into().unwrap()))
                })
                .collect(),
            V::MFMatrix4d(v) => (0..v.len() / 16).map(|i| V::SFMatrix4d(Matrix4(v[i * 16..i * 16 + 16].try_into().unwrap()))).collect(),
            _ => return None,
        };

        Some(out)
    }
}
