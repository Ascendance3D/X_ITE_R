//! Read-only access to the runtime's scene graph: typed field getters, PROTO inner nodes
//! (X3DPrototypeInstance.getInnerNode) and Inline scenes.

use std::collections::HashMap;
use x3d_fields::FieldValue;
use x3d_fields::NodeId;
use x3d_fields::math::{Rotation4, Vector2, Vector3};
use x3d_runtime::Runtime;
use x3d_scene::{CtxId, NodeKind, Outer};

pub struct Sg<'a> {
    pub rt: &'a Runtime,
    /// Instance node → body context.
    bodies: HashMap<NodeId, CtxId>,
}

impl<'a> Sg<'a> {
    pub fn new(rt: &'a Runtime) -> Self {
        let mut bodies = HashMap::new();

        // Later contexts win, like Runtime::instance_body (rposition).
        for (i, c) in rt.scene.contexts.iter().enumerate() {
            match c.outer {
                Outer::Instance(n) => {
                    bodies.insert(n, i);
                }
                _ => {}
            }
        }

        Sg { rt, bodies }
    }

    /// Builtin type name of a node (None for placeholders, internal nodes, instances).
    pub fn type_name(&self, n: NodeId) -> Option<&'static str> {
        self.rt.builtin_name(n)
    }

    /// X3DPrototypeInstance.getInnerNode, recursively; builtin nodes map to themselves.
    pub fn inner(&self, n: NodeId) -> Option<NodeId> {
        let mut n = n;

        for _ in 0..64 {
            match self.rt.scene.node(n).kind {
                NodeKind::Builtin(_) => return Some(n),
                NodeKind::Instance(_) => {
                    let body = *self.bodies.get(&n)?;
                    n = (*self.rt.scene.ctx(body).root_nodes.first()?)?;
                }
                _ => return None,
            }
        }

        None
    }

    /// Root nodes of the runtime's scene (inner nodes of PROTO instances).
    pub fn roots(&self) -> Vec<NodeId> {
        self.rt.scene.ctx(x3d_scene::ROOT).root_nodes.iter().flatten().filter_map(|n| self.inner(*n)).collect()
    }

    pub fn value(&self, n: NodeId, name: &str) -> Option<&FieldValue> {
        self.rt.pre_value(n, name)
    }

    pub fn bool(&self, n: NodeId, name: &str, default: bool) -> bool {
        match self.value(n, name) {
            Some(FieldValue::SFBool(b)) => *b,
            _ => default,
        }
    }

    pub fn f64(&self, n: NodeId, name: &str, default: f64) -> f64 {
        match self.value(n, name) {
            Some(FieldValue::SFFloat(v) | FieldValue::SFDouble(v) | FieldValue::SFTime(v)) => *v,
            Some(FieldValue::SFInt32(v)) => *v as f64,
            _ => default,
        }
    }

    pub fn i32(&self, n: NodeId, name: &str, default: i32) -> i32 {
        match self.value(n, name) {
            Some(FieldValue::SFInt32(v)) => *v,
            _ => default,
        }
    }

    pub fn vec3(&self, n: NodeId, name: &str, default: Vector3) -> Vector3 {
        match self.value(n, name) {
            Some(FieldValue::SFVec3f(v) | FieldValue::SFVec3d(v)) => *v,
            _ => default,
        }
    }

    pub fn vec2(&self, n: NodeId, name: &str, default: Vector2) -> Vector2 {
        match self.value(n, name) {
            Some(FieldValue::SFVec2f(v) | FieldValue::SFVec2d(v)) => *v,
            _ => default,
        }
    }

    pub fn color(&self, n: NodeId, name: &str, default: [f64; 3]) -> [f64; 3] {
        match self.value(n, name) {
            Some(FieldValue::SFColor(c)) => c.components(),
            _ => default,
        }
    }

    pub fn rotation(&self, n: NodeId, name: &str) -> Rotation4 {
        match self.value(n, name) {
            Some(FieldValue::SFRotation(r)) => *r,
            _ => Rotation4::identity(),
        }
    }

    pub fn node(&self, n: NodeId, name: &str) -> Option<NodeId> {
        match self.value(n, name) {
            Some(FieldValue::SFNode(Some(c))) => self.inner(*c),
            _ => None,
        }
    }

    pub fn nodes(&self, n: NodeId, name: &str) -> Vec<NodeId> {
        match self.value(n, name) {
            Some(FieldValue::MFNode(v)) => v.iter().flatten().filter_map(|c| self.inner(*c)).collect(),
            _ => vec![],
        }
    }

    /// Like `nodes`, but keeps the raw (uninnered) node ids, so callers can detect instances.
    pub fn raw_nodes(&self, n: NodeId, name: &str) -> Vec<Option<NodeId>> {
        match self.value(n, name) {
            Some(FieldValue::MFNode(v)) => v.clone(),
            _ => vec![],
        }
    }

    pub fn f32s(&self, n: NodeId, name: &str) -> &[f32] {
        match self.value(n, name) {
            Some(
                FieldValue::MFVec3f(v)
                | FieldValue::MFVec2f(v)
                | FieldValue::MFColor(v)
                | FieldValue::MFColorRGBA(v)
                | FieldValue::MFFloat(v)
                | FieldValue::MFVec4f(v),
            ) => v,
            _ => &[],
        }
    }

    pub fn f64s(&self, n: NodeId, name: &str) -> Vec<f64> {
        match self.value(n, name) {
            Some(FieldValue::MFRotation(v) | FieldValue::MFDouble(v) | FieldValue::MFVec3d(v) | FieldValue::MFVec2d(v)) => v.clone(),
            Some(FieldValue::MFFloat(v) | FieldValue::MFVec3f(v) | FieldValue::MFVec2f(v)) => v.iter().map(|x| *x as f64).collect(),
            _ => vec![],
        }
    }

    pub fn i32s(&self, n: NodeId, name: &str) -> &[i32] {
        match self.value(n, name) {
            Some(FieldValue::MFInt32(v)) => v,
            _ => &[],
        }
    }

    pub fn strings(&self, n: NodeId, name: &str) -> Vec<String> {
        match self.value(n, name) {
            Some(FieldValue::MFString(v)) => v.clone(),
            Some(FieldValue::SFString(s)) => vec![s.clone()],
            _ => vec![],
        }
    }

    pub fn image(&self, n: NodeId, name: &str) -> Option<&x3d_fields::Image> {
        match self.value(n, name) {
            Some(FieldValue::SFImage(i)) => Some(i),
            _ => None,
        }
    }

    /// The node's own context is inside an Inline'd scene (BindableList.getBound skips those).
    pub fn in_inline(&self, n: NodeId) -> bool {
        let mut c = self.rt.scene.node(n).ctx;

        loop {
            let ctx = self.rt.scene.ctx(c);

            if matches!(ctx.outer, Outer::Inline(_)) {
                return true;
            }

            match ctx.parent {
                Some(p) => c = p,
                None => return false,
            }
        }
    }
}
