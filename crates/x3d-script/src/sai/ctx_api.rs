//! The write side of X3DExecutionContext / X3DScene for scripts (Execution/X3DExecutionContext.js, X3DScene.js): imported nodes,
//! proto and extern proto declarations, routes, exported nodes, named nodes and the unique-name helpers. The messages are X_ITE's.

use super::*;

/// X3DCast (X3DConstants .X3DNode, value, false): a node, not null.
fn node_arg<V: Val>(a: &[V], i: usize) -> Option<NodeId> {
    a.get(i).and_then(arg_node).flatten()
}

/// Log what the scene layer warned about.
fn flush_warnings() {
    for w in std::mem::take(&mut rt().scene.warnings) {
        rt().log_warn(w);
    }
}

/// Something a Script changed may have dropped the last reference to a node, declaration or record.
fn dirty() {
    rt().scene_graph_changed();
}

/// `this ._sceneGraph_changed = Date .now () / 1000` on `c`.
fn sg(c: CtxId) {
    rt().scene_graph_changed();
    rt().ctx_event(c, CtxField::SceneGraph, 1);
}

/// The exported node `node` of the Inline in an imported record: X3DImportedNode.getExportedNode.
fn exported_of(imp: usize) -> Option<NodeId> {
    let r = rt();
    let rec = &r.scene.imported[imp];
    let scene = *r.inline_scenes.get(&rec.inline)?;

    r.scene.get_exported_node(scene, &rec.exported_name)
}

/// X3DExecutionContext.getNodeOrImportedNode.
fn node_or_imported(c: CtxId, node: NodeId) -> NodeId {
    let ids: Vec<usize> = rt().scene.ctx(c).imported.values().to_vec();

    for id in ids {
        if exported_of(id) == Some(node) {
            return rt().scene.imported[id].instance;
        }
    }

    node
}

/// X3DImportedNode.dispose: the record leaves the context and the routes of its instance are deleted.
fn remove_imported(c: CtxId, name: &str) -> bool {
    let Some(id) = rt().scene.remove_imported_node(c, name) else {
        return false;
    };

    let instance = rt().scene.imported[id].instance;
    let routes: Vec<usize> = rt()
        .scene
        .ctx(c)
        .routes
        .iter()
        .filter(|r| r.source_node == instance || r.destination_node == instance)
        .map(|r| r.id)
        .collect();

    for r in routes {
        rt().delete_route_by_id(c, r);
    }

    rt().dynamic_dirty_mark();
    true
}

