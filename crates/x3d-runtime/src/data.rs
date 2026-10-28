//! The plain data of a field value, for a JavaScript realm that holds X_ITE's own field objects
//! (src/x_ite/Fields/*.js) and mirrors them with the Rust runtime.
//!
//! The shape is the one the Script prelude uses (x3d-script `value_data`, prelude/bridge.js `fieldData`):
//!
//! * SFBool, SFInt32, SFFloat, SFDouble, SFTime, SFString: the primitive;
//! * SFNode: the node id, or null;
//! * SFRotation: x y z angle, tainted (0 or 1), then its quaternion x y z w (Rotation4 keeps both);
//! * SFImage: [width, height, comp, [pixels]];
//! * the other SF types: their components, in X_ITE's order;
//! * MFBool, MFString, MFNode: a list of the elements; MFImage: a list of SFImage data;
//! * the other MF types: the flat list of the components of every element.

use x3d_fields::jsmath::to_int32;
use x3d_fields::math::*;
use x3d_fields::{FieldType, FieldValue, Image, NodeId};

/// A JavaScript value without objects: what crosses the boundary.
#[derive(Clone, Debug, PartialEq)]
pub enum Data {
    Null,
    Bool(bool),
    Number(f64),
    String(String),
    List(Vec<Data>),
}

impl Data {
    fn number(&self) -> f64 {
        match self {
            Data::Number(x) => *x,
            Data::Bool(b) => f64::from(u8::from(*b)),
            _ => f64::NAN,
        }
    }

    fn items(&self) -> &[Data] {
        match self {
            Data::List(items) => items,
            _ => &[],
        }
    }

    fn numbers(&self) -> Vec<f64> {
        self.items().iter().map(Data::number).collect()
    }

    fn node(&self) -> Option<NodeId> {
        match self {
            Data::Number(x) if *x >= 0.0 => Some(*x as NodeId),
            _ => None,
        }
    }
}

/// The components of an SF value of a numeric type (X_ITE's order); empty for the others.
pub fn sf_numbers(v: &FieldValue) -> Vec<f64> {
    match v {
        FieldValue::SFColor(c) => c.components().to_vec(),
        FieldValue::SFColorRGBA(c) => c.components().to_vec(),
        FieldValue::SFVec2d(a) | FieldValue::SFVec2f(a) => a.to_array().to_vec(),
        FieldValue::SFVec3d(a) | FieldValue::SFVec3f(a) => a.to_array().to_vec(),
        FieldValue::SFVec4d(a) | FieldValue::SFVec4f(a) => a.to_array().to_vec(),
        FieldValue::SFRotation(r) => {
            let mut r = *r;
            r.components().to_vec()
        }
        FieldValue::SFQuaternion(q) => vec![q.x, q.y, q.z, q.w],
        FieldValue::SFMatrix3d(m) | FieldValue::SFMatrix3f(m) => m.0.to_vec(),
        FieldValue::SFMatrix4d(m) | FieldValue::SFMatrix4f(m) => m.0.to_vec(),
        _ => vec![],
    }
}

fn numbers(xs: impl IntoIterator<Item = f64>) -> Data {
    Data::List(xs.into_iter().map(Data::Number).collect())
}

fn node_data(n: &Option<NodeId>) -> Data {
    n.map_or(Data::Null, |n| Data::Number(f64::from(n)))
}

fn image_data(img: &Image) -> Data {
    Data::List(vec![
        Data::Number(f64::from(img.width)),
        Data::Number(f64::from(img.height)),
        Data::Number(f64::from(img.comp)),
        numbers(img.array.iter().map(|p| f64::from(*p))),
    ])
}

fn data_image(d: &Data) -> Image {
    let n = |i: usize| d.items().get(i).map_or(0.0, Data::number);
    let pixels = d.items().get(3).map(Data::numbers).unwrap_or_default();

    Image::new(n(0), n(1), n(2), pixels.into_iter().map(to_int32).collect())
}

