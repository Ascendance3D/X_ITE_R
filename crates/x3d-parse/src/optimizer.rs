//! Port of src/x_ite/Parser/X3DOptimizer.js `optimizeSceneGraph`: remove empty grouping nodes,
//! merge single-child Transforms, lights and viewpoints into their parent Transform, flatten
//! identity grouping nodes into their parent, and fold interpolators with a single key into
//! the value they route to. OBJParser runs it with `removeEmptyGroups`; GLTF2Parser (not ported)
//! enables all three options.

use crate::formats::{node_field, set};
use std::collections::HashSet;
use x3d_fields::math::{Matrix4, Rotation4, Vector3};
use x3d_fields::{FieldValue, NodeId};
use x3d_scene::*;

#[derive(Clone, Copy, Debug, Default)]
pub struct Optimizer {
    pub remove_empty_groups: bool,
    pub combine_grouping_nodes: bool,
    pub optimize_interpolators: bool,
}

struct State {
    skeleton: Vec<bool>,
    removed: Vec<NodeId>,
    seen: HashSet<NodeId>,
}

fn type_name(scene: &Scene, node: NodeId) -> String {
    match scene.node(node).kind {
        NodeKind::Builtin(_) => scene.type_name(node),
        _ => String::new(),
    }
}

fn children_of(scene: &Scene, node: NodeId, field: &str) -> Vec<Option<NodeId>> {
    match node_field(scene, node, field) {
        FieldValue::MFNode(v) => v,
        _ => vec![],
    }
}

fn set_children(scene: &mut Scene, node: NodeId, field: &str, nodes: Vec<Option<NodeId>>) {
    set(scene, node, field, FieldValue::MFNode(nodes));
}

/// X3DNode.getCloneCount in a scene that is not running: the field values and root node
/// entries holding the node.
fn clone_count(scene: &Scene, node: NodeId) -> usize {
    let held = |v: &FieldValue| match v {
        FieldValue::SFNode(Some(n)) => (*n == node) as usize,
        FieldValue::MFNode(l) => l.iter().filter(|n| **n == Some(node)).count(),
        _ => 0,
    };

    let in_nodes: usize = scene
        .nodes
        .iter()
        .enumerate()
        .filter(|(id, _)| !scene.free_nodes.contains(&(*id as NodeId)))
        .flat_map(|(_, n)| n.fields.iter().chain(n.user_fields.iter()))
        .map(|f| held(&f.value))
        .sum();

    let in_protos: usize = scene.protos.iter().flat_map(|p| p.fields.iter()).chain(scene.externprotos.iter().flat_map(|p| p.fields.iter())).map(|f| held(&f.value)).sum();
    let in_roots: usize = scene.contexts.iter().map(|c| c.root_nodes.iter().filter(|n| **n == Some(node)).count()).sum();

    in_nodes + in_protos + in_roots
}

fn vec3(scene: &Scene, node: NodeId, field: &str) -> Vector3 {
    match node_field(scene, node, field) {
        FieldValue::SFVec3f(v) | FieldValue::SFVec3d(v) => v,
        _ => Vector3::ZERO,
    }
}

fn rot(scene: &Scene, node: NodeId, field: &str) -> Rotation4 {
    match node_field(scene, node, field) {
        FieldValue::SFRotation(r) => r,
        _ => Rotation4::identity(),
    }
}

fn has_field(scene: &Scene, node: NodeId, field: &str) -> bool {
    scene.get_predefined_field(node, field).is_some()
}

fn bool_field(scene: &Scene, node: NodeId, field: &str) -> Option<bool> {
    match has_field(scene, node, field).then(|| node_field(scene, node, field)) {
        Some(FieldValue::SFBool(b)) => Some(b),
        _ => None,
    }
}

/// Matrix4.setTransform from a Transform-like node's fields.
fn transform_matrix(scene: &Scene, node: NodeId) -> Matrix4 {
    let mut m = Matrix4::IDENTITY;

    m.set_transform(
        Some(&vec3(scene, node, "translation")),
        Some(&rot(scene, node, "rotation")),
        Some(&vec3(scene, node, "scale")),
        Some(&rot(scene, node, "scaleOrientation")),
        Some(&vec3(scene, node, "center")),
    );
    m
}

