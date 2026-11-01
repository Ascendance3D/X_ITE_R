//! The VRML 1.0 node classes of the extension (`src/nodes/*.js`): field names, types and
//! defaults. A name not listed here is parsed as a custom node whose fields come from its
//! `fields [ ... ]` declaration (the extension's `CustomNode`).

// The defaults below are the literal numbers of the extension (for example 0.785398 for a spot light's cutOffAngle and a camera's
// heightAngle). The extension prints them as written, and output equal to the extension is the point of this crate, so they must not
// be replaced by `std::f32::consts::FRAC_PI_4` (0.7853982).
#![allow(clippy::approx_constant)]

use x3d_fields::FieldValue as V;
use x3d_fields::math::{Color3, Matrix4, Rotation4, Vector2, Vector3};

pub(crate) type Fields = Vec<(String, V)>;

fn sf(x: f64) -> V {
    V::SFFloat(x)
}
fn s(x: &str) -> V {
    V::SFString(x.into())
}
fn v3(x: f64, y: f64, z: f64) -> V {
    V::SFVec3f(Vector3::new(x, y, z))
}
fn col(r: f64, g: f64, b: f64) -> V {
    V::SFColor(Color3::new(r, g, b))
}
fn mff(v: &[f32]) -> V {
    V::MFFloat(v.to_vec())
}
fn mfi(v: &[i32]) -> V {
    V::MFInt32(v.to_vec())
}
fn mfs(v: &[&str]) -> V {
    V::MFString(v.iter().map(|x| x.to_string()).collect())
}
fn mf3(v: &[f32]) -> V {
    V::MFVec3f(v.to_vec())
}
fn mf2(v: &[f32]) -> V {
    V::MFVec2f(v.to_vec())
}
fn mfc(v: &[f32]) -> V {
    V::MFColor(v.to_vec())
}

