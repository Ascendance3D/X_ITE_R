//! The ecmascript sources of the Script nodes GLTF2Parser.js creates, byte for byte
//! (generated from the pinned source).

pub(super) const TEXTURE_TRANSFORM: &str = "ecmascript:\n\nconst m = new SFMatrix3f ();\n\nfunction eventsProcessed ()\n{\n   m .setTransform (translation, -rotation, scale);\n\n   // m' = m * flipY\n\n   value_changed [0]  = m [0];\n   value_changed [1]  = -m [1] + m [2];\n   value_changed [4]  = m [3];\n   value_changed [5]  = -m [4] + m [5];\n   value_changed [12] = m [6];\n   value_changed [13] = -m [7] + m [8];\n}\n";

pub(super) const ORTHO: &str = "ecmascript:\n\nfunction eventsProcessed ()\n{\n   value_changed [0] = -xmag;\n   value_changed [1] = -ymag;\n   value_changed [2] =  xmag;\n   value_changed [3] =  ymag;\n}\n";

pub(super) const COLOR_RGBA: &str = "ecmascript:\n\nfunction eventsProcessed ()\n{\n   value_changed [0] = color [0];\n   value_changed [1] = color [1];\n   value_changed [2] = color [2];\n   value_changed [3] = alpha;\n}\n   ";