fn set_vec3(scene: &mut Scene, node: NodeId, field: &str, v: Vector3) {
    let value = match node_field(scene, node, field) {
        FieldValue::SFVec3d(_) => FieldValue::SFVec3d(v),
        _ => FieldValue::SFVec3f(v),
    };

    set(scene, node, field, value);
}

impl Optimizer {
    /// X3DOptimizer.optimizeSceneGraph (rootNodes) of a context.
    pub fn optimize_scene_graph(&self, scene: &mut Scene, ctx: CtxId) {
        let mut st = State { skeleton: vec![], removed: vec![], seen: HashSet::new() };
        let roots = scene.ctx(ctx).root_nodes.clone();
        let roots = self.optimize_nodes(scene, &roots, true, &mut st);

        scene.ctx_mut(ctx).root_nodes = roots;

        for node in st.removed {
            if clone_count(scene, node) == 0 {
                dispose(scene, node);
            }
        }
    }

    fn optimize_nodes(&self, scene: &mut Scene, nodes: &[Option<NodeId>], combine: bool, st: &mut State) -> Vec<Option<NodeId>> {
        nodes.iter().flat_map(|n| self.optimize_node(scene, *n, combine, st)).collect()
    }

    fn optimize_node(&self, scene: &mut Scene, node: Option<NodeId>, combine: bool, st: &mut State) -> Vec<Option<NodeId>> {
        let Some(node) = node else { return vec![] };
        let name = type_name(scene, node);

        if st.seen.contains(&node) {
            if matches!(name.as_str(), "HAnimJoint" | "HAnimSegment" | "HAnimSite") && !st.skeleton.last().copied().unwrap_or(false) {
                return vec![];
            }

            return vec![Some(node)];
        }

        st.seen.insert(node);

        if self.optimize_interpolators {
            self.remove_interpolators_with_only_one_value(scene, node, st);
        }

        match name.as_str() {
            "Transform" => {
                let children = children_of(scene, node, "children");
                let children = self.optimize_nodes(scene, &children, true, st);
                let empty = children.is_empty();

                set_children(scene, node, "children", children);

                if self.remove_empty_groups && empty {
                    return vec![];
                }
            }
            "Anchor" | "Group" => {
                let children = children_of(scene, node, "children");
                let children = self.optimize_nodes(scene, &children, true, st);

                set_children(scene, node, "children", children);
                return self.remove_if_no_children(scene, node, st);
            }
            "Collision" | "LOD" | "Switch" => {
                let children = children_of(scene, node, "children");

                self.optimize_nodes(scene, &children, false, st);
                return self.remove_if_no_children(scene, node, st);
            }
            "HAnimJoint" | "HAnimSegment" | "HAnimSite" => {
                let children = children_of(scene, node, "children");
                let children = self.optimize_nodes(scene, &children, true, st);

                set_children(scene, node, "children", children);

                if st.skeleton.last().copied().unwrap_or(false) {
                    return vec![Some(node)];
                }

                st.removed.push(node);
                return vec![];
            }
            "HAnimHumanoid" => {
                st.skeleton.push(true);
                let skeleton = children_of(scene, node, "skeleton");
                let skeleton = self.optimize_nodes(scene, &skeleton, true, st);
                set_children(scene, node, "skeleton", skeleton);
                st.skeleton.pop();

                let skin = children_of(scene, node, "skin");
                let skin = self.optimize_nodes(scene, &skin, true, st);
                set_children(scene, node, "skin", skin);

                return self.remove_if_no_children(scene, node, st);
            }
            _ => return vec![Some(node)],
        }

        // Transform
        if !combine || !self.combine_grouping_nodes || scene.has_routes(node) {
            return vec![Some(node)];
        }

        let node = self.combine_single_child(scene, node, st);

        // `node .translation ?.getValue () .equals (…)`: a combined light or viewpoint has no
        // such field, so the test is `!undefined` and it is returned.
        let identity = |f: &str, scene: &Scene| match has_field(scene, node, f).then(|| node_field(scene, node, f)) {
            Some(FieldValue::SFVec3f(v) | FieldValue::SFVec3d(v)) => v.equals(if f == "scale" { &Vector3::ONE } else { &Vector3::ZERO }),
            Some(FieldValue::SFRotation(r)) => r.equals(&Rotation4::identity()),
            _ => false,
        };

        if !identity("translation", scene) || !identity("rotation", scene) || !identity("scale", scene) {
            return vec![Some(node)];
        }

        // node .getNodeType () .includes (X3DConstants .X3DGroupingNode): of the nodes that get
        // here (a Transform, or the HAnimHumanoid a Transform was merged into) only Transform.
        if type_name(scene, node) != "Transform" {
            return vec![Some(node)];
        }

        if bool_field(scene, node, "visible") == Some(false) {
            return vec![Some(node)];
        }

        let children = children_of(scene, node, "children");

        remove_children(scene, node);
        st.removed.push(node);
        children
    }

