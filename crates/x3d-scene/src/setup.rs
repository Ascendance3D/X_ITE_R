//! Side effects of `node .setup ()` that change printed field values. X_ITE sets up every node
//! parsed outside a PROTO body once the scene is parsed (X3DParser.setupNodes). Only effects
//! that need no event loop, Script engine or loading are reproduced here.

use crate::scene::*;
use x3d_fields::{FieldValue, NodeId};

impl Scene {
    /// X3DCast (X3DConstants .X3DCoordinateNode, node): built-in coordinate node, or the inner
    /// node of a PROTO instance (first root node of the proto body).
    fn coordinate_node(&self, node: NodeId, depth: usize) -> bool {
        if depth > 32 {
            return false;
        }

        match &self.node(node).kind {
            NodeKind::Builtin(t) => matches!(self.table.types[*t].name.as_str(), "Coordinate" | "CoordinateDouble" | "GeoCoordinate"),
            NodeKind::Instance(ProtoRef::Proto(p)) => match self.ctx(self.protos[*p].body).root_nodes.first() {
                Some(Some(inner)) => self.coordinate_node(*inner, depth + 1),
                _ => false,
            },
            _ => false,
        }
    }

    /// Run the setup side effects for nodes of the scene's own execution context.
    pub fn setup_nodes(&mut self) {
        for id in 0..self.nodes.len() as NodeId {
            if self.node(id).ctx != ROOT {
                continue;
            }

            let NodeKind::Builtin(t) = self.node(id).kind else {
                continue;
            };

            // X3DInterpolatorNode.initialize → set_key__ → set_keyValue__: a keyValue shorter
            // than key is resized to key's length, filled with its last value (or the type's
            // default); the spline interpolators also resize a non-empty keyVelocity.
            let resize: &[(&str, &[f64])] = match self.table.types[t].name.as_str() {
                "ScalarInterpolator" | "SplineScalarInterpolator" => &[("keyValue", &[0.0])],
                "PositionInterpolator" | "ColorInterpolator" => &[("keyValue", &[0.0, 0.0, 0.0])],
                "PositionInterpolator2D" => &[("keyValue", &[0.0, 0.0])],
                "OrientationInterpolator" | "SquadOrientationInterpolator" => &[("keyValue", &[0.0, 0.0, 1.0, 0.0])],
                "SplinePositionInterpolator" => &[("keyValue", &[0.0, 0.0, 0.0]), ("keyVelocity", &[0.0, 0.0, 0.0])],
                "SplinePositionInterpolator2D" => &[("keyValue", &[0.0, 0.0]), ("keyVelocity", &[0.0, 0.0])],
                "EaseInEaseOut" => &[("easeInEaseOut", &[0.0, 0.0])],
                _ => &[],
            };

            if !resize.is_empty() {
                let key_len = self.get_predefined_field(id, "key").map(|k| self.node(id).fields[k].value.len()).unwrap_or(0);

                for (name, default) in resize {
                    let Some(i) = self.get_predefined_field(id, name) else { continue };
                    let value = &mut self.node_mut(id).fields[i].value;
                    let len = value.len();

                    // keyVelocity is only resized when it has values; it is filled with zeros.
                    if *name == "keyVelocity" {
                        if len != 0 && len < key_len {
                            for _ in len..key_len {
                                value.push_numbers(default);
                            }
                        }
                        continue;
                    }

                    if len < key_len {
                        let fill = last_element(value).unwrap_or_else(|| default.to_vec());

                        for _ in len..key_len {
                            value.push_numbers(&fill);
                        }
                    }
                }
            }

            // X3DGeometryNode.setup → rebuild → IndexedFaceSet.triangulate:
            // "Add -1 (polygon end marker) to coordIndex if not present."
            if self.table.types[t].name == "IndexedFaceSet" {
                let coord = self.get_predefined_field(id, "coord");
                let index = self.get_predefined_field(id, "coordIndex");

                if let (Some(coord), Some(index)) = (coord, index) {
                    let has_coord = match self.node(id).fields[coord].value {
                        FieldValue::SFNode(Some(c)) => self.coordinate_node(c, 0),
                        _ => false,
                    };

                    if has_coord {
                        if let FieldValue::MFInt32(v) = &mut self.node_mut(id).fields[index].value {
                            if v.last().is_some_and(|last| *last > -1) {
                                v.push(-1);
                            }
                        }
                    }
                }
            }
        }
    }
}

/// The numbers of the last element of a typed MF value.
fn last_element(value: &FieldValue) -> Option<Vec<f64>> {
    use FieldValue as V;

    let c = value.field_type().components();

    match value {
        V::MFColor(v) | V::MFFloat(v) | V::MFVec2f(v) | V::MFVec3f(v) => (v.len() >= c).then(|| v[v.len() - c..].iter().map(|x| *x as f64).collect()),
        V::MFRotation(v) => (v.len() >= c).then(|| v[v.len() - c..].to_vec()),
        _ => None,
    }
}
