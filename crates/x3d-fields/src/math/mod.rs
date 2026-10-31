//! Ports of X_ITE's `standard/Math/Numbers` value types.

pub mod color;
pub mod complex;
pub mod matrix;
pub mod quaternion;
pub mod rotation;
pub mod vector;

pub use color::{Color3, Color4};
pub use complex::Complex;
pub use matrix::{Matrix2, Matrix3, Matrix4};
pub use quaternion::Quaternion;
pub use rotation::Rotation4;
pub use vector::{Vector2, Vector3, Vector4};