/// The fields of node class `type_name`, or `None` for a name the extension does not know.
pub(crate) fn defaults(type_name: &str) -> Option<Fields> {
    let f: Vec<(&str, V)> = match type_name {
        "AsciiText" => vec![
            ("string", V::MFString(vec![])),
            ("spacing", sf(1.0)),
            ("justification", s("LEFT")),
            ("width", mff(&[])),
        ],
        "Background" => vec![
            ("groundColors", mfc(&[])),
            ("skyColors", mfc(&[0.0, 0.0, 0.0])),
            ("scenery", mfs(&[""])),
        ],
        "CollideStyle" => vec![("collide", V::SFBool(false)), ("proxy", V::SFNode(None))],
        "Cone" => vec![
            ("parts", s("ALL")),
            ("bottomRadius", sf(1.0)),
            ("height", sf(2.0)),
        ],
        "Coordinate3" => vec![("point", mf3(&[]))],
        "Cube" => vec![("width", sf(2.0)), ("height", sf(2.0)), ("depth", sf(2.0))],
        "Cylinder" => vec![
            ("parts", s("ALL")),
            ("radius", sf(1.0)),
            ("height", sf(2.0)),
        ],
        "DirectedSound" => vec![
            ("name", mfs(&[])),
            ("description", s("")),
            ("intensity", sf(1.0)),
            ("location", v3(0.0, 0.0, 0.0)),
            ("direction", v3(0.0, 0.0, 1.0)),
            ("minRange", sf(10.0)),
            ("maxRange", sf(10.0)),
            ("minAngle", sf(0.785398)),
            ("maxAngle", sf(0.785398)),
            ("loop", V::SFBool(false)),
            ("start", V::SFTime(-1.0)),
            ("pause", V::SFTime(-1.0)),
        ],
        "DirectionalLight" => vec![
            ("on", V::SFBool(true)),
            ("intensity", sf(1.0)),
            ("color", col(1.0, 1.0, 1.0)),
            ("direction", v3(0.0, 0.0, -1.0)),
        ],
        "ElevationGrid" => vec![
            ("verticesPerRow", V::SFInt32(0)),
            ("verticesPerColumn", V::SFInt32(0)),
            ("gridStep", V::SFVec2f(Vector2::new(0.0, 0.0))),
            ("height", mff(&[])),
        ],
        "Environment" => vec![
            ("ambientIntensity", sf(0.2)),
            ("ambientColor", col(1.0, 1.0, 1.0)),
            ("attenuation", v3(0.0, 0.0, 1.0)),
            ("fogType", s("NONE")),
            ("fogColor", col(1.0, 1.0, 1.0)),
            ("fogVisibility", sf(0.0)),
        ],
        "FontStyle" => vec![
            ("size", sf(10.0)),
            ("family", s("SERIF")),
            ("style", s("NONE")),
        ],
        "GeneralCylinder" => vec![
            ("spine", mf3(&[0.0, 0.0, 0.0, 0.0, 1.0, 0.0])),
            (
                "crossSection",
                mf2(&[-1.0, 1.0, -1.0, -1.0, 1.0, -1.0, 1.0, 1.0]),
            ),
            ("profile", mf2(&[1.0, -1.0, 1.0, 1.0])),
            ("twist", mf2(&[0.0, -1.0, 0.0, 1.0])),
            ("parts", s("ALL")),
        ],
        "Group" | "Separator" => vec![],
        "IndexedFaceSet" => vec![
            ("vertexOrdering", s("COUNTERCLOCKWISE")),
            ("coordIndex", mfi(&[0])),
            ("materialIndex", mfi(&[-1])),
            ("normalIndex", mfi(&[-1])),
            ("textureCoordIndex", mfi(&[-1])),
        ],
        "IndexedLineSet" => vec![
            ("coordIndex", mfi(&[0])),
            ("materialIndex", mfi(&[-1])),
            ("normalIndex", mfi(&[-1])),
            ("textureCoordIndex", mfi(&[-1])),
        ],
        "Info" => vec![("string", s(""))],
        "LOD" => vec![("range", mff(&[-1.0])), ("center", v3(0.0, 0.0, 0.0))],
        "Material" => vec![
            ("point", mf3(&[])),
            ("ambientColor", mfc(&[0.2, 0.2, 0.2])),
            ("diffuseColor", mfc(&[0.8, 0.8, 0.8])),
            ("specularColor", mfc(&[0.0, 0.0, 0.0])),
            ("emissiveColor", mfc(&[0.0, 0.0, 0.0])),
            ("shininess", mff(&[0.2])),
            ("transparency", mff(&[0.0])),
        ],
        "MaterialBinding" => vec![("value", s("OVERALL"))],
        "MatrixTransform" => vec![("matrix", V::SFMatrix4f(Matrix4::IDENTITY))],
        "NavigationInfo" => vec![
            ("type", mfs(&["walk"])),
            ("speed", sf(1.0)),
            ("collisionRadius", sf(1.0)),
            ("headlight", V::SFBool(true)),
        ],
        "Normal" => vec![("vector", mf3(&[]))],
        "NormalBinding" => vec![("value", s(""))],
        "OrthographicCamera" => vec![
            ("position", v3(0.0, 0.0, 1.0)),
            ("orientation", V::SFRotation(Rotation4::identity())),
            ("focalDistance", sf(5.0)),
            ("height", sf(2.0)),
            ("nearDistance", sf(1.0)),
            ("farDistance", sf(10.0)),
        ],
        "PerspectiveCamera" => vec![
            ("position", v3(0.0, 0.0, 1.0)),
            ("orientation", V::SFRotation(Rotation4::identity())),
            ("focalDistance", sf(5.0)),
            ("heightAngle", sf(0.785398)),
            ("nearDistance", sf(1.0)),
            ("farDistance", sf(10.0)),
        ],
        "PointLight" => vec![
            ("on", V::SFBool(true)),
            ("intensity", sf(1.0)),
            ("color", col(1.0, 1.0, 1.0)),
            ("location", v3(0.0, 0.0, 1.0)),
        ],
        "PointSet" => vec![("startIndex", V::SFInt32(0)), ("numPoints", V::SFInt32(-1))],
        "PointSound" => vec![
            ("name", mfs(&[])),
            ("description", s("")),
            ("intensity", sf(1.0)),
            ("location", v3(0.0, 0.0, 0.0)),
            ("minRange", sf(10.0)),
            ("maxRange", sf(10.0)),
            ("loop", V::SFBool(false)),
            ("start", V::SFTime(-1.0)),
            ("pause", V::SFTime(-1.0)),
        ],
        "Rotation" => vec![("rotation", V::SFRotation(Rotation4::identity()))],
        "Scale" => vec![("scaleFactor", v3(1.0, 1.0, 1.0))],
        "ShapeHints" => vec![
            ("vertexOrdering", s("COUNTERCLOCKWISE")),
            ("shapeType", s("SOLID")),
            ("faceType", s("CONVEX")),
            ("creaseAngle", sf(0.0)),
        ],
        "Sphere" => vec![("radius", sf(1.0))],
        "SpotLight" => vec![
            ("on", V::SFBool(true)),
            ("intensity", sf(1.0)),
            ("color", col(1.0, 1.0, 1.0)),
            ("location", v3(0.0, 0.0, 1.0)),
            ("direction", v3(0.0, 0.0, -1.0)),
            ("dropOffRate", sf(0.0)),
            ("cutOffAngle", sf(0.785398)),
        ],
        "Switch" => vec![("whichChild", V::SFInt32(-1))],
        "Text" => vec![
            ("string", mfs(&[""])),
            ("direction", s("LR")),
            ("justification", s("BEGIN")),
            ("spacing", sf(1.0)),
            ("width", mff(&[0.0])),
        ],
        "TextLanguage" => vec![("textLanguage", s(""))],
        "Texture2" => vec![
            ("filename", s("")),
            ("image", V::SFImage(Default::default())),
            ("wrapS", s("REPEAT")),
            ("wrapT", s("REPEAT")),
        ],
        "Texture2Transform" => vec![
            ("translation", V::SFVec2f(Vector2::new(0.0, 0.0))),
            ("rotation", sf(0.0)),
            ("scaleFactor", V::SFVec2f(Vector2::new(1.0, 1.0))),
            ("center", V::SFVec2f(Vector2::new(0.0, 0.0))),
        ],
        "TextureCoordinate2" => vec![("point", mf2(&[0.0, 0.0]))],
        "Transform" => vec![
            ("translation", v3(0.0, 0.0, 0.0)),
            ("rotation", V::SFRotation(Rotation4::identity())),
            ("scaleFactor", v3(1.0, 1.0, 1.0)),
            ("scaleOrientation", V::SFRotation(Rotation4::identity())),
            ("center", v3(0.0, 0.0, 0.0)),
        ],
        "Translation" => vec![("translation", v3(0.0, 0.0, 0.0))],
        "WWWAnchor" => vec![
            ("name", mfs(&[])),
            ("description", s("")),
            ("map", s("NONE")),
        ],
        "WWWInline" => vec![
            ("name", mfs(&[])),
            ("bboxSize", v3(-1.0, -1.0, -1.0)),
            ("bboxCenter", v3(0.0, 0.0, 0.0)),
        ],
        "WorldInfo" => vec![("title", s("")), ("info", mfs(&[]))],
        _ => return None,
    };

    Some(f.into_iter().map(|(k, v)| (k.to_string(), v)).collect())
}

/// `VRML1Parser.substitutions`: VRML 1.0 field type names to X3D ones.
pub(crate) fn substitute_field_type(name: &str) -> &str {
    match name {
        "SFBitMask" | "SFEnum" => "SFString",
        "SFLong" => "SFInt32",
        "SFMatrix" => "SFMatrix4f",
        "MFBitMask" | "MFEnum" => "MFString",
        "MFLong" => "MFInt32",
        "MFMatrix" => "MFMatrix4f",
        other => other,
    }
}