/// The data of a field value.
pub fn value_data(v: &FieldValue) -> Data {
    match v {
        FieldValue::SFBool(b) => Data::Bool(*b),
        FieldValue::SFInt32(x) => Data::Number(f64::from(*x)),
        FieldValue::SFFloat(x) | FieldValue::SFDouble(x) | FieldValue::SFTime(x) => {
            Data::Number(*x)
        }
        FieldValue::SFString(s) => Data::String(s.clone()),
        FieldValue::SFNode(n) => node_data(n),
        FieldValue::SFImage(img) => image_data(img),
        FieldValue::SFRotation(r) => {
            let (aa, tainted, q) = r.state();

            numbers(
                aa.into_iter()
                    .chain([f64::from(u8::from(tainted)), q.x, q.y, q.z, q.w]),
            )
        }
        FieldValue::MFBool(a) => Data::List(a.iter().map(|b| Data::Bool(*b)).collect()),
        FieldValue::MFInt32(a) => numbers(a.iter().map(|x| f64::from(*x))),
        FieldValue::MFColor(a)
        | FieldValue::MFColorRGBA(a)
        | FieldValue::MFFloat(a)
        | FieldValue::MFMatrix3f(a)
        | FieldValue::MFMatrix4f(a)
        | FieldValue::MFVec2f(a)
        | FieldValue::MFVec3f(a)
        | FieldValue::MFVec4f(a) => numbers(a.iter().map(|x| f64::from(*x))),
        FieldValue::MFDouble(a)
        | FieldValue::MFMatrix3d(a)
        | FieldValue::MFMatrix4d(a)
        | FieldValue::MFQuaternion(a)
        | FieldValue::MFRotation(a)
        | FieldValue::MFTime(a)
        | FieldValue::MFVec2d(a)
        | FieldValue::MFVec3d(a)
        | FieldValue::MFVec4d(a) => numbers(a.iter().copied()),
        FieldValue::MFString(a) => Data::List(a.iter().cloned().map(Data::String).collect()),
        FieldValue::MFNode(a) => Data::List(a.iter().map(node_data).collect()),
        FieldValue::MFImage(a) => Data::List(a.iter().map(image_data).collect()),
        _ => numbers(sf_numbers(v)),
    }
}