    fn remove_if_no_children(&self, scene: &Scene, node: NodeId, st: &mut State) -> Vec<Option<NodeId>> {
        if !self.remove_empty_groups {
            return vec![Some(node)];
        }

        let has = if type_name(scene, node) == "HAnimHumanoid" {
            !children_of(scene, node, "skeleton").is_empty() || !children_of(scene, node, "skin").is_empty()
        } else {
            !children_of(scene, node, "children").is_empty()
        };

        if has {
            return vec![Some(node)];
        }

        st.removed.push(node);
        vec![]
    }

    fn remove_interpolators_with_only_one_value(&self, scene: &mut Scene, node: NodeId, st: &mut State) {
        let ctx = scene.node(node).ctx;
        let fields: Vec<FieldLoc> = (0..scene.node(node).fields.len()).map(FieldLoc::Pre).chain((0..scene.node(node).user_fields.len()).map(FieldLoc::User)).collect();

        for loc in fields {
            let routes: Vec<usize> = scene.ctx(ctx).routes.iter().enumerate().filter(|(_, r)| r.conn.is_some_and(|c| c.1 == (node, loc))).map(|(i, _)| i).collect();

            if routes.len() != 1 {
                continue;
            }

            let route = scene.ctx(ctx).routes[routes[0]].clone();
            let source = route.source_node;

            if !is_interpolator(scene, source) {
                continue;
            }

            let key = node_field(scene, source, "key");

            if key.len() != 1 {
                continue;
            }

            let Some(first) = node_field(scene, source, "keyValue").elements().and_then(|e| e.into_iter().next()) else { continue };
            let dest = scene.field(node, loc).name.clone();

            let f = scene.field_mut(node, loc);
            f.value = first;
            f.modified = true;
            let _ = dest;

            remove_children(scene, source);
            st.removed.push(source);
            dispose_route(scene, ctx, routes[0]);
        }
    }

    fn combine_single_child(&self, scene: &mut Scene, node: NodeId, st: &mut State) -> NodeId {
        let children = children_of(scene, node, "children");

        if children.len() != 1 {
            return node;
        }

        let Some(child) = children[0] else { return node };

        if clone_count(scene, child) > 1 || scene.has_routes(child) {
            return node;
        }

        match type_name(scene, child).as_str() {
            "Transform" | "HAnimHumanoid" => {
                if bool_field(scene, child, "visible") != bool_field(scene, node, "visible") {
                    return node;
                }

                self.combine_transform(scene, node, child, st)
            }
            "DirectionalLight" | "PointLight" | "SpotLight" => self.combine_light(scene, node, child, st),
            "Viewpoint" | "OrthoViewpoint" => self.combine_viewpoint(scene, node, child, st),
            _ => node,
        }
    }