pub(super) fn op<C: Cx>(ctx: &C, a: &[C::V]) -> Result<J<C::V>, C::E> {
    let c = ctx_of_script(num_arg(ctx, a, 0));
    let what = str_arg(ctx, a, 1);
    let a = &a[2..];
    let str_at = |i: usize| str_arg(ctx, a, i);

    macro_rules! string {
        ($v:expr) => {
            Ok(J::from(&$v))
        };
    }

    match what.as_str() {
        // X3DBaseNode.getExecutionContext of an execution context: the context it was put in (`scene .setExecutionContext (…)` of
        // createX3DFromString and Inline, a PROTO instance's body); none for a world, the page's first world and createScene's scene.
        "parentCtx" => {
            let r = rt();
            let own = c == r.scene.world
                || r.ctx_events.no_forward.contains(&c)
                || r.first_world() == Some(c);

            match r.scene.contexts[c].parent.filter(|_| !own) {
                Some(p) => Ok(J::Number(p as f64)),
                None => Ok(J::Null),
            }
        }
        // X3DBaseNode.isLive of an execution context.
        "live" => Ok(J::Bool(rt().ctx_live(c))),
        // X3DExecutionContext.getLocalScene: the scene the context belongs to.
        "sceneRoot" => Ok(J::Number(rt().scene.scene_root(c) as f64)),
        // X3DExecutionContext.getOuterNode: the PROTO instance whose body the context is; null for a scene.
        "outerNode" => match rt().scene.contexts[c].outer {
            x3d_scene::Outer::Instance(n) => Ok(node_js(Some(n))),
            _ => Ok(J::Null),
        },
        "unique" => {
            let name = str_at(1);
            let r = &rt().scene;
            let v = match str_at(0).as_str() {
                "named" => r.get_unique_name(c, &name),
                "imported" => r.get_unique_import_name(c, &name),
                "proto" => r.get_unique_proto_name(c, &name),
                "extern" => r.get_unique_extern_proto_name(c, &name),
                _ => {
                    let root = r.scene_root(c);
                    r.get_unique_export_name_of(root, &name)
                }
            };
            string!(v)
        }
        // --- named nodes: X3DExecutionContext.addNamedNode / updateNamedNode / removeNamedNode
        "named" => {
            let (mode, name) = (str_at(0), str_at(1));

            if mode == "remove" {
                rt().scene.remove_named_node(c, &name);
                dirty();
                return Ok(J::Undefined);
            }

            let verb = if mode == "add" { "add" } else { "update" };
            let Some(node) = node_arg(a, 2) else {
                return throw(
                    ctx,
                    &format!("Couldn't {verb} named node: node must be of type X3DNode."),
                );
            };

            if rt().scene.is_imported(node) {
                return throw(
                    ctx,
                    "Couldn't add named node: node cannot be of type X3DImportedNodeInstance.",
                );
            }

            if rt().scene.node(node).ctx != c {
                return throw(
                    ctx,
                    &format!(
                        "Couldn't {verb} named node: node does not belong to this execution context."
                    ),
                );
            }

            if name.is_empty() {
                return throw(
                    ctx,
                    &format!("Couldn't {verb} named node: node name is empty."),
                );
            }

            if mode == "add" {
                if rt().scene.get_named_node(c, &name).is_some() {
                    return throw(
                        ctx,
                        &format!("Couldn't add named node: node named '{name}' is already in use."),
                    );
                }

                let own = rt().scene.node(node).name.clone();

                if rt().scene.get_named_node(c, &own) == Some(node) {
                    return throw(
                        ctx,
                        &format!("Couldn't add named node: node named '{own}' already has a name."),
                    );
                }

                // node .setName (name), then the array takes it.
                rt().scene.node_mut(node).name = name.clone();
                let _ = rt().scene.ctx_mut(c).named_nodes.add(&name, node);
            } else {
                rt().scene.update_named_node(c, &name, node);
            }

            dirty();
            Ok(J::Undefined)
        }
        // --- imported nodes
        "importedGet" => {
            let name = str_at(0);

            match rt().scene.ctx(c).imported.get(&name) {
                Some(id) => Ok(node_js(Some(rt().scene.imported[id].instance))),
                None => throw(ctx, &format!("Imported node '{name}' not found.")),
            }
        }
        "localNode" => {
            let name = str_at(0);

            match rt().scene.get_local_node(c, &name) {
                Some(n) => Ok(node_js(Some(n))),
                None => throw(ctx, &format!("Unknown named or imported node '{name}'.")),
            }
        }
        "nodeOrImported" => match node_arg(a, 0) {
            Some(n) => Ok(node_js(Some(node_or_imported(c, n)))),
            None => throw(
                ctx,
                "Couldn't get localized node: node must be of type X3DNode.",
            ),
        },
        "importedAdd" | "importedUpdate" => {
            let (exported, imported_arg) = (
                str_at(1),
                a.get(2)
                    .filter(|v| !v.is_undefined())
                    .map(|v| coerce_string(ctx, v)),
            );
            let imported = imported_arg.unwrap_or_else(|| exported.clone());
            let description = a.get(3).map(|v| coerce_string(ctx, v)).unwrap_or_default();

            if what == "importedAdd" && rt().scene.ctx(c).imported.has(&imported) {
                return throw(
                    ctx,
                    &format!(
                        "Couldn't add imported node: imported name '{imported}' already in use."
                    ),
                );
            }

            let inline = node_arg(a, 0).filter(|&n| {
                matches!(rt().scene.node(n).kind, NodeKind::Builtin(_))
                    && rt().scene.type_name(n) == "Inline"
            });

            let Some(inline) = inline else {
                return throw(ctx, "Node must be of type Inline node.");
            };

            if rt().scene.node(inline).ctx != c {
                return throw(
                    ctx,
                    "Couldn't update imported node: Inline node does not belong to this execution context.",
                );
            }

            if exported.is_empty() {
                return throw(
                    ctx,
                    "Couldn't update imported node: exported name is empty.",
                );
            }

            if imported.is_empty() {
                return throw(
                    ctx,
                    "Couldn't update imported node: imported name is empty.",
                );
            }

            remove_imported(c, &imported);

            match rt()
                .scene
                .add_imported_node(c, inline, &exported, &imported, &description)
            {
                Ok(id) => {
                    let instance = rt().scene.imported[id].instance;

                    rt().nrt_mut(instance);
                    sg(c);
                    Ok(J::Undefined)
                }
                Err(e) => throw(ctx, &e),
            }
        }
        "importedRename" => {
            let (old, new) = (str_at(0), str_at(1));

            match rt().scene.rename_imported_node(c, &old, &new) {
                Ok(()) => {
                    sg(c);
                    Ok(J::Undefined)
                }
                Err(e) => throw(ctx, &e),
            }
        }
        "importedRemove" => {
            let name = str_at(0);

            if remove_imported(c, &name) {
                sg(c);
            }

            Ok(J::Undefined)
        }
        "rootSet" => {
            // X3DScene.setRootNodes (value): rootNodes .assign (value)
            if let Some(FieldValue::MFNode(list)) = value_arg(a, 0) {
                rt().set_root_nodes(c, list);
            }

            Ok(J::Undefined)
        }
        "importedExported" => {
            let id = num_arg(ctx, a, 0) as usize;

            match exported_of(id) {
                Some(n) => Ok(node_js(Some(n))),
                None => throw(
                    ctx,
                    &format!(
                        "Exported node '{}' not found.",
                        rt().scene.imported[id].exported_name
                    ),
                ),
            }
        }
        // X3DExportedNode.setDescription.
        "exportedDescription" => {
            rt().scene.exported[num_arg(ctx, a, 0) as usize].description = str_at(1);
            Ok(J::Undefined)
        }
        "importedDescription" => {
            rt().scene.imported[num_arg(ctx, a, 0) as usize].description = str_at(1);
            Ok(J::Undefined)
        }
        // --- exported nodes
        "exportedUpdate" => {
            let (name, description) = (
                str_at(0),
                a.get(2).map(|v| coerce_string(ctx, v)).unwrap_or_default(),
            );
            let root = rt().scene.scene_root(c);

            if str_at(3) == "add" && rt().scene.ctx(root).exported.has(&name) {
                return throw(
                    ctx,
                    &format!("Couldn't add exported node: exported name '{name}' already in use."),
                );
            }

            let Some(node) = node_arg(a, 1) else {
                return throw(
                    ctx,
                    "Couldn't update exported node: node must be of type X3DNode.",
                );
            };

            match rt()
                .scene
                .update_exported_node(c, &name, node, &description)
            {
                Ok(()) => {
                    sg(root);
                    Ok(J::Undefined)
                }
                Err(e) => throw(ctx, &e),
            }
        }
        "exportedRemove" => {
            let name = str_at(0);
            let root = rt().scene.scene_root(c);

            if rt().scene.ctx(root).exported.has(&name) {
                rt().scene.ctx_mut(root).exported.remove(&name);
                rt().dynamic_dirty_mark();
                sg(root);
            }

            Ok(J::Undefined)
        }
        // --- routes (identified by their id)
        "routeAdd" | "routeGet" => {
            let (sf, df) = (str_at(1), str_at(3));
            let adding = what == "routeAdd";
            let who = if adding { "source node" } else { "sourceNode" };
            let (Some(src), Some(dst)) = (node_arg(a, 0), node_arg(a, 2)) else {
                let which = if node_arg(a, 0).is_none() {
                    who
                } else if adding {
                    "destination node"
                } else {
                    "destinationNode"
                };
                return throw(
                    ctx,
                    &format!("Bad ROUTE specification: {which} must be of type X3DNode."),
                );
            };
            let (src, dst) = (node_or_imported(c, src), node_or_imported(c, dst));

            if let Err(e) = rt().check_imported_route_ends(src, &sf, dst, &df) {
                return throw(ctx, &e);
            }

            if adding {
                if let Err(e) = rt().add_route(c, src, &sf, dst, &df) {
                    return throw(ctx, &e);
                }
            } else if let Err(e) = rt().scene.route_ends(src, &sf, dst, &df) {
                return throw(ctx, &e);
            }

            match rt().scene.find_route_index(c, src, &sf, dst, &df) {
                Some(i) => Ok(J::Number(rt().scene.ctx(c).routes[i].id as f64)),
                None => Ok(J::Null),
            }
        }
        "routeDelete" => {
            let id = num_arg(ctx, a, 0) as usize;
            Ok(J::Bool(rt().delete_route_by_id(c, id)))
        }
        "routeList" => {
            let routes: Vec<_> = rt()
                .scene
                .ctx(c)
                .routes
                .iter()
                .map(|r| {
                    (
                        r.id,
                        r.source_node,
                        r.source_field.clone(),
                        r.destination_node,
                        r.destination_field.clone(),
                    )
                })
                .collect();
            let arr = Arr::new();

            for (i, (id, sn, sf, dn, df)) in routes.into_iter().enumerate() {
                let o = Obj::new();
                o.set("id", id as f64);
                o.set("sourceNode", node_js(Some(sn)));
                o.set("sourceField", sf);
                o.set("destinationNode", node_js(Some(dn)));
                o.set("destinationField", df);
                arr.set(i, o);
            }

            Ok(arr.into())
        }
        // --- proto and extern proto declarations
        "protoIds" => {
            let arr = Arr::new();

            for (i, id) in rt()
                .decl_ids(c, a.first().is_some_and(coerce_bool))
                .into_iter()
                .enumerate()
            {
                arr.set(i, id as f64);
            }

            Ok(arr.into())
        }
        "protoGet" => {
            let (extern_, name) = (a.first().is_some_and(coerce_bool), str_at(1));

            match rt().decl_get(c, extern_, &name) {
                Ok(id) => Ok(J::Number(id as f64)),
                Err(e) => throw(ctx, &e),
            }
        }
        "protoOp" => {
            // (extern, mode add|update|remove, name, id)
            let (extern_, mode, name) = (a.first().is_some_and(coerce_bool), str_at(1), str_at(2));
            let id = a
                .get(3)
                .map(|v| coerce_f64(ctx, v))
                .filter(|n| !n.is_nan())
                .map(|n| n as usize);
            let op = match mode.as_str() {
                "add" => x3d_runtime::DeclOp::Add,
                "remove" => x3d_runtime::DeclOp::Remove,
                _ => x3d_runtime::DeclOp::Update,
            };

            match rt().decl_op(c, extern_, op, &name, id) {
                Ok(()) => {
                    flush_warnings();
                    Ok(J::Undefined)
                }
                Err(e) => throw(ctx, &e),
            }
        }
        // X3DProtoDeclarationNode.createInstance: (extern, id) in the context `c`.
        "protoNew" => {
            let (extern_, id) = (
                a.first().is_some_and(coerce_bool),
                num_arg(ctx, a, 1) as usize,
            );
            let proto = if extern_ {
                x3d_scene::ProtoRef::Extern(id)
            } else {
                x3d_scene::ProtoRef::Proto(id)
            };
            let n = rt().create_instance_setup(c, proto);

            Ok(node_js(Some(n)))
        }
        // X3DProtoDeclarationNode.requestUpdateInstances: (extern, id), at the next event cascade.
        "protoRequest" => {
            let (extern_, id) = (
                a.first().is_some_and(coerce_bool),
                num_arg(ctx, a, 1) as usize,
            );

            if !rt().pending_proto_updates.contains(&(extern_, id)) {
                rt().pending_proto_updates.push((extern_, id));
            }

            Ok(J::Undefined)
        }
        // X3DBaseNode.addUserDefinedField / removeUserDefinedField on a PROTO declaration:
        // (id, "add", name, accessType, value) or (id, "remove", name).
        "protoField" => {
            let id = num_arg(ctx, a, 0) as usize;
            let name = str_at(2);

            if str_at(1) == "remove" {
                rt().proto_remove_user_field(id, &name);
            } else if let Some(value) = value_arg(a, 4) {
                let access = num_arg(ctx, a, 3) as u8;

                rt().proto_add_user_field(id, x3d_scene::Field::new(&name, access, value));
            }

            Ok(J::Undefined)
        }
        // X3DProtoDeclarationNode.updateInstances.
        "protoUpdate" => {
            let (extern_, id) = (
                a.first().is_some_and(coerce_bool),
                num_arg(ctx, a, 1) as usize,
            );

            rt().proto_update_instances(extern_, id);
            Ok(J::Undefined)
        }
        // X3DProtoDeclaration.getBody: the context of the body.
        "protoBody" => Ok(J::Number(
            rt().scene.protos[num_arg(ctx, a, 0) as usize].body as f64,
        )),
        // `declaration .toVRMLString ()` … (Scene::proto_declaration_string).
        "protoString" => {
            let (extern_, id) = (
                a.first().is_some_and(coerce_bool),
                num_arg(ctx, a, 1) as usize,
            );
            let format = str_arg(ctx, a, 2);
            let text = rt().scene.proto_declaration_string(extern_, id, &format);

            Ok(J::from(&text))
        }
        "protoInfo" => {
            let (extern_, id) = (
                a.first().is_some_and(coerce_bool),
                num_arg(ctx, a, 1) as usize,
            );
            let info = rt().decl_info(extern_, id);
            let o = Obj::new();

            o.set("name", info.name.as_str());
            o.set("ctx", info.ctx as f64);
            o.set("isExternProto", info.is_extern);
            o.set("hasMetadata", info.has_metadata);

            // X3DExternProtoDeclaration.loadState (X3DUrlObject.checkLoadState).
            if let Some(state) = info.load_state {
                let name = match state {
                    x3d_runtime::nodes::LoadState::NotStarted => "NOT_STARTED_STATE",
                    x3d_runtime::nodes::LoadState::InProgress => "IN_PROGRESS_STATE",
                    x3d_runtime::nodes::LoadState::Complete => "COMPLETE_STATE",
                    x3d_runtime::nodes::LoadState::Failed => "FAILED_STATE",
                };
                o.set("loadState", name);
            }

            let fa = Arr::new();

            for (j, f) in info.fields.into_iter().enumerate() {
                let fo = Obj::new();
                fo.set("name", f.name.as_str());
                fo.set("accessType", f.access as i32);
                fo.set("dataType", f.type_name);
                fo.set("value", new_value_js(f.value));
                fa.set(j, fo);
            }

            o.set("fields", fa);

            if let Some(urls) = info.urls {
                o.set("urls", new_value_js(FieldValue::MFString(urls)));
            }

            if let Some(p) = info.loaded {
                o.set("loaded", p as f64);
            }

            Ok(o.into())
        }
        other => throw(ctx, &format!("x3d-script: unknown context op '{other}'")),
    }
}