/// The field value of type `ty` with the data `d` (the data of a field object of that type).
pub fn data_value(ty: FieldType, d: &Data) -> FieldValue {
    use FieldType as T;

    let n = d.numbers();
    let g = |i: usize| n.get(i).copied().unwrap_or(0.0);
    let f32s = || n.iter().map(|x| *x as f32).collect::<Vec<f32>>();

    match ty {
        T::SFBool => FieldValue::SFBool(matches!(d, Data::Bool(true))),
        T::SFInt32 => FieldValue::SFInt32(to_int32(d.number())),
        T::SFFloat => FieldValue::SFFloat(d.number()),
        T::SFDouble => FieldValue::SFDouble(d.number()),
        T::SFTime => FieldValue::SFTime(d.number()),
        T::SFString => FieldValue::SFString(match d {
            Data::String(s) => s.clone(),
            _ => String::new(),
        }),
        T::SFNode => FieldValue::SFNode(d.node()),
        T::SFImage => FieldValue::SFImage(data_image(d)),
        T::SFRotation => FieldValue::SFRotation(Rotation4::from_state(
            [g(0), g(1), g(2), g(3)],
            g(4) != 0.0,
            Quaternion::new(g(5), g(6), g(7), g(8)),
        )),
        // A field object's color holds what it holds (Color3.assign), unclamped.
        T::SFColor => FieldValue::SFColor(Color3::from_raw(g(0), g(1), g(2))),
        T::SFColorRGBA => FieldValue::SFColorRGBA(Color4::from_raw(g(0), g(1), g(2), g(3))),
        T::SFVec2d => FieldValue::SFVec2d(Vector2::new(g(0), g(1))),
        T::SFVec2f => FieldValue::SFVec2f(Vector2::new(g(0), g(1))),
        T::SFVec3d => FieldValue::SFVec3d(Vector3::new(g(0), g(1), g(2))),
        T::SFVec3f => FieldValue::SFVec3f(Vector3::new(g(0), g(1), g(2))),
        T::SFVec4d => FieldValue::SFVec4d(Vector4::new(g(0), g(1), g(2), g(3))),
        T::SFVec4f => FieldValue::SFVec4f(Vector4::new(g(0), g(1), g(2), g(3))),
        T::SFQuaternion => FieldValue::SFQuaternion(Quaternion::new(g(0), g(1), g(2), g(3))),
        T::SFMatrix3d | T::SFMatrix3f => {
            let mut m = Matrix3::default();

            if n.len() >= 9 {
                m.0.copy_from_slice(&n[..9]);
            }

            if ty == T::SFMatrix3d {
                FieldValue::SFMatrix3d(m)
            } else {
                FieldValue::SFMatrix3f(m)
            }
        }
        T::SFMatrix4d | T::SFMatrix4f => {
            let mut m = Matrix4::default();

            if n.len() >= 16 {
                m.0.copy_from_slice(&n[..16]);
            }

            if ty == T::SFMatrix4d {
                FieldValue::SFMatrix4d(m)
            } else {
                FieldValue::SFMatrix4f(m)
            }
        }
        T::MFBool => FieldValue::MFBool(
            d.items()
                .iter()
                .map(|x| matches!(x, Data::Bool(true)))
                .collect(),
        ),
        T::MFInt32 => FieldValue::MFInt32(n.into_iter().map(to_int32).collect()),
        T::MFColor => FieldValue::MFColor(f32s()),
        T::MFColorRGBA => FieldValue::MFColorRGBA(f32s()),
        T::MFFloat => FieldValue::MFFloat(f32s()),
        T::MFMatrix3f => FieldValue::MFMatrix3f(f32s()),
        T::MFMatrix4f => FieldValue::MFMatrix4f(f32s()),
        T::MFVec2f => FieldValue::MFVec2f(f32s()),
        T::MFVec3f => FieldValue::MFVec3f(f32s()),
        T::MFVec4f => FieldValue::MFVec4f(f32s()),
        T::MFDouble => FieldValue::MFDouble(n),
        T::MFMatrix3d => FieldValue::MFMatrix3d(n),
        T::MFMatrix4d => FieldValue::MFMatrix4d(n),
        T::MFQuaternion => FieldValue::MFQuaternion(n),
        T::MFRotation => FieldValue::MFRotation(n),
        T::MFTime => FieldValue::MFTime(n),
        T::MFVec2d => FieldValue::MFVec2d(n),
        T::MFVec3d => FieldValue::MFVec3d(n),
        T::MFVec4d => FieldValue::MFVec4d(n),
        T::MFString => FieldValue::MFString(
            d.items()
                .iter()
                .map(|x| match x {
                    Data::String(s) => s.clone(),
                    _ => String::new(),
                })
                .collect(),
        ),
        T::MFNode => FieldValue::MFNode(d.items().iter().map(Data::node).collect()),
        T::MFImage => FieldValue::MFImage(d.items().iter().map(data_image).collect()),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn round_trip(v: FieldValue) {
        let d = value_data(&v);

        assert_eq!(value_data(&data_value(v.field_type(), &d)), d);
    }

    #[test]
    fn every_type_round_trips() {
        round_trip(FieldValue::SFBool(true));
        round_trip(FieldValue::SFInt32(-7));
        round_trip(FieldValue::SFFloat(0.25));
        round_trip(FieldValue::SFDouble(1e300));
        round_trip(FieldValue::SFTime(12.5));
        round_trip(FieldValue::SFString("a \"b\"\n".into()));
        round_trip(FieldValue::SFNode(Some(4)));
        round_trip(FieldValue::SFNode(None));
        round_trip(FieldValue::SFColor(Color3::new(0.1, 0.2, 0.3)));
        round_trip(FieldValue::SFColorRGBA(Color4::new(0.1, 0.2, 0.3, 0.4)));
        round_trip(FieldValue::SFVec2f(Vector2::new(1.0, 2.0)));
        round_trip(FieldValue::SFVec3d(Vector3::new(1.0, 2.0, 3.0)));
        round_trip(FieldValue::SFVec4f(Vector4::new(1.0, 2.0, 3.0, 4.0)));
        round_trip(FieldValue::SFRotation(Rotation4::new(0.0, 1.0, 0.0, 2.0)));
        round_trip(FieldValue::SFQuaternion(Quaternion::new(
            0.0, 0.0, 0.0, 1.0,
        )));
        round_trip(FieldValue::SFMatrix3f(Matrix3::default()));
        round_trip(FieldValue::SFMatrix4d(Matrix4::default()));
        round_trip(FieldValue::SFImage(Image::new(2.0, 1.0, 1.0, vec![1, 255])));
        round_trip(FieldValue::MFBool(vec![true, false]));
        round_trip(FieldValue::MFInt32(vec![1, -2]));
        round_trip(FieldValue::MFFloat(vec![0.5, 1.5]));
        round_trip(FieldValue::MFVec3f(vec![1.0, 2.0, 3.0, 4.0, 5.0, 6.0]));
        round_trip(FieldValue::MFRotation(vec![0.0, 0.0, 1.0, 0.5]));
        round_trip(FieldValue::MFString(vec!["x".into(), "".into()]));
        round_trip(FieldValue::MFNode(vec![Some(1), None]));
        round_trip(FieldValue::MFImage(vec![Image::new(
            1.0,
            1.0,
            3.0,
            vec![0xff0000],
        )]));
    }

    #[test]
    fn rotation_data_keeps_axis_angle_and_quaternion() {
        let d = value_data(&FieldValue::SFRotation(Rotation4::new(0.0, 2.0, 0.0, 1.0)));

        assert_eq!(d.items().len(), 9);
        assert_eq!(d.items()[1], Data::Number(2.0));
    }
}