    fn combine_transform(&self, scene: &mut Scene, node: NodeId, child: NodeId, st: &mut State) -> NodeId {
        let mut node_matrix = transform_matrix(scene, node);
        let child_matrix = transform_matrix(scene, child);
        let center = vec3(scene, child, "center");
        let (mut t, mut r, mut s, mut so) = (Vector3::ZERO, Rotation4::identity(), Vector3::ONE, Rotation4::identity());

        node_matrix.mult_left(&child_matrix);
        node_matrix.get_transform(Some(&mut t), Some(&mut r), Some(&mut s), Some(&mut so), Some(&center));

        set_vec3(scene, child, "translation", t);
        set(scene, child, "rotation", FieldValue::SFRotation(r));
        set_vec3(scene, child, "scale", s);
        set(scene, child, "scaleOrientation", FieldValue::SFRotation(so));

        // `!child .getNodeTypeName ()` is never true, so no name is moved to the child.

        remove_children(scene, node);
        st.removed.push(node);
        child
    }

    fn combine_light(&self, scene: &mut Scene, node: NodeId, child: NodeId, st: &mut State) -> NodeId {
        let m = transform_matrix(scene, node);

        if has_field(scene, child, "location") {
            let mut v = vec3(scene, child, "location");
            m.mult_vec_matrix3(&mut v);
            set_vec3(scene, child, "location", v);
        }

        if has_field(scene, child, "direction") {
            let mut v = vec3(scene, child, "direction");
            m.mult_dir_matrix(&mut v);
            v.normalize();
            set_vec3(scene, child, "direction", v);
        }

        remove_children(scene, node);
        st.removed.push(node);
        child
    }

    fn combine_viewpoint(&self, scene: &mut Scene, node: NodeId, child: NodeId, st: &mut State) -> NodeId {
        let node_matrix = transform_matrix(scene, node);
        let mut viewpoint_matrix = Matrix4::IDENTITY;
        let (mut t, mut r) = (Vector3::ZERO, Rotation4::identity());

        viewpoint_matrix.set_transform(Some(&vec3(scene, child, "position")), Some(&rot(scene, child, "orientation")), None, None, None);
        viewpoint_matrix.mult_right(&node_matrix);
        viewpoint_matrix.get_transform(Some(&mut t), Some(&mut r), None, None, None);

        let mut center = vec3(scene, child, "centerOfRotation");
        node_matrix.mult_vec_matrix3(&mut center);

        set_vec3(scene, child, "position", t);
        set(scene, child, "orientation", FieldValue::SFRotation(r));
        set_vec3(scene, child, "centerOfRotation", center);

        remove_children(scene, node);
        st.removed.push(node);
        child
    }
}

fn is_interpolator(scene: &Scene, node: NodeId) -> bool {
    type_name(scene, node).ends_with("Interpolator") || type_name(scene, node).ends_with("Interpolator2D")
}

/// X3DOptimizer.removeChildren: every SFNode set to NULL, every MFNode emptied.
fn remove_children(scene: &mut Scene, node: NodeId) {
    let n = scene.node_mut(node);

    for f in n.fields.iter_mut().chain(n.user_fields.iter_mut()) {
        match &mut f.value {
            FieldValue::SFNode(v) => *v = None,
            FieldValue::MFNode(v) => v.clear(),
            _ => continue,
        }

        f.modified = true;
    }
}

/// X3DRoute.dispose: the route leaves its context and its fields' route counts.
fn dispose_route(scene: &mut Scene, ctx: CtxId, index: usize) {
    let route = scene.ctx_mut(ctx).routes.remove(index);

    if let Some(((sn, sl), (dn, dl))) = route.conn {
        let s = scene.field_mut(sn, sl);
        s.output_routes = s.output_routes.saturating_sub(1);

        let d = scene.field_mut(dn, dl);
        d.input_routes = d.input_routes.saturating_sub(1);
    }
}

/// X3DNode.dispose of a parsed node: its DEF name and the scene's exports of it go away.
fn dispose(scene: &mut Scene, node: NodeId) {
    let ctx = scene.node(node).ctx;
    let name = scene.node(node).name.clone();

    if !name.is_empty() {
        scene.remove_named_node(ctx, &name);
    }

    if scene.is_scene_root(ctx) {
        let exports: Vec<String> = scene.ctx(ctx).exported.values().iter().filter(|&&e| scene.exported[e].local == node).map(|&e| scene.exported[e].exported_name.clone()).collect();

        for e in exports {
            scene.ctx_mut(ctx).exported.remove(&e);
        }
    }
}
