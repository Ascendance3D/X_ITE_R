//! Field type names and access types (src/x_ite/Base/X3DConstants.js).

/// Access types, same bit values as X_ITE.
pub mod access {
    pub const INITIALIZE_ONLY: u8 = 0b001;
    pub const INPUT_ONLY: u8 = 0b010;
    pub const OUTPUT_ONLY: u8 = 0b100;
    pub const INPUT_OUTPUT: u8 = 0b111;

    pub fn name(access: u8) -> &'static str {
        match access {
            INITIALIZE_ONLY => "initializeOnly",
            INPUT_ONLY => "inputOnly",
            OUTPUT_ONLY => "outputOnly",
            _ => "inputOutput",
        }
    }

    pub fn is_initializable(access: u8) -> bool {
        access & INITIALIZE_ONLY != 0
    }

    pub fn is_input(access: u8) -> bool {
        access & INPUT_ONLY != 0
    }

    pub fn is_output(access: u8) -> bool {
        access & OUTPUT_ONLY != 0
    }
}

macro_rules! field_types {
    ($($name:ident),+ $(,)?) => {
        #[derive(Clone, Copy, Debug, PartialEq, Eq, Hash)]
        pub enum FieldType { $($name),+ }

        impl FieldType {
            pub const ALL: &'static [FieldType] = &[$(FieldType::$name),+];

            pub fn name(self) -> &'static str {
                match self { $(FieldType::$name => stringify!($name)),+ }
            }

            pub fn from_name(name: &str) -> Option<FieldType> {
                match name { $(stringify!($name) => Some(FieldType::$name),)+ _ => None }
            }
        }
    };
}

field_types!(
    SFBool,
    SFColor,
    SFColorRGBA,
    SFDouble,
    SFFloat,
    SFImage,
    SFInt32,
    SFMatrix3d,
    SFMatrix3f,
    SFMatrix4d,
    SFMatrix4f,
    SFNode,
    SFQuaternion,
    SFRotation,
    SFString,
    SFTime,
    SFVec2d,
    SFVec2f,
    SFVec3d,
    SFVec3f,
    SFVec4d,
    SFVec4f,
    MFBool,
    MFColor,
    MFColorRGBA,
    MFDouble,
    MFFloat,
    MFImage,
    MFInt32,
    MFMatrix3d,
    MFMatrix3f,
    MFMatrix4d,
    MFMatrix4f,
    MFNode,
    MFQuaternion,
    MFRotation,
    MFString,
    MFTime,
    MFVec2d,
    MFVec2f,
    MFVec3d,
    MFVec3f,
    MFVec4d,
    MFVec4f,
);

impl FieldType {
    pub fn is_multi(self) -> bool {
        self.name().starts_with("MF")
    }

    /// Number of scalar components per element for typed-array MF fields
    /// (src/x_ite/Fields/ArrayFields.js: TypedArrayTemplate).
    pub fn components(self) -> usize {
        use FieldType::*;
        match self {
            MFColor | MFVec3d | MFVec3f => 3,
            MFColorRGBA | MFQuaternion | MFRotation | MFVec4d | MFVec4f => 4,
            MFVec2d | MFVec2f => 2,
            MFMatrix3d | MFMatrix3f => 9,
            MFMatrix4d | MFMatrix4f => 16,
            _ => 1,
        }
    }
}
