//! Side effects of `node .setup ()` that change printed field values. X_ITE sets up every node
//! parsed outside a PROTO body once the scene is parsed (X3DParser.setupNodes). Only effects
//! that need no event loop, Script engine or loading are reproduced here.

use crate::scene::*;
use crate::fields::{FieldValue, NodeId};

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
